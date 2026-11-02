// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI over the Rust-owned held-key model of the host pad sources (T3926 slice 8).

use tsfp_input_devices::host_source::{Device, HostSource, Target, table};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_source_create(device: u32) -> *mut HostSource {
    Device::from_raw(device).map_or(std::ptr::null_mut(), |d| {
        Box::into_raw(Box::new(HostSource::new(d)))
    })
}

/// # Safety
/// `source` is null or from [`tsfp_input_host_source_create`], and is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_source_free(source: *mut HostSource) {
    if !source.is_null() {
        drop(unsafe { Box::from_raw(source) });
    }
}

/// True when the event was an applicable known event.
///
/// # Safety
/// `source` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_source_apply(
    source: *mut HostSource,
    device: u32,
    action: u32,
    code: u32,
    value: i32,
) -> bool {
    unsafe { source.as_mut() }.is_some_and(|s| s.apply(device, action, code, value))
}

/// Fill the pad fields: `analog` has 8 entries, `thumbs` 4 (LX LY RX RY).
///
/// # Safety
/// `source` is live; the outputs are writable with those sizes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_source_compose(
    source: *const HostSource,
    digital: *mut u16,
    analog: *mut u8,
    thumbs: *mut i16,
) {
    let Some(source) = (unsafe { source.as_ref() }) else {
        return;
    };
    let state = source.compose();
    unsafe {
        *digital = state.digital;
        std::ptr::copy_nonoverlapping(state.analog.as_ptr(), analog, state.analog.len());
        std::ptr::copy_nonoverlapping(state.thumbs.as_ptr(), thumbs, state.thumbs.len());
    }
}

/// Counter `which`: 0 applied, 1 unknown, 2 other device, 3 unmapped.
///
/// # Safety
/// `source` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_source_counter(
    source: *const HostSource,
    which: u32,
) -> u64 {
    let Some(s) = (unsafe { source.as_ref() }) else {
        return 0;
    };
    match which {
        0 => s.counters.applied,
        1 => s.counters.unknown,
        2 => s.counters.other_device,
        3 => s.counters.unmapped,
        _ => 0,
    }
}

/// Count one applied event (the C caller decides when, it also logs and installs the report).
///
/// # Safety
/// `source` is live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_source_count_applied(source: *mut HostSource) {
    if let Some(s) = unsafe { source.as_mut() } {
        s.counters.applied += 1;
    }
}

/// The mapping row `code` of `device` (kind 0 none, 1 digital, 2 analog, 3 stick), for the C table comparison.
/// False past the end of the table.
///
/// # Safety
/// The outputs are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_row(
    device: u32,
    code: u32,
    kind: *mut u32,
    digital: *mut u16,
    analog: *mut u8,
    axis: *mut u8,
    direction: *mut i8,
) -> bool {
    let Some(device) = Device::from_raw(device) else {
        return false;
    };
    let Some(row) = table(device).get(code as usize) else {
        return false;
    };
    let (k, d, a, x, dir) = match row.target {
        Target::None => (0, 0, 0, 0, 0),
        Target::Digital(bit) => (1, bit, 0, 0, 0),
        Target::Analog(index) => (2, 0, index, 0, 0),
        Target::Stick(ax, direction) => (3, 0, 0, ax, direction),
    };
    unsafe {
        *kind = k;
        *digital = d;
        *analog = a;
        *axis = x;
        *direction = dir;
    }
    true
}
