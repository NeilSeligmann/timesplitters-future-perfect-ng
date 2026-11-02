//! T1964: mode-A class tick `0x265CC0` against ORIGINAL controlled worlds (tier 1, tools/t1964_class_a_tick_gold.py).
use tsfp_gameplay::char_tick_class_a::{ClassACtx, ClassAWorld, tick};

const KEYS: [u32; 6] = [0x3E9, 0x3EB, 0x49F, 0x49C, 0x47D, 0x47E];

struct World {
    i: Vec<u32>,
    calls: Vec<u32>,
    draws: u32,
}
impl ClassAWorld for World {
    fn aware(&mut self, _ctx: &mut ClassACtx) {
        self.calls.push(0x15e120);
    }
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.i[18] >> (id % 32) & 1 != 0
    }
    fn play(&mut self, id: u32, b: f32, s: f32) {
        self.calls.extend([0x5f460, id, b.to_bits(), s.to_bits()]);
    }
    fn key_node(&mut self, key: u32) -> i32 {
        self.calls.extend([0x9cc40, key]);
        self.i[10 + KEYS.iter().position(|k| *k == key).unwrap()] as i32
    }
    fn player_node(&mut self) -> i32 {
        self.i[16] as i32
    }
    fn reposition(&mut self, _ctx: &mut ClassACtx, node: i32) {
        self.calls.extend([0xda280, node as u32, 1]);
    }
    fn route(&mut self, node: i32) -> bool {
        self.calls.extend([0x15d500, node as u32]);
        self.i[17] != 0
    }
    fn start_navigation(&mut self, _ctx: &mut ClassACtx, word: u32) {
        self.calls.extend([0x159300, word, 0]);
    }
    fn update_floor(&mut self) {
        self.calls.push(0x15f0d0);
    }
    fn update_yaw(&mut self) {
        self.calls.push(0x15cd00);
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        self.i[19] != 0
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        let value = self.i[20].wrapping_add(self.draws.wrapping_mul(0x9E37_79B1));
        self.draws += 1;
        value
    }
}

#[test]
fn class_a_tick_matches_original() {
    let path = std::env::var("T1964_GOLDEN").ok();
    let text = match &path {
        Some(p) => std::fs::read_to_string(p).unwrap(),
        None => include_str!("data/t1877/class_a_tick.golden").to_string(),
    };
    let mut count = 0;
    for line in text.lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let (i, e) = (words(i), words(e));
        let mut c = ClassACtx {
            state: i[0],
            pending: i[1] as i32,
            previous: i[2],
            state_time: f32::from_bits(i[3]),
            flags_byte: i[4] as u8,
            timer: f32::from_bits(i[5]),
            tick_rate: i[6] as i32,
            time_scale: f32::from_bits(i[7]),
            selector: i[8],
            node: i[9] as i32,
            target_node: 0x55,
            clear_a: 21,
            clear_b: 23,
            route_word_a: (i[22] != 0).then_some(i[23]),
            route_word_b: i[24],
            arrival_index: i[21],
        };
        let mut w = World {
            i,
            calls: vec![],
            draws: 0,
        };
        tick(&mut c, &mut w);
        let mut got = vec![
            c.state,
            c.pending as u32,
            c.previous,
            c.state_time.to_bits(),
            c.timer.to_bits(),
            c.target_node as u32,
            c.node as u32,
            c.clear_a,
            c.clear_b,
            c.arrival_index,
        ];
        got.extend(&w.calls);
        assert_eq!(got, e, "{line}");
        count += 1;
    }
    assert!(count >= 3000, "{count}");
}
