// SPDX-License-Identifier: GPL-3.0-or-later
//! Float helpers that `core` lacks (T1976): the render modules are also compiled into the `no_std` lane build
//! (crates/tsfp-ports), where `f32::sqrt` and `f64::trunc` do not exist, and the workspace forbids `unsafe` (so no SSE
//! intrinsics). Both are exact IEEE operations, implemented here with integer arithmetic and tested bit for bit against
//! `std` (`sqrtss`/`sqrtsd` on x86_64).

/// Integer square root of `n` (floor) and whether the remainder is non zero.
fn isqrt(n: u128) -> (u128, bool) {
    let (mut rest, mut root, mut bit) = (n, 0u128, 1u128 << 126);
    while bit > rest && bit != 0 {
        bit >>= 2;
    }
    while bit != 0 {
        if rest >= root + bit {
            rest -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    (root, rest != 0)
}

/// Correctly rounded (nearest even) IEEE square root on the raw bits of a binary float with `mantissa` fraction bits and
/// `exponent` exponent bits. NaN inputs give a quiet NaN of the same payload, negative inputs the default NaN, `-0` stays `-0`.
fn sqrt_bits(bits: u64, mantissa: u32, exponent: u32) -> u64 {
    let sign = bits >> (mantissa + exponent);
    let max_exponent = (1u64 << exponent) - 1;
    let biased = (bits >> mantissa) & max_exponent;
    let fraction = bits & ((1u64 << mantissa) - 1);
    let quiet = 1u64 << (mantissa - 1);
    let default_nan = (1u64 << (mantissa + exponent)) | (max_exponent << mantissa) | quiet;
    if biased == max_exponent {
        return if fraction != 0 {
            bits | quiet
        } else if sign != 0 {
            default_nan
        } else {
            bits
        };
    }
    if biased == 0 && fraction == 0 {
        return bits;
    }
    if sign != 0 {
        return default_nan;
    }
    let bias = (max_exponent >> 1) as i32;
    let (mut significand, mut unbiased) = if biased == 0 {
        let shift = mantissa - (63 - fraction.leading_zeros());
        (fraction << shift, 1 - bias - shift as i32)
    } else {
        (fraction | (1u64 << mantissa), biased as i32 - bias)
    };
    if unbiased & 1 != 0 {
        significand <<= 1;
        unbiased -= 1;
    }
    let (mut root, sticky) = isqrt((significand as u128) << (mantissa + 2));
    let guard = root & 1 != 0;
    root >>= 1;
    if guard && (sticky || root & 1 != 0) {
        root += 1;
    }
    let mut result_exponent = unbiased / 2 + bias;
    if root >> (mantissa + 1) != 0 {
        root >>= 1;
        result_exponent += 1;
    }
    ((result_exponent as u64) << mantissa) | (root as u64 & ((1u64 << mantissa) - 1))
}

/// Square root, `sqrtss`.
pub fn sqrt32(value: f32) -> f32 {
    f32::from_bits(sqrt_bits(u64::from(value.to_bits()), 23, 8) as u32)
}

/// Square root, `sqrtsd`.
pub fn sqrt64(value: f64) -> f64 {
    f64::from_bits(sqrt_bits(value.to_bits(), 52, 11))
}

/// Round toward zero, keeping the sign of zero. NaN, infinity and values at or above 2^52 are already integral.
pub fn trunc64(value: f64) -> f64 {
    if !(value.abs() < 4_503_599_627_370_496.0) {
        return value;
    }
    let whole = (value as i64) as f64;
    f64::from_bits(whole.to_bits() | (value.to_bits() & (1 << 63)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    #[test]
    fn sqrt_matches_std_bit_for_bit() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let specials = [0u32, 0x8000_0000, 1, 0x007F_FFFF, 0x0080_0000, 0x7F7F_FFFF, 0x7F80_0000, 0xFF80_0000, 0x7FC0_0000, 0x7F80_0001, 0xBF80_0000];
        for bits in specials.into_iter().chain((0..2_000_000).map(|_| next(&mut state) as u32)) {
            let value = f32::from_bits(bits);
            assert_eq!(sqrt32(value).to_bits(), value.sqrt().to_bits(), "f32 {bits:#x}");
        }
        for bits in [0u64, 1 << 63, 1, 0x000F_FFFF_FFFF_FFFF, 0x0010_0000_0000_0000, 0x7FEF_FFFF_FFFF_FFFF, 0x7FF0_0000_0000_0000, 0x7FF8_0000_0000_0000]
            .into_iter()
            .chain((0..2_000_000).map(|_| next(&mut state)))
        {
            let value = f64::from_bits(bits);
            assert_eq!(sqrt64(value).to_bits(), value.sqrt().to_bits(), "f64 {bits:#x}");
        }
    }

    #[test]
    fn trunc_matches_std() {
        let mut state = 0x1234_5678_9ABC_DEF1u64;
        for bits in [0u64, 1 << 63, 0x3FE0_0000_0000_0000, 0xBFE0_0000_0000_0000, 0x4330_0000_0000_0000, 0x7FF0_0000_0000_0000, 0x7FF8_0000_0000_0000]
            .into_iter()
            .chain((0..1_000_000).map(|_| next(&mut state)))
        {
            let value = f64::from_bits(bits);
            let (a, b) = (trunc64(value), value.trunc());
            assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "{bits:#x}");
        }
    }
}
