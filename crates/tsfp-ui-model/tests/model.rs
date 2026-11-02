//! Synthetic fixtures only: ids, text ids and callback tokens below are invented.

use tsfp_ui_model::*;
use tsfp_ui_renderer::{AnchorSpace, Rect, Vec2, Viewport};

/// Small deterministic generator (xorshift64*), no external crates.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const TREE: &str = r#"{
 "schema": 1, "samples": 3, "runs": ["r"], "language_index_values": [0],
 "pages": {
  "0x100": {"page_id": 256, "builder": 1, "gate": 2, "draw_callback": 3, "widget_types": [1],
    "states": [3], "first_poll": 0, "samples": 1, "select_handler": 900, "back_handler": 901,
    "rows": [
      {"row_id": 1, "kind": 0, "enabled": true, "navigable": true, "text_id": 10, "text_dynamic": false,
       "font_index": 2, "value_font_index": 0, "align_mode": 1, "option_count": 0, "selected_index": 0,
       "selected_option_id": 0, "activate_callback": 700, "draw_callback": 0, "layout": {"0x5c": 10.5, "0x60": 20.0, "0x64": 200.0, "0x68": 30.0}, "options": []},
      {"row_id": 2, "kind": 0, "enabled": true, "navigable": true, "text_id": 11, "text_dynamic": false,
       "option_count": 2, "selected_index": 1, "activate_callback": 0,
       "options": [{"option_id": 5, "text_id": 20, "flags": 0, "enabled": true}, {"option_id": 6, "text_id": null, "flags": 8, "enabled": false}]},
      {"row_id": 3, "kind": 1, "enabled": false, "navigable": true, "text_id": null, "text_dynamic": true, "activate_callback": 701, "options": []},
      {"row_id": 4, "kind": 1, "enabled": true, "navigable": false, "text_id": 12, "options": []}
    ]},
  "0x200": {"page_id": 512, "builder": 1, "gate": 0, "draw_callback": 0, "widget_types": [2],
    "states": [], "first_poll": 0, "samples": 1, "back_handler": 0, "rows": []}
 },
 "edges": [
  {"from": "root", "to": "0x100", "kind": "open", "input": [], "runs": [], "polls": []},
  {"from": "0x100", "to": "0x200", "kind": "open", "input": ["A", "DOWN"], "runs": [], "polls": []},
  {"from": "0x200", "to": "0x100", "kind": "close", "input": ["B"], "runs": [], "polls": []}
 ]}"#;

fn viewport() -> Viewport {
    Viewport::new(
        Vec2::new(1920.0, 1080.0),
        2.25,
        Rect::new(Vec2::new(240.0, 0.0), Vec2::new(1440.0, 1080.0)).unwrap(),
    )
    .unwrap()
}

fn button(id: u64, text: u32) -> Widget {
    let mut w = Widget::new(WidgetId(id), WidgetKind::Button);
    w.text = TextRef::Id(TextId(text));
    w.activate = Some(HandlerRef::Legacy(id as u32));
    w
}

fn list_page(id: u64, count: u64, wrap: Wrap, columns: u16) -> Page {
    let mut page = Page::new(PageId(id), 1);
    page.wrap = wrap;
    page.nav = NavLayout::Grid(columns);
    for n in 0..count {
        page.widgets.push(button(n + 1, n as u32));
    }
    page
}

#[test]
fn loader_maps_rows_options_edges_and_handlers() {
    let model = UiModel::from_tree_json(TREE, &LoadOptions::default()).unwrap();
    assert_eq!(model.pages.len(), 2);
    assert_eq!(model.root, Some(PageId(0x100)));
    assert_eq!(model.edges.len(), 3);
    assert_eq!(
        model.edges[1].input,
        vec!["A".to_string(), "DOWN".to_string()]
    );
    let page = model.page(PageId(0x100)).unwrap();
    assert_eq!(page.handlers.back, Some(HandlerRef::Legacy(901)));
    assert_eq!(model.page(PageId(0x200)).unwrap().handlers.back, None);
    let w = &page.widgets;
    assert_eq!(w[0].kind, WidgetKind::Button);
    assert_eq!(w[0].text, TextRef::Id(TextId(10)));
    assert_eq!(w[0].align, Align::Center);
    assert_eq!(w[1].kind, WidgetKind::Cycler);
    assert_eq!(w[1].selected, 1);
    assert_eq!(w[1].options[1].text, TextRef::None);
    assert!(!w[1].options[1].enabled);
    assert_eq!(w[2].text, TextRef::Dynamic);
    assert!(!w[2].enabled);
    assert!(!w[3].navigable);
    // Only rows 1 and 2 are focusable (3 disabled, 4 not navigable).
    assert_eq!(page.focus_graph().focusable(), &[WidgetId(1), WidgetId(2)]);
}

#[test]
fn rect_cells_are_opt_in() {
    let cells = RectCells {
        x: 0x5c,
        y: 0x60,
        width: 0x64,
        height: 0x68,
    };
    let with = UiModel::from_tree_json(
        TREE,
        &LoadOptions {
            rect_cells: Some(cells),
            ..Default::default()
        },
    )
    .unwrap();
    let r = with.page(PageId(0x100)).unwrap().widgets[0].rect;
    assert_eq!(
        (r.offset, r.size),
        (Vec2::new(10.5, 20.0), Vec2::new(200.0, 30.0))
    );
    let without = UiModel::from_tree_json(TREE, &LoadOptions::default()).unwrap();
    let r = without.page(PageId(0x100)).unwrap().widgets[0].rect;
    assert_eq!(r.space, AnchorSpace::SafeArea4x3);
    assert_eq!(r.size.x, without.theme.column_width);
}

#[test]
fn tree_round_trip_is_stable() {
    for opts in [
        LoadOptions::default(),
        LoadOptions {
            rect_cells: Some(RectCells {
                x: 0x5c,
                y: 0x60,
                width: 0x64,
                height: 0x68,
            }),
            ..Default::default()
        },
    ] {
        let first = UiModel::from_tree_json(TREE, &opts).unwrap();
        let text = first.to_tree_json(&opts);
        let second = UiModel::from_tree_json(&text, &opts).unwrap();
        assert_eq!(first, second);
        assert_eq!(second.to_tree_json(&opts), text);
    }
}

#[test]
fn loader_rejects_bad_input() {
    let opts = LoadOptions::default();
    assert!(matches!(
        UiModel::from_tree_json("{", &opts),
        Err(LoadError::Json(_))
    ));
    assert!(matches!(
        UiModel::from_tree_json(r#"{"schema": 2, "pages": {}}"#, &opts),
        Err(LoadError::Schema(_))
    ));
    let dup = r#"{"schema":1,"pages":{"0x1":{"page_id":1,"rows":[{"row_id":1},{"row_id":1}]}},"edges":[]}"#;
    // Real dumps repeat row ids (0, -1 spacers): kept, the repeat gets an anonymous id.
    let m = UiModel::from_tree_json(dup, &opts).unwrap();
    let ids: Vec<WidgetId> = m
        .page(PageId(1))
        .unwrap()
        .widgets
        .iter()
        .map(|w| w.id)
        .collect();
    assert_eq!(ids, vec![WidgetId(1), WidgetId::anon(1)]);
    let neg = r#"{"schema":1,"pages":{"0x1":{"page_id":1,"rows":[{"row_id":-1},{"row_id":-1}]}},"edges":[]}"#;
    assert_eq!(
        UiModel::from_tree_json(neg, &opts)
            .unwrap()
            .page(PageId(1))
            .unwrap()
            .widgets
            .len(),
        2
    );
    let bad_edge = r#"{"schema":1,"pages":{},"edges":[{"from":"zz","to":"root","kind":"open"}]}"#;
    assert!(matches!(
        UiModel::from_tree_json(bad_edge, &opts),
        Err(LoadError::Schema(_))
    ));
}

#[test]
fn json_parser_basics() {
    let v = json::Json::parse(r#"{"a":[1,2.5,-3e1,true,null,"x\nA"],"b":{}}"#).unwrap();
    let a = v.get("a").unwrap().as_array().unwrap();
    assert_eq!(a[2].as_f64(), Some(-30.0));
    assert_eq!(a[5].as_str(), Some("x\nA"));
    assert!(json::Json::parse("[1,]").is_err());
    assert!(json::Json::parse("1 2").is_err());
    assert!(json::Json::parse(&"[".repeat(200)).is_err());
}

#[test]
fn focus_wrap_and_clamp_vertical() {
    let wrap = list_page(1, 3, Wrap::Wrap, 1).focus_graph();
    assert_eq!(wrap.step(WidgetId(3), Dir::Down), Some(WidgetId(1)));
    assert_eq!(wrap.step(WidgetId(1), Dir::Up), Some(WidgetId(3)));
    assert_eq!(wrap.step(WidgetId(1), Dir::Left), None);
    let clamp = list_page(1, 3, Wrap::Clamp, 1).focus_graph();
    assert_eq!(clamp.step(WidgetId(3), Dir::Down), None);
    assert_eq!(clamp.step(WidgetId(2), Dir::Down), Some(WidgetId(3)));
}

#[test]
fn focus_skips_disabled_and_grid_ragged_last_row() {
    let mut page = list_page(1, 5, Wrap::Wrap, 2);
    page.widget_mut(WidgetId(2)).unwrap().enabled = false;
    // Focusable: 1,3,4,5 laid out as rows [1,3] [4,5].
    let g = page.focus_graph();
    assert_eq!(
        g.focusable(),
        &[WidgetId(1), WidgetId(3), WidgetId(4), WidgetId(5)]
    );
    assert_eq!(g.step(WidgetId(1), Dir::Right), Some(WidgetId(3)));
    assert_eq!(g.step(WidgetId(1), Dir::Down), Some(WidgetId(4)));
    assert_eq!(g.step(WidgetId(3), Dir::Down), Some(WidgetId(5)));
    assert!(g.validate(page.start_focus()).is_empty());
}

#[test]
fn link_override_and_validation_issues() {
    let mut page = list_page(1, 3, Wrap::Clamp, 1);
    page.links.push(FocusLink {
        from: WidgetId(1),
        dir: Dir::Right,
        to: WidgetId(3),
    });
    let g = page.focus_graph();
    assert_eq!(g.step(WidgetId(1), Dir::Right), Some(WidgetId(3)));
    assert_eq!(
        g.asymmetries(),
        vec![(WidgetId(1), Dir::Right, WidgetId(3))]
    );
    // A link to a disabled widget is reported.
    page.widget_mut(WidgetId(3)).unwrap().enabled = false;
    let issues = page.focus_graph().validate(page.start_focus());
    assert!(issues.contains(&FocusIssue::TargetNotFocusable {
        from: WidgetId(1),
        dir: Dir::Right,
        to: WidgetId(3)
    }));
}

#[test]
fn generated_focus_graphs_satisfy_invariants() {
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    for case in 0..2000 {
        let count = 1 + rng.below(14);
        let columns = rng.below(5) as u16; // 0 = one row
        let wrap = if rng.below(2) == 0 {
            Wrap::Wrap
        } else {
            Wrap::Clamp
        };
        let mut page = list_page(case, count, wrap, columns);
        for w in &mut page.widgets {
            w.enabled = rng.below(4) != 0;
            w.navigable = rng.below(8) != 0;
        }
        let graph = page.focus_graph();
        let focusable: Vec<WidgetId> = page
            .widgets
            .iter()
            .filter(|w| w.enabled && w.navigable)
            .map(|w| w.id)
            .collect();
        assert_eq!(graph.focusable(), focusable.as_slice(), "case {case}");
        // Disabled widgets never take part.
        for w in &page.widgets {
            if !focusable.contains(&w.id) {
                assert!(graph.neighbours(w.id).is_none());
            }
        }
        // Symmetric neighbours where defined.
        assert!(
            graph.asymmetries().is_empty(),
            "case {case} cols {columns} {wrap:?}"
        );
        // No dead ends, and every enabled widget reachable, when focus can move along the page.
        // A ragged clamped grid can leave a widget with no vertical move, but the row connects it;
        // under Grid(c>1) with Clamp a single-column leftover is still linked by its row.
        let issues = graph.validate(page.start_focus());
        let dead_or_lost: Vec<_> = issues
            .iter()
            .filter(|i| !matches!(i, FocusIssue::Unreachable(_)))
            .collect();
        assert!(dead_or_lost.is_empty(), "case {case}: {issues:?}");
        if columns == 1 || columns == 0 || wrap == Wrap::Wrap || focusable.len() <= columns as usize
        {
            assert!(issues.is_empty(), "case {case}: {issues:?}");
        }
    }
}

#[test]
fn recorder_builds_nested_pages() {
    let mut r = ModelRecorder::new();
    r.push_page(PageId(1), 1).unwrap();
    r.record_row(button(1, 5)).unwrap();
    r.record_option(UiOption {
        id: 1,
        text: TextRef::Id(TextId(6)),
        enabled: true,
    })
    .unwrap();
    r.set_handler(HandlerSlot::Back, HandlerRef::Named("close".into()))
        .unwrap();
    r.push_page(PageId(2), 3).unwrap();
    assert_eq!(
        r.record_option(UiOption {
            id: 1,
            text: TextRef::None,
            enabled: true
        }),
        Err(BuilderError::NoRow)
    );
    r.record_row(button(1, 7)).unwrap();
    assert_eq!(
        r.record_row(button(1, 7)),
        Err(BuilderError::DuplicateWidget(WidgetId(1)))
    );
    r.pop_page().unwrap();
    r.pop_page().unwrap();
    assert_eq!(r.pop_page(), Err(BuilderError::NoPage));
    let model = r.finish().unwrap();
    let p1 = model.page(PageId(1)).unwrap();
    assert_eq!(p1.widgets[0].kind, WidgetKind::Cycler);
    assert_eq!(p1.handlers.back, Some(HandlerRef::Named("close".into())));
    assert_eq!(model.page(PageId(2)).unwrap().widget_type, 3);

    let mut open = ModelRecorder::new();
    open.push_page(PageId(1), 1).unwrap();
    assert_eq!(open.finish().unwrap_err(), BuilderError::UnbalancedPages(1));
    let mut deep = ModelRecorder::new();
    for n in 0..builder::MAX_BUILD_DEPTH {
        deep.push_page(PageId(n as u64), 1).unwrap();
    }
    assert_eq!(deep.push_page(PageId(99), 1), Err(BuilderError::TooDeep));
}

#[test]
fn view_navigates_and_remembers_focus() {
    let model = UiModel::from_tree_json(TREE, &LoadOptions::default()).unwrap();
    let mut view = UiView::new();
    assert!(!view.open(&model, PageId(0x999)));
    assert!(view.open(&model, PageId(0x100)));
    assert_eq!(view.focused(), Some(WidgetId(1)));
    assert_eq!(view.navigate(&model, Dir::Down), Some(WidgetId(2)));
    assert!(view.open(&model, PageId(0x200)));
    assert_eq!(view.focused(), None);
    assert_eq!(view.pop(), Some(PageId(0x200)));
    assert_eq!(view.focused(), Some(WidgetId(2)));
}

#[test]
fn animation_is_deterministic() {
    let t = Transition {
        kind: TransitionKind::Fade,
        duration_ms: 200,
        ease: Ease::InOut,
    };
    let mut a = AnimState::start(t);
    assert_eq!(a.progress(), 0.0);
    a.advance(100);
    assert!((a.progress() - 0.5).abs() < 1e-6);
    a.advance(u32::MAX);
    assert!(a.finished());
    assert_eq!(a.progress(), 1.0);
    assert_eq!(AnimState::start(Transition::NONE).progress(), 1.0);
    for ease in [Ease::Linear, Ease::In, Ease::Out, Ease::InOut] {
        assert_eq!((ease.apply(0.0), ease.apply(1.0)), (0.0, 1.0));
    }
}

// ---- patches ----

fn base_model() -> UiModel {
    let mut model = UiModel::from_tree_json(TREE, &LoadOptions::default()).unwrap();
    let mut third = list_page(0x300, 2, Wrap::Wrap, 1);
    third.handlers.back = Some(HandlerRef::Legacy(5));
    model.pages.insert(third.id, third);
    model
}

/// T2181-style: an overlay Back button on every page with a back handler and no Back widget.
fn back_button_patch() -> UiPatch {
    let mut back = Widget::new(WidgetId::patch(1), WidgetKind::Button);
    back.role = WidgetRole::Back;
    back.text = TextRef::Id(TextId(900));
    back.activate = Some(HandlerRef::Named("ui.back".into()));
    back.rect = tsfp_ui_renderer::AnchoredRect {
        space: AnchorSpace::SafeArea4x3,
        horizontal: tsfp_ui_renderer::Edge::Start,
        vertical: tsfp_ui_renderer::Edge::End,
        offset: Vec2::new(16.0, -16.0),
        size: Vec2::new(96.0, 24.0),
    };
    UiPatch {
        name: "back-button".into(),
        ops: vec![PatchOp::AddWidget {
            page: PageSelector::WithHandler(HandlerSlot::Back),
            at: Insert::End,
            widget: back,
            unless_role: Some(WidgetRole::Back),
        }],
    }
}

/// T2163-style: an extra settings row bound to a launcher value.
fn settings_row_patch() -> UiPatch {
    let mut row = Widget::new(WidgetId::patch(2), WidgetKind::Cycler);
    row.text = TextRef::Id(TextId(901));
    row.bound = Some(BindingId("launcher.invert_y".into()));
    row.options = vec![
        UiOption {
            id: 0,
            text: TextRef::Id(TextId(902)),
            enabled: true,
        },
        UiOption {
            id: 1,
            text: TextRef::Id(TextId(903)),
            enabled: true,
        },
    ];
    UiPatch {
        name: "settings-row".into(),
        ops: vec![PatchOp::AddWidget {
            page: PageSelector::Id(PageId(0x100)),
            at: Insert::After(WidgetId(2)),
            widget: row,
            unless_role: None,
        }],
    }
}

fn preset(patches: Vec<UiPatch>) -> Preset {
    Preset {
        name: "extras".into(),
        patches,
    }
}

#[test]
fn original_preset_is_identity() {
    let base = base_model();
    assert_eq!(Preset::original().apply(&base).unwrap(), base);
}

#[test]
fn back_button_and_settings_row_apply() {
    let base = base_model();
    let out = preset(vec![back_button_patch(), settings_row_patch()])
        .apply(&base)
        .unwrap();
    // 0x100 and 0x300 have back handlers, 0x200 does not.
    assert_eq!(
        out.page(PageId(0x100))
            .unwrap()
            .widgets
            .last()
            .unwrap()
            .role,
        WidgetRole::Back
    );
    assert_eq!(out.page(PageId(0x300)).unwrap().widgets.len(), 3);
    assert_eq!(out.page(PageId(0x200)).unwrap().widgets.len(), 0);
    let order: Vec<u64> = out
        .page(PageId(0x100))
        .unwrap()
        .widgets
        .iter()
        .map(|w| w.id.0)
        .collect();
    assert_eq!(
        order,
        vec![1, 2, WidgetId::patch(2).0, 3, 4, WidgetId::patch(1).0]
    );
    assert_eq!(out.preset, "extras");
    // The base is untouched.
    assert_eq!(base.page(PageId(0x100)).unwrap().widgets.len(), 4);
    // The new Back button is focusable and keeps the graph valid.
    let page = out.page(PageId(0x300)).unwrap();
    assert!(page.focus_graph().validate(page.start_focus()).is_empty());
}

#[test]
fn patch_application_is_idempotent_and_deterministic() {
    let base = base_model();
    let p = preset(vec![back_button_patch(), settings_row_patch()]);
    let once = p.apply(&base).unwrap();
    assert_eq!(p.apply(&once).unwrap(), once);
    assert_eq!(p.apply(&base).unwrap(), once);
}

#[test]
fn random_patch_sets_are_idempotent() {
    let mut rng = Rng(77);
    for case in 0..300 {
        let base = base_model();
        let mut ops = Vec::new();
        for n in 0..(1 + rng.below(6)) {
            let page = PageSelector::Id(PageId([0x100, 0x300][rng.below(2) as usize]));
            let mut w = button(WidgetId::PATCH_BASE + 10 + n, 5);
            w.id = WidgetId::patch(10 + n as u32);
            ops.push(match rng.below(4) {
                0 => PatchOp::AddWidget {
                    page,
                    at: Insert::End,
                    widget: w,
                    unless_role: None,
                },
                1 => PatchOp::ModifyWidget {
                    page,
                    widget: WidgetId(1),
                    change: Change::Enabled(rng.below(2) == 0),
                },
                2 => PatchOp::ReplaceHandler {
                    page,
                    slot: HandlerSlot::Select,
                    handler: Some(HandlerRef::Legacy(rng.below(9) as u32)),
                },
                _ => PatchOp::RemoveWidget {
                    page,
                    widget: WidgetId(2),
                },
            });
        }
        let p = preset(vec![UiPatch {
            name: "rand".into(),
            ops,
        }]);
        match p.apply(&base) {
            Ok(once) => assert_eq!(p.apply(&once).unwrap(), once, "case {case}"),
            // Random sets may conflict; the outcome must still be deterministic.
            Err(e) => assert_eq!(p.apply(&base).unwrap_err(), e),
        }
    }
}

#[test]
fn conflicts_are_detected_without_partial_application() {
    let base = base_model();
    let set = |name: &str, change: Change| UiPatch {
        name: name.into(),
        ops: vec![PatchOp::ModifyWidget {
            page: PageSelector::Id(PageId(0x100)),
            widget: WidgetId(1),
            change,
        }],
    };
    // Same field, different values.
    let err = preset(vec![
        set("a", Change::Enabled(false)),
        set("b", Change::Enabled(true)),
    ])
    .apply(&base)
    .unwrap_err();
    match err {
        PatchError::Conflicts(c) => {
            assert_eq!(c.len(), 1);
            assert_eq!(
                (c[0].first_patch.as_str(), c[0].second_patch.as_str()),
                ("a", "b")
            );
        }
        other => panic!("{other:?}"),
    }
    // Different fields and equal duplicates are fine.
    assert!(
        preset(vec![
            set("a", Change::Enabled(false)),
            set("b", Change::Navigable(false)),
            set("c", Change::Enabled(false))
        ])
        .apply(&base)
        .is_ok()
    );
    // Remove versus modify.
    let remove = UiPatch {
        name: "rm".into(),
        ops: vec![PatchOp::RemoveWidget {
            page: PageSelector::Id(PageId(0x100)),
            widget: WidgetId(1),
        }],
    };
    assert!(matches!(
        preset(vec![set("a", Change::Enabled(false)), remove]).apply(&base),
        Err(PatchError::Conflicts(_))
    ));
    // Two handler replacements with different handlers.
    let handler = |name: &str, v: u32| UiPatch {
        name: name.into(),
        ops: vec![PatchOp::ReplaceHandler {
            page: PageSelector::Id(PageId(0x100)),
            slot: HandlerSlot::Back,
            handler: Some(HandlerRef::Legacy(v)),
        }],
    };
    assert!(matches!(
        preset(vec![handler("a", 1), handler("b", 2)]).apply(&base),
        Err(PatchError::Conflicts(_))
    ));
    let ok = preset(vec![handler("a", 1), handler("b", 1)])
        .apply(&base)
        .unwrap();
    assert_eq!(
        ok.page(PageId(0x100)).unwrap().handlers.back,
        Some(HandlerRef::Legacy(1))
    );
    // Same new id with different content on one page.
    let mut w1 = button(0, 1);
    w1.id = WidgetId::patch(7);
    let mut w2 = w1.clone();
    w2.text = TextRef::Id(TextId(2));
    let add = |name: &str, w: Widget| UiPatch {
        name: name.into(),
        ops: vec![PatchOp::AddWidget {
            page: PageSelector::Id(PageId(0x100)),
            at: Insert::End,
            widget: w,
            unless_role: None,
        }],
    };
    assert!(matches!(
        preset(vec![add("a", w1), add("b", w2)]).apply(&base),
        Err(PatchError::Conflicts(_))
    ));
}

#[test]
fn patch_errors_for_missing_targets() {
    let base = base_model();
    let p = |op| {
        preset(vec![UiPatch {
            name: "x".into(),
            ops: vec![op],
        }])
        .apply(&base)
    };
    assert!(matches!(
        p(PatchOp::ModifyWidget {
            page: PageSelector::Id(PageId(0x100)),
            widget: WidgetId(99),
            change: Change::Enabled(true)
        }),
        Err(PatchError::MissingWidget { .. })
    ));
    assert!(matches!(
        p(PatchOp::RemoveWidget {
            page: PageSelector::Id(PageId(0x777)),
            widget: WidgetId(1)
        }),
        Err(PatchError::MissingPage { .. })
    ));
    assert!(matches!(
        p(PatchOp::AddWidget {
            page: PageSelector::Id(PageId(0x100)),
            at: Insert::Before(WidgetId(99)),
            widget: Widget::new(WidgetId::patch(1), WidgetKind::Label),
            unless_role: None
        }),
        Err(PatchError::MissingWidget { .. })
    ));
    // Clashing id with different content.
    assert!(matches!(
        p(PatchOp::AddWidget {
            page: PageSelector::Id(PageId(0x100)),
            at: Insert::End,
            widget: Widget::new(WidgetId(1), WidgetKind::Label),
            unless_role: None
        }),
        Err(PatchError::WidgetExists { .. })
    ));
    // Removing a missing widget is a no-op; the widget disappears from links too.
    assert_eq!(
        p(PatchOp::RemoveWidget {
            page: PageSelector::Id(PageId(0x100)),
            widget: WidgetId(99)
        })
        .unwrap()
        .pages,
        base.pages
    );
}

#[test]
fn insert_page_and_theme() {
    let base = base_model();
    let page = list_page(0x400, 1, Wrap::Wrap, 1);
    let edge = PageEdge {
        from: Some(PageId(0x100)),
        to: Some(PageId(0x400)),
        kind: EdgeKind::Open,
        input: vec!["A".into()],
    };
    let mut theme = Theme::original();
    theme.name = "bigger".into();
    theme.row_height = 32.0;
    let p = preset(vec![UiPatch {
        name: "ins".into(),
        ops: vec![
            PatchOp::InsertPage {
                page: page.clone(),
                edge: Some(edge),
            },
            PatchOp::SetTheme(theme),
        ],
    }]);
    let out = p.apply(&base).unwrap();
    assert!(out.page(PageId(0x400)).is_some());
    assert_eq!(out.edges.len(), base.edges.len() + 1);
    assert_eq!(out.theme.row_height, 32.0);
    assert_eq!(p.apply(&out).unwrap(), out);
    // Existing page with different content is an error.
    let mut other = page;
    other.widget_type = 9;
    let clash = preset(vec![UiPatch {
        name: "c".into(),
        ops: vec![PatchOp::InsertPage {
            page: other,
            edge: None,
        }],
    }]);
    assert!(matches!(
        clash.apply(&out),
        Err(PatchError::PageExists { .. })
    ));
}

#[test]
fn render_bridge_resolves_pixels_and_highlights_focus() {
    let model = base_model();
    let layout = model
        .to_render_batches(PageId(0x300), viewport(), Some(WidgetId(1)))
        .unwrap()
        .unwrap();
    let tree_layout = model
        .to_render_batches(PageId(0x100), viewport(), None)
        .unwrap()
        .unwrap();
    // Two focusable widgets: two panel quads plus one focus quad right after the first.
    assert_eq!(layout.quads.len(), 3);
    assert_eq!(layout.quads[1].color, model.theme.focus);
    assert_eq!(layout.texts.len(), 2);
    assert!(matches!(layout.batches()[0], tsfp_ui_renderer::Batch::Quads(q) if q.len() == 3));
    // Centre anchored column in the safe area: x = 240 + (1440 - 320*2.25)/2.
    let expected_x = 240.0 + (1440.0 - 320.0 * 2.25) / 2.0;
    let r = tree_layout.quads[1].rect_px;
    assert!((r.origin.x - expected_x).abs() < 1e-3, "{}", r.origin.x);
    assert!((r.size.x - 720.0).abs() < 1e-3);
    // Disabled text uses the disabled colour; unknown page is None.
    assert!(
        model
            .to_render_batches(PageId(1), viewport(), None)
            .is_none()
    );
    let disabled = model.page(PageId(0x100)).unwrap().widgets[2].id;
    let l = model
        .to_render_batches(PageId(0x100), viewport(), None)
        .unwrap()
        .unwrap();
    let t = l.texts.iter().find(|t| t.widget == disabled).unwrap();
    assert_eq!(t.color, model.theme.text_disabled);
    assert_eq!(t.text, TextRef::Dynamic);
}
