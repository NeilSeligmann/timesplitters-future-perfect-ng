// SPDX-License-Identifier: GPL-3.0-or-later
//! T2195: typed, opt-in cutscene presentation. See docs/t-cutscene-presentation.md.
//! Source-inspected behaviour; Tier 1 uses translated references and fabricated inputs.
//! No guest memory, decoder, menu, subtitle renderer, or camera-mode dispatcher is owned here.

#[path = "cutscene_original_core.rs"]
mod original_core;
pub use original_core::movie_setup;
pub use original_core::*;
#[path = "cutscene_overlays.rs"]
mod overlay_model;
pub use overlay_model::*;

impl LensCalibration {
    pub fn original_fov(&self, lens: f32, wide: bool) -> f32 {
        self.original_fov_with(lens, wide, f64::atan2)
    }
}

pub fn camera_sample(
    first: CameraKey,
    next: CameraKey,
    fraction: f32,
    calibration: LensCalibration,
    wide: bool,
    math: &mut impl CameraMath,
) -> CameraPresentation {
    let mut position = first.position;
    for (axis, value) in position.iter_mut().enumerate() {
        *value = (next.position[axis] - first.position[axis]) * fraction + first.position[axis];
    }
    // Lens uses the same key blend as position, before the x87 optical conversion.
    CameraPresentation {
        position,
        basis: math.interpolate_rotation(first.rotation, next.rotation, fraction),
        fov: calibration.original_fov((next.lens - first.lens) * fraction + first.lens, wide),
    }
}
/// T2170 customises geometry/lens here; T2156 supplies the authoritative clock/key snapshot.
/// Original is an identity policy. Features remain opt-in and must not advance playback time.
pub trait PresentationPolicy {
    fn overlay(&self, command: OverlayCommand) -> OverlayCommand {
        command
    }
    fn movie(&self, frame: MovieFrame) -> MovieFrame {
        frame
    }
    fn camera(&self, camera: CameraPresentation) -> CameraPresentation {
        camera
    }
}

#[derive(Default)]
pub struct Original;
impl PresentationPolicy for Original {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresentationState {
    pub clock: PlaybackClock,
    pub movie: MovieState,
    pub camera: Option<CameraPresentation>,
}

impl PresentationState {
    pub fn overlays(
        &self,
        mut input: OverlayInput,
        policy: &impl PresentationPolicy,
        mut emit: impl FnMut(OverlayCommand),
    ) {
        input.clock = Some(self.clock);
        original_overlays(&input, |command| emit(policy.overlay(command)));
    }
}

/// Presentation part of the general camera dispatcher (0xCCAB0, 0xCDC4B..0xCDD35).
/// Mode dispatch, camera tracking and collision stay outside this cutscene slice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LensSource {
    Optical {
        lens: f32,
        wide: bool,
    },
    /// Camera flag 0x01000000 delegates to the camera service 0xCB3B0.
    CameraService {
        fov: f32,
    },
    /// A mode which does not enter either lens-write branch preserves both values.
    Preserve,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensState {
    pub current_fov: f32,
    pub target_fov: f32,
}

impl LensState {
    pub fn apply(&mut self, source: LensSource, calibration: LensCalibration) {
        let fov = match source {
            LensSource::Optical { lens, wide } => calibration.original_fov(lens, wide),
            LensSource::CameraService { fov } => fov,
            LensSource::Preserve => return,
        };
        self.current_fov = fov;
        self.target_fov = fov;
    }
}

/// Opt-in native presentation geometry. Dimensions describe physical square pixels;
/// content aspect is supplied by playback metadata, not inferred from crop margins.
/// Original callers keep using `Original` and `original_fov` unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidePresentation {
    output: Extent,
}

impl WidePresentation {
    pub fn new(output: Extent) -> Option<Self> {
        (output.width > 0 && output.height > 0).then_some(Self { output })
    }

    pub fn aspect(self) -> f64 {
        self.output.width as f64 / self.output.height as f64
    }

    /// Fit the whole declared content in the output. No stretching or extra crop.
    /// Odd remainders go to the right/bottom; both edges stay inside the target.
    pub fn content_rect(self, content_aspect: f64) -> Option<Rect> {
        if !content_aspect.is_finite() || content_aspect <= 0.0 {
            return None;
        }
        let (width, height) = if self.aspect() >= content_aspect {
            (
                ((self.output.height as f64 * content_aspect).round() as i32)
                    .clamp(1, self.output.width),
                self.output.height,
            )
        } else {
            (
                self.output.width,
                ((self.output.width as f64 / content_aspect).round() as i32)
                    .clamp(1, self.output.height),
            )
        };
        let left = (self.output.width - width) / 2;
        let top = (self.output.height - height) / 2;
        Some(Rect {
            left,
            top,
            right: left + width,
            bottom: top + height,
        })
    }

    /// Full decoded source, including the original attract crop, is an explicit opt-in.
    /// Decoder pixel aspect must be represented by `content_aspect`.
    pub fn movie_frame(self, decoded: Extent, content_aspect: f64) -> Option<MovieFrame> {
        if decoded.width <= 0 || decoded.height <= 0 {
            return None;
        }
        Some(MovieFrame {
            source: Rect {
                left: 0,
                top: 0,
                right: decoded.width,
                bottom: decoded.height,
            },
            destination: self.content_rect(content_aspect)?,
        })
    }

    /// Opaque black bars around the content, in left/right/top/bottom order.
    /// This is separate from fades, subtitles and animated strip requests: those
    /// must retain their own blend/order and must not be recognised by rectangle shape.
    pub fn bars(self, content_aspect: f64) -> Option<Vec<Rect>> {
        let content = self.content_rect(content_aspect)?;
        let full = Rect {
            left: 0,
            top: 0,
            right: self.output.width,
            bottom: self.output.height,
        };
        let candidates = [
            Rect {
                right: content.left,
                ..full
            },
            Rect {
                left: content.right,
                ..full
            },
            Rect {
                left: content.left,
                right: content.right,
                bottom: content.top,
                ..full
            },
            Rect {
                left: content.left,
                right: content.right,
                top: content.bottom,
                ..full
            },
        ];
        Some(
            candidates
                .into_iter()
                .filter(|r| r.right > r.left && r.bottom > r.top)
                .collect(),
        )
    }

    /// Only the optical/key lens path uses this policy, never the fixed gameplay FOV.
    /// At exactly 4:3/16:9 retain the original arithmetic and wide multiplier.
    /// Other aspects generalise in tangent space as specified by T2036 section 3.
    pub fn optical_fov(self, calibration: LensCalibration, lens: f32) -> f32 {
        if self.output.width as i64 * 3 == self.output.height as i64 * 4 {
            return calibration.original_fov(lens, false);
        }
        if self.output.width as i64 * 9 == self.output.height as i64 * 16 {
            return calibration.original_fov(lens, true);
        }
        let base = calibration.original_fov(lens, false) as f64;
        let half = base / (2.0 * calibration.angle_units as f64);
        (2.0 * (half.tan() * ((4.0 / 3.0) / self.aspect())).atan() * calibration.angle_units as f64)
            as f32
    }
}
