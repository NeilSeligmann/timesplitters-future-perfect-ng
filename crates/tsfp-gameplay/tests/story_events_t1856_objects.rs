//! T1856: object/handle engine services (`svc_objects`) against golden vectors from the original machine code
//! under the Unicorn oracle (MEASURED). Goldens: `tests/data/t1856/objects_*.golden`.
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::strip_update::{GuestMemory, Service, StripServices};
use tsfp_gameplay::story_events::svc_objects::*;

const SELECT: u32 = 0x2FF_F000;
const ENTRY0: u32 = 0x7F00_0000 - 0x1000 - 4;

fn entry_sp(nargs: u32) -> u32 {
    ENTRY0 - 4 * nargs
}

fn load(name: &str) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1856/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    );
    let cases = parse(&std::fs::read_to_string(path).expect("golden file"));
    assert!(cases.len() >= 60, "{name}: too few cases");
    cases
}

/// Case memory: unwritten addresses read 0 (the generator writes everything the function reads).
struct Mem {
    words: BTreeMap<u32, u32>,
}

impl Mem {
    fn new(case: &Case) -> Self {
        Mem {
            words: case.ins.clone(),
        }
    }
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

fn check_outputs(name: &str, index: usize, case: &Case, memory: &Mem) {
    for (&address, &expected) in &case.outs {
        assert_eq!(
            memory.read_u32(address),
            expected,
            "{name} case {index}: memory {address:#x}"
        );
    }
}

/// Never reaches engine code: any service call of the destroy path fails the test.
struct NoEffects;
impl DestroyEffects for NoEffects {
    fn call(&mut self, _: &mut dyn GuestMemory, address: u32, _: &[u32]) -> u32 {
        panic!("unexpected engine call {address:#x}");
    }
}

#[test]
fn finds_match_original() {
    let mut hits = [0usize; 3];
    for (index, case) in load("objects_find").iter().enumerate() {
        let mut memory = Mem::new(case);
        let which = case.input(SELECT);
        let key = find_key(case);
        let result = match which {
            0x15BA10 => find_object_a(&memory, key),
            0x163730 => find_object_b(&memory, key),
            0x23FF20 => lookup_by_key(&mut memory, key),
            other => panic!("selector {other:#x}"),
        };
        assert_eq!(result, case.eax, "case {index} fn {which:#x} key {key:#x}");
        if result != 0 {
            hits[[0x15BA10, 0x163730, 0x23FF20]
                .iter()
                .position(|&v| v == which)
                .unwrap()] += 1;
        }
        check_outputs("objects_find", index, case, &memory);
    }
    assert!(hits.iter().all(|&h| h > 3), "hit counts {hits:?}");
}

/// The generator stores the stack arguments as `in` words at `STACK_ARGS`.
fn find_key(case: &Case) -> u32 {
    case.input(STACK_ARGS)
}

const STACK_ARGS: u32 = 0x2FF_F100;

#[test]
fn class_services_match_original() {
    let mut nonzero = [0usize; 5];
    for (index, case) in load("objects_class").iter().enumerate() {
        let mut memory = Mem::new(case);
        let which = case.input(SELECT);
        let arg = |k: u32| case.input(STACK_ARGS + 4 * k);
        let result = match which {
            0x59CF0 => class_of(&memory, arg(0)),
            0x585E0 => {
                query_state(&mut memory, arg(0), arg(1), arg(2));
                case.eax
            }
            0x56FC0 => ranges_record(&memory, arg(0)),
            0x61570 => flag_test(&mut memory, arg(0), arg(1), entry_sp(2)),
            0xBDA30 => owner_record(arg(0)),
            other => panic!("selector {other:#x}"),
        };
        assert_eq!(result, case.eax, "case {index} fn {which:#x}");
        if result != 0 {
            let slot = [0x59CF0, 0x585E0, 0x56FC0, 0x61570, 0xBDA30]
                .iter()
                .position(|&v| v == which)
                .unwrap();
            nonzero[slot] += 1;
        }
        check_outputs("objects_class", index, case, &memory);
    }
    assert!(
        nonzero[0] > 5 && nonzero[2] > 3 && nonzero[3] > 3 && nonzero[4] > 5,
        "{nonzero:?}"
    );
}

#[test]
fn poll_matches_original() {
    let mut seen = [0usize; 2];
    for (index, case) in load("objects_poll").iter().enumerate() {
        let mut memory = Mem::new(case);
        let result = poll(&mut memory, case.input(STACK_ARGS));
        assert_eq!(result, case.eax, "case {index}");
        seen[result as usize] += 1;
        check_outputs("objects_poll", index, case, &memory);
    }
    assert!(seen[0] > 2 && seen[1] > 20, "{seen:?}");
}

#[test]
fn counters_match_original() {
    let (mut nonzero_sum, mut clamped) = (0, 0);
    for (index, case) in load("objects_counters").iter().enumerate() {
        let mut memory = Mem::new(case);
        let which = case.input(SELECT);
        let arg = |k: u32| case.input(STACK_ARGS + 4 * k);
        let result = match which {
            0x1B67E0 => controller_sum(&memory, arg(0), arg(1)) as u32,
            0x1BA220 => adjust_counter(&mut memory, arg(0), arg(1), arg(2) as i32) as u32,
            other => panic!("selector {other:#x}"),
        };
        assert_eq!(result, case.eax, "case {index} fn {which:#x}");
        if which == 0x1B67E0 && result != 0 {
            nonzero_sum += 1;
        }
        if which == 0x1BA220 && result != arg(2) {
            clamped += 1;
        }
        check_outputs("objects_counters", index, case, &memory);
    }
    assert!(nonzero_sum > 5 && clamped > 5, "{nonzero_sum} {clamped}");
}

#[test]
fn push_matches_original() {
    let mut written = 0;
    for (index, case) in load("objects_push").iter().enumerate() {
        let mut memory = Mem::new(case);
        let which = case.input(SELECT);
        let arg = |k: u32| case.input(STACK_ARGS + 4 * k);
        match which {
            0xD1EC0 => push_player(&mut memory, arg(0), [arg(1), arg(2), arg(3)]),
            0xD1F10 => push_all_players(&mut memory, [arg(0), arg(1), arg(2)]),
            other => panic!("selector {other:#x}"),
        }
        written += case
            .outs
            .iter()
            .filter(|(a, v)| case.input(**a) != **v)
            .count();
        check_outputs("objects_push", index, case, &memory);
    }
    assert!(written > 20, "{written}");
}

/// Answers the engine calls of the destroy path from the golden call log (callee and arguments checked in order)
/// and the configured results at `RETS` (slot per callee).
struct Recorded<'a> {
    case: &'a Case,
    next: usize,
}

const RETS: u32 = 0x32A_0000;
const SLOTS: [(u32, usize); 22] = [
    (0xD5480, 0),
    (0x35CC10, 1),
    (0x3533F0, 2),
    (0x17BF70, 3),
    (0x22DD10, 4),
    (0x278040, 5),
    (0x3D7B0, 6),
    (0x3C85D4, 7),
    (0x1CDAE0, 8),
    (0x12D350, 9),
    (0x67450, 10),
    (0xB9D00, 11),
    (0x165010, 12),
    (0x644D0, 13),
    (0x238510, 14),
    (0x12E3B0, 15),
    (0x22C3A0, 16),
    (0xCF440, 17),
    (0x12DFB0, 18),
    (0x1252C0, 19),
    (0x15DB20, 20),
    (0x2B24E0, 21),
];

impl DestroyEffects for Recorded<'_> {
    fn call(&mut self, _: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        let expected = self
            .case
            .calls
            .get(self.next)
            .unwrap_or_else(|| panic!("extra call {address:#x} {args:x?}"));
        assert_eq!(expected.callee, address, "call #{}", self.next);
        assert_eq!(expected.args, args, "call #{} {address:#x} args", self.next);
        self.next += 1;
        let slot = SLOTS.iter().find(|(a, _)| *a == address).unwrap().1;
        self.case.input(RETS + 4 * slot as u32)
    }
}

#[test]
fn destroy_matches_original() {
    let cases = load("objects_destroy");
    let mut seen: BTreeMap<u32, usize> = BTreeMap::new();
    for (index, case) in cases.iter().enumerate() {
        let mut memory = Mem::new(case);
        let mut effects = Recorded { case, next: 0 };
        match case.input(SELECT) {
            0xBABA0 => destroy_core(&mut memory, &mut effects, 0x320_0000, entry_sp(1)),
            0xBD090 => destroy_object(&mut memory, &mut effects, 0x321_0000, entry_sp(1)),
            other => panic!("selector {other:#x}"),
        }
        assert_eq!(
            effects.next,
            case.calls.len(),
            "case {index}: missing calls"
        );
        check_outputs("objects_destroy", index, case, &memory);
        for call in &case.calls {
            *seen.entry(call.callee).or_insert(0) += 1;
        }
    }
    // every sound id of the killer paths is exercised
    let mut sounds = std::collections::BTreeSet::new();
    for case in &cases {
        for call in case.calls.iter().filter(|c| c.callee == 0x238510) {
            sounds.insert(call.args[0]);
        }
    }
    for id in [0x5F, 0x48, 0x5E, 0x5B, 0x5D, 0x5C, 0x60, 0x57, 0x59, 0x58] {
        assert!(
            sounds.contains(&id),
            "sound {id:#x} never played, saw {sounds:x?}"
        );
    }
    // every engine callee of 0xBABA0 is exercised
    for (address, _) in SLOTS {
        assert!(
            seen.get(&address).copied().unwrap_or(0) > 0,
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn destroy_request_block_matches_original() {
    let cases = load("objects_bd090");
    for (index, case) in cases.iter().enumerate() {
        let mut memory = Mem::new(case);
        let (block, core_entry) = build_destroy_block(&mut memory, 0x321_0000, entry_sp(1));
        assert_eq!(case.calls.len(), 1);
        let call = &case.calls[0];
        assert_eq!(call.callee, 0xBABA0);
        assert_eq!(call.args[0], block, "case {index} block pointer");
        assert_eq!(core_entry, entry_sp(1) - 0x68, "entry of 0xBABA0");
        let words: Vec<u32> = (0..0x17).map(|k| memory.read_u32(block + 4 * k)).collect();
        assert_eq!(&call.args[1..], &words[..], "case {index} block words");
        check_outputs("objects_bd090", index, case, &memory);
    }
}

struct Rest;
impl StripServices for Rest {
    fn call(&mut self, _: &mut dyn GuestMemory, service: Service, _: &[u32]) -> u32 {
        100 + service as u32
    }
}

#[test]
fn object_services_dispatch_matches_the_functions() {
    let mut rest = Rest;
    let mut effects = NoEffects;
    let case = &load("objects_find")[0];
    let mut a = Mem::new(case);
    let mut b = Mem::new(case);
    let mut services = ObjectServices {
        rest: &mut rest,
        destroy: &mut effects,
        frame: entry_sp(1),
    };
    for tag in [0x48544950, 0x53414645, 0x41525452] {
        assert_eq!(
            services.call(&mut a, Service::FindObjectA, &[tag]),
            find_object_a(&b, tag)
        );
        assert_eq!(
            services.call(&mut a, Service::FindObjectB, &[tag]),
            find_object_b(&b, tag)
        );
        assert_eq!(
            services.call(&mut a, Service::LookupByKey, &[tag]),
            lookup_by_key(&mut b, tag)
        );
    }
    assert_eq!(
        services.call(&mut a, Service::CurrentOwner, &[2]),
        owner_record(2)
    );
    assert_eq!(services.call(&mut a, Service::Poll, &[7]), 1);
    assert_eq!(
        services.call(&mut a, Service::StartEffect, &[1, 2]),
        100 + Service::StartEffect as u32
    );
    assert_eq!(
        services.call(&mut a, Service::SpawnObject, &[0]),
        100 + Service::SpawnObject as u32
    );
}
