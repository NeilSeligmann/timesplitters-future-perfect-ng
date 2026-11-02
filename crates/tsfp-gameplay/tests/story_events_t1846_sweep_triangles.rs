//! T1846 batch 8: the world mesh triangle test 0x4B310 (stationary 0x4A9D0 and moving 0x4AE00 with callees) against
//! emulated-original golden vectors (MEASURED, bit exact).
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::sweep_records::{GuestMemory, TriangleTest, TriangleTestCall};
use tsfp_gameplay::story_events::sweep_triangles::WorldMeshTriangles;
use tsfp_gameplay::story_events::world_queries::SweepBox;

const SW: u32 = 0x140_0000;
const OUT: u32 = 0x140_7000;
const ARGS: u32 = 0x140_8000;
const SENTINEL: u32 = 0x5E57_0001;

/// Guest memory over the case inputs (unmapped reads as zero).
struct CaseMemory<'a>(&'a Case);

impl GuestMemory for CaseMemory<'_> {
    fn read_u8(&self, address: u32) -> u8 {
        (self.0.input(address & !3) >> (8 * (address & 3))) as u8
    }
}

fn sweep(case: &Case) -> SweepBox {
    let f = |i: u32| case.input_f(SW + 4 * i);
    SweepBox {
        stationary: case.input(SW),
        start: [f(1), f(2), f(3)],
        delta: [f(4), f(5), f(6)],
        radius: f(7),
        min: [f(8), f(9), f(10)],
        max: [f(11), f(12), f(13)],
    }
}

fn call_of(case: &Case) -> TriangleTestCall {
    let a = |i: u32| case.input(ARGS + 4 * i);
    TriangleTestCall {
        sweep: sweep(case),
        sub_record_address: a(0),
        bounds_address: a(1),
        header_plus_0x38: a(2),
        quantised: a(3),
        table_address: a(4),
        entry_address: a(5),
        flag: a(6),
        list_flag5: a(7) as u8,
    }
}

fn run(path: &str, text: &str) {
    let cases = parse(text);
    assert!(cases.len() >= 50, "{path}: {} cases", cases.len());
    let mut seen: BTreeMap<&str, u32> = BTreeMap::new();
    for (n, case) in cases.iter().enumerate() {
        let memory = CaseMemory(case);
        let call = call_of(case);
        let mut model = WorldMeshTriangles { memory: &memory };
        let result = model.test(&call);
        assert_eq!(result.code, case.eax, "{path} case {n} code");
        let word = |i: u32| case.output(OUT + 4 * i);
        let kind = if result.code == 0 {
            // A rejection may still have written some outputs (position, f18); only the code matters to the caller.
            "reject"
        } else {
            for i in 0..3 {
                assert_eq!(
                    result.position[i].to_bits(),
                    word(i as u32),
                    "{path} case {n} position[{i}]"
                );
                assert_eq!(
                    result.normal[i].to_bits(),
                    word(3 + i as u32),
                    "{path} case {n} normal[{i}]"
                );
            }
            assert_eq!(result.f18_bits, word(6), "{path} case {n} f18");
            assert_eq!(
                result.fraction.to_bits(),
                word(7),
                "{path} case {n} fraction"
            );
            let table_normal: Vec<f32> = (1..4)
                .map(|i| f32::from(memory.read_u8(call.entry_address + i) as i8) / 127.0)
                .collect();
            let dot: f32 = (0..3).map(|i| result.normal[i] * table_normal[i]).sum();
            if dot.abs() > 0.999 { "face" } else { "edge" }
        };
        let mode = if call.sweep.stationary != 0 {
            "stationary"
        } else {
            "moving"
        };
        *seen.entry(mode).or_insert(0) += 1;
        *seen.entry(kind).or_insert(0) += 1;
        if call.quantised != 0 {
            *seen
                .entry(if result.code != 0 {
                    "quantised hit"
                } else {
                    "quantised miss"
                })
                .or_insert(0) += 1;
        }
        if call.flag != 0 {
            *seen.entry("flag set").or_insert(0) += 1;
        }
        if call.list_flag5 != 0 {
            *seen.entry("flag5 set").or_insert(0) += 1;
        }
    }
    eprintln!("{path}: {seen:?}");
    for required in [
        "reject",
        "face",
        "edge",
        "quantised hit",
        "quantised miss",
        "flag set",
        "flag5 set",
    ] {
        assert!(
            seen.get(required).copied().unwrap_or(0) > 0,
            "{path}: no {required} case"
        );
    }
}

#[test]
fn stationary_matches_original() {
    run(
        "stationary",
        include_str!("data/t1846/sweep_triangles_stationary.golden"),
    );
}

#[test]
fn moving_matches_original() {
    run(
        "moving",
        include_str!("data/t1846/sweep_triangles_moving.golden"),
    );
}

#[test]
fn sentinel_is_a_distinct_word() {
    assert_ne!(SENTINEL, 0);
}
