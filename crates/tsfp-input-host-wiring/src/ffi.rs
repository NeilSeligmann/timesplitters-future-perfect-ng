// SPDX-License-Identifier: GPL-3.0-or-later
//! The C side: XInput HLE modules and libc output. Opaque script/live handles stay pointers.
use std::ffi::{c_char, c_int, c_void};

pub type LogFn = Option<unsafe extern "C" fn()>;

unsafe extern "C" {
    pub static stderr: *mut c_void;
    pub fn printf(format: *const c_char, ...) -> c_int;
    pub fn fprintf(stream: *mut c_void, format: *const c_char, ...) -> c_int;

    pub fn xinput_hle_init();
    pub fn xinput_devices_reset();
    pub fn xinput_hle_set_log(printer: LogFn);
    pub fn xinput_hle_attach_synthetic_pad(port: u32) -> bool;
    pub fn xinput_hle_implemented_count() -> usize;
    pub fn xinput_devices_enable_synthetic_pad(enabled: bool);
    pub fn xinput_devices_enable_multiport(enabled: bool);
    pub fn xinput_pad_remove_after_polls(polls: u32);
    pub fn xinput_pad_connect(port: u32) -> bool;
    pub fn xinput_pad_disconnect(port: u32) -> bool;
    pub fn xinput_devices_set_fatal(handler: Option<unsafe extern "C" fn(u32, *const c_char)>);
    pub fn xinput_devices_register() -> usize;
    pub fn xinput_devices_register_pad() -> usize;
    pub fn xvoice_media_register() -> usize;
    pub fn mu_register() -> usize;
    pub fn xinput_source_reset();
    pub fn xinput_source_install_ports(source: *const c_void, user: *mut c_void);
    pub fn tsfp_input_poll_port(port: u32) -> u64;
    /// Poll count stored by the baseline C record reader; Rust owns the host-facing report.
    pub fn xinput_replay_total_polls() -> u64;
    /// Budget metadata stored by the baseline C record reader.
    pub fn xinput_replay_budgets(out: *mut crate::ReplayBudgets) -> bool;
    pub fn xinput_source_set_pre_install_hook(
        hook: Option<unsafe extern "C" fn(u64, *const c_void, *mut c_void)>,
        user: *mut c_void,
    );
    pub fn xinput_source_t3615_timing_enable(enabled: bool);
    pub fn xinput_source_t3615_timing_print(out: *mut c_void);
    pub fn xinput_script_load(path: *const c_char, error: *mut c_char, size: usize) -> *mut c_void;
    pub fn xinput_script_install(script: *const c_void);
    pub fn xinput_script_entry_count(script: *const c_void) -> usize;
    pub fn xinput_script_total_frames(script: *const c_void) -> u64;
    pub fn xinput_live_open(path: *const c_char, error: *mut c_char, size: usize) -> *mut c_void;
    pub fn xinput_live_install(live: *mut c_void);
    pub fn xinput_hotkeys_needs_device(hotkeys: *const c_void, device: u32) -> bool;
    pub fn xinput_hotkeys_tick(hotkeys: *mut c_void, now_ms: u64, poll: u64);
    pub fn xinput_hotkeys_event(
        hotkeys: *mut c_void,
        device: u32,
        name: *const c_char,
        down: bool,
        now_ms: u64,
        poll: u64,
    ) -> bool;
    pub fn xinput_host_code_of(device: u32, name: *const c_char) -> u32;
    pub fn xinput_host_axis_of(name: *const c_char) -> u32;
}
