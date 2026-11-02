// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/gamestate_wired
// portreg: mirrors=tsfp-port-gameplay-modes/c2r/modes_deferred
//! T2082 (S3 writer wiring 1): lane adapters for the lifted writers of the Rust-owned `GameState` cells
//! (level `0x79094C`, players `0x790950`, extra players `0x7A2958`, mode byte `0x7DE455`, session flags
//! `0x7DE458`, match state `0x7DE30C`). The bodies are the c2r translations of `c2r/gamestate_wired.rs`
//! (Tier 1 differential vs lifted); their stores to those cells go through `state_wire` and so through the
//! tsfp-state `GameState` setters. Selecting a root swaps the lifted writer for the wired Rust one; default
//! behaviour is unchanged (the roots are opt-in under the per-function A/B switch).
//! Not listed here: `0x65D20`, `0x668F0`, `0x23E930` (already selectable through the player lane, whose
//! dispatch, `player_system::run`, now answers them from `gamestate_wired` first) and the modes-lane writers
//! `0x67280`, `0x68610`, `0x68830`, `0x681C0` (raw stores, remaining debt, docs/t-s3-wiring-gamestate.md).
//! Registered through `lane_inv.rs`.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::gamestate_wired,
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
        0x65230 | 0x653b0 | 0x65420 | 0x65470 | 0x658d0 | 0x65c70 | 0x65f40 | 0x667f0 | 0x668a0
        | 0x66fc0 | 0x670c0 | 0x674c0 | 0x67450 | 0x67e60 | 0x67f00 | 0x68720 | 0x15df20
        | 0x15e4f0 | 0x22ce60 | 0x23efe0 | 0x25f830 => {
            if va == 0x670c0 && crate::story_activation_lane::run(va, m, guest, regs) {
                return true;
            }
            crate::state_wire::audit_entry(&*m, va);
            gamestate_wired::run(va, m, guest, regs)
        }
        _ => false,
    }
}

/// The lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 21] = [
    0x65230, 0x653b0, 0x65420, 0x65470, 0x658d0, 0x65c70, 0x65f40, 0x667f0, 0x668a0, 0x66fc0,
    0x670c0, 0x674c0, 0x67450, 0x67e60, 0x67f00, 0x68720, 0x15df20, 0x15e4f0, 0x22ce60, 0x23efe0,
    0x25f830,
];
