//! T2211 extras and rebinding rows: Tier 0 smoke plus seeded property tests (FABRICATED inputs).
use tsfp_input::actions::{ActionMap, InputId, Intent};
use tsfp_input::extras::{DeviceAssignment, PlayerExtras};
use tsfp_ui_ext::input_extras::*;
use tsfp_ui_ext::{Binding, ExtensionPreset};
use tsfp_ui_input::{ActionArg, BindResult, RecordingBindings, UiBindings, UiValue, ValueId};
use tsfp_ui_model::*;
use tsfp_ui_renderer::Vec2;

fn base() -> UiModel {
    let mut model = UiModel::new();
    let mut page = Page::new(PageId(7), 1);
    page.widgets
        .push(Widget::new(WidgetId(1), WidgetKind::Button));
    page.widgets
        .push(Widget::new(WidgetId(2), WidgetKind::Button));
    model.pages.insert(page.id, page);
    model
}

fn rect() -> tsfp_ui_renderer::AnchoredRect {
    menu_rect(Vec2::new(10., 50.), Vec2::new(300., 20.))
}

#[test]
fn defaults_are_todays_behaviour() {
    let extras = InputExtras::new(4);
    assert_eq!(extras.players.len(), 4);
    assert!(extras.players.iter().all(|p| *p == PlayerExtras::default()));
    assert!(
        extras
            .maps
            .iter()
            .all(|m| *m == ActionMap::default_mouse_keyboard())
    );
    let p = PlayerExtras::default();
    assert_eq!(p.look_gain(37.0, false), 1.0);
    assert_eq!(p.look_gain(37.0, true), 1.0);
    assert_eq!(p.scale_rumble(65535), 65535);
    assert_eq!(extras.read(0, ExtraField::Device), UiValue::Index(0));
    assert!(!extras.dirty && extras.capturing.is_none());
}

#[test]
fn writes_clamp_refuse_and_mark_dirty() {
    let mut extras = InputExtras::new(2);
    assert_eq!(
        extras.write(1, ExtraField::Rumble, UiValue::Float(3.0)),
        BindResult::Handled
    );
    assert!(extras.dirty);
    assert_eq!(extras.players[1].rumble, 1.0);
    assert_eq!(extras.players[0], PlayerExtras::default());
    extras.write(0, ExtraField::Smoothing, UiValue::Int(99));
    assert_eq!(extras.players[0].smoothing_frames, 8);
    extras.write(0, ExtraField::ScopeMultiplier, UiValue::Float(0.5));
    assert_eq!(extras.players[0].look_gain(0.0, true), 0.5);
    assert_eq!(extras.players[0].look_gain(0.0, false), 1.0);
    extras.write(0, ExtraField::Acceleration, UiValue::Float(0.25));
    assert_eq!(extras.players[0].look_gain(4.0, false), 2.0);
    extras.write(0, ExtraField::RawLook, UiValue::Index(1));
    assert!(extras.players[0].raw_look);
    extras.write(0, ExtraField::Device, UiValue::Index(3));
    assert_eq!(extras.players[0].device, DeviceAssignment::Pad(1));
    for (field, value) in [
        (ExtraField::Acceleration, UiValue::Float(f32::NAN)),
        (ExtraField::Rumble, UiValue::Bool(true)),
        (ExtraField::Device, UiValue::Index(7)),
        (ExtraField::RawLook, UiValue::Index(2)),
    ] {
        let before = extras.clone();
        assert_eq!(
            extras.write(0, field, value),
            BindResult::Denied,
            "{field:?}"
        );
        assert_eq!(extras, before);
    }
    assert_eq!(
        extras.write(9, ExtraField::Rumble, UiValue::Float(0.0)),
        BindResult::Ignored
    );
    extras.players[1].rumble = 0.5;
    assert_eq!(extras.players[1].scale_rumble(1000), 500);
}

#[test]
fn seeded_id_round_trip_and_clamp() {
    let mut state: u64 = 0x2211_0002;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut extras = InputExtras::new(4);
    for _ in 0..2000 {
        let player = (next() % 4) as u8;
        let field = ExtraField::ALL[(next() % 6) as usize];
        assert_eq!(
            ExtraField::decode(field.value_id(player)),
            Some((player, field))
        );
        let intent = tsfp_input::actions::INTENTS[(next() % 37) as usize].0;
        assert_eq!(
            decode_rebind(rebind_action(player, intent)),
            Some(RebindAction::Rebind(player, intent))
        );
        let v = f32::from_bits(next() as u32);
        let _ = extras.write(player, ExtraField::ScopeMultiplier, UiValue::Float(v));
        let s = extras.players[usize::from(player)].scope_multiplier;
        assert!((0.05..=4.0).contains(&s), "{v} -> {s}");
    }
    assert_eq!(
        decode_rebind(reset_bindings_action(2)),
        Some(RebindAction::Reset(2))
    );
    assert_eq!(ExtraField::decode(ValueId(EXTRAS_BASE - 1)), None);
}

#[test]
fn capture_rebinds_steals_and_cancels() {
    let mut extras = InputExtras::new(2);
    let mut game = RecordingBindings::default();
    {
        let mut b = InputExtrasBindings {
            extras: &mut extras,
            inner: &mut game,
        };
        assert_eq!(
            b.action(rebind_action(1, Intent::Reload), ActionArg::None),
            BindResult::Handled
        );
    }
    assert_eq!(extras.capturing, Some((1, Intent::Reload)));
    // KeyE is Use by default: binding it to Reload removes it from Use.
    let out = extras.capture(InputId::Key("KeyE".into()));
    assert_eq!(
        out,
        CaptureResult::Bound {
            intent: Intent::Reload,
            stolen_from: vec![Intent::Use]
        }
    );
    assert_eq!(extras.binding_text(1, Intent::Reload), "key:KeyE");
    assert_eq!(extras.binding_text(1, Intent::Use), "");
    assert_eq!(extras.binding_text(0, Intent::Reload), "key:KeyR");
    assert!(extras.dirty);
    assert_eq!(
        extras.capture(InputId::Key("KeyX".into())),
        CaptureResult::NotCapturing
    );
    extras.begin_capture(0, Intent::Fire);
    let before = extras.maps.clone();
    assert_eq!(
        extras.capture(InputId::Key("Escape".into())),
        CaptureResult::Cancelled
    );
    assert_eq!(extras.maps, before);
    // Reset covers extras and keymap of that player only.
    extras.players[1].rumble = 0.0;
    {
        let mut b = InputExtrasBindings {
            extras: &mut extras,
            inner: &mut game,
        };
        b.action(reset_bindings_action(1), ActionArg::None);
        assert_eq!(b.value(ExtraField::Rumble.value_id(1)), UiValue::Float(1.0));
        // Foreign ids pass through.
        b.set_value(ValueId(5), UiValue::Int(1));
    }
    assert_eq!(extras.maps[1], ActionMap::default_mouse_keyboard());
    assert_eq!(game.calls.len(), 1);
}

#[test]
fn rows_insert_contiguously_with_bindings() {
    let model = base();
    let extras = extras_rows(
        &model,
        1,
        &ExtrasRowsLayout {
            page: PageId(7),
            at: Insert::After(WidgetId(1)),
            first_widget: WidgetId::patch(60),
            labels: ExtraField::ALL.iter().map(|f| (*f, TextId(200))).collect(),
            off_on: [TextRef::Id(TextId(1)), TextRef::Id(TextId(2))],
            device_texts: vec![TextRef::Id(TextId(3)); 7],
            first_rect: rect(),
            row_step: 24.0,
        },
    );
    let rebind = rebind_rows(
        &model,
        1,
        &RebindRowsLayout {
            page: PageId(7),
            at: Insert::After(WidgetId(WidgetId::patch(60).0 + 5)),
            first_widget: WidgetId::patch(80),
            labels: vec![(Intent::Fire, TextId(300)), (Intent::Jump, TextId(301))],
            reset_label: Some(TextId(302)),
            first_rect: rect(),
            row_step: 24.0,
        },
    );
    assert_eq!(
        rebind.bindings.get(&(PageId(7), WidgetId::patch(80))),
        Some(&Binding::Action(rebind_action(1, Intent::Fire)))
    );
    assert_eq!(
        extras
            .bindings
            .get(&(PageId(7), WidgetId(WidgetId::patch(60).0 + 5))),
        Some(&Binding::Value(ExtraField::Device.value_id(1)))
    );
    let out = ExtensionPreset {
        name: "input".into(),
        layers: vec![extras, rebind],
    }
    .apply(&model)
    .unwrap();
    let ids: Vec<u64> = out.model.pages[&PageId(7)]
        .widgets
        .iter()
        .map(|w| w.id.0)
        .collect();
    let mut expect = vec![1];
    expect.extend((0..6).map(|n| WidgetId::patch(60).0 + n));
    expect.extend((0..3).map(|n| WidgetId::patch(80).0 + n));
    expect.push(2);
    assert_eq!(ids, expect);
}
