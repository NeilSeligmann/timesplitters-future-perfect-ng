// SPDX-License-Identifier: GPL-3.0-or-later
//! T2167 stage 2: preset table, per player settings and the extended TOML file, back compatible with T2160's
//! `bindings::Bindings` format (its sections and keys are untouched; the new sections are split off before it parses).
//!
//! ```toml
//! [preset.snappy]          # user preset, starts from `base` (default modern)
//! base = "modern"
//! dead_zone = 0.2          # also: curve_exponent yaw_deg_per_step pitch_deg_per_step acceleration smoothing
//!                          #       mouse_deg_per_count gyro_gain
//! [player.1]               # slot 0..3
//! preset = "snappy"
//! sensitivity_x = 1.2      # also: sensitivity_y invert_x invert_y dead_zone curve_exponent
//! [actions]                # intents for all slots, see actions.rs
//! fire = "mouse:Left, key:KeyF"
//! [actions.1]              # slot override
//! crouch = "toggle:key:KeyC"
//! ```
use crate::actions::{ActionMap, Intent};
use crate::bindings::Bindings;
use crate::shaping::{Kind, PlayerSettings, Preset, Shaper};

pub const MAX_SLOTS: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct PresetTable {
    presets: Vec<Preset>,
}

impl Default for PresetTable {
    fn default() -> Self {
        Self {
            presets: vec![Preset::original(), Preset::modern(), Preset::mouse_raw()],
        }
    }
}

impl PresetTable {
    pub fn get(&self, name: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.name == name)
    }
    pub fn names(&self) -> Vec<&str> {
        self.presets.iter().map(|p| p.name.as_str()).collect()
    }
    pub fn insert(&mut self, preset: Preset) -> Result<(), String> {
        if matches!(preset.name.as_str(), "original" | "modern" | "mouse-raw") {
            return Err(format!(
                "{:?} is a built in preset and cannot be redefined",
                preset.name
            ));
        }
        self.presets.retain(|p| p.name != preset.name);
        self.presets.push(preset);
        Ok(())
    }
}

/// `--input-preset NAME[:slot]`: no slot = all slots. Validates the name against the table.
pub fn parse_selection(text: &str, table: &PresetTable) -> Result<(Option<u8>, String), String> {
    let (name, slot) = match text.rsplit_once(':') {
        Some((name, slot)) => {
            let slot: u8 = slot
                .parse()
                .map_err(|_| format!("bad slot in --input-preset {text:?}"))?;
            if slot as usize >= MAX_SLOTS {
                return Err(format!("slot {slot} out of range 0..{MAX_SLOTS}"));
            }
            (name, Some(slot))
        }
        None => (text, None),
    };
    if table.get(name).is_none() {
        return Err(format!(
            "unknown preset {name:?} (known: {})",
            table.names().join(", ")
        ));
    }
    Ok((slot, name.to_string()))
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputConfig {
    pub bindings: Bindings,
    pub presets: PresetTable,
    pub players: Vec<PlayerSettings>,
    pub actions: ActionMap,
    pub slot_actions: Vec<Option<ActionMap>>,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            bindings: Bindings::default(),
            presets: PresetTable::default(),
            players: vec![PlayerSettings::default(); MAX_SLOTS],
            actions: ActionMap::default_mouse_keyboard(),
            slot_actions: vec![None; MAX_SLOTS],
        }
    }
}

fn number(value: &str, low: f32, high: f32) -> Option<f32> {
    value
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v >= low && *v <= high)
}

impl InputConfig {
    pub fn parse(text: &str) -> Result<InputConfig, String> {
        let mut config = InputConfig::default();
        // Lines of the new sections are blanked out for Bindings::parse so its line numbers stay right.
        let mut legacy = String::new();
        let mut section = String::new();
        let mut draft: Option<Preset> = None;
        let mut finished: Vec<Preset> = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            let fail = |why: &str| format!("line {}: {why}: {raw:?}", index + 1);
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                section = name.trim().to_string();
                if let Some(preset) = draft.take() {
                    finished.push(preset);
                }
                if let Some(name) = section.strip_prefix("preset.") {
                    if name.is_empty() {
                        return Err(fail("empty preset name"));
                    }
                    draft = Some(Preset {
                        name: name.to_string(),
                        ..Preset::modern()
                    });
                }
            }
            let ours = section.starts_with("preset.")
                || section.starts_with("player.")
                || section == "actions"
                || section.starts_with("actions.");
            if !ours {
                legacy.push_str(raw);
                legacy.push('\n');
                continue;
            }
            legacy.push('\n');
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| fail("expected key = value"))?;
            let (key, value) = (key.trim().trim_matches('"'), value.trim().trim_matches('"'));
            if let Some(preset) = draft.as_mut() {
                let f = |low, high| number(value, low, high).ok_or_else(|| fail("bad number"));
                match key {
                    "base" => {
                        let base = match value {
                            "original" => Preset::original(),
                            "modern" => Preset::modern(),
                            "mouse-raw" => Preset::mouse_raw(),
                            _ => return Err(fail("unknown base preset")),
                        };
                        *preset = Preset {
                            name: preset.name.clone(),
                            ..base
                        };
                    }
                    "dead_zone" => preset.dead_zone = f(0.0, 0.99)?,
                    "curve_exponent" => preset.curve_exponent = f(0.1, 8.0)?,
                    "yaw_deg_per_step" => preset.yaw_deg_per_step = f(0.0, 90.0)?,
                    "pitch_deg_per_step" => preset.pitch_deg_per_step = f(0.0, 90.0)?,
                    "acceleration" => preset.acceleration = f(0.0, 4.0)?,
                    "smoothing" => preset.smoothing = f(0.0, 0.99)?,
                    "mouse_deg_per_count" => preset.mouse_deg_per_count = f(0.0, 10.0)?,
                    "gyro_gain" => preset.gyro_gain = f(0.0, 100.0)?,
                    _ => return Err(fail("unknown preset key")),
                }
            } else if let Some(slot) = section.strip_prefix("player.") {
                let slot = slot_of(slot).ok_or_else(|| fail("bad slot"))?;
                let player = &mut config.players[slot];
                let f = |low, high| number(value, low, high).ok_or_else(|| fail("bad number"));
                let flag = || {
                    value
                        .parse::<bool>()
                        .map_err(|_| fail("expected true or false"))
                };
                match key {
                    "preset" => player.preset = value.to_string(),
                    "sensitivity_x" => player.sensitivity_x = f(0.0, 100.0)?,
                    "sensitivity_y" => player.sensitivity_y = f(0.0, 100.0)?,
                    "invert_x" => player.invert_x = flag()?,
                    "invert_y" => player.invert_y = flag()?,
                    "dead_zone" => player.dead_zone = Some(f(0.0, 0.99)?),
                    "curve_exponent" => player.curve_exponent = Some(f(0.1, 8.0)?),
                    _ => return Err(fail("unknown player key")),
                }
            } else {
                let intent = Intent::parse(key).ok_or_else(|| fail("unknown intent"))?;
                let map = match section.strip_prefix("actions.") {
                    Some(slot) => {
                        let slot = slot_of(slot).ok_or_else(|| fail("bad slot"))?;
                        config.slot_actions[slot].get_or_insert_with(|| config.actions.clone())
                    }
                    None => &mut config.actions,
                };
                map.set(intent, value).map_err(|why| fail(&why))?;
            }
        }
        if let Some(preset) = draft.take() {
            finished.push(preset);
        }
        for preset in finished {
            config
                .presets
                .insert(preset)
                .map_err(|why| format!("{why}"))?;
        }
        config.bindings = Bindings::parse(&legacy)?;
        for (slot, player) in config.players.iter().enumerate() {
            if config.presets.get(&player.preset).is_none() {
                return Err(format!(
                    "[player.{slot}] names unknown preset {:?}",
                    player.preset
                ));
            }
        }
        Ok(config)
    }

    pub fn load(path: &str, required: bool) -> Result<InputConfig, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => InputConfig::parse(&text).map_err(|why| format!("{path}: {why}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !required => {
                Ok(InputConfig::default())
            }
            Err(error) => Err(format!("{path}: {error}")),
        }
    }

    /// Apply a `--input-preset NAME[:slot]` selection.
    pub fn select(&mut self, slot: Option<u8>, name: &str) {
        for (at, player) in self.players.iter_mut().enumerate() {
            if slot.map_or(true, |s| s as usize == at) {
                player.preset = name.to_string();
            }
        }
    }

    pub fn actions_for(&self, slot: u8) -> &ActionMap {
        self.slot_actions
            .get(slot as usize)
            .and_then(|m| m.as_ref())
            .unwrap_or(&self.actions)
    }

    /// A fresh shaper for the slot (falls back to `original` for an unknown name or slot).
    pub fn shaper(&self, slot: u8) -> Shaper {
        let settings = self.players.get(slot as usize).cloned().unwrap_or_default();
        let preset = self
            .presets
            .get(&settings.preset)
            .cloned()
            .unwrap_or_else(Preset::original);
        Shaper::new(preset, settings)
    }
}

fn slot_of(text: &str) -> Option<usize> {
    text.parse::<usize>().ok().filter(|s| *s < MAX_SLOTS)
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Original => "original",
            Kind::Modern => "modern",
            Kind::MouseRaw => "mouse-raw",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Intent;

    #[test]
    fn t2160_files_parse_unchanged_and_defaults_stay_original() {
        let text = "[keys]\nKeyW = \"move_back\"\n[look]\nsensitivity_yaw = 0.05\n";
        let config = InputConfig::parse(text).unwrap();
        assert_eq!(config.bindings, Bindings::parse(text).unwrap());
        assert!(config.players.iter().all(|p| p.preset == "original"));
        assert_eq!(config.shaper(0).preset().kind, Kind::Original);
        assert_eq!(InputConfig::parse("").unwrap(), InputConfig::default());
    }

    #[test]
    fn user_presets_players_and_actions() {
        let text = "[keys]\nKeyZ = a\n[preset.snappy]\nbase = \"modern\"\ndead_zone = 0.2\nsmoothing = 0.3\n[player.1]\npreset = \"snappy\"\nsensitivity_x = 1.5\ninvert_y = true\n[actions]\nfire = \"mouse:Left, key:KeyF\"\n[actions.1]\ncrouch = \"toggle:key:KeyC\"\n";
        let config = InputConfig::parse(text).unwrap();
        let snappy = config.presets.get("snappy").unwrap();
        assert_eq!(
            (snappy.kind, snappy.dead_zone, snappy.smoothing),
            (Kind::Modern, 0.2, 0.3)
        );
        assert_eq!(config.players[1].sensitivity_x, 1.5);
        assert_eq!(config.players[0].preset, "original");
        assert_eq!(config.shaper(1).preset().name, "snappy");
        assert_eq!(config.bindings.keys["KeyZ"], crate::bindings::Action::A);
        assert!(
            config
                .actions_for(0)
                .bindings
                .iter()
                .filter(|(i, _)| *i == Intent::Fire)
                .count()
                == 2
        );
        assert!(
            config
                .actions_for(1)
                .bindings
                .iter()
                .any(|(i, b)| *i == Intent::Crouch && b.toggle)
        );
        assert!(!config.actions_for(0).bindings.iter().any(|(_, b)| b.toggle));
    }

    #[test]
    fn errors_name_the_line() {
        for (text, needle) in [
            ("[preset.x]\ndead_zone = 5\n", "line 2: bad number"),
            ("[player.9]\npreset = original\n", "line 2: bad slot"),
            ("[actions]\njump = fly\n", "line 2:"),
            ("[actions]\nfly = key:A\n", "unknown intent"),
            ("[player.0]\npreset = nope\n", "unknown preset"),
            ("[preset.original]\n", "built in"),
            ("\n\n[keys]\nKeyW = fly\n", "line 4: unknown action"),
        ] {
            let error = InputConfig::parse(text).unwrap_err();
            assert!(error.contains(needle), "{text:?}: {error}");
        }
    }

    #[test]
    fn selection_flag() {
        let table = PresetTable::default();
        assert_eq!(
            parse_selection("modern", &table).unwrap(),
            (None, "modern".into())
        );
        assert_eq!(
            parse_selection("mouse-raw:1", &table).unwrap(),
            (Some(1), "mouse-raw".into())
        );
        assert!(
            parse_selection("nope", &table).is_err()
                && parse_selection("modern:7", &table).is_err()
        );
        let mut config = InputConfig::default();
        config.select(Some(1), "modern");
        assert_eq!(
            (
                config.players[0].preset.as_str(),
                config.players[1].preset.as_str()
            ),
            ("original", "modern")
        );
    }
}
