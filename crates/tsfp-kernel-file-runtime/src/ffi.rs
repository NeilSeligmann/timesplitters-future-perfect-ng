//! Native kernel_file attachment ABI; runtime owns all mutable volume/open state.
use crate::runtime::runtime;
use std::ffi::{CStr, c_char, c_void};

#[cfg(not(test))]
unsafe extern "C" {
    fn kernel_object_file_identity(handle: u32) -> u32;
    fn kernel_object_file_identity_live(handle: u32) -> bool;
}
fn identity(handle: u32) -> u32 {
    #[cfg(not(test))] { unsafe { kernel_object_file_identity(handle) } }
    #[cfg(test)] { handle }
}
fn identity_live(handle: u32) -> bool {
    #[cfg(not(test))] { unsafe { kernel_object_file_identity_live(handle) } }
    #[cfg(test)] { handle != 0 }
}

unsafe fn bytes<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() { None } else { Some(unsafe { CStr::from_ptr(p) }.to_bytes()) }
}
unsafe fn put<T: Copy>(p: *mut T, value: T) { if !p.is_null() { unsafe { *p = value }; } }
fn copy_name<const N: usize>(out: &mut [u8; N], name: &[u8]) {
    *out = [0; N];
    let n = name.len().min(N.saturating_sub(1));
    out[..n].copy_from_slice(&name[..n]);
}

#[repr(C)]
pub struct OpenInfo {
    pub handle: u32, pub path: [u8; 261], pub backing: u32, pub is_directory: bool,
    pub size: u64, pub offset: u64, pub disc_sector: u32, pub writable: bool,
    pub device: bool, pub cache_partition: u32, pub open_options: u32,
    pub desired_access: u32, pub share_access: u32,
}
#[repr(C)]
pub struct DirEntry {
    pub name: [u8; 256], pub is_directory: bool, pub size: u64,
    pub creation_time: u64, pub last_access_time: u64, pub last_write_time: u64,
    pub image_backed: bool, pub attributes: u32, pub allocation: u64,
}
#[repr(C)]
pub struct AttemptInfo {
    pub path: [u8; 261], pub desired_access: u32, pub share_access: u32,
    pub open_options: u32, pub attempts: u32, pub opened: bool,
}
#[repr(C)]
pub struct DeviceBacking { pub root_fd: i32, pub filename: [u8; 256], pub capacity: u64 }

pub type Observer = unsafe extern "C" fn(i32, *const c_char, u64);
fn observer() -> &'static std::sync::Mutex<Option<Observer>> {
    static O: std::sync::Mutex<Option<Observer>> = std::sync::Mutex::new(None);
    &O
}
/// Called after releasing the runtime lock, so observers can query file state.
pub fn observe(kind: i32, path: &[u8], count: u64) {
    let callback = *observer().lock().unwrap();
    if let (Some(callback), Ok(path)) = (callback, std::ffi::CString::new(path)) {
        unsafe { callback(kind, path.as_ptr(), count) };
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_set_observer(callback: Option<Observer>) { *observer().lock().unwrap() = callback; }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_mount_disc(prefix: *const c_char, path: *const c_char) -> bool {
    match (unsafe { bytes(prefix) }, unsafe { bytes(path) }) {
        (Some(p), Some(h)) => runtime().lock().unwrap().mount_disc(p, h), _ => false,
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_mount_host_dir(prefix: *const c_char, path: *const c_char) -> bool {
    match (unsafe { bytes(prefix) }, unsafe { bytes(path) }) {
        (Some(p), Some(h)) => runtime().lock().unwrap().mount_host(p, h), _ => false,
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_mount_host_device(prefix: *const c_char, root: *const c_char, name: *const c_char, capacity: u64) -> bool {
    match (unsafe { bytes(prefix) }, unsafe { bytes(root) }, unsafe { bytes(name) }) {
        (Some(p), Some(r), Some(n)) => runtime().lock().unwrap().mount_device(p, r, n, capacity, None), _ => false,
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_mount_cache_partition(prefix: *const c_char, root: *const c_char, number: u32, name: *const c_char, content: *const c_char, capacity: u64) -> bool {
    match (unsafe { bytes(prefix) }, unsafe { bytes(root) }, unsafe { bytes(name) }, unsafe { bytes(content) }) {
        (Some(p), Some(r), Some(n), Some(c)) => runtime().lock().unwrap().mount_device(p, r, n, capacity, Some((number, c))), _ => false,
    }
}
#[repr(C)]
pub struct FatxVolume {
    image: *mut u8, size: usize, volume_id: u32, sectors_per_cluster: u32,
    root_cluster: u32, cluster_count: u32, fat_offset: usize, fat_size: usize,
    data_offset: usize, fat32: bool,
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_mount_fatx(prefix: *const c_char, image: *mut FatxVolume, flush: Option<unsafe extern "C" fn(*mut c_void) -> bool>, context: *mut c_void) -> bool {
    match (unsafe { bytes(prefix) }, unsafe { image.as_ref() }) {
        (Some(p), Some(i)) => unsafe { runtime().lock().unwrap().mount_fatx(p, i.image, i.size, flush, context as usize) },
        _ => false,
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_unmount_fatx(prefix: *const c_char) -> bool {
    unsafe { bytes(prefix) }.is_some_and(|p| runtime().lock().unwrap().unmount_fatx(p))
}
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_unmount_all() {
    runtime().lock().unwrap().unmount_all();
}
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_volume_count() -> u32 { runtime().lock().unwrap().volumes.iter().flatten().count() as u32 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_open_count() -> u32 { runtime().lock().unwrap().opens.iter().flatten().count() as u32 }
// Native file-I/O has no C shadow oracle. These counters stay zero because
// no parallel comparison is performed in this build.
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_handle_shadow_mismatches() -> u32 { 0 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_payload_shadow_mismatches() -> u32 { 0 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_disposition_shadow_mismatches() -> u32 { 0 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_commit_shadow_mismatches() -> u32 { 0 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_namespace_shadow_mismatches() -> u32 { 0 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_view_shadow_mismatches() -> u32 { 0 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_glue_shadow_mismatches() -> u32 { 0 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_open_set_offset(handle: u32, offset: u64) -> bool { runtime().lock().unwrap().set_offset(identity(handle), offset) }

unsafe fn read_backing(handle: u32, offset: u64, buffer: *mut c_void, length: u32, out: *mut u32, stored: bool) -> bool {
    unsafe { put(out, 0) };
    if out.is_null() || (buffer.is_null() && length != 0) { return false; }
    let buffer = if length == 0 { &mut [] } else { unsafe { std::slice::from_raw_parts_mut(buffer.cast::<u8>(), length as usize) } };
    let handle = identity(handle);
    let (result, path) = {
        let mut r = runtime().lock().unwrap();
        let path = r.open_info(handle).map(|o| o.path.clone());
        (r.read(handle, offset, buffer, stored), path)
    };
    if let (Ok(n), Some(path)) = (&result, path) { observe(2, &path, *n as u64); }
    match result {
        Ok(n) => { unsafe { put(out, n as u32) }; true }, Err(_) => false,
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_read_backing(handle: u32, offset: u64, buffer: *mut c_void, length: u32, out: *mut u32) -> bool {
    unsafe { read_backing(handle, offset, buffer, length, out, false) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_read_backing_stored(handle: u32, offset: u64, buffer: *mut c_void, length: u32, out: *mut u32) -> bool {
    unsafe { read_backing(handle, offset, buffer, length, out, true) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_write_backing(handle: u32, offset: u64, buffer: *const c_void, length: u32, out: *mut u32, status: *mut u32) -> bool {
    unsafe { put(out, 0); put(status, 0xc000000d); }
    if out.is_null() || status.is_null() || (buffer.is_null() && length != 0) { return false; }
    let buffer = if length == 0 { &[] } else { unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), length as usize) } };
    let handle = identity(handle);
    let (result, path, transferred) = {
        let mut r = runtime().lock().unwrap();
        let path = r.open_info(handle).map(|o| o.path.clone());
        let result = r.write(handle, offset, buffer);
        (result, path, r.last_write_count)
    };
    if let (Ok(n), Some(path)) = (&result, path) { observe(3, &path, *n as u64); }
    match result {
        Ok(n) => { unsafe { put(out, n as u32); put(status, 0); }; true },
        Err(e) => { unsafe { put(out, transferred as u32); put(status, e); }; false },
    }
}
unsafe fn resize(handle: u32, length: u64, status: *mut u32, allocation: bool) -> bool {
    match runtime().lock().unwrap().resize(identity(handle), length, allocation) {
        Ok(()) => { unsafe { put(status, 0); }; true },
        Err(e) => { unsafe { put(status, e); }; false },
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_set_end_of_file(handle: u32, length: u64, status: *mut u32) -> bool { unsafe { resize(handle, length, status, false) } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_set_allocation(handle: u32, length: u64, status: *mut u32) -> bool { unsafe { resize(handle, length, status, true) } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_flush_fatx(handle: u32, status: *mut u32) -> bool {
    let r = runtime().lock().unwrap();
    let handle = identity(handle);
    if !r.open_info(handle).is_some_and(|o| o.backing == 3) { return false; }
    unsafe { put(status, r.flush(handle).err().unwrap_or(0)) }; true
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_volume_space(handle: u32, total: *mut u64, available: *mut u64, sectors: *mut u32, bytes_per_sector: *mut u32) -> bool {
    if total.is_null() || available.is_null() || sectors.is_null() || bytes_per_sector.is_null() { return false; }
    match runtime().lock().unwrap().volume_space(identity(handle)) {
        Ok((t, a, s)) => { unsafe { put(total, t); put(available, a); put(sectors, s); put(bytes_per_sector, 512); }; true },
        Err(_) => false,
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_rename(handle: u32, replace: bool, path: *const c_char) -> u32 {
    kernel_file_sweep_closed();
    unsafe { bytes(path) }.map_or(0xc000000d, |p| runtime().lock().unwrap().rename(identity(handle), replace, p))
}
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_set_disposition(handle: u32, delete: bool) -> u32 {
    kernel_file_sweep_closed();
    runtime().lock().unwrap().set_disposition(identity(handle), delete)
}
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_set_missing_policy(policy: i32) { runtime().lock().unwrap().missing_empty = policy == 1; }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_add_openable(path: *const c_char) -> bool { unsafe { bytes(path) }.is_some_and(|p| runtime().lock().unwrap().add_openable(p)) }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_openable_count() -> u32 { runtime().lock().unwrap().openable.len() as u32 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_set_share_enforcement(enforce: bool) { runtime().lock().unwrap().enforce_shares = enforce; }

#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_reset() {
    runtime().lock().unwrap().reset();
    crate::guest::reset_namespace_announcement();
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_run_locked(job: Option<unsafe extern "C" fn(*mut c_void)>, context: *mut c_void) {
    let _guard = runtime().transaction();
    if let Some(job) = job { unsafe { job(context) }; }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_open_info(handle: u32, out: *mut OpenInfo) -> bool {
    if out.is_null() { return false; }
    let r = runtime().lock().unwrap();
    let Some(o) = r.open_info(identity(handle)) else { return false; };
    let mut info = OpenInfo { handle: o.handle, path: [0; 261], backing: o.backing, is_directory: o.metadata.directory,
        size: o.metadata.size, offset: o.offset,
        disc_sector: match &o.resource { crate::storage::Resource::Disc(e) => e.start_sector, _ => 0 },
        writable: o.writable, device: o.metadata.device, cache_partition: o.cache_partition,
        open_options: o.open_options, desired_access: o.desired_access, share_access: o.share_access };
    copy_name(&mut info.path, &o.path);
    unsafe { out.write(info) }; true
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_dir_next(handle: u32, restart: bool, mask: *const c_char, max_name: u32, out: *mut DirEntry, status: *mut u32) -> bool {
    if out.is_null() || status.is_null() { unsafe { put(status, 0xc000000d); }; return false; }
    let mut r = runtime().lock().unwrap();
    let handle = identity(handle);
    let image_backed = r.open_info(handle).is_some_and(|o| o.backing == 3);
    match r.dir_next(handle, restart, unsafe { bytes(mask) }, max_name as usize) {
        Ok(entry) => {
            let m = entry.metadata;
            let mut out_entry = DirEntry { name: [0; 256], is_directory: m.directory,
                size: m.size, creation_time: m.creation, last_access_time: m.access,
                last_write_time: m.write, image_backed, attributes: m.attributes, allocation: m.allocation };
            copy_name(&mut out_entry.name, &entry.name);
            unsafe { out.write(out_entry); put(status, 0); }; true
        }
        Err(e) => { unsafe { put(status, e); }; false }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_device_backing_acquire(prefix: *const c_char, out: *mut DeviceBacking) -> bool {
    use std::os::fd::AsRawFd;
    let Some(prefix) = (unsafe { bytes(prefix) }) else { return false; };
    if out.is_null() { return false; }
    let r = runtime().lock().unwrap();
    let Some(volume) = r.volumes.iter().flatten().find(|v| v.prefix.eq_ignore_ascii_case(prefix)) else { return false; };
    let crate::storage::Storage::Host { root, device: Some(device) } = &volume.storage else { return false; };
    let fd = unsafe { libc::fcntl(root.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if fd < 0 { return false; }
    let mut result = DeviceBacking { root_fd: fd, filename: [0; 256], capacity: device.capacity };
    copy_name(&mut result.filename, &device.name);
    unsafe { out.write(result) }; true
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_add_symlink(name: *const c_char, target: *const c_char) -> bool {
    match (unsafe { bytes(name) }, unsafe { bytes(target) }) {
        (Some(n), Some(t)) => runtime().lock().unwrap().add_symlink(n, t), _ => false,
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_remove_symlink(name: *const c_char) -> bool { unsafe { bytes(name) }.is_some_and(|n| runtime().lock().unwrap().remove_symlink(n)) }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_symlink_count() -> u32 { runtime().lock().unwrap().links.iter().flatten().count() as u32 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_symlink_handle_count() -> u32 {
    let mut r = runtime().lock().unwrap();
    live_symlink_handle_count(&mut r.symlink_handles, identity_live)
}

thread_local! {
    static TARGET: std::cell::RefCell<[u8; 261]> = const { std::cell::RefCell::new([0; 261]) };
    static ATTEMPTS: std::cell::RefCell<Vec<Box<AttemptInfo>>> = const { std::cell::RefCell::new(Vec::new()) };
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_file_symlink_target(name: *const c_char) -> *const c_char {
    let Some(name) = (unsafe { bytes(name) }) else { return std::ptr::null(); };
    let r = runtime().lock().unwrap();
    let Some(target) = r.symlink_target(name) else { return std::ptr::null(); };
    TARGET.with(|slot| { let mut slot = slot.borrow_mut(); copy_name(&mut slot, target); slot.as_ptr().cast() })
}
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_attempt_count() -> u32 { runtime().lock().unwrap().attempts.len() as u32 }
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_attempt_at(index: u32) -> *const AttemptInfo {
    let r = runtime().lock().unwrap();
    let Some(a) = r.attempts.get(index as usize) else { return std::ptr::null(); };
    ATTEMPTS.with(|slots| { let mut slots = slots.borrow_mut();
        while slots.len() <= index as usize { slots.push(Box::new(AttemptInfo {
            path: [0; 261], desired_access: 0, share_access: 0, open_options: 0, attempts: 0, opened: false })); }
        let slot = &mut slots[index as usize];
        copy_name(&mut slot.path, &a.path); slot.desired_access = a.desired_access;
        slot.share_access = a.share_access; slot.open_options = a.open_options;
        slot.attempts = a.attempts; slot.opened = a.opened;
        &**slot as *const AttemptInfo
    })
}
macro_rules! counter {
    ($name:ident, $field:ident, $ty:ty) => {
        #[unsafe(no_mangle)] pub extern "C" fn $name() -> $ty { runtime().lock().unwrap().counters.$field }
    };
}
counter!(kernel_file_refused_count, refused, u32);
counter!(kernel_file_fabricated_count, fabricated, u32);
counter!(kernel_file_relative_refused_count, relative_refused, u32);
counter!(kernel_file_unmodelled_attributes_count, unmodelled_attributes, u32);
counter!(kernel_file_attribute_query_count, attribute_queries, u32);
counter!(kernel_file_deleted_count, deleted, u32);
counter!(kernel_file_delete_refused_count, delete_refused, u32);
counter!(kernel_file_disc_bytes_read, disc_bytes_read, u64);
counter!(kernel_file_disc_opened_count, disc_opened, u32);
counter!(kernel_file_host_opened_count, host_opened, u32);
counter!(kernel_file_host_bytes_read, host_bytes_read, u64);
counter!(kernel_file_host_bytes_written, host_bytes_written, u64);
counter!(kernel_file_write_refused_count, write_refused, u32);
counter!(kernel_file_created_count, created, u32);
counter!(kernel_file_escape_refused_count, escape_refused, u32);
counter!(kernel_file_cache_view_opened_count, cache_view_opened, u32);
counter!(kernel_file_device_opened_count, device_opened, u32);
counter!(kernel_file_device_zero_bytes, device_zero_bytes, u64);
counter!(kernel_file_share_conflicts_seen, share_conflicts, u32);
counter!(kernel_file_share_refusals, share_refusals, u32);
counter!(kernel_file_delete_on_close_count, delete_on_close, u32);

/// Share-mode predicate retained for C contract tests while the C backend is excluded.
#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_share_conflicts_c(
    held_desired: u32,
    held_share: u32,
    new_desired: u32,
    new_share: u32,
) -> bool {
    tsfp_engine_fileio::handles::share_conflicts(
        held_desired,
        held_share,
        new_desired,
        new_share,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_file_sweep_closed() -> u32 {
    let mut r = runtime().lock().unwrap();
    let stale: Vec<u32> = r.opens.iter().flatten().filter(|o| !identity_live(o.handle)).map(|o| o.handle).collect();
    for handle in &stale { let _ = r.close(*handle); }
    let _ = live_symlink_handle_count(&mut r.symlink_handles, identity_live);
    stale.len() as u32
}

fn live_symlink_handle_count(handles: &mut Vec<(u32, Vec<u8>)>, is_live: impl Fn(u32) -> bool) -> u32 {
    handles.retain(|(handle, _)| is_live(*handle));
    handles.len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};
    #[test]
    fn native_shadow_mismatch_counters_are_zero_without_a_c_oracle() {
        assert_eq!(kernel_file_handle_shadow_mismatches(), 0);
        assert_eq!(kernel_file_payload_shadow_mismatches(), 0);
        assert_eq!(kernel_file_disposition_shadow_mismatches(), 0);
        assert_eq!(kernel_file_commit_shadow_mismatches(), 0);
        assert_eq!(kernel_file_namespace_shadow_mismatches(), 0);
        assert_eq!(kernel_file_view_shadow_mismatches(), 0);
        assert_eq!(kernel_file_glue_shadow_mismatches(), 0);
    }
    #[test]
    fn closed_symlink_handles_are_reclaimed_when_counted() {
        let mut handles = vec![(11, b"\\Device\\Disk".to_vec()), (12, b"\\Device\\Raw".to_vec())];
        assert_eq!(live_symlink_handle_count(&mut handles, |handle| handle == 12), 1);
        assert_eq!(handles, vec![(12, b"\\Device\\Raw".to_vec())]);
    }

    #[test]
    fn public_layouts_match_kernel_file_header_on_lp64() {
        assert_eq!(offset_of!(OpenInfo, backing), 268);
        assert_eq!(offset_of!(OpenInfo, is_directory), 272);
        assert_eq!(offset_of!(OpenInfo, size), 280);
        assert_eq!(offset_of!(OpenInfo, offset), 288);
        assert_eq!(offset_of!(OpenInfo, disc_sector), 296);
        assert_eq!(offset_of!(OpenInfo, cache_partition), 304);
        assert_eq!(offset_of!(OpenInfo, share_access), 316);
        assert_eq!(size_of::<OpenInfo>(), 320);
        assert_eq!(offset_of!(DirEntry, size), 264);
        assert_eq!(offset_of!(DirEntry, image_backed), 296);
        assert_eq!(offset_of!(DirEntry, attributes), 300);
        assert_eq!(offset_of!(DirEntry, allocation), 304);
        assert_eq!(size_of::<DirEntry>(), 312);
        assert_eq!(offset_of!(AttemptInfo, desired_access), 264);
        assert_eq!(size_of::<AttemptInfo>(), 284);
        assert_eq!(offset_of!(DeviceBacking, capacity), 264);
        assert_eq!(size_of::<DeviceBacking>(), 272);
        assert_eq!(size_of::<FatxVolume>(), 64);
    }

    #[test]
    fn host_attachment_copies_state_and_observes_without_holding_runtime_lock() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static OBSERVED: AtomicU32 = AtomicU32::new(0);
        unsafe extern "C" fn callback(kind: i32, path: *const c_char, bytes: u64) {
            assert_eq!(kind, 2);
            assert_eq!(unsafe { CStr::from_ptr(path) }.to_bytes(), b"C:\\a.bin");
            assert_eq!(bytes, 3);
            assert!(runtime().try_transaction().is_some(), "observer must run after unlocking");
            OBSERVED.fetch_add(1, Ordering::SeqCst);
        }
        let dir = std::env::temp_dir().join(format!("native-fileio-ffi-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) { kernel_file_set_observer(None); kernel_file_reset(); let _ = std::fs::remove_dir_all(&self.0); }
        }
        let _cleanup = Cleanup(dir.clone());
        std::fs::write(dir.join("a.bin"), b"abc").unwrap();
        let root = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()).unwrap();
        kernel_file_reset();
        unsafe { assert!(kernel_file_mount_host_dir(c"C:".as_ptr(), root.as_ptr())); }
        assert_eq!(kernel_file_volume_count(), 1);
        runtime().lock().unwrap().open(b"C:\\a.bin", 0x80000000, 7, 0, 1, 42, true).unwrap();
        let mut info = std::mem::MaybeUninit::<OpenInfo>::uninit();
        unsafe {
            assert!(kernel_file_open_info(42, info.as_mut_ptr()));
            let info = info.assume_init();
            assert_eq!(info.backing, 2);
            assert_eq!(info.size, 3);
            assert!(!info.is_directory);
        }
        kernel_file_set_observer(Some(callback));
        let mut output = [0u8; 8];
        let mut got = 99;
        unsafe { assert!(kernel_file_read_backing(42, 0, output.as_mut_ptr().cast(), 8, &mut got)); }
        assert_eq!(got, 3);
        assert_eq!(&output[..3], b"abc");
        assert_eq!(OBSERVED.load(Ordering::SeqCst), 1);
        assert_eq!(kernel_file_host_bytes_read(), 3);
        assert!(kernel_file_open_set_offset(42, 2));
        kernel_file_reset();
        assert_eq!(kernel_file_volume_count(), 0);
        assert_eq!(kernel_file_open_count(), 0);
        assert_eq!(kernel_file_host_bytes_read(), 0);
    }
}
