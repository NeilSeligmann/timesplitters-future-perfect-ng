//! T1703 Tier 1 differential checks: the readable setup copy and typed mission result reducer
//! are compared with the retained lifted-C-to-Rust handlers on bounded synthetic states.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tsfp_ports::abi::{ExtRegisters, GuestCall};
use tsfp_ports::c2r::{
    data_missions, gameplay_player, gamestate_wired, leaf_wired, network_session, t2049_w7_001,
    ui_menu,
};
use tsfp_ports::mem::GuestAccess;
use tsfp_ports::memory::GuestMemory;
use tsfp_ports::state_wire::story_flow;

#[derive(Clone, Default)]
struct Memory(BTreeMap<u32, u8>);

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

#[derive(Clone, Default)]
struct SharedMemory(Arc<Mutex<BTreeMap<u32, u8>>>);

impl GuestMemory for SharedMemory {
    fn read8(&self, address: u32) -> u8 {
        self.0.lock().unwrap().get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.lock().unwrap().insert(address, value);
    }
}

#[derive(Default)]
struct StorySelectorCalls {
    memory: SharedMemory,
    selector_lookups: usize,
}

impl GuestCall for StorySelectorCalls {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        assert_eq!(va, 0x6a810, "Story cache should call the selector scan");
        self.selector_lookups += 1;
        // `RegsExt::call` enters the guest trampoline at its return slot. Preserve the
        // cdecl frame expected by the retained helper: its first argument is at esp + 4.
        regs.esp = regs.esp.wrapping_sub(4);
        ui_menu::game_table_515ffc_entry_field_0x1c_by_mode_arg(&mut self.memory.clone(), regs);
        regs.esp = regs.esp.wrapping_add(4); // callee RET
    }
}

#[derive(Default)]
struct NoCalls;

impl GuestCall for NoCalls {
    fn call(&mut self, _va: u32, regs: &mut ExtRegisters) {
        regs.eax = 0;
    }
}

fn put32(m: &mut Memory, address: u32, value: u32) {
    m.write32(address, value);
}
fn regs(stack: u32) -> ExtRegisters {
    let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
    regs.esp = stack;
    regs
}

#[test]
fn staged_setup_lane_copy_matches_lifted_handler_tier1() {
    const STACK: u32 = 0x800000;
    const SOURCE: u32 = 0x810000;
    for seed in 0..2048u32 {
        let mut initial = Memory::default();
        for i in 0..40u32 {
            put32(
                &mut initial,
                SOURCE + i * 4,
                seed.wrapping_mul(0x9e37_79b9) ^ i.wrapping_mul(0x10203),
            );
        }
        initial.write8(SOURCE + 0x54, 0x67);
        put32(&mut initial, 0x79094c, 0x67);
        put32(&mut initial, STACK, 0xdeadbeef);
        put32(&mut initial, STACK + 4, SOURCE);

        let mut lifted = initial.clone();
        let mut wired = initial;
        let mut lifted_regs = regs(STACK);
        let mut wired_regs = regs(STACK);
        assert!(data_missions::run(
            0x658d0,
            &mut lifted,
            &mut NoCalls,
            &mut lifted_regs
        ));
        assert!(gamestate_wired::run(
            0x658d0,
            &mut wired,
            &mut NoCalls,
            &mut wired_regs
        ));
        assert_eq!(wired.0, lifted.0, "seed {seed}");
        assert_eq!(wired_regs.esp, lifted_regs.esp, "seed {seed}");
    }
}

#[test]
fn typed_story_start_projection_matches_lifted_activation_tier1() {
    use story_flow::{
        ACTIVE_SETUP, MATCH_PHASE, SESSION_RESULT, SessionSetup, StoryFlow, StoryStep,
    };

    const STACK: u32 = 0x800000;
    const LEVEL_IDS: [u8; 2] = [2, 4];
    const PLAYER_COUNTS: [u8; 5] = [0, 1, 2, 4, 0xff];
    const ITEM_LIST_CAPACITY: usize = 0xa0 - 4;

    for seed in 0..512u32 {
        let mut words = [0u32; 40];
        for (index, word) in words.iter_mut().enumerate() {
            *word = seed
                .wrapping_mul(0x9e37_79b9)
                .rotate_left((index as u32) & 31)
                ^ (index as u32).wrapping_mul(0x10203);
        }
        let level_id = LEVEL_IDS[(seed as usize) % LEVEL_IDS.len()];
        let players = PLAYER_COUNTS[((seed / 3) as usize) % PLAYER_COUNTS.len()];
        let mut setup = SessionSetup::from_words(words);
        setup.set_byte(0, players);
        // Cover every list length that fits in the typed record. Repeated IDs exercise the
        // original idempotent activation write without asking this projection to own the
        // adjacent memory touched by out-of-record IDs.
        let item_count = (seed as usize) % (ITEM_LIST_CAPACITY + 1);
        let mut item_ids = [0u8; ITEM_LIST_CAPACITY];
        setup.set_byte(3, item_count as u8);
        for (index, item_id) in item_ids.iter_mut().take(item_count).enumerate() {
            *item_id = ((seed + index as u32 * 13) % 41) as u8;
            let item_id = *item_id;
            setup.set_byte(4 + index, item_id);
            setup.set_byte(0x77 + usize::from(item_id), 0);
        }
        let item_ids = &item_ids[..item_count];
        setup.set_byte(0x54, level_id);
        setup.set_byte(0x55, 10);

        let mut lifted = Memory::default();
        setup.store(&mut lifted, 0x7de340);
        put32(&mut lifted, 0x475c78, (seed as f32 / 7.0).to_bits());
        let initial_result_flags = match seed % 4 {
            0 => 0x8000,
            1 => 0x1_0000,
            2 => 0x1_8000,
            _ => 0xa500_0000 | 0x1_8000,
        };
        put32(&mut lifted, SESSION_RESULT, initial_result_flags);

        let mut flow = StoryFlow::default();
        flow.ordinal = story_flow::CampaignOrdinal::new((seed % 13) as u8).unwrap();
        flow.result_flags = initial_result_flags;
        flow.stage(setup);
        flow.start_level();

        let mut r = regs(STACK);
        assert!(network_session::run(
            0x670c0,
            &mut lifted,
            &mut NoCalls,
            &mut r
        ));
        let active = SessionSetup::load(&lifted, ACTIVE_SETUP);
        assert_eq!(
            active.byte(0),
            flow.active.byte(0),
            "seed {seed} players byte"
        );
        assert_eq!(
            active.byte(0x54),
            flow.active.byte(0x54),
            "seed {seed} level byte"
        );
        assert_eq!(
            active.byte(0x55),
            flow.active.byte(0x55),
            "seed {seed} mode byte"
        );
        for offset in [0x5c, 0x78, 0x7c, 0x80, 0x84, 0x88] {
            let word = offset / 4;
            let mut expected_word = 0u32;
            for item_id in item_ids.iter().copied() {
                let active_offset = 0x77 + usize::from(item_id);
                if offset != 0x88 && (offset..offset + 4).contains(&active_offset) {
                    expected_word |= 1 << ((active_offset - offset) * 8);
                }
            }
            assert_eq!(
                active.words[word], expected_word,
                "seed {seed} C2R reset at active setup +{offset:#x}"
            );
            assert_eq!(
                flow.active.words[word], expected_word,
                "seed {seed} Rust reset at active setup +{offset:#x}"
            );
        }
        for item_id in item_ids.iter().copied() {
            let offset = 0x77 + usize::from(item_id);
            let expected = u8::from(!(0x88..0x8c).contains(&offset));
            assert_eq!(
                active.byte(offset),
                expected,
                "seed {seed} lifted item {item_id}"
            );
            assert_eq!(
                flow.active.byte(offset),
                expected,
                "seed {seed} Rust item {item_id}"
            );
        }
        assert_eq!(
            lifted.read32(0x79094c) as i32,
            flow.level_id,
            "seed {seed} level"
        );
        assert_eq!(
            lifted.read32(0x790950) as i32,
            flow.player_count,
            "seed {seed} players"
        );
        assert_eq!(lifted.read32(MATCH_PHASE), flow.phase, "seed {seed} phase");
        assert_eq!(
            lifted.read32(SESSION_RESULT),
            flow.result_flags,
            "seed {seed} result"
        );
        assert_eq!(flow.result_flags, 0, "seed {seed} stale result flags");
        assert_eq!(
            flow.step,
            StoryStep::Gameplay {
                ordinal: story_flow::CampaignOrdinal::new((seed % 13) as u8).unwrap(),
                level_id: level_id as i8 as i32,
            },
            "seed {seed} step"
        );
    }
}

#[test]
fn story_controller_selection_matches_lifted_mode_table_tier1() {
    use story_flow::{MODE_STORY, StoryController, story_controller};

    const STACK: u32 = 0x800000;
    const TABLE: u32 = 0x515ffc;
    const SELECTOR_STRIDE: u32 = 0x80;

    let controllers = [
        StoryController {
            level_id: 2,
            mode_record: 0x0050_8390,
            preload: 0x0021_8ab0,
            initialize: 0x0021_ad50,
            tick: 0x0021_ad00,
        },
        StoryController {
            level_id: 4,
            mode_record: 0x004f_f470,
            preload: 0x001e_1c50,
            initialize: 0x001e_9cc0,
            tick: 0x001e_40d0,
        },
    ];

    let mut lifted = Memory::default();
    for (index, controller) in controllers.iter().enumerate() {
        let selector = TABLE + index as u32 * SELECTOR_STRIDE;
        put32(&mut lifted, selector, 1);
        put32(&mut lifted, selector + 4, controller.level_id as u32);
        put32(&mut lifted, selector + 0x20, controller.mode_record);
        for (offset, callback) in [
            (0x104, controller.preload),
            (0x108, controller.initialize),
            (0x110, controller.tick),
        ] {
            put32(&mut lifted, controller.mode_record + offset, callback);
        }
    }
    // The zeroed third selector record terminates the measured stride-0x80 scan.

    for level_id in [2, 4, 24] {
        put32(&mut lifted, STACK + 4, level_id as u32);
        let mut r = regs(STACK);
        ui_menu::game_table_515ffc_entry_field_0x1c_by_mode_arg(&mut lifted, &mut r);

        let rust = story_controller(level_id, MODE_STORY);
        match rust {
            Some(controller) => {
                assert_eq!(r.eax, controller.mode_record, "level {level_id} record");
                for (offset, callback) in [
                    (0x104, controller.preload),
                    (0x108, controller.initialize),
                    (0x110, controller.tick),
                ] {
                    assert_eq!(
                        lifted.read32(r.eax + offset),
                        callback,
                        "level {level_id} callback at +{offset:#x}"
                    );
                }
            }
            None => assert_eq!(r.eax, 0, "unknown Story level {level_id}"),
        }
    }
}

#[test]
fn story_campaign_level_lookup_matches_c2r_64d60_tier1() {
    use story_flow::{
        CAMPAIGN_LEVEL_ROW_STRIDE_BYTES, CampaignLevelTable, MODE_STORY, StoryContinuation,
        StoryContinuationGate, StoryFlow, StoryStep,
    };

    const STACK: u32 = 0x800000;
    const LEVEL_TABLE: u32 = 0x4d1ac6;
    const ROW_STRIDE: u32 = 0x44;
    const SAVED_ESI: u32 = 0x1357_9bdf;

    for seed in 0..2048u32 {
        let mut level_ids = [0i16; 13];
        for (index, level_id) in level_ids.iter_mut().enumerate() {
            *level_id = seed
                .wrapping_mul(0x9e37_79b9)
                .wrapping_add((index as u32).wrapping_mul(0x10203)) as i8
                as i16;
        }
        if seed % 3 == 0 {
            level_ids[12] = level_ids[2];
        }
        let required = (level_ids.len() - 1) * CAMPAIGN_LEVEL_ROW_STRIDE_BYTES + 2;
        let mut table_rows = vec![0xa5; required];
        for (index, value) in level_ids.iter().copied().enumerate() {
            let offset = index * CAMPAIGN_LEVEL_ROW_STRIDE_BYTES;
            table_rows[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        let rust_table = CampaignLevelTable::decode_runtime_rows(&table_rows).unwrap();

        let level_id = match seed % 4 {
            0 => i32::from(level_ids[(seed as usize / 4) % level_ids.len()]),
            1 => i32::from(level_ids[12]),
            2 => i32::from(
                (-128i16..=127)
                    .find(|candidate| !level_ids.contains(candidate))
                    .unwrap(),
            ),
            _ => i32::from(seed as i8),
        };
        let mut memory = Memory::default();
        memory.write32(STACK, 0xfeed_face);
        memory.write32(STACK + 4, level_id as u32);
        for (index, value) in level_ids.iter().copied().enumerate() {
            memory.write16(LEVEL_TABLE + index as u32 * ROW_STRIDE, value as u16);
        }

        let mut registers = regs(STACK);
        registers.esi = SAVED_ESI;
        assert!(t2049_w7_001::run(
            0x64d60,
            &mut memory,
            &mut NoCalls,
            &mut registers
        ));

        let rust = rust_table
            .ordinal_for_level_id(level_id)
            .map(|ordinal| ordinal.get() as u32)
            .unwrap_or(u32::MAX);
        assert_eq!(rust, registers.eax, "seed {seed}, level {level_id}");
        assert_eq!(registers.esi, SAVED_ESI, "seed {seed} ESI preservation");
        assert_eq!(registers.esp, STACK, "seed {seed} stack restoration");

        let mut next_gameplay = story_flow::SessionSetup::default();
        next_gameplay.set_byte(0x54, level_id as i8 as u8);
        next_gameplay.set_byte(0x55, MODE_STORY);
        let route = StoryContinuation::from_runtime_levels(rust_table, None, next_gameplay);
        let mut flow = StoryFlow::default();
        flow.step = StoryStep::ResultsFrontend;
        flow.staging.set_byte(0x54, 0x6a);
        if rust == u32::MAX {
            assert!(route.is_none(), "seed {seed}, unmapped level {level_id}");
        } else {
            let gate = StoryContinuationGate {
                descriptor_ready: true,
                attract_mode: false,
                session_phase: 1,
                menu_kind: 4,
                unlock_notice_pending: false,
            };
            assert!(flow.continue_after_results(gate, route.unwrap()));
            assert_eq!(flow.ordinal.get() as u32, registers.eax, "seed {seed}");
        }
    }
}

#[test]
fn story_controller_cache_wrapper_matches_lifted_selector_tier1() {
    use story_flow::{MODE_STORY, story_controller};

    const STACK: u32 = 0x800000;
    const TABLE: u32 = 0x515ffc;
    const SELECTOR_STRIDE: u32 = 0x80;
    const STORY_MODE: u8 = 10;

    let controllers = [
        (2u32, 0x0050_8390, 0x0021_8ab0, 0x0021_ad50, 0x0021_ad00),
        (4u32, 0x004f_f470, 0x001e_1c50, 0x001e_9cc0, 0x001e_40d0),
    ];

    let mut memory = SharedMemory::default();
    memory.write8(0x7de455, STORY_MODE);
    for (index, (level, record, preload, initialize, tick)) in controllers.into_iter().enumerate() {
        let selector = TABLE + index as u32 * SELECTOR_STRIDE;
        memory.write32(selector, 1);
        memory.write32(selector + 4, level);
        memory.write32(selector + 0x20, record);
        memory.write32(record + 0x104, preload);
        memory.write32(record + 0x108, initialize);
        memory.write32(record + 0x110, tick);
    }

    let mut calls = StorySelectorCalls {
        memory: memory.clone(),
        selector_lookups: 0,
    };
    let mut previous_key = None;
    let mut expected_lookups = 0;
    let levels = [2u32, 2, 4, 24, 24, 2, 4, 4, 24, 2, 24, 4];

    for seed in 0..512u32 {
        let level_id = levels[(seed as usize) % levels.len()];
        if seed % 37 == 0 {
            // A stale cache key must force the lifted selector scan again.
            memory.write32(0x4d2ce4, level_id.wrapping_add(0x1000));
        }
        memory.write32(STACK + 4, level_id);

        let mut regs = regs(STACK);
        leaf_wired::game_table_515ffc_entry_ptr_cached_by_mode_arg_and_byte_7de455(
            &mut memory,
            &mut calls,
            &mut regs,
        );

        let expected = story_controller(level_id as i32, MODE_STORY)
            .map(|controller| controller.mode_record)
            .unwrap_or(0);
        assert_eq!(regs.eax, expected, "seed {seed}, level {level_id}");
        assert_eq!(memory.read32(0x4d2ce4), level_id, "seed {seed} memo arg");
        assert_eq!(
            memory.read32(0x4d2ce8),
            STORY_MODE as u32,
            "seed {seed} memo mode"
        );
        assert_eq!(
            memory.read32(0x6f32f0),
            expected,
            "seed {seed} cached record"
        );

        let forced_miss = seed % 37 == 0;
        let cache_miss = forced_miss || previous_key != Some(level_id);
        if cache_miss {
            expected_lookups += 1;
        }
        assert_eq!(
            calls.selector_lookups, expected_lookups,
            "seed {seed} cache path"
        );
        previous_key = Some(level_id);
    }
}

fn lifted_mission_result_with_state(
    initial_flags: u32,
    phase: u32,
    transition_timer: u32,
    player_state: u32,
    slots: &[(u8, u8, bool)],
    frame: u32,
) -> (u32, u32, u32, bool) {
    const STACK: u32 = 0x800000;
    const PLAYER: u32 = 0x30000;
    const INDEXES: u32 = 0x31000;
    const SLOT_TABLE: u32 = 0x32000;
    let mut m = Memory::default();
    put32(&mut m, 0x4e7944, 1); // nonzero selects the ordinary slot-update arm
    put32(&mut m, 0x79094c, 2);
    m.write8(0x7de455, 10); // Story
    put32(&mut m, 0x790950, 1);
    put32(&mut m, 0x7b0c48, PLAYER);
    put32(&mut m, PLAYER + 0x114, player_state);
    put32(&mut m, 0x75c134, 1); // positive outer objective gate
    put32(&mut m, 0x78cec8, slots.len() as u32);
    put32(&mut m, 0x75c14c, INDEXES);
    put32(&mut m, 0x78cecc, SLOT_TABLE);
    put32(&mut m, 0x7de318, frame);
    put32(&mut m, 0x7de45c, initial_flags);
    put32(&mut m, 0x7de30c, phase);
    put32(&mut m, 0x7de300, transition_timer);
    for (i, (status, flags, included)) in slots.iter().copied().enumerate() {
        put32(&mut m, INDEXES + i as u32 * 4, i as u32);
        let slot = SLOT_TABLE + i as u32 * 0x4c;
        m.write8(slot + 8, u8::from(!included));
        m.write8(slot + 9, flags);
        m.write8(slot + 0xb, status);
    }
    let mut calls = CallLog::default();
    let mut r = regs(STACK);
    assert!(data_missions::run(0x228870, &mut m, &mut calls, &mut r,));
    (
        m.read32(0x7de45c),
        m.read32(0x7de30c),
        m.read32(0x7de300),
        calls.0.contains(&0x67450),
    )
}

fn lifted_mission_result(player_state: u32, slots: &[(u8, u8, bool)], frame: u32) -> (u32, bool) {
    let (flags, _, _, request_end) =
        lifted_mission_result_with_state(0, 0, 0, player_state, slots, frame);
    (flags, request_end)
}

#[test]
fn all_players_down_and_empty_aggregate_match_lifted_result_writer_tier1() {
    use story_flow::{
        MODE_STORY, MissionResultEvent, ObjectiveState, RESULT_FAILURE, RESULT_SUCCESS,
        evaluate_mission,
    };

    let failure = evaluate_mission(MODE_STORY, 1, true, 0, 1, 1, []);
    assert_eq!(failure.event, Some(MissionResultEvent::AllPlayersDown));
    let (failure_flags, failure_requests_end) = lifted_mission_result(0x10, &[], 1);
    assert_eq!(failure.result_flags, failure_flags);
    assert_eq!(failure.request_end, failure_requests_end);
    assert!(failure.request_end);

    let success = evaluate_mission(MODE_STORY, 1, false, 0, 1, 1, []);
    assert_eq!(success.event, Some(MissionResultEvent::Complete));
    let (success_flags, success_requests_end) = lifted_mission_result(0, &[], 1);
    assert_eq!(success.result_flags, success_flags);
    assert_eq!(success.request_end, success_requests_end);
    assert!(success.request_end);
    assert_eq!(success.result_flags, RESULT_SUCCESS);
    assert_eq!(failure.result_flags, RESULT_FAILURE);

    let objective_failure = evaluate_mission(
        MODE_STORY,
        1,
        false,
        0,
        1,
        1,
        [ObjectiveState {
            included: true,
            status: 4,
            flags: 0,
        }],
    );
    assert_eq!(
        objective_failure.event,
        Some(MissionResultEvent::ObjectivesFailed)
    );
    assert_eq!(
        objective_failure.result_flags,
        lifted_mission_result(0, &[(4, 0, true)], 1).0
    );
    assert_eq!(
        objective_failure.request_end,
        lifted_mission_result(0, &[(4, 0, true)], 1).1
    );
    assert!(objective_failure.request_end);

    for seed in 0..4096u32 {
        let count = (seed % 6) as usize;
        let mut random = seed.wrapping_add(1);
        let mut source = Vec::with_capacity(count);
        let mut typed = Vec::with_capacity(count);
        for _ in 0..count {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            let status = [2, 4, 5][(random as usize) % 3];
            let flags = if random & 0x100 != 0 { 2 } else { 0 };
            let included = random & 0x200 != 0;
            source.push((status, flags, included));
            typed.push(ObjectiveState {
                included,
                status,
                flags,
            });
        }
        let frame = seed % 23;
        let result = evaluate_mission(MODE_STORY, 1, false, 0, 1, frame, typed);
        let (lifted_flags, lifted_requests_end) = lifted_mission_result(0, &source, frame);
        assert_eq!(result.result_flags, lifted_flags, "seed {seed} flags");
        assert_eq!(
            result.request_end, lifted_requests_end,
            "seed {seed} end-helper call"
        );
    }
}

#[test]
fn story_result_reducer_preserves_phase_and_timer_against_lifted_aggregate_tier1() {
    use story_flow::{
        MODE_STORY, ObjectiveState, StoryFlow, StoryMatchPhase, StoryResultInput, StoryResultState,
        reduce_story_result,
    };

    for seed in 0..2048u32 {
        let initial_flags = seed.wrapping_mul(0x9e37_79b9).rotate_left(seed % 31);
        let phase = seed.wrapping_mul(7) % 12;
        let transition_timer = seed.wrapping_mul(0x10203);
        let player_state = if seed % 11 == 0 { 0x10 } else { seed & 3 };
        let count = (seed % 6) as usize;
        let mut random = seed.wrapping_add(1);
        let mut lifted_slots = Vec::with_capacity(count);
        let mut typed_slots = Vec::with_capacity(count);
        for _ in 0..count {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            let status = [2, 4, 5][(random as usize) % 3];
            let flags = if random & 0x100 != 0 { 2 } else { 0 };
            let included = random & 0x200 != 0;
            lifted_slots.push((status, flags, included));
            typed_slots.push(ObjectiveState {
                included,
                status,
                flags,
            });
        }
        let frame = seed % 23;
        let initial = StoryResultState {
            result_flags: initial_flags,
            phase: StoryMatchPhase::from_raw(phase),
            transition_timer,
        };
        let reduced = reduce_story_result(
            initial,
            StoryResultInput {
                mode_type: MODE_STORY,
                player_count: 1,
                all_players_down: player_state == 0x10,
                objective_counter: 1,
                frame,
                objectives: &typed_slots,
            },
        );
        let mut flow = StoryFlow::default();
        flow.apply_result_state(initial);
        flow.apply_result_state(reduced.state);
        assert_eq!(
            flow.result_state(),
            reduced.state,
            "seed {seed} flow projection"
        );
        let (flags, lifted_phase, lifted_timer, request_end) = lifted_mission_result_with_state(
            initial_flags,
            phase,
            transition_timer,
            player_state,
            &lifted_slots,
            frame,
        );
        assert_eq!(reduced.state.result_flags, flags, "seed {seed} flags");
        assert_eq!(reduced.state.phase.raw(), lifted_phase, "seed {seed} phase");
        assert_eq!(
            reduced.state.transition_timer, lifted_timer,
            "seed {seed} timer"
        );
        assert_eq!(reduced.request_end, request_end, "seed {seed} end request");
    }
}

#[test]
fn explicit_story_event_completion_matches_lifted_dispatch_tier1() {
    use story_flow::{
        STORY_EVENT_COMPLETION_TYPE, StoryMatchPhase, StoryResultState,
        reduce_story_event_completion,
    };

    const INSTANCE: u32 = 0x30000;
    const DEFINITION: u32 = 0x31000;
    const DISPATCH_TABLE: u32 = 0x23730c;
    const RESULT_WORD: u32 = 0x7de45c;

    for seed in 0..512u32 {
        let result_flags = seed.wrapping_mul(0x9e37_79b9).rotate_left(seed % 31);
        let state = StoryResultState {
            result_flags,
            phase: StoryMatchPhase::from_raw(seed % 12),
            transition_timer: seed.wrapping_mul(17),
        };
        let expected = reduce_story_event_completion(state, STORY_EVENT_COMPLETION_TYPE);

        let mut memory = Memory::default();
        put32(&mut memory, INSTANCE, DEFINITION);
        put32(&mut memory, DEFINITION + 8, STORY_EVENT_COMPLETION_TYPE);
        put32(
            &mut memory,
            DISPATCH_TABLE + STORY_EVENT_COMPLETION_TYPE * 4,
            0x236609,
        );
        put32(&mut memory, RESULT_WORD, result_flags);
        let mut calls = CallLog::default();
        let mut r = regs(0x800000);
        r.eax = INSTANCE;

        assert!(data_missions::run(
            0x236070,
            &mut memory,
            &mut calls,
            &mut r,
        ));
        assert_eq!(
            memory.read32(RESULT_WORD),
            expected.state.result_flags,
            "seed {seed} result flags"
        );
        assert_eq!(
            calls.0.contains(&0x67450),
            expected.request_end,
            "seed {seed} end-helper request"
        );
        assert!(expected.completed, "seed {seed} completion marker");
        assert_eq!(expected.state.phase, state.phase, "seed {seed} phase");
        assert_eq!(
            expected.state.transition_timer, state.transition_timer,
            "seed {seed} timer"
        );
    }
}

#[derive(Default)]
struct CallLog(Vec<u32>);

impl GuestCall for CallLog {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.0.push(va);
        regs.eax = 0;
    }
}

#[test]
fn typed_cinematic_release_matches_lifted_release_fields_tier1() {
    use story_flow::{
        CINEMATIC_CONTROL_FLAG, CameraInstance, CameraSequence, CameraSequenceTransition,
        CinematicCallback, CinematicPlayer, CinematicSession,
    };
    const STACK: u32 = 0x800000;
    const PLAYERS: u32 = 0x20000;
    const PLAYER_OBJECT: u32 = 0x21000;
    const PLAYER_CONTEXT: u32 = 0x22000;
    const SESSION: u32 = 0x30000;
    const CAMERA: CameraInstance = CameraInstance(0x4400);

    for seed in 0..256u32 {
        let mode = [0, 2, 7, 0x10, 0x100][(seed as usize) % 5];
        let flags = seed.wrapping_mul(0x10203) | 0x0002_0400;
        let selected_inventory = if seed % 3 == 0 { 0 } else { seed % 8 };
        let saved_inventory = if seed % 5 == 0 { 0 } else { (seed % 6) + 1 };
        let finish = if seed & 1 == 0 { seed + 0x5000 } else { 0 };
        let update = seed + 0x6000;

        let mut initial = Memory::default();
        put32(&mut initial, 0x790950, 1);
        put32(&mut initial, 0x7b0c48, PLAYERS);
        put32(&mut initial, PLAYERS + 0x14, PLAYER_OBJECT);
        put32(&mut initial, PLAYER_OBJECT + 0x7c, PLAYER_CONTEXT);
        put32(&mut initial, PLAYERS + 0x114, mode);
        put32(&mut initial, PLAYERS + 0x110, 0xfeed);
        put32(&mut initial, PLAYERS + 0x118, 0xbeef);
        put32(&mut initial, PLAYERS + 0x0c, flags);
        put32(&mut initial, PLAYER_CONTEXT + 0x94, selected_inventory);
        put32(&mut initial, PLAYER_CONTEXT + 0x9c, saved_inventory);
        put32(&mut initial, SESSION + 0x157c, update);
        put32(&mut initial, SESSION + 0x1580, finish);
        put32(
            &mut initial,
            0x7de45c,
            CINEMATIC_CONTROL_FLAG | (seed & 0xffff),
        );
        put32(&mut initial, STACK + 4, SESSION);

        let mut lifted = initial.clone();
        let mut lifted_regs = regs(STACK);
        let mut calls = CallLog::default();
        assert!(gameplay_player::run(
            0xd4be0,
            &mut lifted,
            &mut calls,
            &mut lifted_regs,
        ));

        let sequence = CameraSequence::new(CAMERA, 1, 0).unwrap();
        let mut session = CinematicSession {
            sequence,
            global_flags: CINEMATIC_CONTROL_FLAG | (seed & 0xffff),
            finish_callback: (finish != 0).then_some(CinematicCallback(finish)),
            update_callback: Some(CinematicCallback(update)),
        };
        let mut native_players = [CinematicPlayer {
            local: true,
            camera: Some(CAMERA),
            mode,
            previous_mode: 0xfeed,
            timer: 0xbeef,
            flags,
            selected_inventory,
            saved_inventory,
            ..CinematicPlayer::default()
        }];
        let mut invoked = Vec::new();
        assert_eq!(
            session.complete_segment(Some(CAMERA), false, &mut native_players, |id| {
                invoked.push(id.0)
            }),
            CameraSequenceTransition::Released { local_players: 1 },
            "seed {seed}"
        );
        assert_eq!(session.global_flags, lifted.read32(0x7de45c), "seed {seed}");
        assert_eq!(session.finish_callback, None, "seed {seed}");
        assert_eq!(session.update_callback, None, "seed {seed}");
        assert_eq!(
            invoked,
            if finish == 0 {
                Vec::new()
            } else {
                vec![finish]
            }
        );
        assert_eq!(
            native_players[0].mode,
            lifted.read32(PLAYERS + 0x114),
            "seed {seed} mode"
        );
        assert_eq!(
            native_players[0].previous_mode,
            lifted.read32(PLAYERS + 0x110),
            "seed {seed} previous mode"
        );
        assert_eq!(
            native_players[0].timer,
            lifted.read32(PLAYERS + 0x118),
            "seed {seed} timer"
        );
        assert_eq!(
            native_players[0].flags,
            lifted.read32(PLAYERS + 0x0c),
            "seed {seed} flags"
        );
        assert_eq!(
            native_players[0].selected_inventory,
            lifted.read32(PLAYER_CONTEXT + 0x94),
            "seed {seed} inventory"
        );
        assert_eq!(
            calls.0.first().copied(),
            if finish == 0 {
                Some(0x2488d0)
            } else {
                Some(finish)
            },
            "seed {seed} callback order"
        );
    }
}

#[test]
fn opening_blend_gate_clear_matches_lifted_cleanup_boundary_tier1() {
    use story_flow::InputGateState;

    const STACK: u32 = 0x800000;
    for seed in 0..256u32 {
        let initial_gate = seed.wrapping_mul(0x9e37_79b9) ^ seed.rotate_left(11);
        let mut lifted = Memory::default();
        put32(&mut lifted, 0x6f11bc, initial_gate);
        put32(&mut lifted, 0x78cd54, 0);
        put32(&mut lifted, 0x78cd58, 0);
        put32(&mut lifted, 0x790950, 0);

        let mut calls = CallLog::default();
        let mut lifted_regs = regs(STACK);
        assert!(gamestate_wired::run(
            0x23efe0,
            &mut lifted,
            &mut calls,
            &mut lifted_regs,
        ));
        let lifted_gate = if calls.0.contains(&0x665b0) {
            0
        } else {
            initial_gate
        };

        let mut native = InputGateState {
            opening_movie_locked: initial_gate != 0,
        };
        native.finish_opening_blend();
        assert_eq!(
            u32::from(native.opening_movie_locked),
            lifted_gate,
            "seed {seed}; direct gate-clear call boundary only"
        );
    }
}
