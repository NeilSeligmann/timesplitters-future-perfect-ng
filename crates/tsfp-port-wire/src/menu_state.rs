// SPDX-License-Identifier: GPL-3.0-or-later
//! T2060: thin binding to the shared S3 guest-authoritative menu model.
//! Path inclusion keeps the standalone rustc lane and Cargo using the same contract.
use crate::memory::GuestMemory;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/menu.rs"]
mod model;
pub use model::*;
