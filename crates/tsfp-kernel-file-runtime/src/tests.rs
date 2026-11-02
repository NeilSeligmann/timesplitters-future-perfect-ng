use crate::Runtime;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicU64, Ordering};
use tsfp_engine_fileio::{fatx_img, vfs::status};

struct Scratch(std::path::PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("tsfp-kernel-file-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn bytes(&self) -> &[u8] { self.0.as_os_str().as_bytes() }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
const WRITE: u32 = 0x40010000;

#[test]
fn create_without_mount_returns_name_not_found() {
    let mut rt = Runtime::default();
    assert_eq!(
        rt.open(b"\\Device\\Harddisk0\\partition9\\TDATA\\title.dat", 0x80000000, 7, 0, 3, 1, false),
        Err(status::OBJECT_NAME_NOT_FOUND),
    );
    assert_eq!(rt.counters.created, 0);
    assert!(rt.opens.iter().all(Option::is_none));
}

#[test]
fn delete_missing_respects_synthetic_open_policy_and_refusal_count() {
    let path = b"\\Device\\Harddisk0\\partition9\\x.dat";
    let mut rt = Runtime::default();
    assert_eq!(rt.delete(path), status::OBJECT_NAME_NOT_FOUND);
    assert_eq!(rt.counters.deleted, 0);
    assert_eq!(rt.counters.delete_refused, 0);

    let root = Scratch::new();
    let mut mounted = Runtime::default();
    assert!(mounted.mount_host(b"\\Device\\Disk", root.bytes()));
    assert_eq!(mounted.delete(b"\\Device\\Disk\\missing"), status::OBJECT_NAME_NOT_FOUND);
    assert_eq!(mounted.counters.delete_refused, 0);

    rt.openable.push(path.to_vec());
    assert_eq!(rt.delete(path), 0xC000_0002); // STATUS_NOT_IMPLEMENTED
    assert_eq!(rt.counters.deleted, 0);
    assert_eq!(rt.counters.delete_refused, 1);

    let mut rt = Runtime::default();
    rt.missing_empty = true;
    assert_eq!(rt.delete(path), 0xC000_0002); // STATUS_NOT_IMPLEMENTED
    assert_eq!(rt.counters.deleted, 0);
    assert_eq!(rt.counters.delete_refused, 1);
}

#[test]
fn delete_directory_is_refused_and_preserves_the_directory() {
    let root = Scratch::new();
    std::fs::create_dir(root.0.join("empty")).unwrap();
    let mut rt = Runtime::default();
    assert!(rt.mount_host(b"\\Device\\Disk", root.bytes()));
    assert_eq!(rt.delete(b"\\Device\\Disk\\empty"), 0xC000_0002);
    assert!(root.0.join("empty").is_dir());
    assert_eq!(rt.counters.deleted, 0);
    assert_eq!(rt.counters.delete_refused, 1);
}

#[test]
fn delete_preserves_miss_root_device_and_symlink_policies() {
    let root = Scratch::new();
    std::fs::create_dir(root.0.join("inner")).unwrap();
    std::os::unix::fs::symlink("../outside", root.0.join("inner/link")).unwrap();
    std::fs::write(root.0.join("raw.img"), b"raw").unwrap();
    let mut rt = Runtime::default();
    assert!(rt.mount_host(b"\\Device\\Disk", root.bytes()));
    assert_eq!(rt.delete(b"\\Device\\Disk\\missing\\file"), status::OBJECT_PATH_NOT_FOUND);
    assert_eq!(rt.counters.delete_refused, 0);
    assert_eq!(rt.delete(b"\\Device\\Disk"), status::ACCESS_DENIED);
    assert_eq!(rt.delete(b"\\Device\\Disk\\"), status::ACCESS_DENIED);
    assert_eq!(rt.delete(b"\\Device\\Disk\\inner\\link"), status::ACCESS_DENIED);
    assert_eq!(rt.counters.escape_refused, 1);
    assert_eq!(rt.counters.delete_refused, 3);

    let mut device = Runtime::default();
    assert!(device.mount_device(b"\\Device\\Raw", root.bytes(), b"raw.img", 3, None));
    assert_eq!(device.delete(b"\\Device\\Raw"), status::ACCESS_DENIED);
    assert_eq!(device.delete(b"\\Device\\Raw\\anything"), status::ACCESS_DENIED);
    assert_eq!(device.counters.delete_refused, 2);
}

#[test]
fn delete_uses_formatted_cache_view_and_rejects_unformatted_image() {
    let root = Scratch::new();
    std::fs::write(root.0.join("cache.img"), b"FATX").unwrap();
    std::fs::create_dir(root.0.join("cache-dir")).unwrap();
    std::fs::write(root.0.join("cache-dir/item"), b"cache").unwrap();
    std::fs::write(root.0.join("bad.img"), b"nope").unwrap();
    std::fs::create_dir(root.0.join("bad-dir")).unwrap();
    std::fs::write(root.0.join("bad-dir/item"), b"untouched").unwrap();

    let mut rt = Runtime::default();
    assert!(rt.mount_device(b"\\Device\\Cache", root.bytes(), b"cache.img", 0x80000,
        Some((5, b"cache-dir"))));
    assert_eq!(rt.delete(b"\\Device\\Cache\\item"), 0);
    assert!(!root.0.join("cache-dir/item").exists());

    assert!(rt.mount_device(b"\\Device\\BadCache", root.bytes(), b"bad.img", 0x80000,
        Some((6, b"bad-dir"))));
    assert_eq!(rt.delete(b"\\Device\\BadCache\\item"), status::UNRECOGNIZED_VOLUME);
    assert!(root.0.join("bad-dir/item").exists());
}

#[test]
fn host_symlink_refusal_increments_escape_counter() {
    let root = Scratch::new();
    let mut rt = Runtime::default();
    assert!(rt.mount_host(b"\\Device\\Disk", root.bytes()));
    std::os::unix::fs::symlink("..", root.0.join("escape")).unwrap();
    assert_eq!(rt.open(b"\\Device\\Disk\\escape\\outside", 0, 7, 0, 1, 1, false), Err(status::ACCESS_DENIED));
    assert_eq!(rt.counters.escape_refused, 1);
}

#[test]
fn device_write_range_is_checked_before_access_policy() {
    use crate::runtime::validate_device_write_range;
    assert_eq!(validate_device_write_range(0xF00, 0x200, 0x1000), Err(status::INVALID_PARAMETER));
    assert_eq!(validate_device_write_range(0xE00, 0x200, 0x1000), Ok(()));
}

#[test]
fn relative_descriptors_reject_links_and_dot_components() {
    let root = Scratch::new(); let outside = Scratch::new();
    std::fs::write(outside.0.join("secret"), b"private").unwrap();
    std::os::unix::fs::symlink(&outside.0, root.0.join("escape")).unwrap();
    let mut rt = Runtime::default(); assert!(rt.mount_host(b"\\Device\\Disk", root.bytes()));
    assert_eq!(rt.open(b"\\Device\\Disk\\escape\\secret", 0, 7, 0, 1, 1, false), Err(status::ACCESS_DENIED));
    // The C oracle counts a refused host symlink as an escape too (kernel_file.c).
    assert_eq!(rt.counters.escape_refused, 1);
    assert_eq!(rt.open(b"\\Device\\Disk\\..\\secret", 0, 7, 0, 1, 1, false), Err(status::ACCESS_DENIED));
    assert_eq!(rt.open(b"\\Device\\Disk\\.\\secret", 0, 7, 0, 1, 1, false), Err(status::ACCESS_DENIED));
    assert_eq!(rt.counters.escape_refused, 3);
    assert!(rt.opens.iter().all(Option::is_none));
}

#[test]
fn pinned_host_file_round_trip_and_allocation_preserves_eof() {
    let root = Scratch::new(); let mut rt = Runtime::default();
    assert!(rt.mount_host(b"\\Device\\Disk", root.bytes()));
    let path = b"\\Device\\Disk\\File";
    assert_eq!(rt.open(path, WRITE, 7, 0, 2, 1, false), Ok(2));
    assert_eq!(rt.write(1, 0, b"abcdefgh"), Ok(8));
    // The host can refuse allocation (unsupported FS), but it must never destroy data.
    let _ = rt.resize(1, 2, true);
    let mut bytes = [0; 8]; assert_eq!(rt.read(1, 0, &mut bytes, false), Ok(8)); assert_eq!(&bytes, b"abcdefgh");
    assert_eq!(rt.open_info(1).unwrap().metadata.size, 8);
    assert_eq!(rt.rename(1, false, b"\\Device\\Disk\\Renamed"), 0);
    assert_eq!(rt.set_disposition(1, true), 0); rt.close(1).unwrap();
    assert!(!root.0.join("Renamed").exists());
}

#[test]
fn raw_device_stored_reads_do_not_fabricate_bytes() {
    let root = Scratch::new(); let mut rt = Runtime::default();
    assert!(rt.mount_device(b"\\Device\\Raw", root.bytes(), b"device", 512, None));
    rt.open(b"\\Device\\Raw", WRITE, 7, 0, 1, 1, false).unwrap();
    rt.write(1, 0, b"AB").unwrap();
    let mut stored = [0x55; 4]; assert_eq!(rt.read(1, 0, &mut stored, true), Ok(2)); assert_eq!(stored, [b'A', b'B', 0x55, 0x55]);
    let mut padded = [0x55; 4]; assert_eq!(rt.read(1, 0, &mut padded, false), Ok(4)); assert_eq!(padded, [b'A', b'B', 0, 0]);
    assert_eq!(rt.counters.device_zero_bytes, 2);
    assert!(rt.write(1, 511, b"AB").is_err());
}

#[test]
fn cache_view_revalidates_replaced_image_before_opening() {
    let root = Scratch::new(); let mut rt = Runtime::default();
    assert!(rt.mount_device(b"\\Device\\Cache", root.bytes(), b"cache.bin", 0x80000,
        Some((5, b"cache-dir"))));
    rt.open(b"\\Device\\Cache", WRITE, 7, 0, 1, 1, false).unwrap();
    rt.write(1, 0, b"FATX").unwrap();
    rt.open(b"\\Device\\Cache\\", 0, 7, 1, 1, 2, false).unwrap();
    let fifo = std::ffi::CString::new(root.0.join("cache.bin").as_os_str().as_bytes()).unwrap();
    std::fs::remove_file(root.0.join("cache.bin")).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(rt.open(b"\\Device\\Cache\\", 0, 7, 1, 1, 3, false), Err(status::UNRECOGNIZED_VOLUME));
}

#[test]
fn plain_raw_device_has_no_guest_namespace() {
    let root = Scratch::new(); let mut rt = Runtime::default();
    assert!(rt.mount_device(b"\\Device\\Raw", root.bytes(), b"device", 512, None));
    assert_eq!(rt.open(b"\\Device\\Raw\\", 0, 7, 0, 1, 1, false), Err(status::OBJECT_PATH_NOT_FOUND));
    assert_eq!(rt.open(b"\\Device\\Raw\\inside", 0, 7, 0, 1, 2, false), Err(status::OBJECT_PATH_NOT_FOUND));
    assert_eq!(rt.open(b"\\Device\\Raw\\inside", 0x80000000, 7, 0, 3, 3, false), Err(status::OBJECT_PATH_NOT_FOUND));
    std::fs::remove_file(root.0.join("device")).unwrap();
    assert_eq!(rt.open(b"\\Device\\Raw", 0, 7, 0, 1, 4, false), Err(status::OBJECT_NAME_NOT_FOUND));
    assert_eq!(rt.open(b"\\Device\\Raw", WRITE, 7, 0, 3, 5, false), Err(status::ACCESS_DENIED));
}

#[test]
fn overwrite_disposition_refusal_counts_as_write_refused() {
    let mut counters = crate::runtime::Counters::default();
    assert_eq!(crate::runtime::refused_overwrite(&mut counters), status::ACCESS_DENIED);
    assert_eq!(counters.write_refused, 1);
}

#[test]
fn directory_cursor_survives_small_buffer_and_removed_predecessor() {
    let root = Scratch::new();
    std::fs::write(root.0.join("alpha"), b"a").unwrap(); std::fs::write(root.0.join("beta"), b"b").unwrap();
    let mut rt = Runtime::default(); assert!(rt.mount_host(b"\\Device\\Disk", root.bytes()));
    rt.open(b"\\Device\\Disk\\", 0, 7, 1, 1, 1, false).unwrap();
    assert!(matches!(rt.dir_next(1, false, Some(b"*"), 1), Err(0xc0000004)));
    assert_eq!(rt.dir_next(1, false, Some(b"ignored"), 255).unwrap().name, b"alpha");
    std::fs::remove_file(root.0.join("alpha")).unwrap();
    assert_eq!(rt.dir_next(1, false, None, 255).unwrap().name, b"beta");
    assert!(matches!(rt.dir_next(1, false, None, 255), Err(0x80000006)));
}

#[test]
fn fatx_remount_does_not_revive_old_handle() {
    let mut image = vec![0; fatx_img::MIN_IMAGE * 2];
    assert_eq!(fatx_img::format(&mut image, 42), 0);
    let mut rt = Runtime::default();
    assert!(unsafe { rt.mount_fatx(b"\\Device\\MU", image.as_mut_ptr(), image.len(), None, 0) });
    rt.open(b"\\Device\\MU\\save", WRITE, 7, 0, 2, 1, false).unwrap();
    rt.write(1, 0, b"saved").unwrap();
    rt.open(b"\\Device\\MU\\save", WRITE, 7, 0, 1, 3, false).unwrap();
    rt.write(3, 5, b"-tail").unwrap();
    rt.write(1, 0, b"S").unwrap();
    let mut complete = [0; 10];
    assert_eq!(rt.read(1, 0, &mut complete, false), Ok(10));
    assert_eq!(&complete, b"Saved-tail");
    assert!(rt.unmount_fatx(b"\\Device\\MU"));
    assert!(unsafe { rt.mount_fatx(b"\\Device\\MU", image.as_mut_ptr(), image.len(), None, 0) });
    assert_eq!(rt.read(1, 0, &mut [0; 5], false), Err(status::INVALID_HANDLE));
    assert_eq!(rt.write(1, 0, &[]), Err(status::INVALID_HANDLE));
    rt.open(b"\\Device\\MU\\save", 0, 7, 0, 1, 2, false).unwrap();
    let mut data = [0; 5]; assert_eq!(rt.read(2, 0, &mut data, false), Ok(5)); assert_eq!(&data, b"Saved");
}

#[test]
fn allocation_happens_after_resolution_before_share_refusal() {
    let root = Scratch::new(); std::fs::write(root.0.join("file"), b"x").unwrap();
    let mut rt = Runtime::default(); rt.enforce_shares = true;
    assert!(rt.mount_host(b"\\Device\\Disk", root.bytes()));
    let mut allocations = 0;
    assert!(rt.open_allocate(b"\\Device\\Disk\\missing", 0, 7, 0, 1, false, || { allocations += 1; 1 }).is_err());
    assert_eq!(allocations, 0);
    rt.open(b"\\Device\\Disk\\file", WRITE, 0, 0, 1, 1, false).unwrap();
    assert_eq!(rt.open_allocate(b"\\Device\\Disk\\file", WRITE, 0, 0, 1, false, || { allocations += 1; 2 }), Err(0xc0000043));
    assert_eq!(allocations, 1);
    rt.reset(); assert!(rt.enforce_shares);
}
