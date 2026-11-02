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
#[rustfmt::skip]
#[path = "../../../tsfp-ports/src/c2r/rt.rs"]
pub mod rt;

pub mod render_model;
#[rustfmt::skip]
pub mod engine_mixed;
