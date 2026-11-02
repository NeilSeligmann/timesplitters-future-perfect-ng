//! T2191 UI-8: cinematic letterbox bars as typed, readable Rust (`original` preset).
//!
//! Same behaviour as the Tier 1 guest-memory port `tsfp_ui::hud::letterbox` (0x139520, 0x139570,
//! 0x1395C0, 0x1396B0), but over owned state: no guest addresses, no retail constants. The four
//! tuning values (full amount, fade rate, draw minimum, bar scale) are supplied at run time from
//! the original tables. Contract: docs/t2191-ingame-ui.md, section "Letterbox model".

/// Phase of the bar animation (`0x7490F0` in the original: 1, 0, -1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LetterboxPhase {
    FadingIn,
    Holding,
    FadingOut,
    /// Any other stored value: the update does nothing (kept for faithfulness).
    Idle(i32),
}

/// Owned letterbox state. `amount` is 0 hidden, `tuning.full` fully shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Letterbox {
    pub amount: f32,
    pub phase: LetterboxPhase,
    /// Frames left to hold; negative holds forever.
    pub hold_frames: i32,
    /// Nonzero suppresses the draw (the `force` argument of the start calls).
    pub suppress_draw: u32,
}

impl Default for Letterbox {
    fn default() -> Self {
        Self {
            amount: 0.0,
            phase: LetterboxPhase::Holding,
            hold_frames: 0,
            suppress_draw: 0,
        }
    }
}

/// Run-time constants from the original tables (never committed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LetterboxTuning {
    pub full: f32,
    /// Amount per second (divided by the integer frame rate, times the frame step).
    pub rate: f32,
    /// The draw is skipped while `amount <= draw_min`.
    pub draw_min: f32,
    /// Bar height in pixels per unit amount.
    pub bar_scale: f32,
}

/// Frame timing read by the update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameTiming {
    pub delta_frames: i32,
    pub step: f32,
    pub rate_hz: i32,
}

/// Session facts that gate the start calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionGate {
    /// Mode byte equals the split-screen mode (0x0A).
    pub split_screen_mode: bool,
    pub local_players: i32,
    /// Mode flag 0x200 (cinematics allowed in split screen).
    pub allow_in_split: bool,
}

impl SessionGate {
    fn blocks(self, force: u32) -> bool {
        self.split_screen_mode && self.local_players > 1 && !self.allow_in_split && force == 0
    }
}

/// Guest viewport edges (`[0x7E71C8]`: x0, x1, y0, y1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewportEdges {
    pub x0: i32,
    pub x1: i32,
    pub y0: i32,
    pub y1: i32,
}

/// One filled bar, right and bottom exclusive, as the original passes it to the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Bars to draw this frame, bottom first like the original, both at alpha 0x80.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LetterboxBars {
    pub bottom: Bar,
    pub top: Bar,
}

pub const BAR_ALPHA: u8 = 0x80;

/// x87 `!(a > b)`: true for NaN, like the original compare.
fn not_above(a: f32, b: f32) -> bool {
    a.partial_cmp(&b) != Some(std::cmp::Ordering::Greater)
}

/// `cvttss2si`: truncation, NaN and out of range give `i32::MIN`.
fn truncate(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

impl Letterbox {
    /// 0x139520: show bars at full amount immediately and hold them.
    pub fn start_hold(
        &mut self,
        gate: SessionGate,
        tuning: &LetterboxTuning,
        hold_frames: u32,
        force: u32,
    ) {
        if gate.blocks(force) {
            return;
        }
        self.suppress_draw = force;
        self.amount = tuning.full;
        self.hold_frames = hold_frames as i32;
        self.phase = LetterboxPhase::Holding;
    }

    /// 0x139570: fade the bars in, then hold.
    pub fn start_fade_in(&mut self, gate: SessionGate, hold_frames: u32, force: u32) {
        if gate.blocks(force) {
            return;
        }
        self.suppress_draw = force;
        self.phase = LetterboxPhase::FadingIn;
        self.hold_frames = hold_frames as i32;
    }

    /// 0x1395C0: per-frame update, fade in, hold countdown, fade out.
    pub fn update(&mut self, tuning: &LetterboxTuning, timing: FrameTiming) {
        let per_frame = |rate: f32| rate / timing.rate_hz as f32 * timing.step;
        match self.phase {
            LetterboxPhase::FadingIn => {
                if not_above(tuning.full, self.amount) {
                    return;
                }
                self.amount += per_frame(tuning.rate);
                if !not_above(self.amount, tuning.full) {
                    self.phase = LetterboxPhase::Holding;
                    self.amount = tuning.full;
                }
            }
            LetterboxPhase::Holding => {
                if self.hold_frames < 0 {
                    return;
                }
                self.hold_frames = self.hold_frames.wrapping_sub(timing.delta_frames);
                if self.hold_frames <= 0 {
                    self.hold_frames = 0;
                    self.phase = LetterboxPhase::FadingOut;
                }
            }
            LetterboxPhase::FadingOut => {
                if not_above(self.amount, 0.0) {
                    return;
                }
                self.amount -= per_frame(tuning.rate);
                if !not_above(0.0, self.amount) {
                    self.phase = LetterboxPhase::Holding;
                    self.amount = 0.0;
                }
            }
            LetterboxPhase::Idle(_) => {}
        }
    }

    /// 0x1396B0: the two bars to draw, `None` when suppressed or too small.
    pub fn bars(&self, tuning: &LetterboxTuning, view: ViewportEdges) -> Option<LetterboxBars> {
        if self.suppress_draw != 0 || not_above(self.amount, tuning.draw_min) {
            return None;
        }
        let height = truncate(self.amount * tuning.bar_scale);
        let right = view.x1.wrapping_add(1);
        Some(LetterboxBars {
            bottom: Bar {
                left: view.x0,
                top: view.y1.wrapping_sub(height),
                right,
                bottom: view.y1.wrapping_add(1),
            },
            top: Bar {
                left: view.x0,
                top: view.y0,
                right,
                bottom: view.y0.wrapping_add(height).wrapping_add(1),
            },
        })
    }
}

impl LetterboxPhase {
    /// The original's stored phase word.
    pub fn to_raw(self) -> i32 {
        match self {
            Self::FadingIn => 1,
            Self::Holding => 0,
            Self::FadingOut => -1,
            Self::Idle(raw) => raw,
        }
    }

    pub fn from_raw(raw: i32) -> Self {
        match raw {
            1 => Self::FadingIn,
            0 => Self::Holding,
            -1 => Self::FadingOut,
            other => Self::Idle(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TUNING: LetterboxTuning = LetterboxTuning {
        full: 1.0,
        rate: 0.5,
        draw_min: 0.01,
        bar_scale: 60.0,
    };
    const TIMING: FrameTiming = FrameTiming {
        delta_frames: 1,
        step: 1.0,
        rate_hz: 60,
    };
    const OPEN: SessionGate = SessionGate {
        split_screen_mode: false,
        local_players: 1,
        allow_in_split: false,
    };

    #[test]
    fn fade_in_hold_fade_out_cycle() {
        let mut bars = Letterbox::default();
        bars.start_fade_in(OPEN, 3, 0);
        let mut frames = 0;
        while bars.phase == LetterboxPhase::FadingIn {
            bars.update(&TUNING, TIMING);
            frames += 1;
        }
        assert_eq!(bars.amount, 1.0);
        assert!((119..=121).contains(&frames), "{frames}");
        for _ in 0..3 {
            bars.update(&TUNING, TIMING);
        }
        assert_eq!(bars.phase, LetterboxPhase::FadingOut);
        while bars.phase == LetterboxPhase::FadingOut {
            bars.update(&TUNING, TIMING);
        }
        assert_eq!(bars.amount, 0.0);
        assert!(
            bars.bars(
                &TUNING,
                ViewportEdges {
                    x0: 0,
                    x1: 639,
                    y0: 0,
                    y1: 479
                }
            )
            .is_none()
        );
    }

    #[test]
    fn split_screen_blocks_unforced_start() {
        let gate = SessionGate {
            split_screen_mode: true,
            local_players: 2,
            allow_in_split: false,
        };
        let mut bars = Letterbox::default();
        bars.start_hold(gate, &TUNING, 10, 0);
        assert_eq!(bars, Letterbox::default());
        bars.start_hold(gate, &TUNING, 10, 1);
        assert_eq!(bars.amount, 1.0);
        assert_eq!(bars.suppress_draw, 1);
    }

    #[test]
    fn full_bars_geometry() {
        let mut bars = Letterbox::default();
        bars.start_hold(OPEN, &TUNING, 10, 0);
        let drawn = bars
            .bars(
                &TUNING,
                ViewportEdges {
                    x0: 0,
                    x1: 639,
                    y0: 0,
                    y1: 479,
                },
            )
            .unwrap();
        assert_eq!(
            drawn.top,
            Bar {
                left: 0,
                top: 0,
                right: 640,
                bottom: 61
            }
        );
        assert_eq!(
            drawn.bottom,
            Bar {
                left: 0,
                top: 419,
                right: 640,
                bottom: 480
            }
        );
    }
}
