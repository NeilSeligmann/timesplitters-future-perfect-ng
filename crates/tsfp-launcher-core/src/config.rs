//! Launcher configuration (`launcher.json` beside the game settings in `<config dir>/tsfp/`). Game settings stay in the
//! `tsfp-settings` file so the launcher and the in-game menu edit one source of truth (T4332).
use crate::Result;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Nightly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub game_dir: PathBuf,
    pub channel: Channel,
    pub check_updates: bool,
    /// Mod ids in load order; later entries win conflicts.
    pub enabled_mods: Vec<String>,
    pub enabled_packs: Vec<String>,
    /// Disc image or folder used last (after an install: the verified copy inside the game folder).
    pub last_disc: Option<String>,
}

impl Config {
    pub fn with_game_dir(game_dir: PathBuf) -> Self {
        Config {
            game_dir,
            channel: Channel::Stable,
            check_updates: true,
            enabled_mods: Vec::new(),
            enabled_packs: Vec::new(),
            last_disc: None,
        }
    }

    pub fn to_json(&self) -> String {
        let v = json!({
            "schema": 1,
            "game_dir": self.game_dir.to_string_lossy(),
            "channel": match self.channel { Channel::Stable => "stable", Channel::Nightly => "nightly" },
            "check_updates": self.check_updates,
            "enabled_mods": self.enabled_mods,
            "enabled_packs": self.enabled_packs,
            "last_disc": self.last_disc,
        });
        serde_json::to_string_pretty(&v).unwrap_or_default() + "\n"
    }

    /// Unknown keys are ignored and missing keys take defaults, so older files keep loading.
    pub fn from_json(text: &str, default_game_dir: PathBuf) -> Result<Self> {
        let v: Value = serde_json::from_str(text).map_err(|e| format!("launcher.json: {e}"))?;
        let empty = Map::new();
        let o = v.as_object().unwrap_or(&empty);
        let strings = |k: &str| -> Vec<String> {
            o.get(k)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        };
        let channel = match o.get("channel").and_then(Value::as_str) {
            None | Some("stable") => Channel::Stable,
            Some("nightly") => Channel::Nightly,
            Some(other) => return Err(format!("launcher.json: unknown channel {other:?}")),
        };
        Ok(Config {
            game_dir: o
                .get("game_dir")
                .and_then(Value::as_str)
                .map_or(default_game_dir, PathBuf::from),
            channel,
            check_updates: o
                .get("check_updates")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            enabled_mods: strings("enabled_mods"),
            enabled_packs: strings("enabled_packs"),
            last_disc: o
                .get("last_disc")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from),
        })
    }

    pub fn load(path: &Path, default_game_dir: PathBuf) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_json(&text, default_game_dir),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::with_game_dir(default_game_dir))
            }
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Write through a temporary file so a crash never leaves a half-written config.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, self.to_json()).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Per-user config directory: `$XDG_CONFIG_HOME` or `~/.config` on Linux,
/// `~/Library/Application Support` on macOS, `%APPDATA%` on Windows.
pub fn default_config_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(target_os = "windows") {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library").join("Application Support"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))
    };
    base.map(|b| b.join("tsfp"))
}

/// Default game folder, in the per-user data directory: `$XDG_DATA_HOME` or `~/.local/share` on Linux,
/// `~/Library/Application Support` on macOS, `%LOCALAPPDATA%` on Windows. Never the config directory.
pub fn default_game_dir() -> PathBuf {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(target_os = "windows") {
        env("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library").join("Application Support"))
    } else {
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|h| h.join(".local").join("share")))
    };
    base.map_or_else(|| PathBuf::from("game"), |b| b.join("tsfp").join("game"))
}
