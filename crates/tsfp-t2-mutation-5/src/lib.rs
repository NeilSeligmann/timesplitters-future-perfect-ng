// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only adapter for hand port 0x00363880.
#![no_std]

use tsfp_port_runtime::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};

pub fn run<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    _guest: &mut G,
    registers: &mut ExtRegisters,
) {
    tsfp_engine_render::scene_t1938::game_lighting_object_light_color_from_nuke_puzzle_cell_neighbors(
        memory, registers,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ZeroMemory;

    impl GuestMemory for ZeroMemory {
        fn read8(&self, _: u32) -> u8 {
            0
        }

        fn write8(&mut self, _: u32, _: u8) {}
    }

    impl GuestCall for ZeroMemory {
        fn call(&mut self, _: u32, _: &mut ExtRegisters) {
            panic!("unexpected guest call on the early return path")
        }
    }

    #[test]
    fn adapter_runs_hand_port_and_preserves_stack_on_nonmatching_cell() {
        let mut memory = ZeroMemory;
        let mut guest = ZeroMemory;
        let mut registers = ExtRegisters {
            eax: 1,
            ecx: 2,
            edx: 3,
            esp: 0x1000,
            ebx: 5,
            ebp: 6,
            esi: 7,
            edi: 8,
            eflags: 9,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };

        run(&mut memory, &mut guest, &mut registers);

        assert_eq!(registers.eax, 0);
        assert_eq!(registers.esp, 0x1000);
    }
}
