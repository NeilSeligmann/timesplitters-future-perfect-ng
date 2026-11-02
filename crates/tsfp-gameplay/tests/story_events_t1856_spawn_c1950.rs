//! T1856: the object spawner `0xC1950` and its callees against golden vectors from the original machine code under
//! the Unicorn oracle (MEASURED, xemu level). See `docs/t-story-event-spawn-c1950-t1856.md`.
mod common;
use common::{Case, parse};
use std::collections::{HashMap, HashSet};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_spawn_c1950::*;

/// Sparse byte memory. Reading a file backed data address (below the bss) that the case did not poke is an error:
/// the oracle would have read real game bytes there while this model reads zero.
struct Memory {
    bytes: HashMap<u32, u8>,
    known: HashSet<u32>,
}

const FILE_BACKED: std::ops::Range<u32> = 0x12000..0x54E550;

impl Memory {
    fn from_case(case: &Case) -> Memory {
        let mut memory = Memory {
            bytes: HashMap::new(),
            known: HashSet::new(),
        };
        for (&address, &word) in &case.ins {
            for k in 0..4 {
                memory.bytes.insert(address + k, (word >> (8 * k)) as u8);
                memory.known.insert(address + k);
            }
        }
        memory
    }
}

impl GuestMemory for Memory {
    fn read_u8(&self, address: u32) -> u8 {
        if FILE_BACKED.contains(&address) {
            assert!(
                self.known.contains(&address),
                "read of unpoked file backed address {address:#x}"
            );
        }
        *self.bytes.get(&address).unwrap_or(&0)
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        self.known.insert(address);
        self.bytes.insert(address, value);
    }
    fn read_u32(&self, address: u32) -> u32 {
        (0..4).fold(0, |word, k| {
            word | u32::from(self.read_u8(address.wrapping_add(k))) << (8 * k)
        })
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        for k in 0..4 {
            self.write_u8(address.wrapping_add(k), (value >> (8 * k)) as u8);
        }
    }
}

fn entry_esp(args: u32) -> u32 {
    0x7F00_0000 - 0x1000 - 4 * args - 4
}

fn queue(va: u32) -> u32 {
    0x280_0000 + (va & 0xF_FFFF) * 0x40
}

/// Replays the queued return values of the stubs and records every call as (callee, arguments).
struct Recorder<'a> {
    case: &'a Case,
    log: Vec<(u32, Vec<u32>)>,
    taken: HashMap<u32, u32>,
}

impl<'a> Recorder<'a> {
    fn new(case: &'a Case) -> Self {
        Recorder {
            case,
            log: Vec::new(),
            taken: HashMap::new(),
        }
    }
    fn call(&mut self, va: u32, args: &[u32]) -> u32 {
        self.log.push((va, args.to_vec()));
        let n = self.taken.entry(va).or_insert(0);
        let value = self.case.input(queue(va) + 4 * *n);
        *n += 1;
        value
    }
    fn assert_log(&self, case: &Case, label: &str) {
        assert_eq!(
            self.log.len(),
            case.calls.len(),
            "{label}: call count, log {:x?} expected {:x?}",
            self.log,
            case.calls
        );
        for (index, (va, args)) in self.log.iter().enumerate() {
            let expected = &case.calls[index];
            assert_eq!(*va, expected.callee, "{label}: callee of call {index}");
            assert_eq!(
                args, &expected.args,
                "{label}: args of call {index} ({va:#x})"
            );
        }
    }
}

fn check_outputs(case: &Case, memory: &Memory, label: &str) {
    for (&address, &word) in &case.outs {
        assert_eq!(
            memory.read_u32(address),
            word,
            "{label}: memory {address:#x}"
        );
    }
}

fn args_of(case: &Case, count: u32) -> Vec<u32> {
    (0..count)
        .map(|i| case.input(entry_esp(count) + 4 + 4 * i))
        .collect()
}

// -------------------------------------------------------------------------------------------------------------
// Whole root with the twelve direct callees stubbed
// -------------------------------------------------------------------------------------------------------------

struct RootCallees<'a>(Recorder<'a>);

impl SpawnCallees for RootCallees<'_> {
    fn lookup_record(&mut self, _: &mut dyn GuestMemory, kind: u32, index: u32) -> u32 {
        self.0.call(0xBE860, &[kind, index])
    }
    fn model_for(&mut self, _: &mut dyn GuestMemory, record: u32) -> u32 {
        self.0.call(0xDB2C0, &[record])
    }
    fn allocate(&mut self, mem: &mut dyn GuestMemory, model: u32, params: [u32; 4]) -> u32 {
        let mut args = vec![model, params[0], params[1], params[2], params[3]];
        args.extend((0..3).map(|k| mem.read_u32(params[0].wrapping_add(4 * k))));
        self.0.call(0xA6F90, &args)
    }
    fn reset_children(&mut self, _: &mut dyn GuestMemory, object: u32) {
        self.0.call(0x10E110, &[object]);
    }
    fn clear_child_flag(&mut self, _: &mut dyn GuestMemory, object: u32) {
        self.0.call(0x10E6F0, &[object]);
    }
    fn table_remove(&mut self, _: &mut dyn GuestMemory, object: u32) {
        self.0.call(0x2B26F0, &[object]);
    }
    fn destroy_with_hook(&mut self, _: &mut dyn GuestMemory, object: u32) {
        self.0.call(0xA9070, &[object]);
    }
    fn set_callback(&mut self, _: &mut dyn GuestMemory, object: u32, slot: u32, function: u32) {
        self.0.call(0xA5CE0, &[object, slot, function]);
    }
    fn init_motion(&mut self, _: &mut dyn GuestMemory, object: u32, mode: u32, scale: u32) {
        self.0.call(0x966E0, &[object, mode, scale]);
    }
    fn random(&mut self, _: &mut dyn GuestMemory) -> u32 {
        self.0.call(0x153B40, &[])
    }
    fn next_id(&mut self, _: &mut dyn GuestMemory) -> u32 {
        self.0.call(0x358160, &[])
    }
    fn destroy(&mut self, _: &mut dyn GuestMemory, object: u32) {
        self.0.call(0xA7090, &[object]);
    }
}

fn run_root(case: &Case) -> (Memory, RootCallees<'_>, u32) {
    let mut memory = Memory::from_case(case);
    let mut callees = RootCallees(Recorder::new(case));
    let frame = entry_esp(10);
    let args: [u32; 10] = args_of(case, 10).try_into().unwrap();
    let result = spawn_object(&mut memory, &mut callees, frame, args);
    (memory, callees, result)
}

#[test]
fn root_matches_original_with_stubbed_callees() {
    let cases = parse(include_str!("data/t1856/spawn_c1950_root.golden"));
    assert!(cases.len() >= 60, "{} cases", cases.len());
    let mut returned_null = 0;
    let mut stats: HashMap<u32, usize> = HashMap::new();
    for (n, case) in cases.iter().enumerate() {
        let (memory, callees, result) = run_root(case);
        let label = format!("root case {n}");
        callees.0.assert_log(case, &label);
        assert_eq!(result, case.eax, "{label}: return value");
        check_outputs(case, &memory, &label);
        returned_null += usize::from(result == 0);
        for (va, _) in &callees.0.log {
            *stats.entry(*va).or_default() += 1;
        }
    }
    // every direct callee is exercised, both outcomes of the static ring are seen
    for va in [
        0xBE860, 0xDB2C0, 0xA6F90, 0x10E110, 0x10E6F0, 0x2B26F0, 0xA9070, 0xA5CE0, 0x966E0,
        0x153B40, 0x358160, 0xA7090,
    ] {
        assert!(
            stats.get(&va).copied().unwrap_or(0) > 2,
            "callee {va:#x}: {stats:?}"
        );
    }
    assert!(returned_null > 0 && returned_null < cases.len());
}

// -------------------------------------------------------------------------------------------------------------
// Closure: ported callees, engine leaves recorded
// -------------------------------------------------------------------------------------------------------------

struct Leaves<'a>(Recorder<'a>);

impl SpawnLeaves for Leaves<'_> {
    fn model_hook(&mut self, _: &mut dyn GuestMemory, function: u32, arg: u32) -> u32 {
        self.0.call(function, &[arg])
    }
    fn dispatch(&mut self, _: &mut dyn GuestMemory, function: u32, object: u32, arg: u32) {
        self.0.call(function, &[object, arg]);
    }
    fn pool_get(&mut self, _: &mut dyn GuestMemory, model: u32, p1: u32, p2: u32) -> u32 {
        self.0.call(0xA6880, &[model, p1, p2])
    }
    fn init_object(&mut self, _: &mut dyn GuestMemory, object: u32, p4: u32) {
        self.0.call(0x6B060, &[object, p4]);
    }
    fn alloc_hook(&mut self, _: &mut dyn GuestMemory, function: u32, object: u32, p4: u32) {
        self.0.call(function, &[object, p4]);
    }
    fn post_alloc(&mut self, _: &mut dyn GuestMemory, object: u32, mode: u32) -> u32 {
        self.0.call(0x12E30, &[object, mode])
    }
    fn detach(&mut self, _: &mut dyn GuestMemory, object: u32) {
        self.0.call(0x10C0E0, &[object]);
    }
    fn destroy(&mut self, _: &mut dyn GuestMemory, object: u32) {
        self.0.call(0xA7090, &[object]);
    }
}

#[test]
fn closure_matches_original_with_stubbed_engine_leaves() {
    let cases = parse(include_str!("data/t1856/spawn_c1950_closure.golden"));
    assert!(cases.len() >= 60, "{} cases", cases.len());
    let mut stats: HashMap<u32, usize> = HashMap::new();
    let mut nulls = 0;
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let mut native = NativeCallees {
            leaves: Leaves(Recorder::new(case)),
        };
        let args: [u32; 10] = args_of(case, 10).try_into().unwrap();
        let result = spawn_object(&mut memory, &mut native, entry_esp(10), args);
        let label = format!("closure case {n}");
        native.leaves.0.assert_log(case, &label);
        assert_eq!(result, case.eax, "{label}: return value");
        check_outputs(case, &memory, &label);
        nulls += usize::from(result == 0);
        for (va, _) in &native.leaves.0.log {
            *stats.entry(*va).or_default() += 1;
        }
    }
    for va in [0xA6880, 0x6B060, 0x12E30, 0xA7090, 0x10C0E0] {
        assert!(
            stats.get(&va).copied().unwrap_or(0) > 1,
            "leaf {va:#x}: {stats:?}"
        );
    }
    assert!(
        stats.keys().any(|&va| va >= 0x160_0100),
        "no dispatch call seen"
    );
    assert!(nulls > 0 && nulls < cases.len());
}

// -------------------------------------------------------------------------------------------------------------
// Per function goldens
// -------------------------------------------------------------------------------------------------------------

#[test]
fn lookup_record_be860_matches_original() {
    let cases = parse(include_str!("data/t1856/spawn_be860.golden"));
    assert!(cases.len() >= 60);
    let mut nonzero = 0;
    for (n, case) in cases.iter().enumerate() {
        let memory = Memory::from_case(case);
        let args = args_of(case, 2);
        let result = lookup_record(&memory, args[0], args[1]);
        assert_eq!(
            result, case.eax,
            "case {n} kind {:#x} index {:#x}",
            args[0], args[1]
        );
        nonzero += usize::from(result != 0);
    }
    assert!(
        nonzero > 20 && nonzero < cases.len() - 5,
        "nonzero {nonzero}"
    );
}

#[test]
fn rng_153b40_matches_original() {
    let cases = parse(include_str!("data/t1856/spawn_153b40.golden"));
    assert!(cases.len() >= 40);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let result = rng_next(&mut memory);
        assert_eq!(result, case.eax, "case {n}");
        check_outputs(case, &memory, &format!("rng case {n}"));
    }
}

#[test]
fn small_callees_match_original() {
    let cases = parse(include_str!("data/t1856/spawn_small.golden"));
    assert!(cases.len() >= 60);
    let mut seen = [0usize; 3];
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let which = case.input(0x7F10_0000);
        seen[which as usize] += 1;
        let result = match which {
            0 => {
                clear_child_flag(&mut memory, args_of(case, 1)[0]);
                case.eax
            }
            1 => table_remove(&mut memory, args_of(case, 1)[0]),
            _ => next_object_id(&mut memory),
        };
        assert_eq!(result, case.eax, "small case {n} (function {which})");
        check_outputs(case, &memory, &format!("small case {n}"));
    }
    assert!(seen.iter().all(|&count| count >= 20));
}

#[test]
fn motion_966e0_matches_original() {
    let cases = parse(include_str!("data/t1856/spawn_966e0.golden"));
    assert!(cases.len() >= 40);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let args = args_of(case, 3);
        init_motion(&mut memory, args[0], args[1], f32::from_bits(args[2]));
        check_outputs(case, &memory, &format!("motion case {n}"));
    }
}

#[test]
fn callback_a5ce0_matches_original() {
    let cases = parse(include_str!("data/t1856/spawn_a5ce0.golden"));
    assert!(cases.len() >= 60);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let args = args_of(case, 3);
        set_callback(&mut memory, args[0], args[1], args[2]);
        check_outputs(case, &memory, &format!("callback case {n}"));
    }
}

#[test]
fn allocate_a6f90_matches_original() {
    let cases = parse(include_str!("data/t1856/spawn_a6f90.golden"));
    assert!(cases.len() >= 60);
    let mut nulls = 0;
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let mut leaves = Leaves(Recorder::new(case));
        let args = args_of(case, 5);
        let result = allocate_a6f90(
            &mut memory,
            &mut leaves,
            args[0],
            [args[1], args[2], args[3], args[4]],
        );
        let label = format!("alloc case {n}");
        leaves.0.assert_log(case, &label);
        assert_eq!(result, case.eax, "{label}: return");
        check_outputs(case, &memory, &label);
        nulls += usize::from(result == 0);
    }
    assert!(nulls > 0);
}

#[test]
fn release_matches_original() {
    let cases = parse(include_str!("data/t1856/spawn_release.golden"));
    assert!(cases.len() >= 60);
    let mut unlinked = 0;
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let mut native = NativeCallees {
            leaves: Leaves(Recorder::new(case)),
        };
        let object = args_of(case, 1)[0];
        let label = format!("release case {n}");
        if case.input(0x7F10_0000) == 0 {
            native.destroy_with_hook(&mut memory, object);
            // 0xA9070 pushes only the destroy and the dispatch entry, never the other leaves
            assert!(
                native
                    .leaves
                    .0
                    .log
                    .iter()
                    .all(|(va, _)| *va == 0xA7090 || *va >= 0x160_0000)
            );
            // the stub log of the original also holds 0xA7090 and the dispatch entry only
        } else {
            native.reset_children(&mut memory, object);
            unlinked += native
                .leaves
                .0
                .log
                .iter()
                .filter(|(va, _)| *va == 0x10C0E0)
                .count();
        }
        native.leaves.0.assert_log(case, &label);
        check_outputs(case, &memory, &label);
    }
    assert!(unlinked > 20, "unlinked {unlinked}");
}
