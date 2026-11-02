//! Game logo for the launcher, read from the user's own installed game data (never shipped: the artwork is
//! the rights holder's). Before setup the launcher shows a text title instead.
use crate::install;
use std::path::{Path, PathBuf};

/// Texture key of the "TimeSplitters Future Perfect" logo in the Xbox release (1024x512, BC with alpha).
/// It is stored in several arcade level archives; any copy will do.
pub const LOGO_KEY: &str = "3edb3999";

/// RGBA8 image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Candidate logo textures under every installed version, in a stable order.
pub fn logo_candidates(game_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for id in install::installed_ids(game_dir) {
        let arcade = install::version_path(game_dir, &id).join("textures/pak/arcade");
        let Ok(levels) = std::fs::read_dir(&arcade) else {
            continue;
        };
        let mut levels: Vec<PathBuf> = levels.flatten().map(|e| e.path()).collect();
        levels.sort();
        out.extend(
            levels
                .into_iter()
                .map(|l| l.join(format!("{LOGO_KEY}.ktx2")))
                .filter(|p| p.is_file()),
        );
    }
    out
}

/// The decoded logo, if the game is installed and the texture looks like the logo (2:1, at least 256 wide).
pub fn logo(game_dir: &Path) -> Option<Image> {
    logo_candidates(game_dir).into_iter().find_map(|path| {
        let bytes = std::fs::read(path).ok()?;
        let (width, height, rgba) = tsfp_import::ktx2::decode_rgba(&bytes).ok()?;
        (width >= 256 && width == 2 * height).then_some(Image {
            width,
            height,
            rgba,
        })
    })
}
