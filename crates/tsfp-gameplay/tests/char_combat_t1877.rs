use tsfp_gameplay::char_tick_combat::{
    CombatCtx, CombatDamage, CombatLeaf, CombatPeer, CombatWorld, tick,
};
struct World {
    i: Vec<u32>,
    calls: Vec<u32>,
}
impl CombatWorld for World {
    fn awareness(&mut self, _: &mut CombatCtx) -> bool {
        self.calls.push(0x15e120);
        self.i[2] != 0
    }
    fn leaf(&mut self, l: CombatLeaf, _: &mut CombatCtx) -> u32 {
        self.calls.push(l.original_address());
        match l {
            CombatLeaf::Blocked => self.i[3],
            CombatLeaf::Armed => self.i[4],
            CombatLeaf::Path => self.i[8],
            CombatLeaf::NoWeapon => self.i[18],
            _ => 0,
        }
    }
    fn random_threshold(&mut self) -> f32 {
        self.calls.push(0x15a2d0);
        0.5
    }
    fn ready(&mut self, _: f32) -> bool {
        self.calls.push(0x5a020);
        self.i[5] != 0
    }
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.i[28] >> (id % 32) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d70, id]);
        self.i[29] >> (id % 32) & 1 != 0
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        self.i[6] != 0
    }
    fn event(&mut self, b: u32) -> bool {
        self.calls.extend([0x61570, b]);
        self.i[7] != 0
    }
    fn policy(&mut self, _: u32) -> u32 {
        self.i[31]
    }
    fn selector(&mut self, id: u32) -> u32 {
        self.calls.extend([0x5b720, id]);
        self.i[30]
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        self.i[27]
    }
    fn play(&mut self, id: u32, b: f32, s: f32) {
        self.calls.extend([0x5f460, id, b.to_bits(), s.to_bits()]);
    }
    fn class(&mut self, m: u64) {
        self.calls.extend([0x17570, m as u32, (m >> 32) as u32]);
    }
    fn damage_init(&mut self) {
        self.calls.push(0xb91f0);
    }
    fn double_damage(&mut self, _: &CombatCtx) -> bool {
        self.calls.push(0x350e70);
        self.i[13] != 0
    }
    fn in_cone(&mut self, _: [f32; 3]) -> bool {
        self.calls.push(0x161630);
        self.i[40] != 0
    }
    fn clamp(&mut self, _: f32) -> f32 {
        self.calls.push(0x124f0);
        0.5
    }
    fn damage(&mut self, r: CombatDamage) {
        self.calls.extend([
            0xbb6c0,
            r.object,
            r.amount.to_bits(),
            if r.radial { 23 } else { 0 },
            if r.radial { 23 } else { 0 },
            u32::from(r.radial),
        ]);
        if r.radial {
            self.calls.extend(r.direction.map(f32::to_bits));
            self.calls.push(r.distance.to_bits());
        }
    }
}
#[test]
fn full_combat_tick_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/combat.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let f = |n| f32::from_bits(i[n]);
        let mut c = CombatCtx {
            state: i[0],
            pending: i[1] as i32,
            previous: 9,
            state_time: 0.3,
            auxiliary_flags: i[38] as u8,
            drain: 0.25,
            tick_rate: 60,
            delta_ticks: 1.0,
            flags: i[26],
            actor: 7,
            selected: if i[9] != 0 { 7 } else { 9 },
            player_count: 6,
            model: i[14],
            category: i[15] as u8,
            object_present: true,
            object_flags: if i[20] != 0 { 0x100 } else { 0 },
            object_type: 1,
            target: if i[19] != 0 { 0x2d20000 } else { 0 },
            target_excluded: i[10] != 0,
            target_is_mode_target: (i[11] != 0) == (i[19] != 0),
            weapon_a: i[16],
            weapon_b: 0,
            weapon_cooldown: i[17],
            random_threshold: f(39),
            blocked_count: i[21] as i32,
            origin: 3,
            destination_node: 8,
            target_node: i[37] as i32,
            player_node: i[37] as i32,
            walk_timer: 21,
            walk_progress: 23,
            position: [f(34), f(35), f(36)],
            target_position: [0.0; 3],
            destination: [2.0, 3.0, 4.0],
            target_distance_squared: f(22),
            clock: f(23),
            unseen: f(24),
            action_deadline: f(25),
            action_word: 17,
            room_blocked: i[12] != 0,
            target_room_blocked: i[12] != 0,
            mode: i[32] as u8,
            submode: i[33] as u8,
            team: 1,
            peers: if i[41] != 0 {
                vec![CombatPeer {
                    object: 0x2d80000,
                    position: [1.0, 0.0, 0.0],
                    excluded: false,
                    team: 1,
                }]
            } else {
                vec![]
            },
        };
        let mut w = World {
            i,
            calls: Vec::new(),
        };
        tick(&mut c, &mut w);
        let mut actual = vec![
            c.state,
            c.pending as u32,
            c.previous,
            c.state_time.to_bits(),
            c.flags,
            c.drain.to_bits(),
            c.destination_node as u32,
            c.walk_timer,
            c.walk_progress,
        ];
        actual.extend(c.destination.map(f32::to_bits));
        actual.extend([
            c.clock.to_bits(),
            c.action_deadline.to_bits(),
            c.unseen.to_bits(),
            c.action_word,
        ]);
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 3600);
}
