// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw post-selection scalar update observed in native XBE function 0x18AF40.
//!
//! Inputs retain guest offsets and global-derived values because the original
//! row, actor field, and coordinate/angle meanings are not yet established.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectedRowAdjustment {
    /// Selected-row floats at +0xB4/+0xB8/+0xBC.
    pub row_b4_b8_bc: [f32; 3],
    /// Local floats at stack offsets +0x14/+0x18/+0x1C.
    pub local_14_18_1c: [f32; 3],
    /// Local target scalar at stack offset +0x24.
    pub local_24: f32,
    /// Initial actor field at +0x94.
    pub actor_94: f32,
    /// Runtime value read from global +0x4765F4.
    pub squared_length_threshold: f32,
    /// Integer-to-float value read from global +0x4E8CEC.
    pub divisor: f32,
    /// Integer-to-float value read from global +0x4E7944.
    pub signed_scale: f32,
    /// Scalar read from global +0x475CE4.
    pub step_scale: f32,
    /// Bounds read from globals +0x47DE9C/+0x47DEA0.
    pub clamp_bounds: [f32; 2],
}

/// Apply the measured raw update and clamp from 0x18AF40.
///
/// The arithmetic order follows the scalar SSE instructions. Rust's reciprocal
/// square root is an ordinary `1/sqrt` approximation of the XBE `rsqrtss`; this
/// is Tier 0 code and does not claim instruction-level equivalence.
pub fn update_actor_94(input: SelectedRowAdjustment) -> f32 {
    let dx = input.row_b4_b8_bc[0] - input.local_14_18_1c[0];
    let mut dy = input.row_b4_b8_bc[1] - input.local_14_18_1c[1];
    let dz = input.row_b4_b8_bc[2] - input.local_14_18_1c[2];

    let mut squared_length = dz * dz;
    squared_length += dy * dy;
    squared_length += dx * dx;

    if squared_length > input.squared_length_threshold {
        dy *= 1.0 / squared_length.sqrt();
    }

    let residual = dy - input.local_24;
    let step = (1.0 / input.divisor) * input.signed_scale * input.step_scale;
    let mut actor_94 = input.actor_94;
    if residual.abs() > step {
        if input.local_24 > dy {
            actor_94 += step;
        } else if dy > input.local_24 {
            actor_94 -= step;
        }
        if actor_94 > input.clamp_bounds[1] {
            actor_94 = input.clamp_bounds[1];
        } else if input.clamp_bounds[0] > actor_94 {
            actor_94 = input.clamp_bounds[0];
        }
    }
    actor_94
}

#[cfg(test)]
mod tests {
    use super::{SelectedRowAdjustment, update_actor_94};

    fn sample() -> SelectedRowAdjustment {
        SelectedRowAdjustment {
            row_b4_b8_bc: [0.0, 2.0, 0.0],
            local_14_18_1c: [0.0, 0.0, 0.0],
            local_24: 0.0,
            actor_94: 0.0,
            squared_length_threshold: 0.000_000_1,
            divisor: 60.0,
            signed_scale: 1.0,
            step_scale: 1.0,
            clamp_bounds: [-0.5, 0.5],
        }
    }

    #[test]
    fn ordered_comparisons_choose_add_or_subtract_direction() {
        let mut input = sample();
        assert!((update_actor_94(input) + 1.0 / 60.0).abs() < 0.000_001);

        input.local_24 = 3.0;
        assert!((update_actor_94(input) - 1.0 / 60.0).abs() < 0.000_001);
    }

    #[test]
    fn skips_update_inside_step_and_clamps_after_update() {
        let mut input = sample();
        input.row_b4_b8_bc[1] = 0.0;
        input.local_24 = 0.0;
        assert_eq!(update_actor_94(input), 0.0);

        input = sample();
        input.actor_94 = 0.49;
        input.signed_scale = -10.0;
        assert_eq!(update_actor_94(input), 0.5);
    }
}
