// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-device controller profile editor model for the Rust settings UI.
//! Rendering and live SDL device enumeration are supplied by the caller.

use tsfp_input::controller::{
    AXIS_COUNT, AxisCalibration, BUTTON_COUNT, ControllerIdentity, ControllerProfile,
    ControllerProfiles,
};

const BUTTON_TARGETS: [&str; 15] = [
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
    "Guide",
];
const AXIS_TARGETS: [&str; AXIS_COUNT] = ["Left X", "Left Y", "Right X", "Right Y", "LT", "RT"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowId {
    ButtonTarget(usize),
    AxisTarget(usize),
    Calibration(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowValue {
    Choice { index: usize, options: Vec<String> },
    Calibration(AxisCalibration),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: RowId,
    pub label: String,
    pub value: RowValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    ButtonSource {
        target: usize,
        source: Option<u8>,
    },
    AxisSource {
        target: usize,
        source: u8,
    },
    Calibration {
        source: usize,
        value: AxisCalibration,
    },
}

/// A draft for one stable device identity. Runtime SDL ids never enter profile settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerProfileEditor {
    pub identity: ControllerIdentity,
    profile: ControllerProfile,
}

impl ControllerProfileEditor {
    pub fn new(identity: ControllerIdentity, profiles: &ControllerProfiles) -> Self {
        Self {
            profile: profiles.profile(&identity),
            identity,
        }
    }

    pub fn profile(&self) -> &ControllerProfile {
        &self.profile
    }

    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::with_capacity(15 + AXIS_COUNT * 2);
        let button_options: Vec<String> = (0..BUTTON_COUNT)
            .map(|index| format!("Button {index}"))
            .chain(std::iter::once("Disabled".into()))
            .collect();
        for (target, label) in BUTTON_TARGETS.iter().enumerate() {
            rows.push(Row {
                id: RowId::ButtonTarget(target),
                label: format!("{label} button"),
                value: RowValue::Choice {
                    index: self.profile.button_map[target].map_or(BUTTON_COUNT, usize::from),
                    options: button_options.clone(),
                },
            });
        }
        for (target, label) in AXIS_TARGETS.iter().enumerate() {
            let first = if target < 4 { 0 } else { 4 };
            let last = if target < 4 { 4 } else { AXIS_COUNT };
            rows.push(Row {
                id: RowId::AxisTarget(target),
                label: format!("{label} axis"),
                value: RowValue::Choice {
                    index: usize::from(self.profile.axis_map[target] - first as u8),
                    options: (first..last).map(|index| format!("Axis {index}")).collect(),
                },
            });
        }
        for source in 0..AXIS_COUNT {
            rows.push(Row {
                id: RowId::Calibration(source),
                label: format!("Axis {source} calibration"),
                value: RowValue::Calibration(self.profile.calibration[source]),
            });
        }
        rows
    }

    /// Apply one edit atomically. Invalid mappings or calibration leave the draft unchanged.
    pub fn apply(&mut self, edit: Edit) -> Result<(), String> {
        let mut candidate = self.profile.clone();
        match edit {
            Edit::ButtonSource { target, source } => {
                let slot = candidate
                    .button_map
                    .get_mut(target)
                    .ok_or_else(|| format!("button target {target} is out of range"))?;
                *slot = source;
            }
            Edit::AxisSource { target, source } => {
                let slot = candidate
                    .axis_map
                    .get_mut(target)
                    .ok_or_else(|| format!("axis target {target} is out of range"))?;
                *slot = source;
            }
            Edit::Calibration { source, value } => {
                let slot = candidate
                    .calibration
                    .get_mut(source)
                    .ok_or_else(|| format!("calibration axis {source} is out of range"))?;
                *slot = value;
            }
        }
        candidate.validate()?;
        self.profile = candidate;
        Ok(())
    }

    pub fn save(&self, profiles: &mut ControllerProfiles) -> Result<(), String> {
        profiles.set(self.identity.clone(), self.profile.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> ControllerIdentity {
        ControllerIdentity::from_sdl(
            "0123456789abcdef0123456789abcdef",
            Some("controller-serial"),
            None,
            0,
        )
        .unwrap()
    }

    #[test]
    fn editor_exposes_per_device_button_axis_and_calibration_rows() {
        let editor = ControllerProfileEditor::new(identity(), &ControllerProfiles::default());
        let rows = editor.rows();
        assert_eq!(rows.len(), 15 + AXIS_COUNT * 2);
        assert_eq!(rows[0].id, RowId::ButtonTarget(0));
        assert_eq!(rows[15].id, RowId::AxisTarget(0));
        assert_eq!(rows[21].id, RowId::Calibration(0));
        assert_eq!(
            rows[14].value,
            RowValue::Choice {
                index: BUTTON_COUNT,
                options: (0..BUTTON_COUNT)
                    .map(|index| format!("Button {index}"))
                    .chain(std::iter::once("Disabled".into()))
                    .collect(),
            }
        );
    }

    #[test]
    fn mapping_edits_validate_and_save_under_stable_device_identity() {
        let identity = identity();
        let mut editor =
            ControllerProfileEditor::new(identity.clone(), &ControllerProfiles::default());
        editor
            .apply(Edit::ButtonSource {
                target: 4,
                source: Some(8),
            })
            .unwrap();
        editor
            .apply(Edit::AxisSource {
                target: 0,
                source: 1,
            })
            .unwrap();
        let before = editor.profile().clone();
        assert!(
            editor
                .apply(Edit::AxisSource {
                    target: 0,
                    source: 4
                })
                .is_err()
        );
        assert_eq!(editor.profile(), &before);
        assert!(
            editor
                .apply(Edit::ButtonSource {
                    target: 14,
                    source: Some(5)
                })
                .is_err()
        );
        assert_eq!(editor.profile(), &before);

        let mut profiles = ControllerProfiles::default();
        editor.save(&mut profiles).unwrap();
        assert_eq!(profiles.profile(&identity), before);
        let restored = ControllerProfiles::parse(&profiles.to_text()).unwrap();
        assert_eq!(restored.profile(&identity), before);
    }

    #[test]
    fn calibration_edit_rejects_invalid_ranges_without_mutating_profile() {
        let mut editor = ControllerProfileEditor::new(identity(), &ControllerProfiles::default());
        let before = editor.profile().clone();
        let mut invalid = AxisCalibration {
            min: -100,
            center: 0,
            max: 100,
            deadzone: 0,
            invert: false,
        };
        assert!(
            editor
                .apply(Edit::Calibration {
                    source: 4,
                    value: invalid
                })
                .is_err()
        );
        assert_eq!(editor.profile(), &before);
        invalid.min = -32768;
        editor
            .apply(Edit::Calibration {
                source: 0,
                value: invalid,
            })
            .unwrap();
        assert_eq!(editor.profile().calibration[0], invalid);
    }
}
