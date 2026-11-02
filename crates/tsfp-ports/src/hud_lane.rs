// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/ui_hud
// portreg: mirrors=tsfp-ports/c2r/ui_framework
// portreg: mirrors=tsfp-ports/c2r/ui_hud_2
// portreg: mirrors=tsfp-ports/c2r/t2049_exec_001
//! Tier 2 adapters for Tier 1 HUD translations (`c2r/ui_hud.rs`, T1878/T1974; docs/t-hud-slice.md).
//! The ui/framework roots use `c2r/ui_framework` (T2409w); `lane_inv.rs` registers roots by execution weight.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::{t2049_exec_001, ui_framework, ui_hud, ui_hud_2},
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
        0x80f20 => t2049_exec_001::run(va, m, guest, regs),
        0x2e5900 | 0x315e50 => ui_hud_2::run(va, m, guest, regs),
        0x40260 | 0x23f2e0 | 0x2d7240 | 0x12cb40 | 0x12cbb0 | 0x336570 | 0xfc9f0 | 0x42f90 => {
            ui_hud::run(va, m, guest, regs)
        }
        0x2d8100 | 0x81ec0 | 0x56120 | 0x560e0 | 0x42fd0 | 0x1ccec0 | 0x1cef70 | 0x1ce9a0
        | 0xfe000 | 0x14d480 | 0x81ce0 | 0x2e4b80 | 0xf9250 | 0x14af00 => {
            ui_hud::run(va, m, guest, regs)
        }
        0x3fcf0 | 0x3f8d0 | 0x74760 | 0x40740 | 0x722b0 | 0x40fa0 | 0x77470 | 0x774b0 | 0x40f00
        | 0x76400 | 0x757e0 => ui_framework::run(va, m, guest, regs),
        _ => false,
    }
}

/// Lane roots in registration order: T1974/T3734 and T2409w ui/framework roots; see the respective Tier 2 docs.
pub const ROOTS: [u32; 36] = [
    0x40260, 0x23f2e0, 0x2d7240, 0x12cb40, 0x12cbb0, 0x336570, 0xfc9f0, 0x42f90, 0x80f20, 0x2e5900,
    0x315e50, 0x2d8100, 0x81ec0, 0x56120, 0x560e0, 0x42fd0, 0x1ccec0, 0x1cef70, 0x1ce9a0, 0xfe000,
    0x14d480, 0x81ce0, 0x2e4b80, 0xf9250, 0x14af00, 0x3fcf0, 0x3f8d0, 0x74760, 0x40740, 0x722b0,
    0x40fa0, 0x77470, 0x774b0, 0x40f00, 0x76400, 0x757e0,
];
