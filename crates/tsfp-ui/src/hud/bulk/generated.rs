// SPDX-License-Identifier: GPL-3.0-or-later
// STUB (T1909 handoff). The real file is produced from the retail image, is about 8.7 MB for the 770
// liftable functions (the repository guard rejects files over 5 MB) and is therefore regenerated locally:
//   python3 -m tools.hud_lift --xbe build/default.xbe --extra 3fc90
// The final tracked version should be restricted to the Tier 1 passing entries with
//   --emit-list crates/tsfp-ui/tests/hud_bulk_tier1.txt
// and, if still large, split into several files.
use super::rt::{Callee, Regs};
use crate::memory::GuestMemory;

/// Run the lifted function with entry `entry`. Returns false when it was not lifted (stub: always).
pub fn run<M: GuestMemory, C: Callee<M>>(_entry: u32, _m: &mut M, _c: &mut C, _r: &mut Regs) -> bool {
    false
}

/// Entries of every lifted function (stub: none).
pub const ENTRIES: [u32; 0] = [];
