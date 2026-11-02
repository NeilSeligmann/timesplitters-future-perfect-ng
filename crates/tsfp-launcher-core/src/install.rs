//! Guided install: extract and convert a disc into the game folder, then verify it.
//! All work is done by `tsfp-import` (one extractor, rule 56); this module sequences it for the UI.
use crate::Result;
use crate::identify::{self, Identity};
use std::path::{Path, PathBuf};
use tsfp_import::{convert, sha256, source};

/// Install progress reported to the UI, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Identified(String),
    /// Copying the disc image into the game folder: bytes done of total.
    Copying {
        done: u64,
        total: u64,
    },
    /// The game folder holds a verified copy of the disc image (`reused`: an earlier copy still matched).
    Copied {
        bytes: u64,
        reused: bool,
    },
    /// Checking an existing install of this disc before extracting again.
    Checking {
        done: usize,
        total: usize,
    },
    /// Result of that check. `ok`: every file verified, so extraction and conversion are skipped.
    Checked {
        existing: bool,
        ok: bool,
        files: usize,
    },
    /// Files extracted from the disc so far.
    Extracting {
        done: usize,
        total: usize,
    },
    Extracted(usize),
    /// Source files examined for conversion so far.
    Converting {
        done: usize,
        total: usize,
    },
    Converted {
        outputs: usize,
        lossless: usize,
    },
    /// Files and outputs checked against the manifest so far.
    Verifying {
        done: usize,
        total: usize,
    },
    Verified(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    NotInstalled,
    /// Imported and its manifest verifies. `verified`: every file was re-hashed (not just the quick check).
    Installed {
        id: String,
        files: usize,
        verified: bool,
    },
    /// Imported but verification failed: offer Repair (reinstall from the disc).
    Damaged {
        id: String,
        reason: String,
    },
}

/// Folder holding the verified disc image copies, so later updates can re-read the original disc.
pub fn disc_dir(game_dir: &Path) -> PathBuf {
    game_dir.join("disc")
}

/// The verified disc image copy kept in the game folder for version `id`, if present.
pub fn disc_copy(game_dir: &Path, id: &str) -> Option<PathBuf> {
    let record = std::fs::read_to_string(disc_dir(game_dir).join(format!("{id}.json"))).ok()?;
    let v: serde_json::Value = serde_json::from_str(&record).ok()?;
    let copy = disc_dir(game_dir).join(v.get("file")?.as_str()?);
    copy.is_file().then_some(copy)
}

/// Size of a disc image file (`None` for an extracted folder, whose files the importer keeps anyway).
pub fn image_size(disc: &Path) -> Option<u64> {
    std::fs::metadata(disc)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
}

const CHUNK: usize = 8 << 20;

/// SHA-256 of a file, streamed. `on_progress(done, total)` after every chunk.
pub fn hash_file(path: &Path, on_progress: &mut dyn FnMut(u64, u64)) -> Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let total = f.metadata().map(|m| m.len()).unwrap_or(0);
    let (mut h, mut buf, mut done) = (sha256::Hasher::new(), vec![0u8; CHUNK], 0u64);
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        done += n as u64;
        on_progress(done, total);
    }
    Ok(h.finish_hex())
}

/// Copy `src` to `dst` through `dst.part`, then re-read the copy and require the same SHA-256 before it replaces
/// `dst`. Returns the hash. Progress counts the copy and the check (`total` = 2 x size).
pub fn copy_verified(
    src: &Path,
    dst: &Path,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<String> {
    use std::io::{Read, Write};
    let size = image_size(src).ok_or(format!("{}: not a file", src.display()))?;
    if let Some(dir) = dst.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let part = dst.with_extension("part");
    let mut input = std::fs::File::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let mut output =
        std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
    let (mut h, mut buf, mut done) = (sha256::Hasher::new(), vec![0u8; CHUNK], 0u64);
    loop {
        let n = input
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", src.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        output
            .write_all(&buf[..n])
            .map_err(|e| format!("{}: {e}", part.display()))?;
        done += n as u64;
        on_progress(done, 2 * size);
    }
    output
        .sync_all()
        .map_err(|e| format!("{}: {e}", part.display()))?;
    drop(output);
    let source_hash = h.finish_hex();
    let copy_hash = hash_file(&part, &mut |d, _| on_progress(size + d, 2 * size))?;
    if copy_hash != source_hash {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "{}: copy does not match the source (disk error?)",
            dst.display()
        ));
    }
    std::fs::rename(&part, dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    Ok(source_hash)
}

/// Make sure `<game dir>/disc/<id>.<ext>` is a verified copy of the image `src`, recorded in `<id>.json`.
/// An existing copy is reused when its size and recorded hash still match the source.
pub fn ensure_disc_copy(
    src: &Path,
    game_dir: &Path,
    id: &str,
    xbe_sha256: &str,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<(PathBuf, bool)> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("iso")
        .to_ascii_lowercase();
    let dir = disc_dir(game_dir);
    let copy = dir.join(format!("{id}.{ext}"));
    let record = dir.join(format!("{id}.json"));
    let size = image_size(src).ok_or(format!("{}: not a file", src.display()))?;
    if copy == src {
        return Ok((copy, true));
    }
    let recorded = std::fs::read_to_string(&record)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    let recorded_hash = recorded
        .as_ref()
        .and_then(|v| v.get("sha256")?.as_str().map(String::from));
    if let Some(hash) = recorded_hash
        && image_size(&copy) == Some(size)
        && hash_file(&copy, on_progress)? == hash
        && hash_file(src, on_progress)? == hash
    {
        return Ok((copy, true));
    }
    let hash = copy_verified(src, &copy, on_progress)?;
    let v = serde_json::json!({
        "schema": 1,
        "file": copy.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
        "size": size,
        "sha256": hash,
        "xbe_sha256": xbe_sha256,
        "original_name": src.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
    });
    std::fs::write(
        &record,
        serde_json::to_string_pretty(&v).unwrap_or_default() + "\n",
    )
    .map_err(|e| format!("{}: {e}", record.display()))?;
    Ok((copy, false))
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

/// Full status of every installed version: every file is re-hashed (a minute or more on a full disc). Run it
/// off the UI thread; use [`quick_status`] for startup and folder changes. An empty folder gives `NotInstalled`.
pub fn status(game_dir: &Path) -> Vec<Status> {
    status_progress(game_dir, &mut |_, _| {})
}

/// [`status`] reporting `(done, total)` checked items for the version being verified.
pub fn status_progress(game_dir: &Path, on_progress: &mut dyn FnMut(usize, usize)) -> Vec<Status> {
    let ids = installed_ids(game_dir);
    if ids.is_empty() {
        return vec![Status::NotInstalled];
    }
    ids.into_iter()
        .map(
            |id| match convert::verify_progress(game_dir, &id, on_progress) {
                Ok(files) => Status::Installed {
                    id,
                    files,
                    verified: true,
                },
                Err(reason) => Status::Damaged { id, reason },
            },
        )
        .collect()
}

/// Cheap status (milliseconds): the manifest exists and matches its recorded hash, and its rows are counted.
/// Files are not re-hashed, so `verified` is false.
pub fn quick_status(game_dir: &Path) -> Vec<Status> {
    let ids = installed_ids(game_dir);
    if ids.is_empty() {
        return vec![Status::NotInstalled];
    }
    ids.into_iter()
        .map(|id| {
            let dir = version_path(game_dir, &id);
            let manifest = std::fs::read(dir.join("manifest.json")).unwrap_or_default();
            let declared = std::fs::read_to_string(dir.join("manifest.sha256")).unwrap_or_default();
            if declared.split_whitespace().next() != Some(sha256::hex(&manifest).as_str()) {
                return Status::Damaged {
                    id,
                    reason: "manifest hash mismatch".into(),
                };
            }
            let files = manifest
                .split(|b| *b == b'\n')
                .filter(|l| {
                    let l = l.trim_ascii_start();
                    l.starts_with(b"{\"") && !l.starts_with(b"{\"schema\"")
                })
                .count();
            Status::Installed {
                id,
                files,
                verified: false,
            }
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
    // Disc images are copied (and verified) first and extracted from the copy, so the game folder always holds
    // the original disc for future updates. Extracted folders need no copy: stage A keeps their files verbatim.
    let import_from = if image_size(disc).is_some() {
        let xbe = ident.xbe_sha256.clone().unwrap_or_default();
        let (copy, reused) = ensure_disc_copy(disc, game_dir, id, &xbe, &mut |done, total| {
            on_step(&Step::Copying { done, total })
        })?;
        on_step(&Step::Copied {
            bytes: image_size(&copy).unwrap_or(0),
            reused,
        });
        copy
    } else {
        disc.to_path_buf()
    };
    // An existing install of this disc that still verifies is kept as is: no extraction or conversion.
    if version_path(game_dir, id).join("manifest.json").is_file() {
        match convert::verify_progress(game_dir, id, &mut |done, total| {
            on_step(&Step::Checking { done, total })
        }) {
            Ok(files) => {
                on_step(&Step::Checked {
                    existing: true,
                    ok: true,
                    files,
                });
                return Ok(ident);
            }
            Err(_) => on_step(&Step::Checked {
                existing: true,
                ok: false,
                files: 0,
            }),
        }
    } else {
        on_step(&Step::Checked {
            existing: false,
            ok: false,
            files: 0,
        });
    }
    let src = source::open(&import_from, id, source::Platform::Xbox)?;
    let files = convert::stage_a_progress(src.as_ref(), game_dir, "", &mut |done, total| {
        on_step(&Step::Extracting { done, total })
    })?;
    on_step(&Step::Extracted(files.len()));
    let (outs, _) =
        convert::stage_b_report_progress(src.as_ref(), game_dir, "", &[], &mut |done, total| {
            on_step(&Step::Converting { done, total })
        })?;
    convert::write_override_contract(game_dir)?;
    on_step(&Step::Converted {
        outputs: outs.len(),
        lossless: outs.iter().filter(|o| o.lossless).count(),
    });
    let verified = convert::verify_progress(game_dir, id, &mut |done, total| {
        on_step(&Step::Verifying { done, total })
    })?;
    on_step(&Step::Verified(verified));
    Ok(ident)
}
