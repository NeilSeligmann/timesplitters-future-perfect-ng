// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only ABI adapters for existing Rust-owned miscellaneous ports.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory as CpuGuestMemory,
};
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

struct GuestBridge<'a, C> {
    guest: &'a mut C,
    state: ExtRegisters,
}

struct ClosureMemory<'a, M>(&'a mut M);

impl<M: GuestMemory> CpuGuestMemory for ClosureMemory<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value);
    }
}

impl<M: GuestMemory> GuestMemory for ClosureMemory<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value);
    }
}

impl<M: GuestMemory + CpuGuestMemory, C: GuestCall> Callee<M> for GuestBridge<'_, C> {
    fn call(&mut self, _memory: &mut M, target: u32, cpu: &mut Cpu) {
        let entry_esp = cpu.esp;
        store(cpu, &mut self.state);
        self.state.esp = entry_esp.wrapping_add(4);
        self.guest.call(target, &mut self.state);
        *cpu = cpu_of(&self.state);
        cpu.esp = entry_esp;
    }
}

fn run_cpu_port<M, C>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    port: impl FnOnce(&mut GuestBridge<'_, C>, &mut M, &mut Cpu),
) where
    M: GuestMemory + CpuGuestMemory,
    C: GuestCall,
{
    let mut cpu = cpu_of(regs);
    let mut bridge = GuestBridge {
        guest,
        state: *regs,
    };
    port(&mut bridge, memory, &mut cpu);
    let state = bridge.state;
    regs.fpu = state.fpu;
    regs.xmm = state.xmm;
    regs.xmm_hi = state.xmm_hi;
    regs.eflags = state.eflags;
    store(&cpu, regs);
}

/// Offline host entry for the existing 0x00235240 closure-model port.
pub fn f235240<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let mut memory = ClosureMemory(m);
    run_cpu_port(&mut memory, guest, regs, |c, m, cpu| {
        tsfp_engine_object::w4b_object::trigger_condition_check(c, m, cpu)
    });
}

/// Offline host entry for the existing 0x0009B190 register-ABI port.
pub fn f9b190<M: GuestMemory, C: GuestCall>(m: &mut M, _guest: &mut C, regs: &mut ExtRegisters) {
    tsfp_ports::mission_links::lookup_guest(m, regs)
}

/// Offline host entry for the existing 0x001F2560 closure-model port.
pub fn f1f2560<M: GuestMemory, C: GuestCall>(_m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    guest.call(0x001e_ae30, regs);
}

/// Offline host entry for the existing 0x00012860 register-ABI port.
pub fn f12860<M: GuestMemory, C: GuestCall>(m: &mut M, _guest: &mut C, regs: &mut ExtRegisters) {
    tsfp_engine_render::scene_batch::game_model_vertex_position_read_dequantized(m, regs)
}

/// Offline host entry for the existing 0x000129E0 register-ABI port.
pub fn f129e0<M: GuestMemory, C: GuestCall>(m: &mut M, _guest: &mut C, regs: &mut ExtRegisters) {
    tsfp_engine_render::scene_batch::game_model_vertex_position_read_dequantized_vec4(m, regs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_port_runtime::abi::{Fpu, Xmm};

    struct Ram([u8; 4]);

    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0[address as usize]
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }

    #[derive(Default)]
    struct Calls(Vec<u32>);

    impl GuestCall for Calls {
        fn call(&mut self, target: u32, _regs: &mut ExtRegisters) {
            self.0.push(target);
        }
    }

    #[test]
    fn closure_memory_forwards_both_memory_traits() {
        let mut ram = Ram([1, 2, 3, 4]);
        let mut memory = ClosureMemory(&mut ram);
        assert_eq!(CpuGuestMemory::read8(&memory, 0), 1);
        CpuGuestMemory::write8(&mut memory, 0, 9);
        assert_eq!(GuestMemory::read8(&memory, 0), 9);
        GuestMemory::write8(&mut memory, 1, 8);
        assert_eq!(CpuGuestMemory::read8(&memory, 1), 8);
    }

    #[test]
    fn f1f2560_forwards_the_thunk_target() {
        let mut ram = Ram([0; 4]);
        let mut calls = Calls::default();
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };
        f1f2560(&mut ram, &mut calls, &mut regs);
        assert_eq!(calls.0, [0x001e_ae30]);
    }
}
