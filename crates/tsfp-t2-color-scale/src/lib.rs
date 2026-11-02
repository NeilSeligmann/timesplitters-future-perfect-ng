// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only ABI adapter for the packed color scaler.
#![no_std]

use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

/// Run 0x154FE0 from its captured cdecl stack without registering it for live dispatch.
///
/// Keep the original x87 conversion boundary: `ftol_adjust` consumes the product and updates
/// EAX/EDX (and sometimes ECX). Its call is part of the Tier 2 captured behavior.
pub fn scale_colour<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let alpha_at_entry = regs.esp.wrapping_add(8);
    let alpha = f32::from_bits(memory.read32(alpha_at_entry));
    let one = f32::from_bits(memory.read32(0x475c78));
    if alpha > one {
        memory.write32(alpha_at_entry, one.to_bits());
    } else if !(0.0 <= alpha || alpha.is_nan()) {
        memory.write32(alpha_at_entry, 0);
    }

    let caller_ebp = regs.ebp;
    regs.esp = regs.esp.wrapping_sub(4);
    memory.write32(regs.esp, regs.esi);
    regs.esi = memory.read32(regs.esp.wrapping_add(8));
    regs.eax = regs.esi & 0xff;
    memory.write32(regs.esp.wrapping_add(8), regs.eax);
    regs.fpu.push((regs.eax as i32) as f64);
    if (regs.eax as i32) < 0 {
        let adjusted = regs
            .fpu
            .round_single(regs.fpu.st(0) + f32::from_bits(memory.read32(0x475ccc)) as f64);
        regs.fpu.stack[regs.fpu.top as usize] = adjusted;
    }
    let multiplier = f32::from_bits(memory.read32(regs.esp.wrapping_add(0xc))) as f64;
    let scaled = regs.fpu.round_single(regs.fpu.st(0) * multiplier);
    regs.fpu.stack[regs.fpu.top as usize] = scaled;
    // `GuestCall` owns the bridge return slot and restores ESP to this saved-ESI local.
    guest.call(0x3c848c, regs);
    regs.ebp = caller_ebp;
    regs.eax |= regs.esi & 0xffff_ff00;
    regs.esi = memory.read32(regs.esp);
    regs.esp = regs.esp.wrapping_add(4);
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::vec::Vec;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[derive(Default)]
    struct Calls(Vec<u32>);

    impl GuestCall for Calls {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.0.push(va);
            regs.eax = regs.fpu.st(0) as u32;
            regs.edx = 0;
            regs.fpu.pop();
            // GuestCall owns the bridge return slot and restores ESP for the caller.
        }
    }

    #[test]
    fn captured_adapter_calls_ftol_and_preserves_the_caller_stack() {
        let mut memory = Memory::default();
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x1000,
            ebx: 0,
            ebp: 0x2000,
            esi: 0xdead_beef,
            edi: 0,
            eflags: 0,
            fpu: tsfp_port_runtime::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x300,
            },
            xmm: [tsfp_port_runtime::abi::Xmm::default(); 5],
            xmm_hi: [tsfp_port_runtime::abi::Xmm::default(); 3],
        };
        memory.write32(0x1004, 0x1234_5680);
        memory.write32(0x1008, 0.5f32.to_bits());
        memory.write32(0x475c78, 1.0f32.to_bits());

        let mut calls = Calls::default();
        scale_colour(&mut memory, &mut calls, &mut regs);

        assert_eq!(calls.0, [0x3c848c]);
        assert_eq!(regs.eax, 0x1234_5640);
        assert_eq!(regs.esi, 0xdead_beef);
        assert_eq!(regs.esp, 0x1000);
        assert_eq!(regs.fpu.top, 0);
    }
}
