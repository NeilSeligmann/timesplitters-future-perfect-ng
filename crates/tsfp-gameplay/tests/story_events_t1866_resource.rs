//! T1866: the resource and file layer (`svc_resource`) against golden vectors from the original
//! machine code under the Unicorn oracle (MEASURED). Goldens: `tests/data/t1866/res_*.golden`
//! (generator `tools/t1846_gold/svc_resource.py`).
mod common;
use common::{Case, parse};
use std::collections::{BTreeMap, BTreeSet};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;
use tsfp_gameplay::story_events::svc_pool::{model_find, model_load_a, model_load_b, object_shape};
use tsfp_gameplay::story_events::svc_resource::*;
use tsfp_gameplay::story_events::svc_shape::*;

const SELECT: u32 = 0x2FF_F000;
const STACK_ARGS: u32 = 0x2FF_F100;
const RETS: u32 = 0x3000_0000;
const BUSY: u32 = 0x3100_0000;
const POKES: u32 = 0x3400_0000;
const REGS: u32 = 0x2FF_F140;
const STACK_TOP: u32 = 0x7F00_0000;

/// esp at entry of a call with `nargs` cdecl arguments in the oracle.
fn entry_of(nargs: u32) -> u32 {
    STACK_TOP - 0x1000 - 4 * nargs - 4
}

fn load(name: &str, minimum: usize) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1866/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    );
    let cases = parse(&std::fs::read_to_string(path).expect("golden file"));
    assert!(cases.len() >= minimum, "{name}: too few cases");
    cases
}

struct Mem {
    words: BTreeMap<u32, u32>,
}

impl GuestMemory for Mem {
    fn read_u8(&self, address: u32) -> u8 {
        let word = self.words.get(&(address & !3)).copied().unwrap_or(0);
        (word >> (8 * (address & 3))) as u8
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        let shift = 8 * (address & 3);
        let word = self.words.entry(address & !3).or_insert(0);
        *word = (*word & !(0xFF << shift)) | (u32::from(value) << shift);
    }
    fn read_u32(&self, address: u32) -> u32 {
        if address & 3 == 0 {
            return self.words.get(&address).copied().unwrap_or(0);
        }
        (0..4).fold(0, |acc, k| {
            acc | u32::from(self.read_u8(address.wrapping_add(k))) << (8 * k)
        })
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        if address & 3 == 0 {
            self.words.insert(address, value);
        } else {
            for k in 0..4 {
                self.write_u8(address.wrapping_add(k), (value >> (8 * k)) as u8);
            }
        }
    }
}

/// Replays the stubbed engine calls of the golden: callee and arguments are checked in order (pointed to words
/// of `0x873C0` too), side effects of the stub (`POKES` table) are applied, the result is `RETS + callee`
/// (`BUSY + 4 * index` for `0x9E3C0`).
struct Recorded<'a> {
    case: &'a Case,
    next: usize,
    reached: BTreeSet<u32>,
}

impl DestroyEffects for Recorded<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        let mut expected = self
            .case
            .calls
            .get(self.next)
            .unwrap_or_else(|| panic!("extra call {address:#x} {args:x?}"))
            .clone();
        if address == 0x8FE30 {
            // args logged as [stack word, word at the eax local, eax]: the port passes [stack word, local]
            assert_eq!(
                memory.read_u32(args[1]),
                expected.args[1],
                "call #{} local",
                self.next
            );
            expected.args.remove(1);
            // T1869: the call now also passes the slot (esi)
            expected.args.push(expected.regs[3]);
        }
        let expected = &expected;
        assert_eq!(expected.callee, address, "call #{}", self.next);
        assert!(
            args.len() <= expected.args.len(),
            "call #{} {address:#x}: {args:x?} vs {:x?}",
            self.next,
            expected.args
        );
        assert_eq!(
            expected.args[..args.len()],
            *args,
            "call #{} {address:#x} args",
            self.next
        );
        if address == 0x151E80 {
            let mut words: Vec<u32> = (0..16)
                .map(|k| memory.read_u32(args[0].wrapping_add(4 * k)))
                .collect();
            words.extend((0..3).map(|k| memory.read_u32(args[1].wrapping_add(4 * k))));
            assert_eq!(
                expected.args[2..21],
                words[..],
                "call #{} matrix and point",
                self.next
            );
        }
        if address == 0x873C0 {
            let words: Vec<u32> = (0..6)
                .map(|k| memory.read_u32(args[1].wrapping_add(4 * k)))
                .collect();
            assert_eq!(expected.args[4..10], words[..], "call #{} box", self.next);
        }
        self.next += 1;
        self.reached.insert(address);
        let table = POKES.wrapping_add(address << 6);
        for k in 0..self.case.input(table) {
            let target = self.case.input(table + 4 + 8 * k);
            memory.write_u32(target, self.case.input(table + 8 + 8 * k));
        }
        if address == 0x9E3C0 {
            return self.case.input(BUSY.wrapping_add(args[0].wrapping_mul(4)));
        }
        self.case.input(RETS + address)
    }
}

/// Runs every case with `run(selector, memory, effects, case) -> Option<result>`; checks outputs, calls and (when
/// the function returns a value) eax. Returns the per selector case counts and the callees reached.
fn drive(
    name: &str,
    minimum: usize,
    mut run: impl FnMut(u32, &mut Mem, &mut Recorded, &Case) -> Option<u32>,
) -> (BTreeMap<u32, usize>, BTreeSet<u32>) {
    let mut seen = BTreeMap::new();
    let mut reached = BTreeSet::new();
    for (index, case) in load(name, minimum).iter().enumerate() {
        let mut memory = Mem {
            words: case.ins.clone(),
        };
        let mut effects = Recorded {
            case,
            next: 0,
            reached: BTreeSet::new(),
        };
        let which = case.input(SELECT);
        let result = run(which, &mut memory, &mut effects, case);
        if let Some(value) = result {
            assert_eq!(value, case.eax, "{name} case {index} fn {which:#x} result");
        }
        assert_eq!(
            effects.next,
            case.calls.len(),
            "{name} case {index} fn {which:#x}: missing calls"
        );
        for (&address, &expected) in &case.outs {
            assert_eq!(
                memory.read_u32(address),
                expected,
                "{name} case {index} fn {which:#x}: memory {address:#x}"
            );
        }
        *seen.entry(which).or_insert(0) += 1;
        reached.extend(effects.reached);
    }
    (seen, reached)
}

fn arg(case: &Case, index: u32) -> u32 {
    case.input(STACK_ARGS + 4 * index)
}

fn reg(case: &Case, index: u32) -> u32 {
    case.input(REGS + 4 * index)
}

fn assert_reached(reached: &BTreeSet<u32>, addresses: &[u32]) {
    for address in addresses {
        assert!(
            reached.contains(address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn pak_select_matches_original() {
    drive("res_select", 72, |_, m, _, case| {
        Some(pak_select(m, reg(case, 0), reg(case, 1)))
    });
}

#[test]
fn pak_find_matches_original() {
    drive("res_find", 72, |_, m, _, case| {
        Some(pak_find(m, reg(case, 0)))
    });
}

#[test]
fn find_file_matches_original() {
    drive("res_find_file", 72, |_, m, _, case| {
        Some(find_file(m, arg(case, 0), arg(case, 1)))
    });
}

#[test]
fn open_named_matches_original() {
    drive("res_open", 72, |_, m, _, case| {
        Some(open_named(m, arg(case, 0), arg(case, 1)))
    });
}

#[test]
fn file_size_matches_original() {
    let (_, reached) = drive("res_size", 72, |_, m, e, case| {
        Some(file_size(m, e, arg(case, 0)))
    });
    assert_reached(&reached, &[0x296F0, 0x29030, 0x29090]);
}

#[test]
fn file_read_matches_original() {
    let (_, reached) = drive("res_read", 72, |_, m, e, case| {
        Some(file_read(m, e, arg(case, 0), arg(case, 1), arg(case, 2)))
    });
    assert_reached(
        &reached,
        &[0x296F0, 0x29030, 0x29090, 0x28D50, 0x35DE70, 0x389B0],
    );
}

#[test]
fn load_file_matches_original() {
    let (_, reached) = drive("res_load", 72, |_, m, e, case| {
        Some(load_file(m, e, arg(case, 0), arg(case, 1), arg(case, 2)))
    });
    assert_reached(&reached, &[0x3E0B0, 0x28D50]);
}

#[test]
fn find_or_load_matches_original() {
    let (_, reached) = drive("res_find_or_load", 72, |_, m, e, case| {
        Some(find_or_load(m, e, arg(case, 0), arg(case, 1), arg(case, 2)))
    });
    assert_reached(&reached, &[0x3E0B0, 0x28D50]);
}

#[test]
fn base_name_and_strstr_match_original() {
    drive("res_base_name", 72, |_, m, _, case| {
        Some(base_name(m, arg(case, 0)))
    });
    drive("res_strstr", 72, |_, m, _, case| {
        Some(c_strstr(m, arg(case, 0), arg(case, 1)))
    });
}

#[test]
fn cache_and_note_match_original() {
    drive("res_cache", 72, |_, m, _, case| {
        Some(cache_lookup(m, arg(case, 0)))
    });
    drive("res_note", 72, |_, m, _, case| {
        Some(note_model(m, arg(case, 0)))
    });
}

#[test]
fn archive_helpers_match_original() {
    let (seen, reached) = drive("res_archive", 72, |which, m, e, case| match which {
        0x18380 => Some(archive_id(m, arg(case, 0))),
        _ => Some(archive_add(m, e, arg(case, 0), arg(case, 1), arg(case, 2))),
    });
    assert_eq!(seen.len(), 2);
    assert_reached(&reached, &[0x18390]);
}

#[test]
fn fixup_file_matches_original() {
    let (_, reached) = drive("res_fixup", 72, |_, m, e, case| {
        Some(fixup_file(m, e, arg(case, 0), arg(case, 1)))
    });
    assert_reached(&reached, &[0x8E3E0, 0x8DD10, 0x8E310, 0x637E0, 0x18390]);
}

#[test]
fn model_finish_matches_original() {
    let (_, reached) = drive("res_finish", 72, |_, m, e, case| {
        Some(model_finish(m, e, arg(case, 0), reg(case, 0), reg(case, 1)))
    });
    assert_reached(&reached, &[0x8DF60, 0x8DD10]);
}

#[test]
fn pak_slots_match_original() {
    drive("res_slot_name", 72, |_, m, _, case| {
        Some(slot_by_name(m, arg(case, 0)))
    });
    let (_, reached) = drive("res_slot_alloc", 72, |_, m, e, case| {
        Some(slot_alloc(m, e, arg(case, 0), arg(case, 1)))
    });
    assert_reached(&reached, &[0x3DFA0]);
    drive("res_mount_mem", 72, |_, m, _, case| {
        Some(mount_memory(m, arg(case, 0), arg(case, 1), arg(case, 2)))
    });
}

#[test]
fn bind_file_matches_original() {
    let (_, reached) = drive("res_bind", 72, |_, m, e, case| {
        bind_file(m, e, arg(case, 0), arg(case, 1));
        None
    });
    assert_reached(&reached, &[0x18FB10]);
}

/// The golden st0 (f64 bits of the f32 the original returns on the x87 stack) against the port's f32 bits.
fn assert_float(case: &Case, bits: u32, what: &str) {
    let expected = f64::from_bits(case.st0.expect("st0"));
    let got = f64::from(f32::from_bits(bits));
    assert!(
        (expected.is_nan() && got.is_nan()) || expected.to_bits() == got.to_bits(),
        "{what}: {expected:?} vs {got:?}"
    );
}

#[test]
fn scratch_blocks_match_original() {
    drive("shp_block_alloc", 72, |_, m, _, case| {
        Some(block_alloc(m, arg(case, 0)))
    });
    drive("shp_block_free", 72, |_, m, _, case| {
        block_free(m, arg(case, 0));
        None
    });
}

#[test]
fn pool_and_model_queries_match_original() {
    drive("shp_archive_model", 72, |_, m, _, case| {
        Some(archive_model(m, arg(case, 0)))
    });
    drive("shp_slot_busy", 72, |_, m, _, case| {
        Some(slot_busy(m, arg(case, 0)))
    });
    drive("shp_model_known", 72, |_, m, _, case| {
        Some(model_known(m, arg(case, 0)))
    });
    let (_, reached) = drive("shp_dispatch", 72, |_, m, e, case| {
        Some(dispatch(m, e, arg(case, 0), arg(case, 1), arg(case, 2)))
    });
    assert_reached(&reached, &[0x100000, 0x100020]);
}

#[test]
fn scale_queries_match_original() {
    drive("shp_model_scale", 72, |_, m, _, case| {
        assert_float(case, model_scale(m, arg(case, 0)), "model_scale");
        None
    });
    let (_, reached) = drive("shp_record_table", 72, |_, m, e, case| {
        Some(record_table(m, e, arg(case, 0), arg(case, 1)))
    });
    assert_reached(&reached, &[0x1B99B0]);
    drive("shp_scale_factor", 72, |_, m, e, case| {
        assert_float(case, scale_factor(m, e, arg(case, 0)), "scale_factor");
        None
    });
    let (_, reached) = drive("shp_dynamic", 72, |_, m, e, case| {
        Some(is_dynamic(m, e, arg(case, 0)))
    });
    assert_reached(&reached, &[0x8E5F0]);
}

#[test]
fn box_queries_match_original() {
    let (_, reached) = drive("shp_box_corners", 72, |_, m, e, case| {
        box_corners(m, e, entry_of(3), arg(case, 0), arg(case, 1), arg(case, 2));
        None
    });
    assert_reached(&reached, &[0x151E80]);
    drive("shp_box_transform", 72, |_, m, e, case| {
        box_transform(m, e, entry_of(3), arg(case, 0), arg(case, 1), arg(case, 2));
        None
    });
}

#[test]
fn shape_buffers_match_original() {
    let (_, reached) = drive("shp_mesh", 72, |_, m, e, case| {
        shape_mesh(m, e, entry_of(2), arg(case, 0), arg(case, 1));
        None
    });
    assert_reached(&reached, &[0x3DFA0, 0x389B0, 0x8FE30]);
    let (_, reached) = drive("shp_fill", 72, |_, m, e, case| {
        shape_fill(m, e, arg(case, 0), arg(case, 1), arg(case, 2));
        None
    });
    assert_reached(&reached, &[0x3DFA0, 0x976E0, 0x8FB20]);
}

#[test]
fn light_registration_and_attach_match_original() {
    let (_, reached) = drive("shp_register", 60, |_, m, e, case| {
        shape_register(m, e, arg(case, 0));
        None
    });
    assert_reached(&reached, &[0x6A540]);
    let (_, reached) = drive("shp_attach", 72, |_, m, e, case| {
        shape_attach(m, e, arg(case, 0), arg(case, 1), arg(case, 2));
        None
    });
    assert_reached(&reached, &[0xA5CE0]);
}

/// The WHOLE model finder `0x96410` with the resource layer real (cache, pak lookups, loads, fix-up, bind): only
/// the file system, allocator, formatter and table fix-ups are recorded.
#[test]
fn model_find_root_with_ported_resource_layer() {
    let (_, reached) = drive("res_root_find", 60, |_, m, e, case| {
        let mut layer = PortedResource { inner: e };
        Some(model_find(
            m,
            &mut layer,
            entry_of(2),
            arg(case, 0),
            arg(case, 1),
        ))
    });
    assert_reached(&reached, &[0x3C85D4, 0x8E3E0, 0x8DD10, 0x18FB10]);
}

#[test]
fn archive_loader_root_with_ported_resource_layer() {
    let (_, reached) = drive("res_root_load_a", 48, |_, m, e, case| {
        let mut layer = PortedResource { inner: e };
        Some(model_load_a(
            m,
            &mut layer,
            entry_of(2),
            arg(case, 0),
            arg(case, 1),
        ))
    });
    assert_reached(&reached, &[0x3C85D4, 0x18390, 0x8DD10, 0x18FB10]);
}

#[test]
fn plain_loader_root_with_ported_resource_layer() {
    let (_, reached) = drive("res_root_load_b", 48, |_, m, e, case| {
        let mut layer = PortedResource { inner: e };
        Some(model_load_b(m, &mut layer, arg(case, 0), arg(case, 1)))
    });
    assert_reached(&reached, &[0x3E0B0, 0x8DD10, 0x18FB10]);
}

/// The WHOLE shape slot setup `0x97C00` with the scratch blocks, shape buffers and light registration real.
#[test]
fn object_shape_root_with_ported_shape_layer() {
    let (_, reached) = drive("shp_root_shape", 60, |_, m, e, case| {
        let mut layer = PortedShape {
            inner: e,
            entry: entry_of(2),
            context: ShapeContext::ObjectShape,
        };
        Some(object_shape(m, &mut layer, arg(case, 0), arg(case, 1)))
    });
    assert_reached(
        &reached,
        &[0x3DFA0, 0x8FE30, 0x976E0, 0x8FB20, 0x6A540, 0x18FDC0],
    );
}
