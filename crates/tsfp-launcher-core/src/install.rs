//! Guided install: extract and convert a disc into the game folder, then verify it.
//! All work is done by `tsfp-import` (one extractor, rule 56); this module sequences it for the UI.
use crate::Result;
use crate::identify::{self, Identity};
use std::path::{Path, PathBuf};
use tsfp_import::{convert, source};

/// Install progress reported to the UI, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Identified(String),
    Extracted(usize),
    Converted { outputs: usize, lossless: usize },
    Verified(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    NotInstalled,
    /// Imported and its manifest verifies.
    Installed {
        id: String,
        files: usize,
    },
    /// Imported but verification failed: offer Repair (reinstall from the disc).
    Damaged {
        id: String,
        reason: String,
    },
}

pub fn version_path(game_dir: &Path, id: &str) -> PathBuf {
    game_dir.join("assets").join("original").join(id)
}

/// Version ids present under `assets/original/`, sorted.
pub fn installed_ids(game_dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(game_dir.join("assets").join("original"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("manifest.json").is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    ids.sort();
    ids
}

/// Status of every installed version (empty folder gives one `NotInstalled`).
pub fn status(game_dir: &Path) -> Vec<Status> {
    let ids = installed_ids(game_dir);
    if ids.is_empty() {
        return vec![Status::NotInstalled];
    }
    ids.into_iter()
        .map(|id| match convert::verify(game_dir, &id) {
            Ok(files) => Status::Installed { id, files },
            Err(reason) => Status::Damaged { id, reason },
        })
        .collect()
}

/// Identify, extract, convert and verify `disc` into `game_dir`. Unknown dumps are refused unless
/// `allow_unknown` (the wizard asks the user first).
pub fn install(
    disc: &Path,
    game_dir: &Path,
    allow_unknown: bool,
    on_step: &mut dyn FnMut(&Step),
) -> Result<Identity> {
    let ident = identify::identify(disc)?;
    if ident.xbe_sha256.is_none() || (ident.known.is_none() && !allow_unknown) {
        return Err(ident.verdict());
    }
    on_step(&Step::Identified(ident.verdict()));
    let id = ident.version_id();
    let src = source::open(disc, id, source::Platform::Xbox)?;
    let files = convert::stage_a(src.as_ref(), game_dir, "")?;
    on_step(&Step::Extracted(files.len()));
    let (outs, _) = convert::stage_b_report(src.as_ref(), game_dir, "", &[])?;
    convert::write_override_contract(game_dir)?;
    on_step(&Step::Converted {
        outputs: outs.len(),
        lossless: outs.iter().filter(|o| o.lossless).count(),
    });
    let verified = convert::verify(game_dir, id)?;
    on_step(&Step::Verified(verified));
    Ok(ident)
}
