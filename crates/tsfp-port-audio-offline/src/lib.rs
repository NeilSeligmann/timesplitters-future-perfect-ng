// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only audio adapters; never part of live audio dispatch.
#![no_std]

use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;

macro_rules! audio_root {
    ($name:ident, $module:ident, $va:expr) => {
        pub fn $name<M: GuestMemory, C: GuestCall>(
            memory: &mut M, guest: &mut C, regs: &mut ExtRegisters,
        ) -> bool {
            tsfp_ports::$module::run($va, true, memory, guest, regs)
        }
    };
}

audio_root!(sound_buffer_update, audio, 0x28610);
audio_root!(stream_volume, audio_remainder, 0x29af0);
audio_root!(music_update, audio_remainder, 0x462f0);
audio_root!(music_start, audio, 0x46350);
audio_root!(music_stop, audio, 0x46610);
/// Audio roots exercised by the existing platform gate, available to a capture
/// checker without installing a live dispatch switch.
pub const PLATFORM_ROOTS: &[u32] = &[0x28610, 0x29af0, 0x29f30, 0x462f0, 0x46350, 0x46610];

fn platform_body(va: u32) -> Option<bool> {
    match va {
        0x28610 | 0x46350 | 0x46610 => Some(false),
        0x29af0 | 0x29f30 | 0x462f0 => Some(true),
        _ => None,
    }
}

/// Run one captured root against caller-supplied guest memory and calls.
/// Nested calls are delegated to `guest`; the checker must replay or implement
/// them and supply the captured registers, stack and complete memory read-set.
/// The translated body's register/stack convention is preserved. Returns false
/// without touching state for an unsupported root.
pub fn run_platform_root<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match platform_body(va) {
        Some(false) => tsfp_ports::audio::run(va, true, memory, guest, regs),
        Some(true) => {
            tsfp_ports::audio_remainder::run(va, true, memory, guest, regs)
        }
        _ => false,
    }
}

/// T3879 capture-only dispatch for the existing 0x29F30 translation.
///
/// This wrapper is linked only by the T2 offline lane table. It lets the offline
/// checker call the existing Rust body without changing the live selector.
pub fn sound_stream_update_3d<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    run_platform_root(0x29f30, memory, guest, regs)
}

/// T1528 0x29E40: store the stack argument at ECX+0x64 and execute RET 4.
pub fn sound_stream_set_field_64_ret4<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let value = memory.read32(regs.esp.wrapping_add(4));
    memory.write32(regs.ecx.wrapping_add(0x64), value);
    regs.eax = value;
    regs.esp = regs.esp.wrapping_add(4);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use tsfp_port_runtime::abi::{Fpu, Xmm};

    // The production offline host supplies this symbol; unit tests link the
    // translated bodies independently of that host.
    #[unsafe(no_mangle)]
    extern "C" fn tsfp_lane_set_return(_pc: u32) {}

    struct Memory([u8; 0x2000]);

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0[(address.wrapping_sub(0x1000) as usize) % self.0.len()]
        }

        fn write8(&mut self, address: u32, value: u8) {
            let index = (address.wrapping_sub(0x1000) as usize) % self.0.len();
            self.0[index] = value;
        }
    }

    struct Guest(Cell<u32>);

    impl GuestCall for Guest {
        fn call(&mut self, _va: u32, _regs: &mut ExtRegisters) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn platform_roots_match_the_translated_dispatch_tables() {
        for &va in PLATFORM_ROOTS {
            let in_audio = tsfp_ports::audio::ROOTS.contains(&va);
            let in_remainder = tsfp_ports::audio_remainder::ROOTS.contains(&va);
            assert_ne!(in_audio, in_remainder, "root {va:#x} must have one body");
            assert_eq!(platform_body(va), Some(in_remainder));
        }
        assert_eq!(PLATFORM_ROOTS.len(), 6);
    }

    #[test]
    fn writes_stack_argument_and_advances_for_ret4() {
        let mut memory = Memory([0; 0x2000]);
        let mut guest = Guest(Cell::new(0));
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0x2000,
            edx: 0,
            esp: 0x1010,
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
        assert!(!run_platform_root(0, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.esp, 0x1010);
        assert_eq!(guest.0.get(), 0);
        memory.write32(regs.esp.wrapping_add(4), 0x1234_5678);

        sound_stream_set_field_64_ret4(&mut memory, &mut guest, &mut regs);

        assert_eq!(memory.read32(0x2064), 0x1234_5678);
        assert_eq!(regs.eax, 0x1234_5678);
        assert_eq!(regs.esp, 0x1014);
        assert_eq!(guest.0.get(), 0);
    }
}
