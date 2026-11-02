// SPDX-License-Identifier: GPL-3.0-or-later
//! Safe arithmetic boundary of the lifted f64 model, not x87 hardware precision.
use crate::memory::GuestMemory;
pub(crate) fn sqrt_double(v: f64) -> f64 {
    v.sqrt()
}
pub(crate) fn reciprocal_sqrt(v: f32) -> f32 {
    1.0 / v.sqrt()
}
pub(crate) fn read_double<M: GuestMemory>(m: &M, a: u32) -> f64 {
    f64::from_bits(m.read32(a) as u64 | ((m.read32(a.wrapping_add(4)) as u64) << 32))
}
pub(crate) fn compare_fp(a: f64, b: f64) -> i32 {
    if a.is_nan() || b.is_nan() {
        2
    } else if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}
/// PC24 retains the double exponent and rounds its significand to 24 bits.
pub(crate) fn round_pc(v: f64, control: u16) -> f64 {
    if control & 0x300 != 0 || !v.is_finite() || v == 0.0 {
        return v;
    }
    let a = v.abs();
    if a <= f32::MAX as f64 && a >= f32::MIN_POSITIVE as f64 {
        return v as f32 as f64;
    }
    let bits = v.to_bits();
    let fraction = bits & ((1u64 << 52) - 1);
    let shift = if bits & (0x7ffu64 << 52) == 0 {
        (64 - fraction.leading_zeros()).saturating_sub(24)
    } else {
        29
    };
    if shift == 0 {
        return v;
    }
    let mask = (1u64 << shift) - 1;
    let half = 1u64 << (shift - 1);
    let remainder = bits & mask;
    let mut rounded = bits & !mask;
    if remainder > half || (remainder == half && (rounded >> shift) & 1 != 0) {
        rounded = rounded.wrapping_add(1u64 << shift);
    }
    f64::from_bits(rounded)
}

#[cfg(test)]
mod tests {
    use super::round_pc;
    #[test]
    fn pc24_rounds_ties_even_with_double_exponents() {
        let base = (1500u64 << 52) | (1u64 << 29);
        assert_eq!(
            round_pc(f64::from_bits(base + (1 << 28)), 0).to_bits(),
            base + (1 << 29)
        );
        let base = 1500u64 << 52;
        assert_eq!(
            round_pc(f64::from_bits(base + (1 << 28)), 0).to_bits(),
            base
        );
        assert_eq!(round_pc(f64::from_bits(1), 0).to_bits(), 1);
        assert_eq!(round_pc(-0.0, 0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(
            round_pc(f64::from_bits(base + 1), 0x300).to_bits(),
            base + 1
        );
        assert_eq!(round_pc(1.234567891, 0), 1.234567891f64 as f32 as f64);
    }
}
