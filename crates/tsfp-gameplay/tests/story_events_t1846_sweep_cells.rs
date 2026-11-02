//! T1846 batch 8: cell geometry under the sweep driver (0x88430, 0x477B0, 0xA3140, 0xA3500, 0x88950 with the
//! real cell code) against golden vectors from the original under the Unicorn oracle
//! (`tools/t1846_gold/sweep_cells.py`). Every golden case carries the whole synthetic world as `in` lines; the
//! test rebuilds the typed model world from that memory image.
mod common;
use std::collections::BTreeMap;

use common::{Case, parse};
use tsfp_gameplay::story_events::group_members::*;
use tsfp_gameplay::story_events::sweep_cells::*;
use tsfp_gameplay::story_events::sweep_records::GuestMemory;
use tsfp_gameplay::story_events::world_queries::Mat4;

const ARGS: u32 = 0x130_0000;
const FROM: u32 = 0x150_0000;
const VEC: u32 = 0x150_0100;
const OUTP: u32 = 0x150_0200;
const OUTD: u32 = 0x150_0300;
const OUTC: u32 = 0x150_0400;
const OUTH: u32 = 0x150_0500;
const SCRIPT: u32 = 0x152_0000;
const BUF: u32 = 0x154_0000;
const LIMITP: u32 = 0x158_0200;
const WHICH: u32 = 0x130_0100;
const TRI: u32 = 0x151_0000;
const RECORD: u32 = 12;
const SLOTS_BASE: u32 = 0x7B_9784;

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

fn arg(case: &Case, index: u32) -> u32 {
    case.input(ARGS + 4 * index)
}

fn bits3(v: [f32; 3]) -> [u32; 3] {
    v.map(f32::to_bits)
}

/// Expected memory after the call: input values overlaid with `writes`.
fn check_outs(case: &Case, writes: &BTreeMap<u32, u32>) {
    assert!(!case.outs.is_empty());
    for (&address, &actual) in &case.outs {
        let want = writes
            .get(&address)
            .copied()
            .unwrap_or_else(|| case.input(address));
        assert_eq!(actual, want, "memory {address:#x}");
    }
}

fn put3(writes: &mut BTreeMap<u32, u32>, address: u32, value: [f32; 3]) {
    for (axis, component) in value.iter().enumerate() {
        writes.insert(address + 4 * axis as u32, component.to_bits());
    }
}

// ---------------------------------------------------------------- world loaders

fn load_graph(case: &Case) -> CellGraph {
    let cells_base = case.input(0x70_8788);
    let table = case.input(0x70_878C);
    let max_portal_id = case.input(0x7B_9754) as i32;
    let cells = (0..=MAX_CELL)
        .map(|cell| {
            let list = case.input(cells_base.wrapping_add(0xB4 * cell + 4));
            if list == 0 {
                return None;
            }
            let count = case.input(list) as i32;
            Some(
                (0..count.max(0) as u32)
                    .map(|i| case.input(list + 4 + 4 * i) as i32)
                    .collect(),
            )
        })
        .collect();
    let portals = (0..64u32)
        .map(|id| {
            let base = case.input(table + 4 * id);
            if base == 0 {
                return None;
            }
            let count = case.input(base + 0x14) as u16 as i16;
            let vertices = (0..count.max(0) as u32)
                .map(|k| case.vec3(base + 0x18 + 12 * k))
                .collect();
            Some(Portal {
                cell_a: case.input(base),
                cell_b: case.input(base + 4),
                normal: case.vec3(base + 8),
                plane_point: case.vec3(base + 0x18),
                vertices,
            })
        })
        .collect();
    CellGraph {
        max_portal_id,
        cells,
        portals,
    }
}

fn load_wall(case: &Case, address: u32) -> Wall {
    Wall {
        address,
        mask: case.input(address),
        flags: case.input(address + 4),
        corner0: case.vec3(address + 8),
        corner1: case.vec3(address + 0x14),
        min_x: case.input_f(address + 0x20),
        min_z: case.input_f(address + 0x24),
        max_x: case.input_f(address + 0x28),
        max_z: case.input_f(address + 0x2C),
    }
}

fn load_box(case: &Case, address: u32) -> BoxRecord {
    let packed = case.input(address + 0x3C);
    let word = case.input(address + 0x28);
    BoxRecord {
        address,
        enabled: case.input_f(address),
        portal: case.input(address + 0xC) as u16 as i16,
        flags: word as u16,
        normal: case.vec3(address + 0x30),
        axes: [
            packed as u8 as i8,
            (packed >> 8) as u8 as i8,
            (packed >> 16) as u8 as i8,
        ],
        plane_point: case.vec3(address + 0x40),
        vertices: (0..(word >> 16))
            .map(|k| case.vec3(address + 0x40 + 0x1C * k))
            .collect(),
    }
}

fn load_object(case: &Case, address: u32) -> SolidObject {
    let class = case.input(address + 4);
    let owner = case.input(address + 0x24);
    let array = case.input(address + 0x1F4);
    let count = case.input(address + 0x1E4) as i32;
    let walls = if array == 0 {
        Vec::new()
    } else {
        (0..count.max(0) as u32)
            .map(|i| load_wall(case, case.input(array + 4 * i)))
            .collect()
    };
    let box_array = case.input(address + 0x1F8);
    let boxes = if box_array == 0 {
        Vec::new()
    } else {
        (0..case.input(address + 0x1C) as i32)
            .map(|i| load_box(case, case.input(box_array + 4 * i as u32)))
            .collect()
    };
    SolidObject {
        address,
        cell: case.input(address + 0x34),
        link: case.input(address + 0x78),
        link_flags: case.input(case.input(address + 0x7C) + 0x24),
        boxes,
        class_address: class,
        class_mask: case.input(class + 0x30),
        mask: case.input(address + 0x20),
        owner_flags: case.input(owner + 0x28),
        flags: case.input(address + 0x28),
        has_walls: array != 0,
        walls,
    }
}

fn load_solids(case: &Case) -> SolidWorld {
    let slot_table = case.input(SLOTS_BASE);
    let max_slot = case.input(0x7B_975C) as i32;
    let slots = (0..=MAX_CELL)
        .map(|cell| {
            if slot_table == 0 || cell as i32 > max_slot {
                return None;
            }
            match case.input(slot_table + 4 * cell) {
                0 => None,
                address => Some(load_object(case, address)),
            }
        })
        .collect();
    let dynamic = (0..=MAX_CELL)
        .map(|cell| {
            let count = case.input(0x7B_54E0 + 4 * cell) as i32;
            let array = case.input(0x7B_5F00 + 4 * cell);
            (0..count.max(0) as u32)
                .map(|i| load_object(case, case.input(array + 4 * i)))
                .collect()
        })
        .collect();
    let global_array = case.input(0x7B_6254);
    let global_objects = (0..case.input(0x7B_5834) as i32)
        .map(|i| load_object(case, case.input(global_array + 4 * i as u32)))
        .collect();
    SolidWorld {
        max_slot,
        slots,
        dynamic,
        global_objects,
    }
}

// ---------------------------------------------------------------- 0x88430
#[test]
fn cell_step_matches_original() {
    let (mut moved, mut unchanged, mut ray, mut aliased, mut split, mut nulls) = (0, 0, 0, 0, 0, 0);
    for case in load("sweep_cells_88430") {
        let graph = load_graph(&case);
        let [from_ptr, vec_ptr, cell, previous, flag, out_p, out_v] =
            [0, 1, 2, 3, 4, 5, 6].map(|i| arg(&case, i));
        let from = case.vec3(from_ptr);
        let vec = case.vec3(vec_ptr);
        let step = cell_step(&graph, from, vec, cell, previous, flag);
        assert_eq!(case.eax, step.next, "next cell");
        let mut writes = BTreeMap::new();
        if let Some(hit) = step.entry {
            moved += 1;
            if flag == 0 && out_v != 0 {
                let rest = [0, 1, 2].map(|k| from[k] + vec[k] - hit[k]);
                put3(&mut writes, out_v, rest);
            }
            if out_p != 0 {
                put3(&mut writes, out_p, hit);
            }
        } else {
            unchanged += 1;
        }
        flag.min(1).eq(&1).then(|| ray += 1);
        if out_p == from_ptr && out_v == vec_ptr {
            aliased += 1;
        } else if out_p != 0 && out_v != 0 {
            split += 1;
        } else if out_p == 0 || out_v == 0 {
            nulls += 1;
        }
        check_outs(&case, &writes);
        // The driver form must agree with the aliased buffers.
        if out_p == from_ptr && out_v == vec_ptr {
            let advanced = advance_88430(
                &graph,
                &AdvanceProbe {
                    from_copy: from,
                    vec_copy: vec,
                    cell,
                    previous,
                    flag,
                },
            );
            assert_eq!(advanced.next, case.eax);
            assert_eq!(
                bits3(advanced.from_copy),
                bits3([0, 1, 2].map(|k| f32::from_bits(case.output(from_ptr + 4 * k))))
            );
            assert_eq!(
                bits3(advanced.vec_copy),
                bits3([0, 1, 2].map(|k| f32::from_bits(case.output(vec_ptr + 4 * k))))
            );
        }
    }
    assert!(
        moved >= 15 && unchanged >= 10,
        "moved {moved} unchanged {unchanged}"
    );
    assert!(ray >= 10 && aliased >= 10 && split >= 8 && nulls >= 5);
}

// ---------------------------------------------------------------- 0x477B0
#[test]
fn ray_triangle_matches_original() {
    let (mut hits, mut misses) = (0, 0);
    for case in load("sweep_cells_477b0") {
        let [origin, dir, v0, v1, v2, out_p, out_n] = [0, 1, 2, 3, 4, 5, 6].map(|i| arg(&case, i));
        let result = ray_triangle(
            case.vec3(origin),
            case.vec3(dir),
            case.vec3(v0),
            case.vec3(v1),
            case.vec3(v2),
        );
        assert_eq!(case.eax, u32::from(result.is_some()));
        let mut writes = BTreeMap::new();
        if let Some(hit) = result {
            hits += 1;
            if out_p != 0 {
                put3(&mut writes, out_p, hit.point);
            }
            if out_n != 0 {
                put3(&mut writes, out_n, hit.normal);
            }
        } else {
            misses += 1;
        }
        check_outs(&case, &writes);
    }
    assert!(hits >= 20 && misses >= 15, "hits {hits} misses {misses}");
}

// ---------------------------------------------------------------- 0xA3140
#[test]
fn wall_scan_matches_original() {
    let (mut hits, mut misses, mut flagged, mut filtered) = (0, 0, 0, 0);
    for case in load("sweep_cells_a3140") {
        let [object, flag, out_p, out_a, out_i, filter] = [0, 1, 2, 3, 4, 5].map(|i| arg(&case, i));
        let object = load_object(&case, object);
        let result = wall_scan_a3140(&object, flag, filter != 0, case.vec3(FROM), case.vec3(VEC));
        assert_eq!(case.eax, u32::from(result.is_some()));
        let mut writes = BTreeMap::new();
        if let Some(hit) = result {
            hits += 1;
            if out_p != 0 {
                // The original stores the aux vector unchecked whenever the point pointer is set.
                assert_ne!(out_a, 0);
                put3(&mut writes, out_p, hit.point);
                put3(&mut writes, out_a, hit.normal);
            }
            if out_i != 0 {
                writes.insert(out_i, hit.wall);
            }
        } else {
            misses += 1;
        }
        flagged += usize::from(flag != 0);
        filtered += usize::from(filter != 0);
        check_outs(&case, &writes);
    }
    assert!(hits >= 20 && misses >= 10 && flagged >= 10 && filtered >= 8);
}

// ---------------------------------------------------------------- 0xA3500
fn a3500_probe(case: &Case) -> CellProbe {
    CellProbe {
        cell: arg(case, 0),
        from: case.vec3(arg(case, 1)),
        vec: case.vec3(arg(case, 2)),
        flag: arg(case, 3),
        extra: [arg(case, 7), 0, 0],
        variant_a7a00: false,
    }
}

#[test]
fn cell_test_a3500_matches_original() {
    let (mut hits, mut misses, mut dynamic_hits, mut slot_hits) = (0, 0, 0, 0);
    for case in load("sweep_cells_a3500") {
        let solids = load_solids(&case);
        let probe = a3500_probe(&case);
        let result = cell_test_a3500(&solids, &probe);
        assert_eq!(case.eax, result.ret, "returned object");
        let [out_p, out_a, out_i] = [4, 5, 6].map(|i| arg(&case, i));
        let mut writes = BTreeMap::new();
        if result.ret != 0 {
            hits += 1;
            let slot = solids
                .slots
                .get(probe.cell as usize)
                .and_then(|slot| slot.as_ref())
                .map(|object| object.address);
            if slot == Some(result.ret) {
                slot_hits += 1;
            } else {
                dynamic_hits += 1;
            }
            if out_p != 0 {
                put3(&mut writes, out_p, result.point);
            }
            if out_a != 0 {
                put3(&mut writes, out_a, result.aux);
            }
            if out_i != 0 {
                writes.insert(out_i, result.int_out);
            }
        } else {
            misses += 1;
        }
        check_outs(&case, &writes);
    }
    assert!(hits >= 20 && misses >= 8, "hits {hits} misses {misses}");
    assert!(
        slot_hits >= 5 && dynamic_hits >= 5,
        "slot {slot_hits} dynamic {dynamic_hits}"
    );
}

// ---------------------------------------------------------------- 0x88950 with the real cell code
#[test]
fn sweep_driver_with_real_cells_matches_original() {
    let (mut hits, mut misses, mut early, mut multi) = (0, 0, 0, 0);
    for case in load("sweep_cells_driver") {
        let graph = load_graph(&case);
        let solids = load_solids(&case);
        let mut geometry = SweepCells {
            graph: &graph,
            solids: &solids,
            callees_a7a00: None,
        };
        let with_cell = arg(&case, 4) != 0;
        let args = SweepArgs {
            cell: arg(&case, 9),
            from: case.vec3(FROM),
            vec: case.vec3(VEC),
            use_a7a00: false,
            flag: arg(&case, 1),
            extra: [arg(&case, 6), arg(&case, 7), arg(&case, 8)],
        };
        let mut io = SweepIo {
            position: Some(case.vec3(OUTP)),
            direction: Some([OUTD, OUTD + 4, OUTD + 8].map(|a| case.input_f(a))),
            cell: with_cell.then(|| case.input(OUTC)),
            hit: Some(case.input(OUTH)),
            ..Default::default()
        };
        let ret = sweep_88950(&mut geometry, &args, &mut io).expect("no error");
        assert_eq!(ret, case.eax, "return value");
        let mut writes = BTreeMap::new();
        put3(&mut writes, OUTP, io.position.unwrap());
        put3(&mut writes, OUTD, io.direction.unwrap());
        if let Some(cell) = io.cell {
            writes.insert(OUTC, cell);
        }
        writes.insert(OUTH, io.hit.unwrap());
        check_outs(&case, &writes);
        if ret != 0 {
            hits += 1;
        } else if args.cell == 0 || io.cell == Some(args.cell) {
            early += 1;
        } else {
            misses += 1;
        }
        multi += usize::from(io.cell.is_some_and(|cell| cell != args.cell));
    }
    assert!(
        hits >= 10 && misses + early >= 8 && multi >= 10,
        "hits {hits} misses {misses} early {early} multi {multi}"
    );
}

// ---------------------------------------------------------------- 0xA7A00 (callees replayed from the golden)
/// Replays the scripted answers of the four unported callees and checks every argument the model passes against
/// the call log of the original.
struct Replay<'a> {
    case: &'a Case,
    record: u32,
    call: usize,
    counts: BTreeMap<u32, usize>,
}

impl Replay<'_> {
    fn next_record(&mut self) -> [u32; 12] {
        let base = SCRIPT + 4 * RECORD * self.record;
        self.record += 1;
        std::array::from_fn(|i| self.case.input(base + 4 * i as u32))
    }

    fn expect(&mut self, callee: u32) -> Vec<u32> {
        let call = self
            .case
            .calls
            .get(self.call)
            .unwrap_or_else(|| panic!("model made an extra call to {callee:#x}"));
        assert_eq!(call.callee, callee, "callee #{}", self.call);
        self.call += 1;
        *self.counts.entry(callee).or_default() += 1;
        call.args.clone()
    }
}

fn floats(words: &[u32]) -> [u32; 3] {
    [words[0], words[1], words[2]]
}

fn apply(best: &mut BestHit, record: &[u32; 12]) {
    best.point = [1, 2, 3].map(|i| f32::from_bits(record[i]));
    best.aux = [4, 5, 6].map(|i| f32::from_bits(record[i]));
    best.limit = f32::from_bits(record[7]);
}

fn best_words(best: &BestHit) -> Vec<u32> {
    let mut words: Vec<u32> = best
        .point
        .iter()
        .chain(&best.aux)
        .map(|v| v.to_bits())
        .collect();
    words.push(best.limit.to_bits());
    words
}

impl Callees7a00 for Replay<'_> {
    fn mesh_ray(
        &mut self,
        class: u32,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
    ) -> Option<([f32; 3], [f32; 3])> {
        let args = self.expect(0x919D0);
        assert_eq!((args[0], args[3], args[6]), (class, flag, 0));
        assert_eq!(floats(&args[7..10]), bits3(from));
        assert_eq!(floats(&args[10..13]), bits3(vec));
        let record = self.next_record();
        (record[0] != 0).then(|| {
            (
                [1, 2, 3].map(|i| f32::from_bits(record[i])),
                [4, 5, 6].map(|i| f32::from_bits(record[i])),
            )
        })
    }

    fn note_mesh_hit(&mut self) {
        self.expect(0x91390);
    }

    fn buffer_scan(
        &mut self,
        object: &SolidObject,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
        extra2: u32,
        best: &mut BestHit,
    ) -> bool {
        let args = self.expect(0x128C60);
        assert_eq!((args[0], args[3], args[4]), (object.address, flag, extra2));
        assert_eq!(floats(&args[8..11]), bits3(from));
        assert_eq!(floats(&args[11..14]), bits3(vec));
        assert_eq!(&args[14..21], &best_words(best)[..], "best before 0x128C60");
        let record = self.next_record();
        if record[0] != 0 {
            apply(best, &record);
        }
        record[0] != 0
    }

    fn dynamic_scan(
        &mut self,
        _object: &SolidObject,
        call: &DynamicScan,
        best: &mut BestHit,
    ) -> u32 {
        let args = self.expect(0xA2CD0);
        assert_eq!(
            (args[0], args[5], args[6], args[10], args[11]),
            (
                call.object,
                call.flag,
                call.current,
                call.extra1,
                call.extra2
            )
        );
        assert_eq!(floats(&args[12..15]), bits3(call.from));
        assert_eq!(floats(&args[15..18]), bits3(call.vec));
        assert_eq!(floats(&args[18..21]), bits3(call.direction), "direction");
        assert_eq!(floats(&args[21..24]), bits3(call.end));
        assert_eq!(&args[24..31], &best_words(best)[..], "best before 0xA2CD0");
        let record = self.next_record();
        if record[0] != 0 {
            apply(best, &record);
            record[8]
        } else {
            call.current
        }
    }
}

fn a7a00_probe(case: &Case) -> CellProbe {
    CellProbe {
        cell: arg(case, 0),
        from: case.vec3(arg(case, 1)),
        vec: case.vec3(arg(case, 2)),
        flag: arg(case, 3),
        extra: [arg(case, 6), arg(case, 7), arg(case, 8)],
        variant_a7a00: true,
    }
}

#[test]
fn cell_test_a7a00_matches_original() {
    let mut totals: BTreeMap<u32, usize> = BTreeMap::new();
    let (mut hits, mut misses, mut wall_scans, mut buffers, mut invalid_cells) = (0, 0, 0, 0, 0);
    for case in load("sweep_cells_a7a00") {
        let solids = load_solids(&case);
        let probe = a7a00_probe(&case);
        let mut replay = Replay {
            case: &case,
            record: 0,
            call: 0,
            counts: BTreeMap::new(),
        };
        let result = cell_test_a7a00(&solids, &mut replay, &probe);
        assert_eq!(replay.call, case.calls.len(), "all original calls replayed");
        for (callee, count) in &replay.counts {
            *totals.entry(*callee).or_default() += count;
        }
        assert_eq!(case.eax, result.ret, "returned object");
        let [out_p, out_a] = [4, 5].map(|i| arg(&case, i));
        let mut writes = BTreeMap::new();
        if result.ret != 0 {
            hits += 1;
            if out_p != 0 {
                put3(&mut writes, out_p, result.point);
            }
            if out_a != 0 {
                put3(&mut writes, out_a, result.aux);
            }
        } else {
            misses += 1;
        }
        wall_scans += usize::from(probe.extra[1] & 0x40 != 0);
        buffers += usize::from(probe.extra[2] != 0);
        invalid_cells += usize::from(probe.cell as i32 <= 0 || probe.cell as i32 > solids.max_slot);
        check_outs(&case, &writes);
    }
    for callee in [0x919D0, 0x91390, 0x128C60, 0xA2CD0] {
        assert!(
            totals.get(&callee).copied().unwrap_or(0) >= 5,
            "{callee:#x} {totals:?}"
        );
    }
    assert!(hits >= 20 && misses >= 8, "hits {hits} misses {misses}");
    assert!(wall_scans >= 20 && buffers >= 10 && invalid_cells >= 3);
}

// ---------------------------------------------------------------- 0x49C60, 0x128C60 and the small globals
fn load_buffer(case: &Case, base: u32) -> HitBuffer {
    let count = case.input(base);
    HitBuffer {
        count,
        flag: case.input(base + 4),
        records: (0..count.min(HIT_CAPACITY))
            .map(|k| {
                let at = base + 8 + 0x34 * k;
                HitRecord {
                    object: case.input(at),
                    point: case.vec3(at + 4),
                    vec: case.vec3(at + 0x10),
                    normal: case.vec3(at + 0x1C),
                    d2: case.input_f(at + 0x28),
                    cell: case.input(at + 0x2C),
                    record: case.input(at + 0x30),
                }
            })
            .collect(),
    }
}

/// Live words of a buffer (count, flag and the records below `count`).
fn buffer_words(base: u32, buffer: &HitBuffer, writes: &mut BTreeMap<u32, u32>) {
    writes.insert(base, buffer.count);
    writes.insert(base + 4, buffer.flag);
    for (k, record) in buffer.records.iter().enumerate() {
        let at = base + 8 + 0x34 * k as u32;
        writes.insert(at, record.object);
        put3(writes, at + 4, record.point);
        put3(writes, at + 0x10, record.vec);
        put3(writes, at + 0x1C, record.normal);
        writes.insert(at + 0x28, record.d2.to_bits());
        writes.insert(at + 0x2C, record.cell);
        writes.insert(at + 0x30, record.record);
    }
}

#[test]
fn box_ray_matches_original() {
    let (mut hits, mut plane_only, mut misses) = (0, 0, 0);
    for case in load("sweep_cells_49c60") {
        let [from, vec, limit, record, out_p, out_d] = [0, 1, 2, 3, 4, 5].map(|i| arg(&case, i));
        let hit = box_ray_49c60(
            case.vec3(from),
            case.vec3(vec),
            f32::from_bits(limit),
            &load_box(&case, record),
        );
        assert_eq!(case.eax, u32::from(hit.point.is_some()));
        let mut writes = BTreeMap::new();
        if let (Some(d2), true) = (hit.d2, out_d != 0) {
            writes.insert(out_d, d2.to_bits());
        }
        match (hit.point, hit.d2) {
            (Some(point), _) => {
                hits += 1;
                if out_p != 0 {
                    put3(&mut writes, out_p, point);
                }
            }
            (None, Some(_)) => plane_only += 1,
            (None, None) => misses += 1,
        }
        check_outs(&case, &writes);
    }
    assert!(
        hits >= 10 && plane_only >= 8 && misses >= 2,
        "{hits} {plane_only} {misses}"
    );
}

#[test]
fn buffer_scan_matches_original() {
    let (mut accepted, mut rejected, mut full, mut portal_checks, mut flagged) = (0, 0, 0, 0, 0);
    for case in load("sweep_cells_128c60") {
        let graph = load_graph(&case);
        let object = load_object(&case, arg(&case, 0));
        let [out_p, out_a, limit_p] = [5, 6, 7].map(|i| arg(&case, i));
        let mut buffer = load_buffer(&case, BUF);
        let before = buffer.clone();
        let mut globals = ImpactGlobals::default();
        let mut point = case.vec3(out_p);
        let mut aux = case.vec3(out_a);
        let mut limit = case.input_f(limit_p);
        let mut targets = ScanTargets {
            point: (out_p != 0).then_some(&mut point),
            aux: (out_a != 0).then_some(&mut aux),
            limit: &mut limit,
        };
        let result = buffer_scan_128c60(
            &object,
            &graph,
            case.vec3(arg(&case, 1)),
            case.vec3(arg(&case, 2)),
            &mut buffer,
            &mut globals,
            &mut targets,
        );
        assert_eq!(case.eax, u32::from(result), "returned flag");
        let mut writes = BTreeMap::new();
        buffer_words(BUF, &buffer, &mut writes);
        if result {
            accepted += 1;
            if out_p != 0 {
                put3(&mut writes, out_p, point);
            }
            if out_a != 0 {
                put3(&mut writes, out_a, aux);
            }
            writes.insert(limit_p, limit.to_bits());
            for (k, word) in globals.scratch.iter().enumerate() {
                writes.insert(0x7A_C5A0 + 4 * k as u32, *word);
            }
            for (k, word) in globals.saved.iter().enumerate() {
                writes.insert(0x70_AAAC + 4 * k as u32, *word);
            }
        } else {
            rejected += 1;
        }
        full += usize::from(buffer.count == HIT_CAPACITY);
        flagged += usize::from(buffer.flag != before.flag);
        portal_checks += object
            .boxes
            .iter()
            .filter(|b| b.flags & 2 != 0)
            .count()
            .min(1);
        check_outs(&case, &writes);
    }
    assert!(
        accepted >= 10 && rejected >= 8 && portal_checks >= 8,
        "{accepted} {rejected} {portal_checks}"
    );
    assert!(full >= 1 && flagged >= 1, "{full} {flagged}");
}

#[test]
fn small_globals_match_original() {
    let mut kinds = [0usize; 4];
    for case in load("sweep_cells_globals") {
        let which = case.input(WHICH);
        kinds[which as usize] += 1;
        let mut writes = BTreeMap::new();
        let words =
            |base: u32| -> [u32; 8] { std::array::from_fn(|k| case.input(base + 4 * k as u32)) };
        let mut globals = ImpactGlobals {
            current: words(0x70_AA58),
            saved: words(0x70_AAAC),
            shape_source: case.input(0x73_56E0),
            shape_saved: case.input(0x73_56DC),
            ..Default::default()
        };
        match which {
            0 => {
                globals.snapshot_91390();
            }
            1 => globals.snapshot_913b0(words(TRI)),
            2 => globals.snapshot_d6a70(),
            _ => {
                let mut buffer = load_buffer(&case, BUF);
                compact_128e90(&mut buffer, f32::from_bits(arg(&case, 1)));
                buffer_words(BUF, &buffer, &mut writes);
            }
        }
        if which < 3 {
            for (k, word) in globals.saved.iter().enumerate() {
                writes.insert(0x70_AAAC + 4 * k as u32, *word);
            }
            writes.insert(0x73_56DC, globals.shape_saved);
        }
        // After a compaction the records above `count` are stale: compare the live words only.
        if which == 3 {
            let live: BTreeMap<u32, u32> = case
                .outs
                .iter()
                .filter(|(address, _)| writes.contains_key(address))
                .map(|(a, v)| (*a, *v))
                .collect();
            assert_eq!(live.len(), writes.len());
            for (address, want) in &writes {
                assert_eq!(live[address], *want, "memory {address:#x}");
            }
        } else {
            check_outs(&case, &writes);
        }
    }
    assert!(kinds.iter().all(|&n| n >= 12), "{kinds:?}");
}

// ---------------------------------------------------------------- 0xA2CD0 (0x919D0 and 0xD9760 replayed)
struct ScanReplay<'a> {
    case: &'a Case,
    record: u32,
    call: usize,
    meshes: usize,
    shapes: usize,
}

impl ScanReplay<'_> {
    fn answer(&mut self, callee: u32) -> (Vec<u32>, Option<([f32; 3], [f32; 3])>) {
        let call = self
            .case
            .calls
            .get(self.call)
            .unwrap_or_else(|| panic!("model made an extra call to {callee:#x}"));
        assert_eq!(call.callee, callee, "callee #{}", self.call);
        self.call += 1;
        let base = SCRIPT + 4 * RECORD * self.record;
        self.record += 1;
        let word = |i: u32| self.case.input(base + 4 * i);
        let hit = (word(0) != 0).then(|| {
            (
                [1, 2, 3].map(|i| f32::from_bits(word(i))),
                [4, 5, 6].map(|i| f32::from_bits(word(i))),
            )
        });
        (call.args.clone(), hit)
    }
}

impl ScanCallees for ScanReplay<'_> {
    fn mesh_ray(
        &mut self,
        _globals: &mut ImpactGlobals,
        class: u32,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
        extra1: u32,
    ) -> Option<([f32; 3], [f32; 3])> {
        let (args, hit) = self.answer(0x919D0);
        self.meshes += 1;
        assert_eq!((args[0], args[3], args[6]), (class, flag, extra1));
        assert_eq!(floats(&args[7..10]), bits3(from));
        assert_eq!(floats(&args[10..13]), bits3(vec));
        hit
    }

    fn shape_scan(
        &mut self,
        _globals: &mut ImpactGlobals,
        object: u32,
        from: [f32; 3],
        direction: [f32; 3],
        end: [f32; 3],
        flag: u32,
        extra1: u32,
    ) -> Option<([f32; 3], [f32; 3])> {
        let (args, hit) = self.answer(0xD9760);
        self.shapes += 1;
        assert_eq!((args[0], args[4], args[7]), (object, flag, extra1));
        assert_eq!(floats(&args[8..11]), bits3(from));
        assert_eq!(floats(&args[11..14]), bits3(direction));
        assert_eq!(floats(&args[14..17]), bits3(end));
        hit
    }
}

const BESTP: u32 = 0x158_0000;
const BESTA: u32 = 0x158_0100;
const DIRP: u32 = 0x158_0300;
const ENDP: u32 = 0x158_0400;

fn call_a2cd0(
    case: &Case,
    graph: &CellGraph,
    replay: &mut ScanReplay,
) -> (u32, BestHit, HitBuffer, ImpactGlobals, Option<ObjectLinks>) {
    let object = load_object(case, arg(case, 0));
    let current = arg(case, 6);
    let links = (current != 0).then(|| ObjectLinks {
        class_address: case.input(current + 4),
        link: case.input(current + 0x78),
    });
    let words =
        |base: u32| -> [u32; 8] { std::array::from_fn(|k| case.input(base + 4 * k as u32)) };
    let mut globals = ImpactGlobals {
        current: words(0x70_AA58),
        saved: words(0x70_AAAC),
        scratch: words(0x7A_C5A0),
        shape_source: case.input(0x73_56E0),
        shape_saved: case.input(0x73_56DC),
    };
    let mut best = BestHit {
        point: case.vec3(BESTP),
        aux: case.vec3(BESTA),
        limit: case.input_f(LIMITP),
    };
    let extra2 = arg(case, 11);
    let mut buffer = load_buffer(case, BUF);
    let call = DynamicScan {
        object: arg(case, 0),
        from: case.vec3(arg(case, 1)),
        vec: case.vec3(arg(case, 2)),
        direction: case.vec3(DIRP),
        end: case.vec3(ENDP),
        flag: arg(case, 5),
        current,
        extra1: arg(case, 10),
        extra2,
    };
    let result = dynamic_scan_a2cd0(
        graph,
        &mut globals,
        replay,
        &object,
        links,
        &call,
        &mut best,
        (extra2 != 0).then_some(&mut buffer),
    );
    (result, best, buffer, globals, links)
}

#[test]
fn dynamic_scan_matches_original() {
    let (mut meshes, mut shapes, mut accepted, mut early, mut raised, mut boxes, mut compacted) =
        (0, 0, 0, 0, 0, 0, 0);
    for case in load("sweep_cells_a2cd0") {
        let graph = load_graph(&case);
        let mut replay = ScanReplay {
            case: &case,
            record: 0,
            call: 0,
            meshes: 0,
            shapes: 0,
        };
        let (result, best, buffer, globals, links) = call_a2cd0(&case, &graph, &mut replay);
        assert_eq!(replay.call, case.calls.len(), "all original calls replayed");
        assert_eq!(case.eax, result, "returned object");
        meshes += replay.meshes;
        shapes += replay.shapes;
        accepted += usize::from(result == arg(&case, 0));
        early += usize::from(replay.call == 0 && result == arg(&case, 6));
        raised +=
            usize::from(links.is_some_and(|l| l.class_address == case.input(arg(&case, 0) + 0x78)));
        let mut writes = BTreeMap::new();
        put3(&mut writes, BESTP, best.point);
        put3(&mut writes, BESTA, best.aux);
        writes.insert(LIMITP, best.limit.to_bits());
        for (k, word) in globals.saved.iter().enumerate() {
            writes.insert(0x70_AAAC + 4 * k as u32, *word);
        }
        for (k, word) in globals.scratch.iter().enumerate() {
            writes.insert(0x7A_C5A0 + 4 * k as u32, *word);
        }
        writes.insert(0x73_56DC, globals.shape_saved);
        if arg(&case, 11) != 0 {
            let before = load_buffer(&case, BUF);
            compacted += usize::from(buffer.count < before.count);
            boxes += usize::from(load_object(&case, arg(&case, 0)).boxes.len() > 0);
            buffer_words(BUF, &buffer, &mut writes);
        }
        let buffer_range = BUF..BUF + 8 + 0x34 * HIT_CAPACITY;
        for (&address, &actual) in &case.outs {
            if buffer_range.contains(&address) && !writes.contains_key(&address) {
                continue; // stale tail of a compacted buffer
            }
            let want = writes
                .get(&address)
                .copied()
                .unwrap_or_else(|| case.input(address));
            assert_eq!(actual, want, "memory {address:#x}");
        }
    }
    assert!(
        meshes >= 20 && shapes >= 8,
        "meshes {meshes} shapes {shapes}"
    );
    assert!(
        accepted >= 8 && early >= 2 && raised >= 2,
        "{accepted} {early} {raised}"
    );
    assert!(
        boxes >= 10 && compacted >= 1,
        "boxes {boxes} compacted {compacted}"
    );
}

// ---------------------------------------------------------------- 0xA7A00 with every ported callee
#[test]
fn cell_test_a7a00_with_ported_callees_matches_original() {
    let (mut hits, mut misses, mut boxes, mut compacted, mut shapes, mut meshes) =
        (0, 0, 0, 0, 0, 0);
    for case in load("sweep_cells_a7a00_full") {
        let graph = load_graph(&case);
        let solids = load_solids(&case);
        let probe = a7a00_probe(&case);
        let extra2 = probe.extra[2];
        let words =
            |base: u32| -> [u32; 8] { std::array::from_fn(|k| case.input(base + 4 * k as u32)) };
        let mut globals = ImpactGlobals {
            current: words(0x70_AA58),
            saved: words(0x70_AAAC),
            scratch: words(0x7A_C5A0),
            shape_source: case.input(0x73_56E0),
            shape_saved: case.input(0x73_56DC),
        };
        let mut buffers = BTreeMap::new();
        if extra2 != 0 {
            buffers.insert(extra2, load_buffer(&case, BUF));
        }
        let before = buffers.get(&extra2).cloned();
        let mut replay = ScanReplay {
            case: &case,
            record: 0,
            call: 0,
            meshes: 0,
            shapes: 0,
        };
        let mut scanner = CellScanner {
            graph: &graph,
            world: &solids,
            globals: &mut globals,
            geometry: &mut replay,
            buffers: &mut buffers,
        };
        let result = cell_test_a7a00(&solids, &mut scanner, &probe);
        assert_eq!(replay.call, case.calls.len(), "all original calls replayed");
        assert_eq!(case.eax, result.ret, "returned object");
        meshes += replay.meshes;
        shapes += replay.shapes;
        let mut writes = BTreeMap::new();
        let [out_p, out_a] = [4, 5].map(|i| arg(&case, i));
        if result.ret != 0 {
            hits += 1;
            if out_p != 0 {
                put3(&mut writes, out_p, result.point);
            }
            if out_a != 0 {
                put3(&mut writes, out_a, result.aux);
            }
        } else {
            misses += 1;
        }
        for (k, word) in globals.saved.iter().enumerate() {
            writes.insert(0x70_AAAC + 4 * k as u32, *word);
        }
        for (k, word) in globals.scratch.iter().enumerate() {
            writes.insert(0x7A_C5A0 + 4 * k as u32, *word);
        }
        writes.insert(0x73_56DC, globals.shape_saved);
        if let (Some(after), Some(before)) = (buffers.get(&extra2), before) {
            boxes += usize::from(after.count != before.count || after.flag != before.flag);
            compacted += usize::from(after.count < before.count);
            buffer_words(BUF, after, &mut writes);
        }
        let buffer_range = BUF..BUF + 8 + 0x34 * HIT_CAPACITY;
        for (&address, &actual) in &case.outs {
            if buffer_range.contains(&address) && !writes.contains_key(&address) {
                continue; // stale tail of a compacted buffer
            }
            let want = writes
                .get(&address)
                .copied()
                .unwrap_or_else(|| case.input(address));
            assert_eq!(actual, want, "memory {address:#x}");
        }
    }
    assert!(hits >= 15 && misses >= 8, "hits {hits} misses {misses}");
    assert!(
        meshes >= 30 && shapes >= 5,
        "meshes {meshes} shapes {shapes}"
    );
    assert!(
        boxes >= 3,
        "box scans that touched the buffer: {boxes} (compacted {compacted})"
    );
}

// ---------------------------------------------------------------- mesh ray: 0x47B30, 0x47BD0, 0x47D50, 0x919D0
struct CaseMemory<'a>(&'a BTreeMap<u32, u32>);

impl GuestMemory for CaseMemory<'_> {
    fn read_u8(&self, address: u32) -> u8 {
        let word = self.0.get(&(address & !3)).copied().unwrap_or(0);
        (word >> ((address & 3) * 8)) as u8
    }
}

const RAY: u32 = 0x16F_1000;
const BOX: u32 = 0x16F_1100;
const MATRIX: u32 = 0x16F_0000;

fn load_ray(case: &Case) -> Ray {
    Ray {
        origin: case.vec3(RAY),
        direction: case.vec3(RAY + 12),
        inverse: case.vec3(RAY + 24),
        length: case.input_f(RAY + 36),
    }
}

fn ray_words(ray: &Ray, writes: &mut BTreeMap<u32, u32>) {
    put3(writes, RAY, ray.origin);
    put3(writes, RAY + 12, ray.direction);
    put3(writes, RAY + 24, ray.inverse);
    writes.insert(RAY + 36, ray.length.to_bits());
}

#[test]
fn ray_init_matches_original() {
    let mut zero_axes = 0;
    for case in load("sweep_mesh_47b30") {
        let [ray_ptr, from, vec, length] = [0, 1, 2, 3].map(|i| arg(&case, i));
        assert_eq!(ray_ptr, RAY);
        let ray = ray_init_47b30(case.vec3(from), case.vec3(vec), f32::from_bits(length));
        zero_axes += ray.direction.iter().filter(|d| **d == 0.0).count();
        let mut writes = BTreeMap::new();
        ray_words(&ray, &mut writes);
        check_outs(&case, &writes);
    }
    assert!(zero_axes >= 10);
}

#[test]
fn ray_box_matches_original() {
    let (mut hits, mut misses, mut parallel) = (0, 0, 0);
    for case in load("sweep_mesh_47bd0") {
        let ray = load_ray(&case);
        let bounds = [0, 1, 2, 3, 4, 5].map(|k| case.input_f(BOX + 4 * k));
        let result = ray_box_47bd0(&ray, &bounds);
        assert_eq!(case.eax, u32::from(result));
        hits += usize::from(result);
        misses += usize::from(!result);
        parallel += usize::from(ray.direction.iter().any(|d| *d == 0.0));
    }
    assert!(
        hits >= 15 && misses >= 15 && parallel >= 10,
        "{hits} {misses} {parallel}"
    );
}

#[test]
fn triangle_entry_matches_original() {
    let (mut hits, mut quantised_hits, mut float_hits, mut culled, mut double_sided) =
        (0, 0, 0, 0, 0);
    for case in load("sweep_mesh_47d50") {
        let [
            ray_ptr,
            vertices,
            bounds,
            base,
            quant,
            entry,
            cull,
            out_p,
            out_n,
        ] = [0, 1, 2, 3, 4, 5, 6, 7, 8].map(|i| arg(&case, i));
        assert_eq!(ray_ptr, RAY);
        let memory = CaseMemory(&case.ins);
        let mut ray = load_ray(&case);
        let result = ray_entry_47d50(
            &memory,
            &mut ray,
            vertices,
            bounds,
            base,
            quant != 0,
            entry,
            cull != 0,
        );
        assert_eq!(case.eax, u32::from(result.is_some()));
        let mut writes = BTreeMap::new();
        if let Some(hit) = result {
            hits += 1;
            quantised_hits += usize::from(quant != 0);
            float_hits += usize::from(quant == 0);
            culled += usize::from(cull != 0);
            double_sided += usize::from(cull == 0);
            put3(&mut writes, out_p, hit.position);
            put3(&mut writes, out_n, hit.normal);
            ray_words(&ray, &mut writes);
        }
        check_outs(&case, &writes);
    }
    assert!(hits >= 15, "hits {hits}");
    assert!(quantised_hits >= 3 && float_hits >= 3 && culled >= 3 && double_sided >= 3);
}

#[test]
fn mesh_ray_matches_original() {
    let (mut hits, mut misses, mut moved, mut per_record, mut quantised, mut boxes, mut early) =
        (0, 0, 0, 0, 0, 0, 0);
    for case in load("sweep_mesh_919d0") {
        let memory = CaseMemory(&case.ins);
        let [class, from, vec, flag, out_p, out_a, extra] =
            [0, 1, 2, 3, 4, 5, 6].map(|i| arg(&case, i));
        let object = memory.read_u32(class);
        let mesh = memory.read_u32(class + 4);
        let mut matrix_calls = 0;
        let mut world_matrix = |address: u32| -> Mat4 {
            matrix_calls += 1;
            assert_eq!(address, object);
            std::array::from_fn(|k| case.input_f(MATRIX + 4 * k as u32))
        };
        let mut globals = ImpactGlobals {
            current: std::array::from_fn(|k| case.input(0x70_AA58 + 4 * k as u32)),
            ..Default::default()
        };
        let result = mesh_ray_919d0(
            &memory,
            &mut world_matrix,
            &mut globals,
            class,
            case.vec3(from),
            case.vec3(vec),
            flag,
            extra,
        );
        let moves = memory.read_u8(object + 0x32) != 0;
        let walked = case.calls.len();
        assert_eq!(walked, matrix_calls, "0xA14B0 calls");
        for call in &case.calls {
            assert_eq!(call.callee, 0xA14B0);
            assert_eq!((call.args[1], call.args[2], call.args[3]), (object, 1, 0));
        }
        assert!(moves || matrix_calls == 0);
        assert_eq!(case.eax, u32::from(result.hit), "return value");
        let mut writes = BTreeMap::new();
        if let Some(point) = result.point {
            put3(&mut writes, out_p, point);
        }
        if let Some(aux) = result.aux {
            put3(&mut writes, out_a, aux);
        }
        for (k, word) in globals.current.iter().enumerate() {
            writes.insert(0x70_AA58 + 4 * k as u32, *word);
        }
        check_outs(&case, &writes);
        if result.hit {
            hits += 1;
            moved += usize::from(moves);
            per_record += usize::from(memory.read_u32(mesh + 4) as i32 > 0);
            quantised += usize::from(memory.read_u8(mesh + 0xC) & 0x10 != 0);
            boxes += usize::from(memory.read_u8(mesh + 0xC) & 0x80 != 0);
            early += usize::from(result.point.is_none());
        } else {
            misses += 1;
        }
    }
    assert!(hits >= 15 && misses >= 10, "hits {hits} misses {misses}");
    assert!(
        moved >= 3 && per_record >= 3 && quantised >= 3 && boxes >= 3,
        "{moved} {per_record} {quantised} {boxes}"
    );
    let _ = early;
}

// ---------------------------------------------------------------- 0xA7A00 with the real mesh ray
/// Real `0x919D0` over the golden memory (its `0xA14B0` matrix comes from the call log); `0xD9760` is replayed.
struct MeshReplay<'a> {
    case: &'a Case,
    memory: CaseMemory<'a>,
    record: u32,
    call: usize,
    mesh_rays: usize,
    mesh_hits: usize,
    shapes: usize,
}

impl ScanCallees for MeshReplay<'_> {
    fn mesh_ray(
        &mut self,
        globals: &mut ImpactGlobals,
        class: u32,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
        extra1: u32,
    ) -> Option<([f32; 3], [f32; 3])> {
        self.mesh_rays += 1;
        let case = self.case;
        let call_index = &mut self.call;
        let mut world_matrix = |object: u32| -> Mat4 {
            let call = case
                .calls
                .get(*call_index)
                .expect("model made an extra 0xA14B0 call");
            assert_eq!(call.callee, 0xA14B0);
            assert_eq!((call.args[1], call.args[2], call.args[3]), (object, 1, 0));
            *call_index += 1;
            std::array::from_fn(|k| case.input_f(MATRIX + 4 * k as u32))
        };
        let result = mesh_ray_919d0(
            &self.memory,
            &mut world_matrix,
            globals,
            class,
            from,
            vec,
            flag,
            extra1,
        );
        self.mesh_hits += usize::from(result.hit);
        match (result.hit, result.point, result.aux) {
            (true, Some(point), Some(aux)) => Some((point, aux)),
            (true, _, _) => panic!("mesh ray returned 1 without outputs"),
            _ => None,
        }
    }

    fn shape_scan(
        &mut self,
        _globals: &mut ImpactGlobals,
        object: u32,
        from: [f32; 3],
        direction: [f32; 3],
        end: [f32; 3],
        flag: u32,
        extra1: u32,
    ) -> Option<([f32; 3], [f32; 3])> {
        self.shapes += 1;
        let mut replay = ScanReplay {
            case: self.case,
            record: self.record,
            call: self.call,
            meshes: 0,
            shapes: 0,
        };
        let hit = replay.shape_scan(
            &mut ImpactGlobals::default(),
            object,
            from,
            direction,
            end,
            flag,
            extra1,
        );
        self.record = replay.record;
        self.call = replay.call;
        hit
    }
}

#[test]
fn cell_test_a7a00_with_real_meshes_matches_original() {
    let (mut hits, mut misses, mut mesh_rays, mut shapes, mut matrices, mut mesh_hits) =
        (0, 0, 0, 0, 0, 0);
    for case in load("sweep_mesh_a7a00") {
        let graph = load_graph(&case);
        let solids = load_solids(&case);
        let probe = a7a00_probe(&case);
        let extra2 = probe.extra[2];
        let words =
            |base: u32| -> [u32; 8] { std::array::from_fn(|k| case.input(base + 4 * k as u32)) };
        let mut globals = ImpactGlobals {
            current: words(0x70_AA58),
            saved: words(0x70_AAAC),
            scratch: words(0x7A_C5A0),
            shape_source: case.input(0x73_56E0),
            shape_saved: case.input(0x73_56DC),
        };
        let mut buffers = BTreeMap::new();
        if extra2 != 0 {
            buffers.insert(extra2, load_buffer(&case, BUF));
        }
        let mut replay = MeshReplay {
            case: &case,
            memory: CaseMemory(&case.ins),
            record: 0,
            call: 0,
            mesh_rays: 0,
            mesh_hits: 0,
            shapes: 0,
        };
        let mut scanner = CellScanner {
            graph: &graph,
            world: &solids,
            globals: &mut globals,
            geometry: &mut replay,
            buffers: &mut buffers,
        };
        let result = cell_test_a7a00(&solids, &mut scanner, &probe);
        assert_eq!(replay.call, case.calls.len(), "all original calls replayed");
        assert_eq!(case.eax, result.ret, "returned object");
        mesh_hits += replay.mesh_hits;
        mesh_rays += replay.mesh_rays;
        shapes += replay.shapes;
        matrices += case.calls.iter().filter(|c| c.callee == 0xA14B0).count();
        let mut writes = BTreeMap::new();
        let [out_p, out_a] = [4, 5].map(|i| arg(&case, i));
        if result.ret != 0 {
            hits += 1;
            if out_p != 0 {
                put3(&mut writes, out_p, result.point);
            }
            if out_a != 0 {
                put3(&mut writes, out_a, result.aux);
            }
        } else {
            misses += 1;
        }
        for (base, block) in [
            (0x70_AA58, globals.current),
            (0x70_AAAC, globals.saved),
            (0x7A_C5A0, globals.scratch),
        ] {
            for (k, word) in block.iter().enumerate() {
                writes.insert(base + 4 * k as u32, *word);
            }
        }
        writes.insert(0x73_56DC, globals.shape_saved);
        if let Some(after) = buffers.get(&extra2) {
            buffer_words(BUF, after, &mut writes);
        }
        let buffer_range = BUF..BUF + 8 + 0x34 * HIT_CAPACITY;
        for (&address, &actual) in &case.outs {
            if buffer_range.contains(&address) && !writes.contains_key(&address) {
                continue; // stale tail of a compacted buffer
            }
            let want = writes
                .get(&address)
                .copied()
                .unwrap_or_else(|| case.input(address));
            assert_eq!(actual, want, "memory {address:#x}");
        }
    }
    assert!(hits >= 3 && misses >= 8, "hits {hits} misses {misses}");
    assert!(
        mesh_rays >= 20 && mesh_hits >= 2 && shapes >= 3 && matrices >= 3,
        "{mesh_rays} {mesh_hits} {shapes} {matrices}"
    );
}

// ---------------------------------------------------------------- 0x88950 with `use_a7a00` and the real cell code
#[test]
fn sweep_driver_a7a00_with_real_cells_matches_original() {
    let (mut hits, mut misses, mut multi, mut production, mut mesh_rays) = (0, 0, 0, 0, 0);
    for case in load("sweep_mesh_driver") {
        let graph = load_graph(&case);
        let solids = load_solids(&case);
        let words =
            |base: u32| -> [u32; 8] { std::array::from_fn(|k| case.input(base + 4 * k as u32)) };
        let mut globals = ImpactGlobals {
            current: words(0x70_AA58),
            saved: words(0x70_AAAC),
            scratch: words(0x7A_C5A0),
            shape_source: case.input(0x73_56E0),
            shape_saved: case.input(0x73_56DC),
        };
        let extra = [arg(&case, 6), arg(&case, 7), arg(&case, 8)];
        let mut buffers = BTreeMap::new();
        if extra[2] != 0 {
            buffers.insert(extra[2], load_buffer(&case, BUF));
        }
        let mut replay = MeshReplay {
            case: &case,
            memory: CaseMemory(&case.ins),
            record: 0,
            call: 0,
            mesh_rays: 0,
            mesh_hits: 0,
            shapes: 0,
        };
        let mut scanner = CellScanner {
            graph: &graph,
            world: &solids,
            globals: &mut globals,
            geometry: &mut replay,
            buffers: &mut buffers,
        };
        let mut geometry = SweepCells {
            graph: &graph,
            solids: &solids,
            callees_a7a00: Some(&mut scanner),
        };
        let with_cell = arg(&case, 4) != 0;
        let args = SweepArgs {
            cell: arg(&case, 9),
            from: case.vec3(FROM),
            vec: case.vec3(VEC),
            use_a7a00: true,
            flag: arg(&case, 1),
            extra,
        };
        let mut io = SweepIo {
            position: Some(case.vec3(OUTP)),
            direction: Some([OUTD, OUTD + 4, OUTD + 8].map(|a| case.input_f(a))),
            cell: with_cell.then(|| case.input(OUTC)),
            hit: None,
            ..Default::default()
        };
        let ret = sweep_88950(&mut geometry, &args, &mut io).expect("no error");
        assert_eq!(replay.call, case.calls.len(), "all original calls replayed");
        assert_eq!(ret, case.eax, "return value");
        mesh_rays += replay.mesh_rays;
        let mut writes = BTreeMap::new();
        put3(&mut writes, OUTP, io.position.unwrap());
        put3(&mut writes, OUTD, io.direction.unwrap());
        if let Some(cell) = io.cell {
            writes.insert(OUTC, cell);
        }
        for (base, block) in [
            (0x70_AA58, globals.current),
            (0x70_AAAC, globals.saved),
            (0x7A_C5A0, globals.scratch),
        ] {
            for (k, word) in block.iter().enumerate() {
                writes.insert(base + 4 * k as u32, *word);
            }
        }
        writes.insert(0x73_56DC, globals.shape_saved);
        if let Some(after) = buffers.get(&extra[2]) {
            buffer_words(BUF, after, &mut writes);
        }
        let buffer_range = BUF..BUF + 8 + 0x34 * HIT_CAPACITY;
        for (&address, &actual) in &case.outs {
            if buffer_range.contains(&address) && !writes.contains_key(&address) {
                continue;
            }
            let want = writes
                .get(&address)
                .copied()
                .unwrap_or_else(|| case.input(address));
            assert_eq!(actual, want, "memory {address:#x}");
        }
        production += usize::from(extra == [0xFFFF_FFFD, 0, 0]);
        if ret != 0 {
            hits += 1;
        } else {
            misses += 1;
        }
        multi += usize::from(io.cell.is_some_and(|cell| cell != args.cell));
    }
    assert!(
        hits >= 3 && misses >= 8 && multi >= 5,
        "hits {hits} misses {misses} multi {multi}"
    );
    assert!(
        production >= 15 && mesh_rays >= 10,
        "production {production} mesh rays {mesh_rays}"
    );
}
