// SPDX-License-Identifier: GPL-3.0-or-later
//! T1902 shared arithmetic matching the retained lifted-C model, not hardware evidence.
use crate::memory::GuestMemory;

unsafe extern "C" {
    #[link_name = "sqrt"] fn c_sqrt(value: f64) -> f64;
    #[link_name = "sqrtf"] fn c_sqrtf(value: f32) -> f32;
    #[link_name = "frexp"] fn c_frexp(value: f64, exponent: *mut i32) -> f64;
    #[link_name = "ldexp"] fn c_ldexp(value: f64, exponent: i32) -> f64;
}
pub(crate) fn sqrt_double(value: f64) -> f64 { unsafe { c_sqrt(value) } }
/// The lifted model uses reciprocal sqrtf; the original RSQRTSS approximation is verification debt.
pub(crate) fn reciprocal_sqrt(value: f32) -> f32 { 1.0 / unsafe { c_sqrtf(value) } }
pub(crate) fn read_double<M: GuestMemory>(m: &M, address: u32) -> f64 {
    f64::from_bits(m.read32(address) as u64 | ((m.read32(address.wrapping_add(4)) as u64) << 32))
}
/// PC=24 rounds the significand while preserving the double exponent range.
/// Mirrors recomp_fp_round24, including its normal-f32 fast path and frexp slow path.
pub(crate) fn round_pc(value: f64, control: u16) -> f64 {
    if control & 0x300 != 0 { return value; }
    let magnitude = value.abs();
    if (magnitude <= f32::MAX as f64 && magnitude >= f32::MIN_POSITIVE as f64)
        || magnitude == 0.0 || !value.is_finite() {
        return value as f32 as f64;
    }
    let mut exponent = 0;
    let fraction = unsafe { c_frexp(value, &mut exponent) };
    unsafe { c_ldexp(fraction as f32 as f64, exponent) }
}
pub(crate) fn compare_fp(a: f64, b: f64) -> i32 {
    if a.is_nan() || b.is_nan() { 2 } else if a < b { -1 } else if a > b { 1 } else { 0 }
}
