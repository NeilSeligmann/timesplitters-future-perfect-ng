use tsfp_format_profile::{
    Profile,
    settings::{Binding, HostSettings, OptionRow, Settings, SettingsPages},
};
use tsfp_ui_ext::{ExtensionPreset, profile_settings::*};
use tsfp_ui_input::{BindResult, RecordingBindings, UiBindings, UiValue, ValueId};
use tsfp_ui_model::*;
use tsfp_ui_renderer::Vec2;

struct Host;
impl HostSettings for Host {
    fn set(&mut self, _: u32, _: u32) -> Result<(), &'static str> {
        Ok(())
    }
}

#[test]
fn profile_rows_are_opt_in_and_write_typed_choices() {
    let row = OptionRow {
        page: 7,
        row: 4,
        label_id: 55,
        binding: Binding::ProfileWord { offset: 8 },
        choices: vec![10, 20],
        default: 10,
    };
    let mut pages = SettingsPages {
        defaults: Settings::from_head(&[0; 0x7c]).unwrap(),
        rows: vec![],
    };
    pages.add_row(row).unwrap();
    let mut model = UiModel::new();
    let mut page = Page::new(PageId(7), 1);
    page.widgets
        .push(Widget::new(WidgetId(1), WidgetKind::Button));
    model.pages.insert(page.id, page);
    let layout = ProfileRowLayout {
        ordinal: 0,
        page: PageId(7),
        at: Insert::After(WidgetId(1)),
        widget: WidgetId::patch(7),
        rect: menu_rect(Vec2::new(0., 0.), Vec2::new(200., 20.)),
        choice_text_ids: vec![TextId(70), TextId(71)],
    };
    assert_eq!(
        ExtensionPreset::original().apply(&model).unwrap().model,
        model
    );
    assert!(
        profile_rows(
            &model,
            &pages,
            &[ProfileRowLayout {
                choice_text_ids: vec![],
                ..layout.clone()
            }]
        )
        .is_err()
    );
    let ext = profile_rows(&model, &pages, &[layout]).unwrap();
    let output = ExtensionPreset {
        name: "profile".into(),
        layers: vec![ext],
    }
    .apply(&model)
    .unwrap();
    assert_eq!(output.model.pages[&PageId(7)].widgets.len(), 2);
    let mut profile = Profile {
        payload: vec![0; 0x1f54],
        signature: [0; 20],
    };
    profile.payload[8..12].copy_from_slice(&10u32.to_le_bytes());
    let mut host = Host;
    let mut inner = RecordingBindings::default();
    let mut binding = ProfileSettingsBindings {
        pages: &pages,
        profile: &mut profile,
        host: &mut host,
        inner: &mut inner,
    };
    assert_eq!(binding.value(ValueId(VALUE_BASE)), UiValue::Index(0));
    assert_eq!(
        binding.set_value(ValueId(VALUE_BASE), UiValue::Index(1)),
        BindResult::Handled
    );
    assert_eq!(binding.value(ValueId(VALUE_BASE)), UiValue::Index(1));
    assert_eq!(
        binding.set_value(ValueId(VALUE_BASE), UiValue::Index(2)),
        BindResult::Denied
    );
    assert_eq!(
        u32::from_le_bytes(profile.payload[8..12].try_into().unwrap()),
        20
    );
}
