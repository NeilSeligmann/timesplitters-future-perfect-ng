// SPDX-License-Identifier: GPL-3.0-or-later
//! T1916 increment 2: the framework's default page style, the look of pages whose style record
//! (`page +0x1C`) has no frame or highlight callback (`+0xC4`, `+0xD8` both 0). The Map Maker
//! editor pages use it (MEASURED live, docs/t-system-mapmaker-ui.md "Native Map Maker UI").
//!
//! The original page draw `0x76810` then draws, in this order (MEASURED from the lift):
//!
//! 1. **Frame**: one quad `0x1A6F0` over the page box (`page +0xE4..+0xF0`) with four corner
//!    colours from the page colour block (`[page +0x20] +0x50..+0x5C`), each alpha-scaled by the page
//!    fade (`0x154FE0(colour, page +0xF4)`), skipped when the style word (`0x80430(page)`) has bit
//!    0x200. [`FrameColours`].
//! 2. **Rows**: per row the focused-row highlight `0x72B70(page, row, rect)` for the cursor row (not
//!    when row `+0x28 & 0x100000`), then the row text `0x76150`. [`Highlight`], [`TextColours`].
//! 3. **Pointer**: the pointer sprite (texture `page +0x4C`, `[0x6FEBAC]`, quad `0x1F090`, colour
//!    0x7F7F7F7F) when the UI context's `+0xF8` is 1 or 3. [`PointerSprite`].
//!
//! The highlight `0x72B70`: a bar over the row (or over the page's full width, page flags `+0x40`
//! bit 0x400) in the colour block's `+0x74`, whose alpha pulses with the frame counter (the sine
//! table `0x7A38A0`), and two edge lines, `+0x78` one unit under the row top and `+0x7C` two units
//! over the row bottom. Values without a code source are labelled INFERRED and were fitted on the
//! legacy picture of the editor menu (doc, increment 2).
use crate::bridge::{RenderLayout, TextPlacement};
use crate::list_rows::Edges;
use crate::model::{Align, Page, WidgetId};
use crate::skin::{PackedColor, SkinnedPage};
use tsfp_ui_renderer::{BlendMode, Color, Rect, Sprite, TextureId, UvRect, Vec2, Viewport};

/// Frame corner colours (`[page +0x20] +0x50..+0x5C`, in `0x1A6F0` argument order: top-left,
/// top-right, bottom-left, bottom-right; the order is INFERRED from the gradient direction).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameColours(pub [PackedColor; 4]);

/// The focused-row highlight of `0x72B70`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlight {
    /// Bar colour (`+0x74`).
    pub bar: PackedColor,
    /// Edge lines (`+0x78` top, `+0x7C` bottom).
    pub edge_top: PackedColor,
    pub edge_bottom: PackedColor,
    /// Full-width bar (page flags `+0x40` bit 0x400): the bar's x range in design units, else the
    /// row's own x range.
    pub full_width: Option<(f32, f32)>,
    /// Pulse: alpha factor `base + amplitude * (sin(phase) + 1)`, one period in `period` frames.
    pub pulse_base: f32,
    pub pulse_amplitude: f32,
    pub pulse_period: f32,
}

impl Highlight {
    /// The pulse factor at guest frame `frame`.
    pub fn pulse(&self, frame: u64) -> f32 {
        if !(self.pulse_period.is_finite() && self.pulse_period > 0.0) {
            return (self.pulse_base + 2.0 * self.pulse_amplitude).clamp(0.0, 1.0);
        }
        let phase = (frame as f32 % self.pulse_period) / self.pulse_period * std::f32::consts::TAU;
        (self.pulse_base + self.pulse_amplitude * (phase.sin() + 1.0)).clamp(0.0, 1.0)
    }
}

/// Row text colours of the default style (`0x76150` with this style's colour pair).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextColours {
    pub normal: Color,
    pub focused: Color,
    /// Greyed rows (`+0x28` bit 0): the text is drawn dark, not at half alpha like the front-end
    /// skin (MEASURED on the legacy editor menu: Preview Map, Undo, Redo).
    pub greyed: Color,
}

/// The pointer sprite: where (design units, the tip) and how big, and its texture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSprite {
    pub texture: TextureId,
    /// Sprite size in design units (the texture's size; INFERRED scale, see the doc).
    pub size: (f32, f32),
    /// Tip offset inside the sprite in design units (the sprite's top-left is `pos - hotspot`).
    pub hotspot: (f32, f32),
}

/// Everything the default style needs from the live page (read by
/// `tsfp_ui_input::default_style_live`).
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultStyle {
    /// The page box in design units (`page +0xE4..+0xF0`).
    pub page_box: Edges,
    /// `None` when the style word skips the frame (bit 0x200).
    pub frame: Option<FrameColours>,
    pub highlight: Highlight,
    pub text: TextColours,
    /// Text x inset from the row's left edge in design units (left-aligned rows, INFERRED: legacy
    /// text starts 10.4 units into the row box).
    pub text_inset: f32,
    /// Text moved down by this many design units (the front-end skin's `text_drop` mechanism, INFERRED).
    pub text_drop: f32,
    /// Greyed rows (row `+0x28` bit 0), drawn in [`TextColours::greyed`]. The rows stay focusable (the model's
    /// `enabled` is not used for this, the original hovers greyed rows too).
    pub greyed: Vec<WidgetId>,
}

/// The `original` preset's text values of the default style (INFERRED, measured on the legacy editor menu picture of
/// increment 1, `legacy-menu/shot.p1800.png`: normal glyphs 217,229,241, the focused row 189,185,198 over the bar,
/// greyed rows near black; the text starts 10.4 units into the row box).
pub const TEXT_NORMAL: Color = Color([217.0 / 255.0, 229.0 / 255.0, 241.0 / 255.0, 1.0]);
pub const TEXT_FOCUSED: Color = Color([189.0 / 255.0, 185.0 / 255.0, 198.0 / 255.0, 1.0]);
pub const TEXT_GREYED: Color = Color([15.0 / 255.0, 15.0 / 255.0, 18.0 / 255.0, 1.0]);
pub const TEXT_INSET: f32 = 10.4;
pub const TEXT_DROP: f32 = 2.5;

/// The pointer picture read from the guest (`page +0x4C`) and its placement.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerImage {
    pub image: crate::list_rows::GuestImage,
    pub size: (f32, f32),
    pub hotspot: (f32, f32),
}

/// A default-style page as the in-frame layer gets it.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultPage {
    pub style: DefaultStyle,
    pub pointer: Option<PointerImage>,
    /// T1916 increment 3: the page's row and widget draws (`crate::default_widgets`).
    pub widgets: crate::default_widgets::Widgets,
}

/// What [`style_page`] needs from the renderer side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefaultTextures {
    /// A plain white texture (`filled16x16`, table entry 0).
    pub white: TextureId,
}

const FULL_UV: UvRect = UvRect {
    min: Vec2 { x: 0.0, y: 0.0 },
    max: Vec2 { x: 1.0, y: 1.0 },
};

/// Gradient slices of a four-corner quad.
pub const GRADIENT_STEPS: usize = 32;

fn mix(a: Color, b: Color, t: f32) -> Color {
    let mut out = [0.0; 4];
    for (i, o) in out.iter_mut().enumerate() {
        *o = a.0[i] + (b.0[i] - a.0[i]) * t;
    }
    Color(out)
}

/// A quad with four corner colours (`0x1A6F0`), as horizontal bands of a bilinear blend (each
/// band split in two halves along x when the left and right colours differ).
pub fn corner_gradient(out: &mut Vec<Sprite>, texture: TextureId, area: Rect, corners: [Color; 4]) {
    let [tl, tr, bl, br] = corners;
    let columns = if tl == tr && bl == br {
        1
    } else {
        GRADIENT_STEPS
    };
    for row in 0..GRADIENT_STEPS {
        let (v0, v1) = (
            row as f32 / GRADIENT_STEPS as f32,
            (row + 1) as f32 / GRADIENT_STEPS as f32,
        );
        let v = (v0 + v1) * 0.5;
        let (left, right) = (mix(tl, bl, v), mix(tr, br, v));
        for column in 0..columns {
            let (u0, u1) = (
                column as f32 / columns as f32,
                (column + 1) as f32 / columns as f32,
            );
            let color = mix(left, right, (u0 + u1) * 0.5);
            let rect = Rect::new(
                Vec2::new(
                    area.origin.x + area.size.x * u0,
                    area.origin.y + area.size.y * v0,
                ),
                Vec2::new(area.size.x * (u1 - u0), area.size.y * (v1 - v0)),
            );
            if let Ok(rect_px) = rect {
                out.push(Sprite {
                    rect_px,
                    uv: FULL_UV,
                    color,
                    texture,
                });
            }
        }
    }
}

fn solid(out: &mut Vec<Sprite>, texture: TextureId, area: Option<Rect>, color: Color) {
    if let Some(rect_px) = area {
        out.push(Sprite {
            rect_px,
            uv: FULL_UV,
            color,
            texture,
        });
    }
}

/// The colour block words carry no alpha (low byte 0, MEASURED live: bar 0x32283700); `0x72B70` ors in 0x7F.
fn opaque(color: PackedColor) -> PackedColor {
    PackedColor(color.0 | 0x7F)
}

/// The focused-row highlight of `0x72B70` over `row` (design units), across `span` (x0, x1): the bar is additive
/// (blend 8, MEASURED from the lift; the legacy picture shows the panel plus the bar colour times the pulse), the two
/// edge lines alpha blended (blend 6). Returns the index of the first edge line (where the blend goes back to alpha).
pub fn highlight(
    out: &mut Vec<Sprite>,
    texture: TextureId,
    style: &Highlight,
    row: Edges,
    span: (f32, f32),
    viewport: Viewport,
    fade: f32,
    frame: u64,
) -> usize {
    highlight_with_pulse(
        out,
        texture,
        style,
        row,
        span,
        viewport,
        fade,
        style.pulse(frame),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn highlight_with_pulse(
    out: &mut Vec<Sprite>,
    texture: TextureId,
    style: &Highlight,
    row: Edges,
    span: (f32, f32),
    viewport: Viewport,
    fade: f32,
    pulse: f32,
) -> usize {
    let (x0, x1) = style.full_width.unwrap_or(span);
    // Lines at the truncated row top (one unit) and two units over the truncated row bottom (two units; `0x822E0` takes
    // integers); the bar fills between them (MEASURED on the legacy picture: bar rows end where the bottom line starts).
    let top = row.y0.trunc();
    let bottom = (row.y1 - 2.0).trunc();
    let bar = Edges::new(x0, row.y0, x1, bottom);
    solid(
        out,
        texture,
        bar.to_px(viewport),
        opaque(style.bar).with_alpha_scaled(fade * pulse),
    );
    let lines = out.len();
    solid(
        out,
        texture,
        Edges::new(x0, top, x1, top + 1.0).to_px(viewport),
        opaque(style.edge_top).with_alpha_scaled(fade),
    );
    solid(
        out,
        texture,
        Edges::new(x0, bottom, x1, bottom + 2.0).to_px(viewport),
        opaque(style.edge_bottom).with_alpha_scaled(fade),
    );
    lines
}

/// The pointer sprite at `pos` (design units), drawn last (after the text).
pub fn pointer(
    pointer: &PointerSprite,
    pos: (f32, f32),
    viewport: Viewport,
    fade: f32,
) -> Option<Sprite> {
    let x0 = pos.0 - pointer.hotspot.0;
    let y0 = pos.1 - pointer.hotspot.1;
    let rect_px = Edges::new(x0, y0, x0 + pointer.size.0, y0 + pointer.size.1).to_px(viewport)?;
    Some(Sprite {
        rect_px,
        uv: FULL_UV,
        color: PackedColor(0x7F7F_7F7F).with_alpha_scaled(fade),
        texture: pointer.texture,
    })
}

/// Style one page with the default style: the frame, the focused-row highlight, and the row texts
/// left-aligned and coloured (focused, normal, greyed). The pointer is separate ([`pointer`]),
/// it goes over the text.
#[allow(clippy::too_many_arguments)]
pub fn style_page(
    style: &DefaultStyle,
    ids: DefaultTextures,
    page: &Page,
    layout: &RenderLayout,
    viewport: Viewport,
    focused: Option<WidgetId>,
    fade: f32,
    frame: u64,
) -> Option<SkinnedPage> {
    style_page_with_pulse(
        style,
        ids,
        page,
        layout,
        viewport,
        focused,
        fade,
        style.highlight.pulse(frame),
    )
}

/// Native presentation seam for the default-style highlight bar.
#[allow(clippy::too_many_arguments)]
pub fn style_page_with_pulse(
    style: &DefaultStyle,
    ids: DefaultTextures,
    page: &Page,
    layout: &RenderLayout,
    viewport: Viewport,
    focused: Option<WidgetId>,
    fade: f32,
    pulse: f32,
) -> Option<SkinnedPage> {
    let page_rect = style.page_box.to_px(viewport)?;
    let mut sprites = Vec::new();
    if let Some(FrameColours(corners)) = style.frame {
        corner_gradient(
            &mut sprites,
            ids.white,
            page_rect,
            corners.map(|c| c.with_alpha_scaled(fade)),
        );
    }
    let rows_end = sprites.len();
    let mut blends = Vec::new();
    let unit = viewport.scale();
    let safe = viewport.safe_area().origin;
    let focused_row =
        focused.and_then(|id| page.widgets.iter().find(|w| w.id == id && w.is_focusable()));
    if let Some(row) = focused_row.and_then(|w| viewport.resolve(w.rect).ok()) {
        // Back to design units (the page rows resolve in the 4:3 safe area).
        let edges = Edges::new(
            (row.origin.x - safe.x) / unit,
            (row.origin.y - safe.y) / unit,
            (row.origin.x + row.size.x - safe.x) / unit,
            (row.origin.y + row.size.y - safe.y) / unit,
        );
        // The bar spans the page box, not the row box (INFERRED from the legacy picture: 352..640 against the rows'
        // 361.6..614.4; the lift's x range comes from the viewport query `0x19FC0`, not decoded).
        let at = sprites.len();
        let lines = highlight_with_pulse(
            &mut sprites,
            ids.white,
            &style.highlight,
            edges,
            (style.page_box.x0, style.page_box.x1),
            viewport,
            fade,
            pulse,
        );
        blends.push((at, BlendMode::Additive));
        blends.push((lines, BlendMode::Alpha));
    }
    let highlight_end = sprites.len();
    let texts = layout
        .texts
        .iter()
        .map(|placement| {
            let widget = page.widgets.iter().find(|w| w.id == placement.widget);
            let base = if focused == Some(placement.widget) {
                style.text.focused
            } else if style.greyed.contains(&placement.widget) {
                style.text.greyed
            } else {
                style.text.normal
            };
            let mut color = base;
            color.0[3] *= fade;
            // T1916 increment 3: the row's own text placement of `0x76150` (row `+0x30` align, `+0x3C` x, `+0x40` y,
            // MEASURED by T2315) when the row has one; the INFERRED inset and drop otherwise.
            if let Some(anchor) = widget.and_then(|w| w.text_anchor) {
                return crate::skin::anchored_placement(placement, anchor, unit, color);
            }
            let inset = style.text_inset * unit;
            let rect_px = Rect::new(
                Vec2::new(
                    placement.rect_px.origin.x + inset,
                    placement.rect_px.origin.y + style.text_drop * unit,
                ),
                Vec2::new(
                    (placement.rect_px.size.x - inset).max(1.0),
                    placement.rect_px.size.y,
                ),
            )
            .unwrap_or(placement.rect_px);
            TextPlacement {
                color,
                align: Align::Start,
                rect_px,
                ..placement.clone()
            }
        })
        .collect();
    Some(SkinnedPage {
        sprites,
        blends,
        layout: RenderLayout {
            quads: Vec::new(),
            texts,
        },
        page_rect,
        fade,
        rows_end,
        highlight_end,
        panel: page_rect,
        panel_radius: 0.0,
    })
}

#[cfg(test)]
mod tests {
    //! Tier 0 with FABRICATED pages and colours.
    use super::*;
    use crate::model::{Widget, WidgetKind, menu_rect};

    fn style(frame: bool) -> DefaultStyle {
        DefaultStyle {
            page_box: Edges::new(352.0, 52.0, 640.0, 425.0),
            frame: frame.then_some(FrameColours([
                PackedColor(0x1020_307F),
                PackedColor(0x1020_307F),
                PackedColor(0x0000_007F),
                PackedColor(0x0000_007F),
            ])),
            highlight: Highlight {
                bar: PackedColor(0x6050_7F00),
                edge_top: PackedColor(0x7F7F_7F7F),
                edge_bottom: PackedColor(0x1010_107F),
                full_width: Some((352.0, 640.0)),
                pulse_base: 0.5,
                pulse_amplitude: 0.25,
                pulse_period: 60.0,
            },
            text: TextColours {
                normal: Color([1.0; 4]),
                focused: Color([0.7, 0.7, 0.8, 1.0]),
                greyed: Color([0.05, 0.05, 0.05, 1.0]),
            },
            text_inset: 10.0,
            text_drop: 0.0,
            greyed: vec![WidgetId(2)],
        }
    }

    fn page() -> Page {
        let mut a = Widget::new(WidgetId(1), WidgetKind::Button);
        a.rect = menu_rect(Vec2::new(361.6, 65.28), Vec2::new(252.8, 28.8));
        let mut b = Widget::new(WidgetId(2), WidgetKind::Button);
        b.rect = menu_rect(Vec2::new(361.6, 94.08), Vec2::new(252.8, 28.8));
        Page {
            widgets: vec![a, b],
            ..Page::new(crate::model::PageId(7), 1)
        }
    }

    fn layout(page: &Page, viewport: Viewport) -> RenderLayout {
        RenderLayout {
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
        }
    }

    #[test]
    fn display_pulse_changes_only_the_default_bar_alpha() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let page = page();
        let style = style(true);
        let layout = layout(&page, viewport);
        let ids = DefaultTextures {
            white: TextureId(9),
        };
        let original = style_page(
            &style,
            ids,
            &page,
            &layout,
            viewport,
            Some(WidgetId(1)),
            0.75,
            0,
        )
        .unwrap();
        let sampled = style_page_with_pulse(
            &style,
            ids,
            &page,
            &layout,
            viewport,
            Some(WidgetId(1)),
            0.75,
            0.5,
        )
        .unwrap();
        assert_eq!(original.layout.texts, sampled.layout.texts);
        assert_eq!(original.blends, sampled.blends);
        let bar = original.rows_end;
        for (i, (a, b)) in original.sprites.iter().zip(&sampled.sprites).enumerate() {
            assert_eq!(a.rect_px, b.rect_px);
            assert_eq!(a.uv, b.uv);
            assert_eq!(a.texture, b.texture);
            assert_eq!(&a.color.0[..3], &b.color.0[..3]);
            if i == bar {
                assert_ne!(a.color.0[3], b.color.0[3]);
            } else {
                assert_eq!(a.color, b.color);
            }
        }
    }

    #[test]
    fn highlight_spans_the_full_width_and_texts_are_left_aligned() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let page = page();
        let ids = DefaultTextures {
            white: TextureId(9),
        };
        let styled = style_page(
            &style(false),
            ids,
            &page,
            &layout(&page, viewport),
            viewport,
            Some(WidgetId(1)),
            1.0,
            0,
        )
        .unwrap();
        // No frame: the bar (additive) and the two edge lines (alpha).
        assert_eq!(styled.rows_end, 0);
        assert_eq!(styled.sprites.len(), 3);
        assert_eq!(
            styled.blends,
            vec![(0, BlendMode::Additive), (1, BlendMode::Alpha)]
        );
        let bar = styled.sprites[0].rect_px;
        assert!((bar.origin.x - 352.0).abs() < 1e-3 && (bar.size.x - 288.0).abs() < 1e-3);
        assert!((bar.origin.y - 65.28).abs() < 1e-3);
        // Pulse at frame 0: base + amplitude; the colour words' zero alpha byte is made opaque first.
        assert!((styled.sprites[0].color.0[3] - 190.0 / 255.0).abs() < 1e-3);
        assert_eq!(
            styled.sprites[1].rect_px.origin.y, 65.0,
            "top line at the truncated row top"
        );
        let (focused, greyed) = (&styled.layout.texts[0], &styled.layout.texts[1]);
        assert_eq!(focused.align, Align::Start);
        assert!((focused.rect_px.origin.x - 371.6).abs() < 1e-3);
        assert_eq!(focused.color.0[2], 0.8);
        assert_eq!(greyed.color.0[0], 0.05);
    }

    #[test]
    fn frame_gradient_runs_top_to_bottom_and_fades() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let page = page();
        let ids = DefaultTextures {
            white: TextureId(9),
        };
        let styled = style_page(
            &style(true),
            ids,
            &page,
            &layout(&page, viewport),
            viewport,
            None,
            0.5,
            0,
        )
        .unwrap();
        assert_eq!(
            styled.rows_end, GRADIENT_STEPS,
            "uniform rows: one band per step"
        );
        let (first, last) = (
            styled.sprites[0].color,
            styled.sprites[GRADIENT_STEPS - 1].color,
        );
        assert!(first.0[2] > last.0[2]);
        assert!(
            (first.0[3] - 126.0 / 255.0).abs() < 1e-3,
            "alpha scaled by the fade"
        );
        assert_eq!(
            styled.highlight_end, styled.rows_end,
            "no focus, no highlight"
        );
    }

    #[test]
    fn pointer_sits_at_the_tip() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let p = PointerSprite {
            texture: TextureId(3),
            size: (16.0, 16.0),
            hotspot: (2.0, 1.0),
        };
        let s = pointer(&p, (100.0, 50.0), viewport, 1.0).unwrap();
        assert_eq!((s.rect_px.origin.x, s.rect_px.origin.y), (98.0, 49.0));
        assert_eq!(s.color.0[3], 254.0 / 255.0);
    }

    #[test]
    fn pulse_without_period_is_the_peak() {
        let mut h = style(false).highlight;
        h.pulse_period = 0.0;
        assert_eq!(h.pulse(5), 1.0);
        h.pulse_period = 60.0;
        assert!((h.pulse(15) - 1.0).abs() < 1e-3);
        assert!((h.pulse(45) - 0.5).abs() < 1e-3);
    }
}
