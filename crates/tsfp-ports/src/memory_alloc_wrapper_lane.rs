// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(feature = "std")]
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
};
#[cfg(feature = "std")]
use core::marker::PhantomData;
#[cfg(feature = "std")]
use tsfp_engine_memory::{
    GuestMemory as EngineGuestMemory, w4m_arena::ArenaHooks, wrappers::AllocWrappers,
};

#[cfg(feature = "std")]
struct EngineMemory<'a, M: GuestMemory> {
    runtime: *mut M,
    _borrow: PhantomData<&'a mut M>,
}

#[cfg(feature = "std")]
impl<M: GuestMemory> EngineGuestMemory for EngineMemory<'_, M> {
    fn read(
        &mut self,
        address: tsfp_engine_memory::GuestAddr,
        out: &mut [u8],
    ) -> Result<(), tsfp_engine_memory::MemoryFault> {
        // SAFETY: the exclusive runtime borrow is held until this synchronous adapter is dropped.
        unsafe { (&*self.runtime).read_bytes(address.0, out) };
        Ok(())
    }
    fn write(
        &mut self,
        address: tsfp_engine_memory::GuestAddr,
        bytes: &[u8],
    ) -> Result<(), tsfp_engine_memory::MemoryFault> {
        // SAFETY: the exclusive runtime borrow prevents overlapping runtime access during writes.
        let runtime = unsafe { &mut *self.runtime };
        for (offset, value) in bytes.iter().copied().enumerate() {
            runtime.write8(address.0.wrapping_add(offset as u32), value);
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
struct GuestHooks<'a, M: GuestMemory, C: GuestCall> {
    guest: &'a mut C,
    registers: &'a mut ExtRegisters,
    runtime: *mut M,
}

#[cfg(feature = "std")]
impl<M: GuestMemory, C: GuestCall> ArenaHooks for GuestHooks<'_, M, C> {
    fn out_of_memory(&mut self, _argument: u32) {}
    fn out_of_memory_with_memory<T: EngineGuestMemory>(&mut self, _memory: &mut T, argument: u32) {
        // SAFETY: this synchronous callback runs while EngineMemory holds the exclusive runtime borrow.
        let runtime = unsafe { &mut *self.runtime };
        call_cdecl(self.guest, runtime, self.registers, 0x389B0, &[argument]);
    }
}

#[cfg(feature = "std")]
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    registers: &mut ExtRegisters,
) -> bool {
    let (audio, flag) = match va {
        0x298B0 => (true, "TSFP_RUST_GAME_AUDIO_WORKSPACE"),
        0x220E40 => (false, "TSFP_RUST_GAME_STATE_220E40"),
        _ => return false,
    };
    if !std::env::var(flag).is_ok_and(|value| value == "1") {
        return false;
    }
    let owner = registers.ecx;
    let runtime = memory as *mut M;
    let mut engine_memory = EngineMemory {
        runtime,
        _borrow: PhantomData,
    };
    let mut hooks = GuestHooks {
        guest,
        registers,
        runtime,
    };
    let mut wrappers = AllocWrappers {
        memory: &mut engine_memory,
        hooks: &mut hooks,
    };
    hooks.registers.eax = (if audio {
        wrappers.alloc_audio_workspace_298b0(owner)
    } else {
        wrappers.initialize_state_block_75c108()
    })
    .expect("guest memory is mapped");
    true
}

#[cfg(not(feature = "std"))]
pub fn run<M, C>(
    _va: u32,
    _memory: &mut M,
    _guest: &mut C,
    _registers: &mut crate::abi::ExtRegisters,
) -> bool {
    false
}
