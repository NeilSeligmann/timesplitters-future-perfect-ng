// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw relative mouse accumulation. Counts are summed in f64 as they arrive (one add per event, no allocation) and
//! drained once per tick, so slow frames lose nothing (docs/t-contract-input.md 7.1) and fractional counts keep their remainder.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LookAccumulator {
    x: f64,
    y: f64,
}

impl LookAccumulator {
    /// Add one device event. A subsequent [`Self::take`] drains the sum for the current step.
    ///
    /// ```
    /// use tsfp_input::accum::LookAccumulator;
    /// let mut look = LookAccumulator::default();
    /// look.add(0.25, -0.5);
    /// look.add(0.75, 0.5);
    /// assert_eq!(look.take(), (1.0, 0.0));
    /// assert_eq!(look.take(), (0.0, 0.0));
    /// ```
    pub fn add(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }
    pub fn peek(&self) -> (f64, f64) {
        (self.x, self.y)
    }
    /// Everything since the previous take, exactly.
    pub fn take(&mut self) -> (f64, f64) {
        let out = (self.x, self.y);
        self.x = 0.0;
        self.y = 0.0;
        out
    }
}

/// Degrees per raw count. Default 0.022 (the common source engine convention, docs/t-mouse-keyboard-design.md 8.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sensitivity {
    pub yaw_deg_per_count: f64,
    pub pitch_deg_per_count: f64,
    pub invert_pitch: bool,
}

impl Default for Sensitivity {
    fn default() -> Self {
        Self {
            yaw_deg_per_count: 0.022,
            pitch_deg_per_count: 0.022,
            invert_pitch: false,
        }
    }
}

impl Sensitivity {
    /// Counts (dx right positive, dy down positive as devices report) to (yaw_deg right positive, pitch_deg up positive).
    pub fn to_degrees(&self, counts: (f64, f64)) -> (f32, f32) {
        let yaw = counts.0 * self.yaw_deg_per_count;
        let mut pitch = -counts.1 * self.pitch_deg_per_count;
        if self.invert_pitch {
            pitch = -pitch;
        }
        (yaw as f32, pitch as f32)
    }
}

/// Wheel notches with a fractional remainder (pixel scroll units arrive as fractions).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelAccumulator {
    notches: f64,
}

impl WheelAccumulator {
    /// Add signed fractional notches. The whole part is consumed by [`Self::take_whole`].
    ///
    /// ```
    /// use tsfp_input::accum::WheelAccumulator;
    /// let mut wheel = WheelAccumulator::default();
    /// wheel.add(0.75);
    /// assert_eq!(wheel.take_whole(), 0);
    /// wheel.add(0.5);
    /// assert_eq!(wheel.take_whole(), 1);
    /// ```
    pub fn add(&mut self, notches: f64) {
        self.notches += notches;
    }
    /// Whole notches (up positive), the fraction stays.
    pub fn take_whole(&mut self) -> i64 {
        let whole = self.notches.trunc();
        self.notches -= whole;
        whole as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_drains_and_conserves_the_sum() {
        let mut acc = LookAccumulator::default();
        let mut sent = 0.0f64;
        let mut got = 0.0f64;
        for i in 0..10_000 {
            let dx = (i % 7) as f64 * 0.25 - 0.75;
            acc.add(dx, -dx);
            sent += dx;
            if i % 13 == 0 {
                got += acc.take().0;
            }
        }
        got += acc.take().0;
        assert!((sent - got).abs() < 1e-9);
        assert_eq!(acc.peek(), (0.0, 0.0));
    }

    #[test]
    fn sub_count_motion_is_not_lost() {
        let mut acc = LookAccumulator::default();
        for _ in 0..10 {
            acc.add(0.1, 0.0);
        }
        assert!((acc.take().0 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn degrees_signs() {
        let sens = Sensitivity::default();
        let (yaw, pitch) = sens.to_degrees((100.0, 100.0));
        assert!(
            yaw > 0.0 && pitch < 0.0,
            "right is turn right, mouse down is look down"
        );
        let inverted = Sensitivity {
            invert_pitch: true,
            ..sens
        }
        .to_degrees((0.0, 100.0));
        assert!(inverted.1 > 0.0);
        assert!((yaw - 2.2).abs() < 1e-5);
    }

    #[test]
    fn wheel_keeps_the_fraction() {
        let mut wheel = WheelAccumulator::default();
        wheel.add(0.4);
        assert_eq!(wheel.take_whole(), 0);
        wheel.add(0.7);
        assert_eq!(wheel.take_whole(), 1);
        wheel.add(-2.1);
        assert_eq!(wheel.take_whole(), -2);
    }

    #[test]
    fn look_events_are_conserved_across_step_boundaries() {
        for seed in 0..256u32 {
            let mut acc = LookAccumulator::default();
            let mut expected = (0.0, 0.0);
            for event in 0..32u32 {
                let dx = ((seed.wrapping_mul(17) + event * 13) % 41) as f64 / 8.0 - 2.5;
                let dy = ((seed.wrapping_mul(23) + event * 7) % 31) as f64 / 8.0 - 1.875;
                acc.add(dx, dy);
                expected.0 += dx;
                expected.1 += dy;
                if event % 5 == 4 {
                    assert_eq!(acc.take(), expected, "seed {seed}, event {event}");
                    expected = (0.0, 0.0);
                }
            }
            assert_eq!(acc.take(), expected, "seed {seed}");
            assert_eq!(acc.take(), (0.0, 0.0));
        }
    }

    #[test]
    fn wheel_drain_retains_signed_fraction() {
        for seed in 0..256i64 {
            let mut wheel = WheelAccumulator::default();
            let mut remainder_eighths = 0i64;
            for event in 0..32i64 {
                let eighths = (seed * 17 + event * 11) % 23 - 11;
                wheel.add(eighths as f64 / 8.0);
                remainder_eighths += eighths;
                let expected_whole = remainder_eighths / 8;
                assert_eq!(
                    wheel.take_whole(),
                    expected_whole,
                    "seed {seed}, event {event}"
                );
                remainder_eighths -= expected_whole * 8;
                assert_eq!(wheel.take_whole(), 0);
            }
        }
    }
}
