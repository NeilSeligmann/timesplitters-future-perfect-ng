//! T2187 window-pixel UI overlay. Proposed backend, Tier 0 synthetic evidence only.
//! Shares the tsfp-wgpu device/queue and draws into its finished offscreen target.
//! No host wiring: the caller retains the legacy renderer unless explicitly opted in.
use std::collections::HashMap;
use tsfp_ui_renderer::*;
use wgpu::util::DeviceExt;

pub mod present;

/// Per-UI A/B selection. Production integration must retain Legacy as its default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiBackend {
    #[default]
    Legacy,
    Wgpu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Disabled,
    Lifecycle,
    Geometry,
    Target,
    MissingTexture(TextureId),
    MissingGlyph(GlyphKey),
    Upload,
    Capacity,
    /// The target lacks COPY_SRC or the readback map failed.
    Readback,
}

struct Image {
    _texture: wgpu::Texture,
    binding: wgpu::BindGroup,
}
struct Draw {
    first: u32,
    count: u32,
    texture: Option<TextureId>,
    clip: [u32; 4],
    blend: BlendMode,
}
struct Frame {
    viewport: Viewport,
    clip: [u32; 4],
    blend: BlendMode,
    vertices: Vec<u8>,
    draws: Vec<Draw>,
}

/// Atlas metadata is supplied by a runtime font loader. No shaping, rasterisation,
/// guessed metrics or fallback glyphs are performed here. Regions stay stable;
/// uploads/replacement are rejected during a frame.
#[derive(Default)]
pub struct UploadedAtlas {
    glyphs: HashMap<GlyphKey, AtlasGlyph>,
}
impl GlyphAtlas for UploadedAtlas {
    type Error = Error;
    fn prepare(&mut self, keys: &[GlyphKey]) -> Result<(), Error> {
        for key in keys {
            if !self.glyphs.contains_key(key) {
                return Err(Error::MissingGlyph(*key));
            }
        }
        Ok(())
    }
    fn glyph(&self, key: GlyphKey) -> Option<AtlasGlyph> {
        self.glyphs.get(&key).copied()
    }
}

/// All resources belong to the same device as target. The target is Rgba8Unorm,
/// one sample, RENDER_ATTACHMENT, and exactly window resolution. end_frame loads
/// its existing scene pixels, submits the overlay, and never presents/reads back.
/// Borrowing prevents resizing/replacing the Replayer target during the frame.
/// T2309: the handles are kept as (reference counted) clones so one renderer can live across
/// frames and be pointed at the engine frame's target with [`WgpuUiRenderer::retarget`], and
/// [`WgpuUiRenderer::end_frame_into`] records the UI pass into the engine's own command encoder.
pub struct WgpuUiRenderer<'a> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: wgpu::Texture,
    pipeline: wgpu::RenderPipeline,
    /// T2309: the same pipeline with the additive blend (`BlendMode::Additive`).
    additive: wgpu::RenderPipeline,
    _target_borrow: std::marker::PhantomData<&'a wgpu::Texture>,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: Image,
    images: HashMap<TextureId, Image>,
    atlas: UploadedAtlas,
    frame: Option<Frame>,
}
impl<'a> WgpuUiRenderer<'a> {
    /// Attach only after the scene/present-to-window pass. Explicit Wgpu selection
    /// is mandatory; Legacy creates no GPU resources and returns Disabled.
    pub fn from_replayer(
        replayer: &'a tsfp_wgpu::replay::Replayer,
        backend: UiBackend,
    ) -> Result<Self, Error> {
        if backend != UiBackend::Wgpu {
            return Err(Error::Disabled);
        }
        let (device, queue) = replayer.device_and_queue();
        Self::new(
            device,
            queue,
            replayer.display_texture().ok_or(Error::Target)?,
            backend,
        )
    }

    pub fn new(
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        target: &'a wgpu::Texture,
        backend: UiBackend,
    ) -> Result<Self, Error> {
        Self::create(device, queue, target, backend)
    }

    fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::Texture,
        backend: UiBackend,
    ) -> Result<Self, Error> {
        if backend != UiBackend::Wgpu {
            return Err(Error::Disabled);
        }
        check_target(target)?;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui-texture"),
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
            ],
        });
        // Nearest sampling preserves bitmap atlas texels; scalable rasterisers
        // supply glyphs at the requested physical pixel height.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let make = |label: &str, blend: wgpu::BlendState| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label), layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[wgpu::VertexBufferLayout { array_stride: 32, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4] }] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
        });
        let pipeline = make("ui", wgpu::BlendState::ALPHA_BLENDING);
        let additive = make("ui-additive", ADDITIVE_BLEND);
        let white = upload(device, queue, &layout, &sampler, 1, 1, &[255; 4]);
        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            target: target.clone(),
            pipeline,
            additive,
            _target_borrow: std::marker::PhantomData,
            layout,
            sampler,
            white,
            images: HashMap::new(),
            atlas: UploadedAtlas::default(),
            frame: None,
        })
    }

    /// Linear straight-alpha RGBA8; no automatic unpremultiplication or colour
    /// space detection. The loader must convert premultiplied inputs first.
    /// Uploads only while idle keep accepted batches
    /// and atlas handles stable until end_frame. Replacing atlas pages invalidates
    /// their glyph metadata; the loader must re-register those regions.
    /// Copy the target back as tightly packed RGBA8 rows (frame-hash checks, T2189 slice 13).
    /// Idle only; the target needs COPY_SRC. Blocks on the device.
    pub fn read_target_rgba(&self) -> Result<Vec<u8>, Error> {
        if self.frame.is_some() {
            return Err(Error::Lifecycle);
        }
        if !self.target.usage().contains(wgpu::TextureUsages::COPY_SRC) {
            return Err(Error::Readback);
        }
        let (width, height) = (self.target.width(), self.target.height());
        let pitch = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui-readback"),
            size: u64::from(pitch * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(height),
                },
            },
            self.target.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device.poll(wgpu::Maintain::Wait);
        match receiver.recv() {
            Ok(Ok(())) => {}
            _ => return Err(Error::Readback),
        }
        let mapped = buffer.slice(..).get_mapped_range();
        let rgba = mapped
            .chunks_exact(pitch as usize)
            .flat_map(|row| row[..width as usize * 4].iter().copied())
            .collect();
        drop(mapped);
        buffer.unmap();
        Ok(rgba)
    }

    pub fn target_size(&self) -> (u32, u32) {
        (self.target.width(), self.target.height())
    }

    pub fn upload_rgba(
        &mut self,
        id: TextureId,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(), Error> {
        if self.frame.is_some() {
            return Err(Error::Lifecycle);
        }
        let limits = self.device.limits();
        if width == 0
            || height == 0
            || width > limits.max_texture_dimension_2d
            || height > limits.max_texture_dimension_2d
            || (width as usize)
                .checked_mul(height as usize)
                .and_then(|n| n.checked_mul(4))
                != Some(rgba.len())
        {
            return Err(Error::Upload);
        }
        let image = upload(
            &self.device,
            &self.queue,
            &self.layout,
            &self.sampler,
            width,
            height,
            rgba,
        );
        self.images.insert(id, image);
        self.atlas.glyphs.retain(|_, glyph| glyph.texture != id);
        Ok(())
    }

    /// [`Self::upload_rgba`] sampled bilinearly (T2307: skin textures are scaled, glyph pages are not).
    pub fn upload_rgba_filtered(
        &mut self,
        id: TextureId,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(), Error> {
        let nearest = std::mem::replace(
            &mut self.sampler,
            self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("ui-linear"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        );
        let result = self.upload_rgba(id, width, height, rgba);
        self.sampler = nearest;
        result
    }

    /// Explicit loader-directed eviction; no hidden LRU or capacity policy.
    /// Idle-only, just like replacement, so accepted frame handles stay valid.
    /// Returns false for an already absent page. All its glyph keys are removed.
    pub fn evict_page(&mut self, id: TextureId) -> Result<bool, Error> {
        if self.frame.is_some() {
            return Err(Error::Lifecycle);
        }
        self.atlas.glyphs.retain(|_, glyph| glyph.texture != id);
        Ok(self.images.remove(&id).is_some())
    }

    /// Upload a coverage atlas as white RGB + alpha, without inventing font data.
    pub fn upload_coverage(
        &mut self,
        id: TextureId,
        width: u32,
        height: u32,
        coverage: &[u8],
    ) -> Result<(), Error> {
        if self.frame.is_some() {
            return Err(Error::Lifecycle);
        }
        if width == 0
            || height == 0
            || width > self.device.limits().max_texture_dimension_2d
            || height > self.device.limits().max_texture_dimension_2d
            || (width as usize).checked_mul(height as usize) != Some(coverage.len())
        {
            return Err(Error::Upload);
        }
        let rgba: Vec<u8> = coverage.iter().flat_map(|a| [255, 255, 255, *a]).collect();
        self.upload_rgba(id, width, height, &rgba)
    }

    pub fn register_glyph(&mut self, key: GlyphKey, glyph: AtlasGlyph) -> Result<(), Error> {
        self.register_glyphs(&[(key, glyph)])
    }

    /// Register a loader-supplied glyph table as one transaction. A malformed
    /// entry leaves the previous table intact, so a failed font reload cannot
    /// expose a mixture of old and new metrics. Later duplicate keys win.
    pub fn register_glyphs(&mut self, entries: &[(GlyphKey, AtlasGlyph)]) -> Result<(), Error> {
        if self.frame.is_some() {
            return Err(Error::Lifecycle);
        }
        for (key, glyph) in entries {
            if !self.images.contains_key(&glyph.texture) {
                return Err(Error::MissingTexture(glyph.texture));
            }
            if key.raster_height_px == 0
                || !finite(&[glyph.bearing_px.x, glyph.bearing_px.y, glyph.advance_px])
            {
                return Err(Error::Geometry);
            }
            validate_sprite(Sprite {
                rect_px: Rect {
                    origin: Vec2::default(),
                    size: glyph.size_px,
                },
                uv: glyph.uv,
                color: Color([1.; 4]),
                texture: glyph.texture,
            })?;
        }
        self.atlas.glyphs.extend(entries.iter().copied());
        Ok(())
    }
    /// Validate that the runtime loader has supplied every requested glyph.
    pub fn prepare_glyphs(&mut self, keys: &[GlyphKey]) -> Result<(), Error> {
        self.atlas.prepare(keys)
    }

    /// True when page `id` is uploaded.
    pub fn has_texture(&self, id: TextureId) -> bool {
        self.images.contains_key(&id)
    }

    pub fn atlas(&self) -> &UploadedAtlas {
        &self.atlas
    }

    /// Convenience for model layout: resolves exactly through the shared safe-area
    /// and edge anchoring contract, then enters the same ordered batch stream.
    pub fn anchored_quad(&mut self, rect: AnchoredRect, color: Color) -> Result<(), Error> {
        let viewport = self.frame.as_ref().ok_or(Error::Lifecycle)?.viewport;
        let quad = Quad {
            rect_px: viewport.resolve(rect).map_err(|_| Error::Geometry)?,
            color,
        };
        self.submit(&[Batch::Quads(&[quad])])
    }

    fn sprite(&mut self, sprite: Sprite, textured: bool) -> Result<(), Error> {
        validate_sprite(sprite)?;
        if textured && !self.images.contains_key(&sprite.texture) {
            return Err(Error::MissingTexture(sprite.texture));
        }
        let frame = self.frame.as_ref().ok_or(Error::Lifecycle)?;
        if frame.clip[2] == 0
            || frame.clip[3] == 0
            || sprite.rect_px.size.x == 0.
            || sprite.rect_px.size.y == 0.
        {
            return Ok(());
        }
        let r = sprite.rect_px;
        let (x0, x1) = (r.origin.x, r.origin.x + r.size.x);
        let (y0, y1) = (r.origin.y, r.origin.y + r.size.y);
        let uv = sprite.uv;
        let corners = [Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x0, y1), Vec2::new(x1, y1)];
        let uvs = [
            Vec2::new(uv.min.x, uv.min.y),
            Vec2::new(uv.max.x, uv.min.y),
            Vec2::new(uv.min.x, uv.max.y),
            Vec2::new(uv.max.x, uv.max.y),
        ];
        self.corners(corners, uvs, sprite.color, textured.then_some(sprite.texture))
    }

    /// A textured quad with free corners (T2312), strip order top-left, top-right, bottom-left, bottom-right.
    fn poly(&mut self, poly: Poly) -> Result<(), Error> {
        let flat: Vec<f32> = poly.corners_px.iter().chain(poly.uv.iter()).flat_map(|v| [v.x, v.y]).collect();
        if !finite(&flat) || !valid_color(poly.color) {
            return Err(Error::Geometry);
        }
        if !self.images.contains_key(&poly.texture) {
            return Err(Error::MissingTexture(poly.texture));
        }
        let frame = self.frame.as_ref().ok_or(Error::Lifecycle)?;
        if frame.clip[2] == 0 || frame.clip[3] == 0 {
            return Ok(());
        }
        self.corners(poly.corners_px, poly.uv, poly.color, Some(poly.texture))
    }

    /// Two triangles (0-1-2, 2-1-3) of one quad in pixels, coalesced with the previous draw when the state matches.
    fn corners(&mut self, corners: [Vec2; 4], uvs: [Vec2; 4], color: Color, texture: Option<TextureId>) -> Result<(), Error> {
        let frame = self.frame.as_mut().ok_or(Error::Lifecycle)?;
        let first = u32::try_from(frame.vertices.len() / 32).map_err(|_| Error::Capacity)?;
        first.checked_add(6).ok_or(Error::Capacity)?;
        if frame.vertices.len() as u64 + 192 > self.device.limits().max_buffer_size {
            return Err(Error::Capacity);
        }
        let size = frame.viewport.size_px();
        let ndc: Vec<[f32; 2]> = corners.iter().map(|c| [c.x / size.x * 2. - 1., 1. - c.y / size.y * 2.]).collect();
        if !finite(&ndc.iter().flatten().copied().collect::<Vec<f32>>()) {
            return Err(Error::Geometry);
        }
        for i in [0usize, 2, 1, 1, 2, 3] {
            let values = [ndc[i][0], ndc[i][1], uvs[i].x, uvs[i].y, color.0[0], color.0[1], color.0[2], color.0[3]];
            for value in values {
                frame.vertices.extend_from_slice(&value.to_le_bytes());
            }
        }
        // Coalesce only adjacent equal state: painter order is never sorted.
        if let Some(last) = frame
            .draws
            .last_mut()
            .filter(|last| {
                last.texture == texture && last.clip == frame.clip && last.blend == frame.blend
            })
        {
            last.count += 6;
        } else {
            frame.draws.push(Draw {
                first,
                count: 6,
                texture,
                clip: frame.clip,
                blend: frame.blend,
            });
        }
        Ok(())
    }
}

impl UiRenderer for WgpuUiRenderer<'_> {
    type Error = Error;
    fn begin_frame(&mut self, viewport: Viewport) -> Result<(), Error> {
        if self.frame.is_some() {
            return Err(Error::Lifecycle);
        }
        if viewport.size_px() != Vec2::new(self.target.width() as f32, self.target.height() as f32)
        {
            return Err(Error::Target);
        }
        self.frame = Some(Frame {
            viewport,
            clip: [0, 0, self.target.width(), self.target.height()],
            blend: BlendMode::Alpha,
            vertices: Vec::new(),
            draws: Vec::new(),
        });
        Ok(())
    }
    fn submit(&mut self, batches: &[Batch<'_>]) -> Result<(), Error> {
        if self.frame.is_none() {
            return Err(Error::Lifecycle);
        }
        for batch in batches {
            match batch {
                Batch::Quads(quads) => {
                    for q in *quads {
                        self.sprite(
                            Sprite {
                                rect_px: q.rect_px,
                                uv: UvRect {
                                    min: Vec2::default(),
                                    max: Vec2::new(1., 1.),
                                },
                                color: q.color,
                                texture: TextureId(0),
                            },
                            false,
                        )?;
                    }
                }
                Batch::Sprites(sprites) => {
                    for s in *sprites {
                        self.sprite(*s, true)?;
                    }
                }
                Batch::Polys(polys) => {
                    for p in *polys {
                        self.poly(*p)?;
                    }
                }
                Batch::Blend(mode) => self.frame.as_mut().unwrap().blend = *mode,
                Batch::Clip(rect) => {
                    let frame = self.frame.as_mut().unwrap();
                    frame.clip = scissor(*rect, frame.viewport)?;
                }
                Batch::Text(run) => {
                    if run.raster_height_px == 0
                        || !finite(&[run.baseline_px.x, run.baseline_px.y])
                        || !valid_color(run.color)
                    {
                        return Err(Error::Geometry);
                    }
                    for positioned in run.glyphs {
                        let key = GlyphKey {
                            font: run.font,
                            glyph: positioned.id,
                            raster_height_px: run.raster_height_px,
                        };
                        let glyph = self.atlas.glyph(key).ok_or(Error::MissingGlyph(key))?;
                        self.sprite(
                            Sprite {
                                rect_px: Rect {
                                    origin: Vec2::new(
                                        run.baseline_px.x
                                            + positioned.offset_px.x
                                            + glyph.bearing_px.x,
                                        run.baseline_px.y
                                            + positioned.offset_px.y
                                            + glyph.bearing_px.y,
                                    ),
                                    size: glyph.size_px,
                                },
                                uv: glyph.uv,
                                color: run.color,
                                texture: glyph.texture,
                            },
                            true,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
    fn end_frame(&mut self) -> Result<(), Error> {
        if self.frame.as_ref().ok_or(Error::Lifecycle)?.draws.is_empty() {
            self.frame = None;
            return Ok(());
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.end_frame_into(&mut encoder)?;
        self.queue.submit([encoder.finish()]);
        Ok(())
    }
    fn abort_frame(&mut self) {
        self.frame = None;
    }
}

impl WgpuUiRenderer<'static> {
    /// T2309: a renderer that outlives the borrow of its first target (it keeps clones of the
    /// handles), for a layer that draws into a different engine frame target every frame.
    pub fn new_owned(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::Texture,
    ) -> Result<Self, Error> {
        Self::create(device, queue, target, UiBackend::Wgpu)
    }
}

impl WgpuUiRenderer<'_> {
    /// T2309: point the idle renderer at another target of the same device (the engine frame's
    /// colour target, which changes with the window size). Uploaded images and glyphs are kept.
    pub fn retarget(&mut self, target: &wgpu::Texture) -> Result<(), Error> {
        if self.frame.is_some() {
            return Err(Error::Lifecycle);
        }
        check_target(target)?;
        self.target = target.clone();
        Ok(())
    }

    /// T2309: close the frame by recording its UI pass into `encoder` (the engine's own command
    /// encoder, after the scene and before the present copy), instead of a separate submit.
    /// The pass loads the target's pixels, so the UI is composited over the scene in painter order.
    pub fn end_frame_into(&mut self, encoder: &mut wgpu::CommandEncoder) -> Result<(), Error> {
        let frame = self.frame.take().ok_or(Error::Lifecycle)?;
        if frame.draws.is_empty() {
            return Ok(());
        }
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui-vertices"),
                contents: &frame.vertices,
                usage: wgpu::BufferUsages::VERTEX,
            });
        let view = self.target.create_view(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui-overlay"),
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
            pass.set_vertex_buffer(0, buffer.slice(..));
            let mut bound = None;
            for draw in frame.draws {
                if bound != Some(draw.blend) {
                    pass.set_pipeline(match draw.blend {
                        BlendMode::Alpha => &self.pipeline,
                        BlendMode::Additive => &self.additive,
                    });
                    bound = Some(draw.blend);
                }
                let image = match draw.texture {
                    Some(id) => &self.images[&id],
                    None => &self.white,
                };
                pass.set_bind_group(0, &image.binding, &[]);
                pass.set_scissor_rect(draw.clip[0], draw.clip[1], draw.clip[2], draw.clip[3]);
                pass.draw(draw.first..draw.first + draw.count, 0..1);
            }
            // T2324: restore the full target scissor before handing the shared encoder
            // back to the game renderer; a terminal Clip(None) has no draw to encode it.
            pass.set_scissor_rect(0, 0, self.target.width(), self.target.height());
        }
        Ok(())
    }
}

/// T2309: `BlendMode::Additive`, the original's blend mode 2: colour = src * a + dst, the
/// destination alpha is kept.
const ADDITIVE_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

fn check_target(target: &wgpu::Texture) -> Result<(), Error> {
    if target.format() != wgpu::TextureFormat::Rgba8Unorm
        || target.sample_count() != 1
        || target.dimension() != wgpu::TextureDimension::D2
        || target.depth_or_array_layers() != 1
        || !target
            .usage()
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        return Err(Error::Target);
    }
    Ok(())
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}
fn valid_color(color: Color) -> bool {
    finite(&color.0) && (0.0..=1.0).contains(&color.0[3])
}
fn validate_rect(rect: Rect) -> Result<(), Error> {
    Rect::new(rect.origin, rect.size).map_err(|_| Error::Geometry)?;
    if !finite(&[rect.origin.x + rect.size.x, rect.origin.y + rect.size.y]) {
        return Err(Error::Geometry);
    }
    Ok(())
}
fn validate_sprite(sprite: Sprite) -> Result<(), Error> {
    validate_rect(sprite.rect_px)?;
    if !valid_color(sprite.color)
        || !finite(&[
            sprite.uv.min.x,
            sprite.uv.min.y,
            sprite.uv.max.x,
            sprite.uv.max.y,
        ])
        || ![
            sprite.uv.min.x,
            sprite.uv.min.y,
            sprite.uv.max.x,
            sprite.uv.max.y,
        ]
        .iter()
        .all(|v| (0.0..=1.0).contains(v))
    {
        return Err(Error::Geometry);
    }
    Ok(())
}
/// Continuous clips conservatively include every intersected pixel, then clamp to
/// the target. Empty rectangles remain empty even at fractional coordinates.
fn scissor(rect: Option<Rect>, viewport: Viewport) -> Result<[u32; 4], Error> {
    let size = viewport.size_px();
    let Some(rect) = rect else {
        return Ok([0, 0, size.x as u32, size.y as u32]);
    };
    validate_rect(rect)?;
    let x = rect.origin.x.clamp(0., size.x).floor() as u32;
    let y = rect.origin.y.clamp(0., size.y).floor() as u32;
    let end_x = (rect.origin.x + rect.size.x).clamp(0., size.x).ceil() as u32;
    let end_y = (rect.origin.y + rect.size.y).clamp(0., size.y).ceil() as u32;
    Ok([
        x,
        y,
        if rect.size.x == 0. {
            0
        } else {
            end_x.saturating_sub(x)
        },
        if rect.size.y == 0. {
            0
        } else {
            end_y.saturating_sub(y)
        },
    ])
}
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Image {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ui-image"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    let view = texture.create_view(&Default::default());
    let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ui-image"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    Image {
        _texture: texture,
        binding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipping_and_default() {
        assert_eq!(UiBackend::default(), UiBackend::Legacy);
        let v = Viewport::new(
            Vec2::new(64., 32.),
            1.,
            Rect::new(Vec2::default(), Vec2::new(64., 32.)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            scissor(
                Some(Rect::new(Vec2::new(-2., 3.5), Vec2::new(5., 4.)).unwrap()),
                v
            ),
            Ok([0, 3, 3, 5])
        );
        assert_eq!(
            scissor(
                Some(Rect::new(Vec2::new(2.5, 3.), Vec2::new(0., 4.)).unwrap()),
                v
            ),
            Ok([2, 3, 0, 4])
        );
        assert_eq!(
            scissor(
                Some(Rect::new(Vec2::new(100., 3.), Vec2::new(1., 4.)).unwrap()),
                v
            ),
            Ok([64, 3, 0, 4])
        );
    }
}
pub mod disc;
pub mod hud;
pub mod original_font;
#[cfg(feature = "outline-fonts")]
pub mod outline_font;
pub mod overlay;
pub mod skin_assets;
pub mod standin_font;
pub mod backdrop;
pub mod in_frame;
