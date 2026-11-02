//! T1863: the particle emitter pool, creation and setters (`0x111AE0`, `0x10DF90`, `0x10C220`, `0x10E410`, `0x10E070`,
//! `0x10E510`, `0x873F0`, `0x10E660` and friends) against emulated-original golden vectors (MEASURED), plus semantic tests.
//! Golden: `tests/data/t1863/emitter_*.golden` (`tools/t1846_gold/svc_emitter.py`).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_effect_submit::{SparseSvcMemory, SvcMemory};
use tsfp_gameplay::story_events::svc_emitter::*;
use tsfp_gameplay::story_events::svc_particle_kinds::EmitterParam;

const EM0: u32 = 0x120_0000;
const POS: u32 = 0x122_0000;
const VEC: u32 = 0x122_0010;
const HANDLE: u32 = 0x122_0100;
const ARGS: u32 = 0x125_0000;

/// Sparse memory implementing both memory traits the emitter code needs.
#[derive(Default)]
struct Mem(SparseSvcMemory);

impl SvcMemory for Mem {
    fn r8(&self, address: u32) -> u8 {
        self.0.r8(address)
    }
    fn w8(&mut self, address: u32, value: u8) {
        self.0.w8(address, value);
    }
}

impl GuestMemory for Mem {
    fn read_u8(&self, address: u32) -> u8 {
        self.0.r8(address)
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        self.0.w8(address, value);
    }
    fn read_u32(&self, address: u32) -> u32 {
        self.0.r32(address)
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        self.0.w32(address, value);
    }
}

fn world(case: &Case) -> Mem {
    let mut memory = Mem::default();
    for (&address, &value) in &case.ins {
        SvcMemory::w32(&mut memory, address, value);
    }
    memory
}

#[derive(Default)]
struct Leaves {
    ground: (f32, u32),
    log: Vec<(u32, Vec<u32>)>,
}

impl EmitterServices for Leaves {
    fn ground(&mut self, point: [f32; 3]) -> (f32, u32) {
        let mut words = vec![0x1_0000]; // placeholder replaced below
        words.clear();
        words.extend(point.iter().map(|v| v.to_bits()));
        self.log.push((0xC3E60, words));
        self.ground
    }
    fn free_block(&mut self, block: u32) {
        self.log.push((0x3EAD0, vec![block]));
    }
}

fn expected_calls(case: &Case) -> Vec<(u32, Vec<u32>)> {
    case.calls
        .iter()
        .map(|call| {
            let a = &call.args;
            match call.callee {
                0xC3E60 => (0xC3E60, a[6..9].to_vec()),
                0x3EAD0 => (0x3EAD0, a[..1].to_vec()),
                other => panic!("unexpected stub {other:#x}"),
            }
        })
        .collect()
}

fn check_outputs(case: &Case, memory: &Mem, what: &str, n: usize) {
    assert!(!case.outs.is_empty());
    for (&address, &value) in &case.outs {
        assert_eq!(
            SvcMemory::r32(memory, address),
            value,
            "{what} case {n} word {address:#x}"
        );
    }
}

fn run<F: FnMut(&Case, &mut Mem, &mut Leaves) -> Option<u32>>(
    name: &str,
    text: &str,
    mut body: F,
) -> Vec<(Case, Mem, Leaves, Option<u32>)> {
    let cases = parse(text);
    assert!(cases.len() >= 60, "{name}: {} cases", cases.len());
    let mut done = Vec::new();
    for (n, case) in cases.into_iter().enumerate() {
        let mut memory = world(&case);
        let mut leaves = Leaves::default();
        leaves.ground = (
            f32::from_bits(case.input(ARGS + 0x20)),
            case.input(ARGS + 8),
        );
        let result = body(&case, &mut memory, &mut leaves);
        check_outputs(&case, &memory, name, n);
        assert_eq!(leaves.log, expected_calls(&case), "{name} case {n} calls");
        if let Some(value) = result {
            assert_eq!(value, case.eax, "{name} case {n} result");
        }
        done.push((case, memory, leaves, result));
    }
    done
}

#[test]
fn alloc_matches_original() {
    let done = run(
        "alloc",
        include_str!("data/t1863/emitter_alloc.golden"),
        |_case, memory, _| Some(emitter_alloc(memory)),
    );
    let empty = done.iter().filter(|d| d.3 == Some(0)).count();
    assert!(empty > 2 && empty < done.len() - 10);
}

#[test]
fn init_matches_original() {
    run(
        "init",
        include_str!("data/t1863/emitter_init.golden"),
        |case, memory, _| {
            emitter_init(memory, case.input(ARGS), case.input(ARGS + 4));
            None
        },
    );
}

#[test]
fn visible_matches_original() {
    let done = run(
        "visible",
        include_str!("data/t1863/emitter_visible.golden"),
        |_case, memory, _| Some(u32::from(emitter_visible(memory, EM0))),
    );
    let seen = done.iter().filter(|d| d.3 == Some(1)).count();
    assert!(seen > 10 && seen < done.len() - 10, "{seen}");
}

#[test]
fn free_matches_original() {
    // the victim record is the one with the buffer words set; find it from the case inputs
    run(
        "free",
        include_str!("data/t1863/emitter_free.golden"),
        |case, memory, leaves| {
            let victim = (0..6)
                .map(|i| EM0 + 0x100 * i)
                .find(|&address| case.ins.contains_key(&(address + 0x50)))
                .expect("victim record");
            emitter_free(memory, leaves, victim);
            None
        },
    );
}

#[test]
fn finish_matches_original() {
    let done = run(
        "finish",
        include_str!("data/t1863/emitter_finish.golden"),
        |case, memory, leaves| {
            let target = case.input(ARGS);
            Some(emitter_finish(memory, leaves, target))
        },
    );
    let freed = done.iter().filter(|d| d.3 == Some(1)).count();
    assert!(freed > 3 && freed < done.len() - 3, "{freed}");
}

#[test]
fn bounds_matches_original() {
    run(
        "bounds",
        include_str!("data/t1863/emitter_bounds.golden"),
        |case, memory, _| {
            region_bounds(memory, case.input(ARGS), EM0 + 0xC8, EM0 + 0xD4);
            None
        },
    );
}

#[test]
fn params_match_original() {
    let done = run(
        "param",
        include_str!("data/t1863/emitter_param.golden"),
        |case, memory, _| {
            let f = |k: u32| f32::from_bits(case.input(ARGS + 8 + 4 * k));
            let param = match case.input(ARGS) {
                0x10E660 => EmitterParam::Scale(f(0)),
                0x10EAE0 => EmitterParam::Span(f(0), f(1)),
                0x10EB30 => EmitterParam::Param70(f(0)),
                0x10EB70 => EmitterParam::Param74(f(0)),
                0x10E9D0 => EmitterParam::Vec98([f(0), f(1), f(2)]),
                0x10E8C0 => EmitterParam::Attach(case.input(ARGS + 4)),
                other => panic!("function {other:#x}"),
            };
            apply_param(memory, &param);
            None
        },
    );
    let mut seen = std::collections::BTreeSet::new();
    for (case, ..) in &done {
        seen.insert(case.input(ARGS));
    }
    assert_eq!(seen.len(), 6);
}

#[test]
fn create_matches_original() {
    let done = run(
        "create",
        include_str!("data/t1863/emitter_create.golden"),
        |case, memory, leaves| {
            let a = |k: u32| case.input(ARGS + 4 * k);
            let (handle_out, flags) = (a(0), a(1));
            let position = if a(3) != 0 { POS } else { 0 };
            let vector = if a(4) != 0 { VEC } else { 0 };
            Some(create_emitter(
                memory, leaves, handle_out, 0x121_0000, flags, position, vector,
            ))
        },
    );
    let created = done.iter().filter(|d| d.3 == Some(1)).count();
    let probes = done
        .iter()
        .filter(|d| d.2.log.iter().any(|(c, _)| *c == 0xC3E60))
        .count();
    let failed = done.iter().filter(|d| d.3 == Some(0)).count();
    assert!(
        created > 40 && probes > 10 && failed > 3,
        "{created} {probes} {failed}"
    );
    let _ = (HANDLE, EM0);
}
