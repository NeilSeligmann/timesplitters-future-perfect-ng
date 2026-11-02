//! T1862: engine callees of the object destroy `0xBABA0` (the [`DestroyEffects`] set) and their leaves.
//!
//! Each function mirrors the original machine code (MEASURED against the Unicorn oracle, see
//! `docs/t-story-event-destroy-t1862.md`); names are INFERRED, addresses, offsets and control flow MEASURED.
//! Routines that belong to other subsystems (voice start, particle spawn, heap) are called through
//! [`DestroyEffects::call`] keyed by their original address, exactly like [`super::svc_objects::destroy_core`].

use super::strip_update::GuestMemory;
use super::svc_objects::{DestroyEffects, GLOBAL_FIRST_SLOT, GLOBAL_FLAGS_C, GLOBAL_MODE_A};

/// Original addresses of the routines reached from this module.
pub mod callee {
    pub const TEXT_PUSH_A: u32 = 0x12_D6B0;
    pub const TEXT_PUSH_B: u32 = 0x12_D760;
    pub const HEAP_FREE: u32 = 0x3_EAD0;
    pub const SCORE_FLOAT: u32 = 0x13_BF40;
    pub const MODE_B_FINISH: u32 = 0x22_9C30;
    pub const VOICE_CREATE: u32 = 0x16_C170;
    pub const VOICE_CONFIGURE: u32 = 0x17_2760;
    pub const VOICE_FLAGS: u32 = 0x16_8130;
    pub const VOICE_VOLUME: u32 = 0x16_8100;
}

pub const GLOBAL_NETWORK: u32 = 0x76_B108;
pub const GLOBAL_NETWORK_SESSION: u32 = 0x77_4028;
pub const GLOBAL_DEFAULT_CHANNELS: u32 = 0x52_36C4;
pub const GLOBAL_STRING_TABLE_A: u32 = 0x6B_7B08;
pub const GLOBAL_STRING_TABLE_B: u32 = 0x6B_7B10;
pub const GLOBAL_STRING_FALLBACK: u32 = 0x4C_0A44;
pub const STRING_EMPTY: u32 = 0x47_66D8;
pub const GLOBAL_SCORE_RECORDS: u32 = 0x7A_6160;
pub const GLOBAL_LEVEL: u32 = 0x79_094C;
pub const GLOBAL_MODE_WORD: u32 = 0x7D_E470;
pub const GLOBAL_HUD: u32 = 0x75_CC88;
pub const GLOBAL_LOCAL_PLAYER: u32 = 0x7B_0C7C;
pub const GLOBAL_OBJECT_POOL: u32 = 0x72_F5E8;
pub const GLOBAL_VOICE_MODE: u32 = 0x7D_E30C;
pub const GLOBAL_VOICE_TABLE: u32 = 0x75_CCC0;
pub const GLOBAL_FLAG_TABLE: u32 = 0x72_F2E0;
pub const GLOBAL_PENDING_HIT: u32 = 0x6F_32F4;
pub const HIT_THRESHOLD: u32 = 0x47_5CAC;
pub const HIT_SCALE: u32 = 0x47_5CD4;

fn f32_at(memory: &dyn GuestMemory, address: u32) -> f32 {
    f32::from_bits(memory.read_u32(address))
}

/// `0x3533F0()`: the network/session flag dword.
pub fn is_network(memory: &dyn GuestMemory) -> u32 {
    memory.read_u32(GLOBAL_NETWORK)
}

/// `0x1CDAE0()` (tail jump to `0x3567C0`): number of local channels. With the network flag and a session pointer
/// the session's count at `+0x20` wins when it is below the slot index `[0x790950]` (signed), otherwise the slot
/// index when it is at most 4, else the default `[0x5236C4]`. MEASURED.
pub fn channel_count(memory: &dyn GuestMemory) -> u32 {
    let slot_index = memory.read_u32(GLOBAL_FIRST_SLOT);
    if is_network(memory) != 0 {
        let session = memory.read_u32(GLOBAL_NETWORK_SESSION);
        if session != 0 {
            let count = memory.read_u32(session.wrapping_add(0x20));
            if (count as i32) < (slot_index as i32) {
                return count;
            }
        }
    }
    if (slot_index as i32) > 4 {
        memory.read_u32(GLOBAL_DEFAULT_CHANNELS)
    } else {
        slot_index
    }
}

/// `0x3D7B0(id)`: string lookup. Ids from `0xD87` index the table at `[0x6B7B08] - 0x361C`, smaller ids the table
/// `[0x6B7B10]` (id 0 is the empty string `0x4766D8`); a missing table gives the fallback pointer. MEASURED.
pub fn string_lookup(memory: &dyn GuestMemory, id: u32) -> u32 {
    let table_a = memory.read_u32(GLOBAL_STRING_TABLE_A);
    let fallback = memory.read_u32(GLOBAL_STRING_FALLBACK);
    if table_a == 0 {
        return fallback;
    }
    if (id as i32) >= 0xD87 {
        return memory.read_u32(
            table_a
                .wrapping_add(id.wrapping_mul(4))
                .wrapping_sub(0x361C),
        );
    }
    if id == 0 {
        return STRING_EMPTY;
    }
    let table_b = memory.read_u32(GLOBAL_STRING_TABLE_B);
    if table_b == 0 {
        return fallback;
    }
    memory.read_u32(table_b.wrapping_add(id.wrapping_mul(4)))
}

/// `0x12E3B0(index, value)` (void): per player score record (`0x177C` bytes from `0x7A6160`, indices 0..3).
/// Stores `value` at `+0x28`; an empty counter at `+0x24` becomes 1, otherwise it is replaced by `value - counter`
/// when `counter >= value - 0x20` (signed). MEASURED.
pub fn set_score(memory: &mut dyn GuestMemory, index: u32, value: u32) {
    if (index as i32) < 0 || (index as i32) >= 4 {
        return;
    }
    let record = GLOBAL_SCORE_RECORDS.wrapping_add(index.wrapping_mul(0x177C));
    let counter = memory.read_u32(record.wrapping_add(0x24));
    memory.write_u32(record.wrapping_add(0x28), value);
    if counter == 0 {
        memory.write_u32(record.wrapping_add(0x24), 1);
    } else if (counter as i32) >= (value.wrapping_sub(0x20) as i32) {
        memory.write_u32(record.wrapping_add(0x24), value.wrapping_sub(counter));
    }
}

/// `0x17A120(ctx)`: finds the `0x34` byte link record of `ctx` whose id (`+0`) equals the id `[[ctx]+0x2C]`.
/// `ctx+0xC08` is a handle into the object pool `[0x72F5E8]` (stride `0x2E8`, handle at `+0x2C`); a stale handle is
/// cleared. Records start at `[object+0x7C] + 0x70`, their count is `[..+0x140]` (signed). Returns 0 when none. MEASURED.
pub fn find_link(memory: &mut dyn GuestMemory, ctx: u32) -> u32 {
    let handle = memory.read_u32(ctx.wrapping_add(0xC08));
    if handle == 0 {
        return 0;
    }
    let pool = memory.read_u32(GLOBAL_OBJECT_POOL);
    let object = (handle >> 22).wrapping_mul(0x2E8).wrapping_add(pool);
    if memory.read_u32(object.wrapping_add(0x2C)) != handle {
        memory.write_u32(ctx.wrapping_add(0xC08), 0);
        return 0;
    }
    let owner = memory.read_u32(object.wrapping_add(0x7C));
    let count = memory.read_u32(owner.wrapping_add(0x140)) as i32;
    if count <= 0 {
        return 0;
    }
    let head = memory.read_u32(ctx);
    let wanted = memory.read_u32(head.wrapping_add(0x2C));
    for index in 0..count as u32 {
        let record = owner
            .wrapping_add(0x70)
            .wrapping_add(index.wrapping_mul(0x34));
        if memory.read_u32(record) == wanted {
            return record;
        }
    }
    0
}

/// `0x17BF70(ctx)` (void): clears the first dword of the link record found by [`find_link`], then `ctx+0xC08 = 0`,
/// `byte ctx+0xC0C = 0xFF` and clears bit 8 of `ctx+0x24`. MEASURED.
pub fn notify_ctx(memory: &mut dyn GuestMemory, ctx: u32) {
    let record = find_link(memory, ctx);
    if record != 0 {
        memory.write_u32(record, 0);
    }
    let flags = memory.read_u32(ctx.wrapping_add(0x24)) & 0xFFFF_FEFF;
    memory.write_u32(ctx.wrapping_add(0xC08), 0);
    memory.write_u8(ctx.wrapping_add(0xC0C), 0xFF);
    memory.write_u32(ctx.wrapping_add(0x24), flags);
}

/// `0x9E300(mask, index)`: flag table lookup. `bit = floor(log2(mask))` by shifting (`mask == 1` gives 0, a mask
/// that never reaches 1 in 32 shifts, i.e. 0 or negative, gives -1); returns whether the dword at
/// `0x72F2E0 + 4 * (index + 6 * bit)` is non zero. MEASURED.
pub fn flag_table(memory: &dyn GuestMemory, mask: u32, index: u32) -> u32 {
    let mut value = mask as i32;
    let mut bit: i32 = 0;
    while value != 1 {
        value >>= 1;
        bit += 1;
        if bit >= 0x20 {
            bit = -1;
            break;
        }
    }
    let slot = index.wrapping_add((bit as u32).wrapping_mul(6));
    u32::from(memory.read_u32(GLOBAL_FLAG_TABLE.wrapping_add(slot.wrapping_mul(4))) != 0)
}

/// `0x12E30(object, index)`: 1 when the per object dword `[object + 0x154 + 4 * index]` is set, else the
/// [`flag_table`] result for `[object + 0x20]`. MEASURED.
pub fn object_flag(memory: &dyn GuestMemory, object: u32, index: u32) -> u32 {
    let direct = memory.read_u32(
        object
            .wrapping_add(index.wrapping_mul(4))
            .wrapping_add(0x154),
    );
    if direct != 0 {
        return 1;
    }
    flag_table(memory, memory.read_u32(object.wrapping_add(0x20)), index)
}

/// `0x2382B0` (register argument `esi` = `index`): whether the audio table slot `index` is valid for the current
/// player count. Mode byte `[0x7DE455] == 0xA`: `index < 2` limit; level `[0x79094C] == 0x65`: `index < 0x1A` with
/// limit `[0x5236C4]`; else `index < 0x1A` with limit slots + players (or 0x10 with the network flag when
/// `[0x7DE458] & 0x40`). Signed compares. MEASURED.
pub fn sound_slot_valid(memory: &dyn GuestMemory, index: u32) -> bool {
    let first = memory.read_u32(GLOBAL_FIRST_SLOT);
    let count = memory.read_u32(super::svc_objects::GLOBAL_SLOT_COUNT);
    let total = first.wrapping_add(count);
    let limit = if memory.read_u8(GLOBAL_MODE_A) == 0xA {
        if (index as i32) >= 2 {
            return false;
        }
        if is_network(memory) != 0 { 0x10 } else { total }
    } else if memory.read_u32(GLOBAL_LEVEL) == 0x65 {
        if (index as i32) >= 0x1A {
            return false;
        }
        memory.read_u32(GLOBAL_DEFAULT_CHANNELS)
    } else {
        if (index as i32) >= 0x1A {
            return false;
        }
        if memory.read_u32(GLOBAL_FLAGS_C) & 0x40 == 0 {
            total
        } else if is_network(memory) != 0 {
            0x10
        } else {
            total
        }
    };
    (index as i32) < (limit as i32)
}

/// `0x238510(a, b, delta)` (void): adds the float `delta` to the voice table entry `[0x75CCC0] + 4 * (b * 0x273 + a)`
/// when [`sound_slot_valid`]`(b)` and the voice mode `[0x7DE30C] == 7`. MEASURED.
pub fn add_voice_value(memory: &mut dyn GuestMemory, first: u32, second: u32, delta: u32) {
    if !sound_slot_valid(memory, second) || memory.read_u32(GLOBAL_VOICE_MODE) != 7 {
        return;
    }
    let index = second.wrapping_mul(0x273).wrapping_add(first);
    let address = memory
        .read_u32(GLOBAL_VOICE_TABLE)
        .wrapping_add(index.wrapping_mul(4));
    let sum = f32_at(memory, address) + f32::from_bits(delta);
    memory.write_u32(address, sum.to_bits());
}

/// Shared tail of `0x22C3A0` and `0xCF440`: looks up string `0x164A` and pushes it with the local player pointer
/// `[[0x7B0C7C]+4]` through `routine`. Arguments `(player, text, value, 0, 0)`.
fn push_text(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    routine: u32,
    player: u32,
    value: u32,
) {
    let text = string_lookup(memory, 0x164A);
    effects.call(memory, routine, &[player, text, value, 0, 0]);
}

/// `0x22C3A0(link, other)` (void): when `[link+0x60]` is `0x53505247` pushes the text through `0x12D6B0`, else
/// through `0x12D760`, with `[other+8]` and the local player's `[[0x7B0C7C]+4]`. MEASURED (calls recorded).
pub fn notify_link(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    link: u32,
    other: u32,
) {
    let routine = if memory.read_u32(link.wrapping_add(0x60)) == 0x5350_5247 {
        callee::TEXT_PUSH_A
    } else {
        callee::TEXT_PUSH_B
    };
    let value = memory.read_u32(other.wrapping_add(8));
    let local = memory.read_u32(GLOBAL_LOCAL_PLAYER);
    let player = memory.read_u32(local.wrapping_add(4));
    push_text(memory, effects, routine, player, value);
}

/// `0xCF440(subject, other)` (void): outside mode `0xA`, when `[subject+8]` is set, the id `[other+8]` differs from
/// `[[[subject+0x14]+0x7C]+8]` and `[0x7DE458] & 0x800000` is clear, pushes the text with `0x12D6B0` using
/// `[subject+4]` as player. MEASURED (calls recorded).
pub fn notify_object(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    subject: u32,
    other: u32,
) {
    if memory.read_u8(GLOBAL_MODE_A) == 0xA {
        return;
    }
    if memory.read_u32(subject.wrapping_add(8)) == 0 {
        return;
    }
    let owner = memory.read_u32(subject.wrapping_add(0x14));
    let context = memory.read_u32(owner.wrapping_add(0x7C));
    let value = memory.read_u32(other.wrapping_add(8));
    if value == memory.read_u32(context.wrapping_add(8)) {
        return;
    }
    if memory.read_u32(GLOBAL_FLAGS_C) & 0x80_0000 != 0 {
        return;
    }
    let player = memory.read_u32(subject.wrapping_add(4));
    push_text(memory, effects, callee::TEXT_PUSH_A, player, value);
}

/// `0x10C0E0(record)` (void): releases the buffer at `record+0x40` through the heap free `0x3EAD0`, sets `+0x50 = 1`
/// when bit 0 of the byte at `+4` is set, then zeroes `+0x40 .. +0x4C`. A null record does nothing. MEASURED.
pub fn release_buffer(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, record: u32) {
    if record == 0 {
        return;
    }
    let buffer = memory.read_u32(record.wrapping_add(0x40));
    if buffer != 0 {
        effects.call(memory, callee::HEAP_FREE, &[buffer]);
        if memory.read_u8(record.wrapping_add(4)) & 1 != 0 {
            memory.write_u32(record.wrapping_add(0x50), 1);
        }
    }
    for offset in [0x40, 0x44, 0x48, 0x4C] {
        memory.write_u32(record.wrapping_add(offset), 0);
    }
}

/// `0x22DD10(block)` (void): destroy notification for the HUD counters. Mode word `[0x7DE470]`: negative or above
/// 2 other than 0xB does nothing, 0xB forwards the block to `0x229C30`; modes 0..2 count the HUD record
/// `[0x75CC88]` (byte `+0xC` down, dword `+0x1C` up) and, in mode 1 when the victim or killer of the block is the
/// local player id `[[0x7B0C7C]+0x14]`, award 50 points (through `0x13BF40(block[+0x28], 0x32)` when set).
/// MEASURED (calls recorded).
pub fn notify_block(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, block: u32) {
    let mode = memory.read_u32(GLOBAL_MODE_WORD) as i32;
    if mode < 0 {
        return;
    }
    if mode > 2 {
        if mode == 0xB {
            effects.call(memory, callee::MODE_B_FINISH, &[block]);
        }
        return;
    }
    let mut hud = memory.read_u32(GLOBAL_HUD);
    let low = memory.read_u8(hud.wrapping_add(0xC)).wrapping_sub(1);
    let count = memory.read_u32(hud.wrapping_add(0x1C)).wrapping_add(1);
    memory.write_u8(hud.wrapping_add(0xC), low);
    memory.write_u32(hud.wrapping_add(0x1C), count);
    if memory.read_u32(GLOBAL_MODE_WORD) != 1 {
        return;
    }
    let local = memory.read_u32(GLOBAL_LOCAL_PLAYER);
    let local_id = memory.read_u32(local.wrapping_add(0x14));
    if memory.read_u32(block.wrapping_add(8)) != local_id
        && memory.read_u32(block.wrapping_add(4)) != local_id
    {
        return;
    }
    let subject = memory.read_u32(block.wrapping_add(0x28));
    if subject != 0 {
        effects.call(memory, callee::SCORE_FLOAT, &[subject, 0x32]);
        hud = memory.read_u32(GLOBAL_HUD);
    }
    let total = memory.read_u32(hud).wrapping_add(0x32);
    // `add word [hud+6], 0x32` touches only bytes 6 and 7
    let upper = u16::from(memory.read_u8(hud.wrapping_add(6)))
        | u16::from(memory.read_u8(hud.wrapping_add(7))) << 8;
    let upper = upper.wrapping_add(0x32);
    memory.write_u8(hud.wrapping_add(6), upper as u8);
    memory.write_u8(hud.wrapping_add(7), (upper >> 8) as u8);
    memory.write_u32(hud, total);
    memory.write_u32(hud.wrapping_add(0x3C), 0);
}

/// `0x6AFB0(object, hit)` (void): first contact bring up of the hit effect voice. `hit+0x50` (signed) negative
/// stops. Without a voice (`[object+0x80] == 0`) the fuse `f32[object+0x1CC]` is reset to 0.0 (and flag `0x10` set
/// at `+0x28`) when it is negative or the flag is clear, the voice is created with `0x16C170(object, 1)`, stored
/// at `+0x80` and configured with `0x172760(voice, object, byte hit+0x50, hit+0x54, hit+0x58)`. Then, unless the
/// kind `[object+0x20]` is `0x10000000` or `0x400000`, with the network flag the voice gets
/// `0x168130(voice, 0x800100)` and `0x168100(voice, 3.0)`. MEASURED (calls recorded).
pub fn start_hit_voice(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
    hit: u32,
) {
    if (memory.read_u32(hit.wrapping_add(0x50)) as i32) < 0 {
        return;
    }
    if memory.read_u32(object.wrapping_add(0x80)) == 0 {
        let fuse = f32_at(memory, object.wrapping_add(0x1CC));
        let flags = memory.read_u32(object.wrapping_add(0x28));
        if 0.0 > fuse || flags & 0x10 == 0 {
            memory.write_u32(object.wrapping_add(0x28), flags | 0x10);
            memory.write_u32(object.wrapping_add(0x1D8), 0);
            memory.write_u32(object.wrapping_add(0x1CC), 0);
        }
        let voice = effects.call(memory, callee::VOICE_CREATE, &[object, 1]);
        memory.write_u32(object.wrapping_add(0x80), voice);
        let extra = memory.read_u32(hit.wrapping_add(0x58));
        let position = memory.read_u32(hit.wrapping_add(0x54));
        let channel = u32::from(memory.read_u8(hit.wrapping_add(0x50)));
        effects.call(
            memory,
            callee::VOICE_CONFIGURE,
            &[voice, object, channel, position, extra],
        );
    }
    let kind = memory.read_u32(object.wrapping_add(0x20));
    if kind == 0x1000_0000 || kind == 0x40_0000 {
        return;
    }
    if is_network(memory) == 0 {
        return;
    }
    let voice = memory.read_u32(object.wrapping_add(0x80));
    effects.call(memory, callee::VOICE_FLAGS, &[voice, 0x80_0100]);
    let voice = memory.read_u32(object.wrapping_add(0x80));
    effects.call(memory, callee::VOICE_VOLUME, &[voice, 0x4040_0000]);
}

/// `0x6B060(object, hit)` (void): consumes the pending hit `[0x6F32F4]` when it equals `hit`. Clears the pending
/// word, raises the fuse of `object` (`+0x1CC`, `+0x1D8`, and `+0x1DC = fuse * [0x475CD4]`, flag `0x10` at `+0x28`)
/// when `f32[hit+0x44]` is not below `[0x475CAC]`, ors `hit+0x48` into `object+0x9C`, stores `hit+0x4C` at
/// `[object+4]+0x30` when set, and unless level `[0x79094C] == 0x64` continues with [`start_hit_voice`]. MEASURED.
pub fn consume_hit(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
    hit: u32,
) {
    if hit == 0 || hit != memory.read_u32(GLOBAL_PENDING_HIT) {
        return;
    }
    let strength = f32_at(memory, hit.wrapping_add(0x44));
    let threshold = f32_at(memory, HIT_THRESHOLD);
    memory.write_u32(GLOBAL_PENDING_HIT, 0);
    // comiss + jb: skipped when strength < threshold or unordered, so only an ordered `>=` enters
    if strength >= threshold {
        let flags = memory.read_u32(object.wrapping_add(0x28));
        if flags & 0x10 == 0 {
            memory.write_u32(object.wrapping_add(0x28), flags | 0x10);
            let bits = memory.read_u32(hit.wrapping_add(0x44));
            memory.write_u32(object.wrapping_add(0x1D8), bits);
            memory.write_u32(object.wrapping_add(0x1CC), bits);
            let scaled = f32::from_bits(bits) * f32_at(memory, HIT_SCALE);
            memory.write_u32(object.wrapping_add(0x1DC), scaled.to_bits());
        }
    }
    let mask = memory.read_u32(hit.wrapping_add(0x48));
    let current = memory.read_u32(object.wrapping_add(0x9C));
    memory.write_u32(object.wrapping_add(0x9C), current | mask);
    let attach = memory.read_u32(hit.wrapping_add(0x4C));
    if attach != 0 {
        let target = memory.read_u32(object.wrapping_add(4));
        memory.write_u32(target.wrapping_add(0x30), attach);
    }
    if memory.read_u32(GLOBAL_LEVEL) == 0x64 {
        return;
    }
    start_hit_voice(memory, effects, object, hit);
}

// ---------------------------------------------------------------------------------------------------------
// Object pool: 0xA6880 (get and initialise) and 0xA7090 (destroy)
// ---------------------------------------------------------------------------------------------------------

/// Engine routines reached from the pool object functions.
pub mod pool_callee {
    pub const POOL_ALLOC: u32 = 0x9_EE00;
    pub const MODEL_READY: u32 = 0x1_2EC0;
    pub const MODEL_LOAD_A: u32 = 0x8_E4B0;
    pub const MODEL_LOAD_B: u32 = 0x8_E590;
    pub const MODEL_FIND: u32 = 0x9_6410;
    pub const MODEL_FLUSH: u32 = 0x8_E3D0;
    pub const OBJECT_NET_INIT: u32 = 0x6_CDB0;
    pub const OBJECT_SHAPE: u32 = 0x9_7C00;
    pub const RANDOM_64: u32 = 0x15_3B40;
    pub const OBJECT_ATTACH: u32 = 0x9_66E0;
    pub const OBJECT_PLACE: u32 = 0xA_6530;
    pub const OBJECT_CLASS_INIT: u32 = 0x9_FBE0;
    pub const OBJECT_TAIL_A: u32 = 0xA_67C0;
    pub const OBJECT_TAIL_B: u32 = 0x3_89B0;
    pub const DISPATCH: u32 = 0x9_E330;
    pub const KEY_RELEASE_A: u32 = 0x24_0410;
    pub const KEY_RELEASE_B: u32 = 0x24_04D0;
    pub const DETACH_A: u32 = 0x12_9EC0;
    pub const DETACH_B: u32 = 0x15_BFF0;
    pub const DETACH_C: u32 = 0xD_2D50;
    pub const DETACH_D: u32 = 0x22_7920;
    pub const DETACH_E: u32 = 0x10_E110;
    pub const PHYSICS_RELEASE: u32 = 0xE_0520;
    pub const SHAPE_RELEASE: u32 = 0x11_29B0;
    pub const BUFFER_FREE: u32 = 0x8_E9D0;
    pub const VOICE_RELEASE: u32 = 0x17_0ED0;
    pub const EMITTER_RELEASE: u32 = 0x12_74A0;
    pub const SCRIPT_RELEASE: u32 = 0xA_0010;
    pub const HANDLE_RELEASE: u32 = 0x4_5460;
}

pub const GLOBAL_MODEL_INDEX: u32 = 0x78_B860;
pub const GLOBAL_MODEL_TABLE: u32 = 0x79_1D4C;
pub const GLOBAL_POOL_FAILURES: u32 = 0x7B_65B8;
pub const GLOBAL_HANDLE_COUNTER: u32 = 0x72_F5E0;
pub const GLOBAL_FRAME_SEED: u32 = 0x7D_E318;
pub const GLOBAL_OBJECT_LIST_HEAD: u32 = 0x7B_4CC0;
pub const GLOBAL_OBJECT_LIST_TAIL: u32 = 0x7B_4CB8;

/// The three object lists an object is removed from: `(count cell, array)` for list ids 0, 8 and 9.
fn object_list(list: u32) -> Option<(u32, u32)> {
    match list {
        0 => Some((0x7B_6258, 0x7B_4FC0)),
        8 => Some((0x7B_4FA8, 0x71_61C8)),
        9 => Some((0x7B_625C, 0x71_6148)),
        _ => None,
    }
}

/// The list removal loop of `0xA6880` and `0xA7090` for list ids `0..0x12`: per iteration sets `[object+0x90]` to the
/// frame seed `[0x7DE318]` and `[object + 0x154 + 4*list] = 0`; lists 0, 8 and 9 additionally remove the first
/// occurrence of `object` from their pointer array (later entries move down, count decremented). MEASURED.
fn detach_from_lists(memory: &mut dyn GuestMemory, object: u32) {
    for list in 0..0x12u32 {
        let seed = memory.read_u32(GLOBAL_FRAME_SEED);
        memory.write_u32(object.wrapping_add(0x90), seed);
        memory.write_u32(object.wrapping_add(0x154 + 4 * list), 0);
        let Some((count_cell, array)) = object_list(list) else {
            continue;
        };
        let count = memory.read_u32(count_cell);
        if (count as i32) <= 0 {
            continue;
        }
        let mut found = None;
        for index in 0..count {
            if memory.read_u32(array.wrapping_add(index.wrapping_mul(4))) == object {
                found = Some(index);
                break;
            }
        }
        let Some(index) = found else {
            continue;
        };
        for slot in index..count.wrapping_sub(1) {
            let next = memory.read_u32(array.wrapping_add(slot.wrapping_mul(4)).wrapping_add(4));
            memory.write_u32(array.wrapping_add(slot.wrapping_mul(4)), next);
        }
        memory.write_u32(count_cell, count.wrapping_sub(1));
    }
}

/// `0xA6880(model, position, velocity)` -> object (0 when the model could not be loaded): takes a pool object
/// (`0x9EE00`), binds the model descriptor `[0x791D4C] + 0x30 * model_index[model]`, loads the model on first use,
/// assigns the handle (`[0x72F5E0]` counter in the low 22 bits, pool slot in the high bits) and initialises about 100
/// object fields in the original order. Calls through [`DestroyEffects`] keep their original order and arguments
/// (`pool_callee`). MEASURED (whole routine, callees stubbed).
pub fn pool_get(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    model: u32,
    position: u32,
    velocity: u32,
) -> u32 {
    use pool_callee as c;
    let object = effects.call(memory, c::POOL_ALLOC, &[]);
    let at = |offset: u32| object.wrapping_add(offset);
    let index_word =
        u16::from(memory.read_u8(GLOBAL_MODEL_INDEX.wrapping_add(model.wrapping_mul(2))))
            | u16::from(
                memory.read_u8(
                    GLOBAL_MODEL_INDEX
                        .wrapping_add(model.wrapping_mul(2))
                        .wrapping_add(1),
                ),
            ) << 8;
    memory.write_u8(at(0x96), index_word as u8);
    memory.write_u8(at(0x97), (index_word >> 8) as u8);
    let table = memory.read_u32(GLOBAL_MODEL_TABLE);
    let descriptor = u32::from(index_word).wrapping_mul(0x30).wrapping_add(table);
    memory.write_u32(at(0x24), descriptor);
    if memory.read_u32(descriptor.wrapping_add(4)) == 0 {
        let flags = memory.read_u32(descriptor.wrapping_add(0x28));
        let special =
            memory.read_u32(GLOBAL_LEVEL) == 0x66 && (0x7BC..=0x875).contains(&(model as i32));
        if special {
            let ready = effects.call(memory, c::MODEL_READY, &[descriptor]);
            let loader = if ready != 0 {
                c::MODEL_LOAD_A
            } else {
                c::MODEL_LOAD_B
            };
            let name = memory.read_u32(descriptor.wrapping_add(0x10));
            let mode = if (model as i32) < 0x86A { 1 } else { 2 };
            let loaded = effects.call(memory, loader, &[name, mode]);
            let owner = memory.read_u32(at(0x24));
            memory.write_u32(owner.wrapping_add(4), loaded);
        } else {
            let name = memory.read_u32(descriptor.wrapping_add(0x10));
            let loaded = effects.call(memory, c::MODEL_FIND, &[name, model]);
            let owner = memory.read_u32(at(0x24));
            memory.write_u32(owner.wrapping_add(4), loaded);
            if memory.read_u32(owner.wrapping_add(4)) == 0 {
                let state = memory.read_u32(at(0x28)) | 0x8000_0000;
                memory.write_u32(at(0x2C), 0);
                memory.write_u32(at(0x28), state);
                let failures = memory.read_u32(GLOBAL_POOL_FAILURES).wrapping_add(1);
                memory.write_u32(GLOBAL_POOL_FAILURES, failures);
                return 0;
            }
            let second = memory.read_u32(owner.wrapping_add(0x14));
            if second != 0 {
                let found = effects.call(memory, c::MODEL_FIND, &[second, model]);
                let owner = memory.read_u32(at(0x24));
                memory.write_u32(owner.wrapping_add(8), found);
            }
            let owner = memory.read_u32(at(0x24));
            let value = memory.read_u32(owner.wrapping_add(0x28)) & 0xFBFF_FFFF;
            memory.write_u32(owner.wrapping_add(0x28), value);
        }
        let owner = memory.read_u32(at(0x24));
        if memory.read_u32(owner) == 0x10 {
            effects.call(memory, c::MODEL_FLUSH, &[1]);
        }
        let clear_kind = if flags & 0x0400_0000 != 0 {
            let owner = memory.read_u32(at(0x24));
            effects.call(memory, c::MODEL_READY, &[owner]) == 0
        } else {
            true
        };
        if clear_kind {
            let owner = memory.read_u32(at(0x24));
            memory.write_u32(owner.wrapping_add(0x20), 0);
        }
    }
    // handle: pool slot in the top 10 bits, running counter below
    let counter = memory.read_u32(GLOBAL_HANDLE_COUNTER);
    let handle = (memory.read_u32(at(0x88)) << 0x16) | counter;
    let next = counter.wrapping_add(1);
    memory.write_u32(at(0x2C), handle);
    memory.write_u32(
        GLOBAL_HANDLE_COUNTER,
        if next & 0xFFC0_0000 != 0 { 1 } else { next },
    );
    memory.write_u32(at(0x2D8), 0);
    memory.write_u8(at(0x94), model as u8);
    memory.write_u8(at(0x95), (model >> 8) as u8);
    let level = memory.read_u32(GLOBAL_LEVEL);
    if memory.read_u32(GLOBAL_FLAGS_C) & 0x40 == 0
        && level != 0x64
        && level != 0x38
        && (memory.read_u8(GLOBAL_MODE_A) == 0xA || level == 0x66)
    {
        let owner = memory.read_u32(at(0x24));
        if memory.read_u32(owner.wrapping_add(0x28)) & 0x0200_0000 != 0 {
            effects.call(memory, c::OBJECT_NET_INIT, &[object]);
        }
    }
    let owner = memory.read_u32(at(0x24));
    let kind = memory.read_u32(owner);
    memory.write_u32(at(0x20), kind);
    memory.write_u32(at(0x8C), 0xFFFF_FFFF);
    let shape = effects.call(memory, c::OBJECT_SHAPE, &[object, owner]);
    let kind = memory.read_u32(at(0x20));
    memory.write_u32(at(4), shape);
    memory.write_u32(at(0x74), shape);
    for offset in [0xA4, 0x10, 0x9C, 0x18, 0xA0, 0x1A0] {
        memory.write_u32(at(offset), 0);
    }
    memory.write_u32(at(0x28), if kind == 2 { 0x1061 } else { 0x1161 });
    memory.write_u8(at(0x14), 0);
    memory.write_u8(at(0x15), 0);
    for source in 0..3u32 {
        let value = memory.read_u32(position.wrapping_add(4 * source));
        for base in [0xA8, 0x64, 0xEC] {
            memory.write_u32(at(base + 4 * source), value);
        }
    }
    memory.write_u8(at(0x31), 0);
    memory.write_u32(at(0x34), 0);
    let flat = f32_at(memory, velocity) == 0.0 && f32_at(memory, velocity.wrapping_add(8)) == 0.0;
    memory.write_u8(at(0x32), if flat { 1 } else { 2 });
    let (vx, vy, vz) = (
        memory.read_u32(velocity),
        memory.read_u32(velocity.wrapping_add(4)),
        memory.read_u32(velocity.wrapping_add(8)),
    );
    memory.write_u32(at(0xC0), vx);
    memory.write_u32(at(0x70), vy);
    memory.write_u32(at(0xC4), vy);
    for offset in [0xE0, 0xE4, 0xE8, 0xB4, 0xB8, 0xBC] {
        memory.write_u32(at(offset), 0);
    }
    memory.write_u32(at(0xC8), vz);
    memory.write_u32(at(0x104), vx);
    memory.write_u32(at(0x108), vy);
    for offset in [0xCC, 0xD0, 0x134, 0x138, 0x13C, 0x140, 0x144, 0x148] {
        memory.write_u32(at(offset), 0);
    }
    let fuse = memory.read_u32(0x47_5C78);
    memory.write_u32(at(0x1D8), fuse);
    memory.write_u32(at(0x1CC), fuse);
    let scale = memory.read_u32(HIT_SCALE);
    memory.write_u32(at(0x10C), vz);
    for offset in [0x208, 0x20C, 0x80, 0x84, 0x254] {
        memory.write_u32(at(offset), 0);
    }
    memory.write_u8(at(0x30), 0);
    memory.write_u32(at(0x78), 0);
    memory.write_u32(at(0x128), 0);
    for offset in [0x12C, 0x130, 0x110] {
        memory.write_u32(at(offset), 0xFFFF_FFFF);
    }
    memory.write_u32(at(0x114), 0);
    memory.write_u32(at(0x150), 0);
    memory.write_u32(at(0x1DC), scale);
    for offset in [0x270, 0x228, 0x22C] {
        memory.write_u32(at(offset), 0xFFFF_FFFF);
    }
    memory.write_u32(at(0x2B4), 0x8000_0000);
    for offset in [0x234, 0x238, 0x23C, 0x240] {
        memory.write_u32(at(offset), 0xFFFF_FFFF);
    }
    memory.write_u8(at(0x244), 0xFF);
    memory.write_u8(at(0x245), 0xFF);
    let owner = memory.read_u32(at(0x24));
    memory.write_u32(at(0x258), 0);
    let seed = memory.read_u32(GLOBAL_FRAME_SEED);
    memory.write_u32(at(0x90), seed);
    if (memory.read_u32(owner.wrapping_add(0x24)) as i32) > 0 {
        let random = effects.call(memory, c::RANDOM_64, &[]);
        let owner = memory.read_u32(at(0x24));
        let modulus = memory.read_u32(owner.wrapping_add(0x24));
        let remainder = random % modulus;
        let current = memory.read_u32(at(0x90));
        memory.write_u32(at(0x90), current.wrapping_sub(remainder));
    }
    detach_from_lists(memory, object);
    let second_scale = if memory.read_u32(at(0x20)) == 4 {
        0x47_D55C
    } else {
        0x47_D558
    };
    let value = memory.read_u32(second_scale);
    memory.write_u32(at(0x19C), 0);
    memory.write_u32(at(0x26C), 0);
    memory.write_u32(at(0x25C), value);
    let value = memory.read_u32(0x47_85E4);
    memory.write_u32(at(0x260), value);
    memory.write_u32(at(0x7C), 0);
    let owner = memory.read_u32(at(0x24));
    if memory.read_u8(owner.wrapping_add(0x28)) & 0x10 != 0 {
        let shape = memory.read_u32(at(4));
        effects.call(memory, c::OBJECT_ATTACH, &[shape, 1, 0]);
    }
    memory.write_u32(at(0x1E4), 0);
    effects.call(memory, c::OBJECT_PLACE, &[object]);
    let (a, b, c_) = (
        memory.read_u32(at(0x1A8)),
        memory.read_u32(at(0x1AC)),
        memory.read_u32(at(0x1B0)),
    );
    memory.write_u32(at(0x1BC), c_);
    let flags = memory.read_u32(at(0x18)) | 0x20;
    memory.write_u32(at(0x1B4), a);
    memory.write_u32(at(0x1B8), b);
    memory.write_u32(at(0x230), 0);
    memory.write_u32(at(0x18), flags);
    let level = memory.read_u32(GLOBAL_LEVEL);
    if level == 0x65 || level == 0x66 {
        memory.write_u32(at(0x34), 0);
    } else {
        effects.call(memory, c::OBJECT_CLASS_INIT, &[object]);
        if memory.read_u32(at(0x20)) != 2 {
            let value = memory.read_u32(at(0x38));
            memory.write_u32(at(0x34), value);
        } else {
            let id = u32::from(memory.read_u8(at(0x94))) | u32::from(memory.read_u8(at(0x95))) << 8;
            memory.write_u32(at(0x34), id.wrapping_sub(0x907));
        }
        if memory.read_u32(at(0x34)) == 0 {
            let value = memory.read_u32(at(0x38));
            memory.write_u32(at(0x34), value);
        }
    }
    if memory.read_u32(GLOBAL_LEVEL) != 0x66 {
        effects.call(memory, c::OBJECT_TAIL_A, &[object]);
        let shape = memory.read_u32(at(4));
        effects.call(memory, c::OBJECT_TAIL_B, &[shape]);
    }
    effects.call(memory, c::OBJECT_PLACE, &[object]);
    object
}

/// `0xA7090(object)` (void): destroys a pool object. Runs the destroy callback `[object+0x184](object, 0)`, destroys the
/// child `[object+0x208]` recursively (kind `0x100000` objects keep it), releases the handle keys, unlinks the object
/// from the singly linked list `[0x7B4CC0]` (tail `[0x7B4CB8]`), clears flag bits `0xFFDFFB8F` of `+0x28`, detaches
/// it from the render lists, frees the sub allocations at `+0x74/+0xA4/+0x80/+0x204/+0x78/+0x22C/+0x228`, marks the
/// slot free (`+0x28 |= 0x80000000`, `+0x2C = 0`) and counts it in `[0x7B65B8]`. MEASURED (callees stubbed).
pub fn destroy_pool_object(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
) {
    use pool_callee as c;
    let at = |offset: u32| object.wrapping_add(offset);
    let callback = memory.read_u32(at(0x184));
    if callback != 0 {
        effects.call(memory, callback, &[object, 0]);
    }
    let child = memory.read_u32(at(0x208));
    if child != 0 && memory.read_u32(at(0x20)) != 0x10_0000 {
        effects.call(memory, c::DISPATCH, &[child, 1, 0]);
        destroy_pool_object(memory, effects, child);
        memory.write_u32(at(0x208), 0);
    }
    let handle = memory.read_u32(at(0x2C));
    if handle != 0 {
        effects.call(memory, c::KEY_RELEASE_A, &[handle]);
    }
    let handle = memory.read_u32(at(0x2C));
    effects.call(memory, c::KEY_RELEASE_B, &[handle]);
    memory.write_u32(at(0x60), 0);
    // unlink from the singly linked list (head cell acts as the first "previous" node)
    let mut previous = GLOBAL_OBJECT_LIST_HEAD;
    let mut node = memory.read_u32(GLOBAL_OBJECT_LIST_HEAD);
    while node != 0 {
        if node == object {
            if object == memory.read_u32(GLOBAL_OBJECT_LIST_TAIL) {
                memory.write_u32(GLOBAL_OBJECT_LIST_TAIL, previous);
            }
            let next = memory.read_u32(node);
            memory.write_u32(previous, next);
            memory.write_u32(object, 0);
            break;
        }
        previous = node;
        node = memory.read_u32(node);
    }
    let flags = memory.read_u32(at(0x28)) & 0xFFDF_FB8F;
    memory.write_u32(at(0x28), flags);
    effects.call(memory, c::DETACH_A, &[object]);
    effects.call(memory, c::DETACH_B, &[object]);
    effects.call(memory, c::DETACH_C, &[object]);
    effects.call(memory, c::DETACH_D, &[object]);
    effects.call(memory, c::DETACH_E, &[object]);
    detach_from_lists(memory, object);
    if (memory.read_u32(GLOBAL_LEVEL) as i32) <= 0x64 {
        if memory.read_u32(at(0x28)) & 0x1_0000 != 0 {
            effects.call(memory, c::PHYSICS_RELEASE, &[object]);
        }
        effects.call(memory, c::SHAPE_RELEASE, &[object]);
    }
    for offset in [0x74, 0xA4] {
        let buffer = memory.read_u32(at(offset));
        if buffer != 0 {
            effects.call(memory, c::BUFFER_FREE, &[buffer]);
            memory.write_u32(at(offset), 0);
        }
    }
    let voice = memory.read_u32(at(0x80));
    memory.write_u32(at(4), 0);
    if voice != 0 {
        effects.call(memory, c::VOICE_RELEASE, &[object]);
    }
    let emitter = memory.read_u32(at(0x204));
    if emitter != 0 {
        effects.call(memory, c::EMITTER_RELEASE, &[emitter]);
    }
    if memory.read_u32(at(0x78)) != 0 {
        effects.call(memory, c::SCRIPT_RELEASE, &[object]);
    }
    for offset in [0x22C, 0x228] {
        let handle = memory.read_u32(at(offset));
        if (handle as i32) >= 0 {
            effects.call(memory, c::HANDLE_RELEASE, &[handle]);
            memory.write_u32(at(offset), 0xFFFF_FFFF);
        }
    }
    let state = memory.read_u32(at(0x28)) | 0x8000_0000;
    memory.write_u32(at(0x2C), 0);
    memory.write_u32(at(0x28), state);
    let failures = memory.read_u32(GLOBAL_POOL_FAILURES).wrapping_add(1);
    memory.write_u32(GLOBAL_POOL_FAILURES, failures);
}
