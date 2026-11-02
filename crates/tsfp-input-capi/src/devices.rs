// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI over the Rust-owned XInput device state (T3926): the recursive devices lock, the single
//! synthetic pad and the multiport host handle table of `src/input/xinput_devices.c`.
//!
//! Every entry takes the state mutex only for its own duration; the long-lived serialization the
//! C adapter relies on is the recursive [`DeviceLock`] taken through `tsfp_input_devices_lock`.

use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use tsfp_input_devices::host::{HOST_HANDLES, HostTable, OpenError};
use tsfp_input_devices::lock::DeviceLock;
use tsfp_input_devices::pad::SinglePad;

static LOCK: DeviceLock = DeviceLock::new();
static STATE: OnceLock<Mutex<State>> = OnceLock::new();

#[derive(Default)]
struct State {
    pad: SinglePad,
    host: HostTable,
}

fn state() -> MutexGuard<'static, State> {
    STATE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_devices_lock() {
    LOCK.lock();
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_devices_unlock() -> bool {
    LOCK.unlock()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_devices_depth() -> u32 {
    LOCK.depth()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_devices_state_reset() {
    let mut state = state();
    state.pad.reset();
    state.host.reset();
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_set_remove_after_polls(polls: u32) {
    state().pad.set_remove_after_polls(polls);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_open_allowed(device_type: u32, port: u32, slot: u32) -> bool {
    state().pad.open_allowed(device_type, port, slot)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_open(report_changes: u32) {
    state().pad.open(report_changes);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_close() {
    state().pad.close();
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_is_open() -> bool {
    state().pad.is_open()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_is_removed() -> bool {
    state().pad.is_removed()
}

/// 0 when removal is allowed, else 1 (already removed) or 2 (feedback pending).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_remove_refusal() -> u32 {
    match state().pad.remove_refusal() {
        None => 0,
        Some(message) if message.starts_with("removing") => 1,
        Some(_) => 2,
    }
}

/// 0 when insertion is allowed, else 1 (not removed) or 2 (old handle open).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_insert_refusal() -> u32 {
    match state().pad.insert_refusal() {
        None => 0,
        Some(message) if message.starts_with("inserting") => 1,
        Some(_) => 2,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_set_removed(removed: bool) {
    let mut state = state();
    if removed {
        state.pad.mark_removed();
    } else {
        state.pad.mark_inserted();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_sample(report_changes: u32) -> u32 {
    state().pad.sample(report_changes)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_take_pending() -> u32 {
    state().pad.take_pending()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_set_pending(address: u32) {
    state().pad.set_pending(address);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_record_output(left_right: u32) {
    state().pad.record_output(left_right.to_le_bytes());
}

/// Copies the last rumble report; false when none was sent.
///
/// # Safety
/// `out` must point to six writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_pad_last_output(out: *mut u8) -> bool {
    let Some(report) = state().pad.last_output() else {
        return false;
    };
    if !out.is_null() {
        // SAFETY: the caller guarantees six writable bytes.
        unsafe { std::ptr::copy_nonoverlapping(report.as_ptr(), out, report.len()) };
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_output_count() -> u64 {
    state().pad.output_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_pad_poll_fires_removal() -> bool {
    state().pad.poll_fires_removal()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_open_allowed(
    device_type: u32,
    port: u32,
    slot: u32,
    port_is_synthetic: bool,
) -> bool {
    state()
        .host
        .open_allowed(device_type, port, slot, port_is_synthetic)
}

/// 0 and `*token` set on success, 1 when the token generation is exhausted, 2 when the table is full.
///
/// # Safety
/// `token_out` must point to one writable u32.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_open(
    port: u32,
    report_changes: u32,
    token_out: *mut u32,
) -> u32 {
    if token_out.is_null() {
        return 3;
    }
    match state().host.open(port, report_changes) {
        // SAFETY: non-null, one writable u32 by contract.
        Ok(token) => {
            unsafe { token_out.write(token) };
            0
        }
        Err(OpenError::GenerationExhausted) => 1,
        Err(OpenError::TooManyHandles) => 2,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_contains(token: u32) -> bool {
    state().host.contains(token)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_is_removed(token: u32) -> bool {
    state().host.is_removed(token)
}

/// Port of the handle, or `u32::MAX` for an unknown token.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_port_of(token: u32) -> u32 {
    state().host.port_of(token).unwrap_or(u32::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_packet(token: u32, report_changes: u32) -> u32 {
    state().host.packet(token, report_changes)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_take_pending(token: u32) -> u32 {
    state().host.take_pending(token)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_host_set_pending(token: u32, address: u32) {
    state().host.set_pending(token, address);
}

/// Close a handle. Returns false for an unknown token. `*pending` is the feedback block to
/// cancel (0 none); `*silence_port` is the port to silence, or `u32::MAX`.
///
/// # Safety
/// Both outputs must point to one writable u32.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_close(
    token: u32,
    pending: *mut u32,
    silence_port: *mut u32,
) -> bool {
    if pending.is_null() || silence_port.is_null() {
        return false;
    }
    let Some((address, silence)) = state().host.close(token) else {
        return false;
    };
    // SAFETY: non-null, one writable u32 each by contract.
    unsafe {
        pending.write(address);
        silence_port.write(silence.unwrap_or(u32::MAX));
    }
    true
}

/// Hot unplug of `port`: fills `pending` (capacity [`HOST_HANDLES`]) with the feedback blocks to
/// cancel and returns their count.
///
/// # Safety
/// `pending` must point to `HOST_HANDLES` writable u32 values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_host_disconnect(port: u32, pending: *mut u32) -> u32 {
    let cancelled = state().host.disconnect(port);
    if !pending.is_null() {
        for (index, address) in cancelled.iter().take(HOST_HANDLES).enumerate() {
            // SAFETY: index < HOST_HANDLES writable values by contract.
            unsafe { pending.add(index).write(*address) };
        }
    }
    cancelled.len() as u32
}
