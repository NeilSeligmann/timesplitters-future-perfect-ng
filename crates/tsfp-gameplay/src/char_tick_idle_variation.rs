//! T1964: idle animation with random variation, original `0x2621A0`
//! (`game_char_update_idle_anim_with_random_variation_by_stance_row`).
//!
//! Usercall: `ebx` = character object (state block `C = [obj+0x7C]`), stack args `refresh` and `source`
//! (a pointer whose word `+0x34` carries flag `0x80000`). Tier 1 (differential against the original under Unicorn).
//!
//! The function picks a stance row (`1` when the held weapon table byte `+0xC` has bit 0, `2` when the model word
//! `C[0x94]` is 0xE, 0xF or 0x35, else `0`) and keeps the character on that row's idle loop (selector `0x2AEFB0`
//! variant 0) or on its variation clip (variant 5), choosing a new random wait `C[0xA80]` and a remaining repeat count
//! `C[0xAA0]` after each variation.

const LOOP: u32 = 0;
const VARIATION: u32 = 5;
const SOURCE_FORCE_REFRESH: u32 = 0x8_0000;

#[derive(Clone, Debug, PartialEq)]
pub struct IdleVariationCtx {
    /// Held weapon table byte `[0x4F9BAC + 0x29C * C[0x204] + 0xC]`.
    pub weapon_flags: u8,
    /// `C[0x94]`, the model word.
    pub model: u32,
    /// `C[0x28]`: bit 1 is cleared each tick and set while a variation plays under a refresh.
    pub flags: u32,
    /// `C[0x2C]`: bit `0x400` is set when the source carries `0x80000`.
    pub aux_flags: u32,
    /// `C[0xA00]` refresh countdown (counts down from the last reset by `1 / tick_rate`).
    pub refresh_timer: f32,
    /// `C[0xAA0]` remaining variation repeats (signed).
    pub repeats: i32,
    /// `C[0xA80]` random wait before the next variation.
    pub wait: f32,
    /// `C[0xB30]` idle loop time.
    pub loop_time: f32,
    /// `C[0xB34]` time since the variation started.
    pub variation_time: f32,
    /// `C[0x9D8]` state time compared with the variation clip time.
    pub state_time: f32,
    /// `[0x4E8CEC]`, the tick rate (60 in the title).
    pub tick_rate: i32,
}

pub trait IdleVariationWorld {
    /// `0x2AEFB0(stance, variant)`: the clip id of a stance row.
    fn select_clip(&mut self, stance: u32, variant: u32) -> u32;
    /// `0x59D20(object, id)`: the object plays the id.
    fn plays(&mut self, id: u32) -> bool;
    /// `0x59DD0(object+4)`: the current clip finished.
    fn finished(&mut self) -> bool;
    /// `0x59D50(object)`, float result: clip duration.
    fn clip_time(&mut self) -> f32;
    /// `0x153B40`: raw random dword.
    fn rng_raw(&mut self) -> u32;
    /// `0x5F460(object, id, blend, speed)`.
    fn play(&mut self, id: u32, blend: f32, speed: f32);
}

/// Stance row of the character.
pub fn stance(weapon_flags: u8, model: u32) -> u32 {
    if weapon_flags & 1 != 0 {
        1
    } else if matches!(model, 0xE | 0xF | 0x35) {
        2
    } else {
        0
    }
}

/// Next random wait: unsigned raw * 2^-32 * 0.3 + 0.2, evaluated in double precision as the x87 does, stored as f32.
fn random_wait(raw: u32) -> f32 {
    (f64::from(raw)
        * f64::from(f32::from_bits(0x2F80_0000))
        * f64::from(f32::from_bits(0x3E99_999A))
        + f64::from(f32::from_bits(0x3E4C_CCCD))) as f32
}

fn restart_wait(c: &mut IdleVariationCtx, w: &mut impl IdleVariationWorld) {
    c.variation_time = 0.0;
    c.wait = random_wait(w.rng_raw());
}

/// One tick of `0x2621A0`. `refresh` is the first stack argument, `source_flags` is `[source + 0x34]`.
pub fn tick(
    c: &mut IdleVariationCtx,
    w: &mut impl IdleVariationWorld,
    refresh: u32,
    source_flags: u32,
) {
    let stance = stance(c.weapon_flags, c.model);
    let forced = source_flags & SOURCE_FORCE_REFRESH != 0;
    let mut refresh = refresh != 0;
    if !refresh && !forced {
        c.refresh_timer = 1.0 / c.tick_rate as f32 * -1.0 + c.refresh_timer;
        if 1.0f32 > c.refresh_timer {
            refresh = true;
        }
    } else {
        c.refresh_timer = 0.0;
        refresh = true;
    }
    c.flags &= !2;
    let id = w.select_clip(stance, VARIATION);
    if w.plays(id) {
        if refresh {
            c.flags |= 2;
        }
        if forced {
            c.aux_flags |= 0x400;
            c.flags |= 2;
        }
    }
    let id = w.select_clip(stance, LOOP);
    let mut busy = w.plays(id);
    if !busy {
        let id = w.select_clip(stance, VARIATION);
        busy = w.plays(id);
    }
    if !busy {
        let id = w.select_clip(stance, LOOP);
        w.play(id, f32::from_bits(0x3EA8_F5C3), 1.0);
        c.loop_time = 0.0;
        return;
    }
    // Stance 1 hurries a variation once its wait elapsed.
    if stance == 1 && c.repeats > 0 && c.variation_time > c.wait && refresh {
        let id = w.select_clip(1, VARIATION);
        if w.plays(id) {
            let id = w.select_clip(1, VARIATION);
            w.play(id, 0.0, 1.0);
            restart_wait(c, w);
            c.repeats -= 1;
            return;
        }
    }
    if !w.finished() {
        return;
    }
    let id = w.select_clip(stance, VARIATION);
    let variation_playing = w.plays(id);
    if !variation_playing && refresh {
        let duration = f64::from(w.clip_time()) * 1.5;
        if duration > f64::from(c.state_time) {
            let id = w.select_clip(stance, VARIATION);
            w.play(id, 0.0, 1.0);
            restart_wait(c, w);
            c.repeats = repeats_after_variation(stance);
            return;
        }
    }
    let id = w.select_clip(stance, VARIATION);
    if w.plays(id) && c.repeats > 0 && refresh {
        let id = w.select_clip(stance, VARIATION);
        w.play(id, 0.0, 1.0);
        restart_wait(c, w);
        c.repeats -= 1;
        return;
    }
    let id = w.select_clip(stance, LOOP);
    if !w.plays(id) {
        let id = w.select_clip(stance, LOOP);
        w.play(id, 0.0, 1.0);
        c.loop_time = 0.0;
        return;
    }
    let id = w.select_clip(stance, LOOP);
    if w.plays(id) {
        let limit = if stance == 0 { 0.5 } else { 1.0 };
        if limit > c.loop_time {
            let id = w.select_clip(stance, LOOP);
            w.play(id, 0.0, 1.0);
            return;
        }
    }
    if !refresh {
        return;
    }
    let id = w.select_clip(stance, VARIATION);
    w.play(id, 0.0, 1.0);
    c.repeats = repeats_after_variation(stance);
    if stance == 1 {
        restart_wait(c, w);
    }
}

fn repeats_after_variation(stance: u32) -> i32 {
    match stance {
        0 => 0,
        1 => 8,
        _ => 2,
    }
}
