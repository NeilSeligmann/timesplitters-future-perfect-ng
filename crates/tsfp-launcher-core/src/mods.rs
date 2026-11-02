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
    let visible = |e: &std::fs::DirEntry| !e.file_name().to_string_lossy().starts_with('.');
    for e in rd.flatten().filter(|e| e.path().is_dir() && visible(e)) {
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

/// Install a mod from an unpacked folder (the folder holding `mod.json`, or its only subfolder) into
/// `mods_dir/<id>/`. Replaces an existing install of the same id. Returns the installed mod.
pub fn install_dir(src: &Path, mods_dir: &Path) -> Result<ModInfo> {
    let root = if src.join("mod.json").is_file() {
        src.to_path_buf()
    } else {
        let subdirs: Vec<PathBuf> = std::fs::read_dir(src)
            .map_err(|e| format!("{}: {e}", src.display()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        match &subdirs[..] {
            [only] if only.join("mod.json").is_file() => only.clone(),
            _ => return Err(format!("{}: no mod.json found", src.display())),
        }
    };
    let text = std::fs::read_to_string(root.join("mod.json"))
        .map_err(|e| format!("{}: {e}", root.display()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("mod.json: {e}"))?;
    let id = v
        .get("id")
        .and_then(Value::as_str)
        .ok_or("mod.json: missing id")?
        .to_string();
    if !valid_id(&id) {
        return Err(format!("mod.json: id {id:?} must be 1-64 of [a-z0-9_-]"));
    }
    let dest = mods_dir.join(&id);
    let staging = mods_dir.join(format!(".{id}.installing"));
    let _ = std::fs::remove_dir_all(&staging);
    copy_tree(&root, &staging)?;
    // Validate the staged copy under its final name before replacing anything.
    let staged = parse(&text, &dest).map(|mut m| {
        m.files = list_files(&staging.join("files")).unwrap_or_default();
        m
    });
    if let Err(e) = staged {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&staging, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    parse(&text, &dest)
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    for e in std::fs::read_dir(src)
        .map_err(|e| format!("{}: {e}", src.display()))?
        .flatten()
    {
        let (from, to) = (e.path(), dst.join(e.file_name()));
        let kind = e.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err(format!(
                "{}: symlinks are not allowed in mods",
                from.display()
            ));
        } else if kind.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
        }
    }
    Ok(())
}

pub fn uninstall(mods_dir: &Path, id: &str) -> Result<()> {
    if !valid_id(id) {
        return Err(format!("bad mod id {id:?}"));
    }
    std::fs::remove_dir_all(mods_dir.join(id)).map_err(|e| format!("{id}: {e}"))
}

/// Move `id` one place earlier (`-1`) or later (`+1`) in the load order. Later entries win conflicts.
pub fn move_in_order(order: &mut [String], id: &str, delta: isize) {
    if let Some(i) = order.iter().position(|m| m == id) {
        let j = i as isize + delta;
        if j >= 0 && (j as usize) < order.len() {
            order.swap(i, j as usize);
        }
    }
}

/// Write `<game dir>/mods/active.json`: enabled mods in load order and the winning mod for every
/// overridden file. The game runtime reads this to build its overlay; it never rescans mod folders.
pub fn write_active(mods_dir: &Path, mods: &[ModInfo], load_order: &[String]) -> Result<()> {
    resolve(mods, load_order)?;
    let mut winners: BTreeMap<&str, &str> = BTreeMap::new();
    let mut gameplay = false;
    for id in load_order {
        if let Some(m) = mods.iter().find(|m| &m.id == id) {
            gameplay |= m.affects_gameplay;
            for f in &m.files {
                winners.insert(f, &m.id);
            }
        }
    }
    let v = serde_json::json!({
        "schema": SCHEMA,
        "load_order": load_order,
        "affects_gameplay": gameplay,
        "files": winners,
    });
    std::fs::create_dir_all(mods_dir).map_err(|e| format!("{}: {e}", mods_dir.display()))?;
    let path = mods_dir.join("active.json");
    let tmp = mods_dir.join("active.json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_string_pretty(&v).unwrap_or_default() + "\n",
    )
    .map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}
