// SPDX-License-Identifier: GPL-3.0-or-later
//! T2049 compatibility dispatch; readable logic and typed state live in mission_links.
use crate::abi::{ExtRegisters, GuestCall};
use crate::memory::GuestMemory;

pub fn run<M: GuestMemory, C: GuestCall>(va: u32, memory: &mut M, _guest: &mut C, regs: &mut ExtRegisters) -> bool {
    if va != 0x9b190 {
        return false;
    }
    crate::mission_links::lookup_guest(memory, regs);
    true
}
