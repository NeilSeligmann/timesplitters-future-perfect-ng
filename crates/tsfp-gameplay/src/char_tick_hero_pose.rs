//! Animation-heavy state10/11 decisions of the hero controller0x2784D0.
//! These are state arms; outer dispatch/navigation/command progression is separate.
#[derive(Clone, Debug, PartialEq)]
pub struct HeroPoseCtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub flags: u32,
    pub auxiliary_flags: u32,
    pub clock: f32,
    pub repeat: i32,
    pub weapon_flags: u32,
    pub model: u32,
    pub record_tag: u32,
    pub target_present: bool,
    pub fast_mode: bool,
}
pub trait HeroPoseWorld {
    fn plays(&mut self, id: u32) -> bool;
    fn transition(&mut self, id: u32) -> bool;
    fn finished(&mut self) -> bool;
    fn rng_raw(&mut self) -> u32;
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    fn navigate(&mut self, c: &mut HeroPoseCtx);
    fn stop_motion(&mut self, c: &mut HeroPoseCtx);
}
#[path = "char_hero_pose_tables.rs"]
mod tables;
fn group(c: &HeroPoseCtx) -> usize {
    if c.weapon_flags & 1 != 0 {
        1
    } else if matches!(c.model, 14 | 15 | 53) {
        0
    } else {
        2
    }
}
fn enter(c: &mut HeroPoseCtx, state: u32) {
    if c.state != state {
        c.previous = c.state;
        c.state = state;
        c.pending = state as i32;
        c.state_time = 0.0;
    }
}
fn clear(c: &mut HeroPoseCtx) {
    c.flags &= !2;
    c.auxiliary_flags &= !0x400;
}
fn flags(c: &mut HeroPoseCtx, active: bool) {
    if active {
        c.flags |= 2;
        c.auxiliary_flags |= 0x400;
    } else {
        clear(c);
    }
}
fn unit(w: &mut impl HeroPoseWorld) -> f64 {
    f64::from(w.rng_raw()) / 4294967296.0
}
fn bucket(w: &mut impl HeroPoseWorld) -> i32 {
    (unit(w) + 0.5) as i32
}
fn clock(c: &mut HeroPoseCtx, w: &mut impl HeroPoseWorld) {
    c.clock = ((unit(w) * f64::from(f32::from_bits(0x3ECCCCCE)) + f64::from(0.2_f32))
        / f64::from(if c.fast_mode { 1.0_f32 } else { 0.8_f32 })) as f32;
}
fn burst(c: &mut HeroPoseCtx, w: &mut impl HeroPoseWorld, id: u32) {
    c.repeat = c.repeat.wrapping_add(1);
    w.play(id, 0.0, 1.0);
    clock(c, w);
}
/// State10 arm through0x27A6B0, before shared target refresh/command exit.
pub fn state10(c: &mut HeroPoseCtx, w: &mut impl HeroPoseWorld) {
    let g = group(c);
    let base = tables::IDS[0][g];
    let alternate = tables::IDS[1][g];
    let ending = tables::IDS[6][g];
    let firing = tables::IDS[5][g];
    let entering = tables::IDS[4][g];
    let a = w.plays(base);
    let d = w.plays(ending);
    let b = w.plays(firing);
    let e = w.plays(entering);
    w.navigate(c);
    w.stop_motion(c);
    if !a && !d && !b && !e {
        let id = if g == 0 || c.record_tag == 0x52313130 || bucket(w) == 1 {
            c.clock = 0.0;
            c.repeat = 1;
            base
        } else {
            enter(c, 11);
            alternate
        };
        w.play(id, 0.33, 1.0);
        clear(c);
        return;
    }
    if w.transition(entering) {
        c.clock = 0.0;
        c.repeat = 1;
        w.play(base, 0.0, 1.0);
        flags(c, b);
        return;
    }
    if !a && !d && !b {
        clear(c);
        return;
    }
    let finished = w.finished();
    if !finished || (a && c.clock <= 0.6) {
        if g == 1 {
            if !b {
                clear(c);
                return;
            }
            if c.repeat < 7 && c.clock > 0.8 && c.target_present {
                burst(c, w, tables::IDS[5][1]);
            }
        }
        flags(c, b);
        return;
    }
    if g == 1 && c.repeat < 7 && !d && c.target_present {
        burst(c, w, tables::IDS[5][1]);
        flags(c, b);
        return;
    }
    if g == 0 && c.repeat <= 1 && !d {
        c.repeat = c.repeat.wrapping_add(1);
        w.play(tables::IDS[5][0], 0.0, 1.0);
        flags(c, b);
        return;
    }
    if !b {
        if g != 0 && c.record_tag != 0x52313130 && bucket(w) != 1 {
            c.repeat = 0;
            w.play(alternate, 0.0, 1.0);
            enter(c, 11);
            clear(c);
            return;
        }
        c.repeat = 0;
        w.play(firing, 0.0, 1.0);
        clock(c, w);
        clear(c);
    } else {
        c.repeat = 0;
        c.clock = (unit(w) * f64::from(0.2_f32)) as f32;
        w.play(ending, 0.0, 1.0);
        flags(c, b);
    }
}
/// State11 arm through0x27A6B0, retaining duplicate finished queries and cached flags.
pub fn state11(c: &mut HeroPoseCtx, w: &mut impl HeroPoseWorld) {
    let g = group(c);
    let resting = tables::IDS[2][g];
    let firing = tables::IDS[3][g];
    let entering = tables::IDS[1][g];
    let a = w.plays(resting);
    let b = w.plays(firing);
    let e = w.plays(entering);
    w.navigate(c);
    w.stop_motion(c);
    if !a && !b && !e {
        let id = if bucket(w) == 1 {
            c.clock = 0.0;
            c.repeat = 1;
            firing
        } else {
            c.repeat = 0;
            enter(c, 10);
            tables::IDS[4][g]
        };
        w.play(id, 0.33, 1.0);
        clear(c);
        return;
    }
    if w.transition(entering) {
        c.clock = 0.0;
        c.repeat = 0;
        w.play(resting, 0.0, 1.0);
        flags(c, b);
        return;
    }
    if !a && !b {
        clear(c);
        return;
    }
    let first = w.finished();
    let action = (first && !a) || (!w.finished() && a && c.clock > 0.6);
    if action {
        match g {
            1 => {
                if c.repeat < 7 && c.target_present {
                    burst(c, w, tables::IDS[3][1]);
                } else {
                    c.clock = 0.0;
                    c.repeat = 0;
                    w.play(tables::IDS[2][1], 0.0, 1.0);
                }
            }
            2 => {
                let limit = (unit(w) * 2.0 + 0.5) as i32 + 1;
                if c.repeat < limit && c.target_present {
                    burst(c, w, tables::IDS[3][2]);
                } else {
                    c.clock = 0.0;
                    c.repeat = 0;
                    w.play(tables::IDS[2][2], 0.0, 1.0);
                }
            }
            _ => {
                let random = bucket(w);
                c.repeat = 0;
                if random == 1 {
                    w.play(firing, 0.0, 1.0);
                    clock(c, w);
                } else {
                    enter(c, 10);
                    w.play(tables::IDS[4][g], 0.0, 1.0);
                }
            }
        }
    } else if g == 1 {
        if !b {
            clear(c);
            return;
        }
        if c.repeat < 7 && c.clock > 0.8 && c.target_present {
            burst(c, w, tables::IDS[3][1]);
        }
    }
    flags(c, b);
}
