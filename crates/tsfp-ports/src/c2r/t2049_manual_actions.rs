// SPDX-License-Identifier: GPL-3.0-or-later
//! T2049 readable mission-action leaf dispatch, with per-function hybrid selection.
use crate::abi::{ExtRegisters, GuestCall};
use crate::memory::GuestMemory;

pub fn run<M: GuestMemory, C: GuestCall>(va: u32, memory: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x6b7e0 => crate::mission_action_leaves::entry_gates_guest(memory, regs),
        0x25fa40 => crate::mission_action_leaves::action_value_guest(memory, regs),
        0x2661f0 => crate::character_yaw::compare_heading_guest(memory, guest, regs),
        0x344ac0 => crate::weather_presets::start_preset_guest(memory, guest, regs),
        _ => return false,
    }
    true
}
