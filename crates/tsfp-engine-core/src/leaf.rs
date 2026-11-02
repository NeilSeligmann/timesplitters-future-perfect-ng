// SPDX-License-Identifier: GPL-3.0-or-later
//! Integer primitives recovered from admitted leaves, with explicit 32-bit wrapping.
pub fn popcount(bits: u32) -> u32 { bits.count_ones() }
pub fn decimal_step(value: u32, digit: u32) -> u32 {
    value.wrapping_mul(10).wrapping_add(digit)
}
pub fn align4(offset: u32) -> u32 { offset.wrapping_add(offset.wrapping_neg() & 3) }
