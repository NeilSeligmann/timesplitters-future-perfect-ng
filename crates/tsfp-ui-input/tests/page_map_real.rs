// SPDX-License-Identifier: GPL-3.0-or-later
//! T2168: the tree-generated cursor maps against the MEASURED tree edges. Every captured edge
//! records the live cursor index and the row id under it when the page changed; on every page
//! whose map is trusted for the pointer, row `from_cursor_index` of the map must be that row.
//! Reads the private menu tree (`TSFP_UI_TREE`, default tmp/ui-tree/tree.json); skips without it.
//! Prints counts only, no game data.

use tsfp_ui_input::FrontendPages;
use tsfp_ui_model::json::Json;
use tsfp_ui_model::{UiModel, WidgetId, frontend};

#[test]
fn generated_maps_agree_with_the_captured_cursor_edges() {
    let path = std::env::var("TSFP_UI_TREE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../tmp/ui-tree/tree.json").into()
    });
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skip: no menu tree at {path}");
        return;
    };
    let model = UiModel::from_tree_json(&text, &frontend::dump_load_options()).unwrap();
    let pages = FrontendPages::from_tree(&text, &model);
    let tree = Json::parse(&text).unwrap();
    let page_builder = |key: &str| -> Option<u32> {
        tree.get("pages")?
            .get(key)?
            .get("builder")?
            .as_u64()
            .map(|b| b as u32)
    };
    let trusted = pages.maps.iter().filter(|m| m.pointer_rows_valid()).count();
    let (mut checked, mut agree) = (0, 0);
    for edge in tree.get("edges").and_then(Json::as_array).unwrap_or(&[]) {
        let Some(index) = edge.get("from_cursor_index").and_then(Json::as_f64) else {
            continue;
        };
        let Some(row_id) = edge.get("from_cursor_row_id").and_then(Json::as_f64) else {
            continue;
        };
        if index < 0.0 || row_id < 0.0 {
            continue;
        }
        let Some(from) = edge.get("from").and_then(Json::as_str) else {
            continue;
        };
        let Some(map) = page_builder(from).and_then(|b| pages.get(b)) else {
            continue;
        };
        if !map.pointer_rows_valid() {
            continue;
        }
        checked += 1;
        let at = map.rows.get(index as usize).map(|r| r.widget);
        if at == Some(WidgetId(row_id as u64)) {
            agree += 1;
        } else {
            eprintln!(
                "page {from} builder {:#x}: cursor {index} maps to {at:?}, edge says row {row_id}",
                map.builder
            );
        }
    }
    eprintln!(
        "{} pages, {trusted} with trusted pointer rows; {agree}/{checked} cursor edges agree",
        pages.maps.len()
    );
    assert_eq!(agree, checked);
}
