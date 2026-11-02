// SPDX-License-Identifier: GPL-3.0-or-later
//! T2058: the generated typed views of crates/tsfp-state, compiled into this crate (the lane build
//! cannot depend on a crate; same trick as the renderer sources in lib.rs). `values.rs` and
//! `generated_views.rs` reach `GuestMemory` through `super::`, which is this module's re-export, so
//! ports, views and the lane share a single trait. Importers that are path-included by standalone
//! test roots declare `#[path = "views.rs"] mod views;` themselves (see combat.rs).
pub use crate::memory::GuestMemory;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/generated_views.rs"]
pub mod generated_views;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/values.rs"]
pub mod values;
