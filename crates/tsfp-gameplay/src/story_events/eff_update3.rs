//! T1876: the effect update callback `0xEAD10` (types `0x20`, `0x21`: thrown parts that stick to what they hit) and its helper `0xE2ED0`, ported from the
//! original and validated bit for bit against it under the Unicorn oracle (`tools/t1846_gold/eff_update3.py`, test `tests/story_events_t1876_update3.rs`).
//! Same [`Guest`] method and frame rule as `eff_update` / `eff_update2`. Behaviour MEASURED (xemu level), names INFERRED.

use super::eff_guest::Guest;
use super::eff_helpers::CONTACT_LIVE;
use super::eff_update::{FRAME, FRAME_RATE, STEP_SCALE, STEPS};
use super::eff_update2::ftol;

const PLAYER_FIRST: u32 = 0x0079_0950;
const MODE_FLAGS: u32 = 0x007D_E458;
const MODE_BYTE: u32 = 0x007D_E455;
const RADIANS: f32 = f32::from_bits(0x3C8E_FA35);
const DEGREES: f32 = f32::from_bits(0x4265_2EE0);

fn local(offset: u32) -> u32 {
    FRAME - offset
}

fn zero_translation(g: &mut dyn Guest) {
    for offset in [0x234, 0x230, 0x22C] {
        g.writef(local(offset), 0.0);
    }
}

fn radians(g: &mut dyn Guest, address: u32) -> f32 {
    g.readf(address) * RADIANS
}

/// `0xE2ED0(entry in edi, owner in esi)`: the part expiry. For types `0x20` / `0x21` with a live fade (`owner + 0x1A0`) the countdown `owner + 0x1A4` falls
/// by the frame steps; while it is within `1.5 * rate` the fade sound `0x94220(owner + 4, 15, -1, -1, trunc(127 * count / (1.5 * rate)))` is set; at 0 the
/// attachment is dropped (`entry + 0x158 = 0`, `+0x15C = -1`), the owner is released (`0xA9070`) and the result is 1.
pub fn part_expire(g: &mut dyn Guest, entry: u32, owner: u32) -> bool {
    let entry_type = g.read(entry);
    if entry_type != 0x20 && entry_type != 0x21 {
        return false;
    }
    if g.read(owner + 0x1A0) == 0 {
        return false;
    }
    let count = g.read(owner + 0x1A4).wrapping_sub(g.read(STEPS));
    g.write(owner + 0x1A4, count);
    let limit = (g.read(FRAME_RATE) as i32 as f32) * f32::from_bits(0x3FC0_0000);
    let current = count as i32 as f32;
    g.writef(FRAME - 0x8, current);
    g.writef(FRAME - 0x4, limit);
    if !(limit < current) {
        let level =
            ftol((f64::from(current) * f64::from(f32::from_bits(0x42FE_0000))) / f64::from(limit))
                as u32;
        let handle = g.read(owner + 4);
        g.call(0x94220, &[handle, 0xF, u32::MAX, u32::MAX, level]);
    }
    if (g.read(owner + 0x1A4) as i32) > 0 {
        return false;
    }
    g.write(entry + 0x158, 0);
    g.write(entry + 0x15C, u32::MAX);
    g.call(0xA9070, &[owner]);
    true
}

/// `0xEAD10(entry)`: update of the thrown parts (types `0x20`, `0x21`; the entry's owner `+0xA8` is the visible object). Nothing happens while the frame
/// steps are 0. Names INFERRED, flow MEASURED.
///
/// * AT REST (velocity exactly 0): the owner flag `+0x18 |= 2`; an attached target (`+0x158`) is followed: the stored offset `+0x160 ..` and direction `+0x16C ..`
///   go through the target's bone matrix (`0x1531D0`, `0x151790` / `0x1534E0`, `0x151E80`) into the position and `+0x180 ..`, the owner takes the position
///   and the angles of the direction; the expiry `0xE2ED0` may drop the attachment.
/// * MOVING: the step `velocity * scale` is swept (`0xDCB80`, the aim vector `(0, 0, 1)` rotated by the owner's angles). A HIT on type 2 objects drops the
///   attachment; on a plain object the part sticks (`+0x158`, bone byte of the skeleton table by contact word 1); on an ACTOR it plays `0xDC5D0` / the sound
///   `0x217B0(0xCB9)`, reacts (`0xE9FE0`), is pushed out along the normal, scanned (`0xD9760`) and bound to a bone found through up to five `0xD6A80` draws; a
///   hit record `0x160020` follows for players at or above the first local player. The stored offset / direction are re-expressed in the target's frame
///   (inverse matrices `0x152260`), the position follows the hit, the part stops (velocity 0), `0xE94E0` and a hit record `0xBD0F0` close.
///   No hit: the part moves by the step with a falling `-0.1 / rate * scale` y velocity damped by 0.99, a looping sound (`0x101660`, `0xFF420(5)`) follows it
///   and the owner is aimed along the velocity.
pub fn part_update(g: &mut dyn Guest, entry: u32) {
    let owner = g.read(entry + 0xA8);
    let block = g.read(owner + 0x7C);
    g.write(local(0x280), block);
    if g.read(STEPS) == 0 {
        return;
    }
    let velocity = g.vec3(entry + 0x64);
    if velocity[0] != 0.0 || velocity[1] != 0.0 || velocity[2] != 0.0 {
        return part_moving(g, entry, owner, block);
    }
    part_at_rest(g, entry, owner);
}

fn part_at_rest(g: &mut dyn Guest, entry: u32, owner: u32) {
    let flags = g.read(owner + 0x18) | 2;
    g.write(owner + 0x18, flags);
    let attached = g.read(entry + 0x158);
    if attached == 0 {
        return;
    }
    let owner_now = g.read(entry + 0xA8);
    if part_expire(g, entry, owner_now) {
        return;
    }
    let cell = g.read(attached + 0x34);
    let owner_now = g.read(entry + 0xA8);
    g.write(owner_now + 0x34, cell);
    let (offset, direction, matrix) = (local(0x2A4), local(0x2BC), local(0x264));
    g.copy(entry + 0x160, offset, 3);
    g.copy(entry + 0x16C, direction, 3);
    let bone = g.read(entry + 0x15C);
    if (bone as i32) >= 0 {
        let part = g.at(attached + 4, 0x10).wrapping_add(bone << 6);
        g.call(0x1531D0, &[matrix, part]);
        g.call(0x151E80, &[matrix, offset]);
        zero_translation(g);
        g.call(0x151E80, &[matrix, direction]);
    }
    attached_matrix(g, attached, matrix);
    g.call(0x151E80, &[matrix, offset]);
    zero_translation(g);
    g.call(0x151E80, &[matrix, direction]);
    let (x, y, z) = (g.read(offset), g.read(offset + 4), g.read(offset + 8));
    g.write(entry + 0x20, y);
    g.write(entry + 0x24, z);
    for axis in 0..3u32 {
        let word = g.read(direction + 4 * axis);
        g.write(entry + 0x180 + 4 * axis, word);
    }
    g.write(entry + 0x1C, x);
    let owner_now = g.read(entry + 0xA8);
    if owner_now == 0 {
        return;
    }
    g.write(owner_now + 0x64, x);
    let (py, pz) = (g.read(entry + 0x20), g.read(entry + 0x24));
    let owner_now = g.read(entry + 0xA8);
    g.write(owner_now + 0x68, py);
    let owner_now = g.read(entry + 0xA8);
    g.write(owner_now + 0x6C, pz);
    let (dx, dy, dz) = (
        -g.readf(direction),
        g.readf(direction + 4),
        -g.readf(direction + 8),
    );
    let degrees = f64::from(DEGREES);
    let yaw = f64::from(dz).atan2(f64::from(dx)) * degrees;
    let owner_now = g.read(entry + 0xA8);
    g.writef(
        owner_now + 0x70,
        (-yaw - f64::from(f32::from_bits(0x42B4_0000))) as f32,
    );
    let owner_now = g.read(entry + 0xA8);
    let (dy, dz, dx) = (
        -f64::from(dy),
        f64::from(g.readf(direction + 8)),
        f64::from(g.readf(direction)),
    );
    let flat = (dz * dz + dx * dx).sqrt();
    g.writef(owner_now + 0xC0, (dy.atan2(flat) * degrees) as f32);
}

/// The rotation matrix of the attached object into `matrix` (`0x151790` for objects of type byte 2, else `0x1534E0`).
fn attached_matrix(g: &mut dyn Guest, attached: u32, matrix: u32) {
    let (x, y, z) = (
        g.read(attached + 0x64),
        g.read(attached + 0x68),
        g.read(attached + 0x6C),
    );
    if g.byte(attached + 0x32) == 2 {
        let (a, b, c) = (
            radians(g, attached + 0xC8),
            radians(g, attached + 0x70),
            radians(g, attached + 0xC0),
        );
        g.call(
            0x151790,
            &[matrix, x, y, z, c.to_bits(), b.to_bits(), a.to_bits()],
        );
    } else {
        let angle = radians(g, attached + 0x70);
        g.call(0x1534E0, &[matrix, x, y, z, angle.to_bits()]);
    }
}

fn part_moving(g: &mut dyn Guest, entry: u32, owner: u32, block0: u32) {
    let (dir, hit, offset, step, aim, unit, matrix, contact) = (
        local(0x2BC),
        local(0x2B0),
        local(0x2A4),
        local(0x298),
        local(0x28C),
        local(0x278),
        local(0x264),
        local(0x1C8),
    );
    let (record, record2, ray, rotation) = (local(0x224), local(0x1A8), local(0x10C), local(0x14C));
    g.writef(aim, 0.0);
    g.writef(aim + 4, 0.0);
    g.writef(aim + 8, 1.0);
    let flags = g.read(owner + 0x18) | 2;
    g.write(owner + 0x18, flags);
    let scale = g.readf(STEP_SCALE);
    for axis in 0..3u32 {
        let value = g.readf(entry + 0x64 + 4 * axis) * scale;
        g.writef(step + 4 * axis, value);
    }
    let owner_now = g.read(entry + 0xA8);
    let (a, b, c) = (
        radians(g, owner_now + 0xC8),
        radians(g, owner_now + 0x70),
        radians(g, owner_now + 0xC0),
    );
    g.call(0x151650, &[rotation, c.to_bits(), b.to_bits(), a.to_bits()]);
    g.call(0x151E80, &[rotation, aim]);
    g.write(ray, 0);
    g.write(ray + 4, 0);
    let target = g.call(0xDCB80, &[step, hit, dir, ray, entry]);
    g.copy(CONTACT_LIVE, contact, 8);
    if (g.read(ray) as i32) > 0 {
        g.call(0xDCEF0, &[entry, ray]);
    }
    if target == 0 {
        return part_flying(g, entry, step);
    }
    let velocity = g.vec3(entry + 0x64);
    let position = g.vec3(entry + 0x1C);
    let hit_point = g.vec3(hit);
    let length = (((f64::from(velocity[2]) * f64::from(velocity[2]))
        + (f64::from(velocity[1]) * f64::from(velocity[1])))
        + (f64::from(velocity[0]) * f64::from(velocity[0])))
    .sqrt() as f32;
    g.writef(local(0x27C), length);
    g.set_vec3(offset, hit_point);
    let (dz, dy, dx) = (
        hit_point[2] - position[2],
        hit_point[1] - position[1],
        hit_point[0] - position[0],
    );
    let inverse = f32::from_bits(0x3F80_0000) / length;
    let (ux, uy, uz) = (
        inverse * velocity[0],
        inverse * velocity[1],
        inverse * velocity[2],
    );
    g.writef(local(0x270), uz);
    let travelled = ((dz * dz + dy * dy) + dx * dx) + g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, travelled);
    g.writef(unit, ux);
    g.writef(unit + 4, uy);
    let target_type = g.read(target + 0x20);
    if target_type == 2 {
        g.write(entry + 0x158, 0);
        g.write(entry + 0x15C, u32::MAX);
        g.write(block0 + 0xC0, 2);
    } else if target_type != 8 {
        let skeleton = skeleton_base(g, target);
        g.copy(CONTACT_LIVE, matrix, 8);
        g.write(entry + 0x158, target);
        let flags = g.read(target + 0x28) | 0x10000;
        g.write(target + 0x28, flags);
        let word = g.read(matrix + 4);
        if (word as i32) >= 0 {
            let bone = sbyte_at(
                g,
                word.wrapping_mul(0xC8)
                    .wrapping_add(skeleton)
                    .wrapping_add(0x49),
            );
            g.write(entry + 0x15C, bone);
        }
        g.call(0xFFC50, &[0, hit]);
        g.write(block0 + 0xC0, 2);
    } else {
        let actor_block = g.read(target + 0x7C);
        let skeleton = skeleton_base(g, target);
        actor_hit(
            g,
            entry,
            target,
            actor_block,
            skeleton,
            (dir, hit, step, record, record2, unit),
        );
        let saved = g.read(local(0x280));
        g.write(saved + 0xC0, 1);
    }
    part_stuck(g, entry, target);
}

fn skeleton_base(g: &mut dyn Guest, target: u32) -> u32 {
    let shape = g.read(target + 4);
    let table = g.read(shape + 4);
    let first = g.read(table);
    table.wrapping_sub(first.wrapping_mul(0xC8))
}

fn sbyte_at(g: &mut dyn Guest, address: u32) -> u32 {
    i32::from(g.byte(address) as u8 as i8) as u32
}

fn actor_hit(
    g: &mut dyn Guest,
    entry: u32,
    target: u32,
    block: u32,
    skeleton: u32,
    frame: (u32, u32, u32, u32, u32, u32),
) {
    let (dir, hit, step, record, _record2, unit) = frame;
    g.call(0xDC5D0, &[entry]);
    let entry_type = g.read(entry);
    if entry_type == 0x20 && g.byte(MODE_BYTE) != 0xA && g.read(MODE_FLAGS) & 0x40 == 0 {
        let flags = g.read(MODE_FLAGS) & 0xFF;
        let attached = g.read(entry + 0x104);
        let mut hurt = true;
        let mut who = u32::MAX;
        if attached != 0 {
            who = g.read(attached + 8);
            if flags & 1 != 0 && g.read(attached + 0x84) == g.read(block + 0x84) && flags & 8 == 0 {
                hurt = false;
            }
        }
        if hurt {
            g.call(0x124EA0, &[block + 0xC68, who]);
        }
    }
    let entry_type = g.read(entry);
    if (entry_type == 0x20 || entry_type == 0x21) && g.call(0x278040, &[block]) != 0 {
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x1A0, 1);
        let rate = g.read(FRAME_RATE);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x1A4, rate.wrapping_mul(15));
    }
    g.call(0x217B0, &[0xCB9, hit]);
    let (kind, source) = (g.read(entry), g.read(entry + 0x108));
    g.call(0xE9FE0, &[entry, kind, target, hit, dir, dir, source]);
    let n = g.vec3(dir);
    let h = g.vec3(hit);
    g.writef(entry + 0x1C, n[0] + h[0]);
    g.writef(entry + 0x20, n[1] + h[1]);
    g.writef(entry + 0x24, n[2] + h[2]);
    g.set_vec3(step, [0.0 - n[0], 0.0 - n[1], 0.0 - n[2]]);
    g.call(0xD9760, &[target, entry + 0x1C, step, 0, 1, hit, dir, 0]);
    g.write(entry + 0x158, target);
    let flags = g.read(target + 0x28) | 0x10000;
    g.write(target + 0x28, flags);
    let index = g.call(0xD6A80, &[]);
    let mut bone = sbyte_at(
        g,
        index
            .wrapping_mul(0xC8)
            .wrapping_add(skeleton)
            .wrapping_add(0x49),
    );
    g.write(entry + 0x15C, bone);
    if (bone as i32) < 0 {
        let index = g.call(0xD6A80, &[]);
        let link = sbyte_at(
            g,
            index
                .wrapping_mul(0xC8)
                .wrapping_add(skeleton)
                .wrapping_add(0x4A),
        );
        if (link as i32) >= 0 {
            let index = g.call(0xD6A80, &[]);
            let next = sbyte_at(
                g,
                index
                    .wrapping_mul(0xC8)
                    .wrapping_add(skeleton)
                    .wrapping_add(0x4A),
            );
            bone = sbyte_at(
                g,
                next.wrapping_mul(0xC8)
                    .wrapping_add(skeleton)
                    .wrapping_add(0x49),
            );
            g.write(entry + 0x15C, bone);
        }
    }
    if (g.read(entry + 0x15C) as i32) < 0 {
        let index = g.call(0xD6A80, &[]);
        let link = sbyte_at(
            g,
            index
                .wrapping_mul(0xC8)
                .wrapping_add(skeleton)
                .wrapping_add(0x4B),
        );
        if (link as i32) >= 0 {
            let index = g.call(0xD6A80, &[]);
            let next = sbyte_at(
                g,
                index
                    .wrapping_mul(0xC8)
                    .wrapping_add(skeleton)
                    .wrapping_add(0x4B),
            );
            let bone = sbyte_at(
                g,
                next.wrapping_mul(0xC8)
                    .wrapping_add(skeleton)
                    .wrapping_add(0x49),
            );
            g.write(entry + 0x15C, bone);
        }
    }
    if g.read(target + 0x28) & 0x20_0000 == 0 {
        g.write(entry + 0x15C, u32::MAX);
    }
    let block_now = g.read(target + 0x7C);
    if (g.read(block_now + 8) as i32) >= (g.read(PLAYER_FIRST) as i32) {
        g.call(0xB91F0, &[record]);
        g.call(0xB9220, &[record, entry]);
        let length = g.read(local(0x27C));
        g.write(record, target);
        g.write(record + 0x28, entry + 0x1C);
        g.write(record + 0x2C, dir);
        g.write(record + 0x30, unit);
        g.write(record + 0x34, length);
        g.call(0x160020, &[target, 0, record]);
    }
}

/// The tail shared by every hit (`0xEB508 ..`): an attached part re-expresses the hit offset and direction in the target's frame, then the part is placed,
/// stopped and recorded.
fn part_stuck(g: &mut dyn Guest, entry: u32, target: u32) {
    let (dir, hit, offset, aim, unit, matrix, inverse, contact) = (
        local(0x2BC),
        local(0x2B0),
        local(0x2A4),
        local(0x28C),
        local(0x278),
        local(0x264),
        local(0x224),
        local(0x1C8),
    );
    let record = local(0x1A8);
    if g.read(entry + 0x158) != 0 {
        if g.byte(target + 0x32) == 2 {
            let (a, b, c) = (
                radians(g, target + 0xC8),
                radians(g, target + 0x70),
                radians(g, target + 0xC0),
            );
            g.call(0x151650, &[matrix, c.to_bits(), b.to_bits(), a.to_bits()]);
        } else {
            let angle = radians(g, target + 0x70);
            g.call(0x153440, &[matrix, angle.to_bits()]);
        }
        for axis in 0..3u32 {
            let value = g.readf(hit + 4 * axis) - g.readf(target + 0x64 + 4 * axis);
            g.writef(hit + 4 * axis, value);
        }
        g.copy(aim, dir, 3);
        g.call_f(0x152260, &[inverse, matrix]);
        g.call(0x151E80, &[inverse, hit]);
        g.call(0x151E80, &[inverse, dir]);
        let bone = g.read(entry + 0x15C);
        if (bone as i32) >= 0 {
            let part = g.at(target + 4, 0x10).wrapping_add(bone << 6);
            g.call(0x1531D0, &[matrix, part]);
            for (axis, translation) in [(0u32, 0x234u32), (1, 0x230), (2, 0x22C)] {
                let value = g.readf(hit + 4 * axis) - g.readf(local(translation));
                g.writef(hit + 4 * axis, value);
            }
            zero_translation(g);
            g.call_f(0x152260, &[inverse, matrix]);
            g.call(0x151E80, &[inverse, hit]);
            g.call(0x151E80, &[inverse, dir]);
        }
        let (x, y, z) = (g.readf(dir), g.readf(dir + 4), g.readf(dir + 8));
        let squared = (z * z + y * y) + x * x;
        let (mut nx, mut ny, mut nz) = (x, y, z);
        if squared > f32::from_bits(0x33D6_BF95) {
            g.writef(local(0x268), squared);
            g.writef(local(0x280), 1.0);
            let factor = (1.0f32 / squared.sqrt()) * 1.0;
            g.writef(local(0x26C), factor);
            nx = factor * x;
            ny = factor * y;
            nz = factor * z;
            g.writef(dir, nx);
            g.writef(dir + 4, ny);
            g.writef(dir + 8, nz);
        }
        g.copy(hit, entry + 0x160, 3);
        g.writef(entry + 0x16C, nx);
        g.writef(entry + 0x170, ny);
        g.writef(entry + 0x174, nz);
    }
    let position = entry + 0x1C;
    if g.read(contact + 8) == 2 {
        place_at_hit(g, entry, offset);
    } else {
        let reach = f32::from_bits(0x3D99_999A);
        if g.read(entry + 0x158) != 0 {
            for axis in 0..3u32 {
                let value =
                    g.readf(entry + 0x16C + 4 * axis) * reach + g.readf(entry + 0x160 + 4 * axis);
                g.writef(entry + 0x160 + 4 * axis, value);
            }
            place_at_hit(g, entry, offset);
        } else {
            for axis in 0..3u32 {
                let value = g.readf(aim + 4 * axis) * reach + g.readf(offset + 4 * axis);
                g.writef(position + 4 * axis, value);
            }
        }
    }
    let handle = g.read(entry + 0xB4);
    g.call(0xFF490, &[handle, position]);
    g.call(0xDC2B0, &[entry, 2]);
    g.copy(dir, entry + 0x180, 3);
    for offset_word in [0x64, 0x68, 0x6C] {
        g.writef(entry + offset_word, 0.0);
    }
    let owner = g.read(entry + 0xA8);
    let word = g.read(position);
    g.write(owner + 0x64, word);
    let word = g.read(entry + 0x20);
    let owner = g.read(entry + 0xA8);
    g.write(owner + 0x68, word);
    let word = g.read(entry + 0x24);
    let owner = g.read(entry + 0xA8);
    g.write(owner + 0x6C, word);
    let target_type = g.read(target + 0x20);
    g.call(0xE94E0, &[entry, target_type, hit, 0x40C0_0000]);
    g.call(0xB91F0, &[record]);
    g.call(0xB9220, &[record, entry]);
    let length = g.read(local(0x27C));
    g.write(record, target);
    g.write(record + 0x28, position);
    g.write(record + 0x2C, dir);
    g.write(record + 0x30, unit);
    g.write(record + 0x34, length);
    g.call(0xBD0F0, &[record]);
}

fn place_at_hit(g: &mut dyn Guest, entry: u32, offset: u32) {
    g.copy(offset, entry + 0x1C, 1);
    g.copy(offset + 4, entry + 0x20, 1);
    g.copy(offset + 8, entry + 0x24, 1);
}

/// No hit: the part flies on: position by the step, `+0xF4`, a falling y velocity and 0.99 damping, the looping sound and the owner's aim.
fn part_flying(g: &mut dyn Guest, entry: u32, step: u32) {
    let slot = entry + 0xB4;
    let position = entry + 0x1C;
    if g.read(slot) == 0 {
        let handle = g.call(0x101660, &[slot, 0xA, 0, position, 0xFFFF]);
        g.write(slot, handle);
        g.call(0xFF420, &[handle, 0x40A0_0000]);
    }
    let (sx, sy, sz) = (g.readf(step), g.readf(step + 4), g.readf(step + 8));
    let handle = g.read(slot);
    let x = sx + g.readf(position);
    g.writef(position, x);
    let y = g.readf(entry + 0x20) + sy;
    g.writef(entry + 0x20, y);
    let z = sz + g.readf(entry + 0x24);
    g.writef(entry + 0x24, z);
    let travelled = ((sz * sz + sy * sy) + sx * sx) + g.readf(entry + 0xF4);
    g.writef(entry + 0xF4, travelled);
    let rate = g.read(FRAME_RATE) as i32 as f32;
    let fall = (f32::from_bits(0xBDCC_CCCD) / rate) * g.readf(STEP_SCALE) + g.readf(entry + 0x68);
    g.writef(entry + 0x68, fall);
    let damp = f32::from_bits(0x3F7D_70A4);
    for offset in [0x64, 0x68, 0x6C] {
        let value = g.readf(entry + offset) * damp;
        g.writef(entry + offset, value);
    }
    g.call(0xFF490, &[handle, position]);
    let owner = g.read(entry + 0xA8);
    if owner == 0 {
        return;
    }
    let (vx, vy, vz) = (
        f64::from(g.readf(entry + 0x64)),
        f64::from(g.readf(entry + 0x68)),
        f64::from(g.readf(entry + 0x6C)),
    );
    let degrees = f64::from(DEGREES);
    g.writef(owner + 0x70, (-(vz.atan2(vx) * degrees) - 90.0) as f32);
    let owner = g.read(entry + 0xA8);
    let flat = (vz * vz + vx * vx).sqrt();
    g.writef(owner + 0xC0, (vy.atan2(flat) * degrees) as f32);
    let owner = g.read(entry + 0xA8);
    let word = g.read(position);
    g.write(owner + 0x64, word);
    let word = g.read(entry + 0x20);
    let owner = g.read(entry + 0xA8);
    g.write(owner + 0x68, word);
    let word = g.read(entry + 0x24);
    let owner = g.read(entry + 0xA8);
    g.write(owner + 0x6C, word);
}

// ---------------------------------------------------------------------------------------------------------------------------------------
// 0xE3170, the hit helper of the flare update 0xF1A90 (the update itself is `eff_flare::flare_update`)
// ---------------------------------------------------------------------------------------------------------------------------------------

/// `0x350E70(block, mask)`: a flag of the block's class row. The row is the signed word `+0x48` of the block times 20 bytes in the table `0x53C508` (image
/// data the host maps itself). With bit 0 of the mask and a hit in the row the answer is 1; otherwise in the mode that shows everything (mode byte `10` or mode
/// flag `0x80000`) the answer is `row & mask`, else 0.
pub fn block_flag(g: &mut dyn Guest, block: u32, mask: u32) -> u32 {
    let index = i32::from(g.read(block + 0x48) as u16 as i16);
    let row = 0x0053_C508u32.wrapping_add((index.wrapping_mul(5) as u32).wrapping_mul(4));
    if mask & 1 != 0 && g.read(row) & mask != 0 {
        return 1;
    }
    if g.byte(MODE_BYTE) == 0xA || g.read(MODE_FLAGS) & 0x8_0000 != 0 {
        return g.read(row) & mask;
    }
    0
}

/// `0xE3170(target in eax, entry in ecx; stack: hit, normal)`: what a hit does to the target. An actor takes damage `0x1252C0(block + 0xC68, 1.0 or 0, source)`
/// unless the same team is protected (mode flags 1 and not 8), `0x350E70(block, 0x4000)` choosing the damage factor, and `0xDC5D0`; the result is 1. A
/// non actor with flag 2 in the byte `+0x9C` gets a hit record `0xBB6C0` (result 1); otherwise 0.
pub fn flare_hit(g: &mut dyn Guest, target: u32, entry: u32, hit: u32, normal: u32) -> bool {
    let record = FRAME - 0x214 - 0x5C;
    if g.read(target + 0x20) == 8 {
        let attached = g.read(entry + 0x104);
        let block = g.read(target + 0x7C);
        let same_team = attached != 0 && g.read(block + 0x84) == g.read(attached + 0x84);
        let flags = g.read(MODE_FLAGS) & 0xFF;
        if !(flags & 1 != 0 && flags & 8 == 0 && same_team) {
            let reply = block_flag(g, block, 0x4000);
            let source = g.read(entry + 0x10C);
            let factor = if reply != 0 { 0 } else { 0x3F80_0000 };
            g.call(0x1252C0, &[block.wrapping_add(0xC68), factor, source]);
        }
        g.call(0xDC5D0, &[entry]);
        return true;
    }
    if g.byte(target + 0x9C) & 2 == 0 {
        return false;
    }
    g.call(0xB91F0, &[record]);
    g.call(0xB9220, &[record, entry]);
    g.writef(record + 0x20, 0.0);
    g.write(record, target);
    g.write(record + 0x28, hit);
    g.write(record + 0x2C, normal);
    g.write(record + 0x30, entry + 0x64);
    g.writef(record + 0x34, 1.0);
    g.call(0xBB6C0, &[record]);
    true
}
