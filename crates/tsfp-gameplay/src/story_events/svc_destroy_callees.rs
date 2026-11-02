//! T1862 batch 3: the remaining engine callees of the object destroy `0xBABA0`.
//!
//! Same method as [`super::svc_destroy`]: each routine mirrors the original machine code (MEASURED against the Unicorn
//! oracle with the engine routines below it as logging stubs, see `docs/t-story-event-destroy-t1862.md`), names are
//! INFERRED. Calls to routines of other subsystems go through [`DestroyEffects::call`] keyed by their original address;
//! routines taking register arguments pass the registers as documented per function. Stack frames are modelled where the
//! original writes below or into its caller's frame (`entry` = esp at function entry), because those writes are part of
//! the observable memory state.

use super::strip_update::GuestMemory;
use super::svc_destroy::{GLOBAL_LEVEL, channel_count, is_network, string_lookup};
use super::svc_objects::{
    DestroyEffects, GLOBAL_FIRST_SLOT, GLOBAL_FLAGS_C, GLOBAL_MODE_A, GLOBAL_MODE_B,
};

/// Original addresses of the engine routines reached from this module.
pub mod hooks {
    pub const TEXT_POST_A: u32 = 0x12_D6B0;
    pub const TEXT_POST_B: u32 = 0x12_D760;
    pub const SERVER_SEND: u32 = 0x35_BE90;
    pub const PEER_READY: u32 = 0x35_AF40;
    pub const PLAYER_RESPAWN: u32 = 0x23_8E00;
    pub const RANDOM_64: u32 = 0x15_3B40;
    pub const EFFECT_REQUEST: u32 = 0x2_18A0;
    pub const CAMERA_A: u32 = 0xD_3DE0;
    pub const CAMERA_B: u32 = 0xD_3F80;
    pub const PLAYER_STATE: u32 = 0x6_4D10;
    pub const SPECTATE: u32 = 0x12_1BF0;
    pub const KILL_STAT: u32 = 0x25_A780;
    pub const KILL_LOG: u32 = 0xD_3C50;
    pub const NEXT_PHASE: u32 = 0x2E_6D40;
    pub const STRNCPY: u32 = 0x3C_8960;
    pub const HUD_SOUND: u32 = 0x4_0F30;
    pub const HUD_RANDOM: u32 = 0x3C_848C;
    pub const HUD_WIDTH: u32 = 0x4_0650;
    pub const HOST_SYNC: u32 = 0x35_70D0;
    pub const HOST_PHASE_A: u32 = 0x6_6E90;
    pub const HOST_PHASE_B: u32 = 0x6_6FC0;
    pub const HOST_PHASE_C: u32 = 0x23_1A20;
    pub const HUD_ICON: u32 = 0xF_8AD0;
    pub const HUD_ICON_SHOW: u32 = 0xF_D7E0;
    pub const FEED_LINE: u32 = 0x16_4F60;
    pub const FEED_FLAG: u32 = 0x16_4A30;
    pub const FEED_SKIP: u32 = 0xD_6C90;
    pub const SCORE_RECENT: u32 = 0x6_41A0;
    pub const SCORE_TEAM: u32 = 0x6_4160;
    pub const SCORE_PEER: u32 = 0x6_41C0;
    pub const FORMAT_TEXT: u32 = 0x15_1030;
    pub const SUBMIT_FILL: u32 = 0x4_5390;
    pub const SUBMIT: u32 = 0x4_5820;
    pub const CLEAR_STATE: u32 = 0x35_0E70;
    pub const BLEND_GATE: u32 = 0x35_C4E0;
    pub const CHANNEL_MASK: u32 = 0x9_E720;
    pub const ACTION_TEST: u32 = 0x1_74E0;
    pub const ACTION_SET: u32 = 0x1_7570;
    pub const SOUND_3D: u32 = 0x5_F460;
    pub const BLEND_MIRROR: u32 = 0x11_D8A0;
    pub const BLEND_START: u32 = 0x37_8530;
    pub const BLEND_STOP: u32 = 0x10_E1C0;
    pub const FREE_PRE: u32 = 0x15_C310;
    pub const FREE_CLEANUP: u32 = 0x1_642C0;
    pub const FREE_NOTIFY: u32 = 0x27_C2F0;
    pub const FREE_MODEL: u32 = 0x8_E970;
    pub const FREE_LIMITED: u32 = 0x5_9D20;
    pub const FREE_SCALE: u32 = 0x6_13D0;
    pub const FREE_DROP: u32 = 0x16_1D40;
    pub const FREE_LINK: u32 = 0xD_7D10;
    pub const FREE_STATE: u32 = 0x5_9E00;
    pub const FREE_FALLBACK: u32 = 0xD_AAE0;
    pub const FREE_FX_TEST: u32 = 0xD_6DD0;
    pub const FREE_FX_A: u32 = 0x11_ECE0;
    pub const FREE_FX_B: u32 = 0x11_E7B0;
    pub const FINAL_STATE: u32 = 0x2B_23D0;
    pub const FINAL_PEER: u32 = 0x15_BD00;
    pub const CRT_OUTPUT: u32 = 0x3C_B59D;
    pub const CRT_FLUSH: u32 = 0x3C_B25F;
}
use hooks as h;

const GLOBAL_NETWORK_B: u32 = 0x76_B174;
const GLOBAL_PLAYERS: u32 = 0x7B_0C48;
const GLOBAL_PLAYER_STRIDE: u32 = 0x1584;
const GLOBAL_SLOT_BASE: u32 = 0x73_56D8;
const GLOBAL_SLOT_STRIDE: u32 = 0xEA0;
const GLOBAL_SERVER_STATE: u32 = 0x76_B140;
const GLOBAL_STAT_FLAG: u32 = 0x76_B194;

fn f32_at(memory: &dyn GuestMemory, address: u32) -> f32 {
    f32::from_bits(memory.read_u32(address))
}

fn sext16(memory: &dyn GuestMemory, address: u32) -> u32 {
    let low = u32::from(memory.read_u8(address))
        | u32::from(memory.read_u8(address.wrapping_add(1))) << 8;
    (low as u16 as i16) as i32 as u32
}

fn read_u16(memory: &dyn GuestMemory, address: u32) -> u16 {
    u16::from(memory.read_u8(address)) | u16::from(memory.read_u8(address.wrapping_add(1))) << 8
}

fn write_u16(memory: &mut dyn GuestMemory, address: u32, value: u16) {
    memory.write_u8(address, value as u8);
    memory.write_u8(address.wrapping_add(1), (value >> 8) as u8);
}

// ---------------------------------------------------------------------------------------------------------
// session predicates: 0x355EC0, 0x355F10, 0x357690
// ---------------------------------------------------------------------------------------------------------

/// `0x355EC0()`: the local peer is the server. Level `0x65` tests `[0x76B140] == 1`; otherwise true without a
/// network, with a network `[0x76B140] == 1`. MEASURED.
pub fn is_server(memory: &dyn GuestMemory) -> u32 {
    if memory.read_u32(GLOBAL_LEVEL) == 0x65 {
        return u32::from(memory.read_u32(GLOBAL_SERVER_STATE) == 1);
    }
    if is_network(memory) == 0 {
        return 1;
    }
    u32::from(memory.read_u32(GLOBAL_SERVER_STATE) == 1)
}

/// `0x355F10()`: the local peer is a client (`[0x76B140] == 2`, with a network unless level `0x65`). MEASURED.
pub fn is_client(memory: &dyn GuestMemory) -> u32 {
    if memory.read_u32(GLOBAL_LEVEL) == 0x65 {
        return u32::from(memory.read_u32(GLOBAL_SERVER_STATE) == 2);
    }
    u32::from(is_network(memory) != 0 && memory.read_u32(GLOBAL_SERVER_STATE) == 2)
}

/// `0x357690()`: with a network and the server role the sync routine `0x3570D0` decides, else `[0x76B194]`. MEASURED.
pub fn host_stat(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects) -> u32 {
    if is_network(memory) != 0 && is_server(memory) != 0 {
        return effects.call(memory, h::HOST_SYNC, &[]);
    }
    memory.read_u32(GLOBAL_STAT_FLAG)
}

// ---------------------------------------------------------------------------------------------------------
// 0x359400 / 0x35CC10: network packet for the owner test
// ---------------------------------------------------------------------------------------------------------

/// `0x359400` (register arguments `ecx = object`, `eax = &cursor`): appends an object reference to the packet at the
/// pointer stored in `cursor`. Null: byte 0. Kind `0x400000`: byte 3 then the index of the object in the list
/// `[0x7A20C0]` (node: next `+0`, object `+0x4C`), or `0xFF` when absent. Object with `+0x150`: byte 1 then the first
/// byte of `[+0x150]`. Object with a non zero `+0x230`: byte 2 then that dword. Anything else appends nothing. MEASURED.
pub fn write_ref(memory: &mut dyn GuestMemory, object: u32, cursor: u32) {
    let advance = |memory: &mut dyn GuestMemory, by: u32| {
        let value = memory.read_u32(cursor).wrapping_add(by);
        memory.write_u32(cursor, value);
    };
    let put = |memory: &mut dyn GuestMemory, value: u8| {
        let at = memory.read_u32(cursor);
        memory.write_u8(at, value);
        advance(memory, 1);
    };
    if object == 0 {
        put(memory, 0);
        return;
    }
    if memory.read_u32(object.wrapping_add(0x20)) == 0x40_0000 {
        let mut node = memory.read_u32(0x7A_20C0);
        let mut index = 0u32;
        let mut found = false;
        while node != 0 {
            if memory.read_u32(node.wrapping_add(0x4C)) == object {
                found = true;
                break;
            }
            node = memory.read_u32(node);
            index = index.wrapping_add(1);
        }
        put(memory, 3);
        put(memory, if found { index as u8 } else { 0xFF });
        return;
    }
    let first = memory.read_u32(object.wrapping_add(0x150));
    if first != 0 {
        put(memory, 1);
        let byte = memory.read_u8(first);
        put(memory, byte);
        return;
    }
    let value = memory.read_u32(object.wrapping_add(0x230));
    if value != 0 {
        put(memory, 2);
        let at = memory.read_u32(cursor);
        memory.write_u32(at, value);
        advance(memory, 4);
    }
}

/// Appends the three high bytes of the float `bits` at `at` through the cursor temp, like the original (the float is
/// stored to the cursor cell and read back as bytes 1..3).
fn put_float24(memory: &mut dyn GuestMemory, cursor: u32, at: u32, bits: u32) {
    memory.write_u32(cursor, bits);
    memory.write_u8(at, (bits >> 8) as u8);
    memory.write_u8(at.wrapping_add(1), (bits >> 16) as u8);
    memory.write_u8(at.wrapping_add(2), (bits >> 24) as u8);
}

/// Dwords of the packet buffer handed to the recorded send (`0x35BE90`) so the packet content is part of the call.
pub const PACKET_WORDS: u32 = 16;

/// `0x35CC10(block, word)` (cdecl, eax): builds the destroy packet of a request block and sends it with `0x35BE90`
/// (stack `0x12, 0x13, buffer`, register `ebx` = length, `edi` = 1). Without a network, or while `[0x76B174]` is set,
/// returns 1 without a packet. A killer record with a negative `+4` of the `0x1584` player record, or a peer that
/// is not ready (`0x35AF40`, register `esi`) returns 0. `entry` = esp at entry: the buffer is `entry - 0x100`, its
/// cursor cell `entry - 0x104`, the flag byte `entry - 0xFC`. The recorded send receives
/// `[0x12, 0x13, buffer, PACKET_WORDS dwords at buffer, length, 1]`. MEASURED (send and ready test stubbed).
pub fn owner_test(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    block: u32,
    word: u32,
    entry: u32,
) -> u32 {
    if is_network(memory) == 0 {
        return 1;
    }
    if memory.read_u32(GLOBAL_NETWORK_B) != 0 {
        return 1;
    }
    let at = |offset: u32| block.wrapping_add(offset);
    let mut subject = memory.read_u32(at(8));
    if memory.read_u32(at(0x18)) == 0x15 {
        let object = memory.read_u32(block);
        if memory.read_u32(object.wrapping_add(0x20)) == 8 {
            let ctx = memory.read_u32(object.wrapping_add(0x7C));
            let id = memory.read_u32(ctx.wrapping_add(8));
            let players = memory.read_u32(GLOBAL_PLAYERS);
            let record = id.wrapping_mul(GLOBAL_PLAYER_STRIDE).wrapping_add(players);
            if (memory.read_u32(record.wrapping_add(4)) as i32) < 0 {
                return 0;
            }
            subject = object;
        }
    }
    if subject != 0 && memory.read_u32(subject.wrapping_add(0x20)) == 0x200 {
        let peer = memory.read_u32(block);
        if effects.call(memory, h::PEER_READY, &[peer]) == 0 {
            return 0;
        }
    }
    let mut flags = 0u8;
    if memory.read_u32(at(8)) != 0 {
        flags |= 1;
    }
    if memory.read_u32(at(0x28)) != 0 {
        flags |= 2;
    }
    if memory.read_u32(at(0x30)) != 0 {
        flags |= 4;
    }
    let other = memory.read_u32(at(4));
    if other != 0 {
        flags |= 8;
    }
    let buffer = entry.wrapping_sub(0x100);
    let cursor = entry.wrapping_sub(0x104);
    memory.write_u8(entry.wrapping_sub(0xFC), flags);
    memory.write_u32(cursor, entry.wrapping_sub(0xFB));
    let object = memory.read_u32(block);
    write_ref(memory, object, cursor);
    if flags & 1 != 0 {
        let reference = memory.read_u32(at(8));
        write_ref(memory, reference, cursor);
    }
    let mut position = memory.read_u32(cursor);
    memory.write_u8(position, memory.read_u8(at(0x18)));
    position = position.wrapping_add(1);
    memory.write_u32(cursor, position);
    for (mask, offset) in [(2u8, 0x28u32), (4, 0x30)] {
        if flags & mask != 0 {
            let vector = memory.read_u32(at(offset));
            for k in 0..3u32 {
                let bits = memory.read_u32(vector.wrapping_add(4 * k));
                put_float24(memory, cursor, position, bits);
                position = position.wrapping_add(3);
            }
            memory.write_u32(cursor, position);
        }
    }
    if flags & 8 != 0 {
        write_ref(memory, other, cursor);
        position = memory.read_u32(cursor);
    }
    let bits = memory.read_u32(at(0x34));
    put_float24(memory, cursor, position, bits);
    memory.write_u8(position.wrapping_add(3), memory.read_u8(at(0x48)));
    let low = memory.read_u8(at(0x40));
    position = position.wrapping_add(4);
    memory.write_u8(position, low);
    position = position.wrapping_add(1);
    write_u16(memory, position, word as u16);
    position = position.wrapping_add(2);
    let length = position.wrapping_sub(buffer);
    let mut args = vec![0x12, 0x13, buffer];
    args.extend((0..PACKET_WORDS).map(|k| memory.read_u32(buffer.wrapping_add(4 * k))));
    args.extend([length, 1]);
    effects.call(memory, h::SERVER_SEND, &args)
}

// ---------------------------------------------------------------------------------------------------------
// 0x278040 / 0x2776F0: level kill rule
// ---------------------------------------------------------------------------------------------------------

/// Original address of the byte table `level - 1 -> rule` (81 entries) read by `0x2776F0`.
pub const LEVEL_RULE_TABLE: u32 = 0x27_7784;
const RULE_TAGS: [u32; 5] = [
    0x4A42_5448,
    0x4350_5441,
    0x5231_3130,
    0x4143_484E,
    0x4854_4950,
];

/// `0x2776F0(context_object)`: whether the tag `[object + 8]` matches the tag of the level's rule. The level
/// `[0x79094C] - 1` (unsigned, at most `0x50`) indexes the byte table at `0x277784`; rules 0..4 compare against one
/// tag each, rule 5 (and levels beyond the table) is false. Table read from guest memory. MEASURED.
pub fn level_rule(memory: &dyn GuestMemory, object: u32) -> u32 {
    let index = memory.read_u32(GLOBAL_LEVEL).wrapping_sub(1);
    if index > 0x50 {
        return 0;
    }
    let rule = memory.read_u8(LEVEL_RULE_TABLE.wrapping_add(index)) as usize;
    match RULE_TAGS.get(rule) {
        Some(tag) => u32::from(memory.read_u32(object.wrapping_add(8)) == *tag),
        None => 0,
    }
}

/// `0x278040(ctx)`: true when the context has a controller `[ctx+0xBF0]`, mode byte `[0x7DE455] == 0xA`, exactly one
/// slot `[0x790950] == 1` and [`level_rule`] holds for the controller. MEASURED.
pub fn kill_test(memory: &dyn GuestMemory, ctx: u32) -> u32 {
    let controller = memory.read_u32(ctx.wrapping_add(0xBF0));
    if controller == 0
        || memory.read_u8(GLOBAL_MODE_A) != 0xA
        || memory.read_u32(GLOBAL_FIRST_SLOT) != 1
    {
        return 0;
    }
    level_rule(memory, controller)
}

// ---------------------------------------------------------------------------------------------------------
// 0x12D350: HUD message ring
// ---------------------------------------------------------------------------------------------------------

const HUD_RECORD: u32 = 0x7A_6160;
const HUD_RECORD_STRIDE: u32 = 0x177C;
const HUD_ENTRY_STRIDE: u32 = 0x16C;

fn c_string_equal(memory: &dyn GuestMemory, left: u32, right: u32) -> bool {
    let mut index = 0u32;
    loop {
        let a = memory.read_u8(left.wrapping_add(index));
        let b = memory.read_u8(right.wrapping_add(index));
        if a != b {
            return false;
        }
        if a == 0 {
            return true;
        }
        index = index.wrapping_add(1);
    }
}

fn c_string_length(memory: &dyn GuestMemory, text: u32) -> u32 {
    let mut length = 0u32;
    while memory.read_u8(text.wrapping_add(length)) != 0 {
        length = length.wrapping_add(1);
    }
    length
}

/// `0x12D350(player, kind, text, value)` -> entry pointer or 0: posts a HUD message in the 16 entry ring of the player
/// record (`0x177C` bytes from `0x7A6160`, head `+0x30`, entries of `0x16C` bytes from `+0x38`). Players 0..3, kinds 1..13
/// and a non null text only. Kind 6 always allocates; other kinds first look for the newest entry of the same kind
/// (15 steps back from the head) and return 0 without a new entry when its text (`+4`) is identical. A new entry copies
/// the text (`0x3C8960(dest, text, 0x140)`), sets kind `+0`, `-1` at `+0x15C/0x160/0x168`, `+0x144 = 0.0 - value`,
/// `+0x158 = 1` and fills `+0x148..0x154` per kind from constants (kind 2 also calls `0x40F30(dest, 0)` and
/// `0x3C848C()`, kind 7 `0x40650(dest)`, kind 9 scales the text length). MEASURED (callees stubbed).
pub fn hud_message(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    player: u32,
    kind: u32,
    text: u32,
    value: u32,
) -> u32 {
    let record = player
        .wrapping_mul(HUD_RECORD_STRIDE)
        .wrapping_add(HUD_RECORD);
    if (player as i32) < 0 || (player as i32) >= 4 || text == 0 {
        return 0;
    }
    if (kind as i32) <= 0 || (kind as i32) >= 0xE {
        return 0;
    }
    if kind != 6 {
        let head = memory.read_u32(record.wrapping_add(0x30));
        let mut cursor = head.wrapping_add(0xF);
        let mut step: i32 = 0xF;
        loop {
            let mut slot = cursor & 0x8000_000F;
            if (slot as i32) < 0 {
                slot = (slot.wrapping_sub(1) | 0xFFFF_FFF0).wrapping_add(1);
            }
            let entry = slot
                .wrapping_mul(HUD_ENTRY_STRIDE)
                .wrapping_add(record)
                .wrapping_add(0x38);
            if memory.read_u32(entry) == kind {
                if c_string_equal(memory, entry.wrapping_add(4), text) {
                    return 0;
                }
                break;
            }
            step -= 1;
            cursor = cursor.wrapping_sub(1);
            if step < 0 {
                break;
            }
        }
    }
    let head = memory.read_u32(record.wrapping_add(0x30));
    let entry = head
        .wrapping_mul(HUD_ENTRY_STRIDE)
        .wrapping_add(record)
        .wrapping_add(0x38);
    let next = head.wrapping_add(1);
    memory.write_u32(
        record.wrapping_add(0x30),
        if (next as i32) >= 0x10 { 0 } else { next },
    );
    let dest = entry.wrapping_add(4);
    effects.call(memory, h::STRNCPY, &[dest, text, 0x140]);
    let at = |offset: u32| entry.wrapping_add(offset);
    memory.write_u32(at(0x15C), 0xFFFF_FFFF);
    memory.write_u32(at(0x168), 0xFFFF_FFFF);
    memory.write_u32(at(0x160), 0xFFFF_FFFF);
    let negated = 0.0f32 - f32::from_bits(value);
    memory.write_u32(entry, kind);
    memory.write_u32(at(0x164), 0);
    memory.write_u8(at(0x143), 0);
    memory.write_u32(at(0x144), negated.to_bits());
    memory.write_u32(at(0x158), 1);
    let constant = |memory: &dyn GuestMemory, address: u32| memory.read_u32(address);
    // `+0x148/+0x14C/+0x150/+0x154` per kind; the store pattern follows the original handlers.
    match kind {
        1 => {
            let a = constant(memory, 0x47_5C78);
            let b = constant(memory, 0x47_5D10);
            memory.write_u32(at(0x148), a);
            memory.write_u32(at(0x150), b);
            memory.write_u32(at(0x154), a);
        }
        2 => {
            let a = constant(memory, 0x47_5D20);
            memory.write_u32(at(0x148), a);
            let b = constant(memory, 0x47_5D00);
            memory.write_u32(at(0x150), b);
            let c = constant(memory, 0x47_5D10);
            memory.write_u32(at(0x154), c);
            effects.call(memory, h::HUD_SOUND, &[dest, 0]);
            let random = effects.call(memory, h::HUD_RANDOM, &[]);
            memory.write_u32(at(0x158), random);
        }
        3 => {
            let a = constant(memory, 0x47_5D10);
            memory.write_u32(at(0x148), a);
            let b = constant(memory, 0x47_5CD4);
            memory.write_u32(at(0x14C), b);
            let c = constant(memory, 0x47_5D18);
            memory.write_u32(at(0x150), c);
            memory.write_u32(at(0x154), c);
        }
        4 | 11 => {
            let a = constant(memory, 0x47_5CD0);
            memory.write_u32(at(0x148), a);
            let b = constant(memory, 0x47_5D04);
            memory.write_u32(at(0x14C), b);
            let c = constant(memory, 0x47_8860);
            memory.write_u32(at(0x150), c);
            memory.write_u32(at(0x154), c);
        }
        5 | 13 => {
            let a = constant(memory, 0x47_85E0);
            memory.write_u32(at(0x148), a);
            let b = constant(memory, 0x47_65E0);
            memory.write_u32(at(0x14C), b);
            let c = constant(memory, 0x47_5C78);
            memory.write_u32(at(0x150), c);
            memory.write_u32(at(0x154), c);
        }
        7 => {
            let width = effects.call(memory, h::HUD_WIDTH, &[dest]);
            let mut value = (width as i32 as f32) * f32_at(memory, 0x47_5CE8);
            value += f32_at(memory, 0x47_5D18);
            memory.write_u32(at(0x148), value.to_bits());
            value += f32_at(memory, 0x47_5C78);
            memory.write_u32(at(0x150), value.to_bits());
            memory.write_u32(at(0x154), value.to_bits());
            memory.write_u32(record.wrapping_add(0x1710), 0);
        }
        8 => {
            let a = constant(memory, 0x47_88FC);
            memory.write_u32(at(0x148), a);
            memory.write_u32(at(0x150), a);
            memory.write_u32(at(0x154), a);
        }
        9 => {
            let length = c_string_length(memory, text);
            let scaled = ((length as i32).wrapping_sub(0x14) as f32) * f32_at(memory, 0x47_DA4C);
            let base = if 0.0f32 > scaled { 0.0 } else { scaled };
            let high = base + f32_at(memory, 0x47_5D10);
            let low = base + f32_at(memory, 0x47_5C78);
            memory.write_u32(at(0x148), high.to_bits());
            memory.write_u32(at(0x150), high.to_bits());
            memory.write_u32(at(0x154), low.to_bits());
        }
        _ => {}
    }
    entry
}

/// `0x12D6B0` (kind `0xA`) and `0x12D760` (kind `0xC`), `(player, text, id, value)`: post a HUD message into the ring
/// of the player record like [`hud_message`] but with a fixed layout: text copy, `+0x144 = 0.0 - value`, kind at `+0`,
/// `+0x148 = +0x154 = [0x475D10]`, `+0x150 = [0x475D00]`, `+0x158 = 1`, `+0x168 = id`. Players 0..3 only. MEASURED
/// (text copy stubbed).
pub fn hud_post(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    kind: u32,
    player: u32,
    text: u32,
    id: u32,
    value: u32,
) {
    if (player as i32) < 0 || (player as i32) >= 4 {
        return;
    }
    let record = player
        .wrapping_mul(HUD_RECORD_STRIDE)
        .wrapping_add(HUD_RECORD);
    let head = memory.read_u32(record.wrapping_add(0x30));
    let entry = head
        .wrapping_mul(HUD_ENTRY_STRIDE)
        .wrapping_add(record)
        .wrapping_add(0x38);
    let next = head.wrapping_add(1);
    memory.write_u32(
        record.wrapping_add(0x30),
        if (next as i32) >= 0x10 { 0 } else { next },
    );
    effects.call(memory, h::STRNCPY, &[entry.wrapping_add(4), text, 0x140]);
    let negated = 0.0f32 - f32::from_bits(value);
    let base = memory.read_u32(0x47_5D10);
    let side = memory.read_u32(0x47_5D00);
    memory.write_u32(entry.wrapping_add(0x144), negated.to_bits());
    memory.write_u32(entry, kind);
    memory.write_u32(entry.wrapping_add(0x148), base);
    memory.write_u32(entry.wrapping_add(0x150), side);
    memory.write_u32(entry.wrapping_add(0x154), base);
    memory.write_u32(entry.wrapping_add(0x158), 1);
    memory.write_u32(entry.wrapping_add(0x168), id);
}

// ---------------------------------------------------------------------------------------------------------
// 0x67450 / 0xB9D00
// ---------------------------------------------------------------------------------------------------------

/// `0x67450(flag)` (cdecl, void): phase switch of a finished fight. A client (`0x355F10`) with `0x357690() == 0`
/// does nothing. With `flag` set and mode byte `[0x7DE455]` not 3 or 7 it sets `[0x6F11A4] = 1` and calls `0x66E90`.
/// Unless `[0x7DE458] & 0x8000040`, tail calls `0x66FC0(flag)`; otherwise forces voice mode `[0x7DE30C] = 8` (clearing
/// `[0x7DE300]` to 0.0 when it changed) and tail calls `0x231A20(flag)`. MEASURED.
pub fn set_flags(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, flag: u32) {
    if is_client(memory) != 0 && host_stat(memory, effects) == 0 {
        return;
    }
    if flag != 0 {
        let mode = memory.read_u8(GLOBAL_MODE_A);
        if mode != 3 && mode != 7 {
            memory.write_u32(0x6F_11A4, 1);
            effects.call(memory, h::HOST_PHASE_A, &[]);
        }
    }
    let flags = memory.read_u32(GLOBAL_FLAGS_C);
    if flags & 0x40 == 0 && flags & 0x800_0000 == 0 {
        effects.call(memory, h::HOST_PHASE_B, &[flag]);
        return;
    }
    if memory.read_u32(0x7D_E30C) != 8 {
        memory.write_u32(0x7D_E30C, 8);
        memory.write_u32(0x7D_E300, 0);
    }
    effects.call(memory, h::HOST_PHASE_C, &[flag]);
}

/// `0xB9D00(ctx)` (cdecl, void): death hook of the local player context. For each local channel posts the HUD message
/// kind 4 (text `0x1643` when `[[ctx+0xBF0]+0x28] == 0x50415250`, else `0x1644`, value 1.6) and, for player records in
/// range with a controller, shows the icon `0xFD7E0(0xF8AD0([[record]+0x214]))`; then sets bit `0x10000` of
/// `[0x7DE45C]` and continues with `set_flags(0)`. MEASURED (icon routines stubbed, HUD and set_flags run).
pub fn death_hook(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, ctx: u32) {
    let mut channel = 0u32;
    let mut offset = 0u32;
    if (channel_count(memory) as i32) > 0 {
        loop {
            let controller = memory.read_u32(ctx.wrapping_add(0xBF0));
            let id = if memory.read_u32(controller.wrapping_add(0x28)) == 0x5041_5250 {
                0x1643
            } else {
                0x1644
            };
            let text = string_lookup(memory, id);
            hud_message(memory, effects, channel, 4, text, 0x3FCC_CCCD);
            if (offset as i32) >= 0 && (offset as i32) < 0x5610 {
                let players = memory.read_u32(GLOBAL_PLAYERS);
                if players != 0 {
                    let record = memory.read_u32(offset.wrapping_add(players).wrapping_add(8));
                    if record != 0 {
                        let icon = memory.read_u32(record.wrapping_add(0x214));
                        let handle = effects.call(memory, h::HUD_ICON, &[icon]);
                        effects.call(memory, h::HUD_ICON_SHOW, &[handle]);
                    }
                }
            }
            channel = channel.wrapping_add(1);
            offset = offset.wrapping_add(GLOBAL_PLAYER_STRIDE);
            if (channel as i32) >= (channel_count(memory) as i32) {
                break;
            }
        }
    }
    let flags = memory.read_u32(0x7D_E45C) | 0x1_0000;
    memory.write_u32(0x7D_E45C, flags);
    set_flags(memory, effects, 0);
}

// ---------------------------------------------------------------------------------------------------------
// 0xD5480 finish slot
// ---------------------------------------------------------------------------------------------------------

/// `0xD5480(block)` (cdecl, void): the destroy epilogue for a victim with a controller `[[block]+0x150]`. With the
/// controller in state 4 (`+0x114`) it marks `ctx.flags |= 8`, calls `0x238E00(killer ctx, owner ctx)` and returns.
/// Otherwise `0x238E00(killer ctx, ctx)` unless flag 8 is already set, a random sound `0x4FD/0x4FE` for killer bodies
/// `0x5E8/0x637`, the camera calls, the kill log, and (mode `0xA`) the follow up `0x2E6D40([ctx+8])`. MEASURED.
pub fn finish_slot(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, block: u32) {
    let victim = memory.read_u32(block);
    let controller = memory.read_u32(victim.wrapping_add(0x150));
    let owner = memory.read_u32(controller.wrapping_add(0x14));
    let ctx = memory.read_u32(owner.wrapping_add(0x7C));
    let killer = memory.read_u32(block.wrapping_add(4));
    let mut killer_ctx = 0;
    if killer != 0 && memory.read_u32(killer.wrapping_add(0x20)) == 8 {
        killer_ctx = memory.read_u32(killer.wrapping_add(0x7C));
    }
    if memory.read_u32(controller.wrapping_add(0x114)) == 4 {
        let flags = memory.read_u32(ctx.wrapping_add(0x24)) | 8;
        memory.write_u32(ctx.wrapping_add(0x24), flags);
        let owner = memory.read_u32(controller.wrapping_add(0x14));
        let owner_ctx = memory.read_u32(owner.wrapping_add(0x7C));
        effects.call(memory, h::PLAYER_RESPAWN, &[killer_ctx, owner_ctx]);
        return;
    }
    if memory.read_u32(ctx.wrapping_add(0x24)) & 8 == 0 {
        effects.call(memory, h::PLAYER_RESPAWN, &[killer_ctx, ctx]);
    }
    let killer = memory.read_u32(block.wrapping_add(4));
    if killer != 0 && memory.read_u32(killer.wrapping_add(0x20)) == 8 {
        let body = read_u16(memory, killer.wrapping_add(0x94));
        if body == 0x5E8 || body == 0x637 {
            let random = effects.call(memory, h::RANDOM_64, &[]);
            let mut value = f64::from(random as i32);
            if (random as i32) < 0 {
                value += f64::from(f32_at(memory, 0x47_5CCC));
            }
            value *= f64::from(f32_at(memory, 0x47_85D0));
            let limit = f64::from(f32_at(memory, 0x47_5CD4));
            let id = if limit > value { 0x4FE } else { 0x4FD };
            effects.call(memory, h::EFFECT_REQUEST, &[id, 0x4000_0000]);
        }
    }
    if memory.read_u32(controller.wrapping_add(0xC)) & 0x1_0000 != 0 {
        effects.call(memory, h::CAMERA_A, &[]);
    } else if memory.read_u32(controller.wrapping_add(0x114)) != 4 {
        effects.call(memory, h::CAMERA_B, &[block]);
    }
    if effects.call(memory, h::PLAYER_STATE, &[]) == 0
        && memory.read_u32(block.wrapping_add(0x18)) == 0x19
    {
        effects.call(memory, h::SPECTATE, &[ctx.wrapping_add(0xC68)]);
    }
    effects.call(memory, h::KILL_STAT, &[block]);
    effects.call(memory, h::KILL_LOG, &[controller, 1]);
    if memory.read_u8(GLOBAL_MODE_A) != 0xA {
        return;
    }
    if (memory.read_u32(GLOBAL_FIRST_SLOT) as i32) > 1 {
        let id = memory.read_u32(ctx.wrapping_add(8));
        let players = memory.read_u32(GLOBAL_PLAYERS);
        let record = if id == 0 {
            players.wrapping_add(GLOBAL_PLAYER_STRIDE)
        } else {
            players
        };
        effects.call(memory, h::KILL_LOG, &[record, 0]);
        if memory.read_u8(GLOBAL_MODE_A) != 0xA {
            return;
        }
    }
    let id = memory.read_u32(ctx.wrapping_add(8));
    effects.call(memory, h::NEXT_PHASE, &[id]);
}

// ---------------------------------------------------------------------------------------------------------
// 0x165010 kill feed
// ---------------------------------------------------------------------------------------------------------

/// Original address of the byte table `(code + 1) -> 0/1` read by `0x165010` (32 entries).
pub const FEED_CODE_TABLE: u32 = 0x16_519C;

/// `0x165010(block)` (cdecl, void): the kill feed lines of a destroy between two player objects (kind 8). Needs the
/// frame counter `[0x7DE318] - [0x74C3F8] >= 8 * [0x4E8CEC]`. Line ids go to the register routine `0x164F60`
/// (`eax = line, edi = line + 1`), flags to `0x164A30(id)`, the skip test is `0xD6C90(sext16([ctx+0x48]))`. The kill code
/// `[block+0x24]` is mapped through the byte table at `0x16519C`. MEASURED (callees stubbed).
pub fn kill_feed(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, block: u32) {
    if memory.read_u32(GLOBAL_FLAGS_C) & 0x40 != 0 {
        return;
    }
    let victim = memory.read_u32(block);
    if victim == 0 || memory.read_u32(victim.wrapping_add(0x20)) != 8 {
        return;
    }
    let victim_ctx = memory.read_u32(victim.wrapping_add(0x7C));
    let killer = memory.read_u32(block.wrapping_add(4));
    if killer == 0 || memory.read_u32(killer.wrapping_add(0x20)) != 8 {
        return;
    }
    let delta = memory
        .read_u32(0x7D_E318)
        .wrapping_sub(memory.read_u32(0x74_C3F8));
    let limit = memory.read_u32(0x4E_8CEC).wrapping_mul(8);
    if (delta as i32) < (limit as i32) {
        return;
    }
    let killer_ctx = memory.read_u32(killer.wrapping_add(0x7C));
    let ready = |memory: &dyn GuestMemory, ctx: u32| -> bool {
        let object = memory.read_u32(ctx);
        let controller = memory.read_u32(object.wrapping_add(0x150));
        controller != 0 && (memory.read_u32(controller.wrapping_add(4)) as i32) >= 0
    };
    let line =
        |memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, eax: u32, edi: u32| {
            effects.call(memory, h::FEED_LINE, &[eax, edi]);
        };
    if ready(memory, victim_ctx) {
        if killer_ctx == victim_ctx {
            line(memory, effects, 0x26, 0x27);
            return;
        }
        line(memory, effects, 0x2F, 0x30);
    }
    if ready(memory, killer_ctx) {
        let code = sext16(memory, victim_ctx.wrapping_add(0x48));
        if effects.call(memory, h::FEED_SKIP, &[code]) != 0 {
            effects.call(memory, h::FEED_FLAG, &[0x35]);
        }
        let index = memory.read_u32(block.wrapping_add(0x24)).wrapping_add(1);
        let check = index > 0x1F || memory.read_u8(FEED_CODE_TABLE.wrapping_add(index)) != 0;
        if check && (3..=4).contains(&(memory.read_u32(block.wrapping_add(0x44)) as i32)) {
            line(memory, effects, 0x1F, 0x22);
        }
    }
    if !(ready(memory, killer_ctx) || ready(memory, victim_ctx)) {
        return;
    }
    if (0..=1).contains(&(memory.read_u32(block.wrapping_add(0x44)) as i32)) {
        line(memory, effects, 0x31, 0x33);
    }
    let kind = memory.read_u32(block.wrapping_add(0x14));
    if kind == 0 {
        effects.call(memory, h::FEED_FLAG, &[0x23]);
    } else if kind == 0x21 {
        effects.call(memory, h::FEED_FLAG, &[0x28]);
    }
    if memory.read_u32(block.wrapping_add(0x24)) == 0x15 {
        line(memory, effects, 0x2C, 0x2D);
    }
    if memory.read_u32(block.wrapping_add(0x3C)) & 1 != 0 {
        effects.call(memory, h::FEED_FLAG, &[0x29]);
    }
}

// ---------------------------------------------------------------------------------------------------------
// 0x644D0 score update
// ---------------------------------------------------------------------------------------------------------

/// `0x644D0(record, index, value)` (cdecl): per player score update for the player `index` (slot record
/// `[0x7356D8] + 0xEA0 * index`), killer record `record` may be 0. Returns `Some(address)` only on the path that adds
/// to the i16 counter table of the local player record `[[0x7DE308]+0x7C]+0xA10` (eax is undefined elsewhere).
/// Modes `[0x7DE456]` 0x12/0x13 gate early exits, voice modes 6 and 7 count `+0xA52`, decrement `+0xA60` and add
/// `value` to `record`'s counter; in match mode 9 the team score routine `0x641A0(stack record+0x84, ecx = +-value)`
/// is called. MEASURED (callees stubbed).
pub fn score_update(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    record: u32,
    index: u32,
    value: u32,
) -> Option<u32> {
    let mut mode = memory.read_u8(GLOBAL_MODE_B);
    let slot_of = |memory: &dyn GuestMemory| {
        memory
            .read_u32(GLOBAL_SLOT_BASE)
            .wrapping_add(index.wrapping_mul(GLOBAL_SLOT_STRIDE))
    };
    let mut gate = true;
    if record != 0 && mode == 0x12 {
        if memory.read_u32(record) != memory.read_u32(0x7D_E320) {
            return None;
        }
        gate = false;
    }
    if gate && mode == 0x13 {
        let slot = slot_of(memory);
        let first = memory.read_u32(GLOBAL_FIRST_SLOT);
        if (memory.read_u32(slot.wrapping_add(8)) as i32) >= (first as i32) {
            let code = sext16(memory, slot.wrapping_add(0x48));
            if effects.call(memory, h::FEED_SKIP, &[code]) != 0 {
                return None;
            }
            mode = memory.read_u8(GLOBAL_MODE_B);
        }
    }
    if record != 0 && mode == 0x13 {
        let first = memory.read_u32(GLOBAL_FIRST_SLOT);
        if (memory.read_u32(record.wrapping_add(8)) as i32) >= (first as i32) {
            let code = sext16(memory, record.wrapping_add(0x48));
            if effects.call(memory, h::FEED_SKIP, &[code]) != 0 {
                let local = memory.read_u32(0x7D_E308);
                let table = memory.read_u32(local.wrapping_add(0x7C));
                let address = table
                    .wrapping_add(index.wrapping_mul(2))
                    .wrapping_add(0xA10);
                let sum = read_u16(memory, address).wrapping_add(value as u16);
                write_u16(memory, address, sum);
                return Some(address);
            }
        }
    }
    let voice = memory.read_u32(0x7D_E30C);
    if voice != 7 && voice != 6 {
        return None;
    }
    let slot = slot_of(memory);
    let counter = read_u16(memory, slot.wrapping_add(0xA52)).wrapping_add(1);
    write_u16(memory, slot.wrapping_add(0xA52), counter);
    if memory.read_u32(0x7D_E474) != 0 {
        let left = read_u16(memory, slot.wrapping_add(0xA60));
        if (left as i16) > 0 {
            write_u16(memory, slot.wrapping_add(0xA60), left - 1);
        }
    }
    if record != 0 {
        let address = record
            .wrapping_add(index.wrapping_mul(2))
            .wrapping_add(0xA10);
        let sum = read_u16(memory, address).wrapping_add(value as u16);
        write_u16(memory, address, sum);
    }
    if memory.read_u8(GLOBAL_MODE_A) != 9
        || memory.read_u32(0x7D_E474) != 0
        || memory.read_u8(GLOBAL_MODE_B) == 0xA
    {
        return None;
    }
    let flags = memory.read_u8(GLOBAL_FLAGS_C);
    let negated = value.wrapping_neg();
    if flags & 0x10 != 0 {
        let team = memory.read_u32(slot.wrapping_add(0x84));
        effects.call(memory, h::SCORE_RECENT, &[team, negated]);
    }
    if record == slot {
        if flags & 0x20 == 0 {
            return None;
        }
        let team = memory.read_u32(record.wrapping_add(0x84));
        effects.call(memory, h::SCORE_RECENT, &[team, negated]);
        return None;
    }
    if record == 0 {
        return None;
    }
    let team = memory.read_u32(record.wrapping_add(0x84));
    if team == memory.read_u32(slot.wrapping_add(0x84)) {
        return None;
    }
    effects.call(memory, h::SCORE_RECENT, &[team, value]);
    None
}

// ---------------------------------------------------------------------------------------------------------
// 0x12DFB0 context update
// ---------------------------------------------------------------------------------------------------------

/// `0x12DFB0(ctx)` (cdecl, void): in match mode 9 (not mode `0xA`) with a positive team target `[0x7DE464]`, announces
/// the last point of a score race. The remaining points are the target minus the team score (`0x64160` of
/// `[ctx+0x84]`, the peer score `0x641C0` for a server, or the slot record field `+0xA6C`); exactly 1 posts the HUD
/// message kind 3 to every channel (text from `0x151030(0x47DA50, ...)` with the string `0x1907`/`0x1908`) and submits the
/// effect record `0xD4A` through `0x45390`/`0x45820`. `entry` = esp at entry; the effect record is `entry - 0x30`. MEASURED.
pub fn announce_last_point(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    ctx: u32,
    entry: u32,
) {
    if memory.read_u8(GLOBAL_MODE_A) != 9 || memory.read_u8(GLOBAL_MODE_B) == 0xA {
        return;
    }
    let target = memory.read_u32(0x7D_E464);
    if (target as i32) <= 0 {
        return;
    }
    let flags = memory.read_u32(GLOBAL_FLAGS_C);
    let remaining = if flags & 1 != 0 {
        let team = memory.read_u32(ctx.wrapping_add(0x84));
        target.wrapping_sub(effects.call(memory, h::SCORE_TEAM, &[team]))
    } else if is_server(memory) != 0 {
        let peer = memory.read_u32(ctx.wrapping_add(8));
        target.wrapping_sub(effects.call(memory, h::SCORE_PEER, &[peer]))
    } else {
        let index = memory.read_u32(ctx.wrapping_add(8));
        let slot = memory
            .read_u32(GLOBAL_SLOT_BASE)
            .wrapping_add(index.wrapping_mul(GLOBAL_SLOT_STRIDE));
        target.wrapping_sub(memory.read_u32(slot.wrapping_add(0xA6C)))
    };
    if memory.read_u8(GLOBAL_MODE_B) == 0x12 && memory.read_u32(ctx) != memory.read_u32(0x7D_E320) {
        return;
    }
    if remaining != 1 {
        return;
    }
    let text = if memory.read_u32(GLOBAL_FLAGS_C) & remaining != 0 {
        let first = string_lookup(memory, 0x1907);
        let team = memory.read_u32(ctx.wrapping_add(0x84));
        let second = string_lookup(memory, team.wrapping_add(0xB2B));
        effects.call(memory, h::FORMAT_TEXT, &[0x47_DA50, second, first])
    } else {
        let first = string_lookup(memory, 0x1908);
        let name = memory.read_u32(ctx.wrapping_add(0x70));
        effects.call(memory, h::FORMAT_TEXT, &[0x47_DA50, name, first])
    };
    let mut channel = 0u32;
    if (channel_count(memory) as i32) > 0 {
        loop {
            hud_message(memory, effects, channel, 3, text, 0);
            channel = channel.wrapping_add(1);
            if (channel as i32) >= (channel_count(memory) as i32) {
                break;
            }
        }
    }
    let record = entry.wrapping_sub(0x30);
    effects.call(memory, h::SUBMIT_FILL, &[record]);
    memory.write_u32(record, 0xD4A);
    let mut args = vec![record];
    args.extend((0..12).map(|k| memory.read_u32(record.wrapping_add(4 * k))));
    effects.call(memory, h::SUBMIT, &args);
}

// ---------------------------------------------------------------------------------------------------------
// 0x1252C0 blend
// ---------------------------------------------------------------------------------------------------------

/// `0x1252C0(blend, factor, killer)` (cdecl, void): starts the three blend channels (`blend + 0xBC/0xC0/0xC4`) of a
/// destroy record through `0x378530` (11 stack arguments, the vector `(0, 1, 0)` of the frame is passed by pointer
/// and recorded as three dwords after the arguments), or stops them (`0x10E1C0`) in mode `[0x7DE456] == 0xC` outside
/// level `0x65`. Gate: `object.flags & 2` with `0x350E70(rec, 2) == 0`, mode 0xC or level 0x65, bit 4 of `blend+4` clear
/// and `0x35C4E0(slot, factor, killer) != 0`. Writes the frame locals (`entry - 0x14 .. entry - 4`, and the float result
/// into the caller's first argument slot `entry + 4` on level 0x65). MEASURED (callees stubbed).
pub fn start_blend(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    blend: u32,
    factor: u32,
    killer: u32,
    entry: u32,
) {
    let one = memory.read_u32(0x47_5C78);
    let (l0c, l08, l04, l10, l14) = (
        entry.wrapping_sub(0xC),
        entry.wrapping_sub(8),
        entry.wrapping_sub(4),
        entry.wrapping_sub(0x10),
        entry.wrapping_sub(0x14),
    );
    let record = memory.read_u32(blend);
    let object = memory.read_u32(record);
    memory.write_u32(l0c, 0);
    memory.write_u32(l08, one);
    memory.write_u32(l04, 0);
    memory.write_u32(l10, 3);
    memory.write_u32(l14, one);
    let mut pass = memory.read_u32(object.wrapping_add(0x9C)) & 2 == 0;
    if !pass {
        pass = effects.call(memory, h::CLEAR_STATE, &[record, 2]) != 0;
    }
    if !pass {
        // 0x125328: the gate by mode and level is skipped
    } else if memory.read_u8(GLOBAL_MODE_B) != 0xC && memory.read_u32(GLOBAL_LEVEL) != 0x65 {
        return;
    }
    if memory.read_u32(blend.wrapping_add(4)) & 4 != 0 {
        return;
    }
    let slot_index = memory.read_u32(record.wrapping_add(8));
    if effects.call(memory, h::BLEND_GATE, &[slot_index, factor, killer]) == 0 {
        return;
    }
    let flags = memory.read_u32(blend.wrapping_add(4)) | 4;
    let strength = memory.read_u32(0x47_5D10);
    memory.write_u32(blend.wrapping_add(0xB4), strength);
    memory.write_u32(blend.wrapping_add(0xB8), killer);
    memory.write_u32(blend.wrapping_add(4), flags);
    let mode_c = memory.read_u8(GLOBAL_MODE_B) == 0xC;
    if mode_c {
        let mask = memory.read_u32(0x7A_2BC8);
        let saved_mask = mask;
        let bit = 1u32 << (slot_index & 31);
        memory.write_u32(l10, 0xC0);
        memory.write_u32(0x7A_2BC8, mask | bit);
        effects.call(memory, h::CHANNEL_MASK, &[object, 0xB_C140]);
        let shared = memory.read_u32(0x4D_1E50);
        memory.write_u32(record.wrapping_add(0xA58), shared);
        let first = memory.read_u32(GLOBAL_FIRST_SLOT);
        if (slot_index as i32) >= (first as i32)
            && effects.call(memory, h::ACTION_TEST, &[record, 0x401, 0]) == 0
        {
            effects.call(memory, h::ACTION_SET, &[record, 0x2000, 0]);
            effects.call(
                memory,
                h::SOUND_3D,
                &[object, 0x36, 0x3EA8_F5C3, 0x3F80_0000],
            );
        }
        let current = memory.read_u32(0x7A_2BC8);
        memory.write_u32(0x7A_36C0, 0);
        if current == 0 {
            memory.write_u32(0x7A_2BC8, saved_mask);
        }
    }
    effects.call(memory, h::BLEND_MIRROR, &[blend]);
    let mut speed = one_or_alt(memory, object, l14);
    let level = memory.read_u32(GLOBAL_LEVEL);
    let position = object.wrapping_add(0x64);
    let owner_tag = memory.read_u32(0x73_ED08);
    let vector = l0c;
    let tail = |memory: &mut dyn GuestMemory| -> Vec<u32> {
        (0..3)
            .map(|k| memory.read_u32(vector.wrapping_add(4 * k)))
            .collect()
    };
    let call = |memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, args: [u32; 11]| {
        let mut all = args.to_vec();
        all.extend(tail(memory));
        effects.call(memory, h::BLEND_START, &all);
    };
    let scale_a = f32_at(memory, 0x47_5D18);
    let scale_b = f32_at(memory, 0x47_5D30);
    if level == 0x65 {
        speed = (f32::from_bits(speed) * scale_a).to_bits();
        call(
            memory,
            effects,
            [
                blend.wrapping_add(0xBC),
                0x10,
                position,
                vector,
                0,
                0x1200_0000,
                owner_tag,
                speed,
                0,
                0,
                object,
            ],
        );
        let wide = (f64::from(f32_at(memory, l14)) * f64::from(scale_b)) as f32;
        memory.write_u32(entry.wrapping_add(4), wide.to_bits());
        let bits = wide.to_bits();
        call(
            memory,
            effects,
            [
                blend.wrapping_add(0xC0),
                4,
                position,
                vector,
                0,
                0x1000_0000,
                owner_tag,
                bits,
                0,
                0,
                object,
            ],
        );
        let channel = memory.read_u32(l10);
        call(
            memory,
            effects,
            [
                blend.wrapping_add(0xC4),
                channel,
                position,
                vector,
                0,
                0x1000_0000,
                owner_tag,
                bits,
                0,
                0,
                object,
            ],
        );
        return;
    }
    if !mode_c {
        speed = (f32::from_bits(speed) * scale_a).to_bits();
        call(
            memory,
            effects,
            [
                blend.wrapping_add(0xBC),
                0x10,
                position,
                vector,
                0,
                0x1202_0000,
                owner_tag,
                speed,
                0x3E99_999A,
                0x3E99_999A,
                object,
            ],
        );
        let second = (f32_at(memory, l14) * scale_b).to_bits();
        call(
            memory,
            effects,
            [
                blend.wrapping_add(0xC0),
                4,
                position,
                vector,
                0,
                0x1002_0000,
                owner_tag,
                second,
                0x3E80_0000,
                0x3E80_0000,
                object,
            ],
        );
    } else {
        effects.call(memory, h::BLEND_STOP, &[blend.wrapping_add(0xBC)]);
        effects.call(memory, h::BLEND_STOP, &[blend.wrapping_add(0xC0)]);
    }
    let last = (f32_at(memory, l14) * scale_b).to_bits();
    let channel = memory.read_u32(l10);
    call(
        memory,
        effects,
        [
            blend.wrapping_add(0xC4),
            channel,
            position,
            vector,
            0,
            0x1002_0000,
            owner_tag,
            last,
            0x3E99_999A,
            0x3E99_999A,
            object,
        ],
    );
}

/// The speed value of [`start_blend`]: `[0x475D24]` (also stored at the frame slot `l14`) when the object has a
/// controller `+0x150`, else the frame slot value.
fn one_or_alt(memory: &mut dyn GuestMemory, object: u32, l14: u32) -> u32 {
    if memory.read_u32(object.wrapping_add(0x150)) != 0 {
        let value = memory.read_u32(0x47_5D24);
        memory.write_u32(l14, value);
        value
    } else {
        memory.read_u32(l14)
    }
}

// ---------------------------------------------------------------------------------------------------------
// 0x15DB20 free object
// ---------------------------------------------------------------------------------------------------------

/// `0x15DB20(block)` (cdecl, void): the destroy tail for a victim object (`[block]`): pre-hook `0x15C310(victim, 2,
/// block, 0)` (-1 aborts), clean-up, respawn bookkeeping `0x238E00(killer ctx, ctx)`, copies the hit vector
/// `[block+0x30]` to `ctx+0xB54..0xB5C`, limited objects (`0x59D20(victim, 0xAF4)`) are dropped when their state
/// (`0x585E0`, `0x613D0`) is below the limit, then `0xD7D10`, `0x17570`, the ctx flags (`+0x24 = (v & ~0x40) | 8`) and
/// `0x59E00`. A hit vector is flattened (`y = 0`), normalised through `rsqrtss` (single precision `1 / sqrt`, as the
/// emulator computes it) when its squared length exceeds `[0x4765F4]`, and both hit vectors are copied to the globals
/// `0x74C350..0x74C364`. Finally the effect pair `0x11ECE0`/`0x11E7B0` for body `0x60`/other in mode `0xA`, and the post
/// hook `0x15C310(victim, 3, block, 0)`. Frame effects: the caller's argument slot `entry + 4` holds the context, then
/// `1.0` in the normalisation branch; the state local is `entry - 8`, `entry - 4` the reciprocal. MEASURED (callees stubbed).
pub fn free_object(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    block: u32,
    entry: u32,
) {
    use super::svc_objects::{class_of, query_state};
    let victim = memory.read_u32(block);
    let ctx = memory.read_u32(victim.wrapping_add(0x7C));
    let slot = entry.wrapping_add(4);
    memory.write_u32(slot, ctx);
    if effects.call(memory, h::FREE_PRE, &[victim, 2, block, 0]) == 0xFFFF_FFFF {
        return;
    }
    effects.call(memory, h::FREE_CLEANUP, &[victim]);
    if memory.read_u8(GLOBAL_MODE_B) == 0x12 {
        effects.call(memory, h::FREE_NOTIFY, &[block]);
    }
    let mut killer_ctx = 0;
    let killer = memory.read_u32(block.wrapping_add(4));
    if killer != 0 && memory.read_u32(killer.wrapping_add(0x20)) == 8 {
        killer_ctx = memory.read_u32(killer.wrapping_add(0x7C));
    }
    let model = memory.read_u32(victim.wrapping_add(4));
    effects.call(memory, h::FREE_MODEL, &[model]);
    effects.call(memory, h::PLAYER_RESPAWN, &[killer_ctx, ctx]);
    let hit = memory.read_u32(block.wrapping_add(0x30));
    if hit != 0 {
        for k in 0..3u32 {
            let value = memory.read_u32(hit.wrapping_add(4 * k));
            memory.write_u32(ctx.wrapping_add(0xB54 + 4 * k), value);
        }
    }
    let mut ctx_local = ctx;
    if effects.call(memory, h::FREE_LIMITED, &[victim, 0xAF4]) != 0 {
        let class = class_of(memory, victim);
        let id = memory.read_u32(class);
        let limit = effects.call(memory, h::FREE_SCALE, &[id, 0x200, 1]);
        query_state(memory, class, entry.wrapping_sub(8), 0);
        if (memory.read_u32(entry.wrapping_sub(8)) as i32) < (limit as i32) {
            effects.call(memory, h::FREE_DROP, &[victim, 1, 0]);
        }
        ctx_local = memory.read_u32(slot);
    }
    effects.call(memory, h::FREE_LINK, &[victim, block]);
    effects.call(memory, h::ACTION_SET, &[ctx_local, 1, 0]);
    let owner = memory.read_u32(victim.wrapping_add(0x7C));
    let flags = (memory.read_u32(owner.wrapping_add(0x24)) & 0xFFFF_FFBF) | 8;
    memory.write_u32(owner.wrapping_add(0x24), flags);
    if effects.call(memory, h::FREE_STATE, &[victim, 0, 0x8000]) == 0 {
        effects.call(memory, h::FREE_FALLBACK, &[ctx_local]);
    }
    let hit = memory.read_u32(block.wrapping_add(0x30));
    if hit != 0 {
        memory.write_u32(hit.wrapping_add(4), 0);
        let (x, y, z) = (
            f32_at(memory, hit),
            f32_at(memory, hit.wrapping_add(4)),
            f32_at(memory, hit.wrapping_add(8)),
        );
        let length = (x * x + y * y) + z * z;
        if length > f32_at(memory, 0x47_65F4) {
            memory.write_u32(entry.wrapping_sub(8), length.to_bits());
            let one = memory.read_u32(0x47_5C78);
            memory.write_u32(slot, one);
            let reciprocal = (1.0f32 / length.sqrt()) * f32::from_bits(one);
            memory.write_u32(entry.wrapping_sub(4), reciprocal.to_bits());
            for k in 0..3u32 {
                let at = memory
                    .read_u32(block.wrapping_add(0x30))
                    .wrapping_add(4 * k);
                let scaled = f32_at(memory, at) * reciprocal;
                memory.write_u32(at, scaled.to_bits());
            }
        }
        for k in 0..3u32 {
            let value = memory.read_u32(
                memory
                    .read_u32(block.wrapping_add(0x30))
                    .wrapping_add(4 * k),
            );
            memory.write_u32(0x74_C350 + 4 * k, value);
        }
    }
    let second = memory.read_u32(block.wrapping_add(0x28));
    if second != 0 {
        for k in 0..3u32 {
            let value = memory.read_u32(second.wrapping_add(4 * k));
            memory.write_u32(0x74_C35C + 4 * k, value);
        }
    }
    let code = sext16(memory, ctx_local.wrapping_add(0x48));
    if effects.call(memory, h::FREE_FX_TEST, &[code]) != 0 && memory.read_u8(GLOBAL_MODE_A) == 0xA {
        let routine = if read_u16(memory, ctx_local.wrapping_add(0x48)) == 0x60 {
            h::FREE_FX_A
        } else {
            h::FREE_FX_B
        };
        effects.call(memory, routine, &[victim, 0x4040_0000, 0]);
    }
    effects.call(memory, h::FREE_PRE, &[victim, 3, block, 0]);
}

// ---------------------------------------------------------------------------------------------------------
// 0x2B24E0 final hook
// ---------------------------------------------------------------------------------------------------------

/// `0x2B24E0(object)` (cdecl, void): while the round counter `[0x75F6E8]` is below 30 runs `0x2B23D0(object,
/// [ctx+0x678])`, then for every remote slot (from `[0x790950]` to `[0x74C37C]`) whose peer test `0x15BD00(ctx.id, slot)`
/// holds twice and whose slot record `[0x7356D8] + 0xEA0 * slot` has no flag 8 in `+0x24`, stores the round value
/// `[0x75F66C + 4 * round]` at `+0xAF0` and the observer at `+0x684`, clears `+0x9A8/+0x9AC` and calls
/// `0x17570(record, 0x20000, 0)`. MEASURED (callees stubbed).
pub fn final_hook(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, object: u32) {
    let ctx = memory.read_u32(object.wrapping_add(0x7C));
    if (memory.read_u32(0x75_F6E8) as i32) >= 0x1E {
        return;
    }
    let observer = memory.read_u32(ctx.wrapping_add(0x678));
    effects.call(memory, h::FINAL_STATE, &[object, observer]);
    let mut slot = memory.read_u32(GLOBAL_FIRST_SLOT);
    if (slot as i32) >= (memory.read_u32(0x74_C37C) as i32) {
        return;
    }
    let mut offset = slot.wrapping_mul(GLOBAL_SLOT_STRIDE);
    loop {
        let id = memory.read_u32(ctx.wrapping_add(8));
        'body: {
            if id == slot {
                break 'body;
            }
            let peer = effects.call(memory, h::FINAL_PEER, &[id, slot]);
            if memory.read_u8(peer) == 0 {
                break 'body;
            }
            let id = memory.read_u32(ctx.wrapping_add(8));
            if id == slot {
                break 'body;
            }
            let peer = effects.call(memory, h::FINAL_PEER, &[id, slot]);
            if memory.read_u8(peer) == 0 {
                break 'body;
            }
            let record = memory.read_u32(GLOBAL_SLOT_BASE).wrapping_add(offset);
            if memory.read_u8(record.wrapping_add(0x24)) & 8 != 0 {
                break 'body;
            }
            let round = memory.read_u32(0x75_F6E8);
            let value = memory.read_u32(round.wrapping_mul(4).wrapping_add(0x75_F66C));
            memory.write_u32(record.wrapping_add(0xAF0), value);
            let observer = memory.read_u32(ctx.wrapping_add(0x678));
            if (observer as i32) < 0 {
                let players = memory.read_u32(GLOBAL_PLAYERS);
                let head = memory.read_u32(players.wrapping_add(0x14));
                let head_ctx = memory.read_u32(head.wrapping_add(0x7C));
                let value = memory.read_u32(head_ctx.wrapping_add(0x678));
                memory.write_u32(record.wrapping_add(0x684), value);
            } else {
                memory.write_u32(record.wrapping_add(0x684), observer);
            }
            memory.write_u32(record.wrapping_add(0x9AC), 0);
            memory.write_u32(record.wrapping_add(0x9A8), 0);
            effects.call(memory, h::ACTION_SET, &[record, 0x2_0000, 0]);
        }
        slot = slot.wrapping_add(1);
        offset = offset.wrapping_add(GLOBAL_SLOT_STRIDE);
        if (slot as i32) >= (memory.read_u32(0x74_C37C) as i32) {
            break;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// 0x3C85D4 sprintf
// ---------------------------------------------------------------------------------------------------------

/// `0x3C85D4(dest, format, ...)` (cdecl, C runtime `sprintf`): builds the stream record `{ptr = dest, count =
/// 0x7FFFFFFF, base = dest, flag = 0x42}` in its frame (`entry - 0x24`), runs the printf engine `0x3CB59D(stream,
/// format, args)` (`args = entry + 0xC`, the C runtime formatter, kept behind the effect trait), and terminates the
/// string with a NUL (through the flush `0x3CB25F(0, stream)` when the count went negative). Returns the engine result
/// (the number of characters). MEASURED (engine stubbed).
pub fn format_text(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    dest: u32,
    format: u32,
    entry: u32,
) -> u32 {
    let stream = entry.wrapping_sub(0x24);
    memory.write_u32(stream.wrapping_add(4), 0x7FFF_FFFF);
    memory.write_u32(stream.wrapping_add(0xC), 0x42);
    memory.write_u32(stream.wrapping_add(8), dest);
    memory.write_u32(stream, dest);
    let result = effects.call(
        memory,
        h::CRT_OUTPUT,
        &[stream, format, entry.wrapping_add(0xC)],
    );
    if dest != 0 {
        let count = memory.read_u32(stream.wrapping_add(4)).wrapping_sub(1);
        memory.write_u32(stream.wrapping_add(4), count);
        if (count as i32) < 0 {
            effects.call(memory, h::CRT_FLUSH, &[0, stream]);
        } else {
            let at = memory.read_u32(stream);
            memory.write_u8(at, 0);
        }
    }
    result
}

// ---------------------------------------------------------------------------------------------------------
// dispatcher: the 22 callees of 0xBABA0 by address
// ---------------------------------------------------------------------------------------------------------

/// [`DestroyEffects`] that executes the ported callees of `0xBABA0` ([`super::svc_objects::callee`]) and forwards every
/// other routine (the engine systems below them) to `inner`. `entry` is the esp at entry of `0xBABA0`
/// (`destroy_core`'s `entry`): the callees with a stack frame (`0x35CC10`, `0x3C85D4`, `0x12DFB0`, `0x1252C0`,
/// `0x15DB20`) see `entry - 0x118 / 0x120 / 0x118 / 0x120 / 0x114` (MEASURED from the `esp` at their call sites in
/// `0xBABA0`). The nested calls of the ported routines go back through this dispatcher.
pub struct PortedDestroy<'a> {
    pub inner: &'a mut dyn DestroyEffects,
    pub entry: u32,
}

impl DestroyEffects for PortedDestroy<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        use super::svc_destroy::{
            add_voice_value, notify_block, notify_ctx, notify_link, notify_object, set_score,
        };
        use super::svc_objects::callee as c;
        let arg = |index: usize| args.get(index).copied().unwrap_or(0);
        let entry = self.entry;
        let below = move |amount: u32| entry.wrapping_sub(amount);
        match address {
            c::FINISH_SLOT => finish_slot(memory, self, arg(0)),
            c::OWNER_TEST => return owner_test(memory, self, arg(0), arg(1), below(0x118)),
            c::IS_NETWORK => return is_network(memory),
            c::NOTIFY_CTX => notify_ctx(memory, arg(0)),
            c::NOTIFY_BLOCK => notify_block(memory, self, arg(0)),
            c::KILL_TEST => return kill_test(memory, arg(0)),
            c::STRING_LOOKUP => return string_lookup(memory, arg(0)),
            c::FORMAT => return format_text(memory, self, arg(0), arg(1), below(0x120)),
            c::COUNT_CHANNELS => return channel_count(memory),
            c::START_CHANNEL => return hud_message(memory, self, arg(0), arg(1), arg(2), arg(3)),
            c::SET_FLAGS => set_flags(memory, self, arg(0)),
            c::DEATH_HOOK => death_hook(memory, self, arg(0)),
            c::RELEASE_LINK => kill_feed(memory, self, arg(0)),
            c::REGION_UPDATE => {
                return score_update(memory, self, arg(0), arg(1), arg(2)).unwrap_or(0);
            }
            c::PLAY_SOUND => add_voice_value(memory, arg(0), arg(1), arg(2)),
            c::ADD_SCORE => set_score(memory, arg(0), arg(1)),
            c::NOTIFY_LINK => notify_link(memory, self, arg(0), arg(1)),
            c::NOTIFY_OBJECT => notify_object(memory, self, arg(0), arg(1)),
            c::UPDATE_CTX => announce_last_point(memory, self, arg(0), below(0x118)),
            c::BLEND => start_blend(memory, self, arg(0), arg(1), arg(2), below(0x120)),
            c::FREE_OBJECT => free_object(memory, self, arg(0), below(0x114)),
            c::FINAL_HOOK => final_hook(memory, self, arg(0)),
            h::TEXT_POST_A => hud_post(memory, self, 0xA, arg(0), arg(1), arg(2), arg(3)),
            h::TEXT_POST_B => hud_post(memory, self, 0xC, arg(0), arg(1), arg(2), arg(3)),
            _ => return self.inner.call(memory, address, args),
        }
        0
    }
}
