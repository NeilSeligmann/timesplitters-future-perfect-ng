// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable native route for the bounded CRT atexit-table caller at `0x3C8A84`.
//!
//! `TSFP_RUST_GAME_ARENA=1` opts in. The retained C2R body remains the default and oracle. Heap
//! sizing and realloc still call their existing guest helpers; Rust owns the table growth and
//! publication policy described by the T2365 revision-2 result.

#[cfg(feature = "std")]
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
};
#[cfg(feature = "std")]
use tsfp_engine_memory::crt_atexit::{self, AtexitHost};

#[cfg(feature = "std")]
struct RuntimeHost<'a, M: GuestMemory, C: GuestCall> {
    memory: &'a mut M,
    guest: &'a mut C,
    registers: &'a mut ExtRegisters,
}

#[cfg(feature = "std")]
impl<M: GuestMemory, C: GuestCall> AtexitHost for RuntimeHost<'_, M, C> {
    fn read32(&mut self, address: u32) -> Result<u32, tsfp_engine_memory::MemoryFault> {
        Ok(self.memory.read32(address))
    }

    fn write32(&mut self, address: u32, value: u32) -> Result<(), tsfp_engine_memory::MemoryFault> {
        self.memory.write32(address, value);
        Ok(())
    }

    fn heap_size(
        &mut self,
        heap_handle: u32,
        pointer: u32,
    ) -> Result<u32, tsfp_engine_memory::MemoryFault> {
        // Original push order is heap handle then pointer, so the cdecl stack is (pointer, handle).
        call_cdecl(
            self.guest,
            self.memory,
            self.registers,
            0x3CC96B,
            &[pointer, heap_handle],
        );
        Ok(self.registers.eax)
    }

    fn reallocate(
        &mut self,
        pointer: u32,
        requested_size: u32,
    ) -> Result<u32, tsfp_engine_memory::MemoryFault> {
        call_cdecl(
            self.guest,
            self.memory,
            self.registers,
            0x3CAEEF,
            &[pointer, requested_size],
        );
        Ok(self.registers.eax)
    }
}

/// Select the readable Rust implementation only when the game allocator switch is explicitly on.
#[cfg(feature = "std")]
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    registers: &mut ExtRegisters,
) -> bool {
    if va == 0x3C9800 && std::env::var("TSFP_RUST_CRT_REALLOC").is_ok_and(|v| v == "1") {
        let destination = memory.read32(registers.esp.wrapping_add(4));
        let source = memory.read32(registers.esp.wrapping_add(8));
        let count = memory.read32(registers.esp.wrapping_add(12));
        tsfp_engine_memory::crt_memmove::move_payload_with(
            destination,
            source,
            count,
            |write, address, byte| {
                if write {
                    memory.write8(address, *byte);
                } else {
                    *byte = memory.read8(address);
                }
                Ok(())
            },
        )
        .expect("guest memory is mapped for CRT realloc copy");
        return true;
    }
    if va != 0x3C8A84 || !game_arena_rust_enabled() {
        return false;
    }

    let entry = memory.read32(registers.esp.wrapping_add(4));
    let heap_handle = registers.esi;
    // The lifted caller leaves the pre-growth used-byte count in EDX. It computes this before
    // branching into either the append or realloc path, and EDX is live at its caller.
    registers.edx = memory
        .read32(0x7EB844)
        .wrapping_sub(memory.read32(0x7EB848))
        .wrapping_add(4);
    let pre_growth_used_bytes = registers.edx;
    let mut host = RuntimeHost {
        memory,
        guest,
        registers,
    };
    let result = crt_atexit::register_atexit(&mut host, heap_handle, entry)
        .expect("guest memory is mapped for CRT atexit registration");
    host.registers.eax = result;
    // The lifted body returns its pre-growth byte count in EDX; helper calls may clobber it.
    host.registers.edx = pre_growth_used_bytes;
    true
}

#[cfg(feature = "std")]
fn game_arena_rust_enabled() -> bool {
    std::env::var("TSFP_RUST_GAME_ARENA").is_ok_and(|value| value == "1")
}

/// The native atexit route is host-only; keep the no_std archive on the retained C route.
#[cfg(not(feature = "std"))]
pub fn run<M, C>(
    _va: u32,
    _memory: &mut M,
    _guest: &mut C,
    _registers: &mut crate::abi::ExtRegisters,
) -> bool {
    false
}
