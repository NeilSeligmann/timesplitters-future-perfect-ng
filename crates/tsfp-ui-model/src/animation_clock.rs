//! Native presentation time (T2242). No guest writes or wall-clock reads.
//!
//! Hosts supply monotonic display timestamps and completed guest snapshots. Display mode
//! interpolates one guest interval behind; original mode returns the current snapshot exactly.
//! Missing/stalled frames hold at the current snapshot, never extrapolate game events.

use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationMode {
    #[default]
    Original,
    DisplayRate,
}

/// A presentation-only interval. `alpha` never leaves 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationSample {
    pub alpha: f32,
    /// Interpolated simulation time, independent of render count.
    pub seconds: f64,
}

#[derive(Clone, Copy, Debug)]
struct Stamp {
    display: Duration,
    seconds: f64,
}

/// Clock shared by native UI/HUD animation tracks. Epoch resets discard interpolation history.
#[derive(Clone, Debug, Default)]
pub struct AnimationClock {
    pub mode: AnimationMode,
    previous: Option<Stamp>,
    current: Option<Stamp>,
}

impl AnimationClock {
    /// A completed simulation frame. Reject invalid timestamps without corrupting history.
    /// A repeated/rewound timestamp starts a new epoch (pause, load or scene change).
    pub fn publish(&mut self, display: Duration, seconds: f64) -> bool {
        if !seconds.is_finite() || seconds < 0.0 {
            return false;
        }
        let stamp = Stamp { display, seconds };
        self.previous = self
            .current
            .filter(|old| display > old.display && seconds > old.seconds);
        self.current = Some(stamp);
        true
    }

    pub fn reset(&mut self) {
        self.previous = None;
        self.current = None;
    }

    /// Render reads are pure: sampling 240 times must not advance a timer more than 60 reads.
    pub fn sample(&self, display: Duration) -> AnimationSample {
        let Some(current) = self.current else {
            return AnimationSample {
                alpha: 1.0,
                seconds: 0.0,
            };
        };
        let Some(previous) = self
            .previous
            .filter(|_| self.mode == AnimationMode::DisplayRate)
        else {
            return AnimationSample {
                alpha: 1.0,
                seconds: current.seconds,
            };
        };
        let interval = current.display - previous.display;
        let alpha = (display.saturating_sub(current.display).as_secs_f64()
            / interval.as_secs_f64())
        .clamp(0.0, 1.0);
        AnimationSample {
            alpha: alpha as f32,
            seconds: previous.seconds + (current.seconds - previous.seconds) * alpha,
        }
    }
}

/// Rust-owned visual values. Identity/discrete state belongs to the host's track key;
/// use `snap` on focus changes, new messages, resets or page lifecycle changes.
/// This prevents interpolating unrelated rows/messages or fading across a scene switch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnimationValues {
    pub fade: f32,
    pub pulse: f32,
    pub slide: [f32; 2],
    pub message_age: f32,
    /// Degrees; interpolated along the shortest arc.
    pub radar_sweep: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AnimationTrack {
    previous: AnimationValues,
    current: AnimationValues,
}

impl AnimationTrack {
    pub fn snap(&mut self, values: AnimationValues) {
        self.previous = values;
        self.current = values;
    }

    /// Exactly once per completed guest frame, with values from native Rust handlers.
    pub fn publish(&mut self, values: AnimationValues) {
        self.previous = self.current;
        self.current = values;
    }

    pub fn sample(&self, sample: AnimationSample) -> AnimationValues {
        // Preserve exact f32 bits in the original preset and at interval endpoints.
        if sample.alpha >= 1.0 {
            return self.current;
        }
        if sample.alpha <= 0.0 {
            return self.previous;
        }
        let a = sample.alpha;
        let mix = |p: f32, c: f32| p + (c - p) * a;
        let angle = (self.current.radar_sweep - self.previous.radar_sweep + 180.0)
            .rem_euclid(360.0)
            - 180.0;
        AnimationValues {
            fade: mix(self.previous.fade, self.current.fade),
            pulse: mix(self.previous.pulse, self.current.pulse),
            slide: [
                mix(self.previous.slide[0], self.current.slide[0]),
                mix(self.previous.slide[1], self.current.slide[1]),
            ],
            message_age: mix(self.previous.message_age, self.current.message_age),
            radar_sweep: self.previous.radar_sweep + angle * a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }
    fn clock() -> AnimationClock {
        let mut c = AnimationClock::default();
        c.publish(ms(0), 0.0);
        c.publish(ms(20), 0.02);
        c
    }
    #[test]
    fn original_is_exact_at_every_display_timestamp() {
        let c = clock();
        let mut t = AnimationTrack::default();
        let v = AnimationValues {
            fade: 0.1234567,
            radar_sweep: 359.0,
            ..Default::default()
        };
        t.publish(v);
        for n in 0..100 {
            assert_eq!(t.sample(c.sample(ms(n))), v);
        }
    }
    #[test]
    fn display_clock_is_bounded_delayed_and_read_only() {
        let mut c = clock();
        c.mode = AnimationMode::DisplayRate;
        assert_eq!(c.sample(ms(10)).alpha, 0.0);
        assert_eq!(c.sample(ms(30)).alpha, 0.5);
        assert_eq!(c.sample(ms(30)).seconds, 0.01);
        for _ in 0..1000 {
            assert_eq!(c.sample(ms(30)).alpha, 0.5);
        }
        assert_eq!(c.sample(ms(5000)).seconds, 0.02);
    }
    #[test]
    fn invalid_and_reset_epochs_do_not_interpolate() {
        let mut c = clock();
        c.mode = AnimationMode::DisplayRate;
        assert!(!c.publish(ms(30), f64::NAN));
        assert!(!c.publish(ms(30), -1.0));
        assert_eq!(c.sample(ms(30)).alpha, 0.5);
        c.publish(ms(40), 0.02); // paused simulation
        assert_eq!(c.sample(ms(40)).alpha, 1.0);
        c.publish(ms(10), 0.0); // new epoch
        assert_eq!(c.sample(ms(10)).seconds, 0.0);
        c.reset();
        assert_eq!(c.sample(ms(500)).seconds, 0.0);
    }
    #[test]
    fn values_interpolate_and_radar_wraps_shortest_arc() {
        let mut t = AnimationTrack::default();
        t.snap(AnimationValues {
            radar_sweep: 359.0,
            ..Default::default()
        });
        t.publish(AnimationValues {
            fade: 1.0,
            pulse: 0.5,
            slide: [10.0, -4.0],
            message_age: 2.0,
            radar_sweep: 1.0,
        });
        let v = t.sample(AnimationSample {
            alpha: 0.5,
            seconds: 0.0,
        });
        assert_eq!(
            v,
            AnimationValues {
                fade: 0.5,
                pulse: 0.25,
                slide: [5.0, -2.0],
                message_age: 1.0,
                radar_sweep: 360.0
            }
        );
        t.snap(v);
        assert_eq!(
            t.sample(AnimationSample {
                alpha: 0.0,
                seconds: 0.0
            }),
            v
        );
    }
    #[test]
    fn native_linear_fade_is_independent_of_display_rate() {
        use crate::transition::{RowFade, STEP_SECONDS};
        let mut row = RowFade::IDLE;
        let mut clock = AnimationClock {
            mode: AnimationMode::DisplayRate,
            ..Default::default()
        };
        let mut track = AnimationTrack::default();
        clock.publish(Duration::ZERO, 0.0);
        for frame in 1..=30 {
            row.advance(STEP_SECONDS, true, false);
            let at = Duration::from_secs_f64(frame as f64 / 60.0);
            clock.publish(at, frame as f64 / 60.0);
            track.publish(AnimationValues {
                fade: row.highlight,
                ..Default::default()
            });
            for rate in [60.0, 120.0, 144.0, 240.0] {
                let sample = clock.sample(at + Duration::from_secs_f64(1.0 / rate));
                let got = track.sample(sample).fade;
                assert!((got - sample.seconds as f32).abs() < 1e-5);
            }
        }
    }
}
