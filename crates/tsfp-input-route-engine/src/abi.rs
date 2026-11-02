// SPDX-License-Identifier: GPL-3.0-or-later
//! `repr(C)` mirrors of the structs of `xinput_route.h` / `xinput_source.h` / `xinput_route_nav.h`.
use std::ffi::{c_char, c_int, c_void};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadState {
    pub digital_buttons: u16,
    pub analog: [u8; 8],
    pub thumbs: [i16; 4],
}

pub type ReadMem = Option<unsafe extern "C" fn(u32, u32, *mut u32, *mut c_void) -> bool>;
pub type SourceFn = Option<unsafe extern "C" fn(u64, *mut PadState, *mut c_void) -> bool>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct WatchState {
    pub count: u64,
    pub bytes: u64,
    pub last_ms: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FrameState {
    pub changes: u64,
    pub last_change_ms: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Probe {
    pub now_ms: Option<unsafe extern "C" fn(*mut c_void) -> u64>,
    pub watch: Option<unsafe extern "C" fn(u32, *const c_char, u32, *mut c_void) -> c_int>,
    pub watch_state: Option<unsafe extern "C" fn(c_int, *mut WatchState, *mut c_void) -> bool>,
    pub frame_state: Option<unsafe extern "C" fn(*mut FrameState, *mut c_void) -> bool>,
    pub user: *mut c_void,
}

/// `xinput_route_event_kind`.
pub mod event {
    pub const MARK: u32 = 1;
    pub const WAIT_OK: u32 = 2;
    pub const WAIT_TIMEOUT: u32 = 3;
    pub const END: u32 = 4;
    pub const WAIT_PROGRESS: u32 = 5;
    pub const NAV_START: u32 = 6;
    pub const NAV_OK: u32 = 7;
    pub const NAV_FALLBACK: u32 = 8;
    pub const NAV_PROGRESS: u32 = 9;
    pub const NAV_FAIL: u32 = 10;
}

#[repr(C)]
pub struct Event {
    pub kind: u32,
    pub index: u32,
    pub cursor: u64,
    pub poll: u64,
    pub stalled: u64,
    pub stalled_ms: u64,
    pub detail: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hooks {
    pub read_mem: ReadMem,
    pub mem_user: *mut c_void,
    pub live: SourceFn,
    pub live_user: *mut c_void,
    pub on_event: Option<unsafe extern "C" fn(*const Event, *mut c_void)>,
    pub event_user: *mut c_void,
    pub on_poll: Option<unsafe extern "C" fn(u64, u64, *mut c_void)>,
    pub poll_user: *mut c_void,
    pub probe: Probe,
}

pub type ReadBytes = Option<unsafe extern "C" fn(u32, *mut c_void, usize, *mut c_void) -> bool>;

#[repr(C)]
pub struct NavConfig {
    pub steps: *const tsfp_input_route_spec::nav_spec::Step,
    pub step_count: usize,
    pub menus: *const tsfp_input_route_spec::nav_spec::MenuTable,
    pub mode: u32,
    pub timing: *const tsfp_input_route_spec::nav_spec::Timing,
    pub read_bytes: ReadBytes,
    pub bytes_user: *mut c_void,
}

/// `route_nav_probe`.
#[repr(C)]
pub struct NavProbe {
    pub read_mem: ReadMem,
    pub read_bytes: ReadBytes,
    pub now_ms: Option<unsafe extern "C" fn(*mut c_void) -> u64>,
    pub expect_arm: Option<unsafe extern "C" fn(u32, *mut c_void)>,
    pub expect_holds:
        Option<unsafe extern "C" fn(u32, u64, u64, *mut c_char, usize, *mut c_void) -> bool>,
    pub progress: Option<unsafe extern "C" fn(*const c_char, *mut c_void)>,
    pub user: *mut c_void,
}
