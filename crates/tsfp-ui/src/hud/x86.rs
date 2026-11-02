// SPDX-License-Identifier: GPL-3.0-or-later
//! Small x86 behaviours the HUD bodies depend on, named so the ports read as plain Rust.

use crate::memory::GuestMemory;

/// Read a little endian `f32` from guest memory.
pub fn read_f32<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// Write a little endian `f32` to guest memory.
pub fn write_f32<M: GuestMemory>(m: &mut M, address: u32, value: f32) {
    m.write32(address, value.to_bits());
}

/// Read a signed 32-bit integer.
pub fn read_i32<M: GuestMemory>(m: &M, address: u32) -> i32 {
    m.read32(address) as i32
}

/// `cvtsi2ss`: signed integer to single, round to nearest even.
pub fn int_to_f32(value: i32) -> f32 {
    value as f32
}

/// `cvttss2si`: truncate toward zero. NaN and out of range give the "integer indefinite" value.
pub fn truncate_to_i32(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// After `comiss a, b`: `jb` is taken (CF=1) when `a < b` or the operands are unordered.
pub fn below(a: f32, b: f32) -> bool {
    !(a >= b)
}

/// After `comiss a, b`: `jbe` is taken (CF=1 or ZF=1) when `a <= b` or the operands are unordered.
pub fn below_or_equal(a: f32, b: f32) -> bool {
    !(a > b)
}

/// `strncpy` as the game's bounded copy (0x3C8960): copy up to `count` bytes, stop copying at the
/// terminator and zero fill the rest. Returns nothing (the original returns the destination).
pub fn bounded_copy<M: GuestMemory>(m: &mut M, destination: u32, source: u32, count: u32) {
    let mut terminated = false;
    for i in 0..count {
        let byte = if terminated {
            0
        } else {
            m.read8(source.wrapping_add(i))
        };
        if byte == 0 {
            terminated = true;
        }
        m.write8(destination.wrapping_add(i), byte);
    }
}

/// The CRT `_ftol` the game links (0x3C848C): truncate an x87 value toward zero through a 64 bit
/// `fistp` and return the low dword. NaN and magnitudes of 2^63 or more give 0 (the 64 bit
/// indefinite value has a zero low dword), values beyond 32 bits wrap.
pub fn ftol_low32(value: f64) -> u32 {
    if value.is_nan() || value.abs() >= 9.223_372_036_854_775_8e18 {
        0
    } else {
        (value.trunc() as i64) as u32
    }
}

/// `mulss`/`addss`/`subss`/`divss` NaN rule: a NaN first operand wins, else a NaN second operand
/// (quieted). Rust may commute `a * b`, which would change which NaN payload survives.
fn nan_rule(a: f32, b: f32) -> Option<f32> {
    if a.is_nan() {
        Some(f32::from_bits(a.to_bits() | 0x0040_0000))
    } else if b.is_nan() {
        Some(f32::from_bits(b.to_bits() | 0x0040_0000))
    } else {
        None
    }
}

pub fn mul_ss(a: f32, b: f32) -> f32 {
    nan_rule(a, b).unwrap_or(a * b)
}

pub fn div_ss(a: f32, b: f32) -> f32 {
    nan_rule(a, b).unwrap_or(a / b)
}
