//! C ABI of the Rust per-open payload table (T2089). `src/xbox/kernel_file.c` writes through to it and, with
//! `--rust-fileio`, reads the payload back from here (`kernel_file_payload_backend`).
use std::ffi::{c_int, c_uint};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};
use tsfp_engine_fileio::payload::{OpenPayload, PayloadTable};

/// Mirror of `kernel_file_payload` in `src/xbox/kernel_file.h`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CPayload {
    handle: u32,
    desired_access: u32,
    share_access: u32,
    open_options: u32,
    offset: u64,
    size: u64,
    writable: bool,
    is_directory: bool,
    device: bool,
}

impl From<CPayload> for OpenPayload {
    fn from(c: CPayload) -> Self {
        OpenPayload {
            handle: c.handle,
            desired_access: c.desired_access,
            share_access: c.share_access,
            open_options: c.open_options,
            offset: c.offset,
            size: c.size,
            writable: c.writable,
            is_directory: c.is_directory,
            device: c.device,
        }
    }
}

impl From<OpenPayload> for CPayload {
    fn from(p: OpenPayload) -> Self {
        CPayload {
            handle: p.handle,
            desired_access: p.desired_access,
            share_access: p.share_access,
            open_options: p.open_options,
            offset: p.offset,
            size: p.size,
            writable: p.writable,
            is_directory: p.is_directory,
            device: p.device,
        }
    }
}

static TABLE: OnceLock<Mutex<PayloadTable>> = OnceLock::new();

fn with<R>(count: usize, default: R, body: impl FnOnce(&mut PayloadTable) -> R) -> R {
    let cell = TABLE.get_or_init(|| Mutex::new(PayloadTable::new(count)));
    let Ok(mut guard) = cell.lock() else {
        return default;
    };
    if guard.len() != count && count != 0 {
        *guard = PayloadTable::new(count);
    }
    catch_unwind(AssertUnwindSafe(|| body(&mut guard))).unwrap_or(default)
}

unsafe extern "C" fn c_reset(count: c_uint) {
    with(count as usize, (), |t| t.reset());
}

unsafe extern "C" fn c_open(slot: c_uint, payload: *const CPayload) -> c_int {
    if payload.is_null() {
        return 0;
    }
    let value = unsafe { *payload };
    with(0, false, |t| t.open(slot as usize, value.into())).into()
}

unsafe extern "C" fn c_release(slot: c_uint) {
    with(0, (), |t| t.release(slot as usize));
}

unsafe extern "C" fn c_get(slot: c_uint, out: *mut CPayload) -> c_int {
    crate::counters::bump(crate::counters::PAYLOAD);
    let Some(found) = with(0, None, |t| t.get(slot as usize)) else {
        return 0;
    };
    if !out.is_null() {
        unsafe { *out = found.into() };
    }
    1
}

/// T3786: whether the open in `slot` is a directory (the Rust payload table answers; no entry is not a directory).
pub(crate) fn is_directory(slot: usize) -> bool {
    with(0, None, |t| t.get(slot)).is_some_and(|payload| payload.is_directory)
}

unsafe extern "C" fn c_set_offset(slot: c_uint, offset: u64) -> c_int {
    with(0, false, |t| t.set_offset(slot as usize, offset)).into()
}

unsafe extern "C" fn c_set_size(slot: c_uint, size: u64) -> c_int {
    with(0, false, |t| t.set_size(slot as usize, size)).into()
}

#[repr(C)]
pub struct PayloadBackend {
    reset: unsafe extern "C" fn(c_uint),
    open: unsafe extern "C" fn(c_uint, *const CPayload) -> c_int,
    release: unsafe extern "C" fn(c_uint),
    get: unsafe extern "C" fn(c_uint, *mut CPayload) -> c_int,
    set_offset: unsafe extern "C" fn(c_uint, u64) -> c_int,
    set_size: unsafe extern "C" fn(c_uint, u64) -> c_int,
}
static BACKEND: PayloadBackend = PayloadBackend {
    reset: c_reset,
    open: c_open,
    release: c_release,
    get: c_get,
    set_offset: c_set_offset,
    set_size: c_set_size,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_payload_backend() -> *const PayloadBackend {
    &BACKEND
}

/// Test seam (T3786): a payload entry for `slot`.
#[cfg(test)]
pub(crate) fn test_open(count: usize, slot: usize, handle: u32, directory: bool) {
    let payload = CPayload {
        handle,
        desired_access: 0x0001_0001,
        share_access: 7,
        open_options: 0,
        offset: 0,
        size: 0,
        writable: false,
        is_directory: directory,
        device: false,
    };
    with(count, (), |t| {
        t.release(slot);
        t.open(slot, payload.into());
    });
}
