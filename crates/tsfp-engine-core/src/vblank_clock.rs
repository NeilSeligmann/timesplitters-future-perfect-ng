//! One-shot default display-clock deadline used by the T3409 loading-worker seam.
//!
//! The host supplies monotonic time; this owner decides when the next display
//! period is due. It does not touch guest memory or invoke the title callback.
use core::sync::atomic::{AtomicU64, Ordering};

const UNARMED: u64 = 0;
const CONSUMED: u64 = 1;
const NS_PER_SECOND: u64 = 1_000_000_000;

/// Display timing classes accepted by the startup handoff.
///
/// The host ABI still supplies the measured class as Hz; convert it once at
/// the Rust boundary so the deadline calculation cannot silently accept an
/// arbitrary refresh rate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupVblankRate {
    Hz50,
    Hz60,
}

impl StartupVblankRate {
    pub const fn from_hz(refresh_hz: u32) -> Option<Self> {
        match refresh_hz {
            50 => Some(Self::Hz50),
            60 => Some(Self::Hz60),
            _ => None,
        }
    }

    const fn period_ns(self) -> u64 {
        NS_PER_SECOND
            / match self {
                Self::Hz50 => 50,
                Self::Hz60 => 60,
            }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VblankClockPoll {
    Wait(u64),
    Due,
    Consumed,
    InvalidRefresh,
}

/// Owns one display-period deadline. The caller supplies monotonic nanoseconds
/// at a safe guest call boundary, then performs the host wait and delivery.
#[derive(Debug, Default)]
pub struct StartupVblankClock {
    deadline_ns: AtomicU64,
}

impl StartupVblankClock {
    pub const fn new() -> Self {
        Self {
            deadline_ns: AtomicU64::new(UNARMED),
        }
    }

    pub fn reset(&self) {
        self.deadline_ns.store(UNARMED, Ordering::Release);
    }

    pub fn poll(&self, now_ns: u64, refresh_hz: u32) -> VblankClockPoll {
        let Some(refresh) = StartupVblankRate::from_hz(refresh_hz) else {
            return VblankClockPoll::InvalidRefresh;
        };
        let period_ns = refresh.period_ns();
        loop {
            let deadline = self.deadline_ns.load(Ordering::Acquire);
            if deadline == CONSUMED {
                return VblankClockPoll::Consumed;
            }
            if deadline == UNARMED {
                let next = now_ns.saturating_add(period_ns).max(2);
                if self
                    .deadline_ns
                    .compare_exchange(UNARMED, next, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return VblankClockPoll::Wait(next.saturating_sub(now_ns));
                }
                continue;
            }
            if now_ns < deadline {
                return VblankClockPoll::Wait(deadline - now_ns);
            }
            if self
                .deadline_ns
                .compare_exchange(deadline, CONSUMED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return VblankClockPoll::Due;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StartupVblankClock, StartupVblankRate, VblankClockPoll};

    // FABRICATED monotonic samples: these prove the typed deadline transition,
    // not original display timing or an Xbox vblank rate.
    #[test]
    fn supported_refresh_classes_have_explicit_display_periods() {
        assert_eq!(
            StartupVblankRate::from_hz(50),
            Some(StartupVblankRate::Hz50)
        );
        assert_eq!(
            StartupVblankRate::from_hz(60),
            Some(StartupVblankRate::Hz60)
        );
        assert_eq!(StartupVblankRate::from_hz(59), None);
        assert_eq!(StartupVblankRate::Hz50.period_ns(), 20_000_000);
        assert_eq!(StartupVblankRate::Hz60.period_ns(), 16_666_666);
    }

    #[test]
    fn startup_blank_waits_one_selected_display_period_and_is_one_shot() {
        for (hz, period) in [(50, 20_000_000), (60, 16_666_666)] {
            let clock = StartupVblankClock::new();
            assert_eq!(clock.poll(100, hz), VblankClockPoll::Wait(period));
            assert_eq!(clock.poll(100 + period - 1, hz), VblankClockPoll::Wait(1));
            assert_eq!(clock.poll(100 + period, hz), VblankClockPoll::Due);
            assert_eq!(clock.poll(100 + period * 3, hz), VblankClockPoll::Consumed);
            clock.reset();
            assert_eq!(clock.poll(7, hz), VblankClockPoll::Wait(period));
        }
    }

    #[test]
    fn startup_blank_rejects_unmeasured_refresh_classes() {
        let clock = StartupVblankClock::new();
        assert_eq!(clock.poll(0, 59), VblankClockPoll::InvalidRefresh);
        assert_eq!(clock.poll(0, 60), VblankClockPoll::Wait(16_666_666));
    }

    #[test]
    fn invalid_refresh_does_not_change_an_armed_saturating_deadline() {
        let clock = StartupVblankClock::new();
        let near_limit = u64::MAX - 3;

        assert_eq!(clock.poll(near_limit, 60), VblankClockPoll::Wait(3));
        assert_eq!(clock.poll(u64::MAX, 59), VblankClockPoll::InvalidRefresh);
        assert_eq!(clock.poll(u64::MAX - 1, 60), VblankClockPoll::Wait(1));
        assert_eq!(clock.poll(u64::MAX, 60), VblankClockPoll::Due);
        assert_eq!(clock.poll(u64::MAX, 60), VblankClockPoll::Consumed);
    }

    #[test]
    fn due_startup_blank_is_claimed_once_across_concurrent_pollers() {
        // FABRICATED timestamps; this checks atomic ownership of the one-shot
        // transition, not display timing or the original title's behavior.
        const POLLERS: usize = 8;
        let clock = StartupVblankClock::new();
        let due_at = 100 + 16_666_666;
        assert_eq!(clock.poll(100, 60), VblankClockPoll::Wait(16_666_666));

        let results = std::thread::scope(|scope| {
            let start = std::sync::Arc::new(std::sync::Barrier::new(POLLERS));
            let mut workers = Vec::with_capacity(POLLERS);
            for _ in 0..POLLERS {
                let start = std::sync::Arc::clone(&start);
                let clock = &clock;
                workers.push(scope.spawn(move || {
                    start.wait();
                    clock.poll(due_at, 60)
                }));
            }
            workers
                .into_iter()
                .map(|worker| worker.join().expect("clock poller panicked"))
                .collect::<Vec<_>>()
        });

        assert_eq!(
            results
                .iter()
                .filter(|result| **result == VblankClockPoll::Due)
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == VblankClockPoll::Consumed)
                .count(),
            POLLERS - 1
        );
    }
}
