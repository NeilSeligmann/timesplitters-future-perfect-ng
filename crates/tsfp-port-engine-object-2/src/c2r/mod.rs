// SPDX-License-Identifier: GPL-3.0-or-later
// T2454: machine-translated code keeps the original shape (dead stores, unreachable arms); these lint classes are expected.
#![allow(
    clippy::all,
    dead_code,
    unused_assignments,
    unused_mut,
    unused_parens,
    unused_variables,
    unused_labels,
    unused_comparisons,
    unreachable_code,
    unreachable_patterns
)]
#[path = "../../../tsfp-ports/src/c2r/rt.rs"]
pub mod rt;
#[rustfmt::skip]
pub mod engine_object;
#[rustfmt::skip]
pub mod t2049_nx_002;
#[rustfmt::skip]
pub mod objchar_wired32;
#[rustfmt::skip]
pub mod objchar_wired36;
#[rustfmt::skip]
pub mod objchar_wired5;
#[rustfmt::skip]
pub mod objchar_wired6;
#[rustfmt::skip]
pub mod objchar_wired7;
#[rustfmt::skip]
pub mod objchar_wired8;
