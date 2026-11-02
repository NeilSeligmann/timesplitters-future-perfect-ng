//! Route step executor of the NPC tick, original 0x26E150
//! (`game_actor_route_step_executor_dispatch_by_step_kind_and_start_weapon_anims`).
//!
//! Original ABI: EAX = object, EBX = step array (12 byte entries), the step kind is
//! `steps[object.ctx.0xAA0].kind`. Object context fields used: +0x28 flags, +0xAA0 step index,
//! +0x204 weapon slot, +0x678 route node, +0x9DC node handle. Verification tier 1 (see tests).
//!
//! Step kinds: 1 start, 2/3/16 approach a route, 4 turn to the route node, 5..9 and 17 locomotion
//! (turn left/right, walk) with settle logic, 12 jump back to step 1. Others do nothing.

/// Animation id pairs `[weapon-free, with-weapon-group]` of the original selector tables.
const IDLE: [u32; 2] = [2741, 2742]; // 0x520498
const START: [u32; 2] = [2737, 2739]; // 0x5202C8
const MOVE: [u32; 2] = [2755, 2756]; // 0x5202B8
const SETTLE: [u32; 2] = [2738, 2740]; // 0x5202D8
const TURN_LEFT: [u32; 2] = [2731, 2734]; // 0x520318
const TURN_FWD: [u32; 2] = [2732, 2735]; // 0x520328
const TURN_RIGHT: [u32; 2] = [2733, 2736]; // 0x520338

#[derive(Clone, Debug, PartialEq)]
pub struct RouteStepCtx {
    /// Context +0x28 flags (bit 0x100000 = hold idle).
    pub flags: u32,
    /// Context +0xAA0 step index.
    pub step_index: u32,
    /// Kind of the current step (`steps[step_index * 3]`).
    pub step_kind: u32,
    /// Context +0x204 weapon record slot, -1 = none.
    pub weapon_slot: i32,
    /// Weapon record flags (`records[slot * 0x29C + 0xC]`), only read when slot != -1.
    pub weapon_flags: u32,
    /// Context +0x9DC node handle.
    pub node: u32,
    /// Word +0x18 of node table entry `route_node` (turn target).
    pub route_turn_word: u32,
}

/// Record returned by 0x26D690: word 0 kind, word 2 payload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepRecord {
    pub kind: u32,
    pub word8: u32,
}

/// Calls leaving the executor, each annotated with the original address.
pub trait RouteStepWorld {
    /// 0x59D20 animation `id` is playing.
    fn plays(&mut self, id: u32) -> bool;
    /// 0x5F460 start animation `id` with blend `blend` and speed 1.0.
    fn play(&mut self, id: u32, blend: f32);
    /// 0x59DD0 current clip finished.
    fn finished(&mut self) -> bool;
    /// 0x26CBE0 (ECX = steps) step action ready.
    fn action_ready(&mut self) -> bool;
    /// 0x26CB30 (ECX = steps) reset step action.
    fn action_reset(&mut self);
    /// 0x26CC80 route busy.
    fn route_busy(&mut self) -> bool;
    /// 0x26D2F0 (EDX = kind, ESI = object) step kind pending.
    fn kind_pending(&mut self, kind: u32) -> bool;
    /// 0x26DFB0 (EAX = kind) apply step kind.
    fn kind_apply(&mut self, kind: u32);
    /// 0x26D690 current step record.
    fn record(&mut self) -> StepRecord;
    /// 0x26CFF0 (ECX = object, EAX = word) launch route record.
    fn launch(&mut self, word: u32);
    /// 0x9A4C0 key of a node handle.
    fn node_key(&mut self, node: u32) -> u32;
    /// 0x26CD50 stop condition.
    fn stop_check(&mut self) -> bool;
    /// 0x159300 turn the object toward `word`, rate limited, with parameter 0.
    fn turn_toward(&mut self, word: u32);
    /// 0x26DE30 leave the step.
    fn leave(&mut self);
    /// 0x26E100 step check, 1 = continue locomotion.
    fn step_check(&mut self) -> u32;
    /// 0x9A420 node record of `node` (its position is at +0xC).
    fn node_record(&mut self, node: u32) -> u32;
    /// 0x2661F0 signed bearing delta from the object position to the node record position.
    fn facing_delta(&mut self, node_record: u32) -> f32;
}

fn pick(c: &RouteStepCtx, ids: [u32; 2]) -> u32 {
    if c.weapon_slot == -1 || c.weapon_flags & 1 != 0 {
        ids[1]
    } else {
        ids[0]
    }
}

fn play(c: &RouteStepCtx, w: &mut impl RouteStepWorld, ids: [u32; 2]) {
    w.play(pick(c, ids), 0.0);
}

pub fn tick(c: &mut RouteStepCtx, w: &mut impl RouteStepWorld) {
    let kind = c.step_kind;
    if c.flags & 0x100000 != 0 {
        let id = pick(c, IDLE);
        if !w.plays(id) {
            w.play(id, f32::from_bits(0x3EA8_F5C3));
        }
        return;
    }
    match kind {
        1 => start(c, w),
        2 | 3 | 16 => approach(c, w, kind),
        4 => {
            if !w.action_ready() || w.finished() {
                w.action_reset();
            }
            w.turn_toward(c.route_turn_word);
        }
        5..=9 | 17 => locomotion(c, w, kind),
        12 => c.step_index = 1,
        _ => {}
    }
}

fn start(c: &RouteStepCtx, w: &mut impl RouteStepWorld) {
    let id = pick(c, START);
    if w.plays(id) {
        if w.finished() {
            play(c, w, MOVE);
        }
    } else if !w.plays(pick(c, MOVE)) {
        play(c, w, MOVE);
    }
}

fn approach(c: &RouteStepCtx, w: &mut impl RouteStepWorld, kind: u32) {
    if !w.route_busy() && w.kind_pending(kind) {
        w.kind_apply(kind);
        return;
    }
    if !w.route_busy() {
        return;
    }
    let record = w.record();
    if kind == 16 {
        let key = w.node_key(c.node);
        w.launch(key);
    } else {
        w.launch(record.word8);
    }
}

/// Original label 0x26E918: reset the step action.
fn finish(w: &mut impl RouteStepWorld) {
    w.action_reset();
}

fn locomotion(c: &RouteStepCtx, w: &mut impl RouteStepWorld, kind: u32) {
    let busy =
        w.plays(pick(c, SETTLE)) || w.plays(pick(c, START)) || w.action_ready() || w.stop_check();
    if !busy {
        if w.plays(pick(c, MOVE)) {
            if w.kind_pending(kind) {
                play(c, w, SETTLE);
            }
        } else {
            finish(w);
        }
        return;
    }
    // 0x26E596: a finished settle clip ends the step.
    if w.plays(pick(c, SETTLE)) && w.finished() {
        finish(w);
        return;
    }
    if w.action_ready() && w.finished() {
        match kind {
            5 => play(c, w, TURN_LEFT),
            6 => play(c, w, TURN_FWD),
            7 => play(c, w, TURN_RIGHT),
            8 => {
                if w.step_check() == 1 {
                    play(c, w, START);
                } else {
                    w.leave();
                    finish(w);
                }
            }
            17 => {
                let record = w.node_record(c.node);
                let delta = w.facing_delta(record);
                if 45.0 > delta.abs() {
                    play(c, w, START);
                } else if delta > 0.0 {
                    play(c, w, TURN_FWD);
                } else {
                    play(c, w, TURN_RIGHT);
                }
            }
            _ => {}
        }
        return;
    }
    // 0x26E8D8
    if !w.stop_check() {
        return;
    }
    let record = w.record();
    if record.kind == 1 {
        if w.finished() {
            play(c, w, START);
        } else {
            w.launch(record.word8);
        }
        return;
    }
    if !w.finished() {
        return;
    }
    w.leave();
    finish(w);
}
