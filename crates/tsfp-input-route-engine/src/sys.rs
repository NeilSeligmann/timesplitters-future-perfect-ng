// SPDX-License-Identifier: GPL-3.0-or-later
//! The C modules the player still calls: the T1640 nav machine, the script and the source dispatcher.
use std::ffi::{c_char, c_void};

use tsfp_input_route_spec::nav_spec::{Menu, Step, Timing};

use crate::abi::{NavProbe, PadState, SourceFn};

unsafe extern "C" {
    pub fn route_nav_sizeof() -> usize;
    pub fn route_nav_begin(
        machine: *mut c_void,
        step: *const Step,
        number: u32,
        menu: *const Menu,
        mode: u32,
        timing: *const Timing,
        probe: *const NavProbe,
    );
    pub fn route_nav_poll(machine: *mut c_void, poll_index: u64, pad: *mut PadState) -> u32;
    pub fn route_nav_message(machine: *const c_void) -> *const c_char;
    pub fn route_nav_step_label(step: *const Step, out: *mut c_char, out_size: usize) -> usize;
    pub fn route_nav_summary(
        machine: *const c_void,
        presses: *mut u32,
        lost: *mut u32,
        polls: *mut u64,
        ready_polls: *mut u64,
    );
    pub fn xinput_script_total_frames(script: *const c_void) -> u64;
    pub fn xinput_script_state_at(script: *const c_void, poll_index: u64) -> PadState;
    pub fn xinput_source_install(source: SourceFn, user: *mut c_void);
    pub fn clock_gettime(clock: i32, out: *mut Timespec) -> i32;
}

#[repr(C)]
pub struct Timespec {
    pub sec: i64,
    pub nsec: i64,
}
