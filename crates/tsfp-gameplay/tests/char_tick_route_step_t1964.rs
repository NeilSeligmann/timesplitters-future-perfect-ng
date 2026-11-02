//! T1964: route step executor 0x26E150 lifted-original differential (tier 1, controlled leaves).
//! Golden: `tools/t1877_route_step_gold.py` (private XBE under Unicorn, first 3000 of 10000 worlds).
use tsfp_gameplay::char_tick_route_step::{RouteStepCtx, RouteStepWorld, StepRecord, tick};

const NODE_RECORD: u32 = 0x2E60000;
const KEY: u32 = 0x4B45_5931;

struct World {
    i: Vec<u32>,
    calls: Vec<u32>,
    n: [u32; 5],
}
impl World {
    fn seq(&mut self, slot: usize, mask: u32) -> bool {
        let n = self.n[slot];
        self.n[slot] += 1;
        mask >> (n % 32) & 1 != 0
    }
}
impl RouteStepWorld for World {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.i[7] >> (id % 32) & 1 != 0
    }
    fn play(&mut self, id: u32, blend: f32) {
        self.calls
            .extend([0x5f460, id, blend.to_bits(), 1.0f32.to_bits()]);
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        let m = self.i[8];
        self.seq(0, m)
    }
    fn action_ready(&mut self) -> bool {
        self.calls.push(0x26cbe0);
        let m = self.i[9];
        self.seq(1, m)
    }
    fn action_reset(&mut self) {
        self.calls.push(0x26cb30);
    }
    fn route_busy(&mut self) -> bool {
        self.calls.push(0x26cc80);
        let m = self.i[10];
        self.seq(2, m)
    }
    fn kind_pending(&mut self, kind: u32) -> bool {
        self.calls.extend([0x26d2f0, kind]);
        let m = self.i[11];
        self.seq(3, m)
    }
    fn kind_apply(&mut self, kind: u32) {
        self.calls.extend([0x26dfb0, kind]);
    }
    fn record(&mut self) -> StepRecord {
        self.calls.push(0x26d690);
        StepRecord {
            kind: self.i[14],
            word8: self.i[15],
        }
    }
    fn launch(&mut self, word: u32) {
        self.calls.extend([0x26cff0, word]);
    }
    fn node_key(&mut self, node: u32) -> u32 {
        self.calls.extend([0x9a4c0, node]);
        KEY
    }
    fn stop_check(&mut self) -> bool {
        self.calls.push(0x26cd50);
        let m = self.i[12];
        self.seq(4, m)
    }
    fn turn_toward(&mut self, word: u32) {
        self.calls.extend([0x159300, word]);
    }
    fn leave(&mut self) {
        self.calls.push(0x26de30);
    }
    fn step_check(&mut self) -> u32 {
        self.calls.push(0x26e100);
        self.i[13]
    }
    fn node_record(&mut self, node: u32) -> u32 {
        self.calls.extend([0x9a420, node]);
        NODE_RECORD
    }
    fn facing_delta(&mut self, _: u32) -> f32 {
        self.calls.push(0x2661f0);
        f32::from_bits(self.i[16])
    }
}

#[test]
fn route_step_executor_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/route_step.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut c = RouteStepCtx {
            flags: i[1],
            step_index: i[6],
            step_kind: i[0],
            weapon_slot: i[2] as i32,
            weapon_flags: i[3],
            node: 0x77,
            route_turn_word: i[4],
        };
        let mut w = World {
            i,
            calls: Vec::new(),
            n: [0; 5],
        };
        tick(&mut c, &mut w);
        let mut actual = vec![c.step_index];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 3000);
}
