//! T1855: helpers of the kind 8 shape scan 0xD9760 against emulated-original golden vectors (MEASURED).
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::shape_parts::*;
use tsfp_gameplay::story_events::sweep_records::GuestMemory;
use tsfp_gameplay::story_events::world_queries::{WorldMatrixObject, build_world_matrix};

const BASE: u32 = 0x140_0000;
const SENTINEL: [u32; 3] = [0x5E57_0001, 0x5E57_0002, 0x5E57_0003];

/// Guest memory made of the case's `in` words; unmapped bytes read as zero.
struct CaseMemory<'a>(&'a Case);

impl GuestMemory for CaseMemory<'_> {
    fn read_u8(&self, address: u32) -> u8 {
        let word = self.0.input(address & !3);
        (word >> (8 * (address & 3))) as u8
    }
}

fn bits(case: &Case, address: u32, count: u32) -> Vec<u32> {
    (0..count).map(|i| case.output(address + 4 * i)).collect()
}

fn f3(v: [f32; 3]) -> Vec<u32> {
    v.iter().map(|f| f.to_bits()).collect()
}

#[test]
fn ray_vs_box_475c0_matches_original() {
    let cases = parse(include_str!("data/t1846/shape_parts_475c0.golden"));
    assert!(cases.len() >= 100);
    let mut exits = BTreeMap::new();
    for (n, case) in cases.iter().enumerate() {
        let a = |i: u32| case.vec3(BASE + 0x100 * i);
        let exit = ray_vs_box(a(0), a(1), a(2), a(3));
        *exits.entry(exit).or_insert(0) += 1;
        assert_eq!(case.eax, exit.hit() as u32, "case {n} {exit:?}");
    }
    for exit in [
        RayBoxExit::MissX,
        RayBoxExit::MissY,
        RayBoxExit::MissXyGreater,
        RayBoxExit::MissXyLess,
        RayBoxExit::MissZ,
        RayBoxExit::MissZCombinedHigh,
        RayBoxExit::MissZCombinedLow,
        RayBoxExit::Hit,
    ] {
        assert!(
            exits.get(&exit).copied().unwrap_or(0) >= 3,
            "{exit:?} {exits:?}"
        );
    }
}

#[test]
fn ray_vs_sphere_49200_matches_original() {
    let cases = parse(include_str!("data/t1846/shape_parts_49200.golden"));
    assert!(cases.len() >= 100);
    let (mut hits, mut misses, mut inside, mut stores) = (0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let a = |i: u32| BASE + 0x100 * i;
        let args = a(6);
        let (want_point, want_offset) = (case.input(args + 4) != 0, case.input(args + 8) != 0);
        let hit = ray_vs_sphere(
            case.vec3(a(0)),
            case.vec3(a(1)),
            case.vec3(a(2)),
            case.input_f(args),
        );
        assert_eq!(case.eax, hit.is_some() as u32, "case {n}");
        let (point, offset) = match &hit {
            Some(h) => (f3(h.point), f3(h.offset)),
            None => (SENTINEL.to_vec(), SENTINEL.to_vec()),
        };
        let expect = |wanted: bool, value: Vec<u32>| {
            if wanted && hit.is_some() {
                value
            } else {
                SENTINEL.to_vec()
            }
        };
        assert_eq!(
            bits(case, a(4), 3),
            expect(want_point, point),
            "case {n} point"
        );
        assert_eq!(
            bits(case, a(5), 3),
            expect(want_offset, offset),
            "case {n} offset"
        );
        stores += (want_point || want_offset) as u32;
        match hit {
            None => misses += 1,
            Some(h) => {
                hits += 1;
                let centre = case.vec3(a(2));
                let origin_inside = (0..3)
                    .map(|k| (case.vec3(a(0))[k] - centre[k]).powi(2))
                    .sum::<f32>()
                    < case.input_f(args);
                inside += origin_inside as u32;
                assert!(h.parameter >= 0.0);
            }
        }
    }
    assert!(
        hits >= 20 && misses >= 20 && inside >= 3 && stores >= 50,
        "{hits} {misses} {inside} {stores}"
    );
}

#[test]
fn bone_matrix_transform_d6aa0_matches_original() {
    let cases = parse(include_str!("data/t1846/shape_parts_d6aa0.golden"));
    assert!(cases.len() >= 60);
    let (object, first, second, point, matrices) = (
        BASE,
        BASE + 0x100,
        BASE + 0x200,
        BASE + 0x300,
        BASE + 0x10000,
    );
    let (mut transformed, mut skipped) = (0, 0);
    for (n, case) in cases.iter().enumerate() {
        let memory = CaseMemory(case);
        let input = case.vec3(point);
        let mode = mode_of(case);
        let result =
            transform_by_bone_matrix(&memory, object, first, second, mode, matrices, input);
        let expected = bits(case, point, 4);
        match result {
            Some(p) => {
                transformed += 1;
                assert_eq!(expected[..3], f3(p)[..], "case {n} mode {mode:#x}");
            }
            None => {
                skipped += 1;
                assert_eq!(expected[..3], f3(input)[..], "case {n} untouched");
            }
        }
        assert_eq!(expected[3], 0x5E57_0009, "case {n} w untouched");
    }
    assert!(transformed >= 30 && skipped >= 3, "{transformed} {skipped}");
}

/// The generator records the esi mode of the call in the guest word at `BASE + 0x400`.
fn mode_of(case: &Case) -> u32 {
    case.input(BASE + 0x400)
}

fn world_object(memory: &CaseMemory, object: u32) -> WorldMatrixObject {
    let word = |offset: u32| memory.read_u32(object + offset);
    let float = |offset: u32| memory.read_f32(object + offset);
    let mut matrix = [0.0f32; 16];
    for (i, m) in matrix.iter_mut().enumerate() {
        *m = float(0x274 + 4 * i as u32);
    }
    let mut plane = [0.0f32; 7];
    for (i, p) in plane.iter_mut().enumerate() {
        *p = float(0x2B8 + 4 * i as u32);
    }
    WorldMatrixObject {
        kind: memory.read_u8(object + 0x32) as i8,
        flags: word(0x28),
        position: [float(0x64), float(0x68), float(0x6C)],
        angle_y: float(0x70),
        angle_a: float(0xC0),
        angle_c: float(0xC8),
        matrix,
        mirror_word: word(0x2B4),
        plane,
    }
}

#[test]
fn link_shape_ends_d9510_matches_original() {
    let cases = parse(include_str!("data/t1846/shape_parts_d9510.golden"));
    assert!(cases.len() >= 100);
    let (link, o2, o3, o4, args) = (
        BASE,
        BASE + 0x2000,
        BASE + 0x2100,
        BASE + 0x2200,
        BASE + 0x2300,
    );
    let mut paths = BTreeMap::new();
    for (n, case) in cases.iter().enumerate() {
        let memory = CaseMemory(case);
        let mut build = |object: u32| {
            let mut state = world_object(&memory, object);
            let mut out = [0.0f32; 16];
            build_world_matrix(&mut state, &mut out, true, 0, false);
            out
        };
        let (ends, path) = link_shape_ends(&memory, link, &mut build);
        *paths.entry(path).or_insert(0) += 1;
        let keep = |i: u32| case.input(args + 4 * i) != 0;
        let pick =
            |wanted: bool, value: Vec<u32>, default: Vec<u32>| if wanted { value } else { default };
        assert_eq!(
            bits(case, o2, 3),
            pick(keep(0), f3(ends.upper), SENTINEL.to_vec()),
            "case {n} upper {path:?}"
        );
        assert_eq!(
            bits(case, o3, 3),
            pick(keep(1), f3(ends.lower), SENTINEL.to_vec()),
            "case {n} lower {path:?}"
        );
        assert_eq!(
            bits(case, o4, 1),
            pick(keep(2), vec![ends.radius_bits], vec![0x5E57_0004]),
            "case {n} radius {path:?}"
        );
    }
    for path in [
        LinkEndsPath::Oriented,
        LinkEndsPath::Short,
        LinkEndsPath::Tall,
    ] {
        assert!(
            paths.get(&path).copied().unwrap_or(0) >= 5,
            "{path:?} {paths:?}"
        );
    }
}

#[test]
fn vertex_accessors_match_original() {
    for (name, text) in [
        ("12820", include_str!("data/t1846/shape_parts_12820.golden")),
        ("12ba0", include_str!("data/t1846/shape_parts_12ba0.golden")),
    ] {
        let cases = parse(text);
        assert!(cases.len() >= 50);
        let (mut small, mut large, mut null) = (0, 0, 0);
        for (n, case) in cases.iter().enumerate() {
            let memory = CaseMemory(case);
            let (shape, mesh, face) = (BASE, BASE + 0x100, BASE + 0x200);
            let address = if name == "12820" {
                vertex_slot_address(&memory, shape, mesh, face)
            } else {
                second_stream_address(&memory, shape, mesh, face)
            };
            assert_eq!(case.eax, address, "{name} case {n}");
            if case.input(shape + 0xC) & 0x10 != 0 {
                small += 1;
            } else {
                large += 1;
            }
            null += (address == 0) as u32;
        }
        assert!(small >= 10 && large >= 10, "{name} {small} {large}");
        assert!(name == "12820" || null >= 3, "{name} null {null}");
    }
}

#[test]
fn decode_vertex_12a90_matches_original() {
    let cases = parse(include_str!("data/t1846/shape_parts_12a90.golden"));
    assert!(cases.len() >= 50);
    let (out, shape, src) = (BASE, BASE + 0x100, BASE + 0x200);
    let (mut small, mut large, mut negative_w) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let memory = CaseMemory(case);
        let words = decode_vertex(&memory, shape, src);
        assert_eq!(bits(case, out, 4), words.to_vec(), "case {n}");
        if case.input(shape + 0xC) & 0x10 != 0 {
            small += 1;
            negative_w += (words[3] >> 16 == 0xFFFF) as u32 + (words[3] >> 31 == 1) as u32;
        } else {
            large += 1;
        }
    }
    assert!(
        small >= 20 && large >= 20 && negative_w >= 2,
        "{small} {large} {negative_w}"
    );
}
