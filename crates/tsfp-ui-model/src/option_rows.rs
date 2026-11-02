//! T2315 increment 3: the Audio/Video page's own draws as readable Rust over the `tsfp-ui-renderer` contract: the page
//! draw `0x2C4190` (the safe-area frame of the screen position mode), the slider row draw `0x2C3500` with the bar draw
//! `0x814B0`, and the row icon draw `0x2C3410`.
//!
//! Source: static reading of `default.xbe` (MEASURED from code, docs/t2315-ui-page-completion.md, increment 3). The host
//! reads every guest value these draws read (slider values, texture names and sizes, the safe area, rodata constants) on
//! the guest thread and hands them over as an [`OptionRows`] value, so this module is pure. Texture pixels come from the
//! owner's disc at run time. No game bytes or strings live here.
//!
//! Coordinates are 640x480 design units (the original's), mapped to pixels by the page viewport.

use crate::WidgetId;
use crate::list_rows::{Edges, TextureLookup, alpha_byte};
use crate::skin::{PackedColor, RAMP_STEPS, ramp_steps, uv};
use tsfp_ui_renderer::{Color, Sprite, TextureId, Viewport};

/// Audio/Video slider row draw (`(page, row, rect)`).
pub const SLIDER_ROW_DRAW: u32 = 0x2C_3500;
/// Audio/Video row icon draw (`(page, row, rect)`), also called at the end of [`SLIDER_ROW_DRAW`].
pub const ICON_ROW_DRAW: u32 = 0x2C_3410;
/// Audio/Video page draw (descriptor draw, page `+0x10`).
pub const AV_PAGE_DRAW: u32 = 0x2C_4190;
/// Controls page row draws: the button picture of the layout row and the turn speed slider.
pub const BUTTON_ROW_DRAW: u32 = 0x2E_A080;
pub const TURN_SPEED_ROW_DRAW: u32 = 0x2E_A250;
/// `0x2EA250`: the bar is at most `[0x47D01C]` (200) wide, `[0x47D208]` (25) clear of the label and `[0x4760CC]` (5)
/// left of the row's right edge.
pub const TEXT_SLIDER_MAX: f32 = 200.0;
pub const TEXT_SLIDER_GAP: f32 = 25.0;
pub const TEXT_SLIDER_RIGHT: f32 = 5.0;
/// `0x2EA080`: the picture tint of a row that is not the cursor row (`0x41DC0` colour), alpha `ftol(fade * 127)`.
pub const PICTURE_DIM_RGB: u32 = 0x2828_2800;
/// `0x2C3410`: texture table index of the icon by row id (`+0x14`) 0, 2 and 4; other ids draw no icon.
pub const ICON_TEXTURES: [(i32, u32); 3] = [(0, 0x158), (2, 0x150), (4, 0x151)];
/// `0x2C3500`: the bar colours (RGBA, before the strength), empty part, fill start and fill end.
pub const BAR_EMPTY: u32 = 0x1119_3C7F;
pub const BAR_FROM: u32 = 0x1119_3C7F;
pub const BAR_TO: u32 = 0x6973_7F7F;
/// `0x2C3500`: the bar starts `[0x478900]` (45) right of the row and is `[0x49EF9C]` (65) narrower, 20 high.
pub const BAR_LEFT: f32 = 45.0;
pub const BAR_SHORTER: f32 = 65.0;
pub const BAR_HEIGHT: i32 = 20;
/// `0x814B0`: the end caps are 8 texels (and 8 units) wide; the texture is three columns (cap, middle, cap) of
/// 8, 16 and 8 texels and two rows of 16 (fill on top, empty below), in 1/16 texel units 0..0x200.
pub const BAR_CAP: i32 = 8;

/// The icon colour of `0x2C3410`: grey 0x7F7F7F, alpha `ftol(page fade * 127)`.
pub const ICON_RGB: u32 = 0x7F7F_7F00;

/// The cursor-row pulse `0x2E6430(f)`: `f * ((sine[i] + 1) * amplitude + base)`, `i = trunc(frame / period * 2048 +
/// 0.5) & 0x7FF` over the 2048-entry sine table (`[0x4E8CEC]` 60 frames, `[0x4765E0]` 0.15, `[0x475CD4]` 0.5, read
/// live).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pulse {
    pub base: f32,
    pub amplitude: f32,
    /// Frames per cycle; 0 or non-finite: the peak.
    pub period: f32,
}

impl Pulse {
    pub fn at(&self, frame: u64) -> f32 {
        if !(self.period.is_finite() && self.period > 0.0) {
            return self.base + 2.0 * self.amplitude;
        }
        let index = ((frame as f32 / self.period * 2048.0 + 0.5) as i64) & 0x7FF;
        let sine = (index as f32 / 2048.0 * std::f32::consts::TAU).sin();
        (sine + 1.0) * self.amplitude + self.base
    }
}

/// A texture of the texture table: its name (the disc key) and size in texels (for the 1/16 texel coordinates).
#[derive(Clone, Debug, PartialEq)]
pub struct NamedTexture {
    pub name: String,
    pub width: f32,
    pub height: f32,
}

/// The slider part of a `0x2C3500` row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slider {
    /// `[0x5236BC]` (row id 1) or `[0x5236C0]` (other rows) times `[0x475DC4]` (1/16), 0..1.
    pub value: f32,
    /// Row flag `+0x28` bit 0: strength [`OptionRows::greyed_strength`].
    pub greyed: bool,
    /// The page's cursor (`page +0x18C`) is this row's id: strength pulses.
    pub cursor: bool,
}

/// One row drawn by `0x2C3500` or `0x2C3410`.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionRow {
    /// The row rect the page draw hands the row draw (design units).
    pub area: Edges,
    pub slider: Option<Slider>,
    /// Icon texture of the row id ([`ICON_TEXTURES`]), when it has one.
    pub icon: Option<NamedTexture>,
}

/// The four lines of `0x2C4190` (page state `+0x18C` 4, the screen position mode): the safe area `[0x7E71C8]` (ints
/// `+0x0` left, `+0x4` right, `+0x14` top, `+0x18` bottom) inset by 5, colour 0x7F7F7F with alpha `[0x7BA034]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SafeFrame {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
    pub alpha: u32,
}

/// Rows and page values of an Audio/Video page.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionRows {
    pub rows: Vec<OptionRow>,
    /// The bar texture (`[0x4D45AC]`, a texture handle).
    pub bar: Option<NamedTexture>,
    /// `[0x49F470]` (0.31).
    pub greyed_strength: f32,
    pub pulse: Pulse,
    pub safe_frame: Option<SafeFrame>,
    /// Side arrows of the page's value rows and their texture (`[0x4D6C7C]`).
    pub values: Vec<ValueArrows>,
    pub arrow: Option<NamedTexture>,
    /// Controls page: the layout row's button picture and the turn speed bar.
    pub pictures: Vec<ButtonPicture>,
    pub text_sliders: Vec<TextSlider>,
}

/// `0x81AD0` / `0x81B20`: the side arrow picture, texels 64..95 x 32..63 of the texture `[0x4D6C7C]` (1/16 texel
/// units 0x400..0x5F0 and 0x200..0x3F0 through `0x1B7B0`); `0x81B20` draws it mirrored (u from 0x5F0 to 0x400).
pub const ARROW_U: (f32, f32) = (64.0, 95.0);
pub const ARROW_V: (f32, f32) = (32.0, 63.0);

/// The side arrows of a kind-1 value row (`0x7B7B0`, drawn unless row `+0xB8` has bit 0x400000), MEASURED by static
/// reading. With the value text width `w` and `h = [0x7E7264] - 4`, the arrows and text take `total = w + 2h + 8`:
/// alignment `+0xC0` 1 or 4 starts at `anchor - total`, 2 centres the text in the value box (left arrow at
/// `text - h - 4`, right arrow at `text + w + 4`), else starts at `anchor`; the left arrow sits at the start and the
/// right one at `start + total - h`. Each arrow is an `h` x `h` square from `y + 2`.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueArrows {
    pub widget: WidgetId,
    /// `box x0 + +0xDC * ux` and `box y0 + +0xE0 * uy` (design units).
    pub anchor: f32,
    pub y: f32,
    pub box_x: (f32, f32),
    pub mode: u32,
    /// `[0x7E7264]` of the value style (the arrows are 4 smaller).
    pub height: f32,
    /// Colour table entries (`0x70D10(row, state)`, RGBA before the fade): state 0 unfocused, on the cursor row 1, or
    /// 2 while page `+0xC8` has bit 4 (left) or 8 (right), 3 greyed.
    pub left: u32,
    pub right: u32,
}

impl ValueArrows {
    /// Left edges of the left and right arrow for a value text `width` units wide.
    pub fn positions(&self, width: f32) -> (f32, f32) {
        let size = self.height - 4.0;
        let total = width + 2.0 * size + 8.0;
        match self.mode {
            2 => {
                let text = (self.box_x.0 + self.box_x.1) * 0.5 - width * 0.5;
                (text - size - 4.0, text + width + 4.0)
            }
            mode => {
                let start = if mode == 1 || mode == 4 {
                    self.anchor - total
                } else {
                    self.anchor
                };
                (start, start + total - size)
            }
        }
    }
}

/// Draw the side arrows of `arrows` (`0x81B20` left, mirrored, then `0x81AD0` right; colours `0x154FE0(colour, fade)`).
pub fn value_arrows(
    out: &mut Vec<Sprite>,
    arrows: &ValueArrows,
    width: f32,
    texture: TextureId,
    size: (f32, f32),
    viewport: Viewport,
    fade: f32,
) {
    let (left, right) = arrows.positions(width);
    let edge = arrows.height - 4.0;
    let (tw, th) = (size.0.max(1.0), size.1.max(1.0));
    let (u0, u1) = (ARROW_U.0 / tw, ARROW_U.1 / tw);
    let (v0, v1) = (ARROW_V.0 / th, ARROW_V.1 / th);
    for (x, colour, tex) in [
        (left, arrows.left, uv(u1, v0, u0, v1)),
        (right, arrows.right, uv(u0, v0, u1, v1)),
    ] {
        if let Some(px) =
            Edges::new(x, arrows.y + 2.0, x + edge, arrows.y + 2.0 + edge).to_px(viewport)
        {
            out.push(Sprite {
                rect_px: px,
                uv: tex,
                color: color(scale_alpha(colour, fade)),
                texture,
            });
        }
    }
}

/// `0x2EA080(page, row, rect)` (MEASURED by static reading): the inline button picture string `[0x478220]` (font
/// escape 0x0B '2': picture 1 of the run-time table `[0x7E72A0]`) in text style 3, or style 1 when `[0x79094C] > 100`
/// (and in some split-screen cases, not evaluated here); `x = x1 - trunc(w) * scale` (scale 2.0, 1.5 with style 1) with
/// `w` its measured width (`0x3FCF0`), `y = y0 + (h - trunc(text height)) * 0.6`. The text renderer (`0x40FA0`) draws
/// each picture escape as a square of the style height at the pen (`0x40260` -> `0x1B7B0`). The cursor row (page
/// `+0x198`) draws it through `0x41E30` (no picture in the outline passes, the fill in the first colour of `0x75F60`,
/// white at the text pulse), other rows through `0x41DC0` tinted [`PICTURE_DIM_RGB`].
///
/// T2316: the string is laid out by the general inline picture escapes of the text shaper
/// (`FontTableMeasure::inline_layout`), not as a single row sprite.
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonPicture {
    pub area: Edges,
    /// The picture string as the guest holds it (escapes included).
    pub text: Vec<u8>,
    /// Text style (font object) of the string.
    pub font: u32,
    /// Texture (texture table name or a guest image key) per handle table slot the string names.
    pub textures: Vec<(u8, String)>,
    /// Picture edge (the style's text height, 20 in text mode 0x66) and the text height, and the placement scale.
    pub size: f32,
    pub height: f32,
    pub scale: f32,
    pub cursor: bool,
}

/// The pictures of `p` (`0x2EA080`) laid out as `layout` (`inline_layout` of `p.text` at `p.size`), at text pulse
/// `pulse` (`0x75F60`, cursor row) and page fade `fade`. Glyphs of the string are not drawn by this row (the original
/// string holds only the picture escape).
pub fn button_picture(
    out: &mut Vec<Sprite>,
    p: &ButtonPicture,
    layout: &crate::font::InlineLayout,
    textures: &TextureLookup<'_>,
    viewport: Viewport,
    fade: f32,
    pulse: f32,
) {
    let x = p.area.x1 - layout.width.trunc() * p.scale;
    let y = p.area.y0 + (p.area.height() - p.height.trunc()) * 0.6;
    let colour = if p.cursor {
        PackedColor(0x7F7F_7F7F).with_alpha_scaled(fade * pulse)
    } else {
        color(PICTURE_DIM_RGB | alpha_byte(fade * 127.0))
    };
    for picture in &layout.pictures {
        let Some((_, name)) = p.textures.iter().find(|(slot, _)| *slot == picture.slot) else {
            continue;
        };
        let Some(texture) = textures(name) else {
            continue;
        };
        let left = x + picture.pen;
        if let Some(px) = Edges::new(left, y, left + picture.size, y + picture.size).to_px(viewport)
        {
            out.push(Sprite {
                rect_px: px,
                uv: uv(0.0, 0.0, 1.0, 1.0),
                color: colour,
                texture,
            });
        }
    }
}

/// `0x2EA250(page, row, rect)` (MEASURED by static reading): the Controls turn speed bar. Value `[0x7BA3DC + [0x7BA9E0]
/// * 0x1E0]` (f32, the profile's setting); strength the page fade, times the pulse `0x2E6430` on the cursor row (page
/// `+0x18C` equals the row id); colours as `0x2C3500`. Width `min(200, (x1 - x0) - w(label) - 25)`, `x = trunc(x1 -
/// width - 5)`, `y = trunc((h - H) / 2 + y0)`, height `trunc(H)` (H the label's text height), then `0x814B0`.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSlider {
    pub widget: WidgetId,
    pub area: Edges,
    pub value: f32,
    pub cursor: bool,
    /// The label's text height (`0x40620`).
    pub text_height: f32,
}

/// The bar of `s` (`0x2EA250`) for a label `label_width` units wide.
pub fn text_slider(
    out: &mut Vec<Sprite>,
    s: &TextSlider,
    rows: &OptionRows,
    label_width: f32,
    textures: &TextureLookup<'_>,
    viewport: Viewport,
    fade: f32,
    frame: u64,
) {
    text_slider_with_pulse(
        out,
        s,
        rows,
        label_width,
        textures,
        viewport,
        fade,
        rows.pulse.at(frame),
    )
}

/// Native presentation seam for an options text slider.
#[allow(clippy::too_many_arguments)]
pub fn text_slider_with_pulse(
    out: &mut Vec<Sprite>,
    s: &TextSlider,
    rows: &OptionRows,
    label_width: f32,
    textures: &TextureLookup<'_>,
    viewport: Viewport,
    fade: f32,
    pulse: f32,
) {
    let Some(bar) = &rows.bar else { return };
    let Some(texture) = textures(&bar.name) else {
        return;
    };
    let strength = if s.cursor { pulse * fade } else { fade };
    let available = s.area.width() - label_width - TEXT_SLIDER_GAP;
    let width = if available > TEXT_SLIDER_MAX {
        TEXT_SLIDER_MAX
    } else {
        available
    };
    let x = (s.area.x1 - width - TEXT_SLIDER_RIGHT) as i32;
    let y = ((s.area.height() - s.text_height) * 0.5 + s.area.y0) as i32;
    let [empty, from, to] = [BAR_EMPTY, BAR_FROM, BAR_TO].map(|c| scale_alpha(c, strength));
    bar_sprites(
        out,
        bar,
        texture,
        viewport,
        s.value,
        x,
        y,
        width as i32,
        s.text_height as i32,
        (empty, from, to),
    );
}

/// The sprites of one `0x814B0` bar.
#[allow(clippy::too_many_arguments)]
fn bar_sprites(
    out: &mut Vec<Sprite>,
    bar: &NamedTexture,
    texture: TextureId,
    viewport: Viewport,
    value: f32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    (empty, from, to): (u32, u32, u32),
) {
    for quad in bar_quads(value, width, empty, from, to, false) {
        let edges = Edges::new(
            (x + quad.x0) as f32,
            y as f32,
            (x + quad.x1) as f32,
            (y + height) as f32,
        );
        let Some(px) = edges.to_px(viewport) else {
            continue;
        };
        let (tw, th) = (bar.width.max(1.0) * 16.0, bar.height.max(1.0) * 16.0);
        let tex = uv(
            quad.u0 as f32 / tw,
            quad.v0 as f32 / th,
            quad.u1 as f32 / tw,
            quad.v1 as f32 / th,
        );
        if quad.left == quad.right {
            out.push(Sprite {
                rect_px: px,
                uv: tex,
                color: color(quad.left),
                texture,
            });
        } else {
            // One slice per two pixels at least: the original interpolates the vertex colours.
            let steps = RAMP_STEPS.max((px.size.x / 2.0).ceil() as usize);
            ramp_steps(
                out,
                texture,
                px,
                tex,
                color(quad.left),
                color(quad.right),
                true,
                steps,
            );
        }
    }
}

/// One textured quad of `0x814B0`: integer x range, 1/16-texel u and v ranges, left and right colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarQuad {
    pub x0: i32,
    pub x1: i32,
    pub u0: i32,
    pub u1: i32,
    pub v0: i32,
    pub v1: i32,
    pub left: u32,
    pub right: u32,
}

/// `ftol(a * (1 - v) + b * v)` per byte of two RGBA words (the colour at the fill end).
pub fn lerp_rgba(a: u32, b: u32, value: f32) -> u32 {
    let inverse = 1.0 - value;
    (0..4).fold(0, |word, byte| {
        let shift = 24 - 8 * byte;
        let (ca, cb) = (((a >> shift) & 0xFF) as f64, ((b >> shift) & 0xFF) as f64);
        let mixed = (ca * f64::from(inverse) + cb * f64::from(value)) as i64 as u32 & 0xFF;
        word | (mixed << shift)
    })
}

/// `0x154FE0(colour, f)`: the alpha byte times `f` clamped to 0..1, truncated.
pub fn scale_alpha(colour: u32, factor: f32) -> u32 {
    let factor = if factor > 1.0 {
        1.0
    } else if factor < 0.0 || factor.is_nan() {
        0.0
    } else {
        factor
    };
    (colour & 0xFFFF_FF00) | (((colour & 0xFF) as f32 * factor) as u32 & 0xFF)
}

/// `0x814B0(value, x, y, width, height, empty, from, to, mirrored)` (MEASURED by static reading): a bar of `width` units
/// in two parts, each drawn as left cap, middle and right cap (caps 8 units, the middle stretched; parts shorter than
/// the caps are cut). With `mirrored` false the fill is `[0, trunc(width * value))` (texture row 0..0x100, left cap in
/// `from`, the middle a gradient from `from` to the fill end colour `lerp(from, to, value)`, right cap in the fill end
/// colour, through `0x1BA70`) and the rest is empty (texture row 0x100..0x200, colour `empty`, through `0x1F090`). With
/// `mirrored` true the fill is the right part and its colours run from the fill end colour to `from`. `value` is
/// clamped to 0..1. The quads come out in draw order.
pub fn bar_quads(
    value: f32,
    width: i32,
    empty: u32,
    from: u32,
    to: u32,
    mirrored: bool,
) -> Vec<BarQuad> {
    let value = if value > 1.0 {
        1.0
    } else if value < 0.0 || value.is_nan() {
        0.0
    } else {
        value
    };
    let end = lerp_rgba(from, to, value);
    let (fill_start, fill_len, left, right) = if mirrored {
        let start = (width as f32 * (1.0 - value)) as i32;
        (start, width - start, end, from)
    } else {
        (0, (width as f32 * value) as i32, from, end)
    };
    let (empty_start, empty_len) = if mirrored {
        (0, fill_start)
    } else {
        (fill_len, width - fill_len)
    };
    let mut out = Vec::new();
    part(
        &mut out,
        fill_start,
        fill_len,
        width,
        (0, 0x100),
        (left, right),
    );
    part(
        &mut out,
        empty_start,
        empty_len,
        width,
        (0x100, 0x200),
        (empty, empty),
    );
    out.retain(|q| q.x1 > q.x0);
    out
}

/// One part of [`bar_quads`] from `start` of `len` units: the left cap when the part starts left of 8, the middle
/// when it starts left of `width - 8` and reaches 8, the right cap when it reaches `width - 8`.
fn part(
    out: &mut Vec<BarQuad>,
    start: i32,
    len: i32,
    width: i32,
    (v0, v1): (i32, i32),
    (left, right): (u32, u32),
) {
    let stop = start + len;
    let right_edge = width - BAR_CAP;
    if start < BAR_CAP {
        // The original keeps the part length (not its end) when the part ends left of the cap.
        let cap_end = if stop >= BAR_CAP { BAR_CAP } else { len };
        out.push(BarQuad {
            x0: start,
            x1: cap_end,
            u0: start * 16,
            u1: cap_end * 16,
            v0,
            v1,
            left,
            right: left,
        });
    }
    if start < right_edge && stop >= BAR_CAP {
        let m0 = start.max(BAR_CAP);
        let m1 = if stop >= right_edge { right_edge } else { stop };
        out.push(BarQuad {
            x0: m0,
            x1: m1,
            u0: 0x80,
            u1: 0x180,
            v0,
            v1,
            left,
            right,
        });
    }
    if stop >= right_edge {
        let r0 = start.max(right_edge);
        let u = |x: i32| (x - right_edge + 24) * 16;
        out.push(BarQuad {
            x0: r0,
            x1: stop,
            u0: u(r0),
            u1: u(stop),
            v0,
            v1,
            left: right,
            right,
        });
    }
}

/// The strength of a slider row (`0x2C3500`): greyed, pulsing cursor or 1, times the page fade.
pub fn slider_strength(slider: &Slider, rows: &OptionRows, fade: f32, frame: u64) -> f32 {
    slider_strength_with_pulse(slider, rows, fade, rows.pulse.at(frame))
}

pub fn slider_strength_with_pulse(
    slider: &Slider,
    rows: &OptionRows,
    fade: f32,
    pulse: f32,
) -> f32 {
    let base = if slider.greyed {
        rows.greyed_strength
    } else if slider.cursor {
        pulse
    } else {
        1.0
    };
    base * fade
}

fn color(word: u32) -> Color {
    PackedColor(word).to_color()
}

/// Draw the rows (`0x2C3500` / `0x2C3410` per row) and the page draw (`0x2C4190`). `fade` is page `+0xF4`, `frame`
/// the guest frame counter (the pulse clock), `white` a plain white texture (the frame lines).
pub fn draw_option_rows(
    rows: &OptionRows,
    textures: &TextureLookup<'_>,
    white: Option<TextureId>,
    viewport: Viewport,
    fade: f32,
    frame: u64,
) -> Vec<Sprite> {
    draw_option_rows_with_pulse(rows, textures, white, viewport, fade, rows.pulse.at(frame))
}

/// Native presentation seam for option-row slider bars.
pub fn draw_option_rows_with_pulse(
    rows: &OptionRows,
    textures: &TextureLookup<'_>,
    white: Option<TextureId>,
    viewport: Viewport,
    fade: f32,
    pulse: f32,
) -> Vec<Sprite> {
    let mut out = Vec::new();
    for row in &rows.rows {
        if let (Some(slider), Some(bar)) = (&row.slider, &rows.bar)
            && let Some(texture) = textures(&bar.name)
        {
            let strength = slider_strength_with_pulse(slider, rows, fade, pulse);
            let x = (row.area.x0 + BAR_LEFT) as i32;
            let y = ((row.area.y1 - row.area.y0) * 0.5 + row.area.y0) as i32;
            let width = (row.area.width() - BAR_SHORTER) as i32;
            let [empty, from, to] = [BAR_EMPTY, BAR_FROM, BAR_TO].map(|c| scale_alpha(c, strength));
            bar_sprites(
                &mut out,
                bar,
                texture,
                viewport,
                slider.value,
                x,
                y,
                width,
                BAR_HEIGHT,
                (empty, from, to),
            );
        }
        if let Some(icon) = &row.icon
            && let Some(texture) = textures(&icon.name)
        {
            let (x0, y0) = (row.area.x0 + 5.0, row.area.y0 + 5.0);
            if let Some(px) = Edges::new(x0, y0, x0 + 16.0, row.area.y0 + 20.0).to_px(viewport) {
                let alpha = alpha_byte(fade * 127.0);
                out.push(Sprite {
                    rect_px: px,
                    uv: uv(0.0, 0.0, 1.0, 1.0),
                    color: color(ICON_RGB | alpha),
                    texture,
                });
            }
        }
    }
    if let (Some(frame_lines), Some(white)) = (&rows.safe_frame, white) {
        let c = color(0x7F7F_7F00 | (frame_lines.alpha & 0xFF));
        let (l, r, t, b) = (
            frame_lines.left as f32,
            frame_lines.right as f32,
            frame_lines.top as f32,
            frame_lines.bottom as f32,
        );
        // `0x1B090` lines: top, bottom, left and right (the sides end one unit above the bottom line).
        for edges in [
            Edges::new(l, t, r, t + 1.0),
            Edges::new(l, b, r, b + 1.0),
            Edges::new(l, t, l + 1.0, b - 1.0),
            Edges::new(r, t, r + 1.0, b - 1.0),
        ] {
            if let Some(px) = edges.to_px(viewport) {
                out.push(Sprite {
                    rect_px: px,
                    uv: uv(0.0, 0.0, 1.0, 1.0),
                    color: c,
                    texture: white,
                });
            }
        }
    }
    out
}

/// Every texture name [`draw_option_rows`] may ask for.
pub fn texture_names(rows: &OptionRows) -> Vec<String> {
    let mut out: Vec<String> = rows.bar.iter().map(|b| b.name.clone()).collect();
    out.extend(
        rows.rows
            .iter()
            .filter_map(|r| r.icon.as_ref().map(|i| i.name.clone())),
    );
    out.extend(rows.arrow.iter().map(|a| a.name.clone()));
    out.extend(
        rows.pictures
            .iter()
            .flat_map(|p| p.textures.iter().map(|(_, name)| name.clone())),
    );
    out
}

#[cfg(test)]
mod tests {
    //! Tier 0: FABRICATED rows and values; constants are the MEASURED ones.
    use super::*;

    #[test]
    fn a_half_bar_is_a_fill_and_an_empty_part_with_caps() {
        let q = bar_quads(0.5, 200, 0x1111_1111, 0x2222_2222, 0x4444_4444, false);
        let ranges: Vec<_> = q.iter().map(|q| (q.x0, q.x1, q.u0, q.u1, q.v0)).collect();
        assert_eq!(
            ranges,
            vec![
                (0, 8, 0, 0x80, 0),
                (8, 100, 0x80, 0x180, 0),
                (100, 192, 0x80, 0x180, 0x100),
                (192, 200, 0x180, 0x200, 0x100)
            ],
            "{q:?}"
        );
        // Left cap in `from`, the middle from `from` to the fill end colour, the empty part flat.
        assert_eq!((q[0].left, q[0].right), (0x2222_2222, 0x2222_2222));
        assert_eq!((q[1].left, q[1].right), (0x2222_2222, 0x3333_3333));
        assert_eq!((q[3].left, q[3].right), (0x1111_1111, 0x1111_1111));
    }

    #[test]
    fn a_full_bar_ends_in_the_fill_cap_and_an_empty_bar_starts_with_the_empty_cap() {
        let full = bar_quads(1.5, 100, 1, 2, 3, false);
        assert_eq!(
            full.iter().map(|q| (q.x0, q.x1, q.v0)).collect::<Vec<_>>(),
            vec![(0, 8, 0), (8, 92, 0), (92, 100, 0)]
        );
        assert_eq!(full[2].u0, 0x180);
        let none = bar_quads(0.0, 100, 1, 2, 3, false);
        assert_eq!(
            none.iter().map(|q| (q.x0, q.x1, q.v0)).collect::<Vec<_>>(),
            vec![(0, 8, 0x100), (8, 92, 0x100), (92, 100, 0x100)]
        );
        // A fill shorter than the cap keeps its length as the cap end (the original's quirk).
        let short = bar_quads(0.05, 100, 1, 2, 3, false);
        assert_eq!((short[0].x0, short[0].x1), (0, 5));
        assert_eq!((short[1].x0, short[1].x1, short[1].u0), (5, 8, 0x50));
    }

    #[test]
    fn side_arrows_frame_the_value_text_by_alignment() {
        // Live Audio/Video numbers (MEASURED): mode 1, anchor 153.6 + 50 * 6.4; FABRICATED height and width.
        let mut a = ValueArrows {
            widget: WidgetId(0),
            anchor: 473.6,
            y: 127.0,
            box_x: (153.6, 486.4),
            mode: 1,
            height: 14.0,
            left: 0x1E2D_4646,
            right: 0x2D4B_697F,
        };
        // End aligned: total = 30 + 20 + 8 = 58; left at 473.6 - 58, right at the end - 10.
        let (l, r) = a.positions(30.0);
        assert!(
            (l - 415.6).abs() < 1e-3 && (r - 463.6).abs() < 1e-3,
            "{l} {r}"
        );
        a.mode = 2;
        let (l, r) = a.positions(30.0);
        assert!(
            (l - 291.0).abs() < 1e-3 && (r - 339.0).abs() < 1e-3,
            "{l} {r}"
        );
        a.mode = 0;
        assert_eq!(a.positions(30.0), (473.6, 473.6 + 48.0));
        let mut out = Vec::new();
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        a.mode = 1;
        value_arrows(
            &mut out,
            &a,
            30.0,
            TextureId(4),
            (128.0, 64.0),
            viewport,
            1.0,
        );
        assert_eq!(out.len(), 2);
        assert!(
            out[0].uv.min.x > out[0].uv.max.x,
            "the left arrow is the mirrored picture"
        );
        assert_eq!((out[1].uv.min.x, out[1].uv.min.y), (0.5, 0.5));
        assert_eq!(out[1].rect_px.size.x, 10.0);
    }

    #[test]
    fn the_button_picture_and_the_turn_speed_bar_sit_at_the_right_edge() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        // Controls row 0 (MEASURED rect 70.4, 104.73 .. 569.6, 128.655); FABRICATED style height 14.
        let area = Edges::new(70.4, 104.73, 569.6, 128.655);
        let mut out = Vec::new();
        let p = ButtonPicture {
            area,
            text: b"\x0b2".to_vec(),
            font: 3,
            textures: vec![(1, "b".into())],
            size: 14.0,
            height: 14.0,
            scale: 2.0,
            cursor: false,
        };
        let layout = crate::font::InlineLayout {
            glyphs: Vec::new(),
            pictures: vec![crate::font::InlinePicture {
                slot: 1,
                pen: 0.0,
                size: 14.0,
            }],
            width: 14.0,
        };
        let lookup = |name: &str| (name == "b").then_some(TextureId(1));
        button_picture(&mut out, &p, &layout, &lookup, viewport, 1.0, 1.0);
        let s = out[0];
        assert!((s.rect_px.origin.x - (569.6 - 28.0)).abs() < 1e-3);
        assert!((s.rect_px.origin.y - (104.73 + (23.925 - 14.0) * 0.6)).abs() < 1e-3);
        assert_eq!((s.rect_px.size.x, s.rect_px.size.y), (14.0, 14.0));
        assert!(s.color.0[0] < 0.4, "a row off the cursor dims the picture");
        // Turn speed: a short label leaves 200 units; the bar ends 5 left of the row.
        let rows = OptionRows {
            rows: Vec::new(),
            bar: Some(NamedTexture {
                name: "bar".into(),
                width: 32.0,
                height: 32.0,
            }),
            greyed_strength: 0.31,
            pulse: Pulse {
                base: 0.5,
                amplitude: 0.15,
                period: 60.0,
            },
            safe_frame: None,
            values: Vec::new(),
            arrow: None,
            pictures: Vec::new(),
            text_sliders: Vec::new(),
        };
        let slider = TextSlider {
            widget: WidgetId(9),
            area,
            value: 1.0,
            cursor: false,
            text_height: 14.0,
        };
        let lookup = |name: &str| (name == "bar").then_some(TextureId(2));
        let mut out = Vec::new();
        text_slider(&mut out, &slider, &rows, 100.0, &lookup, viewport, 1.0, 0);
        let (first, last) = (out.first().unwrap(), out.last().unwrap());
        assert_eq!(first.rect_px.origin.x, (569.6f32 - 200.0 - 5.0).trunc());
        assert_eq!(
            last.rect_px.origin.x + last.rect_px.size.x,
            (569.6f32 - 205.0).trunc() + 200.0
        );
        assert_eq!(first.rect_px.size.y, 14.0);
        // Display samples must affect the cursor bar, retaining geometry and RGB.
        let mut cursor = slider.clone();
        cursor.cursor = true;
        let mut guest = Vec::new();
        text_slider(&mut guest, &cursor, &rows, 100.0, &lookup, viewport, 1.0, 0);
        let mut display = Vec::new();
        text_slider_with_pulse(
            &mut display,
            &cursor,
            &rows,
            100.0,
            &lookup,
            viewport,
            1.0,
            0.25,
        );
        assert_eq!(guest.len(), display.len());
        for (a, b) in guest.iter().zip(&display) {
            assert_eq!(a.rect_px, b.rect_px);
            assert_eq!(a.uv, b.uv);
            assert_eq!(&a.color.0[..3], &b.color.0[..3]);
            assert!(b.color.0[3] < a.color.0[3]);
        }
    }

    #[test]
    fn colours_lerp_and_scale_like_the_original() {
        assert_eq!(lerp_rgba(0x1119_3C7F, 0x6973_7F7F, 0.0), 0x1119_3C7F);
        assert_eq!(lerp_rgba(0x1119_3C7F, 0x6973_7F7F, 1.0), 0x6973_7F7F);
        assert_eq!(lerp_rgba(0x0000_0000, 0x6400_0000, 0.5), 0x3200_0000);
        assert_eq!(scale_alpha(0x1119_3C7F, 0.31), 0x1119_3C27);
        assert_eq!(scale_alpha(0x1119_3C7F, 2.0), 0x1119_3C7F);
        let pulse = Pulse {
            base: 0.5,
            amplitude: 0.15,
            period: 60.0,
        };
        assert!((pulse.at(0) - 0.65).abs() < 1e-4);
        assert!((pulse.at(15) - 0.8).abs() < 1e-3);
    }

    #[test]
    fn rows_draw_bar_quads_and_the_icon() {
        let lookup = |name: &str| match name {
            "bar" => Some(TextureId(1)),
            "icon" => Some(TextureId(2)),
            _ => None,
        };
        let rows = OptionRows {
            rows: vec![OptionRow {
                area: Edges::new(100.0, 100.0, 400.0, 130.0),
                slider: Some(Slider {
                    value: 0.5,
                    greyed: false,
                    cursor: false,
                }),
                icon: Some(NamedTexture {
                    name: "icon".into(),
                    width: 16.0,
                    height: 16.0,
                }),
            }],
            bar: Some(NamedTexture {
                name: "bar".into(),
                width: 32.0,
                height: 32.0,
            }),
            greyed_strength: 0.31,
            pulse: Pulse {
                base: 0.5,
                amplitude: 0.15,
                period: 60.0,
            },
            safe_frame: Some(SafeFrame {
                left: 30,
                right: 610,
                top: 25,
                bottom: 455,
                alpha: 0x7F,
            }),
            values: Vec::new(),
            arrow: None,
            pictures: Vec::new(),
            text_sliders: Vec::new(),
        };
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let sprites = draw_option_rows(&rows, &lookup, Some(TextureId(9)), viewport, 1.0, 0);
        assert_eq!(
            sprites.iter().filter(|s| s.texture == TextureId(9)).count(),
            4,
            "four frame lines"
        );
        let icon = sprites
            .iter()
            .rev()
            .find(|s| s.texture == TextureId(2))
            .unwrap();
        assert_eq!(icon.texture, TextureId(2));
        assert_eq!((icon.rect_px.size.x, icon.rect_px.size.y), (16.0, 15.0));
        // Bar 45 right of the row, from the row centre, 20 high; the first quad is the left cap.
        let first = sprites[0];
        assert_eq!(
            (
                first.rect_px.origin.x,
                first.rect_px.origin.y,
                first.rect_px.size.x
            ),
            (145.0, 115.0, 8.0)
        );
        assert!(
            sprites.len() > 4,
            "the middle of the fill is a gradient ramp"
        );
    }
}
