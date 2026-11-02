// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-port-gameplay-modes/c2r/modes_deferred
//! T1923: lane adapters (Tier 2 through `tools.rust_trace`) for the two heaviest `gameplay/modes` functions that already
//! have a Tier 1 machine translation in `c2r/modes_deferred.rs` (T1878, differential vs the lifted original): 0x68830
//! (match state machine tick, 12287 calls in docs/data/t1884-exec-weights.csv) and 0x681C0 (match time limit countdown and
//! warnings, 4798 calls). The translation already uses the `ExtRegisters` plus `GuestCall` contract of the lane, so this file
//! only gates which of the module's roots the host may select. Registered through `lane_inv.rs`. The other four roots of
//! the module have zero recorded execution weight and stay unselectable.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::modes_deferred,
    memory::GuestMemory,
};

/// Dispatches a root; `false` when `va` is not one of the adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x68830 | 0x681c0 => {
            crate::state_wire::audit_entry(&*m, va);
            modes_deferred::run(va, m, guest, regs)
        }
        _ => false,
    }
}

/// The lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 2] = [0x68830, 0x681c0];
