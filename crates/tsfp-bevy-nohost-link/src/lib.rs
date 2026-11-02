// SPDX-License-Identifier: GPL-3.0-or-later
//! ABI stub pulled only by the Bevy no-host test binary.

use std::ffi::{c_char, c_int};

/// `cargo test` builds the shell without its normal C-host entry point.
pub unsafe fn tsfp_host_entry(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
    eprintln!("tsfp-bevy was built without the C host, see docs/t-bevy-shell.md");
    2
}

