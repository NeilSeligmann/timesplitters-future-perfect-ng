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

pub mod gameplay_player_2;
pub mod gameplay_player_3;
#[rustfmt::skip]
pub mod gameplay_player_stats;
#[rustfmt::skip]
pub mod leaf_wired;
#[rustfmt::skip]
pub mod objchar_wired18;
#[rustfmt::skip]
pub mod objchar_wired20;
#[rustfmt::skip]
pub mod objchar_wired21;
#[rustfmt::skip]
pub mod objchar_wired22;
#[rustfmt::skip]
pub mod objchar_wired23;
#[rustfmt::skip]
pub mod objchar_wired28;
#[rustfmt::skip]
pub mod player_wired;
