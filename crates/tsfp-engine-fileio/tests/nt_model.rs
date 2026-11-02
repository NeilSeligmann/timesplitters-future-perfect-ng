//! Spec tests for the NT behaviour model and the minimal FATX reader (T2056). FABRICATED inputs.
#[allow(dead_code)]
mod synthetic;
use std::path::PathBuf;
use tsfp_engine_fileio::fatx::{Fatx, FatxError};
use tsfp_engine_fileio::nt::{InfoClass, NtFiles, SHARE_READ, SHARE_WRITE, mask_matches, nt_status};
use tsfp_engine_fileio::vfs::{Access, Disposition, Vfs, status};
use tsfp_engine_fileio::xiso::Xiso;

fn files(tag: &str) -> (NtFiles, PathBuf) {
    let dir = std::env::temp_dir().join(format!("tsfp-nt-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let iso = dir.join("a.iso");
    std::fs::write(&iso, synthetic::build(&synthetic::tiny_pak())).unwrap();
    (NtFiles::new(Vfs::standard(Xiso::open(&iso).unwrap(), &dir.join("hdd")).unwrap()), dir)
}

#[test]
fn wildcard_masks() {
    assert!(mask_matches("*", "anything.pak"));
    assert!(mask_matches("*.PAK", "l_1.pak"));
    assert!(mask_matches("l_?.pak", "L_1.PAK"));
    assert!(!mask_matches("l_?.pak", "l_12.pak"));
    assert!(mask_matches("a*b*c", "aXXbYYc"));
    assert!(!mask_matches("a*b", "abc"));
    assert!(mask_matches("*", ""));
}

#[test]
fn directory_enumeration_sorted_masked_restartable() {
    let (mut f, _d) = files("enum");
    let root = f.open("d:\\", Access::READ, 7, Disposition::Open).unwrap();
    let a = f.query_directory(root, false, None).unwrap();
    let b = f.query_directory(root, false, None).unwrap();
    assert_eq!((a.name.as_str(), b.name.as_str()), ("default.xbe", "pak"));
    assert_eq!(f.query_directory(root, false, None), Err(nt_status::NO_MORE_FILES));
    assert_eq!(f.query_directory(root, true, Some("p*")).unwrap().name, "pak");
    assert_eq!(f.query_directory(root, false, None), Err(nt_status::NO_MORE_FILES));
    assert_eq!(f.query_directory(root, true, Some("zz*")), Err(nt_status::NO_SUCH_FILE));
    let pak = f.open("d:\\pak", Access::READ, 7, Disposition::Open).unwrap();
    let e = f.query_directory(pak, false, Some("*.pak")).unwrap();
    assert_eq!((e.name.as_str(), e.is_directory), ("L_1.PAK", false));
    let file = f.open("d:\\default.xbe", Access::READ, 7, Disposition::Open).unwrap();
    assert_eq!(f.query_directory(file, false, None), Err(status::INVALID_PARAMETER));
}

#[test]
fn host_directory_enumeration() {
    let (mut f, d) = files("hostenum");
    for n in ["B.sav", "a.sav", "c.txt"] { std::fs::write(d.join("hdd").join(n), b"x").unwrap(); }
    let h = f.open("c:\\", Access::READ, 7, Disposition::Open).unwrap();
    let names: Vec<String> = std::iter::from_fn(|| f.query_directory(h, false, Some("*.sav")).ok()).map(|e| e.name).collect();
    assert_eq!(names, ["a.sav", "B.sav"]);
}

#[test]
fn information_classes() {
    let (mut f, _d) = files("info");
    let h = f.open("d:\\default.xbe", Access::READ, 7, Disposition::Open).unwrap();
    let std_info = f.query_information(h, InfoClass::Standard, 24).unwrap();
    assert_eq!(u64::from_le_bytes(std_info[0..8].try_into().unwrap()), 4096);
    assert_eq!(u64::from_le_bytes(std_info[8..16].try_into().unwrap()), 5);
    assert_eq!(std_info[21], 0);
    let mut buf = [0u8; 2];
    f.vfs.read(h, &mut buf).unwrap();
    assert_eq!(f.query_information(h, InfoClass::Position, 8).unwrap(), 2u64.to_le_bytes());
    let net = f.query_information(h, InfoClass::NetworkOpen, 56).unwrap();
    assert_eq!(net.len(), 56);
    assert_eq!(u64::from_le_bytes(net[40..48].try_into().unwrap()), 5);
    assert_eq!(f.query_information(h, InfoClass::Basic, 10), Err(nt_status::INFO_LENGTH_MISMATCH));
    assert_eq!(InfoClass::from_raw(99), Err(nt_status::INVALID_INFO_CLASS));
    let dir = f.open("d:\\pak", Access::READ, 7, Disposition::Open).unwrap();
    assert_eq!(f.query_information(dir, InfoClass::Standard, 24).unwrap()[21], 1);
}

#[test]
fn share_modes_conflict_and_release() {
    let (mut f, _d) = files("share");
    let w = f.open("c:\\a.sav", Access::READ_WRITE, SHARE_READ, Disposition::OpenIf).unwrap();
    // second reader allowed (writer shares read), second writer refused (writer denies write sharing)
    let r = f.open("C:\\A.SAV", Access::READ, SHARE_READ | SHARE_WRITE, Disposition::Open);
    assert!(r.is_ok());
    f.close(r.unwrap()).unwrap();
    assert_eq!(f.open("c:/a.sav", Access::READ_WRITE, SHARE_READ, Disposition::Open), Err(nt_status::SHARING_VIOLATION));
    // reader that denies write sharing conflicts with the existing writer
    assert_eq!(f.open("c:\\a.sav", Access::READ, SHARE_READ, Disposition::Open), Err(nt_status::SHARING_VIOLATION));
    f.close(w).unwrap();
    assert!(f.open("c:\\a.sav", Access::READ_WRITE, 0, Disposition::Open).is_ok());
    // different files never conflict
    assert!(f.open("c:\\b.sav", Access::READ_WRITE, 0, Disposition::OpenIf).is_ok());
}

#[test]
fn async_reads_complete_in_order_and_cancel_on_close() {
    let (mut f, _d) = files("async");
    let h = f.open("d:\\default.xbe", Access::READ, 7, Disposition::Open).unwrap();
    let a = f.submit_read(h, 0, 3).unwrap();
    let b = f.submit_read(h, 3, 8).unwrap();
    let c = f.submit_read(h, 9, 2).unwrap();
    assert_eq!(f.pending_count(), 3);
    let (ca, cb, cc) = (f.complete_next().unwrap(), f.complete_next().unwrap(), f.complete_next().unwrap());
    assert_eq!((ca.id, ca.data.as_slice()), (a, &b"XBE"[..]));
    assert_eq!((cb.id, cb.information, cb.data.as_slice()), (b, 2, &b"H!"[..]));
    assert_eq!((cc.id, cc.status), (c, status::END_OF_FILE));
    assert!(f.complete_next().is_none());
    f.submit_read(h, 0, 1).unwrap();
    f.close(h).unwrap();
    assert_eq!(f.pending_count(), 0);
    assert!(f.submit_read(h, 0, 1).is_err());
}

/// Build a FATX image: 512-byte sectors, 4 sectors per cluster (2 KiB), FAT16.
fn fatx_image() -> (Vec<u8>, Vec<u8>) {
    let cluster = 2048usize;
    let total = 0x1000 + 0x1000 + 64 * cluster;
    let mut img = vec![0u8; total];
    img[..4].copy_from_slice(b"FATX");
    img[8..12].copy_from_slice(&4u32.to_le_bytes());
    img[12..16].copy_from_slice(&1u32.to_le_bytes());
    let clusters = (total - 0x1000) / cluster; // FAT16 -> 2 bytes each, FAT area 0x1000
    assert!(clusters < 65_525);
    let data = 0x2000;
    let set_fat = |img: &mut Vec<u8>, c: usize, v: u16| img[0x1000 + c * 2..0x1000 + c * 2 + 2].copy_from_slice(&v.to_le_bytes());
    let entry = |name: &str, attr: u8, first: u32, size: u32| {
        let mut e = vec![0xFFu8; 64];
        e[0] = name.len() as u8; e[1] = attr;
        e[2..2 + name.len()].copy_from_slice(name.as_bytes());
        e[44..48].copy_from_slice(&first.to_le_bytes());
        e[48..52].copy_from_slice(&size.to_le_bytes());
        e
    };
    // cluster 1 root: Save (dir, cluster 2), Hello.txt (cluster 3-4, 3000 bytes); 5 is a deleted entry
    let payload: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    let mut root = Vec::new();
    root.extend(entry("Save", 0x10, 2, 0));
    root.extend(entry("Hello.txt", 0x20, 3, 3000));
    let mut deleted = entry("gone", 0x20, 6, 1); deleted[0] = 0xE5;
    root.extend(deleted);
    img[data..data + root.len()].copy_from_slice(&root);
    let sub = entry("Slot1.dat", 0x20, 5, 4);
    img[data + cluster..data + cluster + 64].copy_from_slice(&sub);
    img[data + 2 * cluster..data + 2 * cluster + 2048].copy_from_slice(&payload[..2048]);
    img[data + 3 * cluster..data + 3 * cluster + 952].copy_from_slice(&payload[2048..]);
    img[data + 4 * cluster..data + 4 * cluster + 4].copy_from_slice(b"SAVE");
    for (c, v) in [(1usize, 0xFFFF), (2, 0xFFFF), (3, 4), (4, 0xFFFF), (5, 0xFFFF)] { set_fat(&mut img, c, v); }
    (img, payload)
}

#[test]
fn fatx_lists_reads_and_rejects_corruption() {
    let (img, payload) = fatx_image();
    let size = img.len() as u64;
    let v = Fatx::open(img.clone(), size).unwrap();
    assert!(!v.is_fat32());
    let names: Vec<String> = v.read_dir("").unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["Save", "Hello.txt"]);
    let e = v.lookup("\\hello.TXT").unwrap();
    let mut buf = vec![0u8; 4000];
    assert_eq!(v.read(&e, 0, &mut buf).unwrap(), 3000);
    assert_eq!(&buf[..3000], &payload[..]);
    let mut part = [0u8; 10];
    assert_eq!(v.read(&e, 2045, &mut part).unwrap(), 10);
    assert_eq!(&part, &payload[2045..2055]);
    let mut s = [0u8; 8];
    let slot = v.lookup("save/slot1.dat").unwrap();
    assert_eq!(v.read(&slot, 0, &mut s).unwrap(), 4);
    assert_eq!(&s[..4], b"SAVE");
    assert_eq!(v.lookup("nope"), Err(FatxError::NotFound));
    assert_eq!(v.read_dir("hello.txt"), Err(FatxError::NotDirectory));
    let mut bad = img.clone();
    bad[..4].copy_from_slice(b"XXXX");
    assert!(matches!(Fatx::open(bad, size), Err(FatxError::Magic)));
    let mut looped = img;
    looped[0x1000 + 4 * 2..0x1000 + 4 * 2 + 2].copy_from_slice(&3u16.to_le_bytes()); // 3 -> 4 -> 3
    let v = Fatx::open(looped, size).unwrap();
    let mut buf = vec![0u8; 3000];
    assert_eq!(v.read(&e, 0, &mut buf), Err(FatxError::Chain));
}
