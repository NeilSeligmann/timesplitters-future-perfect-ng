//! Spec tests for the read/write FATX twin (T2074). FABRICATED inputs; the differential against the C
//! implementation lives in `tests/c/t2074_fatx_diff.c`.
use tsfp_engine_fileio::fatx_img as f;

fn volume() -> (Vec<u8>, f::Geo) {
    let mut img = vec![0u8; 1024 * 1024];
    assert_eq!(f::format(&mut img, 7), f::OK);
    let geo = f::open(&img).expect("formatted image opens");
    (img, geo)
}

#[test]
fn write_read_replace_remove() {
    let (mut img, g) = volume();
    let data: Vec<u8> = (0..40_000u32).map(|i| (i % 251) as u8).collect();
    assert_eq!(f::write_path(&g, &mut img, b"UDATA", &data), f::OK);
    assert_eq!(f::mkdir_path(&g, &mut img, b"dir"), f::OK);
    assert_eq!(f::mkdir_path(&g, &mut img, b"DIR"), f::E_EXISTS);
    assert_eq!(f::write_path(&g, &mut img, b"dir\\a.sav", &data), f::OK);
    assert_eq!(f::write_path(&g, &mut img, b"nodir\\a", &data), f::E_PATH_NOT_FOUND);
    assert_eq!(f::stat_path(&g, &img, b"dir/A.SAV"), Ok((false, 40_000)));
    let mut out = vec![0u8; 100];
    assert_eq!(f::read_range(&g, &img, b"dir\\a.sav", 16_380, &mut out), Ok(100));
    assert_eq!(out, data[16_380..16_480]);
    assert_eq!(f::read_range(&g, &img, b"dir\\a.sav", 40_000, &mut out), Ok(0));
    let before = f::free_clusters(&g, &img);
    assert_eq!(f::write_path(&g, &mut img, b"dir\\a.sav", b"x"), f::OK);
    assert_eq!(f::free_clusters(&g, &img), before + 2);
    assert_eq!(f::resize_path(&g, &mut img, b"dir\\a.sav", 5, 1, b"yz"), f::OK);
    let mut small = [0u8; 8];
    let mut len = 0;
    let (parent, leaf) = (g, b"UDATA".to_vec());
    assert_eq!(f::read_file(&parent, &img, &leaf, &mut small, &mut len), f::OK);
    assert_eq!(len, 40_000);
    assert_eq!(f::resize_path(&g, &mut img, b"dir\\a.sav", 5, 1, b"yz"), f::OK);
    assert_eq!(f::remove_file(&g, &mut img, b"udata"), f::OK);
    assert_eq!(f::remove_file(&g, &mut img, b"udata"), f::E_NOT_FOUND);
    assert_eq!(f::file_count(&g, &img), 1);
}

#[test]
fn bad_names_and_images() {
    let (mut img, g) = volume();
    assert_eq!(f::write_path(&g, &mut img, b"a b", b""), f::E_NAME);
    assert_eq!(f::write_path(&g, &mut img, b"dir\\..\\a", b""), f::E_PATH_NOT_FOUND);
    assert_eq!(f::write_path(&g, &mut img, &[b'a'; 43], b""), f::E_NAME);
    let mut bad = img.clone();
    bad[0] ^= 1;
    assert_eq!(f::open(&bad), Err(f::E_BAD_MAGIC));
    assert_eq!(f::open(&img[..100]), Err(f::E_GEOMETRY));
}

#[test]
fn directory_slots() {
    let (mut img, g) = volume();
    assert_eq!(f::mkdir_path(&g, &mut img, b"d"), f::OK);
    assert_eq!(f::write_path(&g, &mut img, b"d\\f", b"hello"), f::OK);
    let (e, end) = f::directory_entry_at(&g, &img, b"d", 0).expect("slot 0");
    assert!(!end);
    assert_eq!((e.name.as_slice(), e.size, e.allocation), (b"f".as_slice(), 5, 16_384));
    assert!(f::directory_entry_at(&g, &img, b"d", 1).expect("slot 1").1);
    assert!(f::directory_entry_at(&g, &img, b"", 300).expect("past").1);
}
