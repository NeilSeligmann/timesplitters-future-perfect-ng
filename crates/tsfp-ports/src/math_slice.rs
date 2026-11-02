// SPDX-License-Identifier: GPL-3.0-or-later
//! T1937 opt-in math attachment boundary. Guest records retain their original aliasing.
use crate::abi::{ExtRegisters, GuestCall};
use crate::memory::GuestMemory;

/// Address of three adjacent little-endian f32 fields (12 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vec3Address(pub u32);
/// Address of four adjacent little-endian f32 fields (16 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuaternionAddress(pub u32);
/// Address of sixteen little-endian f32 fields (64 bytes); translation at +48.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatrixAddress(pub u32);

/// Per-address A/B boundary. `enabled` is the caller's decision for this address.
/// False leaves the registers and memory untouched so the original can run.
/// Calls crossing the boundary remain explicit through `GuestCall`.
pub fn dispatch<M: GuestMemory, C: GuestCall>(
    enabled: bool,
    address: u32,
    memory: &mut M,
    calls: &mut C,
    registers: &mut ExtRegisters,
) -> bool {
    enabled && crate::c2r::t1937_math::run(address, memory, calls, registers)
}
