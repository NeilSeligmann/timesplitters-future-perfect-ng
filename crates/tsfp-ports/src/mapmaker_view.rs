// SPDX-License-Identifier: GPL-3.0-or-later
//! T2427: self-contained pure pieces of the Map Maker per-frame editor update `0x306460`
//! (1,380 instructions, see `docs/t2427-mapmaker-306460-map.md`).
//!
//! These are typed, readable models of the arithmetic and small state machines inside the
//! function. They touch no guest memory and are not wired to a lane root yet: the whole-function
//! port (callees stay guest calls, behind the `TSFP_MAPMAKER_OVERLAY` A/B switch, default on) is the
//! child task recorded in the map doc. Evidence: Tier 0 spec tests derived from REA pseudo code
//! (Ghidra 12.1.4 on `tsfp-analysis-v4.elf`, sha256 prefix `e99faaeb`); the float expression
//! order follows the pseudo code, single precision is INFERRED.

/// `ctx+0x260`: the hint fade timer. Positive counts down, negative counts up, both by
/// `(1 / fps) * scale * 3` per frame, and snap to zero inside the dead band of 0.02.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HintFade {
    pub value: f32,
    /// `0x4E8CEC` (i32): frames per second.
    pub frame_rate: i32,
    /// `0x4E7948`: time scale.
    pub scale: f32,
}

impl HintFade {
    /// `0x4785D4` (0.02f) and `0x49E084` (-0.02f), compared in double precision like the original.
    const DEAD_BAND: f64 = 0.02_f32 as f64;

    fn delta(&self) -> f32 {
        (1.0 / (self.frame_rate as f32)) * self.scale * 3.0
    }

    /// One frame. Zero and NaN stay as they are (T2429: the lifted body leaves a NaN timer alone, the
    /// pseudo code read in T2427 suggested a snap). A positive timer snaps to zero when it drops
    /// below the dead band, a negative one when it rises above it.
    pub fn step(&mut self) {
        let v = self.value;
        if v > 0.0 {
            self.value = v - self.delta();
            if Self::DEAD_BAND > f64::from(self.value) {
                self.value = 0.0;
            }
        } else if v < 0.0 {
            self.value = self.delta() + v;
            if f64::from(self.value) > -Self::DEAD_BAND {
                self.value = 0.0;
            }
        }
    }
}

/// The cursor tool mode `ctx+0x254` (0 to 4 on the ring, plus 5 and up for modal tools).
/// Left and right on the pad cycle it; the ring has `base + 3` entries where `base` is 2 when
/// `ctx+0x200` is set (extended toolset), else 0.
pub fn cycle_tool_mode(current: i32, extended: bool, buttons: u32) -> i32 {
    let base = i32::from(extended) * 2;
    let step = if buttons & 2 != 0 {
        1
    } else if buttons & 1 != 0 {
        -1
    } else {
        return current;
    };
    let next = current + step;
    if next < 0 {
        base + 2
    } else if base + 3 <= next {
        0
    } else {
        next
    }
}

/// Result of a build-height step (`ctx+0x264`, `ctx+0x268`) with the sound to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeightStep {
    pub level: i32,
    /// New `ctx+0x268`; `None` when the original leaves the word untouched (refused step).
    pub below: Option<i32>,
    /// Sound id handed to `0x2F79E0` (`0x157D` moved, `0x1576` refused).
    pub sound: u32,
}

/// Raise (`buttons & 0x800`) or lower (`0x400`) the build height. `None` when neither applies
/// (no extended toolset or no button), in which case nothing plays.
pub fn step_height(level: i32, extended: bool, buttons: u32) -> Option<HeightStep> {
    if !extended {
        return None;
    }
    if buttons & 0x800 != 0 {
        if level < 2 {
            let level = level + 1;
            Some(HeightStep {
                level,
                below: Some(level - 1),
                sound: 0x157d,
            })
        } else {
            Some(HeightStep {
                level,
                below: None,
                sound: 0x1576,
            })
        }
    } else if buttons & 0x400 != 0 {
        if level < -1 {
            return Some(HeightStep {
                level,
                below: None,
                sound: 0x1576,
            });
        }
        let level = level - 1;
        let below = if level < -1 { -2 } else { level - 1 };
        Some(HeightStep {
            level,
            below: Some(below),
            sound: 0x157d,
        })
    } else {
        None
    }
}

/// Which way the editor camera pans, written to `0x783CC8` (1 right, 2 left, 3 up, 4 down).
/// `None` leaves the word unchanged (both axes zero or only a NaN-free down with dy == 0).
pub fn scroll_facing(dx: f32, dy: f32) -> Option<u32> {
    // T2429: the lifted body treats a NaN dx like zero and falls through to the dy tests.
    if dx < 0.0 {
        Some(2)
    } else if dx > 0.0 {
        Some(1)
    } else if dy < 0.0 {
        Some(3)
    } else if dy > 0.0 {
        Some(4)
    } else {
        None
    }
}

/// Viewport mapping used by the pointer reads: `(screen - origin) * scale + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewAxis {
    /// `0x783EE0` / `0x783DC4`: screen origin (i32 words).
    pub origin: i32,
    /// `0x783DD4` / `0x783F20`: world units per screen unit.
    pub scale: f32,
    /// `0x783DD8` / `0x783CC0`: world offset.
    pub offset: f32,
}

impl ViewAxis {
    pub fn to_world(&self, screen: i32) -> f32 {
        (screen as f32 - self.origin as f32) * self.scale + self.offset
    }
}

/// The editor zoom (`0x783DD4..0x783F2C`): a multiplicative step per frame, clamped to
/// `[width / 60, width / 8]`, with the view eased 1/16 of the way toward the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    /// `0x783F2C`, `0x783F28`: screen units per world unit on x and y.
    pub scale_x: f32,
    pub scale_y: f32,
    /// `0x783DE4`, `0x783CCC`: viewport width and height in screen units.
    pub width: f32,
    pub height: f32,
    /// `0x783DE0`, `0x783DDC`: current world extent.
    pub extent_x: f32,
    pub extent_y: f32,
    /// `0x783DD8`, `0x783CC0`: world offset (left, top).
    pub left: f32,
    pub top: f32,
}

/// Outputs of a [`Zoom`] step (the original also calls `0x71140` with `pan`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomResult {
    pub zoom: Zoom,
    /// `0x783DD4`, `0x783F20`: reciprocals of the scales.
    pub inverse_x: f32,
    pub inverse_y: f32,
    /// `0x783DC0`, `0x783DCC`: world right and bottom edges.
    pub right: f32,
    pub bottom: f32,
    /// Arguments of `0x71140`.
    pub pan: (f32, f32),
}

impl Zoom {
    /// One step by `factor` (1.02 in, 0.98 out) toward the pointer world position.
    pub fn step(&self, factor: f32, pointer_x: f32, pointer_y: f32) -> ZoomResult {
        let min = self.width * 0.016_666_668;
        let max = self.width * 0.125;
        let aspect = self.height / self.width;
        let mut sx = self.scale_x * factor;
        let mut sy = self.scale_y * factor;
        if sx < min {
            sx = min;
            sy = self.height / (aspect * 60.0);
        } else if max < sx {
            sx = max;
            sy = self.height / (aspect * 8.0);
        }
        let inverse_x = 1.0 / sx;
        let inverse_y = 1.0 / sy;
        let extent_x = self.width * inverse_x;
        let extent_y = self.height * inverse_y;
        let mid_x = self.extent_x * 0.5 + self.left;
        let mid_y = self.extent_y * 0.5 + self.top;
        let left = ((pointer_x - mid_x) * 0.0625 + mid_x) - extent_x * 0.5;
        let top = ((pointer_y - mid_y) * 0.0625 + mid_y) - extent_y * 0.5;
        ZoomResult {
            zoom: Zoom {
                scale_x: sx,
                scale_y: sy,
                extent_x,
                extent_y,
                left,
                top,
                ..*self
            },
            inverse_x,
            inverse_y,
            right: extent_x + left,
            bottom: extent_y + top,
            pan: ((pointer_x - left) * sx, (pointer_y - top) * sy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_counts_down_and_snaps() {
        let mut f = HintFade {
            value: 1.0,
            frame_rate: 60,
            scale: 1.0,
        };
        f.step();
        assert!((f.value - (1.0 - 0.05)).abs() < 1e-6);
        f.value = 0.03;
        f.step();
        assert_eq!(f.value, 0.0, "falls below the dead band, snaps to zero");
    }

    #[test]
    fn fade_counts_up_from_negative() {
        let mut f = HintFade {
            value: -1.0,
            frame_rate: 60,
            scale: 1.0,
        };
        f.step();
        assert!((f.value + 0.95).abs() < 1e-6);
        f.value = -0.03;
        f.step();
        assert_eq!(f.value, 0.0);
        f.value = 0.0;
        f.step();
        assert_eq!(f.value, 0.0);
    }

    #[test]
    fn mode_ring_wraps() {
        assert_eq!(
            cycle_tool_mode(0, false, 1),
            2,
            "basic ring is 0..=2, left wraps to 2"
        );
        assert_eq!(cycle_tool_mode(2, false, 2), 0);
        assert_eq!(cycle_tool_mode(0, true, 1), 4, "extended ring is 0..=4");
        assert_eq!(cycle_tool_mode(4, true, 2), 0);
        assert_eq!(cycle_tool_mode(1, true, 0), 1);
        assert_eq!(cycle_tool_mode(1, true, 3), 2, "right wins over left");
    }

    #[test]
    fn height_steps() {
        assert_eq!(step_height(0, false, 0x800), None);
        assert_eq!(
            step_height(0, true, 0x800),
            Some(HeightStep {
                level: 1,
                below: Some(0),
                sound: 0x157d
            })
        );
        assert_eq!(step_height(2, true, 0x800).map(|s| s.sound), Some(0x1576));
        assert_eq!(
            step_height(0, true, 0x400),
            Some(HeightStep {
                level: -1,
                below: Some(-2),
                sound: 0x157d
            })
        );
        assert_eq!(
            step_height(-1, true, 0x400),
            Some(HeightStep {
                level: -2,
                below: Some(-2),
                sound: 0x157d
            })
        );
        assert_eq!(step_height(-2, true, 0x400).map(|s| s.sound), Some(0x1576));
        assert_eq!(step_height(0, true, 0), None);
    }

    #[test]
    fn facing() {
        assert_eq!(scroll_facing(1.0, 5.0), Some(1));
        assert_eq!(scroll_facing(-1.0, 0.0), Some(2));
        assert_eq!(scroll_facing(0.0, -1.0), Some(3));
        assert_eq!(scroll_facing(0.0, 1.0), Some(4));
        assert_eq!(scroll_facing(0.0, 0.0), None);
    }

    #[test]
    fn zoom_clamps() {
        let z = Zoom {
            scale_x: 50.0,
            scale_y: 37.5,
            width: 640.0,
            height: 480.0,
            extent_x: 6.4,
            extent_y: 4.8,
            left: 0.0,
            top: 0.0,
        };
        let r = z.step(1.02, 3.0, 2.0);
        assert!((r.zoom.scale_x - 51.0).abs() < 1e-3);
        let hi = Zoom { scale_x: 79.9, ..z }.step(1.02, 0.0, 0.0);
        assert_eq!(hi.zoom.scale_x, 80.0, "capped at width / 8");
        let lo = Zoom { scale_x: 10.0, ..z }.step(0.98, 0.0, 0.0);
        assert!(
            (lo.zoom.scale_x - 640.0 * 0.016_666_668).abs() < 1e-4,
            "floored at width / 60"
        );
    }

    #[test]
    fn axis_mapping() {
        let a = ViewAxis {
            origin: 10,
            scale: 0.5,
            offset: 3.0,
        };
        assert_eq!(a.to_world(30), 13.0);
    }
}
