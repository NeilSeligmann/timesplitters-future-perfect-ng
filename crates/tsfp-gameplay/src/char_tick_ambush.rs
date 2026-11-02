//! Eleven-state ambush class tick0x2A29B0. Shares explicit cover leaves/state.
use crate::char_tick_cover::{CoverCtx, CoverLeaf as L, CoverWorld, common, enter, unit};
const IDS: [[u32; 2]; 10] = [
    [0x5c1, 0x5b4],
    [0x5c7, 0x5ba],
    [0x5c2, 0x5b5],
    [0x5c6, 0x5b9],
    [0x5c3, 0x5b6],
    [0x5e3, 0x5cf],
    [0x5e4, 0x5d0],
    [0x5e5, 0x5d1],
    [0x5c4, 0x5b7],
    [0x5f4, 0x5f3],
];
fn id(c: &CoverCtx, n: usize) -> u32 {
    IDS[n][usize::from(c.posture != 1)]
}
fn exit_tail(c: &mut CoverCtx, w: &mut impl CoverWorld, angle: f32) {
    if c.state_time <= 0.5 || angle.abs() >= 70.0 {
        c.flags &= !0x60000;
        w.leaf(L::Blend(1), c);
    } else {
        let old = c.flags;
        c.flags = old | 2;
        if c.clip_policy & 0x100 != 0 {
            c.flags = (old & !0x40000) | 2;
            if c.slot_flags & 1 != 0 {
                w.leaf(L::Blend(2), c);
            }
        }
    }
    w.leaf(L::Walk, c);
}
fn sustain(c: &mut CoverCtx, w: &mut impl CoverWorld, aware: bool) {
    if ![0, 4, 5, 6, 7, 8].into_iter().any(|n| w.plays(id(c, n))) {
        w.play(id(c, 5), f32::from_bits(0x3EAA7EFA), 1.0);
    }
    if w.transition(id(c, 0)) {
        w.play(id(c, 4), 0.0, 1.0);
    } else if w.transition(id(c, 4)) {
        w.play(id(c, 5), 0.0, 1.0);
    } else if w.transition(id(c, 6)) || w.transition(id(c, 7)) {
        if !w.plays(id(c, 8)) {
            w.play(id(c, 5), 0.0, 1.0);
        }
    } else if w.transition(id(c, 8)) {
        w.play(id(c, 1), 0.0, 1.0);
        enter(c, 2);
        c.auxiliary_flags &= !0x400;
        return;
    }
    if !w.plays(id(c, 8)) {
        if [5, 6, 7].into_iter().any(|n| w.plays(id(c, n))) {
            c.auxiliary_flags |= 0x400;
            if !c.selected {
                w.leaf(L::Assess(false), c);
            }
            let active = if !aware {
                c.unseen < 2.0
            } else {
                c.unseen = 0.0;
                true
            };
            if !w.wants_exit(c) {
                if w.attack_ready(c, active) {
                    let armed = c.weapon_flags & 1 != 0;
                    w.play(id(c, if armed { 6 } else { 7 }), 0.0, 1.0);
                    c.action_deadline = (unit(w.rng_raw()) * 0.5
                        + if armed {
                            f64::from(f32::from_bits(0x3E99999A))
                        } else {
                            1.5
                        }
                        + f64::from(c.clock)) as f32;
                }
            } else {
                let blend = if !w.plays(id(c, 6)) && !w.plays(id(c, 7)) {
                    0.0
                } else {
                    f32::from_bits(0x3EAA7EFA)
                };
                w.play(id(c, 8), blend, 1.0);
            }
        }
    } else {
        w.leaf(L::Align(0.01), c);
    }
    if w.plays(id(c, 4)) {
        w.leaf(L::CoverAction(12.0), c);
        return;
    }
    if w.plays(id(c, 8)) {
        w.leaf(L::FinishAction, c);
        return;
    }
    if ![4, 5, 6, 7].into_iter().any(|n| w.plays(id(c, n))) {
        w.leaf(L::Face, c);
    }
}
pub fn tick(c: &mut CoverCtx, w: &mut impl CoverWorld) {
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
        c.drain + (1.0 / c.tick_rate as f32) * c.delta_ticks
    } else {
        0.0
    };
    w.leaf(L::Navigate, c);
    if c.selected && c.reservation.is_some() {
        if aware {
            w.leaf(L::Alert, c);
        }
        if c.selected && c.callback_present {
            let t = w.leaf(L::Callback, c);
            if t != 0 {
                c.target = t;
            }
        }
        if c.target == 0 {
            w.class(0x200);
            return;
        }
        w.leaf(L::Assess(true), c);
        if w.leaf(L::BreakCover, c) != 0 {
            enter(c, 10);
        }
    }
    match c.state {
        0 => {
            if c.record_mask & 0x20000 != 0 && c.reservation.is_none() {
                c.reservation = w.find_cover(c, c.record_mask & 0x800000 == 0);
                let n = c
                    .reservation
                    .clone()
                    .expect("original ambush assumes reservation resolved");
                w.resolve_node(n.tag);
                c.node_flags |= 0x40000000;
                c.destination = n.position;
            }
            c.eligible = w.leaf(L::Eligible, c) != 0;
            c.saved_reservation = c.reservation.clone();
            if !w.plays(0x5a3) {
                w.play(0x5a3, f32::from_bits(0x3EAA7EFA), 1.0);
            }
            c.destination = c
                .reservation
                .as_ref()
                .expect("ambush requires reservation")
                .position;
            c.flags |= 0x20000;
            enter(c, 1);
            w.leaf(L::Walk, c);
        }
        1 => {
            let p = c.reservation.as_ref().unwrap().position;
            let dx = c.position[0] - p[0];
            let dz = c.position[2] - p[2];
            if dx * dx + dz * dz > 0.1 {
                w.leaf(L::Align(0.05), c);
            } else {
                c.previous = 1;
                c.state = 2;
                c.pending = 2;
                c.state_time = 0.0;
            }
            w.leaf(L::Face, c);
        }
        2 => {
            if w.finished() {
                if !c.selected {
                    w.leaf(L::Assess(true), c);
                }
                let decision = w.leaf(L::AmbushDecision, c);
                if (decision as i32) > 5 {
                    let tag = c.reservation.as_ref().unwrap().tag;
                    let n = w.resolve_node(tag);
                    c.node = if n < 0 { c.player_node } else { n };
                    c.walk_timer = 0;
                    c.walk_progress = 0;
                    w.leaf(L::MarkNode(n), c);
                    c.node_flags |= 0x40000000;
                }
                c.clock = (unit(w.rng_raw()) * 0.5) as f32;
                match decision {
                    0 => {
                        w.play(0x5a3, 0.0, 1.0);
                        enter(c, 3);
                    }
                    1 => {
                        w.play(0x5aa, 0.0, 1.0);
                        enter(c, 4);
                    }
                    2 => {
                        w.play(0x5a4, 0.0, 1.0);
                        enter(c, 6);
                    }
                    3 => {
                        w.play(0x5ad, 0.0, 1.0);
                        c.auxiliary_flags &= !0x20000;
                        c.clock = 0.0;
                        enter(c, 5);
                    }
                    4 | 5 => {
                        c.posture = w.leaf(L::Posture(decision, true), c) as i32;
                        w.play(id(c, 0), 0.0, 1.0);
                        c.unseen = 0.0;
                        enter(c, if decision == 4 { 7 } else { 8 });
                    }
                    6 | 7 => {
                        let col = usize::from(c.posture < 1);
                        w.play(IDS[0][col], 0.0, 1.0);
                        enter(c, 9);
                    }
                    _ => w.class(4),
                }
            }
            common(c, w, Some(0.01), true);
        }
        3 => {
            if c.clock >= 1.5 {
                c.previous = 3;
                c.state = 2;
                c.state_time = 0.0;
                c.pending = 2;
            }
            if !w.plays(0x5a3) {
                w.play(0x5a3, 0.0, 1.0);
            }
            common(c, w, Some(0.01), true);
        }
        4 => {
            if !w.plays(0x5aa) && !w.plays(0x5a9) {
                w.play(0x5a9, f32::from_bits(0x3EAA7EFA), 1.0);
            } else if w.transition(0x5aa) {
                w.play(0x5a9, 0.0, 1.0);
            }
            if c.clock > 8.0 {
                w.play(0x5a8, 0.0, 1.0);
                enter(c, 2);
            }
            common(c, w, Some(0.01), true);
        }
        5 => {
            if w.finished() && !w.plays(0x5ac) {
                w.play(0x5ac, f32::from_bits(0x3EAA7EFA), 1.0);
            }
            if c.clock > 6.0 || (c.auxiliary_flags & 0x20000 != 0 && c.clock > 1.0) {
                w.play(0x5ab, 0.0, 1.0);
                c.auxiliary_flags &= !0x20000;
                enter(c, 2);
            }
            common(c, w, Some(0.01), true);
        }
        6 => {
            let mut blend = None;
            if !w.plays(0x5a4) && !w.plays(0x5a5) && !w.plays(0x5a6) {
                blend = Some(f32::from_bits(0x3EAA7EFA));
            } else if w.transition(0x5a4) {
                blend = Some(0.0);
            } else if w.transition(0x5a5) || w.transition(0x5a6) {
                c.auxiliary_flags &= !0x400;
                w.play(0x5a7, 0.0, 1.0);
                enter(c, 2);
            }
            if let Some(b) = blend {
                w.play(
                    if c.weapon_flags & 1 != 0 {
                        0x5a5
                    } else {
                        0x5a6
                    },
                    b,
                    1.0,
                );
            }
            if w.plays(0x5a5) || w.plays(0x5a6) {
                c.drain = 0.0;
                c.auxiliary_flags |= 0x400;
            }
            common(c, w, Some(0.01), true);
        }
        7 => sustain(c, w, aware),
        8 => {
            if ![0, 2, 9, 3].into_iter().any(|n| w.plays(id(c, n))) {
                w.play(id(c, 3), f32::from_bits(0x3EAA7EFA), 1.0);
            }
            if w.transition(id(c, 0)) {
                w.play(id(c, 2), 0.0, 1.0);
            } else if w.transition(id(c, 2)) {
                w.play(id(c, 9), 0.0, 1.0);
            } else if w.transition(id(c, 9)) {
                if !w.plays(id(c, 3)) {
                    w.play(id(c, 3), 0.0, 1.0);
                }
            } else if w.transition(id(c, 3)) {
                w.play(id(c, 1), 0.0, 1.0);
                enter(c, 2);
                return;
            }
            if !w.plays(id(c, 3)) {
                if w.plays(id(c, 9)) {
                    if !c.selected {
                        w.leaf(L::Assess(false), c);
                    }
                    if aware {
                        c.unseen = 0.0;
                    }
                    if w.event_ready(0x200) {
                        w.leaf(L::Fire, c);
                    }
                }
            } else {
                w.leaf(L::Align(0.01), c);
            }
            if w.plays(id(c, 2)) {
                w.leaf(L::CoverAction(0.0), c);
                return;
            }
            if w.plays(id(c, 3)) {
                w.leaf(L::FinishAction, c);
                return;
            }
            if !w.plays(id(c, 9)) {
                w.leaf(L::Face, c);
            }
        }
        9 => {
            let angle = if !c.selected {
                w.target_angle(c)
            } else {
                c.assessment[1]
            };
            let finished = w.finished();
            let p = c.reservation.as_ref().unwrap().position;
            let delta = std::array::from_fn::<_, 3, _>(|n| c.position[n] - p[n]);
            let d = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
            if finished || d <= 0.8 {
                let col = usize::from(c.posture < 1);
                if !w.plays(IDS[0][col]) {
                    if !w.transition(IDS[2][col]) {
                        match c.posture {
                            -90 => {
                                let b = if w.plays(0x5b3) {
                                    0.0
                                } else {
                                    f32::from_bits(0x3EAA7EFA)
                                };
                                w.play(0x5f2, b, 1.0);
                            }
                            90 => {
                                let b = if w.plays(0x5c0) {
                                    0.0
                                } else {
                                    f32::from_bits(0x3EAA7EFA)
                                };
                                w.play(0x5f1, b, 1.0);
                            }
                            _ => {
                                if !w.plays(0x5f0) {
                                    w.play(0x5f0, f32::from_bits(0x3EAA7EFA), 1.0);
                                }
                                w.leaf(L::Face, c);
                            }
                        }
                        let tag = c.saved_reservation.as_ref().unwrap().tag;
                        w.leaf(L::Unreserve(tag), c);
                        c.node_flags &= !0x40000000;
                        w.class(0x10000);
                    } else {
                        let clip = w.leaf(L::TurnClip, c);
                        w.play(clip, 0.0, 1.0);
                        if c.posture < 10 && c.posture > -10 {
                            c.posture = 0;
                        }
                    }
                } else {
                    w.play(IDS[2][col], 0.0, 1.0);
                }
            }
            exit_tail(c, w, angle);
        }
        10 => {
            if c.action_timer > 0.3 {
                w.leaf(L::ExitMotion, c);
            }
            if c.state_time > 0.3 && w.finished() {
                if let Some(node) = &c.saved_reservation {
                    w.leaf(L::Unreserve(node.tag), c);
                    c.node_flags &= !0x40000000;
                }
                c.reservation = None;
                w.class(if aware { 4 } else { 8 });
            }
            w.leaf(L::Walk, c);
        }
        _ => {}
    }
}
