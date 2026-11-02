//! Mods: folders under `<game dir>/mods/<id>/` with a `mod.json` and a `files/` tree of asset
//! overrides keyed by logical id (`<category>/<name>`, as in the importer's overlay contract).
//! Mods that touch gameplay-read data must say so and are labelled non-faithful (rules 50/51).
use crate::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SCHEMA: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    /// Version ids (`xbox-usa`, ...) the mod supports; empty means any.
    pub game_versions: Vec<String>,
    pub affects_gameplay: bool,
    pub dir: PathBuf,
    /// Logical ids provided under `files/`, sorted.
    pub files: Vec<String>,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Parse and validate one `mod.json`. `dir_name` must equal the declared id.
pub fn parse(text: &str, dir: &Path) -> Result<ModInfo> {
    let where_ = dir.join("mod.json").display().to_string();
    let v: Value = serde_json::from_str(text).map_err(|e| format!("{where_}: {e}"))?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
    let schema = v.get("schema").and_then(Value::as_u64);
    if schema != Some(SCHEMA) {
        return Err(format!("{where_}: schema must be {SCHEMA}"));
    }
    let id = s("id").ok_or(format!("{where_}: missing id"))?;
    if !valid_id(&id) {
        return Err(format!("{where_}: id {id:?} must be 1-64 of [a-z0-9_-]"));
    }
    if dir.file_name().and_then(|n| n.to_str()) != Some(id.as_str()) {
        return Err(format!("{where_}: folder name must equal id {id:?}"));
    }
    Ok(ModInfo {
        name: s("name").unwrap_or_else(|| id.clone()),
        version: s("version").ok_or(format!("{where_}: missing version"))?,
        description: s("description").unwrap_or_default(),
        game_versions: v
            .get("game_versions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        affects_gameplay: v
            .get("affects_gameplay")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        files: list_files(&dir.join("files"))?,
        dir: dir.to_path_buf(),
        id,
    })
}

fn list_files(root: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(root) {
                let parts: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.push(parts.join("/"));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Every mod under `mods_dir`, sorted by id. Broken mods are returned as errors, not skipped.
pub fn scan(mods_dir: &Path) -> (Vec<ModInfo>, Vec<String>) {
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    let Ok(rd) = std::fs::read_dir(mods_dir) else {
        return (ok, bad);
    };
    for e in rd.flatten().filter(|e| e.path().is_dir()) {
        let manifest = e.path().join("mod.json");
        match std::fs::read_to_string(&manifest) {
            Ok(t) => match parse(&t, &e.path()) {
                Ok(m) => ok.push(m),
                Err(err) => bad.push(err),
            },
            Err(err) => bad.push(format!("{}: {err}", manifest.display())),
        }
    }
    ok.sort_by(|a, b| a.id.cmp(&b.id));
    bad.sort();
    (ok, bad)
}

/// A file provided by more than one enabled mod: the last in load order wins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub file: String,
    pub winner: String,
    pub shadowed: Vec<String>,
}

/// Resolve enabled mods in load order. Unknown ids are errors so a stale config is visible.
pub fn resolve(mods: &[ModInfo], load_order: &[String]) -> Result<Vec<Conflict>> {
    let mut owners: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for id in load_order {
        let m = mods
            .iter()
            .find(|m| &m.id == id)
            .ok_or(format!("enabled mod {id:?} is not installed"))?;
        for f in &m.files {
            owners.entry(f).or_default().push(&m.id);
        }
    }
    Ok(owners
        .into_iter()
        .filter(|(_, o)| o.len() > 1)
        .map(|(f, mut o)| {
            let winner = o.pop().unwrap_or_default().to_string();
            Conflict {
                file: f.to_string(),
                winner,
                shadowed: o.into_iter().map(String::from).collect(),
            }
        })
        .collect())
}
