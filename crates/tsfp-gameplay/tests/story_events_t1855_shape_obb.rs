//! T1855: the oriented shape test chain of the kind 8 shape scan (0x1512B0, 0x48590, 0x486C0, 0x48FE0) against
//! golden vectors from the original machine code under the Unicorn oracle (`tools/t1846_gold/shape_obb.py`),
//! compared bit for bit, including the sentinel words of outputs the original does not store.
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::shape_obb::*;

const ARGS: u32 = 0x130_0000;
const AXIS: u32 = 0x140_0000;
const ORIGIN: u32 = 0x140_0100;
const DIR: u32 = 0x140_0200;
const CENTRE: u32 = 0x140_0300;
const TOUT: u32 = 0x140_0400;
const POUT: u32 = 0x140_0500;
const NOUT: u32 = 0x140_0600;
const T1: u32 = 0x140_0700;
const T2: u32 = 0x140_0800;
const N: u32 = 0x140_0900;
const OUT: u32 = 0x140_0A00;
const P0: u32 = 0x140_0B00;
const P1: u32 = 0x140_0C00;

fn load(name: &str, minimum: usize) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1846/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let cases = parse(&text);
    assert!(cases.len() >= minimum, "{name}: only {} cases", cases.len());
    cases
}

fn vec3(case: &Case, address: u32) -> Vec3 {
    case.vec3(address)
}

fn out3(case: &Case, address: u32) -> Vec3 {
    [0, 4, 8].map(|k| case.output_f(address + k))
}

fn bits(v: Vec3) -> [u32; 3] {
    v.map(f32::to_bits)
}

#[test]
fn basis_1512b0_matches_original() {
    let cases = load("shape_obb_1512b0", 100);
    let (mut skipped, mut normalised) = (0, 0);
    for (n, case) in cases.iter().enumerate() {
        let skip = case.input(ARGS) != 0;
        let mut axis = vec3(case, N);
        let (t1, t2) = basis_1512b0(&mut axis, skip);
        assert_eq!(bits(axis), bits(out3(case, N)), "case {n} axis");
        assert_eq!(bits(t1), bits(out3(case, T1)), "case {n} t1");
        assert_eq!(bits(t2), bits(out3(case, T2)), "case {n} t2");
        if skip {
            skipped += 1;
        } else {
            normalised += 1;
        }
    }
    assert!(skipped > 5 && normalised > 5);
}

#[test]
fn radial_normal_48590_matches_original() {
    let cases = load("shape_obb_48590", 100);
    let mut guarded = 0;
    for (n, case) in cases.iter().enumerate() {
        let got = radial_normal_48590(
            vec3(case, AXIS),
            vec3(case, ORIGIN),
            vec3(case, DIR),
            vec3(case, CENTRE),
        );
        assert_eq!(bits(got), bits(out3(case, OUT)), "case {n}");
        let length_sq = got.iter().map(|c| c * c).sum::<f32>();
        if length_sq < 0.99 {
            guarded += 1;
        }
    }
    assert!(guarded > 5, "the 1e-7 guard branch is not exercised");
}

fn expected_hits(case: &Case) -> CylinderHits {
    let at = |address: u32| case.input_f(address);
    let read_vec = |address: u32| [0, 4, 8].map(|k| at(address + k));
    CylinderHits {
        t: [at(TOUT), at(TOUT + 4)],
        points: [read_vec(POUT), read_vec(POUT + 12)],
        normals: [read_vec(NOUT), read_vec(NOUT + 12)],
    }
}

#[test]
fn cylinder_ray_486c0_matches_original() {
    let cases = load("shape_obb_486c0", 150);
    let mut by_count = [0usize; 3];
    for (n, case) in cases.iter().enumerate() {
        let query = CylinderQuery {
            axis: vec3(case, AXIS),
            origin: vec3(case, ORIGIN),
            dir: vec3(case, DIR),
            centre: vec3(case, CENTRE),
            radius_sq: f32::from_bits(case.input(ARGS)),
            length: f32::from_bits(case.input(ARGS + 4)),
        };
        let mut hits = expected_hits(case);
        let count = cylinder_ray_486c0(&query, &mut hits);
        assert_eq!(count, case.eax, "case {n} count");
        for i in 0..2u32 {
            let k = 12 * i;
            assert_eq!(
                hits.t[i as usize].to_bits(),
                case.output(TOUT + 4 * i),
                "case {n} t{i}"
            );
            assert_eq!(
                bits(hits.points[i as usize]),
                bits(out3(case, POUT + k)),
                "case {n} p{i}"
            );
            assert_eq!(
                bits(hits.normals[i as usize]),
                bits(out3(case, NOUT + k)),
                "case {n} n{i}"
            );
        }
        by_count[count as usize] += 1;
    }
    assert!(by_count.iter().all(|&c| c > 5), "counts {by_count:?}");
}

#[test]
fn shape_hit_48fe0_matches_original() {
    let cases = load("shape_obb_48fe0", 150);
    let (mut hit_count, mut miss_count) = (0, 0);
    for (n, case) in cases.iter().enumerate() {
        let contact = shape_hit_48fe0(
            vec3(case, ORIGIN),
            vec3(case, DIR),
            vec3(case, P0),
            vec3(case, P1),
            f32::from_bits(case.input(ARGS)),
        );
        assert_eq!(case.eax, contact.is_some() as u32, "case {n} flag");
        let (position, normal) = match contact {
            Some(c) => (c.position, c.normal),
            None => (vec3(case, POUT), vec3(case, NOUT)),
        };
        assert_eq!(bits(position), bits(out3(case, POUT)), "case {n} position");
        assert_eq!(bits(normal), bits(out3(case, NOUT)), "case {n} normal");
        if contact.is_some() {
            hit_count += 1;
        } else {
            miss_count += 1;
        }
    }
    assert!(
        hit_count > 10 && miss_count > 10,
        "{hit_count} hits {miss_count} misses"
    );
}
