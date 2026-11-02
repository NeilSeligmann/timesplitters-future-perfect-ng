// SPDX-License-Identifier: GPL-3.0-or-later
//! Hashable raster state used to cache wgpu render pipelines.

use crate::nv2a::{CullFace, Equation, Factor, PipelineState};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineKey {
    pub(super) blend_enable: bool,
    pub(super) source: Factor,
    pub(super) destination: Factor,
    pub(super) equation: Equation,
    pub(super) depth_test: bool,
    pub(super) depth_func: crate::nv2a::Compare,
    pub(super) depth_write: bool,
    pub(super) cull: Option<CullFace>,
    pub(super) front_clockwise: bool,
    /// Colour channel write enables (bits: red 1, green 2, blue 4, alpha 8).
    pub(super) write_mask: u8,
    /// Line list topology (NV2A primitives 2 to 4).
    pub(super) lines: bool,
    pub(super) depth_bias_constant: i32,
    pub(super) depth_bias_slope_bits: u32,
}

/// `SET_COLOR_MASK` (0x358): a non-zero byte enables a channel, bit 16 red, 8 green, 0 blue, 24 alpha.
fn write_mask_bits(mask: u32) -> u8 {
    u8::from(mask & 0x0001_0000 != 0)
        | u8::from(mask & 0x0000_0100 != 0) << 1
        | u8::from(mask & 0x0000_0001 != 0) << 2
        | u8::from(mask & 0x0100_0000 != 0) << 3
}

impl PipelineKey {
    pub(super) fn of(
        state: &PipelineState,
        honour_mask: bool,
        lines: bool,
        polygon_offset_opt_in: bool,
    ) -> Self {
        let bias = f32::from_bits(state.polygon_offset_bias_bits);
        let scale = f32::from_bits(state.polygon_offset_scale_bits);
        let bias_fits_wgpu = bias.is_finite()
            && bias.fract() == 0.0
            && bias >= i32::MIN as f32
            && bias < (i32::MAX as f32 + 1.0);
        let offset_enabled = polygon_offset_opt_in
            && state.depth_test
            && state.polygon_offset_fill
            && !lines
            && state.polygon_offset_scale_written
            && state.polygon_offset_bias_written
            && scale.is_finite()
            && bias_fits_wgpu
            && (scale != 0.0 || bias != 0.0);
        Self {
            lines,
            write_mask: if honour_mask {
                write_mask_bits(state.colour_mask)
            } else {
                0xF
            },
            blend_enable: state.blend_enable,
            source: state.source,
            destination: state.destination,
            equation: state.equation,
            depth_test: state.depth_test,
            depth_func: state.depth_func,
            depth_write: state.depth_write,
            cull: state.cull,
            front_clockwise: state.front_clockwise,
            depth_bias_constant: if offset_enabled { bias as i32 } else { 0 },
            depth_bias_slope_bits: if offset_enabled {
                scale.to_bits()
            } else {
                0.0f32.to_bits()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PipelineKey;
    use crate::nv2a::PipelineState;

    #[test]
    fn polygon_offset_requires_opt_in_depth_fill_and_complete_finite_state() {
        let mut state = PipelineState::default();
        state.depth_test = true;
        state.polygon_offset_fill = true;
        state.polygon_offset_scale_written = true;
        state.polygon_offset_bias_written = true;
        state.polygon_offset_scale_bits = 1.0f32.to_bits();
        state.polygon_offset_bias_bits = 2.0f32.to_bits();

        let enabled = PipelineKey::of(&state, true, false, true);
        assert_eq!(enabled.depth_bias_constant, 2);
        assert_eq!(enabled.depth_bias_slope_bits, 1.0f32.to_bits());

        assert_eq!(
            PipelineKey::of(&state, true, false, false).depth_bias_constant,
            0
        );
        state.depth_test = false;
        assert_eq!(
            PipelineKey::of(&state, true, false, true).depth_bias_constant,
            0
        );
        state.depth_test = true;
        assert_eq!(
            PipelineKey::of(&state, true, true, true).depth_bias_constant,
            0
        );
        state.polygon_offset_scale_written = false;
        assert_eq!(
            PipelineKey::of(&state, true, false, true).depth_bias_constant,
            0
        );
        state.polygon_offset_scale_written = true;

        state.polygon_offset_scale_bits = f32::INFINITY.to_bits();
        assert_eq!(
            PipelineKey::of(&state, true, false, true).depth_bias_constant,
            0
        );
        state.polygon_offset_scale_bits = 1.0f32.to_bits();

        state.polygon_offset_bias_bits = f32::NAN.to_bits();
        assert_eq!(
            PipelineKey::of(&state, true, false, true).depth_bias_constant,
            0
        );
        state.polygon_offset_bias_bits = 2.5f32.to_bits();
        assert_eq!(
            PipelineKey::of(&state, true, false, true).depth_bias_constant,
            0
        );
        state.polygon_offset_bias_bits = (i32::MAX as f32).to_bits();
        assert_eq!(
            PipelineKey::of(&state, true, false, true).depth_bias_constant,
            0
        );
    }
}
