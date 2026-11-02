//! T1964: idle-variant tick `0x283730` against ORIGINAL full-entry worlds (Unicorn, private XBE).
//! Verification tier 1 (lifted/original differential on controlled leaves), independent per-call random draws (T1968).
use tsfp_gameplay::char_tick_idle_variant::{
    IdleAnchor, IdleCtx, IdleGlobals, IdleVariantWorld, tick,
};

struct World {
    i: Vec<u32>,
    calls: Vec<u32>,
    draws: u32,
}
impl IdleVariantWorld for World {
    fn awareness(&mut self, _: &mut IdleCtx) -> bool {
        self.calls.push(0x15e120);
        self.i[2] != 0
    }
    fn navigate(&mut self, _: &mut IdleCtx) {
        self.calls.push(0x15d470);
    }
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.i[22] >> (id % 32) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59db0, id]);
        self.i[23] >> (id % 32) & 1 != 0
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        self.i[24] != 0
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5f460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        let value = self.i[21].wrapping_add(self.draws.wrapping_mul(0x9E37_79B1));
        self.draws += 1;
        value
    }
    fn walk(&mut self, _: &mut IdleCtx) {
        self.calls.push(0x15cd00);
    }
    fn request(&mut self, _: &mut IdleCtx, code: u32) {
        self.calls.extend([0x17570, code, 0]);
    }
}
fn words(s: &str) -> Vec<u32> {
    s.split_whitespace()
        .map(|v| u32::from_str_radix(v, 16).unwrap())
        .collect()
}
fn f(v: u32) -> f32 {
    f32::from_bits(v)
}
#[test]
fn idle_variant_tick_matches_original() {
    let mut count = 0;
    let mut bad = Vec::new();
    for (line_no, line) in include_str!("data/t1877/idle_variant.golden")
        .lines()
        .enumerate()
    {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let i = words(i);
        let e = words(e);
        let mut c = IdleCtx {
            state: i[0],
            pending: i[1] as i32,
            previous: 9,
            state_time: 0.3,
            saved_state: i[8],
            wake_time: f(i[7]),
            clock: f(i[6]),
            sway: f(i[4]),
            flags: i[3],
            status_flags: i[18],
            model: i[5] as u16,
            distance_squared: f(i[9]),
            record_flags: i[10],
            weapon_flags: i[11],
            priority: i[13] as i32,
            position: [0.0; 3],
            anchor: (i[15] != 0).then(|| IdleAnchor {
                flags: i[16],
                mode: i[17],
                position: [7.0, 8.0, 9.0],
            }),
            object_position: [f(i[25]), f(i[26]), f(i[27])],
        };
        let g = IdleGlobals {
            clips: std::array::from_fn(|v| std::array::from_fn(|k| i[28 + v * 8 + k])),
            tick_rate: i[19] as i32,
            time_scale: f(i[20]),
            priority_floor: i[14] as i32,
        };
        let mut w = World {
            i,
            calls: Vec::new(),
            draws: 0,
        };
        tick(&mut c, &g, &mut w);
        let mut actual = vec![
            c.state,
            c.pending as u32,
            c.previous,
            c.state_time.to_bits(),
            c.saved_state,
            c.wake_time.to_bits(),
            c.sway.to_bits(),
            c.flags,
            c.clock.to_bits(),
        ];
        actual.extend(c.position.map(f32::to_bits));
        actual.extend(w.calls);
        if actual != e {
            bad.push(line_no);
        }
        count += 1;
    }
    assert!(count >= 1000);
    assert!(
        bad.is_empty(),
        "{} of {count} worlds differ, first {:?}",
        bad.len(),
        &bad[..bad.len().min(10)]
    );
}
