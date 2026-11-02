// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI over the Rust input record reader (T3926 slice 5). C hands over the file bytes and the
//! run identity, gets an opaque loaded replay, and reads its tables back by index.

use std::ffi::{CStr, CString, c_char};

use tsfp_input_record::{Replay, load_replay};

/// A loaded replay with its text tables as C strings.
pub struct Loaded {
    replay: Replay,
    waits: Vec<CString>,
    navs: Vec<CString>,
}

fn c_strings(items: &[String]) -> Vec<CString> {
    items
        .iter()
        .map(|s| CString::new(s.replace('\0', "")).unwrap_or_default())
        .collect()
}

unsafe fn text<'a>(pointer: *const c_char) -> std::borrow::Cow<'a, str> {
    if pointer.is_null() {
        "".into()
    } else {
        unsafe { CStr::from_ptr(pointer) }.to_string_lossy()
    }
}

fn write_error(message: &str, out: *mut c_char, capacity: usize) {
    if out.is_null() || capacity == 0 {
        return;
    }
    let bytes = message.as_bytes();
    let n = bytes.len().min(capacity - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast::<u8>(), n);
        *out.add(n) = 0;
    }
}

/// Load a record. Null with `error` filled when the file is refused.
///
/// # Safety
/// `bytes` must point to `len` readable bytes, the strings must be NUL terminated, and `error`
/// must have room for `error_capacity` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_record_load(
    bytes: *const u8,
    len: usize,
    xbe_sha256: *const c_char,
    flags_sha256: *const c_char,
    flags: *const c_char,
    error: *mut c_char,
    error_capacity: usize,
) -> *mut Loaded {
    let data = if bytes.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, len) }
    };
    let (xbe, sha, flags) = unsafe { (text(xbe_sha256), text(flags_sha256), text(flags)) };
    match load_replay(data, &xbe, &sha, &flags) {
        Ok(replay) => Box::into_raw(Box::new(Loaded {
            waits: c_strings(&replay.waits),
            navs: c_strings(&replay.navs),
            replay,
        })),
        Err(message) => {
            write_error(&message, error, error_capacity);
            std::ptr::null_mut()
        }
    }
}

/// # Safety
/// `loaded` came from `tsfp_input_record_load` and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_record_free(loaded: *mut Loaded) {
    if !loaded.is_null() {
        drop(unsafe { Box::from_raw(loaded) });
    }
}

fn get<'a>(loaded: *const Loaded) -> Option<&'a Loaded> {
    unsafe { loaded.as_ref() }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_record_total(loaded: *const Loaded) -> u64 {
    get(loaded).map_or(0, |l| l.replay.total_polls)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_record_entry_count(loaded: *const Loaded) -> usize {
    get(loaded).map_or(0, |l| l.replay.script.entries().len())
}

/// Entry `index`: frames, digital buttons, 8 analog bytes, 4 thumb axes. False past the end.
///
/// # Safety
/// `analog` has room for 8 bytes and `thumbs` for 4 values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_record_entry(
    loaded: *const Loaded,
    index: usize,
    frames: *mut u32,
    digital: *mut u16,
    analog: *mut u8,
    thumbs: *mut i16,
) -> bool {
    let Some(entry) = get(loaded).and_then(|l| l.replay.script.entries().get(index)) else {
        return false;
    };
    unsafe {
        *frames = entry.frames;
        *digital = entry.state.digital_buttons;
        std::ptr::copy_nonoverlapping(entry.state.analog.as_ptr(), analog, 8);
        std::ptr::copy_nonoverlapping(entry.state.thumbs.as_ptr(), thumbs, 4);
    }
    true
}

/// Budgets as owner waits, worker blanks, thread timeout, interactive. False without the line.
///
/// # Safety
/// `out` has room for 4 values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_record_budgets(loaded: *const Loaded, out: *mut u32) -> bool {
    let Some(b) = get(loaded).and_then(|l| l.replay.budgets) else {
        return false;
    };
    unsafe {
        *out = b.owner_waits;
        *out.add(1) = b.worker_blanks;
        *out.add(2) = b.thread_timeout_ms;
        *out.add(3) = u32::from(b.interactive);
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_record_mark_count(loaded: *const Loaded) -> usize {
    get(loaded).map_or(0, |l| l.replay.marks.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_record_mark(loaded: *const Loaded, index: usize) -> u64 {
    get(loaded)
        .and_then(|l| l.replay.marks.get(index).copied())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_record_wait_count(loaded: *const Loaded) -> usize {
    get(loaded).map_or(0, |l| l.waits.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_record_wait(loaded: *const Loaded, index: usize) -> *const c_char {
    get(loaded)
        .and_then(|l| l.waits.get(index))
        .map_or(std::ptr::null(), |s| s.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_record_nav_count(loaded: *const Loaded) -> usize {
    get(loaded).map_or(0, |l| l.navs.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_record_nav(loaded: *const Loaded, index: usize) -> *const c_char {
    get(loaded)
        .and_then(|l| l.navs.get(index))
        .map_or(std::ptr::null(), |s| s.as_ptr())
}
