// SPDX-License-Identifier: GPL-3.0-or-later
//! T1916 increment 3: the row and widget draws of default-style pages (the Map Maker pages), drawn
//! by Rust after the focused-row highlight like the original's per-row draws in `0x76810`.
//!
//! Each [`WidgetDraw`] is one original draw, MEASURED by static reading of the lift
//! (docs/t-system-mapmaker-ui.md, "Increment 3"):
//!
//! * [`WidgetDraw::Divider`] `0x73110(page, row, rect)`: two horizontal lines (`0x1B090`, integer
//!   pixels) across the row at `trunc(y0 + h * [0x475CD4])` and one unit lower, in the colour
//!   block's `+0x3C` and `+0x40`, alpha scaled by the page fade (`0x154FE0`).
//! * [`WidgetDraw::StorageIcon`] `0x191470(page, row, rect)`: the storage device icon, texture
//!   `0x80F20(0x153)` (hard disk, slots 8 and up) or `0x154` / `0x155` (memory unit, by
//!   `0x24E60(slot)`), a [`STORAGE_ICON_SIZE`] square at `(x0 + 20, mid_y - 8)`, colour
//!   0x7F7F7F with alpha `ftol(fade * 127)`, additive (`0x19D70(2)`), uv over the whole texture.
//! * [`WidgetDraw::GroupBox`] `0x74030(page, row, [outer, inner])`: an etched frame of ten
//!   `0x1B090` lines on the midlines between the row box and the box inset by the style margins
//!   (`[page +0x24]`), the title (style 0x11) centred on the top line with a five-unit gap each side.
//! * [`WidgetDraw::CheckBox`] `0x7BB90(checked)` (kind 4 through `0x7F8B0`, and the radio rows
//!   `0x7BCD0`): the box `0x72560` and, when checked, a white fill inset by five units.
//! * [`WidgetDraw::Slider`] `0x7F5B0` (kind 7): end bars, the track line and the tick lines in two
//!   passes (offset one unit, colour block `+0x40`, then `+0x3C`), the thumb (white under the
//!   pointer, `0x711F0`, else `+0x3C`).
//! * [`WidgetDraw::DropDown`] `0x7F200` (kind 2), [`WidgetDraw::Button`] `0x7F3B0` (kind 5),
//!   [`WidgetDraw::Label`] `0x319E60`, [`WidgetDraw::LightCell`] `0x31AD90` and the page's
//!   [`ScrollBar`] `0x727F0` (docs/t-system-mapmaker-ui.md, increment 3c).
use crate::bridge::TextPlacement;
use crate::list_rows::{Edges, ListDraw, TextureLookup, alpha_byte};
use crate::model::{Align, TextRef, WidgetId};
use crate::skin::PackedColor;
use tsfp_ui_renderer::{BlendMode, Color, Rect, Sprite, TextureId, UvRect, Vec2, Viewport};

/// Storage icon placement of `0x191470`: x offset `[0x4782C4]`, y offset under the row centre
/// `[0x47D1E4]`, size `[0x475DC0]` (design units, MEASURED from the XBE: 20, 8, 16).
pub const STORAGE_ICON_X: f32 = 20.0;
pub const STORAGE_ICON_Y: f32 = 8.0;
pub const STORAGE_ICON_SIZE: f32 = 16.0;
/// Texture table entries of `0x191470`: hard disk, and the two memory unit pictures.
pub const STORAGE_HARD_DISK: u32 = 0x153;
pub const STORAGE_MEMORY_UNIT_A: u32 = 0x154;
pub const STORAGE_MEMORY_UNIT_B: u32 = 0x155;
/// Text style of the group box title (`0x74030`: `0x3FA10(0x11)`).
pub const GROUP_TITLE_STYLE: u32 = 0x11;
/// Gap left and right of the group box title (`[0x4760CC]`, design units).
pub const GROUP_TITLE_GAP: f32 = 5.0;
/// The darker etch colour of `0x74030`: `0x154C50(+0x3C, 0x0C08107F, 0.7)`, then `0x154FE0(.., 0.7)`.
pub const GROUP_ETCH_MIX: PackedColor = PackedColor(0x0C08_107F);
pub const GROUP_ETCH_T: f32 = 0.7;
/// Check box: the white fill of a checked box is inset by `[0x4760CC]` (5); the box frame `0x72560` brightens its
/// colours by `0x154E90(colour, 1.4)` and fills the inside, inset by `[0x475D10]` (2), with 0x0F0F0F.
pub const CHECK_INSET: f32 = 5.0;
pub const BOX_BRIGHTEN: f32 = 1.4;
pub const BOX_INSET: f32 = 2.0;
pub const BOX_FILL_RGB: u32 = 0x0F0F_0F00;
/// Slider (`0x7F0E0`): band height `[0x475DB4]` (4) y units, thumb width `[0x475D20]` (1.5) x units, the value is a
/// percent (`[0x475D1C]` 0.01), tick half length `[0x4760CC]` (5).
pub const SLIDER_BAND: f32 = 4.0;
pub const SLIDER_THUMB: f32 = 1.5;
pub const SLIDER_TICK: f32 = 5.0;
const WHITE_RGB: u32 = 0x7F7F_7F00;

/// The front end's unit globals: `[0x7BA038]` x and `[0x7BA188]` y unit, `[0x7BA180]` / `[0x7BA184]` origin.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Units {
    pub x: f32,
    pub y: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

impl Units {
    /// A box in units (`0x7B520` value boxes) in design units.
    pub fn to_design(&self, b: Edges) -> Edges {
        Edges::new(
            b.x0 * self.x + self.origin_x,
            b.y0 * self.y + self.origin_y,
            b.x1 * self.x + self.origin_x,
            b.y1 * self.y + self.origin_y,
        )
    }
}

/// A text a widget draws itself (the model's own text of that row is not drawn).
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetText {
    pub widget: WidgetId,
    pub text: TextRef,
    /// Runtime bytes of a `TextRef::Dynamic` text.
    pub bytes: Vec<u8>,
}

/// One original row or widget draw, in design units.
#[derive(Clone, Debug, PartialEq)]
pub enum WidgetDraw {
    /// `0x73110`: the divider row (row rect, colour block `+0x3C`, `+0x40`).
    Divider {
        row: Edges,
        upper: PackedColor,
        lower: PackedColor,
    },
    /// `0x191470`: the storage device icon of a load/save row (texture table name).
    StorageIcon { row: Edges, texture: String },
    /// `0x74030`: a group box (row type 5). `outer` is the row box, `inner` the box inset by the style margins,
    /// `title_right` row `+0x30 == 1`, `title_height` the height of style 0x11 (`0x3FCF0`); `light` and `dark` are the
    /// colour block's `+0x3C` and `+0x40`.
    GroupBox {
        outer: Edges,
        inner: Edges,
        title: Option<WidgetText>,
        title_right: bool,
        title_height: f32,
        light: PackedColor,
        dark: PackedColor,
    },
    /// `0x7BB90`: a check box in `area` (top-left anchored), `size` the box size in units
    /// (`[0x7BA030] * style height - 1`), colours `+0x84` (top) and `+0x88` (bottom) of the colour block.
    CheckBox {
        area: Edges,
        size: f32,
        checked: bool,
        top: PackedColor,
        bottom: PackedColor,
    },
    /// `0x7F5B0`: a slider. `band` is the value box (design units), `value` the percent (`+0xC8`), `ticks` the tick
    /// count (row word `+0x9A`), `hovered` the pointer over the thumb; `light` / `dark` the colour block's `+0x3C` /
    /// `+0x40`.
    Slider {
        band: Edges,
        value: f32,
        ticks: u32,
        hovered: bool,
        light: PackedColor,
        dark: PackedColor,
    },
    /// `0x7F200`: a drop-down (kind 2). `area` is the value box, `arrow` its right square (`0x7EEC0`), `glow` the
    /// gradient factor (`0x7BAA0`: colour block `+0x8C` on the cursor row, else 1), `top` / `bottom` the colour block's
    /// `+0x84` / `+0x88`; the arrow sprite `0x81BC0` (closed) and the selected option text (`0x7B620`).
    DropDown {
        area: Edges,
        arrow: Edges,
        glow: f32,
        top: PackedColor,
        bottom: PackedColor,
        arrow_sprite: Option<TexturePart>,
        text: Option<PlacedText>,
    },
    /// `0x7F3B0`: a push button (kind 5): the box `0x72560` over the value box and the row's text (`+0x48`).
    Button {
        area: Edges,
        top: PackedColor,
        bottom: PackedColor,
        text: Option<PlacedText>,
    },
    /// `0x319E60`: the Map Settings label row (string `+0x14` or 0xBC2, the page's style font, wrapped to the row width
    /// minus one x unit, the second font when the first is too tall), drawn at the row's top-left.
    Label {
        text: PlacedText,
        width: f32,
        max_height: f32,
        fallback: Option<(u32, f32)>,
    },
    /// `0x31AD90` with `0x315E50`: the tile light colour cell (INFERRED in part, see the doc): the cell fill, the
    /// nine-slice frame (`0x81EC0`, texture `[0x4D4564]`) and the bulb picture (`[0x7BA040 + 0x27 * 4]`) tinted with the
    /// light colour (`0x30A2D0`).
    /// `0x312610`: centred 70% inset with an eight-unit nine-slice border.
    InsetFrame {
        area: Edges,
        frame: Option<TexturePart>,
    },
    LightCell {
        area: Edges,
        state: u32,
        light: PackedColor,
        frame: Option<TexturePart>,
        bulb: Option<TexturePart>,
    },
}

/// A part of a named texture (texture table name, uv over the texture).
#[derive(Clone, Debug, PartialEq)]
pub struct TexturePart {
    pub texture: String,
    pub uv: UvRect,
}

impl TexturePart {
    /// The texels `(u0, v0)..(u1, v1)` in the 1/16 units the original passes (`0x1B7B0`, `0x1F090`) of a texture of
    /// `size` texels.
    pub fn sixteenths(
        texture: String,
        (u0, v0, u1, v1): (u32, u32, u32, u32),
        size: (f32, f32),
    ) -> Self {
        let (w, h) = (size.0.max(1.0) * 16.0, size.1.max(1.0) * 16.0);
        TexturePart {
            texture,
            uv: UvRect {
                min: Vec2 {
                    x: u0 as f32 / w,
                    y: v0 as f32 / h,
                },
                max: Vec2 {
                    x: u1 as f32 / w,
                    y: v1 as f32 / h,
                },
            },
        }
    }
}

/// A text a widget places itself: `at` is the `0x41DC0` / `0x41E30` position (design units), `align_end` puts the text's
/// end there; `focused` picks the focused row colour; `line_height` is the style's line advance.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedText {
    pub text: WidgetText,
    pub font: u32,
    pub at: (f32, f32),
    pub align_end: bool,
    pub focused: bool,
    pub line_height: f32,
}

/// The page scroll bar of `0x727F0` (page style word bit 0x40) with the rectangles of `0x71400`: `outer`, the thumb,
/// the up and down arrows; textures table entries 0x3F0 (track), 0x3F1 (up), 0x3F2 (down), 0x3F3 (thumb). Colours:
/// `top` / `bottom` the colour block's `+0x60` / `+0x68` (or `+0x64` / `+0x6C` for a right-side bar), brightened by
/// `+0x80` while hovered.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollBar {
    pub outer: Edges,
    pub thumb: Edges,
    pub up: Edges,
    pub down: Edges,
    pub textures: [Option<String>; 4],
    pub top: PackedColor,
    pub bottom: PackedColor,
    /// Thumb colours: the bar gradient at the thumb's top and bottom (`0x154D50`).
    pub thumb_top: PackedColor,
    pub thumb_bottom: PackedColor,
}

impl WidgetDraw {
    /// Texture table names this draw samples.
    pub fn texture_names(&self) -> Vec<&str> {
        match self {
            WidgetDraw::StorageIcon { texture, .. } => vec![texture.as_str()],
            WidgetDraw::DropDown {
                arrow_sprite: Some(part),
                ..
            } => vec![part.texture.as_str()],
            WidgetDraw::InsetFrame { frame, .. } => {
                frame.iter().map(|p| p.texture.as_str()).collect()
            }
            WidgetDraw::LightCell { frame, bulb, .. } => frame
                .iter()
                .chain(bulb.iter())
                .map(|p| p.texture.as_str())
                .collect(),
            _ => Vec::new(),
        }
    }
}

const FULL_UV: UvRect = UvRect {
    min: Vec2 { x: 0.0, y: 0.0 },
    max: Vec2 { x: 1.0, y: 1.0 },
};

/// A row text the original wraps: `0x76150` passes `row +0x44 * [0x7BA038]` as the width to `0x40F00` (0 = one
/// line), which breaks the text into lines drawn `line_height` apart (the selected style's line advance).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wrap {
    pub widget: WidgetId,
    pub width: f32,
    pub line_height: f32,
}

/// Widget ids of the extra lines of a wrapped text (`WRAP_ID_BASE + 64 * widget + line`).
pub const WRAP_ID_BASE: u64 = 0x5752_0000_0000;

/// The widget draws of one page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Widgets {
    /// T1916: complete editor Controls diagram, acquired on the guest thread.
    pub controls: Option<crate::mapmaker_controls::ControlsDiagram>,
    pub draws: Vec<WidgetDraw>,
    pub units: Units,
    pub wraps: Vec<Wrap>,
    /// The page's scroll view `(top, bottom)` in design units: the row widgets are cut to it (the original's row
    /// viewports `0x722B0`); the scroll bar is not.
    pub clip: Option<(f32, f32)>,
    pub scroll_bar: Option<ScrollBar>,
    /// Pixels of slot textures the draws use (not in the disc's named table), by key.
    pub images: Vec<crate::list_rows::GuestImage>,
}

/// Break `text` into lines no wider than `width` at spaces (greedy, INFERRED: `0x40F00` was not read in full; a word
/// wider than the line stays on its own line).
pub fn wrap_lines(text: &[u8], width: f32, measure: &dyn Fn(&[u8]) -> f32) -> Vec<Vec<u8>> {
    crate::text_widget::lines(text, width, false, measure)
}

/// Replace the placements of wrapped rows in `texts` by one placement per line (`TextRef::Dynamic`, the line bytes
/// pushed to `dynamic`), each `line_height` design units under the previous; `bytes_of` gives a placement's text
/// bytes, `measure` a width in design units for `(bytes, style)`, `unit` the window pixels per design unit.
pub fn split_wrapped(
    texts: &mut Vec<TextPlacement>,
    wraps: &[Wrap],
    bytes_of: &dyn Fn(&TextPlacement) -> Option<Vec<u8>>,
    measure: &dyn Fn(&[u8], u32) -> f32,
    unit: f32,
    dynamic: &mut Vec<(WidgetId, Vec<u8>)>,
) {
    let mut out = Vec::with_capacity(texts.len());
    for placement in texts.drain(..) {
        let wrap = wraps.iter().find(|w| w.widget == placement.widget);
        let (Some(wrap), Some(bytes)) = (wrap, bytes_of(&placement)) else {
            out.push(placement);
            continue;
        };
        let font = placement.font_index;
        let lines = wrap_lines(&bytes, wrap.width, &|b| measure(b, font));
        for (index, line) in lines.into_iter().enumerate() {
            let widget = WidgetId(WRAP_ID_BASE + 64 * placement.widget.0 + index as u64);
            let mut rect = placement.rect_px;
            rect.origin.y += wrap.line_height * index as f32 * unit;
            dynamic.push((widget, line));
            out.push(TextPlacement {
                widget,
                text: TextRef::Dynamic,
                rect_px: rect,
                ..placement.clone()
            });
        }
    }
    *texts = out;
}

impl Widgets {
    pub fn texture_names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let bar = self
            .scroll_bar
            .iter()
            .flat_map(|b| b.textures.iter().flatten().map(String::as_str));
        for name in self.draws.iter().flat_map(|d| d.texture_names()).chain(bar) {
            if !out.iter().any(|n| n == name) {
                out.push(name.to_string());
            }
        }
        if let Some(controls) = &self.controls {
            for name in controls
                .labels
                .iter()
                .flat_map(|r| [&r.label_icon, &r.value_icon])
                .flatten()
                .filter_map(|i| i.texture.as_ref())
            {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
            for picture in &controls.pictures.pictures {
                if !out.contains(&picture.texture) {
                    out.push(picture.texture.clone());
                }
            }
        }
        out
    }
}

/// Width of a text in design units as drawn: `(text, runtime bytes, style)`.
pub type TextWidth<'a> = dyn Fn(TextRef, &[u8], u32) -> f32 + 'a;

fn set_blend(out: &mut ListDraw, blend: BlendMode) {
    if out.blends.last().map(|(_, mode)| *mode) != Some(blend) {
        out.blends.push((out.sprites.len(), blend));
    }
}

fn push(
    out: &mut ListDraw,
    texture: TextureId,
    edges: Edges,
    viewport: Viewport,
    color: Color,
    blend: BlendMode,
) {
    let Some(rect_px) = edges.to_px(viewport) else {
        return;
    };
    set_blend(out, blend);
    out.sprites.push(Sprite {
        rect_px,
        uv: FULL_UV,
        color,
        texture,
    });
}

/// An integer line of `0x1B090` from `(x0, y0)` to `(x1, y1)` (design units, truncated like the original), horizontal or
/// vertical, one window pixel wide. MEASURED on the legacy picture at 1280 x 960 (the guest's 640 x 480 lines rasterised
/// at twice the size): a horizontal line at `y` lights pixel row `2y - 1` from `2 x0` up to `2 x1 - 3`, so the line sits
/// one pixel above the scaled coordinate and ends two pixels short (the D3D line rule; vertical lines INFERRED alike).
fn line(
    out: &mut ListDraw,
    white: TextureId,
    (x0, y0): (f32, f32),
    (x1, y1): (f32, f32),
    viewport: Viewport,
    color: Color,
) {
    let (x0, y0, x1, y1) = (x0.trunc(), y0.trunc(), x1.trunc(), y1.trunc());
    let (lo_x, hi_x, lo_y, hi_y) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
    let Some(span) = Edges::new(lo_x, lo_y, hi_x, hi_y).to_px(viewport) else {
        return;
    };
    let rect = if y0 == y1 {
        Rect::new(
            Vec2::new(span.origin.x, span.origin.y - 1.0),
            Vec2::new((span.size.x - 2.0).max(1.0), 1.0),
        )
    } else {
        Rect::new(
            Vec2::new(span.origin.x - 1.0, span.origin.y),
            Vec2::new(1.0, (span.size.y - 2.0).max(1.0)),
        )
    };
    let Ok(rect_px) = rect else { return };
    set_blend(out, BlendMode::Alpha);
    out.sprites.push(Sprite {
        rect_px,
        uv: FULL_UV,
        color,
        texture: white,
    });
}

/// An integer rectangle of `0x1EFB0` (corners truncated, any order).
fn fill_int(out: &mut ListDraw, white: TextureId, e: Edges, viewport: Viewport, color: Color) {
    let (x0, y0, x1, y1) = (e.x0.trunc(), e.y0.trunc(), e.x1.trunc(), e.y1.trunc());
    push(
        out,
        white,
        Edges::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)),
        viewport,
        color,
        BlendMode::Alpha,
    );
}

/// `0x154C50(a, b, t)`: each channel `a * (1 - t) + b * t` (alpha alike, INFERRED: the lift was read for the colour
/// channels).
pub fn mix(a: PackedColor, b: PackedColor, t: f32) -> PackedColor {
    let channel = |shift: u32| {
        let (x, y) = (
            ((a.0 >> shift) & 0xFF) as f32,
            ((b.0 >> shift) & 0xFF) as f32,
        );
        (((x * (1.0 - t) + y * t) as u32) & 0xFF) << shift
    };
    PackedColor(channel(24) | channel(16) | channel(8) | channel(0))
}

/// `0x154E90(colour, factor)`: the colour channels times `factor`, alpha kept.
pub fn brighten(c: PackedColor, factor: f32) -> PackedColor {
    let channel =
        |shift: u32| ((((c.0 >> shift) & 0xFF) as f32 * factor) as u32).min(0xFF) << shift;
    PackedColor(channel(24) | channel(16) | channel(8) | (c.0 & 0xFF))
}

struct GroupBox<'a> {
    outer: Edges,
    inner: Edges,
    title: &'a Option<WidgetText>,
    title_right: bool,
    title_height: f32,
    light: PackedColor,
    dark: PackedColor,
}

fn group_box(
    out: &mut ListDraw,
    white: TextureId,
    viewport: Viewport,
    fade: f32,
    width: &TextWidth<'_>,
    g: GroupBox<'_>,
) {
    let mid = |a: f32, b: f32| (b - a) * 0.5 + a;
    let (l, t) = (mid(g.outer.x0, g.inner.x0), mid(g.outer.y0, g.inner.y0));
    let (r, b) = (mid(g.inner.x1, g.outer.x1), mid(g.inner.y1, g.outer.y1));
    let w = g
        .title
        .as_ref()
        .map(|text| width(text.text, &text.bytes, GROUP_TITLE_STYLE))
        .unwrap_or(0.0);
    let title_x = if g.title_right {
        g.inner.x1 - w
    } else {
        g.inner.x0
    };
    let (gap_left, gap_right) = (title_x - GROUP_TITLE_GAP, title_x + w + GROUP_TITLE_GAP);
    let dark = g.dark.with_alpha_scaled(fade);
    // The light colour is faded first (`0x154FE0(+0x3C, fade)`), then mixed and scaled by 0.7.
    let faded = PackedColor((g.light.0 & !0xFF) | alpha_byte((g.light.0 & 0xFF) as f32 * fade));
    let etch = mix(faded, GROUP_ETCH_MIX, GROUP_ETCH_T).with_alpha_scaled(GROUP_ETCH_T);
    // `0x74030` order: the `+0x40` lines one unit in on the left and top (split at the title) and on the right and
    // bottom edges, then the etch lines on the left and top edges (split) and one unit in on the right and bottom.
    for (from, to, colour) in [
        ((l + 1.0, b - 1.0), (l + 1.0, t + 1.0), dark),
        ((l + 1.0, t + 1.0), (gap_left, t + 1.0), dark),
        ((gap_right, t + 1.0), (r - 1.0, t + 1.0), dark),
        ((r, t), (r, b), dark),
        ((r, b), (l, b), dark),
        ((l, b - 1.0), (l, t), etch),
        ((l, t), (gap_left, t), etch),
        ((gap_right, t), (r, t), etch),
        ((r - 1.0, t + 1.0), (r - 1.0, b - 1.0), etch),
        ((r - 1.0, b - 1.0), (l, b - 1.0), etch),
    ] {
        line(out, white, from, to, viewport, colour);
    }
    let Some(text) = g.title else { return };
    // `0x41E30(x, t - h / 2, text, +0x3C, +0x40)`, placed like the row texts (`skin::anchored_placement`).
    let y = t - g.title_height * 0.5 - crate::skin::ANCHOR_TEXT_LIFT;
    let Some(rect_px) = Edges::new(
        title_x,
        y,
        title_x + w.max(1.0),
        y + g.title_height.max(1.0),
    )
    .to_px(viewport) else {
        return;
    };
    out.texts.push(TextPlacement {
        widget: text.widget,
        text: text.text,
        font_index: GROUP_TITLE_STYLE,
        value_font_index: GROUP_TITLE_STYLE,
        align: Align::Start,
        rect_px,
        color: g.light.with_alpha_scaled(fade),
        value: None,
    });
    if text.text == TextRef::Dynamic {
        out.dynamic.push((text.widget, text.bytes.clone()));
    }
}

/// `0x72560(page, box)`: the box gradient (`+0x84` top, `+0x88` bottom, faded and brightened) and the dark inside.
fn box_frame(
    out: &mut ListDraw,
    white: TextureId,
    area: Edges,
    viewport: Viewport,
    fade: f32,
    top: PackedColor,
    bottom: PackedColor,
) {
    let Some(rect) = area.to_px(viewport) else {
        return;
    };
    let top = brighten(top, BOX_BRIGHTEN).with_alpha_scaled(fade);
    let bottom = brighten(bottom, BOX_BRIGHTEN).with_alpha_scaled(fade);
    set_blend(out, BlendMode::Alpha);
    crate::default_style::corner_gradient(
        &mut out.sprites,
        white,
        rect,
        [top, top, bottom, bottom],
    );
    let inside = Edges::new(
        area.x0 + BOX_INSET,
        area.y0 + BOX_INSET,
        area.x1 - BOX_INSET,
        area.y1 - BOX_INSET,
    );
    let alpha = alpha_byte(fade * 127.0);
    push(
        out,
        white,
        inside,
        viewport,
        PackedColor(BOX_FILL_RGB | alpha).to_color(),
        BlendMode::Alpha,
    );
}

/// `0x7BB90`: the check box, half a unit into `area`, `size` units square.
pub fn check_box_edges(area: Edges, size: f32, units: Units) -> Edges {
    let x0 = area.x0 + units.x * 0.5;
    let y0 = area.y0 + units.y * 0.5;
    Edges::new(x0, y0, x0 + units.x * size, y0 + units.y * size)
}

/// `0x7F0E0`: the slider band (half a y unit into the value box, [`SLIDER_BAND`] units tall, one unit in on each
/// side) and the thumb (centred on the value between the band ends moved in by half its width).
pub fn slider_edges(value_box: Edges, value: f32, units: Units) -> (Edges, Edges) {
    let top = value_box.y0 + units.y * 0.5;
    let band = Edges::new(
        value_box.x0 + 1.0,
        top,
        value_box.x1 - 1.0,
        top + units.y * SLIDER_BAND,
    );
    let half = (units.x * SLIDER_THUMB + units.origin_x) * 0.5;
    let (lo, hi) = (band.x0 + half, band.x1 - half);
    let at = lo + (hi - lo) * (value * 0.01);
    (band, Edges::new(at - half, band.y0, at + half, band.y1))
}

/// The default style's row text colours (`default_style::TEXT_*`), alpha times the fade.
fn text_colour(focused: bool, fade: f32) -> Color {
    let mut c = if focused {
        crate::default_style::TEXT_FOCUSED
    } else {
        crate::default_style::TEXT_NORMAL
    };
    c.0[3] *= fade;
    c
}

/// Ids of the texts the widgets place (`WIDGET_TEXT_BASE + 64 * row widget + line`).
pub const WIDGET_TEXT_BASE: u64 = 0x5754_0000_0000;

/// A text placement at the `0x41DC0` position `at` (the row text rule of `skin::anchored_placement`); `bytes` makes it
/// a runtime text under `id`.
fn place_text(
    out: &mut ListDraw,
    p: &PlacedText,
    bytes: Option<Vec<u8>>,
    width: f32,
    viewport: Viewport,
    fade: f32,
    id: WidgetId,
) {
    let (x, y) = (p.at.0, p.at.1 - crate::skin::ANCHOR_TEXT_LIFT);
    let (w, h) = (width.max(1.0), p.line_height.max(1.0));
    let edges = if p.align_end {
        Edges::new(x - w, y, x, y + h)
    } else {
        Edges::new(x, y, x + w, y + h)
    };
    let Some(rect_px) = edges.to_px(viewport) else {
        return;
    };
    let text = match bytes {
        Some(b) => {
            out.dynamic.push((id, b));
            TextRef::Dynamic
        }
        None => p.text.text,
    };
    out.texts.push(TextPlacement {
        widget: id,
        text,
        font_index: p.font,
        value_font_index: p.font,
        align: if p.align_end {
            Align::End
        } else {
            Align::Start
        },
        rect_px,
        color: text_colour(p.focused, fade),
        value: None,
    });
}

fn textured(
    out: &mut ListDraw,
    texture: Option<TextureId>,
    uv: UvRect,
    e: Edges,
    viewport: Viewport,
    color: Color,
    blend: BlendMode,
) {
    let (Some(texture), Some(rect_px)) = (texture, e.to_px(viewport)) else {
        return;
    };
    set_blend(out, blend);
    out.sprites.push(Sprite {
        rect_px,
        uv,
        color,
        texture,
    });
}

/// A textured quad with a top and a bottom colour (`0x1F7B0`, c1 = c2 top, c3 = c4 bottom, INFERRED order), as slices.
fn textured_gradient(
    out: &mut ListDraw,
    texture: Option<TextureId>,
    e: Edges,
    viewport: Viewport,
    top: Color,
    bottom: Color,
) {
    let Some(texture) = texture else { return };
    const STEPS: usize = 16;
    set_blend(out, BlendMode::Alpha);
    for i in 0..STEPS {
        let (t0, t1) = (i as f32 / STEPS as f32, (i + 1) as f32 / STEPS as f32);
        let slice = Edges::new(
            e.x0,
            e.y0 + (e.y1 - e.y0) * t0,
            e.x1,
            e.y0 + (e.y1 - e.y0) * t1,
        );
        let Some(rect_px) = slice.to_px(viewport) else {
            continue;
        };
        let t = (t0 + t1) * 0.5;
        let mut c = [0.0; 4];
        for (k, v) in c.iter_mut().enumerate() {
            *v = top.0[k] + (bottom.0[k] - top.0[k]) * t;
        }
        out.sprites.push(Sprite {
            rect_px,
            uv: UvRect {
                min: Vec2 { x: 0.0, y: t0 },
                max: Vec2 { x: 1.0, y: t1 },
            },
            color: Color(c),
            texture,
        });
    }
}

/// `0x727F0`: the scroll bar (track between the arrow middles, the arrows, the thumb), alpha blend.
fn scroll_bar(
    out: &mut ListDraw,
    bar: &ScrollBar,
    textures: &TextureLookup<'_>,
    viewport: Viewport,
    fade: f32,
) {
    let look = |i: usize| bar.textures[i].as_deref().and_then(textures);
    let (top, bottom) = (
        bar.top.with_alpha_scaled(fade),
        bar.bottom.with_alpha_scaled(fade),
    );
    let mid_up = (bar.up.y1 - bar.outer.y0) * 0.5 + bar.outer.y0;
    let mid_down = (bar.outer.y1 - bar.down.y0) * 0.5 + bar.down.y0;
    textured_gradient(
        out,
        look(0),
        Edges::new(bar.outer.x0, mid_up, bar.outer.x1, mid_down),
        viewport,
        top,
        bottom,
    );
    textured(
        out,
        look(1),
        FULL_UV,
        Edges::new(bar.up.x0, bar.up.y0, bar.up.x1 + 1.0, bar.up.y1),
        viewport,
        top,
        BlendMode::Alpha,
    );
    textured(
        out,
        look(2),
        FULL_UV,
        Edges::new(bar.down.x0, bar.down.y0, bar.down.x1 + 1.0, bar.down.y1),
        viewport,
        bottom,
        BlendMode::Alpha,
    );
    let (thumb_top, thumb_bottom) = (
        bar.thumb_top.with_alpha_scaled(fade),
        bar.thumb_bottom.with_alpha_scaled(fade),
    );
    textured_gradient(out, look(3), bar.thumb, viewport, thumb_top, thumb_bottom);
}

/// Cut the sprites to the band `(top, bottom)` in window pixels (uv cut alike); blend switches follow.
fn clip_sprites(out: &mut ListDraw, band: (f32, f32)) {
    let mut kept = Vec::with_capacity(out.sprites.len());
    let mut remap = vec![0usize; out.sprites.len() + 1];
    for (i, s) in out.sprites.iter().enumerate() {
        remap[i] = kept.len();
        let (y0, y1) = (s.rect_px.origin.y, s.rect_px.origin.y + s.rect_px.size.y);
        let (c0, c1) = (y0.max(band.0), y1.min(band.1));
        if c1 <= c0 {
            continue;
        }
        let mut cut = *s;
        let h = (y1 - y0).max(f32::EPSILON);
        let (v0, v1) = (s.uv.min.y, s.uv.max.y);
        cut.uv.min.y = v0 + (v1 - v0) * (c0 - y0) / h;
        cut.uv.max.y = v0 + (v1 - v0) * (c1 - y0) / h;
        if let Ok(r) = Rect::new(
            Vec2::new(s.rect_px.origin.x, c0),
            Vec2::new(s.rect_px.size.x, c1 - c0),
        ) {
            cut.rect_px = r;
            kept.push(cut);
        }
    }
    remap[out.sprites.len()] = kept.len();
    for (index, _) in out.blends.iter_mut() {
        *index = remap[(*index).min(remap.len() - 1)];
    }
    // Two switches at the same index: the later one wins.
    let mut merged: Vec<(usize, BlendMode)> = Vec::new();
    for (index, mode) in out.blends.drain(..) {
        if merged.last().is_some_and(|(i, _)| *i == index) {
            merged.pop();
        }
        merged.push((index, mode));
    }
    out.blends = merged;
    out.sprites = kept;
}

/// `0x315E50` colours and sizes: cell fill (its argument 6, 0x0A0A0A20 from `0x31AD90`), frame (focused / not), the
/// fill width share in state 2 (`[0x49F7DC]`) and the cell size share (`[0x4785E0]`).
pub const LIGHT_FILL: u32 = 0x0A0A_0A20;
/// The frame colour: `0x315E50` passes `0x154FE0(0x3C50643F, fade)` to `0x81EC0` (MEASURED from the lift; the
/// `0x0A28787F` / `0x05143C7F` pair it computes for the focus state is not used by this path).
pub const LIGHT_FRAME: u32 = 0x3C50_643F;
pub const LIGHT_FILL_FOCUSED: f32 = 0.975;
pub const LIGHT_ICON: f32 = 0.7;
/// `0x81EC0`: corner size of the nine-slice frame (8 units, 8 texels of a 24-texel texture).
pub const SLICE: f32 = 8.0;

/// `0x81EC0`: a nine-slice frame over `area` (corners [`SLICE`] units; `uv` the 24-texel square `0..0x180` of the
/// frame texture, split in thirds), alpha blend.
fn nine_slice(
    out: &mut ListDraw,
    texture: Option<TextureId>,
    uv: UvRect,
    area: Edges,
    viewport: Viewport,
    color: Color,
) {
    let xs = [area.x0, area.x0 + SLICE, area.x1 - SLICE, area.x1];
    let ys = [area.y0, area.y0 + SLICE, area.y1 - SLICE, area.y1];
    let (du, dv) = ((uv.max.x - uv.min.x) / 3.0, (uv.max.y - uv.min.y) / 3.0);
    let us = [uv.min.x, uv.min.x + du, uv.min.x + 2.0 * du, uv.max.x];
    let vs = [uv.min.y, uv.min.y + dv, uv.min.y + 2.0 * dv, uv.max.y];
    for row in 0..3 {
        for column in 0..3 {
            if row == 1 && column == 1 {
                continue;
            }
            let e = Edges::new(xs[column], ys[row], xs[column + 1], ys[row + 1]);
            let uv = UvRect {
                min: Vec2 {
                    x: us[column],
                    y: vs[row],
                },
                max: Vec2 {
                    x: us[column + 1],
                    y: vs[row + 1],
                },
            };
            textured(out, texture, uv, e, viewport, color, BlendMode::Alpha);
        }
    }
}

/// `0x312610`: shrink about the centre, then truncate each edge before `0x81EC0`.
pub fn inset_frame_edges(area: Edges) -> Edges {
    let w = (area.x1 - area.x0) * LIGHT_ICON;
    let h = (area.y1 - area.y0) * LIGHT_ICON;
    let x = area.x0 + ((area.x1 - area.x0) - w) * 0.5;
    let y = area.y0 + ((area.y1 - area.y0) - h) * 0.5;
    Edges::new(x.trunc(), y.trunc(), (x + w).trunc(), (y + h).trunc())
}

/// Draw `widgets` (after the highlight, before the text) with page fade `fade`.
pub fn draw(
    widgets: &Widgets,
    white: TextureId,
    textures: &TextureLookup<'_>,
    width: &TextWidth<'_>,
    bytes_of: &dyn Fn(TextRef) -> Option<Vec<u8>>,
    viewport: Viewport,
    fade: f32,
) -> ListDraw {
    let mut out = ListDraw::default();
    if let Some(controls) = &widgets.controls {
        out.append(controls.draw(white, textures, width, viewport));
    }
    let units = widgets.units;
    let white_c = PackedColor(WHITE_RGB | alpha_byte(fade * 127.0)).to_color();
    for draw in &widgets.draws {
        match draw {
            WidgetDraw::Divider { row, upper, lower } => {
                let y = (row.y0 + (row.y1 - row.y0) * 0.5).trunc();
                line(
                    &mut out,
                    white,
                    (row.x0, y),
                    (row.x1, y),
                    viewport,
                    upper.with_alpha_scaled(fade),
                );
                line(
                    &mut out,
                    white,
                    (row.x0, y + 1.0),
                    (row.x1, y + 1.0),
                    viewport,
                    lower.with_alpha_scaled(fade),
                );
            }
            WidgetDraw::StorageIcon { row, texture } => {
                let Some(texture) = textures(texture) else {
                    continue;
                };
                let x = row.x0 + STORAGE_ICON_X;
                let y = row.y0 + (row.y1 - row.y0) * 0.5 - STORAGE_ICON_Y;
                let alpha = alpha_byte(fade * 127.0) as f32 / 127.0;
                let edges = Edges::new(x, y, x + STORAGE_ICON_SIZE, y + STORAGE_ICON_SIZE);
                push(
                    &mut out,
                    texture,
                    edges,
                    viewport,
                    Color([1.0, 1.0, 1.0, alpha.min(1.0)]),
                    BlendMode::Additive,
                );
            }
            WidgetDraw::GroupBox {
                outer,
                inner,
                title,
                title_right,
                title_height,
                light,
                dark,
            } => {
                let g = GroupBox {
                    outer: *outer,
                    inner: *inner,
                    title,
                    title_right: *title_right,
                    title_height: *title_height,
                    light: *light,
                    dark: *dark,
                };
                group_box(&mut out, white, viewport, fade, width, g);
            }
            WidgetDraw::CheckBox {
                area,
                size,
                checked,
                top,
                bottom,
            } => {
                let b = check_box_edges(*area, *size, units);
                box_frame(&mut out, white, b, viewport, fade, *top, *bottom);
                if *checked {
                    let inner = Edges::new(
                        b.x0 + CHECK_INSET,
                        b.y0 + CHECK_INSET,
                        b.x1 - CHECK_INSET,
                        b.y1 - CHECK_INSET,
                    );
                    push(&mut out, white, inner, viewport, white_c, BlendMode::Alpha);
                }
            }
            WidgetDraw::Slider {
                band,
                value,
                ticks,
                hovered,
                light,
                dark,
            } => {
                let (a, thumb) = slider_edges(*band, *value, units);
                let mid_y = (a.y0 + a.y1) * 0.5;
                let (left, right) = (a.x0 + 1.0, a.x1 - 1.0);
                // Two passes: offset one unit in `+0x40`, then in place in `+0x3C`; the ticks are white both times.
                for (pass, colour) in [
                    (1.0, dark.with_alpha_scaled(fade)),
                    (0.0, light.with_alpha_scaled(fade)),
                ] {
                    fill_int(
                        &mut out,
                        white,
                        Edges::new(a.x0 + pass, a.y0 + pass, left + pass, a.y1 + pass),
                        viewport,
                        colour,
                    );
                    fill_int(
                        &mut out,
                        white,
                        Edges::new(right + pass, a.y0 + pass, a.x1 + pass, a.y1 + pass),
                        viewport,
                        colour,
                    );
                    line(
                        &mut out,
                        white,
                        (a.x0 + pass, mid_y + pass),
                        (a.x1 + pass, mid_y + pass),
                        viewport,
                        colour,
                    );
                    if *ticks > 0 {
                        let step = (a.x1 - a.x0) / *ticks as f32;
                        for k in 1..=*ticks {
                            let x = a.x0 + step * k as f32;
                            line(
                                &mut out,
                                white,
                                (x, mid_y - SLIDER_TICK),
                                (x, mid_y + SLIDER_TICK),
                                viewport,
                                white_c,
                            );
                        }
                    }
                }
                let thumb_c = if *hovered {
                    white_c
                } else {
                    light.with_alpha_scaled(fade)
                };
                fill_int(&mut out, white, thumb, viewport, thumb_c);
            }
            WidgetDraw::DropDown {
                area,
                arrow,
                glow,
                top,
                bottom,
                arrow_sprite,
                text,
            } => {
                // `0x7BAA0`: the value box gradient of `+0x84` over `+0x88`, faded, times the glow.
                if let Some(rect) = area.to_px(viewport) {
                    let t = brighten(*top, *glow).with_alpha_scaled(fade);
                    let b = brighten(*bottom, *glow).with_alpha_scaled(fade);
                    set_blend(&mut out, BlendMode::Alpha);
                    crate::default_style::corner_gradient(
                        &mut out.sprites,
                        white,
                        rect,
                        [t, t, b, b],
                    );
                }
                // `0x72690(1, ..)` -> `0x81BC0`: the arrow sprite two units in from the square's sides, black with the
                // fade's alpha (the colour argument is the alpha byte alone).
                if let Some(part) = arrow_sprite {
                    let e = Edges::new(arrow.x0 + 2.0, arrow.y0, arrow.x1 - 2.0, arrow.y1);
                    let alpha = (alpha_byte(fade * 127.0) as f32 / 127.0).min(1.0);
                    textured(
                        &mut out,
                        textures(&part.texture),
                        part.uv,
                        e,
                        viewport,
                        Color([0.0, 0.0, 0.0, alpha]),
                        BlendMode::Alpha,
                    );
                }
                if let Some(p) = text {
                    let bytes = (p.text.text == TextRef::Dynamic).then(|| p.text.bytes.clone());
                    let w = (arrow.x0 - p.at.0).max(1.0);
                    place_text(
                        &mut out,
                        p,
                        bytes,
                        w,
                        viewport,
                        fade,
                        WidgetId(WIDGET_TEXT_BASE + 64 * p.text.widget.0),
                    );
                }
            }
            WidgetDraw::Button {
                area,
                top,
                bottom,
                text,
            } => {
                box_frame(&mut out, white, *area, viewport, fade, *top, *bottom);
                if let Some(p) = text {
                    let w = (area.x1 - area.x0).max(1.0);
                    let id = WidgetId(WIDGET_TEXT_BASE + 64 * p.text.widget.0);
                    place_text(
                        &mut out,
                        p,
                        Some(p.text.bytes.clone()),
                        w,
                        viewport,
                        fade,
                        id,
                    );
                }
            }
            WidgetDraw::Label {
                text,
                width: limit,
                max_height,
                fallback,
            } => {
                // `0x40F00(text, width)` then `0x40620`: when the wrapped text is taller than the row, the second font.
                let source = if text.text.bytes.is_empty() {
                    bytes_of(text.text.text).unwrap_or_default()
                } else {
                    text.text.bytes.clone()
                };
                let lines_for =
                    |font: u32| wrap_lines(&source, *limit, &|b| width(TextRef::Dynamic, b, font));
                let mut p = text.clone();
                let mut lines = lines_for(p.font);
                if let Some((font, line_height)) = fallback
                    && lines.len() as f32 * p.line_height > *max_height
                {
                    p.font = *font;
                    p.line_height = *line_height;
                    lines = lines_for(p.font);
                }
                for (index, line) in lines.into_iter().enumerate() {
                    let mut l = p.clone();
                    l.at.1 += p.line_height * index as f32;
                    let id = WidgetId(WIDGET_TEXT_BASE + 64 * p.text.widget.0 + index as u64);
                    place_text(&mut out, &l, Some(line), *limit, viewport, fade, id);
                }
            }
            WidgetDraw::InsetFrame { area, frame } => {
                if let Some(part) = frame {
                    let cell = inset_frame_edges(*area);
                    let outer = Edges::new(
                        cell.x0 - SLICE,
                        cell.y0 - SLICE,
                        cell.x1 + SLICE,
                        cell.y1 + SLICE,
                    );
                    nine_slice(
                        &mut out,
                        textures(&part.texture),
                        part.uv,
                        outer,
                        viewport,
                        PackedColor(LIGHT_FRAME).with_alpha_scaled(fade),
                    );
                }
            }
            WidgetDraw::LightCell {
                area,
                state,
                light,
                frame,
                bulb,
            } => {
                // `0x315E50`: the cell is 0.7 of the row box, centred; its fill (0x0A0A0A20, 0.975 of the width in
                // state 2), the nine-slice frame `0x81EC0` around it (corners outside by 8, [`LIGHT_FRAME`]) and the
                // bulb over it, tinted with the light colour.
                let (w, h) = (
                    (area.x1 - area.x0) * LIGHT_ICON,
                    (area.y1 - area.y0) * LIGHT_ICON,
                );
                let x0 = (area.x1 - area.x0 - w) * 0.5 + area.x0;
                let y0 = (area.y1 - area.y0 - h) * 0.5 + area.y0;
                let cell = Edges::new(x0, y0, x0 + w, y0 + h);
                let fill_w = if *state == 2 {
                    w * LIGHT_FILL_FOCUSED
                } else {
                    w
                };
                let fill = Edges::new(cell.x0, cell.y0, cell.x0 + fill_w, cell.y1);
                push(
                    &mut out,
                    white,
                    fill,
                    viewport,
                    PackedColor(LIGHT_FILL).with_alpha_scaled(fade),
                    BlendMode::Alpha,
                );
                if let Some(part) = frame {
                    let colour = PackedColor(LIGHT_FRAME).with_alpha_scaled(fade);
                    let outer = Edges::new(
                        cell.x0.trunc() - SLICE,
                        cell.y0.trunc() - SLICE,
                        cell.x1.trunc() + SLICE,
                        cell.y1.trunc() + SLICE,
                    );
                    nine_slice(
                        &mut out,
                        textures(&part.texture),
                        part.uv,
                        outer,
                        viewport,
                        colour,
                    );
                }
                if let Some(part) = bulb {
                    textured(
                        &mut out,
                        textures(&part.texture),
                        part.uv,
                        cell,
                        viewport,
                        light.with_alpha_scaled(fade),
                        BlendMode::Alpha,
                    );
                }
            }
        }
    }
    if let Some((top, bottom)) = widgets.clip
        && let (Some(a), Some(b)) = (
            Edges::new(0.0, top, 1.0, top + 1.0).to_px(viewport),
            Edges::new(0.0, bottom, 1.0, bottom + 1.0).to_px(viewport),
        )
    {
        clip_sprites(&mut out, (a.origin.y, b.origin.y));
    }
    if let Some(bar) = &widgets.scroll_bar {
        scroll_bar(&mut out, bar, textures, viewport, fade);
    }
    if out
        .blends
        .last()
        .is_some_and(|(_, mode)| *mode != BlendMode::Alpha)
    {
        out.blends.push((out.sprites.len(), BlendMode::Alpha));
    }
    out
}

#[cfg(test)]
mod tests {
    //! Tier 0 with FABRICATED rows.
    use super::*;

    fn viewport() -> Viewport {
        crate::anchor::window_viewport(640, 480).unwrap()
    }

    #[test]
    fn preview_frame_shrinks_about_centre_and_truncates_each_edge() {
        assert_eq!(
            inset_frame_edges(Edges::new(10.5, 20.5, 111.5, 71.5)),
            Edges::new(25.0, 28.0, 96.0, 63.0)
        );
    }

    #[test]
    fn divider_draws_two_lines_at_the_row_centre() {
        let widgets = Widgets {
            draws: vec![WidgetDraw::Divider {
                row: Edges::new(265.6, 94.0, 614.4, 103.6),
                upper: PackedColor(0x1010_107F),
                lower: PackedColor(0x5050_507F),
            }],
            ..Widgets::default()
        };
        let out = draw(
            &widgets,
            TextureId(1),
            &|_| None,
            &|_, _, _| 0.0,
            &|_| None,
            viewport(),
            0.5,
        );
        assert_eq!(out.sprites.len(), 2);
        // One pixel above the truncated centre (98) and the next unit, from the truncated x0, two pixels short.
        assert_eq!(out.sprites[0].rect_px.origin.y, 97.0);
        assert_eq!(out.sprites[1].rect_px.origin.y, 98.0);
        assert_eq!(out.sprites[0].rect_px.origin.x, 265.0);
        assert_eq!(out.sprites[0].rect_px.size.x, 614.0 - 265.0 - 2.0);
        assert_eq!(out.sprites[0].rect_px.size.y, 1.0);
        assert!((out.sprites[0].color.0[3] - 126.0 / 255.0).abs() < 1e-3);
        assert_eq!(out.blends, vec![(0, BlendMode::Alpha)]);
    }

    #[test]
    fn storage_icon_is_additive_and_sixteen_units() {
        let widgets = Widgets {
            draws: vec![WidgetDraw::StorageIcon {
                row: Edges::new(265.6, 132.3, 614.4, 161.1),
                texture: "hd".into(),
            }],
            ..Widgets::default()
        };
        let out = draw(
            &widgets,
            TextureId(1),
            &|n| (n == "hd").then_some(TextureId(7)),
            &|_, _, _| 0.0,
            &|_| None,
            viewport(),
            1.0,
        );
        assert_eq!(out.sprites.len(), 1);
        let s = out.sprites[0];
        assert_eq!(s.texture, TextureId(7));
        assert!((s.rect_px.origin.x - 285.6).abs() < 1e-3);
        assert!((s.rect_px.size.x - 16.0).abs() < 1e-3);
        assert!((s.rect_px.origin.y - (146.7 - 8.0)).abs() < 1e-3);
        assert_eq!(
            out.blends,
            vec![(0, BlendMode::Additive), (1, BlendMode::Alpha)]
        );
        assert_eq!(widgets.texture_names(), vec!["hd".to_string()]);
        // A missing texture draws nothing.
        assert!(
            draw(
                &widgets,
                TextureId(1),
                &|_| None,
                &|_, _, _| 0.0,
                &|_| None,
                viewport(),
                1.0
            )
            .sprites
            .is_empty()
        );
    }

    fn units() -> Units {
        Units {
            x: 6.4,
            y: 4.8,
            origin_x: 0.0,
            origin_y: 0.0,
        }
    }

    #[test]
    fn group_box_leaves_a_gap_for_the_title() {
        let title = WidgetText {
            widget: WidgetId(3),
            text: TextRef::Id(crate::model::TextId(7)),
            bytes: vec![],
        };
        let widgets = Widgets {
            draws: vec![WidgetDraw::GroupBox {
                outer: Edges::new(360.0, 64.0, 616.0, 224.0),
                inner: Edges::new(380.0, 80.0, 596.0, 208.0),
                title: Some(title),
                title_right: false,
                title_height: 10.0,
                light: PackedColor(0x7F7F_7F7F),
                dark: PackedColor(0x1020_307F),
            }],
            units: units(),
            wraps: vec![],
            clip: None,
            scroll_bar: None,

            controls: None,
            images: vec![],
        };
        let out = draw(
            &widgets,
            TextureId(1),
            &|_| None,
            &|_, _, _| 40.0,
            &|_| None,
            viewport(),
            1.0,
        );
        assert_eq!(out.sprites.len(), 10, "ten etch lines");
        // The top etch line runs from the left midline (370) to the gap (380 - 5) and on from 380 + 40 + 5.
        let top: Vec<_> = out
            .sprites
            .iter()
            .filter(|s| s.rect_px.origin.y == 71.0 && s.rect_px.size.y == 1.0)
            .collect();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].rect_px.origin.x, 370.0);
        assert_eq!(top[0].rect_px.size.x, 375.0 - 370.0 - 2.0);
        assert_eq!(top[1].rect_px.origin.x, 425.0);
        assert_eq!(out.texts.len(), 1);
        assert_eq!(out.texts[0].font_index, GROUP_TITLE_STYLE);
        assert_eq!(out.texts[0].rect_px.origin.x, 380.0);
        // Centred on the top line (72): 72 - 5 - the text lift.
        assert!(
            (out.texts[0].rect_px.origin.y - (72.0 - 5.0 - crate::skin::ANCHOR_TEXT_LIFT)).abs()
                < 1e-3
        );
        // The etch colour is darker than the light colour, with 0.7 of its alpha.
        let etch = top[0].color;
        assert!(etch.0[0] < 1.0 && (etch.0[3] - 0.7).abs() < 0.02);
    }

    #[test]
    fn check_box_fills_when_checked_and_slider_places_the_thumb() {
        let u = units();
        let b = check_box_edges(Edges::new(380.8, 169.9, 595.2, 189.0), 2.0, u);
        assert!((b.x0 - 384.0).abs() < 1e-3 && (b.x1 - 396.8).abs() < 1e-3);
        let make = |checked| Widgets {
            draws: vec![WidgetDraw::CheckBox {
                area: Edges::new(380.8, 169.9, 595.2, 189.0),
                size: 2.8,
                checked,
                top: PackedColor(0x4040_407F),
                bottom: PackedColor(0x1010_107F),
            }],
            units: u,
            wraps: vec![],
            clip: None,
            scroll_bar: None,

            controls: None,
            images: vec![],
        };
        let open = draw(
            &make(false),
            TextureId(1),
            &|_| None,
            &|_, _, _| 0.0,
            &|_| None,
            viewport(),
            1.0,
        )
        .sprites
        .len();
        let shut = draw(
            &make(true),
            TextureId(1),
            &|_| None,
            &|_, _, _| 0.0,
            &|_| None,
            viewport(),
            1.0,
        )
        .sprites
        .len();
        assert_eq!(shut, open + 1);
        // Slider: value 50 sits in the middle of the band.
        let (band, thumb) = slider_edges(Edges::new(380.8, 237.0, 595.2, 263.4), 50.0, u);
        assert!((band.y1 - band.y0 - 19.2).abs() < 1e-3);
        let centre = (thumb.x0 + thumb.x1) * 0.5;
        assert!((centre - (band.x0 + band.x1) * 0.5).abs() < 1e-3);
        assert!((thumb.x1 - thumb.x0 - 9.6).abs() < 1e-3);
    }

    #[test]
    fn wrapped_rows_split_into_lines() {
        // One unit per byte.
        let measure = |b: &[u8]| b.len() as f32;
        let lines = wrap_lines(b"Changing mode from Advanced to Beginner", 14.0, &measure);
        assert_eq!(
            lines,
            vec![
                b"Changing mode".to_vec(),
                b"from Advanced".to_vec(),
                b"to Beginner".to_vec()
            ]
        );
        let base = TextPlacement {
            widget: WidgetId(2),
            text: TextRef::Id(crate::model::TextId(5)),
            font_index: 4,
            value_font_index: 0,
            align: Align::Start,
            rect_px: Rect::new(Vec2::new(10.0, 20.0), Vec2::new(100.0, 10.0)).unwrap(),
            color: Color([1.0; 4]),
            value: None,
        };
        let other = TextPlacement {
            widget: WidgetId(3),
            ..base.clone()
        };
        let mut texts = vec![base, other];
        let mut dynamic = Vec::new();
        let wraps = [Wrap {
            widget: WidgetId(2),
            width: 14.0,
            line_height: 16.0,
        }];
        split_wrapped(
            &mut texts,
            &wraps,
            &|_| Some(b"Changing mode from Advanced".to_vec()),
            &|b, _| b.len() as f32,
            2.0,
            &mut dynamic,
        );
        assert_eq!(texts.len(), 3, "two lines and the untouched row");
        assert_eq!(texts[1].rect_px.origin.y, 52.0);
        assert_eq!(texts[1].text, TextRef::Dynamic);
        assert_eq!(
            dynamic[1],
            (WidgetId(WRAP_ID_BASE + 128 + 1), b"from Advanced".to_vec())
        );
        assert_eq!(texts[2].widget, WidgetId(3));
    }

    #[test]
    fn drop_down_scroll_clip_and_light_cell() {
        let text = WidgetText {
            widget: WidgetId(4),
            text: TextRef::Dynamic,
            bytes: b"Military".to_vec(),
        };
        let part =
            TexturePart::sixteenths("arrow".into(), (0x600, 0x3F0, 0x7F0, 0x200), (128.0, 64.0));
        assert!(
            (part.uv.min.x - 0.75).abs() < 1e-3 && part.uv.max.y < part.uv.min.y,
            "the down arrow is the flipped part"
        );
        let widgets = Widgets {
            draws: vec![
                WidgetDraw::DropDown {
                    area: Edges::new(316.8, 79.7, 595.2, 103.6),
                    arrow: Edges::new(571.0, 79.7, 595.2, 103.9),
                    glow: 1.0,
                    top: PackedColor(0x4040_607F),
                    bottom: PackedColor(0x2020_307F),
                    arrow_sprite: Some(part),
                    text: Some(PlacedText {
                        text,
                        font: 4,
                        at: (330.0, 84.0),
                        align_end: false,
                        focused: false,
                        line_height: 14.0,
                    }),
                },
                WidgetDraw::LightCell {
                    area: Edges::new(540.0, 132.0, 595.0, 184.0),
                    state: 0,
                    light: PackedColor(0x7F7F_7F80),
                    frame: Some(TexturePart::sixteenths(
                        "frame".into(),
                        (0, 0, 0x180, 0x180),
                        (32.0, 32.0),
                    )),
                    bulb: Some(TexturePart {
                        texture: "slot:39".into(),
                        uv: FULL_UV,
                    }),
                },
            ],
            units: units(),
            clip: Some((60.0, 150.0)),
            ..Widgets::default()
        };
        let look = |n: &str| {
            Some(TextureId(match n {
                "arrow" => 2,
                "frame" => 3,
                _ => 4,
            }))
        };
        let out = draw(
            &widgets,
            TextureId(1),
            &look,
            &|_, _, _| 0.0,
            &|_| None,
            viewport(),
            1.0,
        );
        assert_eq!(out.texts.len(), 1);
        assert_eq!(out.dynamic[0].1, b"Military".to_vec());
        let arrow = out
            .sprites
            .iter()
            .find(|s| s.texture == TextureId(2))
            .unwrap();
        assert_eq!(arrow.color.0[0], 0.0, "black arrow");
        assert!((arrow.rect_px.origin.x - 573.0).abs() < 1e-3);
        // The light cell is cut at the view bottom (150): the bulb (0.7 of the row, 139.8..176.2) keeps its top part
        // and its uv is cut alike.
        let bulb = out
            .sprites
            .iter()
            .find(|s| s.texture == TextureId(4))
            .unwrap();
        assert!((bulb.rect_px.origin.y + bulb.rect_px.size.y - 150.0).abs() < 1e-3);
        assert!(bulb.uv.max.y < 0.5 && bulb.uv.max.y > 0.2);
        assert!(
            out.sprites
                .iter()
                .all(|s| s.rect_px.origin.y >= 60.0 - 1e-3)
        );
        let frame = out
            .sprites
            .iter()
            .filter(|s| s.texture == TextureId(3))
            .count();
        assert!(
            frame >= 3 && frame <= 8,
            "the nine-slice minus the cut bottom row"
        );
    }

    #[test]
    fn colour_helpers_follow_the_original() {
        assert_eq!(
            brighten(PackedColor(0x1020_407F), 1.4),
            PackedColor(0x162C_597F)
        );
        assert_eq!(
            mix(PackedColor(0x6464_647F), PackedColor(0x0000_007F), 0.5),
            PackedColor(0x3232_327F)
        );
    }
}
