// SPDX-License-Identifier: GPL-3.0-or-later
//! T1916 readable editor Controls callback (0x314A50): runtime pictures, curved
//! labels, connectors and Rust-owned animation. The surrounding framework stays
//! at its hybrid boundary; alternate layouts retain their original callback.
use crate::list_rows::{Edges, TextureLookup, alpha_byte};
use crate::skin::PackedColor;
use tsfp_ui_renderer::{Sprite, UvRect, Vec2, Viewport};

pub const PAGE_DRAW: u32 = 0x31_4A50;
pub const PICTURE_TEXTURES: [u32; 2] = [0x2A4, 0x2A3];

/// Rust-owned picture geometry in the original 640x480 design space.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub texture: String,
    pub area: Edges,
    pub tint_rgb: u32,
}

/// All runtime inputs to the two-picture draw, independent of guest memory.
#[derive(Clone, Debug, PartialEq)]
pub struct ControllerPictures {
    pub centre: Vec2,
    pub fade: f32,
    pub pictures: [Picture; 2],
}

impl ControllerPictures {
    /// 0x2E42F0 truncates the full width, then truncates the scaled shadow
    /// width, then uses integer division for each half-width. Y vertical_radii are floats.
    pub fn new(
        centre: Vec2,
        fade: f32,
        width: f32,
        shadow_scale: f32,
        vertical_radii: [f32; 2],
        names: [String; 2],
    ) -> Option<Self> {
        if ![
            centre.x,
            centre.y,
            fade,
            width,
            shadow_scale,
            vertical_radii[0],
            vertical_radii[1],
        ]
        .iter()
        .all(|v| v.is_finite())
            || width < 0.0
            || shadow_scale < 0.0
            || vertical_radii.iter().any(|v| *v < 0.0)
            || names.iter().any(String::is_empty)
        {
            return None;
        }
        // The original cvttss2si operands must be representable before division.
        if width >= i32::MAX as f32 || width.trunc() * shadow_scale >= i32::MAX as f32 {
            return None;
        }
        let h = width as i32;
        let widths = [(h as f32 * shadow_scale) as i32, h];
        let pictures = std::array::from_fn(|i| {
            let half = (widths[i] / 2) as f32;
            Picture {
                texture: names[i].clone(),
                area: Edges::new(
                    centre.x - half,
                    centre.y - vertical_radii[i],
                    centre.x + half,
                    centre.y + vertical_radii[i],
                ),
                tint_rgb: [0x2D4B_6900, 0x7F7F_7F00][i],
            }
        });
        Some(Self {
            centre,
            fade,
            pictures,
        })
    }

    /// Alpha-blended sprites, shadow first, with the original byte alpha.
    pub fn draw(&self, lookup: &TextureLookup<'_>, viewport: Viewport) -> Vec<Sprite> {
        let alpha = alpha_byte(self.fade * 127.0);
        self.pictures
            .iter()
            .filter_map(|p| {
                Some(Sprite {
                    rect_px: p.area.to_px(viewport)?,
                    texture: lookup(&p.texture)?,
                    color: PackedColor(p.tint_rgb | alpha).to_color(),
                    uv: UvRect {
                        min: Vec2::new(0.0, 0.0),
                        max: Vec2::new(1.0, 1.0),
                    },
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_renderer::{Rect, TextureId};
    #[test]
    fn truncates_before_integer_half_width_and_keeps_painter_order() {
        let p = ControllerPictures::new(
            Vec2::new(320.0, 240.0),
            0.5,
            101.9,
            1.25,
            [80.0, 70.0],
            ["shadow".into(), "pad".into()],
        )
        .unwrap();
        assert_eq!(p.pictures[0].area, Edges::new(257.0, 160.0, 383.0, 320.0));
        assert_eq!(p.pictures[1].area, Edges::new(270.0, 170.0, 370.0, 310.0));
        let v = Viewport::new(
            Vec2::new(1280.0, 960.0),
            2.0,
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(1280.0, 960.0)).unwrap(),
        )
        .unwrap();
        let s = p.draw(&|n| Some(TextureId(if n == "shadow" { 1 } else { 2 })), v);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].texture, TextureId(1));
        assert_eq!(s[1].texture, TextureId(2));
        assert_eq!(s[0].color, PackedColor(0x2D4B_693F).to_color());
        assert_eq!(s[1].rect_px.origin, Vec2::new(540.0, 340.0));
    }
    #[test]
    fn rejects_nonfinite_and_out_of_range_geometry() {
        for h in [f32::NAN, f32::INFINITY, -1.0, i32::MAX as f32] {
            assert!(
                ControllerPictures::new(
                    Vec2::new(0.0, 0.0),
                    1.0,
                    h,
                    1.0,
                    [1.0, 1.0],
                    ["a".into(), "b".into()]
                )
                .is_none()
            );
        }
    }
}

/// The editor-layout groups selected by the fourth argument of 0x2E4290.
/// `(first, count, outline, text_on_upper_edge)`; order is painter order.
pub const EDITOR_GROUPS: [(usize, usize, usize, bool); 6] = [
    (0, 6, 1, true),
    (6, 3, 2, true),
    (9, 2, 5, true),
    (11, 2, 4, true),
    (13, 2, 0, false),
    (15, 3, 3, false),
];

/// The original record mixes a float fraction with two signed integer indices.
/// In particular, a segment is not an f32 despite the decompiler's pointer type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveCell {
    pub fraction: f32,
    pub target: i32,
    pub segment: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CurveAnimation {
    pub cells: Vec<CurveCell>,
    pub outlines: [f32; 6],
}
impl CurveAnimation {
    /// 0x2E0A20 advances at most ONE segment per draw, with a strict >1 test.
    /// No elapsed-time catch-up loop: a large original delta also advances once.
    pub fn advance(&mut self, step: f32) {
        for cell in &mut self.cells {
            if cell.segment < cell.target {
                cell.fraction += step;
                if cell.fraction > 1.0 {
                    cell.fraction -= 1.0;
                    cell.segment += 1;
                    if cell.segment == cell.target {
                        cell.fraction = 0.0;
                    }
                }
            }
        }
        for (first, count, group, upper) in EDITOR_GROUPS {
            let edge = first + if upper { count - 1 } else { 0 };
            let c = &self.cells[edge];
            if c.segment == c.target && self.outlines[group] > 0.0 {
                self.outlines[group] += step * 2.0;
                if self.outlines[group] > 28.0 {
                    self.outlines[group] = 0.0;
                }
            }
        }
    }
}

/// Pure inline icon labels use the shared 0x40FA0 animated-icon contract.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveIcon {
    pub texture: Option<String>,
    pub uv: UvRect,
    pub size: f32,
    pub advance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CurveLabel {
    pub label_icon: Option<CurveIcon>,
    pub value_icon: Option<CurveIcon>,
    pub label: crate::default_widgets::WidgetText,
    pub value: crate::default_widgets::WidgetText,
}

/// Rust-owned original editor diagram. Runtime acquisition is separate; no guest
/// writes, original calls, synthetic input, or embedded text/assets in this model.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlsDiagram {
    pub pictures: ControllerPictures,
    pub animation: CurveAnimation,
    /// Includes the preceding point: element 1 is original 0x789660.
    pub points: Vec<Vec2>,
    pub connector_segments: [usize; 6],
    pub labels: Vec<CurveLabel>,
    pub unit_x: f32,
    pub scale_x: f32,
    pub line_height: f32,
    /// The original global colours, before the draw replaces their high byte.
    pub cell_colours: [u32; 2],
    /// Runtime RGB bytes of the outline ramps.
    pub outline_rgb: [u8; 3],
}

fn lerp(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

impl ControlsDiagram {
    pub fn geometry(&self, cell: CurveCell) -> Option<[Vec2; 3]> {
        let s = usize::try_from(cell.segment).ok()?;
        let prev = *self.points.get(s)?;
        let a = *self.points.get(s + 1)?;
        let b = *self.points.get(s + 2)?;
        let next = *self.points.get(s + 3)?;
        let top = lerp(a, b, cell.fraction);
        let bottom = lerp(b, next, cell.fraction);
        let middle = Vec2::new(
            (top.x + bottom.x) * 0.565 - (prev.x + next.x) * 0.065,
            top.y + (bottom.y - top.y) * 0.5,
        );
        Some([top, middle, bottom])
    }

    /// Sample each horizontal-edge quad into one-design-pixel bands and 16
    /// colour columns. This preserves painter order through the existing Sprite
    /// renderer contract. Gradient/edge sampling is explicit verification debt.
    pub fn draw(
        &self,
        white: tsfp_ui_renderer::TextureId,
        textures: &TextureLookup<'_>,
        width: &crate::default_widgets::TextWidth<'_>,
        viewport: Viewport,
    ) -> crate::list_rows::ListDraw {
        use crate::bridge::TextPlacement;
        use crate::list_rows::ListDraw;
        use crate::model::{Align, TextRef, WidgetId};
        let mut out = ListDraw::default();
        out.sprites = self.pictures.draw(textures, viewport);
        let fade = self.pictures.fade;
        let byte = (alpha_byte(fade * 64.0) as u32) << 24;
        let colours = self
            .cell_colours
            .map(|c| curve_colour((c & 0xFFFFFF) | byte));
        for (first, count, group, upper) in EDITOR_GROUPS {
            for i in first..first + count {
                let Some([top, middle, bottom]) = self.geometry(self.animation.cells[i]) else {
                    continue;
                };
                for (a, b) in [(top, middle), (middle, bottom)] {
                    gradient_quad(&mut out.sprites, white, a, b, 105.0, colours, viewport);
                    gradient_quad(
                        &mut out.sprites,
                        white,
                        Vec2::new(a.x + 105.0, a.y),
                        Vec2::new(b.x + 105.0, b.y),
                        105.0,
                        [colours[1], colours[0]],
                        viewport,
                    );
                }
                let anchor = if upper { top } else { bottom };
                let row = &self.labels[i];
                let label_width = row
                    .label_icon
                    .as_ref()
                    .map_or_else(
                        || width(row.label.text, &row.label.bytes, 0x12),
                        |i| i.advance,
                    )
                    .trunc();
                let value_width = row
                    .value_icon
                    .as_ref()
                    .map_or_else(
                        || width(row.value.text, &row.value.bytes, 0x12),
                        |i| i.advance,
                    )
                    .trunc();
                let available = (210.0 - label_width - self.unit_x * 5.0).max(0.0);
                let shrink = if value_width > available && value_width > 0.0 {
                    available / value_width
                } else {
                    1.0
                };
                let tint =
                    PackedColor(crate::option_rows::scale_alpha(0x60606060, fade)).to_color();
                for (part, text, x, scale) in [
                    (0, &row.label, anchor.x + self.scale_x * 5.0, 1.0),
                    (
                        1,
                        &row.value,
                        anchor.x + 210.0
                            - self.scale_x * 5.0
                            - self.unit_x * 0.5
                            - value_width * shrink,
                        shrink,
                    ),
                ] {
                    let id = WidgetId(0x1916_0000 + i as u64 * 2 + part);
                    let icon = if part == 0 {
                        &row.label_icon
                    } else {
                        &row.value_icon
                    };
                    if let Some(icon) = icon {
                        if let Some(texture) = icon.texture.as_deref().and_then(textures)
                            && let Some(rect_px) =
                                Edges::new(x, anchor.y, x + icon.size * scale, anchor.y + icon.size)
                                    .to_px(viewport)
                        {
                            out.sprites.push(Sprite {
                                rect_px,
                                texture,
                                color: tint,
                                uv: icon.uv,
                            });
                        }
                        continue;
                    }
                    if let Some(rect) =
                        Edges::new(x, anchor.y, x + 210.0, anchor.y + self.line_height)
                            .to_px(viewport)
                    {
                        out.texts.push(TextPlacement {
                            widget: id,
                            text: text.text,
                            font_index: 0x12,
                            value_font_index: 0x12,
                            align: Align::Start,
                            rect_px: rect,
                            color: tint,
                            value: None,
                        });
                        if text.text == TextRef::Dynamic {
                            out.dynamic.push((id, text.bytes.clone()));
                        }
                        if scale != 1.0 {
                            out.text_scale_x.push((id, scale));
                        }
                    }
                }
            }
            self.outline(
                &mut out.sprites,
                white,
                first,
                count,
                group,
                upper,
                viewport,
            );
        }
        // 0x2DEDC0 follows all label groups. Each has a dark pair of offset
        // lines and two bright centre lines, split at the elbow, integer endpoints.
        let centre = self.pictures.centre;
        for (group, dx, dy, right_edge) in [
            (0, 0.0, -80.0, true),
            (1, 60.0, -29.0, false),
            (2, 30.0, 0.0, false),
            (3, -59.0, -31.0, true),
            (4, -30.0, 14.0, false),
            (5, -69.0, 0.0, false),
        ] {
            let Some(end) = self.points.get(self.connector_segments[group] + 1) else {
                continue;
            };
            let start = Vec2::new(centre.x + dx, centre.y + dy);
            let elbow = Vec2::new(start.x + (end.y - start.y).abs() * 0.4, end.y);
            let end = Vec2::new(end.x + if right_edge { 210.0 } else { 0.0 }, end.y);
            let amount = self.animation.outlines[group];
            let colour = if amount > 0.0 {
                0x4848_7F00 | alpha_byte(amount * fade * 2.0)
            } else {
                crate::option_rows::scale_alpha(0x4848_4840, fade)
            };
            for (offset, c) in [
                (2.0, colour & 255),
                (-1.0, colour & 255),
                (0.0, colour),
                (1.0, colour),
            ] {
                let c = PackedColor(c).to_color();
                line(&mut out.sprites, white, start, elbow, offset, c, viewport);
                line(&mut out.sprites, white, elbow, end, offset, c, viewport);
            }
        }
        out
    }

    fn outline(
        &self,
        out: &mut Vec<Sprite>,
        white: tsfp_ui_renderer::TextureId,
        first: usize,
        count: usize,
        group: usize,
        upper: bool,
        viewport: Viewport,
    ) {
        let edge = first + if upper { count - 1 } else { 0 };
        let cell = self.animation.cells[edge];
        let amount = self.animation.outlines[group];
        if cell.segment != cell.target || amount <= 0.0 {
            return;
        }
        let fade = self.pictures.fade;
        let rgb = self.outline_rgb;
        let inner = PackedColor(
            ((alpha_byte((127.0 - amount) * fade) as u32) << 24)
                | ((rgb[0] as u32) << 16)
                | ((rgb[1] as u32) << 8)
                | rgb[2] as u32,
        )
        .to_color();
        let outer = PackedColor(
            ((alpha_byte(0.0) as u32) << 24)
                | ((rgb[0] as u32) << 16)
                | ((rgb[1] as u32) << 8)
                | rgb[2] as u32,
        )
        .to_color();
        for i in first..first + count {
            let segment = self.animation.cells[i].segment as usize;
            let (Some(a), Some(b)) = (self.points.get(segment + 1), self.points.get(segment + 2))
            else {
                continue;
            };
            gradient_quad(
                out,
                white,
                Vec2::new(a.x - amount, a.y),
                Vec2::new(b.x - amount, b.y),
                amount,
                [outer, inner],
                viewport,
            );
            gradient_quad(
                out,
                white,
                Vec2::new(a.x + 210.0, a.y),
                Vec2::new(b.x + 210.0, b.y),
                amount,
                [inner, outer],
                viewport,
            );
        }
        // The two end caps, including the corner fades.
        for (i, lower) in [(first, false), (first + count - 1, true)] {
            let s = self.animation.cells[i].segment as usize + usize::from(lower);
            let Some(p) = self.points.get(s + 1) else {
                continue;
            };
            let y = if lower { p.y } else { p.y - amount };
            gradient_quad(
                out,
                white,
                Vec2::new(p.x, y),
                Vec2::new(p.x, y + amount),
                210.0,
                [inner, inner],
                viewport,
            );
            gradient_quad(
                out,
                white,
                Vec2::new(p.x - amount, y),
                Vec2::new(p.x - amount, y + amount),
                amount,
                [outer, inner],
                viewport,
            );
            gradient_quad(
                out,
                white,
                Vec2::new(p.x + 210.0, y),
                Vec2::new(p.x + 210.0, y + amount),
                amount,
                [inner, outer],
                viewport,
            );
        }
    }
}

/// 0x1A990 swaps the red/blue bytes before issuing its diffuse vertex word;
/// native channel mapping follows that swap and the common 0x7F normalization.
/// RGB readback is approximate; raw diffuse-alpha agreement remains unverified.
fn curve_colour(word: u32) -> tsfp_ui_renderer::Color {
    let mut colour = PackedColor(word).to_color();
    colour.0.swap(0, 2);
    colour
}

fn gradient_quad(
    out: &mut Vec<Sprite>,
    white: tsfp_ui_renderer::TextureId,
    a: Vec2,
    b: Vec2,
    width: f32,
    colours: [tsfp_ui_renderer::Color; 2],
    viewport: Viewport,
) {
    if ![a.x, a.y, b.x, b.y, width].iter().all(|x| x.is_finite()) || width <= 0.0 {
        return;
    }
    let rows = (b.y - a.y).abs().ceil().clamp(1.0, 64.0) as usize;
    let columns = if colours[0] == colours[1] { 1 } else { 16 };
    for row in 0..rows {
        let p = lerp(a, b, (row as f32 + 0.5) / rows as f32);
        let y0 = a.y + (b.y - a.y) * row as f32 / rows as f32;
        let y1 = a.y + (b.y - a.y) * (row + 1) as f32 / rows as f32;
        for col in 0..columns {
            let t = (col as f32 + 0.5) / columns as f32;
            let colour = tsfp_ui_renderer::Color(std::array::from_fn(|i| {
                colours[0].0[i] + (colours[1].0[i] - colours[0].0[i]) * t
            }));
            let x0 = p.x + width * col as f32 / columns as f32;
            let x1 = p.x + width * (col + 1) as f32 / columns as f32;
            if let Some(rect) = Edges::new(x0, y0.min(y1), x1, y0.max(y1)).to_px(viewport) {
                out.push(Sprite {
                    rect_px: rect,
                    texture: white,
                    color: colour,
                    uv: UvRect {
                        min: Vec2::new(0.0, 0.0),
                        max: Vec2::new(1.0, 1.0),
                    },
                });
            }
        }
    }
}
fn line(
    out: &mut Vec<Sprite>,
    white: tsfp_ui_renderer::TextureId,
    a: Vec2,
    b: Vec2,
    offset: f32,
    colour: tsfp_ui_renderer::Color,
    viewport: Viewport,
) {
    let a = Vec2::new(a.x.trunc(), (a.y + offset).trunc());
    let b = Vec2::new(b.x.trunc(), (b.y + offset).trunc());
    if a.y == b.y {
        if let Some(rect) =
            Edges::new(a.x.min(b.x), a.y, a.x.max(b.x) + 1.0, a.y + 1.0).to_px(viewport)
        {
            out.push(Sprite {
                rect_px: rect,
                texture: white,
                color: colour,
                uv: UvRect {
                    min: Vec2::new(0.0, 0.0),
                    max: Vec2::new(1.0, 1.0),
                },
            });
        }
        return;
    }
    // A one-pixel line sampled in design space, inclusive of its final endpoint.
    let steps = (b.x - a.x)
        .abs()
        .max((b.y - a.y).abs())
        .ceil()
        .clamp(1.0, 2048.0) as usize;
    let mut previous = None;
    for i in 0..=steps {
        let p = lerp(a, b, i as f32 / steps as f32);
        let pixel = (p.x.trunc() as i32, p.y.trunc() as i32);
        if previous == Some(pixel) {
            continue;
        }
        previous = Some(pixel);
        if let Some(rect) = Edges::new(
            p.x.trunc(),
            p.y.trunc(),
            p.x.trunc() + 1.0,
            p.y.trunc() + 1.0,
        )
        .to_px(viewport)
        {
            out.push(Sprite {
                rect_px: rect,
                texture: white,
                color: colour,
                uv: UvRect {
                    min: Vec2::new(0.0, 0.0),
                    max: Vec2::new(1.0, 1.0),
                },
            });
        }
    }
}

#[cfg(test)]
mod curve_tests {
    use super::*;
    #[test]
    fn animation_strict_crossing_and_target_completion_are_draw_counted() {
        let cell = CurveCell {
            fraction: 0.75,
            target: 2,
            segment: 0,
        };
        let mut state = CurveAnimation {
            cells: vec![cell; 18],
            outlines: [0.0; 6],
        };
        state.advance(0.25);
        assert_eq!(state.cells[0].fraction, 1.0);
        assert_eq!(state.cells[0].segment, 0);
        state.advance(0.25);
        assert_eq!(
            state.cells[0],
            CurveCell {
                fraction: 0.25,
                target: 2,
                segment: 1
            }
        );
        state.advance(5.0);
        assert_eq!(
            state.cells[0],
            CurveCell {
                fraction: 0.0,
                target: 2,
                segment: 2
            }
        );
    }
    #[test]
    fn editor_groups_cover_eighteen_once_and_six_outlines() {
        let mut cells = Vec::new();
        let mut groups = Vec::new();
        for (first, count, group, _) in EDITOR_GROUPS {
            cells.extend(first..first + count);
            groups.push(group);
        }
        assert_eq!(cells, (0..18).collect::<Vec<_>>());
        groups.sort();
        assert_eq!(groups, (0..6).collect::<Vec<_>>());
    }
}
