//! T2313 UI-11b: the original page fill's back-buffer pass, drawn natively inside the engine frame.
//!
//! The original fill `0x2D7F90` draws its rounded box, then `0x204F0(x, y, w, h, x*16, y*16, w*16,
//! h*16, 1, 3, 0x80808020, 0)`: with texture 0x12 bound (`0x2A2D0`, a copy of the back buffer) it
//! draws that texture over the box four times, offset by `(1 + 3) / 2 = 2` units, with vertex colour
//! 0x80808020 (alpha doubled to 0x40 by the RRGGBBAA to ARGB conversion at `0x2050B`), then draws the
//! fill again. The scene behind the box ends up blurred and darkened (MEASURED from the lift; the
//! texture's resolution and the stencil state of the taps are not decoded).
//!
//! The Rust pass reproduces the measured result rather than the tap mechanics: inside the page's
//! rounded box the frame is replaced by a Gaussian blur of itself (sigma 2 design units) minus a small
//! darkening offset. Fitted on the T2309 legacy main menu dump (1280x960, RADV, text and focus row
//! masked): legacy = 0.489 * gauss(sigma 4 px)(scene) - 4.0 levels, mean residual 1.25 levels; the two
//! fills of the skin transmit 0.49, so the pass itself is `gauss - 8 levels` (INFERRED parameters,
//! docs/t2313-ui-backdrop.md). Two passes: horizontal blur of a copy of the frame into a scratch
//! texture, then vertical blur back into the frame, masked by the rounded box (1 px soft edge) and
//! weighted by `strength` (the page fade).
use tsfp_ui_renderer::Rect;

/// What to blur and how (pixels of the target).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropParams {
    /// The page box (the fill's rect) and its corner radius.
    pub rect: Rect,
    pub radius: f32,
    /// Gaussian sigma in pixels.
    pub sigma: f32,
    /// Subtracted from the blurred colour (0..1 scale).
    pub darken: f32,
    /// 0 = untouched frame, 1 = full pass (the page fade).
    pub strength: f32,
}

const SHADER: &str = r#"
struct Params {
    rect: vec4<f32>,      // x0, y0, x1, y1 (px)
    shape: vec4<f32>,     // radius, sigma, darken, strength
    axis: vec4<f32>,      // dx, dy, masked (0/1), taps
    size: vec4<f32>,      // target width, height
};
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn coverage(p: vec2<f32>) -> f32 {
    let r = params.shape.x;
    let centre = (params.rect.xy + params.rect.zw) * 0.5;
    let half = (params.rect.zw - params.rect.xy) * 0.5;
    let q = abs(p - centre) - half + vec2<f32>(r, r);
    let distance = length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
    return clamp(0.5 - distance, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let sigma = max(params.shape.y, 0.001);
    let taps = i32(params.axis.w);
    var sum = vec3<f32>(0.0, 0.0, 0.0);
    var weight = 0.0;
    for (var i = -taps; i <= taps; i = i + 1) {
        let offset = f32(i);
        let w = exp(-0.5 * offset * offset / (sigma * sigma));
        let at = (position.xy + params.axis.xy * offset) / params.size.xy;
        sum = sum + w * textureSampleLevel(source, linear_sampler, at, 0.0).rgb;
        weight = weight + w;
    }
    var colour = sum / weight;
    var alpha = 1.0;
    if (params.axis.z > 0.5) {
        colour = max(colour - vec3<f32>(params.shape.z), vec3<f32>(0.0));
        alpha = coverage(position.xy) * params.shape.w;
    }
    return vec4<f32>(colour, alpha);
}
"#;

const UNIFORM_SIZE: u64 = 64;

struct Scratch {
    size: (u32, u32),
    copy: wgpu::Texture,
    horizontal: wgpu::Texture,
    copy_group: wgpu::BindGroup,
    horizontal_group: wgpu::BindGroup,
}

/// The pass's GPU objects (pipelines once, scratch textures per frame size).
pub struct BackdropPass {
    layout: wgpu::BindGroupLayout,
    plain: wgpu::RenderPipeline,
    masked: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    horizontal_params: wgpu::Buffer,
    vertical_params: wgpu::Buffer,
    scratch: Option<Scratch>,
    format: wgpu::TextureFormat,
}

impl BackdropPass {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui-backdrop"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui-backdrop"),
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
                        min_binding_size: wgpu::BufferSize::new(UNIFORM_SIZE),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui-backdrop"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = |blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ui-backdrop"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let plain = pipeline(None);
        // Colour mixes by coverage times strength; the frame's alpha (guest render state) is kept.
        let masked = pipeline(Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        }));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui-backdrop"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: UNIFORM_SIZE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            layout,
            plain,
            masked,
            sampler,
            horizontal_params: uniform("ui-backdrop-h"),
            vertical_params: uniform("ui-backdrop-v"),
            scratch: None,
            format,
        }
    }

    fn ensure_scratch(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if self.scratch.as_ref().is_none_or(|s| s.size != size) {
            let texture = |label, usage| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage: usage | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
            };
            let copy = texture("ui-backdrop-copy", wgpu::TextureUsages::COPY_DST);
            let horizontal = texture(
                "ui-backdrop-horizontal",
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            let group = |texture: &wgpu::Texture, params: &wgpu::Buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("ui-backdrop"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &texture.create_view(&Default::default()),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: params,
                                offset: 0,
                                size: wgpu::BufferSize::new(UNIFORM_SIZE),
                            }),
                        },
                    ],
                })
            };
            let copy_group = group(&copy, &self.horizontal_params);
            let horizontal_group = group(&horizontal, &self.vertical_params);
            self.scratch = Some(Scratch {
                size,
                copy,
                horizontal,
                copy_group,
                horizontal_group,
            });
        }
    }

    /// Record the pass into `encoder` over `target` (the engine frame). Returns false when the box
    /// is empty or off the target (nothing recorded).
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        params: BackdropParams,
    ) -> bool {
        let size = (target.width(), target.height());
        if params.strength <= 0.0 || target.format() != self.format {
            return false;
        }
        let taps = (params.sigma * 3.0).ceil().clamp(1.0, 32.0);
        let x0 = params.rect.origin.x;
        let y0 = params.rect.origin.y;
        let x1 = x0 + params.rect.size.x;
        let y1 = y0 + params.rect.size.y;
        let Some(inner) = scissor((x0, y0, x1, y1), 0.0, size) else {
            return false;
        };
        // The horizontal pass must cover the rows the vertical taps read.
        let Some(outer) = scissor((x0, y0, x1, y1), taps + 1.0, size) else {
            return false;
        };
        let uniform = |axis: (f32, f32), masked: f32| -> Vec<u8> {
            [
                x0,
                y0,
                x1,
                y1,
                params.radius,
                params.sigma,
                params.darken,
                params.strength.min(1.0),
                axis.0,
                axis.1,
                masked,
                taps,
                size.0 as f32,
                size.1 as f32,
                0.0,
                0.0,
            ]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect()
        };
        queue.write_buffer(&self.horizontal_params, 0, &uniform((1.0, 0.0), 0.0));
        queue.write_buffer(&self.vertical_params, 0, &uniform((0.0, 1.0), 1.0));
        self.ensure_scratch(device, size);
        let scratch = self.scratch.as_ref().expect("ensured");
        encoder.copy_texture_to_texture(
            target.as_image_copy(),
            scratch.copy.as_image_copy(),
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        let pass = |encoder: &mut wgpu::CommandEncoder,
                    view: &wgpu::TextureView,
                    pipeline: &wgpu::RenderPipeline,
                    group: &wgpu::BindGroup,
                    area: (u32, u32, u32, u32)| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui-backdrop"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.set_scissor_rect(area.0, area.1, area.2, area.3);
            pass.draw(0..3, 0..1);
        };
        pass(
            encoder,
            &scratch.horizontal.create_view(&Default::default()),
            &self.plain,
            &scratch.copy_group,
            outer,
        );
        pass(
            encoder,
            &target.create_view(&Default::default()),
            &self.masked,
            &scratch.horizontal_group,
            inner,
        );
        true
    }
}

/// The pixel rect `(x0, y0, x1, y1)` grown by `grow`, clamped to the target, as a scissor
/// `(x, y, width, height)`; None when nothing is left.
fn scissor(rect: (f32, f32, f32, f32), grow: f32, size: (u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let clamp = |v: f32, max: u32| v.max(0.0).min(max as f32);
    let x0 = clamp((rect.0 - grow).floor(), size.0) as u32;
    let y0 = clamp((rect.1 - grow).floor(), size.1) as u32;
    let x1 = clamp((rect.2 + grow).ceil(), size.0) as u32;
    let y1 = clamp((rect.3 + grow).ceil(), size.1) as u32;
    (x1 > x0 && y1 > y0).then(|| (x0, y0, x1 - x0, y1 - y0))
}

#[cfg(test)]
mod tests {
    use super::scissor;

    #[test]
    fn scissor_grows_and_clamps() {
        assert_eq!(scissor((10.0, 20.0, 30.0, 40.0), 0.0, (100, 100)), Some((10, 20, 20, 20)));
        assert_eq!(scissor((10.5, 20.0, 30.2, 40.0), 0.0, (100, 100)), Some((10, 20, 21, 20)));
        assert_eq!(scissor((2.0, 2.0, 98.0, 99.0), 5.0, (100, 100)), Some((0, 0, 100, 100)));
        assert_eq!(scissor((120.0, 0.0, 130.0, 10.0), 0.0, (100, 100)), None);
    }
}
