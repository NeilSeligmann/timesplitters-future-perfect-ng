// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI of `xinput_route.h`. Linked into the host instead of `src/input/xinput_route.c` when `TSFP_RUST_INPUT_HLE`
//! is on; the C file is the `=OFF` fallback and the test baseline.
use std::ffi::{c_char, c_void};

use tsfp_input_route_spec::Wait;

use crate::abi::{Hooks, NavConfig, PadState};
use crate::route::Route;
use crate::sys;

/// `snprintf(buffer, size, "%s", message)`.
fn write_error(error: *mut c_char, size: usize, message: &str) {
    if error.is_null() || size == 0 {
        return;
    }
    let kept = message.len().min(size - 1);
    // SAFETY: `error` is writable for `size` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr(), error.cast::<u8>(), kept);
        *error.add(kept) = 0;
    }
}

/// # Safety
/// `script` outlives the route; `marks`/`waits` have the stated counts; `hooks` is valid; `error` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_create(
    script: *const c_void,
    marks: *const u64,
    mark_count: usize,
    waits: *const Wait,
    wait_count: usize,
    hooks: *const Hooks,
    error: *mut c_char,
    error_size: usize,
) -> *mut Route {
    if script.is_null()
        || hooks.is_null()
        || !Route::limits_ok(mark_count, wait_count)
        || (mark_count != 0 && marks.is_null())
        || (wait_count != 0 && waits.is_null())
    {
        write_error(error, error_size, "route: bad arguments");
        return std::ptr::null_mut();
    }
    let marks = if mark_count == 0 {
        &[]
    } else {
        // SAFETY: counted array.
        unsafe { std::slice::from_raw_parts(marks, mark_count) }
    };
    let waits = if wait_count == 0 {
        &[]
    } else {
        // SAFETY: counted array.
        unsafe { std::slice::from_raw_parts(waits, wait_count) }
    };
    // SAFETY: `hooks` is valid.
    match Route::new(script, marks, waits, unsafe { *hooks }) {
        Ok(route) => Box::into_raw(Box::new(route)),
        Err(message) => {
            write_error(error, error_size, &message);
            std::ptr::null_mut()
        }
    }
}

/// # Safety
/// `route` is null or from [`xinput_route_create`] and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_free(route: *mut Route) {
    if !route.is_null() {
        drop(unsafe { Box::from_raw(route) });
    }
}

/// # Safety
/// `out` is writable; `user` is a live route from [`xinput_route_create`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_source(
    poll_index: u64,
    out: *mut PadState,
    user: *mut c_void,
) -> bool {
    let route = unsafe { &*user.cast::<Route>() };
    route.poll(poll_index, unsafe { &mut *out });
    true
}

/// # Safety
/// `route` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_install(route: *mut Route) {
    unsafe { sys::xinput_source_install(Some(xinput_route_source), route.cast()) };
}

/// # Safety
/// `route` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_cursor(route: *const Route) -> u64 {
    unsafe { &*route }.cursor.get()
}

/// # Safety
/// `route` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_ended(route: *const Route) -> bool {
    unsafe { &*route }.ended.get()
}

/// # Safety
/// `route` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_failed(route: *const Route) -> bool {
    unsafe { &*route }.failed.get()
}

/// # Safety
/// `route` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_stalled_total(route: *const Route) -> u64 {
    unsafe { &*route }.stalled_total.get()
}

/// # Safety
/// `route` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_stalled_ms_total(route: *const Route) -> u64 {
    unsafe { &*route }.stalled_ms_total.get()
}

/// # Safety
/// `route` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_nav_done(route: *const Route) -> usize {
    unsafe { &*route }.nav_next.get()
}

/// # Safety
/// `route` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_nav_polls_total(route: *const Route) -> u64 {
    unsafe { &*route }.nav_polls_total.get()
}

/// # Safety
/// `route` is null or live, `config` is null or valid, `error` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_set_nav(
    route: *mut Route,
    config: *const NavConfig,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    if route.is_null() || config.is_null() {
        write_error(error, error_size, "route nav: bad arguments");
        return false;
    }
    match unsafe { &*route }.set_nav(unsafe { &*config }) {
        Ok(()) => true,
        Err(message) => {
            write_error(error, error_size, &message);
            false
        }
    }
}
