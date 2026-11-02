//! T1871: the effect pool update callbacks `0xF4A50` (13 entry types), `0xF08D0`, `0xF2B80` and `0xF39A0`, ported from the original and validated
//! bit for bit against it under the Unicorn oracle (`tools/t1846_gold/eff_update.py`, tests `tests/story_events_t1871_update.rs`). Document
//! `docs/t-story-event-effect-update-t1871.md`. Behaviour MEASURED (xemu level), names INFERRED.
//!
//! The ports run on the [`Guest`] interface of `eff_guest`: entry fields are guest words, engine routines are `Guest::call`s (the ported helper
//! family answers through `eff_dispatch::native`). Locals the original keeps on its stack live at the SAME ADDRESSES relative to the entry esp
//! (`FRAME`), so pointer fields of the records built for engine calls compare equal with the original's.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::eff_guest::Guest;

/// `esp` at entry of `0xF4A50` when called with one argument under the oracle (`0x7F000000 - 0x1000 - 8`). Any value works in production.
pub const FRAME: u32 = 0x7EFF_EFF8;
/// Frame rate `[0x4E8CEC]`, steps `[0x4E7944]`, step scale `[0x4E794C]` (the frame clock).
pub const FRAME_RATE: u32 = 0x004E_8CEC;
pub const STEPS: u32 = 0x004E_7944;
pub const STEP_SCALE: u32 = 0x004E_794C;
pub const LEVEL_ID: u32 = 0x0079_094C;
pub const PLAYER_COUNT_FIRST: u32 = 0x0079_0950;
const MODE_FLAGS: u32 = 0x007D_E458;

fn frame(offset: u32) -> u32 {
    FRAME.wrapping_sub(offset)
}

/// `0xF4A50(entry)`: the update of projectile and debris entries (13 types: 3 .. 0x2E, 0x31 ...). Names INFERRED, flow MEASURED.
///
/// 1. Nothing happens while the frame steps `[0x4E7944]` are 0. `dt = ((1 / rate) * [+0x88]) * [0x4E794C]`. An entry whose timer `+0x98` has
///    reached 2.0 or whose velocity `+0x64..+0x6C` is all zero is retired: `0xE1EB0(entry)` then `0xDC550` (release).
/// 2. The step `velocity * dt` is swept with `0xE97A0` (ray options record at `RAY`, hit point, hit normal). A positive record count starts the light
///    `0xDCEF0`; with `[0x7DE458] & 0x40` and `0x24A150` a trace `0x249B10` runs.
/// 3. NO HIT: the position takes the step, the trail history `+0x28 .. +0x3C` advances by `[+0x70 .. +0x84] * dt`, `+0xF4 += dt^2`.
/// 4. HIT (`target` = the ray result, an object): the travelled squared length is added to `+0xF4`; the velocity is reflected about the hit
///    normal (`v - 2 (v . n) n`); unless the target has flag `0x4000000` `0xE9FE0` reacts (type 7 entries ride a player block: `0xFA540`), else on
///    level 11 `0x2274C0(0x16, 0)`; actors run `0xDC5D0` + `0xE1F80(hit)` (`0xDD130` / `0xDC1D0` for others), others `0xEDBE0`; `0xE94E0(entry,
///    target type, hit, 6.0)`; a light record (`0xB91F0` + `0xB9220` + fields + `0xBD0F0`) unless flagged; the ground effect `0xF4920(hit, 0.4)`;
///    `0xE1EB0`; the surface (contact template word 6): `0x13` sticks to the hit point (0.05 along the normal), otherwise `0xE1400` may retire the
///    entry, else the entry bounces (0.01 along the normal, `f8 = f8^2`, flags `+0xC` lose `0x40` and gain `0x100`, timer reset) and types 3 / 4 and
///    `0x1B .. 0x1F`, `0x2D`, `0x2E` start their sound / trail handles (`0xFFDA0`, `0x102320`, `0xFF420`, `0x10A230`).
/// 5. Tail: type `0x31` runs `0xE17C0`, others retarget the handles `+0xB4` / `+0xB8` (`0xFF490`), then `0xEDBE0(position, old position, entry)`.
pub fn projectile_update(g: &mut dyn Guest, entry: u32) {
    let rate = g.read(FRAME_RATE) as i32 as f32;
    let mut dt = f32::from_bits(0x3F80_0000) / rate;
    dt *= g.readf(entry + 0x88);
    dt *= g.readf(STEP_SCALE);
    g.writef(frame(0x1B8), dt);
    if g.read(STEPS) == 0 {
        return;
    }
    let timer = g.readf(entry + 0x98);
    if timer >= 2.0 {
        g.call(0xE1EB0, &[entry]);
        g.call(0xDC550, &[entry]);
        return;
    }
    let velocity = g.vec3(entry + 0x64);
    if velocity[0] == 0.0 && velocity[1] == 0.0 && velocity[2] == 0.0 {
        g.call(0xE1EB0, &[entry]);
        g.call(0xDC550, &[entry]);
        return;
    }
    let (hit, normal, step, reflected, old, record, template, template_now, ray) = (
        frame(0x1EC),
        frame(0x1DC),
        frame(0x1D0),
        frame(0x1C4),
        frame(0x1B4),
        frame(0x1A8),
        frame(0x14C),
        frame(0x12C),
        frame(0x10C),
    );
    let position_pointer = entry + 0x1C;
    let position = g.vec3(position_pointer);
    g.set_vec3(old, position);
    g.set_vec3(step, [velocity[0] * dt, velocity[1] * dt, velocity[2] * dt]);
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let target = g.call(0xE97A0, &[step, normal, ray, entry, hit]);
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    if g.read(MODE_FLAGS) & 0x40 != 0 && g.call(0x24A150, &[]) != 0 {
        let attached = g.read(entry + 0x104);
        g.call(0x249B10, &[attached, position_pointer, step]);
    }
    if target == 0 {
        advance_without_hit(g, entry, step);
        finish_motion(g, entry, old);
        return;
    }
    g.copy(super::eff_helpers::CONTACT_LIVE, template, 8);
    let vel = g.vec3(entry + 0x64);
    let n = g.vec3(normal);
    let dot = (vel[2] * n[2] + vel[1] * n[1]) + vel[0] * n[0];
    let twice = dot * 2.0;
    g.set_vec3(
        reflected,
        [
            vel[0] - twice * n[0],
            vel[1] - twice * n[1],
            vel[2] - twice * n[2],
        ],
    );
    let point = g.vec3(hit);
    let here = g.vec3(position_pointer);
    let (dz, dy, dx) = (point[2] - here[2], point[1] - here[1], point[0] - here[0]);
    let travelled = ((dz * dz) + (dy * dy)) + dx * dx;
    let total = travelled + g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, total);
    let flagged = g.read(target + 0x9C) & 0x0400_0000;
    if flagged == 0 {
        let entry_type = g.read(entry);
        let source = g.read(entry + 0x108);
        g.call(
            0xE9FE0,
            &[entry, entry_type, target, hit, normal, reflected, source],
        );
        let entry_type = g.read(entry);
        if entry_type == 7 && g.read(entry + 0xC) & 0x40 == 0 && g.read(entry + 0x100) == 1 {
            let attached = g.read(entry + 0x104);
            if attached != 0 {
                let object = g.read(attached);
                if object != 0 {
                    let block = g.read(object + 0x150);
                    if block != 0 {
                        let word = g.read(block + 4);
                        if (word as i32) >= 0 {
                            g.call(0xFA540, &[word, hit, normal, 0x3F80_0000]);
                        }
                    }
                }
            }
        }
    } else if g.read(LEVEL_ID) == 0xB {
        let attached = g.read(entry + 0x104);
        if attached != 0 && (g.read(attached + 8) as i32) < (g.read(PLAYER_COUNT_FIRST) as i32) {
            g.call(0x2274C0, &[0x16, 0]);
        }
    }
    let target_type = g.read(target + 0x20);
    if target_type == 8 {
        if flagged == 0 {
            g.call(0xDC5D0, &[entry]);
            g.call(0xE1F80, &[hit]);
        }
    } else {
        let result = g.call(0xDD130, &[]);
        if g.read(entry + 0xC) & 0x40 == 0 && (result as i32) >= 0 {
            g.call(0xDC1D0, &[hit, result]);
        }
        g.call(0xEDBE0, &[hit, old, entry]);
    }
    let target_type = g.read(target + 0x20);
    g.call(0xE94E0, &[entry, target_type, hit, 0x40C0_0000]);
    g.call(0xB91F0, &[record]);
    if flagged == 0 {
        g.call(0xB9220, &[record, entry]);
        let size = g.read(entry + 0x88);
        g.write(record + 0x28, hit);
        g.write(record + 0x30, entry + 0x64);
        g.write(record, target);
        g.write(record + 0x2C, normal);
        g.write(record + 0x34, size);
        g.write(record + 0x38, template);
        g.call(0xBD0F0, &[record]);
    }
    let attached = g.read(entry + 0x104);
    let owner = if attached != 0 { g.read(attached) } else { 0 };
    g.call(0xF4920, &[hit, 0x3ECC_CCCD, owner]);
    g.call(0xE1EB0, &[entry]);
    g.copy(super::eff_helpers::CONTACT_LIVE, template_now, 8);
    let surface = g.read(template_now + 0x18);
    if surface == 0x13 {
        let n = g.vec3(normal);
        let point = g.vec3(hit);
        let pull = f32::from_bits(0x3D4C_CCCD);
        let placed = [
            point[0] - n[0] * pull,
            point[1] - n[1] * pull,
            point[2] - n[2] * pull,
        ];
        for base in [0x1C, 0x28, 0x34] {
            g.set_vec3(entry + base, placed);
        }
        g.call(0xE9690, &[entry]);
        g.writef(entry + 0x98, 0.0);
        finish_motion(g, entry, old);
        return;
    }
    if flagged == 0 && g.call(0xE1400, &[target, entry]) == 0 {
        g.call(0xDC550, &[entry]);
        return;
    }
    let n = g.vec3(normal);
    let point = g.vec3(hit);
    let reflected_velocity = g.vec3(reflected);
    let bounce = f32::from_bits(0x3C23_D70A);
    let f8 = g.readf(entry + 0xF8);
    let placed = [
        n[0] * bounce + point[0],
        n[1] * bounce + point[1],
        n[2] * bounce + point[2],
    ];
    for base in [0x1C, 0x28, 0x34] {
        g.set_vec3(entry + base, placed);
    }
    g.writef(entry + 0xF8, f8 * f8);
    g.set_vec3(entry + 0x64, reflected_velocity);
    g.call(0xE9690, &[entry]);
    let flags = (g.read(entry + 0xC) & 0xFFFF_FFBF) | 0x100;
    let kind = g.read(entry).wrapping_sub(3);
    g.writef(entry + 0x98, 0.0);
    g.write(entry + 0xC, flags);
    if kind <= 0x2B {
        let position_now = entry + 0x1C;
        let size = g.read(entry + 0x88);
        match BOUNCE_SOUND[kind as usize] {
            0 => {
                let handle = g.call(
                    0xFFDA0,
                    &[entry + 0xAC, 0, position_now, entry + 0x64, size, 0xFFFF],
                );
                g.write(entry + 0xAC, handle);
                let handle = g.call(
                    0x102320,
                    &[entry + 0xB8, 9, 0, position_now, entry + 0x64, 0xFFFF],
                );
                g.write(entry + 0xB8, handle);
                g.call(0xFF420, &[handle, 0x4000_0000]);
            }
            1 => {
                let word = g.read(entry + 4);
                let handle = g.call(
                    0x10A230,
                    &[
                        entry + 0xB8,
                        0,
                        word,
                        position_now,
                        entry + 0x64,
                        size,
                        0xFFFF,
                    ],
                );
                g.write(entry + 0xB8, handle);
            }
            _ => {}
        }
    }
    finish_motion(g, entry, old);
}

/// Table of the bounce sound handles by `type - 3` (0: sound + trail pair, 1: trail only, 2: none).
const BOUNCE_SOUND: [u8; 44] = [
    0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1,
];

/// `0xF4A50` without a hit: the position takes the step, the six trail words advance by their rates times `dt`, `+0xF4 += dt^2`.
fn advance_without_hit(g: &mut dyn Guest, entry: u32, step: u32) {
    let stepped = g.vec3(step);
    let x = g.readf(entry + 0x1C);
    g.writef(entry + 0x1C, x + stepped[0]);
    let y = g.readf(entry + 0x20);
    g.writef(entry + 0x20, stepped[1] + y);
    let z = g.readf(entry + 0x24);
    g.writef(entry + 0x24, z + stepped[2]);
    let dt = g.readf(frame(0x1B8));
    for index in 0..6u32 {
        let rate = g.readf(entry + 0x70 + 4 * index);
        let trail = g.readf(entry + 0x28 + 4 * index);
        g.writef(entry + 0x28 + 4 * index, rate * dt + trail);
    }
    let travelled = g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, dt * dt + travelled);
}

/// The tail every path shares (`0xF5138`): type `0x31` runs `0xE17C0`, others retarget the handles `+0xB4` / `+0xB8`; then
/// `0xEDBE0(position, old position, entry)`.
fn finish_motion(g: &mut dyn Guest, entry: u32, old: u32) {
    if g.read(entry) == 0x31 {
        g.call(0xE17C0, &[entry]);
    } else {
        let handle = g.read(entry + 0xB4);
        if handle != 0 && g.read(handle) != 0xC {
            g.call(0xFF490, &[handle, entry + 0x28]);
        }
        let handle = g.read(entry + 0xB8);
        if handle != 0 && g.read(handle) != 0xC {
            g.call(0xFF490, &[handle, entry + 0x34]);
        }
    }
    g.call(0xEDBE0, &[entry + 0x1C, old, entry]);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// 0xF08D0
// ---------------------------------------------------------------------------------------------------------------------------------

use super::eff_guest::{unsigned_as_f64, visual_rng, write_byte};

fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// The sound request of the death flash and the hit splash: `0x45390(request)`, then position, flag byte, id and ticks, `0x45820(request)`.
fn sound_request(g: &mut dyn Guest, request: u32, id: u32, ticks: u32, position: [f32; 3]) {
    g.call(0x45390, &[request]);
    g.set_vec3(request + 0x1C, position);
    write_byte(g, request + 0x28, 1);
    g.write(request, id);
    g.write(request + 4, ticks);
    g.call(0x45820, &[request]);
}

/// Link a fresh effect entry to its source object and start it: the shared tail of the three spawn sites of `0xF08D0`. Source object type 8
/// with a non negative flag word attaches its player block (`+0x7C`); a block with `+0x150` runs `0x238510(0x52, ...)` (mode 1), else mode 2;
/// a timer of 0 or more starts the entry (`0xEDD40`) and a negative one waits (flag `0x800000`).
fn link_and_start(g: &mut dyn Guest, new: u32) {
    let source = g.read(new + 0x10C);
    if source != 0 && g.read(source + 0x20) == 8 && (g.read(source + 0x28) as i32) >= 0 {
        let block = g.read(source + 0x7C);
        g.write(new + 0x104, block);
        if g.read(source + 0x150) != 0 {
            g.write(new + 0x100, 1);
            let id = g.at(source + 0x150, 0);
            g.call(0x238510, &[0x52, id, 0x3F80_0000]);
        } else {
            g.write(new + 0x100, 2);
        }
    }
    if g.readf(new + 0x98) >= 0.0 {
        g.call(0xEDD40, &[new]);
    } else {
        let flags = g.read(new + 0xC);
        g.write(new + 0xC, flags | 0x80_0000);
    }
}

/// The ticks of the splash sound: `(i32)(rate * 0.0)`.
fn ticks_zero(g: &mut dyn Guest) -> u32 {
    let rate = g.read(FRAME_RATE) as i32 as f32;
    (rate * 0.0f32) as i32 as u32
}

/// The `0xDC2B0(entry, index)` releases of the handles `+0xAC .. +0xB8` (the first retargeted to `point` with `0xFF490` before).
fn release_handles(g: &mut dyn Guest, entry: u32, point: u32) {
    let handle = g.read(entry + 0xAC);
    if handle != 0 {
        g.call(0xFF490, &[handle, point]);
        g.call(0xDC2B0, &[entry, 0]);
    }
    for (offset, index) in [(0xB0u32, 1u32), (0xB4, 2), (0xB8, 3)] {
        if g.read(entry + offset) != 0 {
            g.call(0xDC2B0, &[entry, index]);
        }
    }
}

/// Fields every spawned child gets.
fn new_fields(g: &mut dyn Guest, new: u32, position: [u32; 3], normal: [u32; 3], source: u32) {
    for (index, word) in position.into_iter().enumerate() {
        g.write(new + 0x1C + 4 * index as u32, word);
    }
    for (index, word) in normal.into_iter().enumerate() {
        g.write(new + 0x180 + 4 * index as u32, word);
    }
    g.write(new + 0xA0, 0);
    g.write(new + 0x104, 0);
    g.write(new + 0x108, source);
    g.write(new + 0x10C, source);
    g.writef(new + 0x98, 0.0);
    let cell = g.call(0xC3C50, &[new + 0x1C, 0x3DCC_CCCD, 0]);
    g.write(new + 0xF0, cell);
}

fn words3(g: &mut dyn Guest, address: u32) -> [u32; 3] {
    [g.read(address), g.read(address + 4), g.read(address + 8)]
}

/// `1CDAE0(...) - 1 != 0 ? 0x80000 : 0`: the sound option word of the splash.
fn option_word(level: u32) -> u32 {
    if level.wrapping_sub(1) != 0 {
        0x8_0000
    } else {
        0
    }
}

/// The death flash of a finished `0xF08D0` entry.
fn finish_burst(g: &mut dyn Guest, entry: u32, pos_pointer: u32) {
    release_handles(g, entry, pos_pointer);
    let source = g.read(entry + 0x10C);
    let entry_type_now = g.read(entry);
    let new = g.call(0xDC4A0, &[0x17]);
    g.write(new + 4, 0x23);
    g.write(new + 0x1C0, entry_type_now);
    let ticks = ticks_zero(g);
    g.write(frame(0x1D8), ticks);
    let id = g.rng() % 6 + 0xC33;
    let position = g.vec3(pos_pointer);
    sound_request(g, frame(0x1A4), id, ticks, position);
    let words = words3(g, pos_pointer);
    new_fields(g, new, words, [0, 0x3F80_0000, 0], source);
    link_and_start(g, new);
    let level = g.call(0x1CDAE0, &[]);
    g.call(0x100260, &[0, 0x12, option_word(level)]);
    g.call(0xDC550, &[entry]);
}

/// The splash of a hit: a child entry (kind 5 for type 0x23, else 0x23), sound, trace, light record, handle release and `0xDC550`.
fn hit_splash(g: &mut dyn Guest, entry: u32, target: u32, hit_point: u32, normal: u32) {
    let parent_type = g.read(entry);
    let source = g.read(entry + 0x10C);
    let new = g.call(0xDC4A0, &[0x17]);
    let kind = if parent_type == 0x23 { 5 } else { 0x23 };
    g.write(new + 4, kind);
    g.write(new + 0x1C0, parent_type);
    let ticks = ticks_zero(g);
    g.write(frame(0x1E8), ticks);
    let id = g.rng() % 6 + 0xC33;
    let point = g.vec3(hit_point);
    sound_request(g, frame(0x1A4), id, ticks, point);
    let n = g.vec3(normal);
    let tenth = f(0x3DCC_CCCD);
    let placed = [
        n[0] * tenth + point[0],
        n[1] * tenth + point[1],
        n[2] * tenth + point[2],
    ];
    let normal_words = words3(g, normal);
    new_fields(g, new, placed.map(f32::to_bits), normal_words, source);
    link_and_start(g, new);
    if parent_type != 0x23 {
        let level = g.call(0x1CDAE0, &[]);
        let sound = g.call(0x100260, &[0, 0x12, option_word(level)]);
        g.call(0xFF3D0, &[sound, 0x3EB3_3333]);
        let n = g.vec3(normal);
        let half = 0.5f32;
        g.set_vec3(
            frame(0x1E4),
            [n[0] * half, (n[1] + 1.0) * half, n[2] * half],
        );
        let level = g.call(0x1CDAE0, &[]);
        g.call(0x100AA0, &[0, 0x13, option_word(level)]);
    }
    if g.call(0xDD170, &[]) != 0 {
        let kind = g.read(entry);
        let cell = g.read(entry + 0xF0);
        g.call(0x114AF0, &[kind, 0, cell, target, hit_point, normal]);
    }
    let target_type = g.read(target + 0x20);
    g.call(0xE94E0, &[entry, target_type, hit_point, 0x40C0_0000]);
    let record = frame(0x168);
    g.call(0xB91F0, &[record]);
    g.call(0xB9220, &[record, entry]);
    let size = g.read(entry + 0x88);
    let pointer = g.read(frame(0x1D8));
    g.write(record + 0x2C, normal);
    g.write(record, target);
    g.write(record + 0x28, pointer);
    g.write(record + 0x30, entry + 0x64);
    g.write(record + 0x34, size);
    g.write(record + 0x40, 1);
    g.call(0xBD0F0, &[record]);
    release_handles(g, entry, hit_point);
    g.call(0xDC550, &[entry]);
}

/// The part of `0xF08D0` after a step that hit nothing: advance, grow, trail effects, owner follow.
fn advance_burst(g: &mut dyn Guest, entry: u32, pos_pointer: u32, step: u32, back: u32) {
    let stepped = g.vec3(step);
    let x = g.readf(pos_pointer);
    g.writef(pos_pointer, x + stepped[0]);
    let flags = g.read(entry + 0xC);
    let y = g.readf(entry + 0x20);
    g.writef(entry + 0x20, y + stepped[1]);
    let z = g.readf(entry + 0x24);
    g.writef(entry + 0x24, z + stepped[2]);
    let dt = g.readf(frame(0x1E8));
    let travelled = g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, dt * dt + travelled);
    if flags & 0x2000 == 0 && (g.read(STEPS) as i32) > 0 {
        let growth = f(0x3F82_8F5C);
        let mut count = 0i32;
        loop {
            let size = g.readf(entry + 0x88);
            g.writef(entry + 0x88, size * growth);
            let steps = g.read(STEPS) as i32;
            count += 1;
            if count >= steps {
                break;
            }
        }
    }
    let entry_type_now = g.read(entry);
    if entry_type_now == 0x2A && f(0x41D4_0000) > g.readf(entry + 0x88) {
        let vec = frame(0x1E4);
        let half = f64::from(f(0x3F00_0000));
        let k03 = f64::from(f(0x3E99_999A));
        let a = ((visual_rng(g) - half) * k03) as f32;
        g.writef(vec, a);
        let b = ((visual_rng(g) - half) * k03) as f32;
        g.writef(vec + 4, b);
        let c = ((visual_rng(g) - half) * k03) as f32;
        g.writef(vec + 8, c);
        g.call(
            0xF85C0,
            &[7, 0xD5, pos_pointer, vec, 0x3F19_999A, 0, 0x3F4C_CCCD],
        );
        let rate = g.read(FRAME_RATE) as i32 as f32;
        let mut grow = 1.0f32 / rate;
        grow *= g.readf(STEP_SCALE);
        grow *= f(0x4100_0000);
        let size = g.readf(entry + 0x88);
        g.writef(entry + 0x88, grow + size);
    }
    let owner = g.read(entry + 0xA8);
    if owner != 0 {
        let word = g.read(pos_pointer);
        g.write(owner + 0x64, word);
        let word = g.read(entry + 0x20);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x68, word);
        let word = g.read(entry + 0x24);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x6C, word);
        if g.read(entry) == 0x32 {
            let owner = g.read(entry + 0xA8);
            let cell = g.read(entry + 0xF0);
            g.write(owner + 0x34, cell);
            let owner = g.read(entry + 0xA8);
            let copy = g.read(owner + 0x34);
            g.write(owner + 0x38, copy);
            let owner = g.read(entry + 0xA8);
            write_byte(g, owner + 0x30, 1);
        }
    }
    let kind = g.read(entry);
    match kind {
        8 | 0xA => {
            let vec = frame(0x1E4);
            let half = f64::from(f(0x3F00_0000));
            let k03 = f64::from(f(0x3E99_999A));
            let a = ((visual_rng(g) - half) * k03) as f32;
            g.writef(vec, a);
            let b = (((visual_rng(g) - half) * k03) + f64::from(f(0x3F47_AE14))) as f32;
            g.writef(vec + 4, b);
            let c = ((visual_rng(g) - half) * k03) as f32;
            g.writef(vec + 8, c);
            g.call(0xF85C0, &[7, 0xD5, back, vec, 0x3F19_999A, 0, 0x3F4C_CCCD]);
        }
        0xB | 9 | 0x32 => {
            if g.read(entry + 0xAC) == 0 {
                let which = if kind == 0x32 {
                    0x7F
                } else {
                    u32::from(kind != 0xB) * 2 + 4
                };
                let handle = g.call(0x101660, &[entry + 0xAC, which, 0, back, 0xFFFF]);
                g.write(entry + 0xAC, handle);
            }
            let handle = g.read(entry + 0xAC);
            if handle != 0 {
                g.call(0xFF490, &[handle, back]);
                let handle = g.read(entry + 0xAC);
                g.call(0xFF420, &[handle, 0x41F0_0000]);
            }
        }
        0x2A => {
            if g.read(entry + 0xAC) == 0 {
                let handle = g.call(0x101660, &[entry + 0xAC, 5, 0, pos_pointer, 0xFFFF]);
                g.write(entry + 0xAC, handle);
            }
            let handle = g.read(entry + 0xAC);
            if handle != 0 {
                g.call(0xFF490, &[handle, pos_pointer]);
                let handle = g.read(entry + 0xAC);
                g.call(0xFF420, &[handle, 0x41F0_0000]);
            }
        }
        0x27 => {
            if g.read(entry + 0xAC) == 0 {
                let handle = g.call(0x105910, &[entry + 0xAC, 0x7D, 0, pos_pointer]);
                g.write(entry + 0xAC, handle);
            }
            let handle = g.read(entry + 0xAC);
            if handle != 0 {
                g.call(0xFF490, &[handle, pos_pointer]);
                let scaled = g.readf(entry + 0x88) * f(0x4316_0000);
                let handle = g.read(entry + 0xAC);
                g.call(0xFF420, &[handle, scaled.to_bits()]);
            }
        }
        0x23 => scatter_handles(g, entry, pos_pointer),
        _ => {}
    }
    g.call(0xEDBE0, &[pos_pointer, 0, entry]);
}

/// Type 0x23: the four handles follow the entry with a random scatter (`0xFF490`): `((u - a) * b) + position` per axis, three draws each.
fn scatter_handles(g: &mut dyn Guest, entry: u32, pos_pointer: u32) {
    let vec = frame(0x1E4);
    let draw = |g: &mut dyn Guest, offset: f32, spread: f32, base: f32| -> f32 {
        let word = g.rng();
        ((unsigned_as_f64(word) * f64::from(f(0x2F80_0000)) - f64::from(offset))
            * f64::from(spread)
            + f64::from(base)) as f32
    };
    for (handle_offset, offset, spread) in [
        (0xACu32, f(0x3F00_0000), f(0x3F00_0000)),
        (0xB0, f(0x3F4C_CCCD), f(0x3F00_0000)),
        (0xB4, f(0x3F00_0000), f(0x3ECC_CCCD)),
    ] {
        if g.read(entry + handle_offset) == 0 {
            continue;
        }
        let base_x = g.readf(pos_pointer);
        let x = draw(g, offset, spread, base_x);
        g.writef(vec, x);
        let base_y = g.readf(entry + 0x20);
        let y = draw(g, offset, spread, base_y);
        g.writef(vec + 4, y);
        let base_z = g.readf(entry + 0x24);
        let z = draw(g, offset, spread, base_z);
        g.writef(vec + 8, z);
        let handle = g.read(entry + handle_offset);
        g.call(0xFF490, &[handle, vec]);
    }
    let handle = g.read(entry + 0xB8);
    if handle != 0 {
        g.call(0xFF490, &[handle, pos_pointer]);
    }
}

/// `0xF08D0(entry)`: the update of the timed burst entries (types 8, 9, 0xA, 0xB, 0x23, 0x27, 0x2A, 0x32). Names INFERRED, flow MEASURED.
///
/// * Finished (type 0x32 asks `[[+0xA8] + 0x1A0]`; the others pass a limit: type 0x27 2.5; flag `0x2000` 12.0 or, past 4.0, a source object `+0x19C`
///   with the sign flag; otherwise 4.0; flag `0x10000000` replaces the verdict by "no source or the source has the sign flag"): [`finish_burst`].
/// * Flag `0x2000` entries first run the area hit (`0xE2520` for type 0x32 with the source distance, otherwise `0xE1FA0` + `0xE2370` with a scale
///   chosen by type and timer, 4x with flag `0x10000000`).
/// * Motion: `dt = ((1 / rate) * [0x4E794C]) * [+0x88]`; the step is swept with `0xDCB80` (source attached ray, hit point / normal / ray record at
///   `E-0x1C8` / `E-0x1D4` / `E-0x10C`). A hit reflects the velocity damped by 0.3, reacts (`0xE9FE0` + `0xDC5D0` for actors) and
///   [`hit_splash`]; no hit [`advance_burst`].
pub fn particle_update(g: &mut dyn Guest, entry: u32) {
    if g.read(STEPS) == 0 {
        return;
    }
    let entry_type = g.read(entry);
    let flags = g.read(entry + 0xC);
    let timer = g.readf(entry + 0x98);
    let finished = if entry_type == 0x32 {
        g.at(entry + 0xA8, 0x1A0) != 0
    } else {
        let mut expired = entry_type == 0x27 && timer >= 2.5;
        let c = flags & 0x2000;
        if !expired && c != 0 {
            if timer >= 12.0 {
                expired = true;
            } else if timer >= 4.0 {
                let object = g.read(entry + 0x19C);
                if object != 0 && g.read(object + 0x28) & 0x8000_0000 != 0 {
                    expired = true;
                }
            }
        }
        if !expired && c == 0 && timer >= 4.0 {
            expired = true;
        }
        if flags & 0x1000_0000 != 0 {
            let object = g.read(entry + 0x19C);
            object == 0 || g.read(object + 0x28) & 0x8000_0000 != 0
        } else {
            expired
        }
    };
    let pos_pointer = entry + 0x1C;
    if finished {
        finish_burst(g, entry, pos_pointer);
        return;
    }
    let scale_slot = frame(0x1EC);
    let (vec_a, normal, hit_point, refl, step, back, ray) = (
        frame(0x1E4),
        frame(0x1D4),
        frame(0x1C8),
        frame(0x1BC),
        frame(0x1B0),
        frame(0x174),
        frame(0x10C),
    );
    if flags & 0x2000 != 0 {
        let base: f32 = match entry_type.wrapping_sub(9) {
            0 | 2 => f(0x3D23_D70A),
            26 => {
                if f(0x3E80_0000) > timer {
                    0.5
                } else {
                    f(0x3E4C_CCCD)
                }
            }
            30 => {
                if flags & 0x40_0000 != 0 {
                    f(0x3D23_D70A)
                } else {
                    0.5
                }
            }
            33 => f(0x3D4C_CCCD),
            41 => {
                if f(0x3F19_999A) > timer {
                    f(0x3A83_126F)
                } else {
                    f(0x3E4C_CCCD)
                }
            }
            _ => f(0x3C23_D70A),
        };
        let mut scale = base;
        if flags & 0x1000_0000 != 0 {
            scale *= 4.0;
        }
        g.writef(scale_slot, scale);
        if entry_type == 0x32 {
            let object = g.read(entry + 0x19C);
            let here = g.vec3(pos_pointer);
            let there = g.vec3(object + 0x64);
            let x = (f64::from(there[0]) - f64::from(here[0])) as f32;
            let y = (f64::from(there[1]) - f64::from(here[1])) as f32;
            let z_exact = f64::from(there[2]) - f64::from(here[2]);
            let z = z_exact as f32;
            g.set_vec3(refl, [x, y, z]);
            let sum = (z_exact * f64::from(z) + f64::from(y) * f64::from(y))
                + f64::from(x) * f64::from(x);
            let distance = sum.sqrt() as f32;
            g.call(0xE2520, &[entry, scale.to_bits(), refl, distance.to_bits()]);
        } else {
            let radius = g.call_f(0xE1FA0, &[entry, vec_a]);
            let apply;
            if entry_type == 0x2A {
                let value = (1.0f32 - radius * f(0x3CA3_D70A)) * scale;
                let floor = f(0x3C23_D70A);
                if floor > value {
                    g.writef(scale_slot, floor);
                    apply = true;
                } else {
                    g.writef(scale_slot, value);
                    apply = value > f(0);
                }
            } else {
                apply = scale > f(0);
            }
            if apply {
                let scale_now = g.readf(scale_slot);
                g.call(
                    0xE2370,
                    &[entry, scale_now.to_bits(), vec_a, radius.to_bits()],
                );
            }
        }
    }
    // motion
    let rate = g.read(FRAME_RATE) as i32 as f32;
    let mut dt = 1.0f32 / rate;
    dt *= g.readf(STEP_SCALE);
    dt *= g.readf(entry + 0x88);
    let velocity = g.vec3(entry + 0x64);
    let pos = g.vec3(pos_pointer);
    g.set_vec3(step, [velocity[0] * dt, velocity[1] * dt, velocity[2] * dt]);
    g.writef(frame(0x1E8), dt);
    let k03 = f(0x3E99_999A);
    g.set_vec3(
        back,
        [
            pos[0] - velocity[0] * k03,
            pos[1] - velocity[1] * k03,
            pos[2] - velocity[2] * k03,
        ],
    );
    g.write(frame(0x1D8), pos_pointer);
    let owner = g.read(entry + 0xA8);
    if owner != 0 && entry_type == 0x32 {
        let v = g.read(owner + 0x28);
        g.write(owner + 0x28, v & 0xFFFF_FF9F);
    }
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let mut target = g.call(0xDCB80, &[step, hit_point, normal, ray, entry]);
    g.write(scale_slot, target);
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    if g.read(entry + 0x108) == target && f(0x3F00_0000) > g.readf(entry + 0x98) {
        target = 0;
        g.write(scale_slot, 0);
    }
    let owner = g.read(entry + 0xA8);
    if owner != 0 && entry_type == 0x32 {
        let v = g.read(owner + 0x28);
        g.write(owner + 0x28, v | 0x60);
    }
    if target == 0 {
        advance_burst(g, entry, pos_pointer, step, back);
        return;
    }
    // HIT: reflect (damped by 0.3), react, splash
    let n = g.vec3(normal);
    let h = g.vec3(hit_point);
    let v = g.vec3(entry + 0x64);
    let dot = (v[2] * n[2] + v[1] * n[1]) + v[0] * n[0];
    let twice = dot * 2.0;
    g.set_vec3(
        refl,
        [
            v[0] - (twice * n[0]) * k03,
            v[1] - (twice * n[1]) * k03,
            v[2] - (twice * n[2]) * k03,
        ],
    );
    let here = g.vec3(pos_pointer);
    let (dz, dy, dx) = (h[2] - here[2], h[1] - here[1], h[0] - here[0]);
    let travelled = ((dz * dz) + (dy * dy)) + dx * dx;
    let total = travelled + g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, total);
    if g.read(target + 0x20) == 8 {
        let kind = g.read(entry);
        let source = g.read(entry + 0x108);
        g.call(
            0xE9FE0,
            &[entry, kind, target, hit_point, normal, refl, source],
        );
        g.call(0xDC5D0, &[entry]);
    }
    hit_splash(g, entry, target, hit_point, normal);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// 0xF2B80 and its helpers 0xF2910 / 0xF2AB0
// ---------------------------------------------------------------------------------------------------------------------------------

const UP_VECTOR: u32 = super::eff_guest::SCRATCH + 0x500;

/// Write the up vector `(0, 1, 0)` the impact helpers hand to `0x111AE0` / `0xE00C0`.
fn write_up_vector(g: &mut dyn Guest, address: u32) {
    g.set_vec3(address, [0.0, 1.0, 0.0]);
}

/// Release the sound handle `+0xAC` of an entry: `0xFF420(handle, 0)`, `handle[+0x14] = 0`, `+0xAC = 0`.
fn release_sound_handle(g: &mut dyn Guest, entry: u32) {
    let handle = g.read(entry + 0xAC);
    if handle != 0 {
        g.call(0xFF420, &[handle, 0]);
        let handle = g.read(entry + 0xAC);
        g.write(handle + 0x14, 0);
        g.write(entry + 0xAC, 0);
    }
}

/// `0xF2910(entry)` (usercall eax = entry): the death of a debris entry that struck a hard surface: release the handle, flash `0x111AE0`, spawn a
/// kind `0xD` child with the sound `0x21A40(0x1E6, rate, 75.0, position)` that waits (timer -1) and is started or linked like the splash children
/// of `0xF08D0`.
pub fn impact_death(g: &mut dyn Guest, entry: u32) {
    write_up_vector(g, UP_VECTOR);
    release_sound_handle(g, entry);
    let pos_pointer = entry + 0x1C;
    g.call(0x111AE0, &[0, 0x0073_7C60, 0, pos_pointer, UP_VECTOR]);
    let source = g.read(entry + 0x10C);
    let new = g.call(0xDC4A0, &[0x17]);
    g.write(new + 4, 0xD);
    g.write(new + 0x1C0, 0);
    let rate = g.read(FRAME_RATE) as i32 as f32;
    g.call(
        0x21A40,
        &[0x1E6, rate as i32 as u32, 0x4296_0000, pos_pointer],
    );
    let words = words3(g, pos_pointer);
    for (index, word) in words.into_iter().enumerate() {
        g.write(new + 0x1C + 4 * index as u32, word);
    }
    g.writef(new + 0x180, 0.0);
    g.writef(new + 0x188, 0.0);
    g.writef(new + 0x184, 1.0);
    g.write(new + 0xA0, 0);
    g.write(new + 0x104, 0);
    g.write(new + 0x108, source);
    g.write(new + 0x10C, source);
    g.writef(new + 0x98, -1.0);
    let cell = g.call(0xC3C50, &[new + 0x1C, 0x3DCC_CCCD, 0]);
    g.write(new + 0xF0, cell);
    link_and_start(g, new);
}

/// `0xF2AB0` (usercall esi = entry, edi = struck object or 0; stack `hit`, `normal`): the impact effect of a debris entry. Releases the handle,
/// `0xEFBD0(kind, type, position, edi ? normal : 0, source, 0, 0, 1)` with kind 6 (7 for type 0xD, 0x2B for type 0xF), `0x114AF0(type, 0, cell,
/// edi, edi ? hit : position, normal)` and, except for type 0xD, `0xE00C0(entry, edi ? normal : up, 0x1F)`.
pub fn impact_effect(g: &mut dyn Guest, entry: u32, target: u32, hit: u32, normal: u32) {
    write_up_vector(g, UP_VECTOR);
    let entry_type = g.read(entry);
    let mut kind = u32::from(entry_type == 0xD) + 6;
    if entry_type == 0xF {
        kind = 0x2B;
    }
    release_sound_handle(g, entry);
    let source = g.read(entry + 0x10C);
    let entry_type = g.read(entry);
    let pos_pointer = entry + 0x1C;
    let spawn_normal = if target != 0 { normal } else { 0 };
    g.call(
        0xEFBD0,
        &[kind, entry_type, pos_pointer, spawn_normal, source, 0, 0, 1],
    );
    let cell = g.read(entry + 0xF0);
    let entry_type = g.read(entry);
    let point = if target != 0 { hit } else { pos_pointer };
    g.call(0x114AF0, &[entry_type, 0, cell, target, point, normal]);
    if g.read(entry) != 0xD {
        let direction = if target == 0 { UP_VECTOR } else { normal };
        g.call(0xE00C0, &[entry, direction, 0x1F]);
    }
}

/// One rng draw as the x87 sequence: `u32 * 2^-32` as a double.
fn draw_unit(g: &mut dyn Guest) -> f64 {
    unsigned_as_f64(g.rng()) * f64::from(f(0x2F80_0000))
}

/// `0xF2B80(entry)`: the update of the bouncing debris entries (types 0xC, 0xD, 0xF). Names INFERRED, flow MEASURED.
///
/// * An attached owner (`+0xA8`) without its flag `0x100` first turns its model matrix: `0x153440`, rotation `0x152770` about the owner's axis `+0xD4`,
///   `0x153650`, then `0x1531D0` for every further part.
/// * Per type constants: 0xC lives `1.5 * [+0x1FC] + 1`, 0xD 2.0, 0xF 0.75; the bounce factors, the spin handles and the impact kind differ.
///   A finished entry spawns its impact (`0xEFBD0`, `0x114AF0`, `0xE00C0`) and releases.
/// * Moving: `step = velocity * dt`, probe `velocity * 0.11 + step`; type 0xD throws sparks (`0xF85C0`, the visual generator `0x153BD0`).
///   The probe is swept with `0xDCB80`; a hit places the entry 0.12 along the normal, advances the cell (`0x88430`), scales the size by the
///   bounce factor (a size under 0.001 stops it), reflects the velocity (`v - 2 (v . n) n`, renormalised, bounce counter `+0x198`, sounds
///   `0xD34 ..`), types 0xD and 0xF end on actors (`0xF2AB0`), hard surfaces (`0xF2910`) end the entry.
/// * No hit: advance, `+0xF4 += dt^2`, size x0.99 per step, owner follows with a random spin axis.
/// * Type 0xC keeps a sound handle (`0xFF4B0` / `0xFFF70` / `0xFF490`), type 0xD with bounce count 1 throws two debris entries (`0xEA9C0`),
///   the owner spin decays (x0.99, wraps at 360), and with mode byte 10 old entries call `0x26C9A0`.
pub fn debris_update(g: &mut dyn Guest, entry: u32) {
    let rate = g.read(FRAME_RATE) as i32 as f32;
    let mut dt = 1.0f32 / rate;
    dt *= g.readf(entry + 0x88);
    dt *= g.readf(STEP_SCALE);
    let (e_dt, e_bounce, e_slot_a, e_slot_b, e_vec, e_target, e_limit, e_slot_c) = (
        frame(0x1B0),
        frame(0x1AC),
        frame(0x1A8),
        frame(0x1A4),
        frame(0x1A0),
        frame(0x194),
        frame(0x190),
        frame(0x18C),
    );
    let (e_step, e_normal, e_diff, e_probe, e_hit, e_template, e_ray) = (
        frame(0x188),
        frame(0x17C),
        frame(0x170),
        frame(0x164),
        frame(0x158),
        frame(0x14C),
        frame(0x10C),
    );
    g.write(e_target, 0);
    g.writef(e_slot_b, 0.0);
    g.writef(e_slot_a, 0.0);
    g.writef(e_slot_c, 0.0);
    g.writef(e_limit, 0.0);
    g.writef(e_bounce, 0.0);
    g.writef(e_dt, dt);
    let pos_pointer = entry + 0x1C;
    let owner = g.read(entry + 0xA8);
    if owner != 0 && g.read(owner + 0x28) & 0x100 == 0 {
        spin_owner(g, entry, e_template);
    }
    if g.read(STEPS) == 0 {
        return;
    }
    let entry_type = g.read(entry);
    let mut spinner = 0u32;
    let timer = g.readf(entry + 0x98);
    match entry_type {
        0xC => {
            g.writef(e_slot_b, f(0x3E99_999A));
            g.writef(e_slot_a, f(0x3E99_999A));
            g.writef(e_slot_c, f(0x3DCC_CCCD));
            let weight = g.readf(entry + 0x1FC);
            g.writef(e_limit, weight * f(0x3FC0_0000) + 1.0);
            g.writef(e_bounce, weight * f(0x4120_0000) - f(0x41D8_0000));
        }
        0xD => {
            g.writef(e_slot_b, 0.5);
            g.writef(e_slot_a, f(0x3ECC_CCCD));
            g.writef(e_slot_c, f(0x3E99_999A));
            g.writef(e_limit, 2.0);
            g.writef(e_bounce, f(0xC190_0000));
        }
        0xF => {
            g.writef(e_limit, f(0x3F40_0000));
            g.writef(e_slot_b, 0.0);
            g.writef(e_slot_a, 0.0);
            g.writef(e_slot_c, 0.0);
            spinner = 1;
            g.writef(e_bounce, f(0xC1D8_0000));
        }
        _ => {}
    }
    if timer >= g.readf(e_limit) {
        // finished: the impact effect
        g.set_vec3(e_vec, [0.0, 1.0, 0.0]);
        let mut kind = u32::from(entry_type == 0xD) + 6;
        if entry_type == 0xF {
            kind = 0x2B;
        }
        release_sound_handle(g, entry);
        let source = g.read(entry + 0x10C);
        let entry_type_now = g.read(entry);
        g.call(
            0xEFBD0,
            &[kind, entry_type_now, pos_pointer, 0, source, 0, 0, 1],
        );
        let cell = g.read(entry + 0xF0);
        let entry_type_now = g.read(entry);
        g.call(0x114AF0, &[entry_type_now, 0, cell, 0, pos_pointer, 0]);
        if g.read(entry) != 0xD {
            g.call(0xE00C0, &[entry, e_vec, 0x1F]);
        }
        g.call(0xDC550, &[entry]);
        return;
    }
    let velocity = g.vec3(entry + 0x64);
    let dt_now = g.readf(e_dt);
    let step = [
        velocity[0] * dt_now,
        velocity[1] * dt_now,
        velocity[2] * dt_now,
    ];
    g.set_vec3(e_step, step);
    let size = g.readf(entry + 0x88);
    if !(size > 0.0) {
        stopped_debris(g, entry, e_probe);
        debris_tail(g, entry);
        return;
    }
    let lookahead = f(0x3DE1_47AE);
    g.set_vec3(
        e_probe,
        [
            velocity[0] * lookahead + step[0],
            velocity[1] * lookahead + step[1],
            velocity[2] * lookahead + step[2],
        ],
    );
    if entry_type == 0xD {
        let half = f64::from(f(0x3F00_0000));
        let k03 = f64::from(f(0x3E99_999A));
        let a = ((visual_rng(g) - half) * k03) as f32;
        g.writef(e_vec, a);
        let b = (((visual_rng(g) - half) * k03) + f64::from(f(0x3F47_AE14))) as f32;
        g.writef(e_vec + 4, b);
        let c = ((visual_rng(g) - half) * k03) as f32;
        g.writef(e_vec + 8, c);
        g.call(
            0xF85C0,
            &[6, 0xD5, pos_pointer, e_vec, 0x3F19_999A, 0, 0x3F4C_CCCD],
        );
    }
    g.write(e_ray, 0);
    g.write(e_ray + 4, 0);
    let target = g.call(0xDCB80, &[e_probe, e_hit, e_normal, e_ray, entry]);
    g.write(e_target, target);
    if (g.read(e_ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, e_ray]);
    }
    if target == 0 {
        let moved = g.vec3(e_step);
        let here = g.vec3(pos_pointer);
        g.set_vec3(
            pos_pointer,
            [moved[0] + here[0], moved[1] + here[1], moved[2] + here[2]],
        );
        let bounce_word = g.read(e_bounce);
        let dt_now = g.readf(e_dt);
        let travelled = g.readf(entry + 0xF4);
        g.writef(entry + 0xF4, dt_now * dt_now + travelled);
        g.call(0xDC6F0, &[bounce_word, entry]);
        if (g.read(STEPS) as i32) > 0 {
            let mut count = 0i32;
            loop {
                let size = g.readf(entry + 0x88);
                g.writef(entry + 0x88, size * f(0x3F7D_70A4));
                count += 1;
                if count >= g.read(STEPS) as i32 {
                    break;
                }
            }
        }
        follow_owner_random(g, entry, pos_pointer);
        debris_tail(g, entry);
        return;
    }
    let target_type = g.read(target + 0x20);
    let bounce_factor = if target_type == 8 {
        g.readf(e_slot_c)
    } else {
        g.copy(super::eff_helpers::CONTACT_LIVE, e_template, 8);
        let surface = g.read(e_template + 0x18);
        if surface == 1 || surface == 2 || surface == 7 {
            g.call(0xF2910, &[entry]);
            g.call(0xDC550, &[entry]);
            return;
        }
        g.copy(super::eff_helpers::CONTACT_LIVE, e_template, 8);
        if g.readf(e_normal + 4) >= f(0x3F34_FDF4) {
            g.readf(e_slot_b)
        } else {
            g.readf(e_slot_a)
        }
    };
    g.writef(e_bounce, bounce_factor);
    if spinner != 0 {
        g.call(0xF2AB0, &[e_hit, e_normal, entry, target]);
        g.call(0xDC550, &[entry]);
        return;
    }
    let n = g.vec3(e_normal);
    let h = g.vec3(e_hit);
    let would_be = [
        step[0] + g.readf(pos_pointer),
        step[1] + g.readf(entry + 0x20),
        step[2] + g.readf(entry + 0x24),
    ];
    let cell = g.read(entry + 0xF0);
    let push = f(0x3DF5_C28F);
    let placed = [n[0] * push + h[0], n[1] * push + h[1], n[2] * push + h[2]];
    g.set_vec3(pos_pointer, placed);
    g.set_vec3(e_vec, would_be);
    g.set_vec3(
        e_diff,
        [
            placed[0] - would_be[0],
            placed[1] - would_be[1],
            placed[2] - would_be[2],
        ],
    );
    let new_cell = g.call(0x88430, &[e_vec, e_diff, cell, 0, 0, 0, 0]);
    g.write(entry + 0xF0, new_cell);
    let owner = g.read(entry + 0xA8);
    if owner != 0 {
        g.write(owner + 0x34, new_cell);
    }
    let size = g.readf(entry + 0x88);
    let bounce = g.readf(e_bounce);
    let grown = size * bounce;
    g.writef(entry + 0x88, grown);
    let owner = g.read(entry + 0xA8);
    let mut reflect = true;
    if f(0x3A83_126F) > grown {
        g.writef(entry + 0x88, 0.0);
        g.writef(entry + 0x64, 0.0);
        g.writef(entry + 0x68, 0.0);
        g.writef(entry + 0x6C, 0.0);
        if owner != 0 {
            g.writef(owner + 0xD0, 0.0);
        }
        reflect = false;
    } else if owner != 0 {
        let spin = g.readf(owner + 0xD0);
        g.writef(owner + 0xD0, spin * bounce);
    }
    if reflect {
        reflect_debris(g, entry, e_normal, pos_pointer);
    }
    let owner = g.read(entry + 0xA8);
    if owner != 0 {
        let word = g.read(pos_pointer);
        g.write(owner + 0x64, word);
        let word = g.read(entry + 0x20);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x68, word);
        let word = g.read(entry + 0x24);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x6C, word);
    }
    if g.read(entry) == 0xD && target_type == 8 {
        g.call(0xDC5D0, &[entry]);
        g.call(0xF2AB0, &[e_hit, e_normal, entry, target]);
        g.call(0xDC550, &[entry]);
        return;
    }
    debris_tail(g, entry);
}

/// The owner model matrix turn at the top of `0xF2B80`: `0x153440(part, spin)`, rotation `0x152770(matrix, axis, angle)`, `0x153650(part, matrix)`
/// and `0x1531D0(part_k, part)` for the further parts (`k * 0x40` bytes on).
fn spin_owner(g: &mut dyn Guest, entry: u32, matrix: u32) {
    let radians = f(0x3C8E_FA35);
    let owner = g.read(entry + 0xA8);
    let angle = g.readf(owner + 0x70) * radians;
    let part = g.at(owner + 4, 0x10);
    g.call(0x153440, &[part, angle.to_bits()]);
    let owner = g.read(entry + 0xA8);
    let spin = g.readf(owner + 0xCC) * radians;
    g.call(0x152770, &[matrix, owner + 0xD4, spin.to_bits()]);
    let owner = g.read(entry + 0xA8);
    let part = g.at(owner + 4, 0x10);
    g.call(0x153650, &[part, matrix]);
    let count_of = |g: &mut dyn Guest| {
        let owner = g.read(entry + 0xA8);
        let first = g.read(owner + 4);
        let second = g.read(first + 4);
        g.read(second + 4)
    };
    let mut count = count_of(g) as i32;
    if count > 1 {
        let mut index = 1i32;
        let mut offset = 0x40u32;
        loop {
            let owner = g.read(entry + 0xA8);
            let part = g.at(owner + 4, 0x10);
            g.call(0x1531D0, &[part.wrapping_add(offset), part]);
            count = count_of(g) as i32;
            index += 1;
            offset += 0x40;
            if index >= count {
                break;
            }
        }
    }
}

/// `0xF2B80` with a size of 0 or less: the entry is nudged straight down (`-0.11` probe through `0xDCB80`), restoring the cell when that hits.
fn stopped_debris(g: &mut dyn Guest, entry: u32, probe: u32) {
    g.writef(entry + 0x68, -1.0);
    let cell = g.read(entry + 0xF0);
    g.writef(entry + 0x88, f(0x3F99_999A));
    g.writef(entry + 0x64, 0.0);
    g.writef(entry + 0x6C, 0.0);
    g.set_vec3(probe, [0.0, f(0xBDE1_47AE), 0.0]);
    if g.call(0xDCB80, &[probe, 0, 0, 0, entry]) != 0 {
        g.write(entry + 0xF0, cell);
        let owner = g.read(entry + 0xA8);
        if owner != 0 {
            g.write(owner + 0x34, cell);
        }
        g.writef(entry + 0x64, 0.0);
        g.writef(entry + 0x68, 0.0);
        g.writef(entry + 0x6C, 0.0);
        g.writef(entry + 0x88, 0.0);
    }
}

/// The owner follow after a step without a hit: position words, three unit draws as the new spin axis (`+0xD4 ..`), normalised in x87 double.
fn follow_owner_random(g: &mut dyn Guest, entry: u32, pos_pointer: u32) {
    let owner = g.read(entry + 0xA8);
    if owner == 0 {
        return;
    }
    let word = g.read(pos_pointer);
    g.write(owner + 0x64, word);
    let word = g.read(entry + 0x20);
    let owner = g.read(entry + 0xA8);
    g.write(owner + 0x68, word);
    let word = g.read(entry + 0x24);
    let owner = g.read(entry + 0xA8);
    g.write(owner + 0x6C, word);
    for slot in [0xD4u32, 0xD8, 0xDC] {
        let unit = draw_unit(g);
        let owner = g.read(entry + 0xA8);
        g.writef(owner + slot, unit as f32);
    }
    let owner = g.read(entry + 0xA8);
    let axis = [
        g.readf(owner + 0xD4),
        g.readf(owner + 0xD8),
        g.readf(owner + 0xDC),
    ];
    let sum = (f64::from(axis[0]) * f64::from(axis[0]) + f64::from(axis[1]) * f64::from(axis[1]))
        + f64::from(axis[2]) * f64::from(axis[2]);
    let inverse = 1.0f64 / sum.sqrt();
    g.writef(owner + 0xD4, (inverse * f64::from(axis[0])) as f32);
    let owner = g.read(entry + 0xA8);
    g.writef(owner + 0xD8, (inverse * f64::from(axis[1])) as f32);
    let owner = g.read(entry + 0xA8);
    g.writef(owner + 0xDC, (inverse * f64::from(axis[2])) as f32);
}

/// The velocity reflection of a bounce (`v + d n` with `d = -2 (v . n)`), renormalised when longer than 1e-3.5, the bounce counter `+0x198` and the
/// bounce sounds `0x217B0(0xD34 + count - 1, position)` for the first four.
fn reflect_debris(g: &mut dyn Guest, entry: u32, normal: u32, pos_pointer: u32) {
    let n = g.vec3(normal);
    let v = g.vec3(entry + 0x64);
    let d = ((v[2] * n[2]) + (v[0] * n[0]) + v[1] * n[1]) * f(0xC000_0000);
    let vx = d * n[0] + v[0];
    let vz = d * n[2] + v[2];
    let vy = d * n[1] + v[1];
    g.writef(entry + 0x6C, vz);
    g.writef(entry + 0x68, vy);
    let count = g.read(entry + 0x198).wrapping_add(1);
    let len2 = (vz * vz + vy * vy) + vx * vx;
    g.write(entry + 0x198, count);
    g.writef(entry + 0x64, vx);
    if len2 > f(0x33D6_BF95) {
        let inverse = (1.0f32 / len2.sqrt()) * 1.0;
        g.writef(frame(0x1AC), len2);
        g.writef(frame(0x1B0), 1.0);
        g.writef(frame(0x1A8), inverse);
        let x = g.readf(entry + 0x64);
        g.writef(entry + 0x64, x * inverse);
        let y = g.readf(entry + 0x68);
        g.writef(entry + 0x68, y * inverse);
        let z = g.readf(entry + 0x6C);
        g.writef(entry + 0x6C, z * inverse);
    }
    let count = g.read(entry + 0x198);
    let index = count.wrapping_sub(1);
    if index <= 3 {
        g.call(0x217B0, &[0xD34 + index, pos_pointer]);
    }
}

/// The tail every `0xF2B80` path shares (`0xF3576`): type 0xC keeps a sound handle, type 0xD with bounce count 1 throws two debris entries
/// (`0xEA9C0`) unless it struck an actor, the owner spin decays and wraps, and old entries in mode 10 call `0x26C9A0`.
fn debris_tail(g: &mut dyn Guest, entry: u32) {
    let pos_pointer = entry + 0x1C;
    let (e_target, e_step, e_vec, e_diff, e_hit, e_normal) = (
        frame(0x194),
        frame(0x188),
        frame(0x1A0),
        frame(0x170),
        frame(0x158),
        frame(0x17C),
    );
    let handle_slot = entry + 0xAC;
    if g.read(entry) == 0xC {
        let handle = g.read(handle_slot);
        let mut restart = true;
        if handle != 0 {
            let alive = g.call(0xFF4B0, &[handle]);
            if alive == 0 {
                let handle = g.read(handle_slot);
                if handle != 0 {
                    g.call(0xFF490, &[handle, pos_pointer]);
                }
                restart = false;
            }
        }
        if restart {
            let handle = g.read(handle_slot);
            if handle != 0 {
                g.call(0xFF420, &[handle, 0]);
                let handle = g.read(handle_slot);
                g.write(handle + 0x14, 0);
                g.write(handle_slot, 0);
            }
            let level = g.call(0x1CDAE0, &[]);
            let option = if level.wrapping_sub(1) != 0 {
                0x8_0000
            } else {
                0
            };
            let created = g.call(0xFFF70, &[handle_slot, 3, option]);
            g.write(handle_slot, created);
            g.call(0xFF420, &[created, 0x41F0_0000]);
        }
    }
    let target = g.read(e_target);
    if g.read(entry) == 0xD && target != 0 && g.read(entry + 0x198) == 1 {
        let words = words3(g, pos_pointer);
        for (index, word) in words.into_iter().enumerate() {
            g.write(e_step + 4 * index as u32, word);
        }
        let source = g.read(entry + 0x108);
        let player = g.read(entry + 0x104);
        let half = 0.5f64;
        let scatter = |g: &mut dyn Guest, base: u32| {
            for index in 0..3u32 {
                let unit = draw_unit(g);
                let component = g.readf(entry + 0x64 + 4 * index);
                let value = (((unit + 1.0) * f64::from(component)) * half) as f32;
                g.writef(base + 4 * index, value);
            }
            g.call(0xE33A0, &[base]);
        };
        scatter(g, e_vec);
        scatter(g, e_diff);
        let flags = g.read(entry + 0xC);
        let mode = g.read(entry + 0x100);
        let word8 = g.read(entry + 8);
        let size = g.readf(entry + 0x88);
        let cell = g.read(entry + 0xF0);
        g.write(frame(0x1AC), flags);
        g.write(frame(0x1A8), mode);
        g.write(frame(0x1B0), word8);
        g.writef(frame(0x1A4), size);
        if g.read(target + 0x20) == 8 {
            g.call(0xF2AB0, &[e_hit, e_normal, entry, target]);
            g.call(0xDC550, &[entry]);
            return;
        }
        for vector in [e_vec, e_diff] {
            let child = g.call(
                0xEA9C0,
                &[
                    0xD,
                    e_step,
                    vector,
                    size.to_bits(),
                    source,
                    player,
                    mode,
                    0,
                    cell,
                    flags,
                    word8,
                    0,
                ],
            );
            if child != 0 {
                g.write(child + 0x198, 1);
                g.write(child + 0x14C, 0);
            }
        }
    }
    let owner = g.read(entry + 0xA8);
    if owner != 0 {
        let spin = g.readf(owner + 0xD0);
        if spin > f(0) {
            let decayed = spin * f(0x3F7D_70A4);
            g.writef(owner + 0xD0, decayed);
            let owner = g.read(entry + 0xA8);
            let spin = g.readf(owner + 0xD0);
            let angle = spin * g.readf(STEP_SCALE) + g.readf(owner + 0xCC);
            g.writef(owner + 0xCC, angle);
            let owner = g.read(entry + 0xA8);
            let limit = f(0x43B4_0000);
            if g.readf(owner + 0xCC) > limit {
                loop {
                    let owner = g.read(entry + 0xA8);
                    let value = g.readf(owner + 0xCC) - limit;
                    g.writef(owner + 0xCC, value);
                    let owner = g.read(entry + 0xA8);
                    if !(g.readf(owner + 0xCC) > limit) {
                        break;
                    }
                }
            }
        }
    }
    if g.byte(super::eff_helpers::MODE_BYTE) == 0xA && g.readf(entry + 0x98) > f(0x3DCC_CCCD) {
        let target = g.read(e_target);
        g.call(0x26C9A0, &[entry, target]);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// 0xF39A0
// ---------------------------------------------------------------------------------------------------------------------------------

use super::eff_guest::{sbyte, visual_rng_int};

/// The sound request of `0xF39A0` type 0x12: `0x45390(request)`, position, flag byte and id, `0x45820(request)` (the tick word stays as
/// `0x45390` left it).
fn sound_request_no_ticks(g: &mut dyn Guest, request: u32, id: u32, position: [f32; 3]) {
    g.call(0x45390, &[request]);
    g.set_vec3(request + 0x1C, position);
    write_byte(g, request + 0x28, 1);
    g.write(request, id);
    g.call(0x45820, &[request]);
}

/// The end of a stuck entry (`0xF3ACD`): the usercall `0xF0430` (esi = entry, ebx = position) starts the final effect and releases.
fn finish_stuck(g: &mut dyn Guest, entry: u32) {
    g.call(0xF0430, &[entry, entry + 0x1C]);
}

/// `0xF3AB0`: type 0x13 returns its sound handle `+0x1B0` (`0x45460`, then -1) before the end.
fn release_arrow_handle(g: &mut dyn Guest, entry: u32) {
    let handle = g.read(entry + 0x1B0) as i32;
    if handle >= 0 {
        g.call(0x45460, &[handle as u32]);
        g.write(entry + 0x1B0, u32::MAX);
    }
}

/// `atan2` of the x87 `fpatan` (double), the angle scaled by `57.29578` (`[0x478638]`) and rounded to f32 at the store.
fn degrees(y: f32, x: f32) -> f32 {
    (f64::from(y).atan2(f64::from(x)) * f64::from(f(0x4265_2EE0))) as f32
}

/// The owner angles from the stuck direction: yaw `atan2(d.z, d.y)` into `+0xC0`, pitch `-atan2(d.x, sqrt(d.z^2 + d.y^2))` into `+0xC8`, spin `+0x70 = 0`.
fn aim_owner(g: &mut dyn Guest, owner: u32, d: [f32; 3]) {
    g.writef(owner + 0xC0, degrees(d[2], d[1]));
    let flat = (f64::from(d[2]) * f64::from(d[2]) + f64::from(d[1]) * f64::from(d[1])).sqrt();
    let pitch = -(f64::from(d[0]).atan2(flat) * f64::from(f(0x4265_2EE0)));
    g.writef(owner + 0xC8, pitch as f32);
    g.writef(owner + 0x70, 0.0);
}

/// Copy the owner's child words (`+0x38 ..`, signed byte count `+0x30`) from `source` to `target`.
fn copy_owner_words(g: &mut dyn Guest, source: u32, target: u32) {
    let mut index = 0i32;
    if sbyte(g, source + 0x30) > 0 {
        loop {
            let word = g.read(source + 0x38 + 4 * index as u32);
            g.write(target + 0x38 + 4 * index as u32, word);
            index += 1;
            if index >= sbyte(g, source + 0x30) {
                break;
            }
        }
    }
}

/// `0xF39A0(entry)`: the update of the sticking projectiles (types 0xE, 0x10, 0x11, 0x12, 0x13). Names INFERRED, flow MEASURED.
///
/// * `owner` (`+0xA8`) gets flag 2 of `+0x18`. While the frame steps are 0 only the tail runs.
/// * Retirement: type 0x11 at timer 6.0, type 0x13 at `+0x17C` (returning its sound handle), type 0xE at 2.4 (`0xDC2B0`), any at the
///   accumulated `+0xA4 >= 6.0`; then `0xF0430`.
/// * A flying entry (size `> 0`) moves `velocity * dt` swept with `0xDCB80`; type 0x13 keeps a looping sound (`0x21BB0` / `0x45610`).
///   A hit: type 0x13 recreates its owner object (`0xA6F90`, `0xA9070`), `0x217B0(0xD42)`, and by the hit object: type 2 `0x1609F0`; an
///   actor attaches (`0xD9760`, the bone `+0x15C` from `0xD6A80` and the bone table of the player record, `0x160020`); anything else
///   attaches to the hit object with the bone of the contact template. The entry is then pinned: position `hit + normal * 0.015`, normal kept at
///   `+0x180`, velocity and size 0, the owner follows and is aimed; attached entries store the hit position and normal in the object's
///   frame (`+0x160`, `+0x16C`) through the matrix routines (`0x151650`, `0x152260`, `0x151E80`, `0x1531D0`).
/// * A pinned entry (size 0) follows its object each frame through the same matrices (`0x151790` / `0x1534E0`).
/// * Tail: type 0x12 `0xF04B0`; type 0x11 plays two sounds and flags the owner by age; type 0x12 plays `0xD41` every half second;
///   type 0xE throws sparks (`0x153BB0`, `0x153BD0`, `0xF85C0`); in mode byte 10 old entries call `0x26C9A0`.
pub fn stuck_update(g: &mut dyn Guest, entry: u32) {
    let (e_scalar, e_dt, e_mat, e_step, e_normal, e_hit, e_record, e_ray) = (
        frame(0x1C0),
        frame(0x1BC),
        frame(0x1AC),
        frame(0x1CC),
        frame(0x1D8),
        frame(0x1E4),
        frame(0x168),
        frame(0x10C),
    );
    let pos_pointer = entry + 0x1C;
    let mut scalar = f(0xC1F0_0000);
    if g.read(entry) == 0xE {
        scalar = f(0xC1D8_0000);
    }
    g.writef(e_scalar, scalar);
    let owner = g.read(entry + 0xA8);
    if owner != 0 {
        let v = g.read(owner + 0x18);
        g.write(owner + 0x18, v | 2);
    }
    if (g.read(STEPS) as i32) < 1 {
        stuck_tail_core(g, entry);
        return;
    }
    let entry_type = g.read(entry);
    let limit = f(0x40C0_0000);
    let timer = g.readf(entry + 0x98);
    if entry_type == 0x11 && timer >= limit {
        finish_stuck(g, entry);
        return;
    }
    if entry_type == 0x13 && timer >= g.readf(entry + 0x17C) {
        release_arrow_handle(g, entry);
        finish_stuck(g, entry);
        return;
    }
    if entry_type == 0xE && timer >= f(0x4019_999A) {
        g.call(0xDC2B0, &[entry, 0]);
        finish_stuck(g, entry);
        return;
    }
    let size = g.readf(entry + 0x88);
    if !(size > 0.0) {
        follow_object(g, entry);
        stuck_tail(g, entry);
        return;
    }
    let rate = g.read(FRAME_RATE) as i32 as f32;
    let per_tick = 1.0f32 / rate;
    let scale = g.readf(STEP_SCALE);
    let span = size * per_tick;
    let clock = per_tick * scale + g.readf(entry + 0xA4);
    let dt = span * scale;
    g.writef(e_dt, dt);
    g.writef(entry + 0xA4, clock);
    if !(clock < limit) {
        g.call(0xDC2B0, &[entry, 0]);
        if g.read(entry) == 0x13 {
            release_arrow_handle(g, entry);
        }
        finish_stuck(g, entry);
        return;
    }
    let velocity = g.vec3(entry + 0x64);
    g.set_vec3(
        e_step,
        [velocity[0] * dt, velocity[1] * dt, velocity[2] * dt],
    );
    if entry_type == 0x13 {
        let handle = g.read(entry + 0x1B0) as i32;
        if handle < 0 {
            let created = g.call(0x21BB0, &[0xD3E, 0x4170_0000, pos_pointer]);
            g.write(entry + 0x1B0, created);
        } else {
            g.call(0x45610, &[handle as u32, pos_pointer]);
        }
    }
    g.write(e_ray, 0);
    g.write(e_ray + 4, 0);
    let target = g.call(0xDCB80, &[e_step, e_hit, e_normal, e_ray, entry]);
    if (g.read(e_ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, e_ray]);
    }
    if target == 0 {
        no_hit_advance(g, entry);
        stuck_tail(g, entry);
        return;
    }
    if g.read(entry) == 0x13 {
        let old = g.read(entry + 0xA8);
        g.set_vec3(frame(0x1B8), [0.0, 0.0, 0.0]);
        let created = g.call(0xA6F90, &[0x715, old + 0x64, frame(0x1B8), 0, 0]);
        g.write(entry + 0xA8, created);
        write_byte(g, created + 0x32, 2);
        let flags = g.read(created + 0x28);
        g.write(created + 0x28, flags & 0xFFFF_FF9F);
        let cell = g.read(old + 0x34);
        g.write(created + 0x34, cell);
        let count = g.byte(old + 0x30);
        write_byte(g, created + 0x30, count);
        copy_owner_words(g, old, created);
        g.call(0xA9070, &[old]);
    }
    let entry_type = g.read(entry);
    if entry_type == 0x12 || entry_type == 0x10 {
        let object = g.at(entry + 0xA8, 4);
        let v = g.read(object + 0x30);
        g.write(object + 0x30, v | 1);
    }
    if g.read(entry) != 0xE {
        g.call(0x217B0, &[0xD42, pos_pointer]);
    }
    let target_type = g.read(target + 0x20);
    let player_base = |g: &mut dyn Guest| {
        let table = g.at(target + 4, 4);
        let first = g.read(table);
        table.wrapping_sub(first.wrapping_mul(0xC8))
    };
    match target_type {
        2 => {
            g.write(entry + 0x158, 0);
            g.write(entry + 0x15C, u32::MAX);
            g.call(0x1609F0, &[entry]);
            g.write(entry + 0x154, 0xE_7BE0);
        }
        8 => {
            let base = player_base(g);
            g.call(0xDC5D0, &[entry]);
            if g.read(entry + 0x104) == 0 {
                no_hit_advance(g, entry);
                stuck_tail(g, entry);
                return;
            }
            if g.call(0x3533F0, &[]) != 0 {
                let slot = g.at(entry + 0x104, 8);
                let table = g.read(0x007B_0C48);
                if (g.read(
                    table
                        .wrapping_add(slot.wrapping_mul(0x1584))
                        .wrapping_add(4),
                ) as i32)
                    < 0
                {
                    no_hit_advance(g, entry);
                    stuck_tail(g, entry);
                    return;
                }
            }
            let n = g.vec3(e_normal);
            let h = g.vec3(e_hit);
            g.set_vec3(pos_pointer, [n[0] + h[0], n[1] + h[1], n[2] + h[2]]);
            let reverse = f(0xBF80_0000);
            g.set_vec3(e_step, [n[0] * reverse, n[1] * reverse, n[2] * reverse]);
            g.call(
                0xD9760,
                &[target, pos_pointer, e_step, 0, 1, e_hit, e_normal, 0],
            );
            g.write(entry + 0x158, target);
            let index = g.call(0xD6A80, &[]);
            let bone = sbyte(
                g,
                index
                    .wrapping_mul(0xC8)
                    .wrapping_add(base)
                    .wrapping_add(0x49),
            );
            g.write(entry + 0x15C, bone as u32);
            let flags = g.read(target + 0x28);
            g.write(target + 0x28, flags | 0x1_0000);
            if (g.read(entry + 0x15C) as i32) < 0 {
                let index = g.call(0xD6A80, &[]);
                if sbyte(
                    g,
                    index
                        .wrapping_mul(0xC8)
                        .wrapping_add(base)
                        .wrapping_add(0x4A),
                ) >= 0
                {
                    let index = g.call(0xD6A80, &[]);
                    let parent = sbyte(
                        g,
                        index
                            .wrapping_mul(0xC8)
                            .wrapping_add(base)
                            .wrapping_add(0x4A),
                    );
                    let bone = sbyte(
                        g,
                        (parent as u32)
                            .wrapping_mul(0xC8)
                            .wrapping_add(base)
                            .wrapping_add(0x49),
                    );
                    g.write(entry + 0x15C, bone as u32);
                }
                if (g.read(entry + 0x15C) as i32) < 0 {
                    let index = g.call(0xD6A80, &[]);
                    if sbyte(
                        g,
                        index
                            .wrapping_mul(0xC8)
                            .wrapping_add(base)
                            .wrapping_add(0x4B),
                    ) >= 0
                    {
                        let index = g.call(0xD6A80, &[]);
                        let parent = sbyte(
                            g,
                            index
                                .wrapping_mul(0xC8)
                                .wrapping_add(base)
                                .wrapping_add(0x4B),
                        );
                        let bone = sbyte(
                            g,
                            (parent as u32)
                                .wrapping_mul(0xC8)
                                .wrapping_add(base)
                                .wrapping_add(0x49),
                        );
                        g.write(entry + 0x15C, bone as u32);
                    }
                }
            }
            if g.read(target + 0x28) & 0x20_0000 == 0 {
                g.write(entry + 0x15C, u32::MAX);
            }
            if g.read(entry + 0x104) != 0 {
                let index = g.at(target + 0x7C, 8);
                if (index as i32) >= (g.read(PLAYER_COUNT_FIRST) as i32) {
                    g.call(0xB91F0, &[e_record]);
                    g.call(0xB9220, &[e_record, entry]);
                    g.call(0x160020, &[target, 0, e_record]);
                }
            }
        }
        _ => {
            let base = player_base(g);
            g.copy(super::eff_helpers::CONTACT_LIVE, e_mat, 8);
            let surface = g.read(e_mat + 4);
            g.write(entry + 0x158, target);
            if (surface as i32) >= 0 {
                let bone = sbyte(
                    g,
                    surface
                        .wrapping_mul(0xC8)
                        .wrapping_add(base)
                        .wrapping_add(0x49),
                );
                g.write(entry + 0x15C, bone as u32);
            }
            let flags = g.read(target + 0x28);
            g.write(target + 0x28, flags | 0x1_0000);
        }
    }
    // pin the entry
    let owner = g.read(entry + 0xA8);
    let mut normal = g.vec3(e_normal);
    if owner != 0 {
        aim_owner(g, owner, [normal[0], normal[1], normal[2]]);
    }
    let mut hit = g.vec3(e_hit);
    if g.read(entry + 0x158) != 0 {
        let radians = f(0x3C8E_FA35);
        if g.byte(target + 0x32) == 2 {
            let a = g.readf(target + 0xC0) * radians;
            let b = g.readf(target + 0x70) * radians;
            let c = g.readf(target + 0xC8) * radians;
            g.call(0x151650, &[e_mat, a.to_bits(), b.to_bits(), c.to_bits()]);
        } else {
            let a = g.readf(target + 0x70) * radians;
            g.call(0x153440, &[e_mat, a.to_bits()]);
        }
        let motion = g.vec3(target + 0x64);
        let current = g.vec3(e_hit);
        g.set_vec3(
            e_hit,
            [
                current[0] - motion[0],
                current[1] - motion[1],
                current[2] - motion[2],
            ],
        );
        g.call(0x152260, &[e_record, e_mat]);
        g.call(0x151E80, &[e_record, e_hit]);
        g.call(0x151E80, &[e_record, e_normal]);
        let bone = g.read(entry + 0x15C);
        if (bone as i32) >= 0 {
            let part = g.at(target + 4, 0x10).wrapping_add(bone << 6);
            g.call(0x1531D0, &[e_mat, part]);
            let shift = g.vec3(e_mat + 0x30);
            let current = g.vec3(e_hit);
            g.set_vec3(
                e_hit,
                [
                    current[0] - shift[0],
                    current[1] - shift[1],
                    current[2] - shift[2],
                ],
            );
            g.set_vec3(e_mat + 0x30, [0.0, 0.0, 0.0]);
            g.call(0x152260, &[e_record, e_mat]);
            g.call(0x151E80, &[e_record, e_hit]);
            g.call(0x151E80, &[e_record, e_normal]);
        }
        hit = g.vec3(e_hit);
        normal = g.vec3(e_normal);
        let len2 = (normal[2] * normal[2] + normal[1] * normal[1]) + normal[0] * normal[0];
        if len2 > f(0x33D6_BF95) {
            g.writef(frame(0x16C), len2);
            g.writef(e_scalar, 1.0);
            let inverse = (1.0f32 / len2.sqrt()) * 1.0;
            g.writef(e_dt, inverse);
            normal = [
                inverse * normal[0],
                inverse * normal[1],
                inverse * normal[2],
            ];
            g.set_vec3(e_normal, normal);
        }
        g.set_vec3(entry + 0x160, hit);
        g.set_vec3(entry + 0x16C, normal);
    }
    let nudge = f(0x3C75_C28F);
    let fixed = [
        normal[0] * nudge + hit[0],
        normal[1] * nudge + hit[1],
        normal[2] * nudge + hit[2],
    ];
    g.writef(entry + 0x20, fixed[1]);
    g.writef(pos_pointer, fixed[0]);
    g.writef(entry + 0x24, fixed[2]);
    g.set_vec3(entry + 0x180, normal);
    g.writef(entry + 0x64, 0.0);
    g.writef(entry + 0x68, 0.0);
    g.writef(entry + 0x6C, 0.0);
    g.writef(entry + 0x88, 0.0);
    if owner != 0 {
        g.writef(owner + 0x64, fixed[0]);
        let word = g.read(entry + 0x20);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x68, word);
        let word = g.read(entry + 0x24);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x6C, word);
    }
    if g.read(target + 0x20) == 8 {
        let index = g.at(target + 0x7C, 8);
        let bone = g.read(entry + 0x15C);
        g.call(0x357F70, &[index, entry, bone, pos_pointer]);
    }
    stuck_tail(g, entry);
}

/// A step that hit nothing: advance, `+0xF4 += dt^2`, `0xDC6F0(entry; E-0x1C0)`, size x0.99 per step, owner follows (spin +360 per second).
fn no_hit_advance(g: &mut dyn Guest, entry: u32) {
    let pos_pointer = entry + 0x1C;
    let step = g.vec3(frame(0x1CC));
    let here = g.vec3(pos_pointer);
    g.set_vec3(
        pos_pointer,
        [here[0] + step[0], here[1] + step[1], here[2] + step[2]],
    );
    let scalar = g.read(frame(0x1C0));
    let dt = g.readf(frame(0x1BC));
    let travelled = g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, dt * dt + travelled);
    g.call(0xDC6F0, &[scalar, entry]);
    if (g.read(STEPS) as i32) > 0 {
        let mut count = 0i32;
        loop {
            let size = g.readf(entry + 0x88);
            g.writef(entry + 0x88, size * f(0x3F7D_70A4));
            count += 1;
            if count >= g.read(STEPS) as i32 {
                break;
            }
        }
    }
    let owner = g.read(entry + 0xA8);
    if owner != 0 {
        let word = g.read(pos_pointer);
        g.write(owner + 0x64, word);
        let word = g.read(entry + 0x20);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x68, word);
        let word = g.read(entry + 0x24);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x6C, word);
        if g.read(entry) != 0x13 {
            let rate = g.read(FRAME_RATE) as i32 as f32;
            let mut spin = 1.0f32 / rate;
            spin *= g.readf(STEP_SCALE);
            spin *= f(0x43B4_0000);
            let owner = g.read(entry + 0xA8);
            let value = spin + g.readf(owner + 0x70);
            g.writef(owner + 0x70, value);
        }
    }
}

/// A pinned entry (size 0) follows the object it stuck to: its stored offset and normal go through the object's matrices every frame.
fn follow_object(g: &mut dyn Guest, entry: u32) {
    let object = g.read(entry + 0x158);
    if object == 0 {
        return;
    }
    let (e_mat, e_offset, e_axis) = (frame(0x1AC), frame(0x1CC), frame(0x1E4));
    let offset = g.vec3(entry + 0x160);
    g.set_vec3(e_offset, offset);
    let bone = g.read(entry + 0x15C);
    let axis = g.vec3(entry + 0x16C);
    g.set_vec3(e_axis, axis);
    if (bone as i32) >= 0 {
        let part = g.at(object + 4, 0x10).wrapping_add(bone << 6);
        g.call(0x1531D0, &[e_mat, part]);
        g.call(0x151E80, &[e_mat, e_offset]);
        g.set_vec3(e_mat + 0x30, [0.0, 0.0, 0.0]);
        g.call(0x151E80, &[e_mat, e_axis]);
    }
    let radians = f(0x3C8E_FA35);
    let position = g.vec3(object + 0x64);
    if g.byte(object + 0x32) == 2 {
        let c8 = (g.readf(object + 0xC8) * radians).to_bits();
        let w70 = (g.readf(object + 0x70) * radians).to_bits();
        let c0 = (g.readf(object + 0xC0) * radians).to_bits();
        g.call(
            0x151790,
            &[
                e_mat,
                position[0].to_bits(),
                position[1].to_bits(),
                position[2].to_bits(),
                c0,
                w70,
                c8,
            ],
        );
    } else {
        let w70 = (g.readf(object + 0x70) * radians).to_bits();
        g.call(
            0x1534E0,
            &[
                e_mat,
                position[0].to_bits(),
                position[1].to_bits(),
                position[2].to_bits(),
                w70,
            ],
        );
    }
    g.call(0x151E80, &[e_mat, e_offset]);
    g.set_vec3(e_mat + 0x30, [0.0, 0.0, 0.0]);
    g.call(0x151E80, &[e_mat, e_axis]);
    let owner = g.read(entry + 0xA8);
    let o = g.vec3(e_offset);
    let a = g.vec3(e_axis);
    g.writef(entry + 0x20, o[1]);
    g.writef(entry + 0x24, o[2]);
    g.writef(entry + 0x180, a[0]);
    g.writef(entry + 0x184, a[1]);
    g.writef(entry + 0x1C, o[0]);
    g.writef(entry + 0x188, a[2]);
    if owner != 0 {
        g.writef(owner + 0x64, o[0]);
        let word = g.read(entry + 0x20);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x68, word);
        let word = g.read(entry + 0x24);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x6C, word);
        let owner = g.read(entry + 0xA8);
        g.writef(owner + 0xC0, degrees(a[2], a[1]));
        let flat = (f64::from(a[2]) * f64::from(a[2]) + f64::from(a[1]) * f64::from(a[1])).sqrt();
        let owner = g.read(entry + 0xA8);
        g.writef(
            owner + 0xC8,
            (-(f64::from(a[0]).atan2(flat) * f64::from(f(0x4265_2EE0)))) as f32,
        );
        let owner = g.read(entry + 0xA8);
        g.writef(owner + 0x70, 0.0);
        let owner = g.read(entry + 0xA8);
        let cell = g.read(object + 0x34);
        g.write(owner + 0x34, cell);
        let owner = g.read(entry + 0xA8);
        let count = g.byte(object + 0x30);
        write_byte(g, owner + 0x30, count);
        copy_owner_words(g, object, owner);
    }
}

/// The tail after a step (`0xF4608`): type 0x12 calls `0xF04B0(entry, source)`, then [`stuck_tail_core`].
fn stuck_tail(g: &mut dyn Guest, entry: u32) {
    if g.read(entry) == 0x12 {
        let source = g.read(entry + 0x10C);
        g.call(0xF04B0, &[entry, source]);
    }
    stuck_tail_core(g, entry);
}

/// `0xF4608` and `0xF461D`. NOTE the entry points: `0xF4608` runs the type 0x12 call, `0xF461D` (the steps-0 path) skips it; see [`stuck_update`].
fn stuck_tail_core(g: &mut dyn Guest, entry: u32) {
    let pos_pointer = entry + 0x1C;
    if g.read(entry) == 0x11 {
        let timer2 = g.readf(entry + 0x98) * 2.0;
        let counter = g.read(entry + 0x1B4);
        let late = timer2 - f(0x3F40_0000);
        let early = timer2 - 1.0f32;
        let counter_f = counter as i32 as f32;
        if early > counter_f {
            let counter = counter.wrapping_add(1);
            g.write(entry + 0x1B4, counter);
            if (counter as i32) < 10 {
                g.call(0x217B0, &[0xD3F, pos_pointer]);
                let object = g.at(entry + 0xA8, 4);
                let v = g.read(object + 0x30);
                g.write(object + 0x30, v | 6);
            }
            if g.read(entry + 0x1B4) == 10 {
                g.call(0x217B0, &[0xD40, pos_pointer]);
            }
        } else if late > counter_f {
            let object = g.at(entry + 0xA8, 4);
            g.write(object + 0x30, 0);
        }
    }
    if g.read(entry) == 0x12 {
        let ticks = (g.readf(entry + 0x98) * f(0x3F00_0000)) as i32;
        let counter = g.read(entry + 0x1B4) as i32;
        if ticks > counter {
            let position = g.vec3(pos_pointer);
            sound_request_no_ticks(g, frame(0x1AC), 0xD41, position);
            let counter = g.read(entry + 0x1B4).wrapping_add(1);
            g.write(entry + 0x1B4, counter);
        }
    }
    if g.read(entry) == 0xE {
        let bit = visual_rng_int(g);
        let v = g.vec3(entry + 0x64);
        let skip = bit & 1 != 0 && v[0] == 0.0 && v[1] == 0.0 && v[2] == 0.0;
        if !skip {
            let vec = frame(0x1B8);
            let half = f64::from(f(0x3F00_0000));
            let k03 = f64::from(f(0x3E99_999A));
            let k06 = f64::from(f(0x3F19_999A));
            for (index, slot) in [0x180u32, 0x184, 0x188].into_iter().enumerate() {
                let drawn = (visual_rng(g) - half) * k03;
                let n = g.readf(entry + slot);
                let value = (drawn + f64::from(n) * k06) as f32;
                g.writef(vec + 4 * index as u32, value);
            }
            g.call(
                0xF85C0,
                &[10, 0xD5, pos_pointer, vec, 0x3F80_0000, 0, 0x3F4C_CCCD],
            );
        }
    }
    if g.byte(super::eff_helpers::MODE_BYTE) == 0xA && g.readf(entry + 0x98) > f(0x3DCC_CCCD) {
        g.call(0x26C9A0, &[entry, 0]);
    }
}
