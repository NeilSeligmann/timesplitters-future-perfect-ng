// SPDX-License-Identifier: GPL-3.0-or-later
//! T3879: the DirectSound stream handler glue of `src/audio/dsound_stream.c` in Rust. State of the stream leases, routing,
//! frequency, format, volume, control and spatial sidecars already lives in Rust (`tsfp-audio-capi`); this crate is the
//! former C adapter around it: the embedding-host policy ([`config`]), the per-handler access policy ([`access`]), the
//! Create and Reset device-lease transactions ([`create`], [`reset`]) and the frame-handler shims the Rust HLE
//! dispatcher calls back into ([`hle`]). The exported names and signatures are those of the C file, whose pre-retire copy
//! stays as the oracle in `crates/tsfp-audio-conformance/oracle`.
#![allow(clippy::missing_safety_doc)]
pub mod access;
pub mod config;
pub mod create;
mod ffi;
pub mod hle;
pub mod reset;
pub mod scope;

use ffi::*;
use std::ffi::{CString, c_char};

pub(crate) const CREATE: u32 = 0x0040_967C;
pub(crate) const I3DL2: u32 = 0x0040_8637;
pub(crate) const MINIMUM: u32 = 0x0040_85F1;
pub(crate) const MAX_DISTANCE: u32 = 0x0040_85D9;
pub(crate) const SET_POSITION: u32 = 0x0040_8609;
pub(crate) const ROLLOFF: u32 = 0x0040_8632;
pub(crate) const STATUS: u32 = 0x0040_73D3;
pub(crate) const VOLUME: u32 = 0x0040_7B14;
pub(crate) const PAUSE: u32 = 0x0040_7B23;
pub(crate) const SET_FORMAT: u32 = 0x0040_8C2D;
pub(crate) const HEADROOM: u32 = 0x0040_7B19;
pub(crate) const MIX_VOLUMES: u32 = 0x0040_7B1E;
pub(crate) const MIX_BINS: u32 = 0x0040_85D4;
pub(crate) const FLUSH_EX: u32 = 0x0040_7B28;
pub(crate) const DISCONTINUITY: u32 = 0x0040_733B;
pub(crate) const SET_FREQUENCY: u32 = 0x0040_85CF;
pub(crate) const SINGLETON: u32 = 0x0041_2B30;
pub(crate) const GLOBAL_STATE: u32 = 0x0041_24A8;
pub(crate) const SECONDARY: u32 = 0x004A_1CF0;
pub(crate) const CURVE: u32 = 0x004B_914C;
pub(crate) const CURVE_NEAR: u32 = 0x004B_915C;
pub(crate) const CURVE_FAR: u32 = 0x004B_916C;
pub(crate) const STREAM_BYTES: u32 = 40;
pub(crate) const E_FAIL: u32 = 0x8000_4005;

pub(crate) fn lock_streams() {
    // SAFETY: the Rust-owned stream mutex of tsfp-audio-capi (not recursive).
    unsafe { tsfp_audio_stream_mutex_lock_rust() };
}

pub(crate) fn unlock_streams() {
    // SAFETY: pairs with `lock_streams` on the same thread.
    unsafe { tsfp_audio_stream_mutex_unlock_rust() };
}

pub(crate) fn host_pid() -> i32 {
    // SAFETY: plain host query.
    unsafe { kernel_host_pid() }
}

/// `%#x`: zero prints bare, like C.
pub(crate) fn hex(value: u32) -> String {
    if value == 0 {
        "0".to_string()
    } else {
        format!("{value:#x}")
    }
}

pub(crate) fn log_line(line: &str) {
    if let Ok(text) = CString::new(line) {
        // SAFETY: NUL-terminated finished line.
        unsafe { dsound_hle_log_line(text.as_ptr()) };
    }
}

pub(crate) fn cstr(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: every error text is a static NUL-terminated string.
    unsafe { std::ffi::CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// Fatal refusal: the fatal handler if installed, then abort (never returns).
pub(crate) fn refuse(entry: u32, reason: *const c_char) -> ! {
    let fatal = config::get().fatal_handler;
    log_line(&format!(
        "dsound passive stream {} refused: {}\n",
        hex(entry),
        cstr(reason)
    ));
    if let Some(fatal) = fatal {
        // SAFETY: the host installed the handler for exactly this call.
        unsafe { fatal(entry, reason) };
    }
    std::process::abort()
}

pub(crate) fn overlap(a: u32, an: u32, b: u32, bn: u32) -> bool {
    u64::from(a) < u64::from(b) + u64::from(bn) && u64::from(b) < u64::from(a) + u64::from(an)
}

/// Called with the stream lock held. The provider must only report the current IRQL and never reenter this module.
pub(crate) fn policy_error(config: &config::Config) -> *const c_char {
    let active = config::enabled();
    let mut known = false;
    let mut irql = 0u8;
    let mut pid = 0i32;
    // Preserve the oracle's short-circuit observation order: disabled skips IRQL, and unsupported IRQL skips the
    // guest global read. Rust owns the decision.
    if active {
        if let Some(value) = config::irql(config) {
            known = true;
            irql = value;
        }
        if known && irql == 0 {
            pid = host_pid();
        }
    }
    // SAFETY: pure policy decision returning a static string.
    unsafe {
        tsfp_audio_stream_create_policy_error_rust(
            u32::from(active),
            u32::from(known),
            u32::from(irql),
            pid,
        )
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_policy_error_host_c() -> *const c_char {
    policy_error(&config::get())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_policy_error_snapshot_host_c() -> *const c_char {
    lock_streams();
    let error = policy_error(&config::get());
    unlock_streams();
    error
}

/// Safety facts, NOT a copy of ordered vtable bytes (see dsound_stream.h).
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_original_tables_stopped() -> bool {
    // SAFETY: Rust owns guest reads and table validation.
    unsafe { tsfp_audio_stream_create_tables_stopped_rust(host_pid()) }
}

/// Registers the passive stream handlers; optional ones only with their policy on, so a flags-off boot keeps its exact
/// registry (T602).
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_register() -> usize {
    // SAFETY: registration entry points of the Rust stream handlers.
    unsafe {
        let mut count = tsfp_audio_stream_create_register_rust();
        count += tsfp_audio_stream_i3dl2_register_rust();
        count += tsfp_audio_stream_minimum_register_rust();
        count += tsfp_audio_stream_rolloff_register_rust();
        count += tsfp_audio_stream_control_register_rust();
        lock_streams();
        let config = config::get();
        let policy = config::enabled();
        unlock_streams();
        if policy {
            count += tsfp_audio_stream_format_register_rust();
            count += tsfp_audio_stream_spatial_register_rust();
        }
        if policy && config.frequency_note.is_some() && config.frequency_control_word.is_some() {
            count += tsfp_audio_stream_frequency_register_rust();
        }
        if policy && config.routing_note.is_some() {
            count += tsfp_audio_stream_routing_register_rust();
        }
        count + tsfp_audio_stream_volume_register_rust()
    }
}
