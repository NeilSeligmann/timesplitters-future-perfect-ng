// SPDX-License-Identifier: GPL-3.0-or-later
//! T3975 slice 4: the host side of the input hotkeys in Rust. `grammar` is `xinput_hotkey_parse`, `ffi` the C ABI of
//! `xinput_hotkey.h` (the chord engine itself is `tsfp-input-devices::hotkey`) and `actions` owns host action state in Rust.
//! Linked into the host instead of those two C files when `TSFP_RUST_INPUT_HLE` is on; the C files are the `=OFF` fallback.
pub mod grammar;

#[cfg(not(test))]
pub mod actions;
#[cfg(not(test))]
pub mod ffi;
