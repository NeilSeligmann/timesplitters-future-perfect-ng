// SPDX-License-Identifier: GPL-3.0-or-later
//! Private synthetic UI render-target fixtures; no original-game evidence.
use tsfp_ui_renderer::{Rect, Vec2};
use tsfp_wgpu::context::GpuContext;

pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(Vec2::new(x, y), Vec2::new(w, h)).unwrap()
}

pub fn target(gpu: &GpuContext, label: &str, size: (u32, u32), clear: [u8; 4]) -> wgpu::Texture {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    reset(gpu, &texture, clear);
    texture
}

pub fn reset(gpu: &GpuContext, texture: &wgpu::Texture, clear: [u8; 4]) {
    gpu.queue.write_texture(
        texture.as_image_copy(),
        &clear.repeat((texture.width() * texture.height()) as usize),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(texture.width() * 4),
            rows_per_image: Some(texture.height()),
        },
        texture.size(),
    );
}
