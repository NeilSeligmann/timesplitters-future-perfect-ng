//! T1869: the table relocators and the texture registrar (`svc_reloc`) against golden vectors from the original
//! machine code under the Unicorn oracle (MEASURED). Goldens: `tests/data/t1869/rel_*.golden` (generator
//! `tools/t1846_gold/svc_reloc.py`).
mod common;
use common::{Case, parse};
use std::collections::{BTreeMap, BTreeSet};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;
use tsfp_gameplay::story_events::svc_reloc::*;
use tsfp_gameplay::story_events::svc_shape_services::*;

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
        "{}/tests/data/t1869/{name}.golden",
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
        let expected = self
            .case
            .calls
            .get(self.next)
            .unwrap_or_else(|| panic!("extra call {address:#x} {args:x?}"))
            .clone();
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
        // words behind a pointer argument that the stub logged (`deref`): the vector given to `0x96860`, the quaternion of `0x1506B0`
        let pointed = |memory: &dyn GuestMemory, pointer: u32, words: u32| -> Vec<u32> {
            (0..words)
                .map(|k| memory.read_u32(pointer.wrapping_add(4 * k)))
                .collect()
        };
        if address == 0x96860 {
            assert_eq!(
                expected.args[3..6],
                pointed(memory, args[2], 3)[..],
                "call #{} vector",
                self.next
            );
        }
        if address == 0x1506B0 {
            assert_eq!(
                expected.args[2..6],
                pointed(memory, args[1], 4)[..],
                "call #{} quaternion",
                self.next
            );
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
fn relocate_names_matches_original() {
    drive("rel_names", 72, |_, m, _, case| {
        relocate_names(m, arg(case, 0), arg(case, 1));
        None
    });
}

#[test]
fn register_by_index_matches_original() {
    let (_, reached) = drive("rel_by_index", 72, |_, m, e, case| {
        register_by_index(m, e, entry_of(0), reg(case, 0), reg(case, 1));
        None
    });
    assert_reached(&reached, &[0x3C85D4, 0x624C0, 0x18380, 0x18A30]);
}

#[test]
fn stream_insert_matches_original() {
    drive("rel_insert", 72, |_, m, _, case| {
        stream_insert(m, reg(case, 0));
        None
    });
}

#[test]
fn stream_open_matches_original() {
    let (_, reached) = drive("rel_stream", 72, |_, m, e, case| {
        Some(stream_open(m, e, arg(case, 0)))
    });
    assert_reached(&reached, &[0x290F0]);
}

#[test]
fn register_id_matches_original() {
    let (_, reached) = drive("rel_register", 72, |_, m, e, case| {
        register_id(m, e, entry_of(3), arg(case, 0), arg(case, 1), arg(case, 2));
        None
    });
    assert_reached(&reached, &[0x3C85D4, 0x290F0]);
}

#[test]
fn light_slot_matches_original() {
    drive("rel_light", 72, |_, m, _, _| Some(light_slot(m)));
}

#[test]
fn weapon_class_matches_original() {
    drive("rel_weapon", 72, |_, m, _, case| {
        Some(weapon_class(m, arg(case, 0)))
    });
}

#[test]
fn row_relocate_matches_original() {
    drive("rel_row", 72, |_, m, _, case| {
        Some(row_relocate(m, arg(case, 0), arg(case, 1)))
    });
}

#[test]
fn relocate_table_matches_original() {
    drive("rel_table", 72, |_, m, _, case| {
        relocate_table(m, arg(case, 0), arg(case, 1), arg(case, 2));
        None
    });
}

#[test]
fn classify_streams_matches_original() {
    let (_, reached) = drive("rel_classify", 160, |_, m, e, case| {
        classify_streams(m, e, arg(case, 0));
        None
    });
    assert_reached(&reached, &[0x94620]);
}

#[test]
fn registry_alloc_matches_original() {
    drive("rel_registry", 72, |_, m, _, _| Some(registry_alloc(m)));
}

#[test]
fn texture_register_matches_original() {
    let (_, reached) = drive("rel_texture", 72, |_, m, e, case| {
        Some(texture_register(
            m,
            e,
            entry_of(2),
            reg(case, 0),
            arg(case, 0),
            arg(case, 1),
        ))
    });
    assert_reached(&reached, &[0x3E6684, 0x3D4D70]);
}

#[test]
fn shape_done_matches_original() {
    let (_, reached) = drive("shs_done", 72, |_, m, e, case| {
        shape_done(m, e, reg(case, 1), reg(case, 0), arg(case, 0));
        None
    });
    assert_reached(&reached, &[0x1531D0]);
}

#[test]
fn shape_dynamic_matches_original() {
    let (_, reached) = drive("shs_dynamic", 72, |_, m, e, case| {
        shape_dynamic(m, e, entry_of(1), reg(case, 0), reg(case, 1), arg(case, 0));
        None
    });
    assert_reached(
        &reached,
        &[
            0x58270, 0x59320, 0x1506B0, 0x1531D0, 0x8FE30, 0x583E0, 0x582A0, 0x58340, 0x59BF0,
            0x153240, 0x59C00, 0x96860, 0x59CB0,
        ],
    );
}

#[test]
fn tree_count_matches_original() {
    drive("shs_tree", 72, |_, m, _, case| {
        tree_count(m, arg(case, 0), arg(case, 1), arg(case, 2), arg(case, 3));
        None
    });
}

#[test]
fn register_block_matches_original() {
    drive("shs_block", 72, |_, m, _, case| {
        register_block(m, reg(case, 0), reg(case, 1), arg(case, 0));
        None
    });
}

#[test]
fn retag_streams_matches_original() {
    drive("shs_retag", 72, |_, m, _, case| {
        retag_streams(
            m,
            arg(case, 0),
            arg(case, 1),
            arg(case, 2),
            arg(case, 3),
            arg(case, 4),
        );
        None
    });
}

#[test]
fn texture_present_matches_original() {
    drive("rel_present", 72, |_, m, _, case| {
        Some(texture_present(m, arg(case, 0), arg(case, 1)))
    });
}
