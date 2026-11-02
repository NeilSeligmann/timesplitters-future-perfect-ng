// SPDX-License-Identifier: GPL-3.0-or-later
//! Fabricated content on the real GPU; renderer contract evidence, not game/Tier2.
use std::time::{Duration, Instant};
use tsfp_gameplay::cutscene_presentation::{Extent, WidePresentation};
use tsfp_wgpu::{
    context::{GpuContext, Preference},
    cutscene::{CutsceneRenderer, Error, MovieFrame, Rect},
    readback::ReadbackRing,
    replay::Frame,
};

fn texture(gpu: &GpuContext, width: u32, height: u32, usage: wgpu::TextureUsages) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("T2170 synthetic"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage,
        view_formats: &[],
    })
}

fn read(gpu: &GpuContext, target: &wgpu::Texture, mut encoder: wgpu::CommandEncoder) -> Frame {
    let mut ring = ReadbackRing::default();
    let slot = ring
        .encode(
            &gpu.device,
            &mut encoder,
            target,
            (target.width(), target.height()),
        )
        .unwrap();
    gpu.queue.submit([encoder.finish()]);
    ring.map(slot, 1);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut frames = ring.poll(&gpu.device, 2);
        if let Some((_, frame)) = frames.pop() {
            return frame;
        }
        assert!(Instant::now() < deadline, "readback timeout");
        std::thread::yield_now();
    }
}

fn save(label: &str, frame: &Frame) {
    if let Ok(dir) = std::env::var("TSFP_CUTSCENE_READBACK_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(format!("{dir}/{label}.png"), frame.to_png()).unwrap();
    }
}

#[test]
fn native_movie_and_bars_readback() {
    let preference = if std::env::var_os("TSFP_CUTSCENE_REQUIRE_RADV").is_some() {
        Preference::Any
    } else {
        Preference::Software
    };
    let gpu = GpuContext::new(&preference).expect("GPU contract needs an adapter");
    println!("T2170 adapter {:?}", gpu.info);
    if std::env::var_os("TSFP_CUTSCENE_REQUIRE_RADV").is_some() {
        assert!(
            gpu.info.driver.to_lowercase().contains("radv"),
            "RADV evidence must fail closed"
        );
        assert_ne!(gpu.info.device_type, "Cpu");
    }
    let renderer = CutsceneRenderer::new(&gpu.device);
    let source = texture(
        &gpu,
        64,
        48,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    // Quadrants plus a two-pixel magenta border expose crop, mirroring and stretching.
    let pixels: Vec<u8> = (0..48)
        .flat_map(|y| {
            (0..64).flat_map(move |x| {
                if x < 2 || x >= 62 || y < 2 || y >= 46 {
                    [255, 0, 255, 255]
                } else {
                    match (x < 32, y < 24) {
                        (true, true) => [255, 0, 0, 255],
                        (false, true) => [0, 255, 0, 255],
                        (true, false) => [0, 0, 255, 255],
                        _ => [255, 255, 0, 255],
                    }
                }
            })
        })
        .collect();
    gpu.queue.write_texture(
        source.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(64 * 4),
            rows_per_image: Some(48),
        },
        source.size(),
    );
    for (width, height, label) in [
        (1920, 1080, "16-9"),
        (2520, 1080, "21-9"),
        (1919, 1079, "odd"),
        (1080, 1920, "portrait"),
    ] {
        let policy = WidePresentation::new(Extent { width, height }).unwrap();
        let model_geometry = policy
            .movie_frame(
                Extent {
                    width: 64,
                    height: 48,
                },
                4.0 / 3.0,
            )
            .unwrap();
        let to_rect = |r: tsfp_gameplay::cutscene_presentation::Rect| Rect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        };
        let geometry = MovieFrame {
            source: to_rect(model_geometry.source),
            destination: to_rect(model_geometry.destination),
        };
        let target = texture(
            &gpu,
            width as u32,
            height as u32,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        renderer
            .encode_movie(&gpu.device, &mut encoder, &source, &target, geometry)
            .unwrap();
        let frame = read(&gpu, &target, encoder);
        let r = geometry.destination;
        for y in 0..height {
            for x in 0..width {
                if x < r.left || x >= r.right || y < r.top || y >= r.bottom {
                    assert_eq!(
                        frame.pixel(x as u32, y as u32),
                        [0, 0, 0, 255],
                        "{label} bar {x},{y}"
                    );
                }
            }
        }
        for (fx, fy, color) in [
            (0.25, 0.25, [255, 0, 0, 255]),
            (0.75, 0.25, [0, 255, 0, 255]),
            (0.25, 0.75, [0, 0, 255, 255]),
            (0.75, 0.75, [255, 255, 0, 255]),
        ] {
            assert_eq!(
                frame.pixel(
                    (r.left as f64 + (r.right - r.left) as f64 * fx) as u32,
                    (r.top as f64 + (r.bottom - r.top) as f64 * fy) as u32
                ),
                color
            );
        }
        assert_eq!(
            frame.pixel(r.left as u32, (r.top + (r.bottom - r.top) / 2) as u32),
            [255, 0, 255, 255]
        );
        assert_eq!(
            frame.pixel(
                (r.right - 1) as u32,
                (r.top + (r.bottom - r.top) / 2) as u32
            ),
            [255, 0, 255, 255]
        );
        save(&format!("movie-{label}"), &frame);
        println!(
            "movie {label}: {:?}, {}x{} readback",
            r, frame.width, frame.height
        );
        // Explicit crop is honoured for original/compatibility producer geometry.
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let cropped = MovieFrame {
            source: Rect {
                left: 16,
                top: 12,
                right: 32,
                bottom: 24,
            },
            destination: geometry.destination,
        };
        renderer
            .encode_movie(&gpu.device, &mut encoder, &source, &target, cropped)
            .unwrap();
        let frame = read(&gpu, &target, encoder);
        assert_eq!(
            frame.pixel(
                (r.left + (r.right - r.left) / 2) as u32,
                (r.top + (r.bottom - r.top) / 2) as u32
            ),
            [255, 0, 0, 255]
        );
        for (x, y) in [
            (r.left, r.top),
            (r.right - 1, r.top),
            (r.left, r.bottom - 1),
            (r.right - 1, r.bottom - 1),
        ] {
            assert_eq!(
                frame.pixel(x as u32, y as u32),
                [255, 0, 0, 255],
                "cropped source must not bleed neighbouring texels"
            );
        }
        // Movie fills output for this setup; scene bars load and preserve its centre.
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let full = Rect {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        renderer
            .encode_movie(
                &gpu.device,
                &mut encoder,
                &source,
                &target,
                MovieFrame {
                    destination: full,
                    ..geometry
                },
            )
            .unwrap();
        let before_bars = read(&gpu, &target, encoder);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let bars: Vec<Rect> = policy
            .bars(21.0 / 9.0)
            .unwrap()
            .into_iter()
            .map(to_rect)
            .collect();
        renderer.encode_bars(&mut encoder, &target, &bars).unwrap();
        let frame = read(&gpu, &target, encoder);
        let content = policy.content_rect(21.0 / 9.0).unwrap();
        for y in 0..height {
            for x in 0..width {
                let expected = if x >= content.left
                    && x < content.right
                    && y >= content.top
                    && y < content.bottom
                {
                    before_bars.pixel(x as u32, y as u32)
                } else {
                    [0, 0, 0, 255]
                };
                assert_eq!(
                    frame.pixel(x as u32, y as u32),
                    expected,
                    "bar LOAD contract {label} {x},{y}"
                );
            }
        }
        assert_ne!(
            frame.pixel((width / 4) as u32, (height / 2) as u32),
            [0, 0, 0, 255]
        );
        for bar in bars {
            assert_eq!(
                frame.pixel(
                    ((bar.left + bar.right) / 2) as u32,
                    ((bar.top + bar.bottom) / 2) as u32
                ),
                [0, 0, 0, 255]
            );
        }
        if content.top > 0 {
            assert_eq!(
                frame.pixel((width / 2) as u32, (content.top - 1) as u32),
                [0, 0, 0, 255]
            );
        }
        save(&format!("bars-{label}"), &frame);
        // Rejections must not record a clear or a partial draw.
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        assert_eq!(
            renderer.encode_movie(
                &gpu.device,
                &mut encoder,
                &source,
                &target,
                MovieFrame {
                    source: Rect {
                        right: 65,
                        ..geometry.source
                    },
                    ..geometry
                }
            ),
            Err(Error::Geometry)
        );
        assert_eq!(
            renderer.encode_bars(
                &mut encoder,
                &target,
                &[Rect {
                    right: width + 1,
                    ..full
                }]
            ),
            Err(Error::Geometry)
        );
        renderer.encode_bars(&mut encoder, &target, &[]).unwrap();
        let unchanged = read(&gpu, &target, encoder);
        assert_eq!(unchanged.rgba, frame.rgba);
    }
}
