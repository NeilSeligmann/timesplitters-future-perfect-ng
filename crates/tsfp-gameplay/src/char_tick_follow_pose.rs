//! T1877: animation decision slice of common NPC tick 0x267450 (0x267599..0x267684).
//! Movement, target acquisition and tick epilogue are outside this selector.

pub trait FollowPoseWorld {
    /// Original 0x59D20: queried with short circuit ordering.
    fn plays(&mut self, id: u32) -> bool;
    /// Original 0x59DB0: the clip's transition/reselection predicate.
    fn reselect(&mut self, id: u32) -> bool;
    fn rng_raw(&mut self) -> u32;
    fn play(&mut self, id: u32, blend: f32, speed: f32);
}

/// Weapon flag bit 1 takes precedence over weapon types 0xE, 0x35 and 0xF.
/// The original tables at 0x5201F8/0x520208 contain three corresponding pairs.
pub fn select(weapon_flags: u32, weapon_type: u32, world: &mut impl FollowPoseWorld) {
    let variant = if weapon_flags & 1 != 0 {
        1
    } else if matches!(weapon_type, 0xE | 0x35 | 0xF) {
        2
    } else {
        0
    };
    let first = [1540, 1557, 1585][variant];
    let second = [1541, 1558, 1586][variant];
    if !world.plays(first) && !world.plays(second) {
        world.play(
            first,
            f32::from_bits(0x3EA8F5C3),
            f32::from_bits(0x3FA66666),
        );
    } else if world.reselect(first) || world.reselect(second) {
        // Original unsigned fraction + 0.5, then 0x3C848C truncates to integer.
        // This threshold is exact for every u32 input (no f32 rounding).
        let id = if world.rng_raw() < 0x80000000 {
            first
        } else {
            second
        };
        world.play(id, 0.0, f32::from_bits(0x3FA66666));
    }
}

/// State consumed by the full common follow tick 0x267450 (53 spawn records).
#[derive(Clone, Debug, PartialEq)]
pub struct FollowCtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub flags: u32,
    pub auxiliary_flags: u32,
    pub speed_scale: f32,
    pub target: u32,
    pub position: [u32; 3],
    pub object_present: bool,
    pub object_flags: u32,
    pub object_type: u32,
    pub index: i32,
    pub first_npc: i32,
    /// C+0xAF4 compared against 16.0 before pose selection.
    pub range_value: f32,
    pub clock: f32,
    pub weapon_flags: u32,
    pub weapon_type: u32,
    pub callback_index: i32,
    pub has_record_callback: bool,
}
pub trait FollowWorld: FollowPoseWorld {
    fn aware(&mut self) -> bool;
    fn acquire_target(&mut self) -> u32;
    /// 0x2663D0 and 0x2A5C30, respectively; either nonzero exits the tick.
    fn first_update(&mut self) -> bool;
    fn second_update(&mut self) -> bool;
    fn change_class(&mut self, mask: u64);
    /// 0x15D470 then target O+0x64/68/6C copied into C+0x30/34/38.
    fn track_target(&mut self, target: u32) -> [u32; 3];
    /// 0x15A270 followed by 0x15D500 with its result.
    fn navigation_node(&mut self) -> u32;
    fn navigate(&mut self, node: u32);
    /// 0x59E00(O,8,0).
    fn channel_ready(&mut self) -> bool;
    /// Record+0x78 function pointer; nonzero result replaces the target.
    fn record_callback(&mut self) -> u32;
}

/// Full outer tick with engine leaves supplied by the caller. `drain` is the
/// original frame expression, shared with the other class ticks.
/// Engine leaves in this selection port must preserve the fields modelled here;
/// production integration that permits callee writes must sync them at the boundary.
pub fn tick(ctx: &mut FollowCtx, drain: f32, world: &mut impl FollowWorld) {
    let aware = world.aware();
    if ctx.pending >= 0 {
        let next = ctx.pending as u32;
        if next != ctx.state {
            ctx.previous = ctx.state;
            ctx.state = next;
            ctx.state_time = 0.0;
        }
        ctx.pending = -1;
    }
    ctx.speed_scale = if ctx.auxiliary_flags as u8 & 0x80 != 0 {
        ctx.speed_scale + drain
    } else {
        0.0
    };
    if ctx.target == 0 {
        ctx.target = world.acquire_target();
        return;
    }
    if world.first_update() || world.second_update() {
        return;
    }
    if ctx.range_value < 16.0 {
        world.change_class(4);
        return;
    }
    if aware
        && (!ctx.object_present
            || ctx.index < ctx.first_npc
            || (ctx.object_flags & 0x100 == 0 && ctx.object_type & 0xFFFF != 0))
    {
        ctx.position = world.track_target(ctx.target);
    } else {
        let node = world.navigation_node();
        world.navigate(node);
    }
    select(ctx.weapon_flags, ctx.weapon_type, world);
    if aware {
        ctx.flags |= 2;
    }
    if world.channel_ready() {
        ctx.auxiliary_flags &= !0x400;
    } else {
        ctx.auxiliary_flags |= 0x400;
    }
    if ctx.clock > f32::from_bits(0x3E99999A) && ctx.flags & 1 != 0 {
        world.change_class(8);
    }
    if ctx.index == ctx.callback_index && ctx.has_record_callback {
        let target = world.record_callback();
        if target != 0 {
            ctx.target = target;
        }
    }
}
