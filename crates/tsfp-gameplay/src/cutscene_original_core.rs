// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-free original presentation arithmetic, shared with the no_std hybrid lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoviePlacement {
    Stretch,
    OriginalAspect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovieState {
    pub crop_inset: Extent,
    pub placement: MoviePlacement,
}

#[path = "cutscene_movie_setup.rs"]
pub mod movie_setup;
use movie_setup::trunc_i32;
pub use movie_setup::{Extent, Rect};

impl MovieState {
    /// Setup's -1 means derive from the other dimension; other negative inputs preserve crop.
    /// The stored dimensions are crop insets, not the decoded movie size.
    // portreg: va=0x00030210 tier=1
    // portreg: replaces=tsfp-ports/video
    pub fn configure(&mut self, requested: Extent, mode: i32, aspect: f32, inverse_aspect: f32) {
        let result = movie_setup::configure(
            requested.width,
            requested.height,
            mode,
            aspect,
            inverse_aspect,
        );
        if let Some((width, height)) = result.crop {
            self.crop_inset = Extent { width, height };
        }
        match result.mode {
            Some(0) => self.placement = MoviePlacement::Stretch,
            Some(1) => self.placement = MoviePlacement::OriginalAspect,
            _ => {}
        }
    }

    /// Presentation slice of movie playback, not decoder/resource lifecycle replacement.
    /// Display dimensions must be nonzero; finite derived coordinates must fit i32.
    /// The host validates decoder/display geometry before calling this typed boundary.
    pub fn rectangles(
        &self,
        decoded: Extent,
        display: Extent,
        interval: Extent,
        aspect: f64,
    ) -> MovieFrame {
        let source = Rect {
            left: self.crop_inset.width,
            top: self.crop_inset.height,
            right: decoded.width.wrapping_sub(self.crop_inset.width),
            bottom: decoded.height.wrapping_sub(self.crop_inset.height),
        };
        let (left, right) = match self.placement {
            MoviePlacement::Stretch => (0, interval.width),
            MoviePlacement::OriginalAspect => {
                // x87 arithmetic followed by the CRT ftol adjustment (truncate toward zero).
                let half = (aspect / (display.width as u32 as f64 / display.height as u32 as f64)
                    * interval.width as f64
                    * 0.5) as i32;
                let center = interval.width >> 1;
                (center.wrapping_sub(half), center.wrapping_add(half))
            }
        };
        MovieFrame {
            source,
            destination: Rect {
                left,
                top: 0,
                right,
                bottom: interval.height,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovieFrame {
    pub source: Rect,
    pub destination: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraKey {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub lens: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeySelection {
    pub index: i32,
    pub fraction: f32,
}

/// Selection slice of 0xC95E0. Key storage and discontinuity detection remain data/animation edges.
pub fn select_keys(
    time: f32,
    rate: f32,
    count: i32,
    discontinuity: bool,
    midpoint: f32,
) -> Option<KeySelection> {
    let mut index = trunc_i32(time * rate);
    if index < 0 {
        return None;
    }
    if index > count.wrapping_sub(5) && index > 0 {
        index = count.wrapping_sub(5);
    }
    let raw = (time - index as f32 / rate) * rate;
    let mut fraction = if 0.0 > raw {
        0.0
    } else if raw > 1.0 {
        1.0
    } else {
        raw
    };
    if discontinuity {
        fraction = if midpoint > fraction { 0.0 } else { 1.0 };
    }
    Some(KeySelection { index, fraction })
}

/// Original lens constants are runtime inputs, never guessed disc values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensCalibration {
    pub lens_scale: f32,
    pub numerator: f32,
    pub aperture_scale: f32,
    pub aperture: f32,
    pub angle_units: f32,
    pub wide_scale: f32,
}

impl LensCalibration {
    pub fn original_fov_with(&self, lens: f32, wide: bool, atan2: fn(f64, f64) -> f64) -> f32 {
        let ratio = self.numerator as f64 / (lens as f64 * self.lens_scale as f64)
            * self.aperture_scale as f64
            * self.aperture as f64;
        let fov = (atan2(ratio, 1.0) * 2.0 * self.angle_units as f64) as f32;
        if wide { fov * self.wide_scale } else { fov }
    }
}

/// Explicit animation boundary: preserve the original quaternion interpolation/matrix convention.
pub trait CameraMath {
    fn interpolate_rotation(
        &mut self,
        first: [f32; 4],
        next: [f32; 4],
        fraction: f32,
    ) -> [[f32; 3]; 3];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPresentation {
    pub position: [f32; 3],
    pub basis: [[f32; 3]; 3],
    pub fov: f32,
}

/// Projection-service input shared by the camera roots: the game passes this ratio to 0xC77C0.
/// Positive display geometry is required; physical window aspect is a separate policy input.
pub fn original_projection_ratio(display: Extent, interval: Extent) -> f32 {
    ((interval.height as f64 / display.height as u32 as f64)
        / (interval.width as f64 / display.width as u32 as f64)) as f32
}
