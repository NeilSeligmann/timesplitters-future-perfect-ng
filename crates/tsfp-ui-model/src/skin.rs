//! Original front-end skin (T2307 UI-9): the frame, fill, row highlight and text colours of the
//! front-end theme, as typed data and sprite builders over the `tsfp-ui-renderer` contract.
//!
//! Source (MEASURED by static reading of `default.xbe`, docs/t2307-ui-visual-fidelity.md):
//! the theme init `0x2D9650` installs the page callbacks `0x2D8060` (fill, via `0x2D7F90` and
//! `0x2D7B70`), `0x2D7AE0` (rim, via `0x2D76B0`) and `0x2D9390` (focused row, via `0x2D8100`) and
//! the colour block at `[0x701CC4]` (text colour pairs read by `0x75F60`). Textures are named by
//! the texture table `0x4D2D64` (24-byte records, name at record - 8, getter `0x80F20`). The
//! texture pixels are not here: the host reads them from the owner's disc at run time and passes
//! a [`SkinTextures`] map. No game bytes live in this module, only numbers and names.
//!
//! Approximations (INFERRED, listed in the doc): per-vertex colour ramps of the original quads
//! are drawn as `RAMP_STEPS` flat-coloured slices (the renderer contract has one colour per
//! sprite); the vertical text colour pair is drawn as its mean; the fill's back-buffer pass of
//! `0x2D7F90` is not reproduced; the page rectangle is the union of the row rectangles plus
//! [`OriginalSkin::page_margin`] (measured) because the menu-tree dump has no page rect.

use crate::bridge::{RenderLayout, TextPlacement};
use crate::model::{Align, Page, WidgetId, WidgetKind};
use tsfp_ui_renderer::{BlendMode, Color, Rect, Sprite, TextureId, UvRect, Vec2, Viewport};

/// A colour as the original stores it: `0xRRGGBBAA`, normalized bytes
/// followed by the original texture combiner's times-two mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedColor(pub u32);

impl PackedColor {
    /// Linear straight-alpha colour, each channel `byte * 2 / 255` clamped to 1.
    /// MEASURED in the original main-menu rim vertex program and pixel combiner
    /// (T2307 section24); 0x7F is 254/255, while 0x80 saturates to 1.
    pub fn to_color(self) -> Color {
        let channel = |shift: u32| (((self.0 >> shift) & 0xFF) as f32 / 255.0 * 2.0).min(1.0);
        Color([channel(24), channel(16), channel(8), channel(0)])
    }
    /// Scale the packed alpha byte before normalized upload (`0x154FE0`).
    /// The original clamps finite factors to 0..1, multiplies in the x87
    /// domain and truncates through `0x3C848C`; RGB bytes remain unchanged.
    pub fn with_alpha_scaled(self, factor: f32) -> Color {
        let alpha = ((self.0 & 0xff) as f64 * factor.clamp(0.0, 1.0) as f64).trunc() as u32;
        PackedColor((self.0 & 0xffff_ff00) | alpha).to_color()
    }
}

/// The textures the skin draws, by role. `table_index` is the original texture table index
/// (getter `0x80F20`), `name` its name string; the disc holds them as
/// `assets/ts/textures/xbox/<name>.xbt` (CRC32 key) in the front-end pak.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SkinTexture {
    /// Rounded rim corner, arc in the top-right quadrant (index 0xF).
    FrameCorner,
    /// Rim edge, line along the top (index 0x12).
    FrameEdge,
    /// Filled quarter disc, top-right quadrant (index 0x10).
    FillCorner,
    /// Solid fill (index 0x11).
    FillCentre,
    /// Flat block used for the highlight line (index 0).
    Line,
    /// Soft glow used around the highlight line (index 1).
    Glow,
}

impl SkinTexture {
    pub const ALL: [SkinTexture; 6] = [
        SkinTexture::FrameCorner,
        SkinTexture::FrameEdge,
        SkinTexture::FillCorner,
        SkinTexture::FillCentre,
        SkinTexture::Line,
        SkinTexture::Glow,
    ];
    pub fn table_index(self) -> u32 {
        match self {
            SkinTexture::FrameCorner => 0x0F,
            SkinTexture::FrameEdge => 0x12,
            SkinTexture::FillCorner => 0x10,
            SkinTexture::FillCentre => 0x11,
            SkinTexture::Line => 0x00,
            SkinTexture::Glow => 0x01,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            SkinTexture::FrameCorner => "femenubordernewcorner",
            SkinTexture::FrameEdge => "femenuborderline",
            SkinTexture::FillCorner => "femenubordernewcornerfill",
            SkinTexture::FillCentre => "femenubordernewfill",
            SkinTexture::Line => "filled16x16",
            SkinTexture::Glow => "gradient",
        }
    }
    /// Disc path whose CRC32 is the pak key (MEASURED: 822 of the table names resolve this way).
    pub fn disc_path(self) -> String {
        format!("assets/ts/textures/xbox/{}.xbt", self.name())
    }
}

/// Renderer texture ids of the skin. `edge_transposed` is the rim edge texture with its axes
/// swapped (the original rotates the edge UVs on the left and right sides; the contract's
/// `UvRect` can only flip, so the loader uploads a transposed copy).
#[derive(Clone, Debug, PartialEq)]
pub struct SkinTextures {
    pub ids: Vec<(SkinTexture, TextureId)>,
    pub edge_transposed: Option<TextureId>,
}

impl SkinTextures {
    pub fn get(&self, role: SkinTexture) -> Option<TextureId> {
        self.ids.iter().find(|(r, _)| *r == role).map(|(_, id)| *id)
    }
    /// True when every role and the transposed edge are present.
    pub fn complete(&self) -> bool {
        SkinTexture::ALL.iter().all(|r| self.get(*r).is_some()) && self.edge_transposed.is_some()
    }
}

/// Text colour pair of a row state: top and bottom of the glyphs (`0x75F60`, colour block pairs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextPair {
    pub top: PackedColor,
    pub bottom: PackedColor,
}

impl TextPair {
    /// One colour for a run: the mean RGB of the pair at the first colour's alpha (the contract
    /// has one colour per run; the legacy focused row reads as that mean, opaque).
    pub fn mean(self, alpha: f32) -> Color {
        let (a, b) = (self.top.to_color().0, self.bottom.to_color().0);
        Color([
            (a[0] + b[0]) * 0.5,
            (a[1] + b[1]) * 0.5,
            (a[2] + b[2]) * 0.5,
            (a[3] * alpha).clamp(0.0, 1.0),
        ])
    }
}

/// The `original` skin preset. Values with an address are MEASURED from code; values marked
/// INFERRED were set by comparison with the legacy host picture of the main menu (doc,
/// Calibration).
#[derive(Clone, Debug, PartialEq)]
pub struct OriginalSkin {
    /// Rim colours, bright on the top-left and bottom-right corners (`0x2D76B0`: 0x7F7F7F, 0x282828).
    pub rim_bright: PackedColor,
    pub rim_dark: PackedColor,
    /// Rim alpha (INFERRED: the original takes it from the FPU stack; 1.0 matches the host
    /// legacy main menu, rim corner 222 and far corner 63 on a 0..255 scale).
    pub rim_alpha: f32,
    /// Fill colour (`0x2D7F90`: black, drawn twice) and the alpha of one pass (INFERRED).
    pub fill: PackedColor,
    pub fill_alpha: f32,
    pub fill_passes: u32,
    /// Corner radius in 640x480 units: 32, or half the height minus 1 under 70.4 (`0x2D8060`).
    pub max_radius: f32,
    /// Focused row fill (`0x2D9390`: 0x2F3C503C, alpha times the pulse `0x2E6430`).
    pub highlight_fill: PackedColor,
    /// Highlight line (`0x2D8100`: 0x4B697F32) and its glow (0x0A2D5632), 2 units tall.
    pub highlight_line: PackedColor,
    pub highlight_glow: PackedColor,
    /// Text pairs (`0x2D9650` -> `0x809D0`): 0 normal, 1 focused; disabled is normal at half
    /// alpha. Unfocused rows draw the first colour only (`0x76150` -> `0x41DC0`), the focused row
    /// both (`0x41E30`), drawn here as their mean.
    pub text_normal: TextPair,
    pub text_focused: TextPair,
    /// T2315: entries 3 and 4 of the row colour table (`row + 0x90`, MEASURED live on every batch-1 page): the text of a
    /// greyed row (row flag bit 0, a disabled widget) on the active page, unfocused and focused (`0x75F60`).
    pub text_greyed: TextPair,
    pub text_greyed_focused: TextPair,
    /// Page rect margin (x, y) around the row union (INFERRED from the legacy main menu: rim
    /// peaks 7 left of and 11 above the rows, 640x480 units).
    pub page_margin: (f32, f32),
    /// The rim and fill stop this many units short of the page's right and bottom edge
    /// (`0x2D7AE0` and `0x2D8060` pass `right - 2`, `bottom - 2`; one more unit measured on the
    /// legacy picture, likely the D3D texel-centre convention).
    pub frame_inset: f32,
    /// Centre the labels of pages without cyclers (MEASURED on the legacy main menu picture,
    /// mechanism INFERRED: the row text x offset +0x3C).
    pub centre_list_rows: bool,
    /// Text scale (x, y) of the skinned page. MEASURED in legacy main-menu glyph uploads:
    /// font object 4 (0.67, 0.625) becomes (0.8375, 0.669643), hence (1.25, 15/14).
    /// The shared vertical factor also holds for the Arial button hints (see T2307 findings).
    pub text_scale: (f32, f32),
    /// Text moved down by this many 640x480 units (INFERRED, measured: legacy glyph tops sit
    /// 5 units below the shaped baseline rule of `FontTableShaper` at this scale).
    pub text_drop: f32,
    /// Live original-skin font baseline correction, in design units (INFERRED from the pause readback).
    pub anchor_text_lift: f32,
    /// The fill's back-buffer pass (`0x204F0`, T2313): the scene inside the box is replaced by a
    /// Gaussian blur of itself with this sigma (640x480 units) minus `backdrop_darken` (0..1).
    /// INFERRED: fitted on the legacy main menu dump (docs/t2313-ui-backdrop.md); `0.0` sigma
    /// switches the pass off.
    pub backdrop_sigma: f32,
    pub backdrop_darken: f32,
}

/// Slices of a colour ramp.
pub const RAMP_STEPS: usize = 32;

impl OriginalSkin {
    pub fn original() -> Self {
        Self {
            rim_bright: PackedColor(0x7F7F_7F7F),
            rim_dark: PackedColor(0x2828_287F),
            rim_alpha: 1.0,
            fill: PackedColor(0x0000_007F),
            fill_alpha: 0.3,
            fill_passes: 2,
            max_radius: 32.0,
            highlight_fill: PackedColor(0x2F3C_503C),
            highlight_line: PackedColor(0x4B69_7F32),
            highlight_glow: PackedColor(0x0A2D_5632),
            text_normal: TextPair {
                top: PackedColor(0x7878_787F),
                bottom: PackedColor(0x1E1E_1E7F),
            },
            text_focused: TextPair {
                top: PackedColor(0x7F7F_7F7F),
                bottom: PackedColor(0x0523_5A50),
            },
            text_greyed: TextPair {
                top: PackedColor(0x2828_287F),
                bottom: PackedColor(0x2828_287F),
            },
            text_greyed_focused: TextPair {
                top: PackedColor(0x2828_3250),
                bottom: PackedColor(0x0A0A_1E40),
            },
            page_margin: (7.0, 11.0),
            frame_inset: 3.0,
            centre_list_rows: true,
            text_scale: (1.25, 15.0 / 14.0),
            text_drop: 5.0,
            anchor_text_lift: 0.0,
            backdrop_sigma: 2.0,
            backdrop_darken: 8.0 / 255.0,
        }
    }

    /// Corner radius of a page `height` units tall (`0x2D8060` / `0x2D7AE0`).
    pub fn radius(&self, height: f32) -> f32 {
        if height < 70.4 {
            (height * 0.5 - 1.0).max(0.0)
        } else {
            self.max_radius
        }
    }

    /// The highlight pulse of `0x2E6430` at frame `frame` (60 Hz): 0.5 + 0.15 (sin + 1).
    pub fn pulse(frame: u64) -> f32 {
        let phase = (frame % 60) as f32 / 60.0 * std::f32::consts::TAU;
        0.5 + 0.15 * (phase.sin() + 1.0)
    }

    /// T2315: the cursor-row text alpha factor of `0x75F60` (MEASURED from code): `(sine + 1) * [0x475CDC] (0.3) +
    /// [0x475D04] (0.4)` on the same 60-frame sine clock as [`Self::pulse`], times the page fade.
    pub fn text_pulse(frame: u64) -> f32 {
        let phase = (frame % 60) as f32 / 60.0 * std::f32::consts::TAU;
        0.4 + 0.3 * (phase.sin() + 1.0)
    }
}

/// What the skin adds to a page: sprites under the text, and the layout with its placeholder
/// quads removed and the text colours of the skin.
#[derive(Clone, Debug, PartialEq)]
pub struct SkinnedPage {
    pub sprites: Vec<Sprite>,
    /// Blend switches over `sprites`: `(index, mode)` = from `sprites[index]` on draw with `mode`
    /// (T2309). Empty = all alpha. The focused-row fill is additive (`0x19D70(2)` in `0x2D8100`).
    pub blends: Vec<(usize, BlendMode)>,
    pub layout: RenderLayout,
    pub page_rect: Rect,
    /// Page fade before focused-text pulse (needed by non-pulsing greyed list rows).
    pub fade: f32,
    /// T2311: index in `sprites` right after the fill and the row draws (divider), before the
    /// focused-row highlight.
    pub rows_end: usize,
    /// T2311: index in `sprites` right after the focused-row highlight, before the rim.
    pub highlight_end: usize,
    /// The fill's rounded box in pixels and its corner radius (where the back-buffer pass applies).
    pub panel: Rect,
    pub panel_radius: f32,
}

pub(crate) fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Option<Rect> {
    Rect::new(Vec2::new(x0, y0), Vec2::new(x1 - x0, y1 - y0)).ok()
}

pub(crate) fn uv(u0: f32, v0: f32, u1: f32, v1: f32) -> UvRect {
    UvRect {
        min: Vec2::new(u0, v0),
        max: Vec2::new(u1, v1),
    }
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    let mut out = [0.0; 4];
    for (i, o) in out.iter_mut().enumerate() {
        *o = a.0[i] + (b.0[i] - a.0[i]) * t;
    }
    Color(out)
}

/// A quad whose colour ramps from `from` to `to` along x (`horizontal`) or y, as slices.
pub(crate) fn ramp(
    out: &mut Vec<Sprite>,
    texture: TextureId,
    area: Rect,
    tex: UvRect,
    from: Color,
    to: Color,
    horizontal: bool,
) {
    ramp_steps(out, texture, area, tex, from, to, horizontal, RAMP_STEPS);
}

/// [`ramp`] in `steps` slices (T2315: long bars need finer slices than [`RAMP_STEPS`] to hide the steps).
#[allow(clippy::too_many_arguments)]
pub(crate) fn ramp_steps(
    out: &mut Vec<Sprite>,
    texture: TextureId,
    area: Rect,
    tex: UvRect,
    from: Color,
    to: Color,
    horizontal: bool,
    steps: usize,
) {
    let steps = steps.max(1);
    for step in 0..steps {
        let (t0, t1) = (step as f32 / steps as f32, (step + 1) as f32 / steps as f32);
        let color = lerp(from, to, (t0 + t1) * 0.5);
        let (o, s) = (area.origin, area.size);
        let (slice, slice_uv) = if horizontal {
            (
                rect(o.x + s.x * t0, o.y, o.x + s.x * t1, o.y + s.y),
                uv(
                    tex.min.x + (tex.max.x - tex.min.x) * t0,
                    tex.min.y,
                    tex.min.x + (tex.max.x - tex.min.x) * t1,
                    tex.max.y,
                ),
            )
        } else {
            (
                rect(o.x, o.y + s.y * t0, o.x + s.x, o.y + s.y * t1),
                uv(
                    tex.min.x,
                    tex.min.y + (tex.max.y - tex.min.y) * t0,
                    tex.max.x,
                    tex.min.y + (tex.max.y - tex.min.y) * t1,
                ),
            )
        };
        if let Some(rect_px) = slice {
            out.push(Sprite {
                rect_px,
                uv: slice_uv,
                color,
                texture,
            });
        }
    }
}

fn sprite(
    out: &mut Vec<Sprite>,
    texture: TextureId,
    area: Option<Rect>,
    tex: UvRect,
    color: Color,
) {
    if let Some(rect_px) = area {
        out.push(Sprite {
            rect_px,
            uv: tex,
            color,
            texture,
        });
    }
}

/// Corner UVs of `0x2D76B0` / `0x2D7B70` (the texture's arc is top-right): TL flips u, TR as is,
/// BL flips both, BR flips v.
const CORNER_UV: [(f32, f32, f32, f32); 4] = [
    (1.0, 0.0, 0.0, 1.0),
    (0.0, 0.0, 1.0, 1.0),
    (1.0, 1.0, 0.0, 0.0),
    (0.0, 1.0, 1.0, 0.0),
];

/// Rounded fill of `0x2D7B70`: four corner quarter discs, the four edges and the centre, one colour.
pub fn panel_fill(out: &mut Vec<Sprite>, ids: &SkinTextures, r: Rect, radius: f32, color: Color) {
    let (Some(corner), Some(fill)) = (
        ids.get(SkinTexture::FillCorner),
        ids.get(SkinTexture::FillCentre),
    ) else {
        return;
    };
    let (x0, y0) = (r.origin.x, r.origin.y);
    let (x1, y1) = (x0 + r.size.x, y0 + r.size.y);
    let k = radius;
    let corners = [(x0, y0), (x1 - k, y0), (x0, y1 - k), (x1 - k, y1 - k)];
    for ((cx, cy), (u0, v0, u1, v1)) in corners.into_iter().zip(CORNER_UV) {
        sprite(
            out,
            corner,
            rect(cx, cy, cx + k, cy + k),
            uv(u0, v0, u1, v1),
            color,
        );
    }
    let full = uv(0.0, 0.0, 1.0, 1.0);
    for area in [
        rect(x0 + k, y0, x1 - k, y0 + k),
        rect(x0 + k, y1 - k, x1 - k, y1),
        rect(x0, y0 + k, x1, y1 - k),
    ] {
        sprite(out, fill, area, full, color);
    }
}

/// Approximate the original continuous rim gradient to within half an 8-bit
/// channel value. The flat-colour sprite contract requires slices; a fixed 32
/// slices leaves visible bands on long rims. This bound depends on the source
/// colours. At window resolution, also use at least one slice per pixel along
/// the ramp: a pixel centre then samples its own midpoint instead of a shared
/// multi-pixel band. This remains a flat-colour approximation for fractional
/// edges, not a per-vertex gradient or a fitted brightness correction.
fn rim_ramp(
    out: &mut Vec<Sprite>,
    texture: TextureId,
    area: Rect,
    tex: UvRect,
    from: Color,
    to: Color,
    horizontal: bool,
) {
    let delta = from
        .0
        .iter()
        .zip(to.0)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    let color_steps = ((delta * 255.0).ceil() as usize).clamp(RAMP_STEPS, 255);
    let pixel_steps = (if horizontal { area.size.x } else { area.size.y }).ceil() as usize;
    let steps = color_steps.max(pixel_steps);
    ramp_steps(out, texture, area, tex, from, to, horizontal, steps);
}

/// Rim of `0x2D76B0`: corner arcs and edge lines, bright top-left and bottom-right, dark
/// top-right and bottom-left, edges ramp between their corners.
pub fn panel_rim(
    out: &mut Vec<Sprite>,
    ids: &SkinTextures,
    r: Rect,
    radius: f32,
    bright: Color,
    dark: Color,
) {
    let (Some(corner), Some(edge), Some(edge_t)) = (
        ids.get(SkinTexture::FrameCorner),
        ids.get(SkinTexture::FrameEdge),
        ids.edge_transposed,
    ) else {
        return;
    };
    let (x0, y0) = (r.origin.x, r.origin.y);
    let (x1, y1) = (x0 + r.size.x, y0 + r.size.y);
    let k = radius;
    let corners = [
        ((x0, y0), bright),
        ((x1 - k, y0), dark),
        ((x0, y1 - k), dark),
        ((x1 - k, y1 - k), bright),
    ];
    for (((cx, cy), color), (u0, v0, u1, v1)) in corners.into_iter().zip(CORNER_UV) {
        sprite(
            out,
            corner,
            rect(cx, cy, cx + k, cy + k),
            uv(u0, v0, u1, v1),
            color,
        );
    }
    // Top: line at the outer (top) side, bright -> dark left to right.
    if let Some(area) = rect(x0 + k, y0, x1 - k, y0 + k) {
        rim_ramp(out, edge, area, uv(0.0, 0.0, 1.0, 1.0), bright, dark, true);
    }
    // Bottom: line at the bottom, dark -> bright.
    if let Some(area) = rect(x0 + k, y1 - k, x1 - k, y1) {
        rim_ramp(out, edge, area, uv(0.0, 1.0, 1.0, 0.0), dark, bright, true);
    }
    // Left: transposed texture, line at the left, bright -> dark top to bottom.
    if let Some(area) = rect(x0, y0 + k, x0 + k, y1 - k) {
        rim_ramp(
            out,
            edge_t,
            area,
            uv(0.0, 0.0, 1.0, 1.0),
            bright,
            dark,
            false,
        );
    }
    // Right: line at the right, dark -> bright.
    if let Some(area) = rect(x1 - k, y0 + k, x1, y1 - k) {
        rim_ramp(
            out,
            edge_t,
            area,
            uv(1.0, 0.0, 0.0, 1.0),
            dark,
            bright,
            false,
        );
    }
}

/// One highlight strip of `0x2D8100` from `x0` to `x1` at `y`: a glow `2 * unit` either side and
/// a line `2 * unit` below, alpha ramping from `alpha_from` to `alpha_to`.
#[allow(clippy::too_many_arguments)]
fn highlight_strip(
    out: &mut Vec<Sprite>,
    ids: &SkinTextures,
    skin: &OriginalSkin,
    x0: f32,
    x1: f32,
    y: f32,
    unit: f32,
    alpha_from: f32,
    alpha_to: f32,
) {
    let (Some(glow), Some(line)) = (ids.get(SkinTexture::Glow), ids.get(SkinTexture::Line)) else {
        return;
    };
    if let Some(area) = rect(x0, y - 2.0 * unit, x1, y + 2.0 * unit) {
        let (a, b) = (
            skin.highlight_glow.with_alpha_scaled(alpha_from),
            skin.highlight_glow.with_alpha_scaled(alpha_to),
        );
        ramp(out, glow, area, uv(0.0, 0.0, 1.0, 1.0), a, b, true);
    }
    if let Some(area) = rect(x0, y, x1, y + 2.0 * unit) {
        let (a, b) = (
            skin.highlight_line.with_alpha_scaled(alpha_from),
            skin.highlight_line.with_alpha_scaled(alpha_to),
        );
        ramp(out, line, area, uv(0.0, 0.0, 1.0, 1.0), a, b, true);
    }
}

/// Focused row of `0x2D9390`: a pulsing fill and a line at the top and bottom, brightest at the
/// row centre. `fade` is the page fade (`page + 0xF4`), `pulse` from [`OriginalSkin::pulse`].
pub fn row_highlight(
    out: &mut Vec<Sprite>,
    ids: &SkinTextures,
    skin: &OriginalSkin,
    row: Rect,
    unit: f32,
    fade: f32,
    pulse: f32,
) {
    let Some(fill) = ids.get(SkinTexture::Line) else {
        return;
    };
    sprite(
        out,
        fill,
        Some(row),
        uv(0.0, 0.0, 1.0, 1.0),
        skin.highlight_fill.with_alpha_scaled(fade * pulse),
    );
    let (x0, x1) = (row.origin.x, row.origin.x + row.size.x);
    let mid = (x0 + x1) * 0.5;
    let half = fade * 0.5;
    for y in [row.origin.y, row.origin.y + row.size.y - 2.0 * unit] {
        highlight_strip(out, ids, skin, x0, mid, y, unit, 0.0, half);
        highlight_strip(out, ids, skin, mid, x1, y, unit, half, 0.0);
    }
}

/// `0x2D9390`: top-edge extension takes precedence over bottom-edge extension.
fn focused_row_edges(
    mut row: Rect,
    edges: crate::model::HighlightEdges,
    viewport: Viewport,
) -> Rect {
    let unit = viewport.scale();
    // Resolve design coordinates through the same centred viewport as the widgets.
    let Ok(page) = viewport.resolve(crate::model::menu_rect(
        tsfp_ui_renderer::Vec2::new(0.0, edges.top),
        tsfp_ui_renderer::Vec2::new(1.0, edges.bottom - edges.top),
    )) else {
        return row;
    };
    let bottom = row.origin.y + row.size.y;
    if row.origin.y <= page.origin.y + edges.top_margin * unit {
        row.origin.y = page.origin.y + 4.0 * unit;
        row.size.y = bottom - row.origin.y;
    } else if bottom >= page.origin.y + page.size.y - edges.bottom_margin * unit {
        row.size.y = page.origin.y + page.size.y - 4.0 * unit - row.origin.y;
    }
    row
}

/// Row draw callback `0x2D82F0` (T2311): the divider row of the Xbox Live / System Link,
/// Player Progress and Story pages.
pub const DIVIDER_ROW_DRAW: u32 = 0x2D_82F0;
/// Divider colour of `0x2D82F0`: 0x7F7F7F, alpha `page fade * 127` (`0x475CC0`).
pub const DIVIDER_COLOR: PackedColor = PackedColor(0x7F7F_7F7F);
/// Divider line height in 640x480 units (`0x475D10`, 2.0).
pub const DIVIDER_HEIGHT: f32 = 2.0;

/// The divider of `0x2D82F0` (MEASURED by static reading): a line `DIVIDER_HEIGHT` units tall
/// from the vertical centre of the row box down, texture table entry 0 (`filled16x16`), alpha
/// blend (`0x19D70(1)`), two quads meeting at the horizontal centre whose alpha ramps from 0 at
/// the row edges to `fade` at the centre (vertex colours 0x7F7F7F00 and 0x7F7F7F00 | alpha).
pub fn divider(out: &mut Vec<Sprite>, ids: &SkinTextures, row: Rect, unit: f32, fade: f32) {
    let Some(line) = ids.get(SkinTexture::Line) else {
        return;
    };
    let (x0, x1) = (row.origin.x, row.origin.x + row.size.x);
    let (mid_x, mid_y) = ((x0 + x1) * 0.5, row.origin.y + row.size.y * 0.5);
    let shown = DIVIDER_COLOR.with_alpha_scaled(fade);
    let hidden = DIVIDER_COLOR.with_alpha_scaled(0.0);
    let y1 = mid_y + DIVIDER_HEIGHT * unit;
    if let Some(left) = rect(x0, mid_y, mid_x, y1) {
        ramp(out, line, left, uv(0.0, 0.0, 1.0, 1.0), hidden, shown, true);
    }
    if let Some(right) = rect(mid_x, mid_y, x1, y1) {
        ramp(
            out,
            line,
            right,
            uv(0.0, 0.0, 1.0, 1.0),
            shown,
            hidden,
            true,
        );
    }
}

/// The custom row draws of `page` that the skin draws itself, in row order (after the fill,
/// under the focused-row highlight, as the original row pass runs before the focus callback).
pub fn row_draws(
    out: &mut Vec<Sprite>,
    ids: &SkinTextures,
    page: &Page,
    viewport: Viewport,
    fade: f32,
) {
    for widget in &page.widgets {
        if widget.draw != Some(crate::model::HandlerRef::Legacy(DIVIDER_ROW_DRAW)) {
            continue;
        }
        if let Ok(row) = viewport.resolve(widget.rect) {
            divider(out, ids, row, viewport.scale(), fade);
        }
    }
}

/// T2315: the shaped glyph top sits this many units below the placement origin compared with the original text draw
/// (INFERRED: the main menu calibration `text_drop` 5.0 equals its rows' anchor `1.25 * 4.8` = 6.0 minus this lift).
pub const ANCHOR_TEXT_LIFT: f32 = 1.0;

/// T2315: the label placement of the original default row draw `0x76150` (see [`crate::model::TextAnchor`]); `unit` is
/// pixels per design unit.
/// T2315: the outline passes of `0x41E30` (design units), in draw order.
pub const OUTLINE_OFFSETS: [(f32, f32); 4] = [(-2.0, 0.0), (0.0, -1.0), (2.0, 0.0), (0.0, 1.0)];

pub fn anchored_placement(
    placement: &TextPlacement,
    anchor: crate::model::TextAnchor,
    unit: f32,
    color: Color,
) -> TextPlacement {
    anchored_placement_with_lift(placement, anchor, unit, color, ANCHOR_TEXT_LIFT)
}

fn anchored_placement_with_lift(
    placement: &TextPlacement,
    anchor: crate::model::TextAnchor,
    unit: f32,
    color: Color,
    lift: f32,
) -> TextPlacement {
    let row = placement.rect_px;
    let left = row.origin.x;
    let right = row.origin.x + row.size.x;
    let at = left + anchor.x * unit;
    let (x0, x1) = match anchor.align {
        Align::Start => (at, right.max(at + 1.0)),
        Align::End => (left.min(at - 1.0), at),
        Align::Center => {
            let half = (at - left).max(right - at).max(1.0);
            (at - half, at + half)
        }
    };
    let y = row.origin.y + (anchor.y - lift) * unit;
    TextPlacement {
        color,
        align: anchor.align,
        rect_px: Rect::new(Vec2::new(x0, y), Vec2::new(x1 - x0, row.size.y)).unwrap_or(row),
        ..placement.clone()
    }
}

/// The page rectangle: union of all widget rects (disabled rows included, the visible part of a scrolled row,
/// `Widget::bounds`) plus the margin, in pixels.
pub fn page_rect(page: &Page, viewport: Viewport, margin_units: (f32, f32)) -> Option<Rect> {
    if let Some(bounds) = page.panel_bounds {
        return viewport.resolve(bounds).ok();
    }
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for widget in &page.widgets {
        let Ok(r) = viewport.resolve(widget.bounds.unwrap_or(widget.rect)) else {
            continue;
        };
        let (x0, y0, x1, y1) = (
            r.origin.x,
            r.origin.y,
            r.origin.x + r.size.x,
            r.origin.y + r.size.y,
        );
        bounds = Some(match bounds {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    }
    let (x0, y0, x1, y1) = bounds?;
    let (mx, my) = (
        margin_units.0 * viewport.scale(),
        margin_units.1 * viewport.scale(),
    );
    rect(x0 - mx, y0 - my, x1 + mx, y1 + my)
}

/// Skin one page: fill, focused-row highlight and rim as sprites (painter order), the layout's
/// placeholder quads dropped and its text recoloured. None when a texture role is missing or
/// the page has no rect, so the caller keeps the plain layout.
#[allow(clippy::too_many_arguments)]
pub fn skin_page(
    skin: &OriginalSkin,
    ids: &SkinTextures,
    page: &Page,
    layout: &RenderLayout,
    viewport: Viewport,
    focused: Option<WidgetId>,
    fade: f32,
    frame: u64,
) -> Option<SkinnedPage> {
    skin_page_with_pulses(
        skin,
        ids,
        page,
        layout,
        viewport,
        focused,
        fade,
        OriginalSkin::pulse(frame),
        OriginalSkin::text_pulse(frame),
    )
}

/// Native presentation seam. The caller owns time and supplies bounded pulse samples.
/// The original wrapper above retains its exact integer-frame calculations.
#[allow(clippy::too_many_arguments)]
pub fn skin_page_with_pulses(
    skin: &OriginalSkin,
    ids: &SkinTextures,
    page: &Page,
    layout: &RenderLayout,
    viewport: Viewport,
    focused: Option<WidgetId>,
    fade: f32,
    highlight_pulse: f32,
    text_pulse: f32,
) -> Option<SkinnedPage> {
    if !ids.complete() {
        return None;
    }
    let unit = viewport.scale();
    let page_px = page_rect(page, viewport, skin.page_margin)?;
    // Live page geometry uses the measured original right/bottom inset (0x2D76B0).
    // Keep the calibrated dump-only fallback for pages with no original rectangle.
    let inset = if page.panel_bounds.is_some() {
        2.0
    } else {
        skin.frame_inset
    } * unit;
    let frame_px = rect(
        page_px.origin.x,
        page_px.origin.y,
        page_px.origin.x + page_px.size.x - inset,
        page_px.origin.y + page_px.size.y - inset,
    )?;
    let radius = skin
        .radius(page_px.size.y / unit)
        .min(frame_px.size.x / unit * 0.5)
        * unit;
    let mut sprites = Vec::new();
    for _ in 0..skin.fill_passes {
        panel_fill(
            &mut sprites,
            ids,
            frame_px,
            radius,
            skin.fill.with_alpha_scaled(skin.fill_alpha * fade),
        );
    }
    row_draws(&mut sprites, ids, page, viewport, fade);
    let rows_end = sprites.len();
    let focused_rect = focused.and_then(|id| {
        page.widgets
            .iter()
            .find(|w| w.id == id && w.is_focusable())
            .and_then(|w| viewport.resolve(w.rect).ok())
    });
    let mut blends = Vec::new();
    if let Some(mut row) = focused_rect {
        if let Some(edges) = page.highlight_edges {
            row = focused_row_edges(row, edges, viewport);
        }
        let fill_at = sprites.len();
        row_highlight(&mut sprites, ids, skin, row, unit, fade, highlight_pulse);
        // The first highlight sprite is the pulsing fill (MEASURED blend mode 2), the strips are alpha.
        if sprites.len() > fill_at {
            blends.push((fill_at, BlendMode::Additive));
            blends.push((fill_at + 1, BlendMode::Alpha));
        }
    }
    let highlight_end = sprites.len();
    panel_rim(
        &mut sprites,
        ids,
        frame_px,
        radius,
        skin.rim_bright.with_alpha_scaled(skin.rim_alpha * fade),
        skin.rim_dark.with_alpha_scaled(skin.rim_alpha * fade),
    );
    let centre = skin.centre_list_rows && page.widgets.iter().all(|w| w.kind != WidgetKind::Cycler);
    let texts = layout
        .texts
        .iter()
        .flat_map(|placement| {
            let widget = page.widgets.iter().find(|w| w.id == placement.widget);
            let enabled = widget.is_none_or(|w| w.enabled);
            // T2315 increment 3: the cursor row's text is `0x41E30`: four passes in the pair's second colour moved by
            // (-2, 0), (0, -1), (+2, 0), (0, +1) units (an outline), then the text in the first colour; both at the
            // `0x75F60` alpha pulse. Other rows are one pass (`0x41DC0`) in the first colour.
            let focused_pair = match (focused == Some(placement.widget), enabled) {
                (true, true) => Some(skin.text_focused),
                (true, false) => Some(skin.text_greyed_focused),
                _ => None,
            };
            let strength = if enabled { fade * text_pulse } else { fade };
            let color = match (focused_pair, enabled) {
                // 0x75F60 scales both colours of a live focused row by the same pulse.
                // Greyed rows select their own pair at plain page fade.
                (Some(pair), _) => pair.top.with_alpha_scaled(strength),
                (None, true) => skin.text_normal.top.with_alpha_scaled(fade),
                // T2315: a greyed row keeps the greyed colour pair of its table, focused or not.
                (None, false) => skin.text_greyed.top.with_alpha_scaled(fade),
            };
            let outlined = |texts: Vec<TextPlacement>| -> Vec<TextPlacement> {
                let Some(pair) = focused_pair else {
                    return texts;
                };
                let edge = pair.bottom.with_alpha_scaled(strength);
                let mut out = Vec::with_capacity(texts.len() * 5);
                for fill in texts {
                    for (dx, dy) in OUTLINE_OFFSETS {
                        let mut pass = fill.clone();
                        pass.color = edge;
                        pass.rect_px.origin = Vec2::new(
                            fill.rect_px.origin.x + dx * unit,
                            fill.rect_px.origin.y + dy * unit,
                        );
                        out.push(pass);
                    }
                    out.push(fill);
                }
                out
            };
            if let Some(anchor) = widget.and_then(|w| w.text_anchor) {
                let label = anchored_placement_with_lift(
                    placement,
                    anchor,
                    unit,
                    color,
                    skin.anchor_text_lift,
                );
                // T2315: a cycler value with its own anchor (`0x7B7B0`) is a second placement in the value font.
                if let (Some(value), Some(at)) =
                    (placement.value, widget.and_then(|w| w.value_anchor))
                {
                    let value = TextPlacement {
                        text: value,
                        font_index: placement.value_font_index,
                        value: None,
                        ..placement.clone()
                    };
                    return outlined(vec![
                        TextPlacement {
                            value: None,
                            ..label
                        },
                        anchored_placement_with_lift(
                            &value,
                            at,
                            unit,
                            color,
                            skin.anchor_text_lift,
                        ),
                    ]);
                }
                return outlined(vec![label]);
            }
            outlined(vec![TextPlacement {
                color,
                align: if centre {
                    Align::Center
                } else {
                    placement.align
                },
                rect_px: Rect::new(
                    Vec2::new(
                        placement.rect_px.origin.x,
                        placement.rect_px.origin.y + skin.text_drop * unit,
                    ),
                    placement.rect_px.size,
                )
                .unwrap_or(placement.rect_px),
                ..placement.clone()
            }])
        })
        .collect();
    Some(SkinnedPage {
        sprites,
        blends,
        layout: RenderLayout {
            quads: Vec::new(),
            texts,
        },
        page_rect: page_px,
        fade,
        rows_end,
        highlight_end,
        panel: frame_px,
        panel_radius: radius,
    })
}

#[cfg(test)]
mod tests {
    //! Tier 0 with FABRICATED pages and texture ids; the colours are the MEASURED constants.
    use super::*;
    use crate::model::{Widget, menu_rect};

    #[test]
    fn focused_edge_rows_extend_and_top_wins_when_both_thresholds_match() {
        let viewport = crate::anchor::window_viewport(1280, 960).unwrap();
        let edges = crate::model::HighlightEdges {
            top: 100.5,
            bottom: 300.5,
            top_margin: 20.0,
            bottom_margin: 20.0,
        };
        let row = |top: f32, bottom: f32| rect(100.0, top * 2.0, 900.0, bottom * 2.0).unwrap();
        let first = focused_row_edges(row(120.5, 150.0), edges, viewport);
        assert_eq!(first.origin.y, 209.0);
        assert_eq!(first.origin.y + first.size.y, 300.0);
        let last = focused_row_edges(row(250.0, 280.5), edges, viewport);
        assert_eq!(last.origin.y, 500.0);
        assert_eq!(last.origin.y + last.size.y, 593.0);
        let middle = row(150.0, 250.0);
        assert_eq!(focused_row_edges(middle, edges, viewport), middle);
        let both = focused_row_edges(row(110.0, 290.0), edges, viewport);
        assert_eq!(both.origin.y, 209.0);
        assert_eq!(both.origin.y + both.size.y, 580.0);
    }

    #[test]
    fn rim_gradient_stays_within_half_a_channel_of_the_continuous_ramp() {
        let mut out = Vec::new();
        let from = Color([1.0, 0.2, 0.4, 1.0]);
        let to = Color([40.0 / 127.0, 0.8, 0.1, 1.0]);
        let area = rect(0.0, 0.0, 900.0, 64.0).unwrap();
        rim_ramp(
            &mut out,
            TextureId(1),
            area,
            uv(0.0, 0.0, 1.0, 1.0),
            from,
            to,
            true,
        );
        assert!(out.len() > RAMP_STEPS);
        for sprite in &out {
            for x in [
                sprite.rect_px.origin.x,
                sprite.rect_px.origin.x + sprite.rect_px.size.x,
            ] {
                let exact = lerp(from, to, x / area.size.x);
                for (actual, expected) in sprite.color.0.into_iter().zip(exact.0) {
                    assert!((actual - expected).abs() <= 0.5 / 255.0 + 1e-6);
                }
            }
        }
        assert_eq!(out.first().unwrap().uv.min.x, 0.0);
        assert_eq!(out.last().unwrap().uv.max.x, 1.0);
    }

    #[test]
    fn rim_samples_integer_pixel_centres_at_window_resolution() {
        let from = Color([254.0 / 255.0; 4]);
        let to = Color([80.0 / 255.0; 4]);
        for horizontal in [true, false] {
            for length in [300.0, 600.0, 1200.0] {
                let area = if horizontal {
                    rect(14.0, 22.0, 14.0 + length, 30.0).unwrap()
                } else {
                    rect(14.0, 22.0, 22.0, 22.0 + length).unwrap()
                };
                let mut out = Vec::new();
                rim_ramp(
                    &mut out,
                    TextureId(1),
                    area,
                    uv(0.0, 0.0, 1.0, 1.0),
                    from,
                    to,
                    horizontal,
                );
                assert_eq!(out.len(), length as usize);
                for (pixel, sprite) in out.iter().enumerate() {
                    let exact = lerp(from, to, (pixel as f32 + 0.5) / length);
                    for (actual, expected) in sprite.color.0.into_iter().zip(exact.0) {
                        assert!((actual - expected).abs() < 1e-6);
                    }
                }
            }
        }
    }

    #[test]
    fn anchored_placement_follows_the_row_record() {
        use crate::model::TextAnchor;
        let base = TextPlacement {
            widget: WidgetId(0),
            text: crate::model::TextRef::None,
            font_index: 4,
            value_font_index: 0,
            align: Align::Center,
            rect_px: Rect::new(Vec2::new(100.0, 50.0), Vec2::new(400.0, 30.0)).unwrap(),
            color: Color([1.0; 4]),
            value: None,
        };
        // Start: 2.0 * 6.4 units in at unit 2 px, y 0.65 * 4.8 units down minus the lift.
        let start = anchored_placement(
            &base,
            TextAnchor {
                x: 12.8,
                y: 3.12,
                align: Align::Start,
            },
            2.0,
            base.color,
        );
        assert_eq!(start.align, Align::Start);
        assert!((start.rect_px.origin.x - 125.6).abs() < 1e-3);
        assert!((start.rect_px.origin.y - (50.0 + (3.12 - ANCHOR_TEXT_LIFT) * 2.0)).abs() < 1e-3);
        assert!((start.rect_px.origin.x + start.rect_px.size.x - 500.0).abs() < 1e-3);
        // Centre: the rect is symmetric around the anchor (half the row width in: the row centre).
        let centre = anchored_placement(
            &base,
            TextAnchor {
                x: 100.0,
                y: 6.0,
                align: Align::Center,
            },
            2.0,
            base.color,
        );
        let middle = centre.rect_px.origin.x + centre.rect_px.size.x / 2.0;
        assert!((middle - 300.0).abs() < 1e-3);
    }

    fn ids() -> SkinTextures {
        SkinTextures {
            ids: SkinTexture::ALL
                .iter()
                .enumerate()
                .map(|(i, r)| (*r, TextureId(100 + i as u64)))
                .collect(),
            edge_transposed: Some(TextureId(200)),
        }
    }

    #[test]
    fn packed_colour_scale() {
        assert_eq!(
            PackedColor(0x7F7F_7F7F).to_color(),
            Color([254.0 / 255.0; 4])
        );
        assert_eq!(PackedColor(0x8080_80FF).to_color(), Color([1.0; 4]));
        let c = PackedColor(0x2F3C_503C).to_color().0;
        assert!((c[0] - 94.0 / 255.0).abs() < 1e-6 && (c[3] - 120.0 / 255.0).abs() < 1e-6);
        assert_eq!(
            PackedColor(0x0000_007F).with_alpha_scaled(0.25).0[3],
            62.0 / 255.0
        );
    }

    #[test]
    fn packed_alpha_scales_before_saturation_and_clamps_the_factor() {
        let full_byte = PackedColor(0x2f3c_50ff);
        let half = full_byte.with_alpha_scaled(0.5);
        assert_eq!(
            half.0[3],
            254.0 / 255.0,
            "255 * .5 truncates to127 before normalization"
        );
        assert_eq!(
            &half.0[..3],
            &full_byte.to_color().0[..3],
            "scaling changes only alpha"
        );
        let partial = PackedColor(0x2f3c_503c);
        assert_eq!(
            partial.with_alpha_scaled(2.0),
            partial.to_color(),
            "clamp factor before multiplication"
        );
        assert_eq!(partial.with_alpha_scaled(-1.0).0[3], 0.0);
        assert_eq!(
            PackedColor(0x7f7f_7f7f).with_alpha_scaled(0.7).0[3],
            176.0 / 255.0
        );
    }

    #[test]
    fn radius_rule() {
        let s = OriginalSkin::original();
        assert_eq!(s.radius(200.0), 32.0);
        assert_eq!(s.radius(40.0), 19.0);
    }

    #[test]
    fn disc_paths_and_indices() {
        assert_eq!(
            SkinTexture::FrameCorner.disc_path(),
            "assets/ts/textures/xbox/femenubordernewcorner.xbt"
        );
        let mut indices: Vec<u32> = SkinTexture::ALL.iter().map(|r| r.table_index()).collect();
        indices.sort();
        indices.dedup();
        assert_eq!(indices.len(), 6);
    }

    fn page() -> Page {
        let mut a = Widget::new(WidgetId(1), WidgetKind::Button);
        a.rect = menu_rect(Vec2::new(100.0, 100.0), Vec2::new(200.0, 30.0));
        let mut b = Widget::new(WidgetId(2), WidgetKind::Button);
        b.rect = menu_rect(Vec2::new(100.0, 140.0), Vec2::new(200.0, 30.0));
        b.enabled = false;
        Page {
            widgets: vec![a, b],
            ..Page::new(crate::model::PageId(7), 1)
        }
    }

    #[test]
    fn list_draws_go_after_the_highlight_and_keep_its_blend_switches() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let skin = OriginalSkin::original();
        let page = page();
        let layout = RenderLayout {
            quads: vec![],
            texts: page
                .widgets
                .iter()
                .map(|w| TextPlacement {
                    widget: w.id,
                    text: crate::model::TextRef::Dynamic,
                    font_index: 0,
                    value_font_index: 0,
                    align: Align::Center,
                    rect_px: viewport.resolve(w.rect).unwrap(),
                    color: Color([1.0; 4]),
                    value: None,
                })
                .collect(),
        };
        let mut skinned = skin_page(
            &skin,
            &ids(),
            &page,
            &layout,
            viewport,
            Some(WidgetId(1)),
            1.0,
            0,
        )
        .unwrap();
        let (fill_at, end) = (skinned.rows_end, skinned.highlight_end);
        assert_eq!(
            skinned.blends,
            vec![
                (fill_at, BlendMode::Additive),
                (fill_at + 1, BlendMode::Alpha)
            ]
        );
        let glow = Sprite {
            texture: TextureId(77),
            ..skinned.sprites[0]
        };
        let list = crate::list_rows::ListDraw {
            sprites: vec![glow, glow],
            blends: vec![(1, BlendMode::Additive)],
            texts: Vec::new(),
            left_aligned: vec![(WidgetId(2), 50.0)],
            greyed: vec![WidgetId(2)],
            dynamic: Vec::new(),
            text_scale_x: Vec::new(),
            fixed_spacing: Vec::new(),
        };
        skinned.insert_list(list);
        assert_eq!(skinned.sprites[end].texture, TextureId(77));
        assert_eq!(skinned.highlight_end, end + 2);
        assert_eq!(
            skinned.blends,
            vec![
                (fill_at, BlendMode::Additive),
                (fill_at + 1, BlendMode::Alpha),
                (end + 1, BlendMode::Additive),
                (end + 2, BlendMode::Alpha),
            ]
        );
        let text = skinned
            .layout
            .texts
            .iter()
            .find(|t| t.widget == WidgetId(2))
            .unwrap();
        assert_eq!((text.align, text.rect_px.origin.x), (Align::Start, 50.0));
        assert_eq!(text.color.0[0], GREYED_TEXT);

        // T2315 increment 4 (e): a focused locked row (outlined, five placements) takes the greyed focused pair.
        let mut focused = skin_page(
            &skin,
            &ids(),
            &page,
            &layout,
            viewport,
            Some(WidgetId(1)),
            1.0,
            0,
        )
        .unwrap();
        assert_eq!(
            focused
                .layout
                .texts
                .iter()
                .filter(|t| t.widget == WidgetId(1))
                .count(),
            5
        );
        focused.insert_list(crate::list_rows::ListDraw {
            left_aligned: vec![(WidgetId(1), 50.0)],
            greyed: vec![WidgetId(1)],
            ..Default::default()
        });
        let passes: Vec<&TextPlacement> = focused
            .layout
            .texts
            .iter()
            .filter(|t| t.widget == WidgetId(1))
            .collect();
        let pair = skin.text_greyed_focused;
        assert_eq!(
            passes[4].color,
            pair.top.with_alpha_scaled(1.0),
            "fill: entry 4 top"
        );
        assert_eq!(
            &passes[0].color.0[..3],
            &pair.bottom.to_color().0[..3],
            "outline: entry 4 bottom"
        );
        assert!(passes[0].color.0[0] != GREYED_TEXT);
    }

    #[test]
    fn divider_row_draw_is_two_ramps_meeting_at_the_centre() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let mut page = page();
        page.widgets[1].draw = Some(crate::model::HandlerRef::Legacy(DIVIDER_ROW_DRAW));
        let mut out = Vec::new();
        row_draws(&mut out, &ids(), &page, viewport, 0.5);
        assert_eq!(out.len(), 2 * RAMP_STEPS, "only the divider row draws");
        let line = ids().get(SkinTexture::Line).unwrap();
        assert!(out.iter().all(|s| s.texture == line));
        // Row 100..300 x 140..170: the line starts at the centre y 155, is 2 units tall.
        let (first, last) = (&out[0], &out[out.len() - 1]);
        assert_eq!(
            (
                first.rect_px.origin.x,
                first.rect_px.origin.y,
                first.rect_px.size.y
            ),
            (100.0, 155.0, 2.0)
        );
        let end = last.rect_px.origin.x + last.rect_px.size.x;
        assert!((end - 300.0).abs() < 1e-3);
        // Alpha 0 at the edges, `fade` at the centre.
        let mid = &out[RAMP_STEPS - 1];
        assert!(first.color.0[3] < 0.05 && last.color.0[3] < 0.05);
        assert!((mid.color.0[3] - 0.5).abs() < 0.02);
        assert_eq!(mid.color.0[0], 254.0 / 255.0);
        // A page without the callback gets nothing.
        let mut plain = Vec::new();
        row_draws(&mut plain, &ids(), &self::page(), viewport, 1.0);
        assert!(plain.is_empty());
    }

    #[test]
    fn live_panel_keeps_its_own_extents_at_window_scale() {
        let viewport = crate::anchor::window_viewport(1280, 960).unwrap();
        let mut page = page();
        page.panel_bounds = Some(crate::model::menu_rect(
            Vec2::new(90.0, 80.0),
            Vec2::new(450.0, 280.0),
        ));
        let layout = RenderLayout {
            quads: vec![],
            texts: vec![],
        };
        let skinned = skin_page(
            &OriginalSkin::original(),
            &ids(),
            &page,
            &layout,
            viewport,
            None,
            1.0,
            0,
        )
        .unwrap();
        assert_eq!(
            skinned.page_rect,
            Rect::new(Vec2::new(180.0, 160.0), Vec2::new(900.0, 560.0)).unwrap()
        );
        assert_eq!(
            skinned.panel,
            Rect::new(Vec2::new(180.0, 160.0), Vec2::new(896.0, 556.0)).unwrap()
        );
        // No custom-row union or dump calibration may shrink the live frame.
        page.widgets.clear();
        assert_eq!(
            page_rect(&page, viewport, (7.0, 11.0)),
            Some(skinned.page_rect)
        );
    }

    #[test]
    fn skin_page_orders_fill_highlight_rim_and_recolours_text() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let page = page();
        let layout = RenderLayout {
            quads: vec![],
            texts: page
                .widgets
                .iter()
                .map(|w| TextPlacement {
                    widget: w.id,
                    text: crate::model::TextRef::Dynamic,
                    font_index: 0,
                    value_font_index: 0,
                    align: crate::model::Align::Start,
                    rect_px: viewport.resolve(w.rect).unwrap(),
                    color: Color([1.0; 4]),
                    value: None,
                })
                .collect(),
        };
        let skin = OriginalSkin::original();
        let skinned = skin_page(
            &skin,
            &ids(),
            &page,
            &layout,
            viewport,
            Some(WidgetId(1)),
            1.0,
            0,
        )
        .unwrap();
        assert!(!skinned.sprites.is_empty());
        // Painter order: the fill (FillCorner) first, the rim (FrameCorner) last.
        assert_eq!(skinned.sprites[0].texture, TextureId(102));
        assert_eq!(skinned.sprites.last().unwrap().texture, TextureId(200));
        assert!(skinned.sprites.iter().any(|s| s.texture == TextureId(104)));
        // T2309: exactly the focused-row fill (first highlight sprite, after the two fills) is additive.
        let [(add_at, BlendMode::Additive), (alpha_at, BlendMode::Alpha)] = skinned.blends[..]
        else {
            panic!("blends {:?}", skinned.blends);
        };
        assert_eq!(alpha_at, add_at + 1);
        assert_eq!(
            skinned.sprites[add_at].rect_px,
            viewport.resolve(page.widgets[0].rect).unwrap()
        );
        assert_ne!(
            skinned.sprites[add_at - 1].texture,
            skinned.sprites[add_at].texture
        );
        // Page rect = union (100..300, 100..170) + margin (7, 11).
        let r = skinned.page_rect;
        assert_eq!((r.origin.x, r.origin.y), (93.0, 89.0));
        assert_eq!((r.size.x, r.size.y), (214.0, 92.0));
        // T2315: the focused row is outlined (`0x41E30`): four passes in the pair's second colour (blue) moved by
        // (-2, 0), (0, -1), (2, 0), (0, 1) units, then the first colour (white); the disabled (greyed) row is one pass
        // in the greyed colour pair.
        assert_eq!(skinned.layout.texts.len(), 6);
        let (edge, fill, disabled) = (
            skinned.layout.texts[0].clone(),
            skinned.layout.texts[4].clone(),
            skinned.layout.texts[5].color,
        );
        assert!(
            edge.color.0[2] > edge.color.0[0],
            "outline is the blue second colour"
        );
        assert_eq!(&fill.color.0[..3], &[254.0 / 255.0; 3]);
        assert!(
            (fill.color.0[3] - 176.0 / 255.0).abs() < 1e-6,
            "source pulse at frame0 affects the fill too"
        );
        let unit = viewport.scale();
        assert!((edge.rect_px.origin.x - (fill.rect_px.origin.x - 2.0 * unit)).abs() < 1e-3);
        assert!(
            (skinned.layout.texts[1].rect_px.origin.y - (fill.rect_px.origin.y - unit)).abs()
                < 1e-3
        );
        assert_eq!(
            disabled,
            OriginalSkin::original().text_greyed.top.to_color()
        );
        assert!(
            disabled.0[0] < OriginalSkin::original().text_normal.top.to_color().0[0],
            "greyed is darker"
        );
        assert_eq!(skinned.layout.texts[4].align, crate::model::Align::Center);
        assert!(skinned.layout.quads.is_empty());
    }

    #[test]
    fn loader_reads_the_uppercase_row_flag() {
        // FABRICATED tree: row 1 carries flag 0x2000 among others, row 2 does not.
        let tree = r#"{"schema":1,"pages":{"0x100":{"page_id":256,"builder":1,"widget_types":[1],"rows":[
          {"row_id":1,"row_flags":927232,"activate_callback":4096},
          {"row_id":2,"row_flags":4096,"activate_callback":4096}]}},"edges":[]}"#;
        let model = crate::UiModel::from_tree_json(tree, &crate::LoadOptions::default()).unwrap();
        let page = model.page(crate::model::PageId(256)).unwrap();
        let flags: Vec<bool> = page.widgets.iter().map(|w| w.uppercase).collect();
        assert_eq!(flags, vec![true, false]);
    }

    #[test]
    fn incomplete_textures_decline() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let mut partial = ids();
        partial.edge_transposed = None;
        let layout = RenderLayout {
            quads: vec![],
            texts: vec![],
        };
        let skin = OriginalSkin::original();
        assert!(skin_page(&skin, &partial, &page(), &layout, viewport, None, 1.0, 0).is_none());
    }
}

/// Generic text of a locked list row: an opaque neutral grey (INFERRED, measured on the legacy
/// Story picture: glyphs 77,77,77 at full coverage, independent of the background).
pub const GREYED_TEXT: f32 = 77.0 / 255.0;

impl SkinnedPage {
    /// T2311: put the list row draws (`list_rows::draw_list_rows`) at [`Self::highlight_end`] and their
    /// texts after the generic row texts; the generic text of a list row is left aligned at the
    /// list text x (greyed on locked rows).
    pub fn insert_list(&mut self, list: crate::list_rows::ListDraw) {
        // After the focused-row highlight: the legacy Story picture shows the focused row's icon
        // not washed by the additive highlight fill (MEASURED: icon mean 140,87,29 legacy against
        // 151,117,81 when drawn under the highlight), so the list rows draw over it.
        let at = self.highlight_end;
        let n = list.sprites.len();
        if n > 0 {
            self.sprites.splice(at..at, list.sprites);
            for (index, _) in self.blends.iter_mut() {
                if *index >= at {
                    *index += n;
                }
            }
            let mut added: Vec<(usize, BlendMode)> = list
                .blends
                .into_iter()
                .map(|(i, mode)| (i + at, mode))
                .collect();
            if !added.is_empty() {
                added.push((at + n, BlendMode::Alpha));
            }
            // The list's switches go first so a page switch at the same index (the highlight
            // fill right after the list) still wins; the sort is stable.
            added.append(&mut self.blends);
            added.sort_by_key(|(index, _)| *index);
            self.blends = added;
            self.highlight_end += n;
        }
        // T2315 increment 4 (e): the focused locked row (its text has the four outline passes of `0x41E30`, so five
        // placements) takes the greyed focused pair of `0x75F60` (colour table entry 4: fill `0x28283250`, outline
        // `0x0A0A1E40`, MEASURED table), at the fill's alpha; other locked rows keep [`GREYED_TEXT`].
        let focused_pair = OriginalSkin::original().text_greyed_focused;
        let mut seen: Vec<(WidgetId, usize)> = Vec::new();
        let passes = |widget: WidgetId, texts: &[TextPlacement]| {
            texts.iter().filter(|t| t.widget == widget).count()
        };
        let counts: Vec<(WidgetId, usize)> = list
            .greyed
            .iter()
            .map(|w| (*w, passes(*w, &self.layout.texts)))
            .collect();
        for text in self.layout.texts.iter_mut() {
            if let Some((_, x)) = list.left_aligned.iter().find(|(w, _)| *w == text.widget) {
                let right = text.rect_px.origin.x + text.rect_px.size.x;
                text.align = Align::Start;
                if list.greyed.contains(&text.widget) {
                    let outlined = counts
                        .iter()
                        .any(|(w, n)| *w == text.widget && *n == OUTLINE_OFFSETS.len() + 1);
                    let index = match seen.iter_mut().find(|(w, _)| *w == text.widget) {
                        Some((_, n)) => {
                            *n += 1;
                            *n
                        }
                        None => {
                            seen.push((text.widget, 0));
                            0
                        }
                    };
                    text.color = if outlined {
                        let pair = if index < OUTLINE_OFFSETS.len() {
                            focused_pair.bottom
                        } else {
                            focused_pair.top
                        };
                        // 0x75F60 entries 3/4 are greyed at page fade, without the enabled-row pulse.
                        pair.with_alpha_scaled(self.fade)
                    } else {
                        Color([GREYED_TEXT, GREYED_TEXT, GREYED_TEXT, text.color.0[3]])
                    };
                }
                text.rect_px = Rect::new(
                    Vec2::new(*x, text.rect_px.origin.y),
                    Vec2::new((right - x).max(1.0), text.rect_px.size.y),
                )
                .unwrap_or(text.rect_px);
            }
        }
        // The list texts are placed at the original's text position (`0x41DC0` x, y), so they
        // do not take the skin's `text_drop` (MEASURED on the legacy Story picture: with the drop
        // the level name sat 5 units low).
        self.layout.texts.extend(list.texts);
    }
}
