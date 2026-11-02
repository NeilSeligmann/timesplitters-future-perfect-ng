//! T1876: the remaining effect pool update callbacks (class table `0x4DEB28`, slot `+0x34`), ported from the original and validated bit for bit
//! against it under the Unicorn oracle (`tools/t1846_gold/eff_update2.py`, tests `tests/story_events_t1876_update.rs`). Document
//! `docs/t-story-event-effect-update-t1876.md`. Behaviour MEASURED (xemu level), names INFERRED. Same [`Guest`] method and frame rule as
//! `eff_update` (locals live at the same addresses relative to the entry esp as in the original, so records that carry pointers to locals compare equal).

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::eff_guest::Guest;
use super::eff_update::FRAME;

/// `cvttss2si`: truncate, out of range and NaN give the "integer indefinite" value.
pub fn cvtt(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// The player block table `[0x7B0C48]`, `0x1584` bytes per player.
const PLAYER_BLOCKS: u32 = 0x007B_0C48;
const PLAYER_BLOCK_BYTES: u32 = 0x1584;
/// `[0x7B0CC0]`: the camera / view object, `+0x214` the bit index of the local viewer.
const VIEW: u32 = 0x007B_0CC0;

/// `esp` at entry of `0xF5D30` called from `0xF5FF0` (one pushed argument): `0xC + 4 * 4 + 4` below the callback's entry esp.
const BEAM_TRACE_FRAME: u32 = FRAME - 0x20;

fn beam_local(offset: u32) -> u32 {
    BEAM_TRACE_FRAME - offset
}

/// `0xF5D30(entry in eax)`: the end point of a beam entry (types `0x2F` and kin). Names INFERRED, flow MEASURED.
///
/// 1. `step = [+0x180 .. +0x188] * 320.0`; `0xDCB80(step, hit point +0x1A0, normal, ray record, entry)`; a positive record count starts the light
///    `0xDCEF0`.
/// 2. Type `0x2F` with a hit, an attached object (`+0x104`, mode `+0x100 == 1`): the viewer's player block (`[0x7B0C48] + [attached + 8] * 0x1584`,
///    its target `+0xF84`) replaces the ray result when `0xE3680` (`beam_viewer_ready`) holds and that target is an actor other than the hit: the hit point and
///    the normal come from the block (`+0xF88`, `+0xF94`).
/// 3. A hit on an actor runs `0xDC5D0` and the sound `0x46AB0(0xDAF, 50.0, [0x7E71F0], hit)`; the reaction `0xE9FE0` runs when `0xE3680` does not hold or
///    the target is an actor; type `0x2F` riding a player block with a non negative word `+4` calls `0xFC990(word, hit, normal)`.
/// 4. A light record (`0xB91F0` + `0xB9220` + fields) and `0xBD0F0`, then the ground effect `0xF4920(hit, 0.3, owner)`.
/// 5. No hit: the end point is `position + step`.
pub fn beam_trace(g: &mut dyn Guest, entry: u32) {
    let (normal, step, ray, record) = (
        beam_local(0x180),
        beam_local(0x174),
        beam_local(0x10C),
        beam_local(0x168),
    );
    let scale = f32::from_bits(0x43A0_0000);
    let direction = g.vec3(entry + 0x180);
    g.set_vec3(
        step,
        [
            direction[0] * scale,
            direction[1] * scale,
            direction[2] * scale,
        ],
    );
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let mut target = g.call(0xDCB80, &[step, entry + 0x1A0, normal, ray, entry]);
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    let beam_type = g.read(entry);
    if beam_type == 0x2F {
        if target == 0 {
            return beam_no_hit(g, entry, step);
        }
        let attached = g.read(entry + 0x104);
        if attached != 0 && g.read(entry + 0x100) == 1 {
            let block = g
                .read(PLAYER_BLOCKS)
                .wrapping_add(g.read(attached + 8).wrapping_mul(PLAYER_BLOCK_BYTES));
            let viewed = g.read(block + 0xF84);
            let ready = beam_viewer_ready(g, entry);
            if ready && viewed != 0 && target != viewed && g.read(viewed + 0x20) == 8 {
                g.copy(block + 0xF88, entry + 0x1A0, 3);
                g.copy(block + 0xF94, normal, 3);
                target = viewed;
            }
        }
    }
    if target == 0 {
        return beam_no_hit(g, entry, step);
    }
    if g.read(target + 0x20) == 8 {
        g.call(0xDC5D0, &[entry]);
        let sound_object = g.read(0x007E_71F0);
        g.call(0x46AB0, &[0xDAF, 0x4220_0000, sound_object, entry + 0x1A0]);
    }
    let ready = beam_viewer_ready(g, entry);
    if !ready || g.read(target + 0x20) == 8 {
        let beam_type = g.read(entry);
        let source = g.read(entry + 0x108);
        g.call(
            0xE9FE0,
            &[
                entry,
                beam_type,
                target,
                entry + 0x1A0,
                normal,
                normal,
                source,
            ],
        );
    }
    if g.read(entry) == 0x2F && g.read(entry + 0x100) == 1 {
        let attached = g.read(entry + 0x104);
        if attached != 0 {
            let object = g.read(attached);
            if object != 0 {
                let block = g.read(object + 0x150);
                if block != 0 {
                    let word = g.read(block + 4);
                    if (word as i32) >= 0 {
                        g.call(0xFC990, &[word, entry + 0x1A0, normal]);
                    }
                }
            }
        }
    }
    g.call(0xB91F0, &[record]);
    g.call(0xB9220, &[record, entry]);
    g.write(record, target);
    g.write(record + 0x28, entry + 0x1A0);
    g.write(record + 0x2C, normal);
    g.write(record + 0x30, entry + 0x180);
    g.write(record + 0x34, 0);
    g.call(0xBD0F0, &[record]);
    let attached = g.read(entry + 0x104);
    let owner = if attached != 0 { g.read(attached) } else { 0 };
    g.call(0xF4920, &[entry + 0x1A0, 0x3E99_999A, owner]);
}

/// `0xE3680(entry in esi)`: whether a type `0x2F` beam is held by the local viewer: it needs the attached player (`+0x104`, mode `+0x100 == 1`), `0x64D10()` zero and
/// the player block word `+0x14F8` equal to `0x11`.
pub fn beam_viewer_ready(g: &mut dyn Guest, entry: u32) -> bool {
    if g.read(entry) != 0x2F {
        return false;
    }
    let attached = g.read(entry + 0x104);
    if attached == 0 || g.read(entry + 0x100) != 1 {
        return false;
    }
    if g.call(0x64D10, &[]) != 0 {
        return false;
    }
    let index = g.read(attached + 8);
    let block = g
        .read(PLAYER_BLOCKS)
        .wrapping_add(index.wrapping_mul(PLAYER_BLOCK_BYTES));
    g.read(block.wrapping_add(0x14F8)) == 0x11
}

fn beam_no_hit(g: &mut dyn Guest, entry: u32, step: u32) {
    for axis in 0..3u32 {
        let value = g.readf(entry + 0x1C + 4 * axis) + g.readf(step + 4 * axis);
        g.writef(entry + 0x1A0 + 4 * axis, value);
    }
}

/// `0xF5FF0(entry)`: update of beam entries (types `0x2F` and kin). Without the live word `+0x14C` the entry is released (`0xDC550`). Otherwise the
/// end point is traced (`0xF5D30`), `+0xF4` takes the squared length `end - position` (summation order z, y, x in f32) and the beam is drawn with
/// `0x104F30(0, 0, from, to, colour, colour, width, view mask)`: attached to a player (mode 1, non negative block word of the source) it is two
/// segments, one for the viewer bit mask and a thin one (width 2) from `+0x34` for the others; otherwise one segment for all views. The width is
/// `trunc([+0x1FC] * 3 + 2) + rng % (5 - that)`. The colours are the type base (`0x143C7F`, `0x7EFC14` for type `0x2F`) with the top bytes
/// `0x9F` / `0x1A` / `0x4F`.
pub fn beam_update(g: &mut dyn Guest, entry: u32) {
    if g.read(entry + 0x14C) == 0 {
        g.call(0xDC550, &[entry]);
        return;
    }
    let base = if g.read(entry) != 0x2F {
        0x0014_3C7F
    } else {
        0x006A_FF95u32.wrapping_add(0x0014_3C7F)
    };
    let (colour_a, colour_b, colour_c) =
        (base | 0x4F00_0000, base | 0x9F00_0000, base | 0x1A00_0000);
    beam_trace(g, entry);
    let dz = g.readf(entry + 0x1A8) - g.readf(entry + 0x24);
    let dy = g.readf(entry + 0x1A4) - g.readf(entry + 0x20);
    let dx = g.readf(entry + 0x1A0) - g.readf(entry + 0x1C);
    g.writef(entry + 0xF4, (dz * dz + dy * dy) + dx * dx);
    let to = entry + 0x1A0;
    let base_width =
        cvtt(g.readf(entry + 0x1FC) * f32::from_bits(0x4040_0000) + f32::from_bits(0x4000_0000));
    let range = 5i32.wrapping_sub(base_width);
    let attached_block = g.read(entry + 0x100) == 1 && {
        let source = g.read(entry + 0x108);
        let block = g.read(source + 0x150);
        (g.read(block + 4) as i32) >= 0
    };
    if attached_block {
        let remainder = if range != 0 {
            g.rng() % (range as u32)
        } else {
            0
        };
        let width = remainder.wrapping_add(base_width as u32);
        let mask = 1u32.wrapping_shl(g.at(VIEW, 0x214));
        g.call(
            0x104F30,
            &[0, 0, entry + 0x28, to, colour_b, colour_c, width, mask],
        );
        let mask = !1u32.wrapping_shl(g.at(VIEW, 0x214)) & 0xFFFF;
        g.call(
            0x104F30,
            &[0, 0, entry + 0x34, to, colour_b, colour_a, 2, mask],
        );
    } else {
        let remainder = if range != 0 {
            g.rng() % (range as u32)
        } else {
            0
        };
        let width = remainder.wrapping_add(base_width as u32);
        g.call(
            0x104F30,
            &[0, 0, entry + 0x28, to, colour_b, colour_a, width, 0xFFFF],
        );
    }
}

const PLAYER_COUNT_FIRST: u32 = 0x0079_0950;
const LEVEL_ID: u32 = 0x0079_094C;
const MODE_FLAGS: u32 = 0x007D_E458;

fn step_dt(g: &mut dyn Guest, entry: u32) -> f32 {
    let rate = g.read(super::eff_update::FRAME_RATE) as i32 as f32;
    let dt = f32::from_bits(0x3F80_0000) / rate;
    (dt * g.readf(entry + 0x88)) * g.readf(super::eff_update::STEP_SCALE)
}

/// `0xF5920(entry)`: update of the thrown bolts (types `0x15` and `0x16`). Names INFERRED, flow MEASURED.
///
/// Nothing happens while the frame steps are 0. `step = velocity * dt`. A timer `+0x98 >= 0.75`, or `>= 0.65` while the attached player index
/// (`+0x104`, `+8`) is not below the player count, releases the entry (`0xDC550`). `+0xF4 += dt^2`; a source (`+0x10C`) that is an actor adds its
/// motion (`+0xF8 ..`) to the step. Type `0x15` hands the step to `0x24C510` (if `0x24A150`) and, on level 1, `0x36C760`. The step is swept
/// (`0xE97A0`); a positive record count starts the light (`0xDCEF0`) and the entry ends; no hit moves the entry (type `0x15`: `0x186480(position,
/// 1.0)`). A hit on an actor (not model `0x5EA`): type `0x15` damages the player block of the target unless the same team (`+0x84`) under mode
/// flags 1 and not 8 (`0x1252C0(block + 0xC68, 1.0, source)`), type `0x16` applies `0x11D920` and a hit record `0x160020`; any hit that is not that
/// builds the hit record (normalised velocity through an f64 length, `0xB91F0`, `0xB9220`, `0xBD0F0`); non actors may call `0x114AF0`. The end:
/// type `0x15` runs the ground effect `0xF4920(hit, 0.3, owner)`, then the entry is released.
pub fn bolt_update(g: &mut dyn Guest, entry: u32) {
    let (step, hit, direction, normal, ray, record) = (
        FRAME - 0x19C,
        FRAME - 0x18C,
        FRAME - 0x180,
        FRAME - 0x174,
        FRAME - 0x10C,
        FRAME - 0x168,
    );
    let length_slot = FRAME - 0x190;
    let dt = step_dt(g, entry);
    let velocity = g.vec3(entry + 0x64);
    let mut moved = [dt * velocity[0], dt * velocity[1], dt * velocity[2]];
    g.set_vec3(step, moved);
    if g.read(super::eff_update::STEPS) == 0 {
        return;
    }
    let timer = g.readf(entry + 0x98);
    let release = |g: &mut dyn Guest| g.call(0xDC550, &[entry]);
    if timer >= f32::from_bits(0x3F40_0000) {
        release(g);
        return;
    }
    if timer >= f32::from_bits(0x3F26_6666) {
        let attached = g.read(entry + 0x104);
        if attached != 0 && (g.read(attached + 8) as i32) >= (g.read(PLAYER_COUNT_FIRST) as i32) {
            release(g);
            return;
        }
    }
    let source = g.read(entry + 0x10C);
    let squared = dt * dt + g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, squared);
    if source != 0 && g.read(source + 0x20) == 8 {
        moved = [
            g.readf(source + 0xF8) + moved[0],
            g.readf(source + 0xFC) + moved[1],
            g.readf(source + 0x100) + moved[2],
        ];
        g.set_vec3(step, moved);
    }
    let bolt_type = g.read(entry);
    if bolt_type == 0x15 {
        if g.call(0x24A150, &[]) != 0 {
            g.call(0x24C510, &[entry + 0x1C, step]);
        }
        if g.read(LEVEL_ID) == 1 {
            g.call(0x36C760, &[entry + 0x1C, step]);
        }
    }
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let target = g.call(0xE97A0, &[step, normal, ray, entry, hit]);
    if (g.read(ray) as i32) > 0 {
        g.copy(ray + 0xC, hit, 3);
        g.call(0xDCEF0, &[entry, ray]);
        return bolt_end(g, entry, hit);
    }
    if target == 0 {
        for axis in 0..3u32 {
            let value = g.readf(entry + 0x1C + 4 * axis) + g.readf(step + 4 * axis);
            g.writef(entry + 0x1C + 4 * axis, value);
        }
        if bolt_type == 0x15 {
            g.call(0x186480, &[entry + 0x1C, 0x3F80_0000]);
        }
        return;
    }
    let mut build_record = true;
    if g.read(target + 0x20) == 8 {
        if g.read(target + 0x94) & 0xFFFF != 0x5EA {
            let block = g.read(target + 0x7C);
            build_record = false;
            if bolt_type == 0x15 {
                let attached = g.read(entry + 0x104);
                let same_team = attached != 0 && g.read(block + 0x84) == g.read(attached + 0x84);
                let flags = g.read(MODE_FLAGS) & 0xFF;
                if !(flags & 1 != 0 && flags & 8 == 0 && same_team) {
                    let source = g.read(entry + 0x10C);
                    g.call(0x1252C0, &[block + 0xC68, 0x3F80_0000, source]);
                }
            } else if bolt_type == 0x16 {
                g.call(0x11D920, &[block + 0xC68]);
                g.write(block + 0x8C, 0);
                if (g.read(block + 8) as i32) >= (g.read(PLAYER_COUNT_FIRST) as i32) {
                    g.call(0xB91F0, &[record]);
                    g.call(0xB9220, &[record, entry]);
                    let word = g.read(block);
                    g.call(0x160020, &[word, 0, record]);
                }
            }
        }
    } else if g.call(0xDD170, &[]) != 0 {
        let sort = g.read(entry + 0xF0);
        g.call(0x114AF0, &[bolt_type, 0, sort, target, hit, normal]);
    }
    if build_record {
        let (vx, vy, vz) = (
            f64::from(velocity_word(g, entry, 0)),
            f64::from(velocity_word(g, entry, 1)),
            f64::from(velocity_word(g, entry, 2)),
        );
        let length = (((vz * vz) + (vy * vy)) + (vx * vx)).sqrt() as f32;
        g.writef(length_slot, length);
        let inverse = f32::from_bits(0x3F80_0000) / length;
        let unit = [
            inverse * velocity_word(g, entry, 0),
            inverse * velocity_word(g, entry, 1),
            inverse * velocity_word(g, entry, 2),
        ];
        g.set_vec3(direction, unit);
        g.call(0xB91F0, &[record]);
        g.call(0xB9220, &[record, entry]);
        g.write(record + 0x28, hit);
        g.write(record + 0x2C, normal);
        g.write(record, target);
        g.write(record + 0x30, direction);
        g.writef(record + 0x34, length);
        g.call(0xBD0F0, &[record]);
    }
    bolt_end(g, entry, hit);
}

fn velocity_word(g: &mut dyn Guest, entry: u32, axis: u32) -> f32 {
    g.readf(entry + 0x64 + 4 * axis)
}

fn bolt_end(g: &mut dyn Guest, entry: u32, hit: u32) {
    if g.read(entry) == 0x15 {
        let attached = g.read(entry + 0x104);
        let owner = if attached != 0 { g.read(attached) } else { 0 };
        g.call(0xF4920, &[hit, 0x3E99_999A, owner]);
    }
    g.call(0xDC550, &[entry]);
}

/// `0xF61E0(entry)`: update of the impact emitters (type `0x25`). Names INFERRED, flow MEASURED.
///
/// Nothing happens while the frame steps are 0. `step = velocity * [0x4E794C]` (no rate, no size) is swept with `0xE97A0`; a positive record
/// count starts the light (`0xDCEF0`). The direction is the velocity divided by its f64 length rounded to f32.
///
/// * HIT: the velocity reflects about the normal (`u - 2 (u . n) n`, dot summed z, y, x); `+0xF4` gains `|hit - position|^2` (z, y, x); `0xE9FE0`
///   reacts, `0xE94E0(entry, target type, hit, 6.0)`, a hit record (`0xB91F0`, `0xB9220`, `0xBD0F0`), the ground effect `0xF4920(hit, 0.4, owner)`,
///   the emitter `0x111AE0(0, 0x735FC8, 0, hit, normal)` and the entry is released.
/// * NO HIT: position and the trail words `+0x34 .. +0x3C` advance by the step, `+0xF4` gains `|step|^2` (z, y, x). A timer `+0x98 >=` the limit
///   `+0x17C` creates the emitter at the position and releases the entry. Otherwise an owner `+0xA8` follows the entry (`+0x64 ..`), its yaw
///   `+0xCC` advances by `+0xD0 * step scale` (wrapped by `0x12540(angle, 0, 360)`, returned in st(0)) and its matrix (`+0x274`) is rebuilt from the
///   direction (`0x152B50`, `0x153440`, `0x153650`).
pub fn splash_update(g: &mut dyn Guest, entry: u32) {
    let (step, normal, hit, ray, direction, reflected, record, length_slot) = (
        FRAME - 0x198,
        FRAME - 0x18C,
        FRAME - 0x1A4,
        FRAME - 0x10C,
        FRAME - 0x180,
        FRAME - 0x174,
        FRAME - 0x168,
        FRAME - 0x1A8,
    );
    if g.read(super::eff_update::STEPS) == 0 {
        return;
    }
    let scale = g.readf(super::eff_update::STEP_SCALE);
    let velocity = g.vec3(entry + 0x64);
    let moved = [
        velocity[0] * scale,
        velocity[1] * scale,
        velocity[2] * scale,
    ];
    g.set_vec3(step, moved);
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let target = g.call(0xE97A0, &[step, normal, ray, entry, hit]);
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    let (vx, vy, vz) = (
        f64::from(velocity[0]),
        f64::from(velocity[1]),
        f64::from(velocity[2]),
    );
    let length = (((vz * vz) + (vy * vy)) + (vx * vx)).sqrt() as f32;
    g.writef(length_slot, length);
    let inverse = f32::from_bits(0x3F80_0000) / length;
    let unit = [
        inverse * velocity[0],
        inverse * velocity[1],
        inverse * velocity[2],
    ];
    g.set_vec3(direction, unit);
    if target != 0 {
        let n = g.vec3(normal);
        let source = g.read(entry + 0x108);
        let dot = ((n[2] * unit[2]) + (n[1] * unit[1])) + (n[0] * unit[0]);
        let twice = dot * f32::from_bits(0x4000_0000);
        let bounced = [
            unit[0] - twice * n[0],
            unit[1] - twice * n[1],
            unit[2] - twice * n[2],
        ];
        g.set_vec3(reflected, bounced);
        let point = g.vec3(hit);
        let position = g.vec3(entry + 0x1C);
        let (dz, dy, dx) = (
            point[2] - position[2],
            point[1] - position[1],
            point[0] - position[0],
        );
        let travelled = ((dz * dz + dy * dy) + dx * dx) + g.readf(entry + 0xF4);
        g.writef(entry + 0xF4, travelled);
        let kind = g.read(entry);
        g.call(
            0xE9FE0,
            &[entry, kind, target, hit, normal, reflected, source],
        );
        let target_type = g.read(target + 0x20);
        g.call(0xE94E0, &[entry, target_type, hit, 0x40C0_0000]);
        g.call(0xB91F0, &[record]);
        g.call(0xB9220, &[record, entry]);
        g.write(record + 0x28, hit);
        g.write(record, target);
        g.write(record + 0x2C, normal);
        g.write(record + 0x30, direction);
        g.writef(record + 0x34, length);
        g.call(0xBD0F0, &[record]);
        let attached = g.read(entry + 0x104);
        let owner = if attached != 0 { g.read(attached) } else { 0 };
        g.call(0xF4920, &[hit, 0x3ECC_CCCD, owner]);
        g.call(0x111AE0, &[0, 0x0073_5FC8, 0, hit, normal]);
        g.call(0xDC550, &[entry]);
        return;
    }
    let position = g.vec3(entry + 0x1C);
    let (sx, sy, sz) = (moved[0], moved[1], moved[2]);
    let py = g.readf(entry + 0x20) + sy;
    g.writef(entry + 0x20, py);
    let pz = g.readf(entry + 0x24) + sz;
    g.writef(entry + 0x24, pz);
    for (offset, delta) in [(0x34, sx), (0x38, sy), (0x3C, sz)] {
        let value = g.readf(entry + offset) + delta;
        g.writef(entry + offset, value);
    }
    let px = position[0] + sx;
    let travelled = (((sz * sz) + (sy * sy)) + (sx * sx)) + g.readf(entry + 0xF4);
    let timer = g.readf(entry + 0x98);
    let limit = g.readf(entry + 0x17C);
    g.writef(entry + 0x1C, px);
    g.writef(entry + 0xF4, travelled);
    if timer >= limit {
        g.call(0x111AE0, &[0, 0x0073_5FC8, 0, entry + 0x1C, direction]);
        g.call(0xDC550, &[entry]);
        return;
    }
    let owner = g.read(entry + 0xA8);
    if owner == 0 {
        return;
    }
    g.writef(owner + 0x64, px);
    let word = g.read(entry + 0x20);
    g.write(owner + 0x68, word);
    let word = g.read(entry + 0x24);
    g.write(owner + 0x6C, word);
    let owner = g.read(entry + 0xA8);
    let angle =
        g.readf(owner + 0xD0) * g.readf(super::eff_update::STEP_SCALE) + g.readf(owner + 0xCC);
    let wrapped = g.call_f(0x12540, &[angle.to_bits(), 0, 0x43B4_0000]);
    let owner = g.read(entry + 0xA8);
    g.writef(owner + 0xCC, wrapped);
    let owner = g.read(entry + 0xA8);
    g.call(0x152B50, &[owner + 0x274, direction]);
    let owner = g.read(entry + 0xA8);
    let yaw = g.read(owner + 0xCC);
    g.call(0x153440, &[record, yaw]);
    let owner = g.read(entry + 0xA8);
    g.call(0x153650, &[owner + 0x274, record]);
}

/// The sine table `[0x7A38A0]`, 2048 f32 entries (a runtime table; the goldens provide the entries the original reads).
pub const SINE_TABLE: u32 = 0x007A_38A0;
const UNSIGNED_BIAS: u32 = 0x4F80_0000;
const RNG_UNIT: u32 = 0x2F80_0000;

fn sine(g: &mut dyn Guest, index: i32) -> f32 {
    g.readf(SINE_TABLE.wrapping_add(((index & 0x7FF) as u32) * 4))
}

/// `fild` of a random dword plus `2^32` when negative, times the unit scale (an exact f64 product), as the original's x87 sequence.
pub(super) fn unsigned_random_unit(word: u32) -> f64 {
    let mut value = f64::from(word as i32);
    if (word as i32) < 0 {
        value += f64::from(f32::from_bits(UNSIGNED_BIAS));
    }
    value * f64::from(f32::from_bits(RNG_UNIT))
}

/// `0xEBB00(entry)`: update of the flame jets (type `0x1A`). Names INFERRED, flow MEASURED.
///
/// With frame steps >= 1: the flicker intensity `+0x200` is a sum of three table sines of the timer (clamped to `[0, 1]`); `+0x1DC` takes a
/// random fraction and `+0x1F0` a random `% 127`. The step is `velocity * 50`; `0xDCB80` sweeps it (result `+0xA0`, end point `+0x1A0`); a positive
/// record count starts the light. `+0xF4` is the squared length end - position. A HIT (`0x913D0` copies the live contact template into a local buffer): the struck source is
/// scorched (`+0xE4` or the player block `+0x63C` = 1.5), an actor target gets `0x11E7B0(target, 1.5, end)` and `0xDC5D0`, `0xE9FE0` reacts
/// (always with a source: one draw in four, without a source one in twenty), a hit record is built; with a source and a contact type 2 on a flagged
/// surface (`bit 15` of the surface word) a second record `0x13` is built from the reversed normal. NO HIT: the end point is position + step.
/// The tail advances the phase `+0x1E4` by a random draw (x87 doubles), then the timers `+0x1F4`, `+0x1F8` and the fade `+0x1FC` (rising to 1 after a
/// hit step, falling to 0 otherwise, per frame).
pub fn flame_update(g: &mut dyn Guest, entry: u32) {
    jet_update(g, entry, false);
}

/// `0xEC120(entry)`: update of the spark jets (type `0x19`), the same body as [`flame_update`] with three differences. While `[0x7DE458] & 0x200` a
/// timer above `0.35` releases the entry first. A hit only runs `0xDC5D0` on an actor (no scorching, no `0x11E7B0`). The second hit record uses
/// the reversed entry direction `+0x180 ..` instead of the velocity. After the tail a basis is built from the direction `d = +0x180 ..`: the
/// tangent `+0x1C4 ..` is `(dy - 0*dz, 0*dz - dx, 0*dx - 0*dy)` when `|dy| >= 0.707`, else `(0*dy - dz, dz*0 - dx*0, dx - 0*dy)` (the `0` is a
/// register the compiler left at zero: the signed zeros are part of the result), and `+0x1D0 ..` is the cross product `t x d`.
pub fn spark_update(g: &mut dyn Guest, entry: u32) {
    jet_update(g, entry, true);
}

fn jet_update(g: &mut dyn Guest, entry: u32, spark: bool) {
    let (step, normal, ray, record, contact) = (
        FRAME - 0x1BC,
        FRAME - 0x1B0,
        FRAME - 0x10C,
        FRAME - 0x188,
        FRAME - 0x12C,
    );
    if (g.read(super::eff_update::STEPS) as i32) < 1 {
        return;
    }
    if spark
        && g.read(MODE_FLAGS) & 0x200 != 0
        && g.readf(entry + 0x98) > f32::from_bits(0x3EB3_3333)
    {
        g.call(0xDC550, &[entry]);
        return;
    }
    let (half, wave_scale) = (f32::from_bits(0x3F00_0000), f32::from_bits(0x43A2_F983));
    let timer = g.readf(entry + 0x98);
    let mut first = timer * f32::from_bits(0x4149_0FDB);
    let mut second = timer * f32::from_bits(0x4182_B0B4);
    first *= wave_scale;
    second *= wave_scale;
    first += half;
    second += half;
    g.write(entry + 0xA0, 0);
    let (first_index, second_index) = (cvtt(first), cvtt(second));
    let mut intensity =
        (sine(g, first_index.wrapping_add(0x200)) + half) * f32::from_bits(0x3E19_999A);
    intensity += (sine(g, second_index) + half) * f32::from_bits(0x3DCC_CCCD);
    let third = (timer * f32::from_bits(0x4125_E048)) * wave_scale + half;
    let third_term = (sine(g, cvtt(third)) + half) * f32::from_bits(0x3E4C_CCCD);
    intensity += third_term;
    intensity += half;
    g.writef(entry + 0x200, intensity);
    if 0.0 > intensity {
        g.writef(entry + 0x200, 0.0);
    }
    if g.readf(entry + 0x200) > 1.0 {
        g.writef(entry + 0x200, 1.0);
    }
    let draw = g.rng();
    g.write(FRAME - if spark { 0x1C4 } else { 0x1C0 }, draw);
    let fraction = unsigned_random_unit(draw);
    g.writef(entry + 0x1DC, fraction as f32);
    let draw = g.rng();
    let jitter = draw % 0x7F;
    let fifty = f32::from_bits(0x4248_0000);
    let velocity = g.vec3(entry + 0x64);
    let moved = [
        velocity[0] * fifty,
        velocity[1] * fifty,
        velocity[2] * fifty,
    ];
    let position = g.vec3(entry + 0x1C);
    g.set_vec3(step, moved);
    g.write(ray, 0);
    g.write(ray + 4, 0);
    g.writef(entry + 0x1A4, position[1] + moved[1]);
    g.writef(entry + 0x1A0, position[0] + moved[0]);
    g.write(entry + 0x1F0, jitter);
    g.writef(entry + 0x1A8, position[2] + moved[2]);
    let target = g.call(0xDCB80, &[step, entry + 0x1A0, normal, ray, entry]);
    g.write(entry + 0xA0, target);
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    let dz = g.readf(entry + 0x1A8) - g.readf(entry + 0x24);
    let dy = g.readf(entry + 0x1A4) - g.readf(entry + 0x20);
    let dx = g.readf(entry + 0x1A0) - g.readf(entry + 0x1C);
    g.writef(entry + 0xF4, (dz * dz + dy * dy) + dx * dx);
    let mut hit_flag = false;
    if target == 0 {
        for axis in 0..3u32 {
            let value = g.readf(entry + 0x1C + 4 * axis) + g.readf(step + 4 * axis);
            g.writef(entry + 0x1A0 + 4 * axis, value);
        }
    } else {
        g.copy(super::eff_helpers::CONTACT_LIVE, contact, 8);
        if !spark {
            let struck = g.read(entry + 0xA0);
            if g.read(struck + 0x20) != 2 {
                let source = g.read(entry + 0x108);
                if source != 0 {
                    let source_type = g.read(source + 0x20);
                    if source_type == 8 {
                        let block = g.read(source + 0x150);
                        if block != 0 && g.read(block + 0x58C) == 0xD {
                            g.write(block + 0x63C, 0x3FC0_0000);
                        }
                    } else if source_type == 0x200 {
                        g.write(source + 0xE4, 0x3FC0_0000);
                    }
                }
            }
        }
        let struck = g.read(entry + 0xA0);
        if g.read(struck + 0x20) == 8 {
            if !spark {
                g.call(0x11E7B0, &[struck, 0x3FC0_0000, entry + 0x1A0]);
            }
            g.call(0xDC5D0, &[entry]);
        }
        let source = g.read(entry + 0x108);
        let react = if source != 0 {
            g.rng() & 3 == 0
        } else {
            g.rng() % 20 == 0
        };
        if react {
            let source = g.read(entry + 0x108);
            let kind = g.read(entry);
            let struck = g.read(entry + 0xA0);
            g.call(
                0xE9FE0,
                &[entry, kind, struck, entry + 0x1A0, normal, normal, source],
            );
        }
        g.call(0xB91F0, &[record]);
        g.call(0xB9220, &[record, entry]);
        let struck = g.read(entry + 0xA0);
        g.write(record, struck);
        g.write(record + 0x28, entry + 0x1A0);
        g.write(record + 0x2C, normal);
        g.write(record + 0x30, entry + 0x64);
        let size = g.read(entry + 0x88);
        g.write(record + 0x34, size);
        g.call(0xBD0F0, &[record]);
        let source = g.read(entry + 0x10C);
        if source != 0 && g.read(contact + 8) == 2 {
            let struck = g.read(entry + 0xA0);
            let surface = g.read(struck + 0x24);
            if g.read(surface + 0x28) & 0x8000 != 0 {
                let push = f32::from_bits(0xBF80_0000);
                let (pa, pb) = (FRAME - 0x1A4, FRAME - 0x198);
                for axis in 0..3u32 {
                    let value = if spark {
                        0.0 - g.readf(entry + 0x180 + 4 * axis)
                    } else {
                        g.readf(entry + 0x64 + 4 * axis) * push
                    };
                    g.writef(pa + 4 * axis, value);
                }
                let reversed = [
                    0.0 - g.readf(normal),
                    0.0 - g.readf(normal + 4),
                    0.0 - g.readf(normal + 8),
                ];
                g.set_vec3(pb, reversed);
                g.call(0xB91F0, &[record]);
                g.call(0xB9220, &[record, entry]);
                let source = g.read(entry + 0x10C);
                g.write(record, source);
                g.write(record + 0x24, 0x13);
                g.write(record + 0x28, entry + 0x1C);
                g.write(record + 0x2C, pb);
                g.write(record + 0x30, pa);
                let size = g.read(entry + 0x88);
                g.write(record + 0x34, size);
                g.call(0xBD0F0, &[record]);
                hit_flag = true;
            }
        }
    }
    flame_tail(g, entry, hit_flag);
    if spark {
        direction_basis(g, entry, entry + 0x180);
    }
}

/// The basis of `0xEC120` and `0xE4800` (see [`spark_update`]) from the vector at `vector`.
fn direction_basis(g: &mut dyn Guest, entry: u32, vector: u32) {
    let zero = 0.0f32;
    let direction = g.vec3(vector);
    let (dx, dy, dz) = (direction[0], direction[1], direction[2]);
    let tangent = if dy.abs() >= f32::from_bits(0x3F34_FDF4) {
        [dy - zero * dz, zero * dz - dx, dx * zero - dy * zero]
    } else {
        [dy * zero - dz, dz * zero - dx * zero, dx - dy * zero]
    };
    g.set_vec3(entry + 0x1C4, tangent);
    let tangent = g.vec3(entry + 0x1C4);
    let side = [
        tangent[1] * dz - dy * tangent[2],
        dx * tangent[2] - tangent[0] * dz,
        tangent[0] * dy - tangent[1] * dx,
    ];
    g.set_vec3(entry + 0x1D0, side);
}

/// The tail of `0xEBB00`: the phase `+0x1E4` advances by a random draw (x87 doubles), the timers and the fade.
fn flame_tail(g: &mut dyn Guest, entry: u32, hit_flag: bool) {
    let before = g.readf(entry + 0x1E4);
    let draw = g.rng();
    g.write(FRAME - 0x18C, draw);
    let random = unsigned_random_unit(draw) * f64::from(f32::from_bits(0x4040_0000))
        + f64::from(f32::from_bits(0x40C0_0000));
    let inverse = f64::from(f32::from_bits(0x3F80_0000))
        / f64::from(g.read(super::eff_update::FRAME_RATE) as i32);
    let scale = f64::from(g.readf(super::eff_update::STEP_SCALE));
    let advanced = f64::from(before) - ((random * inverse) * scale);
    g.writef(entry + 0x1E4, advanced as f32);
    if 0.0 > advanced {
        let mut phase = g.readf(entry + 0x1E4);
        loop {
            phase += 1.0;
            if !(0.0 > phase) {
                break;
            }
        }
        g.writef(entry + 0x1E4, phase);
    }
    let scale = g.readf(super::eff_update::STEP_SCALE);
    let angular = f32::from_bits(0x3DD6_7750);
    let value = scale * angular + g.readf(entry + 0x1F4);
    g.writef(entry + 0x1F4, value);
    let value = g.readf(entry + 0x1F8) - scale * angular;
    g.writef(entry + 0x1F8, value);
    let rate = g.read(super::eff_update::FRAME_RATE) as i32 as f32;
    if hit_flag {
        let rise = ((1.0 / rate) * scale) * f32::from_bits(0x40C0_0000);
        let fade = rise + g.readf(entry + 0x1FC);
        g.writef(entry + 0x1FC, fade);
        if fade > 1.0 {
            g.writef(entry + 0x1FC, 1.0);
        }
    } else {
        let fall = (1.0 / rate) * scale;
        let fade = g.readf(entry + 0x1FC) - fall;
        g.writef(entry + 0x1FC, fade);
        if 0.0 > fade {
            g.writef(entry + 0x1FC, 0.0);
        }
    }
}

fn wave_intensity(g: &mut dyn Guest, entry: u32) -> f32 {
    let (half, wave_scale) = (f32::from_bits(0x3F00_0000), f32::from_bits(0x43A2_F983));
    let timer = g.readf(entry + 0x98);
    let first = (timer * f32::from_bits(0x4149_0FDB)) * wave_scale + half;
    let second = (timer * f32::from_bits(0x4182_B0B4)) * wave_scale + half;
    let (first_index, second_index) = (cvtt(first), cvtt(second));
    let mut intensity =
        (sine(g, first_index.wrapping_add(0x200)) + half) * f32::from_bits(0x3E19_999A);
    intensity += (sine(g, second_index) + half) * f32::from_bits(0x3DCC_CCCD);
    let third = (timer * f32::from_bits(0x4125_E048)) * wave_scale + half;
    intensity += (sine(g, cvtt(third)) + half) * f32::from_bits(0x3E4C_CCCD);
    intensity + half
}

/// `0xE4800(entry)`: update of the trace beams (type `0x2C`). Names INFERRED, flow MEASURED.
///
/// Nothing happens while the frame steps are 0. The flicker `+0x200` is the wave sum of `0xEBB00` clamped to `[0, 1]`; `+0x1DC` takes a random
/// fraction and `+0x1F0` a random `% 127`. The first frame (`+0x150 == 0`) clears a field (`+0x108` of the structure `[[[+0x108] + 0x7C] + 0x6C]`)
/// and sets `+0x150`. The position is the origin `+0x40 ..` plus the velocity times `timer * [0x4760AC]`; the step `velocity * (1 / rate * scale *
/// [0x4760AC])` is swept with `0xDCB80`. NO HIT: the position takes the step; once it is further than the limit `[0x47D744]` (squared) from the
/// origin the entry is released (and the source's field cleared when the source is an object of type `0x200`). A positive record count starts the
/// light. A HIT: an actor gets the sound `0x46AB0` and `0xDC5D0`, one draw in sixteen starts a spark `0x100AA0(0, 0x36, flag, position, normal, 0)`
/// (flag `0x80000` unless one local player), a hit record, the effect `0x1FD570([+0xF0], position, normal)`, the source field is cleared and the entry
/// is released. The tail (also after a release): the fade `+0x1E4` rises by `(u * 3 + 6) / rate * q * scale` (x87 doubles) while below 1, the
/// timers `+0x1F4 / +0x1F8` turn, the basis `+0x1C4` / `+0x1D0` is built from the velocity (see [`spark_update`]) and `+0x1A0` is the far end
/// `origin + velocity * 50`.
pub fn trace_update(g: &mut dyn Guest, entry: u32) {
    let (step, hit, normal, ray, record, contact) = (
        FRAME - 0x1B0,
        FRAME - 0x194,
        FRAME - 0x1A4,
        FRAME - 0x10C,
        FRAME - 0x188,
        FRAME - 0x12C,
    );
    let fade_slot = FRAME - 0x1B4;
    if g.read(super::eff_update::STEPS) == 0 {
        return;
    }
    let intensity = wave_intensity(g, entry);
    g.write(entry + 0xA0, 0);
    let clamped = if intensity > 1.0 {
        1.0
    } else if 0.0 > intensity {
        0.0
    } else {
        intensity
    };
    g.writef(entry + 0x200, clamped);
    let draw = g.rng();
    g.writef(entry + 0x1DC, unsigned_random_unit(draw) as f32);
    let draw = g.rng();
    g.write(entry + 0x1F0, draw % 0x7F);
    if g.read(entry + 0x150) == 0 {
        clear_source_field(g, entry, false);
        g.write(entry + 0x150, 1);
    }
    let reach = f32::from_bits(0x4270_0000);
    let travelled = g.readf(entry + 0x98) * reach;
    let velocity = g.vec3(entry + 0x64);
    let origin = g.vec3(entry + 0x40);
    g.writef(entry + 0x1C, travelled * velocity[0] + origin[0]);
    g.writef(entry + 0x24, travelled * velocity[2] + origin[2]);
    g.writef(entry + 0x20, velocity[1] * travelled + origin[1]);
    let rate = g.read(super::eff_update::FRAME_RATE) as i32 as f32;
    let unit =
        ((f32::from_bits(0x3F80_0000) / rate) * g.readf(super::eff_update::STEP_SCALE)) * reach;
    g.set_vec3(
        step,
        [unit * velocity[0], unit * velocity[1], unit * velocity[2]],
    );
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let target = g.call(0xDCB80, &[step, hit, normal, ray, entry]);
    g.write(entry + 0xA0, target);
    let position = entry + 0x1C;
    if target == 0 {
        let moved = g.vec3(step);
        let x = moved[0] + g.readf(position);
        let y = g.readf(entry + 0x20) + moved[1];
        let z = moved[2] + g.readf(entry + 0x24);
        g.writef(position, x);
        let dx = x - g.readf(entry + 0x40);
        g.writef(entry + 0x20, y);
        let dy = y - g.readf(entry + 0x44);
        g.writef(entry + 0x24, z);
        let dz = z - g.readf(entry + 0x48);
        let squared = (dx * dx + dy * dy) + dz * dz;
        if squared > f32::from_bits(0x451C_4000) {
            clear_source_field(g, entry, true);
            g.call(0xDC550, &[entry]);
            return;
        }
    }
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    if g.read(entry + 0xA0) != 0 {
        g.copy(super::eff_helpers::CONTACT_LIVE, contact, 8);
        let struck = g.read(entry + 0xA0);
        if g.read(struck + 0x20) == 8 {
            let sound_object = g.read(0x007E_71F0);
            g.call(0x46AB0, &[0xDAF, 0x4220_0000, sound_object, position]);
            g.call(0xDC5D0, &[entry]);
        }
        if g.rng() & 0xF == 0 {
            let players = g.call(0x1CDAE0, &[]);
            let flag = if players.wrapping_sub(1) != 0 {
                0x80000
            } else {
                0
            };
            g.call(0x100AA0, &[0, 0x36, flag, position, normal, 0]);
        }
        g.call(0xB91F0, &[record]);
        g.call(0xB9220, &[record, entry]);
        let struck = g.read(entry + 0xA0);
        g.write(record, struck);
        g.write(record + 0x28, hit);
        g.write(record + 0x2C, normal);
        g.write(record + 0x30, entry + 0x64);
        let size = g.read(entry + 0x88);
        g.write(record + 0x34, size);
        g.call(0xBD0F0, &[record]);
        let sort = g.read(entry + 0xF0);
        g.call(0x1FD570, &[sort, position, normal]);
        clear_source_field(g, entry, true);
        g.call(0xDC550, &[entry]);
    }
    trace_tail(g, entry, fade_slot);
}

/// `[[[source + 0x7C] + 0x6C] + 0x108] = 0.0` (the first frame always, a release only for a source of type `0x200`).
fn clear_source_field(g: &mut dyn Guest, entry: u32, only_actor_sources: bool) {
    let source = g.read(entry + 0x108);
    if only_actor_sources && (source == 0 || g.read(source + 0x20) != 0x200) {
        return;
    }
    let block = g.read(source + 0x7C);
    let target = g.read(block + 0x6C);
    g.writef(target + 0x108, 0.0);
}

fn trace_tail(g: &mut dyn Guest, entry: u32, fade_slot: u32) {
    let fade = g.readf(entry + 0x1E4);
    if 1.0 > fade {
        let ramp = (fade * f32::from_bits(0x4120_0000)) * (fade * f32::from_bits(0x4120_0000));
        let floor = f32::from_bits(0x3C23_D70A);
        let factor = if ramp > floor { ramp } else { floor };
        g.writef(fade_slot, factor);
        let draw = g.rng();
        g.write(FRAME - 0x198, draw);
        let random = unsigned_random_unit(draw) * f64::from(f32::from_bits(0x4040_0000))
            + f64::from(f32::from_bits(0x40C0_0000));
        let inverse = f64::from(f32::from_bits(0x3F80_0000))
            / f64::from(g.read(super::eff_update::FRAME_RATE) as i32);
        let scale = f64::from(g.readf(super::eff_update::STEP_SCALE));
        let advanced = f64::from(fade) + (((random * inverse) * f64::from(factor)) * scale);
        let stored = advanced as f32;
        g.writef(fade_slot, stored);
        g.writef(entry + 0x1E4, stored);
        if stored > 1.0 {
            g.writef(entry + 0x1E4, 1.0);
        } else if 0.0 > stored {
            g.writef(entry + 0x1E4, 0.0);
        }
    }
    let scale = g.readf(super::eff_update::STEP_SCALE);
    let angular = f32::from_bits(0x3DD6_7750);
    let value = scale * angular + g.readf(entry + 0x1F4);
    g.writef(entry + 0x1F4, value);
    let value = g.readf(entry + 0x1F8) - scale * angular;
    g.writef(entry + 0x1F8, value);
    direction_basis(g, entry, entry + 0x64);
    let velocity = g.vec3(entry + 0x64);
    let origin = g.vec3(entry + 0x40);
    let far = f32::from_bits(0x4248_0000);
    for axis in 0..3 {
        g.writef(
            entry + 0x1A0 + 4 * axis as u32,
            velocity[axis] * far + origin[axis],
        );
    }
}

pub(super) fn degrees_of(y: f32, x: f32) -> f64 {
    f64::from(y).atan2(f64::from(x)) * f64::from(f32::from_bits(0x4265_2EE0))
}

/// `__ftol` (`0x3C848C`): truncate the x87 top of stack to an integer, "integer indefinite" when out of range.
pub(super) fn ftol(value: f64) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value <= -2_147_483_649.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// `owner[offset] = wrap(owner[offset] + step)`: add, one `+360` for a negative result, one `-360` from 360 up (the original's two compare chains).
fn spin_owner(g: &mut dyn Guest, entry: u32, owner_offset: u32, step_offset: u32) {
    let turn = f32::from_bits(0x43B4_0000);
    let owner = g.read(entry + 0xA8);
    let value = g.readf(entry + step_offset) + g.readf(owner + owner_offset);
    g.writef(owner + owner_offset, value);
    let owner = g.read(entry + 0xA8);
    if 0.0 > g.readf(owner + owner_offset) {
        let value = g.readf(owner + owner_offset) + turn;
        g.writef(owner + owner_offset, value);
    }
    let owner = g.read(entry + 0xA8);
    let value = g.readf(owner + owner_offset);
    if value >= turn {
        g.writef(owner + owner_offset, value - turn);
    }
}

/// `0xF51F0(entry)`: update of the bouncing orbs (type `0x14`). Names INFERRED, flow MEASURED.
///
/// Nothing happens while the frame steps are 0. `dt = ((1 / rate) * size) * scale`; a timer `+0x98 >= 10` releases the entry. `+0xF4 += dt^2`.
/// SIZE `+0x88 > 0`: the step `velocity * dt` is swept (`0xDCB80`, a positive record count starts the light).
/// * HIT: an actor gets `0xDC5D0` and `0xE1F80(hit)`; the position is the hit point plus 0.01 along the normal (plus 0.07 in y); the velocity
///   reflects (`v + (v . n) * -2 n`, dot summed z, y, x) and is renormalised through `1 / sqrt` when its squared length exceeds `1e-7`. The size
///   shrinks by 0.4. A small result (< 0.2) on a non actor with `n.y > 0.8` comes to REST: the owner (`+0xA8`) is aimed along the normal
///   (yaw `atan2(n.z, n.y)` + 90 into `+0xC0`, pitch into `+0xC8`), size and velocity become 0 and `+0x1B8 = 1`. Otherwise it BOUNCES: `0xDC6F0(-10;
///   entry)`, the spins `+0x8C/+0x90/+0x94` damp by -0.9, 0.9, 0.9, the counter `+0x198` increments and a sound `0x46AB0(0xD0D, 15, 0.275 * (4 -
///   count), position)` plays while that factor is positive. Then the owner follows the position, the ground effect
///   `0xF4920(hit, 0.2, owner)`, `0xE94E0(.., 6.0)` and (not for contact type 2) a hit record `0xBD0F0`.
/// * NO HIT: the position takes the step, `0xDC6F0`, the size shrinks by 0.99 per frame step, the owner follows and its angles spin (wrapped to
///   `[0, 360)`).
/// SIZE `<= 0` (resting): the rest timer `+0x9C` runs; above 4 seconds the sound-and-fade `0x94220(owner + 4, 15, -1, -1, trunc(+0x1B8 * 127))` runs
/// and `+0x1B8` falls by `0.3 / rate * scale`; at 0 the entry is released. Every path except releases ends with `0xEDBE0(position, 0, entry)`.
pub fn bounce_update(g: &mut dyn Guest, entry: u32) {
    let (step, hit, normal, ray, contact, record, slot) = (
        FRAME - 0x19C,
        FRAME - 0x1A8,
        FRAME - 0x1B8,
        FRAME - 0x10C,
        FRAME - 0x188,
        FRAME - 0x168,
        FRAME - 0x1AC,
    );
    let rate = g.read(super::eff_update::FRAME_RATE) as i32 as f32;
    let one = f32::from_bits(0x3F80_0000);
    let mut dt = one / rate;
    dt *= g.readf(entry + 0x88);
    dt *= g.readf(super::eff_update::STEP_SCALE);
    if g.read(super::eff_update::STEPS) == 0 {
        return;
    }
    if g.readf(entry + 0x98) >= f32::from_bits(0x4120_0000) {
        g.call(0xDC550, &[entry]);
        return;
    }
    let velocity = g.vec3(entry + 0x64);
    g.set_vec3(step, [velocity[0] * dt, velocity[1] * dt, velocity[2] * dt]);
    let squared = dt * dt + g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, squared);
    let position = entry + 0x1C;
    if g.readf(entry + 0x88) > 0.0 {
        g.write(ray, 0);
        g.write(ray + 4, 0);
        let target = g.call(0xDCB80, &[step, hit, normal, ray, entry]);
        if (g.read(ray) as i32) > 0 {
            g.call(0xDCEF0, &[entry, ray]);
        }
        if target == 0 {
            bounce_no_hit(g, entry, step);
        } else {
            bounce_hit(g, entry, target, (step, hit, normal, contact, record, slot));
        }
    } else {
        let value = (one / rate) * g.readf(super::eff_update::STEP_SCALE) + g.readf(entry + 0x9C);
        g.writef(entry + 0x9C, value);
        if value > f32::from_bits(0x4080_0000) {
            let fade = f64::from(g.readf(entry + 0x1B8)) * f64::from(f32::from_bits(0x42FE_0000));
            let level = ftol(fade) as u32;
            let owner = g.read(entry + 0xA8);
            let handle = g.read(owner + 4);
            g.call(0x94220, &[handle, 0xF, u32::MAX, u32::MAX, level]);
            let decay = ((one / rate) * g.readf(super::eff_update::STEP_SCALE))
                * f32::from_bits(0x3E99_999A);
            let faded = g.readf(entry + 0x1B8) - decay;
            g.writef(entry + 0x1B8, faded);
            if !(0.0 < faded) {
                g.call(0xDC550, &[entry]);
                return;
            }
        }
    }
    g.call(0xEDBE0, &[position, 0, entry]);
}

fn bounce_no_hit(g: &mut dyn Guest, entry: u32, step: u32) {
    for axis in 0..3u32 {
        let value = g.readf(step + 4 * axis) + g.readf(entry + 0x1C + 4 * axis);
        g.writef(entry + 0x1C + 4 * axis, value);
    }
    g.call(0xDC6F0, &[0xC120_0000, entry]);
    let shrink = f32::from_bits(0x3F7D_70A4);
    let mut count = 0i32;
    if (g.read(super::eff_update::STEPS) as i32) > 0 {
        loop {
            let size = g.readf(entry + 0x88) * shrink;
            g.writef(entry + 0x88, size);
            count += 1;
            if count >= g.read(super::eff_update::STEPS) as i32 {
                break;
            }
        }
    }
    let owner = g.read(entry + 0xA8);
    for (offset, source) in [(0x64, 0x1C), (0x68, 0x20), (0x6C, 0x24)] {
        let word = g.read(entry + source);
        g.write(owner + offset, word);
    }
    spin_owner(g, entry, 0xC0, 0x8C);
    spin_owner(g, entry, 0x70, 0x90);
    spin_owner(g, entry, 0xC8, 0x94);
}

fn bounce_hit(g: &mut dyn Guest, entry: u32, target: u32, frame: (u32, u32, u32, u32, u32, u32)) {
    let (_, hit, normal, contact, record, slot) = frame;
    let position = entry + 0x1C;
    g.copy(super::eff_helpers::CONTACT_LIVE, contact, 8);
    if g.read(target + 0x20) == 8 {
        g.call(0xDC5D0, &[entry]);
        g.call(0xE1F80, &[hit]);
    }
    let n = g.vec3(normal);
    let h = g.vec3(hit);
    let nudge = f32::from_bits(0x3C23_D70A);
    g.writef(position, n[0] * nudge + h[0]);
    g.writef(
        entry + 0x20,
        (n[1] * nudge + h[1]) + f32::from_bits(0x3D8F_5C29),
    );
    g.writef(entry + 0x24, n[2] * nudge + h[2]);
    let v = g.vec3(entry + 0x64);
    let dot = (((g.readf(entry + 0x6C) * n[2]) + (g.readf(entry + 0x68) * n[1])) + (v[0] * n[0]))
        * f32::from_bits(0xC000_0000);
    let vx = dot * n[0] + v[0];
    let vz = dot * n[2] + g.readf(entry + 0x6C);
    let vy = dot * n[1] + g.readf(entry + 0x68);
    g.writef(entry + 0x6C, vz);
    g.writef(entry + 0x68, vy);
    let length_squared = (vz * vz + vy * vy) + vx * vx;
    g.writef(entry + 0x64, vx);
    if length_squared > f32::from_bits(0x33D6_BF95) {
        g.writef(FRAME - 0x18C, length_squared);
        g.writef(FRAME - 0x190, 1.0);
        let inverse = (1.0f32 / length_squared.sqrt()) * 1.0;
        g.writef(slot, inverse);
        let x = g.readf(entry + 0x64) * inverse;
        g.writef(entry + 0x64, x);
        let y = g.readf(entry + 0x68) * inverse;
        g.writef(entry + 0x68, y);
        let z = g.readf(entry + 0x6C) * inverse;
        g.writef(entry + 0x6C, z);
    }
    let size = g.readf(entry + 0x88) * f32::from_bits(0x3ECC_CCCD);
    g.writef(entry + 0x88, size);
    let resting = f32::from_bits(0x3E4C_CCCD) > size
        && g.read(target + 0x20) != 8
        && n[1] > f32::from_bits(0x3F4C_CCCD);
    if resting {
        let owner = g.read(entry + 0xA8);
        g.writef(
            owner + 0xC0,
            (degrees_of(n[2], n[1]) + f64::from(f32::from_bits(0x42B4_0000))) as f32,
        );
        let owner = g.read(entry + 0xA8);
        let flat = (f64::from(n[2]) * f64::from(n[2]) + f64::from(n[1]) * f64::from(n[1])).sqrt();
        g.writef(
            owner + 0xC8,
            -(f64::from(n[0]).atan2(flat) * f64::from(f32::from_bits(0x4265_2EE0))) as f32,
        );
        let owner = g.read(entry + 0xA8);
        g.writef(owner + 0x70, 0.0);
        g.writef(entry + 0x88, 0.0);
        g.writef(entry + 0x1B8, 1.0);
        g.writef(entry + 0x64, 0.0);
        g.writef(entry + 0x68, 0.0);
        g.writef(entry + 0x6C, 0.0);
    } else {
        let bounces = g.read(entry + 0x198);
        let loudness = (4i32.wrapping_sub(bounces as i32) as f32) * f32::from_bits(0x3E8C_CCCD);
        g.writef(slot, loudness);
        g.call(0xDC6F0, &[0xC120_0000, entry]);
        let spin = g.readf(entry + 0x8C) * f32::from_bits(0xBF66_6666);
        g.writef(entry + 0x8C, spin);
        let damp = f32::from_bits(0x3F66_6666);
        let spin = g.readf(entry + 0x90) * damp;
        g.writef(entry + 0x90, spin);
        let spin = g.readf(entry + 0x94) * damp;
        g.writef(entry + 0x94, spin);
        g.write(entry + 0x198, bounces.wrapping_add(1));
        if loudness > 0.0 {
            g.call(0x46AB0, &[0xD0D, 0x4170_0000, loudness.to_bits(), position]);
        }
    }
    let owner = g.read(entry + 0xA8);
    for (offset, source) in [(0x64, 0x1C), (0x68, 0x20), (0x6C, 0x24)] {
        let word = g.read(entry + source);
        g.write(owner + offset, word);
    }
    let attached = g.read(entry + 0x104);
    let owner = if attached != 0 { g.read(attached) } else { 0 };
    g.call(0xF4920, &[hit, 0x3E4C_CCCD, owner]);
    let target_type = g.read(target + 0x20);
    g.call(0xE94E0, &[entry, target_type, hit, 0x40C0_0000]);
    if g.read(target + 0x20) != 2 {
        g.call(0xB91F0, &[record]);
        g.call(0xB9220, &[record, entry]);
        let size = g.read(entry + 0x88);
        g.write(record, target);
        g.write(record + 0x28, hit);
        g.write(record + 0x2C, normal);
        g.write(record + 0x30, entry + 0x64);
        g.write(record + 0x34, size);
        g.write(record + 0x38, contact);
        g.call(0xBD0F0, &[record]);
    }
}

/// `[0x7B0C7C]`: the viewer object (position `+0xA8 ..`, linked object `+0x14`).
const VIEWER: u32 = 0x007B_0C7C;
const CLOCK: u32 = 0x007D_E318;

/// `0xECB90(entry)`: update of the drones (type `0x2B`): a wobbling projectile that leaves a point trail. Names INFERRED, flow MEASURED.
///
/// * First call: the trail structure (`+0x208`, `0x188` bytes, allocated by `0x3ED20`) is created; ring of 12 byte points at `+0`, stamp `+0x168`,
///   write index `+0x16C`, capacity `+0x170` (`rate / 2`), wrapped flag `+0x174`, fade `+0x178`, latch `+0x17C`, handles `+0x180 / +0x184`.
/// * The height under the entry (`0xC3E60`, st(0)) pulls `y` toward `height + 0.05` by 0.9 per frame step; a distance above 2 flips `vx` instead.
///   The wobble phase `+0x1DC` advances by a random draw (x87 doubles) and the velocity becomes `sin` table values times the size (`+0x88`).
/// * Every third trail point (from the write index mod 3) within `sqrt(0.1)` of the viewer lights it (record `0xBB6C0`, direction to the viewer).
/// * A new point is stored every 2 clock ticks. The step `velocity * dt` is swept (`0xDCB80`). A hit on the source itself is ignored for 0.1 s; a
///   viewer closer than `sqrt(0.2)` counts as a hit on the viewer's object (normal up). On a hit (not latched, model `0x1B9` only with flag 2): the
///   velocity reflects with factor 0.3 (`0xE9FE0` for actors, `0x126260` for a block without `+0x150`, `0xDC5D0`), `0xE94E0`, `0x114AF0` for a
///   surface that allows it, a hit record (`0xBB6C0` for model `0x1B9`, else `0xBD0F0`), the trail latches (`+0x178 = 1`, `+0x17C = 1`, `0x1D7130`).
/// * A latched drone fades the trail (`0x10EB90` on both handles, factor 7) and is released with the trail freed (`0x10E2A0`, `0x3EAD0`) at 0.
///   An unlatched one moves by the step.
pub fn drone_update(g: &mut dyn Guest, entry: u32) {
    let (rel_slot, fade_slot, offset_slot) = (FRAME - 0x1E0, FRAME - 0x1E4, FRAME - 0x1DC);
    let (hit, normal, step, ray, record, contact) = (
        FRAME - 0x1D0,
        FRAME - 0x1C4,
        FRAME - 0x1B8,
        FRAME - 0x10C,
        FRAME - 0x1A0,
        FRAME - 0x144,
    );
    let rate = g.read(super::eff_update::FRAME_RATE) as i32 as f32;
    let one = f32::from_bits(0x3F80_0000);
    let mut trail = g.read(entry + 0x208);
    if trail == 0 {
        trail = g.call(0x3ED20, &[0x188, 1, entry, 0xE7CA0]);
        g.write(entry + 0x208, trail);
        if trail == 0 {
            return;
        }
        g.writef(trail + 0x178, 0.0);
        g.write(trail + 0x17C, 0);
        g.write(trail + 0x16C, 0);
        let capacity = cvtt(rate * f32::from_bits(0x3F00_0000));
        g.write(trail + 0x170, capacity as u32);
        g.write(trail + 0x174, 0);
        let clock = g.read(CLOCK);
        g.write(trail + 0x168, clock);
    }
    let sort = g.read(entry + 0xF0);
    g.call(0x873F0, &[sort, FRAME - 0x118, FRAME - 0x124]);
    let height = g.call_f(0xC3E60, &[entry + 0x1C, 0x3DCC_CCCD, 0x3DCC_CCCD, 0, 0, 1]);
    g.writef(fade_slot, height);
    let relative = (f64::from(height) - f64::from(g.readf(entry + 0x20))) as f32;
    g.writef(rel_slot, relative);
    if relative.abs() > 2.0 {
        let flipped = 0.0 - g.readf(entry + 0x64);
        g.writef(entry + 0x64, flipped);
    } else if (g.read(super::eff_update::STEPS) as i32) > 0 {
        let target_height = height + f32::from_bits(0x3D4C_CCCD);
        let mut count = 0i32;
        loop {
            let y = ((g.readf(entry + 0x20) - target_height) * f32::from_bits(0x3F66_6666)
                + height)
                + f32::from_bits(0x3D4C_CCCD);
            g.writef(entry + 0x20, y);
            count += 1;
            if count >= g.read(super::eff_update::STEPS) as i32 {
                break;
            }
        }
    }
    let draw = g.rng();
    let steps = f64::from(g.read(super::eff_update::STEPS) as i32);
    let unit = unsigned_random_unit(draw);
    let advance = (((unit - 0.5)
        * (f64::from(one) / f64::from(g.read(super::eff_update::FRAME_RATE) as i32)))
        * steps)
        * 10.0;
    let phase = advance + f64::from(g.readf(entry + 0x1DC));
    g.writef(entry + 0x1DC, phase as f32);
    let index = ftol(phase * f64::from(f32::from_bits(0x43A2_F983)) + 0.5);
    let size = g.readf(entry + 0x88);
    let wobble = sine(g, index) * size;
    g.writef(entry + 0x64, wobble);
    g.writef(entry + 0x68, 0.0);
    let second =
        cvtt(g.readf(entry + 0x1DC) * f32::from_bits(0x43A2_F983) + f32::from_bits(0x3F00_0000));
    let wobble = sine(g, second.wrapping_add(0x200)) * size;
    g.writef(entry + 0x6C, wobble);
    // trail points near the viewer
    let write_index = g.read(trail + 0x16C) as i32;
    let slot = write_index % 3;
    let mut point = trail
        .wrapping_add((slot.wrapping_mul(3) as u32).wrapping_mul(4))
        .wrapping_add(8);
    let mut walker = slot;
    loop {
        let limit = if g.read(trail + 0x174) != 0 {
            g.read(trail + 0x170)
        } else {
            g.read(trail + 0x16C)
        } as i32;
        if walker >= limit {
            break;
        }
        let viewer = g.read(VIEWER);
        let dy = g.readf(viewer + 0xAC) - g.readf(point.wrapping_sub(4));
        let dz = g.readf(viewer + 0xB0) - g.readf(point);
        let dx = g.readf(viewer + 0xA8) - g.readf(point.wrapping_sub(8));
        let distance = (dx * dx + dy * dy) + dz * dz;
        g.writef(fade_slot, distance);
        if f32::from_bits(0x3DCC_CCCD) > distance {
            let root = f64::from(distance).sqrt() as f32;
            g.writef(fade_slot, root);
            let divisor = if root == 0.0 { one } else { root };
            let dx = g.readf(viewer + 0xA8) - g.readf(point.wrapping_sub(8));
            let dy = g.readf(viewer + 0xAC) - g.readf(point.wrapping_sub(4));
            let dz = g.readf(viewer + 0xB0) - g.readf(point);
            g.writef(offset_slot, dx);
            g.writef(offset_slot + 4, dy);
            let inverse = one / divisor;
            g.set_vec3(offset_slot, [inverse * dx, inverse * dy, inverse * dz]);
            g.call(0xB91F0, &[record]);
            let linked = g.at(VIEWER, 0x14);
            g.write(record, linked);
            g.write(record + 0x20, 0x3E4C_CCCD);
            g.write(record + 0x14, 0x15);
            g.write(record + 0x18, 0x15);
            g.write(record + 0x28, point.wrapping_sub(8));
            g.write(record + 0x30, offset_slot);
            g.call(0xBB6C0, &[record]);
        }
        walker += 3;
        point = point.wrapping_add(0x24);
    }
    let clock = g.read(CLOCK) as i32;
    if !(clock < (g.read(trail + 0x168) as i32).wrapping_add(2)) {
        let slot = g.read(trail + 0x16C);
        let base = trail.wrapping_add(slot.wrapping_mul(12));
        for (offset, source) in [(0u32, 0x1Cu32), (4, 0x20), (8, 0x24)] {
            let word = g.read(entry + source);
            g.write(base.wrapping_add(offset), word);
        }
        let clock = g.read(CLOCK);
        let next = g.read(trail + 0x16C).wrapping_add(1);
        g.write(trail + 0x168, clock);
        g.write(trail + 0x16C, next);
        if !((next as i32) < (g.read(trail + 0x170) as i32)) {
            g.write(trail + 0x16C, 0);
            g.write(trail + 0x174, 1);
        }
    }
    let dt = (one / rate) * g.readf(super::eff_update::STEP_SCALE);
    let velocity = g.vec3(entry + 0x64);
    g.set_vec3(step, [dt * velocity[0], dt * velocity[1], dt * velocity[2]]);
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let mut target = g.call(0xDCB80, &[step, hit, normal, ray, entry]);
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    let source = g.read(entry + 0x108);
    if source == target && f32::from_bits(0x3DCC_CCCD) > g.readf(entry + 0x98) {
        target = 0;
    }
    let position = entry + 0x1C;
    let viewer = g.read(VIEWER);
    let (cx, cy, cz) = (
        g.readf(viewer + 0xA8) - g.readf(position),
        g.readf(viewer + 0xAC) - g.readf(entry + 0x20),
        g.readf(viewer + 0xB0) - g.readf(entry + 0x24),
    );
    let near = (cx * cx + cy * cy) + cz * cz;
    let unit_normal;
    if f32::from_bits(0x3E4C_CCCD) > near {
        for (axis, offset) in [(0u32, 0xA8u32), (1, 0xAC), (2, 0xB0)] {
            let word = g.read(viewer + offset);
            g.write(hit + 4 * axis, word);
        }
        target = g.read(viewer + 0x14);
        g.set_vec3(normal, [0.0, 1.0, 0.0]);
        unit_normal = [0.0, 1.0, 0.0];
    } else {
        unit_normal = g.vec3(normal);
    }
    let latched_before = g.read(trail + 0x17C);
    let mut blocked = target == 0;
    if !blocked && g.read(target + 0x94) & 0xFFFF == 0x1B9 {
        let shape = g.read(target + 4);
        blocked = g.read(shape + 0x30) & 2 == 0;
    }
    if !blocked && latched_before != 0 {
        blocked = true;
    }
    if !blocked {
        drone_hit(
            g,
            entry,
            trail,
            target,
            unit_normal,
            (hit, normal, record, contact, offset_slot),
        );
    }
    if g.read(trail + 0x17C) == 0 {
        let moved = g.vec3(step);
        let x = g.readf(position) + moved[0];
        g.writef(position, x);
        let y = moved[1] + g.readf(entry + 0x20);
        g.writef(entry + 0x20, y);
        let z = g.readf(entry + 0x24) + moved[2];
        g.writef(entry + 0x24, z);
        return;
    }
    let decay = (one / rate) * (g.read(super::eff_update::STEPS) as i32 as f32);
    let fade = g.readf(trail + 0x178) - decay;
    g.writef(trail + 0x178, fade);
    if 0.0 > fade {
        let first = g.read(trail + 0x180);
        g.call(0x10E2A0, &[first]);
        let second = g.read(trail + 0x184);
        g.call(0x10E2A0, &[second]);
        let block = g.read(entry + 0x208);
        g.call(0x3EAD0, &[block]);
        g.call(0xDC550, &[entry]);
        return;
    }
    let capacity = g.read(trail + 0x170) as i32 as f32;
    let span = ((one / rate) * capacity) * f32::from_bits(0x4000_0000);
    let threshold = one - span;
    let mut fraction = 0.0f32;
    if fade > threshold {
        fraction = (fade - threshold) / span;
    }
    g.writef(fade_slot, fraction);
    let first = g.read(trail + 0x180);
    if g.call(0x10E210, &[first]) != 0 {
        let level = g.readf(fade_slot) * f32::from_bits(0x40E0_0000);
        let first = g.read(trail + 0x180);
        g.call(0x10EB90, &[first, level.to_bits()]);
    }
    let second = g.read(trail + 0x184);
    if g.call(0x10E210, &[second]) != 0 {
        let level = g.readf(fade_slot) * f32::from_bits(0x40E0_0000);
        let second = g.read(trail + 0x184);
        g.call(0x10EB90, &[second, level.to_bits()]);
    }
}

#[allow(clippy::too_many_arguments)]
fn drone_hit(
    g: &mut dyn Guest,
    entry: u32,
    trail: u32,
    target: u32,
    n: [f32; 3],
    frame: (u32, u32, u32, u32, u32),
) {
    let (hit, normal, record, contact, _offset) = frame;
    let (reflected, unit_slot) = (FRAME - 0x1DC, FRAME - 0x1AC);
    let position = entry + 0x1C;
    let v = g.vec3(entry + 0x64);
    let length = (((f64::from(v[2]) * f64::from(v[2])) + (f64::from(v[1]) * f64::from(v[1])))
        + (f64::from(v[0]) * f64::from(v[0])))
    .sqrt() as f32;
    g.writef(FRAME - 0x1E0, length);
    let source = g.read(entry + 0x108);
    let inverse = f32::from_bits(0x3F80_0000) / length;
    let (uz, ux) = (v[2] * inverse, v[0] * inverse);
    let uy = inverse * v[1];
    let dot = ((uy * n[1]) + (ux * n[0])) + (uz * n[2]);
    let twice = dot * f32::from_bits(0x4000_0000);
    g.writef(FRAME - 0x1A8, uy);
    g.writef(FRAME - 0x1A4, uz);
    g.writef(unit_slot, ux);
    let damp = f32::from_bits(0x3E99_999A);
    let (px, py, pz) = (n[1] * twice, n[0] * twice, n[2] * twice);
    let hit_point = g.vec3(hit);
    let dx = hit_point[0] - g.readf(position);
    g.writef(reflected + 4, uy - px * damp);
    let dz = hit_point[2] - g.readf(entry + 0x24);
    g.writef(reflected, ux - py * damp);
    let dy = hit_point[1] - g.readf(entry + 0x20);
    g.writef(reflected + 8, uz - pz * damp);
    let travelled = (((dz * dz) + (dy * dy)) + (dx * dx)) + g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, travelled);
    let target_type = g.read(target + 0x20);
    if target_type == 8 {
        let kind = g.read(entry);
        g.call(
            0xE9FE0,
            &[entry, kind, target, hit, normal, reflected, source],
        );
        if g.read(target + 0x150) == 0 {
            let owner = g.read(entry + 0x10C);
            let block = g.read(target + 0x7C);
            g.call(0x126260, &[block + 0xC68, owner]);
        }
        g.call(0xDC5D0, &[entry]);
    }
    let target_type = g.read(target + 0x20);
    g.call(0xE94E0, &[entry, target_type, hit, 0x40C0_0000]);
    g.copy(super::eff_helpers::CONTACT_LIVE, contact, 8);
    let surface = g.read(contact + 0x18);
    if (surface as i32) >= 0 && g.read(contact + 0x1C) & 0x0100_0000 == 0 {
        let kind = g.read(entry);
        let sort = g.read(entry + 0xF0);
        g.call(0x114AF0, &[kind, 0, sort, target, hit, normal]);
    }
    g.call(0xB91F0, &[record]);
    g.call(0xB9220, &[record, entry]);
    g.write(record, target);
    g.write(record + 0x28, position);
    g.write(record + 0x2C, normal);
    g.write(record + 0x30, unit_slot);
    g.writef(record + 0x34, length);
    g.write(record + 0x40, 1);
    if g.read(target + 0x94) & 0xFFFF == 0x1B9 {
        g.write(record + 0x20, 0x40C0_0000);
        g.call(0xBB6C0, &[record]);
    } else {
        g.call(0xBD0F0, &[record]);
    }
    g.writef(trail + 0x178, 1.0);
    g.write(trail + 0x17C, 1);
    g.call(0x1D7130, &[position]);
}
