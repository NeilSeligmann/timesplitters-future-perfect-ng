//! Hero/script controller0x2784D0. Mutable engine leaves retain original contracts.
use crate::char_tick_hero_pose::{self, HeroPoseCtx, HeroPoseWorld};
#[derive(Clone, Debug, PartialEq)]
pub struct HeroTarget {
    pub object: u32,
    pub position: [f32; 3],
    pub health: f32,
    pub node: i32,
    pub kind: u32,
    pub character: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HeroNode {
    pub index: i32,
    pub tag: i32,
    pub flags: u32,
    pub position: [f32; 3],
    pub yaw: f32,
}
#[derive(Clone, Debug)]
pub struct HeroCommand {
    pub kind: u32,
    pub condition: u32,
    pub target_selector: u32,
    pub nodes: [i32; 3],
    pub parameter: u32,
    pub delay: f32,
    pub cue: u32,
    pub flags: u32,
    pub jump: i32,
    pub clip: i16,
}
#[derive(Clone, Debug)]
pub struct HeroBranch {
    pub step: i32,
    pub nodes: [i32; 3],
}
#[derive(Clone, Debug)]
pub struct HeroGlobals {
    pub mode: u8,
    pub level: u32,
    pub cue_enabled: bool,
    pub cue: u32,
    pub initial: bool,
    pub alternate_step: i32,
    pub override_drive: bool,
    pub feature: bool,
    pub forced_step: i32,
    pub clip_override: [i32; 2],
    pub entity_radius_squared: f32,
    pub branches: [HeroBranch; 2],
    pub events: [(u32, i32); 8],
    pub actor_nodes: Vec<i32>,
    pub player: HeroTarget,
    pub player_team: u32,
    pub player_node_tag: i32,
    pub selected: u32,
}
#[derive(Clone, Debug)]
pub struct HeroCtx {
    pub pose: HeroPoseCtx,
    pub actor: u32,
    pub object: u32,
    pub drain: f32,
    pub tick_rate: i32,
    pub delta_ticks: f32,
    pub step: i32,
    pub sequence_step: i32,
    pub command: HeroCommand,
    pub position: [f32; 3],
    pub destination: [f32; 3],
    pub origin: i32,
    pub destination_node: i32,
    pub node: HeroNode,
    pub saved_node: Option<HeroNode>,
    pub route: i32,
    pub walk_timer: u32,
    pub walk_progress: u32,
    pub target: Option<HeroTarget>,
    pub team: u32,
    pub health: f32,
    pub max_health: f32,
    pub health_protected: bool,
    pub heal_value: f32,
    pub action_clock: f32,
    pub action_word: u32,
    pub motion_value: f32,
    pub entity: u32,
    pub resolved_entity: u32,
    pub object_room: i32,
    pub velocity: [f32; 2],
    pub animation_motion: [f32; 4],
    pub animation_height: f32,
    pub animation_speed: f32,
    pub current_masks: [u64; 2],
    pub pending_mask: u64,
    pub requested_mask: u64,
    pub pickup_handles: [u32; 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeroLeaf {
    Prepare,
    ApproachReady,
    FireWeapon,
    Route,
    RouteStatus,
    Refresh,
    ValidateCue(u32),
    Heal,
    Initial,
    FindTarget(bool),
    Suppressed,
    NodeTag(i32),
    NodeRecordTag(i32),
    NodeIndex(i32),
    NodeRecordIndex,
    Path(i32),
    Walk,
    Motion,
    Stop,
    Stand(f32),
    Blend(u32),
    FacePlayer,
    FaceYaw(f32),
    Align,
    Advance,
    TargetCallback(u32),
    Condition(u32),
    DormantTarget,
    PlayersNear,
    Approach,
    Acquire,
    Exit,
    ChooseClose,
    ReleaseRoute(u32),
    ResolvePickup(u32),
    PickupReady,
    PickupNode,
    BranchCondition(i32),
    EntityClearance,
    PlayerEntity,
    EntityFromTag(i32),
    EntityPosition,
    Locate,
    FloorEnabled(u32),
    FloorHeight,
    FloorRoom,
    FireAnimation,
    StreamZone,
    StreamStatus(u32),
    SetNode(i32, bool),
    EventObject(i32),
    Sound(i32, i32),
    Class(u64),
}
impl HeroLeaf {
    pub fn original_address(self) -> u32 {
        match self {
            Self::Prepare => 0x276b60,
            Self::ApproachReady => 0x276540,
            Self::FireWeapon => 0x277530,
            Self::Route => 0x1596f0,
            Self::RouteStatus => 0x2762e0,
            Self::Refresh => 0x276660,
            Self::ValidateCue(_) => 0x160ec0,
            Self::Heal => 0x2777e0,
            Self::Initial => 0x2774d0,
            Self::FindTarget(_) => 0x276f80,
            Self::Suppressed => 0x2772b0,
            Self::NodeTag(_) => 0x9cc40,
            Self::NodeRecordTag(_) => 0x9cca0,
            Self::NodeIndex(_) => 0x9a420,
            Self::NodeRecordIndex => 0x9a510,
            Self::Path(_) => 0x15d500,
            Self::Walk => 0x15cd00,
            Self::Motion => 0x277e00,
            Self::Stop => 0x158db0,
            Self::Stand(_) => 0x277d90,
            Self::Blend(_) => 0x5c1c0,
            Self::FacePlayer => 0x158f80,
            Self::FaceYaw(_) => 0x159300,
            Self::Align => 0x266250,
            Self::Advance => 0x261990,
            Self::TargetCallback(_) => 0x520648,
            Self::Condition(_) => 0x520628,
            Self::DormantTarget => 0x174e0,
            Self::PlayersNear => 0x2779f0,
            Self::Approach => 0x278070,
            Self::Acquire => 0x277bc0,
            Self::Exit => 0x277d30,
            Self::ChooseClose => 0x277b60,
            Self::ReleaseRoute(_) => 0x9a570,
            Self::ResolvePickup(_) => 0x12f00,
            Self::PickupReady => 0xb0f60,
            Self::PickupNode => 0x9a570,
            Self::BranchCondition(_) => 0x2273c0,
            Self::EntityClearance => 0x17c6a0,
            Self::PlayerEntity => 0x12f00,
            Self::EntityFromTag(_) => 0x23ff20,
            Self::EntityPosition => 0x1679d0,
            Self::Locate => 0x9bcf0,
            Self::FloorEnabled(_) => 0x59e00,
            Self::FloorHeight => 0xc3e60,
            Self::FloorRoom => 0xc3c50,
            Self::FireAnimation => 0x1bb5c0,
            Self::StreamZone => 0x66560,
            Self::StreamStatus(_) => 0x61cb0,
            Self::SetNode(_, _) => 0xda280,
            Self::EventObject(_) => 0x234190,
            Self::Sound(_, _) => 0x21970,
            Self::Class(_) => 0x17570,
        }
    }
}
pub trait HeroWorld: HeroPoseWorld {
    fn awareness(&mut self, c: &mut HeroCtx, update: bool) -> bool;
    /// Non-float engine helpers return original EAX and may synchronise modeled
    /// context/global writes. Node/target/entity lookups populate their typed fields.
    /// Condition and TargetCallback resolve the supplied original selector index.
    fn leaf(&mut self, l: HeroLeaf, c: &mut HeroCtx, g: &mut HeroGlobals) -> u32;
    fn event(&mut self, c: &mut HeroCtx, bits: u32) -> bool;
    fn event_object_ready(&mut self, object: u32) -> bool;
    fn command(&mut self, step: i32) -> HeroCommand;
    fn sequence_step(&mut self, sequence: i32, step: i32) -> crate::char_sequence::Step;
    fn peers(&mut self, c: &mut HeroCtx, g: &HeroGlobals, group: i32) -> Vec<HeroTarget>;
    /// Original9A420(player node),9D4A0,9C550 output list, preserving list order.
    fn neighbors(&mut self, c: &HeroCtx, g: &HeroGlobals) -> Vec<HeroNode>;
    /// Original15BD00(character index,player index), first byte of output.
    fn visible(&mut self, c: &HeroCtx, g: &HeroGlobals) -> bool;
    /// Snapshot query0x174E0 on the current target.
    fn target_dormant(&mut self, c: &HeroCtx) -> bool;
    fn clip_policy(&mut self) -> u32;
    fn clip_slot_flags(&mut self) -> u32;
    fn clip_loaded(&mut self, id: i16) -> bool;
    /// Original floor query's x87 return, with model dimensions and original geometry.
    fn floor_height(&mut self, c: &HeroCtx) -> f32;
    /// Original177EC0(entity,node,command.nodes[1],command.nodes[2]).
    fn install_entity(&mut self, c: &mut HeroCtx, g: &mut HeroGlobals, node: i32);
    fn despawn(&mut self, c: &mut HeroCtx, g: &mut HeroGlobals);
}
fn enter(c: &mut HeroCtx, s: u32) {
    if c.pose.state != s {
        c.pose.previous = c.pose.state;
        c.pose.state = s;
        c.pose.pending = s as i32;
        c.pose.state_time = 0.0;
    }
}
fn decode_enter(c: &mut HeroCtx, s: u32) {
    c.pose.previous = 1;
    c.pose.state = s;
    c.pose.pending = s as i32;
    c.pose.state_time = 0.0;
}
fn next(c: &mut HeroCtx) {
    c.step = c.step.wrapping_add(1);
    enter(c, 1);
}
fn reset_walk(c: &mut HeroCtx) {
    c.walk_timer = 0;
    c.walk_progress = 0;
}
fn destination(c: &mut HeroCtx, g: &HeroGlobals, n: i32) {
    if c.destination_node != n {
        c.destination_node = if n < 0 { g.player.node } else { n };
        reset_walk(c);
    }
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| {
            let d = f64::from(a) - f64::from(b);
            d * d
        })
        .sum()
}
fn horizontal(a: [f32; 3], b: [f32; 3]) -> f64 {
    let x = f64::from(a[0]) - f64::from(b[0]);
    let z = f64::from(a[2]) - f64::from(b[2]);
    x * x + z * z
}
fn leaf(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld, l: HeroLeaf) -> u32 {
    w.leaf(l, c, g)
}
fn aim(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld) {
    leaf(c, g, w, HeroLeaf::Stop);
    c.pose.flags |= 2;
    if w.clip_policy() & 0x100 != 0 {
        c.pose.flags &= !0x40000;
        if w.clip_slot_flags() & 1 != 0 {
            leaf(c, g, w, HeroLeaf::Blend(2));
        }
    }
    c.pose.auxiliary_flags |= 0x400;
}
fn stand_blend(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld) {
    c.pose.flags &= !0x60000;
    leaf(c, g, w, HeroLeaf::Blend(1));
}
fn advance(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld) -> bool {
    let result = leaf(c, g, w, HeroLeaf::Advance);
    if result == 0 {
        return false;
    }
    if result == 1 {
        c.step = c.step.wrapping_add(1);
    } else {
        g.alternate_step = c.step;
        c.step = c.command.jump;
    }
    enter(c, 1);
    true
}
fn exit(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld) {
    if leaf(c, g, w, HeroLeaf::Exit) != 0 {
        next(c);
    }
}
fn peer_acquire(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld) -> bool {
    w.peers(c, g, c.command.nodes[1]);
    leaf(c, g, w, HeroLeaf::Acquire) != 0
}
fn common_pose_tail(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld, aware: bool) {
    if (c.pose.state_time > 3.0 || c.target.is_none()) && c.actor == g.selected {
        w.peers(c, g, c.command.nodes[1]);
        if leaf(c, g, w, HeroLeaf::ChooseClose) != 0 {
            c.pose.flags &= !2;
            c.pose.auxiliary_flags &= !0x400;
            enter(c, 6);
        } else if !aware {
            c.pose.clock = 0.0;
            enter(c, 9);
        }
    }
    exit(c, g, w);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flow {
    Tail,
    Return,
    Decode,
}
fn command_step(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld) -> bool {
    c.pose.clock = 0.0;
    c.pose.flags &= !2;
    match c.command.kind {
        0 => decode_enter(c, 2),
        1 => {
            if c.destination_node < 0 {
                c.destination_node = if c.origin < 0 {
                    g.player.node
                } else {
                    c.origin
                };
                reset_walk(c);
            }
            decode_enter(c, 4);
        }
        2 => decode_enter(c, 3),
        3 => {
            c.target = None;
            decode_enter(c, 5);
        }
        5 => {
            let id = c.command.clip as i32 as u32;
            if w.transition(id) {
                w.play(id, if c.command.flags & 4 == 0 { 0.0 } else { 0.33 }, 1.0);
            }
            enter(c, 12);
        }
        6 => decode_enter(c, 14),
        7 => decode_enter(c, 15),
        9 => {
            g.cue_enabled = false;
            c.step = c.step.wrapping_add(1);
        }
        10 => {
            g.cue_enabled = true;
            g.cue = c.command.cue;
            c.step = c.step.wrapping_add(1);
        }
        11 => decode_enter(c, 19),
        12 => w.despawn(c, g),
        13 | 14 => {
            g.clip_override[if c.command.kind == 13 { 0 } else { 1 }] = i32::from(c.command.clip);
            c.step = c.step.wrapping_add(1);
        }
        15 | 16 => {
            g.clip_override[if c.command.kind == 15 { 0 } else { 1 }] = -1;
            c.step = c.step.wrapping_add(1);
        }
        17 => {
            c.pose.auxiliary_flags |= 0x1000;
            c.step = c.step.wrapping_add(1);
        }
        18 => {
            c.pose.auxiliary_flags |= 0x4000;
            c.step = c.step.wrapping_add(1);
        }
        19 => {
            c.pose.auxiliary_flags &= !0x5000;
            c.step = c.step.wrapping_add(1);
        }
        20 => {
            let v = f32::from_bits(c.command.nodes[0] as u32);
            g.entity_radius_squared = v * v;
            c.step = c.step.wrapping_add(1);
        }
        21 => {
            let id = i32::from(c.command.clip) as u32;
            if c.command.clip != -1 && !w.plays(id) {
                if !w.plays(id) {
                    w.play(id, 0.0, 0.0);
                }
                return false;
            }
            let n = leaf(c, g, w, HeroLeaf::NodeTag(c.command.nodes[0])) as i32;
            leaf(
                c,
                g,
                w,
                HeroLeaf::SetNode(n, c.command.flags & 0x20000 != 0),
            );
            destination(c, g, n);
            if c.command.clip != -1 {
                c.velocity = [0.0; 2];
            }
            c.step = c.step.wrapping_add(1);
        }
        22 => c.step = g.alternate_step,
        23 => decode_enter(c, 16),
        24 | 25 => {
            let idx = if c.command.kind == 24 { 0 } else { 1 };
            let mut branch = HeroBranch {
                step: -1,
                nodes: [-1; 3],
            };
            if c.command.jump != -1 {
                branch.step = c.command.jump;
                branch.nodes[0] = leaf(c, g, w, HeroLeaf::NodeTag(c.command.nodes[0])) as i32;
                if c.command.nodes[1] != -1 {
                    branch.nodes[1] = leaf(c, g, w, HeroLeaf::NodeTag(c.command.nodes[1])) as i32;
                }
                if c.command.parameter as i32 != -1 {
                    branch.nodes[2] =
                        leaf(c, g, w, HeroLeaf::NodeTag(c.command.parameter as i32)) as i32;
                }
            }
            g.branches[idx] = branch;
            c.step = c.step.wrapping_add(1);
        }
        26 => decode_enter(c, 20),
        27 => {
            if advance(c, g, w) {
                return true;
            }
            c.step = c.step.wrapping_add(1);
        }
        30 => decode_enter(c, 21),
        31 | 32 => {
            let index = if c.command.nodes[1] == -1 {
                0
            } else {
                c.command.nodes[1] - 1
            } as usize;
            g.events[index] = if c.command.kind == 31 {
                (
                    leaf(c, g, w, HeroLeaf::EventObject(c.command.nodes[0])),
                    c.command.jump,
                )
            } else {
                (0, -1)
            };
            c.step = c.step.wrapping_add(1);
        }
        33 | 34 => {
            g.feature = c.command.kind == 33;
            c.step = c.step.wrapping_add(1);
        }
        35 => {
            g.forced_step = c.command.jump;
            c.step = c.step.wrapping_add(1);
        }
        36 => {
            g.forced_step = -1;
            c.step = c.step.wrapping_add(1);
        }
        37 | 38 => {
            g.branches[if c.command.kind == 37 { 0 } else { 1 }] = HeroBranch {
                step: -1,
                nodes: [-1; 3],
            };
            c.step = c.step.wrapping_add(1);
        }
        41 => {
            if c.destination_node < 0 {
                c.destination_node = if c.origin < 0 {
                    g.player.node
                } else {
                    c.origin
                };
                reset_walk(c);
            }
            c.target = None;
            c.pose.clock = 0.0;
            c.action_clock = 0.0;
            decode_enter(c, 6);
        }
        42 => {
            if leaf(c, g, w, HeroLeaf::EntityFromTag(c.command.nodes[0])) == 0 {
                if !w.plays(0x25) {
                    w.play(0x25, 0.3, 1.0);
                }
                return false;
            }
            leaf(c, g, w, HeroLeaf::EntityPosition);
            let node = leaf(c, g, w, HeroLeaf::Locate) as i32;
            w.install_entity(c, g, node);
            c.step = c.step.wrapping_add(1);
        }
        45 => {
            let duration =
                (f64::from(c.tick_rate) * f64::from(f32::from_bits(c.command.parameter))) as i32;
            leaf(c, g, w, HeroLeaf::Sound(c.command.nodes[0], duration));
            c.step = c.step.wrapping_add(1);
        }
        _ => {}
    }
    false
}
fn state_step(
    c: &mut HeroCtx,
    g: &mut HeroGlobals,
    w: &mut impl HeroWorld,
    aware: bool,
    player_distance: f64,
) -> Flow {
    match c.pose.state {
        0 => {
            if g.initial {
                leaf(c, g, w, HeroLeaf::Initial);
                c.team = g.player_team;
                leaf(c, g, w, HeroLeaf::FindTarget(true));
                c.pose.clock = 0.0;
                g.initial = false;
                g.alternate_step = -1;
            }
            c.requested_mask = 0x2000;
            enter(c, 1);
        }
        1 => return Flow::Decode,
        2 => {
            if leaf(c, g, w, HeroLeaf::Suppressed) != 0 {
                leaf(c, g, w, HeroLeaf::Stop);
            } else {
                leaf(c, g, w, HeroLeaf::NodeRecordTag(c.command.nodes[0]));
                let n = if c.node.flags & 0x800 == 0 {
                    leaf(c, g, w, HeroLeaf::NodeRecordIndex) as i32
                } else {
                    leaf(c, g, w, HeroLeaf::NodeTag(c.node.tag as i16 as i32)) as i32
                };
                destination(c, g, n);
                if leaf(c, g, w, HeroLeaf::ApproachReady) != 0 {
                    enter(c, 13);
                }
                if leaf(c, g, w, HeroLeaf::Path(c.destination_node)) != 0 {
                    c.destination = c.node.position;
                    if distance(c.position, c.destination) < f64::from(0.45_f32) {
                        next(c);
                    }
                }
                leaf(c, g, w, HeroLeaf::Motion);
                leaf(c, g, w, HeroLeaf::Walk);
                stand_blend(c, g, w);
            }
        }
        3 => {
            let dest_valid = leaf(c, g, w, HeroLeaf::NodeIndex(c.destination_node)) != 0;
            let dest = c.node.clone();
            leaf(c, g, w, HeroLeaf::NodeIndex(c.origin));
            let origin = c.node.clone();
            let visible = w.visible(c, g);
            if leaf(c, g, w, HeroLeaf::Suppressed) != 0 {
                return Flow::Tail;
            }
            if distance(origin.position, g.player.position) >= 100.0 || !visible {
                if !dest_valid
                    || distance(dest.position, g.player.position) > 100.0
                    || (c.origin == c.destination_node && !visible)
                {
                    let neighbors = w.neighbors(c, g);
                    let mut nearest = None;
                    let mut minimum = f64::from(f32::MAX);
                    for n in neighbors {
                        let d = horizontal(g.player.position, n.position);
                        if n.index != g.player.node && (nearest.is_none() || d < minimum) {
                            minimum = d;
                            nearest = Some(n.index);
                        }
                    }
                    c.destination_node = nearest.unwrap_or(if c.origin < 0 {
                        g.player.node
                    } else {
                        c.origin
                    });
                    reset_walk(c);
                }
            } else {
                c.destination_node = if c.origin < 0 {
                    g.player.node
                } else {
                    c.origin
                };
                reset_walk(c);
            }
            if c.origin == c.destination_node {
                if c.command.flags & 1 != 0 && c.target.as_ref().is_some_and(|t| t.health > 0.0) {
                    leaf(c, g, w, HeroLeaf::FireWeapon);
                    if aware {
                        c.pose.flags |= 2;
                    }
                    c.pose.flags |= 0x40000;
                    leaf(c, g, w, HeroLeaf::Blend(2));
                } else {
                    leaf(c, g, w, HeroLeaf::Stand(0.33));
                    stand_blend(c, g, w);
                }
                leaf(c, g, w, HeroLeaf::FacePlayer);
            } else {
                leaf(c, g, w, HeroLeaf::Path(c.destination_node));
                leaf(c, g, w, HeroLeaf::Motion);
                leaf(c, g, w, HeroLeaf::Walk);
            }
            if c.command.condition != 0
                && leaf(c, g, w, HeroLeaf::Condition(c.command.condition)) != 0
            {
                next(c);
            }
        }
        4 => {
            leaf(c, g, w, HeroLeaf::NodeIndex(c.destination_node));
            let d = distance(c.position, c.node.position);
            let firing = c.command.flags & 1 != 0;
            if leaf(c, g, w, HeroLeaf::Suppressed) != 0 {
                if d > f64::from(0.80999994_f32) {
                    leaf(c, g, w, HeroLeaf::Walk);
                } else {
                    leaf(c, g, w, HeroLeaf::Stop);
                }
                return Flow::Tail;
            }
            if c.command.flags & 0x40000 != 0 {
                leaf(c, g, w, HeroLeaf::Align);
            }
            if c.target
                .as_ref()
                .is_some_and(|t| t.kind == 8 && t.character)
                && w.target_dormant(c)
            {
                c.target = None;
            }
            if !aware || (firing && c.actor == g.selected && c.pose.clock > 4.0) {
                let old = if aware {
                    c.target
                        .as_ref()
                        .map(|t| horizontal(c.position, t.position))
                        .unwrap_or(f64::from(f32::MAX))
                } else {
                    f64::from(f32::MAX)
                };
                let prior = c.target.clone();
                leaf(c, g, w, HeroLeaf::FindTarget(false));
                if c.target.is_none()
                    || (aware
                        && c.target.as_ref().is_some_and(|t| {
                            horizontal(c.position, t.position) >= old * f64::from(0.9_f32)
                        }))
                {
                    c.target = prior;
                } else {
                    c.pose.clock = 0.0;
                }
            }
            if c.command.condition != 0
                && leaf(c, g, w, HeroLeaf::Condition(c.command.condition)) != 0
            {
                next(c);
            }
            if advance(c, g, w) {
                return Flow::Return;
            }
            if (!firing || !aware || c.target.is_none())
                && d > f64::from(0.80999994_f32)
                && leaf(c, g, w, HeroLeaf::ApproachReady) == 0
            {
                leaf(c, g, w, HeroLeaf::Walk);
                leaf(c, g, w, HeroLeaf::Motion);
                if leaf(c, g, w, HeroLeaf::Path(c.destination_node)) != 0 {
                    leaf(c, g, w, HeroLeaf::NodeIndex(c.destination_node));
                    c.destination = c.node.position;
                }
                return Flow::Return;
            }
            if !firing || !aware {
                if d <= f64::from(0.80999994_f32) {
                    if c.command.flags & 2 == 0 {
                        let yaw = c.saved_node.as_ref().unwrap_or(&c.node).yaw;
                        leaf(c, g, w, HeroLeaf::FaceYaw(yaw));
                        stand_blend(c, g, w);
                    } else {
                        leaf(c, g, w, HeroLeaf::FacePlayer);
                        stand_blend(c, g, w);
                    }
                } else {
                    leaf(c, g, w, HeroLeaf::Walk);
                    stand_blend(c, g, w);
                }
            } else {
                leaf(c, g, w, HeroLeaf::Stop);
                c.pose.flags |= 0x20000;
                if w.clip_policy() & 0x100 != 0 {
                    c.pose.flags &= !0x40000;
                    if w.clip_slot_flags() & 1 != 0 {
                        leaf(c, g, w, HeroLeaf::Blend(2));
                    }
                }
            }
            if !firing || !aware {
                leaf(c, g, w, HeroLeaf::Stand(0.33));
            } else {
                if c.target.is_some() {
                    c.pose.flags |= 2;
                }
                leaf(c, g, w, HeroLeaf::FireWeapon);
            }
        }
        5 => {
            if leaf(c, g, w, HeroLeaf::Suppressed) != 0 {
                leaf(c, g, w, HeroLeaf::Stop);
                return Flow::Tail;
            }
            if c.command.target_selector != 0 {
                leaf(c, g, w, HeroLeaf::TargetCallback(c.command.target_selector));
            }
            if c.command.flags & 0x40000 != 0 {
                leaf(c, g, w, HeroLeaf::Align);
            }
            if c.target.as_ref().is_none_or(|t| t.health <= 0.0)
                || (!w.awareness(c, false) && c.command.flags & 0x400000 != 0)
            {
                next(c);
            }
            if !aware {
                stand_blend(c, g, w);
            } else {
                aim(c, g, w);
            }
            leaf(c, g, w, HeroLeaf::FireWeapon);
            if advance(c, g, w) {
                return Flow::Return;
            }
        }
        6 => {
            w.peers(c, g, c.command.nodes[1]);
            let state = if leaf(c, g, w, HeroLeaf::PlayersNear) != 0 {
                8
            } else if leaf(c, g, w, HeroLeaf::Approach) != 0 {
                7
            } else if aware {
                10
            } else {
                9
            };
            c.pose.clock = 0.0;
            enter(c, state);
            if leaf(c, g, w, HeroLeaf::Exit) != 0 {
                next(c);
            } else {
                if c.target.is_none() {
                    leaf(c, g, w, HeroLeaf::Acquire);
                }
                if c.target.is_none() {
                    enter(c, 13);
                }
            }
        }
        7 => {
            leaf(c, g, w, HeroLeaf::Motion);
            leaf(c, g, w, HeroLeaf::Walk);
            if leaf(c, g, w, HeroLeaf::Path(c.destination_node)) != 0
                || (c.pose.flags & 0x8000 != 0 && c.pose.clock > 2.0)
            {
                w.peers(c, g, c.command.nodes[1]);
                let acquired = leaf(c, g, w, HeroLeaf::Acquire) != 0;
                c.pose.flags &= !0x8000;
                if acquired {
                    c.pose.clock = 0.0;
                    enter(c, 10);
                } else {
                    enter(c, 9);
                }
            } else if c.actor == g.selected && c.pose.clock > 4.0 {
                w.peers(c, g, c.command.nodes[1]);
                let acquired = leaf(c, g, w, HeroLeaf::Acquire) != 0;
                c.pose.flags &= !0x8000;
                if acquired
                    && c.target
                        .as_ref()
                        .is_some_and(|t| horizontal(c.position, t.position) < 100.0)
                {
                    c.pose.clock = 0.0;
                    enter(c, 10);
                }
            }
            exit(c, g, w);
        }
        8 => {
            leaf(c, g, w, HeroLeaf::Walk);
            if !w.plays(0x1b1) {
                w.play(0x1b1, 0.33, 1.0);
            }
            if c.actor == g.selected && c.pose.clock > 4.0 {
                let peers = w.peers(c, g, c.command.nodes[1]);
                if peers
                    .into_iter()
                    .all(|t| horizontal(c.position, t.position) >= 6.25)
                {
                    enter(c, 6);
                }
            }
            exit(c, g, w);
        }
        9 => {
            if let Some(target) = c.target.clone() {
                leaf(c, g, w, HeroLeaf::Motion);
                leaf(c, g, w, HeroLeaf::Walk);
                destination(c, g, target.node);
                if aware {
                    c.pose.clock = 0.0;
                    enter(c, 10);
                } else if leaf(c, g, w, HeroLeaf::Path(c.destination_node)) != 0 {
                    enter(c, 6);
                }
                if c.actor == g.selected {
                    if peer_acquire(c, g, w) {
                        c.pose.clock = 0.0;
                        enter(c, 10);
                    }
                }
                exit(c, g, w);
            } else {
                peer_acquire(c, g, w);
                enter(c, 6);
            }
        }
        10 | 11 => {
            c.pose.target_present = c.target.is_some();
            if c.pose.state == 10 {
                char_tick_hero_pose::state10(&mut c.pose, w);
            } else {
                char_tick_hero_pose::state11(&mut c.pose, w);
            }
            common_pose_tail(c, g, w, aware);
        }
        12 => {
            if c.animation_height > 0.0 && c.command.flags & 4 != 0 {
                c.animation_motion = [0.0; 4];
            }
            if c.command.flags & 2 == 0 {
                if c.command.flags & 0x20000 != 0 {
                    let yaw = c.saved_node.as_ref().unwrap_or(&c.node).yaw;
                    leaf(c, g, w, HeroLeaf::FaceYaw(yaw));
                }
            } else if leaf(c, g, w, HeroLeaf::FacePlayer) == 0 {
                return Flow::Tail;
            }
            if c.command.flags & 0x40000 != 0 {
                leaf(c, g, w, HeroLeaf::Align);
            }
            if w.event(c, 0x200) {
                if c.command.target_selector != 0 {
                    leaf(c, g, w, HeroLeaf::TargetCallback(c.command.target_selector));
                }
                if c.target.is_none() {
                    c.target = Some(HeroTarget {
                        object: c.object,
                        position: c.position,
                        health: c.health,
                        node: c.origin,
                        kind: 0,
                        character: true,
                    });
                }
                c.pose.flags |= 2;
                c.pose.auxiliary_flags |= 0x400;
                leaf(c, g, w, HeroLeaf::FireAnimation);
                c.pose.flags &= !2;
                c.pose.auxiliary_flags &= !0x400;
            } else {
                c.pose.flags &= !2;
            }
            if advance(c, g, w) {
                return Flow::Return;
            }
            let id = c.command.clip as i32 as u32;
            if !w.clip_loaded(c.command.clip) {
                let zone = leaf(c, g, w, HeroLeaf::StreamZone);
                if leaf(c, g, w, HeroLeaf::StreamStatus(zone)) != 0 && !w.plays(0x25) {
                    w.play(0x25, 0.33, 1.0);
                }
                return Flow::Tail;
            }
            if !w.plays(id) {
                w.play(id, if c.command.flags & 4 == 0 { 0.33 } else { 0.0 }, 1.0);
            }
            let complete = if c.command.condition == 0 {
                w.transition(id)
            } else {
                leaf(c, g, w, HeroLeaf::Condition(c.command.condition)) != 0
            };
            if complete {
                let n = leaf(c, g, w, HeroLeaf::Locate) as i32;
                if n != c.origin {
                    c.origin = n;
                    reset_walk(c);
                }
                if c.command.condition == 0
                    && leaf(c, g, w, HeroLeaf::FloorEnabled(0x800000)) != 0
                    && leaf(c, g, w, HeroLeaf::FloorEnabled(0x40)) == 0
                {
                    let y = w.floor_height(c);
                    if y == -f32::MAX {
                        leaf(c, g, w, HeroLeaf::Stand(0.0));
                    } else if (c.position[1] - y).abs() >= 0.1 {
                        c.motion_value = y;
                        c.position[1] = y;
                        c.object_room = leaf(c, g, w, HeroLeaf::FloorRoom) as i32;
                        leaf(c, g, w, HeroLeaf::Stand(0.0));
                    }
                }
                next(c);
            }
            c.animation_speed = 1.0;
        }
        13 => {
            if leaf(c, g, w, HeroLeaf::Suppressed) == 0 {
                leaf(c, g, w, HeroLeaf::Stand(0.33));
            }
            leaf(c, g, w, HeroLeaf::FacePlayer);
            stand_blend(c, g, w);
            if c.pose.flags & 0x10000 == 0 {
                if matches!(c.command.kind, 1 | 3) {
                    if player_distance <= 36.0 && c.action_clock <= 2.0 {
                        return Flow::Return;
                    }
                } else if c.command.kind == 41 {
                    peer_acquire(c, g, w);
                    if c.target.is_none() {
                        return Flow::Return;
                    }
                    c.pose.clock = 0.0;
                    enter(c, 6);
                    reset_walk(c);
                    return Flow::Return;
                } else {
                    if leaf(c, g, w, HeroLeaf::ApproachReady) != 0 {
                        return Flow::Return;
                    }
                    if c.command.flags & 0x200000 != 0 {
                        let status = leaf(c, g, w, HeroLeaf::RouteStatus);
                        if status != 0 && status != 3 {
                            return Flow::Return;
                        }
                    }
                }
                c.action_clock = 0.0;
                g.override_drive = false;
                enter(c, 1);
                reset_walk(c);
                return Flow::Return;
            }
            if c.actor != g.selected {
                return Flow::Return;
            }
            c.route = leaf(c, g, w, HeroLeaf::Route) as i32;
            if c.route == -2 {
                let mut removed = false;
                for i in 0..3 {
                    if c.pickup_handles[i] != 0 {
                        leaf(c, g, w, HeroLeaf::ResolvePickup(c.pickup_handles[i]));
                        if leaf(c, g, w, HeroLeaf::PickupReady) != 0 {
                            leaf(c, g, w, HeroLeaf::PickupNode);
                            c.pickup_handles[i] = 0;
                            removed = true;
                        }
                    }
                }
                if !removed {
                    return Flow::Return;
                }
            }
            reset_walk(c);
            c.route = leaf(c, g, w, HeroLeaf::Route) as i32;
            if c.route < 1 {
                return Flow::Return;
            }
            c.action_clock = 0.0;
            g.override_drive = false;
            c.pose.flags &= !0x10000;
            enter(c, 1);
            return Flow::Return;
        }
        14 => {
            c.step = c.command.jump;
            enter(c, 1);
        }
        15 => {
            c.step = if leaf(c, g, w, HeroLeaf::ValidateCue(c.command.cue)) != 0 {
                c.command.jump
            } else {
                c.step.wrapping_add(1)
            };
            enter(c, 1);
        }
        16 => {
            c.step = if leaf(c, g, w, HeroLeaf::BranchCondition(c.command.nodes[0])) != 0 {
                c.command.jump
            } else {
                c.step.wrapping_add(1)
            };
            enter(c, 1);
        }
        17 => {
            let s = w.sequence_step(c.pose.repeat, c.sequence_step);
            if s.kind == 18 {
                next(c);
            } else {
                let n = leaf(c, g, w, HeroLeaf::NodeTag(s.value as i32)) as i32;
                c.destination_node = if n < 0 {
                    g.player.node
                } else {
                    leaf(c, g, w, HeroLeaf::NodeTag(s.value as i32)) as i32
                };
                reset_walk(c);
                c.sequence_step = c.sequence_step.wrapping_add(1);
                enter(c, 18);
            }
        }
        18 => {
            if leaf(c, g, w, HeroLeaf::Path(c.destination_node)) != 0 {
                enter(c, 17);
            } else {
                leaf(c, g, w, HeroLeaf::Motion);
                leaf(c, g, w, HeroLeaf::Walk);
            }
        }
        19 => {
            let node = g.player_node_tag;
            c.step = c.step.wrapping_add(if c.command.nodes.contains(&node) {
                c.command.parameter as i32
            } else {
                1
            });
            enter(c, 1);
        }
        20 => {
            let yaw = c.saved_node.as_ref().unwrap_or(&c.node).yaw;
            leaf(c, g, w, HeroLeaf::FaceYaw(yaw));
            stand_blend(c, g, w);
            leaf(c, g, w, HeroLeaf::Stand(0.33));
            if distance(c.position, g.player.position) <= f64::from(g.entity_radius_squared)
                && leaf(c, g, w, HeroLeaf::EntityClearance) != 0
            {
                if leaf(c, g, w, HeroLeaf::PlayerEntity) == 0 {
                    return Flow::Tail;
                }
                leaf(c, g, w, HeroLeaf::Class(0x4000));
                c.entity = c.resolved_entity;
                c.step = c.step.wrapping_add(1);
            } else {
                next(c);
            }
        }
        21 => {
            if leaf(c, g, w, HeroLeaf::EntityFromTag(c.command.nodes[0])) != 0 {
                leaf(c, g, w, HeroLeaf::Class(0x4000));
                c.entity = c.resolved_entity;
                c.step = c.step.wrapping_add(1);
            }
        }
        _ => {}
    }
    Flow::Tail
}
/// Complete hero controller entry. Level/player/node/record snapshots are original
/// host inputs; helpers are explicit mutable leaves rather than guessed engines.
pub fn tick(c: &mut HeroCtx, g: &mut HeroGlobals, w: &mut impl HeroWorld) {
    let aware = w.awareness(c, true);
    if c.pose.pending >= 0 {
        let s = c.pose.pending as u32;
        if s != c.pose.state {
            c.pose.previous = c.pose.state;
            c.pose.state = s;
            c.pose.state_time = 0.0;
        }
        c.pose.pending = -1;
    }
    c.drain = if c.pose.auxiliary_flags & 0x80 != 0 {
        c.drain + (1.0 / c.tick_rate as f32) * c.delta_ticks
    } else {
        0.0
    };
    if c.pose.auxiliary_flags & 4 != 0 {
        return;
    }
    let player_distance = distance(c.position, g.player.position);
    if g.branches.iter().any(|b| b.step != -1) {
        for node in g.actor_nodes.clone() {
            if let Some(index) = g.branches.iter().position(|b| b.nodes.contains(&node)) {
                c.step = g.branches[index].step;
                enter(c, 1);
                g.branches[index] = HeroBranch {
                    step: -1,
                    nodes: [-1; 3],
                };
                break;
            }
        }
    }
    for i in 0..8 {
        if g.events[i].0 != 0 && w.event_object_ready(g.events[i].0) {
            c.step = g.events[i].1;
            enter(c, 1);
            g.events[i] = (0, -1);
        }
    }
    c.heal_value = 0.0;
    leaf(c, g, w, HeroLeaf::Prepare);
    if g.cue_enabled && g.cue != u32::MAX && leaf(c, g, w, HeroLeaf::ValidateCue(g.cue)) != 0 {
        g.cue_enabled = false;
        g.cue = u32::MAX;
    }
    leaf(c, g, w, HeroLeaf::Refresh);
    c.command = w.command(c.step);
    let healing = c.action_word;
    let healing = f32::from_bits(healing);
    if healing > 5.0 && !c.health_protected && g.level != 26 {
        leaf(c, g, w, HeroLeaf::Heal);
        c.health = (c.health + healing).clamp(0.0, c.max_health);
    }
    match state_step(c, g, w, aware, player_distance) {
        Flow::Return => return,
        Flow::Decode => {
            if command_step(c, g, w) {
                return;
            }
        }
        Flow::Tail => {}
    }
    if !g.override_drive && c.pose.flags & 1 != 0 {
        let mask = c.current_masks[if g.mode == 10 { 0 } else { 1 }];
        c.requested_mask = mask;
        if c.pending_mask == 1 && mask & 1 == 0 {
            return;
        }
        c.pending_mask = 0x20;
    }
}
