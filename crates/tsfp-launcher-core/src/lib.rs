//! Launcher logic shared by the GUI (T4332) and the headless CLI. No UI code here.
//! Decision: docs/decisions/launcher.md. The game folder layout is owned by `tsfp-import`.
pub mod branding;
pub mod config;
pub mod game;
pub mod identify;
pub mod install;
pub mod mods;
pub mod packs;
pub mod update;

pub type Result<T> = std::result::Result<T, String>;
