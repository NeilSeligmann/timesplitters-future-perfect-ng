// SPDX-License-Identifier: GPL-3.0-or-later
//! Mechanically lifted `ui/hud` and `ui/framework` functions (T1909), breadth first.
//!
//! `generated.rs` is produced by `tools/hud_lift.py` from the retail image; `rt.rs` is the register,
//! flag, SSE and x87 runtime the lifted bodies share. Tiers: docs/t-hud-slice.md section 6.

pub mod generated;
pub mod rt;
