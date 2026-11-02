//! Production `TextShaper` over the font tables loaded at run time (T2189 UI-6 slice 11).
//!
//! Glyph advances come from `FontTableMeasure::pen_positions_px` (slice 3 formula, INFERRED),
//! so shaped widths equal the widths the layout pass measured with. Font metrics are in the
//! 640 x 480 design space; `px_per_design` scales them to the window pixels of `rect_px`.
//!
//! INFERRED, not frame-hash checked: horizontal alignment inside `rect_px` by `Align`, the
//! baseline one line height below the rect top, `GlyphId` = character code (the atlas maps
//! codes to original font page cells), `FontId` = font object index.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::bridge::TextPlacement;
use crate::font::FontTableMeasure;
use crate::frame::{ShapedRun, TextPart, TextShaper};
use crate::layout::TextMeasure;
use crate::model::{Align, TextRef, WidgetId};
use tsfp_ui_renderer::{FontId, GlyphId, PositionedGlyph, Vec2};

/// Shapes labels and cycler values from the original font tables and string source.
pub struct FontTableShaper<'a> {
    pub measure: &'a FontTableMeasure<'a>,
    /// Window pixels per design pixel (1.0 at 640 x 480).
    pub px_per_design: Vec2,
    /// Runtime text of `TextRef::Dynamic` labels, keyed by widget (bindings fill this per frame).
    pub dynamic: BTreeMap<WidgetId, Vec<u8>>,
    /// Widgets whose text is upper-cased first (`Widget::uppercase`, T2307).
    pub uppercase: BTreeSet<WidgetId>,
    /// Horizontal-only fit applied equally to glyph pens and atlas metrics.
    pub text_scale_x: BTreeMap<WidgetId, f32>,
    pub fixed_spacing: BTreeSet<WidgetId>,
    /// Additional authored bounds, independent of the text pen and wrap width.
    pub clip_bounds: BTreeMap<WidgetId, tsfp_ui_renderer::Rect>,
}

/// The original case conversion `0x3FB90`: `a-z` and Latin-1 `0xE0..=0xFE` minus 0x20.
pub fn original_uppercase(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_lowercase() || (0xE0..=0xFE).contains(&b) {
                b - 0x20
            } else {
                b
            }
        })
        .collect()
}

impl<'a> FontTableShaper<'a> {
    pub fn new(measure: &'a FontTableMeasure<'a>) -> Self {
        Self {
            measure,
            px_per_design: Vec2::new(1.0, 1.0),
            dynamic: BTreeMap::new(),
            uppercase: BTreeSet::new(),
            text_scale_x: BTreeMap::new(),
            fixed_spacing: BTreeSet::new(),
            clip_bounds: BTreeMap::new(),
        }
    }

    fn bytes(&self, widget: WidgetId, text: TextRef) -> Option<&[u8]> {
        match text {
            TextRef::Dynamic => self.dynamic.get(&widget).map(Vec::as_slice),
            other => self.measure.text_bytes(other),
        }
    }
}

impl TextShaper for FontTableShaper<'_> {
    /// Shaping cannot fail; unknown fonts and missing text decline with `None`.
    type Error = core::convert::Infallible;

    fn clip(&self, placement: &TextPlacement, run: &ShapedRun) -> tsfp_ui_renderer::Rect {
        let (font, _) = run.font.source_and_scale();
        let ink = self.measure.fonts.font(font).map_or(
            run.raster_height_px as f32,
            |(object, record)| {
                record
                    .rects
                    .iter()
                    .map(|cell| {
                        (cell.top + f32::from(cell.h)) * object.scale_y * self.px_per_design.y
                    })
                    .fold(run.raster_height_px as f32, f32::max)
            },
        );
        let one_line = run.glyphs.iter().all(|glyph| glyph.offset_px.y == 0.0);
        let point_anchor = placement.rect_px.size.y <= self.px_per_design.y * 2.0;
        let widget = tsfp_ui_renderer::Rect {
            origin: placement.rect_px.origin,
            size: Vec2::new(
                placement.rect_px.size.x,
                if point_anchor || one_line {
                    // Keep a single line's measured bitmap ink inside short authored boxes.
                    // Point anchors remain one line high even if their text wraps.
                    placement.rect_px.size.y.max(ink)
                } else {
                    placement.rect_px.size.y
                },
            ),
        };
        crate::text_widget::intersect_clip(
            Some(widget),
            self.clip_bounds.get(&placement.widget).copied(),
        )
        .unwrap()
    }

    fn shape(
        &mut self,
        placement: &TextPlacement,
        part: TextPart,
    ) -> Result<Option<ShapedRun>, Self::Error> {
        let (text, font) = match part {
            TextPart::Label => (placement.text, placement.font_index),
            TextPart::Value(text) => (text, placement.value_font_index),
        };
        let Some(bytes) = self.bytes(placement.widget, text) else {
            return Ok(None);
        };
        let bytes: Cow<'_, [u8]> = if self.uppercase.contains(&placement.widget) {
            Cow::Owned(original_uppercase(bytes))
        } else {
            Cow::Borrowed(bytes)
        };
        let layout = |bytes: &[u8]| {
            if self.fixed_spacing.contains(&placement.widget) {
                self.measure.fixed_layout(font, bytes)
            } else {
                let pens = self.measure.pen_positions_px(font, bytes);
                let width = pens.last().map_or(0.0, |&(_, pen, step)| pen + step);
                Some((pens, width))
            }
        };
        if layout(&bytes).is_none() {
            return Ok(None);
        }
        let rect = placement.rect_px;
        let requested_fit = self.text_scale_x.get(&placement.widget).copied();
        // Existing original name fitting takes precedence over automatic wrapping.
        let fit = requested_fit.unwrap_or(1.0);
        let font_id = FontId::scaled(font, [fit, 1.0]);
        let fit = font_id.source_and_scale().1[0];
        let scale = Vec2::new(self.px_per_design.x * fit, self.px_per_design.y);
        let line_px = self.measure.line_height(font) / self.measure.grid_y * scale.y;
        // Some font records have a small advance (+0x1C) relative to their
        // texture cells. Multiline text must also clear the actual glyph ink.
        let ink_px = self
            .measure
            .fonts
            .font(font)
            .map_or(line_px, |(object, record)| {
                record
                    .rects
                    .iter()
                    .map(|cell| (cell.top + f32::from(cell.h)) * object.scale_y * scale.y)
                    .fold(line_px, f32::max)
            });
        let line_step = line_px.max(ink_px);
        let lines = crate::text_widget::lines(
            &bytes,
            rect.size.x / scale.x,
            requested_fit.is_some(),
            &|b| layout(b).map_or(0.0, |(_, w)| w),
        );
        let mut glyphs = Vec::new();
        for (row, bytes) in lines.iter().enumerate() {
            let Some((pens, width)) = layout(bytes) else {
                continue;
            };
            let free = (rect.size.x - width * scale.x).max(0.0);
            let x = match placement.align {
                Align::Start => 0.0,
                Align::Center => free / 2.0,
                Align::End => free,
            };
            glyphs.extend(pens.iter().map(|&(code, pen, _)| PositionedGlyph {
                id: GlyphId(u32::from(code)),
                offset_px: Vec2::new(x + pen * scale.x, row as f32 * line_step),
            }));
        }
        if glyphs.is_empty() {
            return Ok(None);
        }
        // Keep alignment on the baseline for single lines (the established contract).
        let x = glyphs.first().map_or(0.0, |g| g.offset_px.x);
        for glyph in &mut glyphs {
            glyph.offset_px.x -= x;
        }
        let x = rect.origin.x + x;
        // Atlas bearings subtract this integer height; use the same value for the baseline
        // so a fractional font/window scale cannot introduce a vertical rounding offset.
        let raster_height_px = line_px.round().max(1.0) as u32;
        Ok(Some(ShapedRun {
            font: font_id,
            raster_height_px,
            glyphs,
            baseline_px: Vec2::new(x, rect.origin.y + raster_height_px as f32),
        }))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! Tier 0, FABRICATED font tables (real structure, made-up numbers).
    use super::*;
    use crate::font::{FontObject, FontRecord, FontSet, Glyph, KERN_SIZE, StringSource};
    use crate::model::TextId;
    use tsfp_ui_renderer::{Color, Rect};

    pub(crate) struct Strings(pub Vec<Vec<u8>>);
    impl StringSource for Strings {
        fn bytes(&self, id: TextId) -> Option<&[u8]> {
            self.0.get(id.0 as usize).map(Vec::as_slice)
        }
    }

    /// Record 0: cell 10 x 20, glyph 'B' adjust +2, kerning A->B = -3. Object 0 scale 1,
    /// object 1 scale 2 x 0.5.
    pub(crate) fn fonts() -> FontSet {
        let mut glyphs = vec![Glyph::default(); 96];
        glyphs[usize::from(b'A' - 0x20)] = Glyph {
            kern_row: 0,
            kern_column: 1,
            adjust: 0,
        };
        glyphs[usize::from(b'B' - 0x20)] = Glyph {
            kern_row: 2,
            kern_column: 0,
            adjust: 2,
        };
        let mut kerning = vec![0; KERN_SIZE * KERN_SIZE];
        kerning[KERN_SIZE + 2] = -3;
        let record = FontRecord {
            cell_width: 10.0,
            cell_height: 20.0,
            glyphs,
            kerning,
            ..FontRecord::default()
        };
        FontSet {
            records: [(0, record)].into(),
            objects: vec![
                FontObject {
                    record: 0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    advance_adjust: 0,
                },
                FontObject {
                    record: 0,
                    scale_x: 2.0,
                    scale_y: 0.5,
                    advance_adjust: 1,
                },
            ],
            ..FontSet::default()
        }
    }

    fn placement(text: TextRef, align: Align, value: Option<TextRef>) -> TextPlacement {
        TextPlacement {
            widget: WidgetId(7),
            text,
            font_index: 0,
            value_font_index: 1,
            align,
            rect_px: Rect::new(Vec2::new(100.0, 50.0), Vec2::new(200.0, 30.0)).unwrap(),
            color: Color([1.0; 4]),
            value,
        }
    }

    fn xs(run: &ShapedRun) -> Vec<f32> {
        run.glyphs.iter().map(|g| g.offset_px.x).collect()
    }

    #[test]
    fn one_line_clip_contains_runtime_ink_but_keeps_multiline_box() {
        use crate::font::GlyphRect;

        // Tier 0: fabricated font ink extends below the authored line box.
        let mut fonts = fonts();
        fonts.records.get_mut(&0).unwrap().rects = vec![GlyphRect {
            x: 0,
            y: 0,
            w: 10,
            h: 24,
            top: 0.0,
        }];
        let strings = Strings(vec![]);
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let mut p = placement(TextRef::None, Align::Start, None);
        p.rect_px.size.y = 18.0;
        let run = |offsets: &[f32]| ShapedRun {
            font: FontId::scaled(0, [1.0, 1.0]),
            raster_height_px: 20,
            glyphs: offsets
                .iter()
                .map(|&y| PositionedGlyph {
                    id: tsfp_ui_renderer::GlyphId(u32::from(b'A')),
                    offset_px: Vec2::new(0.0, y),
                })
                .collect(),
            baseline_px: Vec2::new(100.0, 70.0),
        };

        assert_eq!(shaper.clip(&p, &run(&[0.0])).size.y, 24.0);
        assert_eq!(shaper.clip(&p, &run(&[0.0, 20.0])).size.y, 18.0);

        // Explicit page/list bounds still win after the widget grants a full ink line.
        shaper.clip_bounds.insert(
            p.widget,
            Rect {
                origin: Vec2::new(p.rect_px.origin.x, p.rect_px.origin.y + 4.0),
                size: Vec2::new(p.rect_px.size.x, 10.0),
            },
        );
        let clipped = shaper.clip(&p, &run(&[0.0]));
        assert_eq!(clipped.origin.y, p.rect_px.origin.y + 4.0);
        assert_eq!(clipped.size.y, 10.0);
    }

    #[test]
    fn authored_column_clip_preserves_centered_pen_and_wrap_at_both_aspects() {
        use crate::bridge::RenderLayout;
        use crate::frame::UiFrame;
        use crate::raster::{BoxAtlas, CpuRasterRenderer};
        use tsfp_ui_renderer::Viewport;
        let fonts = fonts();
        let strings = Strings(vec![vec![b'A'; 80]]);
        let measure = FontTableMeasure::new(&fonts, &strings);
        for (width, height) in [(1280, 960), (1280, 720)] {
            let unit = (width as f32 / 640.0).min(height as f32 / 480.0);
            for dx in [-1.1, 0.0, 1.1] {
                let mut p = placement(TextRef::Id(TextId(0)), Align::Start, None);
                p.rect_px = Rect {
                    origin: Vec2::new((20.0 + dx) * unit, 120.0 * unit),
                    size: Vec2::new(280.0 * unit, 40.0 * unit),
                };
                let column = Rect {
                    origin: Vec2::new(100.0 * unit, 120.0 * unit),
                    size: Vec2::new(200.0 * unit, 40.0 * unit),
                };
                let mut shaper = FontTableShaper::new(&measure);
                shaper.px_per_design = Vec2::new(unit, unit);
                let before = shaper.shape(&p, TextPart::Label).unwrap().unwrap();
                shaper.clip_bounds.insert(p.widget, column);
                let after = shaper.shape(&p, TextPart::Label).unwrap().unwrap();
                assert_eq!(
                    before, after,
                    "clipping must not relocate or rewrap the text"
                );
                let frame = UiFrame::build(
                    &RenderLayout {
                        quads: vec![],
                        texts: vec![p],
                    },
                    &mut shaper,
                )
                .unwrap();
                let mut renderer = CpuRasterRenderer::new(BoxAtlas { width_ratio: 0.5 }, [0; 4]);
                let size = Vec2::new(width as f32, height as f32);
                frame
                    .draw(
                        &mut renderer,
                        Viewport::new(size, 1.0, Rect::new(Vec2::new(0.0, 0.0), size).unwrap())
                            .unwrap(),
                    )
                    .unwrap();
                let image = renderer.finished().unwrap();
                let mut painted = 0;
                for y in 0..height {
                    for x in 0..width {
                        if image.pixel(x, y)[3] == 0 {
                            continue;
                        }
                        painted += 1;
                        assert!(
                            x as f32 + 0.5 >= column.origin.x
                                && (x as f32 + 0.5) < column.origin.x + column.size.x
                        );
                        assert!(
                            y as f32 + 0.5 >= column.origin.y
                                && (y as f32 + 0.5) < column.origin.y + column.size.y
                        );
                    }
                }
                assert!(painted > 0);
            }
        }
    }

    #[test]
    fn boxed_long_text_pixels_stay_inside_widget_and_page_at_both_aspects() {
        use crate::bridge::RenderLayout;
        use crate::frame::UiFrame;
        use crate::raster::{BoxAtlas, CpuRasterRenderer};
        use tsfp_ui_renderer::{UiRenderer, Viewport};

        // Tier 0: adversarial fabricated strings/atlas, through production shaping
        // and frame batching. This checks containment, not original font fidelity.
        let fonts = fonts();
        let strings = Strings(vec![
            b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_vec(),
            b"AB AB AB AB AB AB AB AB AB AB".to_vec(),
            b"AB\nAB\nAB\nAB\nAB".to_vec(),
        ]);
        let measure = FontTableMeasure::new(&fonts, &strings);
        for (width, height) in [(640, 480), (1280, 720)] {
            let unit = (width as f32 / 640.0).min(height as f32 / 480.0);
            for id in 0..3 {
                for align in [Align::Start, Align::Center, Align::End] {
                    for box_width in [0.0, 35.0] {
                        let mut shaper = FontTableShaper::new(&measure);
                        shaper.px_per_design = Vec2::new(unit, unit);
                        let mut p = placement(TextRef::Id(TextId(id)), align, None);
                        p.rect_px = Rect {
                            origin: Vec2::new(100.0 * unit, 50.0 * unit),
                            size: Vec2::new(box_width * unit, 25.0 * unit),
                        };
                        let layout = RenderLayout {
                            quads: vec![],
                            texts: vec![p.clone()],
                        };
                        let mut frame = UiFrame::build(&layout, &mut shaper).unwrap();
                        // A scroll band cuts both the first line and subsequent
                        // lines, independently of the widget's own vertical box.
                        let page = Rect {
                            origin: Vec2::new(105.0 * unit, 55.0 * unit),
                            size: Vec2::new(40.0 * unit, 17.0 * unit),
                        };
                        frame.text_clip = Some(page);
                        let mut renderer =
                            CpuRasterRenderer::new(BoxAtlas { width_ratio: 0.5 }, [0; 4]);
                        let size = Vec2::new(width as f32, height as f32);
                        frame
                            .draw(
                                &mut renderer,
                                Viewport::new(
                                    size,
                                    1.0,
                                    Rect::new(Vec2::new(0.0, 0.0), size).unwrap(),
                                )
                                .unwrap(),
                            )
                            .unwrap();
                        let image = renderer.finished().unwrap();
                        let clip = crate::text_widget::intersect_clip(Some(p.rect_px), Some(page))
                            .unwrap();
                        let mut painted = 0;
                        for y in 0..height {
                            for x in 0..width {
                                if image.pixel(x, y)[3] == 0 {
                                    continue;
                                }
                                painted += 1;
                                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                                assert!(
                                    cx >= clip.origin.x
                                        && cx < clip.origin.x + clip.size.x
                                        && cy >= clip.origin.y
                                        && cy < clip.origin.y + clip.size.y,
                                    "text {id}, {align:?}, {width}x{height}: escaped at {x},{y}"
                                );
                            }
                        }
                        if box_width == 0.0 {
                            assert_eq!(painted, 0);
                        } else {
                            assert!(painted > 0, "containment must not pass by dropping text");
                        }
                        // Subsequent draws must not inherit a text scissor.
                        let quad = [tsfp_ui_renderer::Quad {
                            rect_px: Rect::new(Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)).unwrap(),
                            color: Color([1.0; 4]),
                        }];
                        renderer
                            .begin_frame(
                                Viewport::new(
                                    size,
                                    1.0,
                                    Rect::new(Vec2::new(0.0, 0.0), size).unwrap(),
                                )
                                .unwrap(),
                            )
                            .unwrap();
                        renderer.submit(&frame.batches()).unwrap();
                        renderer
                            .submit(&[tsfp_ui_renderer::Batch::Quads(&quad)])
                            .unwrap();
                        renderer.end_frame().unwrap();
                        assert_eq!(renderer.finished().unwrap().pixel(0, 0), [255; 4]);
                    }
                }
            }
        }
    }

    #[test]
    fn label_advances_match_measure_with_kerning_and_adjust() {
        let (fonts, strings) = (fonts(), Strings(vec![b"AB\x01A".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let p = placement(TextRef::Id(TextId(0)), Align::Start, None);
        let run = shaper.shape(&p, TextPart::Label).unwrap().unwrap();
        // A at 0 (step 10), B at 10 (10 + 2 - 3 = 9), control skipped, A at 19.
        assert_eq!(xs(&run), [0.0, 10.0, 19.0]);
        let ids: Vec<u32> = run.glyphs.iter().map(|g| g.id.0).collect();
        assert_eq!(ids, [65, 66, 65]);
        assert_eq!(measure.width_px(0, b"AB\x01A"), 29.0);
        assert_eq!(run.font, FontId(0));
        assert_eq!(run.raster_height_px, 20);
        assert_eq!(run.baseline_px, Vec2::new(100.0, 70.0));
    }

    #[test]
    fn fractional_scale_keeps_line_top_independent_of_raster_height_rounding() {
        let (fonts, strings) = (fonts(), Strings(vec![b"AB".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let p = placement(TextRef::Id(TextId(0)), Align::Start, None);
        for scale_y in [1.0125, 1.025, 1.0375, 0.0125] {
            shaper.px_per_design.y = scale_y;
            let run = shaper.shape(&p, TextPart::Label).unwrap().unwrap();
            // Original/outline atlas bearings use top * scale - raster_height_px.
            assert_eq!(
                run.baseline_px.y - run.raster_height_px as f32,
                p.rect_px.origin.y
            );
        }
    }

    #[test]
    fn fixed_mode_measures_cells_but_offsets_only_one_and_punctuation() {
        let mut fonts = fonts();
        fonts.records.get_mut(&0).unwrap().fixed_spacing = Some(crate::font::FixedSpacing {
            advance: 10.0,
            digit_offset: 3.0,
            punctuation_offset: 1.0,
            punctuation_ratio: 0.4,
            slash_offset: 2.0,
            slash_ratio: 0.6,
        });
        let strings = Strings(vec![b"01:2/3.4".to_vec()]);
        let measure = FontTableMeasure::new(&fonts, &strings);
        let (pens, width) = measure.fixed_layout(0, b"01:2/3.4").unwrap();
        assert_eq!(width, 68.0);
        assert_eq!(
            pens.iter().map(|p| p.1).collect::<Vec<_>>(),
            [0.0, 13.0, 21.0, 25.0, 37.0, 43.0, 54.0, 58.0]
        );
        let mut shaper = FontTableShaper::new(&measure);
        shaper.fixed_spacing.insert(WidgetId(7));
        let run = shaper
            .shape(
                &placement(TextRef::Id(TextId(0)), Align::End, None),
                TextPart::Label,
            )
            .unwrap()
            .unwrap();
        assert_eq!(xs(&run), pens.iter().map(|p| p.1).collect::<Vec<_>>());
        assert_eq!(
            run.baseline_px.x, 232.0,
            "right alignment uses advance, not glyph bearing"
        );
        assert_eq!(run.raster_height_px, 20);
        let mut fonts = fonts.clone();
        fonts.records.get_mut(&0).unwrap().fixed_spacing = None;
        let measure = FontTableMeasure::new(&fonts, &strings);
        assert!(measure.fixed_layout(0, b"12").is_none());
    }

    #[test]
    fn horizontal_fit_scales_pens_and_atlas_width_but_keeps_height() {
        let (fonts, strings) = (fonts(), Strings(vec![b"AAA".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        shaper.text_scale_x.insert(WidgetId(7), 0.5);
        let p = placement(TextRef::Id(TextId(0)), Align::End, None);
        let run = shaper.shape(&p, TextPart::Label).unwrap().unwrap();
        assert_eq!(xs(&run), [0.0, 5.0, 10.0]);
        assert_eq!(run.font.source_and_scale(), (0, [0.5, 1.0]));
        assert_eq!(run.raster_height_px, 20);
        assert_eq!(run.baseline_px, Vec2::new(285.0, 70.0));
    }

    #[test]
    fn alignment_centre_and_end_use_measured_width() {
        let (fonts, strings) = (fonts(), Strings(vec![b"AB\x01A".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let id = TextRef::Id(TextId(0));
        let centre = shaper.shape(&placement(id, Align::Center, None), TextPart::Label);
        let end = shaper.shape(&placement(id, Align::End, None), TextPart::Label);
        assert_eq!(
            centre.unwrap().unwrap().baseline_px.x,
            100.0 + (200.0 - 29.0) / 2.0
        );
        assert_eq!(end.unwrap().unwrap().baseline_px.x, 300.0 - 29.0);
    }

    #[test]
    fn value_uses_value_font_and_window_scale() {
        let (fonts, strings) = (fonts(), Strings(vec![Vec::new(), b"AA".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        shaper.px_per_design = Vec2::new(2.0, 3.0);
        let value = TextRef::Id(TextId(1));
        let p = placement(TextRef::Id(TextId(0)), Align::Start, Some(value));
        let run = shaper.shape(&p, TextPart::Value(value)).unwrap().unwrap();
        // Object 1: step 10 * 2 + 1 = 21 design px, x2 window.
        assert_eq!(xs(&run), [0.0, 42.0]);
        assert_eq!(run.font, FontId(1));
        // Line 20 * 0.5 = 10 design px, x3 window.
        assert_eq!(run.raster_height_px, 30);
        assert_eq!(run.baseline_px.y, 80.0);
    }

    #[test]
    fn boxed_text_wraps_whole_words_and_aligns_each_line() {
        let (fonts, strings) = (fonts(), Strings(vec![b"AA AA AA".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let mut p = placement(TextRef::Id(TextId(0)), Align::End, None);
        p.rect_px.size = Vec2::new(50.0, 60.0);
        let run = shaper.shape(&p, TextPart::Label).unwrap().unwrap();
        assert_eq!(
            run.glyphs.iter().map(|g| g.offset_px.y).collect::<Vec<_>>(),
            [0.0, 0.0, 0.0, 0.0, 0.0, 20.0, 20.0]
        );
        assert_eq!(run.glyphs[5].offset_px.x, 30.0);
        // Original horizontal fitting intentionally keeps this on one line.
        shaper.text_scale_x.insert(p.widget, 0.5);
        let run = shaper.shape(&p, TextPart::Label).unwrap().unwrap();
        assert!(run.glyphs.iter().all(|g| g.offset_px.y == 0.0));
    }

    #[test]
    fn atlas_ink_sets_wrap_spacing_and_anchor_clip_height() {
        let mut fonts = fonts();
        let record = fonts.records.get_mut(&0).unwrap();
        record.cell_height = 3.0;
        record.rects = vec![
            crate::font::GlyphRect {
                w: 10,
                h: 20,
                top: 2.0,
                ..Default::default()
            };
            96
        ];
        let strings = Strings(vec![b"AA AA".to_vec()]);
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let mut p = placement(TextRef::Id(TextId(0)), Align::Start, None);
        p.rect_px.size = Vec2::new(21.0, 1.0);
        let run = shaper.shape(&p, TextPart::Label).unwrap().unwrap();
        assert!(run.glyphs.iter().any(|g| g.offset_px.y >= 22.0));
        assert_eq!(shaper.clip(&p, &run).size.y, 22.0);
        p.rect_px.size.y = 15.0;
        assert_eq!(
            shaper.clip(&p, &run).size.y,
            15.0,
            "real boxes retain their clipping boundary"
        );
    }

    #[test]
    fn original_uppercase_matches_3fb90_ranges() {
        // a-z and 0xE0..=0xFE shift down by 0x20 (0xF7 included, as in the original); 0xFF stays.
        assert_eq!(
            original_uppercase(b"Story 1 \xe9\xf7\xfe\xff{"),
            b"STORY 1 \xc9\xd7\xde\xff{".to_vec()
        );
    }

    #[test]
    fn declines_empty_missing_unbound_and_unknown_font() {
        let (fonts, strings) = (fonts(), Strings(vec![Vec::new(), b"A".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        for text in [
            TextRef::None,
            TextRef::Dynamic,
            TextRef::Id(TextId(0)),
            TextRef::Id(TextId(9)),
        ] {
            let p = placement(text, Align::Start, None);
            assert_eq!(shaper.shape(&p, TextPart::Label), Ok(None), "{text:?}");
        }
        let mut p = placement(TextRef::Id(TextId(1)), Align::Start, None);
        p.font_index = 5;
        assert_eq!(shaper.shape(&p, TextPart::Label), Ok(None));
        shaper.dynamic.insert(WidgetId(7), b"A".to_vec());
        let p = placement(TextRef::Dynamic, Align::Start, None);
        assert!(shaper.shape(&p, TextPart::Label).unwrap().is_some());
    }
}
