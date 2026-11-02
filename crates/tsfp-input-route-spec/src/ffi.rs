// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI of `xinput_route_spec.h`. Linked into the host instead of `src/input/xinput_route_spec.c` when
//! `TSFP_RUST_INPUT_HLE` is on; the C file is the `=OFF` fallback and the test baseline.
use std::ffi::{CStr, c_char, c_void};

use crate::cond::{self, Cond};
use crate::poke::{self, PokeEntry, PokeSet};
use crate::wait::{Wait, wait_parse};

type ReadFn = Option<unsafe extern "C" fn(u32, u32, *mut u32, *mut c_void) -> bool>;

fn text<'a>(pointer: *const c_char) -> Option<&'a [u8]> {
    (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_bytes())
}

/// `snprintf(buffer, size, "%s", message)`: truncated, always terminated when `size > 0`; returns the full length.
fn write_c(buffer: *mut c_char, size: usize, message: &[u8]) -> usize {
    if !buffer.is_null() && size > 0 {
        let kept = message.len().min(size - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(message.as_ptr(), buffer.cast::<u8>(), kept);
            *buffer.add(kept) = 0;
        }
    }
    message.len()
}

fn parse_into(
    spec: *const c_char,
    out: *mut Wait,
    events: bool,
    error: *mut c_char,
    size: usize,
) -> bool {
    match wait_parse(text(spec), events) {
        Ok(wait) => {
            unsafe { *out = wait };
            true
        }
        Err(message) => {
            unsafe {
                *out = Wait {
                    event: events,
                    ..Wait::ZERO
                }
            };
            write_c(error, size, message.as_bytes());
            false
        }
    }
}

/// # Safety
/// `spec` is a C string or null; `out` and `error` (`error_size` bytes) are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_wait_parse(
    spec: *const c_char,
    out: *mut Wait,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    parse_into(spec, out, false, error, error_size)
}

/// # Safety
/// As `xinput_route_wait_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_event_wait_parse(
    spec: *const c_char,
    out: *mut Wait,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    parse_into(spec, out, true, error, error_size)
}

/// # Safety
/// `text` is a C string or null; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_mem_cond_parse(spec: *const c_char, out: *mut Cond) -> bool {
    text(spec)
        .and_then(cond::mem_event_condition)
        .is_some_and(|parsed| {
            unsafe { *out = parsed };
            true
        })
}

/// # Safety
/// As `xinput_route_mem_cond_parse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_mem_ref_parse(spec: *const c_char, out: *mut Cond) -> bool {
    text(spec).and_then(cond::mem_ref).is_some_and(|parsed| {
        unsafe { *out = parsed };
        true
    })
}

/// # Safety
/// `cond` and `address` are valid; `read` is a guest memory reader or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_mem_address(
    cond: *const Cond,
    read: ReadFn,
    user: *mut c_void,
    address: *mut u32,
) -> bool {
    let Some(read) = read else { return false };
    let cond = unsafe { &*cond };
    if !cond.indirect {
        unsafe { *address = cond.address };
        return true;
    }
    let mut pointer = 0u32;
    if !unsafe { read(cond.address, 4, &mut pointer, user) } || pointer == 0 {
        return false;
    }
    unsafe { *address = pointer.wrapping_add(cond.offset) };
    true
}

/// # Safety
/// As `xinput_route_mem_address`; `value` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_mem_read(
    cond: *const Cond,
    read: ReadFn,
    user: *mut c_void,
    value: *mut u32,
) -> bool {
    let mut address = 0u32;
    if !unsafe { xinput_route_mem_address(cond, read, user, &mut address) } {
        return false;
    }
    let width = unsafe { (*cond).width };
    read.is_some_and(|read| unsafe { read(address, width, value, user) })
}

#[unsafe(no_mangle)]
pub extern "C" fn xinput_route_compare(cmp: u32, left: u32, right: u32) -> bool {
    cond::compare(cmp, left, right)
}

/// # Safety
/// `cond` is valid; `out` is writable for `out_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xinput_route_cond_format(
    cond: *const Cond,
    out: *mut c_char,
    out_size: usize,
) -> usize {
    write_c(out, out_size, cond::format(unsafe { &*cond }).as_bytes())
}

/// # Safety
/// `spec` is a C string or null; `out` and `error` (`error_size` bytes) are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn poke_trigger_parse(
    spec: *const c_char,
    out: *mut PokeEntry,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    match poke::parse(text(spec)) {
        Ok(entry) => {
            unsafe { *out = entry };
            true
        }
        Err(message) => {
            unsafe { *out = PokeEntry::ZERO };
            write_c(error, error_size, message.as_bytes());
            false
        }
    }
}

/// # Safety
/// `set` is valid and outlives the returned label pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn poke_trigger_take(
    set: *mut PokeSet,
    where_: u32,
    value: u64,
) -> *const c_char {
    let set = unsafe { &mut *set };
    match set.take(where_, value) {
        Some(index) => set.entries[index].label.as_ptr().cast(),
        None => std::ptr::null(),
    }
}

// ---- T3975 slice 5a: the nav grammar (src/input/xinput_route_nav_spec.h) ----

use crate::nav_spec::{self, Menu, MenuTable, Step, Timing, c_text};

#[unsafe(no_mangle)]
pub extern "C" fn route_nav_timing_default() -> Timing {
    nav_spec::timing_default()
}

/// # Safety
/// `spec` is a C string or null; `out` and `error` (`error_size` bytes) are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn route_nav_timing_parse(
    spec: *const c_char,
    out: *mut Timing,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    match nav_spec::timing_parse(text(spec)) {
        Ok(timing) => {
            unsafe { *out = timing };
            true
        }
        Err(message) => {
            write_c(error, error_size, message.as_bytes());
            false
        }
    }
}

/// # Safety
/// `text` is a C string or null; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn route_nav_mode_parse(text_in: *const c_char, out: *mut u32) -> bool {
    nav_spec::mode_parse(text(text_in)).is_some_and(|mode| {
        unsafe { *out = mode };
        true
    })
}

/// # Safety
/// `text` is a C string or null; `out` and `error` (`error_size` bytes) are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn route_nav_line_parse(
    line: *const c_char,
    out: *mut Step,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    match nav_spec::line_parse(text(line)) {
        Ok(step) => {
            unsafe { *out = step };
            true
        }
        Err(message) => {
            unsafe { *out = Step::zero() };
            write_c(error, error_size, message.as_bytes());
            false
        }
    }
}

/// # Safety
/// `text` is a C string holding at least `length` bytes; `out` (`out_size`) and `error` (`error_size`) are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn route_nav_percent_decode(
    text_in: *const c_char,
    length: usize,
    out: *mut c_char,
    out_size: usize,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    let full = text(text_in).unwrap_or(b"");
    match nav_spec::percent_decode(full, length, out_size) {
        Ok(name) => {
            write_c(out, out_size, &name);
            true
        }
        Err(message) => {
            write_c(error, error_size, message.as_bytes());
            false
        }
    }
}

/// # Safety
/// `steps` holds `count` steps and `marks` `mark_count` marks (either may be null when its count is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn route_nav_steps_check(
    steps: *const Step,
    count: usize,
    marks: *const u64,
    mark_count: usize,
    total: u64,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    let steps = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(steps, count) }
    };
    let marks = if mark_count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(marks, mark_count) }
    };
    match nav_spec::steps_check(steps, marks, total) {
        Ok(()) => true,
        Err(message) => {
            write_c(error, error_size, message.as_bytes());
            false
        }
    }
}

/// # Safety
/// `text` holds `length` bytes; `out` is a writable menu table; `error` has `error_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn route_nav_menus_parse(
    text_in: *const c_char,
    length: usize,
    out: *mut MenuTable,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    let bytes = if length == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(text_in.cast::<u8>(), length) }
    };
    match nav_spec::menus_parse(bytes) {
        Ok(table) => {
            unsafe { std::ptr::copy_nonoverlapping(&raw const *table, out, 1) };
            true
        }
        Err(message) => {
            unsafe { std::ptr::write_bytes(out, 0, 1) };
            write_c(error, error_size, message.as_bytes());
            false
        }
    }
}

/// # Safety
/// `table` is a valid menu table or null, `id` a C string or null. The result points into `table`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn route_nav_menu_find(
    table: *const MenuTable,
    id: *const c_char,
) -> *const Menu {
    let (Some(id), false) = (text(id), table.is_null()) else {
        return std::ptr::null();
    };
    nav_spec::menu_find(unsafe { &*table }, c_text(id)).map_or(std::ptr::null(), std::ptr::from_ref)
}
