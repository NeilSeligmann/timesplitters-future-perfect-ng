//! Readable setup and nine-state controller for scripted NPC tick 0x26EFC0.
//! Joined entry/return comparisons use controlled leaves; live actor/world integration and replay adoption remain separate.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptPoseCtx {
    pub weapon_slot: i32,
    pub weapon_flags: u32,
    pub random_time: f32,
    pub clock: f32,
    pub flags: u32,
    pub record_flags: u32,
}
pub trait ScriptPoseWorld {
    fn plays(&mut self, id: u32) -> bool;
    /// 0x59D70, distinct from the playing and transition predicates.
    fn phase_open(&mut self, id: u32) -> bool;
    fn transition(&mut self, id: u32) -> bool;
    fn rng_raw(&mut self) -> u32;
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    /// 0x26D6D0(O,steps) updates the outer sequence state.
    fn advance_sequence(&mut self);
    /// 0x26DD80, EBX=object (usercall).
    fn post_update(&mut self);
}

pub fn state4(ctx: &mut ScriptPoseCtx, world: &mut impl ScriptPoseWorld) {
    let variant = usize::from(ctx.weapon_slot == -1 || ctx.weapon_flags & 1 != 0);
    let base = [0xAEB, 0xAEC][variant];
    let entry = [0xAE9, 0xAEA][variant];
    let alternate = [0xAED, 0xAEE][variant];
    let timeout = [0xAD1, 0xAD2][variant];
    let blend = f32::from_bits(0x3EA8F5C3);
    if !world.plays(base) && !world.plays(entry) && !world.plays(alternate) {
        world.play(base, blend, 1.0);
    }
    if world.phase_open(base) && ctx.random_time > 3.0 && !world.plays(alternate) {
        world.play(alternate, blend, 1.0);
    } else if world.transition(alternate) {
        ctx.random_time = (f64::from(world.rng_raw()) / 4_294_967_296.0) as f32;
        world.play(base, 0.0, 1.0);
    }
    if ctx.clock > 15.0 && ctx.record_flags & 0x20 == 0 && ctx.flags & 0x10000 == 0 {
        world.play(timeout, blend, 1.0);
        world.advance_sequence();
    }
    world.post_update();
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptMotionCtx {
    pub pose: ScriptPoseCtx,
    /// Original object O+0x64/68/6C and resolved node+0xC/10/14.
    /// These are snapshots: callers must supply stable geometry across the arm.
    pub position: [f32; 3],
    pub node_position: [f32; 3],
}
pub trait ScriptMotionWorld: ScriptPoseWorld {
    /// 0x26EA00(steps), EDI=object. May update the modelled state.
    fn begin_motion(&mut self, ctx: &mut ScriptMotionCtx);
    /// 0x2661F0(object_position,node_position,facing), original x87 result stored f32.
    fn heading_delta(&mut self) -> f32;
    /// 0x61570(object,1), animation event/time predicate.
    fn event_ready(&mut self) -> bool;
}

/// State 5, original 0x26FDA8..0x27071F, selection and advance decisions.
/// Geometry reads are represented by stable snapshots; helper internals remain engine leaves.
pub fn state5(ctx: &mut ScriptMotionCtx, world: &mut impl ScriptMotionWorld) {
    world.begin_motion(ctx);
    let variant = usize::from(ctx.pose.weapon_slot == -1 || ctx.pose.weapon_flags & 1 != 0);
    let id = |base: u32| base + variant as u32;
    let start = id(0xAD7);
    let forward = id(0xADB);
    let backward = id(0xAD9);
    let moving = id(0xADD);
    let alternate = id(0xADF);
    let turn = id(0xAE1);
    let special_a = id(0xAC9);
    let special_b = id(0xACB);
    let height = (ctx.node_position[1] - ctx.position[1]).abs();
    let dx = ctx.position[0] - ctx.node_position[0];
    let dz = ctx.position[2] - ctx.node_position[2];
    let distance_squared = dx * dx + dz * dz;
    let active = [
        start, moving, alternate, turn, forward, backward, special_a, special_b,
    ]
    .into_iter()
    .any(|id| world.plays(id));
    let mut selected = None;
    if !active {
        selected = Some(start);
    } else if [start, special_a, special_b]
        .into_iter()
        .any(|id| world.transition(id))
    {
        selected = Some(if world.heading_delta().abs() <= 90.0 {
            forward
        } else {
            backward
        });
    } else if [forward, backward, alternate, turn]
        .into_iter()
        .any(|id| world.transition(id))
    {
        selected = Some(moving);
    } else if world.plays(moving) && world.event_ready() {
        let draw = (f64::from(world.rng_raw()) / 4_294_967_296.0 * 6.0 + 0.5) as u32;
        if draw == 0 {
            if !(height < 0.5 && distance_squared < 36.0) {
                selected = Some(turn);
            }
        } else if draw < 3 {
            selected = Some(alternate);
        }
    }
    if let Some(id) = selected {
        world.play(id, 0.0, 1.0);
    }
    if [forward, backward, moving, alternate]
        .into_iter()
        .any(|id| world.plays(id))
    {
        let reached = height < 0.5 && distance_squared < 16.0;
        if (reached
            || ctx.pose.flags & 1 != 0
            || (ctx.pose.clock > 20.0 && ctx.pose.record_flags & 0x20 == 0))
            && world.event_ready()
        {
            let blend = if world.plays(alternate) {
                f32::from_bits(0x3EA8F5C3)
            } else {
                0.0
            };
            world.play(id(0xACD), blend, 1.0);
            world.advance_sequence();
        }
    }
    world.post_update();
}

/// State shared by the alert/return arms (2 and 3) of 0x26EFC0.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptAlertCtx {
    pub pose: ScriptPoseCtx,
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub aware: bool,
    pub target_type: u32,
    pub target_extension_node: i32,
    pub target_object_node: i32,
    pub player_node: i32,
    pub parked_node: i32,
    pub node: i32,
    pub walk_timer: u32,
    pub walk_progress: u32,
    /// C+0xB14 copied to C+0xAD0 on the default aware exit.
    pub stored_value: u32,
    pub exit_value: u32,
}
pub trait ScriptAlertWorld: ScriptPoseWorld {
    /// 0x267FF0(0,object,parked_node).
    fn alert_group(&mut self, node: i32);
    fn change_class(&mut self, mask: u64);
    /// 0x158DB0(object,0), only on aware state-2 exits.
    fn common_update(&mut self);
}
fn enter_alert(ctx: &mut ScriptAlertCtx, next: u32) {
    if ctx.state != next {
        ctx.previous = ctx.state;
        ctx.state = next;
        ctx.pending = next as i32;
        ctx.state_time = 0.0;
    }
}
/// Original state 2 at 0x271738..0x2718D8.
pub fn state2(ctx: &mut ScriptAlertCtx, world: &mut impl ScriptAlertWorld) {
    let variant = u32::from(ctx.pose.weapon_slot == -1 || ctx.pose.weapon_flags & 1 != 0);
    if world.transition(0xAC7 + variant) || world.transition(0xAE9 + variant) {
        ctx.parked_node = if ctx.target_type == 8 {
            ctx.target_extension_node
        } else {
            ctx.target_object_node
        };
        if ctx.aware {
            let mask = if ctx.pose.record_flags & 8 != 0 {
                0x0400_0000_0000_0000
            } else if ctx.pose.record_flags & 0x4000 != 0 {
                0x200
            } else {
                ctx.exit_value = ctx.stored_value;
                4
            };
            world.alert_group(ctx.parked_node);
            world.change_class(mask);
        } else {
            ctx.node = if ctx.parked_node < 0 {
                ctx.player_node
            } else {
                ctx.parked_node
            };
            ctx.walk_timer = 0;
            ctx.walk_progress = 0;
            ctx.pose.clock = 0.0;
            world.play(0xAC9 + variant, 0.0, 1.0);
            enter_alert(ctx, 5);
        }
    }
    if ctx.aware {
        world.common_update();
    }
}
/// Original state 3 at 0x271541..0x271738.
pub fn state3(ctx: &mut ScriptAlertCtx, world: &mut impl ScriptAlertWorld) {
    let variant = u32::from(ctx.pose.weapon_slot == -1 || ctx.pose.weapon_flags & 1 != 0);
    let base = 0xAC7 + variant;
    let entry = 0xAE9 + variant;
    if !world.plays(base) && !world.plays(entry) {
        let pose = if world.plays(0xAEB + variant) || world.plays(0xAED + variant) {
            entry
        } else {
            base
        };
        world.play(pose, f32::from_bits(0x3EA8F5C3), 1.0);
    }
    if world.phase_open(base) || world.phase_open(entry) {
        enter_alert(ctx, 2);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptTurnCtx {
    pub pose: ScriptPoseCtx,
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub step_kind: u32,
    pub turn_accumulator: f32,
}
pub trait ScriptTurnWorld: ScriptPoseWorld {
    /// 0x26CD50(object), may be queried twice.
    fn turning_pose(&mut self) -> bool;
    fn finished(&mut self) -> bool;
    /// Original 0x26CE20 offset + facing, subtracted from desired yaw and wrapped
    /// by 0x12540(delta,-180,180); provide its stored f32 result.
    fn phase_delta(&mut self) -> f32;
    /// 0x59D50 duration minus current clip time, times signed tick rate,
    /// converted by 0x3C848C (truncation), not rounded to nearest.
    fn remaining_frames(&mut self) -> i32;
    /// 0x12430(desired_yaw,facing), stored as f32.
    fn aim_delta(&mut self) -> f32;
}
/// Original state 8 at 0x2710F9..0x271541, including turning accumulator update.
pub fn state8(ctx: &mut ScriptTurnCtx, world: &mut impl ScriptTurnWorld) {
    let variant = usize::from(ctx.pose.weapon_slot == -1 || ctx.pose.weapon_flags & 1 != 0);
    let idle = [0xAB5, 0xAB6][variant];
    let stand = [0xAB2, 0xAB4][variant];
    let blend = f32::from_bits(0x3EA8F5C3);
    if !world.plays(idle) && !world.plays(stand) && !world.turning_pose() {
        world.play(idle, blend, 1.0);
        world.post_update();
        return;
    }
    if !world.transition(idle) && !world.transition(stand) {
        if world.turning_pose() {
            if world.finished() {
                world.play(idle, blend, 1.0);
            } else {
                let delta = world.phase_delta();
                if delta.abs() > f32::from_bits(0x3E99999A) {
                    let frames = world.remaining_frames();
                    if frames > 0 {
                        let mut increment = delta / frames as f32;
                        if increment > 0.5 {
                            increment = 0.5;
                        }
                        // Original bounds only the positive increment.
                        ctx.turn_accumulator += increment;
                    }
                }
            }
        }
    } else {
        let delta = world.aim_delta();
        if delta.abs() > 135.0 {
            world.play([0xAAB, 0xAAE][variant], 0.0, 1.0);
        } else if delta.abs() > 30.0 {
            let id = if delta <= 0.0 {
                [0xAAC, 0xAAF][variant]
            } else {
                [0xAAD, 0xAB0][variant]
            };
            world.play(id, 0.0, 1.0);
        } else {
            if ctx.step_kind == 1 {
                world.play([0xAB1, 0xAB3][variant], 0.0, 1.0);
            }
            if ctx.state != 1 {
                ctx.previous = ctx.state;
                ctx.state = 1;
                ctx.state_time = 0.0;
                ctx.pending = 1;
            }
        }
    }
    world.post_update();
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptSequenceCtx {
    pub step_index: u32,
    pub alert: ScriptAlertCtx,
    pub previous_class: u64,
    pub record_node: i32,
    pub sequence_index: i32,
    pub step_argument: u32,
    pub step_kind: u32,
    pub target_present: bool,
    pub fallback_node: i32,
}
pub trait ScriptSequenceWorld: ScriptAlertWorld {
    fn resolve_node(&mut self, tag: u32) -> i32;
    /// 0x26D390(EAX=object), 0x26E150(EAX=object), then 0x26EA00(steps,EDI=object).
    /// Sync original helper writes into the context before further decisions.
    fn update_path(&mut self, ctx: &mut ScriptSequenceCtx);
    fn despawn(&mut self);
}
fn use_parked_node(ctx: &mut ScriptSequenceCtx) {
    ctx.alert.node = if ctx.alert.parked_node < 0 {
        ctx.alert.player_node
    } else {
        ctx.alert.parked_node
    };
    ctx.alert.walk_timer = 0;
    ctx.alert.walk_progress = 0;
}
/// Original state 0 at 0x26F239: record flags choose startup node/pose/state.
pub fn state0(ctx: &mut ScriptSequenceCtx, world: &mut impl ScriptSequenceWorld) {
    let variant =
        u32::from(ctx.alert.pose.weapon_slot == -1 || ctx.alert.pose.weapon_flags & 1 != 0);
    if ctx.previous_class & 0x200 != 0 {
        world.play(0xAB5 + variant, f32::from_bits(0x3EA8F5C3), 1.0);
        world.advance_sequence();
        return;
    }
    let record_flags = ctx.alert.pose.record_flags;
    if record_flags & (0x40000 | 0x20) != 0 {
        ctx.alert.parked_node = if ctx.record_node >= 0 {
            world.resolve_node(ctx.record_node as u32)
        } else if ctx.target_present {
            ctx.alert.target_extension_node
        } else {
            ctx.fallback_node
        };
        ctx.alert.pose.clock = 2.0;
        if record_flags & 0x40000 != 0 {
            use_parked_node(ctx);
            world.play(0xAD3 + variant, 0.0, 1.0);
            enter_alert(&mut ctx.alert, 6);
        } else if ctx.sequence_index == -2 {
            let first = world.resolve_node(ctx.step_argument);
            ctx.alert.node = if first < 0 {
                ctx.alert.player_node
            } else {
                world.resolve_node(ctx.step_argument)
            };
            ctx.alert.walk_timer = 0;
            ctx.alert.walk_progress = 0;
            world.play(0xAEB + variant, 0.0, 1.0);
            enter_alert(&mut ctx.alert, 4);
        } else {
            use_parked_node(ctx);
            world.play(0xADD + variant, 0.0, 1.0);
            enter_alert(&mut ctx.alert, 5);
        }
    } else {
        let node = world.resolve_node(ctx.step_argument);
        ctx.alert.node = if node < 0 {
            ctx.alert.player_node
        } else {
            node
        };
        ctx.alert.walk_timer = 0;
        ctx.alert.walk_progress = 0;
        ctx.alert.pose.clock = 2.0;
        enter_alert(&mut ctx.alert, 1);
    }
}
/// Original state 1 at 0x26F565. Kind11 destroys the actor after post-update.
pub fn state1(ctx: &mut ScriptSequenceCtx, world: &mut impl ScriptSequenceWorld) {
    world.update_path(ctx);
    if ctx.alert.pose.flags & 0x400 != 0 {
        if ctx.alert.parked_node < 0 {
            ctx.alert.parked_node = ctx.fallback_node;
        }
        use_parked_node(ctx);
        ctx.alert.pose.clock = 0.0;
        if ctx.alert.aware {
            enter_alert(&mut ctx.alert, 3);
        } else {
            let variant =
                u32::from(ctx.alert.pose.weapon_slot == -1 || ctx.alert.pose.weapon_flags & 1 != 0);
            world.play(0xACB + variant, f32::from_bits(0x3EAA7EFA), 1.0);
            enter_alert(&mut ctx.alert, 5);
        }
    }
    world.post_update();
    if ctx.step_kind == 11 {
        world.despawn();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptWalkCtx {
    pub route_origin: i32,
    pub alert: ScriptAlertCtx,
    pub position: [f32; 3],
    pub node_position: [f32; 3],
    pub target_present: bool,
}
pub trait ScriptWalkWorld: ScriptAlertWorld {
    /// 0x26EA00; sync helper state before deciding.
    fn begin_walk(&mut self, ctx: &mut ScriptWalkCtx);
    /// Original 0x59D90 is distinct from the phase predicate 0x59D70.
    fn slot_active(&mut self, id: u32) -> bool;
}
/// Original state 6 at 0x26F685..0x26FAC8, under stable node geometry.
pub fn state6(ctx: &mut ScriptWalkCtx, world: &mut impl ScriptWalkWorld) {
    world.begin_walk(ctx);
    let col = u32::from(ctx.alert.pose.weapon_slot == -1 || ctx.alert.pose.weapon_flags & 1 != 0);
    let (base, entry, alternate) = (0xAD3 + col, 0xAC5 + col, 0xAD5 + col);
    if !world.plays(base) && !world.plays(entry) && !world.plays(alternate) {
        world.play(entry, 0.0, 1.0);
    } else if world.transition(entry) {
        world.play(base, 0.0, 1.0);
    } else if world.transition(alternate) {
        world.play(0xAD7 + col, 0.0, 1.0);
        ctx.alert.parked_node = ctx.alert.target_extension_node;
        ctx.alert.node = if ctx.alert.parked_node < 0 {
            ctx.alert.player_node
        } else {
            ctx.alert.parked_node
        };
        ctx.alert.walk_timer = 0;
        ctx.alert.walk_progress = 0;
        ctx.alert.pose.clock = 0.0;
        enter_alert(&mut ctx.alert, 5);
    }
    if world.plays(base) && !world.slot_active(alternate) {
        let dx = ctx.position[0] - ctx.node_position[0];
        let dz = ctx.position[2] - ctx.node_position[2];
        let near = (ctx.position[1] - ctx.node_position[1]).abs() < 0.5 && dx * dx + dz * dz < 4.0;
        if near || ctx.alert.pose.flags & 1 != 0 {
            if ctx.target_present {
                world.play(alternate, f32::from_bits(0x3EA8F5C3), 1.0);
            } else {
                world.advance_sequence();
            }
        }
    }
    world.post_update();
}

pub trait ScriptRouteWorld: ScriptWalkWorld {
    /// 0x9BCF0(object.position,0).
    fn locate_node(&mut self) -> i32;
    /// 0x1596F0(object,&C+678); sync helper writes into context.
    fn route_node(&mut self, ctx: &mut ScriptWalkCtx) -> i32;
    /// 0x2661F0 toward the specified node, stored to f32.
    fn route_heading(&mut self, node: i32) -> f32;
    /// 0x26D930(object), with helper writes synchronized.
    fn arrived(&mut self, ctx: &mut ScriptWalkCtx);
    /// 0x9A4C0(node), followed by 0x26CFF0(object).
    fn update_route(&mut self, ctx: &mut ScriptWalkCtx);
    fn event_ready(&mut self) -> bool;
}
fn close_to_node(ctx: &ScriptWalkCtx, squared: f32) -> bool {
    let dx = ctx.position[0] - ctx.node_position[0];
    let dz = ctx.position[2] - ctx.node_position[2];
    (ctx.position[1] - ctx.node_position[1]).abs() < 0.5 && dx * dx + dz * dz < squared
}
/// Original state7 at 0x27071F..0x2710F9. Geometry snapshots must follow helper writes.
pub fn state7(ctx: &mut ScriptWalkCtx, world: &mut impl ScriptRouteWorld) {
    let col = u32::from(ctx.alert.pose.weapon_slot == -1 || ctx.alert.pose.weapon_flags & 1 != 0);
    let (moving, alternate, turn, entry, forward, backward) = (
        0xAE3 + col,
        0xAE5 + col,
        0xAE7 + col,
        0xACD + col,
        0xAD1 + col,
        0xACF + col,
    );
    let mut selected = None;
    if !world.plays(moving)
        && !world.plays(alternate)
        && !world.plays(turn)
        && !world.plays(entry)
        && !world.plays(forward)
        && !world.plays(backward)
    {
        selected = Some(entry);
    } else if world.transition(entry) {
        let node = world.locate_node();
        // C+678 is the route origin, distinct from the destination C+684.
        // The route helper owns the origin update; it receives the selected node.
        if ctx.route_origin != node {
            ctx.route_origin = node;
            ctx.alert.walk_timer = 0;
            ctx.alert.walk_progress = 0;
        }
        let next = world.route_node(ctx);
        let turn_back = if next >= 0 {
            world.route_heading(next).abs() > 90.0
        } else if next == -1 {
            world.route_heading(ctx.route_origin).abs() > 90.0
        } else {
            false
        };
        selected = Some(if turn_back { backward } else { forward });
    } else if world.transition(forward)
        || world.transition(backward)
        || world.transition(alternate)
        || world.transition(turn)
    {
        if close_to_node(ctx, 9.0) {
            ctx.alert.pose.flags &= !0x400;
            world.arrived(ctx);
        } else {
            selected = Some(moving);
        }
    } else if world.plays(moving) && world.event_ready() {
        let choice = ((world.rng_raw() as f64 / 4294967296.0) * 6.0 + 0.5).trunc() as i32;
        if choice == 0 {
            if !close_to_node(ctx, 64.0) {
                selected = Some(turn);
            }
        } else if choice > 0 && choice < 3 {
            selected = Some(alternate);
        }
    }
    if let Some(id) = selected {
        world.play(id, 0.0, 1.0);
    }
    world.begin_walk(ctx);
    if world.plays(backward) || world.plays(forward) {
        world.update_route(ctx);
    }
    if (world.plays(moving) || world.plays(alternate))
        && close_to_node(ctx, 9.0)
        && world.event_ready()
    {
        ctx.alert.pose.flags &= !0x400;
        world.arrived(ctx);
    }
    world.post_update();
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptStep {
    pub kind: u32,
    pub argument: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptSetupCtx {
    pub sequence: ScriptSequenceCtx,
    pub auxiliary_flags: u8,
    pub drain: f32,
    pub tick_rate: i32,
    pub delta_ticks: f32,
    pub actor_index: i32,
    pub selected_index: i32,
    pub target: u32,
    pub record_tag: u32,
    pub object_flags: u32,
    pub position: [f32; 3],
    pub node_position: [f32; 3],
    pub motion_value: f32,
}
pub trait ScriptSetupWorld: ScriptSequenceWorld {
    /// 0x15E120(C,1), sync writes before processing pending state.
    fn awareness(&mut self, ctx: &mut ScriptSetupCtx) -> bool;
    /// Resolve the sequence table (+34, stride8 pointer/length), or update the two
    /// shared synthetic arguments at 0x52028C/520298 for index -2. Return current step.
    fn current_step(&mut self, ctx: &mut ScriptSetupCtx) -> ScriptStep;
    /// 0x15A540(object,player_object,0), sync target-dependent snapshots.
    fn acquire_target(&mut self, ctx: &mut ScriptSetupCtx) -> u32;
    fn movement_kind(&mut self, ctx: &mut ScriptSetupCtx) -> i32;
    /// 0x613D0(id,2,1).
    fn event_frame(&mut self, id: u32) -> i32;
    /// 0x26D1B0(motion_value), EAX=movement_kind, result XMM0.
    fn motion_distance(&mut self, kind: i32, value: f32) -> f32;
    /// 0x5BA40(object,event_seconds,2,value,remaining_distance,window_seconds).
    fn retime(&mut self, event_seconds: f32, value: f32, remaining: f32, window: f32);
}
/// Original outer setup 0x26EFC0..0x26F226, before the nine-state dispatch.
/// Geometry is stable between the node read and motion leaves, matching arm tests.
pub fn prepare(ctx: &mut ScriptSetupCtx, world: &mut impl ScriptSetupWorld) {
    ctx.sequence.alert.aware = world.awareness(ctx);
    let a = &mut ctx.sequence.alert;
    if a.pending >= 0 {
        let next = a.pending as u32;
        if next != a.state {
            a.state_time = 0.0;
            a.previous = a.state;
            a.state = next;
        }
        a.pending = -1;
    }
    ctx.drain = if ctx.auxiliary_flags & 0x80 != 0 {
        ctx.drain + (1.0 / ctx.tick_rate as f32) * ctx.delta_ticks
    } else {
        0.0
    };
    a.pose.flags |= 0x4000;
    let step = world.current_step(ctx);
    if ctx.target == 0 || ctx.actor_index == ctx.selected_index {
        ctx.target = world.acquire_target(ctx);
    }
    ctx.sequence.step_kind = step.kind;
    ctx.sequence.step_argument = step.argument;
    let col = u32::from(
        ctx.sequence.alert.pose.weapon_slot == -1 || ctx.sequence.alert.pose.weapon_flags & 1 != 0,
    );
    let id = 0xAC3 + col;
    if world.plays(id) && ctx.object_flags & 0x20 == 0 {
        let dx = ctx.position[0] - ctx.node_position[0];
        let dz = ctx.position[2] - ctx.node_position[2];
        let distance = (dx * dx + dz * dz).sqrt();
        let kind = world.movement_kind(ctx);
        let frame = if kind == 2 { world.event_frame(id) } else { 0 };
        let travelled = world.motion_distance(kind, ctx.motion_value);
        let reciprocal = 1.0 / ctx.tick_rate as f32;
        world.retime(
            frame as f32 * reciprocal,
            ctx.motion_value,
            distance - travelled,
            reciprocal * f32::from_bits(0x3E99999A),
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptTickCtx {
    pub setup: ScriptSetupCtx,
    pub route_origin: i32,
    pub turn_accumulator: f32,
}
/// Leaf implementations must synchronize all helper writes represented by their
/// mutable arm context. Other helper writes must be applied after the arm. This
/// final synchronization is a host operation, not an added original engine call.
pub trait ScriptTickWorld:
    ScriptSetupWorld + ScriptMotionWorld + ScriptTurnWorld + ScriptRouteWorld
{
    /// Apply writes performed by helpers without a mutable context (advance,
    /// common/post update), preserving stores made before that helper. The port
    /// has no live object-memory adapter; controlled-leaf worlds have no writes.
    fn finish_tick(&mut self, ctx: &mut ScriptTickCtx);
}
/// Joined original 0x26EFC0 setup and all nine state arms. Out-of-range states
/// return after setup without post-update, as in the original jump-table guard.
pub fn tick(ctx: &mut ScriptTickCtx, world: &mut impl ScriptTickWorld) {
    prepare(&mut ctx.setup, world);
    ctx.setup.sequence.target_present = ctx.setup.target != 0;
    match ctx.setup.sequence.alert.state {
        0 => state0(&mut ctx.setup.sequence, world),
        1 => state1(&mut ctx.setup.sequence, world),
        2 => state2(&mut ctx.setup.sequence.alert, world),
        3 => state3(&mut ctx.setup.sequence.alert, world),
        4 => state4(&mut ctx.setup.sequence.alert.pose, world),
        5 => {
            let mut arm = ScriptMotionCtx {
                pose: ctx.setup.sequence.alert.pose.clone(),
                position: ctx.setup.position,
                node_position: ctx.setup.node_position,
            };
            state5(&mut arm, world);
            ctx.setup.sequence.alert.pose = arm.pose;
            ctx.setup.position = arm.position;
            ctx.setup.node_position = arm.node_position;
        }
        6 | 7 => {
            let state = ctx.setup.sequence.alert.state;
            let mut arm = ScriptWalkCtx {
                alert: ctx.setup.sequence.alert.clone(),
                route_origin: ctx.route_origin,
                position: ctx.setup.position,
                node_position: ctx.setup.node_position,
                target_present: ctx.setup.sequence.target_present,
            };
            if state == 6 {
                state6(&mut arm, world);
            } else {
                state7(&mut arm, world);
            }
            ctx.setup.sequence.alert = arm.alert;
            ctx.route_origin = arm.route_origin;
            ctx.setup.position = arm.position;
            ctx.setup.node_position = arm.node_position;
        }
        8 => {
            let a = &ctx.setup.sequence.alert;
            let mut arm = ScriptTurnCtx {
                pose: a.pose.clone(),
                state: a.state,
                pending: a.pending,
                previous: a.previous,
                state_time: a.state_time,
                step_kind: ctx.setup.sequence.step_kind,
                turn_accumulator: ctx.turn_accumulator,
            };
            state8(&mut arm, world);
            let a = &mut ctx.setup.sequence.alert;
            a.pose = arm.pose;
            a.state = arm.state;
            a.pending = arm.pending;
            a.previous = arm.previous;
            a.state_time = arm.state_time;
            ctx.turn_accumulator = arm.turn_accumulator;
        }
        _ => {}
    }
    world.finish_tick(ctx);
}
