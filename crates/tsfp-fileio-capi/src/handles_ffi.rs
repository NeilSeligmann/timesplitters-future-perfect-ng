//! C ABI of the Rust open-slot index (T2074 slice 3). With `--rust-fileio`,
//! `src/xbox/kernel_file.c` takes slot choice and handle lookups from this table;
//! the C-only build retains the original scans as its A/B path.
use std::ffi::{CStr, c_char, c_int, c_uint};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};
use tsfp_engine_fileio::handles::OpenIndex;

static INDEX: OnceLock<Mutex<OpenIndex>> = OnceLock::new();

fn with<R>(count: usize, default: R, body: impl FnOnce(&mut OpenIndex) -> R) -> R {
    let cell = INDEX.get_or_init(|| Mutex::new(OpenIndex::new(count)));
    let Ok(mut guard) = cell.lock() else {
        return default;
    };
    if guard.len() != count && count != 0 {
        *guard = OpenIndex::new(count);
    }
    catch_unwind(AssertUnwindSafe(|| body(&mut guard))).unwrap_or(default)
}

unsafe extern "C" fn c_reset(count: c_uint) {
    with(count as usize, (), |i| i.reset());
}

/// Returns the slot, or -1 when the table is full. Reclaimed slots are reported through `reclaim` AFTER the lock is dropped, so
/// the C release helper may call back into `c_release`.
unsafe extern "C" fn c_claim_slot(
    count: c_uint,
    is_live: unsafe extern "C" fn(u32) -> bool,
    reclaim: unsafe extern "C" fn(c_uint),
) -> c_int {
    crate::counters::bump(crate::counters::HANDLE_CLAIM);
    let mut reclaimed = Vec::new();
    let slot = with(count as usize, None, |i| {
        i.claim_slot(&mut |h| unsafe { is_live(h) }, &mut reclaimed)
    });
    for index in reclaimed {
        unsafe { reclaim(index as c_uint) };
    }
    slot.map_or(-1, |s| s as c_int)
}

unsafe extern "C" fn c_commit(
    slot: c_uint,
    handle: u32,
    path: *const c_char,
    desired: u32,
    share: u32,
) -> c_int {
    if path.is_null() {
        return 0;
    }
    let bytes = unsafe { CStr::from_ptr(path) }.to_bytes();
    with(0, false, |i| {
        (slot as usize) < i.len() && i.commit(slot as usize, handle, bytes, desired, share)
    })
    .into()
}

unsafe extern "C" fn c_release(slot: c_uint) {
    with(0, (), |i| i.release(slot as usize));
}

unsafe extern "C" fn c_find(handle: u32) -> c_int {
    crate::counters::bump(crate::counters::HANDLE_FIND);
    with(0, None, |i| i.find(handle)).map_or(-1, |s| s as c_int)
}

/// T3786: the slot and guest path of a live open `handle`, for a directory-relative open (`hostio_ffi::root_path`).
pub(crate) fn open_path(handle: u32) -> Option<(usize, Vec<u8>)> {
    with(0, None, |i| {
        i.find(handle)
            .and_then(|slot| i.path(slot).map(|path| (slot, path.to_vec())))
    })
}

unsafe extern "C" fn c_would_conflict() -> c_uint {
    with(0, 0, |i| i.would_conflict())
}

/// T2074 slice 10: share-rule check against the held opens, without recording. 1 = conflict.
unsafe extern "C" fn c_share_check(path: *const c_char, desired: u32, share: u32) -> c_int {
    if path.is_null() {
        return 0;
    }
    let bytes = unsafe { CStr::from_ptr(path) }.to_bytes();
    with(0, false, |i| i.check(bytes, desired, share)).into()
}

#[repr(C)]
pub struct HandleBackend {
    reset: unsafe extern "C" fn(c_uint),
    claim_slot: unsafe extern "C" fn(
        c_uint,
        unsafe extern "C" fn(u32) -> bool,
        unsafe extern "C" fn(c_uint),
    ) -> c_int,
    commit: unsafe extern "C" fn(c_uint, u32, *const c_char, u32, u32) -> c_int,
    release: unsafe extern "C" fn(c_uint),
    find: unsafe extern "C" fn(u32) -> c_int,
    would_conflict: unsafe extern "C" fn() -> c_uint,
    share_check: unsafe extern "C" fn(*const c_char, u32, u32) -> c_int,
}
static BACKEND: HandleBackend = HandleBackend {
    reset: c_reset,
    claim_slot: c_claim_slot,
    commit: c_commit,
    release: c_release,
    find: c_find,
    would_conflict: c_would_conflict,
    share_check: c_share_check,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_handle_backend() -> *const HandleBackend {
    &BACKEND
}

/// Test seam (T3786): record an open in both Rust tables the way `kernel_file.c` does (index commit, payload open).
#[cfg(test)]
pub(crate) fn test_open(count: usize, slot: usize, handle: u32, path: &[u8], directory: bool) {
    with(count, (), |i| {
        i.release(slot);
        i.commit(slot, handle, path, 0x0001_0001, 7);
    });
    crate::payload_ffi::test_open(count, slot, handle, directory);
}
