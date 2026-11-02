//! T1865: the pool support behind `0xA6880` / `0xA7090` (`svc_pool`) against golden vectors from the original
//! machine code under the Unicorn oracle (MEASURED). Goldens: `tests/data/t1865/pool_*.golden`
//! (generator `tools/t1846_gold/svc_pool.py`).
mod common;
use common::{Case, parse};
use std::collections::{BTreeMap, BTreeSet};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_destroy::{destroy_pool_object, pool_get};
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;
use tsfp_gameplay::story_events::svc_pool::*;

const SELECT: u32 = 0x2FF_F000;
const STACK_ARGS: u32 = 0x2FF_F100;
const RETS: u32 = 0x3000_0000;
const BUSY: u32 = 0x3100_0000;
const POKES: u32 = 0x3400_0000;
const STACK_TOP: u32 = 0x7F00_0000;

/// esp at entry of a call with `nargs` cdecl arguments in the oracle.
fn entry_of(nargs: u32) -> u32 {
    STACK_TOP - 0x1000 - 4 * nargs - 4
}

fn load(name: &str, minimum: usize) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1865/{name}.golden",
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
            .unwrap_or_else(|| panic!("extra call {address:#x} {args:x?}"));
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

fn arg(case: &Case, index: u32) -> u32 {
    case.input(STACK_ARGS + 4 * index)
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

fn assert_reached(reached: &BTreeSet<u32>, addresses: &[u32]) {
    for address in addresses {
        assert!(
            reached.contains(address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn pool_alloc_matches_original() {
    let (_, reached) = drive("pool_alloc", 72, |_, m, e, _| Some(pool_alloc(m, e)));
    assert_reached(&reached, &[0x9E3C0]);
}

#[test]
fn model_find_matches_original() {
    let (_, reached) = drive("pool_find", 72, |_, m, e, case| {
        Some(model_find(m, e, entry_of(2), arg(case, 0), arg(case, 1)))
    });
    assert_reached(
        &reached,
        &[
            0x631C0, 0x62FB0, 0x3C85D4, 0x624C0, 0x6B380, 0x62C30, 0x62F00, 0x96330, 0x8DF60,
            0x389B0,
        ],
    );
}

#[test]
fn model_loaders_match_original() {
    let (_, reached) = drive("pool_load_a", 72, |_, m, e, case| {
        Some(model_load_a(m, e, entry_of(2), arg(case, 0), arg(case, 1)))
    });
    assert_reached(
        &reached,
        &[
            0x631C0, 0x62FB0, 0x3C85D4, 0x624C0, 0x18380, 0x18A30, 0x8E420,
        ],
    );
    let (_, reached) = drive("pool_load_b", 72, |_, m, e, case| {
        Some(model_load_b(m, e, arg(case, 0), arg(case, 1)))
    });
    assert_reached(&reached, &[0x631C0, 0x62C30, 0x62F00, 0x8E420]);
}

#[test]
fn object_shape_matches_original() {
    let (_, reached) = drive("pool_shape", 72, |_, m, e, case| {
        Some(object_shape(m, e, arg(case, 0), arg(case, 1)))
    });
    assert_reached(
        &reached,
        &[0x3D6E0, 0x965F0, 0x97A00, 0x18FDC0, 0x8C910, 0x3D6B0],
    );
}

#[test]
fn object_class_matches_original() {
    let (_, reached) = drive("pool_class", 72, |_, m, e, case| {
        object_class(m, e, entry_of(1), arg(case, 0));
        None
    });
    assert_reached(&reached, &[0x873C0]);
}

#[test]
fn object_place_matches_original() {
    let (_, reached) = drive("pool_place", 72, |_, m, e, case| {
        // the caller's pushed argument lives in the stack slot the function reuses as scratch
        m.write_u32(entry_of(1) + 4, arg(case, 0));
        Some(object_place(m, e, entry_of(1), arg(case, 0)))
    });
    assert_reached(
        &reached,
        &[
            0x83790, 0x9EAB0, 0x9E330, 0x100000, 0x9EB60, 0xA14B0, 0x9E840, 0xBE9E0,
        ],
    );
}

#[test]
fn object_tail_matches_original() {
    let (_, reached) = drive("pool_tail", 72, |_, m, e, case| {
        Some(object_tail(m, e, arg(case, 0)))
    });
    assert_reached(
        &reached,
        &[
            0x9EF60, 0xA5F40, 0xA6290, 0x129920, 0xA64D0, 0x12C1F0, 0x128710, 0x128320,
        ],
    );
}

#[test]
fn release_set_matches_original() {
    let (seen, reached) = drive("pool_release", 196, |which, m, e, case| {
        let a = arg(case, 0);
        match which {
            0x129EC0 => release_records(m, a),
            0x15BFF0 => release_links(m, e, entry_of(1), a),
            0xD2D50 => release_session_keys(m, a),
            0x227920 => release_hud(m, e, a),
            0x10E110 => release_slots(m, e, a),
            0xE0520 => release_physics(m, e, a),
            0x1129B0 => release_regions(m, e, a),
            0x8E9D0 => return Some(release_buffer(m, e, a)),
            0x170ED0 => return Some(release_voice(m, e, a)),
            0x1274A0 => release_emitter(m, a),
            0xA0010 => release_script(m, e, a),
            0x45460 => release_sound(m, e, a),
            0x240410 => release_key_node(m, e, a),
            0x2404D0 => release_key_chain(m, a),
            other => panic!("unexpected function {other:#x}"),
        }
        None
    });
    assert_eq!(seen.len(), 14, "{seen:x?}");
    assert_reached(
        &reached,
        &[
            0x277FE0, 0x15A540, 0x275620, 0x17510, 0x174E0, 0x17570, 0x2271F0, 0x10E070, 0xE03D0,
            0xE0300, 0x1123C0, 0x56FF0, 0x165FE0, 0x153200, 0x16C300, 0xA5CE0, 0x9E720, 0x153240,
            0x273E0, 0x23FF70,
        ],
    );
}

#[test]
fn pool_get_whole_root_matches_original() {
    let mut loaded = 0;
    let mut failed = 0;
    let (_, reached) = drive("pool_get_root", 72, |_, m, e, case| {
        let mut pool = PortedPool {
            inner: e,
            entry: entry_of(3),
        };
        let result = pool_get(m, &mut pool, arg(case, 0), arg(case, 1), arg(case, 2));
        if result == 0 {
            failed += 1;
        } else {
            loaded += 1;
        }
        Some(result)
    });
    assert!(loaded > 20 && failed > 3, "loaded {loaded} failed {failed}");
    // engine routines below the ported pool support, all stubbed and all reached at least once
    assert_reached(
        &reached,
        &[
            0x9E3C0, 0x12EC0, 0x6CDB0, 0x153B40, 0x966E0, 0x631C0, 0x62FB0, 0x3C85D4, 0x624C0,
            0x62C30, 0x62F00, 0x8DF60, 0x8E420, 0x3D6E0, 0x97A00, 0x8C910, 0x3D6B0, 0x83790,
            0x9E330, 0x9EB60, 0xA14B0, 0x9E840, 0x873C0, 0x9EF60, 0xA64D0, 0x12C1F0, 0x128710,
            0x128320,
        ],
    );
}

#[test]
fn pool_destroy_whole_root_matches_original() {
    let (_, reached) = drive("pool_destroy_root", 60, |_, m, e, case| {
        let mut pool = PortedPool {
            inner: e,
            entry: entry_of(1),
        };
        destroy_pool_object(m, &mut pool, arg(case, 0));
        None
    });
    // the release set runs real code, only the engine below it is stubbed
    assert_reached(
        &reached,
        &[
            0x100000, 0x277FE0, 0x15A540, 0x275620, 0x2271F0, 0x10E070, 0xE03D0, 0xE0300, 0x1123C0,
            0x56FF0, 0x16C300, 0xA5CE0, 0x9E720, 0x273E0, 0x23FF70,
        ],
    );
}

#[test]
fn wiring_allocates_objects_with_the_real_pool_get() {
    use tsfp_gameplay::story_events::svc_wiring::{HookServices, TraceEvent, fabricated};
    let mut services = HookServices::new(1);
    let position = 0x5900_0000;
    services.ram.borrow_mut().set_float(position, -422.8);
    let args = [0, 8, 5, 0, position, 0, 0, 0, 0, 0];
    let first = services.spawn_object(args);
    let second = services.spawn_object(args);
    assert_eq!(first, fabricated::OBJECT_HEAP);
    assert_eq!(second, fabricated::OBJECT_HEAP + fabricated::OBJECT_SIZE);
    let ram = services.ram.borrow();
    // pool bookkeeping of 0x9EE00, handle counter of 0xA6880
    assert_eq!(ram.word(0x7B_4FAC), 2);
    assert_eq!(ram.word(0x72_F5E0), 3);
    // descriptor bound, shape slots in order, pool slot index of 0x9EE00 at +0x88 (the spawner rewrites kind, flags
    // and handle afterwards)
    assert_eq!((ram.word(first + 0x88), ram.word(second + 0x88)), (0, 1));
    assert_eq!(ram.word(first + 0x24), fabricated::MODEL_TABLE);
    assert_eq!(ram.word(first + 4), fabricated::SHAPE_POOL);
    assert_eq!(ram.word(second + 4), fabricated::SHAPE_POOL + 0x200);
    drop(ram);
    let below: BTreeSet<u32> = services
        .trace()
        .iter()
        .filter_map(|event| match event {
            TraceEvent::DestroyCallee { address, .. } => Some(*address),
            _ => None,
        })
        .collect();
    // T1869: the shape and file layers (`PortedShape`, `PortedShapeServices`, `PortedFiles`) now run ported under the
    // pool support, so the scratch blocks, the light registration and the model queries no longer reach the recorder;
    // what is still recorded (never silently dropped) is the placement and class engine, the bounds query and the
    // part visitor `0x903D0` under the pose engine (the pose engine itself is ported by T1872, `svc_pose`).
    for address in [0xA64D0, 0x128320, 0x873C0, 0x9EF60, 0xA14B0, 0x903D0] {
        assert!(
            below.contains(&address),
            "{address:#x} not recorded: {below:x?}"
        );
    }
    for address in [
        0x9E3C0, 0x8C910, 0x3D6E0, 0x3D6B0, 0x83790, 0x8FE30, 0x976E0, 0x8FB20, 0x6A540, 0x58270,
        0x1531D0, 0x96860,
    ] {
        assert!(
            !below.contains(&address),
            "{address:#x} should run ported: {below:x?}"
        );
    }
}
