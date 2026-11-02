// SPDX-License-Identifier: GPL-3.0-or-later
//! T2095: fabricated two-texel input, hand-computed filter output. No disc data.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::present::Presenter;

#[test]
fn sharp_bilinear_pixels_and_hashes() {
    let gpu = GpuContext::new(&Preference::Software).expect("T2095 requires software Vulkan");
    println!("adapter: {:?}", gpu.info);
    let texture = |width, usage| {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("T2095 synthetic"),
            size: wgpu::Extent3d {
                width,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        })
    };
    let source = texture(
        2,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    gpu.queue.write_texture(
        source.as_image_copy(),
        &[0, 0, 0, 255, 255, 255, 255, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(1),
        },
        source.size(),
    );
    let presenter = Presenter::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
    // Identity, integer upscale, fractional upscale and bilinear downscale.
    for (width, expected) in [
        (2, vec![0, 255]),
        (4, vec![0, 0, 255, 255]),
        (5, vec![0, 0, 128, 255, 255]),
        (1, vec![128]),
    ] {
        let target = texture(
            width,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        assert_eq!(
            presenter.encode(&gpu.device, &mut encoder, &source, &target, width as f32),
            (0, 0, width, 1)
        );
        let created = presenter.bindings_created.get();
        presenter.encode_cached(&gpu.device, &mut encoder, &source, &target, width as f32);
        assert_eq!(
            presenter.bindings_created.get(),
            created + 1,
            "scale change invalidates binding"
        );
        let hits = presenter.cache_hits.get();
        presenter.encode_cached(&gpu.device, &mut encoder, &source, &target, width as f32);
        assert_eq!(presenter.cache_hits.get(), hits + 1);
        // Same dimensions, different handle and reversed content: an old-size-only cache
        // would return the wrong pixels. Source clones above, in contrast, reuse the binding.
        let replacement = texture(
            2,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        gpu.queue.write_texture(
            replacement.as_image_copy(),
            &[255, 255, 255, 255, 0, 0, 0, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(1),
            },
            replacement.size(),
        );
        let created = presenter.bindings_created.get();
        presenter.encode_cached(
            &gpu.device,
            &mut encoder,
            &replacement,
            &target,
            width as f32,
        );
        assert_eq!(presenter.bindings_created.get(), created + 1);
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            target.size(),
        );
        gpu.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
        gpu.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let mapped = slice.get_mapped_range();
        let actual = &mapped[..width as usize * 4];
        let expected: Vec<u8> = expected
            .into_iter()
            .rev()
            .flat_map(|v| [v, v, v, 255])
            .collect();
        assert_eq!(actual, expected, "{width} pixels");
        let hash = |bytes: &[u8]| {
            bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            })
        };
        assert_eq!(hash(actual), hash(&expected));
        println!("width {width}: FNV1a {:016x}", hash(actual));
        drop(mapped);
        buffer.unmap();
    }
}
