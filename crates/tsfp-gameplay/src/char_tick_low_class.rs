//! Readable NPC low-class tick, original VA 0x2AEFD0
//! (`game_char_low_class_tick_handler_turning_toward_target_with_nav_node_update_and_random_rolls`).
//! One call is one 60 Hz decision step of an NPC that melee/ranged-fights by animation
//! state: a common preamble (queued state, drain, weapon class, awareness hold, target and
//! position refresh), a twelve-way state switch (0 spawn, 1 idle, 2 chase, 3 attack,
//! 4 recover, 5 hold, 6 route start, 7 route walk, 8 stagger, 9 alert, 10 lost, 11 revive)
//! and a common tail (walk leaf, aim copy). Navigation, animation and target helpers stay
//! explicit as [`LowWorld`] leaves. Verification tier: 1 (see tests/char_tick_low_class_t1964.rs).

/// Original animation id tables at 0x520A48..0x520B47: sixteen tables of four weapon classes.
const ANIM: [u32; 64] = [
    0x5f9,
    0x60b,
    0x62b,
    0x61c,
    0x5fa,
    0x60c,
    0xffff_ffff,
    0x61d,
    0x5fb,
    0x60d,
    0xffff_ffff,
    0x61e,
    0x5fc,
    0x60e,
    0xffff_ffff,
    0x61f,
    0x5fd,
    0x60f,
    0xffff_ffff,
    0x620,
    0x5fe,
    0x610,
    0x62c,
    0x621,
    0x5ff,
    0x610,
    0x62c,
    0x621,
    0x600,
    0x611,
    0x62d,
    0x622,
    0x601,
    0x612,
    0x62e,
    0x623,
    0x602,
    0x613,
    0x62f,
    0x624,
    0x603,
    0x614,
    0x630,
    0x625,
    0x604,
    0x615,
    0x631,
    0x626,
    0x606,
    0x617,
    0x633,
    0x628,
    0x607,
    0x618,
    0x634,
    0x629,
    0x608,
    0x619,
    0x635,
    0x62a,
    0x60a,
    0x61b,
    0x60a,
    0x61b,
];
// Table slots by original base address (0x520A48 + 0x10 * slot).
const IDLE: usize = 0; // a48
const REST: usize = 1; // a58
const SWING_END: usize = 2; // a68
const SWING_OUT: usize = 3; // a78
const SWING: usize = 4; // a88
const TURN_A: usize = 5; // a98
const TURN_B: usize = 6; // aa8
const ATTACK_IN: usize = 7; // ab8
const ATTACK: usize = 8; // ac8
const ATTACK_OUT: usize = 9; // ad8
const RUN_IN: usize = 10; // ae8
const RUN: usize = 11; // af8
const RUN_OUT: usize = 12; // b08
const RECOVER_A: usize = 13; // b18
const RECOVER_B: usize = 14; // b28
const ALERT: usize = 15; // b38
fn anim(slot: usize, class: usize) -> u32 {
    ANIM[slot * 4 + class]
}

#[derive(Clone, Debug, PartialEq)]
pub struct LowObject {
    /// `obj+0x28`, bit 8 set means the object is its own position source.
    pub flags: u32,
    /// `obj+0x10`, low 16 bits clear means the same.
    pub word10: u32,
    pub position: [f32; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct LowTarget {
    /// Original pointer value, 0 is null.
    pub id: u32,
    pub position: [f32; 3],
    /// `target+0x150`.
    pub extension: u32,
    /// `target->character+0x24`, bit 3 sends the NPC to the notify-only exit.
    pub character_flags: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LowRecord {
    pub flags54: u32,
    pub flags58: u32,
    pub node50: i32,
    pub value28: u32,
    /// `record+0x78` callback present.
    pub callback: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LowGlobals {
    /// 0x790950.
    pub min_kind: u32,
    /// 0x7A3500.
    pub player_kind: u32,
    /// 0x7DE457.
    pub fast: bool,
    /// 0x7DE455.
    pub key_byte: u8,
    /// 0x4E8CEC.
    pub tick_rate: i32,
    /// 0x4E794C.
    pub drain_scale: f32,
    /// Pickup table value for this model slot, `None` when the entry is null.
    pub pickup: Option<f32>,
    /// Player character node (`player->0x14->0x7C+0x678`).
    pub player_node: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LowCtx {
    pub object: LowObject,
    pub target: LowTarget,
    pub kind: u32,
    pub aim_a: [u32; 2],
    pub aim_b: [u32; 2],
    pub state: u32,
    pub flags: u32,
    pub auxiliary_flags: u32,
    pub position: [f32; 3],
    pub model: u32,
    pub mounted: bool,
    pub weapon_flag: bool,
    pub node: i32,
    pub scratch_a: u32,
    pub scratch_b: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub counter: u32,
    pub hold: f32,
    pub drain: f32,
    pub speed: f32,
    pub aim_out: [u32; 2],
    pub distance_squared: f32,
    pub clock: f32,
    pub timer: f32,
    pub health: f32,
    pub threshold: f32,
    /// `ctx+0xBB0`.
    pub extra: u32,
    /// `ctx+0xACC` record present, with its `+0x18` value.
    pub route_a: Option<u32>,
    /// `*(ctx+0x67C)+0x18`.
    pub route_b: u32,
    pub record: LowRecord,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leaf {
    Navigate,
    BlockedA,
    BlockedB,
    Notify(u32),
    Walk,
    NodeIndex(i32),
    Path(i32),
    Floor(u32),
    Trigger,
    Blend,
    BlendReady,
    Obstacle,
    Route(u32),
}
impl Leaf {
    pub fn original_address(self) -> u32 {
        match self {
            Self::Navigate => 0x15D470,
            Self::BlockedA => 0x2663D0,
            Self::BlockedB => 0x2A5C30,
            Self::Notify(_) => 0x17570,
            Self::Walk => 0x15CD00,
            Self::NodeIndex(_) => 0x9CC40,
            Self::Path(_) => 0x15D500,
            Self::Floor(_) => 0x59E00,
            Self::Trigger => 0x160F80,
            Self::Blend => 0x5C1C0,
            Self::BlendReady => 0x5A020,
            Self::Obstacle => 0x15B670,
            Self::Route(_) => 0x159300,
        }
    }
}
pub trait LowWorld {
    fn awareness(&mut self, c: &mut LowCtx) -> bool;
    fn leaf(&mut self, which: Leaf, c: &mut LowCtx) -> u32;
    fn rng_raw(&mut self) -> u32;
    fn plays(&mut self, id: u32) -> bool;
    fn transition(&mut self, id: u32) -> bool;
    fn finished(&mut self) -> bool;
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    /// Original 0x15A2D0, float distance to the engagement point.
    fn distance(&mut self) -> f32;
    /// Record callback at `record+0x78`.
    fn callback(&mut self) -> Option<LowTarget>;
    /// Original 0x15A540, nearest hostile of the player.
    fn retarget(&mut self, c: &LowCtx) -> Option<LowTarget>;
    /// Original 0x25F370, returns `(out, return)`.
    fn phase_pair(&mut self, value: u32) -> (u32, u32);
}

fn switch(c: &mut LowCtx, state: u32) {
    if c.state != state {
        c.previous = c.state;
        c.state = state;
        c.state_time = 0.0;
        c.pending = state as i32;
    }
}
fn unit(w: &mut impl LowWorld) -> f64 {
    f64::from(w.rng_raw()) / 4294967296.0
}
/// `unit * scale + clock + offset` rounded to f32 at the end, as the x87 chain does.
fn hold_roll(c: &mut LowCtx, w: &mut impl LowWorld, scale: f64, offset: f64) {
    let v = unit(w) * scale;
    c.hold = ((v + f64::from(c.clock)) + offset) as f32;
}
fn play(w: &mut impl LowWorld, id: u32, blend: f32, speed: f32) {
    w.play(id, blend, speed);
}
fn speed_roll(c: &mut LowCtx, w: &mut impl LowWorld, g: &LowGlobals) {
    let pickup = f64::from(g.pickup.unwrap_or(0.0));
    let u = unit(w);
    let v = ((pickup * f64::from(1.4f32) - f64::from(0.33f32)) * u + f64::from(0.33f32)) as f32;
    c.speed = if g.fast { v } else { v * 1.25 };
}
enum Next {
    WalkTail,
    Tail,
    Return,
}
fn tail(c: &mut LowCtx, w: &mut impl LowWorld, g: &LowGlobals) {
    if c.flags & 1 != 0 {
        let pair = if g.key_byte == 10 { c.aim_a } else { c.aim_b };
        c.aim_out = pair;
        w.leaf(Leaf::Notify(0x20), c);
    }
}
fn walk_tail(c: &mut LowCtx, w: &mut impl LowWorld, g: &LowGlobals) {
    w.leaf(Leaf::Walk, c);
    tail(c, w, g);
}
/// Back to idle (state 1) and leave without the common tail.
fn rest(c: &mut LowCtx) -> Next {
    switch(c, 1);
    c.counter = 0;
    Next::Return
}

/// One tick. `g` carries the original globals read during the call.
pub fn tick(c: &mut LowCtx, w: &mut impl LowWorld, g: &LowGlobals) {
    let aware = w.awareness(c);
    if c.pending >= 0 {
        let s = c.pending as u32;
        if s != c.state {
            c.state_time = 0.0;
            c.previous = c.state;
            c.state = s;
        }
        c.pending = -1;
    }
    c.drain = if c.auxiliary_flags & 0x80 != 0 {
        (1.0 / g.tick_rate as f32) * g.drain_scale + c.drain
    } else {
        0.0
    };
    // Weapon class: 3 mounted, 1 own weapon flag, 2 fixed models, else 0.
    let class: usize = if c.mounted {
        3
    } else if c.weapon_flag {
        1
    } else if matches!(c.model, 14 | 15 | 53) {
        2
    } else {
        0
    };
    if c.record.flags54 & 0x80_0000 != 0
        && c.target.id != 0
        && c.target.extension != 0
        && c.state != 11
    {
        c.previous = c.state;
        c.state = 11;
        c.state_time = 0.0;
        c.pending = 11;
        return;
    }
    c.auxiliary_flags |= 0x400;
    if !aware && !matches!(c.state, 10 | 6 | 7 | 5) {
        c.counter = c.state;
        hold_roll(c, w, 1.0, 1.0);
        switch(c, 10);
    }
    w.leaf(Leaf::Navigate, c);
    let own =
        c.kind >= g.min_kind && (c.object.flags & 0x100 != 0 || c.object.word10 & 0xFFFF == 0);
    c.position = if own {
        c.object.position
    } else {
        c.target.position
    };
    if c.state != 6 && (w.leaf(Leaf::BlockedA, c) != 0 || w.leaf(Leaf::BlockedB, c) != 0) {
        return;
    }
    if c.target.character_flags & 8 != 0 {
        w.leaf(Leaf::Notify(0x10), c);
        return;
    }
    if c.kind == g.player_kind && c.record.callback {
        if let Some(t) = w.callback() {
            c.target = t;
        }
    }
    let next = match c.state {
        0 => {
            c.clock = 0.0;
            c.previous = 0;
            c.state = 1;
            c.state_time = 0.0;
            c.pending = 1;
            c.counter = 0;
            Next::Tail
        }
        1 => spawn_idle(c, w, g, class, aware),
        2 => run(c, w, g, class),
        3 => attack(c, w, class),
        4 => recover(c, w, class),
        5 => hold_swing(c, w, class, aware),
        6 => route_start(c, w, g),
        7 => route_walk(c, w, g, class, aware),
        8 => stagger(c, w),
        9 => alert(c, w, class),
        10 => lost(c, w, class, aware),
        11 => revive(c, w, g, class),
        _ => Next::Tail,
    };
    match next {
        Next::WalkTail => walk_tail(c, w, g),
        Next::Tail => tail(c, w, g),
        Next::Return => {}
    }
}

/// State 1: idle/turn. Picks the next behaviour from distance and a six-way roll.
fn spawn_idle(
    c: &mut LowCtx,
    w: &mut impl LowWorld,
    g: &LowGlobals,
    e: usize,
    aware: bool,
) -> Next {
    let (idle, turn_a, turn_b) = (anim(IDLE, e), anim(TURN_A, e), anim(TURN_B, e));
    if !(w.plays(idle) || w.plays(turn_a) || w.plays(turn_b)) {
        play(w, turn_a, 0.2, 1.0);
        c.counter = 0;
        if e == 1 {
            c.timer = 0.0;
            speed_roll(c, w, g);
        }
        return Next::WalkTail;
    }
    if e == 1 && c.timer > c.speed && w.plays(0x610) {
        play(w, 0x610, 0.0, 1.0);
        c.counter += 1;
        c.timer = 0.0;
        if c.counter >= 6 {
            c.speed = f32::MAX;
        } else {
            speed_roll(c, w, g);
        }
        return Next::WalkTail;
    }
    if !w.finished() {
        return Next::WalkTail;
    }
    let (out, ret) = w.phase_pair(c.record.value28);
    if aware {
        let pickup = f64::from(g.pickup.unwrap_or(0.0));
        let roll = f64::from(c.threshold) - unit(w);
        if roll > pickup && c.counter < 6 {
            if matches!(c.model, 0x1d | 0x1e) && (w.plays(turn_a) || w.plays(turn_b)) {
                c.clock = 0.0;
                c.extra = 0;
            }
            let d = w.distance();
            if c.clock > d && w.leaf(Leaf::Obstacle, c) != 0 {
                let id = if w.rng_raw() & 1 == 0 { turn_a } else { turn_b };
                play(w, id, 0.0, 1.0);
                if e == 1 {
                    c.counter += 1;
                    c.timer = 0.0;
                    speed_roll(c, w, g);
                }
                return Next::Tail;
            }
        }
    }
    if !w.plays(idle) {
        play(w, idle, 0.33, 1.0);
        c.speed = 100.0;
        return Next::WalkTail;
    }
    if 4.0 > c.distance_squared {
        hold_roll(c, w, 1.0, 1.0);
        switch(c, 9);
        return Next::Tail;
    }
    let flags58 = c.record.flags58;
    if flags58 & 2 != 0
        && c.auxiliary_flags & 0x2000 == 0
        && c.health > 2.0
        && out == 1
        && c.record.node50 != -1
    {
        switch(c, 6);
        return Next::Tail;
    }
    if flags58 & 4 != 0
        && 900.0 > c.distance_squared
        && c.distance_squared > 16.0
        && out > 1
        && (out.wrapping_sub(ret) == 1 || ret == 1)
    {
        w.leaf(Leaf::Notify(0x200_0000), c);
        return Next::Tail;
    }
    if 36.0 > c.distance_squared {
        hold_roll(c, w, 1.0, 1.0);
        switch(c, 3);
        return Next::Tail;
    }
    let roll = (unit(w) * 6.0 + 0.5) as i32;
    if c.distance_squared > 256.0 && roll > 1 {
        switch(c, 2);
        return Next::Tail;
    }
    match roll {
        0 | 1 => {
            if c.state == 2 {
                return Next::WalkTail;
            }
            switch(c, 2);
        }
        2..=4 => {
            if c.state == 4 {
                return Next::WalkTail;
            }
            switch(c, 4);
        }
        5 if e == 2 => {
            if c.state == 4 {
                return Next::WalkTail;
            }
            switch(c, 4);
        }
        5 => {
            hold_roll(c, w, 2.5, 2.5);
            if c.state == 5 {
                return Next::WalkTail;
            }
            switch(c, 5);
        }
        _ => return Next::WalkTail,
    }
    c.state_time = 0.0;
    Next::WalkTail
}

fn run(c: &mut LowCtx, w: &mut impl LowWorld, _g: &LowGlobals, e: usize) -> Next {
    let (run_in, run, run_out) = (anim(RUN_IN, e), anim(RUN, e), anim(RUN_OUT, e));
    let go_idle = if !(w.plays(run_in) || w.plays(run) || w.plays(run_out)) {
        if 36.0 > c.distance_squared {
            play(w, anim(IDLE, e), 0.0, 1.0);
            return rest(c);
        }
        play(w, run_in, 0.0, 1.0);
        false
    } else if w.transition(run_in) {
        play(w, run, 0.0, 1.1);
        false
    } else {
        w.transition(run_out)
    };
    if go_idle {
        return rest(c);
    }
    if !w.transition(run) {
        return Next::WalkTail;
    }
    if !(36.0 > c.distance_squared) && w.rng_raw() % 3 != 1 {
        return Next::WalkTail;
    }
    play(w, run_out, 0.0, 1.0);
    Next::WalkTail
}

fn attack(c: &mut LowCtx, w: &mut impl LowWorld, e: usize) -> Next {
    let (a_in, a, a_out) = (anim(ATTACK_IN, e), anim(ATTACK, e), anim(ATTACK_OUT, e));
    if !(w.plays(a_in) || w.plays(a) || w.plays(a_out)) {
        play(w, a_out, 0.33, 1.0);
    } else {
        let go_idle = if w.transition(a_out) {
            play(w, a_in, 0.0, 1.1);
            false
        } else {
            w.transition(a)
        };
        if go_idle {
            return rest(c);
        }
    }
    if w.transition(a_in) && c.clock > c.hold {
        play(w, a, 0.0, 1.0);
    }
    Next::WalkTail
}

fn recover(c: &mut LowCtx, w: &mut impl LowWorld, e: usize) -> Next {
    let (a, b) = (anim(RECOVER_A, e), anim(RECOVER_B, e));
    if w.plays(a) || w.plays(b) {
        if !w.finished() {
            return Next::WalkTail;
        }
        return rest(c);
    }
    let id = if w.rng_raw() & 1 == 0 { a } else { b };
    play(w, id, 0.2, 1.0);
    Next::WalkTail
}

fn hold_swing(c: &mut LowCtx, w: &mut impl LowWorld, e: usize, aware: bool) -> Next {
    let (swing, out, rest_id, end) = (
        anim(SWING, e),
        anim(SWING_OUT, e),
        anim(REST, e),
        anim(SWING_END, e),
    );
    if (w.transition(swing) || w.transition(out)) && c.clock > c.hold {
        play(w, rest_id, 0.0, 1.0);
    }
    if !w.plays(swing)
        && !(w.plays(end) || w.plays(rest_id) || w.plays(out) || w.leaf(Leaf::Floor(0), c) != 0)
    {
        play(w, end, 0.33, 1.0);
        return Next::WalkTail;
    }
    // Swing finished, blocked or idle: decide between resting and a new swing.
    let swung = if w.transition(end) || w.transition(out) {
        true
    } else {
        w.finished() && w.leaf(Leaf::Floor(0), c) != 0
    };
    if !swung {
        return if w.transition(rest_id) {
            rest(c)
        } else {
            Next::WalkTail
        };
    }
    if aware {
        let roll = f64::from(1.2f32) - unit(w);
        if f64::from(c.threshold) > roll {
            let d = w.distance();
            if c.clock > d && w.leaf(Leaf::Obstacle, c) != 0 {
                play(w, out, 0.0, 1.0);
                return Next::WalkTail;
            }
        }
    }
    play(w, swing, 0.0, 1.0);
    Next::WalkTail
}

fn route_start(c: &mut LowCtx, w: &mut impl LowWorld, g: &LowGlobals) -> Next {
    let mut node = w.leaf(Leaf::NodeIndex(c.record.node50), c) as i32;
    node = if node < 0 {
        g.player_node
    } else {
        w.leaf(Leaf::NodeIndex(c.record.node50), c) as i32
    };
    c.node = node;
    c.scratch_a = 0;
    c.scratch_b = 0;
    if w.leaf(Leaf::Path(node), c) != 0 {
        c.auxiliary_flags |= 0x2000;
        switch(c, 7);
        return Next::Tail;
    }
    w.leaf(Leaf::Walk, c);
    c.flags &= 0xFFF9_FFFF;
    w.leaf(Leaf::Blend, c);
    if !w.plays(0xDE) && !w.plays(0xDD) {
        play(w, 0xDE, 0.33, 1.0);
        return Next::Tail;
    }
    if w.plays(0xDE) && w.leaf(Leaf::BlendReady, c) != 0 {
        play(w, 0xDD, 0.33, 1.0);
    }
    Next::Tail
}

fn route_walk(
    c: &mut LowCtx,
    w: &mut impl LowWorld,
    _g: &LowGlobals,
    e: usize,
    aware: bool,
) -> Next {
    let swing = anim(SWING, e);
    if w.transition(swing) {
        play(w, swing, 0.0, 1.0);
    }
    if aware {
        hold_roll(c, w, 10.0, 10.0);
        switch(c, 5);
        return Next::Tail;
    }
    route_call(c, w);
    Next::Tail
}
fn route_call(c: &mut LowCtx, w: &mut impl LowWorld) {
    let value = c.route_a.unwrap_or(c.route_b);
    w.leaf(Leaf::Route(value), c);
}

fn stagger(c: &mut LowCtx, w: &mut impl LowWorld) -> Next {
    let _ = c;
    if !w.plays(0xDF) && !w.plays(0xE0) {
        play(w, 0xDF, 0.33, 1.0);
    } else if w.plays(0xDF) && w.finished() {
        play(w, 0xE0, 0.33, 1.0);
    }
    Next::Tail
}

fn alert(c: &mut LowCtx, w: &mut impl LowWorld, e: usize) -> Next {
    let id = anim(ALERT, e);
    if !w.plays(id) {
        play(w, id, 0.33, 1.0);
    }
    w.leaf(Leaf::Trigger, c);
    if c.clock > c.hold {
        play(w, anim(ATTACK_IN, e), 0.33, 1.0);
        switch(c, 3);
    }
    if c.distance_squared > 4.0 && c.state != 0 {
        switch(c, 0);
    }
    Next::Tail
}

fn lost(c: &mut LowCtx, w: &mut impl LowWorld, e: usize, aware: bool) -> Next {
    if aware {
        if c.counter == 10 {
            return Next::Tail;
        }
        // Resume the state saved in `counter` when awareness was lost.
        c.previous = 10;
        c.state = c.counter;
        c.state_time = 0.0;
        c.pending = c.counter as i32;
        return Next::Tail;
    }
    if c.clock > c.hold {
        w.leaf(Leaf::Notify(8), c);
    }
    let idle = anim(IDLE, e);
    if !w.plays(idle) {
        play(w, idle, 0.33, 1.0);
    }
    Next::WalkTail
}

fn revive(c: &mut LowCtx, w: &mut impl LowWorld, g: &LowGlobals, e: usize) -> Next {
    let idle = anim(IDLE, e);
    if !w.plays(idle) {
        play(w, idle, 0.33, 1.0);
    }
    if c.kind == g.player_kind {
        if let Some(t) = w.retarget(c) {
            c.target = t;
            if c.target.extension == 0 {
                switch(c, 1);
                c.counter = 0;
            }
        } else {
            c.target = LowTarget {
                id: 0,
                position: [0.0; 3],
                extension: 0,
                character_flags: 0,
            };
        }
    }
    route_call(c, w);
    Next::Tail
}
