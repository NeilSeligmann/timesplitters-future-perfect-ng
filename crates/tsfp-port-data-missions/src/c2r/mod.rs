// SPDX-License-Identifier: GPL-3.0-or-later
// T2454: machine-translated code keeps the original shape (dead stores, unreachable arms); these lint classes are expected.
#![allow(
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

pub mod t2040_missions_2;
#[rustfmt::skip]
pub mod t2040_missions;
#[rustfmt::skip]
pub mod t2049_w23_004;
#[rustfmt::skip]
pub mod t2378_terminal_reference;
#[rustfmt::skip]
pub mod data_missions_4;
#[rustfmt::skip]
pub mod gamestate_wired;
#[rustfmt::skip]
pub mod t2040_missions_3;
#[rustfmt::skip]
pub mod t2064_missions;
#[rustfmt::skip]
pub mod leaf2_wired;
#[rustfmt::skip]
pub mod leaf4_wired;
#[rustfmt::skip]
pub mod objchar_wired12;
#[rustfmt::skip]
pub mod objchar_wired33;
