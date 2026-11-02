use std::collections::BTreeMap;
use tsfp_format_profile::Profile;
use tsfp_ui_ext::{ExtensionPreset, settings_page::*};
use tsfp_ui_input::{
    ActionArg, ActionId, BindResult, RecordingBindings, UiBindings, UiEvent, UiValue,
};
use tsfp_ui_model::{Page, PageId, UiModel, Widget, WidgetId, WidgetKind};

fn profile(seed: u8) -> Profile {
    let mut payload = vec![seed; 0x1f54];
    payload[0..4].fill(0);
    payload[6] = 0;
    payload[0x6c..0x70].copy_from_slice(&0u32.to_le_bytes());
    payload[0x78..0x7c].copy_from_slice(&0u32.to_le_bytes());
    payload[0x7c..0x80].copy_from_slice(&0.5f32.to_le_bytes());
    // SettingsEdit validates the measured slider ranges on Accept. Keep the synthetic fixture
    // inside those bounds so tests exercise page effects instead of malformed profile data.
    let audio_words: [(usize, u32); 6] = [
        (0x198, 0),
        (0x19c, 1),
        (0x1a0, 8),
        (0x1a4, 12),
        (0x1ac, 32),
        (0x1b0, 0),
    ];
    for (offset, value) in audio_words {
        payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    Profile {
        payload,
        signature: [seed; 20],
    }
}

fn routes() -> SettingsRoutes {
    SettingsRoutes {
        root: PageId(10),
        controls: PageId(11),
        audio_video: PageId(12),
        confirmation: PageId(13),
    }
}

fn session(mode: u32) -> SettingsPageSession {
    SettingsPageSession::open(
        vec![profile(0x11), profile(0x22)],
        SettingsPageContext {
            mode,
            viewport: 2,
            gameplay: false,
            profile_slot: 0,
            configured_slot_count: 2,
            profile_slots: vec![0, 1],
        },
        routes(),
    )
    .unwrap()
}

fn actions() -> SettingsSessionActions {
    SettingsSessionActions {
        open_controls: ActionId(101),
        open_audio_video: ActionId(102),
        controls_back: ActionId(103),
        controls_accept: ActionId(104),
        controls_yes: ActionId(105),
        controls_no: ActionId(106),
        audio_cancel: ActionId(107),
        audio_accept: ActionId(109),
        root_back: ActionId(108),
    }
}

#[test]
fn settings_page_actions_attach_to_runtime_widgets_and_original_is_empty() {
    let mut model = UiModel::new();
    let mut root = Page::new(PageId(10), 1);
    root.widgets
        .push(Widget::new(WidgetId(1), WidgetKind::Button));
    root.widgets
        .push(Widget::new(WidgetId(2), WidgetKind::Button));
    let controls = Page::new(PageId(11), 1);
    let audio_video = Page::new(PageId(12), 1);
    model.pages.insert(root.id, root);
    model.pages.insert(controls.id, controls);
    model.pages.insert(audio_video.id, audio_video);

    let base = model.clone();
    assert_eq!(
        ExtensionPreset::original().apply(&base).unwrap().model,
        base
    );
    let extension = settings_page_actions(
        &model,
        SettingsPageWidgets {
            root: PageId(10),
            controls: PageId(11),
            audio_video: PageId(12),
            controls_row: WidgetId(1),
            audio_video_row: WidgetId(2),
            profile_selector: None,
            unlocks_row: None,
            open_controls: ActionId(101),
            open_audio_video: ActionId(102),
        },
    )
    .unwrap();

    let mut state = session(50);
    let mut inner = RecordingBindings::default();
    let mut bindings = SettingsSessionBindings {
        session: &mut state,
        inner: &mut inner,
        actions: actions(),
        values: Default::default(),
        profile_slot_value: None,
        effects: Vec::new(),
    };
    assert_eq!(
        route_settings_event(
            &extension,
            PageId(10),
            &UiEvent::Activate(tsfp_ui_input::layout::WidgetId(1)),
            &mut bindings,
        ),
        BindResult::Handled
    );
    assert_eq!(bindings.session.current_page(), PageId(11));
    assert_eq!(bindings.effects, [SettingsEffect::Open(PageId(11))]);
}

#[test]
fn controls_confirmation_yes_and_direct_accept_keep_distinct_unwinds() {
    let mut direct = session(50);
    direct.edit_mut().controls.set_turn_speed(0.75).unwrap();
    direct.open_controls();
    let result = direct.accept_controls_direct().unwrap();
    assert_eq!(result[2], SettingsEffect::PopPages(1));
    assert_eq!(
        direct.selected_profile().settings().unwrap().turn_speed(),
        0.75
    );

    let mut confirmed = session(50);
    confirmed.edit_mut().controls.set_turn_speed(0.75).unwrap();
    confirmed.open_controls();
    assert_eq!(
        confirmed.controls_back().unwrap(),
        SettingsEffect::Open(PageId(13))
    );
    assert_eq!(
        confirmed.confirm_controls_no_frontend().unwrap(),
        SettingsEffect::PopPages(1)
    );
    assert_eq!(confirmed.current_page(), PageId(11));
    assert_eq!(
        confirmed
            .selected_profile()
            .settings()
            .unwrap()
            .turn_speed(),
        0.5
    );
    // Re-enter confirmation; Yes writes the same typed head but unwinds both pages.
    confirmed.controls_back().unwrap();
    let result = confirmed.confirm_controls_yes_frontend().unwrap();
    assert_eq!(result[2], SettingsEffect::PopPages(2));
    assert_eq!(confirmed.current_page(), PageId(10));
    assert_eq!(
        confirmed
            .selected_profile()
            .settings()
            .unwrap()
            .turn_speed(),
        0.75
    );
}

#[test]
fn controls_page_edits_owned_settings_until_accept() {
    let mut state = session(50);
    let before = state.selected_profile().clone();
    state.open_controls();
    state
        .set_setting_value(SettingsValue::Crosshair, UiValue::Index(2))
        .unwrap();
    assert_eq!(state.selected_profile().payload, before.payload);
    assert_eq!(state.selected_profile().signature, before.signature);
    let effects = state.accept_controls_direct().unwrap();
    assert_eq!(effects[2], SettingsEffect::PopPages(1));
    assert_eq!(
        state
            .selected_profile()
            .settings()
            .unwrap()
            .controls()
            .unwrap()
            .crosshair,
        tsfp_format_profile::settings::Crosshair::Fixed
    );
}

#[test]
fn audio_video_page_edits_owned_settings_until_accept() {
    let mut state = session(50);
    let before = state.selected_profile().clone();
    state.open_audio_video();
    state
        .set_setting_value(
            SettingsValue::AudioRaw(AudioWord::MusicVolume),
            UiValue::Index(14),
        )
        .unwrap();
    assert_eq!(state.selected_profile().payload, before.payload);
    assert_eq!(state.selected_profile().signature, before.signature);
    state.accept_audio_video().unwrap();
    assert_eq!(
        state.selected_profile().audio_video().unwrap().music_volume,
        14
    );
}

#[test]
fn unchanged_controls_pop_and_audio_cancel_applies_restore_twice() {
    let mut state = session(50);
    state.open_controls();
    assert_eq!(state.controls_back().unwrap(), SettingsEffect::PopPages(1));
    state.open_audio_video();
    state.edit_mut().audio_video.music_volume = 16;
    assert_eq!(
        state.cancel_audio_video().unwrap(),
        [
            SettingsEffect::ApplyAudioSettings,
            SettingsEffect::ApplyAudioSettings,
            SettingsEffect::PopPages(1),
        ]
    );
    assert_eq!(
        state.edit().audio_video.music_volume,
        profile(0x11).audio_video().unwrap().music_volume
    );
}

#[test]
fn audio_accept_writes_typed_six_word_set_and_preserves_intervening_word() {
    let mut state = session(50);
    let before = state.selected_profile().clone();
    state.open_audio_video();
    state.edit_mut().audio_video = tsfp_format_profile::settings::AudioVideo {
        music_enabled: 1,
        sound_enabled: 0,
        music_volume: 16,
        sound_volume: 8,
        slider_4: 64,
        option_5: 0x1234_5678,
    };
    assert_eq!(
        state.accept_audio_video().unwrap(),
        [
            SettingsEffect::MarkProfileDirty { slot: 0 },
            SettingsEffect::PopPages(1)
        ]
    );
    let after = state.selected_profile();
    for offset in [0x198, 0x19c, 0x1a0, 0x1a4, 0x1ac, 0x1b0] {
        assert_ne!(
            &after.payload[offset..offset + 4],
            &before.payload[offset..offset + 4]
        );
    }
    assert_eq!(&after.payload[0x1a8..0x1ac], &before.payload[0x1a8..0x1ac]);
    assert_eq!(after.signature, before.signature);
}

#[test]
fn gameplay_audio_accept_requests_context_clear_at_the_explicit_boundary() {
    let mut state = SettingsPageSession::open(
        vec![profile(0x11)],
        SettingsPageContext {
            mode: 101,
            viewport: 1,
            gameplay: true,
            profile_slot: 0,
            configured_slot_count: 1,
            profile_slots: vec![0],
        },
        routes(),
    )
    .unwrap();
    state.open_audio_video();
    assert_eq!(
        state.accept_audio_video().unwrap(),
        [
            SettingsEffect::MarkProfileDirty { slot: 0 },
            SettingsEffect::ClearGameplaySettingsContext { viewport: 1 },
            SettingsEffect::PopCurrentContext,
        ]
    );
}

#[test]
fn profile_slot_and_viewport_are_independent_and_selector_is_conditional() {
    let mut state = session(101);
    assert!(state.context.has_profile_selector());
    assert_eq!(state.context.viewport, 2);
    assert_eq!(state.context.profile_slot, 0);
    state.select_profile(1).unwrap();
    assert_eq!(state.context.viewport, 2);
    assert_eq!(state.context.profile_slot, 1);
    assert_eq!(state.selected_profile().payload[20], 0x22);

    let single = SettingsPageSession::open(
        vec![profile(0x33)],
        SettingsPageContext {
            mode: 0,
            viewport: 3,
            gameplay: false,
            profile_slot: 0,
            configured_slot_count: 1,
            profile_slots: vec![0],
        },
        routes(),
    )
    .unwrap();
    assert!(!single.context.has_profile_selector());
    assert!(single.profile(1).is_none());
}

#[test]
fn profile_selector_uses_runtime_slot_results_instead_of_player_ordinals() {
    let mut state = SettingsPageSession::open(
        vec![profile(0x10), profile(0x20), profile(0x30)],
        SettingsPageContext {
            mode: 0,
            viewport: 3,
            gameplay: false,
            profile_slot: 0,
            configured_slot_count: 2,
            profile_slots: vec![2, 0],
        },
        routes(),
    )
    .unwrap();
    let mut inner = RecordingBindings::default();
    let mut bindings = SettingsSessionBindings {
        session: &mut state,
        inner: &mut inner,
        actions: actions(),
        values: BTreeMap::new(),
        profile_slot_value: Some(tsfp_ui_input::ValueId(33)),
        effects: Vec::new(),
    };
    assert_eq!(
        bindings.value(tsfp_ui_input::ValueId(33)),
        UiValue::Index(1)
    );
    assert_eq!(
        bindings.set_value(tsfp_ui_input::ValueId(33), UiValue::Index(0)),
        BindResult::Handled
    );
    assert_eq!(bindings.session.context.profile_slot, 2);
    assert_eq!(bindings.session.context.viewport, 3);
}

#[test]
fn game_mode_confirmation_uses_dirty_profile_context_path() {
    let mut state = session(101);
    state.edit_mut().controls.set_turn_speed(0.8).unwrap();
    state.open_controls();
    assert_eq!(
        state.open_game_controls_confirmation(true),
        Ok(SettingsEffect::Open(PageId(13)))
    );
    assert_eq!(
        state.confirm_controls_yes_in_game().unwrap(),
        [
            SettingsEffect::MarkProfileDirty { slot: 0 },
            SettingsEffect::PopCurrentContext,
        ]
    );
    assert_eq!(
        state.selected_profile().settings().unwrap().turn_speed(),
        0.8
    );
}

#[test]
fn rejected_mode_or_widget_context_is_not_silently_routed() {
    let mut state = session(101);
    state.open_controls();
    assert_eq!(
        state.controls_back(),
        Err("game-mode controls back requires original context predicates")
    );

    let mut model = UiModel::new();
    model.pages.insert(PageId(10), Page::new(PageId(10), 1));
    model.pages.insert(PageId(11), Page::new(PageId(11), 1));
    model.pages.insert(PageId(12), Page::new(PageId(12), 1));
    assert_eq!(
        settings_page_actions(
            &model,
            SettingsPageWidgets {
                root: PageId(10),
                controls: PageId(11),
                audio_video: PageId(12),
                controls_row: WidgetId(99),
                audio_video_row: WidgetId(2),
                profile_selector: None,
                unlocks_row: None,
                open_controls: ActionId(1),
                open_audio_video: ActionId(2),
            },
        ),
        Err("settings page widget")
    );
}

#[test]
fn unknown_actions_and_values_delegate_to_the_neighbouring_binding() {
    let mut state = session(50);
    let mut inner = RecordingBindings::default();
    inner
        .values
        .insert(tsfp_ui_input::ValueId(9), UiValue::Int(3));
    let mut bindings = SettingsSessionBindings {
        session: &mut state,
        inner: &mut inner,
        actions: actions(),
        values: Default::default(),
        profile_slot_value: None,
        effects: Vec::new(),
    };
    assert_eq!(bindings.value(tsfp_ui_input::ValueId(9)), UiValue::Int(3));
    assert_eq!(
        bindings.action(ActionId(999), ActionArg::Index(5)),
        BindResult::Handled
    );
}

#[test]
fn runtime_row_value_ids_edit_owned_controls_audio_and_profile_selection() {
    let mut state = session(50);
    let mut inner = RecordingBindings::default();
    let values = BTreeMap::from([
        (tsfp_ui_input::ValueId(20), SettingsValue::Crosshair),
        (tsfp_ui_input::ValueId(21), SettingsValue::TurnSpeed),
        (
            tsfp_ui_input::ValueId(22),
            SettingsValue::AudioRaw(AudioWord::MusicVolume),
        ),
    ]);
    let mut bindings = SettingsSessionBindings {
        session: &mut state,
        inner: &mut inner,
        actions: actions(),
        values,
        profile_slot_value: Some(tsfp_ui_input::ValueId(23)),
        effects: Vec::new(),
    };
    assert_eq!(
        bindings.value(tsfp_ui_input::ValueId(20)),
        UiValue::Index(0)
    );
    assert_eq!(
        bindings.set_value(tsfp_ui_input::ValueId(20), UiValue::Index(2)),
        BindResult::Handled
    );
    assert_eq!(
        bindings.value(tsfp_ui_input::ValueId(20)),
        UiValue::Index(2)
    );
    assert_eq!(
        bindings.set_value(tsfp_ui_input::ValueId(21), UiValue::Float(1.0)),
        BindResult::Handled
    );
    assert_eq!(
        bindings.set_value(tsfp_ui_input::ValueId(21), UiValue::Float(1.01)),
        BindResult::Denied
    );
    assert_eq!(
        bindings.set_value(tsfp_ui_input::ValueId(22), UiValue::Index(16)),
        BindResult::Handled
    );
    assert_eq!(
        bindings.set_value(tsfp_ui_input::ValueId(22), UiValue::Index(17)),
        BindResult::Denied
    );
    assert_eq!(
        bindings.set_value(tsfp_ui_input::ValueId(23), UiValue::Index(1)),
        BindResult::Handled
    );
    assert_eq!(
        bindings.value(tsfp_ui_input::ValueId(23)),
        UiValue::Index(1)
    );
    assert_eq!(bindings.session.context.viewport, 2);
    assert_eq!(bindings.session.selected_profile().payload[20], 0x22);
}
