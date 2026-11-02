// SPDX-License-Identifier: GPL-3.0-or-later
//! T2163 (MK-5) host-side mouse and keyboard settings model: per-player device assignment, look sensitivity,
//! pitch invert and binding remap, persisted in the authenticated player profile (T2211).
//! Pure data and edits, no host. Contract: docs/t2163-mk-settings-model.md.
//!
//! File format is the [`Bindings`] TOML subset plus an optional section:
//! ```toml
//! [players]
//! p1 = "keyboard_mouse"   # keyboard_mouse | pad0..pad3 | none
//! p2 = "pad1"
//! ```
use crate::bindings::{Action, Bindings};

pub const PLAYER_COUNT: usize = 4;
/// Sensitivity bounds accepted by [`Bindings::parse`] are (0, 10); the editor keeps a practical range.
pub const SENSITIVITY_MIN: f64 = 0.001;
pub const SENSITIVITY_MAX: f64 = 1.0;
pub const SENSITIVITY_STEP: f64 = 0.001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    None,
    KeyboardMouse,
    /// Physical pad index 0..=3.
    Pad(u8),
}

impl Device {
    pub fn name(self) -> String {
        match self {
            Device::None => "none".into(),
            Device::KeyboardMouse => "keyboard_mouse".into(),
            Device::Pad(index) => format!("pad{index}"),
        }
    }
    pub fn parse(text: &str) -> Option<Device> {
        match text {
            "none" => Some(Device::None),
            "keyboard_mouse" => Some(Device::KeyboardMouse),
            _ => text
                .strip_prefix("pad")?
                .parse()
                .ok()
                .filter(|index| *index < PLAYER_COUNT as u8)
                .map(Device::Pad),
        }
    }
    /// Choice order used by settings rows.
    pub fn choices() -> [Device; 6] {
        [
            Device::None,
            Device::KeyboardMouse,
            Device::Pad(0),
            Device::Pad(1),
            Device::Pad(2),
            Device::Pad(3),
        ]
    }
}

/// Which physical input family a binding row remaps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Key,
    Mouse,
    Controller,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingEdit {
    SensitivityYaw(f64),
    SensitivityPitch(f64),
    InvertPitch(bool),
    FixedDevices(bool),
    Rebind {
        kind: InputKind,
        input: String,
        action: Action,
    },
    AssignDevice {
        player: usize,
        device: Device,
    },
}

/// What an edit changed beyond its own field, so a menu can tell the user.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditOutcome {
    /// Other inputs that were already bound to the same action (kept: several keys may share one action).
    pub shared_with: Vec<String>,
    /// Players whose device was cleared because a device is owned by one player at a time.
    pub displaced_players: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputSettings {
    pub players: [Device; PLAYER_COUNT],
    /// False preserves automatic pad/keyboard hot-swap; true reserves explicit assignments.
    pub fixed_devices: bool,
    /// Player zero map (legacy API). Other players never alias this record.
    pub bindings: Bindings,
    pub other_bindings: [Bindings; PLAYER_COUNT - 1],
}

impl Default for InputSettings {
    /// Player 1 on keyboard and mouse, others on their pad: the hybrid host's current single-player routing (INFERRED).
    fn default() -> Self {
        Self {
            fixed_devices: false,
            players: [
                Device::KeyboardMouse,
                Device::Pad(1),
                Device::Pad(2),
                Device::Pad(3),
            ],
            bindings: Bindings::default(),
            other_bindings: std::array::from_fn(|_| Bindings::default()),
        }
    }
}

impl InputSettings {
    /// Restore this player's native map and default device, reporting exclusive-device displacement.
    pub fn reset_player(&mut self, player: usize) -> Result<EditOutcome, String> {
        if player >= PLAYER_COUNT {
            return Err(format!("player {player} out of range"));
        }
        let outcome = self.apply(SettingEdit::AssignDevice {
            player,
            device: Self::default().players[player],
        })?;
        *self.player_bindings_mut(player).unwrap() = Bindings::default();
        Ok(outcome)
    }

    pub fn player_bindings(&self, player: usize) -> Option<&Bindings> {
        if player == 0 {
            Some(&self.bindings)
        } else {
            self.other_bindings.get(player - 1)
        }
    }
    pub fn player_bindings_mut(&mut self, player: usize) -> Option<&mut Bindings> {
        if player == 0 {
            Some(&mut self.bindings)
        } else {
            self.other_bindings.get_mut(player - 1)
        }
    }
    pub fn apply_player(
        &mut self,
        player: usize,
        edit: SettingEdit,
    ) -> Result<EditOutcome, String> {
        if player >= PLAYER_COUNT {
            return Err(format!("player {player} out of range"));
        }
        if player == 0
            || matches!(
                edit,
                SettingEdit::AssignDevice { .. } | SettingEdit::FixedDevices(_)
            )
        {
            return self.apply(edit);
        }
        // Reuse validation without sharing mutable maps between players.
        let mut local = Self::default();
        local.bindings = self.other_bindings[player - 1].clone();
        let result = local.apply(edit)?;
        self.other_bindings[player - 1] = local.bindings;
        Ok(result)
    }

    pub fn parse(text: &str) -> Result<InputSettings, String> {
        let mut legacy = String::new();
        let mut maps: [String; PLAYER_COUNT - 1] = std::array::from_fn(|_| String::new());
        let mut selected = None;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.starts_with('[') {
                selected = None;
                for player in 2..=PLAYER_COUNT {
                    for section in ["look", "keys", "mouse", "controller"] {
                        if line == format!("[p{player}.{section}]") {
                            selected = Some(player - 2);
                            maps[player - 2] += &format!("[{section}]\n");
                        }
                    }
                }
                if selected.is_some() {
                    legacy.push('\n');
                    continue;
                }
            }
            if let Some(index) = selected {
                maps[index] += raw;
                maps[index].push('\n');
                legacy.push('\n');
            } else {
                legacy += raw;
                legacy.push('\n');
            }
        }
        let text = legacy.as_str();
        let mut explicit: Vec<(usize, Device)> = Vec::new();
        let mut fixed_devices = None;
        let mut rest = String::new();
        let mut in_players = false;
        for (index, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.starts_with('[') {
                in_players = line == "[players]";
            }
            if !in_players {
                rest.push_str(raw);
            } else if !line.is_empty() && !line.starts_with('[') {
                let fail = |why: &str| format!("line {}: {why}: {raw:?}", index + 1);
                let (key, value) = line
                    .split_once('=')
                    .ok_or_else(|| fail("expected key = value"))?;
                if key.trim() == "fixed_devices" {
                    fixed_devices = Some(match value.trim() {
                        "true" => true,
                        "false" => false,
                        _ => return Err(fail("expected true or false")),
                    });
                    rest.push('\n');
                    continue;
                }
                let slot = key
                    .trim()
                    .strip_prefix('p')
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|n| (1..=PLAYER_COUNT).contains(n))
                    .ok_or_else(|| fail("expected p1..p4"))?;
                let device = Device::parse(value.trim().trim_matches('"'))
                    .ok_or_else(|| fail("unknown device"))?;
                explicit.push((slot - 1, device));
            }
            // Keep line numbers stable for Bindings::parse errors.
            rest.push('\n');
        }
        let mut settings = InputSettings {
            players: [Device::None; PLAYER_COUNT],
            fixed_devices: false,
            bindings: Bindings::parse(&rest)?,
            other_bindings: std::array::from_fn(|_| Bindings::default()),
        };
        for (index, text) in maps.iter().enumerate() {
            // Old shared-map profiles migrate by value, preserving their behavior.
            settings.other_bindings[index] = if text.is_empty() {
                settings.bindings.clone()
            } else {
                Bindings::parse(text)?
            };
        }
        // File entries in order (a later duplicate wins), then defaults for unlisted players whose device is free.
        for &(player, device) in &explicit {
            settings.apply(SettingEdit::AssignDevice { player, device })?;
        }
        for (player, device) in InputSettings::default().players.into_iter().enumerate() {
            let listed = explicit.iter().any(|(p, _)| *p == player);
            if !listed && !settings.players.contains(&device) {
                settings.players[player] = device;
            }
        }
        // Older native envelopes always pinned assignments on adoption. Preserve that
        // legacy behavior; newly serialized defaults explicitly opt into hot-swap.
        settings.fixed_devices = fixed_devices.unwrap_or(true);
        Ok(settings)
    }

    /// Canonical text: sorted keys, round-trips through [`InputSettings::parse`].
    fn to_text_base(&self) -> String {
        let s = &self.bindings.sensitivity;
        let mut out = format!("[players]\nfixed_devices = {}\n", self.fixed_devices);
        for (index, device) in self.players.iter().enumerate() {
            out += &format!("p{} = \"{}\"\n", index + 1, device.name());
        }
        out += &format!(
            "[look]\nsensitivity_yaw = {}\nsensitivity_pitch = {}\ninvert_pitch = {}\n",
            s.yaw_deg_per_count, s.pitch_deg_per_count, s.invert_pitch
        );
        for (title, map) in [
            ("keys", &self.bindings.keys),
            ("mouse", &self.bindings.mouse),
            ("controller", &self.bindings.controller),
        ] {
            out += &format!("[{title}]\n");
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            for (input, action) in entries {
                out += &format!("{input} = \"{}\"\n", action_name(*action));
            }
        }
        out
    }

    pub fn to_text(&self) -> String {
        let mut out = self.to_text_base();
        for (index, bindings) in self.other_bindings.iter().enumerate() {
            let mut local = Self::default();
            local.bindings = bindings.clone();
            let text = local.to_legacy_text();
            for line in text.lines().skip(PLAYER_COUNT + 2) {
                if line.starts_with('[') {
                    out += &format!("[p{}.{}\n", index + 2, &line[1..]);
                } else {
                    out += line;
                    out.push('\n');
                }
            }
        }
        out
    }

    fn to_legacy_text(&self) -> String {
        self.to_text_base()
    }

    pub fn apply(&mut self, edit: SettingEdit) -> Result<EditOutcome, String> {
        let mut outcome = EditOutcome::default();
        let clamp = |v: f64| -> Result<f64, String> {
            if v.is_finite() {
                Ok(v.clamp(SENSITIVITY_MIN, SENSITIVITY_MAX))
            } else {
                Err("sensitivity must be finite".into())
            }
        };
        let s = &mut self.bindings.sensitivity;
        match edit {
            SettingEdit::SensitivityYaw(v) => s.yaw_deg_per_count = clamp(v)?,
            SettingEdit::SensitivityPitch(v) => s.pitch_deg_per_count = clamp(v)?,
            SettingEdit::InvertPitch(on) => s.invert_pitch = on,
            SettingEdit::FixedDevices(on) => self.fixed_devices = on,
            SettingEdit::Rebind {
                kind,
                input,
                action,
            } => {
                if input.is_empty() || input.contains(['=', '#', '"', '[', '\n']) {
                    return Err(format!("bad input name {input:?}"));
                }
                if kind == InputKind::Mouse && !MOUSE_INPUTS.contains(&input.as_str()) {
                    return Err(format!("unknown mouse input {input:?}"));
                }
                if action != Action::None {
                    for (other, bound) in self
                        .bindings
                        .keys
                        .iter()
                        .chain(&self.bindings.mouse)
                        .chain(&self.bindings.controller)
                    {
                        if *bound == action && *other != input {
                            outcome.shared_with.push(other.clone());
                        }
                    }
                    outcome.shared_with.sort();
                }
                let map = match kind {
                    InputKind::Key => &mut self.bindings.keys,
                    InputKind::Mouse => &mut self.bindings.mouse,
                    InputKind::Controller => &mut self.bindings.controller,
                };
                map.insert(input, action);
            }
            SettingEdit::AssignDevice { player, device } => {
                if player >= PLAYER_COUNT {
                    return Err(format!("player {player} out of range"));
                }
                if device != Device::None {
                    for (other, slot) in self.players.iter_mut().enumerate() {
                        if other != player && *slot == device {
                            *slot = Device::None;
                            outcome.displaced_players.push(other);
                        }
                    }
                }
                self.players[player] = device;
                self.fixed_devices = true;
            }
        }
        Ok(outcome)
    }
}

/// Mouse input names accepted by [`Bindings::parse`] (see the module docs of `bindings`).
pub const MOUSE_INPUTS: [&str; 7] = [
    "Left",
    "Right",
    "Middle",
    "Back",
    "Forward",
    "WheelUp",
    "WheelDown",
];

/// Inverse of [`Action::parse`].
pub fn action_name(action: Action) -> String {
    use Action::*;
    match action {
        MoveForward => "move_forward",
        MoveBack => "move_back",
        MoveLeft => "move_left",
        MoveRight => "move_right",
        Walk => "walk",
        A => "a",
        B => "b",
        X => "x",
        Y => "y",
        Start => "start",
        Back => "back",
        L3 => "l3",
        R3 => "r3",
        Black => "black",
        White => "white",
        Lt => "lt",
        Rt => "rt",
        DpadUp => "dpad_up",
        DpadDown => "dpad_down",
        DpadLeft => "dpad_left",
        DpadRight => "dpad_right",
        Slot(n) => return format!("slot{n}"),
        None => "none",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_policy_roundtrips_and_legacy_native_profiles_stay_fixed() {
        let mut settings = InputSettings::default();
        settings
            .apply_player(2, SettingEdit::SensitivityYaw(0.2))
            .unwrap();
        assert!(!settings.fixed_devices);
        assert_eq!(InputSettings::parse(&settings.to_text()).unwrap(), settings);
        settings
            .apply(SettingEdit::AssignDevice {
                player: 1,
                device: Device::Pad(0),
            })
            .unwrap();
        assert!(settings.fixed_devices);
        assert_eq!(InputSettings::parse(&settings.to_text()).unwrap(), settings);
        settings
            .apply_player(2, SettingEdit::FixedDevices(false))
            .unwrap();
        assert!(
            !InputSettings::parse(&settings.to_text())
                .unwrap()
                .fixed_devices
        );
        let legacy = settings.to_text().replace("fixed_devices = false\n", "");
        assert!(InputSettings::parse(&legacy).unwrap().fixed_devices);
        assert!(InputSettings::parse("[players]\nfixed_devices = maybe\n").is_err());
    }

    #[test]
    fn independent_maps_roundtrip_and_legacy_maps_migrate_by_value() {
        let mut settings = InputSettings::default();
        settings
            .apply_player(1, SettingEdit::SensitivityYaw(0.2))
            .unwrap();
        settings
            .apply_player(
                1,
                SettingEdit::Rebind {
                    kind: InputKind::Key,
                    input: "KeyW".into(),
                    action: Action::A,
                },
            )
            .unwrap();
        settings
            .apply_player(
                1,
                SettingEdit::Rebind {
                    kind: InputKind::Controller,
                    input: "south".into(),
                    action: Action::Y,
                },
            )
            .unwrap();
        settings
            .apply_player(2, SettingEdit::SensitivityPitch(0.3))
            .unwrap();
        assert_eq!(settings.bindings.keys["KeyW"], Action::MoveForward);
        assert_eq!(settings.bindings.controller["south"], Action::A);
        assert_eq!(settings.player_bindings(1).unwrap().controller["south"], Action::Y);
        assert_ne!(settings.player_bindings(1), settings.player_bindings(2));
        assert_eq!(InputSettings::parse(&settings.to_text()).unwrap(), settings);
        let mut legacy = InputSettings::default();
        legacy.apply(SettingEdit::SensitivityYaw(0.15)).unwrap();
        let migrated = InputSettings::parse(&legacy.to_text_base()).unwrap();
        for player in 0..PLAYER_COUNT {
            assert_eq!(migrated.player_bindings(player), Some(&legacy.bindings));
        }
        assert!(
            settings
                .apply_player(4, SettingEdit::InvertPitch(true))
                .is_err()
        );
        assert_eq!(
            InputSettings::default().player_bindings(1),
            Some(&Bindings::default())
        );
    }

    #[test]
    fn reset_is_player_local_and_roundtrips_with_device_conflicts() {
        let mut settings = InputSettings::default();
        settings
            .apply_player(2, SettingEdit::SensitivityYaw(0.4))
            .unwrap();
        settings
            .apply_player(1, SettingEdit::SensitivityPitch(0.3))
            .unwrap();
        settings
            .apply_player(
                2,
                SettingEdit::Rebind {
                    kind: InputKind::Key,
                    input: "KeyZ".into(),
                    action: Action::Start,
                },
            )
            .unwrap();
        settings
            .apply(SettingEdit::AssignDevice {
                player: 0,
                device: Device::Pad(2),
            })
            .unwrap();
        let neighbour = settings.player_bindings(1).unwrap().clone();
        let outcome = settings.reset_player(2).unwrap();
        assert_eq!(outcome.displaced_players, vec![0]);
        assert_eq!(settings.players[0], Device::None);
        assert_eq!(settings.players[2], Device::Pad(2));
        assert_eq!(settings.player_bindings(2), Some(&Bindings::default()));
        assert_eq!(settings.player_bindings(1), Some(&neighbour));
        assert_eq!(InputSettings::parse(&settings.to_text()).unwrap(), settings);
        let before = settings.clone();
        assert!(settings.reset_player(4).is_err());
        assert_eq!(settings, before);
    }

    #[test]
    fn action_names_round_trip() {
        let mut names = vec![
            "move_forward",
            "move_back",
            "move_left",
            "move_right",
            "walk",
            "a",
            "b",
            "x",
            "y",
            "start",
            "back",
            "l3",
            "r3",
            "black",
            "white",
            "lt",
            "rt",
            "dpad_up",
            "dpad_down",
            "dpad_left",
            "dpad_right",
            "none",
        ];
        let slots: Vec<String> = (1..=9).map(|n| format!("slot{n}")).collect();
        names.extend(slots.iter().map(String::as_str));
        for name in names {
            let action = Action::parse(name).unwrap();
            assert_eq!(action_name(action), name);
        }
    }

    #[test]
    fn default_text_round_trips() {
        let settings = InputSettings::default();
        let text = settings.to_text();
        assert!(text.contains("p1 = \"keyboard_mouse\""));
        let keys: Vec<&str> = text
            .split("[keys]\n")
            .nth(1)
            .unwrap()
            .split("[mouse]")
            .next()
            .unwrap()
            .lines()
            .map(|l| l.split(" = ").next().unwrap())
            .collect();
        assert!(
            keys.len() > 1 && keys.windows(2).all(|w| w[0] < w[1]),
            "canonical sorted keys"
        );
        assert_eq!(InputSettings::parse(&text).unwrap(), settings);
    }

    #[test]
    fn edited_settings_round_trip() {
        let mut settings = InputSettings::default();
        settings.apply(SettingEdit::SensitivityYaw(0.05)).unwrap();
        settings
            .apply(SettingEdit::SensitivityPitch(0.031))
            .unwrap();
        settings.apply(SettingEdit::InvertPitch(true)).unwrap();
        settings
            .apply(SettingEdit::Rebind {
                kind: InputKind::Key,
                input: "KeyZ".into(),
                action: Action::Slot(4),
            })
            .unwrap();
        settings
            .apply(SettingEdit::Rebind {
                kind: InputKind::Mouse,
                input: "Back".into(),
                action: Action::X,
            })
            .unwrap();
        settings
            .apply(SettingEdit::AssignDevice {
                player: 1,
                device: Device::Pad(0),
            })
            .unwrap();
        let back = InputSettings::parse(&settings.to_text()).unwrap();
        assert_eq!(back, settings);
        assert!(back.bindings.sensitivity.invert_pitch);
        assert_eq!(back.bindings.keys["KeyZ"], Action::Slot(4));
        assert_eq!(back.players[1], Device::Pad(0));
    }

    #[test]
    fn device_is_owned_by_one_player() {
        let mut settings = InputSettings::default();
        let outcome = settings
            .apply(SettingEdit::AssignDevice {
                player: 2,
                device: Device::KeyboardMouse,
            })
            .unwrap();
        assert_eq!(outcome.displaced_players, vec![0]);
        assert_eq!(settings.players[0], Device::None);
        assert_eq!(settings.players[2], Device::KeyboardMouse);
        // None never displaces.
        let mut settings = InputSettings::default();
        settings.players[1] = Device::None;
        let outcome = settings
            .apply(SettingEdit::AssignDevice {
                player: 3,
                device: Device::None,
            })
            .unwrap();
        assert!(outcome.displaced_players.is_empty());
        assert_eq!(settings.players[1], Device::None);
        assert!(
            settings
                .apply(SettingEdit::AssignDevice {
                    player: 4,
                    device: Device::None
                })
                .is_err()
        );
    }

    #[test]
    fn duplicate_device_in_file_keeps_the_later_player() {
        let settings = InputSettings::parse("[players]\np1 = \"pad2\"\np2 = \"pad2\"\n").unwrap();
        assert_eq!(settings.players[0], Device::None);
        assert_eq!(settings.players[1], Device::Pad(2));
        assert_eq!(settings.players[2], Device::None, "default pad2 is taken");
        assert_eq!(
            settings.players[3],
            Device::Pad(3),
            "unlisted player keeps its free default"
        );
    }

    #[test]
    fn sensitivity_is_clamped_and_finite() {
        let mut settings = InputSettings::default();
        settings.apply(SettingEdit::SensitivityYaw(50.0)).unwrap();
        assert_eq!(
            settings.bindings.sensitivity.yaw_deg_per_count,
            SENSITIVITY_MAX
        );
        settings.apply(SettingEdit::SensitivityPitch(-1.0)).unwrap();
        assert_eq!(
            settings.bindings.sensitivity.pitch_deg_per_count,
            SENSITIVITY_MIN
        );
        assert!(
            settings
                .apply(SettingEdit::SensitivityYaw(f64::NAN))
                .is_err()
        );
    }

    #[test]
    fn rebind_reports_shared_inputs_and_rejects_bad_names() {
        let mut settings = InputSettings::default();
        let outcome = settings
            .apply(SettingEdit::Rebind {
                kind: InputKind::Key,
                input: "KeyV".into(),
                action: Action::L3,
            })
            .unwrap();
        assert_eq!(
            outcome.shared_with,
            vec![
                "ControlLeft".to_string(),
                "KeyC".to_string(),
                "left_thumb".to_string(),
            ]
        );
        let unbind = settings
            .apply(SettingEdit::Rebind {
                kind: InputKind::Key,
                input: "KeyW".into(),
                action: Action::None,
            })
            .unwrap();
        assert!(unbind.shared_with.is_empty());
        assert!(
            settings
                .apply(SettingEdit::Rebind {
                    kind: InputKind::Mouse,
                    input: "Side9".into(),
                    action: Action::A
                })
                .is_err()
        );
        assert!(
            settings
                .apply(SettingEdit::Rebind {
                    kind: InputKind::Key,
                    input: "a=b".into(),
                    action: Action::A
                })
                .is_err()
        );
    }

    #[test]
    fn parse_errors_keep_line_numbers() {
        let error =
            InputSettings::parse("[players]\np1 = \"pad0\"\n[keys]\nKeyW = \"fly\"\n").unwrap_err();
        assert!(error.starts_with("line 4"), "{error}");
        let error = InputSettings::parse("[players]\np9 = \"pad0\"\n").unwrap_err();
        assert!(error.starts_with("line 2"), "{error}");
    }
}
