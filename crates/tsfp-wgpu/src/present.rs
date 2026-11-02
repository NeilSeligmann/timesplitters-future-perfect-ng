// SPDX-License-Identifier: GPL-3.0-or-later
//! T2095: the present pass. The renderer draws at an internal size (`Replayer::set_output_size`); this pass maps that image to a
//! window sized target, keeping the content aspect (letterbox or pillarbox in black) and using a sharp bilinear filter: the source
//! is sampled nearest-like except for a one destination pixel wide blend at texel edges, so a non-integer scale neither blurs nor
//! shimmers. At equal sizes the pass is the identity (texel centres sample exactly). See docs/t-wgpu-renderer.md (T2095).

const SHADER: &str = r#"
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> u: vec4<f32>; // x,y: destination pixels per source texel

@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(tex));
    // Texel centres are integers here; edge coordinates blur integer upscales.
    let t = in.uv * size - vec2<f32>(0.5);
    let base = floor(t);
    let k = max(u.xy, vec2<f32>(1.0));
    let at = base + clamp((t - base - 0.5) * k + 0.5, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSampleLevel(tex, samp, (at + vec2<f32>(0.5)) / size, 0.0);
}
"#;

/// Largest rectangle of aspect `aspect` (width / height) inside `width` x `height`, centred: `(x, y, w, h)` in pixels.
pub fn fit_rect(width: u32, height: u32, aspect: f32) -> (u32, u32, u32, u32) {
    assert!(width > 0 && height > 0, "present size must be non zero");
    assert!(
        aspect.is_finite() && aspect > 0.0,
        "content aspect must be positive and finite"
    );
    let (mut w, mut h) = (width, ((width as f32 / aspect).round() as u32).max(1));
    if h > height {
        h = height;
        w = ((height as f32 * aspect).round() as u32).clamp(1, width);
    }
    ((width - w) / 2, (height - h) / 2, w, h)
}

/// One retained source/scale binding. Texture handles, rather than sizes alone, prevent
/// sampling a stale surface after resize or render-target replacement.
struct PresentBinding {
    source: wgpu::Texture,
    scale: [u32; 4],
    group: wgpu::BindGroup,
}

pub struct Presenter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    cached: std::cell::RefCell<Option<PresentBinding>>,
    target_view: std::cell::RefCell<Option<(wgpu::Texture, wgpu::TextureView)>>,
    pub cache_hits: std::cell::Cell<u64>,
    pub bindings_created: std::cell::Cell<u64>,
}

impl Presenter {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsfp-present"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tsfp-present"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tsfp-present"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tsfp-present"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
            cached: Default::default(),
            target_view: Default::default(),
            cache_hits: Default::default(),
            bindings_created: Default::default(),
        }
    }

    /// Record the pass: `source` (any size) into `target` (window size), black outside the aspect preserving rectangle.
    /// `aspect` is the content aspect (logical width / height). Returns the destination rectangle.
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        target: &wgpu::Texture,
        aspect: f32,
    ) -> (u32, u32, u32, u32) {
        self.encode_impl(device, encoder, source, target, aspect, false)
    }

    /// Opt-in retained binding path; identical pass and immutable uniform contents.
    pub fn encode_cached(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        target: &wgpu::Texture,
        aspect: f32,
    ) -> (u32, u32, u32, u32) {
        self.encode_impl(device, encoder, source, target, aspect, true)
    }

    fn encode_impl(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        target: &wgpu::Texture,
        aspect: f32,
        reuse: bool,
    ) -> (u32, u32, u32, u32) {
        let (src_w, src_h) = (source.width(), source.height());
        let rect = fit_rect(target.width(), target.height(), aspect);
        let scale = [
            rect.2 as f32 / src_w as f32,
            rect.3 as f32 / src_h as f32,
            0.0,
            0.0,
        ];
        let bits = scale.map(f32::to_bits);
        let cached = self.cached.borrow();
        let group = cached
            .as_ref()
            .filter(|binding| reuse && binding.source == *source && binding.scale == bits)
            .map(|binding| binding.group.clone());
        drop(cached);
        let group = if let Some(group) = group {
            self.cache_hits.set(self.cache_hits.get() + 1);
            group
        } else {
            let bytes: Vec<u8> = scale.iter().flat_map(|v| v.to_le_bytes()).collect();
            let uniform = wgpu::util::DeviceExt::create_buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some("tsfp-present"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::UNIFORM,
                },
            );
            let view = source.create_view(&Default::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tsfp-present"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
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
            self.bindings_created.set(self.bindings_created.get() + 1);
            if reuse {
                *self.cached.borrow_mut() = Some(PresentBinding {
                    source: source.clone(),
                    scale: bits,
                    group: group.clone(),
                });
            }
            group
        };
        let target_view = if reuse {
            let mut cached = self.target_view.borrow_mut();
            if cached.as_ref().is_none_or(|(texture, _)| texture != target) {
                *cached = Some((target.clone(), target.create_view(&Default::default())));
            }
            cached.as_ref().expect("target view").1.clone()
        } else {
            target.create_view(&Default::default())
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tsfp-present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.set_viewport(
            rect.0 as f32,
            rect.1 as f32,
            rect.2 as f32,
            rect.3 as f32,
            0.0,
            1.0,
        );
        pass.draw(0..3, 0..1);
        rect
    }
}

#[cfg(test)]
mod tests {
    use super::fit_rect;

    #[test]
    fn letterbox_and_pillarbox() {
        assert_eq!(fit_rect(1280, 960, 4.0 / 3.0), (0, 0, 1280, 960));
        assert_eq!(fit_rect(1920, 1080, 4.0 / 3.0), (240, 0, 1440, 1080));
        assert_eq!(fit_rect(1280, 1024, 4.0 / 3.0), (0, 32, 1280, 960));
        assert_eq!(fit_rect(1, 1, 4.0 / 3.0).2, 1);
    }
}
