// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only adapters for the input and core/timer platform layers.
//! These entry points are intentionally separate from live dispatch.
#![no_std]

// tsfp-ports' std-only test stub is not compiled when it is used as a dependency.
// The captured timer adapter references that host boundary, so provide the same
// deterministic stub for this crate's unit-test binary.
#[cfg(test)]
#[unsafe(no_mangle)]
extern "C" fn xbox_ReadTimeStampCounter() -> u64 {
    0
}

use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;

pub mod input {
    use super::*;

    /// Input roots included by the platform input gate.
    pub const PLATFORM_ROOTS: &[u32] = &[0x18ff70, 0x42970, 0x1ccf80, 0x438b0, 0x1cce70, 0x42b60];

    /// Run one captured input root. Unsupported roots leave all caller state intact.
    pub fn run_platform_root<M: GuestMemory, C: GuestCall>(
        va: u32,
        memory: &mut M,
        guest: &mut C,
        regs: &mut ExtRegisters,
    ) -> bool {
        if !PLATFORM_ROOTS.contains(&va) {
            return false;
        }
        tsfp_port_engine_input::c2r::engine_input::run(va, memory, guest, regs)
    }
}

pub mod core_timers {
    use super::*;

    /// Roots included by the core/timers gate.
    pub const PLATFORM_ROOTS: &[u32] = &[0x3d370, 0x3c440, 0x1538c0];

    /// Run one captured core/timer root without changing live dispatch selection.
    pub fn run_platform_root<M: GuestMemory, C: GuestCall>(
        va: u32,
        memory: &mut M,
        guest: &mut C,
        regs: &mut ExtRegisters,
    ) -> bool {
        match va {
            0x3d370 => {
                tsfp_ports::ext::frame_loop(memory, guest, regs);
                true
            }
            0x3c440 => {
                tsfp_ports::c2r::t2049_exec_003::game_frame_update_and_render_all_players(
                    memory, guest, regs,
                );
                true
            }
            0x1538c0 => {
                tsfp_ports::c2r::t2123_manual::game_frame_timing_update(memory, guest, regs);
                true
            }
            _ => false,
        }
    }
}

/// Root-specific entry points used by the generated T2 offline lane table.
/// The lane table owns RET cleanup metadata; these wrappers keep root selection
/// and translated dispatch in this capture-only adapter crate.
pub mod offline_dispatch {
    use super::*;

    macro_rules! input_root {
        ($name:ident, $va:expr) => {
            pub fn $name<M: GuestMemory, C: GuestCall>(
                memory: &mut M,
                guest: &mut C,
                regs: &mut ExtRegisters,
            ) -> bool {
                super::input::run_platform_root($va, memory, guest, regs)
            }
        };
    }

    macro_rules! core_timer_root {
        ($name:ident, $va:expr) => {
            pub fn $name<M: GuestMemory, C: GuestCall>(
                memory: &mut M,
                guest: &mut C,
                regs: &mut ExtRegisters,
            ) -> bool {
                super::core_timers::run_platform_root($va, memory, guest, regs)
            }
        };
    }

    input_root!(input_poll, 0x18ff70);
    input_root!(input_axis, 0x42970);
    input_root!(input_sequence, 0x1ccf80);
    input_root!(input_poll_all, 0x438b0);
    input_root!(input_ring_push, 0x1cce70);
    input_root!(input_pad_entry, 0x42b60);

    core_timer_root!(frame_loop, 0x3d370);
    core_timer_root!(frame_update, 0x3c440);
    core_timer_root!(frame_timing_update, 0x1538c0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use tsfp_port_runtime::abi::{Fpu, Xmm};

    struct Memory([u8; 0x4000]);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(address as usize).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            if let Some(slot) = self.0.get_mut(address as usize) {
                *slot = value;
            }
        }
    }

    struct Guest(Cell<u32>);
    impl GuestCall for Guest {
        fn call(&mut self, va: u32, _regs: &mut ExtRegisters) {
            self.0.set(self.0.get().wrapping_add(va));
        }
    }

    fn regs() -> ExtRegisters {
        ExtRegisters {
            eax: 1,
            ecx: 2,
            edx: 3,
            esp: 0x1000,
            ebx: 4,
            ebp: 5,
            esi: 6,
            edi: 7,
            eflags: 0x202,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }

    #[test]
    fn input_dispatch_declines_unknown_root_without_mutation() {
        let mut memory = Memory([0; 0x4000]);
        let mut guest = Guest(Cell::new(0));
        let mut registers = regs();
        let before = registers;
        assert!(!input::run_platform_root(
            0xdead,
            &mut memory,
            &mut guest,
            &mut registers
        ));
        assert_eq!(registers.eax, before.eax);
        assert_eq!(registers.esp, before.esp);
        assert_eq!(guest.0.get(), 0);
    }

    #[test]
    fn core_timer_dispatch_declines_unknown_root_without_mutation() {
        let mut memory = Memory([0; 0x4000]);
        let mut guest = Guest(Cell::new(0));
        let mut registers = regs();
        let before = registers;
        assert!(!core_timers::run_platform_root(
            0xdead,
            &mut memory,
            &mut guest,
            &mut registers
        ));
        assert_eq!(registers.eax, before.eax);
        assert_eq!(registers.esp, before.esp);
        assert_eq!(guest.0.get(), 0);
    }

    #[test]
    fn platform_root_sets_are_explicit_and_unique() {
        for roots in [input::PLATFORM_ROOTS, core_timers::PLATFORM_ROOTS] {
            for (index, root) in roots.iter().enumerate() {
                assert!(!roots[..index].contains(root));
            }
        }
        assert_eq!(input::PLATFORM_ROOTS.len(), 6);
        assert_eq!(core_timers::PLATFORM_ROOTS.len(), 3);
    }

    #[test]
    fn offline_input_wrapper_runs_a_supported_synthetic_root() {
        let mut memory = Memory([0; 0x4000]);
        let mut guest = Guest(Cell::new(0));
        let mut registers = regs();

        assert!(offline_dispatch::input_axis(
            &mut memory,
            &mut guest,
            &mut registers
        ));
    }
}
