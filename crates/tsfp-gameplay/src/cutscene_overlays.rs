// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable cutscene overlay request contract; no guest addresses.
use super::movie_setup::{Extent, Rect, trunc_i32};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackClock {
    pub time: f32,
    pub ending: bool,
    pub end_elapsed: f32,
    pub end_duration: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlaySample {
    pub strip_height: f32,
    pub gray: f32,
    pub color: u32,
    pub additive_color: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayConstants {
    pub threshold: f32,
    pub opening_end: f32,
    pub opening_rate: f32,
    pub alpha_scale: f32,
    pub fixed_bar_fraction: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayInput {
    pub clock: Option<PlaybackClock>,
    pub sample: OverlaySample,
    pub viewport: Rect,
    pub interval: Extent,
    pub wide_display: bool,
    pub suppress_fallback: bool,
    pub force_black: bool,
    pub record_allows_scene: bool,
    pub constants: OverlayConstants,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayCommand {
    /// The strip rasterizer owns gradient/vertex generation; request preserves original arguments.
    Strips {
        height: f32,
        viewport: Rect,
    },
    Gray {
        alpha: i32,
    },
    Fill {
        rect: Rect,
        color: u32,
        blend: Blend,
    },
    /// Boundary event, not a port of the UI/subtitle roots.
    Subtitles,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Normal,
    Additive,
}

/// Ordered renderer requests from the original 0x6F150. 0x6F2A0 is an interior address.
// portreg: va=0x0006f150 tier=1
// portreg: replaces=tsfp-ports/c2r/t2049_exec_009
pub fn original_overlays(input: &OverlayInput, mut emit: impl FnMut(OverlayCommand)) {
    let full = Rect {
        left: 0,
        top: 0,
        right: input.interval.width,
        bottom: input.interval.height,
    };
    let fill = |color, blend| OverlayCommand::Fill {
        rect: full,
        color,
        blend,
    };
    let c = input.constants;
    if let Some(clock) = input.clock {
        if input.sample.strip_height > c.threshold {
            let mut viewport = input.viewport;
            viewport.right = viewport.right.wrapping_add(1);
            viewport.bottom = viewport.bottom.wrapping_add(1);
            emit(OverlayCommand::Strips {
                height: input.sample.strip_height,
                viewport,
            });
        }
        if input.sample.gray > c.threshold {
            emit(OverlayCommand::Gray {
                alpha: (input.sample.gray as f64 * c.alpha_scale as f64) as i32,
            });
        }
        if input.sample.color != 0 {
            emit(fill(input.sample.color, Blend::Normal));
        }
        if input.sample.additive_color != 0 {
            emit(fill(input.sample.additive_color | 0xff, Blend::Additive));
        }
        let fade = if clock.ending {
            Some(clock.end_elapsed / clock.end_duration)
        } else if c.opening_end > clock.time {
            Some(if clock.time <= 0.0 {
                0.0
            } else {
                clock.time * c.opening_rate
            })
        } else {
            None
        };
        if let Some(fade) = fade {
            // The SSE comparison maps unordered values to one, not zero.
            let fade = if 1.0 > fade { fade } else { 1.0 };
            let alpha = trunc_i32(c.alpha_scale - fade * c.alpha_scale) as u32 & 0xff;
            emit(fill(alpha, Blend::Normal));
        }
        if !input.wide_display {
            let height = trunc_i32(input.interval.height as f32 * c.fixed_bar_fraction);
            emit(OverlayCommand::Fill {
                rect: Rect {
                    bottom: height,
                    ..full
                },
                color: 0x7f,
                blend: Blend::Normal,
            });
            emit(OverlayCommand::Fill {
                rect: Rect {
                    top: input.interval.height.wrapping_sub(height),
                    ..full
                },
                color: 0x7f,
                blend: Blend::Normal,
            });
        }
        emit(OverlayCommand::Subtitles);
        if !input.record_allows_scene && input.force_black {
            emit(fill(0xff, Blend::Normal));
        }
        if input.force_black || input.suppress_fallback {
            return;
        }
    }
    emit(fill(0xff, Blend::Normal));
}
