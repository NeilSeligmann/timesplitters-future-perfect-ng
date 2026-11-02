// SPDX-License-Identifier: GPL-3.0-or-later
//! C boundary for a single optional capture. Event payloads are schema-owned
//! by the instrumented producer; this sink never claims completeness itself.
use std::ffi::{CStr, c_char};
use std::path::Path;
use std::sync::Mutex;

use crate::file::CaptureFile;

struct Active {
    file: CaptureFile,
    events: u64,
}
static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

/// Start a new partial capture. Identity is recorded as the first frame (kind 0).
///
/// # Safety
/// `path` must be a valid NUL-terminated UTF-8 path, and `identity` must point
/// to `identity_len` readable bytes. Neither may be changed during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_hostmem_capture_start(
    path: *const c_char,
    identity: *const u8,
    identity_len: usize,
    limit: u64,
) -> bool {
    if path.is_null() || identity.is_null() || identity_len == 0 || identity_len > 4096 {
        return false;
    }
    let Ok(path) = (unsafe { CStr::from_ptr(path) }).to_str() else {
        return false;
    };
    let identity = unsafe { std::slice::from_raw_parts(identity, identity_len) };
    let Ok(mut state) = ACTIVE.lock() else {
        return false;
    };
    if let Some(active) = state.as_mut() {
        active.file.incomplete();
        return false;
    }
    let Ok(mut file) = CaptureFile::create(Path::new(path), limit) else {
        return false;
    };
    if file.record(0, identity).is_err() {
        return false;
    }
    *state = Some(Active { file, events: 0 });
    true
}

/// Append one host event. A rejected record permanently poisons the capture.
///
/// # Safety
/// `data` must point to `len` readable bytes (or be null only if `len` is zero).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_hostmem_capture_record(
    kind: u32,
    data: *const u8,
    len: usize,
) -> bool {
    let Ok(mut state) = ACTIVE.lock() else {
        return false;
    };
    let Some(active) = state.as_mut() else {
        return false;
    };
    if kind == 0 || len > crate::MAX_RECORD || (data.is_null() && len != 0) {
        active.file.stop();
        return false;
    }
    let bytes = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    if active.file.record(kind, bytes).is_err() {
        return false;
    }
    active.events += 1;
    true
}

/// Mark a missing, unsupported or unrecorded event; publication will fail.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_capture_incomplete() {
    if let Ok(mut state) = ACTIVE.lock()
        && let Some(active) = state.as_mut()
    {
        active.file.incomplete();
    }
}

/// Finish once. No-event or incomplete captures remain partial.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_capture_seal() -> bool {
    let Ok(mut state) = ACTIVE.lock() else {
        return false;
    };
    let Some(mut active) = state.take() else {
        return false;
    };
    if active.events == 0 {
        active.file.incomplete();
    }
    active.file.seal().is_ok()
}
