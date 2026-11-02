//! T2211 input settings rows: Tier 0 smoke plus seeded property tests (FABRICATED inputs).
use tsfp_input::shaping::PlayerSettings;
use tsfp_ui_ext::input_settings::*;
use tsfp_ui_ext::{Binding, ExtensionPreset};
use tsfp_ui_input::{ActionArg, BindResult, RecordingBindings, UiBindings, UiValue, ValueId};
use tsfp_ui_model::*;
use tsfp_ui_renderer::Vec2;

fn presets() -> Vec<String> {
    ["original", "modern", "mouse-raw"]
        .map(String::from)
        .to_vec()
}
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
fn layout(fields: &[InputField]) -> InputRowsLayout {
    InputRowsLayout {
        page: PageId(7),
        at: Insert::After(WidgetId(1)),
        first_widget: WidgetId::patch(40),
        labels: fields
            .iter()
            .enumerate()
            .map(|(n, f)| (*f, TextId(100 + n as u32)))
            .collect(),
        off_on: [TextRef::Id(TextId(1)), TextRef::Id(TextId(2))],
        preset_texts: vec![TextRef::Id(TextId(3)); 3],
        first_rect: menu_rect(Vec2::new(10., 50.), Vec2::new(300., 20.)),
        row_step: 24.0,
    }
}

#[test]
fn defaults_are_todays_behaviour() {
    let settings = InputSettings::new(4, presets());
    assert_eq!(settings.players.len(), 4);
    assert!(
        settings
            .players
            .iter()
            .all(|p| *p == PlayerSettings::default())
    );
    assert_eq!(settings.read(0, InputField::Preset), UiValue::Index(0));
    assert_eq!(settings.read(0, InputField::DeadZone), UiValue::None);
    assert!(!settings.dirty);
    let mut edited = settings.clone();
    assert_eq!(
        edited.write(0, InputField::InvertX, UiValue::Bool(true)),
        BindResult::Handled
    );
    assert!(edited.dirty);
    // Opt-in: without the layer the model is untouched.
    assert_eq!(
        ExtensionPreset::original().apply(&base()).unwrap().model,
        base()
    );
}

#[test]
fn rows_insert_contiguously_and_bind_the_player_values() {
    let model = base();
    let ext = input_rows(&model, 2, &layout(&InputField::ALL));
    let preset = ExtensionPreset {
        name: "input".into(),
        layers: vec![ext],
    };
    let out = preset.apply(&model).unwrap();
    let page = &out.model.pages[&PageId(7)];
    let ids: Vec<u64> = page.widgets.iter().map(|w| w.id.0).collect();
    let mut expect = vec![1];
    expect.extend((0..7).map(|n| WidgetId::patch(40).0 + n));
    expect.push(2);
    assert_eq!(ids, expect);
    for (n, field) in InputField::ALL.iter().enumerate() {
        let id = WidgetId(WidgetId::patch(40).0 + n as u64);
        assert_eq!(
            out.bindings[&(PageId(7), id)],
            Binding::Value(field.value_id(2))
        );
        let w = page.widgets.iter().find(|w| w.id == id).unwrap();
        assert_eq!(w.rect.offset.y, 50.0 + 24.0 * n as f32);
        let slider = field.range().is_some();
        assert_eq!(w.kind == WidgetKind::Slider, slider, "{field:?}");
    }
    // Only the requested fields get rows.
    let ext = input_rows(&model, 0, &layout(&[InputField::SensitivityX]));
    assert_eq!(ext.patch.ops.len(), 1);
}

#[test]
fn edits_route_through_bindings_and_others_delegate() {
    let mut settings = InputSettings::new(2, presets());
    let mut game = RecordingBindings::default();
    let mut b = InputSettingsBindings {
        settings: &mut settings,
        inner: &mut game,
    };
    let sx = InputField::SensitivityX.value_id(1);
    assert_eq!(b.set_value(sx, UiValue::Float(2.5)), BindResult::Handled);
    assert_eq!(b.value(sx), UiValue::Float(2.5));
    assert_eq!(b.set_value(sx, UiValue::Float(500.0)), BindResult::Handled);
    assert_eq!(b.value(sx), UiValue::Float(100.0));
    assert_eq!(
        b.set_value(sx, UiValue::Float(f32::NAN)),
        BindResult::Denied
    );
    assert_eq!(b.set_value(sx, UiValue::Bool(true)), BindResult::Denied);
    let inv = InputField::InvertY.value_id(1);
    assert_eq!(b.set_value(inv, UiValue::Index(1)), BindResult::Handled);
    assert_eq!(b.value(inv), UiValue::Bool(true));
    assert_eq!(b.set_value(inv, UiValue::Index(2)), BindResult::Denied);
    let pre = InputField::Preset.value_id(1);
    assert_eq!(b.set_value(pre, UiValue::Index(2)), BindResult::Handled);
    assert_eq!(b.set_value(pre, UiValue::Index(3)), BindResult::Denied);
    let dz = InputField::DeadZone.value_id(1);
    assert_eq!(b.set_value(dz, UiValue::Float(0.3)), BindResult::Handled);
    assert_eq!(b.set_value(dz, UiValue::None), BindResult::Handled);
    assert_eq!(b.value(dz), UiValue::None);
    // Foreign ids and actions reach the game untouched.
    assert_eq!(b.set_value(ValueId(5), UiValue::Int(1)), game_result());
    assert_eq!(
        b.action(reset_action(1), ActionArg::None),
        BindResult::Handled
    );
    assert_eq!(settings.players[1], PlayerSettings::default());
    assert!(settings.dirty);
    assert_eq!(game.values.get(&ValueId(5)), Some(&UiValue::Int(1)));
}
fn game_result() -> BindResult {
    let mut g = RecordingBindings::default();
    g.set_value(ValueId(5), UiValue::Int(1))
}

#[test]
fn reset_touches_one_player_only() {
    let mut settings = InputSettings::new(3, presets());
    for p in 0..3 {
        settings.write(p, InputField::SensitivityY, UiValue::Float(3.0));
    }
    settings.reset(1);
    assert_eq!(
        settings.read(0, InputField::SensitivityY),
        UiValue::Float(3.0)
    );
    assert_eq!(
        settings.read(1, InputField::SensitivityY),
        UiValue::Float(1.0)
    );
    assert_eq!(
        settings.read(2, InputField::SensitivityY),
        UiValue::Float(3.0)
    );
    assert_eq!(settings.reset(9), BindResult::Ignored);
}

#[test]
fn seeded_ids_round_trip_and_floats_stay_in_range() {
    let mut x: u64 = 0x2211;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut settings = InputSettings::new(4, presets());
    for _ in 0..2000 {
        let player = (next() % 4) as u8;
        let field = InputField::ALL[(next() % 7) as usize];
        assert_eq!(
            InputField::decode(field.value_id(player)),
            Some((player, field))
        );
        assert_ne!(reset_action(player).0, field.value_id(player).0);
        let v = f32::from_bits(next() as u32);
        let r = settings.write(player, field, UiValue::Float(v));
        if let (Some((low, high)), BindResult::Handled) = (field.range(), r) {
            let UiValue::Float(got) = settings.read(player, field) else {
                panic!()
            };
            assert!(got >= low && got <= high, "{field:?} {got}");
            assert_eq!(got, v.clamp(low, high));
        } else {
            assert_eq!(r, BindResult::Denied);
        }
    }
    assert_eq!(InputField::decode(ValueId(5)), None);
}
