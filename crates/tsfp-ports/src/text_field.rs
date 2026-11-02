// SPDX-License-Identifier: GPL-3.0-or-later
//! T2197: the readable text-field sources of `crates/tsfp-text-field` compiled into the lane build
//! (the lane archive is built by plain rustc, so the crate is included by path like the render slice).
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-text-field/src/keyboard.rs"]
pub mod keyboard;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-text-field/src/widget.rs"]
pub mod widget;
