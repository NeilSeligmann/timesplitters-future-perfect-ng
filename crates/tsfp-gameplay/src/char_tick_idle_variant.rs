//! Readable idle-variant NPC class tick, original `0x283730` (called from the bit 0x4 branch of
//! `0x27F8E0`). INFERRED role: a seven-state ambient idle/reaction controller whose animation
//! set is picked by model variant (`0x34` -> 0, `0x62` -> 2, other -> 1) from the three
//! eight-word clip rows at `0x520838 + 32*variant`. Verification tier 1 (see T1964 tests).
//! States: 0 spawn, 1 idle wait, 2 alert idle, 3 reaction, 4 inert, 5 special clip 0x99D,
//! 6 unaware/hold. Geometry and animation leaves are explicit [`IdleVariantWorld`] calls.

/// Per-actor controller context (original `obj+0x7C`, the object pointed to by `ctx+4`).
#[derive(Clone, Debug, PartialEq)]
pub struct IdleCtx {
    pub state: u32,
    /// `+0x9D0`, negative means no pending request.
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    /// `+0x9DC`, state to restore once awareness returns.
    pub saved_state: u32,
    /// `+0x9E0`, clock value at which the idle/hold timers expire.
    pub wake_time: f32,
    /// `+0xB30`.
    pub clock: f32,
    /// `+0xA90`.
    pub sway: f32,
    /// `+0x2C`.
    pub flags: u32,
    /// `+0x24`, bit 8 hands control to the world (call `0x17570` code 0x10).
    pub status_flags: u32,
    /// `+0x48`.
    pub model: u16,
    /// `+0xAF4`.
    pub distance_squared: f32,
    /// `+0xBF0 -> +0x58`.
    pub record_flags: u32,
    /// Weapon slot flags (`weapons[ctx+0x204].flags`).
    pub weapon_flags: u32,
    /// `+8`.
    pub priority: i32,
    /// `+0x30..0x38`.
    pub position: [f32; 3],
    /// Optional `ctx+0` anchor, used when priority is high enough.
    pub anchor: Option<IdleAnchor>,
    /// Position of the owner object (`ctx+4`).
    pub object_position: [f32; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct IdleAnchor {
    pub flags: u32,
    pub mode: u32,
    pub position: [f32; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct IdleGlobals {
    /// Clip rows `0x520838 + 32*variant`, eight ids each.
    pub clips: [[u32; 8]; 3],
    pub tick_rate: i32,
    pub time_scale: f32,
    pub priority_floor: i32,
}
pub trait IdleVariantWorld {
    /// `0x15E120(ctx, 1)`.
    fn awareness(&mut self, ctx: &mut IdleCtx) -> bool;
    /// `0x15D470`.
    fn navigate(&mut self, ctx: &mut IdleCtx);
    /// `0x59D20`.
    fn plays(&mut self, id: u32) -> bool;
    /// `0x59DB0`.
    fn transition(&mut self, id: u32) -> bool;
    /// `0x59DD0`.
    fn finished(&mut self) -> bool;
    /// `0x5F460(obj, id, blend, speed)`.
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    /// `0x153B40`.
    fn rng_raw(&mut self) -> u32;
    /// `0x15CD00`.
    fn walk(&mut self, ctx: &mut IdleCtx);
    /// `0x17570(ctx, code, 0)`.
    fn request(&mut self, ctx: &mut IdleCtx, code: u32);
}
pub const WIDE_RANGE: f32 = 289.0;
pub const NEAR_RANGE: f32 = 100.0;
const ALT_RECORD: u32 = 0x80_0000;

/// Every transition resets the state clock and republishes the pending request.
fn enter(c: &mut IdleCtx, state: u32) {
    c.previous = c.state;
    c.state = state;
    c.state_time = 0.0;
    c.pending = state as i32;
}
/// Switch unless already there, returns whether it switched.
fn goto_state(c: &mut IdleCtx, next: u32) -> bool {
    if c.state == next {
        return false;
    }
    enter(c, next);
    true
}
fn unit(raw: u32) -> f64 {
    f64::from(raw) * (1.0 / 4294967296.0)
}
/// Wake timer `unit * span + clock + span` (x87 extended precision in the original).
fn timer(c: &IdleCtx, raw: u32, span: f64) -> f32 {
    (unit(raw) * span + f64::from(c.clock) + span) as f32
}
fn variant(model: u16) -> usize {
    if model == 0x34 {
        0
    } else {
        usize::from(model == 0x62) + 1
    }
}
fn alert_or_react(c: &IdleCtx) -> u32 {
    if c.record_flags & ALT_RECORD != 0 {
        2
    } else {
        3
    }
}

pub fn tick<W: IdleVariantWorld>(c: &mut IdleCtx, g: &IdleGlobals, w: &mut W) {
    let aware = w.awareness(c);
    if c.pending >= 0 {
        let p = c.pending as u32;
        if p != c.state {
            c.state_time = 0.0;
            c.previous = c.state;
            c.state = p;
        }
        c.pending = -1;
    }
    if c.flags & 0x80 != 0 {
        c.sway += 1.0 / g.tick_rate as f32 * g.time_scale;
    } else {
        c.sway = 0.0;
    }
    c.flags |= 0x400;
    let v = variant(c.model);
    if !aware && c.state != 6 {
        c.saved_state = c.state;
        let raw = w.rng_raw();
        c.wake_time = (unit(raw) + f64::from(c.clock) + 1.0) as f32;
        enter(c, 6);
    }
    w.navigate(c);
    match &c.anchor {
        Some(a)
            if c.priority >= g.priority_floor && (a.flags & 0x100 != 0 || a.mode & 0xFFFF == 0) =>
        {
            c.position = a.position;
        }
        _ => c.position = c.object_position,
    }
    if c.status_flags & 8 != 0 {
        w.request(c, 0x10);
        return;
    }
    let row = g.clips[v];
    match c.state {
        0 => {
            c.clock = 0.0;
            let next = if c.distance_squared < WIDE_RANGE {
                2
            } else {
                alert_or_react(c)
            };
            enter(c, next);
        }
        1 => state_idle(c, w, &row, v),
        2 => state_alert(c, w, &row, v),
        3 => state_react(c, g, w, v),
        5 => {
            if !w.plays(0x99D) {
                w.play(0x99D, 0.33, 1.0);
            }
            if c.clock > c.wake_time {
                let raw = w.rng_raw();
                c.wake_time = timer(c, raw, 0.25);
                goto_state(c, 1);
            }
        }
        6 => state_hold(c, w, &row, aware),
        _ => {}
    }
}
/// State 1: hold the idle clip, then pick alert (2) or reaction (3) once the timer expires.
fn state_idle<W: IdleVariantWorld>(c: &mut IdleCtx, w: &mut W, row: &[u32; 8], v: usize) {
    if !(w.plays(row[0]) || w.plays(row[5]) || w.plays(row[6])) {
        w.play(row[0], 0.33, 1.0);
    } else if v == 0 && w.finished() {
        let id = if w.rng_raw() & 1 != 0 { row[6] } else { row[5] };
        w.play(id, 0.0, 1.0);
    }
    if c.clock > c.wake_time {
        let near = c.distance_squared < NEAR_RANGE
            || (w.rng_raw() % 3 == 1 && c.distance_squared < WIDE_RANGE)
            || c.record_flags & ALT_RECORD != 0;
        goto_state(c, if near { 2 } else { 3 });
    } else {
        w.walk(c);
    }
}
/// State 2: alert clip, back to idle (1) when it finishes.
fn state_alert<W: IdleVariantWorld>(c: &mut IdleCtx, w: &mut W, row: &[u32; 8], v: usize) {
    if !w.plays(row[7]) {
        w.play(row[7], 0.2, 1.0);
    } else if w.finished() {
        let span = if v == 2 { 0.75 } else { 0.25 };
        let raw = w.rng_raw();
        c.wake_time = timer(c, raw, span);
        if goto_state(c, 1) {
            w.walk(c);
        }
        return;
    }
    w.walk(c);
}
/// State 3: reaction/turn clip selection, drops to idle (1) after a random delay.
fn state_react<W: IdleVariantWorld>(c: &mut IdleCtx, g: &IdleGlobals, w: &mut W, v: usize) {
    let row = g.clips[v];
    let row0 = g.clips[0];
    let any = [4usize, 2, 3, 1, 5, 6].iter().any(|&k| w.plays(row[k]));
    if !any {
        w.play(row[4], 0.2, 1.0);
    } else if w.transition(row[4]) {
        if v == 0 && c.weapon_flags & 1 != 0 {
            w.play(row0[3], 0.0, 1.0);
        } else {
            w.play(row[2], 0.0, 1.0);
        }
    } else if w.transition(row[1]) {
        let draw = w.rng_raw();
        if unit(draw) > 0.5 {
            let raw = w.rng_raw();
            c.wake_time = timer(c, raw, 0.125);
            goto_state(c, 1);
            return;
        }
    }
    if v == 0 {
        if w.transition(row0[2]) {
            let k = w.rng_raw() % 5;
            let id = if k == 4 {
                row0[2]
            } else if k > 2 {
                row0[5]
            } else {
                row0[6]
            };
            w.play(id, 0.0, 1.0);
        } else if w.transition(row0[5]) || w.transition(row0[6]) {
            w.play(row0[2], 0.0, 1.0);
        }
    }
    if (w.transition(row[2]) || w.transition(row[3]))
        && (c.distance_squared < NEAR_RANGE || w.rng_raw() % 5 == 1)
    {
        w.play(row[1], 0.0, 1.0);
    }
    w.walk(c);
}
/// State 6: unaware hold, restores the saved state once awareness returns.
fn state_hold<W: IdleVariantWorld>(c: &mut IdleCtx, w: &mut W, row: &[u32; 8], aware: bool) {
    if aware {
        let saved = c.saved_state;
        goto_state(c, saved);
        return;
    }
    if c.record_flags & ALT_RECORD == 0 && c.clock > c.wake_time {
        w.request(c, 8);
    }
    if !w.plays(row[0]) {
        w.play(row[0], 0.33, 1.0);
    }
    w.walk(c);
}
