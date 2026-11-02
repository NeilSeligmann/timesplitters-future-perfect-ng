//! T1870: the trail and light service `0xBB850` against emulated-original golden vectors (MEASURED).
//! Golden: `tests/data/t1870/light_trail.golden` (`tools/t1846_gold/svc_light.py`); the services are replaced by scripted
//! answers saved in the golden (tape at `0xE1000000`), the light records handed to `0xBB6C0` are saved at `0xE2000000 + 0x100 * k`.
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::svc_light::*;

const COUNTER: u32 = 0x160_00FC;
const REC: u32 = 0x160_1000;
const POS: u32 = 0x160_2000;
const SCRIPT: u32 = 0xE100_0000;
const EXTRAS: u32 = 0xE200_0000;

struct Host {
    case: usize,
    memory: BTreeMap<u32, u32>,
    tape: Vec<u32>,
    cursor: usize,
    low: u32,
    high: u32,
    draws: u32,
    log: Vec<(u32, Vec<u32>)>,
    applied: Vec<LightCall>,
}

impl Host {
    fn next(&mut self) -> u32 {
        let value = *self
            .tape
            .get(self.cursor)
            .unwrap_or_else(|| panic!("case {}: script tape exhausted", self.case));
        self.cursor += 1;
        value
    }
    fn vector(&mut self) -> [f32; 3] {
        [0, 1, 2].map(|_| f32::from_bits(self.next()))
    }
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|c| c.to_bits()).collect()
}

impl LightHost for Host {
    fn read(&mut self, address: u32) -> u32 {
        *self.memory.get(&address).unwrap_or(&0)
    }
    fn write(&mut self, address: u32, value: u32) {
        self.memory.insert(address, value);
    }
    fn rng(&mut self) -> u32 {
        self.draws += 1;
        let state = (u64::from(self.high) << 32) | u64::from(self.low);
        let next = state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
        self.low = next as u32;
        self.high = (next >> 32) as u32;
        next as u32
    }
    fn ground_effect(&mut self, position: [f32; 3], height: f32, word: u32) {
        assert_eq!(
            position,
            [0, 1, 2]
                .map(|k| f32::from_bits(self.memory.get(&(POS + 4 * k)).copied().unwrap_or(0)))
        );
        self.log.push((0xF4920, vec![POS, height.to_bits(), word]));
    }
    fn rooms_in_box(&mut self, region: [f32; 6]) -> Option<Vec<u32>> {
        let count = self.next();
        let ids: Vec<u32> = (0..count).map(|_| self.next()).collect();
        self.log
            .push((0x873C0, [vec![0, 0x1E], bits(&region)].concat()));
        Some(ids)
    }
    fn light_room(&mut self, room_object: u32, strength: f32, _region: [f32; 6]) {
        self.log
            .push((0x12A590, vec![room_object, strength.to_bits(), 1]));
    }
    fn light_object(&mut self, object: u32, strength: f32) {
        self.log
            .push((0x12A590, vec![object, strength.to_bits(), 0]));
    }
    fn actor_gate(&mut self) -> bool {
        let answer = self.next();
        self.log.push((0x3533F0, vec![]));
        answer != 0
    }
    fn actor_check(&mut self, value: u32) -> bool {
        let answer = self.next();
        self.log.push((0x3568C0, vec![value]));
        answer != 0
    }
    fn actor_centre(&mut self, object: u32) -> [f32; 3] {
        self.log.push((0x2220E0, vec![object]));
        self.vector()
    }
    fn occluded(&mut self, cell: u32, from: [f32; 3], vector: [f32; 3]) -> bool {
        let answer = self.next();
        self.log
            .push((0x88C60, [vec![cell], bits(&from), bits(&vector)].concat()));
        answer != 0
    }
    fn contact(
        &mut self,
        word: u32,
        position: [f32; 3],
        direction: [f32; 3],
    ) -> Option<([f32; 3], [f32; 3])> {
        let answer = self.next();
        let point = self.vector();
        let normal = self.vector();
        self.log.push((
            0x919D0,
            [vec![word], bits(&position), bits(&direction)].concat(),
        ));
        (answer != 0).then_some((point, normal))
    }
    fn apply(&mut self, call: LightCall) {
        self.applied.push(call);
    }
}

fn normalise(call: &common::Call) -> (u32, Vec<u32>) {
    let a = &call.args;
    let words = match call.callee {
        0xF4920 => vec![a[0], a[1], a[2]],
        0x873C0 => [&[a[0], a[3]][..], &a[4..10]].concat(),
        0x12A590 => vec![a[0], a[2], u32::from(a[3] != 0)],
        0x3568C0 => vec![a[0]],
        0x2220E0 => vec![a[0]],
        0x88C60 => {
            assert_eq!(
                a[3..10],
                [0, 0, 0, 0, 0xFFFF_FFF9, 0, 0],
                "constant arguments of 0x88C60"
            );
            [&a[0..1], &a[10..16]].concat()
        }
        0x919D0 => {
            assert_eq!(a[3], 0);
            assert_eq!(a[6], 0);
            [&a[0..1], &a[7..13]].concat()
        }
        _ => a.clone(),
    };
    (call.callee, words)
}

fn extras_of(case: &Case, index: u32) -> Option<(Vec<u32>, [Vec<u32>; 3])> {
    let base = EXTRAS + 0x100 * index;
    case.outs.get(&base)?;
    let word = |k: u32| case.output(base + 4 * k);
    let words: Vec<u32> = (0..0x17).map(word).collect();
    let vec3 = |slot: u32| {
        (0..3)
            .map(|k| word(0x17 + 3 * slot + k))
            .collect::<Vec<u32>>()
    };
    Some((words, [vec3(0), vec3(1), vec3(2)]))
}

fn host_of(case: &Case, index: usize) -> Host {
    let memory: BTreeMap<u32, u32> = case
        .ins
        .iter()
        .filter(|(a, _)| **a < 0xE000_0000)
        .map(|(a, v)| (*a, *v))
        .collect();
    let mut tape = Vec::new();
    let mut address = SCRIPT;
    while let Some(value) = case.ins.get(&address) {
        tape.push(*value);
        address += 4;
    }
    Host {
        case: index,
        memory,
        tape,
        cursor: 0,
        low: case.input(0x4E_7950),
        high: case.input(0x4E_7954),
        draws: 0,
        log: vec![],
        applied: vec![],
    }
}

#[test]
fn trail_light_matches_the_original() {
    let cases = parse(include_str!("data/t1870/light_trail.golden"));
    assert!(cases.len() >= 100);
    let mut seen = std::collections::BTreeMap::<&str, usize>::new();
    for (index, case) in cases.iter().enumerate() {
        let mut host = host_of(case, index);
        let mut counter = case.input(COUNTER);
        let record: [u32; REC_WORDS] = std::array::from_fn(|k| case.input(REC + 4 * k as u32));
        let position = case.vec3(POS);
        let timer = f32::from_bits(case.input(0xF000_0000));
        trail_light(
            &mut host,
            &mut counter,
            &record,
            position,
            timer,
            case.input(0x4E_7944) as i32,
            case.input(0x4E_8CEC) as i32,
        );
        let expected: Vec<(u32, Vec<u32>)> = case
            .calls
            .iter()
            .filter(|c| c.callee != 0xBB6C0)
            .map(normalise)
            .collect();
        assert_eq!(host.log, expected, "case {index} service calls");
        assert_eq!(counter, case.output(COUNTER), "case {index} counter");
        assert_eq!(
            host.memory
                .get(&0x73_2DF0)
                .copied()
                .unwrap_or(case.input(0x73_2DF0)),
            case.output(0x73_2DF0),
            "case {index} recent count"
        );
        for slot in 0..0x28u32 {
            let address = 0x73_2D50 + 4 * slot;
            if let Some(want) = case.outs.get(&address) {
                assert_eq!(
                    host.memory
                        .get(&address)
                        .copied()
                        .unwrap_or(case.input(address)),
                    *want,
                    "case {index} recent {slot}"
                );
            }
        }
        assert_eq!(
            (host.low, host.high),
            (case.output(0x4E_7950), case.output(0x4E_7954)),
            "case {index} rng"
        );
        for k in 0..45u32 {
            let address = 0x161_0000 + 0x400 * k + 0x28;
            if let Some(want) = case.outs.get(&address) {
                assert_eq!(
                    host.memory
                        .get(&address)
                        .copied()
                        .unwrap_or(case.input(address)),
                    *want,
                    "case {index} flags of object {k}"
                );
            }
        }
        let applied = case.calls.iter().filter(|c| c.callee == 0xBB6C0).count();
        let _ = applied;
        let mut k = 0;
        while let Some((words, vectors)) = extras_of(case, k) {
            let call = &host.applied[k as usize];
            let mut want = words.clone();
            for pointer in [10usize, 11, 12] {
                want[pointer] = u32::from(want[pointer] != 0);
            }
            assert_eq!(call.words().to_vec(), want, "case {index} light {k} words");
            assert_eq!(
                bits(&call.point),
                vectors[0],
                "case {index} light {k} point"
            );
            if let Some(normal) = call.normal {
                assert_eq!(bits(&normal), vectors[1], "case {index} light {k} normal");
            }
            assert_eq!(
                bits(&call.direction),
                vectors[2],
                "case {index} light {k} direction"
            );
            *seen
                .entry(if call.short { "short" } else { "normal" })
                .or_default() += 1;
            k += 1;
        }
        assert_eq!(host.applied.len(), k as usize, "case {index} light count");
        for (callee, _) in &host.log {
            *seen
                .entry(match callee {
                    0xF4920 => "ground",
                    0x12A590 => "light12a590",
                    0x919D0 => "contact",
                    0x88C60 => "ray",
                    0x2220E0 => "centre",
                    0x3568C0 => "check",
                    _ => "other",
                })
                .or_default() += 1;
        }
        *seen
            .entry(if counter != case.input(COUNTER) {
                "emitted"
            } else {
                "gated"
            })
            .or_default() += 1;
    }
    println!("coverage {seen:?}");
    for (name, minimum) in [
        ("emitted", 40),
        ("gated", 5),
        ("ground", 5),
        ("light12a590", 10),
        ("contact", 5),
        ("ray", 15),
        ("normal", 10),
        ("short", 1),
    ] {
        assert!(
            seen.get(name).copied().unwrap_or(0) >= minimum,
            "coverage {name}: {seen:?}"
        );
    }
}
