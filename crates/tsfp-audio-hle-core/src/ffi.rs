// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust and C symbols of the final cdylib this crate consumes.
use std::ffi::{c_char, c_int, c_void};

pub type Handler = unsafe extern "C" fn(*mut c_void) -> u32;

unsafe extern "C" {
    pub fn tsfp_audio_hle_register_handler_rust(address: u32, handler: Handler) -> bool;
    pub fn tsfp_audio_hle_mark_implemented_rust(address: u32) -> bool;
    pub fn tsfp_audio_hle_set_default_return_rust(address: u32, value: u32) -> bool;
    pub fn tsfp_audio_hle_registry_reset_rust();
    pub fn tsfp_audio_hle_surface_count() -> usize;
    pub fn tsfp_audio_hle_surface_row(
        index: usize,
        address: *mut u32,
        name: *mut *const c_char,
        sites: *mut u32,
    ) -> bool;
    pub fn tsfp_audio_hle_surface_init() -> bool;
    pub fn tsfp_audio_hle_surface_unknown_calls() -> u64;
    pub fn tsfp_audio_host_hle_call(address: u32, context: *mut c_void) -> u32;
    pub fn tsfp_audio_host_hle_call_count(address: u32) -> u64;
    pub fn atexit(callback: extern "C" fn()) -> c_int;
}
