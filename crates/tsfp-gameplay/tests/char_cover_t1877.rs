use tsfp_gameplay::char_tick_cover::{CoverCtx, CoverLeaf, CoverNode, CoverWorld, tick};
struct World {
    aware: bool,
    broken: u32,
    eligible: u32,
    decision: u32,
    posture: i32,
    exit: bool,
    attack: bool,
    event: bool,
    finished: bool,
    plays: u32,
    trans: u32,
    angle: f32,
    raw: u32,
    calls: Vec<u32>,
    callback_ret: bool,
}
impl CoverWorld for World {
    fn awareness(&mut self, _: &mut CoverCtx) -> bool {
        self.calls.push(0x15e120);
        self.aware
    }
    fn leaf(&mut self, l: CoverLeaf, _: &mut CoverCtx) -> u32 {
        self.calls.push(if matches!(l, CoverLeaf::Callback) {
            0x7e200000
        } else {
            l.original_address()
        });
        match l {
            CoverLeaf::BreakCover => self.broken,
            CoverLeaf::Eligible => self.eligible,
            CoverLeaf::Decision | CoverLeaf::AmbushDecision => self.decision,
            CoverLeaf::Posture(_, _) => self.posture as u32,
            CoverLeaf::TurnClip => 0x5ee,
            CoverLeaf::Callback => {
                if self.callback_ret {
                    0x2d40000
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
    fn find_cover(&mut self, c: &mut CoverCtx, from_record: bool) -> Option<CoverNode> {
        if from_record {
            self.calls.push(0x160b80);
        }
        let n = c.saved_reservation.clone().unwrap();
        if !from_record || n.flags & 0x800 == 0 {
            self.calls.extend([0x9a4c0, 0x9a290]);
        }
        Some(n)
    }
    fn resolve_node(&mut self, _: i16) -> i32 {
        self.calls.push(0x9cc40);
        3
    }
    fn target_angle(&mut self, _: &CoverCtx) -> f32 {
        self.calls.push(0x2661f0);
        self.angle
    }
    fn wants_exit(&mut self, _: &mut CoverCtx) -> bool {
        self.calls.push(0x2a1a00);
        self.exit
    }
    fn attack_ready(&mut self, _: &mut CoverCtx, _: bool) -> bool {
        self.calls.push(0x2a1a90);
        self.attack
    }
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.plays >> (id % 32) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59db0, id]);
        self.trans >> (id % 32) & 1 != 0
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        self.finished
    }
    fn event_ready(&mut self, b: u32) -> bool {
        self.calls.extend([0x61570, b]);
        self.event
    }
    fn play(&mut self, id: u32, b: f32, s: f32) {
        self.calls.extend([0x5f460, id, b.to_bits(), s.to_bits()]);
    }
    fn class(&mut self, m: u64) {
        self.calls.extend([0x17570, m as u32, (m >> 32) as u32]);
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        self.raw
    }
}
#[test]
fn full_cover_tick_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/cover.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let n = CoverNode {
            tag: 3,
            flags: i[28],
            position: [0.0; 3],
            yaw: 0.0,
        };
        let mut c = CoverCtx {
            state: i[0],
            pending: i[1] as i32,
            previous: 9,
            state_time: f32::from_bits(i[15]),
            flags: 0x60001,
            auxiliary_flags: 0x80,
            drain: 0.25,
            tick_rate: 60,
            delta_ticks: 1.0,
            clock: f32::from_bits(i[13]),
            unseen: f32::from_bits(i[14]),
            action_deadline: 0.25,
            action_timer: f32::from_bits(i[16]),
            selected: i[4] != 0,
            target: if i[30] != 0 { 0x2d40000 } else { 0 },
            target_node: 7,
            callback_present: i[29] != 0,
            reservation: if i[26] != 0 { Some(n.clone()) } else { None },
            saved_reservation: Some(n),
            node_flags: 0x40000000,
            node: 19,
            walk_timer: 21,
            walk_progress: 23,
            player_node: 7,
            record_mask: i[27] as u64,
            origin: 0,
            destination: [2.0, 3.0, 4.0],
            position: std::array::from_fn(|n| f32::from_bits(i[17 + n])),
            eligible: false,
            posture: i[2] as i32,
            weapon_flags: i[12],
            assessment: [0.0; 3],
            clip_policy: i[24],
            slot_flags: i[25],
        };
        let mut w = World {
            callback_ret: i[31] != 0,
            aware: i[3] != 0,
            broken: i[5],
            eligible: i[6],
            decision: i[11],
            posture: i[2] as i32,
            exit: i[7] != 0,
            attack: i[8] != 0,
            event: i[9] != 0,
            finished: i[10] != 0,
            plays: i[20],
            trans: i[21],
            angle: f32::from_bits(i[22]),
            raw: i[23],
            calls: Vec::new(),
        };
        tick(&mut c, &mut w);
        let mut actual = vec![
            c.state,
            c.pending as u32,
            c.previous,
            c.state_time.to_bits(),
            c.flags,
            c.auxiliary_flags,
            c.drain.to_bits(),
            c.clock.to_bits(),
            c.unseen.to_bits(),
            c.action_deadline.to_bits(),
            c.action_timer.to_bits(),
            u32::from(c.eligible),
            c.posture as u32,
            if c.reservation.is_some() {
                0x2d30000
            } else {
                0
            },
            if c.saved_reservation.is_some() {
                0x2d30000
            } else {
                0
            },
            c.node as u32,
            c.walk_timer,
            c.walk_progress,
        ];
        actual.extend(c.destination.map(f32::to_bits));
        actual.extend([c.node_flags, c.target]);
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 3400);
}

#[test]
fn full_ambush_tick_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/ambush.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let n = CoverNode {
            tag: 3,
            flags: i[28],
            position: [0.0; 3],
            yaw: 0.0,
        };
        let mut c = CoverCtx {
            state: i[0],
            pending: i[1] as i32,
            previous: 9,
            state_time: f32::from_bits(i[15]),
            flags: 0x60001,
            auxiliary_flags: i[32],
            drain: 0.25,
            tick_rate: 60,
            delta_ticks: 1.0,
            clock: f32::from_bits(i[13]),
            unseen: f32::from_bits(i[14]),
            action_deadline: 0.25,
            action_timer: f32::from_bits(i[16]),
            selected: i[4] != 0,
            target: if i[30] != 0 { 0x2d40000 } else { 0 },
            target_node: 7,
            callback_present: i[29] != 0,
            reservation: if i[26] != 0 { Some(n.clone()) } else { None },
            saved_reservation: Some(n),
            node_flags: 0x40000000,
            node: 19,
            walk_timer: 21,
            walk_progress: 23,
            player_node: 7,
            record_mask: i[27] as u64,
            origin: 0,
            destination: [2.0, 3.0, 4.0],
            position: std::array::from_fn(|n| f32::from_bits(i[17 + n])),
            eligible: false,
            posture: i[2] as i32,
            weapon_flags: i[12],
            assessment: [0.0; 3],
            clip_policy: i[24],
            slot_flags: i[25],
        };
        let mut w = World {
            callback_ret: i[31] != 0,
            aware: i[3] != 0,
            broken: i[5],
            eligible: i[6],
            decision: i[11],
            posture: i[2] as i32,
            exit: i[7] != 0,
            attack: i[8] != 0,
            event: i[9] != 0,
            finished: i[10] != 0,
            plays: i[20],
            trans: i[21],
            angle: f32::from_bits(i[22]),
            raw: i[23],
            calls: Vec::new(),
        };
        tsfp_gameplay::char_tick_ambush::tick(&mut c, &mut w);
        let mut actual = vec![
            c.state,
            c.pending as u32,
            c.previous,
            c.state_time.to_bits(),
            c.flags,
            c.auxiliary_flags,
            c.drain.to_bits(),
            c.clock.to_bits(),
            c.unseen.to_bits(),
            c.action_deadline.to_bits(),
            c.action_timer.to_bits(),
            u32::from(c.eligible),
            c.posture as u32,
            if c.reservation.is_some() {
                0x2d30000
            } else {
                0
            },
            if c.saved_reservation.is_some() {
                0x2d30000
            } else {
                0
            },
            c.node as u32,
            c.walk_timer,
            c.walk_progress,
        ];
        actual.extend(c.destination.map(f32::to_bits));
        actual.extend([c.node_flags, c.target]);
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 3200);
}
