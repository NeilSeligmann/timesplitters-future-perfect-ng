// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 3 gameplay (player, mode and state) leaves. Each body ports a call-free C
//! replacement of `src/game`, see docs/t1836-rust-wave3.md. Reads and writes keep the C order.
use crate::{gprs::Gprs, memory::GuestMemory};

const MODE_STATE: u32 = 0x006d_60d8;
const MODE_TARGET: u32 = 0x006d_60a8;
const MODE_TARGET_MATCH: u32 = 0x007e_3f6c;
const PLAYER_POINTER: u32 = 0x007b_0c7c;
const PLAYER_ARRAY_POINTER: u32 = 0x0073_56d8;

/// 00056E90 game_state_6d60d8_is_one_of_1_3_7_11_12_13. cdecl(0), EAX only.
// portreg: va=0x00056e90 tier=1
pub fn state_is_one_of_1_3_7_11_12_13<M: GuestMemory>(m: &M) -> u32 {
    u32::from(matches!(m.read32(MODE_STATE), 1 | 3 | 7 | 11 | 12 | 13))
}

/// 00056F70: 1 when [6D60A8] equals [7E3F6C], else the list above plus 8 and 9. cdecl(0).
// portreg: va=0x00056f70 tier=1
pub fn state_is_playable_or_target_matches<M: GuestMemory>(m: &M) -> u32 {
    if m.read32(MODE_TARGET) == m.read32(MODE_TARGET_MATCH) {
        return 1;
    }
    u32::from(matches!(
        m.read32(MODE_STATE),
        1 | 3 | 7 | 8 | 9 | 11 | 12 | 13
    ))
}

/// 000D0CA0 game_player_sync_owner_object_position_from_player_position_0xa8_unless_state_0x400.
/// cdecl(0). EDX keeps its entry value `edx_in` on the skip path. The player pointer at 7B0C7C is re-read between the stores, so a store that
/// aliases it changes the later reads exactly as in the C body.
pub fn player_sync_owner_position<M: GuestMemory>(m: &mut M, edx_in: u32) -> Gprs {
    let mut ecx = m.read32(PLAYER_POINTER);
    let skip = m.read32(ecx.wrapping_add(0x114)) == 0x400;
    let eax = m.read32(ecx.wrapping_add(0x14));
    if skip {
        return Gprs {
            eax,
            ecx,
            edx: edx_in,
        };
    }
    ecx = m.read32(ecx.wrapping_add(0xa8));
    m.write32(eax.wrapping_add(0x64), ecx);
    let mut edx = m.read32(PLAYER_POINTER);
    ecx = m.read32(edx.wrapping_add(0xac));
    m.write32(eax.wrapping_add(0x68), ecx);
    edx = m.read32(PLAYER_POINTER);
    ecx = m.read32(edx.wrapping_add(0xb0));
    m.write32(eax.wrapping_add(0x6c), ecx);
    Gprs { eax, ecx, edx }
}

/// 000C4560 game_player_slot_summary_fill_by_index. cdecl(2): (index, out). All three
/// registers are overwritten on both paths.
pub fn player_slot_summary_fill<M: GuestMemory>(m: &mut M, index: u32, out: u32) -> Gprs {
    if index == u32::MAX {
        for offset in (0..32).step_by(4) {
            m.write32(out.wrapping_add(offset), 0);
        }
        m.write32(out, u32::MAX);
        return Gprs {
            eax: out,
            ecx: 0,
            edx: out,
        };
    }
    let base = m.read32(PLAYER_ARRAY_POINTER);
    let slot = index.wrapping_mul(0xea0).wrapping_add(base);
    m.write32(out, index);
    let player = m.read32(slot);
    let mut edx = m.read32(player.wrapping_add(0x34));
    m.write32(out.wrapping_add(4), edx);
    for (field, dest) in [
        (0xbfc, 8),
        (0xc00, 12),
        (0xc1c, 16),
        (0xc20, 20),
        (0xbf8, 24),
    ] {
        edx = m.read32(slot.wrapping_add(field));
        m.write32(out.wrapping_add(dest), edx);
    }
    let eax = m.read32(slot.wrapping_add(0xc04));
    m.write32(out.wrapping_add(28), eax);
    Gprs { eax, ecx: out, edx }
}
