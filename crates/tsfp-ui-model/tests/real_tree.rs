//! Loader checks against a real capture (ignored `tmp/ui-tree/tree.json`, or `TSFP_UI_TREE`).
//! Skips (passes) when no capture exists. Prints statistics, asserts invariants only.

use tsfp_ui_model::*;

fn tree_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("TSFP_UI_TREE") {
        return p.into();
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/ui-tree/tree.json")
}

#[test]
fn real_tree_loads_and_focus_graphs_validate() {
    let Ok(text) = std::fs::read_to_string(tree_path()) else {
        eprintln!("no capture, skipped");
        return;
    };
    let opts = LoadOptions {
        rect_edges: Some(RectEdges::DUMP),
        ignore_row_enabled_flag: true,
        ..Default::default()
    };
    let model = UiModel::from_tree_json(&text, &opts).expect("real tree loads");
    assert!(!model.pages.is_empty());
    let (mut rows, mut bad_rect, mut issues, mut empty) = (0, 0, 0, 0);
    for page in model.pages.values() {
        let issue_list = page.focus_graph().validate(page.start_focus());
        if !issue_list.is_empty() {
            eprintln!("page {:#x}: {:?}", page.id.0, issue_list);
        }
        issues += issue_list.len();
        if page.widgets.is_empty() {
            empty += 1;
        }
        for w in &page.widgets {
            rows += 1;
            if w.rect.size.x <= 0.0 || w.rect.size.y <= 0.0 {
                bad_rect += 1;
            }
        }
    }
    eprintln!(
        "pages {} (empty {empty}) rows {rows} zero-size rects {bad_rect} focus issues {issues} edges {}",
        model.pages.len(),
        model.edges.len()
    );
    // Rects must come from the measured edge cells, not the theme stack.
    let theme_w = model.theme.column_width;
    assert!(
        model
            .pages
            .values()
            .flat_map(|p| p.widgets.iter())
            .any(|w| w.rect.size.x != theme_w)
    );
}
