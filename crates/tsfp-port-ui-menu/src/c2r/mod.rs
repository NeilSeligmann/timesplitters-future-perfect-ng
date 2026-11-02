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

pub mod t2125_003;
#[rustfmt::skip]
pub mod ui_menu_t2042;
#[rustfmt::skip]
pub mod ui_menu;
#[rustfmt::skip]
pub mod ui_menu_4;
