// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared lifted register/call contract; no production admission.
#[path = "../../tsfp-ports/src/abi.rs"]
mod registers;
use crate::memory::GuestMemory;
pub use registers::{ExtRegisters, Fpu, GuestCall, Xmm};
pub(crate) fn write16<M: GuestMemory>(m: &mut M, a: u32, v: u16) {
    m.write8(a, v as u8);
    m.write8(a.wrapping_add(1), (v >> 8) as u8);
}
pub(crate) mod sse {
    pub fn mulss(a: f32, b: f32) -> f32 {
        f32::from_bits(crate::sse::mulss(a.to_bits(), b.to_bits()))
    }
    pub fn addss(a: f32, b: f32) -> f32 {
        f32::from_bits(crate::sse::addss(a.to_bits(), b.to_bits()))
    }
    pub fn subss(a: f32, b: f32) -> f32 {
        f32::from_bits(crate::sse::subss(a.to_bits(), b.to_bits()))
    }
    pub fn divss(a: f32, b: f32) -> f32 {
        f32::from_bits(crate::sse::divss(a.to_bits(), b.to_bits()))
    }
    pub fn cvttss2si(a: f32) -> u32 {
        crate::sse::cvttss2si(a.to_bits())
    }
}
