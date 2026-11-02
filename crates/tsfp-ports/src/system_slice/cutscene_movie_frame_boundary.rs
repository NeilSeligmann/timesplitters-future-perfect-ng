// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoded-surface presentation boundary: values in, owned Rust rectangles out.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../../tsfp-gameplay/src/cutscene_original_core.rs"]
mod model;
use model::{Extent, MoviePlacement, MovieState};

/// Snapshot words: crop x/y, mode, decoded w/h, display w/h, interval w/h, aspect bits.
/// No decoder lifecycle, guest writes, clocks or input synthesis occur here.
pub fn rectangles(words: [u32; 10]) -> Option<[i32; 8]> {
    let aspect = f32::from_bits(words[9]);
    if words[3..9].iter().any(|&v| v == 0 || v > i32::MAX as u32)
        || !aspect.is_finite()
        || aspect <= 0.0
    {
        return None;
    }
    let state = MovieState {
        crop_inset: Extent {
            width: words[0] as i32,
            height: words[1] as i32,
        },
        placement: if words[2] == 0 {
            MoviePlacement::Stretch
        } else {
            MoviePlacement::OriginalAspect
        },
    };
    let frame = state.rectangles(
        Extent {
            width: words[3] as i32,
            height: words[4] as i32,
        },
        Extent {
            width: words[5] as i32,
            height: words[6] as i32,
        },
        Extent {
            width: words[7] as i32,
            height: words[8] as i32,
        },
        aspect as f64,
    );
    Some([
        frame.source.left,
        frame.source.top,
        frame.source.right,
        frame.source.bottom,
        frame.destination.left,
        frame.destination.top,
        frame.destination.right,
        frame.destination.bottom,
    ])
}

/// Hybrid host contract: caller owns readable 10-word snapshot and writable 8-word output.
/// Returns zero without output writes when the diagnostic original preset is disabled.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_cutscene_movie_rectangles(
    input: *const u32,
    output: *mut i32,
) -> i32 {
    if !super::enabled()
        || !crate::runtime::root_selected(0x30210)
        || input.is_null()
        || output.is_null()
    {
        return 0;
    }
    let words = unsafe { core::ptr::read_unaligned(input.cast::<[u32; 10]>()) };
    let Some(result) = rectangles(words) else {
        return 0;
    };
    unsafe {
        core::ptr::copy_nonoverlapping(result.as_ptr(), output, 8);
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_geometry_and_unadmitted_ffi() {
        let input = [
            2,
            1,
            0,
            640,
            480,
            640,
            480,
            640,
            480,
            (4.0f32 / 3.0).to_bits(),
        ];
        assert_eq!(rectangles(input), Some([2, 1, 638, 479, 0, 0, 640, 480]));
        let mut invalid = input;
        invalid[5] = 0;
        assert_eq!(rectangles(invalid), None);
        assert!(!super::super::enabled());
        let mut output = [77; 8];
        assert_eq!(
            unsafe { tsfp_cutscene_movie_rectangles(input.as_ptr(), output.as_mut_ptr()) },
            0
        );
        assert_eq!(output, [77; 8]);
    }
}
