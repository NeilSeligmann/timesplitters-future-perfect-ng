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
    if pack_dir(game_dir, pack.id).join("pack.json").is_file() {
        PackState::Installed
    } else if pack.manifest_url.is_none() {
        PackState::Unavailable
    } else {
        PackState::NotInstalled
    }
}

/// One file of a pack manifest. `path` is relative to the pack folder and uses `/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackFile {
    pub path: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

/// `pack.json` (schema 1), published with a minisign signature `pack.json.minisig`. The launcher verifies the
/// signature before trusting any URL or hash in it; the verified copy is kept as `<pack dir>/pack.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub files: Vec<PackFile>,
}

pub const MANIFEST_SCHEMA: u64 = 1;

fn safe_rel(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}

pub fn parse_manifest(text: &str) -> crate::Result<Manifest> {
    use serde_json::Value;
    let v: Value = serde_json::from_str(text).map_err(|e| format!("pack.json: {e}"))?;
    if v.get("schema").and_then(Value::as_u64) != Some(MANIFEST_SCHEMA) {
        return Err(format!("pack.json: schema must be {MANIFEST_SCHEMA}"));
    }
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).map(String::from);
    let id = s(&v, "id").ok_or("pack.json: missing id")?;
    if !CATALOG.iter().any(|p| p.id == id) {
        return Err(format!("pack.json: unknown pack {id:?}"));
    }
    let mut files = Vec::new();
    for f in v
        .get("files")
        .and_then(Value::as_array)
        .ok_or("pack.json: missing files")?
    {
        let path = s(f, "path").ok_or("pack.json: file without path")?;
        if !safe_rel(&path) {
            return Err(format!("pack.json: unsafe path {path:?}"));
        }
        let sha256 = s(f, "sha256").unwrap_or_default().to_ascii_lowercase();
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("pack.json: {path}: bad sha256"));
        }
        let url = s(f, "url").ok_or(format!("pack.json: {path}: missing url"))?;
        if !url.starts_with("https://") {
            return Err(format!("pack.json: {path}: url must be https"));
        }
        files.push(PackFile {
            size: f
                .get("size")
                .and_then(Value::as_u64)
                .ok_or(format!("pack.json: {path}: missing size"))?,
            path,
            url,
            sha256,
        });
    }
    Ok(Manifest {
        version: s(&v, "version").ok_or("pack.json: missing version")?,
        id,
        files,
    })
}

/// Files that are missing or whose size or hash differs: what an install or repair must download.
pub fn needed<'a>(game_dir: &Path, m: &'a Manifest) -> Vec<&'a PackFile> {
    let dir = pack_dir(game_dir, &m.id);
    m.files
        .iter()
        .filter(|f| match std::fs::read(dir.join(&f.path)) {
            Ok(b) => b.len() as u64 != f.size || tsfp_import::sha256::hex(&b) != f.sha256,
            Err(_) => true,
        })
        .collect()
}

/// Install `data` for one manifest entry after checking size and hash. Writes through a temporary file.
pub fn store(game_dir: &Path, m: &Manifest, f: &PackFile, data: &[u8]) -> crate::Result<()> {
    if data.len() as u64 != f.size || tsfp_import::sha256::hex(data) != f.sha256 {
        return Err(format!(
            "{}: download does not match the pack manifest",
            f.path
        ));
    }
    let dest = pack_dir(game_dir, &m.id).join(&f.path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let tmp = dest.with_extension("part");
    std::fs::write(&tmp, data).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &dest).map_err(|e| format!("{}: {e}", dest.display()))
}

/// Record the verified manifest so Verify and repair work offline.
pub fn commit(game_dir: &Path, m: &Manifest, manifest_text: &str) -> crate::Result<()> {
    let dir = pack_dir(game_dir, &m.id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join("pack.json"), manifest_text)
        .map_err(|e| format!("{}: {e}", dir.display()))
}

/// Installed manifest of a pack, if any.
pub fn installed(game_dir: &Path, id: &str) -> Option<Manifest> {
    parse_manifest(&std::fs::read_to_string(pack_dir(game_dir, id).join("pack.json")).ok()?).ok()
}

pub fn uninstall(game_dir: &Path, id: &str) -> crate::Result<()> {
    if !CATALOG.iter().any(|p| p.id == id) {
        return Err(format!("unknown pack {id:?}"));
    }
    match std::fs::remove_dir_all(pack_dir(game_dir, id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{id}: {e}")),
        _ => Ok(()),
    }
}
