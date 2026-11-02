// SPDX-License-Identifier: GPL-3.0-or-later
//! T2371 Tier 1: Rust-owned Map Maker setup roster and launch record against lifted originals.
#![cfg(modes_lifted)]

use super::*;
use tsfp_mapmaker::modes::{
    self, AssaultModeConfiguration, AssaultPhaseKind, AssaultPhases, EditorKind, GameType,
    LaunchConfiguration, LaunchInput, LaunchVariant, MapMode, ModeConfiguration, ModeSettings,
    PreviewRow, PreviewSelection, RosterMetadata, RosterSource, SetupRoster, SetupSlot, ThemeId,
};

unsafe extern "C" {
    fn md_run(va: u32, regs: *mut u32) -> i32;
    fn md_script(words: *const u32, entries: u32);
    fn md_st0(words: *const u32, entries: u32);
    fn md_writes(words: *const u32, entries: u32);
    fn md_trace_len() -> u32;
    fn md_trace_entry(index: u32) -> *const u32;
}

const POOL: u32 = 0x0090_0000;
const POOL_LEN: u32 = 0x8000;
const STACK: u32 = 0x0030_0000;
const FRAME: u32 = 0x400;
const SETUP: u32 = 0x0090_1000;
const IDS: u32 = 0x0090_2000;
const CONDITIONAL: u32 = 0x0090_2100;
const DERIVED: u32 = 0x0090_2200;
const EDITOR: u32 = 0x0090_3000;
const TABLE_TARGETS: [u32; 8] = [
    0x0031_3cb5,
    0x0031_3d3c,
    0x0031_3d63,
    0x0031_3d82,
    0x0031_3dc4,
    0x0031_3df1,
    0x0031_3e59,
    0x0031_3eb9,
];

fn clear(p: &mut Pair, address: u32, len: u32) {
    unsafe {
        std::ptr::write_bytes(p.c.add(address as usize), 0, len as usize);
        std::ptr::write_bytes(p.rust.add(address as usize), 0, len as usize);
    }
}

#[derive(Clone)]
struct Meta {
    trace: Vec<[u32; 4]>,
}
impl RosterMetadata for Meta {
    fn has_conditional_metadata(&mut self, id: i16) -> bool {
        let value = matches!(id, 10 | 11 | 36 | 99);
        self.trace.push([0x2be270, id as u16 as u32, 0, 0]);
        value
    }
    fn derived_metadata(&mut self, id: i16) -> u8 {
        let value = test_derived(id);
        self.trace.push([0x1b5630, id as u16 as u32, 0, 0]);
        value
    }
}

fn test_derived(id: i16) -> u8 {
    let table_word = (id as u16 as u32).wrapping_mul(7);
    (15i32.wrapping_sub(table_word as i32) / 3) as u8
}

fn launch_mode(rng: &mut Rng) -> MapMode {
    [
        MapMode::Deathmatch,
        MapMode::Bag,
        MapMode::Escort,
        MapMode::Assault,
    ][rng.below(4) as usize]
}

fn game_type(variant: u32) -> u8 {
    match variant {
        2 => 10,
        3 | 4 | 7 | 8 | 9 | 10 | 18 | 19 => 9,
        5 => 2,
        6 => 1,
        12 => 5,
        15 => 6,
        16 => 3,
        _ => 0x71,
    }
}

fn original_launch_record(p: &Pair) -> [u8; modes::LAUNCH_BYTES] {
    for index in 0..unsafe { md_trace_len() } {
        let row = unsafe { *(md_trace_entry(index) as *const [u32; 16]) };
        if row[0] == 0x658d0 {
            // The lifted target consumes its record in EAX. Trace words 3..5 are
            // stack arguments; word 6 captures EAX at the call boundary.
            let address = row[6];
            assert_ne!(address, 0, "lifted handoff record pointer in EAX");
            return std::array::from_fn(|i| p.get(false, address.wrapping_add(i as u32)));
        }
    }
    panic!("lifted 0x313a80 did not hand off a launch record");
}

/// The current lifted C loses x86 condition flags across MOVSX in these two branches. The
/// REA instruction contract in docs/t1688-rea-launch.md establishes the original parity result,
/// so compare the remaining handoff bytes to the lift and verify this field independently.
fn assert_launch_matches_lift_except_known_lifted_parity(
    actual: [u8; modes::LAUNCH_BYTES],
    mut lifted: [u8; modes::LAUNCH_BYTES],
    variant: u32,
    local_slots: u8,
    emitted_count: u8,
    team_values: &[u8; modes::PLAYER_SLOTS],
    seed: usize,
    calls: &[[u32; 4]],
) {
    if matches!(variant as u8, 0x10 | 0x11) {
        let mut expected_teams = [0; modes::PLAYER_SLOTS];
        for index in 0..usize::from(emitted_count).min(modes::PLAYER_SLOTS - 1) {
            let slot = usize::from(local_slots).wrapping_add(index);
            if slot < 8 {
                expected_teams[slot] = match (variant as u8, index % 2) {
                    (0x10, 0) | (0x11, 1) => 1,
                    (0x10, 1) | (0x11, 0) => 2,
                    _ => unreachable!(),
                };
            }
        }
        if variant as u8 == 0x10 {
            for index in 0..2 {
                let slot = usize::from(local_slots).wrapping_add(index);
                if slot < 8 {
                    expected_teams[slot] = team_values[slot].wrapping_add(1);
                }
            }
        } else {
            expected_teams[0] = 1;
        }
        assert_eq!(
            &actual[0x24..0x34],
            &expected_teams,
            "REA emitted-index parity, variant {variant:#x}, seed {seed}; calls {calls:x?}"
        );
        // The lifted C's MOVSX/SETcc translation uses the overwritten register, not flags.
        // Exclude only its affected team bytes; all other record bytes remain differential.
        lifted[0x24..0x34].copy_from_slice(&actual[0x24..0x34]);
    }
    let differences: Vec<_> = actual
        .iter()
        .zip(lifted.iter())
        .enumerate()
        .filter_map(|(offset, (&actual, &lifted))| {
            (actual != lifted).then_some((offset, actual, lifted))
        })
        .collect();
    assert!(
        differences.is_empty(),
        "variant {variant:#x}, seed {seed}, local_slots {local_slots}, emitted {emitted_count}; differences {differences:?}; calls {calls:x?}"
    );
}

#[test]
fn t1_setup_roster_emission_matches_lifted_3125a0_for_3000_seeds() {
    let mut pair = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *pair;
    let mut rng = Rng(0x2371_0001);
    let mut cases = 0usize;
    for case in 0..3000 {
        clear(pair, POOL, POOL_LEN);
        clear(pair, STACK, FRAME);
        pair.log.clear();
        let mut setup = [SetupSlot::default(); modes::ROSTER_SLOTS];
        for (index, slot) in setup.iter_mut().enumerate() {
            let id = [-1i16, 10, 11, 36, 99, 0x5d, 0, 1, 2][rng.below(9) as usize];
            let secondary = rng.next() as u8;
            *slot = SetupSlot {
                participant_id: id,
                conditional_source: secondary,
            };
            let address = SETUP + 4 + index as u32 * 8;
            pair.put32(
                address,
                id as u16 as u32 | (u32::from(rng.next() as u16) << 16),
            );
            pair.put32(
                address + 4,
                (rng.next() & 0x0000_ffff) | (u32::from(secondary) << 16),
            );
        }
        let maximum = [-2i32, -1, 0, 1, 2, 5, 7, 10, 15][rng.below(9) as usize];
        // Use deterministic service answers derived from each actual call's ID argument.
        // These authored service values are FABRICATED; the lifted function body is original.
        let mut helper_script = Vec::new();
        for slot in &setup {
            let emitted_so_far = helper_script.len() / 2;
            if slot.participant_id == -1 || emitted_so_far >= maximum.max(0) as usize {
                continue;
            }
            helper_script.push([
                u32::from(matches!(slot.participant_id, 10 | 11 | 36 | 99)),
                0,
                0,
            ]);
            helper_script.push([u32::from(test_derived(slot.participant_id)), 0, 0]);
        }
        let args = [maximum as u32, IDS, CONDITIONAL, DERIVED];
        for (index, value) in args.iter().enumerate() {
            pair.put32(STACK + 4 + index as u32 * 4, *value);
        }
        pair.put8(IDS, 0x0c);
        for index in 0..modes::PLAYER_SLOTS {
            pair.put8(CONDITIONAL + index as u32, 0xff);
        }
        let rows: Vec<u32> = helper_script.iter().flatten().copied().collect();
        unsafe {
            md_script(rows.as_ptr(), helper_script.len() as u32);
            md_st0(std::ptr::null(), 0);
            md_writes(std::ptr::null(), 0);
        }
        let mut regs = [SETUP, 0, 0, 0, 0, 0, 0, STACK];
        assert_eq!(unsafe { md_run(0x3125a0, regs.as_mut_ptr()) }, 1);

        let mut metadata = Meta { trace: vec![] };
        let expected = modes::emit_setup_roster(&setup, maximum, &mut metadata);
        assert_eq!(
            regs[0],
            u32::from(expected.emitted_count),
            "lifted return, seed {case}"
        );
        for index in 0..modes::PLAYER_SLOTS {
            assert_eq!(
                pair.get(false, IDS + index as u32),
                expected.ids[index],
                "id slot {index}, seed {case}"
            );
            assert_eq!(
                pair.get(false, CONDITIONAL + index as u32),
                expected.conditional[index],
                "conditional slot {index}, seed {case}"
            );
            assert_eq!(
                pair.get(false, DERIVED + index as u32),
                expected.derived[index],
                "derived slot {index}, seed {case}"
            );
        }
        let original_calls: Vec<[u32; 4]> = (0..unsafe { md_trace_len() })
            .map(|index| {
                let row = unsafe { *(md_trace_entry(index) as *const [u32; 16]) };
                [row[0], row[3], 0, 0]
            })
            .collect();
        assert_eq!(metadata.trace, original_calls, "lifted calls, seed {case}");
        cases += 1;
    }
    assert_eq!(cases, 3000);
}

#[test]
fn t1_launch_handoff_matches_lifted_313a80_for_3000_seeds() {
    let mut pair = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *pair;
    let mut rng = Rng(0x2371_0002);
    let variants = [2u32, 3, 4, 5, 6, 7, 9, 12, 15, 16, 17, 0x13];
    let mut case = 0usize;
    while case < 3000 {
        clear(pair, POOL, POOL_LEN);
        clear(pair, STACK, FRAME);
        pair.log.clear();
        let variant = variants[rng.below(variants.len() as u32) as usize];
        let option_bits = rng.below(4) as u8;
        let local_players = rng.below(5) as u8;
        let setup_count = 2 + rng.below(6) as u8;
        let setup_source_count = [-2i8, -1, 0, 1, 2, 5, 7, 10]
            [rng.below(8) as usize];
        let duration = rng.next();
        let draw_a = rng.next();
        let draw_b = rng.next();

        pair.put8(0x783cbc, 0);
        pair.put32(0x52d1d8, variant);
        pair.put8(0x52d1d8 + 0x18, option_bits);
        pair.put8(0x4d1f78 + variant * 0x20 + 4, game_type(variant));
        // Variant 0x09 copies this measured policy scalar into launch +0x8c. The
        // fixture owns that global input too; zero from an uninitialized pool is
        // not the documented original-preset value.
        pair.put32(0x475c78, 1.0f32.to_bits());
        let editor = EDITOR;
        pair.put32(0x7844a8, editor);
        pair.put32(editor + 0x25c, rng.next());
        pair.put8(0x762dbc, local_players);
        pair.put8(0x7de400, setup_count);
        pair.put32(0x7de460, duration);
        // The launch caller consumes either a null setup source or an owned-table pointer
        // returned by its source resolver. Exercise both documented cases.
        let has_setup_source = variant != 0 && variant != 2;
        pair.put32(0x784024, if has_setup_source { 2 } else { 0 });
        pair.put8(SETUP, setup_source_count as u8);
        pair.put32(0x784014, 0x0090_5000);
        let mut team_values = [0u8; modes::PLAYER_SLOTS];
        for (slot, value) in team_values.iter_mut().enumerate() {
            let raw = rng.next();
            *value = raw as u8;
            pair.put32(0x784034 + slot as u32 * 4, raw);
        }

        let mut setup = [SetupSlot::default(); modes::ROSTER_SLOTS];
        for (index, slot) in setup.iter_mut().enumerate() {
            let id = [-1i16, 10, 11, 36, 99, 0x5d, 0, 1][rng.below(8) as usize];
            let secondary = rng.next() as u8;
            *slot = SetupSlot {
                participant_id: id,
                conditional_source: secondary,
            };
            let address = SETUP + 4 + index as u32 * 8;
            pair.put32(
                address,
                id as u16 as u32 | (u32::from(rng.next() as u16) << 16),
            );
            pair.put32(
                address + 4,
                (rng.next() & 0xffff) | (u32::from(secondary) << 16),
            );
        }
        let local_slots = if option_bits & 0x02 == 0 {
            local_players.wrapping_add(1)
        } else {
            setup_count
        };
        let mut metadata = Meta { trace: vec![] };
        let setup_roster = modes::emit_setup_roster(
            &setup,
            i32::from(setup_source_count.min(7)),
            &mut metadata,
        );

        // The typed roster owns sixteen bytes and does not reproduce the original's writes
        // beyond that record. Variant 0x13 appends enough 0x5d IDs to reach three classified
        // entries; keep this differential in the valid owned-record domain so an original
        // overrun into adjacent launch fields is not mistaken for expected native behavior.
        if variant == 0x13 {
            let classified = (0..usize::from(setup_roster.emitted_count))
                .filter(|index| {
                    let slot = usize::from(local_slots).wrapping_add(*index);
                    slot < modes::PLAYER_SLOTS
                        && modes::is_special_participant_id(setup_roster.ids[slot])
                })
                .count();
            let final_count = usize::from(setup_roster.emitted_count)
                + 3usize.saturating_sub(classified);
            if usize::from(local_slots) + final_count > modes::PLAYER_SLOTS {
                continue;
            }
        }

        // The original switch indexes these two exact tables. Seed only its selected entry,
        // retaining the switch target used by the original's mode branch.
        let switch_index = variant.wrapping_sub(4) as u8;
        let target_index = match variant {
            4 | 5 | 15 => 4,
            7 => 1,
            9 => 3,
            12 => 2,
            16 => 5,
            17 => 6,
            0x13 => 0,
            _ => 7,
        };
        pair.put8(0x313ef0 + u32::from(switch_index), target_index);
        pair.put32(
            0x313ed0 + u32::from(target_index) * 4,
            TABLE_TARGETS[target_index as usize],
        );

        // The helper receives fabricated metadata-service answers; its own body and the launch
        // caller are compiled from the pinned lifted originals.
        let mut script = vec![
            [if has_setup_source { SETUP } else { 0 }, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
        ];
        if has_setup_source {
            let mut emitted = 0usize;
            for slot in &setup {
                if slot.participant_id == -1 || emitted >= usize::from(setup_roster.emitted_count) {
                    continue;
                }
                script.push([
                    u32::from(matches!(slot.participant_id, 10 | 11 | 36 | 99)),
                    0,
                    0,
                ]);
                script.push([u32::from(test_derived(slot.participant_id)), 0, 0]);
                emitted += 1;
            }
        }
        if variant == 0x13 {
            for index in 0..usize::from(setup_roster.emitted_count) {
                let slot = usize::from(local_slots).wrapping_add(index);
                let id = setup_roster.ids.get(slot).copied().unwrap_or(0);
                script.push([u32::from(modes::is_special_participant_id(id)), 0, 0]);
            }
        }
        if variant == 16 {
            script.push([draw_a, 0, 0]);
            script.push([draw_b, 0, 0]);
        }
        script.push([0, 0, 0]);
        let flat: Vec<u32> = script.iter().flatten().copied().collect();
        unsafe {
            md_script(flat.as_ptr(), script.len() as u32);
            md_st0(std::ptr::null(), 0);
            md_writes(std::ptr::null(), 0);
        }
        let mut regs = [0; 8];
        regs[7] = STACK;
        assert_eq!(
            unsafe { md_run(0x313a80, regs.as_mut_ptr()) },
            1,
            "seed {case}"
        );
        let original = original_launch_record(pair);

        let settings = ModeSettings {
            editor_kind: EditorKind(rng.next()),
            theme: ThemeId(rng.next()),
            preview: PreviewSelection {
                row: PreviewRow(rng.next() as u8),
                variant: LaunchVariant(variant),
                game_type: GameType(game_type(variant)),
            },
            row_options: option_bits,
            duration,
        };
        let mode = match launch_mode(&mut rng) {
            MapMode::Deathmatch => ModeConfiguration::Deathmatch(settings),
            MapMode::Bag => ModeConfiguration::Bag(settings),
            MapMode::Escort => ModeConfiguration::Escort(settings),
            MapMode::Assault => ModeConfiguration::Assault(AssaultModeConfiguration {
                settings,
                phases: AssaultPhases::default(),
            }),
        };
        let input = LaunchInput {
            mode,
            local_player_count: local_players,
            setup_count: setup_count as i8,
            launch_argument_missing: true,
            team_values,
            random_draws: [draw_a, draw_b],
        };
        let participants = has_setup_source.then_some(&setup_roster);
        let actual = LaunchConfiguration::build(&input, participants).to_original_bytes();
        let calls: Vec<[u32; 4]> = (0..unsafe { md_trace_len() })
            .map(|index| {
                let row = unsafe { *(md_trace_entry(index) as *const [u32; 16]) };
                [row[0], row[3], row[4], row[5]]
            })
            .collect();
        let emitted = if has_setup_source {
            setup_roster.emitted_count
        } else {
            0
        };
        assert_launch_matches_lift_except_known_lifted_parity(
            actual,
            original,
            variant,
            local_slots,
            emitted,
            &team_values,
            case,
            &calls,
        );
        if variant == 16 {
            let config = LaunchConfiguration::build(&input, participants);
            assert_eq!(config.rng_values, [draw_a % 6, draw_b % 6]);
        }
        case += 1;
    }
}

#[test]
fn native_mode_namespaces_and_escort_phase_selection_stay_typed() {
    assert_eq!(
        modes::toggle_phase_selection(2),
        u32::MAX,
        "wrapping 1 - selected word preserves original phase toggle semantics"
    );
    let phase = modes::AssaultPhaseSelection {
        kind: AssaultPhaseKind::Escort,
        node_id: 7,
        selected_word: 2,
    };
    assert_eq!(phase.kind, AssaultPhaseKind::Escort);
    let roster = SetupRoster::from_wire_entries(
        RosterSource::ProfileCustom {
            selector: 12,
            profile_slot: 2,
            epoch: 9,
        },
        &[],
    )
    .unwrap();
    assert_eq!(roster.raw_count, 0);
}
