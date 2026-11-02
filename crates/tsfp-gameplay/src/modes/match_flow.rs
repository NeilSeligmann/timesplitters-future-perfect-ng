// SPDX-License-Identifier: GPL-3.0-or-later
//! Match end flow (all Tier 0).
use super::{Esp, ModeHost, callees, globals, read_f32, x87_round24 as fp24};

/// Shared tail of 00067450 and 00067E60: unless the mode byte is 3 or 7 flags the match end as
/// pending and notifies (66E90); then, with mode flag bit 6 or bit 27 set, forces the match state
/// to 8 (zeroing its timer on change) and tail calls the state descriptor completion (231A20),
/// else tail calls the round-end entry (66FC0).
fn end_match<H: ModeHost>(h: &mut H, notify: bool) -> u32 {
    const NOTIFY_EXCLUDED_MODES: [u8; 2] = [3, 7];
    const STATE_8_FLAG: u32 = 0x40;
    const STATE_8_ALTERNATE_FLAG: u32 = 0x0800_0000;
    const END_STATE: u32 = 8;
    if notify {
        let mode = h.read8(globals::MODE_BYTE);
        if !NOTIFY_EXCLUDED_MODES.contains(&mode) {
            h.write32(globals::MATCH_END_PENDING, 1);
            h.call(callees::MATCH_END_NOTIFY, &[]);
        }
    }
    let flags = h.read32(globals::MODE_FLAGS);
    if flags & (STATE_8_FLAG | STATE_8_ALTERNATE_FLAG) == 0 {
        return h.call(callees::MATCH_END_ENTER_STATE_8, &[]);
    }
    if h.read32(globals::MATCH_STATE) != END_STATE {
        h.write32(globals::MATCH_STATE, END_STATE);
        h.write32(globals::MATCH_STATE_TIMER, 0);
    }
    h.call(callees::STATE_DESCRIPTOR_COMPLETE, &[])
}

/// 00067450 game_match_end_post_player_messages_then_enter_state_8. cdecl(notify). Does nothing
/// while a network session is active and the second session check fails, otherwise ends the match
/// (see [`end_match`]); `notify` selects the pending flag and notification step.
pub fn match_end_enter_state_8<H: ModeHost>(h: &mut H, notify: u32) -> u32 {
    if h.call(callees::NET_SESSION_ACTIVE_A, &[]) != 0
        && h.call(callees::NET_SESSION_ACTIVE_B, &[]) == 0
    {
        return 0;
    }
    end_match(h, notify != 0)
}

/// 00067E60 game_match_end_check_array_7de500_entry_equals_7de468_then_enter_state_8. When
/// `[0x7DE468]` is set, looks at player slots 1 and 2 (64160) and ends the match, with the
/// notification, for the first one equal to it (same session guard as [`match_end_enter_state_8`]).
pub fn match_end_if_slot_matches<H: ModeHost>(h: &mut H) -> u32 {
    const MATCH_END_TARGET: u32 = 0x007d_e468;
    const FIRST_PLAYER_SLOT: u32 = 1;
    const LAST_PLAYER_SLOT: u32 = 2;
    let target = h.read32(MATCH_END_TARGET);
    if target == 0 {
        return 0;
    }
    for slot in FIRST_PLAYER_SLOT..=LAST_PLAYER_SLOT {
        if h.call(callees::SLOT_PLAYER, &[slot]) == target {
            if h.call(callees::NET_SESSION_ACTIVE_A, &[]) != 0
                && h.call(callees::NET_SESSION_ACTIVE_B, &[]) == 0
            {
                return 0;
            }
            return end_match(h, true);
        }
    }
    0
}

/// 00068610 game_match_end_pick_next_state_and_restore_players_via_record_238_and_673b0.
/// Picks the next match state from the game mode and flags (state 7 when the round-end flag
/// 0x20000000 is set after 667B0, else 4 for modes up to 0x64, state 1 above), clears the state
/// timer on change, resets the per-mode scratch globals, restores players through 673B0 when
/// the end condition holds, then runs 45FD0 and tail calls 389B0. Returns 3 untouched when the
/// state is already 3.
pub fn match_end_pick_next_state<H: ModeHost>(h: &mut H) -> u32 {
    let state = h.read32(globals::MATCH_STATE);
    if state == 3 {
        return 3;
    }
    const ROUND_END: u32 = 0x2000_0000;
    let mode = h.read32(globals::GAME_MODE) as i32;
    let single_player = mode <= 0x64;
    let mut changed = true;
    if single_player {
        if h.read32(globals::MODE_FLAGS) & ROUND_END != 0 {
            h.call(0x0006_67b0, &[]);
            if h.read32(globals::MATCH_STATE) == 7 {
                changed = false;
            } else {
                h.write32(globals::MATCH_STATE, 7);
            }
        } else if state == 4 {
            changed = false;
        } else {
            h.write32(globals::MATCH_STATE, 4);
        }
    } else if state == 1 {
        changed = false;
    } else {
        h.write32(globals::MATCH_STATE, 1);
    }
    if changed {
        h.write32(globals::MATCH_STATE_TIMER, 0);
    }
    if h.read8(globals::MODE_BYTE) == 1 {
        h.write32(0x007d_e3e0, u32::MAX);
    }
    h.write32(0x006f_1194, 0);
    if single_player {
        h.call(0x0022_7b50, &[]);
    }
    let restore_pending = h.read32(0x006b_7aac);
    if h.read32(globals::MODE_FLAGS) & ROUND_END == 0 && single_player && restore_pending != 0 {
        h.call(0x0025_f010, &[]);
    }
    h.call(0x0003_89b0, &[]);
    if single_player {
        let flags = h.read32(globals::MODE_FLAGS);
        let wants_restore = h.read8(globals::MODE_BYTE) == 0x0a || flags & 0x40 != 0;
        if flags & ROUND_END == 0 && wants_restore && h.read32(0x006b_7aac) == 0 {
            h.write32(globals::MODE_FLAGS, flags | 0x4000_0000);
            h.call(0x0006_73b0, &[]);
        }
    }
    h.call(0x0004_5fd0, &[]);
    h.call(0x0003_89b0, &[])
}

/// 00067CD0 game_match_leader_change_announce_via_text_0x16f4_0x16f5_and_leader_time_accumulate.
/// The leader is the player index (+8) of the first object in the table at 0x7A3580 whose field
/// +0x80 is non-zero (-1 when none). When it differs from the stored leader `[0x7DE3E0]` the new
/// leader is stored and, if valid, announced to every other player (text 0x16F4 formatted with
/// the leader's name, 0x16F5 for the leader itself); then, for a valid leader, the leader's time
/// accumulator (+0xA64 of its 0xEA0-byte player block) grows by `[0x4E7944]` and a leader message
/// is sent for its slot (12E3B0).
pub fn match_leader_update<H: ModeHost>(h: &mut H) {
    const OBJECTS: u32 = 0x007a_3580;
    const LEADER: u32 = 0x007d_e3e0;
    const PLAYER_BLOCKS: u32 = 0x0073_56d8;
    const SLOT_RECORDS: u32 = 0x007b_0c48;
    const SLOT_SIZE: u32 = 0x1584;
    const PLAYER_SIZE: u32 = 0xea0;
    let object_count = h.read32(0x0074_c37c) as i32;
    let mut leader = u32::MAX;
    for index in 0..object_count.max(0) as u32 {
        let object = h.read32(OBJECTS.wrapping_add(4 * index));
        if object != 0 && h.read32(object.wrapping_add(0x80)) != 0 {
            leader = h.read32(object.wrapping_add(8));
            break;
        }
    }
    let mut current = h.read32(LEADER);
    if leader != current {
        h.write32(LEADER, leader);
        if (leader as i32) < 0 {
            return;
        }
        let text = h.call(callees::TEXT_BY_ID, &[0x16f4]);
        let name = h.read32(
            leader
                .wrapping_mul(PLAYER_SIZE)
                .wrapping_add(h.read32(PLAYER_BLOCKS))
                .wrapping_add(0x70),
        );
        let message = h.call(0x0015_1030, &[0x0047_8904, name, text]);
        let slot_of = |h: &H, leader: u32| {
            leader
                .wrapping_mul(SLOT_SIZE)
                .wrapping_add(h.read32(SLOT_RECORDS))
                .wrapping_add(4)
        };
        if (leader as i32) < (h.read32(0x0079_0950) as i32) {
            let owner = h.read32(slot_of(h, leader));
            if (owner as i32) >= 0 {
                let text = h.call(callees::TEXT_BY_ID, &[0x16f5, 0]);
                let owner = h.read32(slot_of(h, h.read32(LEADER)));
                h.call(0x0012_d350, &[owner, 3, text, 0]);
            }
        }
        let mut player = 0u32;
        if (h.call(callees::PLAYER_COUNT, &[]) as i32) > 0 {
            loop {
                let leader_slot = h.read32(slot_of(h, h.read32(LEADER)));
                if player != leader_slot {
                    let slot = h.call(0x0035_6800, &[player, message]);
                    let record = slot
                        .wrapping_mul(SLOT_SIZE)
                        .wrapping_add(h.read32(SLOT_RECORDS));
                    h.call(0x000c_f4d0, &[record]);
                }
                player += 1;
                if (player as i32) >= (h.call(callees::PLAYER_COUNT, &[]) as i32) {
                    break;
                }
            }
        }
        current = h.read32(LEADER);
    }
    if (current as i32) < 0 {
        return;
    }
    let block = current
        .wrapping_mul(PLAYER_SIZE)
        .wrapping_add(h.read32(PLAYER_BLOCKS))
        .wrapping_add(0xa64);
    let time = h.read32(block).wrapping_add(h.read32(0x004e_7944));
    h.write32(block, time);
    let current = h.read32(LEADER);
    if (current as i32) < (h.read32(0x0079_0950) as i32) {
        let owner = h.read32(
            current
                .wrapping_mul(SLOT_SIZE)
                .wrapping_add(h.read32(SLOT_RECORDS))
                .wrapping_add(4),
        );
        if (owner as i32) >= 0 {
            let argument = h.read32(0x004e_8cec).wrapping_mul(4).wrapping_add(0x40);
            h.call(0x0012_e3b0, &[owner, argument]);
        }
    }
}

/// 000B9D00 game_match_end_post_result_messages_to_all_players_and_trigger_end. cdecl(character).
/// Posts a result message for every player slot (text 0x1643 when the character's brain type tag
/// at `[[character+0xBF0]+0x28]` is "PRAP", else 0x1644; 12D350 with display time 0.8f), notifies
/// the slot's object through F8AD0 and FD7E0 for the first four slots, marks flag bit 16 of
/// `[0x7DE45C]` and ends the match through 67450 with notify off.
pub fn match_end_post_result_messages<H: ModeHost>(h: &mut H, character: u32) -> u32 {
    const SLOT_SIZE: u32 = 0x1584;
    let mut player = 0u32;
    if (h.call(callees::PLAYER_COUNT, &[]) as i32) > 0 {
        let mut offset = 0u32;
        loop {
            let brain = h.read32(character.wrapping_add(0xbf0));
            let text_id = if h.read32(brain.wrapping_add(0x28)) == 0x5041_5250 {
                0x1643
            } else {
                0x1644
            };
            let text = h.call(callees::TEXT_BY_ID, &[text_id, 0x3fcc_cccd]);
            h.call(0x0012_d350, &[player, 4, text, 0x3fcc_cccd]);
            if (offset as i32) >= 0 && (offset as i32) < 0x5610 {
                let slots = h.read32(0x007b_0c48);
                if slots != 0 {
                    let object = h.read32(offset.wrapping_add(slots).wrapping_add(8));
                    if object != 0 {
                        let key = h.read32(object.wrapping_add(0x214));
                        let handle = h.call(0x000f_8ad0, &[key]);
                        h.call(0x000f_d7e0, &[handle]);
                    }
                }
            }
            player += 1;
            offset = offset.wrapping_add(SLOT_SIZE);
            if (player as i32) >= (h.call(callees::PLAYER_COUNT, &[]) as i32) {
                break;
            }
        }
    }
    let flags = h.read32(0x007d_e45c) | 0x1_0000;
    h.write32(0x007d_e45c, flags);
    h.call(0x0006_7450, &[0])
}

/// 00067F00 game_match_per_frame_update_player_scale_0xad0_0xb14_0xb2c_timer_and_round_end_check.
/// Per-frame match update. Advances the match clock `[0x7A36C0]` by `[0x4E794C] / [0x4E8CEC]`,
/// finds the last object of the table at 0x7A3580 whose bit in the active mask `[0x7A2BC8]` is set
/// (`[0x7DE4A0]`, -1 when none) and counts the inactive ones, derives per-object scale values
/// (+0xAD0, +0xB14, +0xB2C of the object) from the step-size callee 350CF0 (float in ST0) and
/// notifies each object's slot through CF370. In sub-mode 0xB the last active object loses
/// `[0x4E7944]` from its time accumulator (+0xA64). Finally, in sub-mode 0xC with at most one
/// inactive object, a known leader, no mode flag 0x8000000 and enough elapsed time, it ends the
/// round: through 66FC0, or via state 8 and 231A20 when mode flag bit 6 or 27 is set. All float
/// math mirrors the original's SSE (f32) and x87 (24-bit precision, f64 intermediates) sequences.
pub fn match_frame_update<H: ModeHost>(h: &mut H) -> u32 {
    const OBJECTS: u32 = 0x007a_3580;
    const LAST_ACTIVE: u32 = 0x007d_e4a0;
    const SLOT_RECORDS: u32 = 0x007b_0c48;
    let tick = read_f32(h, 0x004e_794c) / (h.read32(0x004e_8cec) as i32 as f32);
    let object_count = h.read32(0x0074_c37c) as i32;
    let mut last_active = u32::MAX;
    let mut inactive = 0i32;
    let clock = tick + read_f32(h, 0x007a_36c0);
    h.write32(LAST_ACTIVE, last_active);
    h.write32(0x007a_36c0, clock.to_bits());
    let mut mask = h.read32(0x007a_2bc8);
    let is_active = |mask: u32, object: u32, h: &H| {
        mask & 1u32.wrapping_shl(h.read32(object.wrapping_add(8)) & 31) != 0
    };
    for index in 0..object_count.max(0) as u32 {
        let object = h.read32(OBJECTS.wrapping_add(4 * index));
        if object == 0 {
            continue;
        }
        if is_active(mask, object, h) {
            last_active = index;
            h.write32(LAST_ACTIVE, last_active);
        } else {
            inactive += 1;
        }
    }
    let frame_ticks = h.read32(0x004e_8cec);
    if object_count > 0 {
        let mut index = 0u32;
        loop {
            if index > 0 {
                mask = h.read32(0x007a_2bc8);
            }
            let object = h.read32(OBJECTS.wrapping_add(4 * index));
            if object != 0 {
                let player = h.read32(object.wrapping_add(8));
                let blocked = h.read32(object.wrapping_add(0xc6c)) & 0x1000 != 0;
                let step = |h: &mut H, object: u32| {
                    h.call_f32(
                        0x0035_0cf0,
                        &[h.read16(object.wrapping_add(0x48)) as i16 as i32 as u32],
                    )
                };
                if mask & 1u32.wrapping_shl(player & 31) == 0 {
                    if h.read8(globals::SUBMODE_BYTE) == 0x0c {
                        let now = h.read32(0x007d_e318);
                        h.write32(object.wrapping_add(0xa64), now);
                    }
                    if !blocked {
                        let value = step(h, object);
                        let scaled = fp24(f64::from(value) * f64::from(read_f32(h, 0x0047_5c88)));
                        h.write32(object.wrapping_add(0xb14), value.to_bits());
                        h.write32(object.wrapping_add(0xad0), value.to_bits());
                        let divisor = fp24(
                            f64::from(frame_ticks as i32) * f64::from(read_f32(h, 0x0047_8910)),
                        );
                        h.write32(
                            object.wrapping_add(0xb2c),
                            (fp24(scaled / divisor) as f32).to_bits(),
                        );
                        if (player as i32) < (h.read32(0x0079_0950) as i32) {
                            let record = player
                                .wrapping_mul(0x1584)
                                .wrapping_add(h.read32(SLOT_RECORDS));
                            h.call(0x000c_f370, &[record, value.to_bits()]);
                        }
                    }
                } else if !blocked {
                    let elapsed = h.read32(0x007d_e318) as i32 as f32;
                    let denominator = (frame_ticks.wrapping_mul(0x78) as i32) as f32;
                    let mut bound =
                        elapsed / denominator * read_f32(h, 0x0047_85e4) + read_f32(h, 0x0047_5c78);
                    let ceiling = read_f32(h, 0x0047_8398);
                    if bound > ceiling {
                        bound = ceiling;
                    }
                    let value = step(h, object);
                    let scaled = fp24(f64::from(value) * f64::from(bound));
                    h.write32(object.wrapping_add(0xb14), (scaled as f32).to_bits());
                    h.write32(object.wrapping_add(0xad0), (scaled as f32).to_bits());
                    let rate = fp24(scaled * f64::from(read_f32(h, 0x0047_5c88)));
                    let divisor =
                        fp24(f64::from(frame_ticks as i32) * f64::from(read_f32(h, 0x0047_8910)));
                    h.write32(
                        object.wrapping_add(0xb2c),
                        (fp24(rate / divisor) as f32).to_bits(),
                    );
                    let player = h.read32(object.wrapping_add(8));
                    if (player as i32) < (h.read32(0x0079_0950) as i32) {
                        let again = step(h, object);
                        let argument =
                            fp24(f64::from(again) * f64::from(read_f32(h, 0x0047_890c))) as f32;
                        let record = h
                            .read32(object.wrapping_add(8))
                            .wrapping_mul(0x1584)
                            .wrapping_add(h.read32(SLOT_RECORDS));
                        h.call(0x000c_f370, &[record, argument.to_bits()]);
                    }
                }
            }
            index += 1;
            if (index as i32) >= (h.read32(0x0074_c37c) as i32) {
                break;
            }
        }
        last_active = h.read32(LAST_ACTIVE);
    }
    if h.read8(globals::SUBMODE_BYTE) == 0x0b {
        let object = h.read32(OBJECTS.wrapping_add(last_active.wrapping_mul(4)));
        if object != 0 {
            let time = h
                .read32(object.wrapping_add(0xa64))
                .wrapping_sub(h.read32(0x004e_7944));
            h.write32(object.wrapping_add(0xa64), time);
        }
    }
    if inactive > 1 || h.read8(globals::SUBMODE_BYTE) != 0x0c {
        return 0;
    }
    if (h.read32(LAST_ACTIVE) as i32) < 0 || h.read32(globals::MODE_FLAGS) & 0x800_0000 != 0 {
        return 0;
    }
    let threshold = h.read32(0x004e_8cec).wrapping_mul(5) as i32;
    if (h.read32(0x007d_e318) as i32) <= threshold {
        return 0;
    }
    if h.call(callees::NET_SESSION_ACTIVE_A, &[]) != 0
        && h.call(callees::NET_SESSION_ACTIVE_B, &[]) == 0
    {
        return 0;
    }
    let flags = h.read32(globals::MODE_FLAGS);
    if flags & 0x40 == 0 && flags & 0x800_0000 == 0 {
        return h.call(callees::MATCH_END_ENTER_STATE_8, &[]);
    }
    if h.read32(globals::MATCH_STATE) != 8 {
        h.write32(globals::MATCH_STATE, 8);
        h.write32(globals::MATCH_STATE_TIMER, 0);
    }
    h.call(callees::STATE_DESCRIPTOR_COMPLETE, &[])
}

/// 00066FC0 game_round_end_enter_state_8_run_stats_update_and_play_end_sound_0xd6a. cdecl(0).
/// Enters match state 8 (clearing the timer on change), updates each player's stats and awards
/// (2E4B80 and, in a live session, 2E6D40 and 2E7010), stamps the end time, then plays the round
/// end sound 0xD6A: the sound request block is a 0x30-byte stack local filled by 45390 and handed
/// to 45820. Finally posts the result screen request 45A00: result 2 when the round ended by the
/// flags (bit 6 or 27 and no pending result bit 15), a stalemate (mode byte 0xA outside mode 0x64
/// and not 0x38 returns without a request), otherwise result 1.
pub fn round_end_enter_state_8<H: ModeHost>(h: &mut H) {
    const NET_SESSION_ACTIVE: u32 = 0x0035_33f0;
    let mut esp = Esp::at_entry(h);
    esp.sub(0x30);
    h.call(0x0006_3eb0, &[]);
    if h.read32(globals::MATCH_STATE) != 8 {
        h.write32(globals::MATCH_STATE, 8);
        h.write32(globals::MATCH_STATE_TIMER, 0);
    }
    esp.push(); // saved ESI
    h.call(0x0006_68f0, &[]);
    let mut player = 0u32;
    if (h.call(callees::PLAYER_COUNT, &[]) as i32) > 0 {
        loop {
            if h.call(0x002e_4b80, &[player]) != 0 {
                h.call(0x002e_6d40, &[player]);
                if h.call(NET_SESSION_ACTIVE, &[]) != 0 {
                    h.call(0x002e_7010, &[player]);
                }
            }
            player += 1;
            if (player as i32) >= (h.call(callees::PLAYER_COUNT, &[]) as i32) {
                break;
            }
        }
    }
    let now = h.read32(0x007d_e318);
    h.write32(0x006f_11b8, now);
    h.call(0x0004_55d0, &[]);
    h.call(0x0015_bdc0, &[]);
    let request = esp.at(4);
    esp.push();
    h.call(0x0004_5390, &[request]);
    let sound_block = esp.at(8);
    esp.push();
    h.write32(esp.at(0xc), 0xd6a);
    h.call(0x0004_5820, &[sound_block, request]);
    let flags = h.read32(globals::MODE_FLAGS);
    let result = |h: &mut H, kind: u32| {
        h.call(0x0004_5a00, &[kind, 0]);
    };
    if flags & 0x40 != 0 || flags & 0x800_0000 != 0 {
        if h.read32(0x007d_e45c) & 0x8000 != 0 {
            result(h, 2);
        } else {
            result(h, 1);
        }
        return;
    }
    if h.read32(0x007d_e45c) & 0x1_0000 != 0 {
        result(h, 1);
        return;
    }
    let mode = h.read32(globals::GAME_MODE);
    if h.read8(globals::MODE_BYTE) != 0x0a || mode == 0x64 || mode == 0x38 {
        result(h, 2);
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_host::TestHost;
    use super::*;
    use crate::memory::GuestMemory;

    #[test]
    fn session_guard_blocks_end() {
        let mut h = TestHost::default();
        h.returns.insert(callees::NET_SESSION_ACTIVE_A, 1);
        match_end_enter_state_8(&mut h, 1);
        assert_eq!(h.calls.len(), 2);
        assert_eq!(h.read32(globals::MATCH_END_PENDING), 0);
    }

    #[test]
    fn plain_end_notifies_and_enters_round_end() {
        let mut h = TestHost::default();
        h.write8(globals::MODE_BYTE, 1);
        match_end_enter_state_8(&mut h, 1);
        assert_eq!(h.read32(globals::MATCH_END_PENDING), 1);
        assert_eq!(h.calls.last().unwrap().0, callees::MATCH_END_ENTER_STATE_8);
        let mut g = TestHost::default();
        g.write8(globals::MODE_BYTE, 3);
        match_end_enter_state_8(&mut g, 1);
        assert_eq!(g.read32(globals::MATCH_END_PENDING), 0);
    }

    #[test]
    fn flag_bit_forces_state_8() {
        let mut h = TestHost::default();
        h.write32(globals::MODE_FLAGS, 0x40);
        h.write32(globals::MATCH_STATE, 2);
        h.write32(globals::MATCH_STATE_TIMER, 5);
        match_end_enter_state_8(&mut h, 0);
        assert_eq!(h.read32(globals::MATCH_STATE), 8);
        assert_eq!(h.read32(globals::MATCH_STATE_TIMER), 0);
        assert_eq!(
            h.calls.last().unwrap().0,
            callees::STATE_DESCRIPTOR_COMPLETE
        );
    }

    #[test]
    fn slot_match_ends_match() {
        let mut h = TestHost::default();
        h.write32(0x007d_e468, 7);
        h.returns.insert(callees::SLOT_PLAYER, 7);
        match_end_if_slot_matches(&mut h);
        assert!(
            h.calls
                .iter()
                .any(|c| c.0 == callees::MATCH_END_ENTER_STATE_8)
        );
        let mut g = TestHost::default();
        g.write32(0x007d_e468, 7);
        assert_eq!(match_end_if_slot_matches(&mut g), 0);
        assert_eq!(g.calls.len(), 2);
    }
}
