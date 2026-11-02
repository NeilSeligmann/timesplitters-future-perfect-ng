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
pub mod frontier_001;
#[rustfmt::skip]
pub mod t2008_mmx;
#[rustfmt::skip]
pub mod t2008_mmx_fixture;
#[rustfmt::skip]
pub mod t2008_tail;
#[rustfmt::skip]
pub mod t2125_remote11_001;
#[rustfmt::skip]
pub mod t2008_integer;
#[rustfmt::skip]
pub mod t2008_original;
#[rustfmt::skip]
pub mod t2125_remote10_001;
#[rustfmt::skip]
pub mod t2125_remote12_001;
#[rustfmt::skip]
pub mod t2125_remote13_001;
#[rustfmt::skip]
pub mod t2125_remote7_001;
