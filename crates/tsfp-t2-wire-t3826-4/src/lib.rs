// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]

use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    calls_w8,
    memory::GuestMemory as ClosureMemory,
};
use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

/// Capture-only adapter for the hand port at 0x001845C0.
pub fn game_table_7a18a0_merge<M: GuestMemory>(
    m: &mut M,
    _guest: &mut impl GuestCall,
    r: &mut ExtRegisters,
) {
    tsfp_port_engine_object_4::c2r::engine_object_3::game_table_7a18a0_merge_overlapping_entry_boxes_and_sync_counts(m, r);
}

/// Capture-only adapters for the two call-bearing hand ports at 0x001EC900/0x001EC940.
pub fn game_object_gas0_type_4c<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    tsfp_port_engine_object_5::c2r::engine_object_4::game_object_key_0x47415330_type_0x4c_global_75bf0c_1_and_field_0x1cc_above_zero(m, guest, r);
}

pub fn game_object_gas1_type_4c<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    tsfp_port_engine_object_5::c2r::engine_object_4::game_object_key_0x47415331_type_0x4c_global_75bf0c_1_and_field_0x1cc_above_zero(m, guest, r);
}

struct Memory<'a, M>(&'a mut M);

impl<M: GuestMemory> ClosureMemory for Memory<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value);
    }
}

struct Guest<'a, C>(&'a mut C);

impl<M: ClosureMemory, C: GuestCall> Callee<M> for Guest<'_, C> {
    fn call(&mut self, _m: &mut M, target: u32, cpu: &mut Cpu) {
        let mut r = ExtRegisters {
            eax: cpu.eax,
            ecx: cpu.ecx,
            edx: cpu.edx,
            // `Callee` includes the return slot at ESP; `GuestCall` expects ESP at arg 0.
            esp: cpu.esp.wrapping_add(4),
            ebx: cpu.ebx,
            ebp: cpu.ebp,
            esi: cpu.esi,
            edi: cpu.edi,
            eflags: 0,
            fpu: tsfp_port_runtime::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [tsfp_port_runtime::abi::Xmm::default(); 5],
            xmm_hi: [tsfp_port_runtime::abi::Xmm::default(); 3],
        };
        self.0.call(target, &mut r);
        cpu.eax = r.eax;
        cpu.ecx = r.ecx;
        cpu.edx = r.edx;
        cpu.esp = r.esp.wrapping_sub(4);
        cpu.ebx = r.ebx;
        cpu.ebp = r.ebp;
        cpu.esi = r.esi;
        cpu.edi = r.edi;
    }
}

fn run_closure<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    gas1: bool,
) {
    let mut memory = Memory(m);
    let mut bridge = Guest(guest);
    let mut cpu = Cpu {
        eax: regs.eax,
        ecx: regs.ecx,
        edx: regs.edx,
        ebx: regs.ebx,
        esi: regs.esi,
        edi: regs.edi,
        ebp: regs.ebp,
        esp: regs.esp,
    };
    if gas1 {
        calls_w8::object_key_gas1_type_0x4c(&mut bridge, &mut memory, &mut cpu);
    } else {
        calls_w8::object_key_gas0_type_0x4c(&mut bridge, &mut memory, &mut cpu);
    }
    regs.eax = cpu.eax;
    regs.ecx = cpu.ecx;
    regs.edx = cpu.edx;
    regs.esp = cpu.esp;
    regs.ebx = cpu.ebx;
    regs.ebp = cpu.ebp;
    regs.esi = cpu.esi;
    regs.edi = cpu.edi;
}

/// Capture-only adapters for the closure hand ports at 0x001EC980/0x001EC9C0.
pub fn object_gas0_type_4c<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    run_closure(m, guest, r, false);
}

pub fn object_gas1_type_4c<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    run_closure(m, guest, r, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Memory;

    impl ClosureMemory for Memory {
        fn read8(&self, _address: u32) -> u8 {
            0
        }

        fn write8(&mut self, _address: u32, _value: u8) {}
    }

    struct GuestRecorder {
        target: u32,
        entry_esp: u32,
    }

    impl GuestCall for GuestRecorder {
        fn call(&mut self, target: u32, regs: &mut ExtRegisters) {
            self.target = target;
            self.entry_esp = regs.esp;
            regs.eax = 0x1234;
            regs.edi = 0x5678;
        }
    }

    #[test]
    fn guest_bridge_forwards_target_and_register_updates() {
        let mut recorder = GuestRecorder {
            target: 0,
            entry_esp: 0,
        };
        let mut bridge = Guest(&mut recorder);
        let mut memory = Memory;
        let mut cpu = Cpu {
            eax: 1,
            ecx: 2,
            edx: 3,
            ebx: 4,
            esi: 5,
            edi: 6,
            ebp: 7,
            esp: 8,
        };

        bridge.call(&mut memory, 0x12345678, &mut cpu);

        assert_eq!(recorder.target, 0x12345678);
        assert_eq!(recorder.entry_esp, 12);
        assert_eq!(cpu.eax, 0x1234);
        assert_eq!(cpu.edi, 0x5678);
        assert_eq!(
            (cpu.ecx, cpu.edx, cpu.ebx, cpu.esi, cpu.ebp, cpu.esp),
            (2, 3, 4, 5, 7, 8)
        );
    }
}
