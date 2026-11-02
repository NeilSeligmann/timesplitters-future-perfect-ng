// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable mapmaker mode validation callbacks (0x3127D0 and 0x3127F0).
//!
//! The validation service remains a guest call until its own contract is ported.
//! The guest ABI is kept here so the menu callback roots can run in the hybrid lane.
use crate::abi::{ExtRegisters, GuestCall};
use crate::memory::GuestMemory;

const VALIDATE_MODE: u32 = 0x3265a0;

#[derive(Clone, Copy)]
enum Mode { Story = 0, Assault = 1 }

fn validate<M: GuestMemory, C: GuestCall>(mode: Mode, memory: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let caller_esp = regs.esp;
    let caller_ebp = regs.ebp;
    regs.esp = regs.esp.wrapping_sub(4);
    memory.write32(regs.esp, mode as u32);
    guest.call(VALIDATE_MODE, regs);
    regs.ebp = caller_ebp; // The guest bridge exposes its temporary C frame.
    regs.esp = caller_esp;
    regs.eax = u32::from(regs.eax != 0);
}

/// Handle the two mode callbacks before the generated fallback in `c2r::run`.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, memory: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    let mode = match va {
        0x3127d0 => Mode::Story,
        0x3127f0 => Mode::Assault,
        _ => return false,
    };
    validate(mode, memory, guest, regs);
    true
}
