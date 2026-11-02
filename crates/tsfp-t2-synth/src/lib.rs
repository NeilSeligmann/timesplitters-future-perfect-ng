// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only bridge for synthesized object leaves.
#![no_std]

use tsfp_engine_object::ObjectMemory;
pub use tsfp_engine_object::synth_leaves;
use tsfp_port_runtime::{abi::ExtRegisters, memory::GuestMemory};

pub struct Memory<'a, M>(pub &'a mut M);
impl<M: GuestMemory> ObjectMemory for Memory<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn read16(&self, address: u32) -> u16 {
        self.0.read16(address)
    }
    fn read32(&self, address: u32) -> u32 {
        self.0.read32(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value);
    }
    fn write32(&mut self, address: u32, value: u32) {
        self.0.write32(address, value);
    }
}

/// Convert the general registers and direction flag to the synthesized CPU contract.
pub fn run<'a, M: GuestMemory>(
    m: &'a mut M,
    regs: &mut ExtRegisters,
    port: impl FnOnce(&mut Memory<'a, M>, &mut synth_leaves::Cpu),
) {
    let mut cpu = synth_leaves::Cpu {
        eax: regs.eax,
        ecx: regs.ecx,
        edx: regs.edx,
        ebx: regs.ebx,
        esi: regs.esi,
        edi: regs.edi,
        ebp: regs.ebp,
        esp: regs.esp,
        df: regs.eflags & 0x400 != 0,
    };
    port(&mut Memory(m), &mut cpu);
    regs.eax = cpu.eax;
    regs.ecx = cpu.ecx;
    regs.edx = cpu.edx;
    regs.ebx = cpu.ebx;
    regs.esi = cpu.esi;
    regs.edi = cpu.edi;
    regs.ebp = cpu.ebp;
    regs.esp = cpu.esp;
    regs.eflags = (regs.eflags & !0x400) | if cpu.df { 0x400 } else { 0 };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_port_runtime::abi::{Fpu, Xmm};

    struct TestMemory([u8; 16]);
    impl GuestMemory for TestMemory {
        fn read8(&self, address: u32) -> u8 {
            self.0[address as usize]
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }

    #[test]
    fn bridge_preserves_registers_and_forwards_memory_and_direction_flag() {
        let mut memory = TestMemory([0; 16]);
        let mut regs = ExtRegisters {
            eax: 1,
            ecx: 2,
            edx: 3,
            ebx: 4,
            esi: 5,
            edi: 6,
            ebp: 7,
            esp: 8,
            eflags: 0x401,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };
        run(&mut memory, &mut regs, |mem, cpu| {
            assert_eq!(cpu.to_array(), [1, 2, 3, 5, 6, 4, 7, 8, 1]);
            mem.write32(0, 0x12345678);
            cpu.eax = mem.read32(0);
            cpu.df = false;
        });
        assert_eq!(regs.eax, 0x12345678);
        assert_eq!(
            (
                regs.ecx, regs.edx, regs.ebx, regs.esi, regs.edi, regs.ebp, regs.esp
            ),
            (2, 3, 4, 5, 6, 7, 8)
        );
        assert_eq!(regs.eflags, 1);
    }
}
