use tsfp_gameplay::char_tick_script_pose::{ScriptPoseCtx, ScriptPoseWorld, state4};
struct World {
    predicates: [u32; 5],
    raw: u32,
    base: u32,
    entry: u32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for World {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59D20, id]);
        self.predicates[if id == self.base {
            0
        } else if id == self.entry {
            1
        } else {
            2
        }] != 0
    }
    fn phase_open(&mut self, id: u32) -> bool {
        self.calls.extend([0x59D70, id]);
        self.predicates[3] != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59DB0, id]);
        self.predicates[4] != 0
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153B40);
        self.raw
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5F460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn advance_sequence(&mut self) {
        self.calls.push(0x26D6D0);
    }
    fn post_update(&mut self) {
        self.calls.push(0x26DD80);
    }
}
#[test]
fn scripted_state4_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_pose.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let variant = usize::from(i[0] == u32::MAX || i[1] & 1 != 0);
        let mut s = ScriptPoseCtx {
            weapon_slot: i[0] as i32,
            weapon_flags: i[1],
            random_time: f32::from_bits(i[2]),
            clock: f32::from_bits(i[3]),
            flags: i[4],
            record_flags: i[5],
        };
        let mut w = World {
            predicates: i[6..11].try_into().unwrap(),
            raw: i[11],
            base: [0xAEB, 0xAEC][variant],
            entry: [0xAE9, 0xAEA][variant],
            calls: Vec::new(),
        };
        state4(&mut s, &mut w);
        let mut actual = vec![s.random_time.to_bits()];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 1800);
}

use tsfp_gameplay::char_tick_script_pose::{ScriptMotionCtx, ScriptMotionWorld, state5};
struct MotionWorld {
    ids: [u32; 8],
    plays: u32,
    transitions: u32,
    event: bool,
    angle: f32,
    raw: u32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for MotionWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59D20, id]);
        self.ids
            .iter()
            .position(|v| *v == id)
            .is_some_and(|n| self.plays >> n & 1 != 0)
    }
    fn phase_open(&mut self, _: u32) -> bool {
        panic!("unused in motion")
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59DB0, id]);
        self.ids
            .iter()
            .position(|v| *v == id)
            .is_some_and(|n| self.transitions >> n & 1 != 0)
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153B40);
        self.raw
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5F460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn advance_sequence(&mut self) {
        self.calls.push(0x26D6D0);
    }
    fn post_update(&mut self) {
        self.calls.push(0x26DD80);
    }
}
impl ScriptMotionWorld for MotionWorld {
    fn begin_motion(&mut self, _: &mut ScriptMotionCtx) {
        self.calls.push(0x26EA00);
    }
    fn heading_delta(&mut self) -> f32 {
        self.angle
    }
    fn event_ready(&mut self) -> bool {
        self.calls.push(0x61570);
        self.event
    }
}
#[test]
fn scripted_state5_selection_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_motion.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let variant = u32::from(i[0] == u32::MAX || i[1] & 1 != 0);
        let mut s = ScriptMotionCtx {
            pose: ScriptPoseCtx {
                weapon_slot: i[0] as i32,
                weapon_flags: i[1],
                random_time: 0.0,
                clock: f32::from_bits(i[4]),
                flags: i[2],
                record_flags: i[3],
            },
            position: [0.0; 3],
            node_position: std::array::from_fn(|n| f32::from_bits(i[n + 5])),
        };
        let mut w = MotionWorld {
            ids: [0xAD7, 0xADD, 0xADF, 0xAE1, 0xADB, 0xAD9, 0xAC9, 0xACB].map(|id| id + variant),
            plays: i[9],
            transitions: i[10],
            event: i[11] != 0,
            angle: f32::from_bits(i[8]),
            raw: i[12],
            calls: Vec::new(),
        };
        state5(&mut s, &mut w);
        assert_eq!(w.calls, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 2200);
}

use tsfp_gameplay::char_tick_script_pose::{ScriptAlertCtx, ScriptAlertWorld, state2, state3};
struct AlertWorld {
    ids: [u32; 4],
    bits: u32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for AlertWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59D20, id]);
        self.bits >> self.ids.iter().position(|v| *v == id).unwrap() & 1 != 0
    }
    fn phase_open(&mut self, id: u32) -> bool {
        self.calls.extend([0x59D70, id]);
        self.bits >> (4 + self.ids.iter().position(|v| *v == id).unwrap()) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59DB0, id]);
        self.bits >> self.ids.iter().position(|v| *v == id).unwrap() & 1 != 0
    }
    fn rng_raw(&mut self) -> u32 {
        panic!("no RNG in alert arms")
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5F460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn advance_sequence(&mut self) {
        panic!("no advance in alert arms")
    }
    fn post_update(&mut self) {
        panic!("no post update in alert arms")
    }
}
impl ScriptAlertWorld for AlertWorld {
    fn alert_group(&mut self, node: i32) {
        self.calls.extend([0x267FF0, node as u32]);
    }
    fn change_class(&mut self, mask: u64) {
        self.calls
            .extend([0x17570, mask as u32, (mask >> 32) as u32]);
    }
    fn common_update(&mut self) {
        self.calls.push(0x158DB0);
    }
}
#[test]
fn scripted_alert_arms_match_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_alert.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let variant = u32::from(i[1] == u32::MAX || i[2] & 1 != 0);
        let mut s = ScriptAlertCtx {
            pose: ScriptPoseCtx {
                weapon_slot: i[1] as i32,
                weapon_flags: i[2],
                random_time: 0.0,
                clock: 5.0,
                flags: 0,
                record_flags: i[4],
            },
            state: i[0],
            pending: -1,
            previous: 9,
            state_time: 1.0,
            aware: i[3] != 0,
            target_type: i[5],
            target_extension_node: i[6] as i32,
            target_object_node: i[7] as i32,
            player_node: 29,
            parked_node: 17,
            node: 19,
            walk_timer: 21,
            walk_progress: 23,
            stored_value: 27,
            exit_value: 25,
        };
        let mut w = AlertWorld {
            ids: [0xAC7, 0xAE9, 0xAEB, 0xAED].map(|id| id + variant),
            bits: i[8],
            calls: Vec::new(),
        };
        if s.state == 2 {
            state2(&mut s, &mut w);
        } else {
            state3(&mut s, &mut w);
        }
        let mut actual = vec![
            s.state,
            s.pending as u32,
            s.previous,
            s.state_time.to_bits(),
            s.parked_node as u32,
            s.node as u32,
            s.walk_timer,
            s.walk_progress,
            s.pose.clock.to_bits(),
            s.exit_value,
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 1200);
}

use tsfp_gameplay::char_tick_script_pose::{ScriptTurnCtx, ScriptTurnWorld, state8};
struct TurnWorld {
    idle: u32,
    bits: u32,
    turning: bool,
    finished: bool,
    delta: f32,
    frames: i32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for TurnWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59D20, id]);
        self.bits >> (if id == self.idle { 0 } else { 1 }) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59DB0, id]);
        self.bits >> (if id == self.idle { 2 } else { 3 }) & 1 != 0
    }
    fn phase_open(&mut self, _: u32) -> bool {
        panic!("unused in turning arm")
    }
    fn rng_raw(&mut self) -> u32 {
        panic!("unused in turning arm")
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5F460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn advance_sequence(&mut self) {
        panic!("unused in turning arm")
    }
    fn post_update(&mut self) {
        self.calls.push(0x26DD80);
    }
}
impl ScriptTurnWorld for TurnWorld {
    fn turning_pose(&mut self) -> bool {
        self.calls.push(0x26CD50);
        self.turning
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59DD0);
        self.finished
    }
    fn phase_delta(&mut self) -> f32 {
        self.calls.extend([0x26CE20, 0x12540]);
        self.delta
    }
    fn remaining_frames(&mut self) -> i32 {
        self.calls.push(0x59D50);
        self.frames
    }
    fn aim_delta(&mut self) -> f32 {
        self.calls.push(0x12430);
        self.delta
    }
}
#[test]
fn scripted_turn_arm_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_turn.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let variant = usize::from(i[0] == u32::MAX || i[1] & 1 != 0);
        let mut s = ScriptTurnCtx {
            pose: ScriptPoseCtx {
                weapon_slot: i[0] as i32,
                weapon_flags: i[1],
                random_time: 0.0,
                clock: 0.0,
                flags: 0,
                record_flags: 0,
            },
            state: 8,
            pending: -1,
            previous: 9,
            state_time: 1.0,
            step_kind: i[7],
            turn_accumulator: 0.25,
        };
        let mut w = TurnWorld {
            idle: [0xAB5, 0xAB6][variant],
            bits: i[2],
            turning: i[3] != 0,
            finished: i[4] != 0,
            delta: f32::from_bits(i[5]),
            frames: i[6] as i32,
            calls: Vec::new(),
        };
        state8(&mut s, &mut w);
        let mut actual = vec![
            s.state,
            s.pending as u32,
            s.previous,
            s.state_time.to_bits(),
            s.turn_accumulator.to_bits(),
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 1600);
}

use tsfp_gameplay::char_tick_script_pose::{
    ScriptSequenceCtx, ScriptSequenceWorld, state0, state1,
};
struct StartWorld {
    resolved: i32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for StartWorld {
    fn plays(&mut self, _: u32) -> bool {
        panic!("unused")
    }
    fn phase_open(&mut self, _: u32) -> bool {
        panic!("unused")
    }
    fn transition(&mut self, _: u32) -> bool {
        panic!("unused")
    }
    fn rng_raw(&mut self) -> u32 {
        panic!("unused")
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5f460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn advance_sequence(&mut self) {
        self.calls.push(0x26d6d0);
    }
    fn post_update(&mut self) {
        self.calls.push(0x26dd80);
    }
}
impl ScriptAlertWorld for StartWorld {
    fn alert_group(&mut self, _: i32) {
        panic!("unused")
    }
    fn change_class(&mut self, _: u64) {
        panic!("unused")
    }
    fn common_update(&mut self) {
        panic!("unused")
    }
}
impl ScriptSequenceWorld for StartWorld {
    fn resolve_node(&mut self, tag: u32) -> i32 {
        self.calls.extend([0x9cc40, tag]);
        self.resolved
    }
    fn update_path(&mut self, _: &mut ScriptSequenceCtx) {
        self.calls.extend([0x26d390, 0x26e150, 0x26ea00]);
    }
    fn despawn(&mut self) {
        self.calls.push(0x15dda0);
    }
}
#[test]
fn scripted_start_path_arms_match_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_start.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut s = ScriptSequenceCtx {
            step_index: 0,
            alert: ScriptAlertCtx {
                pose: ScriptPoseCtx {
                    weapon_slot: i[1] as i32,
                    weapon_flags: i[2],
                    random_time: 0.0,
                    clock: 5.0,
                    flags: i[13],
                    record_flags: i[4],
                },
                state: i[0],
                pending: -1,
                previous: 9,
                state_time: 1.0,
                aware: i[3] != 0,
                target_type: 8,
                target_extension_node: i[9] as i32,
                target_object_node: 0,
                player_node: 29,
                parked_node: i[11] as i32,
                node: 19,
                walk_timer: 21,
                walk_progress: 23,
                stored_value: 27,
                exit_value: 25,
            },
            previous_class: i[5] as u64,
            record_node: i[6] as i32,
            sequence_index: i[7] as i32,
            step_argument: 41,
            step_kind: i[12],
            target_present: i[8] != 0,
            fallback_node: i[10] as i32,
        };
        let mut w = StartWorld {
            resolved: i[14] as i32,
            calls: Vec::new(),
        };
        if s.alert.state == 0 {
            state0(&mut s, &mut w);
        } else {
            state1(&mut s, &mut w);
        }
        let a = &s.alert;
        let mut actual = vec![
            a.state,
            a.pending as u32,
            a.previous,
            a.state_time.to_bits(),
            a.parked_node as u32,
            a.node as u32,
            a.walk_timer,
            a.walk_progress,
            a.pose.clock.to_bits(),
            a.exit_value,
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 1800);
}

use tsfp_gameplay::char_tick_script_pose::{ScriptWalkCtx, ScriptWalkWorld, state6};
struct WalkWorld {
    ids: [u32; 3],
    bits: u32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for WalkWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.bits >> self.ids.iter().position(|v| *v == id).unwrap() & 1 != 0
    }
    fn phase_open(&mut self, _: u32) -> bool {
        panic!("unused")
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59db0, id]);
        self.bits >> (3 + self.ids.iter().position(|v| *v == id).unwrap()) & 1 != 0
    }
    fn rng_raw(&mut self) -> u32 {
        panic!("unused")
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5f460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn advance_sequence(&mut self) {
        self.calls.push(0x26d6d0);
    }
    fn post_update(&mut self) {
        self.calls.push(0x26dd80);
    }
}
impl ScriptAlertWorld for WalkWorld {
    fn alert_group(&mut self, _: i32) {
        panic!("unused")
    }
    fn change_class(&mut self, _: u64) {
        panic!("unused")
    }
    fn common_update(&mut self) {
        panic!("unused")
    }
}
impl ScriptWalkWorld for WalkWorld {
    fn begin_walk(&mut self, _: &mut ScriptWalkCtx) {
        self.calls.push(0x26ea00);
    }
    fn slot_active(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d90, id]);
        self.bits >> 3 & 1 != 0
    }
}
#[test]
fn scripted_walk_arm_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_walk.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let col = u32::from(i[1] == u32::MAX || i[2] & 1 != 0);
        let mut s = ScriptWalkCtx {
            route_origin: 0,
            alert: ScriptAlertCtx {
                pose: ScriptPoseCtx {
                    weapon_slot: i[1] as i32,
                    weapon_flags: i[2],
                    random_time: 0.0,
                    clock: 5.0,
                    flags: i[9],
                    record_flags: i[4],
                },
                state: i[0],
                pending: -1,
                previous: 9,
                state_time: 1.0,
                aware: i[3] != 0,
                target_type: i[5],
                target_extension_node: i[6] as i32,
                target_object_node: i[7] as i32,
                player_node: 29,
                parked_node: 17,
                node: 19,
                walk_timer: 21,
                walk_progress: 23,
                stored_value: 27,
                exit_value: 25,
            },
            position: [0.0; 3],
            node_position: std::array::from_fn(|n| f32::from_bits(i[11 + n])),
            target_present: i[10] != 0,
        };
        let mut w = WalkWorld {
            ids: [0xad3, 0xac5, 0xad5].map(|v| v + col),
            bits: i[8],
            calls: Vec::new(),
        };
        state6(&mut s, &mut w);
        let a = &s.alert;
        let mut actual = vec![
            a.state,
            a.pending as u32,
            a.previous,
            a.state_time.to_bits(),
            a.parked_node as u32,
            a.node as u32,
            a.walk_timer,
            a.walk_progress,
            a.pose.clock.to_bits(),
            a.exit_value,
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 2000);
}

use tsfp_gameplay::char_tick_script_pose::{ScriptRouteWorld, state7};
struct RouteWorld {
    ids: [u32; 6],
    plays: u32,
    transitions: u32,
    event: bool,
    angle: f32,
    raw: u32,
    origin: i32,
    route: i32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for RouteWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.plays >> self.ids.iter().position(|v| *v == id).unwrap() & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59db0, id]);
        self.transitions >> self.ids.iter().position(|v| *v == id).unwrap() & 1 != 0
    }
    fn phase_open(&mut self, _: u32) -> bool {
        panic!("unused")
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        self.raw
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5f460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn advance_sequence(&mut self) {
        panic!("unused")
    }
    fn post_update(&mut self) {
        self.calls.push(0x26dd80);
    }
}
impl ScriptAlertWorld for RouteWorld {
    fn alert_group(&mut self, _: i32) {
        panic!("unused")
    }
    fn change_class(&mut self, _: u64) {
        panic!("unused")
    }
    fn common_update(&mut self) {
        panic!("unused")
    }
}
impl ScriptWalkWorld for RouteWorld {
    fn begin_walk(&mut self, _: &mut ScriptWalkCtx) {
        self.calls.push(0x26ea00);
    }
    fn slot_active(&mut self, _: u32) -> bool {
        panic!("unused")
    }
}
impl ScriptRouteWorld for RouteWorld {
    fn locate_node(&mut self) -> i32 {
        self.calls.push(0x9bcf0);
        self.origin
    }
    fn route_node(&mut self, _: &mut ScriptWalkCtx) -> i32 {
        self.calls.push(0x1596f0);
        self.route
    }
    fn route_heading(&mut self, _: i32) -> f32 {
        self.angle
    }
    fn arrived(&mut self, _: &mut ScriptWalkCtx) {
        self.calls.push(0x26d930);
    }
    fn update_route(&mut self, _: &mut ScriptWalkCtx) {
        self.calls.extend([0x9a4c0, 0x26cff0]);
    }
    fn event_ready(&mut self) -> bool {
        self.calls.push(0x61570);
        self.event
    }
}
#[test]
fn scripted_route_arm_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_route.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let col = u32::from(i[0] == u32::MAX || i[1] & 1 != 0);
        let mut s = ScriptWalkCtx {
            route_origin: 3,
            alert: ScriptAlertCtx {
                pose: ScriptPoseCtx {
                    weapon_slot: i[0] as i32,
                    weapon_flags: i[1],
                    random_time: 0.0,
                    clock: f32::from_bits(i[4]),
                    flags: i[2],
                    record_flags: i[3],
                },
                state: 7,
                pending: -1,
                previous: 9,
                state_time: 1.0,
                aware: false,
                target_type: 8,
                target_extension_node: 0,
                target_object_node: 0,
                player_node: 29,
                parked_node: 17,
                node: 0,
                walk_timer: 21,
                walk_progress: 23,
                stored_value: 27,
                exit_value: 25,
            },
            position: [0.0; 3],
            node_position: std::array::from_fn(|n| f32::from_bits(i[5 + n])),
            target_present: true,
        };
        let mut w = RouteWorld {
            ids: [0xae3, 0xae5, 0xae7, 0xacd, 0xad1, 0xacf].map(|v| v + col),
            plays: i[9],
            transitions: i[10],
            event: i[11] != 0,
            angle: f32::from_bits(i[8]),
            raw: i[12],
            origin: i[13] as i32,
            route: i[14] as i32,
            calls: Vec::new(),
        };
        state7(&mut s, &mut w);
        let mut actual = vec![
            s.alert.pose.flags,
            s.route_origin as u32,
            s.alert.walk_timer,
            s.alert.walk_progress,
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 2400);
}

use tsfp_gameplay::char_tick_script_pose::{ScriptSetupCtx, ScriptSetupWorld, ScriptStep, prepare};
struct SetupWorld {
    synthetic: [u32; 2],
    plays: bool,
    aware: bool,
    kind: i32,
    frame: i32,
    travel: f32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for SetupWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.plays
    }
    fn transition(&mut self, _: u32) -> bool {
        panic!("unused")
    }
    fn phase_open(&mut self, _: u32) -> bool {
        panic!("unused")
    }
    fn rng_raw(&mut self) -> u32 {
        panic!("unused")
    }
    fn play(&mut self, _: u32, _: f32, _: f32) {
        panic!("unused")
    }
    fn advance_sequence(&mut self) {
        panic!("unused")
    }
    fn post_update(&mut self) {
        panic!("unused")
    }
}
impl ScriptAlertWorld for SetupWorld {
    fn alert_group(&mut self, _: i32) {
        panic!("unused")
    }
    fn change_class(&mut self, _: u64) {
        panic!("unused")
    }
    fn common_update(&mut self) {
        panic!("unused")
    }
}
impl ScriptSequenceWorld for SetupWorld {
    fn resolve_node(&mut self, _: u32) -> i32 {
        panic!("unused")
    }
    fn update_path(&mut self, _: &mut ScriptSequenceCtx) {
        panic!("unused")
    }
    fn despawn(&mut self) {
        panic!("unused")
    }
}
impl ScriptSetupWorld for SetupWorld {
    fn awareness(&mut self, _: &mut ScriptSetupCtx) -> bool {
        self.calls.push(0x15e120);
        self.aware
    }
    fn current_step(&mut self, s: &mut ScriptSetupCtx) -> ScriptStep {
        let index = s.sequence.step_index;
        if s.sequence.sequence_index == -2 {
            self.synthetic = [s.record_tag; 2];
            ScriptStep {
                kind: if index == 0 { 0 } else { 4 },
                argument: s.record_tag,
            }
        } else {
            ScriptStep {
                kind: if index == 0 { 1 } else { 11 },
                argument: 31 + index,
            }
        }
    }
    fn acquire_target(&mut self, _: &mut ScriptSetupCtx) -> u32 {
        self.calls.push(0x15a540);
        0x2d40000
    }
    fn movement_kind(&mut self, _: &mut ScriptSetupCtx) -> i32 {
        self.calls.push(0x26e100);
        self.kind
    }
    fn event_frame(&mut self, id: u32) -> i32 {
        self.calls.extend([0x613d0, id, 2, 1]);
        self.frame
    }
    fn motion_distance(&mut self, _: i32, v: f32) -> f32 {
        self.calls.extend([0x26d1b0, v.to_bits()]);
        self.travel
    }
    fn retime(&mut self, e: f32, v: f32, r: f32, w: f32) {
        self.calls.extend([
            0x5ba40,
            e.to_bits(),
            2,
            v.to_bits(),
            r.to_bits(),
            w.to_bits(),
        ]);
    }
}
#[test]
fn scripted_outer_setup_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_setup.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut s = ScriptSetupCtx {
            sequence: ScriptSequenceCtx {
                alert: ScriptAlertCtx {
                    pose: ScriptPoseCtx {
                        weapon_slot: i[0] as i32,
                        weapon_flags: i[1],
                        random_time: 0.0,
                        clock: 0.0,
                        flags: 1,
                        record_flags: 0,
                    },
                    state: i[2],
                    pending: i[3] as i32,
                    previous: 9,
                    state_time: 1.0,
                    aware: false,
                    target_type: 8,
                    target_extension_node: 0,
                    target_object_node: 0,
                    player_node: 29,
                    parked_node: 0,
                    node: 0,
                    walk_timer: 0,
                    walk_progress: 0,
                    stored_value: 0,
                    exit_value: 0,
                },
                previous_class: 0,
                record_node: 0,
                sequence_index: i[7] as i32,
                step_argument: 0,
                step_kind: 0,
                step_index: i[8],
                target_present: i[9] != 0,
                fallback_node: 0,
            },
            auxiliary_flags: i[4] as u8,
            drain: 0.25,
            tick_rate: i[5] as i32,
            delta_ticks: f32::from_bits(i[6]),
            actor_index: 7,
            selected_index: i[10] as i32,
            target: if i[9] != 0 { 0x2d40000 } else { 0 },
            record_tag: 41,
            object_flags: i[12],
            position: [0.0; 3],
            node_position: std::array::from_fn(|n| f32::from_bits(i[17 + n])),
            motion_value: f32::from_bits(i[15]),
        };
        let mut w = SetupWorld {
            synthetic: [u32::MAX; 2],
            plays: i[11] != 0,
            aware: i[20] != 0,
            kind: i[13] as i32,
            frame: i[14] as i32,
            travel: f32::from_bits(i[16]),
            calls: Vec::new(),
        };
        prepare(&mut s, &mut w);
        let a = &s.sequence.alert;
        let mut actual = vec![
            a.state,
            a.pending as u32,
            a.previous,
            a.state_time.to_bits(),
            a.pose.flags,
            s.drain.to_bits(),
            s.target,
            s.sequence.step_kind,
            w.synthetic[0],
            w.synthetic[1],
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 1600);
}
use tsfp_gameplay::char_tick_script_pose::{ScriptTickCtx, ScriptTickWorld, tick};
struct TickWorld {
    bits: u32,
    transitions: u32,
    phases: u32,
    aware: bool,
    event: bool,
    angle: f32,
    raw: u32,
    calls: Vec<u32>,
}
impl ScriptPoseWorld for TickWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        id != 0xac3 && id != 0xac4 && self.bits >> (id % 32) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59db0, id]);
        self.transitions >> (id % 32) & 1 != 0
    }
    fn phase_open(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d70, id]);
        self.phases >> (id % 32) & 1 != 0
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        self.raw
    }
    fn play(&mut self, id: u32, b: f32, s: f32) {
        self.calls.extend([0x5f460, id, b.to_bits(), s.to_bits()]);
    }
    fn advance_sequence(&mut self) {
        self.calls.push(0x26d6d0);
    }
    fn post_update(&mut self) {
        self.calls.push(0x26dd80);
    }
}
impl ScriptAlertWorld for TickWorld {
    fn alert_group(&mut self, n: i32) {
        self.calls.extend([0x267ff0, n as u32]);
    }
    fn change_class(&mut self, m: u64) {
        self.calls.extend([0x17570, m as u32, (m >> 32) as u32]);
    }
    fn common_update(&mut self) {
        self.calls.push(0x158db0);
    }
}
impl ScriptSequenceWorld for TickWorld {
    fn resolve_node(&mut self, n: u32) -> i32 {
        self.calls.extend([0x9cc40, n]);
        13
    }
    fn update_path(&mut self, _: &mut ScriptSequenceCtx) {
        self.calls.extend([0x26d390, 0x26e150, 0x26ea00]);
    }
    fn despawn(&mut self) {
        self.calls.push(0x15dda0);
    }
}
impl ScriptSetupWorld for TickWorld {
    fn awareness(&mut self, _: &mut ScriptSetupCtx) -> bool {
        self.calls.push(0x15e120);
        self.aware
    }
    fn current_step(&mut self, s: &mut ScriptSetupCtx) -> ScriptStep {
        let n = s.sequence.step_index;
        if s.sequence.sequence_index == -2 {
            ScriptStep {
                kind: if n == 0 { 0 } else { 4 },
                argument: s.record_tag,
            }
        } else {
            ScriptStep {
                kind: if n == 0 { 1 } else { 11 },
                argument: 31 + n,
            }
        }
    }
    fn acquire_target(&mut self, _: &mut ScriptSetupCtx) -> u32 {
        panic!("all joined worlds have independent target")
    }
    fn movement_kind(&mut self, _: &mut ScriptSetupCtx) -> i32 {
        panic!("covered separately")
    }
    fn event_frame(&mut self, _: u32) -> i32 {
        panic!("covered separately")
    }
    fn motion_distance(&mut self, _: i32, _: f32) -> f32 {
        panic!("covered separately")
    }
    fn retime(&mut self, _: f32, _: f32, _: f32, _: f32) {
        panic!("covered separately")
    }
}
impl ScriptMotionWorld for TickWorld {
    fn begin_motion(&mut self, _: &mut ScriptMotionCtx) {
        self.calls.push(0x26ea00);
    }
    fn heading_delta(&mut self) -> f32 {
        self.angle
    }
    fn event_ready(&mut self) -> bool {
        self.calls.push(0x61570);
        self.event
    }
}
impl ScriptTurnWorld for TickWorld {
    fn turning_pose(&mut self) -> bool {
        self.calls.push(0x26cd50);
        false
    }
    fn finished(&mut self) -> bool {
        panic!("covered separately")
    }
    fn phase_delta(&mut self) -> f32 {
        panic!("covered separately")
    }
    fn remaining_frames(&mut self) -> i32 {
        panic!("covered separately")
    }
    fn aim_delta(&mut self) -> f32 {
        self.calls.push(0x12430);
        self.angle
    }
}
impl ScriptWalkWorld for TickWorld {
    fn begin_walk(&mut self, _: &mut ScriptWalkCtx) {
        self.calls.push(0x26ea00);
    }
    fn slot_active(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d90, id]);
        self.phases >> (id % 32) & 1 != 0
    }
}
impl ScriptRouteWorld for TickWorld {
    fn locate_node(&mut self) -> i32 {
        self.calls.push(0x9bcf0);
        7
    }
    fn route_node(&mut self, _: &mut ScriptWalkCtx) -> i32 {
        self.calls.push(0x1596f0);
        5
    }
    fn route_heading(&mut self, _: i32) -> f32 {
        self.angle
    }
    fn arrived(&mut self, _: &mut ScriptWalkCtx) {
        self.calls.push(0x26d930);
    }
    fn update_route(&mut self, _: &mut ScriptWalkCtx) {
        self.calls.extend([0x9a4c0, 0x26cff0]);
    }
    fn event_ready(&mut self) -> bool {
        self.calls.push(0x61570);
        self.event
    }
}
impl ScriptTickWorld for TickWorld {
    fn finish_tick(&mut self, _: &mut ScriptTickCtx) {}
}
#[test]
fn joined_scripted_tick_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/script_tick.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut s = ScriptTickCtx {
            setup: ScriptSetupCtx {
                sequence: ScriptSequenceCtx {
                    alert: ScriptAlertCtx {
                        pose: ScriptPoseCtx {
                            weapon_slot: i[2] as i32,
                            weapon_flags: i[3],
                            random_time: 4.0,
                            clock: 21.0,
                            flags: i[7],
                            record_flags: i[4],
                        },
                        state: i[0],
                        pending: i[1] as i32,
                        previous: 9,
                        state_time: 1.0,
                        aware: false,
                        target_type: 8,
                        target_extension_node: 7,
                        target_object_node: 0,
                        player_node: 7,
                        parked_node: 17,
                        node: 19,
                        walk_timer: 21,
                        walk_progress: 23,
                        stored_value: 27,
                        exit_value: 25,
                    },
                    previous_class: i[8] as u64,
                    record_node: -1,
                    sequence_index: i[5] as i32,
                    step_index: i[6],
                    step_argument: 0,
                    step_kind: 0,
                    target_present: true,
                    fallback_node: 11,
                },
                auxiliary_flags: 0x80,
                drain: 0.25,
                tick_rate: 60,
                delta_ticks: 1.0,
                actor_index: 7,
                selected_index: 9,
                target: 0x2d40000,
                record_tag: 41,
                object_flags: 0,
                position: [0.0; 3],
                node_position: std::array::from_fn(|n| f32::from_bits(i[16 + n])),
                motion_value: 0.0,
            },
            route_origin: 3,
            turn_accumulator: 0.25,
        };
        let mut w = TickWorld {
            bits: i[9],
            transitions: i[10],
            phases: i[11],
            aware: i[12] != 0,
            event: i[13] != 0,
            angle: f32::from_bits(i[14]),
            raw: i[15],
            calls: Vec::new(),
        };
        tick(&mut s, &mut w);
        let a = &s.setup.sequence.alert;
        let mut actual = vec![
            a.state,
            a.pending as u32,
            a.previous,
            a.state_time.to_bits(),
            a.parked_node as u32,
            a.node as u32,
            a.walk_timer,
            a.walk_progress,
            a.pose.clock.to_bits(),
            a.exit_value,
            a.pose.flags,
            s.setup.drain.to_bits(),
            s.route_origin as u32,
            a.pose.random_time.to_bits(),
            s.turn_accumulator.to_bits(),
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 2200);
}
