//! Optional content packs (higher-quality music, textures). Installed into
//! `<game dir>/assets/overrides/<pack id>/`, opt-in and labelled as differing from the original
//! (rule 56). Sources are placeholders until hosting is decided (T4334).
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Music,
    Textures,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pack {
    pub id: &'static str,
    pub kind: Kind,
    pub title: &'static str,
    pub description: &'static str,
    /// Signed pack manifest URL; `None` while the source is a placeholder.
    pub manifest_url: Option<&'static str>,
}

pub const CATALOG: &[Pack] = &[
    Pack {
        id: "hq-music",
        kind: Kind::Music,
        title: "High-quality music",
        description: "Higher bitrate soundtrack replacing the in-game streams. Placeholder: no source yet.",
        manifest_url: None,
    },
    Pack {
        id: "hq-textures",
        kind: Kind::Textures,
        title: "High-resolution textures",
        description: "Upscaled or remastered textures. Placeholder: no source yet.",
        manifest_url: None,
    },
];

pub fn pack_dir(game_dir: &Path, id: &str) -> PathBuf {
    game_dir.join("assets").join("overrides").join(id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackState {
    /// No download source yet.
    Unavailable,
    NotInstalled,
    Installed,
}

pub fn state(game_dir: &Path, pack: &Pack) -> PackState {
    if pack_dir(game_dir, pack.id).is_dir() {
        PackState::Installed
    } else if pack.manifest_url.is_none() {
        PackState::Unavailable
    } else {
        PackState::NotInstalled
    }
}
