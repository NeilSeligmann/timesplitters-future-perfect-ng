//! T1872: the pose and skeleton engine below `0x976E0` (`svc_pose`) against golden vectors from the original machine
//! code under the Unicorn oracle (MEASURED, xemu level). Goldens: `tests/data/t1872/pose_*.golden` (generator
//! `tools/t1846_gold/pose_engine.py`). T1875: `pose_bone`, `pose_blend` and `pose_root` were regenerated with the REAL
//! streamed sampler `0x58DF0` and matrix blend `0x1526C0` (the ports call `svc_stream::stream_sample` and
//! `matrix_blend_call` directly), so only the part visitor `0x903D0` and the error report `0x389B0` are replayed.
mod common;
use common::{Case, parse};
use std::collections::{BTreeMap, BTreeSet};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;
use tsfp_gameplay::story_events::svc_pose::*;

const SELECT: u32 = 0x2FF_F000;
const STACK_ARGS: u32 = 0x2FF_F100;
const RETS: u32 = 0x3000_0000;

fn load(name: &str, minimum: usize) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1872/{name}.golden",
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
fn misc_matches_original() {
    let (seen, _) = drive("pose_misc", 144, |which, m, _, case| match which {
        0x58270 => Some(pose_context(m, arg(case, 0))),
        0x59BF0 => {
            set_tag(m, arg(case, 0));
            None
        }
        0x59CB0 => {
            stack_pop(m);
            None
        }
        0x153240 => {
            identity(m, arg(case, 0));
            None
        }
        0x1531D0 => {
            matrix_copy(m, arg(case, 0), arg(case, 1));
            None
        }
        0x1506B0 => {
            quat_matrix(m, arg(case, 0), arg(case, 1));
            None
        }
        other => panic!("selector {other:#x}"),
    });
    assert_eq!(seen.len(), 6);
    assert!(seen.values().all(|&n| n >= 10), "{seen:?}");
}

#[test]
fn stack_apply_matches_original() {
    drive("pose_stack", 60, |_, m, _, case| {
        stack_apply(m, arg(case, 0), arg(case, 1));
        None
    });
}

#[test]
fn chains_match_original() {
    drive("pose_chain_a", 48, |_, m, _, case| {
        pose_chain_a(m, arg(case, 0), arg(case, 1), arg(case, 2), arg(case, 3));
        None
    });
    drive("pose_chain_b", 48, |_, m, _, case| {
        pose_chain_b(m, arg(case, 0), arg(case, 1), arg(case, 2), arg(case, 3));
        None
    });
}

#[test]
fn blend_walk_matches_original() {
    let (seen, reached) = drive("pose_blend", 48, |_, m, e, case| {
        pose_blend_walk(m, e, arg(case, 0), arg(case, 1), arg(case, 2));
        None
    });
    assert!(
        !seen.is_empty() && reached.is_empty(),
        "the real matrix blend leaves no replayed call"
    );
}

#[test]
fn visit_parts_matches_original() {
    let (_, reached) = drive("pose_visit", 48, |_, m, e, case| {
        visit_parts(m, e, arg(case, 0), arg(case, 1), arg(case, 2));
        None
    });
    assert!(reached.contains(&0x903D0));
}

#[test]
fn bone_matches_original() {
    let mut streamed = 0;
    let (_, reached) = drive("pose_bone", 183, |_, m, e, case| {
        streamed += usize::from(case.input(arg(case, 0).wrapping_add(8)) & 2 != 0);
        pose_bone(
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
    assert!(streamed >= 20, "only {streamed} streamed clips");
    assert!(
        reached.iter().all(|callee| *callee == 0x389B0),
        "{reached:x?}"
    );
}

#[test]
fn unpack_quaternion_matches_original() {
    drive("pose_unpack", 60, |_, m, _, case| {
        unpack_quaternion(m, arg(case, 0), arg(case, 1));
        None
    });
}

/// The whole root `0x976E0` (`shape_dynamic`) with the pose engine ported: the only recorded calls are the deeper
/// engine routines `0x903D0` and the error report `0x389B0` (`0x58DF0`, `0x1526C0` run as ports); memory (scratch, matrices, stack cells, frame locals) must equal
/// the original running every pose routine for real.
#[test]
fn whole_root_matches_original() {
    use tsfp_gameplay::story_events::svc_shape_services::{PortedShapeServices, shape_dynamic};
    let (_, reached) = drive("pose_root", 40, |_, m, e, case| {
        let mut services = PortedShapeServices { inner: e, entry: 0 };
        let entry = 0x7F00_0000 - 0x1000 - 4 - 4;
        shape_dynamic(
            m,
            &mut services,
            entry,
            case.input(0x2FF_F140),
            case.input(0x2FF_F144),
            arg(case, 0),
        );
        None
    });
    assert!(reached.contains(&0x903D0));
}
