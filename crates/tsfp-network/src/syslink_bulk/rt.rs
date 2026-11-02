// SPDX-License-Identifier: GPL-3.0-or-later
//! T1917 local safe helpers for word stores, partial registers, scalar SSE reads,
//! stack/callee boundaries and REPE CMPSW. Derived from core_bulk runtime support.
#![allow(dead_code, clippy::all)]
pub use super::state::{Fpu, NetworkCall as GuestCall, NetworkRegisters as ExtRegisters, Xmm};
pub use crate::memory::GuestMemory;

// ---- guest memory -------------------------------------------------------------------------------

/// 16-bit store (the shared `GuestMemory` leaf trait has 8 and 32 bit stores).
pub fn write16<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    m.write8(address, value as u8);
    m.write8(address.wrapping_add(1), (value >> 8) as u8);
}

// ---- partial registers ----------------------------------------------------------------------------

/// `mov al, v`: replace the low byte.
pub fn set_lo8(reg: u32, value: u32) -> u32 {
    (reg & 0xffff_ff00) | (value & 0xff)
}
/// `mov ah, v`: replace bits 8..16.
pub fn set_hi8(reg: u32, value: u32) -> u32 {
    (reg & 0xffff_00ff) | ((value & 0xff) << 8)
}
/// `mov ax, v`: replace the low word.
pub fn set_lo16(reg: u32, value: u32) -> u32 {
    (reg & 0xffff_0000) | (value & 0xffff)
}

/// Local extension over the existing ABI. The production GuestCall trampoline owns
/// the nested return slot; translated code pushes only arguments. No ABI file edits.
pub trait NetworkStack {
    fn push<M: GuestMemory>(&mut self, m: &mut M, value: u32);
    fn call<C: GuestCall>(&mut self, guest: &mut C, target: u32);
    fn tail_call<C: GuestCall>(&mut self, guest: &mut C, target: u32);
}
impl NetworkStack for ExtRegisters {
    fn push<M: GuestMemory>(&mut self, m: &mut M, value: u32) {
        self.esp = self.esp.wrapping_sub(4);
        m.write32(self.esp, value);
    }
    fn call<C: GuestCall>(&mut self, guest: &mut C, target: u32) {
        guest.call(target, self);
    }
    fn tail_call<C: GuestCall>(&mut self, guest: &mut C, target: u32) {
        // Skip the root's live return slot: the trampoline supplies the callee's slot.
        self.esp = self.esp.wrapping_add(4);
        guest.call(target, self);
        self.esp = self.esp.wrapping_sub(4);
    }
}

impl Xmm {
    pub fn f(&self, lane: usize) -> f32 {
        f32::from_bits(self.lanes[lane])
    }
}

/// Lifted REPE CMPSW, retaining the last comparison when count is zero.
pub fn repe_cmpsw<M: GuestMemory>(
    m: &M,
    esi: &mut u32,
    edi: &mut u32,
    ecx: &mut u32,
    zflag: &mut u32,
    eflags: u32,
) {
    let step = if eflags & 0x400 != 0 {
        2u32.wrapping_neg()
    } else {
        2
    };
    while *ecx != 0 {
        *zflag = u32::from(m.read16(*esi) == m.read16(*edi));
        *esi = esi.wrapping_add(step);
        *edi = edi.wrapping_add(step);
        *ecx = ecx.wrapping_sub(1);
        if *zflag == 0 {
            break;
        }
    }
}
