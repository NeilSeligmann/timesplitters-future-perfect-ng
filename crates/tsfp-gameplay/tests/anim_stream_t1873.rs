//! T1873: the streamed key sampler `0x58DF0` and the key cache `0x60410` / `0x60390` against golden vectors from the
//! ORIGINAL machine code under the Unicorn oracle (MEASURED, xemu level; FABRICATED stream worlds, no disc clip is
//! streamed). Goldens `tests/data/t1873/stream_*.golden`, generator `tools/t1846_gold/stream_engine.py`.
mod common;
use common::{Case, parse};
use std::collections::{BTreeMap, BTreeSet};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;
use tsfp_gameplay::story_events::svc_stream::*;

const SELECT: u32 = 0x2FF_F000;
const STACK_ARGS: u32 = 0x2FF_F100;
const RETS: u32 = 0x3000_0000;

fn load(name: &str, minimum: usize) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1873/{name}.golden",
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

/// Replays the stubbed engine calls of the golden in order (callee and arguments checked).
struct Recorded<'a> {
    case: &'a Case,
    next: usize,
    reached: BTreeSet<u32>,
}

impl DestroyEffects for Recorded<'_> {
    fn call(&mut self, _memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        let expected = self
            .case
            .calls
            .get(self.next)
            .unwrap_or_else(|| panic!("extra call {address:#x} {args:x?}"));
        assert_eq!(expected.callee, address, "call #{}", self.next);
        assert_eq!(
            expected.args[..args.len()],
            *args,
            "call #{} {address:#x} args",
            self.next
        );
        self.next += 1;
        self.reached.insert(address);
        self.case.input(RETS + address)
    }
}

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

#[test]
fn key_base_matches_original() {
    drive("stream_base", 100, |_, m, _, case| {
        Some(stream_key_base(m, arg(case, 0)))
    });
}

#[test]
fn key_cache_matches_original() {
    drive("stream_cache", 150, |_, m, _, case| {
        Some(stream_key_cache(m, arg(case, 0), arg(case, 1)))
    });
}

#[test]
fn sampler_matches_original() {
    let (_, reached) = drive("stream_sample", 150, |_, m, e, case| {
        stream_sample(
            m,
            e,
            arg(case, 0),
            arg(case, 1),
            arg(case, 2),
            arg(case, 3),
            arg(case, 4),
        );
        None
    });
    assert!(
        reached.contains(&ERROR_REPORT),
        "the unknown selector report was never reached"
    );
}
