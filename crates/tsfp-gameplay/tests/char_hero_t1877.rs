use tsfp_gameplay::char_tick_hero_pose::{HeroPoseCtx, HeroPoseWorld, state10, state11};
struct World {
    i: Vec<u32>,
    calls: Vec<u32>,
}
impl HeroPoseWorld for World {
    fn plays(&mut self, id: u32) -> bool {
        self.calls.extend([0x59d20, id]);
        self.i[8] >> (id % 32) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.calls.extend([0x59db0, id]);
        self.i[9] >> (id % 32) & 1 != 0
    }
    fn finished(&mut self) -> bool {
        self.calls.push(0x59dd0);
        self.i[10] != 0
    }
    fn rng_raw(&mut self) -> u32 {
        self.calls.push(0x153b40);
        self.i[11]
    }
    fn play(&mut self, id: u32, b: f32, s: f32) {
        self.calls.extend([0x5f460, id, b.to_bits(), s.to_bits()]);
    }
    fn navigate(&mut self, _: &mut HeroPoseCtx) {
        self.calls.push(0x15d470);
    }
    fn stop_motion(&mut self, _: &mut HeroPoseCtx) {
        self.calls.push(0x158db0);
    }
}
#[test]
fn hero_pose_arms_match_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/hero_pose.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut c = HeroPoseCtx {
            state: i[0],
            pending: -1,
            previous: 9,
            state_time: 0.3,
            flags: 0x60003,
            auxiliary_flags: 0x440,
            clock: f32::from_bits(i[6]),
            repeat: i[7] as i32,
            weapon_flags: i[1],
            model: i[2],
            record_tag: i[3],
            target_present: i[4] != 0,
            fast_mode: i[5] != 0,
        };
        let mut w = World {
            i,
            calls: Vec::new(),
        };
        if c.state == 10 {
            state10(&mut c, &mut w);
        } else {
            state11(&mut c, &mut w);
        }
        let mut actual = vec![
            c.state,
            c.pending as u32,
            c.previous,
            c.state_time.to_bits(),
            c.flags,
            c.auxiliary_flags,
            c.clock.to_bits(),
            c.repeat as u32,
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 5000);
}
use tsfp_gameplay::char_tick_hero::{
    self, HeroBranch, HeroCommand, HeroCtx, HeroGlobals, HeroLeaf, HeroNode, HeroTarget, HeroWorld,
};
fn command(i: &[u32]) -> HeroCommand {
    HeroCommand {
        kind: i[2],
        condition: i[39],
        target_selector: i[40],
        nodes: [3, 1, 8],
        parameter: i[49],
        delay: 0.0,
        cue: 7,
        flags: i[38],
        jump: 5,
        clip: i[41] as i16,
    }
}
fn target(i: &[u32]) -> HeroTarget {
    HeroTarget {
        object: 0x2d20000,
        position: [0.0; 3],
        health: f32::from_bits(i[30]),
        node: i[31] as i32,
        kind: 0,
        character: true,
    }
}
impl HeroWorld for World {
    fn awareness(&mut self, _: &mut HeroCtx, _: bool) -> bool {
        self.calls.push(0x15e120);
        self.i[3] != 0
    }
    fn leaf(&mut self, l: HeroLeaf, c: &mut HeroCtx, _: &mut HeroGlobals) -> u32 {
        let va = match l {
            HeroLeaf::Condition(_) => 0x7e200000,
            HeroLeaf::TargetCallback(_) => 0x7e210000,
            _ => l.original_address(),
        };
        self.calls.push(va);
        match l {
            HeroLeaf::Suppressed => self.i[4],
            HeroLeaf::Path(_) => self.i[5],
            HeroLeaf::ApproachReady => self.i[6],
            HeroLeaf::Advance => self.i[7],
            HeroLeaf::Exit => self.i[8],
            HeroLeaf::PlayersNear => self.i[9],
            HeroLeaf::Approach => self.i[10],
            HeroLeaf::Acquire => self.i[11],
            HeroLeaf::ChooseClose => self.i[12],
            HeroLeaf::Condition(_) => self.i[16],
            HeroLeaf::BranchCondition(_) => self.i[17],
            HeroLeaf::EntityClearance => self.i[19],
            HeroLeaf::ValidateCue(_) => self.i[25],
            HeroLeaf::NodeIndex(_) | HeroLeaf::NodeRecordTag(_) => 0x2d70000,
            HeroLeaf::NodeTag(n) => {
                self.calls.push(n as u32);
                3
            }
            HeroLeaf::NodeRecordIndex | HeroLeaf::Locate => 3,
            HeroLeaf::FindTarget(_) => {
                c.target = None;
                0
            }
            HeroLeaf::TargetCallback(_) => {
                c.target = if self.i[29] != 0 {
                    Some(target(&self.i))
                } else {
                    None
                };
                c.target.as_ref().map(|t| t.object).unwrap_or(0)
            }
            HeroLeaf::FacePlayer => 1,
            HeroLeaf::Class(m) => {
                self.calls.extend([m as u32, (m >> 32) as u32]);
                0
            }
            HeroLeaf::EntityFromTag(_) | HeroLeaf::PlayerEntity => {
                c.resolved_entity = 77;
                if self.i[18] != 0 { 0x2de0000 } else { 0 }
            }
            HeroLeaf::EntityPosition => 0x2d7000c,
            HeroLeaf::EventObject(_) => 7,
            HeroLeaf::Route => self.i[57],
            HeroLeaf::ResolvePickup(_) => 0x2de0000,
            HeroLeaf::PickupReady => 1,
            HeroLeaf::FloorEnabled(bits) => {
                self.calls.push(bits);
                if bits == 0x800000 { self.i[59] } else { 0 }
            }
            HeroLeaf::FloorRoom => 9,
            HeroLeaf::SetNode(n, b) => {
                self.calls.extend([n as u32, u32::from(b)]);
                0
            }
            HeroLeaf::Sound(sample, duration) => {
                self.calls
                    .extend([sample as u32, duration as u32, 1.5_f32.to_bits()]);
                0
            }
            HeroLeaf::StreamZone => 7,
            HeroLeaf::StreamStatus(_) => 1,
            _ => 0,
        }
    }
    fn event(&mut self, _: &mut HeroCtx, b: u32) -> bool {
        self.calls.extend([0x61570, b]);
        self.i[15] != 0
    }
    fn event_object_ready(&mut self, _: u32) -> bool {
        self.calls.push(0x234270);
        self.i[51] != 0
    }
    fn command(&mut self, _: i32) -> HeroCommand {
        command(&self.i)
    }
    fn sequence_step(&mut self, _: i32, _: i32) -> tsfp_gameplay::char_sequence::Step {
        tsfp_gameplay::char_sequence::Step {
            kind: self.i[56],
            argument: 0,
            value: 3,
        }
    }
    fn peers(&mut self, _: &mut HeroCtx, _: &HeroGlobals, group: i32) -> Vec<HeroTarget> {
        self.calls.extend([0x276f20, group as u32]);
        vec![target(&self.i)]
    }
    fn neighbors(&mut self, _: &HeroCtx, _: &HeroGlobals) -> Vec<HeroNode> {
        self.calls.extend([0x9a420, 0x9d4a0, 0x9c550]);
        if self.i[62] != 0 {
            vec![
                HeroNode {
                    index: 3,
                    tag: 3,
                    flags: 0,
                    position: [0.0; 3],
                    yaw: 0.0,
                },
                HeroNode {
                    index: 8,
                    tag: 8,
                    flags: 0,
                    position: [2.0, 0.0, 0.0],
                    yaw: 0.0,
                },
            ]
        } else {
            vec![]
        }
    }
    fn visible(&mut self, _: &HeroCtx, _: &HeroGlobals) -> bool {
        self.calls.push(0x15bd00);
        self.i[22] != 0
    }
    fn target_dormant(&mut self, _: &HeroCtx) -> bool {
        self.calls.push(0x174e0);
        false
    }
    fn clip_policy(&mut self) -> u32 {
        self.i[23] * 0x100
    }
    fn clip_slot_flags(&mut self) -> u32 {
        self.i[24]
    }
    fn clip_loaded(&mut self, _: i16) -> bool {
        self.i[14] != 0
    }
    fn floor_height(&mut self, _: &HeroCtx) -> f32 {
        self.calls.push(0xc3e60);
        f32::from_bits(self.i[60])
    }
    fn install_entity(&mut self, _: &mut HeroCtx, _: &mut HeroGlobals, _: i32) {
        self.calls.extend([0x177ec0, 3, 1, 8]);
    }
    fn despawn(&mut self, _: &mut HeroCtx, _: &mut HeroGlobals) {
        self.calls.push(0x15dda0);
    }
}
struct JoinedWorld(World);
impl HeroPoseWorld for JoinedWorld {
    fn plays(&mut self, id: u32) -> bool {
        self.0.calls.extend([0x59d20, id]);
        self.0.i[43] >> (id % 32) & 1 != 0
    }
    fn transition(&mut self, id: u32) -> bool {
        self.0.calls.extend([0x59db0, id]);
        self.0.i[44] >> (id % 32) & 1 != 0
    }
    fn finished(&mut self) -> bool {
        self.0.calls.push(0x59dd0);
        self.0.i[45] != 0
    }
    fn rng_raw(&mut self) -> u32 {
        self.0.calls.push(0x153b40);
        self.0.i[42]
    }
    fn play(&mut self, id: u32, b: f32, s: f32) {
        self.0.calls.extend([0x5f460, id, b.to_bits(), s.to_bits()]);
    }
    fn navigate(&mut self, _: &mut HeroPoseCtx) {
        self.0.calls.push(0x15d470);
    }
    fn stop_motion(&mut self, _: &mut HeroPoseCtx) {
        self.0.calls.push(0x158db0);
    }
}
impl HeroWorld for JoinedWorld {
    fn awareness(&mut self, c: &mut HeroCtx, b: bool) -> bool {
        self.0.awareness(c, b)
    }
    fn leaf(&mut self, l: HeroLeaf, c: &mut HeroCtx, g: &mut HeroGlobals) -> u32 {
        self.0.leaf(l, c, g)
    }
    fn event(&mut self, c: &mut HeroCtx, b: u32) -> bool {
        self.0.event(c, b)
    }
    fn event_object_ready(&mut self, o: u32) -> bool {
        self.0.event_object_ready(o)
    }
    fn command(&mut self, s: i32) -> HeroCommand {
        self.0.command(s)
    }
    fn sequence_step(&mut self, s: i32, n: i32) -> tsfp_gameplay::char_sequence::Step {
        self.0.sequence_step(s, n)
    }
    fn peers(&mut self, c: &mut HeroCtx, g: &HeroGlobals, n: i32) -> Vec<HeroTarget> {
        self.0.peers(c, g, n)
    }
    fn neighbors(&mut self, c: &HeroCtx, g: &HeroGlobals) -> Vec<HeroNode> {
        self.0.neighbors(c, g)
    }
    fn visible(&mut self, c: &HeroCtx, g: &HeroGlobals) -> bool {
        self.0.visible(c, g)
    }
    fn target_dormant(&mut self, c: &HeroCtx) -> bool {
        self.0.target_dormant(c)
    }
    fn clip_policy(&mut self) -> u32 {
        self.0.clip_policy()
    }
    fn clip_slot_flags(&mut self) -> u32 {
        self.0.clip_slot_flags()
    }
    fn clip_loaded(&mut self, id: i16) -> bool {
        self.0.clip_loaded(id)
    }
    fn floor_height(&mut self, c: &HeroCtx) -> f32 {
        self.0.floor_height(c)
    }
    fn install_entity(&mut self, c: &mut HeroCtx, g: &mut HeroGlobals, n: i32) {
        self.0.install_entity(c, g, n)
    }
    fn despawn(&mut self, c: &mut HeroCtx, g: &mut HeroGlobals) {
        self.0.despawn(c, g)
    }
}
#[test]
fn joined_hero_tick_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/hero_tick.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut c = HeroCtx {
            pose: HeroPoseCtx {
                state: i[0],
                pending: i[1] as i32,
                previous: 9,
                state_time: 0.3,
                flags: i[34],
                auxiliary_flags: i[33],
                clock: f32::from_bits(i[35]),
                repeat: i[37] as i32,
                weapon_flags: i[27],
                model: i[26],
                record_tag: i[28],
                target_present: i[29] != 0,
                fast_mode: i[21] != 0,
            },
            actor: 7,
            object: 0x2d00000,
            drain: 0.25,
            tick_rate: 60,
            delta_ticks: 1.0,
            step: 2,
            sequence_step: 0,
            command: command(&i),
            position: std::array::from_fn(|n| f32::from_bits(i[46 + n])),
            destination: [2.0, 3.0, 4.0],
            origin: i[54] as i32,
            destination_node: i[55] as i32,
            node: HeroNode {
                index: 3,
                tag: 3,
                flags: i[32],
                position: [0.0; 3],
                yaw: 0.0,
            },
            saved_node: None,
            route: 0,
            walk_timer: 21,
            walk_progress: 23,
            target: if i[29] != 0 { Some(target(&i)) } else { None },
            team: 1,
            health: 50.0,
            max_health: 100.0,
            health_protected: false,
            heal_value: 3.0,
            action_clock: f32::from_bits(i[36]),
            action_word: i[53],
            motion_value: 0.0,
            entity: 0,
            resolved_entity: 0,
            object_room: 9,
            velocity: [1.0; 2],
            animation_motion: [1.0; 4],
            animation_height: i[61] as f32,
            animation_speed: 0.5,
            current_masks: [0x2000, 8],
            pending_mask: 8,
            requested_mask: 0,
            pickup_handles: [i[58], 0, 0],
        };
        let mut g = HeroGlobals {
            mode: 10,
            level: 5,
            cue_enabled: i[52] != 0,
            cue: if i[52] != 0 { 7 } else { u32::MAX },
            initial: i[20] != 0,
            alternate_step: 9,
            override_drive: false,
            feature: false,
            forced_step: -1,
            clip_override: [-1; 2],
            entity_radius_squared: 100.0,
            branches: std::array::from_fn(|n| HeroBranch {
                step: if n == 0 && i[50] != 0 { 5 } else { -1 },
                nodes: [
                    if n == 0 && i[50] != 0 {
                        i[54] as i32
                    } else {
                        -1
                    },
                    -1,
                    -1,
                ],
            }),
            events: std::array::from_fn(|n| {
                if n == 0 && i[51] != 0 {
                    (123, 4)
                } else {
                    (0, -1)
                }
            }),
            actor_nodes: if i[50] != 0 {
                vec![i[54] as i32, i[31] as i32]
            } else {
                vec![]
            },
            player: target(&i),
            player_team: 1,
            player_node_tag: i[31] as i32,
            selected: if i[13] != 0 { 7 } else { 9 },
        };
        let mut w = JoinedWorld(World {
            i,
            calls: Vec::new(),
        });
        char_tick_hero::tick(&mut c, &mut g, &mut w);
        let mut actual = vec![
            c.pose.state,
            c.pose.pending as u32,
            c.pose.previous,
            c.pose.state_time.to_bits(),
            c.pose.flags,
            c.pose.auxiliary_flags,
            c.drain.to_bits(),
            c.pose.clock.to_bits(),
            c.pose.repeat as u32,
            c.step as u32,
            c.destination_node as u32,
            c.walk_timer,
            c.walk_progress,
        ];
        actual.extend(c.destination.map(f32::to_bits));
        actual.extend([
            c.target.as_ref().map(|t| t.object).unwrap_or(0),
            c.action_clock.to_bits(),
            c.route as u32,
            c.sequence_step as u32,
            c.entity,
            c.motion_value.to_bits(),
            c.pending_mask as u32,
            c.requested_mask as u32,
            (c.requested_mask >> 32) as u32,
            c.heal_value.to_bits(),
            c.velocity[0].to_bits(),
            c.velocity[1].to_bits(),
            c.object_room as u32,
        ]);
        actual.extend(c.animation_motion.map(f32::to_bits));
        actual.push(c.animation_speed.to_bits());
        actual.extend([
            u32::from(g.cue_enabled),
            g.cue,
            g.alternate_step as u32,
            u32::from(g.override_drive),
            u32::from(g.feature),
            g.forced_step as u32,
            g.clip_override[0] as u32,
            g.clip_override[1] as u32,
            g.entity_radius_squared.to_bits(),
            g.branches[0].step as u32,
            g.branches[0].nodes[0] as u32,
            g.branches[0].nodes[1] as u32,
            g.branches[0].nodes[2] as u32,
            g.branches[1].step as u32,
            g.branches[1].nodes[0] as u32,
            g.branches[1].nodes[1] as u32,
            g.branches[1].nodes[2] as u32,
            g.events[0].0,
            g.events[0].1 as u32,
        ]);
        actual.extend([c.health.to_bits(), 0]); // OBJ+0x68 is never written by the original tick
        actual.extend(w.0.calls.into_iter().filter(|a| *a != 0x9a420));
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 3000);
}
