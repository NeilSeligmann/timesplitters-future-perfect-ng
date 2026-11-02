// SPDX-License-Identifier: GPL-3.0-or-later
//! T2282 host-boundary policy. Injected monotonic nanoseconds; no guest memory or std.
//! Deterministic simulation time follows fixed60 wall deadlines; replay events consume no time.
/// Exact 60 Hz absolute schedule, with fractional nanoseconds carried rather than rounded per tick.
pub fn deadline(epoch_ns: u64, tick: u64) -> u64 {
    epoch_ns
        .saturating_add((u128::from(tick) * 1_000_000_000 / 60).min(u128::from(u64::MAX)) as u64)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_simulation_deadline(epoch_ns: u64, tick: u64) -> u64 {
    deadline(epoch_ns, tick)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sixty_ticks_are_exactly_one_wall_second() {
        for seconds in 1..10000 {
            assert_eq!(deadline(27, seconds * 60), 27 + seconds * 1_000_000_000);
        }
        assert_eq!(deadline(0, 1), 16_666_666);
        assert_eq!(deadline(0, 2), 33_333_333);
        assert_eq!(deadline(0, 3), 50_000_000);
        assert_eq!(deadline(u64::MAX, u64::MAX), u64::MAX);
    }
}

/// Render-only presents may deliver device callbacks, but do not represent
/// simulation time. Keep this classification independent of presenter load.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct FrameClock {
    replay: u64,
}
impl FrameClock {
    pub fn set_replay(&mut self, replay: bool) {
        self.replay = u64::from(replay);
    }
    pub fn advances_time(&self) -> bool {
        self.replay == 0
    }
}
/// # Safety
/// `state` is valid and exclusively borrowed under the clock mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_frame_clock_replay(state: *mut FrameClock, replay: u64) {
    unsafe {
        (*state).set_replay(replay != 0);
    }
}
/// # Safety
/// `state` is valid and exclusively borrowed under the clock mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_frame_clock_advances(state: *const FrameClock) -> bool {
    unsafe { (*state).advances_time() }
}
#[cfg(test)]
mod frame_tests {
    use super::*;
    #[test]
    fn replay_count_and_load_do_not_change_simulation_clock() {
        let mut state = FrameClock::default();
        let mut ticks = 0;
        for _ in 0..6000 {
            ticks += u64::from(state.advances_time());
            state.set_replay(true);
            for _ in 0..7 {
                ticks += u64::from(state.advances_time());
            }
            state.set_replay(false);
        }
        assert_eq!(ticks, 6000);
    }
}

/// T2207: simulation tick time versus render time. The simulation owns one absolute
/// 60 Hz schedule ([`deadline`]); render code only samples it and never advances it.
/// Policy moved from `render_rate.c` (T2282): re-base after a stall of more than three
/// periods (50 ms), matching the engine's 1..3 step clamp at 0x1539EE, lost time is not
/// caught up. Contract: docs/t-contract-kernel-clock.md, "Simulation and render time".
pub const REBASE_SLACK_NS: u64 = 50_000_000;

/// Per-simulation-thread handoff for the fixed-tick deadline. The tick begins
/// before frame work, but its wait belongs after the original timing/vblank
/// update in the outer loop. This state makes that boundary explicit at the C
/// host edge without mixing simulation time into guest memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct SimulationFrameBoundary {
    pending_timing_completion: u32,
}

impl SimulationFrameBoundary {
    pub fn begin_frame(&mut self) {
        self.pending_timing_completion = 1;
    }

    /// Consume one pending frame only after the original vblank/timing update.
    pub fn complete_timing(&mut self) -> bool {
        core::mem::replace(&mut self.pending_timing_completion, 0) != 0
    }
}

/// # Safety
/// `state` points to an exclusively borrowed [`SimulationFrameBoundary`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_simulation_frame_begin(state: *mut SimulationFrameBoundary) {
    unsafe { (*state).begin_frame() };
}

/// # Safety
/// `state` points to an exclusively borrowed [`SimulationFrameBoundary`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_simulation_frame_timing_complete(
    state: *mut SimulationFrameBoundary,
) -> bool {
    unsafe { (*state).complete_timing() }
}

#[cfg(test)]
mod frame_boundary_tests {
    use super::*;

    #[test]
    fn fixed_tick_wait_remains_pending_through_frame_work_and_completes_once_after_timing() {
        let mut boundary = SimulationFrameBoundary::default();
        assert!(!boundary.complete_timing());

        boundary.begin_frame();
        // Frame work, including its game timers, does not consume the deadline.
        assert_eq!(boundary.pending_timing_completion, 1);
        assert!(boundary.complete_timing());
        assert!(!boundary.complete_timing());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct SimulationClock {
    /// Monotonic host nanoseconds of tick 0 of the current schedule; 0 = not started.
    pub epoch_ns: u64,
    /// Ticks begun since the last re-base.
    pub tick: u64,
    /// Deadline of the tick in flight.
    pub until_ns: u64,
    /// Ticks begun since start, never reset.
    pub total: u64,
    pub rebases: u64,
}

/// What a render frame may know about simulation time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct RenderSample {
    /// Newest completed simulation tick (state N). Render interpolates N-1 to N.
    pub sim_tick: u64,
    /// Fraction of the current tick period elapsed at `now`, in 0..=1.
    pub alpha: f32,
}

impl SimulationClock {
    /// Begin the next simulation tick at host time `now_ns`; returns its absolute deadline.
    pub fn begin_tick(&mut self, now_ns: u64) -> u64 {
        if self.epoch_ns == 0 || now_ns > self.until_ns.saturating_add(REBASE_SLACK_NS) {
            if self.epoch_ns != 0 {
                self.rebases += 1;
            }
            self.epoch_ns = now_ns.max(1);
            self.tick = 0;
        }
        self.total += 1;
        self.tick += 1;
        self.until_ns = deadline(self.epoch_ns, self.tick);
        self.until_ns
    }

    /// Begin a deterministic fixed-rate tick without tying progress to host wall time.
    /// The first tick uses the supplied host epoch; later ticks use the prior logical
    /// deadline, so scheduler delays and rendering cost cannot change game time.
    pub fn begin_tick_fast_forward(&mut self, initial_epoch_ns: u64) -> u64 {
        let logical_now = if self.epoch_ns == 0 {
            initial_epoch_ns
        } else {
            self.until_ns
        };
        self.begin_tick(logical_now)
    }
    /// Render-side query: never changes simulation state.
    pub fn sample(&self, now_ns: u64) -> RenderSample {
        if self.epoch_ns == 0 {
            return RenderSample::default();
        }
        let start = deadline(self.epoch_ns, self.tick - 1);
        let period = self.until_ns.saturating_sub(start).max(1);
        let elapsed = now_ns.saturating_sub(start).min(period);
        RenderSample {
            sim_tick: self.total,
            alpha: elapsed as f32 / period as f32,
        }
    }
}

/// # Safety
/// `state` is valid and exclusively borrowed by the simulation thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_simulation_clock_begin(
    state: *mut SimulationClock,
    now_ns: u64,
) -> u64 {
    unsafe { (*state).begin_tick(now_ns) }
}

/// # Safety
/// `state` is valid and exclusively borrowed by the simulation thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_simulation_clock_begin_fast_forward(
    state: *mut SimulationClock,
    initial_epoch_ns: u64,
) -> u64 {
    unsafe { (*state).begin_tick_fast_forward(initial_epoch_ns) }
}
/// # Safety
/// `state` is valid; readers on other threads must hold the owner's lock or copy it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_simulation_clock_sample(
    state: *const SimulationClock,
    now_ns: u64,
) -> RenderSample {
    unsafe { (*state).sample(now_ns) }
}

#[cfg(test)]
mod simulation_tests {
    use super::*;
    const P: u64 = 16_666_666;
    #[test]
    fn steady_ticks_follow_the_absolute_schedule() {
        let mut c = SimulationClock::default();
        assert_eq!(c.begin_tick(1000), 1000 + P);
        for tick in 2..=600u64 {
            let now = deadline(1000, tick - 1);
            assert_eq!(c.begin_tick(now), deadline(1000, tick));
        }
        assert_eq!(c.until_ns, 1000 + 10_000_000_000);
        assert_eq!((c.total, c.tick, c.rebases), (600, 600, 0));
    }
    #[test]
    fn late_within_slack_catches_up_beyond_rebases() {
        let mut c = SimulationClock::default();
        c.begin_tick(1000);
        // 50 ms late is still within slack: schedule kept.
        assert_eq!(
            c.begin_tick(c.until_ns + REBASE_SLACK_NS),
            deadline(1000, 2)
        );
        let late = c.until_ns + REBASE_SLACK_NS + 1;
        assert_eq!(c.begin_tick(late), late + P);
        assert_eq!((c.tick, c.total, c.rebases, c.epoch_ns), (1, 3, 1, late));
    }
    #[test]
    fn epoch_zero_host_time_still_starts() {
        let mut c = SimulationClock::default();
        c.begin_tick(0);
        assert_eq!(c.epoch_ns, 1);
        c.begin_tick(5);
        assert_eq!(c.rebases, 0);
    }
    #[test]
    fn render_sample_is_read_only_and_bounded() {
        let mut c = SimulationClock::default();
        assert_eq!(c.sample(99), RenderSample::default());
        c.begin_tick(1000);
        c.begin_tick(1000 + P);
        let before = c;
        assert_eq!(c.sample(0).alpha, 0.0);
        assert_eq!(c.sample(1000 + P).alpha, 0.0);
        let half = c.sample(1000 + P + P / 2);
        assert!((half.alpha - 0.5).abs() < 1e-6, "{half:?}");
        assert_eq!(c.sample(u64::MAX).alpha, 1.0);
        assert_eq!(half.sim_tick, 2);
        assert_eq!(c, before);
    }

    #[test]
    fn fast_forward_ticks_advance_on_logical_time_not_wall_time() {
        let mut c = SimulationClock::default();
        assert_eq!(c.begin_tick_fast_forward(1000), deadline(1000, 1));
        for tick in 2..=600u64 {
            // A wildly changing wall clock is deliberately irrelevant after the epoch.
            assert_eq!(
                c.begin_tick_fast_forward(u64::MAX - tick),
                deadline(1000, tick)
            );
        }
        assert_eq!((c.total, c.tick, c.rebases), (600, 600, 0));
        assert_eq!(c.epoch_ns, 1000);
    }
}
