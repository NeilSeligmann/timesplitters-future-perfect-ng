//! Readable mode-A class tick, original `0x265CC0` (T1964, class mask `0x40_0000_0000_0000`, dispatched by
//! `0x27D590` when the mode byte `0x7DE455` is `0xA`). Verification tier 1 (original-world differential,
//! controlled leaves), see `tests/char_tick_class_a_t1964.rs`.
//!
//! The class is a walking follower with five states. State 0 only queues state 1. State 1 (setup) plays the idle
//! clip `0x16C`, looks up the node of the selector key, repositions to it when it changed, picks the follow target
//! node from key `0x3E9` (falling back to the player's node) and starts navigation to the route node. State 2
//! (walk) selects the key, target node and walk clip from the selector `E+8` (1..=5), then queues state 1 while the
//! route query `0x15D500` is true, otherwise plays the clip and refreshes floor and yaw. State 3 (arrival) does the
//! same but on route success it rolls a random clip index 0..=5 from the table `0x520240` and queues state 4.
//! State 4 plays that clip until it finishes (or its duration passes) and queues state 2.

/// One row of the table at `0x520240` (stride 12): clip id, duration in seconds, "wait for finish" flag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrivalClip {
    pub id: u32,
    pub duration: f32,
    pub wait_for_finish: bool,
}

/// Original table `0x520240`, six rows.
pub const ARRIVAL_CLIPS: [ArrivalClip; 6] = [
    ArrivalClip {
        id: 267,
        duration: 15.0,
        wait_for_finish: false,
    },
    ArrivalClip {
        id: 1791,
        duration: 15.0,
        wait_for_finish: true,
    },
    ArrivalClip {
        id: 2482,
        duration: 15.0,
        wait_for_finish: true,
    },
    ArrivalClip {
        id: 268,
        duration: 15.0,
        wait_for_finish: false,
    },
    ArrivalClip {
        id: 269,
        duration: 15.0,
        wait_for_finish: false,
    },
    ArrivalClip {
        id: 270,
        duration: 15.0,
        wait_for_finish: false,
    },
];

const IDLE_CLIP: u32 = 0x16C;
const WALK_BLEND: f32 = 0.33; // 0x3EA8F5C3
/// Node key looked up for the follow target (`0x3E9`).
pub const TARGET_KEY: u32 = 0x3E9;
/// Selector `E+8` (1..=5) to (node key, walk clip id): `0x2661A8` keys, `0x2660B0` ids.
pub const SELECTOR_ROWS: [(u32, u32); 5] = [
    (0x3EB, 0x171),
    (0x49F, 0x2A),
    (0x49C, 0x113),
    (0x47D, 0x6BF),
    (0x47E, 0x2A),
];
const DEFAULT_WALK_CLIP: u32 = 0x2A;

#[derive(Clone, Debug, PartialEq)]
pub struct ClassACtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    /// Byte `E+0x2C`; bit 7 enables the timer.
    pub flags_byte: u8,
    pub timer: f32,
    pub tick_rate: i32,
    pub time_scale: f32,
    /// `E+8`, the selector.
    pub selector: u32,
    /// `E+0x678`, the current node.
    pub node: i32,
    /// `E+0x684`, the target node.
    pub target_node: i32,
    pub clear_a: u32,
    pub clear_b: u32,
    /// `[E+0xACC] + 0x18`, else `[E+0x67C] + 0x18`.
    pub route_word_a: Option<u32>,
    pub route_word_b: u32,
    /// `obj+0x1A0`, the arrival clip row.
    pub arrival_index: u32,
}

pub trait ClassAWorld {
    /// `0x15E120(E, 1)`, result unused.
    fn aware(&mut self, ctx: &mut ClassACtx);
    /// `0x59D20(obj, id)`.
    fn plays(&mut self, id: u32) -> bool;
    /// `0x5F460(obj, id, blend, speed)`.
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    /// `0x9CC40(key)`: node of a key, negative when absent.
    fn key_node(&mut self, key: u32) -> i32;
    /// `[[0x7B0C48]+0x14]->ext+0x678`: the player's node.
    fn player_node(&mut self) -> i32;
    /// `0xDA280(E, node, 1)`.
    fn reposition(&mut self, ctx: &mut ClassACtx, node: i32);
    /// `0x15D500(obj, node)`.
    fn route(&mut self, node: i32) -> bool;
    /// `0x159300(obj, word, 0)`.
    fn start_navigation(&mut self, ctx: &mut ClassACtx, word: u32);
    /// `0x15F0D0(obj)`.
    fn update_floor(&mut self);
    /// `0x15CD00(obj)`.
    fn update_yaw(&mut self);
    /// `0x59DD0(info)`.
    fn finished(&mut self) -> bool;
    /// `0x153B40()`, raw 32 bit random.
    fn rng_raw(&mut self) -> u32;
}

fn selector_row(selector: u32) -> Option<(u32, u32)> {
    SELECTOR_ROWS
        .get((selector as usize).wrapping_sub(1))
        .copied()
}

fn queue(ctx: &mut ClassACtx, next: i32) {
    if ctx.state as i32 != next {
        ctx.pending = next;
    }
}

fn start_clip(ctx: &ClassACtx, world: &mut impl ClassAWorld, id: u32) {
    let _ = ctx;
    if !world.plays(id) {
        world.play(id, WALK_BLEND, 1.0);
    }
}

fn finish(world: &mut impl ClassAWorld) {
    world.update_floor();
    world.update_yaw();
}

/// Target node of key `0x3E9`, the player's node when the key is absent (the lookup is repeated when present).
fn follow_target(world: &mut impl ClassAWorld, key: u32) -> i32 {
    if world.key_node(key) < 0 {
        world.player_node()
    } else {
        world.key_node(key)
    }
}

/// One tick of `0x265CC0`.
pub fn tick(ctx: &mut ClassACtx, world: &mut impl ClassAWorld) {
    world.aware(ctx);
    if ctx.pending >= 0 {
        let next = ctx.pending as u32;
        if next != ctx.state {
            ctx.state_time = 0.0;
            ctx.previous = ctx.state;
            ctx.state = next;
        }
        ctx.pending = -1;
    }
    ctx.timer = if ctx.flags_byte & 0x80 != 0 {
        1.0f32 / ctx.tick_rate as f32 * ctx.time_scale + ctx.timer
    } else {
        0.0
    };
    match ctx.state {
        0 => ctx.pending = 1,
        1 => setup(ctx, world),
        2 => walk(ctx, world),
        3 => arrival(ctx, world),
        4 => arrival_play(ctx, world),
        _ => {}
    }
}

fn setup(ctx: &mut ClassACtx, world: &mut impl ClassAWorld) {
    if !world.plays(IDLE_CLIP) {
        world.play(IDLE_CLIP, 0.0, 1.0);
    }
    let mut node = -1;
    if let Some((key, _)) = selector_row(ctx.selector) {
        node = world.key_node(key);
    }
    if ctx.node != node {
        world.reposition(ctx, node);
    }
    ctx.target_node = follow_target(world, TARGET_KEY);
    ctx.clear_a = 0;
    ctx.clear_b = 0;
    let word = ctx.route_word_a.unwrap_or(ctx.route_word_b);
    world.start_navigation(ctx, word);
}

/// Shared first half of states 2 and 3: pick key, target node and clip from the selector.
fn select(ctx: &mut ClassACtx, world: &mut impl ClassAWorld) -> u32 {
    let Some((key, clip)) = selector_row(ctx.selector) else {
        return DEFAULT_WALK_CLIP;
    };
    ctx.target_node = follow_target(world, key);
    ctx.clear_a = 0;
    ctx.clear_b = 0;
    clip
}

fn walk(ctx: &mut ClassACtx, world: &mut impl ClassAWorld) {
    let clip = select(ctx, world);
    if world.route(ctx.target_node) {
        queue(ctx, 1);
        return;
    }
    start_clip(ctx, world, clip);
    finish(world);
}

fn arrival(ctx: &mut ClassACtx, world: &mut impl ClassAWorld) {
    // Route failure plays the selector row clip; route success plays a random arrival clip and then checks the
    // default walk clip `0x2A` (original `0x2660CA` reuses EDI = 0x2A, a quirk kept as is).
    let mut clip = selector_row(ctx.selector).map_or(DEFAULT_WALK_CLIP, |row| row.1);
    if world.route(ctx.target_node) {
        let raw = world.rng_raw();
        let index = (f64::from(raw) * 2.328_306_436_538_696_3e-10_f64 * 5.0 + 0.5) as u32;
        if ctx.state != 4 {
            ctx.pending = 4;
        }
        ctx.arrival_index = index;
        let row = ARRIVAL_CLIPS[(index as usize).min(5)];
        world.play(row.id, WALK_BLEND, 1.0);
        clip = DEFAULT_WALK_CLIP;
    }
    start_clip(ctx, world, clip);
    finish(world);
}

fn arrival_play(ctx: &mut ClassACtx, world: &mut impl ClassAWorld) {
    let Some(row) = ARRIVAL_CLIPS.get(ctx.arrival_index as usize).copied() else {
        return;
    };
    if row.wait_for_finish {
        if world.finished() {
            queue(ctx, 2);
            return;
        }
    } else if ctx.state_time >= row.duration {
        queue(ctx, 2);
        return;
    }
    if !world.plays(row.id) {
        world.play(row.id, WALK_BLEND, 1.0);
    }
}
