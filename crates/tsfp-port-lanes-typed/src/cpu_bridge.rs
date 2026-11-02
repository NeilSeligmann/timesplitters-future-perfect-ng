// SPDX-License-Identifier: GPL-3.0-or-later
//! Runs a closure-model port (`tsfp-engine-object/src/closure/callee.rs` `Cpu` plus `Callee`) on
//! the lane register contract: the eight general registers go in and come back, a guest call
//! through `Callee` becomes a `GuestCall` with the entry FPU and XMM state carried along.
use crate::closure::callee::{Callee, Cpu};
use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

fn cpu_of(regs: &ExtRegisters) -> Cpu {
    Cpu {
        eax: regs.eax,
        ecx: regs.ecx,
        edx: regs.edx,
        ebx: regs.ebx,
        esi: regs.esi,
        edi: regs.edi,
        ebp: regs.ebp,
        esp: regs.esp,
    }
}

fn store(cpu: &Cpu, regs: &mut ExtRegisters) {
    regs.eax = cpu.eax;
    regs.ecx = cpu.ecx;
    regs.edx = cpu.edx;
    regs.ebx = cpu.ebx;
    regs.esi = cpu.esi;
    regs.edi = cpu.edi;
    regs.ebp = cpu.ebp;
    regs.esp = cpu.esp;
}

/// `Callee` over the lane's guest calls; `state` keeps the non-general registers between calls.
pub struct Bridge<'a, C> {
    guest: &'a mut C,
    state: ExtRegisters,
}

impl<M: GuestMemory, C: GuestCall> Callee<M> for Bridge<'_, C> {
    /// `cpu.esp` is the callee entry ESP (return slot, see `guest_call`); `GuestCall::call` takes
    /// the argument block and pushes the return itself, like `gp_bridge::CalleeBridge` (T3702).
    fn call(&mut self, _m: &mut M, target: u32, cpu: &mut Cpu) {
        let entry_esp = cpu.esp;
        let saved = (cpu.ebx, cpu.esi, cpu.edi, cpu.ebp);
        store(cpu, &mut self.state);
        self.state.esp = entry_esp.wrapping_add(4);
        self.guest.call(target, &mut self.state);
        *cpu = cpu_of(&self.state);
        (cpu.ebx, cpu.esi, cpu.edi, cpu.ebp) = saved;
        cpu.esp = entry_esp;
    }
}

/// Runs `port` with the registers of `regs` and stores its general registers back.
pub fn run<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    port: impl FnOnce(&mut Bridge<'_, C>, &mut M, &mut Cpu),
) {
    let mut cpu = cpu_of(regs);
    let mut bridge = Bridge {
        guest,
        state: *regs,
    };
    port(&mut bridge, m, &mut cpu);
    let state = bridge.state;
    regs.fpu = state.fpu;
    regs.xmm = state.xmm;
    regs.xmm_hi = state.xmm_hi;
    regs.eflags = state.eflags;
    store(&cpu, regs);
}

#[cfg(test)]
mod tests {
    use crate::c2r::closure_ports;
    use std::collections::BTreeMap;
    use std::vec::Vec;
    use tsfp_port_runtime::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
    use tsfp_port_runtime::memory::GuestMemory;

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

    /// Records calls; the callee returns 7 in EAX and clobbers ECX and the XMM0 lane 0.
    #[derive(Default)]
    struct Guest(Vec<(u32, u32)>);
    impl GuestCall for Guest {
        fn call(&mut self, target: u32, regs: &mut ExtRegisters) {
            self.0.push((target, regs.esp));
            regs.eax = 7;
            regs.ecx = 0x55;
            regs.ebx = 0x66;
            regs.esi = 0x77;
            regs.edi = 0x88;
            regs.ebp = 0x99;
            regs.xmm[0] = Xmm::scalar(0x3f80_0000);
        }
    }

    fn registers(esp: u32) -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 1,
            edx: 2,
            esp,
            ebx: 3,
            ebp: 4,
            esi: 5,
            edi: 6,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x27f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }

    #[test]
    fn leaf_port_sets_eax_and_keeps_the_rest() {
        let mut m = Memory::default();
        m.write32(0x1004, 0x4e);
        let mut regs = registers(0x1000);
        assert!(closure_ports::run(
            0xd6ec0,
            &mut m,
            &mut Guest::default(),
            &mut regs
        ));
        assert_eq!(regs.eax, 1);
        assert_eq!(
            (regs.esp, regs.ebx, regs.ebp, regs.esi, regs.edi),
            (0x1000, 3, 4, 5, 6)
        );
    }

    #[test]
    fn guest_call_carries_the_register_file_both_ways() {
        // 0xD6C90 maps id 0x5d through the byte table at 0xD6CBC to a target table at 0xD6CB4;
        // a target that is neither "yes" nor "no" is called (the original's indirect tail).
        let mut m = Memory::default();
        m.write32(0x1004, 0x5d);
        m.write8(0xd6cbc, 1);
        m.write32(0xd6cb4 + 4, 0x0012_3450);
        let mut regs = registers(0x1000);
        let mut guest = Guest::default();
        assert!(closure_ports::run(0xd6c90, &mut m, &mut guest, &mut regs));
        // A tail call: the bridge's return slot is the root's own (ESP one word up).
        assert_eq!(guest.0, [(0x0012_3450, 0x1004)]);
        assert_eq!((regs.eax, regs.ecx), (7, 0x55));
        assert_eq!((regs.ebx, regs.esi, regs.edi, regs.ebp), (3, 5, 6, 4));
        assert_eq!(regs.xmm[0], Xmm::scalar(0x3f80_0000));
        assert!(!closure_ports::run(0xd6c80, &mut m, &mut guest, &mut regs));
    }

    #[test]
    fn nested_guest_call_hands_over_the_argument_block() {
        // `guest_call` leaves ESP on the return slot; the lane call must see the arguments at
        // [esp] (it pushes the return itself). Before T3702 argument 0 read as the return PC.
        let mut m = Memory::default();
        let mut regs = registers(0x1000);
        let mut guest = Guest::default();
        super::run(&mut m, &mut guest, &mut regs, |c, m, cpu| {
            crate::closure::callee::guest_call(c, m, cpu, 0x45590, 0x1c_6ac1, 0, &[0xc87, 9]);
        });
        assert_eq!(guest.0, [(0x45590, 0x0ff8)]);
        assert_eq!((m.read32(0x0ff8), m.read32(0x0ffc)), (0xc87, 9));
        assert_eq!((regs.esp, regs.eax), (0x1000, 7));
    }
}
