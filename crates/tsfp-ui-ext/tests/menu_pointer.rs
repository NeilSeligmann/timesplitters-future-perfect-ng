//! T2168 menu pointer routing as an opt-in extension layer, end to end through UiInput.
use std::collections::BTreeMap;
use tsfp_ui_ext::*;
use tsfp_ui_input::{
    ActionArg, ActionId, BindResult, ContextId, ModelLayout, Ownership, PointerButtons,
    PointerFrame, PointerPolicy, RecordedCall, RecordingBindings, Routed, UiEvent, UiInput,
    WidgetId as InputWidgetId,
};
use tsfp_ui_model::*;

const BACK: ActionId = ActionId(7);
const ON: MenuPointer = MenuPointer {
    hover_focus: true,
    click_activate: true,
    right_back: true,
    wheel_step: 1,
};

fn base() -> UiModel {
    let mut model = UiModel::new();
    let mut page = Page::new(PageId(1), 1);
    page.handlers.back = Some(HandlerRef::Named("back".into()));
    model.pages.insert(page.id, page);
    model
}

fn extended(routing: Option<MenuPointer>) -> ExtendedModel {
    let base = base();
    let actions = BTreeMap::from([(PageId(1), BACK)]);
    let mut layers = vec![back_buttons(&base, TextId(1), WidgetId::patch(1), &actions)];
    if let Some(routing) = routing {
        layers.push(menu_pointer(&base, routing));
    }
    ExtensionPreset {
        name: "extras".into(),
        layers,
    }
    .apply(&base)
    .unwrap()
}

fn frame(x: f32, y: f32, left: bool, right: bool) -> PointerFrame {
    PointerFrame {
        pos: Some((x, y)),
        buttons: PointerButtons {
            left,
            right,
            middle: false,
        },
        wheel: 0,
    }
}

/// Runs a pointer stream over page 1 at 1280x720 and returns routed events and binding calls.
fn run(ext: &ExtendedModel, stream: &[PointerFrame]) -> (Vec<Routed>, RecordingBindings) {
    let page = ext.model.page(PageId(1)).unwrap();
    let layout = ModelLayout::from_page(page, 1280.0, 720.0).unwrap();
    let mut input = UiInput::new();
    input.open_context(ContextId(0), Ownership::Any);
    if let Some(policy) = ext.menu_pointer_policy(&PointerPolicy::default()) {
        input.pointer_policy = policy;
    }
    let mut out = Vec::new();
    for (n, raw) in stream.iter().enumerate() {
        let filtered = ext.menu_pointer_frame(*raw);
        input.update(
            ContextId(0),
            n as u64 * 16,
            &layout,
            &[],
            filtered.as_ref(),
            &mut out,
        );
    }
    let mut bindings = RecordingBindings::default();
    for routed in &out {
        ext.route(PageId(1), &routed.event, &mut bindings);
    }
    (out, bindings)
}

// The Back button sits bottom-left of the 4:3 safe area: design x 16..112 plus the
// widescreen offset (106.7 at 1280x720), y 440..464.
fn click_back() -> Vec<PointerFrame> {
    vec![
        frame(170.0, 450.0, false, false),
        frame(170.0, 450.0, true, false),
        frame(170.0, 450.0, false, false),
    ]
}

#[test]
fn original_preset_routes_no_pointer() {
    let original = ExtensionPreset::original().apply(&base()).unwrap();
    assert_eq!(original.menu_pointer, None);
    assert_eq!(
        original.menu_pointer_policy(&PointerPolicy::default()),
        None
    );
    let ext = extended(None);
    let (events, bindings) = run(&ext, &click_back());
    assert!(events.is_empty());
    assert!(bindings.calls.is_empty());
}

#[test]
fn hover_focuses_and_click_activates_back_overlay() {
    let ext = extended(Some(ON));
    let id = InputWidgetId(WidgetId::patch(1).0);
    let (events, bindings) = run(&ext, &click_back());
    let kinds: Vec<_> = events.iter().map(|r| r.event.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            UiEvent::Hover(id),
            UiEvent::Focus(id),
            UiEvent::Activate(id)
        ]
    );
    assert_eq!(
        bindings.calls,
        vec![RecordedCall::Action(BACK, ActionArg::None)]
    );
}

#[test]
fn disabled_gestures_are_masked() {
    let ext = extended(Some(MenuPointer {
        hover_focus: false,
        click_activate: false,
        right_back: false,
        wheel_step: 0,
    }));
    let mut stream = click_back();
    stream.push(frame(170.0, 450.0, false, true));
    stream[0].wheel = 3;
    let (events, bindings) = run(&ext, &stream);
    let id = InputWidgetId(WidgetId::patch(1).0);
    // Hover is still reported for highlight, but nothing focuses, activates or goes back.
    assert_eq!(
        events.iter().map(|r| r.event.clone()).collect::<Vec<_>>(),
        vec![UiEvent::Hover(id)]
    );
    assert!(bindings.calls.is_empty());
}

#[test]
fn right_click_is_back_and_clicks_off_target_do_nothing() {
    let ext = extended(Some(ON));
    let stream = [
        frame(400.0, 100.0, false, false),
        frame(400.0, 100.0, true, false),
        frame(400.0, 100.0, false, false),
        frame(400.0, 100.0, false, true),
    ];
    let (events, bindings) = run(&ext, &stream);
    assert_eq!(
        events.iter().map(|r| r.event.clone()).collect::<Vec<_>>(),
        vec![UiEvent::Back]
    );
    // Back without a widget goes to the host's page handler, not an extension binding.
    assert!(bindings.calls.is_empty());
    let mut denied = RecordingBindings::default();
    denied.denied.push(BACK);
    let id = InputWidgetId(WidgetId::patch(1).0);
    assert_eq!(
        ext.route(PageId(1), &UiEvent::Activate(id), &mut denied),
        BindResult::Denied
    );
    assert_eq!(
        ext.route(PageId(2), &UiEvent::Activate(id), &mut denied),
        BindResult::Ignored
    );
}

#[test]
fn routing_layers_conflict_atomically_and_pair_with_auto_release() {
    let base = base();
    let other = MenuPointer {
        wheel_step: 2,
        ..ON
    };
    let clash = ExtensionPreset {
        name: "clash".into(),
        layers: vec![menu_pointer(&base, ON), menu_pointer(&base, other)],
    };
    assert_eq!(clash.apply(&base), Err(ExtensionError::MenuPointerConflict));
    let flags = PointerFlags {
        visible: true,
        auto_release: true,
    };
    let both = ExtensionPreset {
        name: "pointer".into(),
        layers: vec![
            menu_pointer(&base, ON),
            menu_pointer(&base, ON),
            pointer_policy(&base, flags),
        ],
    }
    .apply(&base)
    .unwrap();
    assert_eq!(both.menu_pointer, Some(ON));
    assert_eq!(both.pointer, Some(flags));
    assert_eq!(both.model.pages, base.pages);
    let policy = ON.policy(&PointerPolicy {
        idle_hide_ms: 99,
        ..PointerPolicy::default()
    });
    assert_eq!(policy.idle_hide_ms, 99);
    assert_eq!(policy.wheel_step, 1);
}
