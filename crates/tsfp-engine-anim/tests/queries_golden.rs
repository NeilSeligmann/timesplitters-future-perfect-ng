//! T1668 tier 1: key number and event queries against the ORIGINAL (`tools/t1668_queries_gold.py`, worlds regenerated from the seed).
use tsfp_engine_anim::layers::{Body, CURRENT, PREVIOUS};
use tsfp_engine_anim::queries::*;

mod support;
use support::Rng;

const TIMES: [f32; 8] = [0.0, 0.5, 1.0, 1.99, 0.0166, 2.0, 0.75, 1.25];
const SPEEDS: [f32; 5] = [0.0, 1.0, -1.0, 0.5, -2.0];
const BLENDS: [f32; 4] = [0.0, 0.5, -0.5, f32::NAN];
const FLAGS: [u16; 5] = [0, 1, 2, 4, 0xFFFF];
const MASKS: [u32; 6] = [0, 1, 2, 4, 0xFFFF, 0xFFFF_FFFF];

struct World {
    events: Vec<Option<Vec<Event>>>,
    body: Body,
}

fn world(seed: u64) -> (Rng, World) {
    let mut r = Rng(seed);
    let mut events = Vec::new();
    for _ in 0..4 {
        let null = r.below(4) == 0;
        let count = r.below(5);
        let list: Vec<Event> = (0..count)
            .map(|_| {
                let frame = if r.below(3) == 0 { 0 } else { r.below(150) as u16 };
                Event { frame, flags: FLAGS[r.below(5) as usize] }
            })
            .collect();
        events.push((!null).then_some(list));
    }
    let mut body = Body([0; tsfp_engine_anim::layers::BODY_DWORDS]);
    for off in [0xD4, 0x12C] {
        body.0[off / 4] = r.below(4) as u32;
    }
    for off in [0xDC, 0xE0, 0x134, 0x138] {
        body.0[off / 4] = TIMES[r.below(8) as usize].to_bits();
    }
    for off in [0x108, 0x160] {
        body.0[off / 4] = SPEEDS[r.below(5) as usize].to_bits();
    }
    body.0[0x198 / 4] = BLENDS[r.below(4) as usize].to_bits();
    (r, World { events, body })
}

impl World {
    fn list(&self, id: u32) -> Option<&[Event]> {
        self.events.get(id as usize).and_then(|e| e.as_deref())
    }
    /// (events of the active layer, current key, previous key, speed)
    fn active(&self) -> (Option<&[Event]>, i32, i32, f32) {
        let layer = self.body.active_layer();
        let (cur, prev) = key_numbers(self.body.float(layer + 8), self.body.float(layer + 0xC));
        (self.list(self.body.layer_id(layer)), cur, prev, self.body.float(layer + 0x34))
    }
}

#[test]
fn event_queries_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/queries.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut counts = std::collections::BTreeMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let function = u32::from_str_radix(f[0], 16).unwrap();
        let seed: u64 = f[1].parse().unwrap();
        let (mut r, w) = world(seed);
        let eax = |v: u32| format!("{v:x}");
        let got: String = match function {
            0x585E0 => {
                let layer = if r.below(2) != 0 { CURRENT } else { PREVIOUS };
                let (first_out, second_out) = (r.below(4) != 0, r.below(4) != 0);
                let (cur, prev) = key_numbers(w.body.float(layer + 8), w.body.float(layer + 0xC));
                let a = if first_out { format!("{:x}", cur as u32) } else { "7777".into() };
                let b = if second_out { format!("{:x}", prev as u32) } else { "7777".into() };
                assert_eq!(format!("{a} {b}"), format!("{} {}", f[3], f[4]), "{line}");
                "-".into()
            }
            0x613D0 => {
                let (id, mask, n) = (r.below(4) as u32, MASKS[r.below(6) as usize], r.below(3) as i32 + 1);
                eax(nth_event_frame(w.list(id), mask, n).unwrap_or(0xFFFF_FFFF))
            }
            0x61570 => {
                let mask = MASKS[r.below(6) as usize];
                let (events, cur, prev, _) = w.active();
                eax(u32::from(event_in_window(events, mask, cur, prev)))
            }
            0x614B0 => {
                let (mask, n) = (MASKS[r.below(6) as usize], r.below(3) as i32 + 1);
                let (events, cur, prev, speed) = w.active();
                eax(u32::from(passed_event(events, mask, n, speed, cur, prev)))
            }
            0x61610 => eax(w.active().1 as u32),
            0x61630 => {
                let (mask, n) = (MASKS[r.below(6) as usize], r.below(3) as i32 + 1);
                let (events, cur, _, _) = w.active();
                eax(u32::from(at_or_past_event(events, mask, n, cur)))
            }
            other => panic!("{other:x}"),
        };
        assert_eq!(got, f[2], "{line}");
        *counts.entry(function).or_insert(0usize) += 1;
    }
    assert_eq!(counts.len(), 6);
}
