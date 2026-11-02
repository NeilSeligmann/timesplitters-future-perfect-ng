// SPDX-License-Identifier: GPL-3.0-or-later
//! T2163 (MK-5) opt-in mouse and keyboard settings rows: pure data a menu page or the launcher renders,
//! and the inverse mapping from a row gesture to a [`SettingEdit`]. Absent in the `original` preset; the
//! in-game Controls page attachment waits for T2190 (extension mechanism) and T2196 (typed settings model).
//! Labels are host strings, not game text ids. Contract: docs/t2163-mk-settings-model.md.
use tsfp_input::bindings::Action;
use tsfp_input::settings::{
    action_name, Device, InputKind, InputSettings, SettingEdit, PLAYER_COUNT, SENSITIVITY_MAX,
    SENSITIVITY_MIN, SENSITIVITY_STEP,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RowId {
    EditingPlayer,
    FixedDevices,
    ResetPlayer,
    SensitivityYaw,
    SensitivityPitch,
    InvertPitch,
    PlayerDevice(usize),
    ControllerButton(String, u8),
    ControllerAxis(String, u8),
    ControllerCalibrationDeadzone(String, u8),
    ControllerCalibrationInvert(String, u8),
    Binding(InputKindKey, String),
}

/// `InputKind` with an order so rows sort deterministically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputKindKey {
    Key,
    Mouse,
    Controller,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RowValue {
    Button,
    Slider {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    Toggle(bool),
    Choice {
        index: usize,
        options: Vec<String>,
    },
    /// Shows the bound action; activating it starts a capture of the next action choice.
    Binding {
        action: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsRow {
    pub id: RowId,
    pub label: String,
    pub value: RowValue,
}

/// Gesture on a focused row: left and right step, activate toggles or commits a captured binding.
#[derive(Clone, Debug, PartialEq)]
pub enum RowGesture {
    Step(i32),
    Activate,
    Bind(Action),
}

/// Rows in display order: look, devices, then key and mouse bindings sorted by input name.
pub fn rows(settings: &InputSettings) -> Vec<SettingsRow> {
    let s = &settings.bindings.sensitivity;
    let slider = |value| RowValue::Slider {
        value,
        min: SENSITIVITY_MIN,
        max: SENSITIVITY_MAX,
        step: SENSITIVITY_STEP,
    };
    let mut out = vec![
        SettingsRow {
            id: RowId::SensitivityYaw,
            label: "Mouse sensitivity (horizontal)".into(),
            value: slider(s.yaw_deg_per_count),
        },
        SettingsRow {
            id: RowId::SensitivityPitch,
            label: "Mouse sensitivity (vertical)".into(),
            value: slider(s.pitch_deg_per_count),
        },
        SettingsRow {
            id: RowId::InvertPitch,
            label: "Invert mouse Y".into(),
            value: RowValue::Toggle(s.invert_pitch),
        },
    ];
    let options: Vec<String> = Device::choices().iter().map(|d| d.name()).collect();
    for (player, device) in settings.players.iter().enumerate() {
        let index = Device::choices()
            .iter()
            .position(|d| d == device)
            .unwrap_or(0);
        out.push(SettingsRow {
            id: RowId::PlayerDevice(player),
            label: format!("Player {} device", player + 1),
            value: RowValue::Choice {
                index,
                options: options.clone(),
            },
        });
    }
    out.push(SettingsRow {
        id: RowId::FixedDevices,
        label: "Fixed device assignments".into(),
        value: RowValue::Toggle(settings.fixed_devices),
    });
    let mut bindings: Vec<SettingsRow> = [
        (InputKindKey::Key, &settings.bindings.keys),
        (InputKindKey::Mouse, &settings.bindings.mouse),
        (InputKindKey::Controller, &settings.bindings.controller),
    ]
    .into_iter()
    .flat_map(|(kind, map)| {
        map.iter().map(move |(input, action)| SettingsRow {
            id: RowId::Binding(kind, input.clone()),
            label: match kind {
                InputKindKey::Key => format!("Key {input}"),
                InputKindKey::Mouse => format!("Mouse {input}"),
                InputKindKey::Controller => format!("Controller {input}"),
            },
            value: RowValue::Binding {
                action: action_name(*action),
            },
        })
    })
    .collect();
    bindings.sort_by(|a, b| a.id.cmp(&b.id));
    out.extend(bindings);
    out
}

/// The edit a gesture on a row asks for, or None when the gesture does not apply to that row.
pub fn edit_for(settings: &InputSettings, id: &RowId, gesture: &RowGesture) -> Option<SettingEdit> {
    let s = &settings.bindings.sensitivity;
    let stepped = |v: f64, n: i32| v + SENSITIVITY_STEP * f64::from(n);
    match (id, gesture) {
        (RowId::SensitivityYaw, RowGesture::Step(n)) => Some(SettingEdit::SensitivityYaw(stepped(
            s.yaw_deg_per_count,
            *n,
        ))),
        (RowId::SensitivityPitch, RowGesture::Step(n)) => Some(SettingEdit::SensitivityPitch(
            stepped(s.pitch_deg_per_count, *n),
        )),
        (RowId::InvertPitch, RowGesture::Activate | RowGesture::Step(_)) => {
            Some(SettingEdit::InvertPitch(!s.invert_pitch))
        }
        (RowId::FixedDevices, RowGesture::Activate | RowGesture::Step(_)) => {
            Some(SettingEdit::FixedDevices(!settings.fixed_devices))
        }
        (RowId::PlayerDevice(player), RowGesture::Step(n)) if *player < PLAYER_COUNT => {
            let choices = Device::choices();
            let current = choices
                .iter()
                .position(|d| *d == settings.players[*player])
                .unwrap_or(0) as i32;
            let next = (current + n).rem_euclid(choices.len() as i32) as usize;
            Some(SettingEdit::AssignDevice {
                player: *player,
                device: choices[next],
            })
        }
        (RowId::Binding(kind, input), RowGesture::Bind(action)) => Some(SettingEdit::Rebind {
            kind: match kind {
                InputKindKey::Key => InputKind::Key,
                InputKindKey::Mouse => InputKind::Mouse,
                InputKindKey::Controller => InputKind::Controller,
            },
            input: input.clone(),
            action: *action,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(settings: &InputSettings, id: &RowId) -> RowValue {
        rows(settings)
            .into_iter()
            .find(|r| &r.id == id)
            .unwrap()
            .value
    }

    #[test]
    fn rows_cover_look_devices_and_every_binding() {
        let settings = InputSettings::default();
        let all = rows(&settings);
        let bindings = settings.bindings.keys.len()
            + settings.bindings.mouse.len()
            + settings.bindings.controller.len();
        assert!(bindings > 0);
        assert_eq!(all.len(), 4 + PLAYER_COUNT + bindings);
        assert_eq!(all[0].id, RowId::SensitivityYaw);
        assert_eq!(
            value(&settings, &RowId::PlayerDevice(0)),
            RowValue::Choice {
                index: 1,
                options: Device::choices().iter().map(|d| d.name()).collect()
            }
        );
        assert_eq!(
            value(
                &settings,
                &RowId::Binding(InputKindKey::Mouse, "Left".into())
            ),
            RowValue::Binding {
                action: "rt".into()
            }
        );
        let ids: Vec<_> = all.iter().map(|r| r.id.clone()).collect();
        let mut sorted = ids[4 + PLAYER_COUNT..].to_vec();
        sorted.sort();
        assert_eq!(&ids[4 + PLAYER_COUNT..], &sorted[..]);
    }

    #[test]
    fn gestures_edit_and_rows_reflect_them() {
        let mut settings = InputSettings::default();
        let yaw = settings.bindings.sensitivity.yaw_deg_per_count;
        let edit = edit_for(&settings, &RowId::SensitivityYaw, &RowGesture::Step(3)).unwrap();
        settings.apply(edit).unwrap();
        assert!((settings.bindings.sensitivity.yaw_deg_per_count - (yaw + 0.003)).abs() < 1e-12);
        let pitch = settings.bindings.sensitivity.pitch_deg_per_count;
        settings
            .apply(edit_for(&settings, &RowId::SensitivityPitch, &RowGesture::Step(-2)).unwrap())
            .unwrap();
        assert!(
            (settings.bindings.sensitivity.pitch_deg_per_count - (pitch - 0.002)).abs() < 1e-12
        );
        settings
            .apply(edit_for(&settings, &RowId::InvertPitch, &RowGesture::Activate).unwrap())
            .unwrap();
        assert_eq!(
            value(&settings, &RowId::InvertPitch),
            RowValue::Toggle(true)
        );
        // Player 2 steps back from pad1 to pad0.
        settings
            .apply(edit_for(&settings, &RowId::PlayerDevice(1), &RowGesture::Step(-1)).unwrap())
            .unwrap();
        assert_eq!(settings.players[1], Device::Pad(0));
        // Wraps from none backwards to pad3, displacing player 4.
        settings.players[0] = Device::None;
        let outcome = settings
            .apply(edit_for(&settings, &RowId::PlayerDevice(0), &RowGesture::Step(-1)).unwrap())
            .unwrap();
        assert_eq!(
            (settings.players[0], outcome.displaced_players),
            (Device::Pad(3), vec![3])
        );
        let id = RowId::Binding(InputKindKey::Key, "KeyF".into());
        settings
            .apply(edit_for(&settings, &id, &RowGesture::Bind(Action::Y)).unwrap())
            .unwrap();
        assert_eq!(
            value(&settings, &id),
            RowValue::Binding { action: "y".into() }
        );
        let controller = RowId::Binding(InputKindKey::Controller, "south".into());
        settings
            .apply(edit_for(&settings, &controller, &RowGesture::Bind(Action::A)).unwrap())
            .unwrap();
        assert_eq!(settings.bindings.controller["south"], Action::A);
        assert_eq!(
            value(&settings, &controller),
            RowValue::Binding { action: "a".into() }
        );
    }

    #[test]
    fn controller_button_rebind_row_edits_the_persisted_profile_map() {
        let mut settings = InputSettings::default();
        let row = RowId::Binding(InputKindKey::Controller, "south".into());
        settings
            .apply(edit_for(&settings, &row, &RowGesture::Bind(Action::A)).unwrap())
            .unwrap();
        assert_eq!(settings.bindings.controller["south"], Action::A);
        assert_eq!(
            value(&settings, &row),
            RowValue::Binding { action: "a".into() }
        );
        let restored = InputSettings::parse(&settings.to_text()).unwrap();
        assert_eq!(restored.bindings.controller["south"], Action::A);
    }

    #[test]
    fn inapplicable_gestures_are_none() {
        let settings = InputSettings::default();
        assert_eq!(
            edit_for(
                &settings,
                &RowId::SensitivityYaw,
                &RowGesture::Bind(Action::A)
            ),
            None
        );
        assert_eq!(
            edit_for(
                &settings,
                &RowId::Binding(InputKindKey::Key, "KeyW".into()),
                &RowGesture::Step(1)
            ),
            None
        );
        assert_eq!(
            edit_for(&settings, &RowId::PlayerDevice(9), &RowGesture::Step(1)),
            None
        );
    }
}
