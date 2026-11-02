// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI over the Rust-owned pad source poll counters (T3926 slice 6).

use std::sync::{Mutex, PoisonError};

use tsfp_input_devices::poll::PollCounters;

static COUNTERS: Mutex<PollCounters> = Mutex::new(PollCounters::default_const());

fn with<R>(f: impl FnOnce(&mut PollCounters) -> R) -> R {
    f(&mut COUNTERS.lock().unwrap_or_else(PoisonError::into_inner))
}

/// Count a poll of `port`; its zero based index, or `u64::MAX` for a port out of range.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_poll_begin(port: u32) -> u64 {
    with(|c| c.begin(port as usize).unwrap_or(u64::MAX))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_poll_reset() {
    with(PollCounters::reset);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_poll_total() -> u64 {
    with(|c| c.total())
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_poll_port(port: u32) -> u64 {
    with(|c| c.port(port as usize))
}
