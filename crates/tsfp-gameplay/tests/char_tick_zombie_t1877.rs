//! T1877: zombie class tick `0x2806B0` against the ORIGINAL (golden `tests/data/t1877/zombie_tick.golden`,
//! `tools/t1877_zombie_gold.py`, numbers only).
use tsfp_gameplay::char_anim::Lcg64;
use tsfp_gameplay::char_tick_zombie::{TickEnd, ZombieCtx, ZombieWorld, tick};

fn hex(token: &str) -> u32 {
    u32::from_str_radix(token, 16).unwrap()
}

struct World {
    aware: bool,
    rng: Lcg64,
    plays_mask: u32,
    finished: bool,
    time: f32,
    tflag: u32,
    plays_made: Vec<(u32, u32, u32)>,
    mode_calls: u32,
}

impl ZombieWorld for World {
    fn aware(&mut self) -> bool {
        self.aware
    }
    fn rng_raw(&mut self) -> u32 {
        self.rng.next_u32()
    }
    fn plays(&self, id: u32) -> bool {
        [0x112, 0x113, 0x121, 0x122]
            .iter()
            .position(|x| *x == id)
            .is_some_and(|i| self.plays_mask >> i & 1 == 1)
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.plays_made.push((id, blend.to_bits(), speed.to_bits()));
    }
    fn clip_finished(&self) -> bool {
        self.finished
    }
    fn clip_time(&self) -> f32 {
        self.time
    }
    fn acquire_target(&mut self) -> u32 {
        0x02D4_0000
    }
    fn bind_record(&mut self) {}
    fn common_update(&mut self) {}
    fn target_flag8(&self, _target: u32) -> bool {
        self.tflag & 8 != 0
    }
    fn request_mode_bit(&mut self, bit: u32) {
        assert_eq!(bit, 0x10);
        self.mode_calls += 1;
    }
}

#[test]
fn zombie_tick_matches_the_original() {
    let text = include_str!("data/t1877/zombie_tick.golden");
    let (mut cases, mut attacks, mut transitions, mut ids) =
        (0, 0, 0, std::collections::BTreeSet::new());
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!((t[0], t[20]), ("Z", "="));
        let mut ctx = ZombieCtx {
            state: hex(t[1]),
            pending: hex(t[2]) as i32,
            previous: hex(t[3]),
            state_time: 1.0,
            clock: f32::from_bits(hex(t[11])),
            deadline: f32::from_bits(hex(t[12])),
            flags_byte: hex(t[4]) as u8,
            speed_scale: f32::from_bits(hex(t[5])),
            target: hex(t[19]),
            record_present: hex(t[6]) != 0,
            aim_source: [hex(t[13]), hex(t[14]), hex(t[15])],
            aim: [0; 3],
        };
        let mut world = World {
            aware: hex(t[7]) != 0,
            rng: Lcg64 {
                state: u64::from(hex(t[16])) | u64::from(hex(t[17])) << 32,
            },
            plays_mask: hex(t[8]),
            finished: hex(t[9]) != 0,
            time: f32::from_bits(hex(t[10])),
            tflag: hex(t[18]),
            plays_made: Vec::new(),
            mode_calls: 0,
        };
        let end = tick(&mut ctx, &mut world);
        let attack = t[21] == "attack";
        assert_eq!(end == TickEnd::Attack, attack, "{line}");
        let n_plays = hex(t[34]) as usize;
        let want_plays: Vec<(u32, u32, u32)> = (0..n_plays)
            .map(|i| (hex(t[35 + 3 * i]), hex(t[36 + 3 * i]), hex(t[37 + 3 * i])))
            .collect();
        assert_eq!(world.plays_made, want_plays, "{line}");
        if attack {
            attacks += 1;
        } else {
            let got = [
                ctx.state,
                ctx.previous,
                ctx.pending as u32,
                ctx.state_time.to_bits(),
                ctx.deadline.to_bits(),
                ctx.aim[0],
                ctx.aim[1],
                ctx.aim[2],
                ctx.speed_scale.to_bits(),
                world.rng.state as u32,
                (world.rng.state >> 32) as u32,
                world.mode_calls,
            ];
            let want: Vec<u32> = (22..34).map(|i| hex(t[i])).collect();
            assert_eq!(got.to_vec(), want, "{line}");
            transitions += usize::from(ctx.state != hex(t[1]));
        }
        ids.extend(world.plays_made.iter().map(|p| p.0));
        cases += 1;
    }
    eprintln!(
        "MEASURED zombie tick: {cases} cases, {attacks} attack, {transitions} state changes, ids {ids:x?}"
    );
    assert!(cases >= 3000 && attacks > 30 && transitions > 500 && ids.len() == 4);
}
