//! C ABI of the Rust link-handle table and FATX volume glue (T2107, `kernel_file_glue_backend`). With `--rust-fileio`
//! `kernel_file.c` takes the slot choice, the copied target, the live count, the mount generation, the FATX liveness and
//! the flush status from Rust, cross-checked against its C tables (kernel_file_glue_shadow_mismatches).
use std::ffi::{CStr, c_char, c_int};
use std::sync::Mutex;
use tsfp_engine_fileio::glue::{FatxVolumes, LinkHandles, VolumeState, fatx_live, flush_status};

static LINKS: Mutex<Option<LinkHandles>> = Mutex::new(None);
static VOLUMES: Mutex<Option<FatxVolumes>> = Mutex::new(None);

// Empty logical tables; generations resume after the C compatibility checkpoint.
unsafe extern "C" fn c_reset(
    link_capacity: u32,
    volume_capacity: u32,
    open_capacity: u32,
    last_generation: u64,
) {
    *LINKS.lock().unwrap() = Some(LinkHandles::new(link_capacity as usize));
    *VOLUMES.lock().unwrap() = Some(FatxVolumes::new(
        volume_capacity as usize,
        open_capacity as usize,
        last_generation,
    ));
}

// Slot index or -1 (every slot live, or reset never ran).
unsafe extern "C" fn c_link_claim(
    handle: u32,
    target: *const c_char,
    live: unsafe extern "C" fn(u32) -> c_int,
) -> c_int {
    if target.is_null() {
        return -1;
    }
    crate::counters::bump(crate::counters::GLUE);
    let target = unsafe { CStr::from_ptr(target) }.to_bytes();
    let mut links = LINKS.lock().unwrap();
    let Some(table) = links.as_mut() else {
        return -1;
    };
    table
        .claim(handle, target, |h| unsafe { live(h) } != 0)
        .map_or(-1, |slot| slot as c_int)
}

// 1 and `out` filled when the handle has a slot and the target fits, else 0.
unsafe extern "C" fn c_link_target(handle: u32, out: *mut c_char, out_bytes: usize) -> c_int {
    crate::counters::bump(crate::counters::GLUE);
    let links = LINKS.lock().unwrap();
    let Some(target) = links.as_ref().and_then(|table| table.target(handle)) else {
        return 0;
    };
    if out.is_null() || target.len() >= out_bytes {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(target.as_ptr(), out.cast::<u8>(), target.len());
        *out.add(target.len()) = 0;
    }
    1
}

unsafe extern "C" fn c_link_count(live: unsafe extern "C" fn(u32) -> c_int) -> u32 {
    let links = LINKS.lock().unwrap();
    links.as_ref().map_or(0, |table| {
        table.live_count(|h| unsafe { live(h) } != 0) as u32
    })
}

// The next mount generation, 0 when exhausted.
unsafe extern "C" fn c_issue_generation() -> u64 {
    crate::counters::bump(crate::counters::GLUE);
    VOLUMES
        .lock()
        .unwrap()
        .as_mut()
        .and_then(FatxVolumes::issue_generation)
        .unwrap_or(0)
}

unsafe extern "C" fn c_mount_fatx(
    slot: u32,
    slot_count: u32,
    has_image: c_int,
    prefix: *const c_char,
) -> u64 {
    if prefix.is_null() {
        return 0;
    }
    crate::counters::bump(crate::counters::GLUE);
    let prefix = unsafe { CStr::from_ptr(prefix) }.to_bytes();
    let generation = VOLUMES
        .lock()
        .unwrap()
        .as_mut()
        .and_then(|volumes| volumes.mount(slot as usize, has_image != 0, prefix))
        .unwrap_or(0);
    if generation != 0 {
        crate::lifecycle_ffi::publish_fatx_volume(slot as usize, slot_count as usize, prefix);
    }
    generation
}

unsafe extern "C" fn c_seed_fatx(
    slot: u32,
    in_use: c_int,
    fatx: c_int,
    has_image: c_int,
    generation: u64,
    prefix: *const c_char,
) -> c_int {
    if prefix.is_null() {
        return 0;
    }
    let prefix = unsafe { CStr::from_ptr(prefix) }.to_bytes();
    VOLUMES
        .lock()
        .unwrap()
        .as_mut()
        .is_some_and(|volumes| {
            volumes.seed(
                slot as usize,
                in_use != 0,
                fatx != 0,
                has_image != 0,
                generation,
                prefix,
            )
        })
        .into()
}

unsafe extern "C" fn c_find_fatx_prefix(prefix: *const c_char) -> c_int {
    if prefix.is_null() {
        return -1;
    }
    crate::counters::bump(crate::counters::GLUE);
    let prefix = unsafe { CStr::from_ptr(prefix) }.to_bytes();
    VOLUMES
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|volumes| volumes.find_prefix(prefix))
        .and_then(|slot| c_int::try_from(slot).ok())
        .unwrap_or(-1)
}

unsafe extern "C" fn c_unmount_fatx(slot: u32) -> c_int {
    VOLUMES
        .lock()
        .unwrap()
        .as_mut()
        .is_some_and(|volumes| volumes.unmount(slot as usize))
        .into()
}

unsafe extern "C" fn c_clear_fatx() {
    if let Some(volumes) = VOLUMES.lock().unwrap().as_mut() {
        volumes.clear();
    }
}

unsafe extern "C" fn c_fatx_slot_live(slot: u32, handle_generation: u64) -> c_int {
    crate::counters::bump(crate::counters::GLUE);
    c_int::from(
        VOLUMES
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|volumes| volumes.live(slot as usize, handle_generation)),
    )
}

unsafe extern "C" fn c_track_fatx_handle(
    open_slot: u32,
    handle: u32,
    volume_slot: u32,
    generation: u64,
) -> c_int {
    crate::counters::bump(crate::counters::GLUE);
    VOLUMES
        .lock()
        .unwrap()
        .as_mut()
        .is_some_and(|volumes| {
            volumes.track_open_handle(open_slot as usize, handle, volume_slot as usize, generation)
        })
        .into()
}

unsafe extern "C" fn c_release_fatx_handle(open_slot: u32) -> c_int {
    VOLUMES
        .lock()
        .unwrap()
        .as_mut()
        .is_some_and(|volumes| volumes.release_open_handle(open_slot as usize))
        .into()
}

unsafe extern "C" fn c_fatx_handle_live(open_slot: u32, handle: u32, volume_slot: u32) -> c_int {
    crate::counters::bump(crate::counters::GLUE);
    c_int::from(VOLUMES.lock().unwrap().as_ref().is_some_and(|volumes| {
        volumes.open_handle_live(open_slot as usize, handle, volume_slot as usize)
    }))
}

unsafe extern "C" fn c_fatx_live(
    in_use: c_int,
    fatx: c_int,
    has_image: c_int,
    volume_generation: u64,
    handle_generation: u64,
) -> c_int {
    let volume = VolumeState {
        in_use: in_use != 0,
        fatx: fatx != 0,
        has_image: has_image != 0,
        generation: volume_generation,
    };
    c_int::from(fatx_live(volume, handle_generation))
}

unsafe extern "C" fn c_flush_status(live: c_int, has_flush: c_int, flushed: c_int) -> u32 {
    flush_status(live != 0, has_flush != 0, flushed != 0)
}

#[repr(C)]
pub struct GlueBackend {
    reset: unsafe extern "C" fn(u32, u32, u32, u64),
    link_claim:
        unsafe extern "C" fn(u32, *const c_char, unsafe extern "C" fn(u32) -> c_int) -> c_int,
    link_target: unsafe extern "C" fn(u32, *mut c_char, usize) -> c_int,
    link_count: unsafe extern "C" fn(unsafe extern "C" fn(u32) -> c_int) -> u32,
    issue_generation: unsafe extern "C" fn() -> u64,
    fatx_live: unsafe extern "C" fn(c_int, c_int, c_int, u64, u64) -> c_int,
    flush_status: unsafe extern "C" fn(c_int, c_int, c_int) -> u32,
    mount_fatx: unsafe extern "C" fn(u32, u32, c_int, *const c_char) -> u64,
    seed_fatx: unsafe extern "C" fn(u32, c_int, c_int, c_int, u64, *const c_char) -> c_int,
    find_fatx_prefix: unsafe extern "C" fn(*const c_char) -> c_int,
    unmount_fatx: unsafe extern "C" fn(u32) -> c_int,
    clear_fatx: unsafe extern "C" fn(),
    fatx_slot_live: unsafe extern "C" fn(u32, u64) -> c_int,
    track_fatx_handle: unsafe extern "C" fn(u32, u32, u32, u64) -> c_int,
    release_fatx_handle: unsafe extern "C" fn(u32) -> c_int,
    fatx_handle_live: unsafe extern "C" fn(u32, u32, u32) -> c_int,
}
static BACKEND: GlueBackend = GlueBackend {
    reset: c_reset,
    link_claim: c_link_claim,
    link_target: c_link_target,
    link_count: c_link_count,
    issue_generation: c_issue_generation,
    fatx_live: c_fatx_live,
    flush_status: c_flush_status,
    mount_fatx: c_mount_fatx,
    seed_fatx: c_seed_fatx,
    find_fatx_prefix: c_find_fatx_prefix,
    unmount_fatx: c_unmount_fatx,
    clear_fatx: c_clear_fatx,
    fatx_slot_live: c_fatx_slot_live,
    track_fatx_handle: c_track_fatx_handle,
    release_fatx_handle: c_release_fatx_handle,
    fatx_handle_live: c_fatx_handle_live,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_glue_backend() -> *const GlueBackend {
    &BACKEND
}

#[cfg(test)]
mod tests {
    use super::{
        c_fatx_handle_live, c_fatx_slot_live, c_find_fatx_prefix, c_issue_generation, c_mount_fatx,
        c_release_fatx_handle, c_reset, c_seed_fatx, c_track_fatx_handle, c_unmount_fatx,
    };
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn mount_epochs_and_liveness_are_owned_by_the_rust_volume_table() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe { c_reset(0, 2, 2, 41) };
        assert_eq!(
            unsafe { c_seed_fatx(0, 1, 1, 1, 41, c"\\Device\\SeededFatx".as_ptr(),) },
            1
        );
        assert_eq!(
            unsafe { c_find_fatx_prefix(c"\\Device\\SeededFatx".as_ptr()) },
            0
        );
        unsafe { c_reset(0, 2, 2, 41) };
        let first = unsafe { c_mount_fatx(1, 2, 1, c"\\Device\\FATX".as_ptr()) };
        assert_eq!(first, 42);
        assert_eq!(unsafe { c_find_fatx_prefix(c"\\Device\\FATX".as_ptr()) }, 1);
        assert_eq!(
            unsafe { c_find_fatx_prefix(c"\\device\\fatx".as_ptr()) },
            -1
        );
        assert_eq!(unsafe { c_fatx_slot_live(1, first) }, 1);
        assert_eq!(unsafe { c_fatx_slot_live(0, first) }, 0);
        assert_eq!(unsafe { c_unmount_fatx(1) }, 1);
        assert_eq!(
            unsafe { c_find_fatx_prefix(c"\\Device\\FATX".as_ptr()) },
            -1
        );
        assert_eq!(unsafe { c_fatx_slot_live(1, first) }, 0);

        let second = unsafe { c_mount_fatx(1, 2, 1, c"\\Device\\FATX".as_ptr()) };
        assert_eq!(second, 43);
        assert_eq!(unsafe { c_fatx_slot_live(1, first) }, 0);
        assert_eq!(unsafe { c_fatx_slot_live(1, second) }, 1);
    }

    #[test]
    fn mount_epoch_clock_resumes_and_refuses_to_wrap() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe { c_reset(0, 1, 2, 41) };
        assert_eq!(unsafe { c_issue_generation() }, 42);
        unsafe { c_reset(0, 1, 2, u64::MAX) };
        assert_eq!(unsafe { c_issue_generation() }, 0);
    }

    #[test]
    fn ffi_tracks_fatx_open_epoch_until_release_or_unmount() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe { c_reset(0, 2, 4, 0) };
        let generation = unsafe { c_mount_fatx(1, 2, 1, c"\\Device\\FATX".as_ptr()) };
        assert_eq!(unsafe { c_track_fatx_handle(2, 0x120, 1, generation) }, 1);
        assert_eq!(unsafe { c_fatx_handle_live(2, 0x120, 1) }, 1);
        assert_eq!(unsafe { c_fatx_handle_live(2, 0x121, 1) }, 0);
        assert_eq!(unsafe { c_unmount_fatx(1) }, 1);
        assert_eq!(unsafe { c_fatx_handle_live(2, 0x120, 1) }, 0);
        assert_eq!(unsafe { c_release_fatx_handle(2) }, 1);
        assert_eq!(unsafe { c_fatx_handle_live(2, 0x120, 1) }, 0);
    }

    #[test]
    fn ffi_reuses_fatx_open_slot_after_release() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe { c_reset(0, 2, 4, 0) };
        let generation = unsafe { c_mount_fatx(1, 2, 1, c"\\Device\\FATX".as_ptr()) };
        assert_eq!(unsafe { c_track_fatx_handle(2, 0x120, 1, generation) }, 1);
        assert_eq!(unsafe { c_track_fatx_handle(2, 0x121, 1, generation) }, 0);
        assert_eq!(unsafe { c_fatx_handle_live(2, 0x120, 1) }, 1);
        assert_eq!(unsafe { c_unmount_fatx(1) }, 1);
        assert_eq!(unsafe { c_fatx_handle_live(2, 0x120, 1) }, 0);
        let remounted = unsafe { c_mount_fatx(1, 2, 1, c"\\Device\\FATX".as_ptr()) };
        assert_ne!(remounted, generation);
        assert_eq!(unsafe { c_track_fatx_handle(2, 0x121, 1, remounted) }, 0);
        assert_eq!(unsafe { c_release_fatx_handle(2) }, 1);
        assert_eq!(unsafe { c_track_fatx_handle(2, 0x121, 1, remounted) }, 1);
        assert_eq!(unsafe { c_fatx_handle_live(2, 0x121, 1) }, 1);
        assert_eq!(unsafe { c_fatx_handle_live(2, 0x120, 1) }, 0);
    }
}
