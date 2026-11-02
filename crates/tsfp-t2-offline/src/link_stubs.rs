// SPDX-License-Identifier: GPL-3.0-or-later
//! Link stubs for the lib test binary. The offline checker links the C host, which owns every
//! `xbox_*` symbol the translated lane crates declare; the test binary has no host, so each
//! symbol is defined here. `tests/test_offline_lanes.py` checks this list against the lane crates.

#[unsafe(no_mangle)]
pub extern "C" fn xbox_ReadTimeStampCounter() -> u64 {
    0
}

/// The no-host test binary does not link `rust_lane_call.c`; captured nested calls own their
/// return frame in the offline executor, so the translated audio lane's live-host hint is inert.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_lane_set_return(_pc: u32) {}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_enabled() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_render_trace_line(_text: *const core::ffi::c_char, _length: usize) {}
