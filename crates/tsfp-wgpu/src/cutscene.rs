// SPDX-License-Identifier: GPL-3.0-or-later
//! T2170 native renderer consumer. Playback supplies typed geometry and an RGB(A)
//! texture on this device. No decoder, guest state, synthetic input or global install.
//! Original presentation remains selected until a caller explicitly uses this pass.
/// Renderer boundary in physical pixels, independent of the gameplay model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovieFrame {
    pub source: Rect,
    pub destination: Rect,
}
use wgpu::util::DeviceExt;

const SHADER: &str = r#"
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> crop: vec4<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}
@fragment fn movie(in: Out) -> @location(0) vec4<f32> {
    let half_texel = vec2<f32>(0.5) / vec2<f32>(textureDimensions(tex));
    let uv = clamp(crop.xy + in.uv * crop.zw, crop.xy + half_texel, crop.xy + crop.zw - half_texel);
    return vec4<f32>(textureSampleLevel(tex, samp, uv, 0.0).rgb, 1.0);
}
@fragment fn black() -> @location(0) vec4<f32> { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Target,
    Source,
    Geometry,
}

/// Retained pipelines; the caller owns source texture lifetime and decoder uploads.
/// The target contract matches FrameLayer: single-sample Rgba8Unorm.
pub struct CutsceneRenderer {
    movie: wgpu::RenderPipeline,
    black: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

fn in_bounds(rect: Rect, texture: &wgpu::Texture) -> bool {
    rect.left >= 0
        && rect.top >= 0
        && rect.right > rect.left
        && rect.bottom > rect.top
        && rect.right as u32 <= texture.width()
        && rect.bottom as u32 <= texture.height()
}

fn target_valid(target: &wgpu::Texture) -> bool {
    target.format() == wgpu::TextureFormat::Rgba8Unorm
        && target.sample_count() == 1
        && target.dimension() == wgpu::TextureDimension::D2
        && target.depth_or_array_layers() == 1
        && target
            .usage()
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
}

impl CutsceneRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("native-cutscene"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("native-cutscene"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let movie_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let black_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let pipeline = |entry, pipeline_layout: &wgpu::PipelineLayout| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("native-cutscene"),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        Self {
            movie: pipeline("movie", &movie_layout),
            black: pipeline("black", &black_layout),
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    /// Replace the movie presentation target with black bars and the supplied source
    /// crop, sampled into its destination. Reject invalid rectangles before encoding.
    /// Never use this clear operation for scene overlays: use `encode_bars` there.
    pub fn encode_movie(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        target: &wgpu::Texture,
        frame: MovieFrame,
    ) -> Result<(), Error> {
        if !target_valid(target) {
            return Err(Error::Target);
        }
        if source == target
            || !source
                .usage()
                .contains(wgpu::TextureUsages::TEXTURE_BINDING)
            || source.sample_count() != 1
            || source.dimension() != wgpu::TextureDimension::D2
            || source.depth_or_array_layers() != 1
            || source.format() != wgpu::TextureFormat::Rgba8Unorm
        {
            return Err(Error::Source);
        }
        if !in_bounds(frame.source, source) || !in_bounds(frame.destination, target) {
            return Err(Error::Geometry);
        }
        let r = frame.source;
        let uv = [
            r.left as f32 / source.width() as f32,
            r.top as f32 / source.height() as f32,
            (r.right - r.left) as f32 / source.width() as f32,
            (r.bottom - r.top) as f32 / source.height() as f32,
        ];
        let bytes: Vec<u8> = uv.into_iter().flat_map(f32::to_le_bytes).collect();
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let source_view = source.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let view = target.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("native movie"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.movie);
        pass.set_bind_group(0, &group, &[]);
        let r = frame.destination;
        pass.set_viewport(
            r.left as f32,
            r.top as f32,
            (r.right - r.left) as f32,
            (r.bottom - r.top) as f32,
            0.0,
            1.0,
        );
        pass.draw(0..3, 0..1);
        Ok(())
    }

    /// Scene bars LOAD the existing scene and touch only explicit rectangles.
    /// Caller places this at the ordered view boundary, before subtitles/fades.
    pub fn encode_bars(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        bars: &[Rect],
    ) -> Result<(), Error> {
        if !target_valid(target) {
            return Err(Error::Target);
        }
        if bars.iter().any(|r| !in_bounds(*r, target)) {
            return Err(Error::Geometry);
        }
        if bars.is_empty() {
            return Ok(());
        }
        let view = target.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("native scene bars"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.black);
        for r in bars {
            pass.set_viewport(
                r.left as f32,
                r.top as f32,
                (r.right - r.left) as f32,
                (r.bottom - r.top) as f32,
                0.0,
                1.0,
            );
            pass.draw(0..3, 0..1);
        }
        Ok(())
    }
}
