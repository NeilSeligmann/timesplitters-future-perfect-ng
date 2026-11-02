//! T1855: the shape scan root 0xD9760 against golden vectors from the original under the Unicorn oracle
//! (`tools/t1846_gold/shape_scan.py`). `shape_scan_d9760_script` runs the original root with its eight private callees
//! stubbed: the stub log in the RESULT region (inputs and answers) is replayed by `Replay` and every input the port
//! passes to a callee is checked. Numbers only, no code or asset bytes.
mod common;
use std::collections::BTreeMap;

use common::{Case, parse};
use tsfp_gameplay::story_events::shape_scan::*;
use tsfp_gameplay::story_events::sweep_records::GuestMemory;

const ARGS: u32 = 0x145_0800;
const POINT: u32 = 0x145_0300;
const AUX: u32 = 0x145_0400;
const RESULT: u32 = 0x146_0000;
const RECORD: usize = 22;

fn load(name: &str) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1846/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let cases = parse(&text);
    assert!(cases.len() >= 50, "{name}: only {} cases", cases.len());
    cases
}

/// The `in` image of a case as guest memory (unmapped reads as zero).
struct Image<'a>(&'a Case);

impl GuestMemory for Image<'_> {
    fn read_u8(&self, address: u32) -> u8 {
        let word = self.0.ins.get(&(address & !3)).copied().unwrap_or(0);
        (word >> (8 * (address & 3))) as u8
    }
}

fn call_of(case: &Case) -> ShapeScanCall {
    let arg = |k: u32| case.input(ARGS + 4 * k);
    let vec = |k: u32| case.vec3(arg(k));
    ShapeScanCall {
        object: arg(0),
        from: vec(1),
        direction: vec(2),
        end: vec(3),
        flag: arg(4),
        extra1: arg(7),
    }
}

fn bits(v: [f32; 3]) -> [u32; 3] {
    v.map(f32::to_bits)
}

fn f3(words: &[u32]) -> [f32; 3] {
    [
        f32::from_bits(words[0]),
        f32::from_bits(words[1]),
        f32::from_bits(words[2]),
    ]
}

/// Replays the stub log of one golden case and checks the callee inputs.
struct Replay<'a> {
    records: Vec<&'a [u32]>,
    next: usize,
    case: usize,
    calls: BTreeMap<u32, u32>,
}

impl<'a> Replay<'a> {
    fn new(case: &'a Case, number: usize, storage: &'a mut Vec<u32>) -> Self {
        let total = case.output(RESULT) as usize;
        storage.clear();
        storage.extend((1..total).map(|k| case.output(RESULT + 4 * k as u32)));
        assert_eq!(storage.len() % RECORD, 0, "case {number} log length");
        Self {
            records: storage.chunks(RECORD).collect(),
            next: 0,
            case: number,
            calls: BTreeMap::new(),
        }
    }

    /// Next record, checked for tag and inputs.
    fn take(&mut self, tag: u32, inputs: &[u32]) -> &'a [u32] {
        let record = *self
            .records
            .get(self.next)
            .unwrap_or_else(|| panic!("case {}: more calls than the original made", self.case));
        assert_eq!(
            record[0], tag,
            "case {} call {}: callee tag",
            self.case, self.next
        );
        assert_eq!(
            &record[2..2 + inputs.len()],
            inputs,
            "case {} call {} (tag {tag}): inputs",
            self.case,
            self.next
        );
        self.next += 1;
        *self.calls.entry(tag).or_insert(0) += 1;
        record
    }

    fn part_out(record: &[u32]) -> PartOut {
        let hit = record[1] != 0;
        PartOut {
            ret: record[1],
            point: hit.then(|| f3(&record[15..18])),
            aux: hit.then(|| f3(&record[18..21])),
        }
    }
}

fn join(parts: &[&[f32; 3]]) -> Vec<u32> {
    parts.iter().flat_map(|v| bits(**v)).collect()
}

impl ShapeParts for Replay<'_> {
    fn ray_sphere(
        &mut self,
        from: Vec3,
        direction: Vec3,
        centre: Vec3,
        radius_squared: f32,
    ) -> PartOut {
        let mut inputs = join(&[&from, &direction, &centre]);
        inputs.push(radius_squared.to_bits());
        Self::part_out(self.take(1, &inputs))
    }

    fn ray_box(&mut self, from: Vec3, direction: Vec3, low: Vec3, high: Vec3) -> bool {
        let record = self.take(2, &join(&[&from, &direction, &low, &high]));
        record[1] != 0
    }

    fn link_ends(&mut self, link: u32) -> LinkEndPoints {
        let record = self.take(3, &[link]);
        LinkEndPoints {
            end_a: f3(&record[15..18]),
            end_b: f3(&record[18..21]),
            flag: record[21],
        }
    }

    fn ray_shape(
        &mut self,
        from: Vec3,
        direction: Vec3,
        first: Vec3,
        second: Vec3,
        parameter: u32,
    ) -> PartOut {
        let mut inputs = join(&[&from, &direction, &first, &second]);
        inputs.push(parameter);
        Self::part_out(self.take(4, &inputs))
    }

    fn vertex_cursor(&mut self, list: u32, element: u32, group: u32) -> u32 {
        self.take(5, &[list, element, group])[1]
    }

    fn index_cursor(&mut self, list: u32, element: u32, group: u32) -> u32 {
        self.take(6, &[list, element, group])[1]
    }

    fn read_vertex(&mut self, list: u32, cursor: u32) -> Vertex {
        let record = self.take(7, &[list, cursor]);
        [record[15], record[16], record[17], record[18]]
    }

    fn skin_vertex(
        &mut self,
        element: u32,
        matrices: u32,
        vertex: Vertex,
        code: u32,
        previous: u32,
        next: u32,
    ) -> Vertex {
        let inputs = [
            element, matrices, code, previous, next, vertex[0], vertex[1], vertex[2], vertex[3],
        ];
        let record = self.take(8, &inputs);
        [record[15], record[16], record[17], record[18]]
    }
}

fn check_outcome(case: &Case, number: usize, outcome: &ShapeScanOutcome) {
    assert_eq!(case.eax, outcome.ret, "case {number}: return value");
    let sentinel = |address: u32| case.input(address);
    for k in 0..3u32 {
        let (point, aux) = (POINT + 4 * k, AUX + 4 * k);
        let want_point = outcome
            .point
            .map_or(sentinel(point), |v| v[k as usize].to_bits());
        let want_aux = outcome
            .aux
            .map_or(sentinel(aux), |v| v[k as usize].to_bits());
        assert_eq!(case.output(point), want_point, "case {number}: point[{k}]");
        assert_eq!(case.output(aux), want_aux, "case {number}: aux[{k}]");
    }
    let want_global = outcome
        .best_element
        .unwrap_or_else(|| sentinel(BEST_ELEMENT_GLOBAL));
    assert_eq!(
        case.output(BEST_ELEMENT_GLOBAL),
        want_global,
        "case {number}: best element global"
    );
}

#[test]
fn root_with_scripted_callees_matches_original() {
    let cases = load("shape_scan_d9760_script");
    let mut calls: BTreeMap<u32, u32> = BTreeMap::new();
    let (mut hits, mut misses, mut stored) = (0, 0, 0);
    let mut storage = Vec::new();
    for (number, case) in cases.iter().enumerate() {
        let image = Image(case);
        let mut replay = Replay::new(case, number, &mut storage);
        let outcome = shape_scan_d9760(&image, &mut replay, &call_of(case));
        check_outcome(case, number, &outcome);
        assert_eq!(
            replay.next,
            replay.records.len(),
            "case {number}: fewer calls than the original made"
        );
        for (tag, count) in &replay.calls {
            *calls.entry(*tag).or_insert(0) += count;
        }
        if outcome.ret != 0 {
            hits += 1;
        } else {
            misses += 1;
        }
        stored += u32::from(outcome.best_element.is_some());
    }
    // Every callee was reached, both outcomes and the scan result (global store) occur.
    for tag in 1..=8u32 {
        assert!(
            calls.get(&tag).copied().unwrap_or(0) >= 5,
            "tag {tag} {calls:?}"
        );
    }
    assert!(
        hits >= 20 && misses >= 20 && stored >= 10,
        "{hits} {misses} {stored}"
    );
}

/// The whole root with the ported callees against the original with all callees real.
#[test]
fn root_with_real_callees_matches_original() {
    let cases = load("shape_scan_d9760_real");
    let (mut hits, mut sphere_or_link, mut scans) = (0, 0, 0);
    for (number, case) in cases.iter().enumerate() {
        let image = Image(case);
        let mut no_matrix = |_object: u32| -> [f32; 16] {
            panic!("case {number}: the golden worlds have no owner of kind 2")
        };
        let mut parts = RealShapeParts {
            memory: &image,
            world_matrix: &mut no_matrix,
        };
        let outcome = shape_scan_d9760(&image, &mut parts, &call_of(case));
        check_outcome(case, number, &outcome);
        hits += usize::from(outcome.ret != 0);
        scans += usize::from(outcome.best_element.is_some());
        sphere_or_link += usize::from(outcome.ret != 0 && outcome.best_element.is_none());
    }
    assert!(
        hits >= 20 && scans >= 10 && sphere_or_link >= 3,
        "{hits} {scans} {sphere_or_link}"
    );
}

/// The wiring used by the sweep driver: `MeshWorld::shape_scan` without an override runs the ported root over the
/// guest image and mirrors the global `0x7356E0` into `ImpactGlobals::shape_source`.
#[test]
fn mesh_world_shape_scan_uses_the_ported_root() {
    use tsfp_gameplay::story_events::sweep_cells::{ImpactGlobals, MeshWorld, ScanCallees};
    let cases = load("shape_scan_d9760_real");
    let (mut hits, mut stores) = (0, 0);
    for (number, case) in cases.iter().enumerate() {
        let image = Image(case);
        let mut no_matrix =
            |_object: u32| -> [f32; 16] { panic!("case {number}: no kind 2 owner") };
        let mut world = MeshWorld {
            memory: &image,
            world_matrix: &mut no_matrix,
            shape: None,
        };
        let call = call_of(case);
        let mut globals = ImpactGlobals {
            shape_source: case.input(BEST_ELEMENT_GLOBAL),
            ..Default::default()
        };
        let hit = world.shape_scan(
            &mut globals,
            call.object,
            call.from,
            call.direction,
            call.end,
            call.flag,
            call.extra1,
        );
        assert_eq!(hit.is_some(), case.eax != 0, "case {number}: hit");
        assert_eq!(
            globals.shape_source,
            case.output(BEST_ELEMENT_GLOBAL),
            "case {number}: global"
        );
        if let Some((point, aux)) = hit {
            hits += 1;
            for k in 0..3u32 {
                assert_eq!(point[k as usize].to_bits(), case.output(POINT + 4 * k));
                assert_eq!(aux[k as usize].to_bits(), case.output(AUX + 4 * k));
            }
        }
        stores += usize::from(globals.shape_source != case.input(BEST_ELEMENT_GLOBAL));
    }
    assert!(hits >= 20 && stores >= 10, "{hits} {stores}");
}
