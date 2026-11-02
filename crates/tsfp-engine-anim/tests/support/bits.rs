// SPDX-License-Identifier: GPL-3.0-or-later
//! Hexadecimal golden-field decoding; retain each fixture bit pattern.

pub fn f32_bits(hex: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(hex, 16).unwrap())
}
