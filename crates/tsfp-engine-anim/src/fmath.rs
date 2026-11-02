// SPDX-License-Identifier: GPL-3.0-or-later
//! Double math for the shared pose sources. Here the `std` methods (glibc through the LLVM intrinsics); the `no_std` `tsfp-ports` crate has its own
//! `fmath.rs` with the same function names that calls the libm symbols directly, so both builds evaluate the same functions (T1918).
pub fn sin_f64(x: f64) -> f64 {
    x.sin()
}
pub fn cos_f64(x: f64) -> f64 {
    x.cos()
}
pub fn sqrt_f64(x: f64) -> f64 {
    x.sqrt()
}
pub fn atan2_f64(y: f64, x: f64) -> f64 {
    y.atan2(x)
}
pub fn floor_f64(x: f64) -> f64 {
    x.floor()
}
