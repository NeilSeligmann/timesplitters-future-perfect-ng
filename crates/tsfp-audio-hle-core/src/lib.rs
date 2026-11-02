// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust owner of what `crates/tsfp-audio-conformance/oracle/dsound_hle.c` held: the 41-row DirectSound entry table, codec readiness, the DSP
//! command-word acknowledgement, the log hook, degraded-call accounting and the run report. Dispatch itself, call
//! counts and the measured surface are Rust-owned in `tsfp-audio-capi` (`hle_host_entry`, `hle_surface`); this crate
//! keeps the C ABI of `dsound_hle.h` so the host and the Rust host wiring link unchanged.
//!
//! `dsound_hle_log()` (a variadic function pointer) is not exported: Rust cannot define variadic functions, and its
//! only users were the retired C device shim and the C oracle suites, which provide their own logger.

#![allow(clippy::missing_safety_doc)]
mod ffi;
mod readiness;
mod report;
mod table;

pub use readiness::*;
pub use report::*;
pub use table::*;

use std::ffi::{CStr, CString, c_char, c_int};
use std::sync::Mutex;

/// `dsound_log_fn`: the host's printf-like logger.
pub type LogFn = unsafe extern "C" fn(*const c_char, ...) -> c_int;

static LOG: Mutex<Option<LogFn>> = Mutex::new(None);

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Emit one finished line through the configured hook (stderr when none).
pub(crate) fn log(line: &str) {
    let hook = *lock(&LOG);
    match (hook, CString::new(line)) {
        // SAFETY: "%s" with one live NUL-terminated string.
        (Some(printer), Ok(text)) => unsafe {
            printer(c"%s".as_ptr(), text.as_ptr());
        },
        _ => eprint!("{line}"),
    }
}

/// `dsound_hle_set_log`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_set_log(printer: Option<LogFn>) {
    *lock(&LOG) = printer;
}

/// `dsound_hle_log_line`: one finished line from the Rust dispatcher.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_hle_log_line(line: *const c_char) {
    if line.is_null() {
        return;
    }
    // SAFETY: the caller passes a NUL-terminated string.
    log(&unsafe { CStr::from_ptr(line) }.to_string_lossy());
}
