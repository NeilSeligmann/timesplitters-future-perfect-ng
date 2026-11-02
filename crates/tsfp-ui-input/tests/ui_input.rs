use tsfp_ui_input::*;

struct W {
    id: u64,
    r: Rect,
    layer: i32,
    enabled: bool,
    focusable: bool,
    list: Option<ListId>,
}
fn w(id: u64, x: f32, y: f32, ww: f32, h: f32) -> W {
    W {
        id,
        r: Rect::new(x, y, ww, h),
        layer: 0,
        enabled: true,
        focusable: true,
        list: None,
    }
}
impl HitTarget for W {
    fn id(&self) -> WidgetId {
        WidgetId(self.id)
    }
    fn rect(&self) -> Rect {
        self.r
    }
    fn layer(&self) -> i32 {
        self.layer
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn focusable(&self) -> bool {
        self.focusable
    }
    fn scroll_list(&self) -> Option<ListId> {
        self.list
    }
}
struct L(Vec<W>);
impl UiLayout for L {
    fn visit(&self, f: &mut dyn FnMut(&dyn HitTarget)) {
        for t in &self.0 {
            f(t)
        }
    }
}
/// Vertical column of three buttons: 1,2,3.
fn column() -> L {
    L(vec![
        w(1, 100.0, 100.0, 200.0, 40.0),
        w(2, 100.0, 150.0, 200.0, 40.0),
        w(3, 100.0, 200.0, 200.0, 40.0),
    ])
}
const C: ContextId = ContextId(0);
fn held(i: MenuIntent) -> Vec<PlayerFrame> {
    vec![PlayerFrame {
        held: IntentSet::default().with(i),
        chars: vec![],
    }]
}
fn none() -> Vec<PlayerFrame> {
    vec![PlayerFrame::default()]
}
fn run(
    ui: &mut UiInput,
    l: &L,
    t: u64,
    p: &[PlayerFrame],
    ptr: Option<&PointerFrame>,
) -> Vec<UiEvent> {
    let mut out = vec![];
    ui.update(C, t, l, p, ptr, &mut out);
    out.into_iter().map(|r| r.event).collect()
}
fn ui() -> UiInput {
    let mut u = UiInput::new();
    u.open_context(C, Ownership::Any);
    u
}

#[test]
fn nav_down_up_accept_back() {
    let (l, mut u) = (column(), ui());
    assert_eq!(
        run(&mut u, &l, 0, &held(MenuIntent::Down), None),
        vec![UiEvent::Focus(WidgetId(1))]
    );
    run(&mut u, &l, 1, &none(), None);
    assert_eq!(
        run(&mut u, &l, 2, &held(MenuIntent::Down), None),
        vec![UiEvent::Focus(WidgetId(2))]
    );
    run(&mut u, &l, 3, &none(), None);
    assert_eq!(
        run(&mut u, &l, 4, &held(MenuIntent::Accept), None),
        vec![UiEvent::Activate(WidgetId(2))]
    );
    run(&mut u, &l, 5, &none(), None);
    assert_eq!(
        run(&mut u, &l, 6, &held(MenuIntent::Back), None),
        vec![UiEvent::Back]
    );
    run(&mut u, &l, 7, &none(), None);
    assert_eq!(
        run(&mut u, &l, 8, &held(MenuIntent::Backspace), None),
        vec![UiEvent::Back]
    );
}

#[test]
fn wrap_and_no_wrap() {
    let l = column();
    let mut u = ui();
    u.set_focus(C, Some(WidgetId(3)));
    assert_eq!(
        run(&mut u, &l, 0, &held(MenuIntent::Down), None),
        vec![UiEvent::Focus(WidgetId(1))]
    );
    let mut u = ui();
    u.wrap_focus = false;
    u.set_focus(C, Some(WidgetId(3)));
    assert!(run(&mut u, &l, 0, &held(MenuIntent::Down), None).is_empty());
}

#[test]
fn disabled_and_unfocusable_skipped() {
    let mut l = column();
    l.0[1].enabled = false;
    let mut u = ui();
    u.set_focus(C, Some(WidgetId(1)));
    assert_eq!(
        run(&mut u, &l, 0, &held(MenuIntent::Down), None),
        vec![UiEvent::Focus(WidgetId(3))]
    );
    l.0[2].focusable = false;
    let mut u = ui();
    u.wrap_focus = false;
    u.set_focus(C, Some(WidgetId(1)));
    assert!(run(&mut u, &l, 0, &held(MenuIntent::Down), None).is_empty());
}

#[test]
fn geometric_grid_and_explicit_neighbour() {
    // 2x2 grid ids 1 2 / 3 4
    let g = L(vec![
        w(1, 0.0, 0.0, 50.0, 50.0),
        w(2, 100.0, 0.0, 50.0, 50.0),
        w(3, 0.0, 100.0, 50.0, 50.0),
        w(4, 100.0, 100.0, 50.0, 50.0),
    ]);
    let mut u = ui();
    u.set_focus(C, Some(WidgetId(1)));
    assert_eq!(
        run(&mut u, &g, 0, &held(MenuIntent::Right), None),
        vec![UiEvent::Focus(WidgetId(2))]
    );
    run(&mut u, &g, 1, &none(), None);
    assert_eq!(
        run(&mut u, &g, 2, &held(MenuIntent::Down), None),
        vec![UiEvent::Focus(WidgetId(4))]
    );
    struct Linked(L);
    impl UiLayout for Linked {
        fn visit(&self, f: &mut dyn FnMut(&dyn HitTarget)) {
            self.0.visit(f)
        }
        fn neighbour(&self, from: WidgetId, d: Dir) -> Option<WidgetId> {
            (from == WidgetId(1) && d == Dir::Right).then_some(WidgetId(4))
        }
    }
    let lk = Linked(g);
    let mut u = ui();
    u.set_focus(C, Some(WidgetId(1)));
    let mut out = vec![];
    u.update(C, 0, &lk, &held(MenuIntent::Right), None, &mut out);
    assert_eq!(out[0].event, UiEvent::Focus(WidgetId(4)));
}

#[test]
fn tab_cycles() {
    let (l, mut u) = (column(), ui());
    assert_eq!(
        run(&mut u, &l, 0, &held(MenuIntent::Tab), None),
        vec![UiEvent::Focus(WidgetId(1))]
    );
    run(&mut u, &l, 1, &none(), None);
    assert_eq!(
        run(&mut u, &l, 2, &held(MenuIntent::BackTab), None),
        vec![UiEvent::Focus(WidgetId(3))]
    );
}

#[test]
fn repeat_with_fake_clock_and_acceleration() {
    let (l, mut u) = (column(), ui());
    let h = held(MenuIntent::Down);
    assert_eq!(run(&mut u, &l, 0, &h, None).len(), 1);
    assert!(run(&mut u, &l, 349, &h, None).is_empty());
    // first repeat at 350, then every 120 ms: 350, 470 -> by 480 two repeats
    assert_eq!(run(&mut u, &l, 480, &h, None).len(), 2);
    // release stops repeats
    run(&mut u, &l, 481, &none(), None);
    assert!(run(&mut u, &l, 5000, &none(), None).is_empty());
    // acceleration: held 3000 ms produces more than the slow interval alone would
    let mut u = ui();
    run(&mut u, &l, 0, &h, None);
    let mut total = 0;
    for t in (0..=3000).step_by(10) {
        total += run(&mut u, &l, t, &h, None).len();
    }
    let policy = RepeatPolicy::default();
    assert!(
        total as u64 > (3000 - policy.initial_delay_ms) / 120,
        "total {total}"
    );
    assert_eq!(policy.interval_at(0), 120);
    assert_eq!(policy.interval_at(1000), 60);
    assert_eq!(policy.interval_at(9999), 30);
}

#[test]
fn stalled_frame_is_capped() {
    let (l, mut u) = (column(), ui());
    let h = held(MenuIntent::Down);
    run(&mut u, &l, 0, &h, None);
    assert!(run(&mut u, &l, 600_000, &h, None).len() <= u.repeat.max_per_update as usize);
}

fn ptr(x: f32, y: f32, left: bool, right: bool, wheel: i32) -> PointerFrame {
    PointerFrame {
        pos: Some((x, y)),
        buttons: PointerButtons {
            left,
            right,
            middle: false,
        },
        wheel,
    }
}

#[test]
fn hover_focus_click_rightclick() {
    let (l, mut u) = (column(), ui());
    let ev = run(
        &mut u,
        &l,
        0,
        &none(),
        Some(&ptr(150.0, 160.0, false, false, 0)),
    );
    assert_eq!(
        ev,
        vec![UiEvent::Hover(WidgetId(2)), UiEvent::Focus(WidgetId(2))]
    );
    assert!(u.wants_pointer(C));
    run(
        &mut u,
        &l,
        1,
        &none(),
        Some(&ptr(150.0, 160.0, true, false, 0)),
    );
    assert_eq!(
        run(
            &mut u,
            &l,
            2,
            &none(),
            Some(&ptr(150.0, 160.0, false, false, 0))
        ),
        vec![UiEvent::Activate(WidgetId(2))]
    );
    assert_eq!(
        run(
            &mut u,
            &l,
            3,
            &none(),
            Some(&ptr(150.0, 160.0, false, true, 0))
        ),
        vec![UiEvent::Back]
    );
    // press on one widget, release on another: no activation
    run(
        &mut u,
        &l,
        4,
        &none(),
        Some(&ptr(150.0, 110.0, true, false, 0)),
    );
    let ev = run(
        &mut u,
        &l,
        5,
        &none(),
        Some(&ptr(150.0, 210.0, false, false, 0)),
    );
    assert!(!ev.iter().any(|e| matches!(e, UiEvent::Activate(_))));
    assert_eq!(
        run(
            &mut u,
            &l,
            6,
            &none(),
            Some(&ptr(5.0, 5.0, false, false, 0))
        ),
        vec![UiEvent::HoverLeave]
    );
}

#[test]
fn pointer_idle_hide_and_hotswap() {
    let (l, mut u) = (column(), ui());
    run(
        &mut u,
        &l,
        0,
        &none(),
        Some(&ptr(150.0, 160.0, false, false, 0)),
    );
    assert!(u.wants_pointer(C));
    run(
        &mut u,
        &l,
        3001,
        &none(),
        Some(&ptr(150.0, 160.0, false, false, 0)),
    );
    assert!(!u.wants_pointer(C));
    run(
        &mut u,
        &l,
        3002,
        &none(),
        Some(&ptr(160.0, 160.0, false, false, 0)),
    );
    assert!(u.wants_pointer(C));
    run(
        &mut u,
        &l,
        3003,
        &held(MenuIntent::Down),
        Some(&ptr(160.0, 160.0, false, false, 0)),
    );
    assert!(!u.wants_pointer(C));
}

#[test]
fn wheel_and_page_scroll() {
    let mut l = column();
    for t in &mut l.0 {
        t.list = Some(ListId(9));
    }
    let mut u = ui();
    assert_eq!(
        run(
            &mut u,
            &l,
            0,
            &none(),
            Some(&ptr(150.0, 160.0, false, false, 2))
        )
        .last(),
        Some(&UiEvent::Scroll(ListId(9), -2))
    );
    assert_eq!(
        run(&mut u, &l, 1, &held(MenuIntent::PageDown), None),
        vec![UiEvent::Scroll(ListId(9), 5)]
    );
}

#[test]
fn per_player_ownership() {
    let l = column();
    let mut u = UiInput::new();
    u.open_context(C, Ownership::Player(PlayerId(1)));
    u.open_context(ContextId(1), Ownership::Any);
    let mut frames = vec![PlayerFrame::default(), PlayerFrame::default()];
    frames[0].held = IntentSet::default().with(MenuIntent::Down);
    let mut out = vec![];
    u.update(C, 0, &l, &frames, None, &mut out);
    assert!(out.is_empty(), "player 0 does not own context 0");
    frames[0].held = IntentSet::default();
    frames[1].held = IntentSet::default().with(MenuIntent::Down);
    u.update(C, 1, &l, &frames, None, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].player, out[0].context), (PlayerId(1), C));
    // pointer player 0 cannot drive a context owned by player 1
    let mut out = vec![];
    u.update(
        C,
        2,
        &l,
        &none(),
        Some(&ptr(150.0, 160.0, false, false, 0)),
        &mut out,
    );
    assert!(out.is_empty());
    u.pointer_player = PlayerId(1);
    u.update(
        C,
        3,
        &l,
        &none(),
        Some(&ptr(150.0, 160.0, false, false, 0)),
        &mut out,
    );
    assert!(!out.is_empty());
    // independent focus per context
    assert_ne!(u.focus(C), u.focus(ContextId(1)));
}

fn text_spec() -> TextEntrySpec {
    TextEntrySpec {
        initial: b"AB".to_vec(),
        limit: 4,
        charset: b"ABCDEFG".to_vec(),
    }
}

#[test]
fn text_entry_commit_and_cancel() {
    let (l, mut u) = (column(), ui());
    assert!(u.begin_text(C, WidgetId(1), &text_spec()));
    let mut f = PlayerFrame::default();
    f.chars = vec!['C', 'z'];
    let ev = run(&mut u, &l, 0, &[f], None);
    assert_eq!(
        ev,
        vec![UiEvent::TextInput('C')],
        "invalid char rejected by text field"
    );
    run(&mut u, &l, 1, &none(), None);
    run(&mut u, &l, 2, &held(MenuIntent::Backspace), None);
    run(&mut u, &l, 3, &none(), None);
    let ev = run(&mut u, &l, 4, &held(MenuIntent::Accept), None);
    assert_eq!(
        ev,
        vec![UiEvent::TextCommit {
            widget: WidgetId(1),
            text: b"AB".to_vec()
        }]
    );
    assert!(!u.text_active(C));
    u.begin_text(C, WidgetId(2), &text_spec());
    run(&mut u, &l, 5, &none(), None);
    assert_eq!(
        run(&mut u, &l, 6, &held(MenuIntent::Back), None),
        vec![UiEvent::TextCancel(WidgetId(2))]
    );
    // right click cancels too
    u.begin_text(C, WidgetId(2), &text_spec());
    assert_eq!(
        run(&mut u, &l, 7, &none(), Some(&ptr(1.0, 1.0, false, true, 0))),
        vec![UiEvent::TextCancel(WidgetId(2))]
    );
}

#[test]
fn text_entry_uses_runtime_width_before_emitting_input() {
    struct Width;
    impl tsfp_text_field::TextWidth for Width {
        fn fits(&mut self, bytes: &[u8]) -> bool {
            bytes.len() <= 2
        }
    }
    let (layout, mut input) = (column(), ui());
    assert!(input.begin_text(C, WidgetId(1), &text_spec()));
    let mut frame = PlayerFrame::default();
    frame.chars = vec!['C'];
    let mut routed = Vec::new();
    input.update_with_text_width(C, 0, &layout, &[frame], None, &mut Width, &mut routed);
    assert!(
        routed.is_empty(),
        "width rejected the otherwise valid character"
    );
    let committed = run(&mut input, &layout, 1, &[PlayerFrame::default()], None);
    assert!(committed.is_empty());
    let committed = run(&mut input, &layout, 2, &held(MenuIntent::Accept), None);
    assert_eq!(
        committed,
        vec![UiEvent::TextCommit {
            widget: WidgetId(1),
            text: b"AB".to_vec()
        }]
    );
}

#[test]
fn hit_test_properties() {
    let mut l = column();
    l.0.push({
        let mut o = w(10, 150.0, 110.0, 50.0, 200.0);
        o.layer = 5;
        o
    });
    // deterministic LCG sweep
    let mut s: u64 = 0x1234_5678;
    let mut next = || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (s >> 33) as f32 / 2147483648.0
    };
    let mut hits = 0;
    for _ in 0..5000 {
        let (x, y) = (next() * 700.0 - 30.0, next() * 520.0 - 20.0);
        let got = hit_test(&l, x, y);
        let inside: Vec<&W> = l.0.iter().filter(|t| t.r.contains(x, y)).collect();
        match got {
            None => assert!(inside.is_empty()),
            Some(id) => {
                hits += 1;
                let top = inside.iter().max_by_key(|t| t.layer).unwrap().layer;
                let g = inside
                    .iter()
                    .find(|t| WidgetId(t.id) == id)
                    .expect("hit must contain point");
                assert_eq!(g.layer, top, "topmost layer wins");
            }
        }
    }
    assert!(hits > 100);
    // abutting rects never both claim a shared edge
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(10.0, 0.0, 10.0, 10.0);
    assert!(a.contains(9.999, 5.0) && !a.contains(10.0, 5.0) && b.contains(10.0, 5.0));
    // disabled widgets are not hit
    l.0[0].enabled = false;
    assert_eq!(hit_test(&l, 110.0, 110.0), None);
}

#[test]
fn design_mapping_widescreen() {
    let m = DesignMapping::from_window(1920.0, 1080.0);
    let (x, y) = m.to_design(960.0, 540.0);
    assert!((x - 320.0).abs() < 1e-3 && (y - 240.0).abs() < 1e-3);
    let (x0, _) = m.to_design(0.0, 0.0);
    assert!(x0 < 0.0, "widescreen extends left of the 4:3 area");
    let m43 = DesignMapping::from_window(640.0, 480.0);
    assert_eq!(m43.to_design(100.0, 50.0), (100.0, 50.0));
}

#[test]
fn bindings_actionmap_and_recording() {
    let mut b = RecordingBindings::default();
    b.texts.insert(TextId(1), "x".into());
    b.denied.push(ActionId(7));
    let mut m = ActionMap::new();
    m.bind_activate(WidgetId(2), ActionId(5), ActionArg::Index(3));
    m.bind_activate(WidgetId(3), ActionId(7), ActionArg::None);
    m.bind_back(ActionId(6));
    // T2447: activation is rechecked against the typed page and focus (`dispatch_activation`);
    // `apply` no longer handles `Activate`.
    use tsfp_ui_model::{MenuFocus, Page, PageId, Widget, WidgetId as MW, WidgetKind};
    let mut page = Page::new(PageId(1), 1);
    for id in 2..=3 {
        page.widgets.push(Widget::new(MW(id), WidgetKind::Button));
    }
    let mut focus = MenuFocus::new(&page);
    assert_eq!(m.apply(&UiEvent::Activate(WidgetId(2)), &mut b), None);
    focus.selected = Some(MW(2));
    assert_eq!(
        m.dispatch_activation(&page, &focus, &UiEvent::Activate(WidgetId(2)), &mut b)
            .binding_result(),
        Some(BindResult::Handled)
    );
    focus.selected = Some(MW(3));
    assert_eq!(
        m.dispatch_activation(&page, &focus, &UiEvent::Activate(WidgetId(3)), &mut b)
            .binding_result(),
        Some(BindResult::Denied)
    );
    assert_eq!(m.apply(&UiEvent::Back, &mut b), Some(BindResult::Handled));
    assert_eq!(m.apply(&UiEvent::Start, &mut b), None);
    assert_eq!(m.apply(&UiEvent::Hover(WidgetId(2)), &mut b), None);
    b.set_value(ValueId(1), UiValue::Int(4));
    assert_eq!(b.value(ValueId(1)), UiValue::Int(4));
    assert_eq!(b.value(ValueId(2)), UiValue::None);
    assert_eq!(b.text(TextId(1)), "x");
    assert_eq!(b.list(ListId(1)), ListView::default());
    assert_eq!(
        b.calls[0],
        RecordedCall::Action(ActionId(5), ActionArg::Index(3))
    );
}

#[test]
fn input_intent_adapter() {
    use tsfp_input::actions::Intent;
    assert_eq!(
        MenuIntent::from_input(Intent::MenuAccept),
        Some(MenuIntent::Accept)
    );
    assert_eq!(MenuIntent::from_input(Intent::Fire), None);
}

#[test]
fn name_entry_page_routes_commit_and_cancel_to_builder_handlers() {
    use tsfp_text_field::field_spec::FieldSpec;
    use tsfp_text_field::keyboard::{FieldFlags, all_tables};
    use tsfp_text_field::name_entry::NameEntry;
    use tsfp_ui_input::{TextAction, begin_name_entry, dispatch_text_event};
    use tsfp_ui_model::HandlerRef;
    let spec = FieldSpec {
        call_site: 0,
        source: None,
        limit: 4,
        width_scale: 0.0,
        select_all: false,
        flags: FieldFlags(0),
        done: 0xd0,
        cancel: 0xc0,
    };
    // Synthetic grids: every key is 'A'..'C'.
    let read = |a: u32| {
        all_tables()
            .into_iter()
            .any(|t| a >= t.bytes && a < t.bytes + t.key_count() as u32)
            .then(|| b'A' + (a % 3) as u8)
    };
    let entry = NameEntry::new(spec, b"AB", false, read).unwrap();
    let (layout, mut input) = (column(), ui());
    let page = begin_name_entry(&mut input, C, WidgetId(1), &entry).unwrap();
    run(&mut input, &layout, 0, &none(), None);
    let ev = run(&mut input, &layout, 1, &held(MenuIntent::Accept), None);
    assert_eq!(
        dispatch_text_event(&page, &ev[0]),
        Some(TextAction::Commit {
            text: b"AB".to_vec(),
            handler: HandlerRef::Legacy(0xd0)
        })
    );
    let page = begin_name_entry(&mut input, C, WidgetId(1), &entry).unwrap();
    run(&mut input, &layout, 2, &none(), None);
    let ev = run(&mut input, &layout, 3, &held(MenuIntent::Back), None);
    assert_eq!(
        dispatch_text_event(&page, &ev[0]),
        Some(TextAction::Cancel {
            handler: HandlerRef::Legacy(0xc0)
        })
    );
    assert_eq!(dispatch_text_event(&page, &UiEvent::Back), None);
}
