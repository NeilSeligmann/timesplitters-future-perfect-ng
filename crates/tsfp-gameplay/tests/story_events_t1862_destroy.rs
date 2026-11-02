//! T1862: engine callees of the object destroy `0xBABA0` (`svc_destroy`) against golden vectors from the original
//! machine code under the Unicorn oracle (MEASURED). Goldens: `tests/data/t1862/destroy_*.golden`.
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_destroy::*;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;

const SELECT: u32 = 0x2FF_F000;
const STACK_ARGS: u32 = 0x2FF_F100;
const RETS: u32 = 0x3000_0000;

fn load(name: &str) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1862/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    );
    let cases = parse(&std::fs::read_to_string(path).expect("golden file"));
    assert!(cases.len() >= 72, "{name}: too few cases");
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
            acc | u32::from(self.read_u8(address + k)) << (8 * k)
        })
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        if address & 3 == 0 {
            self.words.insert(address, value);
        } else {
            for k in 0..4 {
                self.write_u8(address + k, (value >> (8 * k)) as u8);
            }
        }
    }
}

/// Replays the engine calls of the golden (callee and arguments checked in order, result from `RETS + callee`).
struct Recorded<'a> {
    case: &'a Case,
    next: usize,
}

impl DestroyEffects for Recorded<'_> {
    fn call(&mut self, _: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        let expected = self
            .case
            .calls
            .get(self.next)
            .unwrap_or_else(|| panic!("extra call {address:#x} {args:x?}"));
        assert_eq!(expected.callee, address, "call #{}", self.next);
        if address == 0xA0010 {
            // register argument: the oracle logs esi
            assert_eq!(expected.regs[3], args[0], "call #{} esi", self.next);
        } else {
            assert_eq!(
                expected.args[..args.len()],
                *args,
                "call #{} {address:#x} args",
                self.next
            );
        }
        self.next += 1;
        self.case.input(RETS + address)
    }
}

fn arg(case: &Case, index: u32) -> u32 {
    case.input(STACK_ARGS + 4 * index)
}

/// Runs every case with `run(selector, memory, effects, case) -> Option<result>`; checks outputs, calls and (when
/// the function returns a value) eax. Returns the per selector case counts.
fn drive(
    name: &str,
    mut run: impl FnMut(u32, &mut Mem, &mut Recorded, &Case) -> Option<u32>,
) -> BTreeMap<u32, usize> {
    let mut seen = BTreeMap::new();
    for (index, case) in load(name).iter().enumerate() {
        let mut memory = Mem {
            words: case.ins.clone(),
        };
        let mut effects = Recorded { case, next: 0 };
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
    }
    seen
}

#[test]
fn leaves_match_original() {
    let mut distinct: BTreeMap<u32, std::collections::BTreeSet<u32>> = BTreeMap::new();
    let seen = drive("destroy_leaf", |which, m, _, case| {
        let result = match which {
            0x3533F0 => Some(is_network(m)),
            0x1CDAE0 => Some(channel_count(m)),
            0x3D7B0 => Some(string_lookup(m, arg(case, 0))),
            0x12E3B0 => {
                set_score(m, arg(case, 0), arg(case, 1));
                None
            }
            0x12E30 => Some(object_flag(m, arg(case, 0), arg(case, 1))),
            0x9E300 => Some(flag_table(m, arg(case, 0), arg(case, 1))),
            0x2382B0 => Some(u32::from(sound_slot_valid(m, arg(case, 0)))),
            0x238510 => {
                add_voice_value(m, arg(case, 0), arg(case, 1), arg(case, 2));
                None
            }
            other => panic!("selector {other:#x}"),
        };
        if let Some(value) = result {
            distinct.entry(which).or_default().insert(value);
        }
        result
    });
    assert_eq!(seen.len(), 8, "every leaf exercised");
    assert!(distinct[&0x3D7B0].len() >= 4);
    assert!(distinct[&0x1CDAE0].len() >= 3);
    assert_eq!(distinct[&0x9E300].len(), 2, "both flag outcomes");
    assert_eq!(distinct[&0x12E30].len(), 2, "both flag outcomes");
    assert_eq!(distinct[&0x2382B0].len(), 2, "both validity outcomes");
}

#[test]
fn links_match_original() {
    let mut found = 0;
    let mut cleared = 0;
    drive("destroy_link", |which, m, _, case| match which {
        0x17A120 => {
            let result = find_link(m, arg(case, 0));
            found += usize::from(result != 0);
            Some(result)
        }
        0x17BF70 => {
            notify_ctx(m, arg(case, 0));
            cleared += 1;
            None
        }
        other => panic!("selector {other:#x}"),
    });
    assert!(found > 5 && cleared > 5, "found {found} cleared {cleared}");
}

#[test]
fn notifications_match_original() {
    let seen = drive("destroy_notify", |which, m, e, case| {
        match which {
            0x22C3A0 => notify_link(m, e, arg(case, 0), arg(case, 1)),
            0xCF440 => notify_object(m, e, arg(case, 0), arg(case, 1)),
            0x10C0E0 => release_buffer(m, e, arg(case, 0)),
            0x22DD10 => notify_block(m, e, arg(case, 0)),
            other => panic!("selector {other:#x}"),
        }
        None
    });
    assert_eq!(seen.len(), 4);
    // every callee behind them is reached by some case
    let mut callees = std::collections::BTreeSet::new();
    for case in load("destroy_notify") {
        callees.extend(case.calls.iter().map(|c| c.callee));
    }
    for address in [0x12D6B0, 0x12D760, 0x3EAD0, 0x13BF40, 0x229C30] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn hit_handlers_match_original() {
    let seen = drive("destroy_hit", |which, m, e, case| {
        match which {
            0x6B060 => consume_hit(m, e, arg(case, 0), arg(case, 1)),
            0x6AFB0 => start_hit_voice(m, e, arg(case, 0), arg(case, 1)),
            other => panic!("selector {other:#x}"),
        }
        None
    });
    assert_eq!(seen.len(), 2);
    let mut callees = std::collections::BTreeSet::new();
    for case in load("destroy_hit") {
        callees.extend(case.calls.iter().map(|c| c.callee));
    }
    for address in [0x16C170, 0x172760, 0x168130, 0x168100] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn pool_get_matches_original() {
    let mut loaded = 0;
    let mut failed = 0;
    let mut callees = std::collections::BTreeSet::new();
    drive("destroy_pool_get", |_, m, e, case| {
        let result = pool_get(m, e, arg(case, 0), arg(case, 1), arg(case, 2));
        if result == 0 {
            failed += 1;
        } else {
            loaded += 1;
        }
        callees.extend(case.calls.iter().map(|c| c.callee));
        Some(result)
    });
    assert!(loaded > 10 && failed > 2, "loaded {loaded} failed {failed}");
    for address in [
        0x9EE00, 0x12EC0, 0x8E4B0, 0x8E590, 0x96410, 0x8E3D0, 0x6CDB0, 0x97C00, 0x153B40, 0x966E0,
        0xA6530, 0x9FBE0, 0xA67C0, 0x389B0,
    ] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn pool_destroy_matches_original() {
    let mut callees = std::collections::BTreeSet::new();
    drive("destroy_pool_destroy", |_, m, e, case| {
        destroy_pool_object(m, e, arg(case, 0));
        callees.extend(case.calls.iter().map(|c| c.callee));
        None
    });
    for address in [
        0x9E330, 0x240410, 0x2404D0, 0x129EC0, 0x15BFF0, 0xD2D50, 0x227920, 0x10E110, 0xE0520,
        0x1129B0, 0x8E9D0, 0x170ED0, 0x1274A0, 0xA0010, 0x45460, 0x100000,
    ] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}
