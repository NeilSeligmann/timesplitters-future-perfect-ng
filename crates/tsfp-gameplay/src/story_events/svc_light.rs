//! T1870: the trail and light service `0xBB850`, called by the effect update `0xEFF90` for every live effect entry, ported from the
//! original and validated bit for bit against it under the Unicorn oracle (`tools/t1846_gold/svc_light.py`, test
//! `tests/story_events_t1870_light.rs`). Document `docs/t-story-event-pool-classes-t1870.md`. Behaviour MEASURED (xemu level), names INFERRED.
//!
//! `0xBB850(&entry[0xFC], &record, timer)`: the cell `[entry + 0xFC]` is an emission counter. The effect lights the world in bursts: one
//! burst every `rate / 10` ticks (`rate = [0x4E8CEC]`), five emissions per phase (`counter / 5`: phase 0 full strength and the "first" flag,
//! phase 1 `0.15`, phase 2 `0.05`, later phases zero and therefore silent), each with a random jitter (`0.8 + 0.2 u`). Per burst:
//!
//! 1. Optional ground effect: while `0.15 * rate <= ticks < 0.15 * rate + steps` (`ticks = rate * timer`), `0xF4920(position, 4.5, record[1])`.
//! 2. The strength `s = base * jitter` (`base = record[0x16]` as float, scaled by phase) must be positive (x87 compare in double).
//! 3. The box `position +- radius` (`record[0x15]`) is turned into a room list by `0x873C0` (the list gets the sentinel room `0xD5`, which is
//!    always visited). On the very first emission (`counter == 1` after increment) `0x12A590(room object, record, s, &box)` runs for every room but the sentinel.
//! 4. Every object of every listed room (counts `[0x7B54E0 + 4 room]`, arrays `[0x7B5F00 + 4 room]`) is considered when its flags `+0x28` have
//!    bit `0x10` and not the sign bit. Type 8 objects (actors) go by their player record (`+0x7C`: flags `+0x18` bit `0x10000` excludes, otherwise
//!    flag words `+0x10`/`+0x18` select with `counter % 5 == 0`, `0x3533F0` / `0x3568C0` can veto); other objects are chosen by `+0x80`, `+0x88 % 5` and
//!    `+0x34 != 0xD5`. With more than one room and a byte `+0x30 > 1` the object enters the recent list `[0x732D50]` (40 entries, count `[0x732DF0]`) once.
//! 5. For a chosen object the light centre is found (`+0x1A8 + 0x64`, `+0x1B0 + 0x6C`, height from `+0x68`/`+0x1AC`, or `0x2220E0` when `+0x94 == 0xC4`), reach is
//!    `radius * 0.66` (actors) or `radius`, state `0x42` raises the centre by `3.5` and the reach by `1.5`. Inside the reach (squared distance) a visibility
//!    ray `0x88C60` from the centre toward the entry (scaled `0.95`, with the object's `0x60` flag bits masked) must not hit. The intensity is
//!    `(1 - clamp(d / reach)^2) * s`; objects with `+0x9C & 0x800000` also get `0x12A590(object, record, s, 0)`; non actors ask the contact test `0x919D0`;
//!    actors blend with `+0x1CC`. The light is handed to `0xBB6C0` as a 0x17 dword record ([`LightCall`]); actors in state `0x46`/`0x64` take a shorter record.

// The x87 / SSE compares of the original are unordered aware (NaN takes the other branch): `!(a > b)` is not `a <= b`.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::svc_particle_efbd0::cvtt;

/// Words of the record `0xEFF90` builds.
pub const REC_WORDS: usize = 0x17;
/// Sentinel room (always visited, appended to the list).
pub const SENTINEL_ROOM: u32 = 0xD5;
/// Guest addresses of the globals the service owns.
pub const ROOM_COUNTS: u32 = 0x007B_54E0;
pub const ROOM_ARRAYS: u32 = 0x007B_5F00;
pub const ROOM_OBJECTS: u32 = 0x007B_9784;
pub const RECENT_COUNT: u32 = 0x0073_2DF0;
pub const RECENT_LIST: u32 = 0x0073_2D50;
pub const MODE_WORD: u32 = 0x007D_E454;
/// Capacity of the recent list.
pub const RECENT_CAPACITY: i32 = 0x28;

/// The light handed to `0xBB6C0` (record words by index, vectors the record points to).
#[derive(Clone, Debug, PartialEq)]
pub struct LightCall {
    pub object: u32,
    /// Copies of record words `1`, `2`, `5`, `6`, `7`.
    pub copied: [u32; 5],
    /// Word `8` (`+0x20`): the intensity, or the strength for the short actor record.
    pub intensity: f32,
    /// Vector at `+0x28` (the entry position, or the contact point of `0x919D0`).
    pub point: [f32; 3],
    /// Vector at `+0x2C` (the contact normal of `0x919D0`), when the contact test answered.
    pub normal: Option<[f32; 3]>,
    /// Vector at `+0x30`: the unit direction to the light, or for the short actor record the previous object's offset (stale stack).
    pub direction: [f32; 3],
    /// Word `13` (`+0x34`): distance from the object to the entry.
    pub distance: f32,
    /// Word `16` (`+0x40`): 1 on the first phase.
    pub first: u32,
    /// Word `21` (`+0x54`): the radius, only in the short actor record.
    pub radius: f32,
    /// The short record of actors in state `0x46` / `0x64`.
    pub short: bool,
}

impl LightCall {
    /// The record words with pointer fields as 0/1 placeholders (1 = non null).
    pub fn words(&self) -> [u32; REC_WORDS] {
        let mut words = [0u32; REC_WORDS];
        words[0] = self.object;
        for (slot, index) in [1usize, 2, 5, 6, 7].into_iter().enumerate() {
            words[index] = self.copied[slot];
        }
        words[8] = self.intensity.to_bits();
        words[9] = u32::MAX;
        words[10] = 1;
        words[11] = u32::from(self.normal.is_some());
        words[12] = 1;
        words[13] = self.distance.to_bits();
        words[16] = self.first;
        words[17] = u32::MAX;
        if self.short {
            words[13] = 0;
            words[21] = self.radius.to_bits();
        }
        words
    }
}

/// Engine side of the service (guest memory, the nine engine routines).
pub trait LightHost {
    fn read(&mut self, address: u32) -> u32;
    fn write(&mut self, address: u32, value: u32);
    /// `0x153B40`.
    fn rng(&mut self) -> u32;
    /// `0xF4920(&position, 4.5, record[1])`.
    fn ground_effect(&mut self, position: [f32; 3], height: f32, word: u32);
    /// `0x873C0(0, &box, &list, 0x1E)`: the rooms touching the box, `None` for a negative count.
    fn rooms_in_box(&mut self, region: [f32; 6]) -> Option<Vec<u32>>;
    /// `0x12A590(room object, &record, strength, &box)`.
    fn light_room(&mut self, room_object: u32, strength: f32, region: [f32; 6]);
    /// `0x12A590(object, &record, strength, 0)`.
    fn light_object(&mut self, object: u32, strength: f32);
    /// `0x3533F0()`.
    fn actor_gate(&mut self) -> bool;
    /// `0x3568C0(value)`.
    fn actor_check(&mut self, value: u32) -> bool;
    /// `0x2220E0(object, &out)`.
    fn actor_centre(&mut self, object: u32) -> [f32; 3];
    /// `0x88C60(cell, &from, &vector, 0, 0, 0, 0, -7, 0, 0)`: true when the ray hits.
    fn occluded(&mut self, cell: u32, from: [f32; 3], vector: [f32; 3]) -> bool;
    /// `0x919D0(word, &position, &direction, 0, &point, &normal, 0)`: the contact point and normal when it answers.
    fn contact(
        &mut self,
        word: u32,
        position: [f32; 3],
        direction: [f32; 3],
    ) -> Option<([f32; 3], [f32; 3])>;
    /// `0xBB6C0(&record)`.
    fn apply(&mut self, call: LightCall);
}

fn float(host: &mut dyn LightHost, address: u32) -> f32 {
    f32::from_bits(host.read(address))
}

/// `rate / 10` by the compiler's multiply and shift (truncating).
fn tenth(rate: i32) -> i32 {
    let product = i64::from(rate) * 0x6666_6667;
    let high = (product >> 32) as i32;
    let quotient = high >> 2;
    quotient.wrapping_add(((quotient as u32) >> 31) as i32)
}

/// `0xBB850(counter cell, record, timer)`. `position` is the entry position record[10] points to, `steps` is `[0x4E7944]`, `rate` is `[0x4E8CEC]`.
pub fn trail_light(
    host: &mut dyn LightHost,
    counter: &mut u32,
    record: &[u32; REC_WORDS],
    position: [f32; 3],
    timer: f32,
    steps: i32,
    rate: i32,
) {
    let rate_f = rate as f32;
    let window = cvtt(rate_f * f32::from_bits(0x3E19_999A));
    let ticks = cvtt(rate_f * timer);
    if ticks >= window && ticks < steps.wrapping_add(window) {
        host.ground_effect(position, 4.5, record[1]);
    }
    let old = *counter as i32;
    if ticks < tenth(rate).wrapping_mul(old) {
        return;
    }
    let phase = old / 5;
    let step = old % 5;
    *counter = old.wrapping_add(1) as u32;
    host.write(RECENT_COUNT, 0);
    let word = f32::from_bits(record[0x16]);
    let (base, first) = match phase {
        0 => (word, 1u32),
        1 => (word * f32::from_bits(0x3E19_999A), 0),
        2 => (word * f32::from_bits(0x3D4C_CCCD), 0),
        _ => (0.0, 0),
    };
    let unit = f64::from(host.rng()) * f64::from(f32::from_bits(0x2F80_0000));
    let jittered = ((unit * f64::from(f32::from_bits(0x3E4C_CCCD)))
        + f64::from(f32::from_bits(0x3F4C_CCCD)))
        * f64::from(base);
    let strength = jittered as f32;
    if 0.0 >= jittered {
        return;
    }
    let radius = f32::from_bits(record[0x15]);
    let region = [
        position[0] - radius,
        position[1] - radius,
        position[2] - radius,
        position[0] + radius,
        position[1] + radius,
        position[2] + radius,
    ];
    let Some(found) = host.rooms_in_box(region) else {
        return;
    };
    let mut rooms = found;
    rooms.push(SENTINEL_ROOM);
    let total = rooms.len();
    let mut stale_offset = [0.0f32; 3];
    for &room in &rooms {
        if room != SENTINEL_ROOM && *counter == 1 {
            let table = host.read(ROOM_OBJECTS);
            let room_object = host.read(table.wrapping_add(4 * room));
            host.light_room(room_object, strength, region);
        }
        let mut index = 0i32;
        loop {
            let count = host.read(ROOM_COUNTS.wrapping_add(4 * room)) as i32;
            if index >= count {
                break;
            }
            let array = host.read(ROOM_ARRAYS.wrapping_add(4 * room));
            let object = host.read(array.wrapping_add(4 * index as u32));
            index += 1;
            if light_object(
                host,
                record,
                position,
                strength,
                first,
                step,
                total,
                &mut stale_offset,
                object,
            ) {
                break;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn light_object(
    host: &mut dyn LightHost,
    record: &[u32; REC_WORDS],
    position: [f32; 3],
    strength: f32,
    first: u32,
    step: i32,
    total: usize,
    stale_offset: &mut [f32; 3],
    object: u32,
) -> bool {
    let flags = host.read(object.wrapping_add(0x28));
    if (flags as i32) < 0 || flags & 0x10 == 0 {
        return false;
    }
    let kind = host.read(object.wrapping_add(0x20));
    let mut include = false;
    let mut height = 0.0f32;
    if kind == 8 {
        let player = host.read(object.wrapping_add(0x7C));
        let low = host.read(player.wrapping_add(0x18));
        let high = host.read(player.wrapping_add(0x1C));
        if low & 0x10000 != 0 {
            return false;
        }
        let mode_ten = (host.read(MODE_WORD) >> 8) & 0xFF == 0xA;
        let (selected_low, selected_high) = if mode_ten {
            (
                host.read(player.wrapping_add(0x10)),
                host.read(player.wrapping_add(0x14)),
            )
        } else {
            (low, high)
        };
        if selected_low | selected_high != 0
            && selected_low & 1 == 0
            && host.read(player.wrapping_add(0x10)) & 0x400_0000 == 0
            && step == 0
        {
            height = float(host, object.wrapping_add(0x68)) + 1.5;
            include = true;
        }
        if host.actor_gate() {
            let pointer = host.read(object.wrapping_add(0x150));
            if pointer != 0 {
                let value = host.read(pointer);
                if !host.actor_check(value) {
                    include = false;
                }
            }
        }
    } else if host.read(object.wrapping_add(0x80)) != 0 {
        if step == 1 {
            include = true;
            height =
                float(host, object.wrapping_add(0x1AC)) + float(host, object.wrapping_add(0x68));
        }
    } else if (host.read(object.wrapping_add(0x88)) as i32) % 5 == step
        && host.read(object.wrapping_add(0x34)) != SENTINEL_ROOM
    {
        include = true;
        height = float(host, object.wrapping_add(0x1AC)) + float(host, object.wrapping_add(0x68));
    }
    if total > 1 && (host.read(object.wrapping_add(0x30)) as u8 as i8) > 1 {
        let used = host.read(RECENT_COUNT) as i32;
        let mut slot = 0;
        while slot < used {
            if host.read(RECENT_LIST.wrapping_add(4 * slot as u32)) == object {
                include = false;
            }
            slot += 1;
        }
        if !include {
            return false;
        }
        if used < RECENT_CAPACITY {
            host.write(RECENT_LIST.wrapping_add(4 * used as u32), object);
        }
        host.write(RECENT_COUNT, used.wrapping_add(1) as u32);
    }
    if !include {
        return false;
    }
    let is_actor = kind == 8;
    let mut reach = if is_actor {
        f32::from_bits(0x3F28_F5C3)
    } else {
        1.0
    };
    reach *= f32::from_bits(record[0x15]);
    let (x, z);
    if host.read(object.wrapping_add(0x94)) as u16 == 0xC4 {
        let centre = host.actor_centre(object);
        x = centre[0];
        height = centre[1];
        z = centre[2];
    } else {
        x = float(host, object.wrapping_add(0x1A8)) + float(host, object.wrapping_add(0x64));
        z = float(host, object.wrapping_add(0x1B0)) + float(host, object.wrapping_add(0x6C));
    }
    if is_actor {
        let player = host.read(object.wrapping_add(0x7C));
        let state = host.read(player.wrapping_add(0x48)) as u16 as i16;
        if state == 0x46 || state == 0x64 {
            let copied = [record[1], record[2], record[5], record[6], record[7]];
            host.apply(LightCall {
                object,
                copied,
                intensity: strength,
                point: position,
                normal: None,
                direction: *stale_offset,
                distance: 0.0,
                first,
                radius: f32::from_bits(record[0x15]),
                short: true,
            });
            // the short record's block falls through to the next room: the rest of this room's objects is skipped
            return true;
        }
        if state == 0x42 {
            height += f32::from_bits(0x4060_0000);
            reach *= 1.5;
        }
    }
    let delta = [position[0] - x, position[1] - height, position[2] - z];
    *stale_offset = delta;
    let squared = (delta[2] * delta[2] + delta[1] * delta[1]) + delta[0] * delta[0];
    if !(reach * reach > squared) {
        return false;
    }
    let mask = host.read(object.wrapping_add(0x28));
    let masked = mask & 0x60;
    host.write(object.wrapping_add(0x28), mask & !masked);
    let cell = host.read(object.wrapping_add(0x34));
    let scale = f32::from_bits(0x3F73_3333);
    // the scaled offset overwrites the stack slots that held `delta` (and that the short actor record points to)
    let scaled = [delta[0] * scale, delta[1] * scale, delta[2] * scale];
    *stale_offset = scaled;
    let hit = host.occluded(cell, [x, height, z], scaled);
    let restored = host.read(object.wrapping_add(0x28)) | masked;
    host.write(object.wrapping_add(0x28), restored);
    if hit {
        return false;
    }
    let root = f64::from(squared).sqrt();
    let distance = root as f32;
    let ratio = (root / f64::from(reach)) as f32;
    let inverse = if distance > 0.0 {
        1.0 / distance
    } else {
        f32::from_bits(0x47C3_5000)
    };
    let ratio = if 0.0 > ratio { 0.0 } else { ratio };
    let mut intensity = (1.0 - ratio * ratio) * strength;
    let towards = [0.0 - scaled[0], 0.0 - scaled[1], 0.0 - scaled[2]];
    if host.read(object.wrapping_add(0x9C)) & 0x80_0000 != 0 {
        host.light_object(object, strength);
    }
    let mut normal = None;
    let mut point = position;
    if kind != 8 {
        let word = host.read(object.wrapping_add(4));
        if let Some((contact_point, contact_normal)) = host.contact(word, position, towards) {
            point = contact_point;
            normal = Some(contact_normal);
        }
    } else {
        let blend = float(host, object.wrapping_add(0x1CC)) - intensity;
        if f32::from_bits(0x3E80_0000) > blend {
            intensity += f32::from_bits(blend.to_bits() & 0x7FFF_FFFF);
        }
    }
    host.apply(LightCall {
        object,
        copied: [record[1], record[2], record[5], record[6], record[7]],
        intensity,
        point,
        normal,
        direction: [
            towards[0] * inverse,
            towards[1] * inverse,
            towards[2] * inverse,
        ],
        distance,
        first,
        radius: 0.0,
        short: false,
    });
    false
}
