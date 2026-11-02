//! Frame assembly: turns a `RenderLayout` into the complete batch list for `UiRenderer`,
//! with text shaped by a provider, and drives one renderer frame (T2189 UI-6 slice 10).
//!
//! Painter order (INFERRED, not yet frame-hash checked): every quad of the layout first, in
//! layout order, then one text run per placement, the label before the cycler value. Text is
//! never under a panel of a later widget because menu rows do not overlap. Placements the
//! shaper declines (`None`, e.g. empty or dynamic text without a binding) are skipped and
//! counted, never replaced silently.

use crate::bridge::{RenderLayout, TextPlacement};
use crate::model::TextRef;
use tsfp_ui_renderer::{Batch, FontId, PositionedGlyph, TextRun, UiRenderer, Vec2};

/// Which text of a placement is being shaped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextPart {
    /// The widget label, `placement.text` with `placement.font_index`.
    Label,
    /// The selected cycler option, `placement.value` with `placement.value_font_index`.
    Value(TextRef),
}

/// A shaped text run with owned glyphs, borrowed into a `TextRun` at submit time.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedRun {
    pub font: FontId,
    pub raster_height_px: u32,
    pub glyphs: Vec<PositionedGlyph>,
    pub baseline_px: Vec2,
}

/// Text layout provider: resolves the text, measures it, applies alignment inside
/// `placement.rect_px` and returns glyph positions. Owns fonts and strings, the model owns none.
pub trait TextShaper {
    type Error;
    /// Native text box, allowing the provider to account for actual atlas ink.
    fn clip(&self, placement: &TextPlacement, run: &ShapedRun) -> tsfp_ui_renderer::Rect {
        tsfp_ui_renderer::Rect {
            origin: placement.rect_px.origin,
            size: Vec2::new(
                placement.rect_px.size.x,
                placement.rect_px.size.y.max(run.raster_height_px as f32),
            ),
        }
    }
    fn shape(
        &mut self,
        placement: &TextPlacement,
        part: TextPart,
    ) -> Result<Option<ShapedRun>, Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
struct ColoredRun {
    run: ShapedRun,
    color: tsfp_ui_renderer::Color,
    clip: Option<tsfp_ui_renderer::Rect>,
}

/// Everything one page needs for a frame, owning its storage.
#[derive(Clone, Debug, PartialEq)]
pub struct UiFrame {
    pub quads: Vec<tsfp_ui_renderer::Quad>,
    /// Textured skin sprites drawn after the quads and before the text (T2307), painter order.
    pub sprites: Vec<tsfp_ui_renderer::Sprite>,
    /// Blend switches over `sprites` (`skin::SkinnedPage::blends`, T2309); empty = all alpha.
    pub sprite_blends: Vec<(usize, tsfp_ui_renderer::BlendMode)>,
    /// Textured quads with free corners (rotated HUD sprites, T2312), drawn after the quads and
    /// before the sprites with `poly_blend`, switched at `poly_blends` (index, mode) like the sprites.
    pub polys: Vec<tsfp_ui_renderer::Poly>,
    pub poly_blend: tsfp_ui_renderer::BlendMode,
    pub poly_blends: Vec<(usize, tsfp_ui_renderer::BlendMode)>,
    runs: Vec<ColoredRun>,
    /// T2315: pixel scissor for the text runs (a scrolled list's view band), None = the viewport.
    pub text_clip: Option<tsfp_ui_renderer::Rect>,
    /// T2312: blend switches over the text runs (index, mode), like `sprite_blends`; empty = all alpha.
    pub text_blends: Vec<(usize, tsfp_ui_renderer::BlendMode)>,
    /// Text parts the shaper declined.
    pub skipped_text: usize,
    /// T1916: sprites drawn after the text, alpha blended (the Map Maker pointer of the default page style).
    pub overlay: Vec<tsfp_ui_renderer::Sprite>,
}

impl UiFrame {
    /// Shape every placement of `layout`.
    pub fn build<S: TextShaper>(layout: &RenderLayout, shaper: &mut S) -> Result<Self, S::Error> {
        let mut runs = Vec::new();
        let mut skipped_text = 0;
        for placement in &layout.texts {
            let parts = [Some(TextPart::Label), placement.value.map(TextPart::Value)];
            for part in parts.into_iter().flatten() {
                match shaper.shape(placement, part)? {
                    Some(run) => runs.push(ColoredRun {
                        clip: Some(shaper.clip(placement, &run)),
                        run,
                        color: placement.color,
                    }),
                    None => skipped_text += 1,
                }
            }
        }
        Ok(Self {
            quads: layout.quads.clone(),
            sprites: Vec::new(),
            sprite_blends: Vec::new(),
            polys: Vec::new(),
            poly_blend: tsfp_ui_renderer::BlendMode::Alpha,
            poly_blends: Vec::new(),
            runs,
            text_clip: None,
            text_blends: Vec::new(),
            skipped_text,
            overlay: Vec::new(),
        })
    }

    /// Appends the text runs of `other` (shaped at another glyph scale) with their font ids mapped by
    /// `font` (T2312: HUD strings at their own text scale).
    pub fn append_text(
        &mut self,
        other: UiFrame,
        font: impl Fn(tsfp_ui_renderer::FontId) -> tsfp_ui_renderer::FontId,
    ) {
        self.skipped_text += other.skipped_text;
        self.runs.extend(other.runs.into_iter().map(|mut r| {
            r.run.font = font(r.run.font);
            r
        }));
    }

    pub fn text_runs(&self) -> usize {
        self.runs.len()
    }

    /// Batches in painter order: one quad batch and one sprite batch (each omitted when
    /// empty), then the text runs.
    pub fn batches(&self) -> Vec<Batch<'_>> {
        let mut out = Vec::with_capacity(2 + self.runs.len());
        if !self.quads.is_empty() {
            out.push(Batch::Quads(&self.quads));
        }
        if !self.polys.is_empty() {
            let mut mode = tsfp_ui_renderer::BlendMode::Alpha;
            if self.poly_blend != mode {
                out.push(Batch::Blend(self.poly_blend));
                mode = self.poly_blend;
            }
            let mut start = 0;
            for &(at, next) in &self.poly_blends {
                let at = at.min(self.polys.len());
                if at > start {
                    out.push(Batch::Polys(&self.polys[start..at]));
                    start = at;
                }
                if next != mode {
                    out.push(Batch::Blend(next));
                    mode = next;
                }
            }
            if start < self.polys.len() {
                out.push(Batch::Polys(&self.polys[start..]));
            }
            if mode != tsfp_ui_renderer::BlendMode::Alpha {
                out.push(Batch::Blend(tsfp_ui_renderer::BlendMode::Alpha));
            }
        }
        if !self.sprites.is_empty() {
            // Split at the blend switches; a frame that switched away from alpha switches back before the text.
            let mut start = 0;
            let mut mode = tsfp_ui_renderer::BlendMode::Alpha;
            for &(at, next) in &self.sprite_blends {
                let at = at.min(self.sprites.len());
                if at > start {
                    out.push(Batch::Sprites(&self.sprites[start..at]));
                    start = at;
                }
                if next != mode {
                    out.push(Batch::Blend(next));
                    mode = next;
                }
            }
            if start < self.sprites.len() {
                out.push(Batch::Sprites(&self.sprites[start..]));
            }
            if mode != tsfp_ui_renderer::BlendMode::Alpha {
                out.push(Batch::Blend(tsfp_ui_renderer::BlendMode::Alpha));
            }
        }
        let clipped = self.text_clip.is_some() && !self.runs.is_empty();
        if clipped {
            out.push(Batch::Clip(self.text_clip));
        }
        let mut active_clip = if clipped { self.text_clip } else { None };
        let mut mode = tsfp_ui_renderer::BlendMode::Alpha;
        let mut switches = self.text_blends.iter().peekable();
        for (index, r) in self.runs.iter().enumerate() {
            while let Some(&&(at, next)) = switches.peek() {
                if at > index {
                    break;
                }
                switches.next();
                if next != mode {
                    out.push(Batch::Blend(next));
                    mode = next;
                }
            }
            let clip = crate::text_widget::intersect_clip(r.clip, self.text_clip);
            if clip != active_clip {
                out.push(Batch::Clip(clip));
                active_clip = clip;
            }
            out.push(Batch::Text(TextRun {
                font: r.run.font,
                raster_height_px: r.run.raster_height_px,
                glyphs: &r.run.glyphs,
                baseline_px: r.run.baseline_px,
                color: r.color,
            }));
        }
        if mode != tsfp_ui_renderer::BlendMode::Alpha {
            out.push(Batch::Blend(tsfp_ui_renderer::BlendMode::Alpha));
        }
        if active_clip.is_some() {
            out.push(Batch::Clip(None));
        }
        if !self.overlay.is_empty() {
            out.push(Batch::Sprites(&self.overlay));
        }
        out
    }

    /// One renderer frame: begin, submit all batches, end. Any error aborts the frame so
    /// the renderer can begin again (the `UiRenderer` lifecycle contract).
    pub fn draw<R: UiRenderer>(
        &self,
        renderer: &mut R,
        viewport: tsfp_ui_renderer::Viewport,
    ) -> Result<(), R::Error> {
        renderer.begin_frame(viewport)?;
        let result = renderer
            .submit(&self.batches())
            .and_then(|()| renderer.end_frame());
        if result.is_err() {
            renderer.abort_frame();
        }
        result
    }
}

#[cfg(test)]
mod clip_tests {
    use super::*;
    use tsfp_ui_renderer::{Color, Rect, Vec2};

    #[test]
    fn text_clip_wraps_only_the_text_runs() {
        let run = ColoredRun {
            run: ShapedRun {
                font: FontId(0),
                raster_height_px: 10,
                glyphs: Vec::new(),
                baseline_px: Vec2::new(0.0, 0.0),
            },
            color: Color([1.0; 4]),
            clip: None,
        };
        let band = Rect {
            origin: Vec2::new(0.0, 10.0),
            size: Vec2::new(640.0, 100.0),
        };
        let mut frame = UiFrame {
            quads: Vec::new(),
            sprites: Vec::new(),
            sprite_blends: Vec::new(),
            polys: Vec::new(),
            poly_blend: tsfp_ui_renderer::BlendMode::Alpha,
            poly_blends: Vec::new(),
            runs: vec![run.clone()],
            text_clip: Some(band),
            text_blends: Vec::new(),
            skipped_text: 0,
            overlay: Vec::new(),
        };
        let batches = frame.batches();
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], Batch::Clip(Some(band)));
        assert!(matches!(batches[1], Batch::Text(_)));
        assert_eq!(batches[2], Batch::Clip(None));
        frame.text_clip = None;
        assert_eq!(frame.batches().len(), 1);
        // T2312: an additive switch before the second run, back to alpha after the last.
        frame.runs.push(run);
        frame.text_blends = vec![(1, tsfp_ui_renderer::BlendMode::Additive)];
        let batches = frame.batches();
        assert_eq!(batches.len(), 4);
        assert!(matches!(batches[0], Batch::Text(_)));
        assert_eq!(
            batches[1],
            Batch::Blend(tsfp_ui_renderer::BlendMode::Additive)
        );
        assert!(matches!(batches[2], Batch::Text(_)));
        assert_eq!(batches[3], Batch::Blend(tsfp_ui_renderer::BlendMode::Alpha));
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED layouts and a recording renderer.
    use super::*;
    use crate::model::{Align, WidgetId};
    use tsfp_ui_renderer::{Color, GlyphId, Quad, Rect, Viewport};

    fn rect(x: f32, y: f32) -> Rect {
        Rect::new(Vec2::new(x, y), Vec2::new(100.0, 20.0)).unwrap()
    }

    fn placement(id: u64, y: f32, value: Option<TextRef>, color: Color) -> TextPlacement {
        TextPlacement {
            widget: WidgetId(id),
            text: TextRef::Dynamic,
            font_index: 3,
            value_font_index: 4,
            align: Align::Start,
            rect_px: rect(10.0, y),
            color,
            value,
        }
    }

    fn layout() -> RenderLayout {
        let white = Color([1.0; 4]);
        let grey = Color([0.5, 0.5, 0.5, 1.0]);
        RenderLayout {
            quads: vec![Quad {
                rect_px: rect(10.0, 0.0),
                color: white,
            }],
            texts: vec![
                placement(1, 0.0, None, white),
                placement(2, 30.0, Some(TextRef::Dynamic), grey),
                placement(3, 60.0, None, white),
            ],
        }
    }

    /// One glyph per part at the rect origin, font = font index; declines widget 3.
    struct StubShaper;
    impl TextShaper for StubShaper {
        type Error = ();
        fn shape(&mut self, p: &TextPlacement, part: TextPart) -> Result<Option<ShapedRun>, ()> {
            if p.widget == WidgetId(3) {
                return Ok(None);
            }
            let font = match part {
                TextPart::Label => p.font_index,
                TextPart::Value(_) => p.value_font_index,
            };
            Ok(Some(ShapedRun {
                font: FontId(font as u64),
                raster_height_px: 16,
                glyphs: vec![PositionedGlyph {
                    id: GlyphId(p.widget.0 as u32),
                    offset_px: Vec2::new(0.0, 0.0),
                }],
                baseline_px: p.rect_px.origin,
            }))
        }
    }

    #[derive(Default)]
    struct Recorder {
        log: Vec<String>,
        fail_submit: bool,
    }
    impl UiRenderer for Recorder {
        type Error = &'static str;
        fn begin_frame(&mut self, _: Viewport) -> Result<(), Self::Error> {
            self.log.push("begin".into());
            Ok(())
        }
        fn submit(&mut self, batches: &[Batch<'_>]) -> Result<(), Self::Error> {
            for b in batches {
                self.log.push(match b {
                    Batch::Quads(q) => format!("quads {}", q.len()),
                    Batch::Text(t) => format!("text font {} glyph {}", t.font.0, t.glyphs[0].id.0),
                    _ => "other".into(),
                });
            }
            if self.fail_submit {
                Err("submit")
            } else {
                Ok(())
            }
        }
        fn end_frame(&mut self) -> Result<(), Self::Error> {
            self.log.push("end".into());
            Ok(())
        }
        fn abort_frame(&mut self) {
            self.log.push("abort".into());
        }
    }

    fn viewport() -> Viewport {
        Viewport::new(
            Vec2::new(640.0, 480.0),
            1.0,
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(640.0, 480.0)).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn frame_orders_quads_then_label_then_value_and_counts_declined() {
        let frame = UiFrame::build(&layout(), &mut StubShaper).unwrap();
        assert_eq!(frame.text_runs(), 3);
        assert_eq!(frame.skipped_text, 1);
        let mut r = Recorder::default();
        frame.draw(&mut r, viewport()).unwrap();
        assert_eq!(
            r.log,
            [
                "begin",
                "quads 1",
                "other",
                "text font 3 glyph 1",
                "other",
                "text font 3 glyph 2",
                "text font 4 glyph 2",
                "other",
                "end"
            ]
        );
        // Value run keeps the placement colour (disabled grey).
        let batches = frame.batches();
        assert!(matches!(batches[5], Batch::Text(t) if t.color == Color([0.5, 0.5, 0.5, 1.0])));
    }

    #[test]
    fn empty_quads_are_omitted_and_submit_error_aborts() {
        let mut l = layout();
        l.quads.clear();
        let frame = UiFrame::build(&l, &mut StubShaper).unwrap();
        assert!(matches!(frame.batches()[0], Batch::Clip(Some(_))));
        let mut r = Recorder {
            fail_submit: true,
            ..Default::default()
        };
        assert_eq!(frame.draw(&mut r, viewport()), Err("submit"));
        assert_eq!(r.log.last().map(String::as_str), Some("abort"));
        assert!(!r.log.iter().any(|s| s == "end"));
    }

    #[test]
    fn sprite_blend_switches_split_the_sprite_batch_and_restore_alpha() {
        use tsfp_ui_renderer::{BlendMode, Sprite, TextureId, UvRect};
        let mut l = layout();
        l.quads.clear();
        let mut frame = UiFrame::build(&l, &mut StubShaper).unwrap();
        let sprite = Sprite {
            rect_px: rect(0.0, 0.0),
            uv: UvRect {
                min: Vec2::new(0.0, 0.0),
                max: Vec2::new(1.0, 1.0),
            },
            color: Color([1.0; 4]),
            texture: TextureId(1),
        };
        frame.sprites = vec![sprite; 4];
        frame.sprite_blends = vec![(2, BlendMode::Additive), (3, BlendMode::Alpha)];
        let batches = frame.batches();
        let shape: Vec<String> = batches
            .iter()
            .take(5)
            .map(|b| match b {
                Batch::Sprites(s) => format!("sprites {}", s.len()),
                Batch::Blend(m) => format!("{m:?}"),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(
            shape,
            ["sprites 2", "Additive", "sprites 1", "Alpha", "sprites 1"]
        );
        assert!(matches!(batches[6], Batch::Text(_)));
        // A trailing additive switch is closed before the text.
        frame.sprite_blends = vec![(3, BlendMode::Additive)];
        let batches = frame.batches();
        assert!(matches!(batches[3], Batch::Blend(BlendMode::Alpha)));
        assert!(matches!(batches[5], Batch::Text(_)));
    }

    #[test]
    fn shaper_error_propagates() {
        struct Failing;
        impl TextShaper for Failing {
            type Error = u8;
            fn shape(&mut self, _: &TextPlacement, _: TextPart) -> Result<Option<ShapedRun>, u8> {
                Err(7)
            }
        }
        assert_eq!(UiFrame::build(&layout(), &mut Failing), Err(7));
    }

    #[test]
    fn polys_switch_blend_in_painter_order_and_return_to_alpha() {
        use tsfp_ui_renderer::{BlendMode, Poly, TextureId};
        let poly = |x: f32| Poly {
            corners_px: [
                Vec2::new(x, 0.0),
                Vec2::new(x + 1.0, 0.0),
                Vec2::new(x, 1.0),
                Vec2::new(x + 1.0, 1.0),
            ],
            uv: [Vec2::new(0.0, 0.0); 4],
            color: Color([1.0; 4]),
            texture: TextureId(1),
        };
        let mut frame = UiFrame::build(
            &RenderLayout {
                quads: Vec::new(),
                texts: Vec::new(),
            },
            &mut StubShaper,
        )
        .unwrap();
        frame.polys = vec![poly(0.0), poly(1.0), poly(2.0)];
        // Additive for the first, alpha for the other two (hit sector, then radar).
        frame.poly_blends = vec![(0, BlendMode::Additive), (1, BlendMode::Alpha)];
        let shape: Vec<String> = frame
            .batches()
            .iter()
            .map(|b| match b {
                Batch::Polys(p) => format!("polys {}", p.len()),
                Batch::Blend(m) => format!("{m:?}"),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(shape, ["Additive", "polys 1", "Alpha", "polys 2"]);
        frame.poly_blends = vec![(2, BlendMode::Additive)];
        let last: Vec<String> = frame
            .batches()
            .iter()
            .map(|b| match b {
                Batch::Polys(p) => format!("polys {}", p.len()),
                Batch::Blend(m) => format!("{m:?}"),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(last, ["polys 2", "Additive", "polys 1", "Alpha"]);
    }
}
