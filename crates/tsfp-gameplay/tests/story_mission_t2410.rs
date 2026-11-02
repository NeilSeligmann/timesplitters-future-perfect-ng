use tsfp_gameplay::modes::story_sequencer::ModeBlock;
use tsfp_gameplay::story_events::record::Link;
use tsfp_gameplay::story_events::setu::SetuAssetSelector;
use tsfp_gameplay::story_events::slots::{CampaignOrdinal, MissionEvent, ObjectiveSlotId};
use tsfp_gameplay::story_events::world::{
    CallbackTarget, IdVariant, PropRequest, SpawnRequest, VoiceRequest, World,
};
use tsfp_gameplay::story_events::{BuildOptions, Engine, ModeContext};
use tsfp_gameplay::story_mission::{StoryMission, StoryMissionLoadError, StoryTerminalInputError};
use tsfp_gameplay::story_schedule::{StoryExecution, StorySchedule};
use tsfp_state::story_flow::{
    CAMPAIGN_LEVEL_ROW_STRIDE_BYTES, CampaignLevelTable, CampaignLevelTableDecodeError, MODE_STORY,
    RESULT_FAILURE, RESULT_SUCCESS, SessionSetup, StoryContinuationGate, StoryController,
    StoryFlow, StoryStep,
};
use tsfp_state::terminals::{
    InputSource, PhysicalTerminalInput, PointerPosition, ScreenId, ScriptOverride, ScriptState,
    TerminalEvent, TerminalPadBank, TerminalPadRecord, TerminalScreen, TerminalState,
    TerminalWidget, WidgetId,
};

fn story_setup() -> SessionSetup {
    let mut setup = SessionSetup::default();
    setup.set_byte(0, 1);
    setup.set_byte(0x54, 2);
    setup.set_byte(0x55, 10);
    setup
}

fn sequencer_block(active_type: u8, level_id: u8) -> ModeBlock {
    let mut bytes = core::array::from_fn(|index| (index * 37) as u8);
    bytes[0] = 1;
    bytes[3] = 0;
    bytes[0x54] = level_id;
    bytes[0x55] = active_type;
    ModeBlock::from_bytes(bytes)
}

fn campaign_levels() -> CampaignLevelTable {
    CampaignLevelTable::new([2, 10, 12, 4, 6, 8, 14, 16, 18, 20, 22, 24, 26])
}

fn campaign_level_rows() -> Vec<u8> {
    let levels = [2i16, 10, 12, 4, 6, 8, 14, 16, 18, 20, 22, 24, 26];
    let required = (levels.len() - 1) * CAMPAIGN_LEVEL_ROW_STRIDE_BYTES + 2;
    let mut rows = vec![0xa5; required];
    for (index, level_id) in levels.into_iter().enumerate() {
        let offset = index * CAMPAIGN_LEVEL_ROW_STRIDE_BYTES;
        rows[offset..offset + 2].copy_from_slice(&level_id.to_le_bytes());
    }
    rows
}

fn activated_setup_words(mut words: [u32; 40]) -> [u32; 40] {
    for offset in [0x5c, 0x78, 0x7c, 0x80, 0x84, 0x88] {
        words[offset / 4] = 0;
    }
    words
}

fn owner() -> StoryMission {
    let mut flow = StoryFlow::default();
    flow.stage(story_setup());
    flow.start_level();
    flow.ordinal = CampaignOrdinal::new(2).unwrap();
    flow.result_flags = RESULT_SUCCESS | RESULT_FAILURE;

    let context = ModeContext::story(0, 0);
    let mut events = Engine::new(&[], BuildOptions { context });
    assert!(
        events
            .slots_mut()
            .create(&context, 0, 0x1234, [0, 0, 0], 0x700)
    );

    StoryMission::new(
        flow,
        events,
        TerminalState::new(
            vec![TerminalScreen {
                id: ScreenId(3),
                children: vec![WidgetId(7)],
                pointer: PointerPosition { x: 0.0, y: 0.0 },
                timer: 0,
                previous_held: 0,
                script: ScriptState::default(),
            }],
            vec![TerminalWidget {
                id: WidgetId(7),
                rank: 1,
                flags: 0,
            }],
        ),
    )
}

fn empty_setu_entry() -> Vec<u8> {
    let mut entry = vec![0; 0x9C];
    entry[..4].copy_from_slice(b"SETU");
    let write_word = |at: usize, value: u32, bytes: &mut [u8]| {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    };
    write_word(0x84, 0, &mut entry);
    write_word(0x88, 0, &mut entry);
    write_word(0x8C, 0x38, &mut entry);
    write_word(0x90, 0, &mut entry);
    write_word(0x94, 0, &mut entry);
    write_word(0x98, 0xC8, &mut entry);
    entry
}

fn synthetic_pak(entries: &[(u32, &[u8])]) -> Vec<u8> {
    let mut pak = vec![0; 12];
    pak[..4].copy_from_slice(b"P5CK");
    let mut rows = Vec::with_capacity(entries.len());
    for (key, payload) in entries {
        let offset = pak.len() as u32;
        pak.extend_from_slice(payload);
        rows.push((*key, offset, payload.len() as u32));
    }
    let toc = pak.len() as u32;
    for (key, offset, size) in rows {
        pak.extend_from_slice(&key.to_le_bytes());
        pak.extend_from_slice(&offset.to_le_bytes());
        pak.extend_from_slice(&size.to_le_bytes());
        pak.extend_from_slice(&0u32.to_le_bytes());
    }
    pak[4..8].copy_from_slice(&toc.to_le_bytes());
    pak[8..12].copy_from_slice(&((entries.len() * 16) as u32).to_le_bytes());
    pak
}

#[test]
fn owner_keeps_story_objectives_terminals_and_progress_under_one_typed_session() {
    let mut mission = owner();
    assert!(mission.is_story());
    assert_eq!(mission.flow().level_id, 2);
    assert_eq!(mission.terminals().widgets[0].id, WidgetId(7));
    assert_eq!(mission.events().slots().len(), 1);

    mission.capture_progress_candidate(Some(MissionEvent::Complete));
    let candidate = mission.progress_candidate().unwrap().clone();
    assert_eq!(candidate.mode_id, 10);
    assert_eq!(candidate.result_flags, RESULT_SUCCESS | RESULT_FAILURE);
    assert_eq!(candidate.phase, 2);
    assert_eq!(candidate.campaign_ordinal, CampaignOrdinal::new(2));
    assert_eq!(candidate.objectives.len(), 1);
    assert_eq!(candidate.objectives[0].id, ObjectiveSlotId(0));
    assert!(candidate.has_success_result());
    assert!(candidate.has_failure_result());
    assert_eq!(mission.progress_candidate(), Some(&candidate));
}

#[test]
fn owner_resolves_only_the_known_story_controllers_from_its_active_setup() {
    let level2 = owner();
    assert_eq!(
        level2.controller(),
        Some(StoryController {
            level_id: 2,
            mode_record: 0x0050_8390,
            preload: 0x0021_8ab0,
            initialize: 0x0021_ad50,
            tick: 0x0021_ad00,
        })
    );

    let mut level4_setup = story_setup();
    level4_setup.set_byte(0x54, 4);
    let mut level4_flow = StoryFlow::default();
    level4_flow.stage(level4_setup);
    level4_flow.start_level();
    let level4 = StoryMission::new(
        level4_flow,
        Engine::new(&[], BuildOptions::story(0, 0)),
        TerminalState::new(Vec::new(), Vec::new()),
    );
    assert_eq!(
        level4.controller(),
        Some(StoryController {
            level_id: 4,
            mode_record: 0x004f_f470,
            preload: 0x001e_1c50,
            initialize: 0x001e_9cc0,
            tick: 0x001e_40d0,
        })
    );

    let mut unknown_setup = story_setup();
    unknown_setup.set_byte(0x54, 24);
    let mut unknown_flow = StoryFlow::default();
    unknown_flow.stage(unknown_setup);
    unknown_flow.start_level();
    let unknown = StoryMission::new(
        unknown_flow,
        Engine::new(&[], BuildOptions::story(0, 0)),
        TerminalState::new(Vec::new(), Vec::new()),
    );
    assert_eq!(unknown.controller(), None);

    let mut non_story_setup = story_setup();
    non_story_setup.set_byte(0x55, 9);
    let mut non_story_flow = StoryFlow::default();
    non_story_flow.stage(non_story_setup);
    non_story_flow.start_level();
    let non_story = StoryMission::new(
        non_story_flow,
        Engine::new(&[], BuildOptions::story(0, 0)),
        TerminalState::new(Vec::new(), Vec::new()),
    );
    assert_eq!(non_story.controller(), None);
}

#[test]
fn progress_candidate_is_observational_and_clears_only_when_consumed() {
    let mut mission = owner();
    let flags_before = mission.flow().result_flags;
    mission.capture_progress_candidate(None);

    assert_eq!(mission.flow().result_flags, flags_before);
    assert!(mission.progress_candidate().is_some());
    assert!(mission.clear_progress_candidate().is_some());
    assert!(mission.progress_candidate().is_none());
}

#[test]
fn mission_step_aggregates_its_owned_objective_into_story_result_flags() {
    let context = ModeContext::story(0, 0);
    let mut events = Engine::new(&[], BuildOptions { context });
    assert!(
        events
            .slots_mut()
            .create(&context, 0, 0x1234, [0, 0, 0], 0x700)
    );

    let mut flow = StoryFlow::default();
    flow.stage(story_setup());
    flow.start_level();
    let mut mission = StoryMission::new(flow, events, TerminalState::new(Vec::new(), Vec::new()));
    let mut world = EmptyStoryWorld;
    mission.finish_objective_slot(0, &mut world);

    mission.step(1.0 / 60.0, &mut world);

    assert_eq!(mission.events().slots().status(0), 5);
    assert_eq!(mission.flow().result_flags, RESULT_SUCCESS);
    assert_eq!(mission.events().session_flags(), RESULT_SUCCESS);
    assert!(
        mission
            .capture_progress_candidate(None)
            .has_success_result()
    );
}

#[test]
fn mission_objective_success_preserves_an_existing_failure_result_bit() {
    let mut mission = owner();
    let mut world = EmptyStoryWorld;
    mission.set_result_flags(RESULT_FAILURE);

    mission.finish_objective_slot(0, &mut world);
    mission.step(1.0 / 60.0, &mut world);

    assert_eq!(mission.flow().result_flags, RESULT_FAILURE | RESULT_SUCCESS);
    assert_eq!(
        mission.events().session_flags(),
        RESULT_FAILURE | RESULT_SUCCESS
    );
}

#[test]
fn completed_objective_flows_through_results_to_next_level_owner() {
    let mut mission = owner();
    let mut world = EmptyStoryWorld;
    mission.finish_objective_slot(0, &mut world);
    mission.step(1.0 / 60.0, &mut world);

    assert_ne!(mission.flow().result_flags & RESULT_SUCCESS, 0);
    assert!(mission.stage_success_frontend(true));

    let next_block = sequencer_block(10, 4);
    let next_words = next_block.words();
    let route =
        StoryMission::continuation_from_sequencer(campaign_levels(), None, &next_block).unwrap();
    assert!(mission.continue_after_results(
        StoryContinuationGate {
            descriptor_ready: true,
            attract_mode: false,
            session_phase: 1,
            menu_kind: 4,
            unlock_notice_pending: false,
        },
        route,
    ));
    assert_eq!(
        mission.flow().step,
        StoryStep::NextGameplay {
            ordinal: CampaignOrdinal::new(3).unwrap(),
            level_id: 4,
        }
    );
    assert_eq!(mission.flow().result_flags & RESULT_SUCCESS, RESULT_SUCCESS);

    mission
        .start_staged_level(
            &empty_setu_entry(),
            BuildOptions::story(0, 0),
            TerminalState::new(Vec::new(), Vec::new()),
            TerminalPadBank::default(),
        )
        .unwrap();

    assert_eq!(
        mission.flow().active.words,
        activated_setup_words(next_words)
    );
    assert_eq!(mission.flow().result_flags, 0);
    assert_eq!(mission.flow().ordinal, CampaignOrdinal::new(3).unwrap());
    assert_eq!(
        mission.flow().step,
        StoryStep::Gameplay {
            ordinal: CampaignOrdinal::new(3).unwrap(),
            level_id: 4
        }
    );
    assert_eq!(mission.controller().unwrap().level_id, 4);
}

#[test]
fn owner_routes_native_actor_event_and_checkpoint_ingress_to_its_engine() {
    let mut mission = owner();
    mission.register_actor(0x100, 3);
    mission.unregister_actor(0x100);
    assert_eq!(mission.signal_event_key(0x1234, false, &[5, 12, 13]), 0);

    mission.set_checkpoint_mask(0x25);
    assert_eq!(mission.checkpoint_mask(), 0x25);
    assert_eq!(
        mission.flow().result_flags,
        mission.events().session_flags()
    );
}

#[test]
fn owner_applies_terminal_focus_and_activation_to_owned_state() {
    let mut mission = owner();
    assert_eq!(
        mission.activate_terminal_child(ScreenId(3), 0, 17).unwrap(),
        true
    );
    assert_eq!(mission.terminals().focus, Some(WidgetId(7)));
    assert_eq!(mission.terminals().screens[0].timer, 17);
    assert_eq!(mission.terminals().current_parent, Some(ScreenId(3)));
    assert_eq!(mission.terminals().current_widget, Some(WidgetId(7)));
    assert_eq!(mission.terminals().current_timer, 17);
    assert_eq!(
        mission.focus_terminal_topmost(ScreenId(3)).unwrap(),
        Some(WidgetId(7))
    );
}

#[test]
fn runtime_setu_entry_builds_the_owned_engine_from_decoded_tables() {
    let entry = empty_setu_entry();
    let mission = StoryMission::from_setu_entry(
        StoryFlow::default(),
        &entry,
        BuildOptions::story(0, 0),
        TerminalState::new(Vec::new(), Vec::new()),
    )
    .unwrap();

    assert!(mission.events().is_empty());
    assert!(mission.events().slots().is_empty());
    assert!(mission.terminals().widgets.is_empty());
    assert_eq!(
        mission.flow().result_flags,
        mission.events().session_flags()
    );
}

#[test]
fn sequencer_setup_becomes_active_story_state_with_activation_owned_fields_cleared() {
    let block = sequencer_block(10, 2);
    let mut flow = StoryFlow::default();
    flow.ordinal = CampaignOrdinal::new(7).unwrap();
    let mission = StoryMission::from_sequencer_setup(
        flow,
        &block,
        &empty_setu_entry(),
        BuildOptions::story(0, 0),
        TerminalState::new(Vec::new(), Vec::new()),
    )
    .unwrap();

    assert_eq!(
        mission.flow().active.words,
        activated_setup_words(block.words())
    );
    assert_eq!(mission.flow().staging.words, block.words());
    assert_eq!(mission.flow().level_id, 2);
    assert_eq!(mission.flow().phase, 2);
    assert_eq!(mission.flow().ordinal, CampaignOrdinal::new(7).unwrap());
}

#[test]
fn sequencer_setup_rejects_non_story_mode_before_loading_mission_data() {
    let result = StoryMission::from_sequencer_setup(
        StoryFlow::default(),
        &sequencer_block(9, 0x6a),
        b"malformed setup data is not read",
        BuildOptions::story(0, 0),
        TerminalState::new(Vec::new(), Vec::new()),
    );

    assert!(matches!(
        result,
        Err(StoryMissionLoadError::NotStorySetup { actual_mode: 9 })
    ));
}

#[test]
fn sequencer_records_derive_continuation_ordinal_without_rewriting_words() {
    let post_movie = sequencer_block(10, 0x2c);
    let next_gameplay = sequencer_block(10, 4);
    let route = StoryMission::continuation_from_sequencer(
        campaign_levels(),
        Some(&post_movie),
        &next_gameplay,
    )
    .unwrap();

    assert_eq!(route.next_ordinal(), CampaignOrdinal::new(3).unwrap());
    assert_eq!(route.post_movie().unwrap().words, post_movie.words());
    assert_eq!(route.next_gameplay().words, next_gameplay.words());

    let direct_route =
        StoryMission::continuation_from_sequencer(campaign_levels(), None, &sequencer_block(10, 6))
            .unwrap();
    assert_eq!(direct_route.post_movie(), None);
    assert_eq!(
        direct_route.next_ordinal(),
        CampaignOrdinal::new(4).unwrap()
    );
    assert_eq!(
        direct_route.next_gameplay().words,
        sequencer_block(10, 6).words()
    );
}

#[test]
fn runtime_level_rows_decode_into_a_validated_sequencer_continuation() {
    let rows = campaign_level_rows();
    let post_movie = sequencer_block(10, 0x2c);
    let next_gameplay = sequencer_block(10, 4);
    let route = StoryMission::continuation_from_runtime_level_rows(
        &rows,
        Some(&post_movie),
        &next_gameplay,
    )
    .unwrap();

    assert_eq!(route.next_ordinal(), CampaignOrdinal::new(3).unwrap());
    assert_eq!(route.post_movie().unwrap().words, post_movie.words());
    assert_eq!(route.next_gameplay().words, next_gameplay.words());

    let error = StoryMission::continuation_from_runtime_level_rows(
        &rows[..rows.len() - 1],
        Some(&post_movie),
        &next_gameplay,
    )
    .unwrap_err();
    assert_eq!(
        error,
        StoryMissionLoadError::CampaignLevelTable(CampaignLevelTableDecodeError::TooShort {
            required: rows.len(),
            actual: rows.len() - 1,
        })
    );
}

#[test]
fn sequencer_continuation_rejects_result_frontend_as_a_gameplay_record() {
    let result = StoryMission::continuation_from_sequencer(
        campaign_levels(),
        None,
        &sequencer_block(9, 0x6a),
    );

    assert!(matches!(
        result,
        Err(StoryMissionLoadError::NotStorySetup { actual_mode: 9 })
    ));

    assert!(matches!(
        StoryMission::continuation_from_sequencer(
            campaign_levels(),
            None,
            &sequencer_block(MODE_STORY, 9),
        ),
        Err(StoryMissionLoadError::UnknownCampaignLevel { level_id: 9 })
    ));

    let wrong_post_movie = sequencer_block(9, 0x2c);
    let result = StoryMission::continuation_from_sequencer(
        campaign_levels(),
        Some(&wrong_post_movie),
        &sequencer_block(10, 4),
    );
    assert!(matches!(
        result,
        Err(StoryMissionLoadError::NotStorySetup { actual_mode: 9 })
    ));
}

#[test]
fn runtime_setu_entry_rejects_malformed_assets_without_fallback_state() {
    let result = StoryMission::from_setu_entry(
        StoryFlow::default(),
        b"not a SETU entry",
        BuildOptions::story(0, 0),
        TerminalState::new(Vec::new(), Vec::new()),
    );

    assert!(matches!(
        result,
        Err(StoryMissionLoadError::EventDefinitions(_))
    ));
}

#[test]
fn runtime_pak_selector_preserves_duplicate_key_order_and_builds_owner() {
    let decoy = b"not a SETU entry";
    let entry = empty_setu_entry();
    let pak = synthetic_pak(&[(0x4afe_3652, decoy), (0x4afe_3652, &entry)]);
    let mission = StoryMission::from_pak_entry(
        StoryFlow::default(),
        &pak,
        SetuAssetSelector::new(0x4afe_3652, 1),
        BuildOptions::story(0, 0),
        TerminalState::new(Vec::new(), Vec::new()),
    )
    .unwrap();

    assert!(mission.events().is_empty());
    assert!(mission.events().slots().is_empty());
}

#[test]
fn runtime_pak_selector_rejects_missing_or_non_setu_rows() {
    let entry = empty_setu_entry();
    let pak = synthetic_pak(&[(0x4afe_3652, &entry)]);
    let missing = StoryMission::from_pak_entry(
        StoryFlow::default(),
        &pak,
        SetuAssetSelector::new(0x4afe_3652, 1),
        BuildOptions::story(0, 0),
        TerminalState::new(Vec::new(), Vec::new()),
    );
    assert!(matches!(missing, Err(StoryMissionLoadError::PakEntry(_))));

    let decoy = synthetic_pak(&[(0x4afe_3652, b"not SETU")]);
    let malformed = StoryMission::from_pak_entry(
        StoryFlow::default(),
        &decoy,
        SetuAssetSelector::new(0x4afe_3652, 0),
        BuildOptions::story(0, 0),
        TerminalState::new(Vec::new(), Vec::new()),
    );
    assert!(matches!(malformed, Err(StoryMissionLoadError::PakEntry(_))));

    let mut compressed = synthetic_pak(&[(0x4afe_3652, b"stored payload")]);
    let stored_size = compressed.len() - 4;
    compressed[stored_size..].copy_from_slice(&14u32.to_le_bytes());
    let compressed = StoryMission::from_pak_entry(
        StoryFlow::default(),
        &compressed,
        SetuAssetSelector::new(0x4afe_3652, 0),
        BuildOptions::story(0, 0),
        TerminalState::new(Vec::new(), Vec::new()),
    );
    assert!(matches!(
        compressed,
        Err(StoryMissionLoadError::PakEntry(_))
    ));
}

#[test]
fn story_owner_samples_terminal_input_and_commits_handler_edge() {
    let mut mission = owner();
    mission.set_terminal_input_bank(TerminalPadBank {
        physical_port_by_slot: vec![0],
        physical_records: vec![TerminalPadRecord {
            held: 0x10,
            consumed: 0,
            dead_zone: 0.1,
            axes: [128, 255, 0],
        }],
        ..TerminalPadBank::default()
    });

    let (first, first_event) = mission
        .terminal_input_event(ScreenId(3), 0, 0x10, false, None)
        .unwrap();
    assert_eq!(first.source, InputSource::Physical);
    assert!(first.held);
    assert_eq!(first_event, TerminalEvent::Press);
    mission
        .commit_terminal_input_event(ScreenId(3), first)
        .unwrap();

    let (_, held_event) = mission
        .terminal_input_event(ScreenId(3), 0, 0x10, false, None)
        .unwrap();
    assert_eq!(held_event, TerminalEvent::Held);

    let (script, script_event) = mission
        .terminal_input_event(
            ScreenId(3),
            0,
            0x10,
            false,
            Some(ScriptOverride {
                held: false,
                cursor_delta: [2.0, -1.0],
            }),
        )
        .unwrap();
    assert_eq!(script.source, InputSource::Script);
    assert_eq!(script_event, TerminalEvent::Release);
    mission
        .commit_terminal_input_event(ScreenId(3), script)
        .unwrap();

    assert!(matches!(
        mission.terminal_input_event(ScreenId(99), 0, 0x10, false, None),
        Err(StoryTerminalInputError::Terminal(_))
    ));
}

#[test]
fn story_owner_resolves_native_terminal_input_without_pad_records() {
    let mut mission = owner();
    let physical = PhysicalTerminalInput {
        held_after_consumption: 0x10,
        activation_mask: 0x10,
        left_x: 0.25,
        left_y: -0.5,
    };

    let (pressed, pressed_event) = mission
        .native_terminal_input_event(ScreenId(3), physical, false, None)
        .unwrap();
    assert_eq!(pressed.source, InputSource::Physical);
    assert!(pressed.held);
    assert_eq!(pressed.cursor_delta, [0.25, 0.5]);
    assert_eq!(pressed_event, TerminalEvent::Press);
    mission
        .commit_terminal_input_event(ScreenId(3), pressed)
        .unwrap();

    let released = PhysicalTerminalInput {
        held_after_consumption: 0,
        ..physical
    };
    let (released, released_event) = mission
        .native_terminal_input_event(ScreenId(3), released, false, None)
        .unwrap();
    assert!(!released.held);
    assert_eq!(released_event, TerminalEvent::Release);

    assert!(matches!(
        mission.native_terminal_input_event(ScreenId(99), physical, false, None),
        Err(StoryTerminalInputError::Terminal(_))
    ));
}

struct EmptyStoryWorld;

impl World for EmptyStoryWorld {
    fn play_voice(&mut self, _request: &VoiceRequest) -> Option<u32> {
        None
    }
    fn voice_finished(&mut self, _handle: u32) -> bool {
        true
    }
    fn run_callback(&mut self, _callback: u32, _target: CallbackTarget) {}
    fn set_objective(&mut self, _actor_key: u32, _index: u32) {}
    fn control_prop(&mut self, _request: &PropRequest) {}
    fn prop_settled(&mut self, _key: u32) -> bool {
        true
    }
    fn spawn_object(&mut self, _request: &SpawnRequest) -> Option<u32> {
        None
    }
    fn object_ready(&mut self, _handle: u32) -> bool {
        true
    }
    fn destroy_object(&mut self, _handle: u32) {}
    fn player_count(&self) -> usize {
        0
    }
    fn player_matches(&self, _player: usize, _variant: IdVariant, _ids: &[u32]) -> bool {
        false
    }
    fn first_player_flag(&self) -> bool {
        false
    }
    fn tag_exists(&self, _key: u32) -> bool {
        false
    }
    fn poll_link(&mut self, _link: &Link) -> bool {
        false
    }
}

fn empty_story_owner() -> StoryMission {
    StoryMission::new(
        StoryFlow::default(),
        Engine::new(
            &[],
            BuildOptions {
                context: ModeContext::story(0, 0),
            },
        ),
        TerminalState::new(vec![], vec![]),
    )
}

#[test]
fn native_schedule_step_matches_direct_owner_step_on_empty_typed_state() {
    let mut expected_owner = empty_story_owner();
    let mut actual_owner = empty_story_owner();
    let mut expected_world = EmptyStoryWorld;
    let mut actual_world = EmptyStoryWorld;
    let expected = expected_owner.step(1.0 / 60.0, &mut expected_world);

    let mut schedule = StorySchedule::new(StoryExecution::Native);
    let actual = schedule.step_fixed(&mut actual_owner, &mut actual_world, || {
        panic!("native schedule must not call the lifted branch")
    });

    assert_eq!(actual.fired, expected.fired);
    assert_eq!(actual.errors, expected.errors);
    assert_eq!(actual.checkpoints, expected.checkpoints);
    assert_eq!(actual_owner.flow(), expected_owner.flow());
    assert_eq!(schedule.execution(), StoryExecution::Native);
}

#[test]
fn staged_story_level_replaces_mission_state_after_runtime_setu_parse() {
    let mut mission = owner();
    assert_eq!(mission.events().slots().len(), 1);
    mission.capture_progress_candidate(None);

    let mut next = story_setup();
    next.set_byte(0x54, 4);
    mission.stage(next);
    mission
        .start_staged_level(
            &empty_setu_entry(),
            BuildOptions {
                context: ModeContext::story(0, 0),
            },
            TerminalState::new(vec![], vec![]),
            TerminalPadBank::default(),
        )
        .unwrap();

    assert_eq!(mission.flow().active.level_id(), 4);
    assert_eq!(mission.events().slots().len(), 0);
    assert!(mission.progress_candidate().is_none());
    assert!(mission.is_story());
}

#[test]
fn staged_story_level_rejects_invalid_inputs_without_replacing_current_mission() {
    let mut mission = owner();
    mission.capture_progress_candidate(None);
    let mut next = story_setup();
    next.set_byte(0x54, 4);
    mission.stage(next);

    assert!(matches!(
        mission.start_staged_level(
            b"bad",
            BuildOptions {
                context: ModeContext::story(0, 0),
            },
            TerminalState::new(vec![], vec![]),
            TerminalPadBank::default(),
        ),
        Err(StoryMissionLoadError::EventDefinitions(_))
    ));

    let mut non_story = next;
    non_story.set_byte(0x55, 9);
    mission.stage(non_story);
    assert_eq!(
        mission.start_staged_level(
            &empty_setu_entry(),
            BuildOptions {
                context: ModeContext::story(0, 0),
            },
            TerminalState::new(vec![], vec![]),
            TerminalPadBank::default(),
        ),
        Err(StoryMissionLoadError::NotStorySetup { actual_mode: 9 })
    );

    assert_eq!(mission.flow().active.level_id(), 2);
    assert_eq!(mission.flow().staging.level_id(), 4);
    assert_eq!(mission.events().slots().len(), 1);
    assert!(mission.progress_candidate().is_some());
}
