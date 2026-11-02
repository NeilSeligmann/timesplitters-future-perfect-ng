// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/audio
// portreg: wraps=tsfp-ports/audio_remainder
//! T1990: lane adapters (Tier 2 through `tools.rust_trace`) for the heaviest `engine/audio` functions that already have a
//! mechanical machine translation in `audio.rs` (T1913) and `audio_remainder.rs` (T1924). Those translations use the
//! `ExtRegisters` plus `GuestCall` contract of the lane, so this file only gates which of their roots the host may select
//! (they are otherwise opt-in: `run(.., enabled, ..)`). Registered through `lane_inv.rs`. Roots are the 31 translated audio
//! roots with at least 1,900 recorded calls in `docs/data/t1884-exec-weights.csv`, ordered by weight.
//! The underlying translations are Tier 0 (T1913/T1924); this lane is what lets the Story route check them.
use crate::{
    abi::{ExtRegisters, GuestCall},
    audio, audio_remainder,
    memory::{GuestMemory, MappedGuest},
};
pub use tsfp_port_audio_dispatch::ROOTS;

fn is_remainder(va: u32) -> bool {
    matches!(
        va,
        0x28180
            | 0x29e40
            | 0x29f30
            | 0x29bb0
            | 0x26f00
            | 0x462f0
            | 0x28760
            | 0x29af0
            | 0x44ac0
            | 0x164e00
            | 0x45540
            | 0x29dd0
            | 0x45610
    )
}

fn route_nested_audio<C: GuestCall>(va: u32, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    let mut memory = unsafe { MappedGuest::new(crate::raw_look::offset()) };
    run(va, &mut memory, guest, regs)
}

/// Dispatches a root; `false` when `va` is not one of the adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    let entry_esp = regs.esp;
    let mut nested = tsfp_port_audio_dispatch::GuestRouter::new(guest, route_nested_audio::<C>);
    let taken = if is_remainder(va) {
        audio_remainder::run(va, true, m, &mut nested, regs)
    } else {
        audio::run(va, true, m, &mut nested, regs)
    };
    // FINDING (T1990, MEASURED on the hybrid host): the lane contract is "ESP comes back as it entered", the generated
    // adapter (`tools/play/rust_adapters.py`) pops the return address and the `ret N` arguments afterwards. The T1913/T1924
    // translations follow the original instruction stream instead: they pop the return address (`esp += 4`) and lower
    // `ret N` to that same `esp += 4`, never popping the N argument bytes. Taken as is, every call leaked the return slot
    // into the caller's frame (the host aborted or faulted a few samples in). The adapter restores the entry ESP.
    if taken && va != 0x29e40 {
        regs.esp = entry_esp;
    }
    taken
}

#[cfg(not(test))]
unsafe extern "C" {
    fn tsfp_lane_set_return(pc: u32);
}

/// Tells the host which return address the NEXT guest call of this thread must leave in its call frame. The mechanical
/// translations of `audio.rs` and `audio_remainder.rs` do not push one (the host used 0), but the DirectSound entry
/// points compare the caller of a buffer method against the measured call sites (`src/audio/dsound_buffer.c`
/// `frame_handler`), so a call from a translated root needs the address of the instruction after the original `call`.
/// The translations carry it as the next block label (`call!(X); pc = N; continue;`).
#[inline(always)]
pub fn set_return(pc: u32) {
    #[cfg(not(test))]
    unsafe {
        tsfp_lane_set_return(pc)
    };
    #[cfg(test)]
    let _ = pc;
}
