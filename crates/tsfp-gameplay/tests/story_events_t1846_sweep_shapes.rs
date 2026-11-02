//! T1846 batch 8: sweep shape tests 0x4A670, 0x4FE00, 0x52550 against emulated-original golden vectors (MEASURED).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::sweep_shapes::*;
use tsfp_gameplay::story_events::world_queries::SweepItem;

const START: u32 = 0x140_0000;
const A: u32 = 0x140_0100;
const B: u32 = 0x140_0200;
const M: u32 = 0x140_0300;
const POS: u32 = 0x140_0400;
const NRM: u32 = 0x140_0500;
const ID: u32 = 0x140_0600;
const ARGS: u32 = 0x140_0700;
const SENTINEL: [u32; 3] = [0x5E57_0001, 0x5E57_0002, 0x5E57_0003];

fn matrix(case: &Case) -> Matrix {
    let mut m = [0.0f32; 16];
    for (i, v) in m.iter_mut().enumerate() {
        *v = case.input_f(M + 4 * i as u32);
    }
    m
}

/// Compare the model output with the oracle's: a hit must match all seven words, a miss must leave the sentinels.
fn check(case: &Case, hit: bool, out: &ShapeOut, n: usize) {
    assert_eq!(case.eax, hit as u32, "case {n} code");
    for i in 0..3 {
        let (p, q) = (
            case.output(POS + 4 * i as u32),
            case.output(NRM + 4 * i as u32),
        );
        if hit {
            assert_eq!(out.position[i].to_bits(), p, "case {n} position[{i}]");
            assert_eq!(out.normal[i].to_bits(), q, "case {n} normal[{i}]");
        } else {
            assert_eq!((p, q), (SENTINEL[i], SENTINEL[i]), "case {n} sentinel");
        }
    }
    if hit {
        assert_eq!(out.id.to_bits(), case.output(ID), "case {n} id");
    } else {
        assert_eq!(case.output(ID), 0x5E57_0004);
    }
}

#[test]
fn sphere_4a670_matches_original() {
    let cases = parse(include_str!("data/t1846/sweep_sphere_4a670.golden"));
    assert!(cases.len() >= 50);
    let mut exits = std::collections::BTreeMap::new();
    for (n, case) in cases.iter().enumerate() {
        let (radius, r1) = (case.input_f(ARGS), case.input_f(ARGS + 4));
        // Garbage in the outputs must not matter: start from the sentinel-equivalent state.
        let mut out = ShapeOut::default();
        let exit = sphere_vs_sphere(case.vec3(A), r1, case.vec3(START), radius, &mut out);
        *exits.entry(format!("{exit:?}")).or_insert(0) += 1;
        check(case, exit == SphereExit::Hit, &out, n);
    }
    // NoDepth is only reachable through rounding at s ~ 0/1 (never seen), so it is not required.
    assert!(exits.get("Hit").copied().unwrap_or(0) >= 10, "{exits:?}");
    assert!(exits.get("Far").copied().unwrap_or(0) >= 10, "{exits:?}");
    for key in ["BelowZero", "AboveOne"] {
        assert!(exits.get(key).copied().unwrap_or(0) >= 1, "{key} {exits:?}");
    }
}

#[test]
fn box_4fe00_matches_original() {
    let cases = parse(include_str!("data/t1846/sweep_box_4fe00.golden"));
    assert!(cases.len() >= 50);
    let mut exits = std::collections::BTreeMap::new();
    for (n, case) in cases.iter().enumerate() {
        let mut out = ShapeOut::default();
        let exit = sphere_vs_box(
            case.vec3(START),
            case.input_f(ARGS),
            case.vec3(A),
            case.vec3(B),
            &matrix(case),
            &mut out,
        );
        *exits.entry(format!("{exit:?}")).or_insert(0) += 1;
        check(case, exit != BoxExit::Miss, &out, n);
    }
    for key in ["Miss", "Outside", "Inside"] {
        assert!(
            exits.get(key).copied().unwrap_or(0) >= 10,
            "{key} {exits:?}"
        );
    }
}

#[test]
fn capsule_52550_matches_original() {
    let cases = parse(include_str!("data/t1846/sweep_capsule_52550.golden"));
    assert!(cases.len() >= 50);
    let mut exits = std::collections::BTreeMap::new();
    for (n, case) in cases.iter().enumerate() {
        let mut out = ShapeOut::default();
        let exit = sphere_vs_capsule(
            case.vec3(START),
            case.input_f(ARGS),
            case.vec3(A),
            case.vec3(B),
            &matrix(case),
            &mut out,
        );
        *exits.entry(format!("{exit:?}")).or_insert(0) += 1;
        check(case, exit != CapsuleExit::Miss, &out, n);
    }
    for key in ["Miss", "HitOnAxis", "HitNormalised"] {
        assert!(exits.get(key).copied().unwrap_or(0) >= 3, "{key} {exits:?}");
    }
}

#[test]
fn dispatcher_uses_identity_and_persists_outputs() {
    let item = SweepItem {
        kind: 0,
        a: [0.0; 3],
        b: [1.0, 0.0, 0.0],
    };
    let mut tester = ShapeTester::default();
    let hit = tester.shape_test(&item, [1.0, 0.0, 0.0], 0.5);
    assert_eq!(hit.code, 1);
    let miss = tester.shape_test(&item, [9.0, 0.0, 0.0], 0.5);
    assert_eq!(miss.code, 0);
    assert_eq!(miss.normal, hit.normal);
    let unknown = SweepItem { kind: 7, ..item };
    assert_eq!(tester.shape_test(&unknown, [1.0, 0.0, 0.0], 0.5).code, 0);
}
