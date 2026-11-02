// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline render of bounded live NV2A draw JSONL (UNVERIFIED).
//! Usage: render_live_capture <draw.jsonl> <out.png> [width height]

use tsfp_render_preview::{NativePreviewRenderer, gpu_capture};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 || args.len() > 5 {
        eprintln!("usage: render_live_capture <draw.jsonl> <out.png> [width height]");
        std::process::exit(2);
    }
    let width = args
        .get(3)
        .map_or(640, |value| value.parse().expect("valid width"));
    let height = args
        .get(4)
        .map_or(480, |value| value.parse().expect("valid height"));
    let metadata = std::fs::metadata(&args[1]).expect("draw capture metadata");
    assert!(
        metadata.len() <= 64 * 1024 * 1024,
        "draw capture exceeds 64 MiB"
    );
    let text = std::fs::read_to_string(&args[1]).expect("draw capture");
    let (frame, meshes, coverage) =
        gpu_capture::build_draw_list(&text, [width, height], 0).expect("decode draw capture");
    let valid = frame.validate().expect("validated frame");
    let [width, height] = frame.passes[0].extent;
    let instance = wgpu::Instance::default();
    let adapter = pollster_block(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .expect("adapter");
    let (device, queue) =
        pollster_block(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("T3972 captured live-draw preview"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut renderer = NativePreviewRenderer::new(&device, format);
    for mesh in meshes {
        renderer
            .upload_frame_mesh(&device, mesh)
            .expect("captured geometry");
    }
    let report = renderer.render(
        &device,
        &queue,
        &texture.create_view(&Default::default()),
        &valid,
    );
    let row_bytes = (width * 4).next_multiple_of(256);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row_bytes * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| result.expect("map"));
    device.poll(wgpu::Maintain::Wait);
    let mapped = buffer.slice(..).get_mapped_range();
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height as usize {
        let start = y * row_bytes as usize;
        rgba.extend_from_slice(&mapped[start..start + width as usize * 4]);
    }
    std::fs::write(
        &args[2],
        tsfp_render_preview::capture::encode_png(width, height, &rgba),
    )
    .expect("write preview png");
    println!("UNVERIFIED gpu_draws={coverage:?} render={report:?} extent={width}x{height}");
}

fn pollster_block<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct Noop;
    impl Wake for Noop {
        fn wake(self: std::sync::Arc<Self>) {}
    }
    let waker = Waker::from(std::sync::Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}
