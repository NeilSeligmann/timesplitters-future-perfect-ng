//! Narrow host seam for the T3409 startup display-clock deadline.
use crate::vblank_clock::{StartupVblankClock, VblankClockPoll};

static STARTUP_VBLANK_CLOCK: StartupVblankClock = StartupVblankClock::new();

pub fn reset_startup_vblank_clock() {
    STARTUP_VBLANK_CLOCK.reset();
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_startup_vblank_clock_reset() {
    reset_startup_vblank_clock();
}

/// Return 0 with a wait duration, 1 when due, 2 when already consumed, and 3
/// for invalid input. The host keeps time and callback execution outside Rust.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_startup_vblank_clock_poll(
    now_ns: u64,
    refresh_hz: u32,
    wait_ns: *mut u64,
) -> u32 {
    if wait_ns.is_null() {
        return 3;
    }
    // SAFETY: the C boundary requires a writable u64 output pointer.
    unsafe { wait_ns.write(0) };
    match STARTUP_VBLANK_CLOCK.poll(now_ns, refresh_hz) {
        VblankClockPoll::Wait(remaining) => {
            // SAFETY: checked non-null above and the caller owns the output slot.
            unsafe { wait_ns.write(remaining) };
            0
        }
        VblankClockPoll::Due => 1,
        VblankClockPoll::Consumed => 2,
        VblankClockPoll::InvalidRefresh => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_seam_reports_wait_then_due_then_consumed() {
        reset_startup_vblank_clock();
        let mut wait = u64::MAX;
        assert_eq!(tsfp_startup_vblank_clock_poll(0, 60, &mut wait), 0);
        assert_eq!(wait, 16_666_666);
        let due_at = wait;
        assert_eq!(tsfp_startup_vblank_clock_poll(due_at, 60, &mut wait), 1);
        assert_eq!(tsfp_startup_vblank_clock_poll(due_at, 60, &mut wait), 2);
    }
}
