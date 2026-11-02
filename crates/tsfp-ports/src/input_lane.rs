// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/engine_input
// portreg: mirrors=tsfp-port-ai-pathfinding/c2r/ai_pathfinding
//! T1988: lane adapters (Tier 2 through `tools.rust_trace`) for the heaviest `engine/input` and `ai/pathfinding` functions
//! that already have a Tier 1 machine translation in `c2r/engine_input.rs` and `c2r/ai_pathfinding.rs`. Same pattern as
//! `hud_lane.rs` (T1974): this file only gates which roots the host may select. Registered through `lane_inv.rs`. Roots
//! are ordered by execution weight (`docs/data/t1884-exec-weights.csv`, calls): input 0x18FF70 (49,372), 0x42970 (41,231),
//! 0x1CCF80 (12,339), 0x438B0 (12,339), 0x1CCE70 (12,334), 0x42B60 (12,330), 0x426A0 (12,289), 0x190520 (9,640);
//! pathfinding 0x2AEFD0 (2) and 0x2A5C30 (2), the only reached pathfinding translations. T2122 routes
//! 0x2AEFD0 through the leaf-setter translation because this root also writes CharacterSet fields.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::{ai_pathfinding, engine_input, objchar_wired30},
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
        0x18ff70 | 0x1ccf80 | 0x438b0 | 0x1cce70 | 0x42b60 | 0x426a0 | 0x190520 => {
            engine_input::run(va, m, guest, regs)
        }
        0x42970 => {
            crate::input_axis_quality::read(m, regs);
            true
        }
        0x2aefd0 => objchar_wired30::run(va, m, guest, regs),
        0x2a5c30 => ai_pathfinding::run(va, m, guest, regs),
        _ => false,
    }
}

/// The lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 10] = [
    0x18ff70, 0x42970, 0x1ccf80, 0x438b0, 0x1cce70, 0x42b60, 0x426a0, 0x190520, 0x2aefd0, 0x2a5c30,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_tick_has_leaf_setter_translation() {
        assert!(ROOTS.contains(&0x2aefd0));
        assert!(objchar_wired30::ROOTS.contains(&0x2aefd0));
    }
}
