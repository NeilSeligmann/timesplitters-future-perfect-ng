//! Layout pass of the original menu toolkit as readable Rust (T2189 slice 2, UI-6).
//!
//! The original computes row rectangles in three stages, all in a 100 x 100 percent grid that
//! is mapped to the screen by one scale pair (`Grid`):
//!
//! 1. **Flow** (original 0x750F0): every pending row gets its start, width, height and a cursor
//!    position inside its page (`flow_rows`). Heights come from the row's own value plus the
//!    measured text height (`TextMeasure`), the page cursor advances by the row height.
//! 2. **Placement** (0x73CC0, 0x74580, 0x771D0): the page is centred in the screen
//!    (`PageSpec::place`), its pixel origin and size follow from the grid (`PageRect`).
//! 3. **Screen rectangles** (0x71F30): each row becomes a pixel rectangle
//!    `left, top, right, bottom` clipped to the page clip box and the viewport, with a hidden
//!    flag for empty rectangles (`screen_rects`). These are the cells +0x78..+0x84 of the dump.
//!
//! Every formula keeps the original `f32` operation order, so equal inputs give equal bits.
//! Evidence class: Tier 1 against the dump (`tests/layout_real.rs`), see docs/t-ui-model.md.
//! No fonts are needed: text heights and widths come from a `TextMeasure`, so the pass works with
//! synthetic tables and with private runtime tables alike.

use crate::model::{Page, TextRef, menu_rect};
use std::collections::BTreeMap;
use tsfp_ui_renderer::Vec2;

/// Mapping from the 100 x 100 percent grid to pixels (original 0x80EA0, viewport scale cells).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub origin_x: f32,
    pub origin_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

impl Grid {
    /// The 640 x 480 design space: 6.4 and 4.8 pixels per grid unit (as the original computes them, `size * 0.01` in `f32`). Widescreen does not change
    /// it: the menus are laid out in the 4:3 safe area and anchored by the renderer.
    pub const DESIGN: Grid = Grid::from_viewport(0.0, 0.0, 640.0, 480.0);
    /// 0x80EA0 `(x0, y0, x1, y1)`: scale = size * 0.01, origin = top left.
    pub const fn from_viewport(x0: f32, y0: f32, x1: f32, y1: f32) -> Grid {
        Grid {
            origin_x: x0,
            origin_y: y0,
            scale_x: (x1 - x0) * 0.01,
            scale_y: (y1 - y0) * 0.01,
        }
    }
}

/// Source of text sizes. Implemented by synthetic tables, by tables read at run time from the
/// owner's image, or by a calibration table. All results are in grid units (percent of the
/// screen height for heights).
pub trait TextMeasure {
    /// Height of one line of text in `font` (original 0x744F0 for a single line).
    fn line_height(&self, font: u32) -> f32;
    /// Number of lines `text` takes in a box `max_width` grid units wide. Default: one line.
    fn lines(&self, _font: u32, _text: TextRef, _max_width: f32) -> u32 {
        1
    }
    /// Width of `text` in grid units (used by alignment, not by the row flow). Default: unknown.
    fn text_width(&self, _font: u32, _text: TextRef) -> f32 {
        0.0
    }
}

/// Horizontal placement of text inside a row box (INFERRED mapping of the original align modes,
/// which are not yet decoded: the builders store a small integer, modes 3 to 5 are OPEN).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Left edge of text `text_width` wide inside a box `[start, start + width]` (grid units). Text
/// wider than the box starts at the box edge (it is clipped, not shifted).
pub fn align_offset(align: Align, start: f32, width: f32, text_width: f32) -> f32 {
    let free = (width - text_width).max(0.0);
    match align {
        Align::Left => start,
        Align::Center => start + free * 0.5,
        Align::Right => start + free,
    }
}

/// Value column of a value row (label left, value right): the value starts after the label column
/// `label_share` of the row width, aligned inside the rest. Returns (label_x, value_x).
pub fn value_columns(
    start: f32,
    width: f32,
    label_share: f32,
    value_align: Align,
    value_width: f32,
) -> (f32, f32) {
    let label_width = width * label_share.clamp(0.0, 1.0);
    let value_start = start + label_width;
    (
        start,
        align_offset(value_align, value_start, width - label_width, value_width),
    )
}

/// Fixed line heights per font index (grid units), e.g. calibrated from a capture.
#[derive(Clone, Debug, Default)]
pub struct TableMeasure {
    pub line_heights: BTreeMap<u32, f32>,
    /// Used for fonts missing in `line_heights`.
    pub fallback: f32,
}
impl TextMeasure for TableMeasure {
    fn line_height(&self, font: u32) -> f32 {
        self.line_heights
            .get(&font)
            .copied()
            .unwrap_or(self.fallback)
    }
}

/// Behaviour bits of a row during the flow (original row flag bytes, see docs/t-ui-model.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlowFlags {
    /// Row flag 0x400: add the measured text height to the row's own height (pad).
    pub measure_text: bool,
    /// Row flag 0x40: the row lives in the scrolling region. It uses the second page cursor
    /// (page cell 0x5C, starting at 0) instead of the main cursor (cell 0x68) and is clipped to
    /// the scroll window (`ScrollWindow`).
    pub scrolling: bool,
    /// Row cell 0xB0 non-zero: the page cursor advances by the row height and the line ends.
    /// A row without it shares its line with the next row, which starts one row width further
    /// right (horizontal strips, T2189 slice 6, fits all 536 captured rows).
    pub advance: bool,
    /// Value row (align mode 3, 4 or 5 with a kind): height adds the value text height.
    pub value_row: bool,
    /// Container row (node type 5): later `nested` rows flow inside it and it grows to hold
    /// them (nested scroll grids such as captured page 0x52ddfc, T2189 slice 8).
    pub container: bool,
    /// Row flag 0x20000: the row is a child of the last container row. Its y comes from the
    /// container's own cursor, which starts one inset (child start minus container start) below
    /// the container top, and the container height is inset + children + inset (MEASURED fit,
    /// 0x52ddfc). Without an open container the row flows like any other row.
    pub nested: bool,
}

/// Inputs of one row before the flow (the cells the builder leaves in the row).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSpec {
    pub font: u32,
    pub text: TextRef,
    pub flags: FlowFlags,
    /// Cell 0x5C: left edge inside the page.
    pub start: f32,
    /// Cell 0x60: right edge inside the page (used when `width` is 0).
    pub end: f32,
    /// Cell 0x6C: explicit width, 0 derives it from `end - start`.
    pub width: f32,
    /// Cell 0x70 before the flow (normally 0).
    pub height: f32,
    /// Cell 0x74 before the text is added: the row padding or fixed height.
    pub pad: f32,
    /// Cell 0xF0: content height of the row kind (0x801C0), 0 for plain text rows.
    pub content: f32,
}

impl RowSpec {
    pub fn text_row(font: u32, text: TextRef, start: f32, end: f32, pad: f32) -> Self {
        Self {
            font,
            text,
            flags: FlowFlags {
                measure_text: true,
                advance: true,
                ..FlowFlags::default()
            },
            start,
            end,
            width: 0.0,
            height: 0.0,
            pad,
            content: 0.0,
        }
    }
    /// A row with a fixed height (divider, spacer): no text measure.
    pub fn fixed(start: f32, end: f32, height: f32) -> Self {
        Self {
            font: 0,
            text: TextRef::None,
            flags: FlowFlags {
                advance: true,
                ..FlowFlags::default()
            },
            start,
            end,
            width: 0.0,
            height: 0.0,
            pad: height,
            content: 0.0,
        }
    }
}

/// Row cells after the flow (grid units, relative to the page).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowCells {
    /// Cell 0x64.
    pub x: f32,
    /// Cell 0x68.
    pub y: f32,
    /// Cell 0x6C.
    pub width: f32,
    /// Cell 0x70.
    pub height: f32,
    /// Cell 0x74: pad plus text height.
    pub inner_height: f32,
}

/// Page geometry inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSpec {
    /// Page width in grid units (the "title size" argument of the begin-page call, cell 0x50).
    pub width: f32,
    /// First row position inside the page (cursor start, 2.2 on the front end pages).
    pub top: f32,
    /// Space below the last row (the context pad, 2.0 on the front end pages).
    pub bottom: f32,
    /// Page height when the page does not grow with its rows (scroll lists: window height + pad).
    pub fixed_height: Option<f32>,
    /// Bottom of the scroll window inside the page (page cell 0x70). When set, the first
    /// scrolling row moves the main cursor to it, so fixed rows after the list sit below the
    /// window and the page height is their end plus `bottom` (MEASURED fit on every captured
    /// scroll page, T2189 slice 8).
    pub window_end: Option<f32>,
    /// Vertical placement span `[start, end]`: the page is centred between them. MEASURED fit
    /// of the front end pages: start 6.5 (title height), end 98.75, so the centre sits 2.625
    /// below the screen centre. The original reads both from the build context (0x74580).
    pub span_start: f32,
    pub span_end: f32,
    /// Explicit page position (grid units) set by the builder instead of the centred placement
    /// (left anchored popups such as captured page 0x524a10, INFERRED from the dump).
    pub anchor: Option<(f32, f32)>,
}

impl PageSpec {
    /// Front end defaults (menu pages of the dump) for a page of `width`.
    pub fn front_end(width: f32) -> Self {
        Self {
            width,
            top: 2.2,
            bottom: 2.0,
            fixed_height: None,
            window_end: None,
            span_start: 6.5,
            span_end: 98.75,
            anchor: None,
        }
    }
    /// Page position in grid units: x centred in 0..100, y centred in the placement span
    /// (`(end - start - height) * 0.5 + start`, original 0x74580).
    pub fn place(&self, height: f32) -> (f32, f32) {
        if let Some(anchor) = self.anchor {
            return anchor;
        }
        let x = (100.0 - self.width) * 0.5;
        let y = (self.span_end - self.span_start - height) * 0.5 + self.span_start;
        (x, y)
    }
}

/// Result of the flow stage.
#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    pub rows: Vec<RowCells>,
    /// Page cell 0x68: top + all advances + bottom pad (or the fixed height).
    pub height: f32,
    /// Page cell 0x5C: total height of the scrolling rows.
    pub scroll_extent: f32,
}

/// Original 0x750F0 for plain rows: widths, heights and cursor positions.
pub fn flow_rows(spec: &PageSpec, rows: &[RowSpec], measure: &dyn TextMeasure) -> Flow {
    let mut cursor = spec.top;
    let mut scroll_cursor = 0.0f32;
    let mut line_offset = 0.0f32;
    let mut out: Vec<RowCells> = Vec::with_capacity(rows.len());
    // Open container: row index and child cursor (None before the first child).
    let mut open: Option<(usize, Option<f32>)> = None;
    for row in rows {
        let width = if row.width == 0.0 {
            row.end - row.start
        } else {
            row.width
        };
        let text_height = if row.flags.measure_text && row.text != TextRef::None {
            let lines = measure.lines(row.font, row.text, width).max(1);
            measure.line_height(row.font) * lines as f32
        } else {
            0.0
        };
        let inner = row.pad + text_height;
        let height = if row.flags.value_row {
            row.height + row.content + inner
        } else {
            row.height + inner.max(row.content)
        };
        if row.flags.nested {
            if let Some((index, child)) = open.as_mut() {
                let inset = row.start - rows[*index].start;
                let child_y = child.get_or_insert(out[*index].y + inset);
                let y = *child_y;
                let x = row.start + line_offset;
                if row.flags.advance {
                    line_offset = 0.0;
                    *child_y += height;
                } else {
                    line_offset += width;
                }
                out.push(RowCells {
                    x,
                    y,
                    width,
                    height,
                    inner_height: inner,
                });
                continue;
            }
        }
        close_container(&mut open, &mut out, rows, &mut cursor, &mut scroll_cursor);
        if row.flags.scrolling {
            if let Some(end) = spec.window_end {
                cursor = cursor.max(end);
            }
        }
        let y = if row.flags.scrolling {
            scroll_cursor
        } else {
            cursor
        };
        let x = row.start + line_offset;
        if row.flags.advance {
            line_offset = 0.0;
            if row.flags.scrolling {
                scroll_cursor += height;
            } else {
                cursor += height;
            }
        } else {
            line_offset += width;
        }
        if row.flags.container {
            open = Some((out.len(), None));
        }
        out.push(RowCells {
            x,
            y,
            width,
            height,
            inner_height: inner,
        });
    }
    close_container(&mut open, &mut out, rows, &mut cursor, &mut scroll_cursor);
    Flow {
        rows: out,
        height: spec.fixed_height.unwrap_or(cursor + spec.bottom),
        scroll_extent: scroll_cursor,
    }
}

/// Ends an open container: when it got children it grows to inset + children + inset and the
/// cursor it advanced moves by the growth.
fn close_container(
    open: &mut Option<(usize, Option<f32>)>,
    out: &mut [RowCells],
    rows: &[RowSpec],
    cursor: &mut f32,
    scroll_cursor: &mut f32,
) {
    let Some((index, Some(child_end))) = open.take() else {
        return;
    };
    let first_child = &rows[index + 1];
    let inset = first_child.start - rows[index].start;
    let cell = &mut out[index];
    let grown = child_end - cell.y + inset;
    let growth = grown - cell.height;
    cell.height = grown;
    if rows[index].flags.advance {
        if rows[index].flags.scrolling {
            *scroll_cursor += growth;
        } else {
            *cursor += growth;
        }
    }
}

/// A pixel rectangle (the dump cells +0x78 left, +0x7C top, +0x80 right, +0x84 bottom).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl ScreenRect {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

/// Page position and size in pixels (original 0x73CC0 cells 0xA8, 0xAC, 0xB0, 0xB4) and the clip box
/// (0xE4, 0xE8, 0xEC, 0xF0, computed by 0x71F30 from the page and the viewport).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PageRect {
    /// 0x73CC0 for a page at `(x, y)` grid units of size `(width, height)`.
    pub fn from_grid(grid: &Grid, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: grid.scale_x * x + grid.origin_x,
            y: y * grid.scale_y + grid.origin_y,
            width: width * grid.scale_x,
            height: height * grid.scale_y,
        }
    }
    /// Clip box: the page rectangle intersected with the viewport `[left, right] x [top, bottom]`
    /// (first part of 0x71F30). `None` when it is at most one pixel wide or high: then every
    /// row is hidden.
    pub fn clip(&self, viewport: ScreenRect) -> Option<ScreenRect> {
        let left = viewport.left.max(self.x);
        let top = viewport.top.max(self.y);
        let right = viewport.right.min(self.width + self.x);
        let bottom = viewport.bottom.min(self.height + self.y);
        (right > left + 1.0 && bottom > top + 1.0).then_some(ScreenRect {
            left,
            top,
            right,
            bottom,
        })
    }
}

/// Scroll window cells of a page (original page cells 0x6C, 0x84, 0xA0, 0xA4, 0x70, all zero on
/// pages that do not scroll).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollWindow {
    /// Cell 0x6C: top of the window inside the page.
    pub top: f32,
    /// Cell 0x84: current scroll position.
    pub offset: f32,
    /// Cell 0xA0: window top used for the pixel clip.
    pub clip_top: f32,
    /// Cell 0xA4: window start used for the bottom clip.
    pub clip_bottom: f32,
    /// Cell 0x70: window height.
    pub height: f32,
}

/// Per row facts the rectangle stage needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RectFlags {
    /// Node type 9 (divider): looser visibility rule.
    pub divider: bool,
    /// Row flag 0x40: clipped to the scroll window.
    pub scrolling: bool,
}

/// Screen rectangle of one row with its visibility (cell flag 0x20 set when hidden).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowRect {
    pub rect: ScreenRect,
    pub hidden: bool,
}

/// Original 0x71F30 child loop: pixel rectangles of `cells` for a page whose top edge is at grid
/// position `page_y`. `flags[i]` selects the divider visibility rule and the scroll window.
pub fn screen_rects(
    grid: &Grid,
    page: &PageRect,
    page_y: f32,
    clip: Option<ScreenRect>,
    cells: &[RowCells],
    flags: &[RectFlags],
    window: &ScrollWindow,
) -> Vec<RowRect> {
    let Some(clip) = clip else {
        return cells
            .iter()
            .map(|_| RowRect {
                rect: ScreenRect::default(),
                hidden: true,
            })
            .collect();
    };
    cells
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let flag = flags.get(i).copied().unwrap_or_default();
            let left_px = c.x * grid.scale_x + page.x;
            let mut grid_y = page_y + c.y;
            if flag.scrolling {
                grid_y += window.top - window.offset;
            }
            let top_px = grid.scale_y * grid_y + grid.origin_y;
            let right_px = c.width * grid.scale_x + left_px;
            let bottom_px = c.height * grid.scale_y + top_px;
            let left = left_px.max(clip.left);
            let mut top = top_px.max(clip.top);
            let right = right_px.min(clip.right);
            let mut bottom = bottom_px.min(clip.bottom);
            if flag.scrolling {
                // Whole pixel window of the scroll region (truncating conversions as the original).
                let window_top =
                    ((window.top - window.clip_top) * grid.scale_y) as i32 as f32 + page.y + 1.0;
                let window_bottom =
                    ((window.clip_bottom + window.height) * grid.scale_y) as i32 as f32 + page.y;
                if window_top > top {
                    top = window_top;
                }
                if bottom > window_bottom {
                    bottom = window_bottom;
                }
            }
            let hidden = if flag.divider {
                !(right > left - 1.0 && bottom > top - 1.0)
            } else {
                !(right > left + 1.0 && bottom > top + 1.0)
            };
            RowRect {
                rect: ScreenRect {
                    left,
                    top,
                    right,
                    bottom,
                },
                hidden,
            }
        })
        .collect()
}

/// Whole page: flow, placement, pixel rectangles. `viewport` is the pixel box of the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct PageLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub flow: Flow,
    pub rects: Vec<RowRect>,
}

pub fn layout_page(
    grid: &Grid,
    viewport: ScreenRect,
    spec: &PageSpec,
    rows: &[RowSpec],
    flags: &[RectFlags],
    window: &ScrollWindow,
    measure: &dyn TextMeasure,
) -> PageLayout {
    let flow = flow_rows(spec, rows, measure);
    let (x, y) = spec.place(flow.height);
    let page = PageRect::from_grid(grid, x, y, spec.width, flow.height);
    let clip = page.clip(viewport);
    let rects = screen_rects(grid, &page, y, clip, &flow.rows, flags, window);
    PageLayout {
        x,
        y,
        width: spec.width,
        height: flow.height,
        flow,
        rects,
    }
}

/// Writes the rectangles of `layout` into the widgets of `page` (design space pixels, top-left
/// anchored in the 4:3 safe area). Hidden rows get a zero size.
pub fn apply_to_page(page: &mut Page, layout: &PageLayout) {
    for (widget, row) in page.widgets.iter_mut().zip(&layout.rects) {
        let r = row.rect;
        widget.rect = if row.hidden {
            menu_rect(Vec2::new(r.left, r.top), Vec2::new(0.0, 0.0))
        } else {
            menu_rect(Vec2::new(r.left, r.top), Vec2::new(r.width(), r.height()))
        };
    }
}

/// Apply a builder-produced layout only when it covers every retained widget.
/// Builder rows and layout rows come from separate stages; a partial zip would leave
/// old rectangles on trailing widgets and make those widgets clickable off screen.
pub fn apply_complete_to_page(page: &mut Page, layout: &PageLayout) -> Result<(), LayoutRowCount> {
    if page.widgets.len() != layout.rects.len() {
        return Err(LayoutRowCount {
            widgets: page.widgets.len(),
            rectangles: layout.rects.len(),
        });
    }
    apply_to_page(page, layout);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutRowCount {
    pub widgets: usize,
    pub rectangles: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{ModelRecorder, PageBuilder};
    use crate::model::{PageId, TextId, Widget, WidgetId, WidgetKind};

    fn measure() -> TableMeasure {
        TableMeasure {
            line_heights: BTreeMap::from([(4, 4.75)]),
            fallback: 3.0,
        }
    }

    #[test]
    fn rows_without_advance_form_a_horizontal_strip_and_anchor_overrides_placement() {
        // FABRICATED two-cell strip then one full row (T2189 slice 6 rule).
        let mut cell = RowSpec::fixed(1.0, 1.0, 3.0);
        cell.width = 4.0;
        cell.flags.advance = false;
        let mut last = cell;
        last.flags.advance = true;
        let rows = [cell, last, RowSpec::fixed(1.0, 9.0, 1.0)];
        let mut spec = PageSpec::front_end(20.0);
        let flow = flow_rows(&spec, &rows, &TableMeasure::default());
        let xs: Vec<f32> = flow.rows.iter().map(|c| c.x).collect();
        let ys: Vec<f32> = flow.rows.iter().map(|c| c.y).collect();
        assert_eq!(xs, [1.0, 5.0, 1.0]);
        assert_eq!(ys, [2.2, 2.2, 5.2]);
        assert_eq!(spec.place(10.0).0, 40.0);
        spec.anchor = Some((7.0, 8.0));
        assert_eq!(spec.place(10.0), (7.0, 8.0));
    }

    #[test]
    fn nested_rows_grow_their_container_and_fixed_rows_follow_the_window() {
        // FABRICATED scroll page: a container with a two-line child strip, then a fixed row
        // after a window ending at 30 (T2189 slice 8 rules).
        let mut container = RowSpec::fixed(4.0, 40.0, 0.0);
        container.flags.scrolling = true;
        container.flags.container = true;
        let mut child = RowSpec::fixed(7.0, 20.0, 12.0);
        child.flags.scrolling = true;
        child.flags.nested = true;
        child.width = 9.0;
        child.flags.advance = false;
        let mut line_end = child;
        line_end.flags.advance = true;
        let mut after = RowSpec::fixed(4.0, 40.0, 5.0);
        after.flags.scrolling = true;
        let rows = [
            container,
            child,
            line_end,
            child,
            line_end,
            after,
            RowSpec::fixed(1.0, 9.0, 4.0),
        ];
        let mut spec = PageSpec::front_end(50.0);
        spec.window_end = Some(30.0);
        let flow = flow_rows(&spec, &rows, &TableMeasure::default());
        let ys: Vec<f32> = flow.rows.iter().map(|c| c.y).collect();
        let xs: Vec<f32> = flow.rows.iter().map(|c| c.x).collect();
        assert_eq!(ys, [0.0, 3.0, 3.0, 15.0, 15.0, 30.0, 30.0]);
        assert_eq!(xs, [4.0, 7.0, 16.0, 7.0, 16.0, 4.0, 1.0]);
        assert_eq!(flow.rows[0].height, 30.0, "inset + two lines + inset");
        assert_eq!(flow.scroll_extent, 35.0);
        assert_eq!(
            flow.height, 36.0,
            "fixed row below the window plus the bottom pad"
        );
    }

    #[test]
    fn complete_layout_rejects_partial_builder_rows_without_mutation() {
        let mut recorder = ModelRecorder::new();
        let id = PageId(42);
        recorder.push_page(id, 1).unwrap();
        recorder
            .record_row(Widget::new(WidgetId(1), WidgetKind::Button))
            .unwrap();
        recorder
            .record_row(Widget::new(WidgetId(2), WidgetKind::Button))
            .unwrap();
        recorder.pop_page().unwrap();
        let mut model = recorder.finish().unwrap();
        let page = model.page_mut(id).unwrap();
        let original = page.widgets.iter().map(|w| w.rect).collect::<Vec<_>>();
        let layout = layout_page(
            &Grid::DESIGN,
            ScreenRect {
                left: 0.0,
                top: 0.0,
                right: 640.0,
                bottom: 480.0,
            },
            &PageSpec::front_end(60.0),
            &[RowSpec::fixed(0.0, 60.0, 4.0)],
            &[RectFlags::default()],
            &ScrollWindow::default(),
            &measure(),
        );
        assert_eq!(
            apply_complete_to_page(page, &layout),
            Err(LayoutRowCount {
                widgets: 2,
                rectangles: 1
            })
        );
        assert_eq!(
            page.widgets.iter().map(|w| w.rect).collect::<Vec<_>>(),
            original
        );
        page.widgets.pop();
        apply_complete_to_page(page, &layout).unwrap();
        assert!(page.widgets[0].rect.size.y > 0.0);
    }

    #[test]
    fn alignment_and_value_columns() {
        assert_eq!(align_offset(Align::Left, 10.0, 40.0, 10.0), 10.0);
        assert_eq!(align_offset(Align::Center, 10.0, 40.0, 10.0), 25.0);
        assert_eq!(align_offset(Align::Right, 10.0, 40.0, 10.0), 40.0);
        assert_eq!(
            align_offset(Align::Right, 10.0, 40.0, 50.0),
            10.0,
            "overflow stays at the edge"
        );
        let (label, value) = value_columns(10.0, 40.0, 0.5, Align::Right, 5.0);
        assert_eq!((label, value), (10.0, 45.0));
    }

    #[test]
    fn grid_matches_design_space() {
        let g = Grid::from_viewport(0.0, 0.0, 640.0, 480.0);
        assert!((g.scale_x - 6.4).abs() < 1e-5 && (g.scale_y - 4.8).abs() < 1e-5);
        assert_eq!(Grid::DESIGN, g);
    }

    #[test]
    fn rows_flow_down_the_page_and_the_page_is_centred() {
        let spec = PageSpec::front_end(47.0);
        let rows: Vec<RowSpec> = (0..8)
            .map(|i| RowSpec::text_row(4, TextRef::Id(TextId(i)), 1.0, 46.0, 2.5))
            .collect();
        let flow = flow_rows(&spec, &rows, &measure());
        assert_eq!(flow.rows[0].y, 2.2);
        assert_eq!(flow.rows[0].width, 45.0);
        assert_eq!(flow.rows[0].height, 7.25);
        assert!((flow.rows[1].y - 9.45).abs() < 1e-5);
        assert!((flow.height - (2.2 + 8.0 * 7.25 + 2.0)).abs() < 1e-4);
        let (x, y) = spec.place(flow.height);
        assert_eq!(x, 26.5);
        assert!((y - (98.75 - 6.5 - flow.height) * 0.5 - 6.5).abs() < 1e-5);
    }

    #[test]
    fn fixed_rows_and_hidden_clip() {
        let spec = PageSpec::front_end(40.0);
        let rows = [
            RowSpec::fixed(1.0, 39.0, 0.5),
            RowSpec::text_row(4, TextRef::Id(TextId(1)), 1.0, 39.0, 2.0),
        ];
        let layout = layout_page(
            &Grid::DESIGN,
            ScreenRect {
                left: 0.0,
                top: 0.0,
                right: 640.0,
                bottom: 480.0,
            },
            &spec,
            &rows,
            &[
                RectFlags {
                    divider: true,
                    scrolling: false,
                },
                RectFlags::default(),
            ],
            &ScrollWindow::default(),
            &measure(),
        );
        assert!(!layout.rects[0].hidden && !layout.rects[1].hidden);
        assert!(layout.rects[1].rect.top >= layout.rects[0].rect.bottom - 1e-3);
        // A viewport that does not intersect the page hides every row.
        let hidden = layout_page(
            &Grid::DESIGN,
            ScreenRect {
                left: 600.0,
                top: 0.0,
                right: 601.0,
                bottom: 480.0,
            },
            &spec,
            &rows,
            &[
                RectFlags {
                    divider: true,
                    scrolling: false,
                },
                RectFlags::default(),
            ],
            &ScrollWindow::default(),
            &measure(),
        );
        assert!(hidden.rects.iter().all(|r| r.hidden));
    }
}
