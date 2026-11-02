// SPDX-License-Identifier: GPL-3.0-or-later
//! Mode setup record accessors, setup entry scans and preload helpers (all Tier 0).
use super::challenge_setup::{
    ArcadeCategoryId, ChallengeId, ChallengeLaunchRequest, ChallengeUiMode, LaunchRecord,
    SelectedChallengeSetup, SetupHandoff,
};
use super::{Esp, ModeHost, callees, globals};

/// 00069E70 game_global_6f1e10_get_plus_0x140. Address of the transition block inside the setup
/// record (record pointer + 0x140).
pub fn setup_record_transition_block<H: ModeHost>(h: &H) -> u32 {
    h.read32(globals::SETUP_RECORD_POINTER).wrapping_add(0x140)
}

/// 000431F0 game_state_6b98a4_set_from_args_store_6f1e10_plus_0x140_then_dispatch.
/// cdecl(state, arg, extra). Stores the state and its argument, records the transition block
/// address and `extra`, then tail calls the transition dispatcher 42510.
pub fn transition_set<H: ModeHost>(h: &mut H, state: u32, arg: u32, extra: u32) -> u32 {
    h.write32(globals::TRANSITION_STATE, state);
    h.write32(globals::TRANSITION_ARG, arg);
    let block = setup_record_transition_block(h);
    h.write32(globals::TRANSITION_RESULT_A, block);
    h.write32(globals::TRANSITION_RESULT_B, extra);
    h.call(callees::TRANSITION_DISPATCH, &[state, arg, extra])
}

/// 00069E80 game_apply_mode_setup_record_6f1e10_fields_0xc_0x10_via_431f0. Applies state 1 with
/// the record's dword at +0xC and its signed byte at +0x10 as argument and extra.
pub fn apply_setup_record<H: ModeHost>(h: &mut H) -> u32 {
    let record = h.read32(globals::SETUP_RECORD_POINTER);
    let word = h.read32(record.wrapping_add(0xc));
    let signed_byte = h.read8(record.wrapping_add(0x10)) as i8 as i32 as u32;
    h.call(0x0004_31f0, &[1, signed_byte, word])
}

/// 001B6030 game_thunk_weapon_record_preload_or_spawn_resources_args_0_0. cdecl(record).
/// Calls 1B5C70(record, 0, 0).
pub fn weapon_record_preload<H: ModeHost>(h: &mut H, record: u32) -> u32 {
    h.call(0x001b_5c70, &[record, 0, 0])
}

/// 00233EF0 game_mode_setup_count_entries_matching_flag_mask. For the active setup (looked up
/// from the game mode) and only while 0x6B7AA8 is clear, counts its entries (200 bytes each from
/// +0xCC, count at +0xD0) whose flag test 6B2B0 passes: 0x75CCB4 counts them and 0x75CCB8 counts
/// them again for entries with a non-zero first dword.
pub fn setup_count_entries_matching<H: ModeHost>(h: &mut H) {
    const SCAN_DISABLED: u32 = 0x006b_7aa8;
    const ENTRIES_FIELD: u32 = 0xcc;
    const ENTRY_COUNT_FIELD: u32 = 0xd0;
    const FIRST_ENTRY_OFFSET: u32 = 0x60;
    const ENTRY_STRIDE: u32 = 0xc8;
    const ENTRY_FLAGS_FIELD: u32 = 0x54;
    const MATCH_COUNT: u32 = 0x0075_ccb4;
    const WEIGHTED_COUNT: u32 = 0x0075_ccb8;
    const FLAGS_MATCH: u32 = 0x0006_b2b0;

    let setup = h.call(callees::MODE_SETUP_LOOKUP, &[h.read32(globals::GAME_MODE)]);
    if h.read32(SCAN_DISABLED) != 0 || setup == 0 {
        return;
    }
    let entries = h.read32(setup.wrapping_add(ENTRIES_FIELD));
    h.write32(MATCH_COUNT, 0);
    h.write32(WEIGHTED_COUNT, 0);
    if (h.read32(setup.wrapping_add(ENTRY_COUNT_FIELD)) as i32) <= 0 {
        return;
    }
    let mut entry = entries.wrapping_add(FIRST_ENTRY_OFFSET);
    let mut index = 0u32;
    loop {
        let flag_word = h.read32(entry.wrapping_add(ENTRY_FLAGS_FIELD));
        if h.call(FLAGS_MATCH, &[flag_word]) != 0 {
            let matches = h.read32(MATCH_COUNT).wrapping_add(1);
            let mut weighted = h.read32(WEIGHTED_COUNT).wrapping_add(1);
            let marker = h.read32(entry);
            h.write32(MATCH_COUNT, matches);
            h.write32(WEIGHTED_COUNT, weighted);
            if marker != 0 {
                // Preserve the port's second write for marked entries.
                weighted = weighted.wrapping_add(1);
                h.write32(WEIGHTED_COUNT, weighted);
            }
        }
        index += 1;
        entry = entry.wrapping_add(ENTRY_STRIDE);
        // Re-read the count: a guest callee can mutate the setup record during the scan.
        if (index as i32) >= (h.read32(setup.wrapping_add(ENTRY_COUNT_FIELD)) as i32) {
            return;
        }
    }
}

/// 00238D50 game_count_leading_nonzero_dwords_of_row_75cd90_stride_0xc_index_below_0x10.
/// cdecl(row). Number of non-zero dwords (of three) in row `row` (12 bytes) of the table at
/// 0x75CD90; 0 for rows 16 and up (signed).
pub fn row_nonzero_dword_count<H: ModeHost>(h: &H, row: u32) -> u32 {
    if (row as i32) >= 0x10 {
        return 0;
    }
    let base = row.wrapping_mul(12).wrapping_add(0x0075_cd90);
    (0..3)
        .filter(|i| h.read32(base.wrapping_add(4 * i)) != 0)
        .count() as u32
}

/// Calls the preload function D7780(id, id) for a signed 16-bit id unless it is -1.
fn preload_id<H: ModeHost>(h: &mut H, id: i16) {
    if id != -1 {
        let value = id as i32 as u32;
        h.call(callees::PRELOAD_BY_ID, &[value, value]);
    }
}

// The two resource lists share signed 16-bit IDs but have different headers and strides.
// These offsets describe the guest records until they have generated typed layouts.
const COUNTED_LIST_COUNT: u32 = 0;
const COUNTED_LIST_FIRST_ID: u32 = 2;
const COUNTED_LIST_ID_STRIDE: u32 = 4;
const TYPE_LIST_TABLE: u32 = 0x0053_e41c;
const TYPE_LIST_RECORD_STRIDE: u32 = 0xa4;
const TYPE_LIST_COUNT: u32 = 0;
const TYPE_LIST_FIRST_ID: u32 = 4;
const TYPE_LIST_ID_STRIDE: u32 = 8;

/// 0023F320 game_call_d7780_for_each_word_in_counted_list_stride_4_skipping_minus1.
/// Register input EBX = list: a byte count at +0 followed from +2 by signed 16-bit ids spaced
/// four bytes apart; each id except -1 is preloaded.
pub fn preload_counted_list<H: ModeHost>(h: &mut H, list: u32) {
    let count = u32::from(h.read8(list.wrapping_add(COUNTED_LIST_COUNT)));
    for index in 0..count {
        let id = h.read16(list.wrapping_add(COUNTED_LIST_FIRST_ID + COUNTED_LIST_ID_STRIDE * index))
            as i16;
        preload_id(h, id);
    }
}

/// 0023F350 game_type_list_request_resource_preload_by_index. Register input EAX = type index:
/// the type record is at 0x53E41C + index * 0xA4 with a signed count at +0 and ids every 8 bytes
/// from +4. Same preload rule as [`preload_counted_list`].
pub fn preload_type_list<H: ModeHost>(h: &mut H, type_index: u32) {
    let record = type_index
        .wrapping_mul(TYPE_LIST_RECORD_STRIDE)
        .wrapping_add(TYPE_LIST_TABLE);
    let mut index = 0u32;
    while (index as i32) < (h.read32(record.wrapping_add(TYPE_LIST_COUNT)) as i32) {
        let id =
            h.read16(record.wrapping_add(TYPE_LIST_FIRST_ID + TYPE_LIST_ID_STRIDE * index)) as i16;
        preload_id(h, id);
        index += 1;
    }
}

/// 002719F0 game_mode_info_6a850_find_entry_by_id_stride_0x20. cdecl(id). Searches the entries
/// (32 bytes each, count at +0xF8, base pointer at +0xF4) of the active setup for the dword id at
/// +0 and returns the entry address, or 0.
pub fn find_setup_entry_by_id<H: ModeHost>(h: &mut H, id: u32) -> u32 {
    // These fields describe the mode setup's separate ID-indexed entry table. They are
    // distinct from the 0xC8-byte flag records scanned above.
    const ENTRIES_POINTER_FIELD: u32 = 0xf4;
    const ENTRY_COUNT_FIELD: u32 = 0xf8;
    const ENTRY_STRIDE: u32 = 0x20;
    const ENTRY_ID_FIELD: u32 = 0;

    let setup = h.call(callees::MODE_SETUP_LOOKUP, &[h.read32(globals::GAME_MODE)]);
    let count = h.read32(setup.wrapping_add(ENTRY_COUNT_FIELD)) as i32;
    let entries = h.read32(setup.wrapping_add(ENTRIES_POINTER_FIELD));
    for index in 0..count.max(0) as u32 {
        let entry = entries.wrapping_add(index.wrapping_mul(ENTRY_STRIDE));
        if h.read32(entry.wrapping_add(ENTRY_ID_FIELD)) == id {
            return entry;
        }
    }
    0
}

/// 002B0AF0 game_mode_3_alloc_group_tables_then_mode_2_alloc_extra_tables. Mode byte 3 allocates
/// the group tables (271910), then a multiplayer setup (275620 true) also tail calls 274650.
pub fn alloc_mode_tables<H: ModeHost>(h: &mut H) -> u32 {
    if h.read8(globals::MODE_BYTE) == 3 {
        h.call(0x0027_1910, &[]);
    }
    if h.call(0x0027_5620, &[]) != 0 {
        return h.call(0x0027_4650, &[]);
    }
    0
}

/// 00158C80 game_mode_side_tables_stamp_or_setup_by_mode_bytes_2_and_3. Runs 275EF0 when 275620
/// reports a setup; in mode 3 with a positive setup entry count tail calls 273240.
pub fn side_tables_stamp_or_setup<H: ModeHost>(h: &mut H) -> u32 {
    if h.call(0x0027_5620, &[]) != 0 {
        h.call(0x0027_5ef0, &[]);
    }
    if h.read8(globals::MODE_BYTE) == 3 && (h.read8(globals::SETUP_ENTRY_COUNT_BYTE) as i8) > 0 {
        return h.call(0x0027_3240, &[]);
    }
    0
}

/// Copies `dwords` dwords between guest buffers (a `rep movsd` of the original).
fn copy_dwords<H: ModeHost>(h: &mut H, from: u32, to: u32, dwords: u32) {
    for word in 0..dwords {
        let value = h.read32(from.wrapping_add(4 * word));
        h.write32(to.wrapping_add(4 * word), value);
    }
}

fn read_launch_record<H: ModeHost>(h: &H, address: u32) -> LaunchRecord {
    let mut bytes = [0; super::challenge_setup::LAUNCH_RECORD_BYTES];
    for (offset, byte) in bytes.iter_mut().enumerate() {
        *byte = h.read8(address.wrapping_add(offset as u32));
    }
    LaunchRecord::from_bytes(bytes)
}

fn write_launch_record<H: ModeHost>(h: &mut H, address: u32, record: &LaunchRecord) {
    for (offset, byte) in record.as_bytes().iter().copied().enumerate() {
        h.write8(address.wrapping_add(offset as u32), byte);
    }
}

/// Publish the selected Challenge record through T2381's typed handoff when the current menu
/// mode is one of the documented descriptor modes. Other menu modes retain their existing copy.
fn publish_challenge_handoff<H: ModeHost>(
    h: &mut H,
    staging_address: u32,
    active_address: u32,
    saved_record: Option<&LaunchRecord>,
) -> bool {
    let ui_mode = match h.read32(0x78ad2c) {
        6 => ChallengeUiMode::Mode6,
        7 => ChallengeUiMode::Mode7,
        _ => return false,
    };
    let record = read_launch_record(h, staging_address);
    let request = ChallengeLaunchRequest {
        category: ArcadeCategoryId(h.read32(0x78ad38)),
        row: None,
        challenge: ChallengeId(record.read_u32(0x70)),
        ui_mode,
    };
    let mut handoff = SetupHandoff::selected(SelectedChallengeSetup {
        request,
        record,
        variant2_participant: None,
    });
    handoff
        .publish(saved_record)
        .expect("saved setup record is present when the save flag requests it");
    write_launch_record(h, staging_address, &handoff.staging.record);
    let active = handoff
        .active
        .as_ref()
        .expect("publish assigns active state");
    write_launch_record(h, active_address, &active.record);
    true
}

/// 00067280 game_mode_setup_load_or_copy_7de340_to_7de400_roll_rng_mod3_then_start_670c0.
/// Loads (flag bit 23 of `[0x7DE398]` set: from the setup record after 69E80) or copies the
/// 0x28-dword mode setup block 0x7DE340 to 0x7DE400, starts the transition, rolls a random
/// 0..2 into the byte 0x7DE49A, applies the side-table seeding 1CEB00, publishes the derived
/// flags and mode words, starts 670C0 and tail calls 1CD1E0.
pub fn mode_setup_load_or_copy_then_start<H: ModeHost>(h: &mut H) -> u32 {
    const BLOCK: u32 = 0x007d_e340;
    const WORKING: u32 = 0x007d_e400;
    const SNAPSHOT_DWORDS: u32 = 0x28;
    if h.read32(0x007d_e398) & 0x80_0000 != 0 {
        let setup_slot = h.read8(0x007d_e3df) as i8 as i32 as u32;
        h.call(0x0006_9e80, &[setup_slot]);
        let record = h.call(0x0006_9e60, &[]);
        let saved = if matches!(h.read32(0x78ad2c), 6 | 7) {
            Some(read_launch_record(h, record.wrapping_add(0x10)))
        } else {
            None
        };
        if !publish_challenge_handoff(h, BLOCK, WORKING, saved.as_ref()) {
            copy_dwords(h, record.wrapping_add(0x10), BLOCK, SNAPSHOT_DWORDS);
            let flags = (h.read32(0x007d_e398) & 0x9fff_ffff) | 0x80_0000;
            h.write32(0x007d_e398, flags);
            copy_dwords(h, BLOCK, WORKING, SNAPSHOT_DWORDS);
        }
        h.call(0x0006_9a60, &[0]);
    } else {
        if !publish_challenge_handoff(h, BLOCK, WORKING, None) {
            copy_dwords(h, BLOCK, WORKING, SNAPSHOT_DWORDS);
        }
        if h.read8(globals::MODE_FLAGS) & 0x40 == 0 {
            h.call(callees::TRANSITION_SET, &[0, 0, 0]);
        } else {
            let record = h.call(0x0006_9e60, &[]);
            copy_dwords(h, WORKING, record.wrapping_add(0x10), SNAPSHOT_DWORDS);
            let player_count = h.read32(0x0079_0950);
            h.call(callees::TRANSITION_SET, &[3, player_count, 0x31ec0]);
        }
    }
    let roll = h.call(0x0015_3b40, &[]);
    h.write8(0x007d_e49a, (roll % 3) as u8);
    let sub_mode = h.read8(0x007d_e454) as i8 as i32 as u32;
    let seeded = h.call(0x001c_eb00, &[sub_mode]);
    let mut flags = h.read32(globals::MODE_FLAGS);
    if seeded != 0 {
        flags |= 0x2000_0000;
        h.write32(globals::MODE_FLAGS, flags);
        h.write8(0x007d_e49b, 0);
    }
    h.write32(0x007d_e304, !(flags >> 28) & 2);
    h.write32(0x007d_e328, 0);
    h.write32(0x007d_e314, u32::MAX);
    h.write32(0x007d_e310, u32::MAX);
    h.write32(0x006b_7aac, 0);
    h.call(0x0006_70c0, &[]);
    h.call(0x001c_d1e0, &[])
}

/// 00158BC0 game_mode_apply_d7780_per_setup_entry_or_player_slot_unless_flag_0x20000000. cdecl(0).
/// Unless mode flag bit 29 is set, runs 389B0 and then preloads the resources of the mode: in
/// multiplayer mode byte 0xA (outside game mode 0x64) for every setup entry (6BC50 returns the two
/// ids through two 4-byte stack locals, handed to D7780), otherwise for each per-player slot byte
/// at 0x7DE414 when the table `[0x7A2958]` has entries; sub-mode 0x13 additionally preloads id 0x5D.
// portreg: va=0x00158bc0
pub fn apply_mode_preloads<H: ModeHost>(h: &mut H) {
    let mut esp = Esp::at_entry(h);
    esp.sub(8);
    if h.read32(globals::MODE_FLAGS) & 0x2000_0000 != 0 {
        return;
    }
    esp.push(); // saved ESI
    h.call(0x0003_89b0, &[]);
    let multiplayer = h.read8(globals::MODE_BYTE) == 0x0a && h.read32(globals::GAME_MODE) != 0x64;
    if multiplayer {
        let mut entry = 0u32;
        if (h.call(0x0006_bc00, &[]) as i32) > 0 {
            loop {
                let first_out = esp.at(4);
                esp.push();
                let second_out = esp.at(0xc);
                esp.push();
                esp.push();
                h.call(0x0006_bc50, &[entry, second_out, first_out]);
                let (first, second) = (h.read32(esp.at(0x10)), h.read32(esp.at(0x14)));
                esp.push();
                esp.push();
                h.call(callees::PRELOAD_BY_ID, &[second, first]);
                esp.add(0x14);
                entry += 1;
                if (entry as i32) >= (h.call(0x0006_bc00, &[]) as i32) {
                    break;
                }
            }
        }
    } else if (h.read32(0x007a_2958) as i32) > 0
        && (h.read8(globals::SETUP_ENTRY_COUNT_BYTE) as i8) > 0
    {
        let mut slot = 0u32;
        loop {
            let player_count = h.read32(0x0079_0950);
            let id = u32::from(h.read8(player_count.wrapping_add(slot).wrapping_add(0x007d_e414)));
            h.call(callees::PRELOAD_BY_ID, &[id, id]);
            slot += 1;
            if (slot as i32) >= i32::from(h.read8(globals::SETUP_ENTRY_COUNT_BYTE) as i8) {
                break;
            }
        }
    }
    if h.read8(globals::SUBMODE_BYTE) == 0x13 {
        h.call(callees::PRELOAD_BY_ID, &[0x5d, 0x5d]);
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_host::TestHost;
    use super::*;
    use crate::memory::GuestMemory;

    #[test]
    fn transition_set_records_state_and_dispatches() {
        let mut h = TestHost::default();
        h.write32(globals::SETUP_RECORD_POINTER, 0x1000);
        transition_set(&mut h, 3, 4, 5);
        assert_eq!(h.read32(globals::TRANSITION_STATE), 3);
        assert_eq!(h.read32(globals::TRANSITION_RESULT_A), 0x1140);
        assert_eq!(h.read32(globals::TRANSITION_RESULT_B), 5);
        assert_eq!(h.calls, vec![(callees::TRANSITION_DISPATCH, vec![3, 4, 5])]);
    }

    #[test]
    fn apply_setup_record_sign_extends_byte() {
        let mut h = TestHost::default();
        h.write32(globals::SETUP_RECORD_POINTER, 0x2000);
        h.write32(0x200c, 77);
        h.write8(0x2010, 0xff);
        apply_setup_record(&mut h);
        assert_eq!(h.calls, vec![(0x0004_31f0, vec![1, u32::MAX, 77])]);
    }

    #[test]
    fn count_entries_counts_and_doubles_marked_entries() {
        let mut h = TestHost::default();
        h.returns.insert(callees::MODE_SETUP_LOOKUP, 0x3000);
        h.returns.insert(0x0006_b2b0, 1);
        h.write32(0x3000 + 0xcc, 0x4000);
        h.write32(0x3000 + 0xd0, 2);
        h.write32(0x4000 + 0x60, 9); // first entry marked
        setup_count_entries_matching(&mut h);
        assert_eq!(h.read32(0x0075_ccb4), 2);
        assert_eq!(h.read32(0x0075_ccb8), 3);
        h.write32(0x006b_7aa8, 1);
        h.write32(0x0075_ccb4, 99);
        setup_count_entries_matching(&mut h);
        assert_eq!(h.read32(0x0075_ccb4), 99);
    }

    #[test]
    fn row_count_and_find_entry() {
        let mut h = TestHost::default();
        h.write32(0x0075_cd90 + 12 + 4, 5);
        h.write32(0x0075_cd90 + 12 + 8, 5);
        assert_eq!(row_nonzero_dword_count(&h, 1), 2);
        assert_eq!(row_nonzero_dword_count(&h, 0x10), 0);
        h.returns.insert(callees::MODE_SETUP_LOOKUP, 0x5000);
        h.write32(0x5000 + 0xf8, 3);
        h.write32(0x5000 + 0xf4, 0x6000);
        h.write32(0x6000 + 0x40, 42);
        assert_eq!(find_setup_entry_by_id(&mut h, 42), 0x6040);
        assert_eq!(find_setup_entry_by_id(&mut h, 43), 0);
    }

    #[test]
    fn setup_entry_lookup_uses_first_match_and_signed_count() {
        let mut h = TestHost::default();
        h.returns.insert(callees::MODE_SETUP_LOOKUP, 0x5000);
        h.write32(0x5000 + 0xf4, 0x6000);
        h.write32(0x5000 + 0xf8, 3);
        h.write32(0x6000, 7);
        h.write32(0x6020, 42);
        h.write32(0x6040, 42);
        assert_eq!(find_setup_entry_by_id(&mut h, 42), 0x6020);
        h.write32(0x5000 + 0xf8, u32::MAX);
        assert_eq!(find_setup_entry_by_id(&mut h, 42), 0);
    }

    #[test]
    fn preload_skips_minus_one() {
        let mut h = TestHost::default();
        h.write8(0x7000, 3);
        h.bytes.insert(0x7002, 5);
        h.bytes.insert(0x7006, 0xff);
        h.bytes.insert(0x7007, 0xff);
        h.bytes.insert(0x700a, 9);
        preload_counted_list(&mut h, 0x7000);
        assert_eq!(
            h.calls,
            vec![
                (callees::PRELOAD_BY_ID, vec![5, 5]),
                (callees::PRELOAD_BY_ID, vec![9, 9])
            ]
        );
        let mut g = TestHost::default();
        g.write32(0x0053_e41c + 0xa4, 2);
        g.bytes.insert(0x0053_e41c + 0xa4 + 4, 7);
        preload_type_list(&mut g, 1);
        assert_eq!(g.calls.len(), 2);
        assert_eq!(g.calls[0], (callees::PRELOAD_BY_ID, vec![7, 7]));
    }

    #[test]
    fn alloc_and_stamp_branches() {
        let mut h = TestHost::default();
        h.write8(globals::MODE_BYTE, 3);
        h.returns.insert(0x0027_5620, 1);
        alloc_mode_tables(&mut h);
        assert_eq!(
            h.calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            vec![0x0027_1910, 0x0027_5620, 0x0027_4650]
        );
        let mut g = TestHost::default();
        g.write8(globals::MODE_BYTE, 3);
        g.write8(globals::SETUP_ENTRY_COUNT_BYTE, 2);
        side_tables_stamp_or_setup(&mut g);
        assert_eq!(g.calls.last().unwrap().0, 0x0027_3240);
    }
}
