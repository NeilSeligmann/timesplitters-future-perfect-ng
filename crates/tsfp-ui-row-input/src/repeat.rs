// SPDX-License-Identifier: GPL-3.0-or-later
//! Moved from `tsfp-ui-input` (T3817, rule 37: that crate is at its soft limit); re-exported there as `repeat`.
//! Key/pad repeat and focus acceleration as data.

/// Repeat schedule. Values are PLACEHOLDERS (FABRICATED) until ported from the original
/// repeat timer (0x7A7B0, T2159 facts); the structure is what UI-6 keeps.
///
/// Steps are searched from the end, so keep them in ascending hold-time order.
/// A zero interval is treated as one millisecond to keep updates finite.
///
/// ```
/// use tsfp_ui_row_input::repeat::RepeatPolicy;
/// let policy = RepeatPolicy::default();
/// assert_eq!(policy.interval_at(999), 120);
/// assert_eq!(policy.interval_at(1000), 60);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatPolicy {
    /// Hold time before the first repeat.
    pub initial_delay_ms: u64,
    /// `(held_ms_at_least, interval_ms)` ascending; the last matching step wins. Acceleration.
    pub steps: Vec<(u64, u64)>,
    /// Upper bound of repeats produced by a single update (stalled frames).
    pub max_per_update: u32,
}

impl Default for RepeatPolicy {
    fn default() -> Self {
        Self {
            initial_delay_ms: 350,
            steps: vec![(0, 120), (1000, 60), (2500, 30)],
            max_per_update: 8,
        }
    }
}

impl RepeatPolicy {
    pub fn interval_at(&self, held_ms: u64) -> u64 {
        self.steps
            .iter()
            .rfind(|(from, _)| held_ms >= *from)
            .map_or(self.initial_delay_ms.max(1), |s| s.1.max(1))
    }
}

/// Per intent hold tracker.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hold {
    pub active: bool,
    pub since: u64,
    pub next: u64,
}

impl Hold {
    pub fn press(&mut self, now: u64, p: &RepeatPolicy) {
        let next = now.checked_add(p.initial_delay_ms);
        *self = Hold {
            active: next.is_some(),
            since: now,
            next: next.unwrap_or(u64::MAX),
        };
    }
    /// Number of repeat fires due at `now`.
    pub fn due(&mut self, now: u64, p: &RepeatPolicy) -> u32 {
        let mut n = 0;
        while self.active && now >= self.next && n < p.max_per_update {
            n += 1;
            let held = self.next - self.since;
            match self.next.checked_add(p.interval_at(held)) {
                Some(next) => self.next = next,
                None => self.active = false,
            }
        }
        if n == p.max_per_update {
            match now.checked_add(1) {
                Some(next) => self.next = self.next.max(next),
                None => self.active = false,
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::{Hold, RepeatPolicy};

    #[test]
    fn interval_steps_and_zero_intervals_are_bounded() {
        let policy = RepeatPolicy {
            initial_delay_ms: 0,
            steps: vec![(10, 7), (30, 0), (60, 3)],
            max_per_update: 8,
        };
        for held in 0..100 {
            let expected = if held < 10 {
                1
            } else if held < 30 {
                7
            } else if held < 60 {
                1
            } else {
                3
            };
            assert_eq!(policy.interval_at(held), expected, "held={held}");
        }
    }

    #[test]
    fn hold_due_is_capped_and_rearms_after_stall() {
        for delay in 1..=16 {
            for cap in 1..=8 {
                let policy = RepeatPolicy {
                    initial_delay_ms: delay,
                    steps: vec![(0, 1)],
                    max_per_update: cap,
                };
                let mut hold = Hold::default();
                hold.press(100, &policy);
                assert_eq!(hold.due(100 + delay - 1, &policy), 0);
                assert_eq!(hold.due(100 + delay + 100, &policy), cap);
                assert_eq!(hold.due(100 + delay + 100, &policy), 0);
                assert_eq!(hold.due(100 + delay + 101, &policy), 1);
            }
        }
    }

    #[test]
    fn hold_stops_when_next_timestamp_is_unrepresentable() {
        let policy = RepeatPolicy {
            initial_delay_ms: 2,
            steps: vec![(0, 1)],
            max_per_update: 8,
        };
        let mut hold = Hold::default();
        hold.press(u64::MAX - 1, &policy);
        assert_eq!(hold.due(u64::MAX, &policy), 0);

        hold.press(u64::MAX - 2, &policy);
        assert_eq!(hold.due(u64::MAX, &policy), 1);
        assert_eq!(hold.due(u64::MAX, &policy), 0);
    }
}
