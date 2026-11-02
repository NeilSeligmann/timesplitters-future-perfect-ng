//! T3879 FIO-1 host setup ABI. The callback retains C physical volumes only as
//! a TEMPORARY-SHIM until FIO-3; setup ordering is owned by the Rust leaf.
use std::ffi::{CStr, CString, c_char, c_void};
use tsfp_host_fileio_setup::{Action, Config, Mount, Partition0};

#[repr(C)]
pub struct SetupConfig {
    mounts: *const *const c_char,
    mount_count: u32,
    disc_device: *const c_char,
    disc_path: *const c_char,
    hdd_device: *const c_char,
    hdd_path: *const c_char,
    partition0_device: *const c_char,
    partition0_image: *const c_char,
    partition0_capacity: u64,
    cache_count: u32,
    cache_first: u32,
    cache_capacity: u64,
    pre_mounted: bool,
    window_mapped: bool,
    missing_empty: bool,
}

#[repr(C)]
pub struct SetupAction {
    kind: u32,
    device: *const c_char,
    path: *const c_char,
    image: *const c_char,
    directory: *const c_char,
    number: u32,
    capacity: u64,
    skip_mount: bool,
}

type Execute = unsafe extern "C" fn(*mut c_void, *const SetupAction) -> bool;

// In a native-fileio host these resolve to tsfp-kernel-file-runtime exports;
// in the legacy comparison build they resolve to the selected C oracle. The
// setup sequence and its production callers remain Rust-owned in both cases.
unsafe extern "C" {
    fn kernel_file_add_openable(path: *const c_char) -> bool;
    fn kernel_file_mount_disc(device: *const c_char, path: *const c_char) -> bool;
    fn kernel_file_mount_host_dir(device: *const c_char, path: *const c_char) -> bool;
    fn kernel_file_mount_host_device(
        device: *const c_char,
        path: *const c_char,
        image: *const c_char,
        capacity: u64,
    ) -> bool;
    fn kernel_file_mount_cache_partition(
        device: *const c_char,
        path: *const c_char,
        number: u32,
        image: *const c_char,
        directory: *const c_char,
        capacity: u64,
    ) -> bool;
    fn kernel_file_set_missing_policy(policy: i32);
    fn kernel_file_unmount_all();
}

// Safety: all non-null pointers are NUL-terminated and borrowed for the call.
unsafe fn bytes<'a>(value: *const c_char) -> Option<&'a [u8]> {
    if value.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(value) }.to_bytes())
    }
}

fn dispatch(action: &Action<'_>, execute: Execute, context: *mut c_void) -> bool {
    let (kind, device, path, image, directory, number, capacity, skip_mount) = match action {
        Action::Empty { path } => (0, None, Some(*path), None, None, 0, 0, false),
        Action::Disc { device, path } => (1, Some(*device), Some(*path), None, None, 0, 0, false),
        Action::HostDir { device, path } => {
            (2, Some(*device), Some(*path), None, None, 0, 0, false)
        }
        Action::Partition0 {
            device,
            host_dir,
            file_name,
            capacity,
            skip_mount,
        } => (
            3,
            Some(*device),
            Some(*host_dir),
            Some(*file_name),
            None,
            0,
            *capacity,
            *skip_mount,
        ),
        Action::PublishCacheCount { count } => (4, None, None, None, None, *count, 0, false),
        Action::CachePartition {
            device,
            host_dir,
            number,
            image_name,
            content_dir,
            capacity,
        } => (
            5,
            Some(device.as_slice()),
            Some(*host_dir),
            Some(image_name.as_slice()),
            Some(content_dir.as_slice()),
            *number,
            *capacity,
            false,
        ),
        Action::MissingEmpty => (6, None, None, None, None, 0, 0, false),
    };
    let strings: Result<Vec<_>, _> = [device, path, image, directory]
        .into_iter()
        .map(|s| s.map(CString::new).transpose())
        .collect();
    let Ok(strings) = strings else {
        return false;
    };
    let ptr = |i: usize| strings[i].as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    let ffi = SetupAction {
        kind,
        device: ptr(0),
        path: ptr(1),
        image: ptr(2),
        directory: ptr(3),
        number,
        capacity,
        skip_mount,
    };
    unsafe { apply_action(&ffi) && execute(context, &ffi) }
}

unsafe fn apply_action(ffi: &SetupAction) -> bool {
    unsafe {
        match ffi.kind {
            0 => kernel_file_add_openable(ffi.path),
            1 => kernel_file_mount_disc(ffi.device, ffi.path),
            2 => kernel_file_mount_host_dir(ffi.device, ffi.path),
            3 if !ffi.skip_mount => kernel_file_mount_host_device(
                ffi.device,
                ffi.path,
                ffi.image,
                ffi.capacity,
            ),
            3 => true,
            4 => true, // The C host publishes this guest thunk value.
            5 => kernel_file_mount_cache_partition(
                ffi.device,
                ffi.path,
                ffi.number,
                ffi.image,
                ffi.directory,
                ffi.capacity,
            ),
            6 => {
                kernel_file_set_missing_policy(1);
                true
            }
            _ => false,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_host_apply_action(raw: *const SetupAction) -> bool {
    unsafe { raw.as_ref().is_some_and(|action| apply_action(action)) }
}

/// # Safety
/// `raw` points to the matching C struct. Mount arrays cover `mount_count` entries;
/// strings are NUL-terminated and live throughout this call. The callback must not
/// retain borrowed action/string pointers or unwind across the C boundary.
/// Partition0 policy strings are required even without an HDD, matching the host's
/// complete configuration; disc/HDD device strings are required only with a path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_host_setup(
    raw: *const SetupConfig,
    execute: Option<Execute>,
    context: *mut c_void,
) -> bool {
    let (Some(raw), Some(execute)) = (unsafe { raw.as_ref() }, execute) else {
        return false;
    };
    if raw.mount_count != 0 && raw.mounts.is_null() {
        return false;
    }
    let pointers = if raw.mount_count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(raw.mounts, raw.mount_count as usize) }
    };
    let Some(mounts): Option<Vec<_>> = pointers.iter().map(|&p| unsafe { bytes(p) }).collect()
    else {
        return false;
    };
    let optional_mount = |device, path| -> Result<Option<Mount<'_>>, ()> {
        let Some(path) = (unsafe { bytes(path) }) else {
            return Ok(None);
        };
        Ok(Some(Mount {
            device: unsafe { bytes(device) }.ok_or(())?,
            path,
        }))
    };
    let (Ok(disc), Ok(hdd)) = (
        optional_mount(raw.disc_device, raw.disc_path),
        optional_mount(raw.hdd_device, raw.hdd_path),
    ) else {
        return false;
    };
    let (Some(device), Some(file_name)) = (unsafe { bytes(raw.partition0_device) }, unsafe {
        bytes(raw.partition0_image)
    }) else {
        return false;
    };
    let config = Config {
        mounts: &mounts,
        disc,
        hdd,
        partition0: Partition0 {
            device,
            file_name,
            capacity: raw.partition0_capacity,
        },
        pre_mounted: raw.pre_mounted,
        window_mapped: raw.window_mapped,
        missing_empty: raw.missing_empty,
        cache_count: raw.cache_count,
        cache_first: raw.cache_first,
        cache_capacity: raw.cache_capacity,
    };
    tsfp_host_fileio_setup::run(&config, |action| dispatch(action, execute, context)).is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_host_unmount_all() {
    unsafe { kernel_file_unmount_all() }
}
