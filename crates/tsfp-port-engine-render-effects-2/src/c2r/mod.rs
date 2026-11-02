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
pub mod t2006_002;
#[rustfmt::skip]
pub mod t2049_exec_004;
#[rustfmt::skip]
pub mod t2049_exec_005;
#[rustfmt::skip]
pub mod t2049_exec_006;
#[rustfmt::skip]
pub mod t2049_exec_007;
#[rustfmt::skip]
pub mod t2049_exec_008;
#[rustfmt::skip]
pub mod t2049_nx_001;
#[rustfmt::skip]
pub mod t2049_nx_003;
#[rustfmt::skip]
pub mod t2049_nx_004;
#[rustfmt::skip]
pub mod t2049_nx_005;
#[rustfmt::skip]
pub mod t2125c_002;
#[rustfmt::skip]
pub mod objchar_wired35;
#[rustfmt::skip]
pub mod t2049_nx_007;

#[cfg(test)]
mod frame_pause_differential;
