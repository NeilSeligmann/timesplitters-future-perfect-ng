// SPDX-License-Identifier: GPL-3.0-or-later
//! Borrowed Story buffer boundary for 0x227C00. Layout meaning is INFERRED.
use crate::{abi::{ExtRegisters, GuestCall}, memory::GuestMemory};

/// The pointer and byte count live in adjacent Story globals, but have distinct lifetimes.
#[derive(Clone, Copy)]
struct StoryBuffer { address: u32, bytes: u32 }
impl StoryBuffer {
    fn clear<M: GuestMemory>(self, memory: &mut M) {
        // Preserve the original's dword stores, byte tail, and wrapping guest addresses.
        for index in 0..self.bytes / 4 {
            memory.write32(self.address.wrapping_add(index.wrapping_mul(4)), 0);
        }
        let tail = self.address.wrapping_add((self.bytes / 4).wrapping_mul(4));
        for index in 0..self.bytes % 4 {
            memory.write8(tail.wrapping_add(index), 0);
        }
    }
}

/// Tier 0 readable override; the generated c2r root remains the A/B fallback.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32, memory: &mut M, _guest: &mut C, registers: &mut ExtRegisters,
) -> bool {
    if va != 0x227c00 { return false; }

    // The translated callback saves EDI to the guest stack before reading globals.
    let saved_edi = registers.edi;
    registers.esp = registers.esp.wrapping_sub(4);
    memory.write32(registers.esp, saved_edi);
    registers.edi = memory.read32(0x75c148);
    if registers.edi != 0 {
        registers.ecx = memory.read32(0x75c150);
        if registers.ecx != 0 {
            let buffer = StoryBuffer { address: registers.edi, bytes: registers.ecx };
            registers.edx = buffer.bytes;
            registers.eax = 0;
            buffer.clear(memory);
            registers.edi = buffer.address.wrapping_add(buffer.bytes);
            registers.ecx = 0;
        }
    }
    registers.edi = memory.read32(registers.esp);
    registers.esp = registers.esp.wrapping_add(4);
    true
}
