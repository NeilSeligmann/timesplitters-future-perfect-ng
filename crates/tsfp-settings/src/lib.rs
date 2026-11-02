// SPDX-License-Identifier: GPL-3.0-or-later
//! User-owned, opt-in settings. Nothing is sent to the host until an active setting is applied.
use serde_json::{Map, Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub const VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Display,
    Graphics,
    Audio,
    Gameplay,
    Diagnostics,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Active,
    Placeholder(&'static str),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Bool,
    Enum(&'static [&'static str]),
    Range { min: f64, max: f64 },
    List,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting {
    pub id: &'static str,
    pub label: &'static str,
    pub group: Group,
    pub kind: Kind,
    pub default: &'static str,
    pub status: Status,
}

macro_rules! setting {
    ($id:literal, $group:ident, $kind:expr, $default:literal, $needs:literal) => {
        Setting {
            id: $id,
            label: $id,
            group: Group::$group,
            kind: $kind,
            default: $default,
            status: Status::Placeholder($needs),
        }
    };
}
use Kind::{Bool, Enum, List, Range};
pub static SCHEMA: &[Setting] = &[
    setting!(
        "window_mode",
        Display,
        Enum(&["windowed", "borderless", "fullscreen"]),
        "windowed",
        "video"
    ),
    setting!("resolution", Display, List, "display", "video"),
    setting!(
        "resolution_width",
        Display,
        Range {
            min: 320.0,
            max: 16384.0
        },
        "1920",
        "video"
    ),
    setting!(
        "resolution_height",
        Display,
        Range {
            min: 200.0,
            max: 16384.0
        },
        "1080",
        "video"
    ),
    setting!(
        "aspect_anchor",
        Display,
        Enum(&["center", "left", "right", "stretch"]),
        "center",
        "renderer"
    ),
    setting!(
        "vsync",
        Display,
        Enum(&["off", "on", "adaptive"]),
        "on",
        "video"
    ),
    setting!(
        "fps_game",
        Display,
        Enum(&[
            "off", "30", "60", "90", "120", "144", "165", "240", "custom"
        ]),
        "60",
        "host"
    ),
    setting!(
        "fps_menus",
        Display,
        Enum(&[
            "off", "30", "60", "90", "120", "144", "165", "240", "custom"
        ]),
        "60",
        "host"
    ),
    setting!(
        "fps_game_custom",
        Display,
        Range {
            min: 1.0,
            max: 1000.0
        },
        "60",
        "host"
    ),
    setting!(
        "fps_menus_custom",
        Display,
        Range {
            min: 1.0,
            max: 1000.0
        },
        "60",
        "host"
    ),
    setting!(
        "ui_scale",
        Display,
        Range { min: 0.5, max: 2.0 },
        "1",
        "host"
    ),
    setting!("monitor", Display, List, "default", "video"),
    setting!(
        "anti_aliasing",
        Graphics,
        Enum(&["off", "fxaa", "smaa", "msaa", "taa"]),
        "off",
        "renderer"
    ),
    setting!(
        "msaa_samples",
        Graphics,
        Enum(&["2x", "4x", "8x"]),
        "2x",
        "renderer"
    ),
    setting!(
        "upscaling",
        Graphics,
        Enum(&["native", "bilinear", "fsr1"]),
        "native",
        "renderer"
    ),
    setting!(
        "render_scale",
        Graphics,
        Range {
            min: 50.0,
            max: 200.0
        },
        "100",
        "renderer"
    ),
    setting!(
        "sharpening",
        Graphics,
        Range { min: 0.0, max: 1.0 },
        "0",
        "renderer"
    ),
    setting!(
        "texture_filter",
        Graphics,
        Enum(&["nearest", "linear", "anisotropic"]),
        "linear",
        "renderer"
    ),
    setting!(
        "anisotropy",
        Graphics,
        Enum(&["2x", "4x", "8x", "16x"]),
        "2x",
        "renderer"
    ),
    setting!(
        "shadows",
        Graphics,
        Enum(&["off", "low", "medium", "high"]),
        "off",
        "renderer"
    ),
    setting!(
        "shadow_resolution",
        Graphics,
        Enum(&["512", "1024", "2048", "4096"]),
        "1024",
        "renderer"
    ),
    setting!(
        "shadow_distance",
        Graphics,
        Range {
            min: 0.0,
            max: 100.0
        },
        "50",
        "renderer"
    ),
    setting!("particles", Graphics, Bool, "true", "renderer"),
    setting!("bloom", Graphics, Bool, "false", "renderer"),
    setting!("post_processing", Graphics, Bool, "true", "renderer"),
    setting!(
        "texture_pack",
        Graphics,
        Enum(&["original", "enhanced"]),
        "original",
        "renderer"
    ),
    setting!(
        "master_volume",
        Audio,
        Range { min: 0.0, max: 1.0 },
        "1",
        "audio"
    ),
    setting!(
        "music_volume",
        Audio,
        Range { min: 0.0, max: 1.0 },
        "1",
        "audio"
    ),
    setting!(
        "effects_volume",
        Audio,
        Range { min: 0.0, max: 1.0 },
        "1",
        "audio"
    ),
    setting!(
        "voice_volume",
        Audio,
        Range { min: 0.0, max: 1.0 },
        "1",
        "audio"
    ),
    setting!("output_device", Audio, List, "default", "audio"),
    setting!(
        "channel_mode",
        Audio,
        Enum(&["stereo", "surround"]),
        "stereo",
        "audio"
    ),
    setting!("positional_audio", Audio, Bool, "false", "audio"),
    setting!("mute_unfocused", Audio, Bool, "false", "audio"),
    setting!(
        "field_of_view",
        Gameplay,
        Range {
            min: 30.0,
            max: 150.0
        },
        "90",
        "renderer"
    ),
    setting!(
        "hud_scale",
        Gameplay,
        Range { min: 0.5, max: 2.0 },
        "1",
        "renderer"
    ),
    setting!(
        "hud_opacity",
        Gameplay,
        Range { min: 0.0, max: 1.0 },
        "1",
        "renderer"
    ),
    setting!("fps_counter", Gameplay, Bool, "false", "host"),
    setting!("frame_stats", Diagnostics, Bool, "false", "host"),
    setting!(
        "log_level",
        Diagnostics,
        Enum(&["error", "warn", "info", "debug", "trace"]),
        "info",
        "host"
    ),
    setting!("crash_reports", Diagnostics, Bool, "false", "host"),
];

impl Setting {
    pub fn default_value(&self) -> Value {
        match self.kind {
            Bool => json!(self.default == "true"),
            Range { .. } => json!(self.default.parse::<f64>().expect("numeric schema default")),
            _ => json!(self.default),
        }
    }
    /// Reject wrong types and non-enum values; clamp finite numeric values.
    pub fn validate(&self, value: Value) -> Option<Value> {
        match self.kind {
            Bool => value.as_bool().map(|v| json!(v)),
            Enum(options) => value
                .as_str()
                .filter(|v| options.contains(v))
                .map(|v| json!(v)),
            Range { min, max } => value
                .as_f64()
                .filter(|v| v.is_finite())
                .map(|v| json!(v.clamp(min, max))),
            List => value.as_str().filter(|v| !v.is_empty()).map(|v| json!(v)),
        }
    }
}

/// Host implementations consume only active settings; placeholders are never dispatched.
pub trait SettingsSink {
    fn changed(&mut self, setting: &Setting, value: &Value);
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    root: Map<String, Value>,
    values: Map<String, Value>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            root: Map::new(),
            values: SCHEMA
                .iter()
                .map(|s| (s.id.into(), s.default_value()))
                .collect(),
        }
    }
}
impl Settings {
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.values.get(id)
    }
    pub fn set(&mut self, id: &str, value: Value) -> bool {
        let Some(s) = SCHEMA.iter().find(|s| s.id == id) else {
            return false;
        };
        let Some(value) = s.validate(value) else {
            return false;
        };
        self.values.insert(id.into(), value);
        true
    }
    pub fn notify_changes(&self, previous: &Self, sink: &mut impl SettingsSink) {
        for s in SCHEMA {
            if s.status == Status::Active && self.get(s.id) != previous.get(s.id) {
                sink.changed(s, &self.values[s.id]);
            }
        }
    }
    /// All changed settings, including placeholders, for UI refresh and other observers.
    pub fn changes<'a>(
        &'a self,
        previous: &Self,
    ) -> impl Iterator<Item = (&'static Setting, &'a Value)> + 'a {
        let ids: Vec<_> = SCHEMA
            .iter()
            .filter(|s| self.get(s.id) != previous.get(s.id))
            .collect();
        ids.into_iter().map(|s| (s, &self.values[s.id]))
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut root: Map<String, Value> =
            serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let version = match root.remove("version") {
            None => 0,
            Some(value) => value.as_u64().ok_or("invalid settings version")?,
        };
        if version > VERSION {
            return Err("unsupported settings version".into());
        }
        let mut values = match root.remove("settings") {
            Some(Value::Object(v)) => v,
            None if version == 0 => Map::new(),
            _ => return Err("invalid settings object".into()),
        };
        // Version 0 stored settings at the top level. Preserve all unknown keys.
        if version == 0 {
            for s in SCHEMA {
                if let Some(v) = root.remove(s.id) {
                    values.insert(s.id.into(), v);
                }
            }
        }
        for s in SCHEMA {
            let v = values
                .remove(s.id)
                .and_then(|v| s.validate(v))
                .unwrap_or_else(|| s.default_value());
            values.insert(s.id.into(), v);
        }
        Ok(Self { root, values })
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut root = self.root.clone();
        root.insert("version".into(), json!(VERSION));
        root.insert("settings".into(), Value::Object(self.values.clone()));
        serde_json::to_vec_pretty(&root)
    }
    /// Atomically replace a user configuration file in its own directory.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        use std::io::Write;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent"))?;
        fs::create_dir_all(parent)?;
        let mut temp = None;
        for n in 0..1000 {
            let p = parent.join(format!(".tsfp-settings-{}-{n}.tmp", std::process::id()));
            match fs::OpenOptions::new().write(true).create_new(true).open(&p) {
                Ok(f) => {
                    temp = Some((p, f));
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        let (tmp, mut file) = temp.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "no temporary filename available",
            )
        })?;
        let result = (|| {
            file.write_all(&self.to_bytes().map_err(io::Error::other)?)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&tmp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(tmp);
        }
        result
    }
}
/// Resolve a user config location, never a game profile or disc path.
pub fn user_settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| Path::new(p).is_absolute())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    base.map(|p| p.join("tsfp").join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_valid_and_round_trip() {
        let settings = Settings::default();
        for s in SCHEMA {
            assert_eq!(
                s.validate(s.default_value()),
                Some(s.default_value()),
                "{}",
                s.id
            );
        }
        assert_eq!(
            Settings::parse(&settings.to_bytes().unwrap()).unwrap(),
            settings
        );
    }
    #[test]
    fn migration_clamps_and_preserves_unknown_keys() {
        let s = Settings::parse(br#"{"ui_scale":9,"future":42}"#).unwrap();
        assert_eq!(s.get("ui_scale"), Some(&json!(2.0)));
        let root: Value = serde_json::from_slice(&s.to_bytes().unwrap()).unwrap();
        assert_eq!(root["future"], 42);
        let s = Settings::parse(br#"{"version":1,"settings":{"future_setting":{"x":1}}}"#).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&s.to_bytes().unwrap()).unwrap()["settings"]["future_setting"]
                ["x"],
            1
        );
    }
    #[test]
    fn placeholders_do_not_notify_sink() {
        struct Counter(usize);
        impl SettingsSink for Counter {
            fn changed(&mut self, _: &Setting, _: &Value) {
                self.0 += 1;
            }
        }
        let old = Settings::default();
        let mut new = old.clone();
        assert!(new.set("ui_scale", json!(1.5)));
        let mut sink = Counter(0);
        new.notify_changes(&old, &mut sink);
        assert_eq!(sink.0, 0);
        assert_eq!(
            new.changes(&old).map(|(s, _)| s.id).collect::<Vec<_>>(),
            vec!["ui_scale"]
        );
    }
    #[test]
    fn range_boundaries_round_trip_and_versions_are_checked() {
        for s in SCHEMA {
            if let Kind::Range { min, max } = s.kind {
                for (input, expected) in [
                    (min - 1000.0, min),
                    (min, min),
                    ((min + max) / 2.0, (min + max) / 2.0),
                    (max, max),
                    (max + 1000.0, max),
                ] {
                    let mut settings = Settings::default();
                    assert!(settings.set(s.id, json!(input)));
                    assert_eq!(settings.get(s.id), Some(&json!(expected)), "{}", s.id);
                    assert_eq!(
                        Settings::parse(&settings.to_bytes().unwrap()).unwrap(),
                        settings
                    );
                }
            }
        }
        assert!(Settings::parse(br#"{"version":"2","settings":{}}"#).is_err());
        assert!(Settings::parse(br#"{"version":2,"settings":{}}"#).is_err());
    }
    #[test]
    fn save_replaces_existing_file() {
        let dir =
            std::env::temp_dir().join(format!("tsfp-settings-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut s = Settings::default();
        s.save(&path).unwrap();
        assert!(s.set("ui_scale", json!(1.5)));
        s.save(&path).unwrap();
        assert_eq!(Settings::parse(&std::fs::read(&path).unwrap()).unwrap(), s);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
