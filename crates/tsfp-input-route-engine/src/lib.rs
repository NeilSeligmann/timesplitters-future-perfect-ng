// SPDX-License-Identifier: GPL-3.0-or-later
//! T3975 slice 5b: the route player of `src/input/xinput_route.c` in Rust. The player owns its state (cursor, wait
//! runtime, nav bookkeeping) as Rust data behind the same opaque `xinput_route` handle and C ABI. The T1640 nav state
//! machine (`xinput_route_nav.c`) still runs in C behind `sys` (TEMPORARY-SHIM, removed with the nav machine port).
//! `TSFP_RUST_INPUT_HLE=0` builds keep the C player as the baseline.
pub mod abi;
mod describe;
pub mod ffi;
pub mod host_route;
mod nav;
pub mod route;
mod sys;
