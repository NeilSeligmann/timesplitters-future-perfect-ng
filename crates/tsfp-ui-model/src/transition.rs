//! Page and row fade timing of the original toolkit as plain data (T2189 slice 2, UI-6).
//!
//! MEASURED on the captured timelines (`tools.ui_tree layout`) and read from the translated code
//! (0x7A7B0 page update, 0x71D00 page alpha, 0x76400 row fades). Everything is linear in time;
//! the original has no ease curve for page fades. One step is `(1 / steps_per_second) * speed`
//! seconds with 60 steps per second and speed 1.0, so a full fade is a whole number of steps.
//!
//! Page life cycle (page cell 0x1B0): `Waiting` (1) until the page is ready and `delay` seconds
//! passed, `Opening` (2) with alpha = timer / open, `Open` (3) alpha 1, `Closing` (4) with
//! alpha = 1 - timer / close, `Closed` (5).

use crate::model::{Ease, Transition, TransitionKind};

/// Seconds per step at 60 steps per second and speed 1.0.
pub const STEP_SECONDS: f32 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagePhase {
    Waiting,
    Opening,
    Open,
    Closing,
    Closed,
}

impl PagePhase {
    /// Value of the original page cell 0x1B0.
    pub fn code(self) -> u32 {
        match self {
            PagePhase::Waiting => 1,
            PagePhase::Opening => 2,
            PagePhase::Open => 3,
            PagePhase::Closing => 4,
            PagePhase::Closed => 5,
        }
    }
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            1 => PagePhase::Waiting,
            2 => PagePhase::Opening,
            3 => PagePhase::Open,
            4 => PagePhase::Closing,
            5 => PagePhase::Closed,
            _ => return None,
        })
    }
}

/// Durations in seconds (build context cells 0x34, 0x38, 0x3C).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageTiming {
    pub delay: f32,
    pub open: f32,
    pub close: f32,
}

impl PageTiming {
    /// Front end menu pages: 0.6 s in, 0.25 s out (MEASURED, the common case of the captured pages).
    pub const MENU: PageTiming = PageTiming {
        delay: 0.0,
        open: 0.6,
        close: 0.25,
    };
    /// The pause and in-game list family: 0.35 s both ways (MEASURED).
    pub const PAUSE: PageTiming = PageTiming {
        delay: 0.0,
        open: 0.35,
        close: 0.35,
    };
    /// Popup pages: 0.1 s in, 0.2 s out (MEASURED).
    pub const POPUP: PageTiming = PageTiming {
        delay: 0.0,
        open: 0.1,
        close: 0.2,
    };
    /// The model transitions for this timing (linear fades, milliseconds rounded).
    pub fn transitions(&self) -> (Transition, Transition) {
        let fade = |seconds: f32| Transition {
            kind: TransitionKind::Fade,
            duration_ms: (seconds * 1000.0).round() as u32,
            ease: Ease::Linear,
        };
        (fade(self.open), fade(self.close))
    }
}

/// Running page life cycle: the timer cell 0x1B4 and the state cell 0x1B0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageLife {
    pub phase: PagePhase,
    pub timer: f32,
    pub timing: PageTiming,
}

impl PageLife {
    /// Create a page that waits for readiness before opening.
    ///
    /// ```
    /// use tsfp_ui_model::transition::{PageLife, PagePhase, PageTiming};
    /// let mut page = PageLife::new(PageTiming::MENU);
    /// assert_eq!(page.phase, PagePhase::Waiting);
    /// assert!(page.advance(0.0, true));
    /// assert_eq!(page.phase, PagePhase::Opening);
    /// ```
    pub fn new(timing: PageTiming) -> Self {
        Self {
            phase: PagePhase::Waiting,
            timer: 0.0,
            timing,
        }
    }
    /// Page alpha (cell 0x1B8, original 0x71D00).
    pub fn alpha(&self) -> f32 {
        match self.phase {
            PagePhase::Opening if self.timing.open > 0.0 => self.timer / self.timing.open,
            PagePhase::Open => 1.0,
            PagePhase::Closing if self.timing.close > 0.0 => 1.0 - self.timer / self.timing.close,
            _ => 0.0,
        }
    }
    /// One frame of `dt` seconds (original 0x7A7B0). `ready` is the "page may start" predicate
    /// (0x75920). Returns true when the phase changed.
    pub fn advance(&mut self, dt: f32, ready: bool) -> bool {
        self.timer += dt;
        match self.phase {
            PagePhase::Waiting => {
                if !ready {
                    self.timer = 0.0;
                } else if self.timer >= self.timing.delay {
                    self.phase = PagePhase::Opening;
                    self.timer = 0.0;
                    return true;
                }
            }
            PagePhase::Opening => {
                if self.timer >= self.timing.open {
                    self.phase = PagePhase::Open;
                    self.timer = 0.0;
                    return true;
                }
            }
            PagePhase::Closing => {
                if self.timer >= self.timing.close {
                    self.timer = self.timing.close;
                }
            }
            PagePhase::Open | PagePhase::Closed => {}
        }
        false
    }
    /// Starts the close fade (original 0x78D80 sets state 4 and clears the timer).
    pub fn close(&mut self) {
        if !matches!(self.phase, PagePhase::Closing | PagePhase::Closed) {
            self.phase = PagePhase::Closing;
            self.timer = 0.0;
        }
    }
}

/// Row highlight fades (row cells 0x34 and 0x104, original 0x76400): linear one step per frame
/// between 0 and 1, up while the row is the page cursor and the page accepts input, else down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowFade {
    pub highlight: f32,
    pub pulse: f32,
}

impl RowFade {
    pub const IDLE: RowFade = RowFade {
        highlight: 0.0,
        pulse: 0.0,
    };
    /// `step` is `STEP_SECONDS * speed`. `cursor`: the row is the active cursor row;
    /// `pulse_up`: row kind flag 0x10.
    pub fn advance(&mut self, step: f32, cursor: bool, pulse_up: bool) {
        self.highlight = if cursor {
            (self.highlight + step).min(1.0)
        } else {
            (self.highlight - step).max(0.0)
        };
        self.pulse = if pulse_up {
            (self.pulse + step).min(1.0)
        } else {
            (self.pulse - step).max(0.0)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_takes_thirty_six_steps_and_alpha_is_linear() {
        let mut life = PageLife::new(PageTiming::MENU);
        assert!(life.advance(STEP_SECONDS, true));
        assert_eq!(life.phase, PagePhase::Opening);
        let mut steps = 0;
        while life.phase == PagePhase::Opening {
            life.advance(STEP_SECONDS, true);
            steps += 1;
            assert!(steps < 100);
        }
        // 0.6 s at 60 steps per second, within one step of float accumulation.
        assert!((35..=37).contains(&steps), "{steps}");
        assert_eq!(life.alpha(), 1.0);
    }

    #[test]
    fn close_fades_out_over_the_close_time() {
        let mut life = PageLife {
            phase: PagePhase::Open,
            timer: 0.0,
            timing: PageTiming::MENU,
        };
        life.close();
        for _ in 0..7 {
            life.advance(STEP_SECONDS, true);
        }
        assert!((life.alpha() - (1.0 - 7.0 / 60.0 / 0.25)).abs() < 1e-5);
        for _ in 0..30 {
            life.advance(STEP_SECONDS, true);
        }
        assert_eq!(life.alpha(), 0.0);
    }

    #[test]
    fn waiting_page_does_not_start_until_ready() {
        let mut life = PageLife::new(PageTiming::MENU);
        for _ in 0..10 {
            life.advance(STEP_SECONDS, false);
        }
        assert_eq!(life.phase, PagePhase::Waiting);
        assert_eq!(life.timer, 0.0);
    }

    #[test]
    fn row_fade_is_a_one_second_ramp() {
        let mut fade = RowFade::IDLE;
        for _ in 0..30 {
            fade.advance(STEP_SECONDS, true, false);
        }
        assert!((fade.highlight - 0.5).abs() < 1e-4);
        for _ in 0..40 {
            fade.advance(STEP_SECONDS, true, false);
        }
        assert_eq!(fade.highlight, 1.0);
        for _ in 0..70 {
            fade.advance(STEP_SECONDS, false, false);
        }
        assert_eq!(fade.highlight, 0.0);
    }

    #[test]
    fn timing_maps_to_linear_fades() {
        let (open, close) = PageTiming::MENU.transitions();
        assert_eq!((open.duration_ms, close.duration_ms), (600, 250));
        assert_eq!(open.ease, Ease::Linear);
    }

    #[test]
    fn lifecycle_holds_across_frame_sizes_and_preset_timings() {
        for timing in [PageTiming::MENU, PageTiming::PAUSE, PageTiming::POPUP] {
            for frames in 1..=64 {
                let dt = timing.open / frames as f32;
                let mut life = PageLife::new(timing);
                for _ in 0..frames {
                    assert!(!life.advance(dt, false));
                    assert_eq!(life.timer, 0.0);
                }
                assert!(life.advance(0.0, true));
                assert_eq!(life.phase, PagePhase::Opening);
                assert_eq!(life.alpha(), 0.0);
                for _ in 0..frames - 1 {
                    life.advance(dt, true);
                    assert!((0.0..=1.0).contains(&life.alpha()));
                }
                life.advance(timing.open, true);
                assert_eq!(life.phase, PagePhase::Open);
                assert_eq!(life.alpha(), 1.0);
                life.close();
                life.close();
                assert_eq!(life.phase, PagePhase::Closing);
                life.advance(timing.close * 2.0, true);
                assert_eq!(life.alpha(), 0.0);
                assert_eq!(life.timer, timing.close);
            }
        }
    }
}
