// SPDX-License-Identifier: GPL-3.0-or-later
//! T1923: lane adapters (Tier 2 through `tools.rust_trace`) for the T1899 mapmaker port that the Story route reaches:
//! 0x312AD0 (weapon set preset index, `content.rs`). 0x31DCD0 (`storage::pack_flag_struct`) is already a lane root of an earlier
//! implementation (`bridge.rs`, T1815) and is not registered a second time. The pure ports live in
//! `tsfp-mapmaker` (compiled here by `#[path]`; its `tsfp_gameplay::memory` import resolves to `crate::memory` through
//! `extern crate self as tsfp_gameplay` in `lib.rs`). Tier 1 against the lifted original is in `crates/tsfp-c-diff` `save_t1.rs`.
//! The other mapmaker storage / content ports are not reached by the Story route (docs/t1923-tier2.md).
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-mapmaker/src/content.rs"]
mod content;

/// Dispatches a root; `false` when `va` is not one of the mapmaker adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x312ad0 => regs.eax = content::weapon_set_preset_index(m),
        _ => return false,
    }
    true
}

/// The mapmaker lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 1] = [0x312ad0];
