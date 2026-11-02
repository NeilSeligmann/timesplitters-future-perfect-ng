// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only bridge for readable cutscene ports.

use tsfp_data_formats::cutscenes::CutsceneState;
use tsfp_engine_memory::{GuestMemory as TypedMemory, MemoryFault};
use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
use tsfp_types::GuestAddr;

struct Memory<'a, M>(&'a mut M);

impl<M: GuestMemory> TypedMemory for Memory<'_, M> {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        for (offset, byte) in out.iter_mut().enumerate() {
            *byte = self.0.read8(address.0.wrapping_add(offset as u32));
        }
        Ok(())
    }

    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        for (offset, byte) in bytes.iter().enumerate() {
            self.0.write8(address.0.wrapping_add(offset as u32), *byte);
        }
        Ok(())
    }
}

/// Lifted 0x23ef00 reads one global into EAX, with cdecl cleanup and no other register effects.
pub fn current_sequence<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    _: &mut C,
    regs: &mut ExtRegisters,
) {
    let mut memory = Memory(m);
    regs.eax = CutsceneState {
        memory: &mut memory,
    }
    .current_sequence()
    .expect("capture memory bridge cannot return a fault");
}
