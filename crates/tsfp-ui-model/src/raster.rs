//! CPU reference `UiRenderer` (T2189 UI-6 slice 12): rasterises a `PagePresenter` frame into an
//! RGBA8 buffer with no GPU, so frames render offscreen in tests and can be hashed
//! (`framehash.rs`). It is a deterministic reference of the renderer contract (pixel-centre
//! coverage, scissor clip, straight-alpha source-over), not of the original NV2A output.
//!
//! Glyphs come from a `GlyphAtlas`: each glyph fills its atlas box (`size_px` at `bearing_px`)
//! with the run colour. Glyph coverage bitmaps are not sampled yet (the original font pages
//! need the private image), so text is a box mask. Sprites are rejected (no texture source).

use tsfp_ui_renderer::{
    AtlasGlyph, Batch, BlendMode, Color, GlyphAtlas, GlyphKey, Rect, TextRun, UiRenderer, Vec2,
    Viewport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterError {
    /// `submit`/`end_frame` without `begin_frame`, or `begin_frame` twice.
    Lifecycle,
    /// Sprite batches need a texture source the CPU reference does not have.
    SpritesUnsupported,
}

/// Placeholder atlas: every glyph is a box `raster_height` tall and `width_ratio` of that
/// wide, sitting on the baseline. FABRICATED metrics, for scaffolding only.
#[derive(Clone, Copy, Debug)]
pub struct BoxAtlas {
    pub width_ratio: f32,
}

impl GlyphAtlas for BoxAtlas {
    type Error = std::convert::Infallible;
    fn prepare(&mut self, _keys: &[GlyphKey]) -> Result<(), Self::Error> {
        Ok(())
    }
    fn glyph(&self, key: GlyphKey) -> Option<AtlasGlyph> {
        let height = key.raster_height_px as f32;
        let width = (height * self.width_ratio).max(1.0);
        Some(AtlasGlyph {
            texture: tsfp_ui_renderer::TextureId(0),
            uv: tsfp_ui_renderer::UvRect {
                min: Vec2::new(0.0, 0.0),
                max: Vec2::new(1.0, 1.0),
            },
            size_px: Vec2::new(width, height),
            bearing_px: Vec2::new(0.0, height),
            advance_px: width,
        })
    }
}

/// Finished frame: RGBA8, row-major, top-left origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl RasterImage {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }
}

/// Integer pixel span `[x0, x1) x [y0, y1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Span {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Span {
    /// Pixels whose centre lies inside `rect` (left/top inclusive, right/bottom exclusive).
    fn covered(rect: Rect, width: u32, height: u32) -> Self {
        let first = |edge: f32, limit: u32| ((edge - 0.5).ceil().max(0.0) as u32).min(limit);
        Self {
            x0: first(rect.origin.x, width),
            y0: first(rect.origin.y, height),
            x1: first(rect.origin.x + rect.size.x, width),
            y1: first(rect.origin.y + rect.size.y, height),
        }
    }
    fn intersect(self, other: Self) -> Self {
        Self {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1).max(self.x0.max(other.x0)),
            y1: self.y1.min(other.y1).max(self.y0.max(other.y0)),
        }
    }
}

/// CPU reference renderer. The target is cleared to `clear` on every `begin_frame`.
pub struct CpuRasterRenderer<A> {
    pub atlas: A,
    pub clear: [u8; 4],
    /// Glyphs the atlas had no entry for, summed over the frame (omitted, not replaced).
    pub missing_glyphs: usize,
    image: RasterImage,
    clip: Span,
    blend: BlendMode,
    in_frame: bool,
    finished: Option<RasterImage>,
}

impl<A: GlyphAtlas> CpuRasterRenderer<A> {
    pub fn new(atlas: A, clear: [u8; 4]) -> Self {
        Self {
            atlas,
            clear,
            missing_glyphs: 0,
            image: RasterImage {
                width: 0,
                height: 0,
                rgba: Vec::new(),
            },
            clip: Span {
                x0: 0,
                y0: 0,
                x1: 0,
                y1: 0,
            },
            blend: BlendMode::Alpha,
            in_frame: false,
            finished: None,
        }
    }

    /// The last frame closed by `end_frame`; `None` before the first or after an abort.
    pub fn finished(&self) -> Option<&RasterImage> {
        self.finished.as_ref()
    }

    fn full(&self) -> Span {
        Span {
            x0: 0,
            y0: 0,
            x1: self.image.width,
            y1: self.image.height,
        }
    }

    fn fill(&mut self, rect: Rect, color: Color) {
        let span = Span::covered(rect, self.image.width, self.image.height).intersect(self.clip);
        let [red, green, blue, alpha] = color.0.map(|c| c.clamp(0.0, 1.0));
        for y in span.y0..span.y1 {
            for x in span.x0..span.x1 {
                let at = ((y * self.image.width + x) * 4) as usize;
                let dst = &mut self.image.rgba[at..at + 4];
                let keep = match self.blend {
                    BlendMode::Alpha => 1.0 - alpha,
                    // T2309: the original's additive mode adds src * a and keeps the destination.
                    BlendMode::Additive => 1.0,
                };
                for (channel, src) in dst.iter_mut().zip([red, green, blue]) {
                    let mixed = src * alpha + f32::from(*channel) / 255.0 * keep;
                    *channel = (mixed.min(1.0) * 255.0).round() as u8;
                }
                if self.blend == BlendMode::Alpha {
                    let out_alpha = alpha + f32::from(dst[3]) / 255.0 * (1.0 - alpha);
                    dst[3] = (out_alpha * 255.0).round() as u8;
                }
            }
        }
    }

    fn text(&mut self, run: &TextRun<'_>) {
        for glyph in run.glyphs {
            let key = GlyphKey {
                font: run.font,
                glyph: glyph.id,
                raster_height_px: run.raster_height_px,
            };
            let Some(entry) = self.atlas.glyph(key) else {
                self.missing_glyphs += 1;
                continue;
            };
            let origin = Vec2::new(
                run.baseline_px.x + glyph.offset_px.x + entry.bearing_px.x,
                run.baseline_px.y + glyph.offset_px.y - entry.bearing_px.y,
            );
            if let Ok(rect) = Rect::new(origin, entry.size_px) {
                self.fill(rect, run.color);
            }
        }
    }
}

impl<A: GlyphAtlas> UiRenderer for CpuRasterRenderer<A> {
    type Error = RasterError;
    fn begin_frame(&mut self, viewport: Viewport) -> Result<(), RasterError> {
        if self.in_frame {
            return Err(RasterError::Lifecycle);
        }
        let size = viewport.size_px();
        let (width, height) = (size.x.ceil() as u32, size.y.ceil() as u32);
        self.image = RasterImage {
            width,
            height,
            rgba: self.clear.repeat((width * height) as usize),
        };
        self.clip = self.full();
        self.blend = BlendMode::Alpha;
        self.missing_glyphs = 0;
        self.in_frame = true;
        Ok(())
    }
    fn submit(&mut self, batches: &[Batch<'_>]) -> Result<(), RasterError> {
        if !self.in_frame {
            return Err(RasterError::Lifecycle);
        }
        for batch in batches {
            match batch {
                Batch::Quads(quads) => quads.iter().for_each(|q| self.fill(q.rect_px, q.color)),
                Batch::Text(run) => self.text(run),
                Batch::Clip(None) => self.clip = self.full(),
                Batch::Clip(Some(rect)) => {
                    self.clip = Span::covered(*rect, self.image.width, self.image.height)
                }
                Batch::Sprites(_) | Batch::Polys(_) => return Err(RasterError::SpritesUnsupported),
                Batch::Blend(mode) => self.blend = *mode,
            }
        }
        Ok(())
    }
    fn end_frame(&mut self) -> Result<(), RasterError> {
        if !self.in_frame {
            return Err(RasterError::Lifecycle);
        }
        self.in_frame = false;
        self.finished = Some(self.image.clone());
        Ok(())
    }
    fn abort_frame(&mut self) {
        self.in_frame = false;
        self.finished = None;
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED geometry: contract rules of the CPU reference.
    use super::*;
    use tsfp_ui_renderer::Quad;

    fn viewport(width: f32, height: f32) -> Viewport {
        let size = Vec2::new(width, height);
        Viewport::new(size, 1.0, Rect::new(Vec2::new(0.0, 0.0), size).unwrap()).unwrap()
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(Vec2::new(x, y), Vec2::new(w, h)).unwrap()
    }

    fn render(batches: &[Batch<'_>]) -> RasterImage {
        let mut renderer = CpuRasterRenderer::new(BoxAtlas { width_ratio: 0.5 }, [0, 0, 0, 255]);
        renderer.begin_frame(viewport(8.0, 8.0)).unwrap();
        renderer.submit(batches).unwrap();
        renderer.end_frame().unwrap();
        renderer.finished().unwrap().clone()
    }

    #[test]
    fn pixel_centre_coverage_and_half_alpha_blend() {
        let quads = [Quad {
            rect_px: rect(1.5, 1.0, 2.0, 1.0),
            color: Color([1.0, 0.0, 0.0, 0.5]),
        }];
        let image = render(&[Batch::Quads(&quads)]);
        // Pixel centres 1.5 and 2.5 are inside [1.5, 3.5), centre 3.5 sits on the open right edge.
        assert_eq!(image.pixel(1, 1), [128, 0, 0, 255]);
        assert_eq!(image.pixel(2, 1), [128, 0, 0, 255]);
        assert_eq!(image.pixel(3, 1), [0, 0, 0, 255]);
        assert_eq!(image.pixel(1, 2), [0, 0, 0, 255]);
    }

    #[test]
    fn additive_blend_adds_and_alpha_restores() {
        let grey = [Quad {
            rect_px: rect(0.0, 0.0, 8.0, 8.0),
            color: Color([0.2, 0.2, 0.2, 1.0]),
        }];
        let add = [Quad {
            rect_px: rect(0.0, 0.0, 4.0, 8.0),
            color: Color([0.5, 0.25, 0.0, 0.5]),
        }];
        let image = render(&[
            Batch::Quads(&grey),
            Batch::Blend(BlendMode::Additive),
            Batch::Quads(&add),
            Batch::Blend(BlendMode::Alpha),
            Batch::Quads(&[Quad {
                rect_px: rect(4.0, 0.0, 4.0, 8.0),
                color: Color([0.5, 0.25, 0.0, 0.5]),
            }]),
        ]);
        // 51 + 0.5 * 0.5 * 255 = 114.75, 51 + 0.25 * 0.5 * 255 = 82.9; alpha untouched.
        assert_eq!(image.pixel(1, 1), [115, 83, 51, 255]);
        // Alpha again: 0.5 * src + 0.5 * dst.
        assert_eq!(image.pixel(5, 1), [89, 57, 26, 255]);
    }

    #[test]
    fn clip_replaces_scissor_and_none_resets() {
        let all = [Quad {
            rect_px: rect(0.0, 0.0, 8.0, 8.0),
            color: Color([0.0, 1.0, 0.0, 1.0]),
        }];
        let image = render(&[
            Batch::Clip(Some(rect(2.0, 2.0, 2.0, 2.0))),
            Batch::Quads(&all),
        ]);
        assert_eq!(image.pixel(2, 2), [0, 255, 0, 255]);
        assert_eq!(image.pixel(4, 2), [0, 0, 0, 255]);
        let image = render(&[
            Batch::Clip(Some(rect(0.0, 0.0, 0.0, 0.0))),
            Batch::Clip(None),
            Batch::Quads(&all),
        ]);
        assert_eq!(image.pixel(7, 7), [0, 255, 0, 255]);
    }

    #[test]
    fn glyph_box_sits_on_baseline() {
        let glyphs = [tsfp_ui_renderer::PositionedGlyph {
            id: tsfp_ui_renderer::GlyphId(65),
            offset_px: Vec2::new(1.0, 0.0),
        }];
        let run = TextRun {
            font: tsfp_ui_renderer::FontId(0),
            raster_height_px: 4,
            glyphs: &glyphs,
            baseline_px: Vec2::new(1.0, 6.0),
            color: Color([1.0; 4]),
        };
        let image = render(&[Batch::Text(run)]);
        // Box 2 x 4 at (2, 2)..(4, 6).
        assert_eq!(image.pixel(2, 2), [255; 4]);
        assert_eq!(image.pixel(3, 5), [255; 4]);
        assert_eq!(image.pixel(4, 5), [0, 0, 0, 255]);
        assert_eq!(image.pixel(2, 6), [0, 0, 0, 255]);
        assert_eq!(image.pixel(2, 1), [0, 0, 0, 255]);
    }

    #[test]
    fn lifecycle_and_sprites_are_rejected_and_abort_drops_frame() {
        let mut renderer = CpuRasterRenderer::new(BoxAtlas { width_ratio: 0.5 }, [0; 4]);
        assert_eq!(renderer.submit(&[]), Err(RasterError::Lifecycle));
        assert_eq!(renderer.end_frame(), Err(RasterError::Lifecycle));
        renderer.begin_frame(viewport(2.0, 2.0)).unwrap();
        assert_eq!(
            renderer.begin_frame(viewport(2.0, 2.0)),
            Err(RasterError::Lifecycle)
        );
        assert_eq!(
            renderer.submit(&[Batch::Sprites(&[])]),
            Err(RasterError::SpritesUnsupported)
        );
        renderer.abort_frame();
        assert!(renderer.finished().is_none());
    }
}
