// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-port-gameplay-modes-multiplayer-1/lib
// portreg: wraps=tsfp-gameplay/t1900_multiplayer
//! T1923: lane adapters (Tier 2 through `tools.rust_trace`) for the T1900 multiplayer ports that the Story route reaches:
//! 0xC0C50 (flag dropped countdown tick, EAX = flag object) and 0xCB240 (leave party mode, cdecl 0). The pure ports live in
//! `tsfp-port-gameplay-modes-multiplayer-1` and `tsfp-gameplay` (`t1900_multiplayer.rs`, compiled here by `#[path]`, Tier 1 against the lifted
//! original in `crates/tsfp-c-diff`). Same `run` contract as `anim_lane.rs`, registered through `lane_inv.rs`. The other 15 T1900
//! functions are not reached by the Story route (docs/t1923-tier2.md) and have no adapter.
use crate::{
    abi::{ExtRegisters, GuestCall},
    gp_bridge::{CalleeBridge, cpu_of, store_cpu},
    memory::GuestMemory,
};

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/t1900_multiplayer.rs"]
mod multiplayer;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-port-gameplay-modes-multiplayer-1/src/lib.rs"]
mod team_pickup;

/// Dispatches a root; `false` when `va` is not one of the multiplayer adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    let entry = *regs;
    let mut cpu = cpu_of(&entry);
    let mut callee = CalleeBridge::new(guest, &entry);
    match va {
        0xc0c50 => team_pickup::dropped_countdown_tick(&mut callee, m, &mut cpu),
        0xcb240 => multiplayer::leave_party_mode(&mut callee, m, &mut cpu),
        _ => return false,
    }
    store_cpu(regs, &cpu);
    true
}

/// The multiplayer lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 2] = [0xc0c50, 0xcb240];
