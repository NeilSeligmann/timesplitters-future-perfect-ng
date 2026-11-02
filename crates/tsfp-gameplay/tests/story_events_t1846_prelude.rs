//! T1846 batch 8: scene prelude `0x1E2B80` against emulated-original golden vectors (MEASURED), plus semantic unit tests.
mod common;
use common::parse;
use tsfp_gameplay::story_events::prelude_1e2b80::*;

const LOG: u32 = 0x1201000;
const LOGP: u32 = 0x1200F00;
const OUT_DIR: u32 = 0x1200E00;
const OUT_HIT: u32 = 0x1200E20;

fn world_of(case: &common::Case) -> SparseMemory {
    let mut world = SparseMemory::default();
    world.words = case.ins.iter().map(|(a, v)| (*a, *v)).collect();
    let f = |a: u32| f32::from_bits(case.input(a));
    world.sweep_result = Some(SweepResult {
        reach: [f(OUT_DIR), f(OUT_DIR + 4), f(OUT_DIR + 8)],
        hit: [f(OUT_HIT), f(OUT_HIT + 4), f(OUT_HIT + 8), f(OUT_HIT + 12)],
    });
    world
}

#[test]
fn prelude_matches_original() {
    let cases = parse(include_str!("data/t1846/prelude_1e2b80.golden"));
    assert!(cases.len() >= 40);
    let mut seen = std::collections::BTreeMap::<u32, usize>::new();
    let (mut ring, mut stale, mut obj_zero, mut sp_6f, mut sp_1a6) = (0, 0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut world = world_of(case);
        prelude_1e2b80(&mut world);
        let used = ((case.output(LOGP) - LOG) / 4) as usize;
        let expected: Vec<u32> = (0..used as u32).map(|i| case.output(LOG + 4 * i)).collect();
        let got: Vec<u32> = world.effects.iter().flat_map(|e| e.words()).collect();
        assert_eq!(got, expected, "case {n} effects");
        for effect in &world.effects {
            *seen.entry(effect.words()[0]).or_default() += 1;
            match effect {
                PreludeEffect::Release { entry: 0 } => stale += 1,
                PreludeEffect::SpawnObject { words } if words[0] == 0xC6F => sp_6f += 1,
                PreludeEffect::SpawnObject { words } if words[0] == 0x1A6 => sp_1a6 += 1,
                _ => {}
            }
        }
        if world.read(0x1100760) != case.input(0x1100760) {
            ring += 1;
        }
        if world.read(0x1130070) == 0 && case.input(0x1130070) != 0 {
            obj_zero += 1;
        }
        for (&address, &value) in &case.outs {
            if (LOG..LOG + 0x400).contains(&address) || address == LOGP {
                continue;
            }
            assert_eq!(world.read(address), value, "case {n} memory {address:#x}");
        }
    }
    assert!(
        ring > 0 && stale > 0 && obj_zero > 0 && sp_6f > 0 && sp_1a6 > 0,
        "branch coverage"
    );
    for callee in [
        0xEFBD0, 0x45820, 0x218A0, 0x21860, 0xD1EC0, 0xBD090, 0x468A0, 0x88C60, 0x10E2B0, 0xD1F10,
    ] {
        assert!(
            seen.get(&callee).copied().unwrap_or(0) > 0,
            "callee {callee:#x} never exercised"
        );
    }
}

#[test]
fn zone_modes_and_rng() {
    assert_eq!(zone_mode(0), 0);
    assert_eq!(zone_mode(1), 4);
    assert_eq!(zone_mode(0x42), 2);
    assert_eq!(zone_mode(0x1000), 0);
    let mut world = SparseMemory::default();
    let first = rng(&mut world);
    assert_eq!(first, 0x26_9EC3);
    assert_eq!(world.read(RNG_CALLS), 1);
}

#[test]
fn approach_steps_and_snaps() {
    let mut world = SparseMemory::default();
    world.write(0x100, 1.0f32.to_bits());
    assert!(!approach(&mut world, 0x100, 5.0, 0.5));
    assert_eq!(f32::from_bits(world.read(0x100)), 1.5);
    assert!(approach(&mut world, 0x100, 1.6, 0.5));
    assert_eq!(f32::from_bits(world.read(0x100)), 1.6);
}

#[test]
fn spawn_record_init_layout() {
    let words = init_spawn_record();
    assert_eq!(
        (words[2], words[4], words[11]),
        (0x3F80_0000, 0x14, 0x4248_0000)
    );
}
