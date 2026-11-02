//! Rust-owned Map Maker overlay presentation state (T2408, based on T2418).
//!
//! Callers supply frame values and pointer interpolation from their runtime
//! services. This module stores no guest addresses and performs no rendering.

/// Per-frame alpha ramp for the editor overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapMakerOverlayRamp {
    alpha: f32,
    frame_rate: f32,
    one: f32,
    scale: f32,
}

/// Typed per-frame facts that decide whether the editor overlay is visible.
///
/// The fields correspond to the measured `0x2FB4C0` widget-tree predicates in
/// T2418. Runtime adapters provide booleans at the Rust seam; guest addresses
/// and widget pointers never enter the owned UI state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapMakerOverlayVisibility {
    pub first_leaf_present: bool,
    pub second_leaf_flag_218_bit_0: bool,
    pub third_leaf_is_page: bool,
    pub hud_bit_15: bool,
}

impl MapMakerOverlayVisibility {
    /// T2418 shows the overlay only when all four measured conditions allow it.
    pub const fn shown(self) -> bool {
        self.first_leaf_present
            && !self.second_leaf_flag_218_bit_0
            && !self.third_leaf_is_page
            && !self.hud_bit_15
    }
}

impl MapMakerOverlayRamp {
    pub const fn new(alpha: f32, frame_rate: f32, one: f32, scale: f32) -> Self {
        Self {
            alpha,
            frame_rate,
            one,
            scale,
        }
    }

    pub const fn alpha(self) -> f32 {
        self.alpha
    }

    /// Advance the overlay alpha by `(one / frame_rate) * scale`.
    ///
    /// The explicit unordered cases retain NaN alpha as the original SSE
    /// branch does; generic `min`/`max` helpers have different NaN behavior.
    pub fn step(&mut self, shown: bool) {
        let delta = (self.one / self.frame_rate) * self.scale;
        if shown {
            let next = self.alpha + delta;
            self.alpha = if next <= self.one || next.is_nan() {
                next
            } else {
                self.one
            };
        } else {
            let next = self.alpha - delta;
            self.alpha = if 0.0 <= next || next.is_nan() {
                next
            } else {
                0.0
            };
        }
    }
}

/// Runtime operands for the pointer-relative hint anchor transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HintAnchorTransform {
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub horizontal_scale: f32,
    pub horizontal_bias: f32,
    pub vertical_pointer_scale: f32,
    pub vertical_anchor_scale: f32,
}

/// Apply T2418's x87 arithmetic order and `CVTTSS2SI` conversion contract.
pub fn shift_hint_anchor(transform: HintAnchorTransform, x: i32, y_offset: i32) -> (i32, i32) {
    let shifted_x =
        ((transform.pointer_x * transform.horizontal_scale) + x as f32) + transform.horizontal_bias;
    let shifted_y = ((transform.pointer_y * transform.vertical_pointer_scale)
        * transform.vertical_anchor_scale)
        + y_offset as f32;
    (cvttss2si(shifted_x), cvttss2si(shifted_y))
}

/// x86 `CVTTSS2SI`: truncate toward zero and return INT_MIN for NaN or overflow.
fn cvttss2si(value: f32) -> i32 {
    if !value.is_finite() || value < i32::MIN as f32 || value >= 2_147_483_648.0 {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HintAnchorTransform, MapMakerOverlayRamp, MapMakerOverlayVisibility, shift_hint_anchor,
    };

    #[test]
    fn overlay_visibility_requires_all_measured_widget_conditions() {
        let visible = MapMakerOverlayVisibility {
            first_leaf_present: true,
            second_leaf_flag_218_bit_0: false,
            third_leaf_is_page: false,
            hud_bit_15: false,
        };
        assert!(visible.shown());

        for hidden in [
            MapMakerOverlayVisibility {
                first_leaf_present: false,
                ..visible
            },
            MapMakerOverlayVisibility {
                second_leaf_flag_218_bit_0: true,
                ..visible
            },
            MapMakerOverlayVisibility {
                third_leaf_is_page: true,
                ..visible
            },
            MapMakerOverlayVisibility {
                hud_bit_15: true,
                ..visible
            },
        ] {
            assert!(!hidden.shown());
        }
    }

    #[test]
    fn overlay_ramp_moves_by_runtime_delta_and_clamps_both_ends() {
        let mut ramp = MapMakerOverlayRamp::new(0.95, 10.0, 1.0, 1.0);
        ramp.step(true);
        assert_eq!(ramp.alpha(), 1.0);
        ramp = MapMakerOverlayRamp::new(0.05, 10.0, 1.0, 1.0);
        ramp.step(false);
        assert_eq!(ramp.alpha(), 0.0);
    }

    #[test]
    fn overlay_ramp_preserves_unordered_alpha() {
        let mut ramp = MapMakerOverlayRamp::new(f32::NAN, 60.0, 1.0, 1.0);
        ramp.step(true);
        assert!(ramp.alpha().is_nan());
        ramp.step(false);
        assert!(ramp.alpha().is_nan());
    }

    #[test]
    fn hint_anchor_uses_ordered_transform_and_truncation() {
        let transform = HintAnchorTransform {
            pointer_x: 0.5,
            pointer_y: 2.0,
            horizontal_scale: 2.0,
            horizontal_bias: 3.0,
            vertical_pointer_scale: 0.5,
            vertical_anchor_scale: 3.0,
        };
        assert_eq!(shift_hint_anchor(transform, 10, 7), (14, 10));
    }

    #[test]
    fn hint_anchor_returns_int_min_for_cvtt_overflow_or_nan() {
        let transform = HintAnchorTransform {
            pointer_x: f32::INFINITY,
            pointer_y: f32::NAN,
            horizontal_scale: 1.0,
            horizontal_bias: 0.0,
            vertical_pointer_scale: 1.0,
            vertical_anchor_scale: 1.0,
        };
        assert_eq!(shift_hint_anchor(transform, 0, 0), (i32::MIN, i32::MIN));
    }
}
