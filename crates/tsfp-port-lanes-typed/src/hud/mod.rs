// SPDX-License-Identifier: GPL-3.0-or-later
//! tsfp-ui HUD sources (`crate::hud::*` paths inside them resolve here).
#[path = "../../../tsfp-ui/src/hud/glyph.rs"]
pub mod glyph;

/// The one `tsfp-ui/src/hud/x86.rs` helper the glyph port uses. That file also holds f64
/// helpers that need std (`f64::trunc`), so it is not compiled here.
pub mod x86 {
    /// `cvttss2si`: truncate toward zero. NaN and out of range give the "integer indefinite" value.
    pub fn truncate_to_i32(value: f32) -> i32 {
        if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
            i32::MIN
        } else {
            value as i32
        }
    }
}
