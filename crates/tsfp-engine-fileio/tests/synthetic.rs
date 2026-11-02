//! Synthetic-image tests: no owner data. Image built in memory, written to a temp file.
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;
use tsfp_engine_fileio::assets::{AssetKey, AssetLoader};
use tsfp_engine_fileio::pak::PakIndex;
use tsfp_engine_fileio::save::{PROFILE_FILE_BYTES, load_profile, save_profile};
use tsfp_engine_fileio::vfs::{Access, Disposition, Vfs, status};
use tsfp_engine_fileio::xiso::{Xiso, XisoError};

fn dirent(next_link: u16, sector: u32, size: u32, attr: u8, name: &str) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend(next_link.to_le_bytes());
    v.extend(0u16.to_le_bytes()); // right: nil is 0 as on the real disc
    v.extend(sector.to_le_bytes());
    v.extend(size.to_le_bytes());
    v.extend([attr, name.len() as u8]);
    v.extend(name.as_bytes());
    while v.len() % 4 != 0 { v.push(0xFF); }
    v
}
/// Left-chained directory blob (each node's left link is the next node, last is nil 0).
fn dir_blob(items: &[(&str, u32, u32, u8)]) -> Vec<u8> {
    let sizes: Vec<usize> = items.iter().map(|i| (14 + i.0.len()).div_ceil(4) * 4).collect();
    let mut blob = Vec::new();
    let mut off = 0;
    for (i, it) in items.iter().enumerate() {
        off += sizes[i];
        let link = if i + 1 < items.len() { (off / 4) as u16 } else { 0 };
        blob.extend(dirent(link, it.1, it.2, it.3, it.0));
    }
    blob
}
fn put_sector(img: &mut Vec<u8>, sector: u32, data: &[u8]) {
    let end = sector as usize * 2048 + data.len();
    if img.len() < end.div_ceil(2048) * 2048 { img.resize(end.div_ceil(2048) * 2048, 0xFF); }
    img[sector as usize * 2048..end].copy_from_slice(data);
}
pub fn build(pak_bytes: &[u8]) -> Vec<u8> {
    let mut img = vec![0u8; 33 * 2048];
    let root = dir_blob(&[("pak", 40, 0, 0x10), ("default.xbe", 41, 5, 0x20)]);
    let sub = dir_blob(&[("L_1.PAK", 42, pak_bytes.len() as u32, 0x20)]);
    let mut desc = vec![0u8; 2048];
    desc[..20].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
    desc[0x14..0x18].copy_from_slice(&39u32.to_le_bytes());
    desc[0x18..0x1C].copy_from_slice(&(root.len() as u32).to_le_bytes());
    desc[0x7EC..0x7EC + 20].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
    put_sector(&mut img, 32, &desc);
    put_sector(&mut img, 39, &root);
    // "pak" directory entry has size 0 above; give it the real blob size.
    let mut root2 = root.clone();
    root2[8..12].copy_from_slice(&(sub.len() as u32).to_le_bytes());
    put_sector(&mut img, 39, &root2);
    put_sector(&mut img, 40, &sub);
    put_sector(&mut img, 41, b"XBEH!");
    put_sector(&mut img, 42, pak_bytes);
    img
}
pub fn tiny_pak() -> Vec<u8> {
    let mut b = b"P5CK".to_vec();
    b.extend(16u32.to_le_bytes()); // toc offset
    b.extend(32u32.to_le_bytes()); // two entries
    b.extend(b"abcd");
    for (key, off) in [(7u32, 12u32), (9, 14)] { b.extend([key, off, 2, 0].iter().flat_map(|w| w.to_le_bytes())); }
    b
}
fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tsfp-fileio-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
fn mounted(tag: &str) -> (Vfs, PathBuf) {
    let dir = temp(tag);
    let iso = dir.join("a.iso");
    std::fs::write(&iso, build(&tiny_pak())).unwrap();
    (Vfs::standard(Xiso::open(&iso).unwrap(), &dir.join("hdd")).unwrap(), dir)
}

#[test]
fn asset_loader_selects_entries_and_releases_handles() {
    let (vfs, _dir) = mounted("assets");
    let mut loader = AssetLoader::new(vfs);
    let first = AssetKey { key: 7, occurrence: 0 };
    assert_eq!(loader.stored("D:\\pak\\l_1.pak", first).unwrap(), b"ab");
    assert_eq!(loader.vfs().open_count(), 0);
    assert_eq!(loader.stored("D:\\pak\\l_1.pak", AssetKey { key: 7, occurrence: 1 }),
        Err(status::OBJECT_NAME_NOT_FOUND));
    assert_eq!(loader.texture("D:\\pak\\l_1.pak", first).err(), Some(status::UNRECOGNIZED_VOLUME));
    assert_eq!(loader.level("D:\\pak\\l_1.pak", first).err(), Some(status::UNRECOGNIZED_VOLUME));
    assert_eq!(loader.stored("D:\\default.xbe", first), Err(status::UNRECOGNIZED_VOLUME));
    assert_eq!(loader.stored("D:\\absent.pak", first), Err(status::OBJECT_NAME_NOT_FOUND));
    assert_eq!(loader.vfs().open_count(), 0);
}

#[test]
fn asset_loader_parses_fabricated_xbt_and_xbr() {
    let dir = temp("asset-types");
    let mut vfs = Vfs::new();
    vfs.mount_host_dir("C:", &dir).unwrap();
    let texture = tsfp_format_xbt::Texture {
        words: [4, 4, 4, 4, 0, 0, 0, 0, 0],
        remainder: vec![0; 92], pixels: vec![1, 2, 3],
    }.encode().unwrap();
    let mut level = vec![0u8; 232];
    level[4..8].copy_from_slice(&48u32.to_le_bytes());
    for (name, key, bytes) in [("a.pak", 3u32, texture.as_slice()), ("b.pak", 4, level.as_slice())] {
        let toc = (12 + bytes.len()) as u32;
        let mut pak = b"P5CK".to_vec();
        pak.extend(toc.to_le_bytes());
        pak.extend(16u32.to_le_bytes());
        pak.extend(bytes);
        for word in [key, 12, bytes.len() as u32, 0] { pak.extend(word.to_le_bytes()); }
        std::fs::write(dir.join(name), pak).unwrap();
    }
    let mut loader = AssetLoader::new(vfs);
    let decoded = loader.texture("C:/a.pak", AssetKey { key: 3, occurrence: 0 }).unwrap();
    assert_eq!(decoded.encode().unwrap(), texture);
    let decoded = loader.level("C:/b.pak", AssetKey { key: 4, occurrence: 0 }).unwrap();
    assert_eq!(decoded.encode().unwrap(), level);
    assert_eq!(loader.vfs().open_count(), 0);
}

#[test]
fn compressed_assets_require_complete_bounded_gzip() {
    use flate2::{Compression, write::GzEncoder};
    let dir = temp("compressed-assets");
    let mut vfs = Vfs::new();
    vfs.mount_host_dir("C:", &dir).unwrap();
    let texture = tsfp_format_xbt::Texture {
        words: [4, 4, 4, 4, 0, 0, 0, 0, 0],
        remainder: vec![0; 92], pixels: vec![1, 2, 3],
    }.encode().unwrap();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&texture).unwrap();
    let compressed = encoder.finish().unwrap();
    let archive = |stored: &[u8], decoded_size: u32| {
        let mut pak = b"P5CK".to_vec();
        pak.extend((12 + stored.len() as u32).to_le_bytes());
        pak.extend(16u32.to_le_bytes());
        pak.extend(stored);
        for word in [3u32, 12, decoded_size, stored.len() as u32] {
            pak.extend(word.to_le_bytes());
        }
        pak
    };
    std::fs::write(dir.join("valid.pak"), archive(&compressed, texture.len() as u32)).unwrap();
    std::fs::write(dir.join("short.pak"), archive(&compressed, texture.len() as u32 - 1)).unwrap();
    std::fs::write(dir.join("long.pak"), archive(&compressed, texture.len() as u32 + 1)).unwrap();
    std::fs::write(dir.join("corrupt.pak"), archive(&compressed[..compressed.len() - 4], texture.len() as u32)).unwrap();
    let mut loader = AssetLoader::new(vfs);
    let key = AssetKey { key: 3, occurrence: 0 };
    assert_eq!(loader.stored("C:/valid.pak", key).unwrap(), compressed);
    assert_eq!(loader.texture("C:/valid.pak", key).unwrap().encode().unwrap(), texture);
    for name in ["short", "long", "corrupt"] {
        assert_eq!(loader.texture(&format!("C:/{name}.pak"), key).err(), Some(status::UNRECOGNIZED_VOLUME));
    }
    assert_eq!(loader.vfs().open_count(), 0);
}

#[test]
fn lists_and_resolves_paths() {
    let (mut v, _d) = mounted("list");
    for p in ["D:\\pak/l_1.pak", "\\??\\D:\\PAK\\L_1.PAK", "\\Device\\CdRom0\\pak\\l_1.pak", "d:/pak//l_1.pak"] {
        assert_eq!(v.query(p).unwrap().size, 48, "{p}");
    }
    assert!(v.query("D:\\pak").unwrap().is_directory);
    assert_eq!(v.query("D:\\pak\\nope.pak"), Err(status::OBJECT_NAME_NOT_FOUND));
    assert_eq!(v.query("D:\\nodir\\x"), Err(status::OBJECT_PATH_NOT_FOUND));
    assert_eq!(v.query("D:\\pak\\..\\default.xbe"), Err(status::OBJECT_NAME_NOT_FOUND));
    assert_eq!(v.query("E:\\x"), Err(status::OBJECT_PATH_NOT_FOUND));
    assert_eq!(v.query("D:\\default.xbe\\x"), Err(status::OBJECT_PATH_NOT_FOUND));
}
#[test]
fn seek_read_and_eof() {
    let (mut v, _d) = mounted("read");
    let h = v.open("D:\\default.xbe", Access::READ, Disposition::Open).unwrap();
    let mut b = [0u8; 8];
    assert_eq!(v.seek(h, SeekFrom::End(-2)).unwrap(), 3);
    assert_eq!(v.read(h, &mut b).unwrap(), 2);
    assert_eq!(&b[..2], b"H!");
    assert_eq!(v.read(h, &mut b).unwrap(), 0); // EOF is Ok(0)
    assert_eq!(v.seek(h, SeekFrom::Current(-100)), Err(status::INVALID_PARAMETER));
    v.close(h).unwrap();
    assert_eq!(v.close(h), Err(status::INVALID_HANDLE));
    assert_eq!(v.open_count(), 0);
}
#[test]
fn disc_is_read_only_and_handle_table_bounded() {
    let (mut v, _d) = mounted("ro");
    assert_eq!(v.open("D:\\default.xbe", Access::READ_WRITE, Disposition::Open), Err(status::ACCESS_DENIED));
    assert_eq!(v.open("D:\\new", Access::READ, Disposition::Create), Err(status::ACCESS_DENIED));
    assert_eq!(v.delete("D:\\default.xbe"), Err(status::ACCESS_DENIED));
    for _ in 0..64 { v.open("D:\\default.xbe", Access::READ, Disposition::Open).unwrap(); }
    assert_eq!(v.open("D:\\default.xbe", Access::READ, Disposition::Open), Err(status::INSUFFICIENT_RESOURCES));
}
#[test]
fn cyclic_symlink_refused() {
    let (mut v, _d) = mounted("cycle");
    v.add_symlink("X:", "Y:");
    v.add_symlink("Y:", "X:");
    assert!(v.query("X:\\a").is_err());
}
#[test]
fn bad_images_rejected() {
    let d = temp("bad");
    let mut img = build(&tiny_pak());
    let p = d.join("m.iso");
    img[32 * 2048] = b'X';
    std::fs::write(&p, &img).unwrap();
    assert_eq!(Xiso::open(&p).err(), Some(XisoError::Magic));
    let mut img = build(&tiny_pak());
    img[32 * 2048 + 0x14..32 * 2048 + 0x18].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
    std::fs::write(&p, &img).unwrap();
    assert!(matches!(Xiso::open(&p).err(), Some(XisoError::Malformed(_))));
    // directory cycle: left link of the first node points back at itself via offset 0 is nil, so
    // make node 0 point at node 1 and node 1 at node 0.
    let mut img = build(&tiny_pak());
    let root = 39 * 2048;
    let second = u16::from_le_bytes([img[root], img[root + 1]]) as usize * 4;
    img[root + second..root + second + 2].copy_from_slice(&1u16.to_le_bytes()); // -> offset 4, mid-name
    std::fs::write(&p, &img).unwrap();
    let x = Xiso::open(&p).unwrap();
    assert!(x.walk().is_err() || x.walk().unwrap().len() < 100); // bounded either way, never hangs
    assert_eq!(Xiso::open(&d.join("missing.iso")).err().map(|e| matches!(e, XisoError::Image(_))), Some(true));
}
#[test]
fn pak_streaming_matches_format_crate() {
    let (mut v, _d) = mounted("pak");
    let h = v.open("D:\\pak\\l_1.pak", Access::READ, Disposition::Open).unwrap();
    let idx = PakIndex::open(&mut v, h).unwrap();
    let whole = tsfp_format_pak::Pak::parse(&tiny_pak()).unwrap();
    assert_eq!(idx.entries, whole.entries);
    for i in 0..idx.entries.len() { assert_eq!(idx.stored(&mut v, i).unwrap(), whole.stored(i).unwrap()); }
    assert_eq!(idx.find(9).collect::<Vec<_>>(), vec![1]);
    let mut b = [0u8; 1];
    assert_eq!(idx.read_stored(&mut v, 0, 5, &mut b).unwrap(), 0);
    // not a pak
    let h2 = v.open("D:\\default.xbe", Access::READ, Disposition::Open).unwrap();
    assert_eq!(PakIndex::open(&mut v, h2).err(), Some(status::UNRECOGNIZED_VOLUME));
}
#[test]
fn pak_rejects_oversized_toc_and_truncated_payload() {
    let (mut v, dir) = mounted("pak-corrupt");
    let mut bad = tiny_pak();
    bad[8..12].copy_from_slice(&0xffff_fff0u32.to_le_bytes());
    std::fs::write(dir.join("hdd/bad.pak"), bad).unwrap();
    let h = v.open("C:\\bad.pak", Access::READ, Disposition::Open).unwrap();
    assert_eq!(PakIndex::open(&mut v, h).err(), Some(status::UNRECOGNIZED_VOLUME));

    let path = dir.join("hdd/good.pak");
    std::fs::write(&path, tiny_pak()).unwrap();
    let h = v.open("C:\\good.pak", Access::READ, Disposition::Open).unwrap();
    let idx = PakIndex::open(&mut v, h).unwrap();
    // The index is valid, but the backing file can change before a stored read.
    std::fs::OpenOptions::new().write(true).open(path).unwrap().set_len(12).unwrap();
    assert_eq!(idx.stored(&mut v, 0), Err(status::UNRECOGNIZED_VOLUME));
}
#[test]
fn host_dir_save_roundtrip_case_insensitive() {
    let (mut v, dir) = mounted("save");
    let h = v.open("C:\\TDATA\\x.bin", Access::READ_WRITE, Disposition::OpenIf);
    assert_eq!(h, Err(status::OBJECT_PATH_NOT_FOUND)); // parent missing
    let h = v.open("C:\\x.bin", Access::READ_WRITE, Disposition::Create).unwrap();
    assert_eq!(v.write(h, b"hello").unwrap(), 5);
    v.close(h).unwrap();
    assert_eq!(v.open("C:\\X.BIN", Access::READ, Disposition::Create), Err(status::OBJECT_NAME_COLLISION));
    assert_eq!(v.query("c:\\X.bin").unwrap().size, 5);
    assert!(dir.join("hdd/x.bin").exists());
    v.delete("C:\\x.bin").unwrap();
    assert_eq!(v.query("C:\\x.bin"), Err(status::OBJECT_NAME_NOT_FOUND));
    // profile
    let mut raw = vec![3u8; PROFILE_FILE_BYTES];
    raw[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    let p = tsfp_format_profile::Profile::parse(&raw).unwrap();
    save_profile(&mut v, "C:\\tsftrpft", &p).unwrap();
    assert_eq!(std::fs::read(dir.join("hdd/tsftrpft")).unwrap(), raw);
    assert!(!dir.join("hdd/tsftrpft.tmp").exists());
    assert_eq!(load_profile(&mut v, "C:\\TSFTRPFT").unwrap().encode().unwrap(), raw);
    std::fs::write(dir.join("hdd/short"), b"abc").unwrap();
    assert_eq!(load_profile(&mut v, "C:\\short").err(), Some(status::UNRECOGNIZED_VOLUME));
}
