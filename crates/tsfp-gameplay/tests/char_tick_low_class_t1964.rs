//! T1964: low-class NPC tick 0x2AEFD0 against private original worlds
//! (tools/t1877_low_class_tick_gold.py). Verification tier 1: lifted/original differential.
use tsfp_gameplay::char_tick_low_class::{
    self, Leaf, LowCtx, LowGlobals, LowObject, LowRecord, LowTarget, LowWorld,
};

struct World {
    i: Vec<u32>,
    calls: Vec<u32>,
    raw_index: usize,
}
fn f(v: u32) -> f32 {
    f32::from_bits(v)
}
fn target(i: &[u32], id: u32, extension: u32, flags: u32) -> LowTarget {
    LowTarget {
        id,
        position: [f(i[69]), f(i[70]), f(i[71])],
        extension,
        character_flags: flags,
    }
}
impl LowWorld for World {
    fn awareness(&mut self, _: &mut LowCtx) -> bool {
        self.calls.push(0x15e120);
        self.i[20] != 0
    }
    fn leaf(&mut self, which: Leaf, _: &mut LowCtx) -> u32 {
        let va = which.original_address();
        self.calls.push(va);
        match which {
            Leaf::Navigate | Leaf::Walk => 0,
            Leaf::BlockedA => self.i[18],
            Leaf::BlockedB => self.i[19],
            Leaf::Notify(flags) => {
                self.calls.extend([flags, 0]);
                0
            }
            Leaf::NodeIndex(arg) => {
                self.calls.push(arg as u32);
                self.i[53]
            }
            Leaf::Path(node) => {
                self.calls.push(node as u32);
                self.i[52]
            }
            Leaf::Floor(x) => {
                self.calls.extend([8, x]);
                self.i[50]
            }
            Leaf::Trigger => {
                self.calls.extend([1, 0]);
                0
            }
            Leaf::Blend => {
                self.calls.extend([1, 0x3ecccccd]);
                0
            }
            Leaf::BlendReady => {
                self.calls.push(0x3ea8f5c3);
                self.i[51]
            }
            Leaf::Obstacle => self.i[49],
            Leaf::Route(value) => {
                self.calls.extend([value, 0]);
                0
            }
        }
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        let v = self.i[21 + self.raw_index % 6];
        self.raw_index += 1;
        v
    }
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.i[27] >> (id % 32) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59db0, id]);
        self.i[28] >> (id % 32) & 1 != 0
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        self.i[29] != 0
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5f460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn distance(&mut self) -> f32 {
        self.calls.push(0x15a2d0);
        f(self.i[31])
    }
    fn callback(&mut self) -> Option<LowTarget> {
        self.calls.push(0x7e220000);
        (self.i[16] != 0).then(|| target(&self.i, 0x2D90000, self.i[17], 0))
    }
    fn retarget(&mut self, _: &LowCtx) -> Option<LowTarget> {
        self.calls.push(0x15a540);
        (self.i[55] != 0).then(|| target(&self.i, 0x2DA0000, self.i[56], 0))
    }
    fn phase_pair(&mut self, value: u32) -> (u32, u32) {
        self.calls.extend([0x25f370, value]);
        (self.i[47], self.i[48])
    }
}

fn run(line: &str) -> (Vec<u32>, Vec<u32>) {
    let (i, e) = line[2..].split_once(" = ").unwrap();
    let words = |s: &str| {
        s.split_whitespace()
            .map(|v| u32::from_str_radix(v, 16).unwrap())
            .collect::<Vec<_>>()
    };
    let i = words(i);
    let expected = words(e);
    let mut c = LowCtx {
        object: LowObject {
            flags: i[12],
            word10: i[13],
            position: [f(i[66]), f(i[67]), f(i[68])],
        },
        target: target(&i, 0x2D20000, i[8], i[14]),
        kind: i[9],
        aim_a: [i[60], i[61]],
        aim_b: [i[62], i[63]],
        state: i[0],
        flags: i[2],
        auxiliary_flags: i[3],
        position: [2.0, 3.0, 4.0],
        model: i[4],
        mounted: i[5] != 0,
        weapon_flag: i[6] != 0,
        node: 0x77,
        scratch_a: 21,
        scratch_b: 23,
        pending: i[1] as i32,
        previous: i[65],
        state_time: f(i[64]),
        counter: i[38],
        hold: f(i[33]),
        drain: f(i[39]),
        speed: f(i[35]),
        aim_out: [0, 0],
        distance_squared: f(i[30]),
        clock: f(i[32]),
        timer: f(i[34]),
        health: f(i[36]),
        threshold: f(i[37]),
        extra: 0x5555,
        route_a: (i[57] != 0).then_some(i[58]),
        route_b: i[59],
        record: LowRecord {
            flags54: i[7],
            flags58: i[44],
            node50: i[45] as i32,
            value28: i[46],
            callback: i[15] != 0,
        },
    };
    let g = LowGlobals {
        min_kind: i[10],
        player_kind: i[11],
        fast: i[42] != 0,
        key_byte: i[43] as u8,
        tick_rate: 60,
        drain_scale: -1.0,
        pickup: (i[40] != 0).then(|| f(i[41])),
        player_node: i[54] as i32,
    };
    let mut w = World {
        i,
        calls: Vec::new(),
        raw_index: 0,
    };
    char_tick_low_class::tick(&mut c, &mut w, &g);
    let mut actual = vec![
        c.target.id,
        c.state,
        c.flags,
        c.auxiliary_flags,
        c.position[0].to_bits(),
        c.position[1].to_bits(),
        c.position[2].to_bits(),
        c.pending as u32,
        c.previous,
        c.state_time.to_bits(),
        c.counter,
        c.hold.to_bits(),
        c.drain.to_bits(),
        c.speed.to_bits(),
        c.aim_out[0],
        c.aim_out[1],
        c.clock.to_bits(),
        c.timer.to_bits(),
        c.extra,
        c.node as u32,
        c.scratch_a,
        c.scratch_b,
    ];
    actual.extend(w.calls);
    (actual, expected)
}

#[test]
fn low_class_tick_matches_original_worlds() {
    let mut count = 0;
    let mut bad = Vec::new();
    for line in include_str!("data/t1877/low_class.golden").lines() {
        let (actual, expected) = run(line);
        if actual != expected {
            let at = actual.iter().zip(&expected).position(|(a, b)| a != b);
            bad.push(format!(
                "state {} first diff at {at:?} (22 fields then calls)\n actual   {actual:x?}\n expected {expected:x?}",
                &line[2..line.find(' ').map_or(4, |_| 5).min(line.len())]
            ));
        }
        count += 1;
    }
    assert!(
        bad.is_empty(),
        "{} of {count} worlds differ, first: {}",
        bad.len(),
        bad[..bad.len().min(4)].join("\n")
    );
    assert!(count >= 1000, "golden unexpectedly small: {count}");
}
