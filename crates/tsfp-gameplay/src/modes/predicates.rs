// SPDX-License-Identifier: GPL-3.0-or-later
//! Mode predicates and small state helpers (all Tier 0).
use super::{ModeHost, callees, globals};

// Guest jump tables: index bytes select an entry in the adjacent target table.
// Keep the target comparison rather than decoding the table into host constants.
const KEY_FIRST: u32 = 0x71;
const KEY_LAST_INDEX: u32 = 0x32;
const KEY_INDEX_BYTES: u32 = 0x0006_5300;
const KEY_TARGETS: u32 = 0x0006_52f8;
const KEY_ACCEPT_TARGET: u32 = 0x0006_52f0;
const SUBMODE_FIRST: u32 = 5;
const SUBMODE_LAST_INDEX: u32 = 0x0d;
const SUBMODE_INDEX_BYTES: u32 = 0x0013_dd30;
const SUBMODE_TARGETS: u32 = 0x0013_dd28;
const SUBMODE_ACCEPT_TARGET: u32 = 0x0013_dd1e;
const FLAG_CLEAR_MODE_STATE: u32 = 0x80_0040;
const MODE_WITH_DIRECT_TEXT_DISPATCH: u32 = 0x64;
const DIRECT_MODE_TEXT_TARGET: u32 = 0x0033_ed70;
// VA 0x00080F40: masks from mode and level records; table entries remain guest data.
const MASK_UNSUPPORTED: u32 = 0x100_0000;
const MASK_SPECIAL: u32 = 0x4000_0000;
const MASK_FALLBACK: u32 = 0x800_0000;
const MASK_RECORD_OFFSET: u32 = 8;
const SPECIAL_DISABLED_FLAG: u32 = 0x10_0000;
const MODE_JUMP_FIRST: u32 = 0x65;
const MODE_JUMP_LAST_INDEX: u32 = 6;
const MODE_JUMP_TARGETS: u32 = 0x0008_1098;
const MODE_FALLBACK_CHECK: u32 = 0x0036_cfa0;
const MODE_RECORD_LOOKUP: u32 = 0x0006_4d90;

/// 000425E0 game_state_6b98a4_enter_5_from_3_or_5_set_float. Moves the transition state from 3 or
/// 5 to 5 and copies the float constant at 0x475D18 to 0x6B98B0, unless mode flag bit 23 is set.
pub fn transition_enter_5<H: ModeHost>(h: &mut H) {
    if h.read32(globals::MODE_FLAGS) & 0x80_0000 != 0 {
        return;
    }
    let state = h.read32(globals::TRANSITION_STATE);
    if state == 3 || state == 5 {
        let value = h.read32(globals::TRANSITION_FLOAT_SOURCE);
        h.write32(globals::TRANSITION_FLOAT, value);
        h.write32(globals::TRANSITION_STATE, 5);
    }
}

/// 00042620 game_state_6b98a4_is_2_or_5_returns_zero. 0 while the transition state is 2 or 5.
pub fn transition_idle<H: ModeHost>(h: &H) -> u32 {
    u32::from(!matches!(h.read32(globals::TRANSITION_STATE), 2 | 5))
}

/// 000652C0 game_mode_or_key_predicate_modes_0x3a_0x19_0x3b_or_jump_table_arg2_0x71.
/// cdecl(mode, key). True for modes 0x3A, 0x19 and 0x3B, otherwise decided by an index table for
/// keys 0x71..=0xA3 that the original keeps in the XBE (index bytes at 0x65300, targets at
/// 0x652F8); read here through the host so the data stays in the guest image.
pub fn mode_or_key_predicate<H: ModeHost>(h: &H, mode: u32, key: u32) -> u32 {
    if matches!(mode, 0x3a | 0x19 | 0x3b) {
        return 1;
    }
    let index = key.wrapping_sub(KEY_FIRST);
    if index > KEY_LAST_INDEX {
        return 0;
    }
    let slot = u32::from(h.read8(KEY_INDEX_BYTES + index));
    u32::from(h.read32(KEY_TARGETS + slot * 4) == KEY_ACCEPT_TARGET)
}

/// 0013DCF0 game_mode_predicate_true_for_mode_byte_7de455_1_to_3_or_6_or_sub_mode_7de456_5_6_7_9_0xb_0xc_0xf_0x12.
/// Mode byte 1..=3 or 6 is true; otherwise the sub-mode byte 5..=0x12 is looked up in an index
/// table (bytes at 0x13DD30, targets at 0x13DD28) kept in the guest image.
pub fn mode_predicate<H: ModeHost>(h: &H) -> u32 {
    let mode = h.read8(globals::MODE_BYTE);
    if (1..=3).contains(&mode) || mode == 6 {
        return 1;
    }
    let index = u32::from(h.read8(globals::SUBMODE_BYTE)).wrapping_sub(SUBMODE_FIRST);
    if index > SUBMODE_LAST_INDEX {
        return 0;
    }
    let slot = u32::from(h.read8(SUBMODE_INDEX_BYTES + index));
    u32::from(h.read32(SUBMODE_TARGETS + slot * 4) == SUBMODE_ACCEPT_TARGET)
}

/// 002E7EB0 game_mode_le_0x64_and_players_above_2_and_count_is_4_or_arg_is_1_or_2.
/// cdecl(kind). Only for game modes up to 0x64 with more than two players: true with exactly four
/// players or for kind 1 and 2.
pub fn players_allow_mode<H: ModeHost>(h: &mut H, kind: u32) -> u32 {
    if (h.read32(globals::GAME_MODE) as i32) > 0x64 {
        return 0;
    }
    if (h.call(callees::PLAYER_COUNT, &[]) as i32) <= 2 {
        return 0;
    }
    let count = h.call(callees::PLAYER_COUNT, &[]);
    u32::from(count == 4 || kind == 1 || kind == 2)
}

/// VA 0x001CD1E0: clear the mode state word when flag bit 6 or 23 is set; otherwise
/// call the mode refresh function (VA 0x001CD170). The meaning of the state word is unknown.
pub fn clear_790958_or_refresh<H: ModeHost>(h: &mut H) -> u32 {
    if h.read32(globals::MODE_FLAGS) & FLAG_CLEAR_MODE_STATE != 0 {
        h.write32(globals::MODE_STATE_WORD, 0);
        return 0;
    }
    h.call(callees::MODE_REFRESH, &[])
}

/// VA 0x00227C30: dispatch the caller's arguments to the mode-specific text handler
/// for mode 0x64, or to the shared text lookup for other modes.
pub fn dispatch_text_or_mode_0x64<H: ModeHost>(h: &mut H, args: &[u32]) -> u32 {
    let target = if h.read32(globals::GAME_MODE) == MODE_WITH_DIRECT_TEXT_DISPATCH {
        DIRECT_MODE_TEXT_TARGET
    } else {
        callees::TEXT_BY_ID
    };
    h.call(target, args)
}

/// 00080F40 game_mode_flag_mask_applies_to_current_game_mode. cdecl(mask). Decides whether a
/// mode flag mask (as found in mode and level tables) applies to the current game mode, mode
/// byte, sub-mode and mode flags. The per-mode bit tests for game modes 0x65..=0x6B come from a
/// jump table in the guest image (0x81098), read through the host.
pub fn mode_flag_mask_applies<H: ModeHost>(h: &mut H, mask: u32) -> u32 {
    let mode = h.read32(globals::GAME_MODE);
    let flags = h.read32(globals::MODE_FLAGS);
    if mask & MASK_UNSUPPORTED != 0 {
        return 0;
    }
    if mask == MASK_SPECIAL {
        if flags & SPECIAL_DISABLED_FLAG == 0 || mode == 0x67 {
            return 1;
        }
        return 0;
    }
    let index = mode.wrapping_sub(MODE_JUMP_FIRST);
    if index <= MODE_JUMP_LAST_INDEX {
        return match h.read32(MODE_JUMP_TARGETS + index * 4) {
            0x0008_0f82 => u32::from(mask & 0x8000 != 0 || mask == 0x1000_0000),
            0x0008_0f9e => u32::from(flags & 0x20_0000 == 0 && mask & 0x20_1000 != 0),
            0x0008_0fb9 => (mask >> 11) & 1,
            0x0008_0fc3 => (mask >> 16) & 1,
            0x0008_0fcd => (mask >> 26) & 1,
            0x0008_0fd7 => (mask >> 29) & 1,
            0x0008_0fe1 => u32::from(mask & 0x8000_0000 != 0),
            _ => 0,
        };
    }
    let signed_mode = mode as i32;
    let by_table = (1..=0x53).contains(&signed_mode) || mode == 0x64;
    if !by_table {
        if mask & MASK_FALLBACK == 0 {
            return 0;
        }
        return u32::from(h.call(MODE_FALLBACK_CHECK, &[]) != 0);
    }
    if h.read8(globals::MODE_BYTE) == 0x0a && mode != 0x64 {
        let entry = h.call(MODE_RECORD_LOOKUP, &[mode]);
        if entry != 0 && h.read32(entry.wrapping_add(MASK_RECORD_OFFSET)) & mask != 0 {
            return 1;
        }
    }
    if mask & 0x80_0000 != 0 && mode == 0x64 {
        return 1;
    }
    if mask & 0x2_0000 != 0 && h.read8(globals::MODE_BYTE) != 0x0a {
        return 1;
    }
    if mask & 0x40_0000 != 0 && h.read8(globals::MODE_FLAGS) & 0x40 != 0 {
        return 1;
    }
    if mask & 0x20_0000 != 0 {
        return 1;
    }
    if h.read8(globals::SUBMODE_BYTE) == 0x0c && mask & 0x1000_0000 != 0 {
        return 1;
    }
    (mask >> 13) & 1
}

#[cfg(test)]
mod tests {
    use super::super::test_host::TestHost;
    use super::*;
    use crate::memory::GuestMemory;

    #[test]
    fn transition_enter_5_copies_float_and_sets_state() {
        let mut h = TestHost::default();
        h.write32(globals::TRANSITION_STATE, 3);
        h.write32(globals::TRANSITION_FLOAT_SOURCE, 0x3f80_0000);
        transition_enter_5(&mut h);
        assert_eq!(h.read32(globals::TRANSITION_STATE), 5);
        assert_eq!(h.read32(globals::TRANSITION_FLOAT), 0x3f80_0000);
        h.write32(globals::TRANSITION_STATE, 4);
        h.write32(globals::TRANSITION_FLOAT, 0);
        transition_enter_5(&mut h);
        assert_eq!(
            (
                h.read32(globals::TRANSITION_STATE),
                h.read32(globals::TRANSITION_FLOAT)
            ),
            (4, 0)
        );
        h.write32(globals::TRANSITION_STATE, 3);
        h.write32(globals::MODE_FLAGS, 0x80_0000);
        transition_enter_5(&mut h);
        assert_eq!(h.read32(globals::TRANSITION_STATE), 3);
    }

    #[test]
    fn transition_idle_is_zero_for_2_and_5() {
        let mut h = TestHost::default();
        for (state, want) in [(2, 0), (5, 0), (0, 1), (3, 1)] {
            h.write32(globals::TRANSITION_STATE, state);
            assert_eq!(transition_idle(&h), want);
        }
    }

    #[test]
    fn key_predicate_modes_and_table() {
        let mut h = TestHost::default();
        assert_eq!(mode_or_key_predicate(&h, 0x3a, 0), 1);
        h.write8(0x0006_5300 + 2, 1);
        h.write32(0x0006_52f8 + 4, 0x0006_52f0);
        assert_eq!(mode_or_key_predicate(&h, 0, 0x73), 1);
        assert_eq!(mode_or_key_predicate(&h, 0, 0x72), 0);
        assert_eq!(mode_or_key_predicate(&h, 0, 0x71 + 0x33), 0);
    }

    #[test]
    fn mode_predicate_ranges_and_table() {
        let mut h = TestHost::default();
        for (mode, want) in [(1u8, 1u32), (3, 1), (6, 1)] {
            h.write8(globals::MODE_BYTE, mode);
            assert_eq!(mode_predicate(&h), want);
        }
        h.write8(globals::MODE_BYTE, 4);
        h.write8(globals::SUBMODE_BYTE, 9);
        h.write8(0x0013_dd30 + 4, 1);
        h.write32(0x0013_dd28 + 4, 0x0013_dd1e);
        assert_eq!(mode_predicate(&h), 1);
        h.write8(globals::SUBMODE_BYTE, 0x20);
        assert_eq!(mode_predicate(&h), 0);
    }

    #[test]
    fn players_allow_mode_cases() {
        let mut h = TestHost::default();
        h.returns.insert(callees::PLAYER_COUNT, 4);
        assert_eq!(players_allow_mode(&mut h, 0), 1);
        h.returns.insert(callees::PLAYER_COUNT, 3);
        assert_eq!(players_allow_mode(&mut h, 0), 0);
        assert_eq!(players_allow_mode(&mut h, 2), 1);
        h.write32(globals::GAME_MODE, 0x65);
        assert_eq!(players_allow_mode(&mut h, 2), 0);
    }

    #[test]
    fn clear_or_refresh_and_dispatch() {
        let mut h = TestHost::default();
        h.write32(globals::MODE_STATE_WORD, 7);
        h.write32(globals::MODE_FLAGS, 0x40);
        clear_790958_or_refresh(&mut h);
        assert_eq!(h.read32(globals::MODE_STATE_WORD), 0);
        h.write32(globals::MODE_STATE_WORD, 9);
        h.write32(globals::MODE_FLAGS, 0x80_0000);
        assert_eq!(clear_790958_or_refresh(&mut h), 0);
        assert_eq!(h.read32(globals::MODE_STATE_WORD), 0);
        assert!(h.calls.is_empty());
        h.write32(globals::MODE_STATE_WORD, 11);
        h.write32(globals::MODE_FLAGS, 0);
        h.returns.insert(callees::MODE_REFRESH, 17);
        assert_eq!(clear_790958_or_refresh(&mut h), 17);
        assert_eq!(h.read32(globals::MODE_STATE_WORD), 11);
        assert_eq!(h.calls.last().unwrap(), &(callees::MODE_REFRESH, vec![]));
        h.write32(globals::GAME_MODE, 0x64);
        dispatch_text_or_mode_0x64(&mut h, &[5]);
        assert_eq!(h.calls.last().unwrap(), &(DIRECT_MODE_TEXT_TARGET, vec![5]));
        h.write32(globals::GAME_MODE, 1);
        dispatch_text_or_mode_0x64(&mut h, &[5]);
        assert_eq!(h.calls.last().unwrap(), &(callees::TEXT_BY_ID, vec![5]));
    }

    #[test]
    fn mode_mask_table_record_and_fallback_contracts() {
        let mut h = TestHost::default();
        h.write32(globals::GAME_MODE, 0x65);
        h.write32(MODE_JUMP_TARGETS, 0x0008_0f82);
        assert_eq!(mode_flag_mask_applies(&mut h, 0x8000), 1);
        assert_eq!(mode_flag_mask_applies(&mut h, MASK_UNSUPPORTED), 0);

        h.write32(globals::GAME_MODE, 1);
        h.write8(globals::MODE_BYTE, 0x0a);
        h.returns.insert(MODE_RECORD_LOOKUP, 0x2000);
        h.write32(0x2000 + MASK_RECORD_OFFSET, 0x100);
        assert_eq!(mode_flag_mask_applies(&mut h, 0x100), 1);
        assert_eq!(h.calls.last().unwrap(), &(MODE_RECORD_LOOKUP, vec![1]));

        h.write32(globals::GAME_MODE, 0x70);
        h.returns.insert(MODE_FALLBACK_CHECK, 1);
        assert_eq!(mode_flag_mask_applies(&mut h, MASK_FALLBACK), 1);
        assert_eq!(h.calls.last().unwrap(), &(MODE_FALLBACK_CHECK, vec![]));
    }
}
