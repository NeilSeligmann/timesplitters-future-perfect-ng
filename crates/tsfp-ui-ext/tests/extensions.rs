use std::collections::BTreeMap;
use tsfp_ui_ext::*;
use tsfp_ui_input::{ActionId, BindResult, RecordedCall, RecordingBindings, UiValue, ValueId};
use tsfp_ui_model::*;
use tsfp_ui_renderer::{AnchorSpace, Rect, Vec2, Viewport};

fn preset(layers: Vec<Extension>) -> ExtensionPreset {
    ExtensionPreset {
        name: "extras".into(),
        layers,
    }
}
fn synthetic(seed: u64) -> UiModel {
    let mut model = UiModel::new();
    for n in 0..seed % 17 + 1 {
        let mut page = Page::new(PageId(n), 1);
        if n % 3 != 0 {
            page.handlers.back = Some(HandlerRef::Named(format!("back-{n}")));
        }
        for row in 0..(seed + n) % 9 {
            let mut w = Widget::new(WidgetId(row), WidgetKind::Button);
            if row == 0 && n % 5 == 0 {
                w.role = WidgetRole::Back;
            }
            page.widgets.push(w);
        }
        model.pages.insert(page.id, page);
    }
    model
}
fn properties(base: &UiModel) {
    let snapshot = base.clone();
    assert_eq!(
        ExtensionPreset::original().apply(base).unwrap().model,
        *base
    );
    let actions = base.pages.keys().map(|id| (*id, ActionId(77))).collect();
    let layer = back_buttons(base, TextId(123), WidgetId::patch(1), &actions);
    let p = preset(vec![layer]);
    let result = p.apply(base).unwrap();
    assert_eq!(*base, snapshot);
    assert_eq!(p.apply(&result.model).unwrap().model, result.model);
    assert_eq!(p.apply(base).unwrap(), result);
    for page in base.pages.values() {
        let added = page.handlers.back.is_some()
            && !page.widgets.iter().any(|w| w.role == WidgetRole::Back);
        let after = &result.model.pages[&page.id];
        assert_eq!(after.widgets.len(), page.widgets.len() + usize::from(added));
        assert_eq!(&after.widgets[..page.widgets.len()], &page.widgets);
        if added {
            let w = after.widgets.last().unwrap();
            assert_eq!(w.activate, page.handlers.back);
            assert_eq!(w.text, TextRef::Id(TextId(123)));
            assert_eq!(w.rect.space, AnchorSpace::SafeArea4x3);
            let mut b = RecordingBindings::default();
            assert_eq!(
                result.activate((page.id, w.id), &mut b),
                BindResult::Handled
            );
            assert_eq!(
                b.calls,
                vec![RecordedCall::Action(
                    ActionId(77),
                    tsfp_ui_input::ActionArg::None
                )]
            );
            for width in [640., 1280., 2560.] {
                let safe =
                    Rect::new(Vec2::new((width - 640.) / 2., 0.), Vec2::new(640., 480.)).unwrap();
                let r = Viewport::new(Vec2::new(width, 480.), 1., safe)
                    .unwrap()
                    .resolve(w.rect)
                    .unwrap();
                assert!(r.origin.x >= safe.origin.x && r.origin.y >= 0.);
                assert!(r.origin.x + r.size.x <= safe.origin.x + safe.size.x);
                assert!(r.origin.y + r.size.y <= 480.);
            }
        }
    }
}
#[test]
fn synthetic_properties() {
    for seed in 0..2000 {
        properties(&synthetic(seed));
    }
}
#[test]
fn captured_properties_when_available() {
    let path = std::env::var_os("TSFP_UI_TREE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/ui-tree/tree.json")
        });
    if !path.exists() {
        eprintln!("SKIP real captured tree: unavailable {}", path.display());
        return;
    }
    let text = std::fs::read_to_string(path).unwrap();
    let options = LoadOptions {
        rect_edges: Some(RectEdges::DUMP),
        ignore_row_enabled_flag: true,
        ..Default::default()
    };
    let base = UiModel::from_tree_json(&text, &options).unwrap();
    assert!(!base.pages.is_empty());
    properties(&base);
}
#[test]
fn bindings_text_and_anchor() {
    let base = synthetic(5);
    let target = (PageId(1), WidgetId::patch(3));
    let rect = menu_rect(Vec2::new(30., 30.), Vec2::new(200., 24.));
    let row = SettingRow {
        page: target.0,
        id: target.1,
        at: Insert::End,
        label: TextId(1),
        rect,
        value: ValueId(4),
        kind: SettingKind::Cycler(vec![UiOption {
            id: 0,
            text: TextRef::Id(TextId(2)),
            enabled: true,
        }]),
    };
    let text = TextRow {
        page: target.0,
        id: WidgetId::patch(4),
        at: Insert::End,
        label: TextId(3),
        rect,
        commit: ActionId(9),
        initial: b"a".to_vec(),
        limit: 2,
        charset: b"ab ".to_vec(),
    };
    let p = preset(vec![
        settings_rows(&base, &[row]),
        text_row(&base, &text).unwrap(),
        pointer_policy(
            &base,
            PointerFlags {
                visible: true,
                auto_release: true,
            },
        ),
    ]);
    let result = p.apply(&base).unwrap();
    let mut bindings = RecordingBindings::default();
    assert_eq!(
        result.set_value(target, UiValue::Bool(true), &mut bindings),
        BindResult::Handled
    );
    assert_eq!(result.value(target, &bindings), UiValue::Bool(true));
    let (action, mut field) = result.text_field((text.page, text.id)).unwrap();
    field.apply(tsfp_text_field::Event::Character('b')).unwrap();
    assert_eq!(
        field.apply(tsfp_text_field::Event::Character('a')),
        Err(tsfp_text_field::EditError::TooLong)
    );
    struct Sink(Vec<u8>);
    impl TextBindings for Sink {
        fn commit_text(&mut self, _: ActionId, text: &[u8]) -> BindResult {
            self.0 = text.to_vec();
            BindResult::Handled
        }
    }
    let mut sink = Sink(Vec::new());
    assert_eq!(commit_text(action, &field, &mut sink), BindResult::Denied);
    field.apply(tsfp_text_field::Event::Confirm).unwrap();
    assert_eq!(commit_text(action, &field, &mut sink), BindResult::Handled);
    assert_eq!(sink.0, b"ab");
    assert_eq!(
        result.text_field((text.page, text.id)).unwrap().1.text(),
        b"a"
    );
    let anchor = anchoring_preset(&result.model, &[target], AnchoringPreset::HudWindowEdges);
    let anchored = preset(vec![anchor]).apply(&result.model).unwrap();
    assert_eq!(
        anchored
            .model
            .page(target.0)
            .unwrap()
            .widget(target.1)
            .unwrap()
            .rect
            .space,
        AnchorSpace::Window
    );
}
#[test]
fn conflicts_are_atomic() {
    let base = synthetic(5);
    let a = pointer_policy(
        &base,
        PointerFlags {
            visible: true,
            auto_release: true,
        },
    );
    let b = pointer_policy(&base, PointerFlags::default());
    assert_eq!(
        preset(vec![a.clone(), b]).apply(&base),
        Err(ExtensionError::PointerConflict)
    );
    assert!(preset(vec![a.clone(), a]).apply(&base).is_ok());
    let target = (PageId(1), WidgetId(0));
    let mut a = Extension::new("a");
    a.bindings.insert(target, Binding::Value(ValueId(1)));
    let mut b = a.clone();
    b.bindings.insert(target, Binding::Value(ValueId(2)));
    assert_eq!(
        preset(vec![a, b]).apply(&base),
        Err(ExtensionError::BindingConflict(target))
    );
    let mut targets = BTreeMap::new();
    targets.insert(target, menu_rect(Vec2::new(1., 1.), Vec2::new(2., 2.)));
    let a = anchoring(&base, &targets);
    targets.get_mut(&target).unwrap().offset.x = 2.;
    assert!(matches!(
        preset(vec![a, anchoring(&base, &targets)]).apply(&base),
        Err(ExtensionError::Patch(PatchError::Conflicts(_)))
    ));
}

#[test]
fn missing_targets_refuse_all_bindings_without_changing_base() {
    for seed in 0..256 {
        let base = synthetic(seed);
        let original = base.clone();
        let page = *base.pages.keys().next().unwrap();
        let absent = (page, WidgetId::patch(10_000 + seed as u32));
        let mut layer = Extension::new("missing-target");
        layer.bindings.insert(absent, Binding::Action(ActionId(7)));
        assert_eq!(
            preset(vec![layer.clone()]).apply(&base),
            Err(ExtensionError::MissingTarget(absent))
        );
        assert_eq!(base, original);

        let mut widget = Widget::new(absent.1, WidgetKind::Button);
        widget.role = WidgetRole::Back;
        layer.patch.ops.push(PatchOp::AddWidget {
            page: PageSelector::Id(page),
            at: Insert::End,
            widget,
            unless_role: None,
        });
        let applied = preset(vec![layer]).apply(&base).unwrap();
        assert!(applied.model.page(page).unwrap().widget(absent.1).is_some());
        assert_eq!(applied.bindings[&absent], Binding::Action(ActionId(7)));
        assert_eq!(base, original);
    }
}
