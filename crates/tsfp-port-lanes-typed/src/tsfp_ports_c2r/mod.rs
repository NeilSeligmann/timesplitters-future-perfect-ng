// SPDX-License-Identifier: GPL-3.0-or-later
//! tsfp-ports c2r hand ports that never moved to a family crate, with the shared c2r runtime
//! (`super::rt`) they name.
#[allow(unused)]
#[path = "../../../tsfp-ports/src/c2r/rt.rs"]
pub mod rt;
#[allow(clippy::all)]
#[path = "../../../tsfp-ports/src/c2r/t2125c_manual.rs"]
pub mod t2125c_manual;
