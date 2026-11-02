//! Content pack install and repair (T4334): fetch `pack.json` and its `.minisig`, verify against the release key,
//! then download only missing or changed files, each checked against the manifest hash before it is stored.
use crate::updater;
use std::path::Path;
use tsfp_launcher_core::packs;

pub fn install(
    game_dir: &Path,
    manifest_url: &str,
    progress: &dyn Fn(String),
) -> Result<String, String> {
    let text = String::from_utf8(updater::get(manifest_url)?).map_err(|e| e.to_string())?;
    let sig = String::from_utf8(updater::get(&format!("{manifest_url}.minisig"))?)
        .map_err(|e| e.to_string())?;
    updater::verify(text.as_bytes(), &sig, updater::PUBLIC_KEY)?;
    let manifest = packs::parse_manifest(&text)?;
    let todo = packs::needed(game_dir, &manifest);
    let total = todo.len();
    for (i, f) in todo.into_iter().enumerate() {
        progress(format!("{}: {}/{total} {}", manifest.id, i + 1, f.path));
        packs::store(game_dir, &manifest, f, &updater::get(&f.url)?)?;
    }
    packs::commit(game_dir, &manifest, &text)?;
    Ok(format!(
        "{} {} installed ({total} files downloaded).",
        manifest.id, manifest.version
    ))
}
