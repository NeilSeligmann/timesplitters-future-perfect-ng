// SPDX-License-Identifier: GPL-3.0-or-later
//! T1848 wave 4 input (pad) leaves, ports of call-free C replacements of `src/game`. Stack
//! arguments are passed as parameters, `entry` supplies registers a root leaves untouched. Reads
//! and writes keep the C order. See docs/t1848-rust-wave4.md.
use crate::{gprs::Gprs, memory::GuestMemory};

const PAD_LOCKED: u32 = 0x006b_9888;
const ALT_MAP_FLAG: u32 = 0x0076_b0fc;
const PAD_MAP: u32 = 0x006b_93c0;
const PAD_STATE: u32 = 0x006b_93e0;
const ALT_STATE: u32 = 0x006b_94d8;
const PAD_MASK16: u32 = 0x007e_7240;
const INPUT_OBJECT: u32 = 0x0075_eaf8;
const PLAYER_BOUND: u32 = 0x0052_36c4;
const PLAYER_MAP: u32 = 0x0074_e914;
const REC_PRIMARY_BASE: u32 = 0x006b_93d0;
const REC_ALT_BASE: u32 = 0x006b_94c8;
const REC_STRIDE: u32 = 0x3c;

/// 000427B0 game_input_pad_current_button_state_by_pad_index_zero_when_6b9888_set. cdecl(1).
pub fn pad_current_button_state<M: GuestMemory>(m: &M, pad: u32, entry: Gprs) -> Gprs {
    if m.read32(PAD_LOCKED) != 0 {
        return Gprs { eax: 0, ..entry };
    }
    if m.read32(ALT_MAP_FLAG) != 0 {
        let eax = m.read32(pad.wrapping_mul(REC_STRIDE).wrapping_add(ALT_STATE));
        return Gprs { eax, ..entry };
    }
    let mapped = m.read32(pad.wrapping_mul(4).wrapping_add(PAD_MAP));
    let edx = mapped.wrapping_mul(REC_STRIDE);
    Gprs { eax: m.read32(edx.wrapping_add(PAD_STATE)), ecx: pad, edx }
}

/// 00042C30 game_input_pad_get_field_0x10_direct. cdecl(1): the state word, 0 while locked.
pub fn pad_get_field_0x10_direct<M: GuestMemory>(m: &M, pad: u32, entry: Gprs) -> Gprs {
    if m.read32(PAD_LOCKED) != 0 {
        return Gprs { eax: 0, ..entry };
    }
    let eax = m.read32(pad.wrapping_mul(REC_STRIDE).wrapping_add(PAD_STATE));
    Gprs { eax, ..entry }
}

/// 000431C0 game_input_pad_map_is_bit_set. cdecl(1): bit (5-bit masked) of the 16-bit [7E7240]
/// selected by the pad's mapped index.
pub fn pad_map_is_bit_set<M: GuestMemory>(m: &M, pad: u32) -> Gprs {
    let ecx = m.read32(pad.wrapping_mul(4).wrapping_add(PAD_MAP));
    let edx = u32::from(m.read16(PAD_MASK16));
    Gprs { eax: (1u32.wrapping_shl(ecx & 31) & edx != 0) as u32, ecx, edx }
}

/// 00246D10 game_input_driven_object_75eaf8_is_null. cdecl(0).
pub fn driven_object_is_null<M: GuestMemory>(m: &M, entry: Gprs) -> Gprs {
    let ecx = m.read32(INPUT_OBJECT);
    Gprs { eax: (ecx == 0) as u32, ecx, edx: entry.edx }
}

/// 00190D60 game_count_local_players_with_nonnegative_pad_mapping_from_one_to_player_count.
/// cdecl(0). Counts non-negative map words for indices 1..bound (signed), plus one. The bound
/// is read once and the loop has no iteration cap, as in the original.
pub fn count_local_players_with_pad_mapping<M: GuestMemory>(m: &M, entry: Gprs) -> Gprs {
    let bound = m.read32(PLAYER_BOUND) as i32;
    let (mut eax, mut ecx, mut edx) = (1u32, 1u32, entry.edx);
    while (ecx as i32) < bound {
        edx = ((m.read32(ecx.wrapping_mul(4).wrapping_add(PLAYER_MAP)) as i32) >= 0) as u32;
        eax = eax.wrapping_add(edx);
        ecx = ecx.wrapping_add(1);
    }
    Gprs { eax, ecx, edx }
}

fn record<M: GuestMemory>(m: &M, pad: u32) -> u32 {
    if m.read32(ALT_MAP_FLAG) != 0 {
        pad.wrapping_mul(REC_STRIDE).wrapping_add(REC_ALT_BASE)
    } else {
        m.read32(pad.wrapping_mul(4).wrapping_add(PAD_MAP)).wrapping_mul(REC_STRIDE).wrapping_add(REC_PRIMARY_BASE)
    }
}

/// 00043A90 game_input_export_record. cdecl(3): writes the 16-bit word at record+12 to `dest`,
/// then `count - 2` bytes `record[0x1C + 4*k] & 0xFE`. `count == 2` writes nothing more.
/// `count` of 0 or 1 wraps to about 4 billion iterations in the original and is not modelled.
pub fn export_record<M: GuestMemory>(m: &mut M, pad: u32, dest: u32, count: u32, entry: Gprs) -> Gprs {
    let mut ecx = record(m, pad);
    let word = m.read16(ecx.wrapping_add(12));
    m.write8(dest, word as u8);
    m.write8(dest.wrapping_add(1), (word >> 8) as u8);
    let mut eax = dest.wrapping_add(2);
    let mut edx = count.wrapping_sub(2);
    if edx != 0 {
        let mut left = edx;
        ecx = ecx.wrapping_add(0x1c);
        loop {
            edx = (edx & 0xffff_ff00) | u32::from(m.read8(ecx) & 0xfe);
            m.write8(eax, edx as u8);
            eax = eax.wrapping_add(1);
            ecx = ecx.wrapping_add(4);
            left = left.wrapping_sub(1);
            if left == 0 {
                break;
            }
        }
    }
    let _ = entry;
    Gprs { eax, ecx, edx }
}

/// 00043AF0 game_input_import_record. cdecl(3): loads the 16-bit word at `src` into record
/// +8/+12 (+16 = newly set bits, +20 = newly cleared bits), then expands `count - 2` bytes into
/// the dwords from record+0x1C. Same unbounded-count caveat as [`export_record`].
pub fn import_record<M: GuestMemory>(m: &mut M, pad: u32, src: u32, count: u32) -> Gprs {
    let mut eax = record(m, pad);
    let mut edx = src;
    let mut ecx = u32::from(m.read16(edx));
    let previous = m.read32(eax.wrapping_add(8));
    m.write32(eax.wrapping_add(16), !previous & ecx);
    let word = ecx;
    m.write32(eax.wrapping_add(12), word);
    m.write32(eax.wrapping_add(8), word);
    ecx = count;
    edx = edx.wrapping_add(2);
    ecx = ecx.wrapping_sub(2);
    m.write32(eax.wrapping_add(20), !word & previous);
    if ecx != 0 {
        eax = eax.wrapping_add(0x1c);
        let mut left = ecx;
        loop {
            ecx = u32::from(m.read8(edx));
            m.write32(eax, ecx);
            edx = edx.wrapping_add(1);
            eax = eax.wrapping_add(4);
            left = left.wrapping_sub(1);
            if left == 0 {
                break;
            }
        }
    }
    Gprs { eax, ecx, edx }
}
