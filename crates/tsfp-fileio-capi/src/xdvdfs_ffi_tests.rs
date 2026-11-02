use super::*;
use std::ffi::CString;
use std::mem::{offset_of, size_of};
use std::os::unix::ffi::OsStrExt;

#[test]
fn reader_layout_and_result_strings_match_c_contract() {
    assert_eq!(size_of::<Reader>(), 32);
    assert_eq!(offset_of!(Reader, fd), 0);
    assert_eq!(offset_of!(Reader, image_bytes), 8);
    assert_eq!(offset_of!(Reader, root_sector), 16);
    assert_eq!(offset_of!(Reader, root_size), 20);
    assert_eq!(offset_of!(Reader, open), 24);
    assert_eq!(offset_of!(Reader, backend_handle), 28);
    assert_eq!(size_of::<CEntry>(), 272);
    let expected = ["ok", "image could not be opened or stat'd",
        "no MICROSOFT*XBOX*MEDIA at byte offset 0x10000",
        "read fell outside the image or returned short", "directory blob or entry is malformed",
        "directory walk hit its node or pending limit", "path is not on the disc", "unknown xdvdfs result"];
    for (i, value) in expected.iter().enumerate() {
        assert_eq!(unsafe { CStr::from_ptr(tsfp_native_xdvdfs_result_str(i as i32)) }.to_str().unwrap(), *value);
    }
}

// Same descriptor/dirent encoding as engine-fileio/tests/synthetic.rs, reduced to
// one entry to keep attachment tests independent of unrelated asset fixtures.
fn fixture() -> Vec<u8> {
    let mut image = vec![0; 35 * 2048];
    let d = &mut image[32 * 2048..33 * 2048];
    d[..20].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
    d[0x7ec..].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
    d[0x14..0x18].copy_from_slice(&33u32.to_le_bytes());
    d[0x18..0x1c].copy_from_slice(&20u32.to_le_bytes());
    let e = &mut image[33 * 2048..33 * 2048 + 20];
    e[4..8].copy_from_slice(&34u32.to_le_bytes());
    e[8..12].copy_from_slice(&3u32.to_le_bytes());
    e[12] = 0x20;
    e[13] = 5;
    e[14..19].copy_from_slice(b"A.bin");
    image[34 * 2048..34 * 2048 + 3].copy_from_slice(b"abc");
    image
}

unsafe extern "C" fn stop(context: *mut c_void, path: *const c_char, entry: *const CEntry) -> bool {
    assert_eq!(unsafe { CStr::from_ptr(path) }.to_bytes(), b"A.bin");
    assert_eq!(unsafe { (*entry).size }, 3);
    unsafe { *context.cast::<u32>() += 1 };
    false
}

#[test]
fn native_attachment_reads_walks_closes_and_emits_exact_call_log() {
    // Sole test touching the global native call log/backend; other tests are pure.
    let dir = std::env::temp_dir().join(format!("tsfp-native-xdvdfs-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let _cleanup = Cleanup(dir.clone());
    let iso = dir.join("test.iso");
    let log_path = dir.join("calls.log");
    std::fs::write(&iso, fixture()).unwrap();
    let path = CString::new(iso.as_os_str().as_bytes()).unwrap();
    let log_name = CString::new(log_path.as_os_str().as_bytes()).unwrap();
    let mut reader = std::mem::MaybeUninit::<Reader>::uninit();
    unsafe {
        assert!(tsfp_native_xdvdfs_log_open(log_name.as_ptr()));
        assert_eq!(tsfp_native_xdvdfs_open(reader.as_mut_ptr(), path.as_ptr()), OK);
        let mut reader = reader.assume_init();
        assert_eq!(tsfp_native_xdvdfs_image_bytes(&reader), 35 * 2048);
        assert_eq!(reader.fd, -1);
        assert_ne!(reader.backend_handle, 0);
        let mut entry = std::mem::MaybeUninit::<CEntry>::uninit();
        assert_eq!(tsfp_native_xdvdfs_lookup(&reader, c"\\a.BIN".as_ptr(), entry.as_mut_ptr()), OK);
        let entry = entry.assume_init();
        let mut bytes = [0; 8];
        let mut got = 99;
        assert_eq!(tsfp_native_xdvdfs_read(&reader, &entry, 0, bytes.as_mut_ptr().cast(), 8, &mut got), OK);
        assert_eq!(got, 3);
        assert_eq!(&bytes[..3], b"abc");
        assert_eq!(tsfp_native_xdvdfs_read(&reader, &entry, 3, std::ptr::null_mut(), 0, &mut got), OK);
        assert_eq!(got, 0);
        let mut visits = 0u32;
        assert_eq!(tsfp_native_xdvdfs_walk(&reader, Some(stop), (&mut visits as *mut u32).cast()), OK);
        assert_eq!(visits, 1);
        assert_eq!(tsfp_native_xdvdfs_walk(&reader, None, std::ptr::null_mut()), ERR_IMAGE);
        assert_eq!(tsfp_native_xdvdfs_lookup(&reader, std::ptr::null(), entry_pointer()), ERR_MALFORMED);
        tsfp_native_xdvdfs_close(&mut reader);
        tsfp_native_xdvdfs_close(&mut reader);
        assert_eq!(tsfp_native_xdvdfs_image_bytes(&reader), 0);
        assert_eq!(tsfp_native_xdvdfs_open(std::ptr::null_mut(), path.as_ptr()), ERR_IMAGE);
        assert!(tsfp_native_xdvdfs_log_open(std::ptr::null()));
    }
    assert_eq!(std::fs::read_to_string(log_path).unwrap(), concat!(
        "open rc=0\nlookup \\a.BIN rc=0 sector=34 size=3 attr=20\n",
        "read sector=34 off=0 len=8 rc=0 got=3 fnv=e71fa2190541574b\n",
        "read sector=34 off=3 len=0 rc=0 got=0 fnv=0000000000000000\n",
        "lookup  rc=4\nopen rc=1\n"));
}

// A null path must be rejected without dereferencing the otherwise nonnull out.
fn entry_pointer() -> *mut CEntry { std::ptr::NonNull::<CEntry>::dangling().as_ptr() }
