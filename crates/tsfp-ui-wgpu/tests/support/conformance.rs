//! Independent CPU mock: pixel-centre coverage, nearest texture lookup and
//! straight source-over. Synthetic spec oracle, never the lifted game oracle.
use super::*;
use std::collections::HashMap;

struct MockRenderer {
    frame: Frame,
    active: bool,
    clip: Rect,
    blend: BlendMode,
    images: HashMap<TextureId, (u32, u32, Vec<u8>)>,
    glyphs: HashMap<GlyphKey, AtlasGlyph>,
}
impl MockRenderer {
    fn new(w: u32, h: u32) -> Self {
        Self {
            frame: expected(w, h, |_, _| [0; 4]),
            active: false,
            clip: rect(0., 0., w as f32, h as f32),
            blend: BlendMode::Alpha,
            images: HashMap::new(),
            glyphs: HashMap::new(),
        }
    }
    fn draw(&mut self, sprite: Sprite, textured: bool) -> Result<(), Error> {
        let r = sprite.rect_px;
        if ![r.origin.x, r.origin.y, r.size.x, r.size.y]
            .iter()
            .all(|v| v.is_finite())
            || r.size.x < 0.
            || r.size.y < 0.
            || !sprite.color.0.iter().all(|v| v.is_finite())
            || !(0. ..=1.).contains(&sprite.color.0[3])
        {
            return Err(Error::Geometry);
        }
        let image = if textured {
            Some(
                self.images
                    .get(&sprite.texture)
                    .ok_or(Error::MissingTexture(sprite.texture))?,
            )
        } else {
            None
        };
        for y in 0..self.frame.height {
            for x in 0..self.frame.width {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                if px < r.origin.x
                    || px >= r.origin.x + r.size.x
                    || py < r.origin.y
                    || py >= r.origin.y + r.size.y
                    || x < self.clip.origin.x as u32
                    || y < self.clip.origin.y as u32
                    || x >= (self.clip.origin.x + self.clip.size.x) as u32
                    || y >= (self.clip.origin.y + self.clip.size.y) as u32
                {
                    continue;
                }
                let mut source = sprite.color.0;
                if let Some((w, h, rgba)) = image {
                    let u = sprite.uv.min.x
                        + (sprite.uv.max.x - sprite.uv.min.x) * (px - r.origin.x) / r.size.x;
                    let v = sprite.uv.min.y
                        + (sprite.uv.max.y - sprite.uv.min.y) * (py - r.origin.y) / r.size.y;
                    let tx = ((u * *w as f32).floor() as u32).min(w - 1);
                    let ty = ((v * *h as f32).floor() as u32).min(h - 1);
                    for c in 0..4 {
                        source[c] *= rgba[((ty * w + tx) * 4) as usize + c] as f32 / 255.;
                    }
                }
                // Software Vulkan UNORM blend factors quantise source alpha.
                // This explicit policy is tested separately at alpha 0, 1, 0.5.
                let a = (source[3].clamp(0., 1.) * 255.).round() / 255.;
                let offset = ((y * self.frame.width + x) * 4) as usize;
                for c in 0..4 {
                    let dst = self.frame.rgba[offset + c] as f32 / 255.;
                    let out = match (self.blend, c) {
                        // T2309 additive: src * a added, destination alpha kept.
                        (BlendMode::Additive, 3) => dst,
                        (BlendMode::Additive, _) => source[c].clamp(0., 1.) * a + dst,
                        (BlendMode::Alpha, 3) => a + dst * (1. - a),
                        (BlendMode::Alpha, _) => source[c].clamp(0., 1.) * a + dst * (1. - a),
                    };
                    self.frame.rgba[offset + c] = (out.clamp(0., 1.) * 255.).round() as u8;
                }
            }
        }
        Ok(())
    }
}
impl UiRenderer for MockRenderer {
    type Error = Error;
    fn begin_frame(&mut self, v: Viewport) -> Result<(), Error> {
        if self.active {
            return Err(Error::Lifecycle);
        }
        if v.size_px() != Vec2::new(self.frame.width as f32, self.frame.height as f32) {
            return Err(Error::Target);
        }
        self.active = true;
        self.clip = rect(0., 0., self.frame.width as f32, self.frame.height as f32);
        self.blend = BlendMode::Alpha;
        Ok(())
    }
    fn submit(&mut self, batches: &[Batch<'_>]) -> Result<(), Error> {
        if !self.active {
            return Err(Error::Lifecycle);
        }
        for batch in batches {
            match batch {
                Batch::Quads(qs) => {
                    for q in *qs {
                        self.draw(
                            Sprite {
                                rect_px: q.rect_px,
                                color: q.color,
                                texture: TextureId(0),
                                uv: full_uv(),
                            },
                            false,
                        )?;
                    }
                }
                Batch::Sprites(ss) => {
                    for s in *ss {
                        self.draw(*s, true)?;
                    }
                }
                Batch::Polys(_) => {}
                Batch::Blend(mode) => self.blend = *mode,
                Batch::Clip(c) => {
                    self.clip = match c {
                        None => rect(0., 0., self.frame.width as f32, self.frame.height as f32),
                        Some(r) => {
                            let x = r.origin.x.clamp(0., self.frame.width as f32).floor();
                            let y = r.origin.y.clamp(0., self.frame.height as f32).floor();
                            let ex = (r.origin.x + r.size.x)
                                .clamp(0., self.frame.width as f32)
                                .ceil();
                            let ey = (r.origin.y + r.size.y)
                                .clamp(0., self.frame.height as f32)
                                .ceil();
                            rect(
                                x,
                                y,
                                if r.size.x == 0. { 0. } else { ex - x },
                                if r.size.y == 0. { 0. } else { ey - y },
                            )
                        }
                    };
                }
                Batch::Text(run) => {
                    for g in run.glyphs {
                        let key = GlyphKey {
                            font: run.font,
                            glyph: g.id,
                            raster_height_px: run.raster_height_px,
                        };
                        let glyph = *self.glyphs.get(&key).ok_or(Error::MissingGlyph(key))?;
                        self.draw(
                            Sprite {
                                rect_px: rect(
                                    run.baseline_px.x + g.offset_px.x + glyph.bearing_px.x,
                                    run.baseline_px.y + g.offset_px.y + glyph.bearing_px.y,
                                    glyph.size_px.x,
                                    glyph.size_px.y,
                                ),
                                color: run.color,
                                uv: glyph.uv,
                                texture: glyph.texture,
                            },
                            true,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
    fn end_frame(&mut self) -> Result<(), Error> {
        if !self.active {
            return Err(Error::Lifecycle);
        }
        self.active = false;
        Ok(())
    }
    fn abort_frame(&mut self) {
        self.active = false;
    }
}
fn full_uv() -> UvRect {
    UvRect {
        min: Vec2::default(),
        max: Vec2::new(1., 1.),
    }
}
fn key() -> GlyphKey {
    GlyphKey {
        font: FontId(3),
        glyph: GlyphId(42),
        raster_height_px: 12,
    }
}
fn glyph() -> AtlasGlyph {
    AtlasGlyph {
        texture: TextureId(9),
        uv: full_uv(),
        size_px: Vec2::new(4., 4.),
        bearing_px: Vec2::new(1., -3.),
        advance_px: 5.,
    }
}

/// One script, consumed through the public trait by both implementations.
fn script<R: UiRenderer<Error = Error>>(r: &mut R, v: Viewport) -> Result<(), Error> {
    r.begin_frame(v)?;
    let panel = v
        .resolve(AnchoredRect {
            space: AnchorSpace::SafeArea4x3,
            horizontal: Edge::Center,
            vertical: Edge::Center,
            offset: Vec2::default(),
            size: Vec2::new(24., 16.),
        })
        .unwrap();
    // Nested anchor: an explicitly supplied child viewport, then translate its
    // resolved local origin to window pixels; no implicit stack in the contract.
    let nested = Viewport::new(
        panel.size,
        v.scale(),
        rect(0., 0., panel.size.x, panel.size.y),
    )
    .unwrap();
    let mut child = nested
        .resolve(AnchoredRect {
            space: AnchorSpace::SafeArea4x3,
            horizontal: Edge::End,
            vertical: Edge::End,
            offset: Vec2::new(-1., -1.),
            size: Vec2::new(4., 4.),
        })
        .unwrap();
    child.origin.x += panel.origin.x;
    child.origin.y += panel.origin.y;
    assert_eq!(
        child.origin,
        Vec2::new(
            panel.origin.x + panel.size.x - 5. * v.scale(),
            panel.origin.y + panel.size.y - 5. * v.scale()
        )
    );
    let edge = v
        .resolve(AnchoredRect {
            space: AnchorSpace::Window,
            horizontal: Edge::End,
            vertical: Edge::End,
            offset: Vec2::new(-2., -2.),
            size: Vec2::new(4., 4.),
        })
        .unwrap();
    r.submit(&[
        Batch::Quads(&[Quad {
            rect_px: rect(0., 0., v.size_px().x, v.size_px().y),
            color: Color([0., 0., 1., 1.]),
        }]),
        Batch::Clip(Some(panel)),
        Batch::Quads(&[Quad {
            rect_px: panel,
            color: Color([1., 0., 0., 0.5]),
        }]),
        Batch::Sprites(&[Sprite {
            rect_px: child,
            uv: full_uv(),
            texture: TextureId(7),
            color: Color([0.5, 1., 0.5, 1.]),
        }]),
        Batch::Text(TextRun {
            font: key().font,
            raster_height_px: 12,
            baseline_px: Vec2::new(panel.origin.x + 2., panel.origin.y + 5.),
            color: Color([1., 1., 0., 1.]),
            glyphs: &[PositionedGlyph {
                id: key().glyph,
                offset_px: Vec2::new(2., 0.),
            }],
        }),
        Batch::Clip(None),
        Batch::Quads(&[
            Quad {
                rect_px: edge,
                color: Color([0., 1., 0., 1.]),
            },
            Quad {
                rect_px: edge,
                color: Color([1., 0., 0., 0.]),
            },
        ]),
        Batch::Clip(Some(rect(2.5, 3., 0., 5.))),
        Batch::Quads(&[Quad {
            rect_px: panel,
            color: Color([1.; 4]),
        }]),
        Batch::Clip(None),
    ])?;
    r.end_frame()
}
#[test]
fn mock_wgpu_conformance_aspect_scale_and_stable_hashes() {
    let gpu = GpuContext::new(&Preference::Software).expect("software GPU required");
    println!("conformance {}", gpu.describe());
    let hashes = [
        [0xef88c0fed6a5b039, 0xc0f1a6cab37657f9, 0xa20806749f461939],
        [0xd3f2932d2ef21d85, 0x935045dcd39d5245, 0x39d31d14cd2c0e85],
        [0x161f9228a8a5d956, 0xc364f0a55fe34016, 0x794ad691ea0d7656],
    ];
    for (aspect, (w, h)) in [(192, 108), (252, 108), (384, 108)].into_iter().enumerate() {
        for (dpi, scale) in [1., 2., 3.].into_iter().enumerate() {
            let v = Viewport::new(
                Vec2::new(w as f32, h as f32),
                scale,
                rect((w - 144) as f32 / 2., 0., 144., 108.),
            )
            .unwrap();
            let texture = target(&gpu, w, h);
            let mut ui =
                WgpuUiRenderer::new(&gpu.device, &gpu.queue, &texture, UiBackend::Wgpu).unwrap();
            ui.upload_rgba(TextureId(7), 1, 1, &[255; 4]).unwrap();
            ui.upload_coverage(TextureId(9), 2, 1, &[255, 0]).unwrap();
            ui.register_glyph(key(), glyph()).unwrap();
            let mut mock = MockRenderer::new(w, h);
            mock.images.insert(TextureId(7), (1, 1, vec![255; 4]));
            mock.images.insert(
                TextureId(9),
                (2, 1, vec![255, 255, 255, 255, 255, 255, 255, 0]),
            );
            mock.glyphs.insert(key(), glyph());
            assert_eq!(script(&mut mock, v), script(&mut ui, v));
            let first = read(&gpu.device, &gpu.queue, &texture);
            assert_eq!(first.hash(), hashes[aspect][dpi], "pinned software fixture");
            check(
                &format!("conformance-{w}x{h}-scale{scale}"),
                first.clone(),
                mock.frame,
            );
            reset(&gpu, &texture);
            script(&mut ui, v).unwrap();
            assert_eq!(
                first.rgba,
                read(&gpu.device, &gpu.queue, &texture).rgba,
                "repeat is stable"
            );
        }
    }
}

#[test]
fn alpha_tint_nonfinite_and_atlas_growth_eviction_reupload() {
    let gpu = GpuContext::new(&Preference::Software).expect("software GPU required");
    let texture = target(&gpu, 64, 32);
    let mut ui = WgpuUiRenderer::new(&gpu.device, &gpu.queue, &texture, UiBackend::Wgpu).unwrap();
    let mut mock = MockRenderer::new(64, 32);
    ui.upload_rgba(TextureId(7), 1, 1, &[128, 64, 255, 255])
        .unwrap();
    mock.images
        .insert(TextureId(7), (1, 1, vec![128, 64, 255, 255]));
    let quads = [
        Quad {
            rect_px: rect(0., 0., 64., 32.),
            color: Color([0., 0., 1., 1.]),
        },
        Quad {
            rect_px: rect(0., 0., 8., 8.),
            color: Color([1., 0., 0., 0.]),
        },
        Quad {
            rect_px: rect(8., 0., 8., 8.),
            color: Color([1., 0., 0., 1.]),
        },
        Quad {
            rect_px: rect(16., 0., 8., 8.),
            color: Color([1., 0., 0., 0.5]),
        },
        // Looks premultiplied, but the contract interprets it as straight.
        Quad {
            rect_px: rect(24., 0., 8., 8.),
            color: Color([0.5, 0., 0., 0.5]),
        },
        Quad {
            rect_px: rect(32., 0., 8., 8.),
            color: Color([0., 1., 0., 0.5]),
        },
    ];
    let sprites = [Sprite {
        rect_px: rect(40., 0., 8., 8.),
        uv: full_uv(),
        texture: TextureId(7),
        color: Color([0.5, 0.5, 1., 1.]),
    }];
    let batches = [Batch::Quads(&quads), Batch::Sprites(&sprites)];
    mock.begin_frame(viewport(64, 32)).unwrap();
    ui.begin_frame(viewport(64, 32)).unwrap();
    assert_eq!(mock.submit(&batches), ui.submit(&batches));
    mock.end_frame().unwrap();
    ui.end_frame().unwrap();
    let actual = read(&gpu.device, &gpu.queue, &texture);
    assert_eq!(actual.pixel(0, 0), [0, 0, 255, 255]);
    assert_eq!(actual.pixel(8, 0), [255, 0, 0, 255]);
    assert_eq!(actual.pixel(16, 0), [128, 0, 127, 255]);
    assert_eq!(actual.pixel(24, 0), [64, 0, 127, 255]);
    assert_eq!(actual.pixel(40, 0), [64, 32, 255, 255]);
    check("alpha-tint-straight", actual, mock.frame);

    reset(&gpu, &texture);
    ui.begin_frame(viewport(64, 32)).unwrap();
    ui.submit(&[Batch::Quads(&[
        Quad {
            rect_px: rect(0., 0., 8., 8.),
            color: Color([1., 0., 0., 0.5]),
        },
        Quad {
            rect_px: rect(4., 0., 4., 8.),
            color: Color([0., 1., 0., 0.5]),
        },
    ])])
    .unwrap();
    ui.end_frame().unwrap();
    check(
        "alpha-transparent-destination",
        read(&gpu.device, &gpu.queue, &texture),
        expected(64, 32, |x, y| {
            if x < 4 && y < 8 {
                [128, 0, 0, 128]
            } else if x < 8 && y < 8 {
                [64, 128, 0, 192]
            } else {
                [0; 4]
            }
        }),
    );

    check_nonfinite_geometry(&mut ui, quads[0], sprites[0]);
    check_atlas_lifecycle(&gpu, &texture, &mut ui, sprites[0]);
}

/// Invalid partial batches remain invisible after aborting the active frame.
fn check_nonfinite_geometry(ui: &mut WgpuUiRenderer<'_>, quad: Quad, sprite: Sprite) {
    // Finite checks cover each tint channel, geometry, UV, glyph metrics and
    // text baseline/position. Abort makes partial submissions unobservable.
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for channel in 0..4 {
            let mut color = Color([1.; 4]);
            color.0[channel] = bad;
            ui.begin_frame(viewport(64, 32)).unwrap();
            assert_eq!(
                ui.submit(&[Batch::Quads(&[Quad { color, ..quad }])]),
                Err(Error::Geometry)
            );
            ui.abort_frame();
        }
        ui.begin_frame(viewport(64, 32)).unwrap();
        assert_eq!(
            ui.submit(&[Batch::Sprites(&[Sprite {
                uv: UvRect {
                    min: Vec2::new(bad, 0.),
                    max: Vec2::new(1., 1.)
                },
                ..sprite
            }])]),
            Err(Error::Geometry)
        );
        ui.abort_frame();
        ui.begin_frame(viewport(64, 32)).unwrap();
        assert_eq!(
            ui.submit(&[Batch::Quads(&[Quad {
                rect_px: Rect {
                    origin: Vec2::new(bad, 0.),
                    size: Vec2::new(1., 1.)
                },
                ..quad
            }])]),
            Err(Error::Geometry)
        );
        ui.abort_frame();
    }
    for alpha in [-0.1, 1.1] {
        ui.begin_frame(viewport(64, 32)).unwrap();
        assert_eq!(
            ui.submit(&[Batch::Quads(&[Quad {
                color: Color([1., 1., 1., alpha]),
                ..quad
            }])]),
            Err(Error::Geometry)
        );
        ui.abort_frame();
    }
}

/// Growth, eviction and same-ID replacement retain their original submission order.
fn check_atlas_lifecycle(
    gpu: &GpuContext,
    texture: &wgpu::Texture,
    ui: &mut WgpuUiRenderer<'_>,
    sprite: Sprite,
) {
    // Growth adds independent pages without moving existing regions.
    ui.upload_coverage(TextureId(9), 2, 1, &[255, 0]).unwrap();
    ui.register_glyph(key(), glyph()).unwrap();
    for page in 10..42 {
        ui.upload_coverage(TextureId(page), 1, 1, &[255]).unwrap();
        ui.register_glyph(
            GlyphKey {
                glyph: GlyphId(page as u32),
                ..key()
            },
            AtlasGlyph {
                texture: TextureId(page),
                ..glyph()
            },
        )
        .unwrap();
    }
    assert_eq!(ui.atlas().glyph(key()), Some(glyph()));
    ui.begin_frame(viewport(64, 32)).unwrap();
    assert_eq!(ui.evict_page(TextureId(9)), Err(Error::Lifecycle));
    assert_eq!(ui.register_glyph(key(), glyph()), Err(Error::Lifecycle));
    assert_eq!(
        ui.upload_coverage(TextureId(9), 2, 1, &[0, 255]),
        Err(Error::Lifecycle)
    );
    let run = TextRun {
        font: key().font,
        raster_height_px: 12,
        baseline_px: Vec2::new(2., 5.),
        color: Color([1.; 4]),
        glyphs: &[PositionedGlyph {
            id: key().glyph,
            offset_px: Vec2::default(),
        }],
    };
    ui.submit(&[Batch::Text(run)]).unwrap();
    ui.end_frame().unwrap();
    assert_eq!(ui.evict_page(TextureId(9)), Ok(true));
    assert_eq!(ui.evict_page(TextureId(9)), Ok(false));
    assert_eq!(ui.prepare_glyphs(&[key()]), Err(Error::MissingGlyph(key())));
    assert!(
        ui.atlas()
            .glyph(GlyphKey {
                glyph: GlyphId(10),
                ..key()
            })
            .is_some()
    );
    ui.begin_frame(viewport(64, 32)).unwrap();
    assert_eq!(
        ui.submit(&[Batch::Text(run)]),
        Err(Error::MissingGlyph(key()))
    );
    assert_eq!(
        ui.submit(&[Batch::Sprites(&[Sprite {
            texture: TextureId(9),
            ..sprite
        }])]),
        Err(Error::MissingTexture(TextureId(9)))
    );
    ui.abort_frame();
    // Same-ID reupload requires re-registration, and new texels reach the GPU.
    ui.upload_coverage(TextureId(9), 2, 1, &[0, 255]).unwrap();
    assert_eq!(ui.atlas().glyph(key()), None);
    ui.register_glyph(key(), glyph()).unwrap();
    for bad in [f32::NAN, f32::INFINITY] {
        assert_eq!(
            ui.register_glyph(
                key(),
                AtlasGlyph {
                    advance_px: bad,
                    ..glyph()
                }
            ),
            Err(Error::Geometry)
        );
        ui.begin_frame(viewport(64, 32)).unwrap();
        assert_eq!(
            ui.submit(&[Batch::Text(TextRun {
                baseline_px: Vec2::new(bad, 0.),
                ..run
            })]),
            Err(Error::Geometry)
        );
        ui.abort_frame();
        ui.begin_frame(viewport(64, 32)).unwrap();
        assert_eq!(
            ui.submit(&[Batch::Text(TextRun {
                glyphs: &[PositionedGlyph {
                    id: key().glyph,
                    offset_px: Vec2::new(bad, 0.)
                }],
                ..run
            })]),
            Err(Error::Geometry)
        );
        ui.abort_frame();
    }
    reset(gpu, texture);
    ui.begin_frame(viewport(64, 32)).unwrap();
    ui.submit(&[Batch::Text(run)]).unwrap();
    ui.end_frame().unwrap();
    check(
        "atlas-evict-reupload",
        read(&gpu.device, &gpu.queue, texture),
        expected(64, 32, |x, y| {
            if (5..7).contains(&x) && (2..6).contains(&y) {
                [255; 4]
            } else {
                [0; 4]
            }
        }),
    );
    // Same page grows in place: old UVs no longer describe its new dimensions.
    ui.upload_coverage(TextureId(9), 4, 1, &[255, 0, 0, 0])
        .unwrap();
    assert_eq!(ui.atlas().glyph(key()), None);
    let grown = AtlasGlyph {
        uv: UvRect {
            min: Vec2::default(),
            max: Vec2::new(0.5, 1.),
        },
        ..glyph()
    };
    ui.register_glyph(key(), grown).unwrap();
    reset(gpu, texture);
    ui.begin_frame(viewport(64, 32)).unwrap();
    ui.submit(&[Batch::Text(run)]).unwrap();
    ui.end_frame().unwrap();
    check(
        "atlas-grow-reregister",
        read(&gpu.device, &gpu.queue, texture),
        expected(64, 32, |x, y| {
            if (3..5).contains(&x) && (2..6).contains(&y) {
                [255; 4]
            } else {
                [0; 4]
            }
        }),
    );
}
