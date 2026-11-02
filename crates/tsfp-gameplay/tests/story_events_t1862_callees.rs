//! T1862 batch 3: the remaining engine callees of `0xBABA0` (`svc_destroy_callees`) against golden vectors from the
//! original machine code under the Unicorn oracle (MEASURED). Goldens: `tests/data/t1862/callees_*.golden`.
mod common;
use common::{Case, parse};
use std::collections::{BTreeMap, BTreeSet};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_destroy_callees::*;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;

const SELECT: u32 = 0x2FF_F000;
const STACK_ARGS: u32 = 0x2FF_F100;
const RETS: u32 = 0x3000_0000;
const ENTRY0: u32 = 0x7F00_0000 - 0x1000 - 4;

fn entry_sp(nargs: u32) -> u32 {
    ENTRY0 - 4 * nargs
}

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

/// Replays the engine calls of the golden: callee and arguments (stack words, dereferenced words and register
/// arguments in the stub's order) are checked in order, the result comes from `RETS + callee`.
struct Recorded<'a> {
    case: &'a Case,
    next: usize,
    seen: &'a mut BTreeSet<u32>,
}

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
        self.seen.insert(address);
        self.case.input(RETS + address)
    }
}

fn arg(case: &Case, index: u32) -> u32 {
    case.input(STACK_ARGS + 4 * index)
}

fn drive(
    name: &str,
    mut run: impl FnMut(u32, &mut Mem, &mut Recorded, &Case) -> Option<u32>,
) -> (
    BTreeMap<u32, usize>,
    BTreeSet<u32>,
    BTreeMap<u32, BTreeSet<u32>>,
) {
    let mut seen = BTreeMap::new();
    let mut callees = BTreeSet::new();
    let mut results: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for (index, case) in load(name).iter().enumerate() {
        let mut memory = Mem {
            words: case.ins.clone(),
        };
        let which = case.input(SELECT);
        let mut effects = Recorded {
            case,
            next: 0,
            seen: &mut callees,
        };
        let result = run(which, &mut memory, &mut effects, case);
        assert_eq!(
            effects.next,
            case.calls.len(),
            "{name} case {index} fn {which:#x}: missing calls"
        );
        if let Some(value) = result {
            assert_eq!(value, case.eax, "{name} case {index} fn {which:#x} result");
            results.entry(which).or_default().insert(value);
        }
        for (&address, &expected) in &case.outs {
            assert_eq!(
                memory.read_u32(address),
                expected,
                "{name} case {index} fn {which:#x}: memory {address:#x}"
            );
        }
        *seen.entry(which).or_insert(0) += 1;
    }
    (seen, callees, results)
}

#[test]
fn session_and_rules_match_original() {
    let (seen, callees, results) = drive("callees_session", |which, m, e, case| match which {
        0x355EC0 => Some(is_server(m)),
        0x355F10 => Some(is_client(m)),
        0x357690 => Some(host_stat(m, e)),
        0x359400 => {
            write_ref(m, arg(case, 0), arg(case, 1));
            None
        }
        0x2776F0 => Some(level_rule(m, arg(case, 0))),
        0x278040 => Some(kill_test(m, arg(case, 0))),
        other => panic!("selector {other:#x}"),
    });
    assert_eq!(seen.len(), 6);
    assert!(callees.contains(&0x3570D0));
    for which in [0x355EC0, 0x355F10, 0x2776F0, 0x278040] {
        assert_eq!(results[&which].len(), 2, "{which:#x} both outcomes");
    }
}

#[test]
fn owner_packet_matches_original() {
    let (_, callees, results) = drive("callees_owner", |_, m, e, case| {
        Some(owner_test(m, e, arg(case, 0), arg(case, 1), entry_sp(2)))
    });
    assert!(callees.contains(&0x35BE90) && callees.contains(&0x35AF40));
    assert!(results[&0x35CC10].len() >= 3, "{:?}", results[&0x35CC10]);
}

#[test]
fn hud_ring_matches_original() {
    let (_, callees, results) = drive("callees_hud", |_, m, e, case| {
        Some(hud_message(
            m,
            e,
            arg(case, 0),
            arg(case, 1),
            arg(case, 2),
            arg(case, 3),
        ))
    });
    for address in [0x3C8960, 0x40F30, 0x3C848C, 0x40650] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
    let nonzero = results[&0x12D350].iter().filter(|&&r| r != 0).count();
    assert!(nonzero >= 8 && results[&0x12D350].contains(&0), "{nonzero}");
}

#[test]
fn phase_flags_and_death_hook_match_original() {
    let (_, callees, _) = drive("callees_flags", |_, m, e, case| {
        set_flags(m, e, arg(case, 0));
        None
    });
    for address in [0x66E90, 0x66FC0, 0x231A20] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
    let (_, callees, _) = drive("callees_death", |_, m, e, case| {
        death_hook(m, e, arg(case, 0));
        None
    });
    for address in [0xF8AD0, 0xFD7E0, 0x3C8960, 0x66FC0] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn finish_slot_matches_original() {
    let (_, callees, _) = drive("callees_finish", |_, m, e, case| {
        finish_slot(m, e, arg(case, 0));
        None
    });
    for address in [
        0x238E00, 0x153B40, 0x218A0, 0xD3DE0, 0xD3F80, 0x64D10, 0x121BF0, 0x25A780, 0xD3C50,
        0x2E6D40,
    ] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn kill_feed_matches_original() {
    let (_, callees, _) = drive("callees_feed", |_, m, e, case| {
        kill_feed(m, e, arg(case, 0));
        None
    });
    for address in [0x164F60, 0x164A30, 0xD6C90] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn score_update_matches_original() {
    let (_, callees, results) = drive("callees_score", |_, m, e, case| {
        score_update(m, e, arg(case, 0), arg(case, 1), arg(case, 2)).map(|address| {
            // the golden `ret` is eax, only defined on this path
            address
        });
        None
    });
    assert!(callees.contains(&0xD6C90) && callees.contains(&0x641A0));
    assert!(results.is_empty());
}

#[test]
fn announce_last_point_matches_original() {
    let (_, callees, _) = drive("callees_announce", |_, m, e, case| {
        announce_last_point(m, e, arg(case, 0), entry_sp(1));
        None
    });
    for address in [0x64160, 0x641C0, 0x151030, 0x3C8960, 0x45390, 0x45820] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn start_blend_matches_original() {
    let (_, callees, _) = drive("callees_blend", |_, m, e, case| {
        // the caller's argument slots live above the entry esp
        for k in 0..3 {
            m.write_u32(entry_sp(3) + 4 + 4 * k, arg(case, k));
        }
        start_blend(m, e, arg(case, 0), arg(case, 1), arg(case, 2), entry_sp(3));
        None
    });
    for address in [
        0x350E70, 0x35C4E0, 0x9E720, 0x174E0, 0x17570, 0x5F460, 0x11D8A0, 0x378530, 0x10E1C0,
    ] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn free_object_matches_original() {
    let (_, callees, _) = drive("callees_free", |_, m, e, case| {
        free_object(m, e, arg(case, 0), entry_sp(1));
        None
    });
    for address in [
        0x15C310, 0x1642C0, 0x27C2F0, 0x8E970, 0x238E00, 0x59D20, 0x613D0, 0x161D40, 0xD7D10,
        0x17570, 0x59E00, 0xDAAE0, 0xD6DD0, 0x11ECE0, 0x11E7B0,
    ] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn final_hook_matches_original() {
    let (_, callees, _) = drive("callees_final", |_, m, e, case| {
        final_hook(m, e, arg(case, 0));
        None
    });
    for address in [0x2B23D0, 0x15BD00, 0x17570] {
        assert!(
            callees.contains(&address),
            "callee {address:#x} never reached"
        );
    }
}

#[test]
fn format_text_matches_original() {
    let (_, callees, results) = drive("callees_format", |_, m, e, case| {
        Some(format_text(m, e, arg(case, 0), arg(case, 1), entry_sp(3)))
    });
    assert!(callees.contains(&0x3CB59D));
    assert!(!results.is_empty());
}

#[test]
fn whole_destroy_root_over_the_ported_callees_matches_original() {
    use tsfp_gameplay::story_events::svc_objects::destroy_core;
    let mut engine = BTreeSet::new();
    let (_, callees, _) = drive("callees_root", |_, m, e, _case| {
        let mut ported = PortedDestroy {
            inner: e,
            entry: entry_sp(1),
        };
        destroy_core(m, &mut ported, 0x320_0000, entry_sp(1));
        None
    });
    engine.extend(callees);
    // the engine systems below the callees are all reached by some case
    for address in [
        0x35BE90, 0x3C8960, 0x238E00, 0x378530, 0x15C310, 0x2B23D0, 0x35C4E0, 0x9E720, 0x174E0,
        0x17570, 0x66FC0, 0x11D8A0, 0x10E1C0, 0x59D20, 0x59E00, 0x613D0, 0x64D10, 0xD3C50, 0xD3F80,
        0xD7D10,
    ] {
        assert!(
            engine.contains(&address),
            "engine routine {address:#x} never reached"
        );
    }
}

#[test]
fn hud_post_matches_original() {
    let (seen, callees, _) = drive("callees_post", |which, m, e, case| {
        let kind = if which == 0x12D6B0 { 0xA } else { 0xC };
        hud_post(
            m,
            e,
            kind,
            arg(case, 0),
            arg(case, 1),
            arg(case, 2),
            arg(case, 3),
        );
        None
    });
    assert_eq!(seen.len(), 2);
    assert!(callees.contains(&0x3C8960));
}
