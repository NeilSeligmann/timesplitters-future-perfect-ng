// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in, explicitly unverified renderer for early in-game DrawList previews.
//!
//! Missing mesh and shader resources are drawn as a conspicuous neutral cube;
//! this is a visibility aid, not a resource resolver or parity implementation.
pub mod capture;
mod captured;
mod geometry;
pub mod gpu_capture;
pub mod mesh;
pub mod texture;

pub use captured::{
    CapturedDrawError, GeometryDecodeCoverage, build_draw_list, build_draw_list_with_meshes,
    build_draw_list_with_meshes_and_coverage,
};
pub use geometry::GeometryDecodeRejection;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Mutex;

use tsfp_scene_model::{
    Draw, DrawList, DrawPass, MaterialId, Matrix4, MeshRef, Rect, ValidatedDrawList,
};
use wgpu::util::DeviceExt;

const SHADER: &str = r#"
struct Uniforms { mvp: mat4x4<f32>, tint: vec4<f32>, };
@group(0) @binding(0) var<uniform> u: Uniforms;
struct In { @location(0) position: vec4<f32>, @location(1) color: vec4<f32>, @location(2) uv: vec2<f32>, };
struct Out { @builtin(position) clip: vec4<f32>, @location(0) color: vec4<f32>, @location(1) uv: vec2<f32>, };
@vertex fn vs_main(v: In) -> Out {
    var o: Out;
    o.clip = u.mvp * v.position;
    o.color = v.color;
    o.uv = v.uv;
    return o;
}
@fragment fn fs_main(v: Out) -> @location(0) vec4<f32> { return u.tint * v.color; }
"#;

// Unit cube, 12 triangles. Reused for unresolved mesh references.
const CUBE: [[f32; 3]; 36] = [
    [-1., -1., 1.],
    [1., -1., 1.],
    [1., 1., 1.],
    [-1., -1., 1.],
    [1., 1., 1.],
    [-1., 1., 1.],
    [1., -1., -1.],
    [-1., -1., -1.],
    [-1., 1., -1.],
    [1., -1., -1.],
    [-1., 1., -1.],
    [1., 1., -1.],
    [-1., -1., -1.],
    [-1., -1., 1.],
    [-1., 1., 1.],
    [-1., -1., -1.],
    [-1., 1., 1.],
    [-1., 1., -1.],
    [1., -1., 1.],
    [1., -1., -1.],
    [1., 1., -1.],
    [1., -1., 1.],
    [1., 1., -1.],
    [1., 1., 1.],
    [-1., 1., 1.],
    [1., 1., 1.],
    [1., 1., -1.],
    [-1., 1., 1.],
    [1., 1., -1.],
    [-1., 1., -1.],
    [-1., -1., -1.],
    [1., -1., -1.],
    [1., -1., 1.],
    [-1., -1., -1.],
    [1., -1., 1.],
    [-1., -1., 1.],
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreviewReport {
    pub mesh_placeholders: u32,
    pub particle_batches_omitted: u32,
    pub offscreen_passes_composited_to_view: u32,
}

pub struct NativePreviewRenderer {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    meshes: BTreeMap<MeshRef, PreviewMesh>,
    report: PreviewReport,
    skip_unresolved_meshes: bool,
}

struct PreviewMesh {
    vertices: wgpu::Buffer,
    count: u32,
    has_vertex_colors: bool,
}

fn should_render_mesh(mesh_available: bool, skip_unresolved: bool) -> bool {
    mesh_available || !skip_unresolved
}

fn preview_depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth32Float,
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::LessEqual,
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn preview_blend_state() -> Option<wgpu::BlendState> {
    // The frame capture has no pixel or blend state. This inferred policy is
    // limited to the opt-in preview and was selected by matched-frame metrics.
    Some(wgpu::BlendState::ALPHA_BLENDING)
}

/// A native scene producer's indexed position resource and optional vertex colors.
pub struct FrameMesh {
    pub id: MeshRef,
    pub positions: Vec<[f32; 3]>,
    /// Optional homogeneous clip W values for sources that already provide clip-space XYZ.
    pub clip_w: Option<Vec<f32>>,
    pub colors: Option<Vec<[f32; 4]>>,
    /// Optional primary texture coordinates, already emitted by the captured vertex shader.
    pub uvs: Option<Vec<[f32; 2]>>,
    /// First supported captured stage texture; unsupported formats are omitted.
    pub texture: Option<texture::DecodedTexture>,
    pub indices: Vec<u32>,
}

#[derive(Debug, PartialEq, Eq)]
struct PreparedMesh {
    bytes: Vec<u8>,
    vertex_count: u32,
    has_vertex_colors: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PublishError {
    Contract(tsfp_scene_model::ContractError),
    InvalidMesh(mesh::MeshError),
    DuplicateMesh,
    ResourceBudget,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CapturedPreviewError {
    Draw(CapturedDrawError),
    Publish(PublishError),
}

struct PublishedFrame {
    frame: DrawList,
    meshes: BTreeMap<MeshRef, PreparedMesh>,
    generation: u64,
}

static PUBLISHED_FRAME: Mutex<Option<PublishedFrame>> = Mutex::new(None);
const MAX_FRAME_MESH_BYTES: usize = 16 * 1024 * 1024;
const MAX_FRAME_MESHES: usize = 1024;

fn prepare_meshes(
    resources: Vec<FrameMesh>,
) -> Result<BTreeMap<MeshRef, PreparedMesh>, PublishError> {
    if resources.len() > MAX_FRAME_MESHES {
        return Err(PublishError::ResourceBudget);
    }
    let mut total = 0usize;
    let mut meshes = BTreeMap::new();
    for resource in resources {
        total = total
            .checked_add(
                resource
                    .indices
                    .len()
                    .checked_mul(40)
                    .ok_or(PublishError::ResourceBudget)?,
            )
            .ok_or(PublishError::ResourceBudget)?;
        if total > MAX_FRAME_MESH_BYTES {
            return Err(PublishError::ResourceBudget);
        }
        let mut resource = resource;
        if let (Some(texture), Some(uvs)) = (resource.texture.as_ref(), resource.uvs.as_ref()) {
            if uvs.len() == resource.positions.len() {
                let sampled: Vec<[f32; 4]> = uvs
                    .iter()
                    .enumerate()
                    .map(|(vertex, uv)| {
                        let x = (uv[0].clamp(0.0, 1.0) * (texture.width.saturating_sub(1)) as f32)
                            .round() as usize;
                        let y = (uv[1].clamp(0.0, 1.0) * (texture.height.saturating_sub(1)) as f32)
                            .round() as usize;
                        let i = (y * texture.width as usize + x) * 4;
                        let texel = &texture.rgba[i..i + 4];
                        let base = resource
                            .colors
                            .as_ref()
                            .and_then(|c| c.get(vertex))
                            .copied()
                            .unwrap_or([1.0; 4]);
                        [
                            base[0] * texel[0] as f32 / 255.0,
                            base[1] * texel[1] as f32 / 255.0,
                            base[2] * texel[2] as f32 / 255.0,
                            base[3] * texel[3] as f32 / 255.0,
                        ]
                    })
                    .collect();
                resource.colors = Some(sampled);
            }
        }
        let bytes = mesh::triangle_vertices_with_attributes(
            &resource.positions,
            resource.clip_w.as_deref(),
            resource.colors.as_deref(),
            resource.uvs.as_deref(),
            &resource.indices,
        )
        .map_err(PublishError::InvalidMesh)?;
        let prepared = PreparedMesh {
            bytes,
            vertex_count: u32::try_from(resource.indices.len())
                .map_err(|_| PublishError::ResourceBudget)?,
            has_vertex_colors: resource.colors.is_some(),
        };
        if meshes.insert(resource.id, prepared).is_some() {
            return Err(PublishError::DuplicateMesh);
        }
    }
    Ok(meshes)
}

/// Publish a frame and its native geometry atomically for the opt-in compositor.
/// Missing resources retain the visible cube fallback. The producer owns the
/// geometry and transforms; this handoff does not infer them from GPU packets.
pub fn publish_frame(frame: DrawList, resources: Vec<FrameMesh>) -> Result<(), PublishError> {
    frame.validate().map_err(PublishError::Contract)?;
    let meshes = prepare_meshes(resources)?;
    let mut published = PUBLISHED_FRAME
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let generation = published
        .as_ref()
        .map_or(0, |previous| previous.generation.wrapping_add(1));
    *published = Some(PublishedFrame {
        frame,
        meshes,
        generation,
    });
    Ok(())
}

/// Build and publish one captured frame with its supported register-1 geometry.
pub fn publish_captured_draws(
    packets: &[tsfp_engine_render::native_scene::RoomDrawPacket],
    view: &tsfp_render_prep::live::NativeSceneView,
    tick: u64,
) -> Result<(), CapturedPreviewError> {
    let (frame, meshes) =
        build_draw_list_with_meshes(packets, view, tick).map_err(CapturedPreviewError::Draw)?;
    publish_frame(frame, meshes).map_err(CapturedPreviewError::Publish)
}

/// Publish a structurally checked frame for the opt-in in-game preview layer.
pub fn publish_draw_list(frame: DrawList) -> Result<(), tsfp_scene_model::ContractError> {
    frame.validate()?;
    let mut published = PUBLISHED_FRAME
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let generation = published
        .as_ref()
        .map_or(0, |previous| previous.generation.wrapping_add(1));
    *published = Some(PublishedFrame {
        frame,
        meshes: BTreeMap::new(),
        generation,
    });
    Ok(())
}

/// Install the DrawList preview in the host's existing presentation compositor.
pub fn install() {
    tsfp_wgpu::frame_layer::install(
        "t3887-drawlist-preview",
        90,
        Box::new(NativePreviewLayer {
            renderer: None,
            generation: None,
            last_drawn_frame: None,
        }),
    );
}

struct NativePreviewLayer {
    renderer: Option<NativePreviewRenderer>,
    generation: Option<u64>,
    last_drawn_frame: Option<u64>,
}

impl tsfp_wgpu::frame_layer::FrameLayer for NativePreviewLayer {
    fn encode(&mut self, target: tsfp_wgpu::frame_layer::LayerTarget<'_>) -> bool {
        self.encode_once(target)
    }

    fn encode_before_grade(&mut self, target: tsfp_wgpu::frame_layer::LayerTarget<'_>) -> bool {
        self.encode_once(target)
    }
}

impl NativePreviewLayer {
    fn encode_once(&mut self, target: tsfp_wgpu::frame_layer::LayerTarget<'_>) -> bool {
        let Self {
            renderer,
            generation,
            last_drawn_frame,
        } = self;
        encode_once_per_frame(last_drawn_frame, target.frame, || {
            let frame = PUBLISHED_FRAME
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let Some(frame) = frame.as_ref() else {
                return false;
            };
            let Ok(validated) = frame.frame.validate() else {
                return false;
            };
            let renderer = renderer.get_or_insert_with(|| {
                NativePreviewRenderer::new(target.device, wgpu::TextureFormat::Rgba8Unorm)
            });
            // Unknown geometry should not obscure the original frame while the
            // live producer is still incomplete. Offline renderer users retain the
            // conspicuous cube fallback for diagnostics.
            renderer.skip_unresolved_meshes = true;
            if *generation != Some(frame.generation) {
                renderer.meshes.clear();
                for (&id, mesh) in &frame.meshes {
                    let vertices =
                        target
                            .device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("T3887 published preview mesh"),
                                contents: &mesh.bytes,
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                    renderer.meshes.insert(
                        id,
                        PreviewMesh {
                            vertices,
                            count: mesh.vertex_count,
                            has_vertex_colors: mesh.has_vertex_colors,
                        },
                    );
                }
                *generation = Some(frame.generation);
            }
            let view = target
                .target
                .create_view(&wgpu::TextureViewDescriptor::default());
            renderer.encode_over_existing(target.device, target.encoder, &view, &validated);
            true
        })
    }
}

fn encode_once_per_frame(last: &mut Option<u64>, frame: u64, draw: impl FnOnce() -> bool) -> bool {
    if *last == Some(frame) || !draw() {
        return false;
    }
    *last = Some(frame);
    true
}

impl NativePreviewRenderer {
    /// Create a renderer for the target format used by the caller's swapchain/view.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("T3887 unverified DrawList preview"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("T3887 preview uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(80),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("T3887 preview pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("T3887 placeholder geometry pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 40,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 32,
                            shader_location: 2,
                        },
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(preview_depth_stencil_state()),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: preview_blend_state(),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("T3887 unresolved-mesh cube"),
            contents: &mesh::triangle_vertices(
                &CUBE,
                None,
                &(0..CUBE.len() as u32).collect::<Vec<_>>(),
            )
            .expect("static cube geometry is valid"),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self {
            pipeline,
            vertices,
            meshes: BTreeMap::new(),
            report: PreviewReport::default(),
            skip_unresolved_meshes: false,
        }
    }

    /// Supply a native resource's triangle positions. Unresolved meshes keep
    /// the visibly unverified cube fallback until a producer supplies them.
    pub fn upload_mesh(
        &mut self,
        device: &wgpu::Device,
        id: MeshRef,
        positions: &[[f32; 3]],
        indices: &[u32],
    ) -> Result<(), mesh::MeshError> {
        let bytes = mesh::triangle_vertices(positions, None, indices)?;
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("T3887 preview mesh"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.meshes.insert(
            id,
            PreviewMesh {
                vertices,
                count: indices.len() as u32,
                has_vertex_colors: false,
            },
        );
        Ok(())
    }

    /// Supply a native resource with optional per-vertex colours.
    pub fn upload_frame_mesh(
        &mut self,
        device: &wgpu::Device,
        resource: FrameMesh,
    ) -> Result<(), mesh::MeshError> {
        let bytes = mesh::triangle_vertices_with_clip_w(
            &resource.positions,
            resource.clip_w.as_deref(),
            resource.colors.as_deref(),
            &resource.indices,
        )?;
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("T3887 preview frame mesh"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.meshes.insert(
            resource.id,
            PreviewMesh {
                vertices,
                count: resource.indices.len() as u32,
                has_vertex_colors: resource.colors.is_some(),
            },
        );
        Ok(())
    }

    /// Skip unresolved resources so diagnostics do not draw placeholder cubes over the source.
    pub fn set_skip_unresolved_meshes(&mut self, skip: bool) {
        self.skip_unresolved_meshes = skip;
    }

    pub fn remove_mesh(&mut self, id: MeshRef) {
        self.meshes.remove(&id);
    }

    /// Draw a structurally admitted frame to an existing color view. This path never
    /// resolves or guesses original assets; each mesh is marked as a placeholder.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        frame: &ValidatedDrawList<'_>,
    ) -> PreviewReport {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("T3887 preview frame"),
        });
        let report = self.encode(device, &mut encoder, target, frame, true);
        queue.submit(Some(encoder.finish()));
        self.report = report;
        report
    }

    /// Encode the preview on top of the existing host frame without clearing it.
    pub fn encode_over_existing(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        frame: &ValidatedDrawList<'_>,
    ) -> PreviewReport {
        let report = self.encode(device, encoder, target, frame, false);
        self.report = report;
        report
    }

    fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        frame: &ValidatedDrawList<'_>,
        clear_target: bool,
    ) -> PreviewReport {
        let list = frame.frame();
        let mut report = PreviewReport::default();
        for pass in &list.passes {
            self.render_pass(
                device,
                encoder,
                target,
                list,
                pass,
                &mut report,
                clear_target,
            );
        }
        report
    }

    pub fn last_report(&self) -> PreviewReport {
        self.report
    }

    fn render_pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        list: &DrawList,
        pass: &DrawPass,
        report: &mut PreviewReport,
        clear_target: bool,
    ) {
        let mut draws = Vec::new();
        for command in &pass.draws {
            match command {
                Draw::Mesh {
                    instance, scissor, ..
                } => {
                    if let Some(model) = list.models.iter().find(|m| m.id == *instance) {
                        let mesh_available = self.meshes.contains_key(&model.mesh);
                        if !mesh_available {
                            report.mesh_placeholders += 1;
                        }
                        if should_render_mesh(mesh_available, self.skip_unresolved_meshes) {
                            let tint = if self
                                .meshes
                                .get(&model.mesh)
                                .is_some_and(|mesh| mesh.has_vertex_colors)
                            {
                                [1.0; 4]
                            } else {
                                preview_tint(list, model.material)
                            };
                            draws.push((model, tint, *scissor));
                        }
                    }
                }
                Draw::Particles { .. } => report.particle_batches_omitted += 1,
                Draw::Clear { .. } => {}
            }
        }
        if matches!(pass.target, tsfp_scene_model::Target::Offscreen(_)) {
            report.offscreen_passes_composited_to_view += 1;
        }
        let clear = pass
            .draws
            .iter()
            .find_map(|d| {
                if let Draw::Clear {
                    colour: Some(c), ..
                } = d
                {
                    Some(*c)
                } else {
                    None
                }
            })
            .unwrap_or([0.035, 0.055, 0.09, 1.]);
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("T3887 preview depth"),
            size: wgpu::Extent3d {
                width: pass.extent[0],
                height: pass.extent[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let mut prepared = Vec::new();
        let camera = list.cameras.iter().find(|c| c.id == pass.camera);
        let bind_layout = self.pipeline.get_bind_group_layout(0);
        for (model, tint, scissor) in draws {
            let view = camera.map(|c| c.view).unwrap_or(tsfp_scene_model::IDENTITY);
            let projection = camera
                .map(|c| c.projection)
                .unwrap_or(tsfp_scene_model::IDENTITY);
            let mvp = mat_mul(projection, mat_mul(view, model.transform));
            let mut bytes = Vec::with_capacity(80);
            for col in mvp {
                for value in col {
                    bytes.extend_from_slice(&value.to_ne_bytes());
                }
            }
            for value in tint {
                bytes.extend_from_slice(&value.to_ne_bytes());
            }
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("T3887 preview draw uniform"),
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("T3887 preview draw bindings"),
                layout: &bind_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }],
            });
            prepared.push((uniform, bind, scissor, model.mesh));
        }
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("T3887 preview pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear_target {
                        wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear[0] as f64,
                            g: clear[1] as f64,
                            b: clear[2] as f64,
                            a: clear[3] as f64,
                        })
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&self.pipeline);
        let camera = list.cameras.iter().find(|c| c.id == pass.camera);
        let (viewport, depth_range) = camera.map(|c| (c.viewport, c.depth_range)).unwrap_or((
            Rect {
                x: 0,
                y: 0,
                width: pass.extent[0],
                height: pass.extent[1],
            },
            [0.0, 1.0],
        ));
        rpass.set_viewport(
            viewport.x as f32,
            viewport.y as f32,
            viewport.width as f32,
            viewport.height as f32,
            depth_range[0],
            depth_range[1],
        );
        for (_uniform, bind, scissor, mesh) in &prepared {
            rpass.set_bind_group(0, bind, &[]);
            rpass.set_scissor_rect(scissor.x, scissor.y, scissor.width, scissor.height);
            let (vertices, count) = self
                .meshes
                .get(mesh)
                .map(|mesh| (&mesh.vertices, mesh.count))
                .unwrap_or((&self.vertices, 36));
            rpass.set_vertex_buffer(0, vertices.slice(..));
            rpass.draw(0..count, 0..1);
        }
    }
}

fn preview_tint(list: &DrawList, material: MaterialId) -> [f32; 4] {
    list.materials
        .iter()
        .find(|m| m.id == material)
        .and_then(|m| m.parameters.first().copied())
        .map(|c| {
            [
                c[0].clamp(0., 1.),
                c[1].clamp(0., 1.),
                c[2].clamp(0., 1.),
                1.,
            ]
        })
        .unwrap_or([0.85, 0.25, 0.9, 1.])
}

fn mat_mul(a: Matrix4, b: Matrix4) -> Matrix4 {
    let mut out = [[0.; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            for k in 0..4 {
                out[c][r] += a[k][r] * b[c][k];
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_depth_state_matches_captured_less_equal_writes() {
        let depth = preview_depth_stencil_state();
        assert_eq!(depth.format, wgpu::TextureFormat::Depth32Float);
        assert!(depth.depth_write_enabled);
        assert_eq!(depth.depth_compare, wgpu::CompareFunction::LessEqual);
    }

    #[test]
    fn preview_blending_uses_inferred_alpha_policy() {
        assert_eq!(
            preview_blend_state(),
            Some(wgpu::BlendState::ALPHA_BLENDING)
        );
    }

    #[test]
    fn live_preview_skips_unknown_meshes_but_diagnostic_mode_keeps_fallback() {
        assert!(!should_render_mesh(false, true));
        assert!(should_render_mesh(true, true));
        assert!(should_render_mesh(false, false));
    }

    #[test]
    fn preview_falls_back_once_per_present_and_retries_unavailable_frames() {
        let mut last = None;
        let mut draws = 0;
        // No publication at the early hook: the final compositor must retry.
        assert!(!encode_once_per_frame(&mut last, 4, || false));
        assert!(encode_once_per_frame(&mut last, 4, || {
            draws += 1;
            true
        }));
        assert!(!encode_once_per_frame(&mut last, 4, || {
            draws += 1;
            true
        }));
        // An early draw suppresses the later grade and final compositor hooks.
        assert!(encode_once_per_frame(&mut last, 5, || {
            draws += 1;
            true
        }));
        assert!(!encode_once_per_frame(&mut last, 5, || {
            draws += 1;
            true
        }));
        assert_eq!(draws, 2);
    }

    #[test]
    fn mesh_handoff_rejects_bad_resources_and_enforces_total_budget() {
        let id = MeshRef(tsfp_scene_model::ContentId([1; 32]));
        let resource = || FrameMesh {
            id,
            positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            clip_w: None,
            colors: None,
            uvs: None,
            texture: None,
            indices: vec![0, 1, 2],
        };
        let meshes = prepare_meshes(vec![resource()]).unwrap();
        assert_eq!(meshes[&id].bytes.len(), 120);
        assert_eq!(
            prepare_meshes(vec![resource(), resource()]),
            Err(PublishError::DuplicateMesh)
        );
        let invalid = FrameMesh {
            indices: vec![0, 1, 3],
            ..resource()
        };
        assert_eq!(
            prepare_meshes(vec![invalid]),
            Err(PublishError::InvalidMesh(mesh::MeshError::IndexOutOfBounds))
        );
        let too_large = FrameMesh {
            indices: vec![0; MAX_FRAME_MESH_BYTES / 40 + 1],
            ..resource()
        };
        assert_eq!(
            prepare_meshes(vec![too_large]),
            Err(PublishError::ResourceBudget)
        );
    }

    #[test]
    fn matrix_multiplication_preserves_identity_and_translation() {
        let mut translation = tsfp_scene_model::IDENTITY;
        translation[3][0] = 3.;
        translation[3][1] = -2.;
        assert_eq!(
            mat_mul(tsfp_scene_model::IDENTITY, translation),
            translation
        );
    }

    #[test]
    fn missing_material_uses_loud_placeholder_tint() {
        let id = MaterialId(tsfp_scene_model::Lifetime {
            slot: 1,
            generation: 1,
        });
        let list = DrawList {
            sample: tsfp_scene_model::FrameSample {
                previous_tick: 0,
                current_tick: 0,
                sampling: tsfp_scene_model::Sampling::Original,
            },
            targets: vec![],
            cameras: vec![],
            models: vec![],
            materials: vec![],
            lights: vec![],
            effects: vec![],
            passes: vec![],
        };
        assert_eq!(preview_tint(&list, id), [0.85, 0.25, 0.9, 1.]);
    }
}
