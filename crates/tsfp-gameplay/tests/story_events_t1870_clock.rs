//! T1870: the frame clock `0x1538C0` (+ `0x150190`), the mode setter `0x153AC0`/`0x153AF0` and `0x153B00` against
//! emulated-original golden vectors (MEASURED). Golden: `tests/data/t1870/{frame_clock,clock_setter,clock_sample}.golden`
//! (`tools/t1846_gold/svc_clock.py`). The cycle counter of the originals is patched to a known value (see the generator).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::frame_clock::*;

struct Env {
    tick: i32,
    cycles: u32,
    log: Vec<u32>,
}

impl ClockEnv for Env {
    fn tick_counter(&mut self) -> i32 {
        self.tick
    }
    fn cycles_low(&mut self) -> u32 {
        self.cycles
    }
    fn pause_hook(&mut self, _clock: &mut FrameClock) {
        self.log.push(0x455B0);
    }
    fn resume_hook(&mut self, _clock: &mut FrameClock) {
        self.log.push(0x455C0);
    }
}

fn load(case: &Case) -> FrameClock {
    let word = |address: u32| case.input(address);
    let mut clock = FrameClock::default();
    for i in 0..5 {
        clock.history.e[i] = word(0x7A58E0 + 4 * i as u32 + if i == 4 { 0 } else { 0 });
        clock.history.c[i] = word(0x7A58C0 + 4 * i as u32);
        clock.ticks[i] = word(0x7A58A0 + 4 * i as u32);
    }
    clock.now = word(0x7497A8);
    clock.stamp = word(0x7497AC);
    clock.measured = word(0x4E7940) as i32;
    clock.steps = word(0x4E7944) as i32;
    clock.measured_f = case.input_f(0x4E7948);
    clock.scale = case.input_f(0x4E794C);
    clock.frame_counter = word(0x7497B0);
    clock.mode = word(0x7497B4) as i32;
    clock.frames = word(0x7497B8);
    clock.slow_frames = word(0x7497BC);
    clock.rate = word(0x4E8CEC) as i32;
    clock.sub_second = word(0x74914C) as i32;
    clock.seconds = word(0x749148) as i32;
    clock.minutes = word(0x749158) as i32;
    clock.hours = word(0x749160) as i32;
    clock
}

fn fields(clock: &FrameClock) -> Vec<(u32, u32)> {
    let mut out = vec![
        (0x7497A8, clock.now),
        (0x7497AC, clock.stamp),
        (0x4E7940, clock.measured as u32),
        (0x4E7944, clock.steps as u32),
        (0x4E7948, clock.measured_f.to_bits()),
        (0x4E794C, clock.scale.to_bits()),
        (0x7497B0, clock.frame_counter),
        (0x7497B4, clock.mode as u32),
        (0x7497B8, clock.frames),
        (0x7497BC, clock.slow_frames),
        (0x74914C, clock.sub_second as u32),
        (0x749148, clock.seconds as u32),
        (0x749158, clock.minutes as u32),
        (0x749160, clock.hours as u32),
    ];
    for i in 0..5 {
        out.push((0x7A58E0 + 4 * i as u32, clock.history.e[i]));
        out.push((0x7A58C0 + 4 * i as u32, clock.history.c[i]));
        out.push((0x7A58A0 + 4 * i as u32, clock.ticks[i]));
    }
    out
}

#[test]
fn frame_clock_matches_the_original() {
    let cases = parse(include_str!("data/t1870/frame_clock.golden"));
    assert!(cases.len() >= 80);
    let (mut paused, mut reinit, mut clamped, mut carried) = (0, 0, 0, 0);
    for (index, case) in cases.iter().enumerate() {
        let mut clock = load(case);
        let now = case.input(0x563918);
        let imm = case.input(0xF000_0000);
        let mut env = Env {
            tick: now as i32,
            cycles: (now & !0xFF) | imm,
            log: vec![],
        };
        let mode = clock.mode;
        let steps_before = clock.steps;
        let seconds_before = clock.seconds;
        clock.frame(&mut env);
        for (address, value) in fields(&clock) {
            // 0x7A58A0 + 16 is the newest tick slot, the other golden cells are the same names
            if case.outs.contains_key(&address) {
                assert_eq!(value, case.output(address), "case {index} {address:#x}");
            }
        }
        let expected: Vec<u32> = case.calls.iter().map(|c| c.callee).collect();
        assert_eq!(env.log, expected, "case {index} hooks");
        paused += i32::from(mode > 0 && steps_before != 0);
        reinit += i32::from(mode == -1);
        clamped += i32::from(
            clock.measured == 3 && case.input(0x563918).wrapping_sub(case.input(0x7A58AC)) > 3,
        );
        carried += i32::from(clock.seconds != seconds_before);
    }
    assert!(
        paused > 0 && reinit > 0 && clamped > 0 && carried > 0,
        "{paused} {reinit} {clamped} {carried}"
    );
}

#[test]
fn mode_setter_matches_the_original() {
    let cases = parse(include_str!("data/t1870/clock_setter.golden"));
    assert!(cases.len() >= 80);
    let mut changed = 0;
    for (index, case) in cases.iter().enumerate() {
        let (va, arg) = (case.input(0xF000_0004), case.input(0xF000_0008));
        let mut clock = FrameClock {
            mode: case.input(0x7497B4) as i32,
            ..FrameClock::default()
        };
        let before = clock.mode;
        if va == 0x153AC0 {
            clock.set_run(arg);
            assert_eq!(clock.mode as u32, case.output(0x7497B4), "case {index}");
        } else {
            assert_eq!(u32::from(clock.run_flag()), case.eax, "case {index}");
        }
        changed += i32::from(clock.mode != before);
    }
    assert!(changed > 0);
}

#[test]
fn cycle_sample_matches_the_original() {
    let cases = parse(include_str!("data/t1870/clock_sample.golden"));
    assert!(cases.len() >= 80);
    for (index, case) in cases.iter().enumerate() {
        let mut clock = FrameClock {
            rate: case.input(0x4E8CEC) as i32,
            ..FrameClock::default()
        };
        let mut env = Env {
            tick: 0,
            cycles: case.input(0xF000_0000),
            log: vec![],
        };
        clock.sample_cycles(&mut env);
        assert_eq!(clock.history.c[4], case.output(0x7A58D0), "case {index}");
    }
}

#[test]
fn frozen_wrapper_restores_the_steps() {
    let mut clock = FrameClock {
        steps: 3,
        scale: 3.0,
        ..FrameClock::default()
    };
    let seen = clock.frozen(|inner| (inner.steps, inner.scale));
    assert_eq!(seen, (0, 0.0));
    assert_eq!((clock.steps, clock.scale), (3, 3.0));
    assert_eq!(clock.tick_globals().steps, 3);
}
