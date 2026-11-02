//! T1964: idle variation tick `0x2621A0` against the ORIGINAL (golden `tests/data/t1877/idle_variation.golden`, Unicorn).
use tsfp_gameplay::char_tick_idle_variation::{IdleVariationCtx, IdleVariationWorld, tick};

struct World {
    plays: u32,
    finished: bool,
    clip_time: u32,
    raw: u32,
    draws: u32,
    calls: Vec<u32>,
}

impl IdleVariationWorld for World {
    fn select_clip(&mut self, stance: u32, variant: u32) -> u32 {
        self.calls.extend([0x2aefb0, stance, variant]);
        0x20 + 16 * stance + variant
    }
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.plays >> (id % 32) & 1 != 0
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        self.finished
    }
    fn clip_time(&mut self) -> f32 {
        self.calls.push(0x59d50);
        f32::from_bits(self.clip_time)
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        let value = self.raw.wrapping_add(self.draws.wrapping_mul(0x9E37_79B1));
        self.draws += 1;
        value
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5f460, id, blend.to_bits(), speed.to_bits()]);
    }
}

#[test]
fn idle_variation_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/idle_variation.golden").lines() {
        let (input, expected) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let (i, e) = (words(input), words(expected));
        let mut c = IdleVariationCtx {
            weapon_flags: i[2] as u8,
            model: i[3],
            flags: i[4],
            aux_flags: i[5],
            refresh_timer: f32::from_bits(i[6]),
            repeats: i[7] as i32,
            wait: f32::from_bits(i[8]),
            loop_time: f32::from_bits(i[9]),
            variation_time: f32::from_bits(i[10]),
            state_time: f32::from_bits(i[11]),
            tick_rate: 60,
        };
        let mut w = World {
            plays: i[12],
            finished: i[13] != 0,
            clip_time: i[14],
            raw: i[15],
            draws: 0,
            calls: Vec::new(),
        };
        tick(&mut c, &mut w, i[0], i[1]);
        let mut actual = vec![
            c.flags,
            c.aux_flags,
            c.refresh_timer.to_bits(),
            c.repeats as u32,
            c.wait.to_bits(),
            c.loop_time.to_bits(),
            c.variation_time.to_bits(),
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 4000);
}
