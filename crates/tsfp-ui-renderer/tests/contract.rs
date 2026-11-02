use tsfp_ui_renderer::*;

fn viewport() -> Viewport {
    Viewport::new(
        Vec2::new(1920.0, 1080.0),
        2.25,
        Rect::new(Vec2::new(240.0, 0.0), Vec2::new(1440.0, 1080.0)).unwrap(),
    )
    .unwrap()
}
#[test]
fn layout_distinguishes_safe_area_and_window_edges() {
    let mut rect = AnchoredRect {
        space: AnchorSpace::SafeArea4x3,
        horizontal: Edge::End,
        vertical: Edge::End,
        offset: Vec2::new(-10.0, -20.0),
        size: Vec2::new(100.0, 40.0),
    };
    assert_eq!(
        viewport().resolve(rect).unwrap(),
        Rect::new(Vec2::new(1432.5, 945.0), Vec2::new(225.0, 90.0)).unwrap()
    );
    rect.space = AnchorSpace::Window;
    assert_eq!(viewport().resolve(rect).unwrap().origin.x, 1672.5);
    rect.horizontal = Edge::Center;
    rect.vertical = Edge::Center;
    rect.offset = Vec2::default();
    assert_eq!(
        viewport().resolve(rect).unwrap().origin,
        Vec2::new(847.5, 495.0)
    );
}
#[test]
fn viewport_edge_grid_preserves_translation_and_scale() {
    for scale in [0.5, 1.0, 1.5, 2.0, 3.0] {
        let safe = Rect::new(Vec2::new(120.0, 30.0), Vec2::new(960.0, 720.0)).unwrap();
        let view = Viewport::new(Vec2::new(1200.0, 800.0), scale, safe).unwrap();
        for space in [AnchorSpace::SafeArea4x3, AnchorSpace::Window] {
            let area = if space == AnchorSpace::Window {
                Rect::new(Vec2::default(), view.size_px()).unwrap()
            } else {
                safe
            };
            for (edge, fraction) in [(Edge::Start, 0.0), (Edge::Center, 0.5), (Edge::End, 1.0)] {
                for offset in [-12.0, 0.0, 12.0] {
                    let rect = AnchoredRect {
                        space,
                        horizontal: edge,
                        vertical: edge,
                        offset: Vec2::new(offset, offset),
                        size: Vec2::new(80.0, 40.0),
                    };
                    let result = view.resolve(rect).unwrap();
                    assert_eq!(result.size, Vec2::new(80.0 * scale, 40.0 * scale));
                    assert_eq!(
                        result.origin.x,
                        area.origin.x + (area.size.x - result.size.x) * fraction + offset * scale
                    );
                    assert_eq!(
                        result.origin.y,
                        area.origin.y + (area.size.y - result.size.y) * fraction + offset * scale
                    );
                }
            }
        }
    }
}
#[test]
fn invalid_layout_and_overflow_are_rejected() {
    let full = Rect::new(Vec2::default(), Vec2::new(640.0, 480.0)).unwrap();
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(Viewport::new(full.size, scale, full).is_err());
    }
    assert!(Rect::new(Vec2::default(), Vec2::new(-1.0, 1.0)).is_err());
    assert!(Rect::new(Vec2::new(f32::NAN, 0.0), Vec2::default()).is_err());
    assert!(Viewport::new(Vec2::new(320.0, 240.0), 1.0, full).is_err());
    assert!(
        viewport()
            .resolve(AnchoredRect {
                space: AnchorSpace::Window,
                horizontal: Edge::Start,
                vertical: Edge::Start,
                offset: Vec2::default(),
                size: Vec2::new(f32::MAX, 1.0)
            })
            .is_err()
    );
}

#[derive(Default)]
struct Mock {
    active: bool,
    order: Vec<&'static str>,
}
impl LegacyDrawSink for Mock {
    type Error = &'static str;
    fn begin_ui(&mut self, _: Viewport) -> Result<(), Self::Error> {
        if self.active {
            return Err("already active");
        }
        self.active = true;
        Ok(())
    }
    fn emit_ui(&mut self, batch: Batch<'_>) -> Result<(), Self::Error> {
        if !self.active {
            return Err("not active");
        }
        self.order.push(match batch {
            Batch::Quads(_) => "quad",
            Batch::Sprites(_) => "sprite",
            Batch::Polys(_) => "poly",
            Batch::Text(_) => "text",
            Batch::Clip(Some(_)) => "clip",
            Batch::Clip(None) => "reset",
            Batch::Blend(_) => "blend",
        });
        Ok(())
    }
    fn end_ui(&mut self) -> Result<(), Self::Error> {
        if !self.active {
            return Err("not active");
        }
        self.active = false;
        Ok(())
    }
    fn abort_ui(&mut self) {
        self.active = false;
    }
}
impl UiRenderer for Mock {
    type Error = &'static str;
    fn begin_frame(&mut self, v: Viewport) -> Result<(), Self::Error> {
        self.begin_ui(v)
    }
    fn submit(&mut self, batches: &[Batch<'_>]) -> Result<(), Self::Error> {
        for batch in batches {
            self.emit_ui(*batch)?;
        }
        Ok(())
    }
    fn end_frame(&mut self) -> Result<(), Self::Error> {
        self.end_ui()
    }
    fn abort_frame(&mut self) {
        self.abort_ui();
    }
}
struct MockAtlas {
    prepared: Vec<GlyphKey>,
}
impl GlyphAtlas for MockAtlas {
    type Error = &'static str;
    fn prepare(&mut self, keys: &[GlyphKey]) -> Result<(), Self::Error> {
        if keys.iter().any(|key| key.raster_height_px == 0) {
            return Err("zero raster height");
        }
        self.prepared.extend_from_slice(keys);
        Ok(())
    }
    fn glyph(&self, key: GlyphKey) -> Option<AtlasGlyph> {
        self.prepared.contains(&key).then_some(AtlasGlyph {
            texture: TextureId(1),
            uv: UvRect {
                min: Vec2::default(),
                max: Vec2::new(1.0, 1.0),
            },
            size_px: Vec2::new(8.0, 12.0),
            bearing_px: Vec2::new(0.0, -10.0),
            advance_px: 9.0,
        })
    }
}
#[test]
fn mock_consumes_ordered_batches_and_recovers_after_abort() {
    let mut renderer = Mock::default();
    assert!(renderer.end_frame().is_err());
    renderer.begin_frame(viewport()).unwrap();
    assert!(renderer.begin_frame(viewport()).is_err());
    let rect = Rect::new(Vec2::default(), Vec2::new(10.0, 20.0)).unwrap();
    let color = Color([1.0; 4]);
    let quad = Quad {
        rect_px: rect,
        color,
    };
    let sprite = Sprite {
        rect_px: rect,
        color,
        texture: TextureId(1),
        uv: UvRect {
            min: Vec2::default(),
            max: Vec2::new(1.0, 1.0),
        },
    };
    let glyph = PositionedGlyph {
        id: GlyphId(7),
        offset_px: Vec2::new(4.0, 0.0),
    };
    renderer
        .submit(&[
            Batch::Clip(Some(rect)),
            Batch::Quads(&[quad]),
            Batch::Sprites(&[sprite]),
            Batch::Text(TextRun {
                font: FontId(3),
                raster_height_px: 12,
                glyphs: &[glyph],
                baseline_px: Vec2::new(10.0, 20.0),
                color,
            }),
            Batch::Clip(None),
        ])
        .unwrap();
    assert_eq!(renderer.order, ["clip", "quad", "sprite", "text", "reset"]);
    renderer.end_frame().unwrap();
    assert!(renderer.submit(&[Batch::Quads(&[])]).is_err());
    renderer.begin_frame(viewport()).unwrap();
    renderer.abort_frame();
    renderer.begin_frame(viewport()).unwrap();
    renderer.submit(&[]).unwrap();
    renderer.end_frame().unwrap();
}
#[test]
fn atlas_lookup_is_explicit_and_raster_size_specific() {
    let mut atlas = MockAtlas { prepared: vec![] };
    let key = GlyphKey {
        font: FontId(3),
        glyph: GlyphId(7),
        raster_height_px: 12,
    };
    assert_eq!(atlas.glyph(key), None);
    atlas.prepare(&[key]).unwrap();
    assert_eq!(atlas.glyph(key).unwrap().advance_px, 9.0);
    assert_eq!(
        atlas.glyph(GlyphKey {
            raster_height_px: 24,
            ..key
        }),
        None
    );
    assert!(
        atlas
            .prepare(&[GlyphKey {
                raster_height_px: 0,
                ..key
            }])
            .is_err()
    );
}

#[test]
fn legacy_signature_adapter_forwards_order_errors_and_abort() {
    let mut adapter = LegacyAdapter(Mock::default());
    assert_eq!(adapter.end_frame(), Err("not active"));
    adapter.begin_frame(viewport()).unwrap();
    assert_eq!(adapter.begin_frame(viewport()), Err("already active"));
    adapter
        .submit(&[Batch::Quads(&[]), Batch::Clip(None), Batch::Sprites(&[])])
        .unwrap();
    assert_eq!(adapter.0.order, ["quad", "reset", "sprite"]);
    adapter.abort_frame();
    assert_eq!(adapter.submit(&[Batch::Clip(None)]), Err("not active"));
    adapter.begin_frame(viewport()).unwrap();
    adapter.end_frame().unwrap();
}

#[test]
fn full_size_aspects_and_dpi_keep_safe_area_and_window_edges_distinct() {
    for width in [1920., 2520., 3840.] {
        for scale in [2., 3.] {
            let safe_left = (width - 1440.) / 2.;
            let v = Viewport::new(
                Vec2::new(width, 1080.),
                scale,
                Rect::new(Vec2::new(safe_left, 0.), Vec2::new(1440., 1080.)).unwrap(),
            )
            .unwrap();
            for (horizontal, x_factor) in [(Edge::Start, 0.), (Edge::Center, 0.5), (Edge::End, 1.)]
            {
                for (vertical, y_factor) in
                    [(Edge::Start, 0.), (Edge::Center, 0.5), (Edge::End, 1.)]
                {
                    for (space, origin_x, reference_width) in [
                        (AnchorSpace::Window, 0., width),
                        (AnchorSpace::SafeArea4x3, safe_left, 1440.),
                    ] {
                        let r = v
                            .resolve(AnchoredRect {
                                space,
                                horizontal,
                                vertical,
                                offset: Vec2::new(-2., -4.),
                                size: Vec2::new(12., 8.),
                            })
                            .unwrap();
                        assert_eq!(r.size, Vec2::new(12. * scale, 8. * scale));
                        assert_eq!(
                            r.origin,
                            Vec2::new(
                                origin_x + (reference_width - 12. * scale) * x_factor - 2. * scale,
                                (1080. - 8. * scale) * y_factor - 4. * scale
                            )
                        );
                    }
                }
            }
        }
    }
}
