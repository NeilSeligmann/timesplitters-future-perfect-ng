use std::time::{Duration, Instant};
use tsfp_wgpu::{
    context::{GpuContext, Preference},
    readback::ReadbackRing,
};

fn submit(
    gpu: &GpuContext,
    ring: &mut ReadbackRing,
    number: u64,
    size: (u32, u32),
    red: f64,
) -> bool {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: red,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }
    let index = ring.encode(&gpu.device, &mut encoder, &texture, size);
    gpu.queue.submit([encoder.finish()]);
    if let Some(index) = index {
        ring.map(index, number);
        true
    } else {
        false
    }
}
fn drain(
    gpu: &GpuContext,
    ring: &mut ReadbackRing,
    now: u64,
) -> Vec<(u64, tsfp_wgpu::replay::Frame)> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut frames = Vec::new();
    while ring.stats.occupancy != 0 {
        frames.extend(ring.poll(&gpu.device, now));
        assert!(Instant::now() < deadline, "async callback did not complete");
        std::thread::yield_now();
    }
    frames
}
#[test]
fn ordering_resize_reuse_and_no_duplicates() {
    let gpu = GpuContext::new(&Preference::Software).expect("lavapipe adapter");
    let mut ring = ReadbackRing::default();
    for round in 0..5 {
        let n = round * 2 + 1;
        assert!(submit(&gpu, &mut ring, n, (65, 3), 1.0));
        assert!(submit(&gpu, &mut ring, n + 1, (17, 7), 0.5));
        let frames = drain(&gpu, &mut ring, n + 2);
        assert_eq!(frames.iter().map(|f| f.0).collect::<Vec<_>>(), [n, n + 1]);
        assert_eq!((frames[0].1.width, frames[0].1.height), (65, 3));
        assert_eq!((frames[1].1.width, frames[1].1.height), (17, 7));
        assert_eq!(frames[0].1.pixel(64, 2), [255, 0, 0, 255]);
        assert_eq!(frames[1].1.pixel(16, 6), [128, 0, 0, 255]);
        assert!(ring.poll(&gpu.device, n + 2).is_empty());
    }
    assert_eq!(ring.stats.submitted, 10);
    assert_eq!(ring.stats.completed, 10);
    assert_eq!(ring.stats.late, 0);
}
#[test]
fn bounded_overload_and_late_frames() {
    let gpu = GpuContext::new(&Preference::Software).expect("lavapipe adapter");
    let mut ring = ReadbackRing::default();
    for n in 1..=3 {
        assert!(submit(&gpu, &mut ring, n, (3, 2), 1.0));
    }
    assert!(!submit(&gpu, &mut ring, 4, (3, 2), 1.0));
    assert_eq!(ring.stats.peak, 3);
    assert_eq!(ring.stats.saturated, 1);
    assert!(drain(&gpu, &mut ring, 10).is_empty());
    assert_eq!(ring.stats.late, 3);
    assert!(submit(&gpu, &mut ring, 10, (7, 1), 1.0));
    assert_eq!(drain(&gpu, &mut ring, 11).len(), 1);
}

#[test]
fn live_idle_delivery_keeps_source_present_identity() {
    use tsfp_wgpu::{
        live::{LiveConfig, LiveRenderer},
        stream::Stream,
    };
    let mut config = LiveConfig::software();
    config.output = Some((65, 3));
    config.knobs.push(("pipelined_readback".into(), "1".into()));
    let mut live = LiveRenderer::new(config).expect("lavapipe adapter");
    let stream = Stream::from_text(include_str!("data/title.stream")).unwrap();
    live.name_next_present(100);
    for event in &stream.events {
        live.feed(event).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while live.last_frame().is_none() {
        live.poll_readbacks().unwrap();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(live.presents(), 100);
    assert_eq!(live.last_present(), 100);
    let frame = live.take_last_frame().unwrap();
    assert_eq!((frame.width, frame.height), (65, 3));
    assert!(!frame.rgba.is_empty());
    live.poll_readbacks().unwrap();
    assert!(live.take_last_frame().is_none());
}
