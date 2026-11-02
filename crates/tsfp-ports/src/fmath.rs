// SPDX-License-Identifier: GPL-3.0-or-later
//! libm double math through `extern "C"` for the `core` only pose sources shared with `tsfp-engine-anim` (T1918). The host links libm
//! (`TSFP_LIBM`); same symbols as the `std` methods used by the engine-anim crate, so results are identical.
unsafe extern "C" {
    fn sin(x: f64) -> f64;
    fn cos(x: f64) -> f64;
    fn sqrt(x: f64) -> f64;
    fn atan2(y: f64, x: f64) -> f64;
    fn floor(x: f64) -> f64;
}

pub fn sin_f64(x: f64) -> f64 {
    unsafe { sin(x) }
}
pub fn cos_f64(x: f64) -> f64 {
    unsafe { cos(x) }
}
pub fn sqrt_f64(x: f64) -> f64 {
    unsafe { sqrt(x) }
}
pub fn atan2_f64(y: f64, x: f64) -> f64 {
    unsafe { atan2(y, x) }
}
pub fn floor_f64(x: f64) -> f64 {
    unsafe { floor(x) }
}
