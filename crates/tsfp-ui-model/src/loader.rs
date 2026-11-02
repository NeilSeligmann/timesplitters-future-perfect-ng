//! Loader and writer for the menu-tree JSON of `tools/ui_tree.py` (schema in docs/t-ui-tree-schema.md).
//! Captures stay in ignored `tmp/ui-tree/`; this crate only ever sees them as input text.

use crate::json::{Json, JsonError};
use crate::model::*;
use tsfp_ui_renderer::{AnchorSpace, AnchoredRect, Edge, Vec2};

#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    Json(JsonError),
    Schema(String),
}
impl From<JsonError> for LoadError {
    fn from(e: JsonError) -> Self {
        LoadError::Json(e)
    }
}
fn schema<T>(message: impl Into<String>) -> Result<T, LoadError> {
    Err(LoadError::Schema(message.into()))
}

/// Row layout cells (row byte offsets in the dump's `layout` map) holding x, y, width, height.
/// Their roles are INFERRED in the data-model docs, so the default is `None` (rows are stacked
/// by the theme) and a caller who has confirmed the cells passes them explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectCells {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Row layout cells holding the left, top, right and bottom edges of the row rectangle.
/// MEASURED on the real dump (T2189): `0x78`, `0x7C`, `0x80`, `0x84` (see `RectEdges::DUMP`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectEdges {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}
impl RectEdges {
    pub const DUMP: RectEdges = RectEdges {
        left: 0x78,
        top: 0x7C,
        right: 0x80,
        bottom: 0x84,
    };
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadOptions {
    pub rect_cells: Option<RectCells>,
    /// Takes precedence over `rect_cells` when both are set and resolve.
    pub rect_edges: Option<RectEdges>,
    /// Treat every row as enabled. MEASURED (T2189): the dump's "enabled" (row flag bit 0 clear) is
    /// not a disabled state: the player-count page sets that flag on all four rows through the
    /// toolkit call named set_enabled, and the cursor must rest on them. Default false keeps the
    /// T2186 reading for synthetic data; real dumps should pass true.
    pub ignore_row_enabled_flag: bool,
}

fn parse_id(text: &str) -> Option<u64> {
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}
fn u(value: &Json, key: &str) -> Option<u64> {
    value.get(key).and_then(Json::as_u64)
}
fn handler(value: &Json, key: &str) -> Option<HandlerRef> {
    u(value, key)
        .filter(|v| *v != 0)
        .map(|v| HandlerRef::Legacy(v as u32))
}
fn text_ref(value: &Json) -> TextRef {
    if value.get("text_dynamic").and_then(Json::as_bool) == Some(true) {
        TextRef::Dynamic
    } else {
        match u(value, "text_id") {
            Some(id) => TextRef::Id(TextId(id as u32)),
            None => TextRef::None,
        }
    }
}
fn align_from_mode(mode: u64) -> Align {
    match mode {
        1 => Align::Center,
        2 => Align::End,
        _ => Align::Start,
    }
}
/// Row flag of the original row record (+0x28) that upper-cases the label (`0x76150` -> `0x3FB90`).
pub const ROW_FLAG_UPPERCASE: u64 = 0x2000;

fn align_mode(align: Align) -> u32 {
    match align {
        Align::Start => 0,
        Align::Center => 1,
        Align::End => 2,
    }
}

fn load_row(
    row: &Json,
    index: usize,
    theme: &Theme,
    opts: &LoadOptions,
) -> Result<Widget, LoadError> {
    // Real dumps carry signed row ids (-1 spacers, repeated); negative ids become anonymous ids.
    let id = match row.get("row_id").and_then(Json::as_f64) {
        Some(v) if v >= 0.0 && v.fract() == 0.0 && v < 9.0e15 => v as u64,
        Some(v) if v.fract() == 0.0 && v < 0.0 && v > -4.0e9 => WidgetId::anon(index).0,
        _ => return schema(format!("row {index} lacks a usable row_id")),
    };
    let mut options = Vec::new();
    for option in row.get("options").and_then(Json::as_array).unwrap_or(&[]) {
        options.push(UiOption {
            // Real dumps use -1 for "no option id" (signed), kept as the same 32 bits.
            id: option
                .get("option_id")
                .and_then(Json::as_f64)
                .map_or(0, |v| v as i64 as u32),
            text: text_ref(option),
            enabled: option
                .get("enabled")
                .and_then(Json::as_bool)
                .unwrap_or(true),
        });
    }
    let activate = handler(row, "activate_callback");
    // node_type 9 is the divider node of the toolkit (MEASURED: row id -1, never navigable).
    let kind = if u(row, "node_type") == Some(9) {
        WidgetKind::Panel
    } else if let Some(t @ (5 | 8 | 10)) = u(row, "node_type") {
        WidgetKind::Custom(t as u32)
    } else if !options.is_empty() {
        WidgetKind::Cycler
    } else if activate.is_some() {
        WidgetKind::Button
    } else {
        WidgetKind::Label
    };
    let stride = theme.row_height + theme.row_gap;
    let mut rect = AnchoredRect {
        space: AnchorSpace::SafeArea4x3,
        horizontal: Edge::Center,
        vertical: Edge::Start,
        offset: Vec2::new(0.0, theme.top + index as f32 * stride),
        size: Vec2::new(theme.column_width, theme.row_height),
    };
    if let (Some(edges), Some(layout)) = (opts.rect_edges, row.get("layout")) {
        let cell = |offset: u32| {
            layout
                .get(&format!("0x{offset:x}"))
                .and_then(Json::as_f64)
                .map(|v| v as f32)
        };
        if let (Some(l), Some(t), Some(r), Some(b)) = (
            cell(edges.left),
            cell(edges.top),
            cell(edges.right),
            cell(edges.bottom),
        ) {
            if r >= l && b >= t {
                rect = menu_rect(Vec2::new(l, t), Vec2::new(r - l, b - t));
            }
        }
    } else if let (Some(cells), Some(layout)) = (opts.rect_cells, row.get("layout")) {
        let cell = |offset: u32| {
            layout
                .get(&format!("0x{offset:x}"))
                .and_then(Json::as_f64)
                .map(|v| v as f32)
        };
        if let (Some(x), Some(y), Some(w), Some(h)) = (
            cell(cells.x),
            cell(cells.y),
            cell(cells.width),
            cell(cells.height),
        ) {
            if w >= 0.0 && h >= 0.0 {
                rect = menu_rect(Vec2::new(x, y), Vec2::new(w, h));
            }
        }
    }
    let mut widget = Widget::new(WidgetId(id), kind);
    widget.rect = rect;
    widget.text = text_ref(row);
    widget.font_index = u(row, "font_index").unwrap_or(0) as u32;
    widget.value_font_index = u(row, "value_font_index").unwrap_or(0) as u32;
    widget.align = align_from_mode(u(row, "align_mode").unwrap_or(0));
    widget.uppercase = u(row, "row_flags").is_some_and(|flags| flags & ROW_FLAG_UPPERCASE != 0);
    widget.enabled =
        opts.ignore_row_enabled_flag || row.get("enabled").and_then(Json::as_bool).unwrap_or(true);
    widget.navigable = row.get("navigable").and_then(Json::as_bool).unwrap_or(true);
    widget.selected =
        (u(row, "selected_index").unwrap_or(0) as usize).min(options.len().saturating_sub(1));
    widget.options = options;
    widget.activate = activate;
    widget.draw = handler(row, "draw_callback");
    Ok(widget)
}

impl UiModel {
    /// Build a model from the text of a menu-tree JSON file.
    pub fn from_tree_json(text: &str, opts: &LoadOptions) -> Result<UiModel, LoadError> {
        let tree = Json::parse(text)?;
        if u(&tree, "schema") != Some(1) {
            return schema("unsupported or missing schema version");
        }
        let Some(pages) = tree.get("pages").and_then(Json::as_object) else {
            return schema("missing pages object");
        };
        let mut model = UiModel::new();
        for (key, page) in pages {
            let Some(id) = u(page, "page_id").or_else(|| parse_id(key)) else {
                return schema(format!("page {key} lacks an id"));
            };
            let mut out = Page::new(
                PageId(id),
                page.get("widget_types")
                    .and_then(Json::as_array)
                    .and_then(|a| a.first())
                    .and_then(Json::as_u64)
                    .unwrap_or(0) as u32,
            );
            out.handlers = PageHandlers {
                select: handler(page, "select_handler"),
                back: handler(page, "back_handler"),
                after_select: None,
                gate: handler(page, "gate"),
                draw: handler(page, "draw_callback"),
            };
            for (index, row) in page
                .get("rows")
                .and_then(Json::as_array)
                .unwrap_or(&[])
                .iter()
                .enumerate()
            {
                let mut widget = load_row(row, index, &model.theme, opts)?;
                if out.widget(widget.id).is_some() {
                    // A repeated id (real dumps repeat 0 and -1): keep the row under an anonymous id.
                    widget.id = WidgetId::anon(index);
                }
                out.widgets.push(widget);
            }
            model.pages.insert(out.id, out);
        }
        let page_ref = |value: &Json| -> Result<Option<PageId>, LoadError> {
            match value.as_str() {
                Some("root") => Ok(None),
                Some(s) => parse_id(s)
                    .map(|v| Some(PageId(v)))
                    .ok_or_else(|| LoadError::Schema(format!("bad page reference {s}"))),
                None => schema("edge endpoint is not a string"),
            }
        };
        for edge in tree.get("edges").and_then(Json::as_array).unwrap_or(&[]) {
            let (Some(from), Some(to), Some(kind)) = (
                edge.get("from"),
                edge.get("to"),
                edge.get("kind").and_then(Json::as_str),
            ) else {
                return schema("edge lacks from, to or kind");
            };
            let kind = match kind {
                "open" => EdgeKind::Open,
                "close" => EdgeKind::Close,
                other => return schema(format!("unknown edge kind {other}")),
            };
            model.edges.push(PageEdge {
                from: page_ref(from)?,
                to: page_ref(to)?,
                kind,
                input: edge
                    .get("input")
                    .and_then(Json::as_array)
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
            });
        }
        model.root = model
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::Open && e.from.is_none())
            .and_then(|e| e.to);
        Ok(model)
    }

    /// Write the model in the tree schema (the subset this crate keeps; schema 1, required
    /// capture-only fields are written as zero). Reloading the output with the same options
    /// gives an equal model for tree-derived content.
    pub fn to_tree_json(&self, opts: &LoadOptions) -> String {
        let mut out = String::from(
            "{\"schema\":1,\"samples\":0,\"runs\":[],\"language_index_values\":[],\"pages\":{",
        );
        let hex = |v: u64| format!("0x{v:x}");
        for (n, (id, page)) in self.pages.iter().enumerate() {
            if n > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "\"{}\":{{\"page_id\":{},\"builder\":0,\"gate\":{},\"draw_callback\":{},\"widget_types\":[{}],\"states\":[],\"first_poll\":0,\"samples\":0,\"select_handler\":{},\"back_handler\":{},\"rows\":[",
                hex(id.0),
                id.0,
                legacy(&page.handlers.gate),
                legacy(&page.handlers.draw),
                page.widget_type,
                legacy(&page.handlers.select),
                legacy(&page.handlers.back),
            ));
            for (m, w) in page.widgets.iter().enumerate() {
                if m > 0 {
                    out.push(',');
                }
                out.push_str(&row_json(w, opts));
            }
            out.push_str("]}");
        }
        out.push_str("},\"edges\":[");
        for (n, e) in self.edges.iter().enumerate() {
            if n > 0 {
                out.push(',');
            }
            let endpoint = |p: Option<PageId>| match p {
                Some(id) => format!("\"{}\"", hex(id.0)),
                None => "\"root\"".to_string(),
            };
            let inputs: Vec<String> = e
                .input
                .iter()
                .map(|s| format!("\"{}\"", escape(s)))
                .collect();
            out.push_str(&format!(
                "{{\"from\":{},\"to\":{},\"kind\":\"{}\",\"input\":[{}],\"runs\":[],\"polls\":[]}}",
                endpoint(e.from),
                endpoint(e.to),
                if e.kind == EdgeKind::Open {
                    "open"
                } else {
                    "close"
                },
                inputs.join(",")
            ));
        }
        out.push_str("]}");
        out
    }
}

fn legacy(h: &Option<HandlerRef>) -> u32 {
    match h {
        Some(HandlerRef::Legacy(v)) => *v,
        _ => 0,
    }
}
fn escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            c if (c as u32) < 0x20 => format!("\\u{:04x}", c as u32).chars().collect(),
            c => vec![c],
        })
        .collect()
}
fn text_fields(t: TextRef) -> String {
    match t {
        TextRef::Id(id) => format!("\"text_id\":{},\"text_dynamic\":false", id.0),
        TextRef::Dynamic => "\"text_id\":null,\"text_dynamic\":true".to_string(),
        TextRef::None => "\"text_id\":null,\"text_dynamic\":false".to_string(),
    }
}
fn row_json(w: &Widget, opts: &LoadOptions) -> String {
    let options: Vec<String> = w
        .options
        .iter()
        .map(|o| {
            format!(
                "{{\"option_id\":{},{},\"flags\":0,\"enabled\":{}}}",
                o.id,
                text_fields(o.text),
                o.enabled
            )
        })
        .collect();
    let selected_option = w.options.get(w.selected).map_or(0, |o| o.id);
    let plain = w.rect.space == AnchorSpace::SafeArea4x3
        && w.rect.horizontal == Edge::Start
        && w.rect.vertical == Edge::Start;
    let layout = match opts.rect_cells {
        Some(c) if plain => format!(
            "{{\"0x{:x}\":{},\"0x{:x}\":{},\"0x{:x}\":{},\"0x{:x}\":{}}}",
            c.x,
            w.rect.offset.x,
            c.y,
            w.rect.offset.y,
            c.width,
            w.rect.size.x,
            c.height,
            w.rect.size.y
        ),
        _ => "{}".to_string(),
    };
    format!(
        "{{\"row_id\":{},\"node_type\":{},\"kind\":0,\"enabled\":{},\"navigable\":{},{},\"font_index\":{},\"value_font_index\":{},\"align_mode\":{},\"option_count\":{},\"selected_index\":{},\"selected_option_id\":{},\"activate_callback\":{},\"draw_callback\":{},\"layout\":{},\"options\":[{}]}}",
        w.id.0,
        match w.kind {
            WidgetKind::Panel => 9,
            WidgetKind::Custom(t) => t,
            _ => 7,
        },
        w.enabled,
        w.navigable,
        text_fields(w.text),
        w.font_index,
        w.value_font_index,
        align_mode(w.align),
        w.options.len(),
        w.selected,
        selected_option,
        legacy(&w.activate),
        legacy(&w.draw),
        layout,
        options.join(",")
    )
}
