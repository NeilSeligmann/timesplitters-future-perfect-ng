// SPDX-License-Identifier: GPL-3.0-or-later
//! Device-specific controller axis settings exposed to native settings pages.
use crate::mk_settings::{RowGesture, RowId, RowValue, SettingsRow};
use tsfp_input::controller::{
    ConnectedController, ControllerIdentity, ControllerProfile, ControllerProfiles, AXIS_COUNT,
    BUTTON_COUNT,
};

const AXIS_NAMES: [&str; AXIS_COUNT] = [
    "Left X",
    "Left Y",
    "Right X",
    "Right Y",
    "Left trigger",
    "Right trigger",
];
const BUTTON_NAMES: [&str; 14] = [
    "A",
    "B",
    "X",
    "Y",
    "LB",
    "RB",
    "Start",
    "Back",
    "L3",
    "R3",
    "D-pad up",
    "D-pad down",
    "D-pad left",
    "D-pad right",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerAxisRow {
    pub identity: ControllerIdentity,
    pub device_name: String,
    pub target_axis: u8,
    pub label: String,
    pub source_axis: u8,
    pub choices: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerProfileEdit {
    pub identity: ControllerIdentity,
    pub profile: ControllerProfile,
}

#[derive(Default)]
pub struct LiveSettings {
    devices: Vec<ConnectedController>,
    profiles: ControllerProfiles,
    pending: Option<ControllerProfileEdit>,
}
impl LiveSettings {
    pub fn update(&mut self, devices: Vec<ConnectedController>, profiles: ControllerProfiles) {
        self.devices = devices;
        self.profiles = profiles;
    }
    pub fn take_edit(&mut self) -> Option<ControllerProfileEdit> {
        self.pending.take()
    }
    pub fn rows(&self) -> Vec<SettingsRow> {
        let mut rows = Vec::new();
        let labels = device_labels(&self.devices);
        for (device, device_label) in self.devices.iter().zip(labels) {
            let profile = self.profiles.profile(&device.identity);
            let button_choices: Vec<String> = (0..BUTTON_COUNT)
                .map(|source| format!("Button {source}"))
                .chain(std::iter::once("Disabled".into()))
                .collect();
            for (target, name) in BUTTON_NAMES.iter().enumerate() {
                rows.push(SettingsRow {
                    id: RowId::ControllerButton(device.identity.profile_key(), target as u8),
                    label: format!("{device_label}: {name} button"),
                    value: RowValue::Choice {
                        index: profile.button_map[target].map_or(BUTTON_COUNT, usize::from),
                        options: button_choices.clone(),
                    },
                });
            }
            rows.extend(axis_rows(device, &self.profiles).into_iter().map(|axis| {
                let index = if axis.target_axis < 4 {
                    axis.source_axis as usize
                } else {
                    axis.source_axis.saturating_sub(4) as usize
                };
                SettingsRow {
                    id: RowId::ControllerAxis(axis.identity.profile_key(), axis.target_axis),
                    label: format!("{device_label}: {}", axis.label),
                    value: RowValue::Choice {
                        index,
                        options: axis.choices,
                    },
                }
            }));
            for (source, calibration) in profile.calibration.iter().enumerate() {
                rows.push(SettingsRow {
                    id: RowId::ControllerCalibrationDeadzone(
                        device.identity.profile_key(),
                        source as u8,
                    ),
                    label: format!("{device_label}: Axis {source} deadzone"),
                    value: RowValue::Slider {
                        value: f64::from(calibration.deadzone),
                        min: 0.0,
                        max: 32766.0,
                        step: 256.0,
                    },
                });
                rows.push(SettingsRow {
                    id: RowId::ControllerCalibrationInvert(
                        device.identity.profile_key(),
                        source as u8,
                    ),
                    label: format!("{device_label}: Axis {source} invert"),
                    value: RowValue::Toggle(calibration.invert),
                });
            }
        }
        rows
    }
    pub fn edit(&mut self, id: &RowId, gesture: RowGesture) -> Result<bool, String> {
        let (profile_key, target, field) = match id {
            RowId::ControllerButton(key, target) => (key, *target, 0),
            RowId::ControllerAxis(key, target) => (key, *target, 1),
            RowId::ControllerCalibrationDeadzone(key, target) => (key, *target, 2),
            RowId::ControllerCalibrationInvert(key, target) => (key, *target, 3),
            _ => return Ok(false),
        };
        let Some(device) = self
            .devices
            .iter()
            .find(|device| device.identity.profile_key() == *profile_key)
        else {
            return Ok(false);
        };
        let current = self.profiles.profile(&device.identity);
        let updated = if field == 0 {
            let Some(slot) = current.button_map.get(usize::from(target)) else {
                return Err("controller button target is out of range".into());
            };
            let at = slot.map_or(BUTTON_COUNT, usize::from) as i32;
            let next = match gesture {
                RowGesture::Step(step) => (at + step).rem_euclid((BUTTON_COUNT + 1) as i32),
                RowGesture::Activate => (at + 1).rem_euclid((BUTTON_COUNT + 1) as i32),
                RowGesture::Bind(_) => return Ok(false),
            };
            let source = (next < BUTTON_COUNT as i32).then_some(next as u8);
            edit_button(&device.identity, &current, target, source)?
        } else if field == 1 {
            let source_axis = match gesture {
                RowGesture::Step(step) => {
                    let choices: Vec<u8> = if target < 4 {
                        (0..4).collect()
                    } else {
                        (4..6).collect()
                    };
                    let at = choices
                        .iter()
                        .position(|axis| *axis == current.axis_map[target as usize])
                        .unwrap_or(0) as i32;
                    choices[(at + step).rem_euclid(choices.len() as i32) as usize]
                }
                RowGesture::Activate => current.axis_map[target as usize],
                RowGesture::Bind(_) => return Ok(false),
            };
            edit_axis(&device.identity, &current, target, source_axis)?
        } else {
            let Some(calibration) = current.calibration.get(usize::from(target)).copied() else {
                return Err("controller calibration axis is out of range".into());
            };
            let mut profile = current.clone();
            if field == 2 {
                let next = match gesture {
                    RowGesture::Step(step) => (i64::from(calibration.deadzone)
                        + i64::from(step) * 256)
                        .clamp(0, 32766) as u32,
                    RowGesture::Activate => calibration.deadzone.saturating_add(256).min(32766),
                    RowGesture::Bind(_) => return Ok(false),
                };
                profile.calibration[usize::from(target)].deadzone = next;
            } else {
                match gesture {
                    RowGesture::Step(_) | RowGesture::Activate => {
                        profile.calibration[usize::from(target)].invert = !calibration.invert;
                    }
                    RowGesture::Bind(_) => return Ok(false),
                }
            }
            profile.validate()?;
            ControllerProfileEdit {
                identity: device.identity.clone(),
                profile,
            }
        };
        self.profiles
            .set(updated.identity.clone(), updated.profile.clone())
            .map_err(|error| error.to_string())?;
        self.pending = Some(updated);
        Ok(true)
    }
}

fn device_labels(devices: &[ConnectedController]) -> Vec<String> {
    let mut totals = std::collections::BTreeMap::<String, usize>::new();
    for device in devices {
        *totals.entry(device.name.clone()).or_default() += 1;
    }
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    devices
        .iter()
        .map(|device| {
            let total = totals[&device.name];
            if total == 1 {
                return device.name.clone();
            }
            let index = seen.entry(device.name.clone()).or_default();
            *index += 1;
            format!("{} ({}/{total})", device.name, *index)
        })
        .collect()
}

/// Return a validated per-device profile with one physical button source changed.
pub fn edit_button(
    identity: &ControllerIdentity,
    current: &ControllerProfile,
    target: u8,
    source: Option<u8>,
) -> Result<ControllerProfileEdit, String> {
    let target = usize::from(target);
    if target >= BUTTON_NAMES.len()
        || source.is_some_and(|source| usize::from(source) >= BUTTON_COUNT)
    {
        return Err("controller button target/source is out of range".into());
    }
    let mut profile = current.clone();
    profile.button_map[target] = source;
    profile.validate()?;
    Ok(ControllerProfileEdit {
        identity: identity.clone(),
        profile,
    })
}

/// Rows for one connected physical device. Stick axes and triggers remain separate choice groups.
pub fn axis_rows(
    device: &ConnectedController,
    profiles: &ControllerProfiles,
) -> Vec<ControllerAxisRow> {
    let profile = profiles.profile(&device.identity);
    (0..AXIS_COUNT)
        .map(|target| {
            let choices: Vec<usize> = if target < 4 {
                (0..4).collect()
            } else {
                (4..6).collect()
            };
            ControllerAxisRow {
                identity: device.identity.clone(),
                device_name: device.name.clone(),
                target_axis: target as u8,
                label: format!("{} input", AXIS_NAMES[target]),
                source_axis: profile.axis_map[target],
                choices: choices
                    .into_iter()
                    .map(|source| AXIS_NAMES[source].to_owned())
                    .collect(),
            }
        })
        .collect()
}

/// Return a validated per-device profile with one axis source changed.
pub fn edit_axis(
    identity: &ControllerIdentity,
    current: &ControllerProfile,
    target_axis: u8,
    source_axis: u8,
) -> Result<ControllerProfileEdit, String> {
    let target = usize::from(target_axis);
    let source = usize::from(source_axis);
    if target >= AXIS_COUNT || source >= AXIS_COUNT {
        return Err("controller axis must be 0..5".into());
    }
    let mut updated = current.clone();
    updated.axis_map[target] = source_axis;
    updated.validate()?;
    Ok(ControllerProfileEdit {
        identity: identity.clone(),
        profile: updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_input::controller::ControllerInventory;

    #[test]
    fn connected_device_rows_expose_independent_stick_and_trigger_mappings() {
        let mut inventory = ControllerInventory::default();
        let device = inventory
            .connect(
                42,
                "0123456789abcdef0123456789abcdef",
                Some("serial-a"),
                None,
                "Pad A",
            )
            .unwrap()
            .clone();
        let profiles = ControllerProfiles::default();
        let rows = axis_rows(&device, &profiles);
        assert_eq!(rows.len(), AXIS_COUNT);
        assert_eq!(rows[0].source_axis, 0);
        assert_eq!(rows[0].choices, ["Left X", "Left Y", "Right X", "Right Y"]);
        assert_eq!(rows[4].choices, ["Left trigger", "Right trigger"]);
        assert_eq!(rows[0].identity, device.identity);
        assert_eq!(rows[0].device_name, "Pad A");
    }

    #[test]
    fn live_settings_expose_editable_physical_button_rows_and_emit_profile_edits() {
        let mut inventory = ControllerInventory::default();
        let device = inventory
            .connect(
                42,
                "0123456789abcdef0123456789abcdef",
                Some("serial-a"),
                None,
                "Pad A",
            )
            .unwrap()
            .clone();
        let mut settings = LiveSettings::default();
        settings.update(vec![device.clone()], ControllerProfiles::default());
        let rows = settings.rows();
        assert_eq!(rows.len(), 14 + AXIS_COUNT + AXIS_COUNT * 2);
        assert_eq!(
            rows[0].id,
            RowId::ControllerButton(device.identity.profile_key(), 0)
        );
        assert_eq!(rows[0].label, "Pad A: A button");
        assert!(settings.edit(&rows[0].id, RowGesture::Step(1)).unwrap());
        let edit = settings.take_edit().unwrap();
        assert_eq!(edit.identity, device.identity);
        assert_eq!(edit.profile.button_map[0], Some(1));
    }

    #[test]
    fn duplicate_names_are_distinguished_across_per_device_profile_rows() {
        let mut inventory = ControllerInventory::default();
        let first = inventory
            .connect(
                10,
                "0123456789abcdef0123456789abcdef",
                None,
                None,
                "Twin Pad",
            )
            .unwrap()
            .clone();
        let second = inventory
            .connect(
                20,
                "0123456789abcdef0123456789abcdef",
                None,
                None,
                "Twin Pad",
            )
            .unwrap()
            .clone();
        let mut settings = LiveSettings::default();
        settings.update(vec![first.clone(), second.clone()], ControllerProfiles::default());

        let rows = settings.rows();
        let first_row = rows
            .iter()
            .find(|row| row.id == RowId::ControllerButton(first.identity.profile_key(), 0))
            .unwrap();
        let second_row = rows
            .iter()
            .find(|row| row.id == RowId::ControllerButton(second.identity.profile_key(), 0))
            .unwrap();
        assert_eq!(first_row.label, "Twin Pad (1/2): A button");
        assert_eq!(second_row.label, "Twin Pad (2/2): A button");
    }

    #[test]
    fn live_settings_edit_per_device_axis_deadzone_and_inversion() {
        let mut inventory = ControllerInventory::default();
        let device = inventory
            .connect(
                42,
                "0123456789abcdef0123456789abcdef",
                Some("serial-a"),
                None,
                "Pad A",
            )
            .unwrap()
            .clone();
        let key = device.identity.profile_key();
        let mut settings = LiveSettings::default();
        settings.update(vec![device.clone()], ControllerProfiles::default());
        let rows = settings.rows();
        let deadzone = rows
            .iter()
            .find(|row| row.id == RowId::ControllerCalibrationDeadzone(key.clone(), 0))
            .unwrap();
        assert!(settings.edit(&deadzone.id, RowGesture::Step(2)).unwrap());
        let edit = settings.take_edit().unwrap();
        assert_eq!(edit.identity, device.identity);
        assert_eq!(edit.profile.calibration[0].deadzone, 512);

        let invert = rows
            .iter()
            .find(|row| row.id == RowId::ControllerCalibrationInvert(key.clone(), 1))
            .unwrap();
        assert!(settings.edit(&invert.id, RowGesture::Activate).unwrap());
        let edit = settings.take_edit().unwrap();
        assert!(!edit.profile.calibration[1].invert);
    }

    #[test]
    fn button_edit_rejects_guide_and_out_of_range_sources_without_mutation() {
        let identity = ControllerIdentity::from_sdl(
            "0123456789abcdef0123456789abcdef",
            Some("serial-a"),
            None,
            0,
        )
        .unwrap();
        let profile = ControllerProfile::default();
        assert!(edit_button(&identity, &profile, 14, Some(0)).is_err());
        assert!(edit_button(&identity, &profile, 0, Some(BUTTON_COUNT as u8)).is_err());
        assert_eq!(profile.button_map[0], Some(0));
    }

    #[test]
    fn axis_edit_accepts_within_class_mapping_and_rejects_cross_class_mapping() {
        let identity = ControllerIdentity::from_sdl(
            "0123456789abcdef0123456789abcdef",
            Some("serial-a"),
            None,
            0,
        )
        .unwrap();
        let profile = ControllerProfile::default();
        let updated = edit_axis(&identity, &profile, 0, 2).unwrap();
        assert_eq!(updated.profile.axis_map[0], 2);
        assert_eq!(updated.identity, identity);
        assert_eq!(profile.axis_map[0], 0);
        assert!(edit_axis(&identity, &profile, 0, 4).is_err());
        assert!(edit_axis(&identity, &profile, 9, 0).is_err());
    }
}
