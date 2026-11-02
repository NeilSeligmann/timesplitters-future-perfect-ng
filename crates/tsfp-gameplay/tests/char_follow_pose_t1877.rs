use tsfp_gameplay::char_tick_follow_pose::{FollowPoseWorld, select};
struct World {
    predicates: [u32; 4],
    raw: u32,
    first: u32,
    calls: Vec<u32>,
}
impl FollowPoseWorld for World {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59D20, id]);
        self.predicates[if id == self.first { 0 } else { 1 }] != 0
    }
    fn reselect(&mut self, id: u32) -> bool {
        self.calls.extend([0x59DB0, id]);
        self.predicates[if id == self.first { 2 } else { 3 }] != 0
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153B40);
        self.raw
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5F460, id, blend.to_bits(), speed.to_bits()]);
    }
}
#[test]
fn follow_pose_matches_original_calls() {
    let mut count = 0;
    for line in include_str!("data/t1877/follow_pose.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|x| u32::from_str_radix(x, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let variant = if i[0] & 1 != 0 {
            1
        } else if matches!(i[1], 0xE | 0x35 | 0xF) {
            2
        } else {
            0
        };
        let mut w = World {
            predicates: i[2..6].try_into().unwrap(),
            raw: i[6],
            first: [1540, 1557, 1585][variant],
            calls: Vec::new(),
        };
        select(i[0], i[1], &mut w);
        assert_eq!(w.calls, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 1280);
}

use tsfp_gameplay::char_tick_follow_pose::{FollowCtx, FollowWorld, tick};
struct TickWorld {
    pose: World,
    input: Vec<u32>,
}
impl FollowPoseWorld for TickWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.pose.plays(id)
    }
    fn reselect(&mut self, id: u32) -> bool {
        self.pose.reselect(id)
    }
    fn rng_raw(&mut self) -> u32 {
        self.pose.rng_raw()
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.pose.play(id, blend, speed);
    }
}
impl FollowWorld for TickWorld {
    fn aware(&mut self) -> bool {
        self.pose.calls.push(0x15E120);
        self.input[13] != 0
    }
    fn acquire_target(&mut self) -> u32 {
        self.pose.calls.push(0x15A540);
        0x02D20000
    }
    fn first_update(&mut self) -> bool {
        self.pose.calls.push(0x2663D0);
        self.input[14] != 0
    }
    fn second_update(&mut self) -> bool {
        self.pose.calls.push(0x2A5C30);
        self.input[15] != 0
    }
    fn change_class(&mut self, mask: u64) {
        self.pose
            .calls
            .extend([0x17570, mask as u32, (mask >> 32) as u32]);
    }
    fn track_target(&mut self, _: u32) -> [u32; 3] {
        self.pose.calls.push(0x15D470);
        [11, 12, 13]
    }
    fn navigation_node(&mut self) -> u32 {
        self.pose.calls.push(0x15A270);
        17
    }
    fn navigate(&mut self, node: u32) {
        self.pose.calls.extend([0x15D500, node]);
    }
    fn channel_ready(&mut self) -> bool {
        self.pose.calls.push(0x59E00);
        self.input[16] != 0
    }
    fn record_callback(&mut self) -> u32 {
        self.pose.calls.push(0x15DCC0);
        0x02D90000
    }
}
#[test]
fn follow_tick_matches_original_state_and_calls() {
    let mut count = 0;
    for line in include_str!("data/t1877/follow_tick.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let variant = if i[11] & 1 != 0 {
            1
        } else if matches!(i[12], 0xE | 0xF | 0x35) {
            2
        } else {
            0
        };
        let mut s = FollowCtx {
            state: i[0],
            pending: i[1] as i32,
            previous: 9,
            state_time: 1.0,
            flags: i[2],
            auxiliary_flags: i[3],
            speed_scale: 0.5,
            target: i[4],
            position: [7, 8, 9],
            object_present: i[5] != 0,
            object_flags: i[6],
            object_type: i[7],
            index: i[8] as i32,
            first_npc: 1,
            range_value: f32::from_bits(i[9]),
            clock: f32::from_bits(i[10]),
            weapon_flags: i[11],
            weapon_type: i[12],
            callback_index: 1,
            has_record_callback: i[17] != 0,
        };
        let mut w = TickWorld {
            pose: World {
                predicates: std::array::from_fn(|n| (i[18] >> n) & 1),
                raw: i[19],
                first: [1540, 1557, 1585][variant],
                calls: Vec::new(),
            },
            input: i,
        };
        tick(&mut s, 1.0 / 60.0, &mut w);
        let mut actual = vec![
            s.state,
            s.pending as u32,
            s.previous,
            s.state_time.to_bits(),
            s.speed_scale.to_bits(),
            s.target,
            s.position[0],
            s.position[1],
            s.position[2],
            s.flags,
            s.auxiliary_flags,
        ];
        actual.extend(w.pose.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 1200);
}
