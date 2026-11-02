// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline-only adapter for capture checking of the hand-ported weapon root.
#![no_std]

use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

/// Invoke the Tier 1 hand port for its offline Tier 2 capture entry.
pub fn run<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let _ = tsfp_ports::weapons_slice::run(0x000b_c4a0, true, m, guest, regs);
}
