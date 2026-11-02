//! Tier 0 native persistence/migration contract, fabricated data only.
use std::path::{Path, PathBuf};
use tsfp_engine_fileio::{
    fatx::Fatx,
    fatx_img,
    local_storage::{
        DirectorySource, FatxSource, LOCAL_DEVICE_ID, LocalStorage, SaveSource, standard_path,
    },
    save::{load_profile, save_profile},
};
use tsfp_format_profile::Profile;

fn scratch(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("tsfp-local-storage-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn xdg_absolute_path_and_single_device_profile_reload() {
    assert_eq!(
        standard_path(Some(Path::new("<private>")), None).unwrap(),
        Path::new("<private>")
    );
    assert_eq!(
        standard_path(None, Some(Path::new("/data"))).unwrap(),
        Path::new("/data/tsfp/saves")
    );
    assert_eq!(
        standard_path(Some(Path::new("<private>")), Some(Path::new("relative"))).unwrap(),
        Path::new("<private>")
    );
    assert!(standard_path(None, Some(Path::new("relative"))).is_err());
    let root = scratch("profile");
    let storage = LocalStorage::open(root.clone()).unwrap();
    assert_eq!(storage.devices().len(), 1);
    assert_eq!(storage.device(LOCAL_DEVICE_ID).unwrap().root, root);
    assert!(storage.device(10).is_none());
    let mut vfs = storage.vfs().unwrap();
    let mut profile = Profile {
        payload: vec![0; 0x1f54],
        signature: [23; 20],
    };
    profile.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    save_profile(&mut vfs, "U:\\profile", &profile).unwrap();
    drop(vfs);
    drop(storage);
    let storage = LocalStorage::open(root.clone()).unwrap();
    let loaded = load_profile(&mut storage.vfs().unwrap(), "\\??\\U:\\profile").unwrap();
    assert_eq!(loaded.payload, profile.payload);
    assert_eq!(loaded.signature, profile.signature);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn directory_migration_is_idempotent_and_retains_conflicting_saves() {
    let src = scratch("source");
    let dst = scratch("dest");
    let save = src.join("udata/45410066/Synthetic");
    std::fs::create_dir_all(&save).unwrap();
    std::fs::write(src.join("udata/45410066/TitleMeta.xbx"), b"synthetic title").unwrap();
    std::fs::write(save.join("SaveMeta.xbx"), b"synthetic metadata").unwrap();
    std::fs::write(save.join("map.mkr"), b"synthetic map").unwrap();
    let storage = LocalStorage::open(dst.clone()).unwrap();
    let source = DirectorySource(src.clone());
    assert_eq!(storage.migrate(&source).unwrap().imported.len(), 2);
    assert_eq!(storage.migrate(&source).unwrap().already_present.len(), 2);
    std::fs::write(save.join("map.mkr"), b"changed source").unwrap();
    assert_eq!(storage.migrate(&source).unwrap().conflicts.len(), 1);
    assert_eq!(
        std::fs::read(dst.join("UDATA/45410066/synthetic/map.mkr")).unwrap(),
        b"synthetic map"
    );
    assert_eq!(
        std::fs::read(save.join("map.mkr")).unwrap(),
        b"changed source"
    );
    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dst).unwrap();
}

#[cfg(unix)]
#[test]
fn directory_migration_rejects_symlinks_without_publishing_a_partial_save() {
    use std::os::unix::fs::symlink;

    let src = scratch("symlink-source");
    let dst = scratch("symlink-dest");
    let save = src.join("udata/45410066/Synthetic");
    std::fs::create_dir_all(&save).unwrap();
    std::fs::write(save.join("SaveMeta.xbx"), b"safe metadata").unwrap();
    let outside = src.join("outside.bin");
    std::fs::write(&outside, b"outside data").unwrap();
    symlink(&outside, save.join("linked.bin")).unwrap();

    let storage = LocalStorage::open(dst.clone()).unwrap();
    let error = storage.migrate(&DirectorySource(src.clone())).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("symlink"));
    assert!(!dst.join("UDATA/45410066/synthetic").exists());
    assert_eq!(std::fs::read(outside).unwrap(), b"outside data");

    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dst).unwrap();
}

#[test]
fn directory_migration_rejects_overdeep_save_trees_before_publication() {
    let src = scratch("deep-source");
    let dst = scratch("deep-dest");
    let mut nested = src.join("udata/45410066/Synthetic");
    for depth in 0..18 {
        nested.push(format!("d{depth}"));
        std::fs::create_dir_all(&nested).unwrap();
    }
    std::fs::write(nested.join("map.mkr"), b"synthetic map").unwrap();

    let storage = LocalStorage::open(dst.clone()).unwrap();
    let error = storage.migrate(&DirectorySource(src.clone())).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("too deep"));
    assert!(!dst.join("UDATA/45410066/synthetic").exists());

    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dst).unwrap();
}

struct InvalidSource(Vec<(std::path::PathBuf, Vec<u8>)>);
impl SaveSource for InvalidSource {
    fn title_files(&self, namespace: &str) -> std::io::Result<Vec<(std::path::PathBuf, Vec<u8>)>> {
        Ok(if namespace == "UDATA" {
            self.0.clone()
        } else {
            Vec::new()
        })
    }
}

struct InvalidTdataSource;
impl SaveSource for InvalidTdataSource {
    fn title_files(&self, namespace: &str) -> std::io::Result<Vec<(std::path::PathBuf, Vec<u8>)>> {
        Ok(if namespace == "UDATA" {
            vec![("Profile".into(), b"valid UDATA".to_vec())]
        } else {
            vec![("../escape".into(), b"invalid TDATA".to_vec())]
        })
    }
}

#[test]
fn migration_validates_both_namespaces_before_publishing_either() {
    let root = scratch("invalid-tdata");
    let storage = LocalStorage::open(root.clone()).unwrap();
    let error = storage.migrate(&InvalidTdataSource).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(!root.join("UDATA/45410066").exists());
    assert!(!root.join("TDATA/45410066").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn migration_rejects_traversal_and_case_collisions_before_any_publication() {
    let root = scratch("invalid-source");
    let storage = LocalStorage::open(root.clone()).unwrap();
    for entries in [
        vec![(std::path::PathBuf::from("../escape/file"), b"x".to_vec())],
        vec![
            (std::path::PathBuf::from("Save/file"), b"x".to_vec()),
            (std::path::PathBuf::from("save/other"), b"y".to_vec()),
        ],
        vec![
            (std::path::PathBuf::from("Save"), b"x".to_vec()),
            (std::path::PathBuf::from("save/child"), b"y".to_vec()),
        ],
    ] {
        let error = storage.migrate(&InvalidSource(entries)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(!root.join("UDATA/45410066").exists());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn raw_fatx_migration_reads_title_only_without_modifying_image() {
    let mut image = vec![0; 4 * 1024 * 1024];
    assert_eq!(fatx_img::format(&mut image, 42), 0);
    let g = fatx_img::open(&image).unwrap();
    for path in [
        b"UDATA".as_slice(),
        b"UDATA/45410066",
        b"UDATA/45410066/test",
    ] {
        assert_eq!(fatx_img::mkdir_path(&g, &mut image, path), 0);
    }
    assert_eq!(
        fatx_img::write_path(
            &g,
            &mut image,
            b"UDATA/45410066/test/map.mkr",
            b"synthetic map"
        ),
        0
    );
    let original = image.clone();
    let fatx = Fatx::open(image, original.len() as u64).unwrap();
    let dst = scratch("fatx");
    let storage = LocalStorage::open(dst.clone()).unwrap();
    assert_eq!(
        storage.migrate(&FatxSource(fatx)).unwrap().imported.len(),
        1
    );
    assert_eq!(
        std::fs::read(dst.join("UDATA/45410066/test/map.mkr")).unwrap(),
        b"synthetic map"
    );
    std::fs::remove_dir_all(dst).unwrap();
}
