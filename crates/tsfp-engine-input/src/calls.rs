// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-input/lib
//! T1848 wave 4b input port with nested guest calls. See docs/t1848-rust-wave4.md.
use crate::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

/// 002D74B0 cdecl(mask): 1 when any of the four pads (0042710 reports present) has a button
/// from `mask` in its 00042C30 state word, else 0. ESI/EDI are stored below ESP, not changed.
// portreg: va=0x002d74b0 tier=0
pub fn any_pad_has_button_mask<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    m.write32(cpu.esp.wrapping_sub(8), cpu.edi);
    let saved_esi = cpu.esi;
    let saved_edi = cpu.edi;
    let mask = stack_arg(m, cpu, 0);
    for pad in 0..4u32 {
        guest_call(c, m, cpu, 0x0004_2710, 0x002d_74be, 8, &[pad]);
        if cpu.eax == 0 {
            continue;
        }
        guest_call(c, m, cpu, 0x0004_2c30, 0x002d_74cb, 8, &[pad]);
        if mask & cpu.eax != 0 {
            cpu.eax = 1;
            cpu.esi = saved_esi;
            cpu.edi = saved_edi;
            return;
        }
    }
    cpu.eax = 0;
    cpu.esi = saved_esi;
    cpu.edi = saved_edi;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Memory(Vec<u8>);

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0[address as usize]
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }

    struct ClobberingCallee;

    impl Callee<Memory> for ClobberingCallee {
        fn call(&mut self, _m: &mut Memory, target: u32, cpu: &mut Cpu) {
            cpu.esi = 3;
            cpu.edi = 7;
            cpu.eax = match target {
                0x0004_2710 => 1,
                0x0004_2c30 => 0x80,
                _ => panic!("unexpected target {target:#x}"),
            };
        }
    }

    #[test]
    fn any_pad_button_match_restores_callee_saved_registers() {
        let mut memory = Memory(vec![0; 0x1000]);
        let mut cpu = Cpu {
            esp: 0x800,
            esi: 1,
            edi: u32::MAX,
            ..Cpu::default()
        };
        memory.write32(cpu.esp + 4, 0x80);

        any_pad_has_button_mask(&mut ClobberingCallee, &mut memory, &mut cpu);

        assert_eq!(cpu.eax, 1);
        assert_eq!(cpu.esi, 1);
        assert_eq!(cpu.edi, u32::MAX);
        assert_eq!(memory.read32(0x7fc), 1);
        assert_eq!(memory.read32(0x7f8), u32::MAX);
    }

    #[test]
    fn any_pad_button_miss_restores_callee_saved_registers() {
        let mut memory = Memory(vec![0; 0x1000]);
        let mut cpu = Cpu {
            esp: 0x800,
            esi: 2,
            edi: 9,
            ..Cpu::default()
        };
        memory.write32(cpu.esp + 4, 1);

        any_pad_has_button_mask(&mut ClobberingCallee, &mut memory, &mut cpu);

        assert_eq!(cpu.eax, 0);
        assert_eq!(cpu.esi, 2);
        assert_eq!(cpu.edi, 9);
    }
}
