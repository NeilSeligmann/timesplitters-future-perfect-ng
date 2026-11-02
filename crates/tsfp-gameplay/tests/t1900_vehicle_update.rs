// SPDX-License-Identifier: GPL-3.0-or-later
//! T1900 slice B Tier 0 smoke tests for `0x1E2B80`: fake guest memory plus a recorded callee trace.
use std::collections::HashMap;
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use tsfp_gameplay::t1900_vehicle_update as v;

#[derive(Default)]
struct Mem(HashMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, x: u8) {
        self.0.insert(a, x);
    }
}
/// Records (target, first four stack args); answers with scripted EAX (a queue per target, else the default).
#[derive(Default)]
struct Trace {
    calls: Vec<(u32, [u32; 4])>,
    answers: HashMap<u32, u32>,
    rng: u64,
}
impl Callee<Mem> for Trace {
    fn call(&mut self, m: &mut Mem, target: u32, cpu: &mut Cpu) {
        let a = |i: u32| m.read32(cpu.esp + 4 + 4 * i);
        self.calls.push((target, [a(0), a(1), a(2), a(3)]));
        cpu.eax = if target == v::target::RNG {
            self.rng = self
                .rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.rng >> 32) as u32
        } else {
            *self.answers.get(&target).unwrap_or(&0)
        };
    }
}
const SCENE: u32 = 0x10_0000;
const PLAYER: u32 = 0x20_0000;
fn cpu() -> Cpu {
    Cpu {
        esp: 0x8000,
        ..Cpu::default()
    }
}
fn seed(m: &mut Mem) {
    let f = |m: &mut Mem, a: u32, x: f32| m.write32(a, x.to_bits());
    for (a, x) in [
        (0x4785d4, 0.02),
        (0x475ccc, 4294967296.0),
        (0x4785d0, 1.0 / 4294967296.0),
        (0x47d6dc, 0.98),
        (0x475cd4, 0.5),
        (0x475c78, 1.0),
        (0x47862c, 10.0),
        (0x4765f4, 1e-7),
        (0x47d5bc, 0.1),
        (0x475d24, 0.1),
        (0x475dc8, 0.999),
        (0x4785e0, 0.7),
        (0x475cdc, 0.3),
        (0x475d40, 0.75),
        (0x475cf8, 0.03),
        (0x476040, 0.9),
        (0x475cc8, 0.6),
        (0x47da88, -50.0),
        (0x475db4, 4.0),
        (0x475ce8, 0.05),
        (0x475cd0, 0.8),
        (0x475d04, 0.4),
    ] {
        f(m, a, x);
    }
    m.write32(0x49e120, 3.75f64.to_bits() as u32);
    m.write32(0x49e124, (3.75f64.to_bits() >> 32) as u32);
    m.write32(0x49e118, 50.0f64.to_bits() as u32);
    m.write32(0x49e11c, (50.0f64.to_bits() >> 32) as u32);
    for (i, t) in [0u32, 100, 25, 75, 0].iter().enumerate() {
        m.write32(0x4fea1c + 4 * i as u32, *t);
    }
    m.write32(v::global::SCENE_PTR, SCENE);
    m.write32(v::global::PLAYER_PTR, PLAYER);
    m.write32(v::global::STEP, (-1i32) as u32);
    m.write32(v::global::LEVEL, 60);
}
fn run(m: &mut Mem, t: &mut Trace) -> Cpu {
    let mut c = cpu();
    v::scene_per_frame_update(t, m, &mut c);
    c
}

#[test]
fn mode_zero_clears_object_flag_and_ramps_lights() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    seed(&mut m);
    m.write32(SCENE + v::scene::OBJECT, 0x5000);
    t.answers.insert(v::target::ZONE_MODE, 0);
    let c = run(&mut m, &mut t);
    assert_eq!(c.esp, 0x8000);
    assert_eq!(m.read32(SCENE + v::scene::MODE), 0);
    let targets: Vec<u32> = t.calls.iter().map(|c| c.0).collect();
    assert_eq!(targets[..2], [v::target::ZONE_MODE, v::target::APPROACH]);
    assert_eq!(t.calls[1].1[0], SCENE + v::scene::LEVEL_FACTOR);
    assert_eq!(
        targets
            .iter()
            .filter(|x| **x == v::target::LIGHT_RAMP)
            .count(),
        2
    );
    assert_eq!(*targets.last().unwrap(), v::target::OBJECT_FLAG_OFF);
    assert_eq!(t.calls.last().unwrap().1[0], 0x5000);
}

#[test]
fn mode_two_target_is_table_times_step() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    seed(&mut m);
    t.answers.insert(v::target::ZONE_MODE, 2);
    run(&mut m, &mut t);
    assert_eq!(f32::from_bits(t.calls[1].1[1]), 25.0 * 0.02);
}

#[test]
fn mode_four_runs_tracked_object_sweep() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    seed(&mut m);
    m.write32(SCENE + v::scene::OBJECT, 0x5000);
    t.answers.insert(v::target::ZONE_MODE, 4);
    run(&mut m, &mut t);
    let targets: Vec<u32> = t.calls.iter().map(|c| c.0).collect();
    for needed in [
        v::target::OBJECT_FLAG_ON,
        v::target::SWEEP,
        v::target::OBJECT_SET_VECTOR,
    ] {
        assert!(targets.contains(&needed), "{needed:#x}");
    }
    assert!(!targets.contains(&v::target::OBJECT_FLAG_OFF));
}

#[test]
fn stale_entity_handles_are_released_once_per_live_handle() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    seed(&mut m);
    m.write32(v::global::ENTITY_LIST_HEAD, 0x6000);
    m.write32(0x6000 + 0x70, 0x1234);
    t.answers.insert(v::target::ANIM_RECORD, 0x7000);
    m.write32(0x7004, (-80.0f32).to_bits());
    run(&mut m, &mut t);
    assert_eq!(
        t.calls
            .iter()
            .filter(|c| c.0 == v::target::RELEASE_HANDLE)
            .count(),
        1
    );
    assert_eq!(m.read32(0x6000 + 0x70), 0, "stale handle cleared");
}

#[test]
fn seeded_loop_keeps_stack_balanced_and_never_panics() {
    for seed_value in 0..3000u64 {
        let (mut m, mut t) = (Mem::default(), Trace::default());
        seed(&mut m);
        t.rng = seed_value;
        let mode = (seed_value % 5) as u32;
        t.answers.insert(v::target::ZONE_MODE, mode);
        m.write32(v::global::ENABLE_FLAG, 1);
        m.write32(SCENE + v::scene::OBJECT, 0x5000);
        m.write32(v::global::SPAWN_COUNTER, 5);
        for k in 0..5u32 {
            m.write32(SCENE + k * 0x2c + 0x6a8, (seed_value as u32 + k) % 3);
        }
        let c = run(&mut m, &mut t);
        assert_eq!(c.esp, 0x8000);
        assert!(t.calls.len() >= 4);
    }
}
