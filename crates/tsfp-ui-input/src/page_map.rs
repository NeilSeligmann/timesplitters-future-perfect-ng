//! T2168: per-page cursor-row maps generated from the captured menu tree (no hand-written page
//! tables). The original page slot counts its cursor over the item rows of the page in build
//! order: node types 7 (text item) and 8 (value item) take a cursor position, 9 (divider), 5 and
//! 10 (panel/list decorations) do not. MEASURED on the 47-page merged tree: for every page whose
//! item count is not a scrolling window the item count equals the captured `cursor_row_count`
//! (the live slot's row field, +0x180), and every tree edge's `from_cursor_index` names the
//! `from_cursor_row_id` row at that item position (docs/t-ui-input.md, T2168 section).
//!
//! A page whose item count differs from `cursor_row_count` is a scrolling list (save slots,
//! level lists with more items than the window): the cursor is window-relative there and the row
//! map cannot be trusted, so the pointer only scrolls (wheel) and backs (right click) on it.

use tsfp_ui_model::json::Json;
use tsfp_ui_model::{Page, PageId, UiModel, WidgetId, WidgetKind};

/// One cursor position of an original page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorRow {
    pub widget: WidgetId,
    /// The original cursor can rest here (enabled and navigable in the tree).
    pub focusable: bool,
    /// The row has option values (left/right change them).
    pub cycler: bool,
}

/// The cursor map of one page, keyed by the builder the live slot reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageMap {
    pub builder: u32,
    pub page: PageId,
    pub rows: Vec<CursorRow>,
    /// The captured row field of the live slot (`cursor_row_count`).
    pub cursor_row_count: u32,
}

/// Whether a widget of the loaded model takes an original cursor position.
pub fn is_cursor_item(kind: WidgetKind) -> bool {
    !matches!(
        kind,
        WidgetKind::Panel | WidgetKind::Image | WidgetKind::Custom(5) | WidgetKind::Custom(10)
    )
}

impl PageMap {
    pub fn from_page(page: &Page, builder: u32, cursor_row_count: u32) -> Self {
        let rows = page
            .widgets
            .iter()
            .filter(|w| is_cursor_item(w.kind))
            .map(|w| CursorRow {
                widget: w.id,
                focusable: w.is_focusable(),
                cycler: !w.options.is_empty() || w.kind == WidgetKind::Cycler,
            })
            .collect();
        Self {
            builder,
            page: page.id,
            rows,
            cursor_row_count,
        }
    }

    /// Row indices map 1:1 to the original cursor (no scroll window, unique widget ids).
    pub fn pointer_rows_valid(&self) -> bool {
        if self.rows.len() as u32 != self.cursor_row_count || self.rows.is_empty() {
            return false;
        }
        let mut ids: Vec<u64> = self.rows.iter().map(|r| r.widget.0).collect();
        ids.sort_unstable();
        ids.windows(2).all(|w| w[0] != w[1])
    }

    pub fn row_of(&self, id: WidgetId) -> Option<u32> {
        self.rows
            .iter()
            .position(|r| r.widget == id)
            .map(|r| r as u32)
    }

    pub fn widgets(&self) -> Vec<WidgetId> {
        self.rows.iter().map(|r| r.widget).collect()
    }
    pub fn navigable(&self) -> Vec<bool> {
        self.rows.iter().map(|r| r.focusable).collect()
    }
    pub fn cyclers(&self) -> Vec<bool> {
        self.rows.iter().map(|r| r.cycler).collect()
    }
}

/// Every page of a menu tree with its map, ordered by builder.
#[derive(Clone, Debug, Default)]
pub struct FrontendPages {
    pub maps: Vec<PageMap>,
}

impl FrontendPages {
    /// Build the maps for every page of `model` named in the tree text (`builder`,
    /// `cursor_row_count`, `page_id` of each page). Pages without a builder are skipped.
    pub fn from_tree(tree_json: &str, model: &UiModel) -> Self {
        let mut maps = Vec::new();
        let tree = Json::parse(tree_json).ok();
        let pages = tree
            .as_ref()
            .and_then(|t| t.get("pages"))
            .and_then(Json::as_object);
        for (key, entry) in pages.into_iter().flatten() {
            let Some(builder) = entry.get("builder").and_then(Json::as_u64) else {
                continue;
            };
            let id = entry
                .get("page_id")
                .and_then(Json::as_u64)
                .or_else(|| u64::from_str_radix(key.trim_start_matches("0x"), 16).ok());
            let Some(page) = id.and_then(|id| model.page(PageId(id))) else {
                continue;
            };
            let count = entry
                .get("cursor_row_count")
                .and_then(Json::as_u64)
                .unwrap_or(0);
            maps.push(PageMap::from_page(page, builder as u32, count as u32));
        }
        maps.sort_by_key(|m| m.builder);
        maps.dedup_by_key(|m| m.builder);
        Self { maps }
    }

    pub fn get(&self, builder: u32) -> Option<&PageMap> {
        self.maps
            .binary_search_by_key(&builder, |m| m.builder)
            .ok()
            .map(|i| &self.maps[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_model::frontend::dump_load_options as frontend_load_options;

    // FABRICATED tree shaped like the settings page: item, divider, item, value item, and a
    // scrolling list page (3 items, window of 2). No game data.
    const TREE: &str = r#"{"schema":1,"pages":{
      "0x10":{"page_id":16,"builder":4096,"cursor_row_count":3,"widget_types":[1],"rows":[
        {"row_id":-1,"node_type":7,"navigable":true},
        {"row_id":-1,"node_type":9,"navigable":false},
        {"row_id":1,"node_type":7,"navigable":true,"option_count":2,
         "options":[{"option_id":0},{"option_id":1}]},
        {"row_id":2,"node_type":8,"navigable":false}]},
      "0x20":{"page_id":32,"builder":8192,"cursor_row_count":2,"widget_types":[1],"rows":[
        {"row_id":0,"node_type":7},{"row_id":1,"node_type":7},{"row_id":2,"node_type":7}]}},
      "edges":[]}"#;

    #[test]
    fn cursor_rows_skip_dividers_and_flag_lists() {
        let model = UiModel::from_tree_json(TREE, &frontend_load_options()).unwrap();
        let pages = FrontendPages::from_tree(TREE, &model);
        assert_eq!(pages.maps.len(), 2);
        let settings = pages.get(4096).unwrap();
        assert_eq!(settings.rows.len(), 3);
        assert_eq!(settings.navigable(), [true, true, false]);
        assert_eq!(settings.cyclers(), [false, true, false]);
        assert_eq!(settings.rows[1].widget, WidgetId(1));
        assert!(settings.pointer_rows_valid());
        let list = pages.get(8192).unwrap();
        assert_eq!(list.rows.len(), 3);
        assert!(
            !list.pointer_rows_valid(),
            "scroll window: 3 items, 2 cursor rows"
        );
        assert!(pages.get(1).is_none());
    }
}
