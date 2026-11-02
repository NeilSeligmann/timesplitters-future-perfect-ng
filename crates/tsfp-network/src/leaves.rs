// SPDX-License-Identifier: GPL-3.0-or-later
//! T1848 wave 4 network leaves, ports of call-free C replacements of `src/game`. See
//! docs/t1848-rust-wave4.md. Stack arguments are parameters; `entry` supplies registers a root
//! leaves untouched.
use crate::{gprs::Gprs, memory::GuestMemory};

const STATE_A: u32 = 0x0068_ddac;
const STATE_B: u32 = 0x0068_ddb0;
const SLOT_LIMIT: u32 = 0x0076_bbbc;
const SLOT_EXTRA: u32 = 0x0076_bbc4;
const RECORD_STRIDE: u32 = 0x124;
const PLAYER_SLOT_BASE: u32 = 0x0077_4180;
const PLAYER_SLOT_STRIDE: u32 = 0x30c;

/// 000387D0 game_network_state_000387d0. cdecl(0): [68DDB0] when [68DDAC] and it are both
/// nonzero, else -1.
pub fn state_000387d0<M: GuestMemory>(m: &M, entry: Gprs) -> Gprs {
    let first = m.read32(STATE_A);
    if first != 0 {
        let second = m.read32(STATE_B);
        if second != 0 {
            return Gprs { eax: second, ..entry };
        }
    }
    Gprs { eax: u32::MAX, ..entry }
}

/// 003B20F0, 003B2110, 003B2130, 003B2150 game_network_b2_*. thiscall(0), `this` in ECX: a byte
/// flag at `this + flag_offset` (0x34 or 0x35) gated by signed `this + 0x30 >= min` (3 or 5).
/// The flag load replaces only AL, but EAX is then fully overwritten with 0 or 1.
pub fn flag_and_level_at_least<M: GuestMemory>(m: &M, this: u32, flag_offset: u32, min: i32, entry: Gprs) -> Gprs {
    let flag = m.read8(this.wrapping_add(flag_offset));
    let eax = if flag == 0 || (m.read32(this.wrapping_add(0x30)) as i32) < min { 0 } else { 1 };
    Gprs { eax, ..entry }
}

/// 0035E590 game_net_is_slot_limit_reached_76bbbc. cdecl(2).
pub fn is_slot_limit_reached<M: GuestMemory>(m: &M, ignore_extra: u32, limit: u32, entry: Gprs) -> Gprs {
    let used = m.read32(SLOT_LIMIT);
    if used as i32 >= limit as i32 {
        return Gprs { eax: 1, ecx: limit, edx: entry.edx };
    }
    if ignore_extra != 0 {
        return Gprs { eax: 0, ecx: limit, edx: ignore_extra };
    }
    let edx = m.read32(SLOT_EXTRA).wrapping_add(used);
    Gprs { eax: (edx as i32 >= limit as i32) as u32, ecx: limit, edx }
}

/// 00356290 game_net_count_free_record_124_slots_of_16. cdecl(0): 16 records of stride 0x124,
/// the count of flag bit 0 clear is 16 minus the set ones, clamped at 0.
pub fn count_free_record_slots<M: GuestMemory>(m: &M, entry: Gprs) -> Gprs {
    let mut used = 0u32;
    let mut base = 0x0077_73b0u32;
    while base < 0x0077_85f0 {
        for step in [0u32, 1, 2, 3] {
            let address = base.wrapping_add(step.wrapping_mul(RECORD_STRIDE)).wrapping_sub(RECORD_STRIDE);
            if m.read8(address) & 1 != 0 {
                used += 1;
            }
        }
        base = base.wrapping_add(0x490);
    }
    let free = 16u32.wrapping_sub(used);
    let ecx = u32::from((free as i32) < 0).wrapping_sub(1);
    Gprs { eax: free & ecx, ecx, edx: (entry.edx & 0xffff_ff00) | 1 }
}

/// 00356020 game_net_player_slot_30c_claim_by_index. cdecl(3): claims player slot `index`
/// (0..15, signed) with two caller words and the set-up flags.
pub fn player_slot_claim<M: GuestMemory>(m: &mut M, index: u32, word_a: u32, word_b: u32, entry: Gprs) -> Gprs {
    if (index as i32) < 0 || (index as i32) >= 16 {
        return Gprs { ecx: index, ..entry };
    }
    let slot = index.wrapping_mul(PLAYER_SLOT_STRIDE);
    let at = |offset: u32| slot.wrapping_add(PLAYER_SLOT_BASE).wrapping_add(offset);
    let flags = m.read8(at(0x34));
    m.write32(at(0), index);
    m.write32(at(4), word_a);
    m.write8(at(0x34), flags | 1);
    m.write32(at(0x20), word_b);
    m.write8(at(8), 1);
    m.write8(at(9), 0);
    m.write8(at(0x36), 0);
    m.write8(at(0x37), 0);
    Gprs { eax: slot, ecx: word_b, edx: (entry.edx & 0xffff_ff00) | 1 }
}

/// 003BC270 game_fesl_md5_context_init. cdecl(1): count word zero, then the four MD5 chaining
/// constants. EAX = the context pointer, ECX/EDX untouched.
pub fn md5_context_init<M: GuestMemory>(m: &mut M, context: u32, entry: Gprs) -> Gprs {
    for (offset, value) in [(0u32, 0u32), (4, 0x6745_2301), (8, 0xefcd_ab89), (12, 0x98ba_dcfe), (16, 0x1032_5476)] {
        m.write32(context.wrapping_add(offset), value);
    }
    Gprs { eax: context, ..entry }
}
