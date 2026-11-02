// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/c2r/engine_object
// portreg: wraps=tsfp-ports/c2r/engine_object_6
//! T2026: lane adapters (Tier 2 through `tools.rust_trace`) for the heaviest `engine/object` functions that already have a
//! c2r translation (`c2r/engine_object.rs`, `c2r/engine_object_6.rs`, Tier 1 differential on fabricated fixtures). The
//! translations use the `ExtRegisters` plus `GuestCall` contract of the lane, so this file only gates which of their roots
//! the hybrid host may select. Registered through `lane_inv.rs`. Roots are the 30 c2r engine/object functions with the
//! highest recorded call counts in `docs/data/t1884-exec-weights.csv`, ordered by weight (0x9E430, the pool
//! next-free-slot index, is the hottest function named by T2014 and sits fifth).
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r,
    memory::GuestMemory,
};

/// Dispatches a root; `false` when `va` is not one of the adapters.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    let entry_esp = regs.esp;
    if va == 0xa14b0 && crate::object_transform::selected() {
        crate::object_transform::run(m, regs);
        return true;
    }
    let taken = match va {
        // T2122: pool/record writers answer from the wired translation (stores go through ObjectPool::classify)
        0x9e430 | 0x9fba0 | 0xa7360 | 0xa6530 | 0x862c0 | 0xa0c50 | 0xa2430 | 0x9fbe0 | 0xa00b0 | 0x9fd60 | 0xa1b80
        | 0xa14b0 => c2r::objchar_wired::run(va, m, guest, regs),
        0xa14b0 => c2r::engine_object::run(va, m, guest, regs),
        0x1518f0 => c2r::engine_object::run(va, m, guest, regs),
        0x9e840 => c2r::engine_object::run(va, m, guest, regs),
        0xa1ab0 => c2r::engine_object::run(va, m, guest, regs),
        0x9e430 => c2r::engine_object_6::run(va, m, guest, regs),
        0xa0c50 => c2r::engine_object::run(va, m, guest, regs),
        0xa7360 => c2r::engine_object::run(va, m, guest, regs),
        0xbe860 => c2r::engine_object::run(va, m, guest, regs),
        0xa00b0 => c2r::engine_object::run(va, m, guest, regs),
        0x9e330 => c2r::engine_object::run(va, m, guest, regs),
        0x9fba0 => c2r::engine_object::run(va, m, guest, regs),
        0x74720 => c2r::engine_object::run(va, m, guest, regs),
        0x9fd60 => c2r::engine_object::run(va, m, guest, regs),
        0x9fbe0 => c2r::engine_object::run(va, m, guest, regs),
        0xa1b80 => c2r::engine_object::run(va, m, guest, regs),
        0x10e210 => c2r::engine_object::run(va, m, guest, regs),
        0x162200 => c2r::engine_object::run(va, m, guest, regs),
        0x2776f0 => c2r::engine_object::run(va, m, guest, regs),
        0xbdd50 => c2r::engine_object::run(va, m, guest, regs),
        0x1622c0 => c2r::engine_object::run(va, m, guest, regs),
        0xa1fa0 => c2r::engine_object::run(va, m, guest, regs),
        0xa2430 => c2r::engine_object::run(va, m, guest, regs),
        0x862c0 => c2r::engine_object::run(va, m, guest, regs),
        0x48fe0 => c2r::engine_object::run(va, m, guest, regs),
        0xa6530 => c2r::engine_object::run(va, m, guest, regs),
        0x9a330 => c2r::engine_object::run(va, m, guest, regs),
        0x153190 => c2r::engine_object::run(va, m, guest, regs),
        0x9eb60 => c2r::engine_object::run(va, m, guest, regs),
        0x23ff20 => c2r::engine_object::run(va, m, guest, regs),
        0x80430 => c2r::engine_object::run(va, m, guest, regs),
        _ => false,
    };
    // Lane contract: ESP comes back as it entered (the host pops the return address and any `ret N` bytes).
    if taken {
        regs.esp = entry_esp;
    }
    taken
}

/// The lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 30] = [
    0xa14b0, 0x1518f0, 0x9e840, 0xa1ab0, 0x9e430, 0xa0c50, 0xa7360, 0xbe860,
    0xa00b0, 0x9e330, 0x9fba0, 0x74720, 0x9fd60, 0x9fbe0, 0xa1b80, 0x10e210,
    0x162200, 0x2776f0, 0xbdd50, 0x1622c0, 0xa1fa0, 0xa2430, 0x862c0, 0x48fe0,
    0xa6530, 0x9a330, 0x153190, 0x9eb60, 0x23ff20, 0x80430,
];
