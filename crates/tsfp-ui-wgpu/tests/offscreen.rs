//! FABRICATED UI batches, MEASURED software-GPU output. No game parity evidence.
//! These tests require a software adapter and fail rather than silently skip.
use tsfp_ui_renderer::*;
use tsfp_ui_wgpu::{Error, UiBackend, WgpuUiRenderer};
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::{Frame, Replayer};

mod support;
use support::rect;

fn viewport(w: u32, h: u32) -> Viewport {
    Viewport::new(
        Vec2::new(w as f32, h as f32),
        1.,
        rect(16., 0., (w - 32) as f32, h as f32),
    )
    .unwrap()
}
fn target(gpu: &GpuContext, w: u32, h: u32) -> wgpu::Texture {
    support::target(gpu, "ui-test", (w, h), [0; 4])
}
fn reset(gpu: &GpuContext, texture: &wgpu::Texture) {
    support::reset(gpu, texture, [0; 4]);
}
fn read(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Frame {
    let pitch = (texture.width() * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ui-readback"),
        size: (pitch * texture.height()) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pitch),
                rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap()
        });
    device.poll(wgpu::Maintain::Wait);
    receiver.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let rgba = mapped
        .chunks_exact(pitch as usize)
        .flat_map(|row| row[..texture.width() as usize * 4].iter().copied())
        .collect();
    drop(mapped);
    buffer.unmap();
    Frame {
        width: texture.width(),
        height: texture.height(),
        rgba,
    }
}
fn expected(w: u32, h: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Frame {
    let mut rgba = Vec::new();
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&pixel(x, y));
        }
    }
    Frame {
        width: w,
        height: h,
        rgba,
    }
}
fn check(label: &str, actual: Frame, expected: Frame) {
    println!("{label} hash={:016x}", actual.hash());
    assert_eq!(actual.hash(), expected.hash(), "{label}");
    assert_eq!(actual.rgba, expected.rgba, "{label}");
}

#[test]
fn fabricated_glyph_table_reload_is_atomic() {
    let gpu = GpuContext::new(&Preference::Software).expect("software GPU required");
    let texture = target(&gpu, 64, 32);
    let mut renderer =
        WgpuUiRenderer::new(&gpu.device, &gpu.queue, &texture, UiBackend::Wgpu).unwrap();
    renderer
        .upload_coverage(TextureId(7), 1, 1, &[255])
        .unwrap();
    let first = GlyphKey {
        font: FontId(1),
        glyph: GlyphId(1),
        raster_height_px: 12,
    };
    let second = GlyphKey {
        font: FontId(1),
        glyph: GlyphId(2),
        raster_height_px: 12,
    };
    let original = AtlasGlyph {
        texture: TextureId(7),
        uv: UvRect {
            min: Vec2::default(),
            max: Vec2::new(1., 1.),
        },
        size_px: Vec2::new(1., 1.),
        bearing_px: Vec2::default(),
        advance_px: 1.,
    };
    renderer.register_glyphs(&[(first, original)]).unwrap();
    let changed = AtlasGlyph {
        advance_px: 2.,
        ..original
    };
    let invalid = AtlasGlyph {
        texture: TextureId(8),
        ..original
    };
    assert_eq!(
        renderer.register_glyphs(&[(first, changed), (second, invalid)]),
        Err(Error::MissingTexture(TextureId(8)))
    );
    assert_eq!(renderer.atlas().glyph(first), Some(original));
    assert_eq!(renderer.atlas().glyph(second), None);
    renderer
        .register_glyphs(&[(first, changed), (second, original)])
        .unwrap();
    assert_eq!(renderer.atlas().glyph(first), Some(changed));
    assert_eq!(renderer.atlas().glyph(second), Some(original));
    renderer.begin_frame(viewport(64, 32)).unwrap();
    assert_eq!(
        renderer.register_glyphs(&[(first, original)]),
        Err(Error::Lifecycle)
    );
    renderer.abort_frame();
    assert_eq!(renderer.atlas().glyph(first), Some(changed));
}
#[test]
fn software_batches_hashes_and_lifecycle() {
    println!("adapters: {:?}", tsfp_wgpu::context::list_adapters());
    let gpu =
        GpuContext::new(&Preference::Software).expect("software GPU required (no silent skip)");
    println!("{}", gpu.describe());
    let texture = target(&gpu, 64, 32);
    assert!(matches!(
        WgpuUiRenderer::new(&gpu.device, &gpu.queue, &texture, UiBackend::default()),
        Err(Error::Disabled)
    ));
    let mut renderer =
        WgpuUiRenderer::new(&gpu.device, &gpu.queue, &texture, UiBackend::Wgpu).unwrap();
    assert_eq!(renderer.submit(&[]), Err(Error::Lifecycle));
    assert_eq!(renderer.end_frame(), Err(Error::Lifecycle));
    assert_eq!(renderer.begin_frame(viewport(96, 32)), Err(Error::Target));
    renderer.begin_frame(viewport(64, 32)).unwrap();
    assert_eq!(
        renderer.begin_frame(viewport(64, 32)),
        Err(Error::Lifecycle)
    );
    renderer
        .submit(&[Batch::Quads(&[Quad {
            rect_px: rect(0., 0., 64., 32.),
            color: Color([0., 0., 1., 1.]),
        }])])
        .unwrap();
    renderer
        .submit(&[
            Batch::Clip(Some(rect(8., 4., 16., 8.))),
            Batch::Quads(&[Quad {
                rect_px: rect(0., 0., 32., 16.),
                color: Color([1., 0., 0., 0.5]),
            }]),
            Batch::Clip(None),
        ])
        .unwrap();
    renderer
        .anchored_quad(
            AnchoredRect {
                space: AnchorSpace::Window,
                horizontal: Edge::End,
                vertical: Edge::End,
                offset: Vec2::new(-2., -2.),
                size: Vec2::new(4., 4.),
            },
            Color([0., 1., 0., 1.]),
        )
        .unwrap();
    renderer
        .anchored_quad(
            AnchoredRect {
                space: AnchorSpace::SafeArea4x3,
                horizontal: Edge::Start,
                vertical: Edge::Start,
                offset: Vec2::default(),
                size: Vec2::new(4., 4.),
            },
            Color([1., 1., 1., 1.]),
        )
        .unwrap();
    renderer.end_frame().unwrap();
    check(
        "blend-clip-anchors",
        read(&gpu.device, &gpu.queue, &texture),
        expected(64, 32, |x, y| {
            if (58..62).contains(&x) && (26..30).contains(&y) {
                [0, 255, 0, 255]
            } else if (16..20).contains(&x) && y < 4 {
                [255; 4]
            } else if (8..24).contains(&x) && (4..12).contains(&y) {
                [128, 0, 127, 255]
            } else {
                [0, 0, 255, 255]
            }
        }),
    );

    let sprites = check_sprite_phase(&gpu, &texture, &mut renderer);
    let key = check_glyph_phase(&gpu, &texture, &mut renderer);
    check_abort_phase(&gpu, &texture, &mut renderer, &sprites, key);
}

/// Check flipped sprite texels and empty clipping on the same renderer.
fn check_sprite_phase(
    gpu: &GpuContext,
    texture: &wgpu::Texture,
    renderer: &mut WgpuUiRenderer<'_>,
) -> [Sprite; 1] {
    reset(gpu, texture);
    renderer
        .upload_rgba(TextureId(7), 2, 1, &[255, 0, 0, 255, 0, 255, 0, 255])
        .unwrap();
    renderer.begin_frame(viewport(64, 32)).unwrap();
    let sprites = [Sprite {
        rect_px: rect(0., 0., 16., 8.),
        uv: UvRect {
            min: Vec2::new(1., 0.),
            max: Vec2::new(0., 1.),
        },
        color: Color([1.; 4]),
        texture: TextureId(7),
    }];
    renderer
        .submit(&[
            Batch::Sprites(&sprites),
            Batch::Clip(Some(rect(2.5, 2., 0., 4.))),
            Batch::Quads(&[Quad {
                rect_px: rect(0., 0., 64., 32.),
                color: Color([1.; 4]),
            }]),
        ])
        .unwrap();
    assert_eq!(
        renderer.upload_rgba(TextureId(7), 1, 1, &[255; 4]),
        Err(Error::Lifecycle)
    );
    renderer.end_frame().unwrap();
    check(
        "sprite-flip-empty-clip",
        read(&gpu.device, &gpu.queue, texture),
        expected(64, 32, |x, y| {
            if y < 8 && x < 8 {
                [0, 255, 0, 255]
            } else if y < 8 && x < 16 {
                [255, 0, 0, 255]
            } else {
                [0; 4]
            }
        }),
    );

    sprites
}

/// Check atlas coverage, bearings and raster-size-specific lookup.
fn check_glyph_phase(
    gpu: &GpuContext,
    texture: &wgpu::Texture,
    renderer: &mut WgpuUiRenderer<'_>,
) -> GlyphKey {
    reset(gpu, texture);
    let key = GlyphKey {
        font: FontId(3),
        glyph: GlyphId(42),
        raster_height_px: 12,
    };
    renderer
        .upload_coverage(TextureId(9), 2, 1, &[255, 0])
        .unwrap();
    let glyph = AtlasGlyph {
        texture: TextureId(9),
        uv: UvRect {
            min: Vec2::default(),
            max: Vec2::new(1., 1.),
        },
        size_px: Vec2::new(4., 4.),
        bearing_px: Vec2::new(1., -3.),
        advance_px: 5.,
    };
    renderer.register_glyph(key, glyph).unwrap();
    assert_eq!(renderer.atlas().glyph(key), Some(glyph));
    renderer.prepare_glyphs(&[key]).unwrap();
    let wrong_size = GlyphKey {
        raster_height_px: 13,
        ..key
    };
    assert_eq!(
        renderer.prepare_glyphs(&[wrong_size]),
        Err(Error::MissingGlyph(wrong_size))
    );
    assert_eq!(
        renderer.register_glyph(
            GlyphKey {
                raster_height_px: 0,
                ..key
            },
            glyph
        ),
        Err(Error::Geometry)
    );
    renderer.begin_frame(viewport(64, 32)).unwrap();
    renderer
        .submit(&[Batch::Text(TextRun {
            font: FontId(3),
            raster_height_px: 12,
            glyphs: &[PositionedGlyph {
                id: GlyphId(42),
                offset_px: Vec2::new(2., 0.),
            }],
            baseline_px: Vec2::new(10., 10.),
            color: Color([1., 1., 0., 1.]),
        })])
        .unwrap();
    renderer.end_frame().unwrap();
    check(
        "glyph-coverage-bearing",
        read(&gpu.device, &gpu.queue, texture),
        expected(64, 32, |x, y| {
            if (13..15).contains(&x) && (7..11).contains(&y) {
                [255, 255, 0, 255]
            } else {
                [0; 4]
            }
        }),
    );

    key
}

/// Check failed submission/abort leaves the target intact and invalidates replaced glyphs.
fn check_abort_phase(
    gpu: &GpuContext,
    texture: &wgpu::Texture,
    renderer: &mut WgpuUiRenderer<'_>,
    sprites: &[Sprite; 1],
    key: GlyphKey,
) {
    reset(gpu, texture);
    renderer.begin_frame(viewport(64, 32)).unwrap();
    renderer
        .submit(&[Batch::Quads(&[Quad {
            rect_px: rect(0., 0., 64., 32.),
            color: Color([1.; 4]),
        }])])
        .unwrap();
    assert_eq!(
        renderer.submit(&[Batch::Sprites(&[Sprite {
            texture: TextureId(99),
            ..sprites[0]
        }])]),
        Err(Error::MissingTexture(TextureId(99)))
    );
    renderer.abort_frame();
    check(
        "abort-no-submit",
        read(&gpu.device, &gpu.queue, texture),
        expected(64, 32, |_, _| [0; 4]),
    );
    renderer.begin_frame(viewport(64, 32)).unwrap();
    assert_eq!(
        renderer.submit(&[Batch::Quads(&[Quad {
            rect_px: Rect {
                origin: Vec2::new(f32::NAN, 0.),
                size: Vec2::new(1., 1.)
            },
            color: Color([1.; 4])
        }])]),
        Err(Error::Geometry)
    );
    renderer.abort_frame();
    renderer.upload_rgba(TextureId(9), 1, 1, &[255; 4]).unwrap();
    assert_eq!(
        renderer.atlas().glyph(key),
        None,
        "replaced page invalidates glyph regions"
    );
}

#[test]
fn replayer_offscreen_attachment_preserves_scene_and_resizes() {
    let gpu = GpuContext::new(&Preference::Software).expect("software GPU required");
    let mut replay = Replayer::new(gpu, 640, 480);
    replay.set_output_size(64, 32);
    replay.set_window_size(Some((96, 32)));
    replay.run(&tsfp_wgpu::scenes::title_scene());
    let (device, queue) = replay.device_and_queue();
    let before = read(device, queue, replay.display_texture().unwrap());
    let mut ui = WgpuUiRenderer::from_replayer(&replay, UiBackend::Wgpu).unwrap();
    ui.begin_frame(viewport(96, 32)).unwrap();
    ui.submit(&[Batch::Quads(&[Quad {
        rect_px: rect(90., 26., 4., 4.),
        color: Color([1., 0., 1., 1.]),
    }])])
    .unwrap();
    ui.end_frame().unwrap();
    let after = read(device, queue, replay.display_texture().unwrap());
    let expected = expected(96, 32, |x, y| {
        if (90..94).contains(&x) && (26..30).contains(&y) {
            [255, 0, 255, 255]
        } else {
            before.pixel(x, y)
        }
    });
    check("replayer-overlay-load", after, expected);
    drop(ui);
    replay.set_window_size(Some((128, 64)));
    replay.run(&tsfp_wgpu::scenes::title_scene());
    let mut ui = WgpuUiRenderer::from_replayer(&replay, UiBackend::Wgpu).unwrap();
    assert_eq!(ui.begin_frame(viewport(96, 32)), Err(Error::Target));
    ui.begin_frame(viewport(128, 64)).unwrap();
    ui.end_frame().unwrap();
}

#[path = "support/conformance.rs"]
mod conformance;
