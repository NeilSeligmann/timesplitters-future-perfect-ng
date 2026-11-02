use tsfp_gameplay::char_tick_railbot::{Leaf, Motion, RailbotCtx, RailbotWorld, tick};
struct World {
    bits: u32,
    aware: bool,
    trigger: u32,
    finished: bool,
    angle: f32,
    raw: u32,
    calls: Vec<u32>,
}
impl RailbotWorld for World {
    fn awareness(&mut self, _: &mut RailbotCtx) -> bool {
        self.calls.push(0x15e120);
        self.aware
    }
    fn leaf(&mut self, l: Leaf, _: &mut RailbotCtx) -> u32 {
        self.calls.push(l.original_address());
        match l {
            Leaf::TriggerReady => self.trigger,
            Leaf::Event(b) => {
                self.calls.push(b);
                self.bits >> (b % 32) & 1
            }
            Leaf::CurrentFrame => 1,
            Leaf::EventFrame(_) => 4,
            _ => 0,
        }
    }
    fn button_mask(&mut self, _: u8, fire: bool) -> u32 {
        self.calls.push(if fire { 0x42760 } else { 0x427b0 });
        1
    }
    fn acquire_target(&mut self, _: &mut RailbotCtx) -> u32 {
        self.calls.push(0x15a540);
        0x2d40000
    }
    fn allocate(&mut self, c: &mut RailbotCtx) -> Motion {
        self.calls.push(0x379710);
        c.motion.clone().unwrap()
    }
    fn register_callback(&mut self, _: &mut RailbotCtx) {
        self.calls.push(0xa5ce0);
    }
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.bits >> (id % 32) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59db0, id]);
        self.bits >> (id % 32) & 1 != 0
    }
    fn phase(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d70, id]);
        self.bits >> (id % 32) & 1 != 0
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        self.finished
    }
    fn play(&mut self, id: u32, b: f32, s: f32) {
        self.calls.extend([0x5f460, id, b.to_bits(), s.to_bits()]);
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        self.raw
    }
    fn wrap_angle(&mut self, _: f32) -> f32 {
        self.calls.push(0x12540);
        0.0
    }
    fn scale_curve(&mut self, _: f32) -> f32 {
        self.calls.push(0x123d0);
        0.5
    }
    fn remaining_clip_time(&mut self) -> f32 {
        self.calls.push(0x59cf0);
        1.0
    }
    fn heading(&mut self, _: &RailbotCtx) -> f32 {
        self.calls.push(0x2661f0);
        self.angle
    }
    fn locate_node(&mut self, _: &RailbotCtx) -> i32 {
        self.calls.push(0x9bcf0);
        7
    }
}
#[test]
fn full_railbot_tick_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/railbot.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut c = RailbotCtx {
            state: i[0],
            pending: i[1] as i32,
            previous: 9,
            state_time: f32::from_bits(i[6]),
            flags: 0x60001,
            auxiliary_flags: 0x80,
            drain: 0.25,
            clock: f32::from_bits(i[5]),
            alert_time: f32::from_bits(i[7]),
            aware: false,
            sticky_aware: i[22] != 0,
            tick_rate: 60,
            delta_ticks: 1.0,
            whole_ticks: 1,
            time_scale: 1.0,
            audio_scale: 1.0,
            motion: Some(Motion {
                flags: i[20],
                current: std::array::from_fn(|n| f32::from_bits(i[25 + n])),
                desired: std::array::from_fn(|n| f32::from_bits(i[28 + n])),
                velocity: [0.0; 3],
                anchor: [0.0; 2],
                controlled: i[13] != 0,
                passenger: i[23] != 0,
                request_jump: i[24] != 0,
                player: 0,
                timer: 0.25,
                cooldown: 0.1,
                random_timer: 0.0,
                scale: 0.5,
                audio: 0.0,
                initial: [0; 3],
            }),
            target: 0x2d40000,
            previous_target: 0,
            model: i[3] as u16,
            record_flags: i[4],
            record_event: 31,
            callback: 0,
            position: [0.0, f32::from_bits(i[9]), 0.0],
            facing: 0.0,
            pitch: 0.0,
            turn: 0.0,
            destination: [0.0; 3],
            floor: 0.0,
            bounds_low: 0.25,
            bounds_high: 0.5,
            height_snap: 0,
            target_distance_squared: f32::from_bits(i[8]),
            node: 19,
            route_origin: 3,
            player_class: i[14],
            held: i[17],
            pressed: i[15],
            released: i[16],
            effect_flags: i[21],
        };
        let mut w = World {
            bits: i[10],
            aware: i[2] != 0,
            trigger: i[18],
            finished: i[19] != 0,
            angle: f32::from_bits(i[11]),
            raw: i[12],
            calls: Vec::new(),
        };
        tick(&mut c, &mut w);
        let m = c.motion.as_ref().unwrap();
        let mut actual = vec![
            c.state,
            c.pending as u32,
            c.previous,
            c.state_time.to_bits(),
            c.flags,
            c.auxiliary_flags,
            c.drain.to_bits(),
            c.clock.to_bits(),
            c.alert_time.to_bits(),
            u32::from(c.sticky_aware),
            c.target,
            c.previous_target,
            c.callback,
            c.route_origin as u32,
            m.flags,
            m.initial[0],
            m.initial[1],
            m.initial[2],
        ];
        actual.extend(m.current.map(f32::to_bits));
        actual.extend(m.desired.map(f32::to_bits));
        actual.extend(m.velocity.map(f32::to_bits));
        actual.extend([
            u32::from(m.controlled),
            m.timer.to_bits(),
            m.cooldown.to_bits(),
            u32::from(m.request_jump),
            m.random_timer.to_bits(),
            m.scale.to_bits(),
            m.audio.to_bits(),
        ]);
        actual.extend(c.position.map(f32::to_bits));
        actual.extend([
            c.facing.to_bits(),
            c.turn.to_bits(),
            (c.height_snap as u32) << 16,
        ]);
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 2600);
}
