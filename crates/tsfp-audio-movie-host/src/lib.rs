// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned host-facing entry points for the XMV movie stream.
//!
//! The behavioral model lives in `tsfp-audio::movie`; this crate owns the C ABI
//! facade so the Rust audio host can omit `dsound_movie_stream.c` entirely.

#![allow(clippy::missing_safety_doc)]
pub mod access;
pub mod create;
pub mod drain;
mod ffi;
pub mod host;
pub mod route;
pub mod state;

use std::sync::atomic::{AtomicBool, Ordering};

static MOVIE_AUDIO_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable or disable the opt-in XMV audio path.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_set_enabled(enabled: bool) {
    MOVIE_AUDIO_ENABLED.store(enabled, Ordering::Release);
}

/// Read whether the opt-in XMV audio path is enabled.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_enabled() -> bool {
    MOVIE_AUDIO_ENABLED.load(Ordering::Acquire)
}

/// True when the return address is in the admitted XMV or title movie region.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_caller_ok(address: u32) -> bool {
    tsfp_audio::movie::caller_ok(address)
}

/// True only for the measured EnableAudioStream creation call site.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_create_caller_ok(address: u32) -> bool {
    tsfp_audio::movie::create_caller_ok(address)
}
