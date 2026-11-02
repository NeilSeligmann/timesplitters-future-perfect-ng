//! T1871: the shared helper family of the effect pool update callbacks, ported from the original and validated bit for bit against it under
//! the Unicorn oracle (`tools/t1846_gold/eff_helpers.py`, tests `tests/story_events_t1871_helpers.rs`). Document
//! `docs/t-story-event-effect-update-t1871.md`. Behaviour MEASURED (xemu level), names INFERRED.

// The compares of the original are unordered aware (NaN takes the other branch): `!(a > b)` is not `a <= b`.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::eff_guest::{CLASS_TABLE, ENTRY_BYTES, Guest, POOL_BASE, SCRATCH};

/// Pool range globals of the ground effect loop (`[0x736680]` first index, `[0x737E44]` last index).
pub const RANGE_FIRST: u32 = 0x0073_6680;
pub const RANGE_LAST: u32 = 0x0073_7E44;
/// Class flag word (`+0x2C` of the class record of the type): bit 9 marks the types the ground effect considers.
pub const CLASS_FLAGS: u32 = CLASS_TABLE + 0x2C;

/// `0xF3950(entry, arg)`: release helper of an entry hit by a ground effect. Type `0x13` returns its handle `+0x1B0` (`0x45460`, then -1);
/// a non zero `arg` runs `0x35CB80(entry, arg)`; then the usercall `0xF0430` (esi = entry, ebx = entry + 0x1C: the effect start with
/// `0xEFBD0` / `0x114AF0` and the release `0xDC550`) runs.
pub fn release_hit(g: &mut dyn Guest, entry: u32, arg: u32) {
    if g.read(entry) == 0x13 {
        let handle = g.read(entry + 0x1B0) as i32;
        if handle >= 0 {
            g.call(0x45460, &[handle as u32]);
            g.write(entry + 0x1B0, u32::MAX);
        }
    }
    if arg != 0 {
        g.call(0x35CB80, &[entry, arg]);
    }
    g.call(0xF0430, &[entry, entry + 0x1C]);
}

/// `0xF4920(position, radius, arg)`: the ground effect. Walks the pool slots `[first ..= last]` (`[0x736680]`, `[0x737E44]`); a live
/// entry whose class has flag `0x200`, within `radius` of `position` (squared compare `radius^2 > d^2`, strict, `d^2 = (dz^2 + dy^2) + dx^2`
/// in f32) and without a blocking ray (`0x88C60(entry cell +0xF0, entry position, delta, 0, 0, 0, 0, 0x2000070, 0, 0)` answering 0): type
/// `0x12` raises the counter `+0x178` to at least 3 and the walk goes on, any other type is handed to `0xF3950` and the walk ENDS.
pub fn ground_effect(g: &mut dyn Guest, position: u32, radius: f32, arg: u32) {
    let first = g.read(RANGE_FIRST) as i32;
    let last_plus_one = (g.read(RANGE_LAST) as i32).wrapping_add(1);
    let mut index = first;
    while index < last_plus_one {
        let base = g.read(POOL_BASE);
        let entry = (index as u32).wrapping_mul(ENTRY_BYTES).wrapping_add(base);
        index = index.wrapping_add(1);
        let entry_type = g.read(entry);
        if entry_type == u32::MAX {
            continue;
        }
        if g.read(entry_type.wrapping_shl(6).wrapping_add(CLASS_FLAGS)) & 0x200 == 0 {
            continue;
        }
        let at = g.vec3(position);
        let here = g.vec3(entry + 0x1C);
        let delta = [at[0] - here[0], at[1] - here[1], at[2] - here[2]];
        let distance_squared = (delta[2] * delta[2] + delta[1] * delta[1]) + delta[0] * delta[0];
        if !(radius * radius > distance_squared) {
            continue;
        }
        g.set_vec3(SCRATCH, delta);
        let cell = g.read(entry + 0xF0);
        if g.call(
            0x88C60,
            &[cell, entry + 0x1C, SCRATCH, 0, 0, 0, 0, 0x0200_0070, 0, 0],
        ) != 0
        {
            continue;
        }
        if entry_type == 0x12 {
            if (g.read(entry + 0x178) as i32) < 3 {
                g.write(entry + 0x178, 3);
            }
            continue;
        }
        g.call(0xF3950, &[entry, arg]);
        return;
    }
}

/// `[0x47D000]` (0.07): the height `type 0x14` entries are lowered by for the ray origin.
pub const LOWER_HEIGHT: f32 = f32::from_bits(0x3D8F_5C29);
/// Scratch layout of [`source_ray`]: the ray origin (`from`), the ray vector (`delta`), the entry position and the in/out cell word.
const FROM: u32 = SCRATCH;
const DELTA: u32 = SCRATCH + 0x20;
const POSITION: u32 = SCRATCH + 0x40;
const CELL_OUT: u32 = SCRATCH + 0x60;
/// `[0x739A90]`: when it equals 1 the ray mode word is `0x20` (neither class flag `0x200` nor `0x1000`).
pub const RAY_MODE_GLOBAL: u32 = 0x0073_9A90;
const FLAGS_PLAIN: u32 = 0x0200_0010;
const FLAGS_BIT_0X10: u32 = 0x0201_0010;

/// `0xDCB80` (usercall: esi = entry; stack `a1..a4` = the ray controls of the caller): ray casts of an effect entry against the level from
/// the object it is attached to (`+0x108`), refreshing the entry's cell word `+0xF0` (copied to the owner's `+0x34` / `+0x38`).
/// Returns the ray result (non zero = blocked / hit). Names INFERRED, flow MEASURED:
///
/// 1. The ray mode word is 4 for classes with flag `0x200` or `0x1000`, else `0x20` when `[0x739A90] == 1`, else 0; the ray flags are
///    `0x2000010` (`0x2010010` when the entry has flag `0x10`). Type `0x14` lowers the entry height by `0.07`.
/// 2. FIRST PASS, only when the source is non zero and (`+0x14C != 0` or entry flag `0x20`): `+0x14C = 0`; by `+0x100`: 1 origin from the source's
///    player block (`+0x150`: `+0xB4..+0xBC`, cell `+0xF54`), 2 origin `+0x40..+0x48` with cell `0xC3C50(&origin, 0.1, 0)`, 3 and 4 origin from
///    `0x242E60(source, &origin)`, cell `[player + 0xB0]` and the object `[player + 0x68]` whose flags `+0x28` lose `0x60` for the ray,
///    6 origin = entry position with the entry cell, others origin = source `+0x64..+0x6C` with cell `+0x34`. The source's flag bits `0x60` are
///    cleared; with a positive cell `0x88C60(cell, &origin, &delta, 0, a2, a3, &cell_out, flags, mode, a4)`, `delta = position - origin`;
///    the cell word `+0xF0` takes `cell_out` unless entry flag `0x20`. The owner `+0xA8` gets the cell in `+0x34` and `+0x38`. A non zero result restores
///    the source flags and returns.
/// 3. SECOND PASS (also the only pass without a source): with a positive cell (`cell_out`, or the entry cell) `0x88C60(cell, &position, a1, 0, a2, a3,
///    &cell_out, flags, mode, a4)`; same cell and owner updates; the source flags are restored.
pub fn source_ray(g: &mut dyn Guest, entry: u32, a1: u32, a2: u32, a3: u32, a4: u32) -> u32 {
    let source = g.read(entry + 0x108);
    let initial_cell = g.read(entry + 0xF0);
    let mut cell_out = initial_cell;
    let mut result = 0u32;
    let mut saved_flags = 0u32;
    if source != 0 {
        saved_flags = g.read(source + 0x28);
    }
    let x = g.readf(entry + 0x1C);
    let mut y = g.readf(entry + 0x20);
    let z = g.readf(entry + 0x24);
    let entry_type = g.read(entry);
    if entry_type == 0x14 {
        y -= LOWER_HEIGHT;
    }
    let class_flags = if entry_type == u32::MAX {
        0
    } else {
        g.read(entry_type.wrapping_shl(6).wrapping_add(CLASS_FLAGS))
    };
    let mode = if class_flags & 0x200 != 0 || class_flags & 0x1000 != 0 {
        4
    } else if g.read(RAY_MODE_GLOBAL) == 1 {
        0x20
    } else {
        0
    };
    let ray_flags = |g: &mut dyn Guest| {
        if g.read(entry + 0xC) & 0x10 != 0 {
            FLAGS_BIT_0X10
        } else {
            FLAGS_PLAIN
        }
    };
    g.set_vec3(POSITION, [x, y, z]);
    let mut object = 0u32;
    let mut object_flags = 0u32;
    let first_pass = (g.read(entry + 0x14C) != 0 || g.read(entry + 0xC) & 0x20 != 0) && source != 0;
    if first_pass {
        let kind = g.read(entry + 0x100);
        g.write(entry + 0x14C, 0);
        let handle;
        match kind {
            1 => {
                let block = |g: &mut dyn Guest, offset: u32| {
                    let player = g.read(source + 0x150);
                    g.readf(player + offset)
                };
                let origin = [block(g, 0xB4), block(g, 0xB8), block(g, 0xBC)];
                g.set_vec3(FROM, origin);
                let player = g.read(source + 0x150);
                handle = g.read(player + 0xF54);
            }
            2 => {
                let origin = g.vec3(entry + 0x40);
                g.set_vec3(FROM, origin);
                handle = g.call(0xC3C50, &[FROM, 0x3DCC_CCCD, 0]);
            }
            3 | 4 => {
                g.call(0x242E60, &[source, FROM]);
                let player = g.at(entry + 0x108, 0x7C);
                object = g.read(player + 0x68);
                handle = g.read(player + 0xB0);
                if object != 0 {
                    object_flags = g.read(object + 0x28);
                    g.write(object + 0x28, object_flags & 0xFFFF_FF9F);
                }
            }
            6 => {
                let raw_y = g.readf(entry + 0x20);
                g.set_vec3(FROM, [x, raw_y, z]);
                handle = initial_cell;
            }
            _ => {
                let origin = g.vec3(source + 0x64);
                g.set_vec3(FROM, origin);
                handle = g.read(source + 0x34);
            }
        }
        let origin = g.vec3(FROM);
        let delta = [x - origin[0], y - origin[1], z - origin[2]];
        g.set_vec3(DELTA, delta);
        let masked = g.read(source + 0x28);
        g.write(source + 0x28, masked & 0xFFFF_FF9F);
        let flags = ray_flags(g);
        if (handle as i32) > 0 {
            g.write(CELL_OUT, cell_out);
            result = g.call(
                0x88C60,
                &[handle, FROM, DELTA, 0, a2, a3, CELL_OUT, flags, mode, a4],
            );
            cell_out = g.read(CELL_OUT);
            if g.read(entry + 0xC) & 0x20 == 0 {
                g.write(entry + 0xF0, cell_out);
            }
        }
        if object != 0 {
            g.write(object + 0x28, object_flags);
        }
        update_owner(g, entry);
        if result != 0 {
            let source_now = g.read(entry + 0x108);
            g.write(source_now + 0x28, saved_flags);
            return result;
        }
    }
    let cell = if first_pass { cell_out } else { initial_cell };
    let flags = ray_flags(g);
    if (cell as i32) > 0 {
        g.write(CELL_OUT, cell_out);
        result = g.call(
            0x88C60,
            &[cell, POSITION, a1, 0, a2, a3, CELL_OUT, flags, mode, a4],
        );
        cell_out = g.read(CELL_OUT);
        if g.read(entry + 0xC) & 0x20 == 0 {
            g.write(entry + 0xF0, cell_out);
        }
    }
    let source_now = g.read(entry + 0x108);
    if source_now != 0 {
        g.write(source_now + 0x28, saved_flags);
    }
    update_owner(g, entry);
    result
}

/// The owner (`+0xA8`) takes the entry's cell word in `+0x34` and `+0x38`.
fn update_owner(g: &mut dyn Guest, entry: u32) {
    let owner = g.read(entry + 0xA8);
    if owner != 0 {
        let cell = g.read(entry + 0xF0);
        g.write(owner + 0x34, cell);
        let cell = g.read(entry + 0xF0);
        let owner = g.read(entry + 0xA8);
        g.write(owner + 0x38, cell);
    }
}

/// `0xDCEF0(entry, record)`: starts the sound / light of an entry. The strength is `1.0` (`[0x475C78]`), or for classes without flag 4 the
/// f32 result of `0xB9920(+8, type, +0xF4)`; a non zero `record[1]` zeroes `+0xF8`; an attached object (`+0x104`) in mode `+0x100 == 1`
/// runs `0x238510(0x21, object+8, 1.0)`; `0x12A170(record, type, strength, +0x108)` applies it.
pub fn start_light(g: &mut dyn Guest, entry: u32, record: u32) {
    let mut strength = 1.0f32;
    let entry_type = g.read(entry);
    if g.read(entry_type.wrapping_shl(6).wrapping_add(CLASS_FLAGS)) & 4 == 0 {
        let a = g.read(entry + 8);
        let b = g.read(entry + 0xF4);
        strength = g.call_f(0xB9920, &[a, entry_type, b]);
    }
    if g.read(record + 4) != 0 {
        g.writef(entry + 0xF8, 0.0);
    }
    let object = g.read(entry + 0x104);
    if object != 0 && g.read(entry + 0x100) == 1 {
        let id = g.read(object + 8);
        g.call(0x238510, &[0x21, id, 0x3F80_0000]);
    }
    let entry_type = g.read(entry);
    let source = g.read(entry + 0x108);
    g.call(0x12A170, &[record, entry_type, strength.to_bits(), source]);
}

/// `0xDC5D0(entry)`: for a live entry (type != -1, flag `0x80000` clear) with an attached object (`+0x104`): `0x2385B0(2, id, [object+0x94], 1.0)`,
/// then when `0x238580(8, id)` exceeds `0x238580(9, id)` (the first rounded to f32) `0x238510(9, id, 1.0)` and `0x2384D0(4, id, 1.0)`
/// (`id = [object + 8]`; INFERRED: a channel level comparison).
pub fn attached_channels(g: &mut dyn Guest, entry: u32) {
    let object = g.read(entry + 0x104);
    if object == 0 || g.read(entry) == u32::MAX || g.read(entry + 0xC) & 0x80000 != 0 {
        return;
    }
    let object = g.read(entry + 0x104);
    let param = g.read(object + 0x94);
    let id = g.read(object + 8);
    g.call(0x2385B0, &[2, id, param, 0x3F80_0000]);
    let id = g.at(entry + 0x104, 8);
    let low = g.call_f(0x238580, &[9, id]);
    let id = g.at(entry + 0x104, 8);
    let high = g.call_f(0x238580, &[8, id]);
    if !(high > low) {
        return;
    }
    let id = g.at(entry + 0x104, 8);
    g.call(0x238510, &[9, id, 0x3F80_0000]);
    let id = g.at(entry + 0x104, 8);
    g.call(0x2384D0, &[4, id, 0x3F80_0000]);
}

/// The two 8 dword contact templates: `0x70AAAC` is the live one, `0x70AA58` the reset value (`0x91390` copies it over the live one,
/// `0x913D0(dst)` saves the live one, `0x913B0(src)` restores it).
pub const CONTACT_LIVE: u32 = 0x0070_AAAC;
pub const CONTACT_RESET: u32 = 0x0070_AA58;
const CONTACT_WORDS: u32 = 8;

/// `[0x47864C]` (1.8): height added to the player centre in [`mark_players`].
pub const PLAYER_LIFT: f32 = f32::from_bits(0x3FE6_6666);
/// Player table `[0x7A3580 + 4 * index]`, count `[0x74C37C]`, first index `[0x790950]`.
pub const PLAYER_TABLE: u32 = 0x007A_3580;
pub const PLAYER_COUNT: u32 = 0x0074_C37C;
pub const PLAYER_FIRST: u32 = 0x0079_0950;
/// Build mode globals: `[0x7DE458]` flags, `[0x7DE455]` byte (10 = the mode that marks players), clock `[0x7DE318]`.
pub const MODE_FLAGS: u32 = 0x007D_E458;
pub const MODE_BYTE: u32 = 0x007D_E455;
pub const CLOCK: u32 = 0x007D_E318;

/// `0xE94E0(entry, kind, position, radius)`: marks the players near an effect. Only in mode byte 10 with flag `0x200` of `[0x7DE458]` clear
/// and `kind != 8`; the effect must be tied to a player: kinds `+0x100` of 1 or 2 take the player of the source (`+0x108`, an object of
/// type `0x200` whose block `+0x7C` has bit 1 of `+8`: `block[0xD8]`), others the object's `+0x104` indirection's first word; a player
/// without `+0x150` needs `0x278040(block)` to answer. Then every player from `[0x790950]` to `[0x74C37C]` whose flag word (`+0x10` in mode 10,
/// else `+0x18`) has any of `0x9180200` and whose centre (`[player] + 0x64`, lifted by `1.8`) is within `radius` (`radius^2 > d^2`,
/// `d^2 = (dz^2 + dy^2) + dx^2`) takes the clock in `+0xB60`, `-1` in `+0xB64` and loses bit `0x08000000` of its object flags. INFERRED meaning:
/// a shock or knock marker.
pub fn mark_players(g: &mut dyn Guest, entry: u32, kind: u32, position: u32, radius: f32) {
    if g.read(MODE_FLAGS) & 0x200 != 0 || g.byte(MODE_BYTE) != 0xA || kind == 8 {
        return;
    }
    let mode = g.read(entry + 0x100);
    let tied_to_player = |g: &mut dyn Guest| {
        let source = g.read(entry + 0x108);
        source != 0 && g.read(source + 0x20) == 0x200 && g.at(source + 0x7C, 8) & 1 != 0
    };
    if mode != 1 && mode != 2 && !tied_to_player(g) {
        return;
    }
    let player;
    if tied_to_player(g) {
        let source = g.read(entry + 0x108);
        let block = g.read(source + 0x7C);
        if block == 0 {
            return;
        }
        player = g.read(block + 0xD8);
    } else {
        let attached = g.read(entry + 0x104);
        if attached == 0 {
            return;
        }
        player = g.read(attached);
        if player == 0 {
            return;
        }
    }
    if player == 0 {
        return;
    }
    if g.read(player + 0x150) == 0 {
        let block = g.read(player + 0x7C);
        if g.call(0x278040, &[block]) == 0 {
            return;
        }
    }
    let mut index = g.read(PLAYER_FIRST) as i32;
    if index >= g.read(PLAYER_COUNT) as i32 {
        return;
    }
    let centre = g.vec3(position);
    loop {
        let slot = g.read(PLAYER_TABLE.wrapping_add((index as u32).wrapping_mul(4)));
        let words = if g.byte(MODE_BYTE) == 0xA {
            g.read(slot + 0x10)
        } else {
            g.read(slot + 0x18)
        };
        if words & 0x0918_0200 != 0 {
            let object = g.read(slot);
            let at = g.vec3(object + 0x64);
            let dz = at[2] - centre[2];
            let dy = (at[1] - centre[1]) + PLAYER_LIFT;
            let dx = at[0] - centre[0];
            let distance_squared = (dz * dz + dy * dy) + dx * dx;
            if radius * radius > distance_squared {
                let clock = g.read(CLOCK);
                g.write(slot + 0xB60, clock);
                let flags = g.read(slot + 0x28);
                g.write(slot + 0xB64, u32::MAX);
                g.write(slot + 0x28, flags & 0xF7FF_FFFF);
            }
        }
        index = index.wrapping_add(1);
        if index >= g.read(PLAYER_COUNT) as i32 {
            break;
        }
    }
}

/// `0xE9690(entry)`: refreshes the ground contact of an entry. A cell word of 0 or `0xD5` is first looked up with
/// `0xC3C50(&(velocity * 2 + position), 1.5, 1.5)` (no cell clears `+0x128` and ends); then the contact template is saved, the room object
/// `[[0x7B9784] + cell * 4]` is asked `0x919D0(room[+4], &position, &velocity, 1, &entry[0x110], &entry[0x11C], 0)`; an answer stores the room
/// object in `+0x128`, resets the live template (`0x91390`), keeps it in `entry + 0x12C` and restores the saved one; no answer clears `+0x128`.
pub fn refresh_contact(g: &mut dyn Guest, entry: u32) {
    let cell = g.read(entry + 0xF0);
    if cell == 0xD5 || cell == 0 {
        let velocity = g.vec3(entry + 0x64);
        let position = g.vec3(entry + 0x1C);
        let lifted = [0, 1, 2].map(|k| velocity[k] * 2.0 + position[k]);
        g.set_vec3(SCRATCH, lifted);
        let found = g.call(0xC3C50, &[SCRATCH, 0x3FC0_0000, 0x3FC0_0000]);
        if (found as i32) <= 0 {
            g.write(entry + 0x128, 0);
            return;
        }
        g.write(entry + 0xF0, found);
    }
    let saved = SCRATCH + 0x100;
    g.copy(CONTACT_LIVE, saved, CONTACT_WORDS);
    let rooms = g.read(0x007B_9784);
    let cell = g.read(entry + 0xF0);
    let room = g.read(rooms.wrapping_add(cell.wrapping_mul(4)));
    let room_word = g.read(room + 4);
    if g.call(
        0x919D0,
        &[
            room_word,
            entry + 0x1C,
            entry + 0x64,
            1,
            entry + 0x110,
            entry + 0x11C,
            0,
        ],
    ) == 0
    {
        g.write(entry + 0x128, 0);
        return;
    }
    let rooms = g.read(0x007B_9784);
    let cell = g.read(entry + 0xF0);
    let room = g.read(rooms.wrapping_add(cell.wrapping_mul(4)));
    g.write(entry + 0x128, room);
    g.copy(CONTACT_RESET, CONTACT_LIVE, CONTACT_WORDS);
    g.copy(CONTACT_LIVE, entry + 0x12C, CONTACT_WORDS);
    g.copy(saved, CONTACT_LIVE, CONTACT_WORDS);
}

const ORIGIN: u32 = FROM;
const POSITION_COPY: u32 = SCRATCH + 0x40;
const HIT_POINT: u32 = SCRATCH + 0x60;
const HIT_NORMAL: u32 = SCRATCH + 0x80;
const SAVED_TEMPLATE: u32 = SCRATCH + 0xA0;

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    // the compiler's order: z term first, then x, then y
    (a[2] * b[2] + a[0] * b[0]) + a[1] * b[1]
}

fn restore_source_flags(g: &mut dyn Guest, entry: u32, flags: u32) {
    let source = g.read(entry + 0x108);
    if source != 0 {
        g.write(source + 0x28, flags);
    }
}

/// `dot(N, previous - (N + position))` of the contact test: positive means the entry has not passed the contact plane.
fn plane_side(g: &mut dyn Guest, entry: u32, normal: u32) -> f32 {
    let n = g.vec3(normal);
    let position = g.vec3(entry + 0x1C);
    let previous = g.vec3(entry + 0x110);
    let q = [0, 1, 2].map(|k| previous[k] - (n[k] + position[k]));
    dot(n, q)
}

/// `dot(N, hit - previous)`.
fn hit_side(g: &mut dyn Guest, entry: u32, normal: u32, hit: u32) -> f32 {
    let n = g.vec3(normal);
    let point = g.vec3(hit);
    let previous = g.vec3(entry + 0x110);
    dot(n, [0, 1, 2].map(|k| point[k] - previous[k]))
}

/// `0xE97A0` (usercall: eax = entry, ebx = `hit`, stack `s1 s2 s3`): sweeps an effect entry's last step against the level, the default
/// projectile / debris motion test of `0xF4A50` and its siblings. Names INFERRED, flow MEASURED. Returns the ray result (non zero = the
/// step hit), or the contact room object (`+0x128`) when the step crossed a contact plane.
///
/// * `s1` points at the contact normal, `hit` receives the hit point (or the previous position), `s2` (optional) the hit normal (or the
///   contact normal words `+0x11C`), `s3` is the ray option word passed through.
/// * Cell `0xD5` (the "unknown" room) is first resolved from `position + velocity * 2` (`0xC3C50(.., 1.5, 1.5)`); no cell returns 0.
/// * PASS A (when `+0x14C != 0`, cleared here): ray from the attached object (`+0x100`: 1 the source's player block, 2 `+0x40`, 3 and 4 the
///   source's player block through `0xB6140` or `0x242E60` with the player's object flags `0x60` masked, 0 and 6 the entry position, others the
///   source's `+0x64`) to the entry position, flags `0x2000010` (`0x2010010` for entry flag `0x10`); a hit returns it.
/// * PASS B: ray from the entry position along `s1` with flags `0x2000012`; a hit saves the contact template. A contact room (`+0x128`) whose
///   plane the previous position has not crossed (`plane_side <= 0`) returns the room object after restoring the template from `+0x12C`
///   (or returns the ray result when the hit lies behind the plane); a changed cell triggers the third ray (flags `-3`) and a refresh `0xE9690`.
pub fn sweep_move(g: &mut dyn Guest, entry: u32, hit: u32, s1: u32, s2: u32, s3: u32) -> u32 {
    let cell_word = entry + 0xF0;
    let mut result = 0u32;
    let mut object = 0u32;
    let mut object_flags = 0u32;
    if g.read(cell_word) == 0xD5 {
        let velocity = g.vec3(entry + 0x64);
        let position = g.vec3(entry + 0x1C);
        g.set_vec3(ORIGIN, [0, 1, 2].map(|k| velocity[k] * 2.0 + position[k]));
        let found = g.call(0xC3C50, &[ORIGIN, 0x3FC0_0000, 0x3FC0_0000]);
        if (found as i32) <= 0 {
            g.write(entry + 0x128, 0);
            return 0;
        }
        g.write(cell_word, found);
    }
    let source = g.read(entry + 0x108);
    let source_flags = if source != 0 {
        g.read(source + 0x28)
    } else {
        0
    };
    let ray_mode = |g: &mut dyn Guest| {
        if g.read(RAY_MODE_GLOBAL) == 1 {
            0x20
        } else {
            0
        }
    };
    if g.read(entry + 0x14C) != 0 {
        let kind = g.read(entry + 0x100);
        g.write(entry + 0x14C, 0);
        let cell;
        if kind == 1 && source != 0 {
            let block = g.read(source + 0x150);
            let x = g.readf(block + 0xB4);
            let block = g.read(source + 0x150);
            let y = g.readf(block + 0xB8);
            let block = g.read(source + 0x150);
            let z = g.readf(block + 0xBC);
            g.set_vec3(ORIGIN, [x, y, z]);
            cell = g.at(source + 0x150, 0xF54);
        } else if kind == 2 && source != 0 {
            let origin = g.vec3(entry + 0x40);
            g.set_vec3(ORIGIN, origin);
            cell = g.call(0xC3C50, &[ORIGIN, 0x3DCC_CCCD, 0]);
        } else if (kind == 3 || kind == 4) && source != 0 {
            let flags = g.read(source + 0x28);
            let block = g.read(source + 0x7C);
            if (flags as i32) < 0 {
                return 0;
            }
            if g.read(block + 8) & 0x40 != 0 {
                let word = g.read(block + 0x70);
                g.call(0xB6140, &[source, word, 0, ORIGIN, 0]);
            } else {
                g.call(0x242E60, &[source, ORIGIN]);
            }
            cell = g.read(block + 0xB0);
            let player_object = g.read(block + 0xD8);
            if player_object != 0 {
                object_flags = g.read(player_object + 0x28);
                object = player_object;
                g.write(player_object + 0x28, object_flags & 0xFFFF_FF9F);
            }
        } else if kind == 6 || kind == 0 {
            let position = g.vec3(entry + 0x1C);
            g.set_vec3(ORIGIN, position);
            cell = g.read(cell_word);
        } else if source == 0 {
            cell = 0;
        } else {
            let origin = g.vec3(source + 0x64);
            g.set_vec3(ORIGIN, origin);
            cell = g.read(source + 0x34);
        }
        let position = g.vec3(entry + 0x1C);
        let origin = g.vec3(ORIGIN);
        g.set_vec3(DELTA, [0, 1, 2].map(|k| position[k] - origin[k]));
        let source_now = g.read(entry + 0x108);
        if source_now != 0 && g.read(entry + 0xC) & 0x100 == 0 {
            let flags = g.read(source_now + 0x28);
            g.write(source_now + 0x28, flags & 0xFFFF_FF9F);
        }
        let flags = if g.read(entry + 0xC) & 0x10 != 0 {
            0x0201_0010
        } else {
            0x0200_0010
        };
        if (cell as i32) > 0 {
            let mode = ray_mode(g);
            result = g.call(
                0x88C60,
                &[cell, ORIGIN, DELTA, 0, hit, s2, cell_word, flags, mode, s3],
            );
        }
        g.call(0xE9690, &[entry]);
        if object != 0 {
            g.write(object + 0x28, object_flags);
        }
        if result != 0 {
            restore_source_flags(g, entry, source_flags);
            update_owner(g, entry);
            return result;
        }
    }
    // PASS B
    let cell_before = g.read(cell_word);
    let mode = ray_mode(g);
    let position = g.vec3(entry + 0x1C);
    g.set_vec3(POSITION_COPY, position);
    let flags = if g.read(entry + 0xC) & 0x10 != 0 {
        0x0201_0012
    } else {
        0x0200_0012
    };
    if (cell_before as i32) > 0 {
        result = g.call(
            0x88C60,
            &[
                cell_before,
                entry + 0x1C,
                s1,
                0,
                hit,
                s2,
                cell_word,
                flags,
                mode,
                s3,
            ],
        );
        if result != 0 {
            g.copy(CONTACT_LIVE, SAVED_TEMPLATE, CONTACT_WORDS);
        }
    }
    let mut contact = g.read(entry + 0x128);
    let mut try_third = true;
    if contact != 0 && plane_side(g, entry, s1) <= 0.0 {
        if result != 0 && hit_side(g, entry, s1, hit) <= 0.0 {
            restore_source_flags(g, entry, source_flags);
            update_owner(g, entry);
            return result;
        }
        return contact_exit(g, entry, hit, s2, source_flags, contact, false);
    }
    if cell_before == g.read(cell_word) {
        try_third = false;
    }
    if try_third {
        let second = g.call(
            0x88C60,
            &[
                cell_before,
                POSITION_COPY,
                s1,
                0,
                HIT_POINT,
                HIT_NORMAL,
                cell_word,
                0xFFFF_FFFD,
                0,
                0,
            ],
        );
        if second != 0 {
            let point = g.vec3(HIT_POINT);
            if result != 0 && hit_side_point(g, entry, s1, hit, point) <= 0.0 {
                restore_source_flags(g, entry, source_flags);
                update_owner(g, entry);
                g.copy(SAVED_TEMPLATE, CONTACT_LIVE, CONTACT_WORDS);
                return result;
            }
            g.set_vec3(hit, point);
            if s2 == 0 {
                return second;
            }
            let normal = g.vec3(HIT_NORMAL);
            g.set_vec3(s2, normal);
            return second;
        }
        g.call(0xE9690, &[entry]);
        contact = g.read(entry + 0x128);
        if contact != 0 && plane_side(g, entry, s1) <= 0.0 {
            if result != 0 && hit_side(g, entry, s1, hit) <= 0.0 {
                restore_source_flags(g, entry, source_flags);
                update_owner(g, entry);
                return result;
            }
            return contact_exit(g, entry, hit, s2, source_flags, contact, true);
        }
    }
    restore_source_flags(g, entry, source_flags);
    update_owner(g, entry);
    result
}

/// `dot(N, hit - point)` of the third ray.
fn hit_side_point(g: &mut dyn Guest, _entry: u32, normal: u32, hit: u32, point: [f32; 3]) -> f32 {
    let n = g.vec3(normal);
    let mine = g.vec3(hit);
    // the compiler's order here: z, then y, then x
    (n[2] * (mine[2] - point[2]) + n[1] * (mine[1] - point[1])) + n[0] * (mine[0] - point[0])
}

/// The contact exit of [`sweep_move`]: restore the template from `+0x12C`, report the previous position (and the contact normal words
/// `+0x11C` when `s2` is given), restore the source flags and return the contact room object. `owner_first` marks the exit of the refresh
/// branch, which also updates the owner cell words.
fn contact_exit(
    g: &mut dyn Guest,
    entry: u32,
    hit: u32,
    s2: u32,
    source_flags: u32,
    contact: u32,
    owner_update_after: bool,
) -> u32 {
    g.copy(entry + 0x12C, CONTACT_LIVE, CONTACT_WORDS);
    let previous = g.vec3(entry + 0x110);
    g.set_vec3(hit, previous);
    if s2 != 0 {
        let normal = g.vec3(entry + 0x11C);
        g.set_vec3(s2, normal);
    }
    restore_source_flags(g, entry, source_flags);
    if owner_update_after {
        update_owner(g, entry);
    }
    contact
}

/// `[0x7DE318]` is the game clock tick, `[0x739A8C]` / `[0x73986C]` / `[0x7394BC]` the last tick the spray, the sound and the flash fired.
pub const LAST_SPRAY: u32 = 0x0073_9A8C;
pub const LAST_SOUND: u32 = 0x0073_986C;
pub const LAST_FLASH: u32 = 0x0073_94BC;
/// Reaction table: surface type (contact template word 6, minus 1) -> handler; `type - 3` of the effect type -> table 2 index.
const TEMPLATE_TYPE: u32 = 0x18;
const TEMPLATE_FLAGS: u32 = 0x1C;
const EFFECT_REACTION: [u8; 50] = [
    0, 0, 0, 0, 1, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 1, 7, 7, 2, 3, 1, 4, 5, 7, 7, 7,
    7, 7, 7, 7, 7, 6, 7, 7, 7, 7, 4, 2, 7, 7, 7, 7, 7, 1,
];

fn unit(word: u32) -> f64 {
    f64::from(word) * f64::from(f32::from_bits(0x2F80_0000))
}

/// `0xE9FE0(a0, kind, target, position, vector, a5, a6)`: the reaction of a world object to an effect hit (names INFERRED, flow MEASURED).
/// `target` null ends. An ACTOR target (type `+0x20 == 8`) with a player block (`+0x7C`, `0x124C70` must answer 0): the attacker `a0` with a kind
/// whose class flag is not 2 asks `0x345780` / `0x345B60` / `0x3457E0`, throttled by the clock (`[0x739A8C]`, 8 ticks, class flag `0x8000000`)
/// fires two spray streaks (`0x345C60`, the second with four rng draws, x87 double arithmetic rounded to f32 at the stores), else `0xD6DD0(word +0x48)`
/// decides `0xFF550`; non flag-2 kinds `0x113070`, then without a player pointer `+0x150` a throttled (20 ticks, class flag `0x40000`) sound
/// `0x108250` and always `0x104BD0(0, 0x2D, 0, position, 0)`. A NON actor target reads the contact template word 6 (surface, negative or flag
/// `0x2000000` of word 7 = nothing): surfaces 1, 7 and 19 push the target (`0x128580`), 4, 5, 6, 9, 11, 12, 15, 21 .. 24 combinations of `0xDFE50`
/// (impact), `0xDFD00`, `0xFF550`, `0x100260`, `0x111AE0` (flash, throttled 3 ticks), `0x187DF0` / `0x188560`; others the default
/// `0xDFE50` + `0xDFD00` and a second table keyed by `kind - 3` (`0x1CDAE0`, `0x100AA0`, `0xFFC50`, `0xFF7C0`).
#[allow(clippy::too_many_arguments)]
pub fn hit_reaction(
    g: &mut dyn Guest,
    a0: u32,
    kind: u32,
    target: u32,
    position: u32,
    vector: u32,
    a5: u32,
    a6: u32,
) {
    if target == 0 {
        return;
    }
    if g.read(target + 0x20) != 8 {
        surface_reaction(g, kind, target, position, vector, a5, a6);
        return;
    }
    let block = g.read(target + 0x7C);
    if g.call(0x124C70, &[block]) != 0 {
        return;
    }
    let flags_of = |kind: u32| (kind << 6).wrapping_add(CLASS_FLAGS);
    if a0 != 0 && kind != u32::MAX && g.read(flags_of(kind)) & 2 == 0 {
        if g.call(0x345780, &[block]) == 0 {
            let word = g.read(block + 0x48);
            if g.call(0xD6DD0, &[sign_extend_word(word)]) != 0 {
                g.call(0xFF550, &[0, position, vector]);
            }
        } else {
            g.call(0x345B60, &[0, target, 0, position, vector]);
            if g.call(0x3457E0, &[block]) != 0 {
                let clock = g.read(CLOCK);
                let throttled = g.read(flags_of(kind)) & 0x0800_0000 != 0
                    && (g.read(LAST_SPRAY) as i32) >= (clock as i32).wrapping_sub(8);
                if !throttled {
                    spray_pair(g, a0, position);
                }
            }
        }
    }
    let kind_now = kind;
    if kind_now != u32::MAX && g.read(flags_of(kind_now)) & 2 == 0 {
        g.call(0x113070, &[target, position, vector]);
    }
    if g.read(target + 0x150) != 0 {
        return;
    }
    if kind_now != u32::MAX && g.read(flags_of(kind_now)) & 0x4_0000 == 0 {
        let clock = g.read(CLOCK);
        if (g.read(LAST_SOUND) as i32) < (clock as i32).wrapping_sub(0x14) {
            let v = g.vec3(vector);
            let scaled = [v[0] * 0.25, v[1] * 0.25, v[2] * 0.25];
            g.set_vec3(SCRATCH + 0x200, scaled);
            g.call(
                0x108250,
                &[
                    0,
                    5,
                    u32::MAX,
                    1,
                    position,
                    SCRATCH + 0x200,
                    0x4040_407F,
                    0x7F,
                ],
            );
            let clock = g.read(CLOCK);
            g.write(LAST_SOUND, clock);
        }
    }
    g.call(0x104BD0, &[0, 0x2D, 0, position, 0]);
}

fn sign_extend_word(word: u32) -> u32 {
    i32::from(word as u16 as i16) as u32
}

/// The two streaks of [`hit_reaction`]: a straight one from the attacker's velocity and a scattered one (four rng draws).
fn spray_pair(g: &mut dyn Guest, attacker: u32, position: u32) {
    let clock = g.read(CLOCK);
    let velocity = g.vec3(attacker + 0x64);
    let scaled = [0, 1, 2].map(|k| velocity[k] * 2.5);
    g.set_vec3(SCRATCH + 0x300, scaled);
    g.write(LAST_SPRAY, clock);
    let cell = g.read(attacker + 0xF0);
    if g.call(0x345C60, &[position, SCRATCH + 0x300, cell, 0x3F80_0000, 0]) != 0 {
        return;
    }
    let half = 0.5f64;
    let tail = 0.3f32 as f64;
    let seven = f64::from(f32::from_bits(0x3F33_3333));
    let r1 = g.rng();
    let x_part = ((unit(r1) - half) * f64::from(g.readf(attacker + 0x64)) * seven) as f32;
    let r2 = g.rng();
    let x = ((((unit(r2) - half) * tail) + f64::from(x_part)) * 2.0) as f32;
    let r3 = g.rng();
    let z_part = ((unit(r3) - half) * f64::from(g.readf(attacker + 0x6C)) * seven) as f32;
    let r4 = g.rng();
    let cell = g.read(attacker + 0xF0);
    let z = (((((unit(r4) - half) * tail) + f64::from(z_part)) * 2.0) * 3.0) as f32;
    let x = (f64::from(x) * 3.0) as f32;
    g.set_vec3(SCRATCH + 0x300, [x, -3.0, z]);
    g.call(0x345C60, &[position, SCRATCH + 0x300, cell, 0x3F80_0000, 2]);
}

fn surface_reaction(
    g: &mut dyn Guest,
    kind: u32,
    target: u32,
    position: u32,
    vector: u32,
    a5: u32,
    a6: u32,
) {
    g.copy(CONTACT_LIVE, SCRATCH + 0x400, CONTACT_WORDS);
    let surface = g.read(SCRATCH + 0x400 + TEMPLATE_TYPE) as i32;
    if surface < 0 || g.read(SCRATCH + 0x400 + TEMPLATE_FLAGS) & 0x0200_0000 != 0 {
        return;
    }
    let index = (surface - 1) as u32;
    let impact = |g: &mut dyn Guest, mode: u32| {
        g.call(0xDFE50, &[kind, mode, position, vector]);
    };
    match if index > 0x17 { 99 } else { index } {
        0 | 6 | 18 => {
            let v = g.vec3(vector);
            // multiplication by -1.0 (not a negation): a NaN keeps its sign bit, as `mulss` does
            #[allow(clippy::neg_multiply)]
            let negated = [v[0] * -1.0, v[1] * -1.0, v[2] * -1.0];
            g.set_vec3(SCRATCH + 0x440, negated);
            g.call(0x128580, &[target, position, SCRATCH + 0x440, 0]);
        }
        1 | 2 | 9 | 12 => {}
        3 => {
            impact(g, 2);
            g.call(0xDFD00, &[vector, a6, kind]);
        }
        4 => {
            impact(g, 1);
            g.call(0xFF550, &[0, position, a5]);
        }
        5 => impact(g, 1),
        8 => {
            impact(g, 3);
            let y_word = g.read(position + 4);
            g.call(0x100260, &[0, 0x7C, 0, position, vector, y_word]);
            g.call(0xFF550, &[0, position, a5]);
        }
        10 | 11 => {
            let clock = g.read(CLOCK);
            if (g.read(LAST_FLASH) as i32) < (clock as i32).wrapping_sub(3) {
                g.write(LAST_FLASH, clock);
                g.call(0x111AE0, &[0, 0x0073_97E8, 0, position, vector]);
            }
            impact(g, 0);
            g.call(0xDFD00, &[vector, a6, kind]);
        }
        13 => {
            impact(g, 0);
            g.call(0xDFD00, &[vector, a6, kind]);
        }
        14 => {
            impact(g, 0);
            g.call(0xFF550, &[0, position, a5]);
        }
        19 => impact(g, 4),
        20 | 22 | 23 => {
            impact(g, 0);
            if g.call(0x187DF0, &[kind]) == 0 {
                return;
            }
            let mode = match index {
                20 => 2,
                22 => 1,
                _ => 3,
            };
            g.call(0x188560, &[mode, position, vector]);
        }
        21 => {
            impact(g, 2);
            g.call(0x111AE0, &[0, 0x0073_74E0, 0, position, vector]);
            if g.read((kind << 6).wrapping_add(CLASS_FLAGS)) & 2 == 0 {
                return;
            }
            g.call(0x10EB70, &[0x4248_0000]);
        }
        _ => {
            impact(g, 0);
            g.call(0xDFD00, &[vector, a6, kind]);
            let slot = kind.wrapping_sub(3);
            if slot > 0x31 {
                return;
            }
            let handler = EFFECT_REACTION[slot as usize];
            let ledger = |g: &mut dyn Guest, effect: u32| {
                let level = g.call(0x1CDAE0, &[]);
                let word = if level.wrapping_sub(1) != 0 {
                    0x8_0000
                } else {
                    0
                };
                g.call(0x100AA0, &[0, effect, word]);
                g.call(0xFFC50, &[0, position]);
            };
            match handler {
                0 => {
                    g.call(0xFF550, &[0, position, a5]);
                }
                1 => ledger(g, 0x31),
                2 => ledger(g, 0x2E),
                3 => ledger(g, 0x30),
                4 => ledger(g, 0x2F),
                5 => ledger(g, 0x32),
                6 => {
                    g.call(0xFF7C0, &[position, a5]);
                    g.call(0xFFC50, &[0, position]);
                }
                _ => {}
            }
        }
    }
}

/// Words of the light / hit record `0xB91F0` initialises and `0xBD0F0` / `0xBB6C0` consume.
pub const RECORD_WORDS: u32 = 0x17;
/// Per class table of the damage / light strength: row `0x3C * id` at `0x4DAD48` (mode word), `+0x34`... see [`strength_b`].
pub const STRENGTH_TABLE: u32 = 0x004D_AD48;

/// `0xB91F0(record)`: zero the 23 words, then `-1` at `+0x14 +0x18 +0x24 +0x44 +0x1C` and 0 at `+0x48`.
pub fn record_init(g: &mut dyn Guest, record: u32) {
    for index in 0..RECORD_WORDS {
        g.write(record + 4 * index, 0);
    }
    for offset in [0x14u32, 0x18, 0x24, 0x44] {
        g.write(record + offset, u32::MAX);
    }
    g.write(record + 0x48, 0);
    g.write(record + 0x1C, u32::MAX);
}

/// `0xB9220(record, entry)`: fill a light record from an effect entry (`+4` and `+8` the source `+0x10C`, `+0x14` and `+0x18` the type, `+0x24`
/// the entry word `+8`, `+0xC` the entry `+0x108`, `+0x10` the entry `+0x104`, `+0x3C` the entry flags `+0xC`, `+0x54` the entry `+0xF4`).
pub fn record_fill(g: &mut dyn Guest, record: u32, entry: u32) {
    let source = g.read(entry + 0x10C);
    g.write(record + 4, source);
    g.write(record + 8, source);
    let entry_type = g.read(entry);
    g.write(record + 0x14, entry_type);
    g.write(record + 0x18, entry_type);
    let word = g.read(entry + 8);
    g.write(record + 0x24, word);
    let word = g.read(entry + 0x108);
    g.write(record + 0xC, word);
    let word = g.read(entry + 0x104);
    g.write(record + 0x10, word);
    let word = g.read(entry + 0xC);
    g.write(record + 0x3C, word);
    let word = g.read(entry + 0xF4);
    g.write(record + 0x54, word);
}

/// `0xBD0F0(record)`: set the record amount `+0x20` and hand the record to the light applier `0xBB6C0`. Amount 0 first; with a hit id `+0x24 != -1`:
/// an actor entity (`[record]+0x20 == 8`) asks `0xB9790(entity, hit, kind +0x18, scale +0x54, record)`; an entity of class `0x20` with a player block
/// `+0x7C` marks the record flag `+0x40 |= 8` and asks for the block's owner `[[block + 0xEC]]`; everything else `0xB9960(hit, kind, scale)`.
/// The amount is the f32 the callee leaves in st(0).
pub fn resolve_hit_amount(g: &mut dyn Guest, record: u32) {
    g.writef(record + 0x20, 0.0);
    let hit = g.read(record + 0x24);
    if hit != u32::MAX {
        let entity = g.read(record);
        let class = g.read(entity + 0x20);
        if class == 8 {
            let scale = g.read(record + 0x54);
            let kind = g.read(record + 0x18);
            let amount = g.call_f(0xB9790, &[entity, hit, kind, scale, record]);
            g.writef(record + 0x20, amount);
            g.call(0xBB6C0, &[record]);
            return;
        }
        if class == 0x20 {
            let block = g.read(entity + 0x7C);
            if block != 0 {
                let flags = g.read(record + 0x40);
                let owner = g.read(block + 0xEC);
                g.write(record + 0x40, flags | 8);
                let scale = g.read(record + 0x54);
                let owner = g.read(owner);
                let kind = g.read(record + 0x18);
                let amount = g.call_f(0xB9790, &[owner, hit, kind, scale, record]);
                g.writef(record + 0x20, amount);
                g.call(0xBB6C0, &[record]);
                return;
            }
        }
        let scale = g.read(record + 0x54);
        let kind = g.read(record + 0x18);
        let amount = g.call_f(0xB9960, &[hit, kind, scale]);
        g.writef(record + 0x20, amount);
    }
    g.call(0xBB6C0, &[record]);
}

/// `0xB9920(id, a, b)`: the light strength of class `id` (`-1` = 0): `0xB9720(id, a, b) * [0x4DAD78 + 0x3C * id]` in x87 double (the caller's `fstp`
/// rounds to f32).
pub fn strength_a(g: &mut dyn Guest, id: u32, a: u32, b: u32) -> f32 {
    if id == u32::MAX {
        return 0.0;
    }
    let base = g.call_f(0xB9720, &[id, a, b]);
    let factor = g.readf(id.wrapping_mul(0x3C).wrapping_add(0x004D_AD78));
    (f64::from(base) * f64::from(factor)) as f32
}

/// `0xB9960(id, a, b)`: the damage strength of class `id` (`-1` = 0): `0xB9720(id, a, b) * [0x4DAD7C + 0x3C * id]` rounded to f32, and when the class mode
/// word `[0x4DAD48 + 0x3C * id]` is 1 scaled by `(1 / rate) * [0x4E794C]` (per tick).
pub fn strength_b(g: &mut dyn Guest, id: u32, a: u32, b: u32) -> f32 {
    if id == u32::MAX {
        return 0.0;
    }
    let row = id.wrapping_mul(0x3C);
    let base = g.call_f(0xB9720, &[id, a, b]);
    let mode = g.read(row.wrapping_add(STRENGTH_TABLE));
    let factor = g.readf(row.wrapping_add(0x004D_AD7C));
    let mut value = (f64::from(base) * f64::from(factor)) as f32;
    if mode == 1 {
        let rate = g.read(super::eff_update::FRAME_RATE) as i32 as f32;
        let per_tick = 1.0f32 / rate;
        value *= per_tick * g.readf(super::eff_update::STEP_SCALE);
    }
    value
}
