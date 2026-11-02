// SPDX-License-Identifier: GPL-3.0-or-later
//! Library methods consumed by the live replayer, excluding diagnostic-only registers.
//! Keep this list in step with `Replayer::step`, `effective_pixel`, `fog_state`,
//! `fog_colour`, texture addressing and `PipelineState::apply` in tsfp-wgpu.
//! Clears are commands and must remain ordered even when their value repeats.
pub const fn live_library_method(method: u32) -> bool {
    matches!(
        method,
        0x0300
            | 0x0304
            | 0x0308
            | 0x030C
            | 0x033C
            | 0x0340
            | 0x0344
            | 0x0348
            | 0x034C
            | 0x0350
            | 0x0354
            | 0x0358
            | 0x035C
            | 0x039C
            | 0x03A0
            | 0x1D90
            | 0x1D94
            | 0x029C
            | 0x02A4
            | 0x02A8
            | 0x09C0
            | 0x09C4
            | 0x1EA4 | 0x0B80..=0x0BFC
            | 0x0A60..=0x0A9C | 0x1E20 | 0x1E24 | 0x1B08 | 0x1B48 | 0x1B88 | 0x1BC8
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_effect_state_and_vertex_constants_stay() {
        for method in [
            0x1D94, 0x358, 0xA60, 0xA9C, 0x1E24, 0x2A4, 0x9C4, 0x1BC8, 0x1EA4, 0xB80, 0xBFC,
        ] {
            assert!(live_library_method(method));
        }
        for method in [0xB00, 0x1720, 0x1760, 0x1800, 0x1818] {
            assert!(!live_library_method(method));
        }
    }
}
