//! T1846 batch 8: dynamic shape test 0x4B3B0 (with leaves 0x484D0 and 0x4A270) and the impact spawners 0x129C20 and
//! 0x129CA0 against emulated-original golden vectors (MEASURED). The shape functions and the spawner control flow ran
//! as original machine code; the engine calls 0xE94C0, 0x45820, 0x103A00 are logged stubs and 0xC3E60 a logging thunk
//! returning a recorded height. Generator: `tools/t1846_gold/sweep_dynamic_shape.py`.
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::sweep_dynamic_shape::*;
use tsfp_gameplay::story_events::sweep_records::{DynamicShapeResult, GuestMemory};

const OBJ: u32 = 0x1700000;
const VEC_A: u32 = 0x1710000;
const VEC_D: u32 = 0x1710010;
const OUTS: u32 = 0x1720000;
const ARGS: u32 = 0x1780000;

struct CaseMemory<'a>(&'a BTreeMap<u32, u32>);

impl GuestMemory for CaseMemory<'_> {
    fn read_u8(&self, address: u32) -> u8 {
        let word = self.0.get(&(address & !3)).copied().unwrap_or(0);
        (word >> ((address & 3) * 8)) as u8
    }
}

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn same_vec(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| same(a[i], b[i]))
}

fn load(name: &str) -> Vec<Case> {
    let text = std::fs::read_to_string(format!(
        "{}/tests/data/t1846/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let cases = parse(&text);
    assert!(cases.len() >= 50, "{name}: {} cases", cases.len());
    cases
}

#[test]
fn dynamic_shape_test_matches_original() {
    let cases = load("sweep_dynamic_shape");
    let mut routes: BTreeMap<String, usize> = BTreeMap::new();
    for (index, case) in cases.iter().enumerate() {
        let memory = CaseMemory(&case.ins);
        let mut out = DynamicShapeResult {
            code: 0,
            position: case.vec3(OUTS),
            normal: case.vec3(OUTS + 0x10),
            f18_bits: case.input(OUTS + 0x20),
            fraction: case.input_f(OUTS + 0x24),
        };
        let (code, route) = dynamic_shape_test_routed(
            &memory,
            case.vec3(VEC_A),
            case.vec3(VEC_D),
            case.input_f(ARGS),
            OBJ,
            case.input(ARGS + 4) != 0,
            case.input(ARGS + 8) != 0,
            &mut out,
        );
        *routes.entry(format!("{route:?}/{code}")).or_default() += 1;
        assert_eq!(code, case.eax, "case {index} return");
        assert!(
            same_vec(out.position, case.vec3_out(OUTS)),
            "case {index} position"
        );
        assert!(
            same_vec(out.normal, case.vec3_out(OUTS + 0x10)),
            "case {index} normal"
        );
        assert!(
            same(f32::from_bits(out.f18_bits), case.output_f(OUTS + 0x20)),
            "case {index} f18"
        );
        assert!(
            same(out.fraction, case.output_f(OUTS + 0x24)),
            "case {index} fraction"
        );
    }
    println!("routes {routes:?}");
    for route in [
        "PlaneRejected/0",
        "FractionRejected/0",
        "PlaneHit/1",
        "EdgeMiss/0",
        "EdgeHit/1",
    ] {
        assert!(
            routes.get(route).copied().unwrap_or(0) >= 3,
            "route {route} {routes:?}"
        );
    }
}

trait Vec3Out {
    fn vec3_out(&self, address: u32) -> [f32; 3];
}

impl Vec3Out for Case {
    fn vec3_out(&self, address: u32) -> [f32; 3] {
        [
            self.output_f(address),
            self.output_f(address + 4),
            self.output_f(address + 8),
        ]
    }
}

#[test]
fn guest_shape_test_implements_the_trait() {
    use tsfp_gameplay::story_events::sweep_records::{DynamicShapeCall, DynamicShapeTest};
    let cases = load("sweep_dynamic_shape");
    let mut checked = 0;
    for case in cases
        .iter()
        .filter(|c| c.input(ARGS + 4) == 0 && c.input(ARGS + 8) == 0)
    {
        let memory = CaseMemory(&case.ins);
        let mut shapes = GuestShapeTest { memory: &memory };
        let mut out = DynamicShapeResult {
            code: 0,
            position: case.vec3(OUTS),
            normal: case.vec3(OUTS + 0x10),
            f18_bits: case.input(OUTS + 0x20),
            fraction: case.input_f(OUTS + 0x24),
        };
        shapes.test(
            &DynamicShapeCall {
                start: case.vec3(VEC_A),
                delta: case.vec3(VEC_D),
                radius: case.input_f(ARGS),
                object: OBJ,
            },
            &mut out,
        );
        assert_eq!(out.code, case.eax);
        assert!(same(out.fraction, case.output_f(OUTS + 0x24)));
        checked += 1;
    }
    assert!(checked >= 30);
}

#[test]
fn edge_sphere_sweep_matches_original() {
    let cases = load("sweep_dynamic_edge");
    let (b0_at, b1_at, out_at, t_at) = (0x1730000, 0x1730010, 0x1740000, 0x1740010);
    let (mut line, mut vertex, mut miss) = (0, 0, 0);
    for (index, case) in cases.iter().enumerate() {
        let mut best = case.input_f(t_at);
        let mut point = case.vec3(out_at);
        let b0 = case.vec3(b0_at);
        let hit = edge_sphere_sweep(
            case.vec3(VEC_A),
            case.vec3(VEC_D),
            case.input_f(ARGS),
            b0,
            case.vec3(b1_at),
            &mut best,
            &mut point,
        );
        assert_eq!(u32::from(hit), case.eax, "case {index} return");
        assert!(same(best, case.output_f(t_at)), "case {index} t");
        assert!(same_vec(point, case.vec3_out(out_at)), "case {index} point");
        match (hit, point == b0) {
            (false, _) => miss += 1,
            (true, true) => vertex += 1,
            (true, false) => line += 1,
        }
    }
    println!("edge: line {line} vertex {vertex} miss {miss}");
    assert!(line >= 5 && vertex >= 5 && miss >= 5);
}

#[test]
fn point_in_polygon_matches_original() {
    let cases = load("sweep_dynamic_inside");
    let point_at = 0x1750000;
    let (mut inside, mut outside) = (0, 0);
    for (index, case) in cases.iter().enumerate() {
        let memory = CaseMemory(&case.ins);
        let result = point_in_polygon(&memory, case.vec3(point_at), OBJ);
        assert_eq!(u32::from(result), case.eax, "case {index}");
        if result {
            inside += 1;
        } else {
            outside += 1;
        }
    }
    println!("inside {inside} outside {outside}");
    assert!(inside >= 10 && outside >= 10);
}

// ---------------------------------------------------------------------------------------------------------------
// Effect spawners
// ---------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct SparseMemory {
    words: BTreeMap<u32, u32>,
}

impl GuestMemory for SparseMemory {
    fn read_u8(&self, address: u32) -> u8 {
        (self.words.get(&(address & !3)).copied().unwrap_or(0) >> ((address & 3) * 8)) as u8
    }
}

impl EffectMemory for SparseMemory {
    fn write_u8(&mut self, address: u32, value: u8) {
        let shift = (address & 3) * 8;
        let word = self.words.entry(address & !3).or_default();
        *word = (*word & !(0xFF << shift)) | (u32::from(value) << shift);
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        assert_eq!(address & 3, 0, "unaligned dword write {address:#x}");
        self.words.insert(address, value);
    }
}

#[derive(Debug, PartialEq)]
enum Call {
    Disturb([f32; 3], f32, u32),
    Spawn([u32; 12]),
    Release(u32),
    Ground([f32; 3], f32, f32),
}

#[derive(Default)]
struct Recorder {
    calls: Vec<Call>,
    height: f32,
}

impl ImpactServices for Recorder {
    fn disturb(&mut self, position: [f32; 3], radius: f32, flags: u32) {
        self.calls.push(Call::Disturb(position, radius, flags));
    }
    fn spawn_request(&mut self, record: [u32; 12]) {
        self.calls.push(Call::Spawn(record));
    }
    fn build_debris(&mut self, slot: u32) {
        self.calls.push(Call::Release(slot));
    }
    fn ground_height(&mut self, point: [f32; 3], y_bias: f32, half_extent: f32) -> f32 {
        self.calls.push(Call::Ground(point, y_bias, half_extent));
        self.height
    }
}

fn memory_of(case: &Case) -> SparseMemory {
    SparseMemory {
        words: case.ins.clone(),
    }
}

const RNG_AT: u32 = 0x4E7950;

#[test]
fn ambient_effect_matches_original() {
    let cases = load("sweep_dynamic_ambient");
    for (index, case) in cases.iter().enumerate() {
        let mut memory = memory_of(case);
        let mut services = Recorder::default();
        let position = case.vec3(0x1760000);
        ambient_effect(&mut memory, &mut services, position);
        assert_eq!(case.calls.len(), 2, "case {index} golden calls");
        // 0xE94C0(&pos, 36.0, 0): stack args then the pointed to vector
        let disturb = &case.calls[0];
        assert_eq!(disturb.callee, 0xE94C0);
        let expected = Call::Disturb(
            [
                f32::from_bits(disturb.args[3]),
                f32::from_bits(disturb.args[4]),
                f32::from_bits(disturb.args[5]),
            ],
            f32::from_bits(disturb.args[1]),
            disturb.args[2],
        );
        assert_eq!(services.calls[0], expected, "case {index} disturb");
        // 0x45820(&record): the 12 record words follow the pointer
        let spawn = &case.calls[1];
        assert_eq!(spawn.callee, 0x45820);
        let mut record = [0u32; 12];
        record.copy_from_slice(&spawn.args[1..13]);
        assert_eq!(services.calls[1], Call::Spawn(record), "case {index} spawn");
        for offset in [0, 4, 0x7497C8 - RNG_AT] {
            let address = RNG_AT + offset;
            assert_eq!(
                memory.words.get(&address).copied().unwrap_or(0),
                case.output(address),
                "case {index} rng {address:#x}"
            );
        }
    }
}

const LOG: u32 = 0x1860000;
const RES: u32 = 0x1860040;

#[test]
fn contact_effect_matches_original() {
    let cases = load("sweep_dynamic_contact");
    let (w0, object, edge_at, dir_at, pos_at) =
        (0x1800000, 0x1840000, 0x1870000, 0x1870010, 0x1870020);
    let (mut grounds, mut releases, mut claims, mut no_position, mut rejected) = (0, 0, 0, 0, 0);
    for (index, case) in cases.iter().enumerate() {
        let mut memory = memory_of(case);
        let mut services = Recorder {
            height: case.input_f(RES),
            ..Default::default()
        };
        let use_position = case.input(ARGS + 4) != 0;
        contact_effect_at(
            &mut memory,
            &mut services,
            w0,
            object,
            [case.input_f(edge_at), case.input_f(edge_at + 4)],
            case.vec3(dir_at),
            case.input_f(ARGS),
            use_position.then(|| case.vec3(pos_at)),
        );
        // every dumped word except the thunk log
        for (&address, &word) in &case.outs {
            if (LOG..LOG + 0x28).contains(&address) {
                continue;
            }
            let got = memory.words.get(&address).copied().unwrap_or(0);
            assert_eq!(got, word, "case {index} word {address:#x}");
        }
        // engine calls: 0x103A00(0, slot) stubs in order, and the ground thunk log
        let released: Vec<u32> = services
            .calls
            .iter()
            .filter_map(|c| {
                if let Call::Release(slot) = c {
                    Some(*slot)
                } else {
                    None
                }
            })
            .collect();
        let golden_released: Vec<u32> = case
            .calls
            .iter()
            .inspect(|c| assert_eq!(c.callee, 0x103A00))
            .map(|c| {
                assert_eq!(c.args[0], 0);
                c.args[1]
            })
            .collect();
        assert_eq!(released, golden_released, "case {index} releases");
        let ground = services.calls.iter().find_map(|c| {
            if let Call::Ground(p, b, h) = c {
                Some((*p, *b, *h))
            } else {
                None
            }
        });
        match ground {
            Some((point, bias, half)) => {
                assert_eq!(case.output(LOG + 4), bias.to_bits(), "case {index} bias");
                assert_eq!(case.output(LOG + 8), half.to_bits(), "case {index} half");
                assert_eq!((case.output(LOG + 16), case.output(LOG + 20)), (0, 0));
                for axis in 0..3 {
                    assert_eq!(
                        case.output(LOG + 24 + 4 * axis as u32),
                        point[axis].to_bits(),
                        "case {index} point"
                    );
                }
                grounds += 1;
            }
            None => assert_eq!(
                case.output(LOG + 4),
                0,
                "case {index} thunk must not have run"
            ),
        }
        releases += released.len();
        if memory.words.get(&0x742360).is_some() && !use_position {
            no_position += 1;
        }
        let _ = (&mut claims, &mut rejected);
        if services.calls.is_empty() {
            rejected += 1;
        }
        claims += 1;
    }
    println!(
        "grounds {grounds} releases {releases} no_position {no_position} quiet {rejected} of {claims}"
    );
    assert!(grounds >= 10 && releases >= 10 && rejected >= 3);
}
