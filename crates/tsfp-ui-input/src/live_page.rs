// SPDX-License-Identifier: GPL-3.0-or-later
//! T2311 UI-13: the drawable content of a live page, read from the original page record.
//!
//! The Rust page drawer (crates/tsfp-ui-wgpu `in_frame`) needs what the original draw 0x76810
//! reads: every row of the page in creation order (`+0x4` chain from page `+0x178`), its node
//! type, flags, text, fonts, alignment, layout box and option entries. [`read_page`] reads them
//! from the live guest records (the same fields the menu-tree dump tool `tools/ui_tree.py`
//! decodes, so a live page and a dumped page give the same model), and [`LivePage::to_model_page`]
//! turns them into a `tsfp_ui_model::Page` the skin and renderer already draw. Nothing is
//! written. Text pointers are mapped back to string ids through the two bank pointer tables
//! (common bank `[0x6B7B10]`, story bank `[0x6B7B08]` from id 0xD87); a pointer outside both
//! banks is runtime text (a level name, a profile name) and its bytes are read as they are.

use crate::menu_state::layout::{PAGE_STRIDE, PAGE_TABLE, option, page, row};
use crate::native_page::GuestUi;
use std::collections::BTreeMap;
use tsfp_ui_model::loader::ROW_FLAG_UPPERCASE;
use tsfp_ui_model::{
    Align, HandlerRef, Page, PageId, TextAnchor, TextId, TextRef, UiOption, Widget, WidgetId,
    WidgetKind, menu_rect,
};
use tsfp_ui_renderer::Vec2;

/// Row record fields the drawer needs beyond [`row`] (tools/ui_tree.py `decode_row`, MEASURED).
pub mod draw_fields {
    pub const FONT: u32 = 0x1C;
    pub const TEXT: u32 = 0x20;
    pub const DRAW: u32 = 0x50;
    /// Row brightness (f32), read by the list row draws `0x2CA9F0` and `0x2C8B50` (MEASURED).
    pub const BRIGHTNESS: u32 = 0x34;
    /// The row's layout box, f32 left, top, right, bottom (design units, `RectEdges::DUMP`).
    pub const BOX_LEFT: u32 = 0x78;
    pub const VALUE_FONT: u32 = 0xBC;
    pub const ALIGN: u32 = 0xC0;
    /// T2315: label placement of the default row draw `0x76150` (MEASURED from code): alignment mode, x and y offsets
    /// in units of [`UNIT_X`] / [`UNIT_Y`] from the row's top-left corner (the row layout `0x750F0` writes them).
    pub const TEXT_ALIGN: u32 = 0x30;
    pub const TEXT_X: u32 = 0x3C;
    /// T2315: `0x76150` alignment 2 centres on half the row width when this word is zero, else on half of it plus page `+0x120`.
    pub const TEXT_WIDTH: u32 = 0x44;
    pub const PAGE_LABEL_CENTRE: u32 = 0x120;
    pub const TEXT_Y: u32 = 0x40;
    /// Unit globals of the front end (6.4 and 4.8 design units on the owner's run, MEASURED live).
    pub const UNIT_X: u32 = 0x7B_A038;
    pub const UNIT_Y: u32 = 0x7B_A188;
    /// T2315 item 4: the row geometry of `0x71E90` (MEASURED from code and live): row `+0x64` x, `+0x68` y, `+0x6C`
    /// width, `+0x70` height in units, the parent record `+0x88` (the page record on the batch-1 pages) with its top
    /// `+0x80`, view top `+0x6C` and scroll offset `+0x84` (units), the page x origin `+0xA8` (design units) and the y
    /// origin `[0x7BA184]`. Rows with flag [`FLAG_SCROLLED`] belong to the page's scrolled list.
    pub const GEOM_X: u32 = 0x64;
    pub const GEOM_Y: u32 = 0x68;
    pub const GEOM_W: u32 = 0x6C;
    pub const GEOM_H: u32 = 0x70;
    pub const PARENT: u32 = 0x88;
    pub const PARENT_TOP: u32 = 0x80;
    pub const PARENT_VIEW_TOP: u32 = 0x6C;
    pub const PARENT_SCROLL: u32 = 0x84;
    pub const PAGE_X: u32 = 0xA8;
    pub const ORIGIN_Y: u32 = 0x7B_A184;
    /// Row flag (`+0x28`): the row scrolls with the page list (`0x71E90` adds view top minus scroll offset).
    pub const FLAG_SCROLLED: u32 = 0x40;
    /// T2315 3b: the option value draw `0x7F8B0` / `0x7B7B0` (MEASURED from code): value box `+0xE4..+0xF0` (units),
    /// text offset `+0xDC` / `+0xE0` (units), alignment `+0xC0`, value font `+0xBC`, row value count `+0xC4`; globals
    /// `[0x7BA180]` (x origin), `[0x7BA18C]` (page x scale of the value box).
    pub const VALUE_BOX: u32 = 0xE4;
    pub const VALUE_DX: u32 = 0xDC;
    pub const VALUE_DY: u32 = 0xE0;
    pub const VALUE_COUNT: u32 = 0xC4;
    pub const ORIGIN_X: u32 = 0x7B_A180;
    pub const VALUE_X_SCALE: u32 = 0x7B_A18C;
    /// Font height of the selected text style `[0x7E7264]` (`0x3F8D0`): the record height `[0x4C8C40 + 0x38 * record]`
    /// times `[0x7E7274]`, `int [0x4E8CF4]`, `[0x478390]`, the style's y scale (`+0xC` of the style `0x4C8DE8 + 20 *
    /// style`) and `[0x7E7280]`.
    pub const STYLES: u32 = 0x4C_8DE8;
    pub const STYLE_STRIDE: u32 = 20;
    pub const RECORD_HEIGHTS: u32 = 0x4C_8C40;
    pub const RECORD_STRIDE: u32 = 0x38;
    pub const HEIGHT_SCALE_A: u32 = 0x7E_7274;
    pub const HEIGHT_LINES: u32 = 0x4E_8CF4;
    pub const HEIGHT_SCALE_B: u32 = 0x47_8390;
    pub const HEIGHT_SCALE_C: u32 = 0x7E_7280;
    /// Option entry text pointer.
    pub const OPTION_TEXT: u32 = 0x04;
    /// Page record fade (f32, 1.0 = shown).
    pub const PAGE_FADE: u32 = 0xF4;
    /// Text bank pointer tables (pointers to arrays of string pointers).
    pub const STORY_BANK: u32 = 0x6B_7B08;
    pub const COMMON_BANK: u32 = 0x6B_7B10;
    pub const STORY_FIRST_ID: u32 = 0xD87;
    /// Bank sizes in entries (`tools/data/rust_trace_regions_ui_tree.json`: 0x361C and 0x40D4 bytes).
    pub const COMMON_ENTRIES: u32 = 0x361C / 4;
    pub const STORY_ENTRIES: u32 = 0x40D4 / 4;
}
use draw_fields as f;

/// Longest runtime text read from guest memory.
const MAX_TEXT: usize = 128;
/// Rows read at most per page (the node heap holds 0xD4C0 / 0x10C = 203 nodes).
const MAX_ROWS: usize = 256;

/// The text of a row or option.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveText {
    None,
    Id(u32),
    /// Runtime text (not in a string bank), as the bytes the original draws.
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveOption {
    pub id: u32,
    pub text: LiveText,
    pub enabled: bool,
}

/// One row (node) of a live page, in creation order.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveRow {
    pub node: u32,
    pub node_type: u32,
    pub id: i32,
    pub flags: u32,
    pub text: LiveText,
    pub font: u32,
    pub value_font: u32,
    pub align: u32,
    /// Left, top, right, bottom in design units, when all four are finite and ordered. For a row of a scrolled list
    /// (T2315) this is the row's full geometry (`0x71E90`), also when the row is only partly inside the view.
    pub rect: Option<[f32; 4]>,
    /// T2315: the layout box (`+0x78`, cut to the list view by the layout) of a scrolled-list row: the original draws the
    /// row with the viewport set from it (`0x722B0`). None for other rows (their box is their rect).
    pub clip: Option<[f32; 4]>,
    /// T2315 3b: where `0x7B7B0` draws the selected option text of a kind-1 value row (None for other rows).
    pub value: Option<LiveValue>,
    pub activate: u32,
    pub draw: u32,
    pub kind: u32,
    pub selected: u32,
    pub options: Vec<LiveOption>,
    /// T2311: row brightness `+0x34` (f32, rises while focused; the list row draws read it).
    pub brightness: f32,
    /// T2311: row height `+0x70` (f32, design units).
    pub height: f32,
    /// T2315: `+0x30` label alignment mode, `+0x3C` / `+0x40` label offsets (unit multiples, see [`draw_fields`]).
    pub text_align: u32,
    pub text_x: f32,
    pub text_y: f32,
}

/// T2315 3b: the option text placement of `0x7B7B0` (design units, absolute): the anchor and how the text sits on it
/// (`+0xC0` 1 and 4: the text ends at `x`; 2: centred on `x`; else starts at `x`), the anchor already moved by the value
/// font height as the original moves it (the side arrows take that room). `arrows`: `+0xB8` bit 0x400000 clear (the
/// side arrows `0x81AD0` / `0x81B20` are not drawn by Rust yet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiveValue {
    pub x: f32,
    pub y: f32,
    pub align: Align,
    pub arrows: bool,
    /// T2315 increment 3, the side arrows of `0x7B7B0`: the anchor before the height shift (`box x0 + +0xDC * ux`), the
    /// value box's left and right edges, the alignment mode `+0xC0` and the value style height `[0x7E7264]`.
    pub anchor: f32,
    pub box_x: (f32, f32),
    pub mode: u32,
    pub height: f32,
}

/// The drawable content of one live page.
#[derive(Clone, Debug, PartialEq)]
pub struct LivePage {
    pub slot: u32,
    pub builder: u32,
    pub state: u32,
    /// The node the cursor is on (`+0x198`), 0 for none.
    pub cursor_node: u32,
    pub rows: Vec<LiveRow>,
    /// T2315: `[0x7BA038]`, `[0x7BA188]` (design units per row offset unit, x and y).
    pub units: (f32, f32),
}

/// String pointer to id, built once per bank base (the banks do not move while the language stays).
#[derive(Clone, Debug, Default)]
pub struct TextIndex {
    bases: (u32, u32),
    ids: BTreeMap<u32, u32>,
}

impl TextIndex {
    /// Refresh when the bank bases changed (language switch or first use).
    pub fn refresh(&mut self, g: &dyn GuestUi) {
        let bases = (
            g.read32(f::COMMON_BANK).unwrap_or(0),
            g.read32(f::STORY_BANK).unwrap_or(0),
        );
        if bases == self.bases && !self.ids.is_empty() {
            return;
        }
        self.bases = bases;
        self.ids.clear();
        for (base, first, count) in [
            (bases.0, 0, f::COMMON_ENTRIES),
            (bases.1, f::STORY_FIRST_ID, f::STORY_ENTRIES),
        ] {
            if base == 0 {
                continue;
            }
            for index in 0..count {
                if let Some(pointer) = g.read32(base + index * 4).filter(|p| *p != 0) {
                    self.ids.entry(pointer).or_insert(first + index);
                }
            }
        }
    }

    pub fn id(&self, pointer: u32) -> Option<u32> {
        self.ids.get(&pointer).copied()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

fn read_bytes(g: &dyn GuestUi, mut va: u32) -> Vec<u8> {
    let mut out = Vec::new();
    while out.len() < MAX_TEXT {
        let Some(word) = g.read32(va & !3) else { break };
        let byte = (word >> ((va & 3) * 8)) as u8;
        if byte == 0 {
            break;
        }
        out.push(byte);
        va += 1;
    }
    out
}

fn text_at(g: &dyn GuestUi, texts: &TextIndex, pointer: u32) -> LiveText {
    if pointer == 0 {
        LiveText::None
    } else if let Some(id) = texts.id(pointer) {
        LiveText::Id(id)
    } else {
        LiveText::Bytes(read_bytes(g, pointer))
    }
}

fn read_options(g: &dyn GuestUi, texts: &TextIndex, first: u32) -> Vec<LiveOption> {
    let mut out = Vec::new();
    let mut at = first;
    while at != 0 && out.len() < 256 && g.read32(at).is_some_and(|w| w != 0) {
        let flags = g.read32(at + option::FLAGS).unwrap_or(0);
        out.push(LiveOption {
            id: g.read32(at + option::ID).unwrap_or(0),
            text: text_at(g, texts, g.read32(at + f::OPTION_TEXT).unwrap_or(0)),
            enabled: flags & option::FLAG_DISABLED == 0,
        });
        let next = g.read32(at + option::NEXT).unwrap_or(0);
        if next == at {
            break;
        }
        at = next;
    }
    out
}

fn read_f32(g: &dyn GuestUi, va: u32) -> f32 {
    f32::from_bits(g.read32(va).unwrap_or(0))
}

/// T2315: the row rectangle `0x71E90` computes for the row draw (design units), None when a field is not finite or the
/// row has no parent. x from the page x origin, y from the parent's top plus, for scrolled rows, the view top minus the
/// scroll offset; width and height in units.
pub fn row_geometry(g: &dyn GuestUi, node: u32) -> Option<[f32; 4]> {
    let parent = g.read32(node + f::PARENT).filter(|p| *p != 0)?;
    let (ux, uy) = (read_f32(g, f::UNIT_X), read_f32(g, f::UNIT_Y));
    let x0 = read_f32(g, node + f::GEOM_X) * ux + read_f32(g, parent + f::PAGE_X);
    let mut y = read_f32(g, parent + f::PARENT_TOP) + read_f32(g, node + f::GEOM_Y);
    if g.read32(node + row::FLAGS).unwrap_or(0) & f::FLAG_SCROLLED != 0 {
        y += read_f32(g, parent + f::PARENT_VIEW_TOP) - read_f32(g, parent + f::PARENT_SCROLL);
    }
    let y0 = uy * y + read_f32(g, f::ORIGIN_Y);
    let out = [
        x0,
        y0,
        x0 + read_f32(g, node + f::GEOM_W) * ux,
        y0 + read_f32(g, node + f::GEOM_H) * uy,
    ];
    out.iter().all(|v| v.is_finite()).then_some(out)
}

/// T2315 3b: `[0x7E7264]` for text style `style`, as `0x3F8D0` computes it (design units).
pub fn style_height(g: &dyn GuestUi, style: u32) -> Option<f32> {
    let object = f::STYLES + f::STYLE_STRIDE * style;
    let record = g.read32(object)?;
    let lines = g.read32(f::HEIGHT_LINES)? as i32 as f32;
    let height = read_f32(g, f::RECORD_HEIGHTS + f::RECORD_STRIDE * record)
        * read_f32(g, f::HEIGHT_SCALE_A)
        * lines
        * read_f32(g, f::HEIGHT_SCALE_B)
        * read_f32(g, object + 0xC)
        * read_f32(g, f::HEIGHT_SCALE_C);
    (height.is_finite() && height > 0.0).then_some(height)
}

/// T2315 3b: the value placement of a kind-1 row (`0x76810` second row loop: row type 4, 7 or 8 with `+0xC4 > 0`; kind
/// `+0xB4` 1, the option list), from the value box `0x7B520` and `0x7B7B0`. None for other rows or unreadable fields.
pub fn read_value(g: &dyn GuestUi, node: u32) -> Option<LiveValue> {
    let get = |field: u32| g.read32(node + field).unwrap_or(0);
    let shown = matches!(get(row::TYPE), 4 | 7 | 8) && get(f::VALUE_COUNT) as i32 > 0;
    if !shown || get(row::KIND) != row::KIND_CYCLER {
        return None;
    }
    let parent = g.read32(node + f::PARENT).filter(|p| *p != 0)?;
    let (ux, uy) = (read_f32(g, f::UNIT_X), read_f32(g, f::UNIT_Y));
    let origin_x = read_f32(g, f::ORIGIN_X);
    let field = |offset: u32| read_f32(g, node + offset);
    let x0_units = (read_f32(g, parent + f::PAGE_X) - origin_x) * read_f32(g, f::VALUE_X_SCALE)
        + field(f::VALUE_BOX);
    let mut y0_units = field(f::VALUE_BOX + 4) + read_f32(g, parent + f::PARENT_TOP);
    if get(row::FLAGS) & f::FLAG_SCROLLED != 0 {
        y0_units +=
            read_f32(g, parent + f::PARENT_VIEW_TOP) - read_f32(g, parent + f::PARENT_SCROLL);
    }
    let box_x0 = x0_units * ux + origin_x;
    let box_x1 = box_x0 + field(f::VALUE_BOX + 8) * ux;
    let box_y0 = y0_units * uy + read_f32(g, f::ORIGIN_Y);
    let height = style_height(g, get(f::VALUE_FONT))?;
    let x = box_x0 + field(f::VALUE_DX) * ux;
    let (x, align) = match get(f::ALIGN) {
        1 | 4 => (x - height, Align::End),
        2 => ((box_x0 + box_x1) * 0.5, Align::Center),
        _ => (x + height, Align::Start),
    };
    let value = LiveValue {
        x,
        y: box_y0 + field(f::VALUE_DY) * uy,
        align,
        arrows: get(row::KIND_FLAGS) & row::KIND_NO_CYCLE == 0,
        anchor: box_x0 + field(f::VALUE_DX) * ux,
        box_x: (box_x0, box_x1),
        mode: get(f::ALIGN),
        height,
    };
    [value.x, value.y]
        .iter()
        .all(|v| v.is_finite())
        .then_some(value)
}

/// T2315 increment 4: the label x offset (text units) of `0x76150`. Alignment 2 recomputes it at every draw and stores it
/// in `+0x3C`: half the row width `+0x6C` when `+0x44` is zero, else half of `+0x44` plus the page's `+0x120`. Since the Rust drawer skips
/// the original draw, `+0x3C` keeps the builder's value, which is wrong on the pause list (labels shifted left, looked
/// at in Arcade and Story); alignments 0 and 1 use `+0x3C` as is.
fn label_x(g: &dyn GuestUi, node: u32) -> f32 {
    if g.read32(node + f::TEXT_ALIGN) != Some(2) {
        return read_f32(g, node + f::TEXT_X);
    }
    let width = read_f32(g, node + f::TEXT_WIDTH);
    if width == 0.0 {
        read_f32(g, node + f::GEOM_W) * 0.5
    } else {
        width * 0.5
            + g.read32(node + f::PARENT)
                .filter(|p| *p != 0)
                .map_or(0.0, |p| read_f32(g, p + f::PAGE_LABEL_CENTRE))
    }
}

/// Read one row node.
pub fn read_row(g: &dyn GuestUi, texts: &TextIndex, node: u32) -> LiveRow {
    let get = |field: u32| g.read32(node + field).unwrap_or(0);
    let edges: [f32; 4] = std::array::from_fn(|i| read_f32(g, node + f::BOX_LEFT + 4 * i as u32));
    let boxed =
        (edges.iter().all(|v| v.is_finite()) && edges[2] >= edges[0] && edges[3] >= edges[1])
            .then_some(edges);
    let scrolled = get(row::FLAGS) & f::FLAG_SCROLLED != 0;
    let (rect, clip) = match boxed {
        Some(edges) if scrolled => (Some(row_geometry(g, node).unwrap_or(edges)), Some(edges)),
        other => (other, None),
    };
    LiveRow {
        node,
        node_type: get(row::TYPE),
        id: get(row::ID) as i32,
        flags: get(row::FLAGS),
        text: text_at(g, texts, get(f::TEXT)),
        font: get(f::FONT),
        value_font: get(f::VALUE_FONT),
        align: get(f::ALIGN),
        rect,
        clip,
        value: read_value(g, node),
        activate: get(row::ON_ACTIVATE),
        draw: get(f::DRAW),
        kind: get(row::KIND),
        selected: get(row::OPTION_INDEX),
        options: read_options(g, texts, get(row::OPTIONS)),
        brightness: read_f32(g, node + f::BRIGHTNESS),
        height: read_f32(g, node + row::HEIGHT),
        text_align: get(f::TEXT_ALIGN),
        text_x: label_x(g, node),
        text_y: read_f32(g, node + f::TEXT_Y),
    }
}

/// The page record of `slot`, or `None` when the slot is empty.
pub fn read_page(g: &dyn GuestUi, texts: &TextIndex, slot: u32) -> Option<LivePage> {
    let record = PAGE_TABLE + slot * PAGE_STRIDE;
    if g.read32(record + page::VTABLE).unwrap_or(0) == 0 {
        return None;
    }
    let builder = g.read32(record + page::BUILDER)?;
    let mut rows = Vec::new();
    let mut node = g.read32(record + page::FIRST_NODE).unwrap_or(0);
    while node != 0 && rows.len() < MAX_ROWS {
        rows.push(read_row(g, texts, node));
        let next = g.read32(node + row::NEXT).unwrap_or(0);
        if rows.iter().any(|r| r.node == next) {
            break;
        }
        node = next;
    }
    Some(LivePage {
        slot,
        builder,
        state: g.read32(record + page::STATE).unwrap_or(0),
        cursor_node: g.read32(record + page::CURSOR_ROW).unwrap_or(0),
        rows,
        units: (read_f32(g, f::UNIT_X), read_f32(g, f::UNIT_Y)),
    })
}

/// Original panel rectangle (`0x2D7B70`/`0x2D76B0`, page +0xE4..+0xF0), in design units.
/// Missing or invalid geometry leaves dump-only callers on the row-union fallback.
pub fn panel_bounds(g: &dyn GuestUi, slot: u32) -> Option<tsfp_ui_renderer::AnchoredRect> {
    let record = PAGE_TABLE + slot * PAGE_STRIDE;
    let mut edges = [0.0; 4];
    for (index, edge) in edges.iter_mut().enumerate() {
        *edge = f32::from_bits(g.read32(record + 0xE4 + index as u32 * 4)?);
        if !edge.is_finite() {
            return None;
        }
    }
    // 0x2D7AE0 converts the page edges to integer draw coordinates before the rim.
    // Preserve truncation: live builder products can be just below an integer.
    let [left, top, right, bottom] = edges.map(f32::trunc);
    (right > left && bottom > top)
        .then(|| menu_rect(Vec2::new(left, top), Vec2::new(right - left, bottom - top)))
}

/// Read-only edge contract of `0x2D9390`, preserving fractional page edges and style margins.
pub fn highlight_edges(g: &dyn GuestUi, slot: u32) -> Option<tsfp_ui_model::model::HighlightEdges> {
    let record = PAGE_TABLE + slot * PAGE_STRIDE;
    let style = g.read32(record + page::STYLE)?;
    if style == 0 {
        return None;
    }
    let top = f32::from_bits(g.read32(record + 0xE8)?);
    let bottom = f32::from_bits(g.read32(record + 0xF0)?);
    let unit = f32::from_bits(g.read32(f::UNIT_Y)?);
    let top_margin = f32::from_bits(g.read32(style + 0x10)?) * unit;
    let bottom_margin = f32::from_bits(g.read32(style + 0x18)?) * unit;
    [top, bottom, top_margin, bottom_margin]
        .iter()
        .all(|v| v.is_finite())
        .then_some(tsfp_ui_model::model::HighlightEdges {
            top,
            bottom,
            top_margin,
            bottom_margin,
        })
        .filter(|edges| edges.bottom > edges.top)
}

/// The page fade of `slot` (`+0xF4`), raw f32 bits.
pub fn page_fade_bits(g: &dyn GuestUi, slot: u32) -> u32 {
    g.read32(PAGE_TABLE + slot * PAGE_STRIDE + f::PAGE_FADE)
        .unwrap_or(0x3F80_0000)
}

impl LiveText {
    fn model(&self) -> TextRef {
        match self {
            LiveText::None => TextRef::None,
            LiveText::Id(id) => TextRef::Id(TextId(*id)),
            LiveText::Bytes(_) => TextRef::Dynamic,
        }
    }
}

/// Widget id of the row at `index` (rows are keyed by position: real pages repeat row ids).
pub fn widget_id(index: usize) -> WidgetId {
    WidgetId(index as u64)
}

impl LivePage {
    /// The model page the skin draws, keyed by `id`, plus the runtime text of its rows. Mirrors
    /// the dump loader (`tsfp_ui_model::loader`, `dump_load_options`): rects from the layout box,
    /// the row enabled bit ignored, node type 9 a divider panel, 5/8/10 custom, an option list a
    /// cycler, an activate callback a button.
    pub fn to_model_page(&self, id: PageId) -> (Page, BTreeMap<WidgetId, Vec<u8>>) {
        let mut out = Page::new(id, 1);
        let mut dynamic = BTreeMap::new();
        for (index, r) in self.rows.iter().enumerate() {
            // T2311: a row without a valid layout box is not shown (the scrolled lists of Story
            // and Challenge leave the rows outside the view without one, MEASURED live), so it
            // gets no widget; widget ids stay the row index.
            let Some([l, t, rr, b]) = r.rect else {
                continue;
            };
            let options: Vec<UiOption> = r
                .options
                .iter()
                .map(|o| UiOption {
                    id: o.id,
                    text: o.text.model(),
                    enabled: o.enabled,
                })
                .collect();
            let kind = match r.node_type {
                9 => WidgetKind::Panel,
                t @ (5 | 8 | 10) => WidgetKind::Custom(t),
                _ if !options.is_empty() => WidgetKind::Cycler,
                _ if r.activate != 0 => WidgetKind::Button,
                _ => WidgetKind::Label,
            };
            let mut w = Widget::new(widget_id(index), kind);
            w.rect = menu_rect(Vec2::new(l, t), Vec2::new(rr - l, b - t));
            w.bounds = r
                .clip
                .map(|[l, t, rr, b]| menu_rect(Vec2::new(l, t), Vec2::new(rr - l, b - t)));
            w.text = r.text.model();
            if let LiveText::Bytes(bytes) = &r.text {
                dynamic.insert(w.id, bytes.clone());
            }
            w.font_index = r.font;
            w.value_font_index = r.value_font;
            w.align = match r.align {
                1 => Align::Center,
                2 => Align::End,
                _ => Align::Start,
            };
            w.uppercase = u64::from(r.flags) & ROW_FLAG_UPPERCASE != 0;
            w.text_anchor = self.text_anchor(r);
            // T2315 3b: the selected option text where `0x7B7B0` draws it; runtime option text is bound like a label's.
            if let (Some(value), Some(selected)) = (r.value, r.options.get(r.selected as usize)) {
                w.value_anchor = Some(TextAnchor {
                    x: value.x - l,
                    y: value.y - t,
                    align: value.align,
                });
                if let (LiveText::Bytes(bytes), false) =
                    (&selected.text, matches!(r.text, LiveText::Bytes(_)))
                {
                    dynamic.insert(w.id, bytes.clone());
                }
            }
            // T2315: the default row text colour `0x75F60` of the active page takes the greyed entries of the row's colour
            // table for row flag bit 0 (MEASURED from code; the Players rows 2 to 4 without a pad). Rows with a draw
            // callback (the list rows) grey their own text (`list_rows`), so only plain rows are disabled here.
            w.enabled = r.flags & row::FLAG_GREYED == 0 || r.draw != 0;
            w.navigable = r.flags & row::FLAG_SKIP == 0;
            // T3811: the original cursor and pointer rest on a greyed row (only the skip flag passes it over), and A there
            // gives the denied sound or, beside a help button, the help (`native_page::help_member`).
            w.focus_when_disabled = !w.enabled && w.navigable;
            w.selected = (r.selected as usize).min(options.len().saturating_sub(1));
            w.options = options;
            w.activate = (r.activate != 0).then_some(HandlerRef::Legacy(r.activate));
            w.draw = (r.draw != 0).then_some(HandlerRef::Legacy(r.draw));
            out.widgets.push(w);
        }
        (out, dynamic)
    }

    /// T2315: the label anchor of a row the default row draw `0x76150` labels (list rows with a draw callback too), when the offsets and
    /// units are finite and the units positive.
    pub fn text_anchor(&self, r: &LiveRow) -> Option<TextAnchor> {
        let (ux, uy) = self.units;
        let finite = [ux, uy, r.text_x, r.text_y].iter().all(|v| v.is_finite());
        (finite && ux > 0.0 && uy > 0.0).then(|| TextAnchor {
            x: r.text_x * ux,
            y: r.text_y * uy,
            align: match r.text_align {
                2 => Align::Center,
                1 => Align::End,
                _ => Align::Start,
            },
        })
    }

    /// The widget the cursor is on.
    pub fn focused(&self) -> Option<WidgetId> {
        (self.cursor_node != 0)
            .then(|| self.rows.iter().position(|r| r.node == self.cursor_node))
            .flatten()
            .map(widget_id)
    }

    /// Row draw callbacks of the page (non-zero), deduplicated: what the original draw would call
    /// besides the generic row draw.
    pub fn draw_callbacks(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .rows
            .iter()
            .map(|r| r.draw)
            .filter(|d| *d != 0)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Cursor index (position among cursor rows, node types 4, 7, 8 on the `+0x8` chain) of the row
    /// at `index`, given the cursor-row nodes in cursor order.
    /// T3781: the model widgets of the guest cursor rows, in cursor order. A greyed row keeps its cursor position (the
    /// two player main menu greys one row), so a row map of the focusable widgets alone is shorter than the guest list
    /// and shifts every row after the greyed one.
    pub fn cursor_widgets(&self, cursor_nodes: &[u32]) -> Vec<WidgetId> {
        cursor_nodes
            .iter()
            .filter_map(|node| self.rows.iter().position(|r| r.node == *node))
            .map(widget_id)
            .collect()
    }

    pub fn cursor_index(&self, index: usize, cursor_nodes: &[u32]) -> Option<u32> {
        let node = self.rows.get(index)?.node;
        cursor_nodes
            .iter()
            .position(|n| *n == node)
            .map(|p| p as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// FABRICATED guest memory (no game data).
    #[derive(Default)]
    struct Mem(HashMap<u32, u32>);
    impl GuestUi for Mem {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(self.0.get(&va).copied().unwrap_or(0))
        }
        fn write32(&mut self, _: u32, _: u32) -> bool {
            false
        }
        fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
            None
        }
    }
    impl Mem {
        fn f(&mut self, va: u32, v: f32) {
            self.0.insert(va, v.to_bits());
        }
    }

    #[test]
    fn highlight_contract_keeps_fractional_edges_and_scales_style_margins() {
        let mut memory = Mem::default();
        let record = PAGE_TABLE + PAGE_STRIDE;
        let style = 0x1000;
        memory.0.insert(record + page::STYLE, style);
        memory.f(record + 0xE8, 100.5);
        memory.f(record + 0xF0, 300.5);
        memory.f(f::UNIT_Y, 4.8);
        memory.f(style + 0x10, 2.0);
        memory.f(style + 0x18, 3.0);
        let edges = highlight_edges(&memory, 1).unwrap();
        assert_eq!((edges.top, edges.bottom), (100.5, 300.5));
        assert_eq!(edges.top_margin, 9.6);
        assert_eq!(edges.bottom_margin, 3.0 * 4.8);
        memory.f(style + 0x10, f32::NAN);
        assert_eq!(highlight_edges(&memory, 1), None);
    }

    #[test]
    fn panel_geometry_preserves_non_row_extents_and_rejects_invalid_boxes() {
        let mut memory = Mem::default();
        let record = PAGE_TABLE + PAGE_STRIDE;
        for (index, value) in [100.9, 120.9, 550.9, 410.9].into_iter().enumerate() {
            memory.f(record + 0xE4 + index as u32 * 4, value);
        }
        assert_eq!(
            panel_bounds(&memory, 1),
            Some(menu_rect(Vec2::new(100.0, 120.0), Vec2::new(450.0, 290.0)))
        );
        memory.f(record + 0xEC, 90.0);
        assert_eq!(panel_bounds(&memory, 1), None);
        memory.f(record + 0xEC, f32::NAN);
        assert_eq!(panel_bounds(&memory, 1), None);
        assert_eq!(panel_bounds(&memory, 0), None);
    }

    fn fabricated() -> Mem {
        let mut m = Mem::default();
        let slot = 3;
        let rec = PAGE_TABLE + slot * PAGE_STRIDE;
        m.0.insert(rec, 1);
        m.0.insert(rec + page::BUILDER, 0x2D_08C0);
        m.0.insert(rec + page::STATE, 2);
        // Banks: common bank array at 0x10000 with entry 5 -> string at 0x20000.
        m.0.insert(f::COMMON_BANK, 0x1_0000);
        m.0.insert(0x1_0000 + 5 * 4, 0x2_0000);
        // Rows at 0x30000 and 0x30200; the second has runtime text "Ab" at 0x40000.
        let (a, b) = (0x3_0000, 0x3_0200);
        m.0.insert(rec + page::FIRST_NODE, a);
        m.0.insert(rec + page::CURSOR_ROW, b);
        for (node, top) in [(a, 100.0), (b, 140.0)] {
            m.0.insert(node + row::TYPE, 7);
            m.f(node + f::BOX_LEFT, 176.0);
            m.f(node + f::BOX_LEFT + 4, top);
            m.f(node + f::BOX_LEFT + 8, 464.0);
            m.f(node + f::BOX_LEFT + 12, top + 30.0);
            m.0.insert(node + f::FONT, 4);
        }
        m.0.insert(a + row::NEXT, b);
        m.0.insert(a + f::TEXT, 0x2_0000);
        m.0.insert(a + row::ON_ACTIVATE, 0x1234);
        m.0.insert(b + f::TEXT, 0x4_0000);
        m.0.insert(b + row::FLAGS, 0x2000 | row::FLAG_SKIP);
        m.0.insert(0x4_0000, u32::from_le_bytes(*b"Ab\0\0"));
        m
    }

    /// T2315 increment 4: alignment 2 labels centre on half the row width when `+0x44` is zero (pause row live: 57 units
    /// wide, `+0x44` 0, a stale builder `+0x3C` of 3.9), else on half of `+0x44` plus page `+0x120`; other alignments keep
    /// `+0x3C`.
    #[test]
    fn centred_labels_recompute_the_offset_like_0x76150() {
        let mut m = fabricated();
        let rec = PAGE_TABLE + 3 * PAGE_STRIDE;
        let b = 0x3_0200;
        m.0.insert(b + f::PARENT, rec);
        m.0.insert(b + f::TEXT_ALIGN, 2);
        m.f(b + f::TEXT_X, 4.0);
        m.f(b + f::GEOM_W, 57.0);
        m.f(rec + f::PAGE_LABEL_CENTRE, 11.0);
        assert_eq!(label_x(&m, b), 28.5);
        m.f(b + f::TEXT_WIDTH, 30.0);
        assert_eq!(label_x(&m, b), 26.0);
        m.0.insert(b + f::TEXT_ALIGN, 0);
        assert_eq!(label_x(&m, b), 4.0);
    }

    /// T2315: the Story list numbers measured live (TSFP_UI_ROW_GEOM, docs/t2315-ui-page-completion.md item 4): page x
    /// 80, top 22.025, view top 2.2, units 6.4 / 4.8; the fifth level row (y 39, 73 x 19 units) is cut by the view at
    /// 389.72 and must keep its full geometry, a row of an unscrolled page keeps its box.
    #[test]
    fn scrolled_rows_keep_their_geometry_and_the_cut_box_as_clip() {
        let mut m = fabricated();
        let rec = PAGE_TABLE + 3 * PAGE_STRIDE;
        m.f(f::UNIT_X, 6.4);
        m.f(f::UNIT_Y, 4.8);
        m.f(rec + f::PAGE_X, 80.0);
        m.f(rec + f::PARENT_TOP, 22.025);
        m.f(rec + f::PARENT_VIEW_TOP, 2.2);
        let b = 0x3_0200;
        m.0.insert(b + row::FLAGS, 0xC_0041);
        m.0.insert(b + f::PARENT, rec);
        for (field, v) in [
            (f::GEOM_X, 1.0),
            (f::GEOM_Y, 39.0),
            (f::GEOM_W, 73.0),
            (f::GEOM_H, 19.0),
        ] {
            m.f(b + field, v);
        }
        for (i, v) in [86.4, 303.48, 553.6, 389.72].into_iter().enumerate() {
            m.f(b + f::BOX_LEFT + 4 * i as u32, v);
        }
        let live = read_page(&m, &TextIndex::default(), 3).unwrap();
        let [x0, y0, x1, y1] = live.rows[1].rect.unwrap();
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        assert!(
            near(x0, 86.4) && near(y0, 303.48) && near(x1, 553.6) && near(y1, 394.68),
            "{:?}",
            live.rows[1].rect
        );
        assert_eq!(live.rows[1].clip, Some([86.4, 303.48, 553.6, 389.72]));
        m.f(rec + f::PARENT_SCROLL, 10.0);
        let scrolled = row_geometry(&m, b).unwrap();
        assert!(
            near(scrolled[1], 303.48 - 48.0),
            "scroll offset moves the row up: {scrolled:?}"
        );
        assert_eq!(live.rows[0].clip, None);
        assert_eq!(live.rows[0].rect, Some([176.0, 100.0, 464.0, 130.0]));
    }

    /// T2315 3b: the Player Progress first row as measured live (value box 1, 2.2, 73 units, `+0xDC` 71, `+0xE0` 0.65,
    /// `+0xC0` 1, `+0xBC` 4, `[0x7BA18C]` 0.15625, page x 80, top 22.93839) with a FABRICATED style height of 20.
    #[test]
    fn value_rows_place_the_option_text_like_0x7b7b0() {
        let mut m = fabricated();
        let rec = PAGE_TABLE + 3 * PAGE_STRIDE;
        m.f(f::UNIT_X, 6.4);
        m.f(f::UNIT_Y, 4.8);
        m.f(f::VALUE_X_SCALE, 0.15625);
        m.f(rec + f::PAGE_X, 80.0);
        m.f(rec + f::PARENT_TOP, 22.93839);
        let style = f::STYLES + f::STYLE_STRIDE * 4;
        m.0.insert(style, 2);
        m.f(style + 0xC, 1.0);
        m.f(f::RECORD_HEIGHTS + 2 * f::RECORD_STRIDE, 20.0);
        m.f(f::HEIGHT_SCALE_A, 1.0);
        m.0.insert(f::HEIGHT_LINES, 448);
        m.f(f::HEIGHT_SCALE_B, 1.0 / 448.0);
        m.f(f::HEIGHT_SCALE_C, 1.0);
        let b = 0x3_0200;
        m.0.insert(b + row::FLAGS, 0xE_2604);
        for (field, v) in [
            (row::KIND, 1),
            (row::KIND_FLAGS, 0x42_2600),
            (f::VALUE_FONT, 4),
            (f::ALIGN, 1),
            (f::VALUE_COUNT, 1),
            (f::PARENT, rec),
        ] {
            m.0.insert(b + field, v);
        }
        for (field, v) in [
            (f::VALUE_BOX, 1.0),
            (f::VALUE_BOX + 4, 2.2),
            (f::VALUE_BOX + 8, 73.0),
            (f::VALUE_DX, 71.0),
            (f::VALUE_DY, 0.65),
        ] {
            m.f(b + field, v);
        }
        // One option with runtime text "P1".
        m.0.insert(b + row::OPTIONS, 0x5_0000);
        m.0.insert(0x5_0000, 6);
        m.0.insert(0x5_0000 + f::OPTION_TEXT, 0x4_0100);
        m.0.insert(0x4_0100, u32::from_le_bytes(*b"P1\0\0"));
        let value = read_value(&m, b).expect("a kind-1 value row");
        let near = |a: f32, e: f32| (a - e).abs() < 0.01;
        assert!(
            near(value.x, 540.8 - 20.0) && near(value.y, 123.784),
            "{value:?}"
        );
        assert_eq!((value.align, value.arrows), (Align::End, false));
        let live = read_page(&m, &TextIndex::default(), 3).unwrap();
        let (page, dynamic) = live.to_model_page(PageId(7));
        let anchor = page.widgets[1].value_anchor.expect("anchored value");
        assert!(
            near(anchor.x, 520.8 - 176.0) && near(anchor.y, 123.784 - 140.0),
            "{anchor:?}"
        );
        assert_eq!(
            dynamic.get(&widget_id(1)).map(Vec::as_slice),
            Some(&b"Ab"[..]),
            "a runtime label keeps its binding"
        );
        m.0.insert(b + row::KIND, 2);
        assert_eq!(
            read_value(&m, b),
            None,
            "only the option list kind is placed"
        );
    }

    #[test]
    fn reads_rows_texts_and_cursor_from_the_record() {
        let m = fabricated();
        let mut texts = TextIndex::default();
        texts.refresh(&m);
        assert_eq!(texts.id(0x2_0000), Some(5));
        let live = read_page(&m, &texts, 3).expect("live");
        assert_eq!(live.builder, 0x2D_08C0);
        assert_eq!(live.rows.len(), 2);
        assert_eq!(live.rows[0].text, LiveText::Id(5));
        assert_eq!(live.rows[1].text, LiveText::Bytes(b"Ab".to_vec()));
        assert_eq!(live.rows[0].rect, Some([176.0, 100.0, 464.0, 130.0]));
        assert_eq!(live.focused(), Some(widget_id(1)));
        assert!(read_page(&m, &texts, 4).is_none(), "empty slot");
    }

    /// T3781: the cursor rows keep a greyed (disabled in the model) row at its guest position, in cursor order.
    #[test]
    fn cursor_widgets_keep_greyed_rows_in_guest_order() {
        let mut m = fabricated();
        m.0.insert(0x3_0000 + row::FLAGS, row::FLAG_GREYED);
        let mut texts = TextIndex::default();
        texts.refresh(&m);
        let live = read_page(&m, &texts, 3).unwrap();
        let (p, _) = live.to_model_page(PageId(7));
        assert!(
            !p.widgets[0].enabled,
            "the greyed row is not focusable in the model"
        );
        assert!(
            p.widgets[0].is_focusable(),
            "T3811: hover still rests on it, so a press reaches the guest"
        );
        assert_eq!(
            live.cursor_widgets(&[0x3_0200, 0x3_0000]),
            [widget_id(1), widget_id(0)]
        );
        assert_eq!(
            live.cursor_widgets(&[0x3_0000, 0x9_9999, 0x3_0200]),
            [widget_id(0), widget_id(1)],
            "a node without a row is skipped"
        );
    }

    #[test]
    fn model_page_matches_the_dump_loader_mapping() {
        let m = fabricated();
        let mut texts = TextIndex::default();
        texts.refresh(&m);
        let live = read_page(&m, &texts, 3).unwrap();
        let (p, dynamic) = live.to_model_page(PageId(7));
        assert_eq!(p.widgets.len(), 2);
        assert_eq!(p.widgets[0].kind, WidgetKind::Button);
        assert_eq!(p.widgets[0].text, TextRef::Id(TextId(5)));
        assert_eq!(p.widgets[0].font_index, 4);
        assert!(p.widgets[0].navigable && !p.widgets[0].uppercase);
        assert_eq!(p.widgets[1].kind, WidgetKind::Label);
        assert_eq!(p.widgets[1].text, TextRef::Dynamic);
        assert!(p.widgets[1].uppercase && !p.widgets[1].navigable);
        assert_eq!(
            dynamic.get(&widget_id(1)).map(Vec::as_slice),
            Some(&b"Ab"[..])
        );
        // Same rect as the dump loader builds from the layout box.
        assert_eq!(
            p.widgets[0].rect,
            menu_rect(Vec2::new(176.0, 100.0), Vec2::new(288.0, 30.0))
        );
    }

    #[test]
    fn a_looping_row_chain_ends() {
        let mut m = fabricated();
        m.0.insert(0x3_0200 + row::NEXT, 0x3_0000);
        let texts = TextIndex::default();
        assert_eq!(read_page(&m, &texts, 3).unwrap().rows.len(), 2);
    }
}
