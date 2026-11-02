//! Mod archives: unpack a `.zip` into a temporary folder, then hand it to `mods::install_dir`.
//! Entries that would escape the folder (absolute paths, `..`) are refused.
use std::path::{Path, PathBuf};
use tsfp_launcher_core::mods::{self, ModInfo};

pub fn unpack(zip_path: &Path, into: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("{}: {e}", zip_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", zip_path.display()))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let rel = entry.enclosed_name().ok_or_else(|| {
            format!(
                "unsafe path in archive: {}",
                String::from_utf8_lossy(entry.name_raw())
            )
        })?;
        let out = into.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut f = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        std::io::copy(&mut entry, &mut f).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Install a mod from a `.zip` or a folder.
pub fn install(src: &Path, mods_dir: &Path) -> Result<ModInfo, String> {
    let is_zip = src
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if !is_zip {
        return mods::install_dir(src, mods_dir);
    }
    let tmp: PathBuf = std::env::temp_dir().join(format!("tsfp-mod-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let result = unpack(src, &tmp).and_then(|()| mods::install_dir(&tmp, mods_dir));
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tsfp-modzip-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn zip_with(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, body) in entries {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(body).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn zip_with_wrapping_folder_installs() {
        let d = scratch("ok");
        let z = d.join("m.zip");
        zip_with(
            &z,
            &[
                (
                    "delta/mod.json",
                    br#"{"schema":1,"id":"delta","version":"1"}"#,
                ),
                ("delta/files/textures/a.ktx2", b"x"),
            ],
        );
        let m = install(&z, &d.join("mods")).unwrap();
        assert_eq!(
            (m.id.as_str(), m.files.as_slice()),
            ("delta", &["textures/a.ktx2".to_string()][..])
        );
    }

    #[test]
    fn zip_slip_is_refused() {
        let d = scratch("slip");
        let z = d.join("evil.zip");
        zip_with(&z, &[("../escaped.txt", b"x")]);
        assert!(
            install(&z, &d.join("mods"))
                .unwrap_err()
                .contains("unsafe path")
        );
        assert!(!d.join("escaped.txt").exists());
    }
}
