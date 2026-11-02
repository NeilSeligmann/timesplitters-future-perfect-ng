//! T1870: the frame clock `0x1538C0` (the writer of `[0x4E7940]`, `[0x4E7944]`, `[0x4E7948]` and `[0x4E794C]`) and its
//! satellites, ported bit for bit.
//!
//! Source `build/default.xbe` (SHA-256 `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`), validated against the
//! original under the Unicorn oracle (`tools/t1846_gold/svc_clock.py`, test `tests/story_events_t1870_clock.rs`). Document:
//! `docs/t-story-event-pool-classes-t1870.md`. Behaviour MEASURED (xemu level), names INFERRED.
//!
//! | address | port |
//! |---|---|
//! | `0x1538C0()` + tail `0x150190` | [`FrameClock::frame`] |
//! | `0x153AC0(arg)` / `0x153AF0()` | [`FrameClock::set_run`] / [`FrameClock::run_flag`] |
//! | `0x153B00()` | [`FrameClock::sample_cycles`] |
//! | `0x5FE10` (wrapper around `0x5CEF0`) | [`FrameClock::frozen`] |
//!
//! The clock is a trait ([`ClockEnv`]): the tick counter `0x22030` (reads `[0x563918]`), the processor cycle counter and the two
//! hooks `0x455B0` / `0x455C0` are the only impure parts. What the story and effect code sees per frame: `steps = [0x4E7944]` and
//! `scale = [0x4E794C]` (the [`super::svc_effect_tick::TickGlobals`] pair).
//!
//! `frame` (per rendered frame): shifts the float histories, spins until the tick counter advanced at least one tick past `[0x7A58AC]`,
//! measures `n = now - previous`, clamps it to `1..=3` (`[0x4E7940]`, float copy `[0x4E7948]`), then by the run mode `[0x7497B4]`:
//! `0` runs (`steps = n`, `scale = n as f32`), `-1` re-initialises (`n = steps = 1`, scale `1.0`, mode `0`, resume hook), any other value
//! is paused (pause hook when `steps != 0`, then `steps = 0`, `scale = 0.0`). The game clock accumulator (`0x150190`) then adds `n` in
//! sub second units of `[0x4E8CEC]` (the frame rate) and carries seconds, minutes and hours.

use super::svc_effect_tick::TickGlobals;

/// The impure parts of the clock.
pub trait ClockEnv {
    /// `0x22030`: the tick counter `[0x563918]` (advanced by the timer service).
    fn tick_counter(&mut self) -> i32;
    /// `rdtsc`, low dword.
    fn cycles_low(&mut self) -> u32;
    /// `0x455B0`: pause hook (called when a frame arrives in the paused mode while `steps != 0`).
    fn pause_hook(&mut self, _clock: &mut FrameClock) {}
    /// `0x455C0`: resume hook (re-initialisation frame).
    fn resume_hook(&mut self, _clock: &mut FrameClock) {}
}

/// The float history ring: `[0x7A58E0..0x7A58EC]` and `[0x7A58C0..0x7A58CC]` (word index = (address - base) / 4).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClockHistory {
    /// `[0x7A58E0] [0x7A58E4] [0x7A58E8] [0x7A58EC]` then `[0x7A58F0]` (newest, the cycle counter seconds).
    pub e: [u32; 5],
    /// `[0x7A58C0] [0x7A58C4] [0x7A58C8] [0x7A58CC]` then `[0x7A58D0]` (the sampled cycle counter).
    pub c: [u32; 5],
}

/// The globals of the frame clock, named by address.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameClock {
    pub history: ClockHistory,
    /// `[0x7A58A0] [0x7A58A4] [0x7A58A8] [0x7A58AC] [0x7A58B0]`: the tick counter history (`[0x7A58AC]` is the previous frame's tick).
    pub ticks: [u32; 5],
    /// `[0x7497A8]` (the tick read) and `[0x7497AC]` (copy of `[0x7A58B0]`).
    pub now: u32,
    pub stamp: u32,
    /// `[0x4E7940]`: measured steps, clamped to `1..=3` (also the game clock increment).
    pub measured: i32,
    /// `[0x4E7944]` and `[0x4E794C]`, plus `[0x4E7948]` (float copy of `measured`).
    pub steps: i32,
    pub scale: f32,
    pub measured_f: f32,
    /// `[0x7497B0]` frame counter, `[0x7497B4]` run mode (`0` run, `-1` re-initialise, other paused), `[0x7497B8]` frame total,
    /// `[0x7497BC]` frames that took more than one step.
    pub frame_counter: u32,
    pub mode: i32,
    pub frames: u32,
    pub slow_frames: u32,
    /// `[0x4E8CEC]` frame rate (ticks per second).
    pub rate: i32,
    /// Game clock: `[0x74914C]` sub second, `[0x749148]` seconds, `[0x749158]` minutes, `[0x749160]` hours.
    pub sub_second: i32,
    pub seconds: i32,
    pub minutes: i32,
    pub hours: i32,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self {
            history: ClockHistory::default(),
            ticks: [0; 5],
            now: 0,
            stamp: 0,
            measured: 1,
            steps: 1,
            scale: 1.0,
            measured_f: 1.0,
            frame_counter: 0,
            mode: 0,
            frames: 0,
            slow_frames: 0,
            rate: 60,
            sub_second: 0,
            seconds: 0,
            minutes: 0,
            hours: 0,
        }
    }
}

/// Cycles per second of the nominal processor (`0x2BB5C755`, 733.33 MHz).
const CYCLES: i32 = 0x2BB5_C755;

impl FrameClock {
    /// The pair the effect and story code reads.
    pub fn tick_globals(&self) -> TickGlobals {
        TickGlobals {
            steps: self.steps,
            scale: self.scale,
        }
    }

    /// `0x153B00`: `[0x7A58D0] = (f32) (cycles / (CYCLES / rate))` (x87 double division, rounded once to f32).
    pub fn sample_cycles(&mut self, env: &mut dyn ClockEnv) {
        let cycles = f64::from(env.cycles_low());
        let per_tick = CYCLES
            .checked_div(self.rate)
            .expect("divide error: [0x4E8CEC] is zero");
        self.history.c[4] = ((cycles / f64::from(per_tick)) as f32).to_bits();
    }

    /// `0x153AC0(arg)`: `arg != 0` leaves the paused/idle mode `0` for `1`; `arg == 0` sends any non zero mode to `-1`.
    pub fn set_run(&mut self, arg: u32) {
        if self.mode == 0 && arg != 0 {
            self.mode = 1;
        }
        if self.mode == 0 && arg == 0 {
            return;
        }
        // reached with a non zero mode, or after the store above
        if arg == 0 {
            self.mode = -1;
        }
    }

    /// `0x153AF0()`: whether the mode is non zero.
    pub fn run_flag(&self) -> bool {
        self.mode != 0
    }

    /// `0x5FE10` around `0x5CEF0`: runs `body` with the step globals frozen (`0`, `0.0`) and restores them from the saved step count.
    pub fn frozen<R>(&mut self, body: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.steps;
        self.steps = 0;
        self.scale = 0.0;
        let result = body(self);
        self.steps = saved;
        self.scale = saved as f32;
        result
    }

    /// `0x1538C0` followed by its tail `0x150190`.
    pub fn frame(&mut self, env: &mut dyn ClockEnv) {
        let history = &mut self.history;
        history.e[0] = history.e[1];
        history.c[0] = history.c[1];
        history.e[1] = history.e[2];
        history.c[1] = history.c[2];
        history.e[2] = history.e[3];
        history.c[2] = history.c[3];
        history.e[3] = history.e[4];
        history.c[3] = history.c[4];
        let previous = self.ticks[4];
        let ticks = &mut self.ticks;
        ticks[0] = ticks[1];
        ticks[1] = ticks[2];
        ticks[2] = ticks[3];
        ticks[3] = ticks[4];
        // spin until the counter moved by at least one tick
        loop {
            self.now = env.tick_counter() as u32;
            self.measured = self.now.wrapping_sub(previous) as i32;
            if self.measured >= 1 {
                break;
            }
        }
        self.ticks[4] = self.now;
        let cycles = f64::from(env.cycles_low());
        self.stamp = self.ticks[4];
        let per_tick = CYCLES
            .checked_div(self.rate)
            .expect("divide error: [0x4E8CEC] is zero");
        self.history.e[4] = ((cycles / f64::from(per_tick)) as f32).to_bits();
        let mut steps = self.measured;
        if steps > 3 {
            steps = 3;
            self.measured = 3;
        } else if steps == 0 {
            steps = 1;
            self.measured = 1;
        }
        self.frames = self.frames.wrapping_add(1);
        self.measured_f = steps as f32;
        if steps > 1 {
            self.slow_frames = self.slow_frames.wrapping_add(1);
        }
        let mut counted = steps;
        match self.mode {
            -1 => {
                self.measured = 1;
                self.steps = 1;
                self.measured_f = 1.0;
                self.scale = 1.0;
                self.mode = 0;
                env.resume_hook(self);
                counted = self.measured;
            }
            0 => {
                self.steps = steps;
                self.scale = steps as f32;
            }
            _ => {
                if self.steps != 0 {
                    env.pause_hook(self);
                    counted = self.measured;
                }
                self.steps = 0;
                self.scale = 0.0;
            }
        }
        self.frame_counter = self.frame_counter.wrapping_add(counted as u32);
        self.advance_game_clock();
    }

    /// `0x150190`: the game clock accumulator.
    fn advance_game_clock(&mut self) {
        self.sub_second = self.sub_second.wrapping_add(self.measured);
        if self.sub_second < self.rate {
            return;
        }
        self.sub_second = self.sub_second.wrapping_sub(self.rate);
        self.seconds = self.seconds.wrapping_add(1);
        if self.seconds != 60 {
            return;
        }
        self.seconds = 0;
        self.minutes = self.minutes.wrapping_add(1);
        if self.minutes != 60 {
            return;
        }
        self.minutes = 0;
        self.hours = self.hours.wrapping_add(1);
        if self.hours == 24 {
            self.hours = 0;
        }
    }
}
