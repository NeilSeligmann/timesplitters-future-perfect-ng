//! T1863: the ember engine services (`0xEDBE0`, `0x46A00`, ...) against emulated-original golden vectors (MEASURED),
//! plus semantic tests. Golden: `tests/data/t1863/ember_*.golden` (`tools/t1846_gold/svc_ember_services.py`).
mod common;
use common::{Call, Case, parse};
use tsfp_gameplay::story_events::svc_effect_submit::{SparseSvcMemory, SvcMemory};
use tsfp_gameplay::story_events::svc_ember_services::*;

const ENT: u32 = 0x110_0000;
const POS: u32 = 0x114_0000;
const STACKARGS: u32 = 0x115_0000;

type Log = Vec<(u32, Vec<u32>)>;

fn world(case: &Case) -> SparseSvcMemory {
    let mut memory = SparseSvcMemory::default();
    for (&address, &value) in &case.ins {
        memory.w32(address, value);
    }
    memory
}

fn expected(case: &Case) -> Log {
    case.calls
        .iter()
        .map(|call: &Call| {
            let a = &call.args;
            let words = match call.callee {
                0x278040 => a[..1].to_vec(),
                0xE9380 => a[..7].to_vec(),
                0x45820 => a[1..13].to_vec(),
                0x27690 => a[..3].to_vec(),
                other => panic!("unexpected stub {other:#x}"),
            };
            (call.callee, words)
        })
        .collect()
}

struct Settle {
    gate: bool,
    log: Log,
}

impl SettleServices for Settle {
    fn actor_gate(&mut self, link: u32) -> bool {
        self.log.push((0x278040, vec![link]));
        self.gate
    }
    fn disturb(
        &mut self,
        mem: &mut dyn SvcMemory,
        position: u32,
        shape_end: u32,
        radius: f32,
        out: u32,
    ) {
        let mut words = vec![position, shape_end, radius.to_bits(), out];
        words.extend((0..3).map(|k| mem.r32(position + 4 * k)));
        self.log.push((0xE9380, words));
    }
}

#[test]
fn settle_matches_original() {
    let cases = parse(include_str!("data/t1863/ember_settle.golden"));
    assert!(cases.len() >= 100);
    let (mut disturbed, mut gated, mut skipped, mut fourth_power) = (0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = world(case);
        let mut services = Settle {
            gate: case.input(STACKARGS) != 0,
            log: Vec::new(),
        };
        settle_entry(
            &mut memory,
            &mut services,
            POS,
            case.input(STACKARGS + 4),
            ENT,
        );
        assert_eq!(services.log, expected(case), "case {n}");
        let called = |callee: u32| services.log.iter().any(|(c, _)| *c == callee);
        if called(0xE9380) {
            disturbed += 1;
            if memory.r32(ENT) == 0x17 {
                fourth_power += 1;
            }
        } else {
            skipped += 1;
        }
        if called(0x278040) {
            gated += 1;
        }
    }
    assert!(disturbed > 20 && gated > 5 && skipped > 20 && fourth_power > 5);
}

#[derive(Default)]
struct Pair {
    handles: Vec<u32>,
    next: usize,
    log: Log,
}

impl PairServices for Pair {
    fn submit(&mut self, request: [u32; 12]) -> i32 {
        self.log.push((0x45820, request.to_vec()));
        let handle = self.handles[self.next];
        self.next += 1;
        handle as i32
    }
    fn voice_volumes(&mut self, handle: i32, left: i32, right: i32) {
        self.log
            .push((0x27690, vec![handle as u32, left as u32, right as u32]));
    }
}

#[test]
fn sound_pair_matches_original() {
    let cases = parse(include_str!("data/t1863/ember_sound_pair.golden"));
    assert!(cases.len() >= 100);
    let (mut volumes, mut both, mut neither) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let memory = world(case);
        let mut services = Pair {
            handles: vec![case.input(0x117_0000), case.input(0x117_0004)],
            ..Pair::default()
        };
        let f = |k: u32| f32::from_bits(case.input(STACKARGS + 4 * k));
        sound_pair(
            &memory,
            &mut services,
            case.input(STACKARGS),
            f(1),
            f(2),
            f(3),
        );
        assert_eq!(services.log, expected(case), "case {n}");
        let calls = services.log.iter().filter(|(c, _)| *c == 0x27690).count();
        volumes += calls;
        if calls == 2 {
            both += 1;
        }
        if calls == 0 {
            neither += 1;
        }
    }
    assert!(volumes > 20 && both > 5 && neither > 5);
}

struct Pulse {
    script: Vec<u32>,
    next: usize,
    log: Log,
}

impl Pulse {
    fn answer(&mut self) -> u32 {
        let value = self.script[self.next];
        self.next += 1;
        value
    }
}

impl PulseServices for Pulse {
    fn node_pulse(&mut self, node: i32, value: f32) -> u32 {
        self.log
            .push((0x1836A0, vec![node as u32, value.to_bits()]));
        self.answer()
    }
    fn region_pulse(&mut self, position: u32) -> u32 {
        self.log.push((0x184040, vec![position]));
        self.answer()
    }
    fn submit(&mut self, request: [u32; 12]) {
        self.log.push((0x45820, request.to_vec()));
    }
}

#[test]
fn pulse_matches_original() {
    let cases = parse(include_str!("data/t1863/ember_pulse.golden"));
    assert!(cases.len() >= 100);
    let (mut pulsed, mut sounds, mut grouped, mut regions) = (0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = world(case);
        let mut services = Pulse {
            script: (0..120).map(|k| case.input(0x117_0000 + 4 * k)).collect(),
            next: 0,
            log: Vec::new(),
        };
        let radius = f32::from_bits(case.input(STACKARGS));
        let result = light_pulse(&mut memory, &mut services, POS, radius);
        let wanted: Log = case
            .calls
            .iter()
            .map(|call| {
                let a = &call.args;
                let words = match call.callee {
                    0x1836A0 => a[..2].to_vec(),
                    0x184040 => a[..1].to_vec(),
                    0x45820 => a[1..13].to_vec(),
                    other => panic!("unexpected stub {other:#x}"),
                };
                (call.callee, words)
            })
            .collect();
        assert_eq!(services.log, wanted, "case {n}");
        assert_eq!(result, case.eax, "case {n} result");
        for (&address, &value) in &case.outs {
            assert_eq!(memory.r32(address), value, "case {n} word {address:#x}");
        }
        let node_calls = services.log.iter().filter(|(c, _)| *c == 0x1836A0).count();
        pulsed += usize::from(node_calls > 0);
        sounds += usize::from(services.log.iter().any(|(c, _)| *c == 0x45820));
        grouped += usize::from(
            case.outs
                .iter()
                .any(|(&a, &v)| (0x74_C680..0x74_C6F8).contains(&a) && v == 1),
        );
        regions += usize::from(result > 1);
    }
    assert!(
        pulsed > 30 && sounds > 20 && grouped > 10,
        "{pulsed} {sounds} {grouped} {regions}"
    );
}

struct Burst {
    view: u32,
    region: u32,
    log: Log,
}

const BURST_REC: u32 = 0x11B_0000;

impl TriggerServices for Burst {
    fn allocate(&mut self) -> u32 {
        self.log.push((0xFF2B0, vec![1]));
        BURST_REC
    }
    fn view_mode(&mut self) -> u32 {
        self.log.push((0x1CDAE0, vec![]));
        self.view
    }
    fn region_query(&mut self, box_start: u32, ring: u32) -> u32 {
        // the box words are read by the caller of the test through the memory snapshot below
        self.log.push((0x873C0, vec![0, box_start, ring, 0xC]));
        self.region
    }
}

#[test]
fn burst_matches_original() {
    let cases = parse(include_str!("data/t1863/ember_burst.golden"));
    assert!(cases.len() >= 100);
    let (mut wide, mut tight, mut boxed, mut empty) = (0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = world(case);
        let mut services = Burst {
            view: case.input(STACKARGS + 8),
            region: case.input(STACKARGS + 12),
            log: Vec::new(),
        };
        let record = trigger_effect(
            &mut memory,
            &mut services,
            case.input(STACKARGS),
            case.input(STACKARGS + 4),
            0x11D_0000,
        );
        assert_eq!(record, case.eax, "case {n} result");
        // the original logs the six box words the query reads, which are the record words at +0x80
        let wanted: Log = case
            .calls
            .iter()
            .map(|call| {
                let a = &call.args;
                let words = match call.callee {
                    0xFF2B0 => a[..1].to_vec(),
                    0x1CDAE0 => vec![],
                    0x873C0 => a[..4].to_vec(),
                    other => panic!("unexpected stub {other:#x}"),
                };
                (call.callee, words)
            })
            .collect();
        assert_eq!(services.log, wanted, "case {n}");
        for call in case.calls.iter().filter(|c| c.callee == 0x873C0) {
            for (k, word) in call.args[4..10].iter().enumerate() {
                assert_eq!(
                    memory.r32(record + 0x80 + 4 * k as u32),
                    *word,
                    "case {n} box word {k}"
                );
            }
        }
        for (&address, &value) in &case.outs {
            assert_eq!(memory.r32(address), value, "case {n} word {address:#x}");
        }
        let count = memory.r32(record + 0x1C) as i32;
        wide += usize::from(count > 1);
        tight += usize::from(count == 1);
        empty += usize::from(count <= 0);
        boxed += usize::from(!case.calls.iter().all(|c| c.callee != 0x873C0));
    }
    assert!(
        wide > 40 && tight > 3 && empty > 5 && boxed > 20,
        "{wide} {tight} {empty} {boxed}"
    );
}
