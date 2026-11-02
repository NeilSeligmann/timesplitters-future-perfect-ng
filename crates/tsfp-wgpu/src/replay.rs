// SPDX-License-Identifier: GPL-3.0-or-later
//! The wgpu replayer: executes a recorded [`Stream`] (the T1898 `RenderDevice` boundary) on a
//! headless wgpu device and returns the frames at each `present`.
//!
//! Draws are collected for the frame (vertices decoded from the replay image at call time, so
//! ring reuse is honoured), then encoded into passes at `present`. A `CLEAR_SURFACE` method
//! starts a new pass. State is NV2A methods ([`crate::nv2a`]); what the replayer does not
//! execute is counted in the [`Report`].
use crate::clear_policy;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tsfp_engine_render::device::DeviceCall;
use tsfp_engine_render::mem::{GuestAccess, SparseMemory};
use tsfp_engine_render::memory::GuestMemory;
use wgpu::util::DeviceExt;

use crate::combiner::{MAX_STAGES, PixelProgram, argb};
use crate::context::GpuContext;
use crate::nv2a::{ClearRequest, Compare, CullFace, Equation, Factor, PipelineState, Report};
use crate::pipeline_key::PipelineKey;
use crate::scissor::scissor_rect;
use crate::stream::{Event, Stream, is_bind_vertex_stream};
use crate::texture;
use crate::vertex::{Vertex, decode_vertex, line_list, stride_supported, triangulate};
use crate::vsh::{self, CONSTANT_COUNT, INPUT_COUNT, ProgramMemory, Vec4};

/// One entry of the vertex declaration block: byte offset in the vertex and the `size << 4 | type` format byte.
#[derive(Clone, Copy, Default)]
struct DeclEntry {
    offset: u32,
    format: u32,
}

/// Decode one vertex attribute (NV2A vertex array types: 0 D3DCOLOR bytes, 1 normalised short, 2 float, 4 OGL bytes,
/// 5 raw short, 6 packed 11/11/10) into a four component register with the hardware default (0, 0, 0, 1) filled in.
fn fetch_attribute(memory: &SparseMemory, address: u32, format: u32) -> Vec4 {
    let size = (format >> 4) & 0xF;
    let kind = format & 0xF;
    let mut out = [0.0, 0.0, 0.0, 1.0];
    if size == 0 {
        return out;
    }
    let float = |index: u32| memory.read_f32(address.wrapping_add(index * 4));
    match kind {
        2 => {
            for index in 0..size.min(4) {
                out[index as usize] = float(index);
            }
        }
        0 => {
            // D3DCOLOR: bytes B, G, R, A in memory, register order R, G, B, A.
            let byte = |index: u32| f32::from(memory.read8(address.wrapping_add(index))) / 255.0;
            out = [byte(2), byte(1), byte(0), byte(3)];
        }
        4 => {
            for index in 0..size.min(4) {
                out[index as usize] = f32::from(memory.read8(address.wrapping_add(index))) / 255.0;
            }
        }
        1 | 5 => {
            for index in 0..size.min(4) {
                let value = memory.read16(address.wrapping_add(index * 2)) as i16;
                out[index as usize] = if kind == 1 {
                    f32::from(value) / 32767.0
                } else {
                    f32::from(value)
                };
            }
        }
        6 => {
            let word = memory.read32(address);
            let signed = |bits: u32, width: u32| {
                let shift = 32 - width;
                (((bits << shift) as i32) >> shift) as f32
            };
            out = [
                (signed(word & 0x7FF, 11) / 1023.0).max(-1.0),
                (signed((word >> 11) & 0x7FF, 11) / 1023.0).max(-1.0),
                (signed((word >> 22) & 0x3FF, 10) / 511.0).max(-1.0),
                1.0,
            ];
        }
        _ => {}
    }
    out
}

#[path = "gpu_vsh.rs"]
pub(crate) mod gpu_vsh;
pub use gpu_vsh::GpuVshStats;

/// Whether vertex programs run on the GPU when nothing says otherwise (`--opt gpu_vsh`, `TSFP_WGPU_GPU_VSH`, [`Replayer::set_gpu_vsh`]).
pub const GPU_VSH_DEFAULT: bool = false;

const SHADER: &str = include_str!("shader.wgsl");
/// Full screen triangle with one colour, used for clears that write only some channels.
const CLEAR_SHADER: &str = "
struct Params { colour: vec4<f32> };
@group(0) @binding(0) var<uniform> params: Params;
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs_main() -> @location(0) vec4<f32> { return params.colour; }
";

const COLOUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A resolved frame, straight RGBA8, row 0 at the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Frame {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }
    pub fn hash(&self) -> u64 {
        crate::png::image_hash(self.width, self.height, &self.rgba)
    }
    pub fn to_png(&self) -> Vec<u8> {
        crate::png::encode_rgba(self.width, self.height, &self.rgba)
    }
}

struct TextureImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    /// Linear formats take unnormalised texel coordinates.
    linear: bool,
    /// Mip levels the texture record declares (format word bits 19..16, at least 1). Only level 0 is decoded, the others are
    /// generated from it (slice 8), so a texture with one level is sampled flat.
    mips: u32,
}

/// Frame-local immutable image copies. One padded staging buffer feeds all mip copies.
#[derive(Default)]
struct TextureUploads {
    bytes: Vec<u8>,
    copies: Vec<(wgpu::Texture, u32, u64, u32, wgpu::Extent3d)>,
}

impl TextureUploads {
    fn append(
        &mut self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        levels: u32,
        chain: &[u8],
    ) {
        let mut source = 0;
        for level in 0..levels {
            let w = (width >> level).max(1);
            let h = (height >> level).max(1);
            let pitch = (w * 4).div_ceil(256) * 256;
            let offset = self.bytes.len() as u64;
            for row in 0..h as usize {
                let start = source + row * w as usize * 4;
                self.bytes
                    .extend_from_slice(&chain[start..start + w as usize * 4]);
                self.bytes
                    .resize(self.bytes.len() + (pitch - w * 4) as usize, 0);
            }
            source += w as usize * h as usize * 4;
            self.copies.push((
                texture.clone(),
                level,
                offset,
                pitch,
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            ));
        }
    }

    fn encode(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        if self.bytes.is_empty() {
            return;
        }
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tsfp-texture-batch"),
            contents: &self.bytes,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        for (texture, mip_level, offset, pitch, size) in &self.copies {
            encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: *offset,
                        bytes_per_row: Some(*pitch),
                        rows_per_image: None,
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: *mip_level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                *size,
            );
        }
    }
}

/// How often the render target is copied for the draws of one frame that sample a frame buffer surface (the stream does not say
/// when the engine copies the frame into the texture, INFERRED, measured on two paired frames in docs/t-wgpu-renderer.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FbSnapshot {
    /// Before every such draw (each draw sees the results of the earlier ones).
    PerDraw,
    /// Once per frame, at the first such draw.
    Once,
    /// Once per frame and texture record: the first draw that samples a record copies, later draws of that record reuse the copy.
    PerRecord,
}

struct DrawRecord {
    state: PipelineState,
    texture: u32,
    nearest: bool,
    /// Textures and point sampling flags of stages 1 to 3 (slice 4).
    extra_textures: [u32; 3],
    extra_nearest: [bool; 3],
    /// Stages (bit i) that sample a frame buffer surface: the target is copied to a texture just before the draw (slice 4).
    fb_mask: u32,
    scissor: Option<[i32; 4]>,
    /// Indices are line segment pairs, not triangles (slice 8).
    lines: bool,
    first_index: u32,
    index_count: u32,
    /// Vertices are clip space (vertex program path).
    clip: bool,
    /// ARGB colour when the draw is a full screen opaque overwrite quad (see [`clear_like_colour`]).
    clear_colour: Option<u32>,
    /// Stage 0 combiner decoded from the bound pixel shader definition.
    pixel: Option<PixelProgram>,
    /// Data word of the surface the draw renders into (`Surface` events, 0 before the first one).
    target: u32,
    /// Per stage: the Data word of the render target surface the stage's texture record aliases (slice 5 render to texture).
    surface_stage: [Option<u32>; 4],
    /// The closing grade pass of the frame: stage 0 samples the scene target in pixel coordinates (T2140).
    grade: bool,
    /// Per stage texture address words (`lib` method 0x1B08 + 0x40 * stage), None when the stream has none.
    address: [Option<u32>; 4],
    /// The combiner's fog register colour (`lib` 0x2A8), zero when never pushed (slice 6).
    fog_colour: [f32; 4],
}

impl DrawRecord {
    /// Texture record IDs in hardware stage order, including unbound stages.
    fn stage_ids(&self) -> [u32; 4] {
        [
            self.texture,
            self.extra_textures[0],
            self.extra_textures[1],
            self.extra_textures[2],
        ]
    }
}

/// Fog parameters of the pinned xemu post vertex fog (`tools/nv2a/fog.py`), modes 0 to 5: linear, exp, exp2 and their absolute forms.
#[derive(Clone, Copy)]
struct FogState {
    mode: u32,
    parameter0: f32,
    parameter1: f32,
}

impl FogState {
    /// The fog amount of a vertex whose program wrote `distance` to `oFog`: the xemu factor clamped to 0 to 1. INFERRED: that the
    /// combiner's fog alpha is this factor and not its complement (the shader's final combiner form is `fog.a * fog.rgb +
    /// (1 - fog.a) * spare 0`, and the factor of the mode 0 parameters (1, 1) is the distance itself).
    fn amount(&self, distance: f32) -> f32 {
        let mut value = match self.mode {
            0 | 3 => (self.parameter0 + distance * self.parameter1) - 1.0,
            1 | 4 => (self.parameter0 + (distance * self.parameter1 * 16.0).exp2()) - 1.5,
            _ => {
                (self.parameter0
                    + (-distance * distance * self.parameter1 * self.parameter1 * 32.0).exp2())
                    - 1.5
            }
        };
        if self.mode >= 3 {
            value = value.abs();
        }
        if value.is_nan() {
            value = if self.mode <= 1 || self.mode == 3 {
                1.0
            } else {
                0.0
            };
        }
        value.clamp(0.0, 1.0)
    }
}

enum Op {
    /// Native view-pass replacement at its original command position.
    ViewLayer(u32),
    Clear(ClearRequest),
    Draw(Box<DrawRecord>),
    /// `SetRenderTarget`: later ops draw into the surface with this Data word.
    Target(u32),
}

/// A render target surface kept across frames (render to texture).
struct SurfaceGpu {
    /// Unique per created surface, part of the bind group cache keys.
    id: u64,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

/// Bytes of one draw's parameter block (alignment 256 for dynamic uniform offsets).
const PARAM_STRIDE: usize = 512;

/// What a stage binds, for the bind group cache: a decoded texture (id, generation), a render target surface (its unique id) or the
/// 1x1 stand-in view (black?).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum StageBinding {
    Texture(u32, u64),
    Surface(u64),
    Null(bool),
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct BindKey {
    params: u64,
    stages: [(StageBinding, (bool, u8, u8)); 4],
}

/// Key of the single per frame target of streams without `Surface` events (the pre slice 5 behaviour).
const LEGACY_SURFACE: u32 = u32::MAX;

/// Where a frame's time goes, summed over the presented frames (nanoseconds). Measured around the phases of `Replayer::present`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhaseTimings {
    pub frames: u64,
    /// Decode and upload of texture images that changed (the cached views are free).
    pub textures_ns: u64,
    pub texture_uploads: u64,
    /// Coalesced texture staging buffers and padded bytes (opt-in cold path).
    pub texture_upload_batches: u64,
    pub texture_staging_bytes: u64,
    pub texture_cache_hits: u64,
    /// Largest per-frame texture phase (decode/upload spike candidate; CPU wall time).
    pub textures_frame_max_ns: u64,
    /// Uniform buffers, samplers and bind groups (one of each per draw).
    pub bind_groups_ns: u64,
    pub draws: u64,
    /// Command encoding of all passes.
    pub encode_ns: u64,
    /// Submit, plus the CPU readback and wait when the frame is read back.
    pub submit_ns: u64,
    pub total_ns: u64,
    /// Pipelines built (on the render thread, or by the async workers), their summed and longest build time, and the draws
    /// skipped because the pipeline was not ready (async only).
    pub pipelines_built: u64,
    pub pipeline_ns: u64,
    pub pipeline_ns_max: u64,
    pub draws_deferred: u64,
    /// T2024k: the vertex program interpreter alone, the attribute fetch alone (both inside `shade_ns`), and the vertices run through
    /// the interpreter.
    pub interp_ns: u64,
    pub fetch_ns: u64,
    pub interp_vertices: u64,
    /// Time in `gpu_shade` (job recording for the GPU vertex path: dedup, attribute fetch, constants snapshot).
    pub gpu_prep_ns: u64,
    /// The slowest frame so far: its total and how it splits (textures, bind groups and pipelines, encode, submit), nanoseconds.
    pub worst_frame_ns: u64,
    pub worst_frame_phases: [u64; 4],
    /// Render pipeline creation (T2136f): time and number of pipelines built (a pipeline key seen for the first time).
    pub pipelines_ns: u64,
    pub pipelines_created: u64,
    /// Part of `submit_ns` spent in the GPU readback (copy, map, wait and row unpadding) of a read back frame.
    pub readback_ns: u64,
    /// Blocking GPU completion poll on the CPU readback route, separate from in-flight throttling.
    pub readback_wait_ns: u64,
    pub readback_waits: u64,
    /// Callback delivery after poll and CPU row unpadding (not GPU execution timings).
    pub map_callback_ns: u64,
    pub row_copy_ns: u64,
    pub present_bind_cache_hits: u64,
    pub present_bindings_created: u64,
    pub present_encode_ns: u64,
    /// Texture decodes (a SetTexture whose image had to be decoded again), their time and texel bytes written / really changed.
    pub decodes: u64,
    pub decode_ns: u64,
    pub texel_bytes: u64,
    pub texel_bytes_changed: u64,
    /// Why a decode ran: the record was never decoded / its header words changed / its texels changed.
    pub decode_why: [u64; 3],
    /// Decodes whose (format, size, texel hash) was decoded before under another record or earlier.
    pub decode_repeat: u64,
    /// Time in `Maintain::Wait` for the frames in flight (the GPU is the bottleneck) and the submit to completion latency of the
    /// frames (summed, and the number of frames measured), T2136f.
    pub wait_ns: u64,
    pub waits: u64,
    /// Part of `submit_ns`: `CommandEncoder::finish` and `Queue::submit` (the rest is the poll).
    /// Time in `draw` (building a draw record: vertex fetch, vertex program, state) and in the vertex program alone, and the
    /// vertices shaded (T2136f).
    pub draw_ns: u64,
    pub shade_ns: u64,
    pub shaded_vertices: u64,
    pub bind_cache_hits: u64,
    pub bind_groups_created: u64,
    pub finish_ns: u64,
    pub queue_submit_ns: u64,
    /// Submit-to-callback observation includes scheduling and poll cadence, not GPU time.
    pub gpu_latency_ns: u64,
    pub gpu_latency_max_ns: u64,
    pub gpu_latency_frames: u64,
}

impl PhaseTimings {
    fn phases(&self) -> [u64; 4] {
        [
            self.textures_ns,
            self.bind_groups_ns,
            self.encode_ns,
            self.submit_ns,
        ]
    }

    fn end_frame(&mut self, total: u64, base: [u64; 4]) {
        self.frames += 1;
        self.total_ns += total;
        if total > self.worst_frame_ns {
            let now = self.phases();
            self.worst_frame_ns = total;
            self.worst_frame_phases = std::array::from_fn(|i| now[i].saturating_sub(base[i]));
        }
    }

    fn note_pipeline(&mut self, nanoseconds: u64) {
        self.pipelines_built += 1;
        self.pipelines_ns += nanoseconds;
        self.pipelines_created += 1;
        self.pipeline_ns += nanoseconds;
        self.pipeline_ns_max = self.pipeline_ns_max.max(nanoseconds);
    }
}

/// Worker threads that compile render pipelines (see [`Replayer::set_async_pipelines`]).
struct AsyncPipelines {
    requests: std::sync::mpsc::Sender<PipelineKey>,
    done: std::sync::mpsc::Receiver<(PipelineKey, wgpu::RenderPipeline, u64)>,
    pending: HashSet<PipelineKey>,
}

impl AsyncPipelines {
    fn new(
        device: wgpu::Device,
        module: Arc<wgpu::ShaderModule>,
        layout: Arc<wgpu::PipelineLayout>,
        threads: usize,
    ) -> Self {
        let (requests, request_rx) = std::sync::mpsc::channel::<PipelineKey>();
        let (done_tx, done) = std::sync::mpsc::channel();
        let request_rx = Arc::new(std::sync::Mutex::new(request_rx));
        for _ in 0..threads {
            let (device, module, layout) = (device.clone(), module.clone(), layout.clone());
            let (request_rx, done_tx) = (request_rx.clone(), done_tx.clone());
            std::thread::Builder::new()
                .name("tsfp-pipeline".into())
                .spawn(move || {
                    loop {
                        let Ok(key) = request_rx.lock().unwrap().recv() else {
                            return; // the renderer is gone
                        };
                        let started = std::time::Instant::now();
                        let pipeline = build_pipeline(&device, &module, &layout, key);
                        let nanoseconds = started.elapsed().as_nanos() as u64;
                        if done_tx.send((key, pipeline, nanoseconds)).is_err() {
                            return;
                        }
                    }
                })
                .expect("pipeline worker thread");
        }
        Self {
            requests,
            done,
            pending: HashSet::new(),
        }
    }

    fn request(&mut self, key: PipelineKey) {
        if self.pending.insert(key) {
            let _ = self.requests.send(key);
        }
    }

    /// Pipelines the workers finished since the last call.
    fn finished(&mut self) -> Vec<(PipelineKey, wgpu::RenderPipeline, u64)> {
        let finished: Vec<_> = self.done.try_iter().collect();
        for (key, _, _) in &finished {
            self.pending.remove(key);
        }
        finished
    }
}

/// One presented frame's cost (the difference of [`PhaseTimings`] over the frame), kept per frame for the spike analysis
/// (T2136f). Nanoseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameSample {
    pub total_ns: u64,
    pub pipelines_ns: u64,
    pub pipelines_created: u64,
    pub textures_ns: u64,
    pub texture_uploads: u64,
    /// Coalesced texture staging buffers and padded bytes (opt-in cold path).
    pub texture_upload_batches: u64,
    pub texture_staging_bytes: u64,
    pub bind_groups_ns: u64,
    pub encode_ns: u64,
    pub submit_ns: u64,
    pub readback_ns: u64,
    /// Blocking GPU completion poll on the CPU readback route, separate from in-flight throttling.
    pub readback_wait_ns: u64,
    pub readback_waits: u64,
    /// Callback delivery after poll and CPU row unpadding (not GPU execution timings).
    pub map_callback_ns: u64,
    pub row_copy_ns: u64,
    pub present_bind_cache_hits: u64,
    pub present_bindings_created: u64,
    pub present_encode_ns: u64,
    pub draws: u64,
}

impl FrameSample {
    fn between(before: &PhaseTimings, after: &PhaseTimings) -> Self {
        Self {
            total_ns: after.total_ns - before.total_ns,
            pipelines_ns: after.pipelines_ns - before.pipelines_ns,
            pipelines_created: after.pipelines_created - before.pipelines_created,
            textures_ns: after.textures_ns - before.textures_ns,
            texture_uploads: after.texture_uploads - before.texture_uploads,
            texture_upload_batches: after.texture_upload_batches - before.texture_upload_batches,
            texture_staging_bytes: after.texture_staging_bytes - before.texture_staging_bytes,
            bind_groups_ns: after.bind_groups_ns - before.bind_groups_ns,
            encode_ns: after.encode_ns - before.encode_ns,
            submit_ns: after.submit_ns - before.submit_ns,
            readback_ns: after.readback_ns - before.readback_ns,
            readback_wait_ns: after.readback_wait_ns - before.readback_wait_ns,
            readback_waits: after.readback_waits - before.readback_waits,
            map_callback_ns: after.map_callback_ns - before.map_callback_ns,
            row_copy_ns: after.row_copy_ns - before.row_copy_ns,
            present_bind_cache_hits: after.present_bind_cache_hits - before.present_bind_cache_hits,
            present_bindings_created: after.present_bindings_created
                - before.present_bindings_created,
            present_encode_ns: after.present_encode_ns - before.present_encode_ns,
            draws: after.draws - before.draws,
        }
    }
}

/// T2294: frames in a row a back buffer must start with a full colour clear before its draws may be pruned.
const CLEAR_STREAK: u32 = 8;

/// Executes streams on a [`GpuContext`].
pub struct Replayer {
    pub shared_frames: Option<std::sync::Arc<std::sync::Mutex<crate::shared::SharedRing>>>,
    gpu: GpuContext,
    width: u32,
    height: u32,
    /// Physical size of every render surface and of the frames: the logical size times the render scale (T2136c). Geometry is
    /// normalised by the logical size in the shader, so only allocations, copies, scissors and the readback use these.
    out_width: u32,
    out_height: u32,
    /// Copy each finished frame to the CPU (`Frame`). Off for a caller that takes [`Replayer::display_texture`] instead.
    pub readback: bool,
    pub readback_ring: crate::readback::ReadbackRing,
    pub readback_present: u64,
    pub suppress_presentation: bool,
    display_texture: Option<wgpu::Texture>,
    /// T2095: window sized present target (None = frames are the internal size). The finished frame goes through the sharp bilinear
    /// present pass, aspect preserved with black bars, and the display texture / readback are then this size.
    window_size: Option<(u32, u32)>,
    presenter: Option<crate::present::Presenter>,
    /// T2347: the native colour grade pass, drawn instead of the replayed closing grade draw (control `--opt replay_grade`).
    native_grade: Option<crate::grade::NativeGrade>,
    /// T2347: the grade of the last frame that had one (latched per guest frame, for display-rate re-grading).
    pub last_grade: Option<crate::grade::GradeModel>,
    /// T2347: closing grade draws drawn by the native pass and by the replayed combiner.
    pub native_grades: u64,
    pub replayed_grades: u64,
    window_target: Option<wgpu::Texture>,
    /// T2309: private copy of the shown guest surface the frame layer draws on (no window present pass).
    layer_composite: Option<wgpu::Texture>,
    /// T2309: presented frames handed to the frame layer, and how many it drew on.
    pub layer_frames: u64,
    pub layer_drawn: u64,
    /// Cost counters of the frame phases (T2136c), summed over all presented frames.
    pub timings: PhaseTimings,
    /// Per frame cost samples (first 65,536 presented frames), T2136f.
    pub samples: Vec<FrameSample>,
    /// Texture views kept between frames by texture id: the image generation they were built from and the view.
    view_cache: RefCell<TextureViewCache>,
    /// Bumped whenever the image bound to a texture id changes.
    texture_generation: HashMap<u32, u64>,
    /// Frames submitted and not yet finished by the GPU (only tracked when the frame is not read back).
    in_flight: Arc<AtomicUsize>,
    module: Arc<wgpu::ShaderModule>,
    /// Persistent per frame buffers (T2136f): all draw parameter blocks of a frame (`PARAM_STRIDE` bytes each, dynamic offsets),
    /// the vertices and the indices. Grown, never recreated per frame.
    param_buffer: Option<(wgpu::Buffer, u64)>,
    param_generation: u64,
    vertex_buffer: Option<(wgpu::Buffer, u64)>,
    index_buffer: Option<(wgpu::Buffer, u64)>,
    /// Bind groups by what they bind (views, samplers, parameter buffer), samplers by (nearest, u mode, v mode), and the
    /// 1x1 stand-in views of stages with no texture (black, white).
    bind_cache: HashMap<BindKey, wgpu::BindGroup>,
    sampler_cache: HashMap<(bool, u8, u8), wgpu::Sampler>,
    null_views: RefCell<[Option<wgpu::TextureView>; 2]>,
    surface_counter: std::cell::Cell<u64>,
    /// Sum, max and count of the submit to completion latency of the frames (written by the completion callback).
    latency: Arc<(
        std::sync::atomic::AtomicU64,
        std::sync::atomic::AtomicU64,
        std::sync::atomic::AtomicU64,
    )>,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: Arc<wgpu::PipelineLayout>,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    /// T2136d: build pipelines on worker threads (live display). A draw whose pipeline is not ready yet is skipped instead of
    /// stalling the render thread for the driver's compile (MEASURED on the owner's machine: 300 to 570 ms spikes).
    async_pipelines: Option<AsyncPipelines>,
    /// T2024k: vertex programs on the GPU (compute), created on first use, see `gpu_vsh.rs`.
    gpu_vsh: Option<gpu_vsh::GpuVsh>,
    gpu_vsh_choice: Option<bool>,
    /// Masked colour clears (a clear that writes only some channels, for example the alpha only clear the engine issues between
    /// post process passes) are a full screen triangle with a colour write mask: shader, layout and one pipeline per mask.
    clear_module: wgpu::ShaderModule,
    clear_layout: wgpu::BindGroupLayout,
    clear_pipeline_layout: wgpu::PipelineLayout,
    clear_pipelines: HashMap<u8, wgpu::RenderPipeline>,
    pub report: Report,
    /// When false draws and presents are skipped (state, memory and constants still apply), to fast forward a long stream.
    pub rendering: bool,
    /// Print this many shaded vertices (inputs, position, clip result) to stderr, a bring up aid.
    pub debug_vertices: usize,
    /// Skip texture decoding (every draw is untextured): a bring up aid to separate geometry from texture problems.
    pub textures_enabled: bool,
    /// Keep only the first n draws of each frame (a bring up aid for finding the draw that makes a frame go wrong).
    pub max_draws: usize,
    /// Skip draws whose pixel shader samples textures 1 to 3 (the slice 3 behaviour, when only stage 0 was bound). Default false
    /// since slice 4 binds all four stages.
    pub skip_multitexture: bool,
    /// Skip draws that sample a texture record with no texel data (a frame buffer surface) instead of binding a copy of the
    /// current render target for them (slice 4). Default false.
    pub skip_framebuffer_samples: bool,
    /// When the target is copied for draws that sample the surface they render into, see [`FbSnapshot`]. Default per draw (slice 5: the host and xemu copy per draw).
    pub fb_snapshot: FbSnapshot,
    /// Sample non linear (swizzled, power of two) textures with repeat addressing, linear (rectangle) ones clamped. INFERRED:
    /// the D3D8 default is WRAP and the engine's ribbon draws run u over several repeats (slice 4, docs/t-wgpu-renderer.md). The
    /// stream carries no address mode word. Default false (the slice 1 scenes and their tests clamp), `examples/replay.rs` turns it
    /// on for recorded streams unless `--clamp` is given.
    pub wrap_textures: bool,
    /// Colour of the full screen overwrite quad that ended the previous frame, applied as a clear at the start of this one.
    carry_clear: Option<u32>,
    /// T2294: surfaces any draw has ever sampled (render to texture, feedback, frame buffer record). Never pruned.
    ever_sampled: HashSet<u32>,
    /// T2294: surfaces that have been the displayed surface of a frame (the back buffers). Only these are pruned.
    ever_displayed: HashSet<u32>,
    /// T2294: per surface, consecutive frames whose first op on it is a full colour clear (it does not depend on older content).
    clear_streak: HashMap<u32, u32>,
    /// T2294: draws omitted from suppressed presents because their target is never read back.
    pub unread_draws_skipped: u64,
    /// The decoded pixel shader (`SetPixelShader`), None until one with a definition is bound.
    pixel: Option<PixelProgram>,
    /// The bound pixel shader samples textures 1 to 3.
    pixel_other_textures: bool,
    // stream state
    memory: SparseMemory,
    bound: Option<(u32, u32)>,
    state: PipelineState,
    clear_colour: u32,
    texture_id: u32,
    nearest: bool,
    extra_textures: [u32; 3],
    extra_nearest: [bool; 3],
    scissor: Option<[i32; 4]>,
    textures: HashMap<u32, TextureImage>,
    /// Texture records whose header decodes but whose texel snapshot is all zero or absent: frame buffer surfaces the engine samples
    /// for post processing (the contents were never in the stream, render to texture is not replayed).
    empty_textures: HashSet<u32>,
    /// Size of the frame buffer surface records (record -> width, height, linear): the vertex program writes linear texture
    /// coordinates in texels, they are normalised by this.
    fb_surfaces: HashMap<u32, (u32, u32, bool)>,
    // vertex program state (slice 3)
    program_memory: ProgramMemory,
    vsh_constants: [Vec4; CONSTANT_COUNT],
    /// NV2A constant-load cursor in dwords, shared by B80..BFC upload methods.
    constant_cursor: usize,
    vsh_start: Option<usize>,
    declaration: [DeclEntry; INPUT_COUNT],
    /// NV2A vertex arrays the engine sets up itself (`VERTEX_DATA_ARRAY_OFFSET` 0x1720 + 4i, `_FORMAT` 0x1760 + 4i: type in bits
    /// 0..3, component count 4..7, stride 8..31), one array per vertex attribute register. Active from the first such write until
    /// the next `BindVertexStream` host call (which makes the lifted library write the arrays, out of sight).
    arrays: [(u32, u32); INPUT_COUNT],
    arrays_active: bool,
    pixel_shader: u32,
    /// A draw call waits for the library's own pushes that follow it in the stream (texture addressing, enables) before it runs.
    pending_draw: Option<PendingDraw>,
    /// Render target surfaces by Data word (slice 5): width and height. Empty for streams without `Surface` events.
    surfaces: HashMap<u32, (u32, u32)>,
    /// Surfaces with a depth buffer bound when last set.
    surface_depth: HashMap<u32, bool>,
    current_surface: u32,
    /// The target before the last `Surface` switch (the scene target the closing grade pass reads).
    previous_surface: u32,
    display_surface: u32,
    /// The surface the first op of the next frame draws into (the target at the end of the previous frame).
    frame_start_surface: u32,
    /// Surfaces that persist between frames.
    surface_gpu: HashMap<u32, SurfaceGpu>,
    /// Experiment switches set by `--opt name[=value]` of the replay example (documented where they are read: `legacy_fb`,
    /// `no_lib_address`, and the slice 5 grade experiments). Empty in normal use.
    pub knobs: BTreeMap<String, String>,
    /// Use the `Surface` events (render to texture). False replays them as the slice 4 single target with the copy heuristic.
    pub use_surfaces: bool,
    /// Use the per stage texture address words of `lib` events. False keeps the slice 4 inference (repeat for swizzled textures).
    pub use_lib_address: bool,
    /// Last value of every method the D3D8 library pushed itself (`lib` events, slice 5), by method number.
    pub lib_regs:
        HashMap<u32, u32, std::hash::BuildHasherDefault<tsfp_engine_render::mem::PageHasher>>,
    /// Bumped whenever texel data (replay image addresses with the physical alias bit) is written.
    texel_epoch: u64,
    /// Texture record address -> (epoch, header words) of the image in `textures`.
    /// Also the texel range (address, bytes) the decode read.
    texture_versions: HashMap<u32, (u64, [u32; 5], u32, u32, u64)>,
    /// 4 KiB page of texel memory -> the epoch of its last write (T2136f): a texture is only decoded again when a write hit its own
    /// texel range, instead of after any texel write anywhere.
    seen_texel_hashes: std::collections::HashSet<(u32, u32, u64)>,
    texel_pages:
        HashMap<u32, u64, std::hash::BuildHasherDefault<tsfp_engine_render::mem::PageHasher>>,
    // frame state
    vertices: Vec<u8>,
    indices: Vec<u32>,
    ops: Vec<Op>,
    pub frames: Vec<Frame>,
}

impl Replayer {
    /// Render at `width` x `height` pixels instead of the logical size (T2136c). Call between frames: surfaces are
    /// recreated cleared at this size when next used; accumulated render-to-texture contents are discarded. Geometry, scissors and texture coordinates stay in logical units.
    pub fn set_output_size(&mut self, width: u32, height: u32) {
        assert!(width > 0 && height > 0, "output size must be non zero");
        if (width, height) != (self.out_width, self.out_height) {
            // Runtime resize (T2136e): every GPU surface and the handed out display texture have the old size. Drop them, they
            // are recreated cleared on first use. Stream state (programs, constants, textures, combiner) is size independent.
            self.surface_gpu.clear();
            self.bind_cache.clear();
            self.display_texture = None;
        }
        self.out_width = width;
        self.out_height = height;
    }

    /// T2095: present at `size` (window pixels) through the sharp bilinear pass with the logical aspect preserved, or `None` for the
    /// internal size. Safe between frames at any time; the internal size (`set_output_size`) is independent.
    pub fn set_window_size(&mut self, size: Option<(u32, u32)>) {
        assert!(
            size.is_none_or(|(w, h)| w > 0 && h > 0),
            "window size must be non zero"
        );
        if size != self.window_size {
            self.window_target = None;
            self.display_texture = None;
        }
        self.window_size = size;
    }

    /// The window sized present target size, if any.
    pub fn window_size(&self) -> Option<(u32, u32)> {
        self.window_size
    }

    /// The GPU texture of the last finished frame (`Rgba8`, the physical size, `COPY_SRC`), valid until the next frame is drawn.
    /// Same device as [`Replayer::device`]; with `readback` off this is the only output.
    pub fn display_texture(&self) -> Option<&wgpu::Texture> {
        self.display_texture.as_ref()
    }

    /// Service pending CPU pictures without blocking GPU completion.
    pub fn poll_readbacks(&mut self, now: u64) -> Vec<(u64, Frame)> {
        self.poll_gpu();
        let started = std::time::Instant::now();
        let frames = self.readback_ring.poll(&self.gpu.device, now);
        self.timings.readback_ns += started.elapsed().as_nanos() as u64;
        frames
    }

    /// The device and queue the renderer draws with.
    pub fn device_and_queue(&self) -> (&wgpu::Device, &wgpu::Queue) {
        (&self.gpu.device, &self.gpu.queue)
    }

    /// The physical frame size.
    pub fn output_size(&self) -> (u32, u32) {
        (self.out_width, self.out_height)
    }

    pub fn new(gpu: GpuContext, width: u32, height: u32) -> Self {
        let device = &gpu.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsfp-hud"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        // binding 0 params, then per stage s: texture at 1 + 2s, sampler at 2 + 2s (slice 4 binds all four stages)
        let mut layout_entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: None,
            },
            count: None,
        }];
        for stage in 0..4u32 {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: 1 + 2 * stage,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: 2 + 2 * stage,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let clear_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsfp-clear"),
            source: wgpu::ShaderSource::Wgsl(CLEAR_SHADER.into()),
        });
        let clear_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tsfp-clear-bind"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let clear_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tsfp-clear-layout"),
                bind_group_layouts: &[&clear_layout],
                push_constant_ranges: &[],
            });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tsfp-hud-bind"),
            entries: &layout_entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tsfp-hud-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        Self {
            gpu,
            width,
            height,
            out_width: width,
            out_height: height,
            readback: true,
            readback_ring: Default::default(),
            shared_frames: None,
            readback_present: 0,
            suppress_presentation: false,
            display_texture: None,
            window_size: None,
            presenter: None,
            native_grade: None,
            last_grade: None,
            native_grades: 0,
            replayed_grades: 0,
            window_target: None,
            layer_composite: None,
            layer_frames: 0,
            layer_drawn: 0,
            timings: PhaseTimings::default(),
            samples: Vec::new(),
            view_cache: RefCell::new(HashMap::new()),
            texture_generation: HashMap::new(),
            in_flight: Arc::new(AtomicUsize::new(0)),
            gpu_vsh: None,
            gpu_vsh_choice: None,
            module: Arc::new(module),
            param_buffer: None,
            param_generation: 0,
            vertex_buffer: None,
            index_buffer: None,
            bind_cache: HashMap::new(),
            sampler_cache: HashMap::new(),
            null_views: RefCell::new([None, None]),
            surface_counter: std::cell::Cell::new(0),
            latency: Arc::default(),
            layout,
            pipeline_layout: Arc::new(pipeline_layout),
            pipelines: HashMap::new(),
            async_pipelines: None,
            clear_module,
            clear_layout,
            clear_pipeline_layout,
            clear_pipelines: HashMap::new(),
            report: Report::default(),
            rendering: true,
            debug_vertices: 0,
            textures_enabled: true,
            max_draws: usize::MAX,
            skip_multitexture: false,
            skip_framebuffer_samples: false,
            fb_snapshot: FbSnapshot::PerDraw,
            wrap_textures: false,
            carry_clear: None,
            ever_sampled: HashSet::new(),
            ever_displayed: HashSet::new(),
            clear_streak: HashMap::new(),
            unread_draws_skipped: 0,
            pixel: None,
            pixel_other_textures: false,
            memory: SparseMemory::new(),
            bound: None,
            state: PipelineState::default(),
            clear_colour: 0,
            texture_id: 0,
            nearest: false,
            extra_textures: [0; 3],
            extra_nearest: [false; 3],
            scissor: None,
            textures: HashMap::new(),
            empty_textures: HashSet::new(),
            fb_surfaces: HashMap::new(),
            program_memory: ProgramMemory::default(),
            vsh_constants: default_vsh_constants(width, height),
            constant_cursor: 0,
            vsh_start: None,
            declaration: [DeclEntry::default(); INPUT_COUNT],
            arrays: [(0, 0); INPUT_COUNT],
            arrays_active: false,
            pixel_shader: 0,
            pending_draw: None,
            surfaces: HashMap::new(),
            surface_depth: HashMap::new(),
            current_surface: 0,
            previous_surface: 0,
            display_surface: 0,
            frame_start_surface: 0,
            surface_gpu: HashMap::new(),
            knobs: BTreeMap::new(),
            use_surfaces: true,
            use_lib_address: true,
            lib_regs: HashMap::default(),
            texel_epoch: 0,
            texture_versions: HashMap::new(),
            texel_pages: HashMap::default(),
            seen_texel_hashes: Default::default(),
            vertices: Vec::new(),
            indices: Vec::new(),
            ops: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// Name, backend, type, driver and the limits that matter, as one line (the `wgpu-live:` banner of the host).
    pub fn adapter_description(&self) -> String {
        self.gpu.describe()
    }

    pub fn adapter_name(&self) -> String {
        format!(
            "{} ({}, {})",
            self.gpu.info.name, self.gpu.info.backend, self.gpu.info.device_type
        )
    }

    /// Run a whole stream. A trailing frame without `present` is resolved too.
    pub fn run(&mut self, stream: &Stream) {
        for event in &stream.events {
            self.step(event);
        }
        if !self.ops.is_empty() {
            self.present();
        }
    }

    /// Execute one event (streaming use: the caller keeps no event list).
    pub fn feed(&mut self, event: &Event) {
        self.step(event);
    }

    /// Queue a native view-pass replacement at this exact command boundary.
    /// Flush delayed library-backed draws first: an earlier draw must precede the layer.
    /// The caller must mark the skipped original draw, before any framebuffer feedback copy.
    /// This is a renderer contract; it does not infer a boundary from shaders or draw counts.
    pub fn view_layer_boundary(&mut self) {
        self.flush_draw();
        let target = if self.surfaces.is_empty() {
            LEGACY_SURFACE
        } else {
            self.current_surface
        };
        self.ops.push(Op::ViewLayer(target));
    }

    /// True when the experiment switch `name` is set and not `0`.
    pub fn knob(&self, name: &str) -> bool {
        self.knobs.get(name).is_some_and(|value| value != "0")
    }

    /// The numeric value of experiment switch `name`.
    pub fn knob_value(&self, name: &str) -> Option<f32> {
        self.knobs.get(name).and_then(|value| value.parse().ok())
    }

    /// Run the draw call that waited for the library pushes that follow it.
    fn flush_draw(&mut self) {
        if let Some((primitive, run, indexed)) = self.pending_draw.take() {
            self.draw(primitive, run, indexed);
        }
    }

    fn step(&mut self, event: &Event) {
        if !matches!(event, Event::Lib { .. }) {
            self.flush_draw();
        }
        match event {
            Event::Call(call) => self.call(call),
            Event::Host(function, args) => {
                if is_bind_vertex_stream(*function) && args.len() == 2 {
                    self.bound = Some((args[0], args[1]));
                    self.arrays_active = false;
                } else {
                    self.report.ignore("host_call");
                }
            }
            Event::Memory { address, bytes } => {
                if *address >= 0x8000_0000 && !bytes.is_empty() {
                    // Only pages whose bytes really changed count as a texel write (T2136f).
                    let epoch = self.texel_epoch + 1;
                    let mut any = false;
                    let pages = &mut self.texel_pages;
                    let different = self.memory.write_bytes_diff(*address, bytes, |page| {
                        pages.insert(page, epoch);
                        any = true;
                    });
                    if address.checked_add(bytes.len() as u32 - 1).is_none() {
                        // wraps at 4 GiB: treat as a write everywhere
                        self.texel_pages.insert(u32::MAX, epoch);
                        any = true;
                    }
                    if any {
                        self.texel_epoch = epoch;
                    }
                    self.timings.texel_bytes += bytes.len() as u64;
                    self.timings.texel_bytes_changed += different as u64;
                } else {
                    self.memory.write_bytes(*address, bytes);
                }
            }
            Event::Texture {
                id,
                width,
                height,
                rgba,
            } => {
                *self.texture_generation.entry(*id).or_insert(0) += 1;
                self.textures.insert(
                    *id,
                    TextureImage {
                        width: *width,
                        height: *height,
                        rgba: rgba.clone(),
                        linear: false,
                        mips: 1,
                    },
                );
            }
            Event::Surface {
                data,
                size,
                depth,
                flags,
                ..
            } => {
                if self.use_surfaces && !self.knob("legacy_fb") {
                    let (width, height) = ((size & 0xFFF) + 1, ((size >> 12) & 0xFFF) + 1);
                    if (width, height) != (self.width, self.height) {
                        *self
                            .report
                            .pixel_shader_notes
                            .entry("surface size differs from the replay size (not supported)")
                            .or_insert(0) += 1;
                    }
                    self.surfaces.insert(*data, (width, height));
                    self.surface_depth.insert(*data, *depth != 0);
                    if self.current_surface != *data {
                        self.previous_surface = self.current_surface;
                    }
                    self.current_surface = *data;
                    if flags & 1 != 0 {
                        self.display_surface = *data;
                    }
                    self.ops.push(Op::Target(*data));
                }
            }
            Event::Lib { method, value } => {
                if self.knob("no_lib") {
                    return;
                }
                // Library pushbuffer uploads also come from lifted callers that never pass
                // through the high-level Device wrappers. Preserve their ordered constant file.
                if *method == 0x1EA4 {
                    self.flush_draw();
                    self.constant_cursor = (*value as usize).saturating_mul(4);
                } else if (0x0B80..0x0C00).contains(method) && method & 3 == 0 {
                    self.flush_draw();
                    let cursor = self.constant_cursor;
                    if cursor < CONSTANT_COUNT * 4 {
                        self.vsh_constants[cursor / 4][cursor % 4] = f32::from_bits(*value);
                    } else {
                        self.report.ignore("constant_upload_out_of_range");
                    }
                    self.constant_cursor = cursor.saturating_add(1);
                }
                self.lib_regs.insert(*method, *value);
                // The methods the pipeline state decodes (blend, depth, cull, colour mask, clear) apply like an engine push; the
                // vertex array methods stay with the engine's own array setup, everything else is only recorded in `lib_regs`.
                if !(0x1720..=0x179C).contains(method) {
                    let mut scratch = Report::default();
                    if let Some(clear) = self.state.apply(
                        method | 0x4_0000,
                        *value,
                        &mut self.clear_colour,
                        &mut scratch,
                    ) {
                        let clear = self.clear_with_mask(clear);
                        self.ops.push(Op::Clear(clear));
                    }
                }
            }
            Event::Present => self.present(),
            Event::ViewLayerBoundary => self.view_layer_boundary(),
        }
    }

    fn call(&mut self, call: &DeviceCall) {
        use DeviceCall as C;
        match call {
            C::PushMethod { header, value } => {
                match crate::nv2a::header_method(*header) {
                    method @ 0x1720..=0x175C => {
                        self.arrays[((method - 0x1720) / 4) as usize].0 = *value;
                        self.arrays_active = true;
                    }
                    method @ 0x1760..=0x179C => {
                        self.arrays[((method - 0x1760) / 4) as usize].1 = *value;
                        self.arrays_active = true;
                    }
                    _ => {}
                }
                if let Some(clear) =
                    self.state
                        .apply(*header, *value, &mut self.clear_colour, &mut self.report)
                {
                    let clear = self.clear_with_mask(clear);
                    self.ops.push(Op::Clear(clear));
                }
            }
            C::SetVertexBindings { formats, offsets } => {
                for index in 0..16 {
                    self.arrays[index] = (offsets[index], formats[index]);
                }
                self.arrays_active = true;
            }
            C::SetTexture { stage, texture } => {
                if *stage == 0 {
                    self.texture_id = *texture;
                    if self.rendering && self.textures_enabled {
                        self.decode_texture(*texture);
                    }
                } else if *stage < 4 {
                    self.extra_textures[*stage as usize - 1] = *texture;
                    if self.rendering && self.textures_enabled {
                        self.decode_texture(*texture);
                    }
                } else {
                    self.report.ignore("set_texture_stage4+");
                }
            }
            C::DeferTextureStageWord {
                stage,
                word: _,
                value,
            } => {
                // Filter control: 1 is point sampling. Both words of stage 0 share one sampler
                // here (min and mag are set together by bind_texture_with_constants).
                if *stage == 0 {
                    self.nearest = *value == 1;
                } else if *stage < 4 {
                    self.extra_nearest[*stage as usize - 1] = *value == 1;
                } else {
                    self.report.ignore("texture_stage_word_stage4+");
                }
            }
            C::SetScissor { rects, .. } => self.scissor = rects.first().copied(),
            C::DrawVertices {
                primitive,
                start,
                count,
            } => self.pending_draw = Some((*primitive, Some((*start, *count)), None)),
            C::DrawIndexedVertices {
                primitive,
                count,
                indices,
            } => self.pending_draw = Some((*primitive, None, Some((*indices, *count)))),
            C::DeferState { .. } => self.report.ignore("defer_state"),
            C::StateHandler { .. } => self.report.ignore("state_handler"),
            C::SetVertexConstants {
                first_register,
                words,
            }
            | C::SetVertexConstantsUncached {
                first_register,
                words,
            }
            | C::SetVertexConstantOneVector {
                first_register,
                words,
            }
            | C::SetVertexConstantFourVectors {
                first_register,
                words,
            } => {
                for (offset, word) in words.iter().enumerate() {
                    let register = *first_register as usize + offset / 4;
                    if register < CONSTANT_COUNT {
                        self.vsh_constants[register][offset % 4] = f32::from_bits(*word);
                    }
                }
            }
            C::SetPixelConstants { .. } => self.report.ignore("pixel_constants"),
            C::LoadVertexProgram { program, address } => self.load_program(*program, *address),
            C::SelectVertexDeclaration {
                declaration,
                address,
            } => self.select_declaration(*declaration, *address),
            C::SetPixelShader { program } => {
                self.pixel_shader = *program;
                self.pixel = None;
                self.pixel_other_textures = false;
                if *program != 0 {
                    let words: Vec<u32> = (0..0x40u32)
                        .map(|i| self.memory.read32(program.wrapping_add(i * 4)))
                        .collect();
                    self.pixel = PixelProgram::from_words(&words);
                    if let Some(decoded) = self.pixel {
                        self.pixel_other_textures = decoded.uses_other_textures();
                        if self.pixel_other_textures {
                            *self
                                .report
                                .pixel_shader_notes
                                .entry("shader samples textures 1 to 3")
                                .or_insert(0) += 1;
                        }
                    }
                    self.report.pixel_shaders.insert(*program, words);
                }
            }
            C::InsertFence
            | C::BlockUntilFence { .. }
            | C::LockVertexBuffer { .. }
            | C::BeginPush { .. }
            | C::EndPush { .. } => {}
        }
    }

    fn draw(&mut self, primitive: u32, run: Option<(u32, u32)>, indexed: Option<(u32, u32)>) {
        let started = std::time::Instant::now();
        self.draw_inner(primitive, run, indexed);
        self.timings.draw_ns += started.elapsed().as_nanos() as u64;
    }

    fn draw_inner(&mut self, primitive: u32, run: Option<(u32, u32)>, indexed: Option<(u32, u32)>) {
        if !self.rendering
            || self
                .ops
                .iter()
                .filter(|op| matches!(op, Op::Draw(_)))
                .count()
                >= self.max_draws
        {
            return;
        }
        if self.skip_multitexture && self.pixel_other_textures {
            *self
                .report
                .pixel_shader_notes
                .entry("draws skipped: shader samples textures 1 to 3")
                .or_insert(0) += 1;
            return;
        }
        let stage_ids = [
            self.texture_id,
            self.extra_textures[0],
            self.extra_textures[1],
            self.extra_textures[2],
        ];
        let mask = self.pixel.map_or(1, |program| program.texture_mask());
        // Slice 5: a texture record whose Data word is the Data word of a render target surface samples that surface.
        let mut surface_stage = [None; 4];
        if !self.surfaces.is_empty() {
            for stage in 0..4usize {
                if mask >> stage & 1 != 0 && stage_ids[stage] != 0 {
                    let data = self.memory.read32(stage_ids[stage].wrapping_add(4));
                    if self.surfaces.contains_key(&data) {
                        surface_stage[stage] = Some(data);
                    }
                }
            }
        }
        // T2140 (the cause of the slice 8 brightness gap): the closing full screen pass of a frame, one quad into the other buffer of
        // the flip chain with a combiner that samples texture 0, has no texture bound (`SetTexture 0 0`): it reads the scene target
        // (the surface drawn into before the last `Surface` switch) in pixel coordinates and writes the displayed image (a 5 stage
        // grade: luma, saturation x1.12, gain 1.09, bias -10/255 from the library's combiner constants). Earlier slices rendered it with
        // a flat clear: the pass was drawn into the other buffer with a null texture (black) and never displayed, the replay showed the
        // scene target, which is the grade's input. Control: `--opt no_grade_pass`.
        let grade = !self.knob("no_grade_pass")
            && !self.surfaces.is_empty()
            && mask & 1 != 0
            && stage_ids[0] == 0
            && self.previous_surface != self.current_surface
            && self.surfaces.contains_key(&self.previous_surface);
        let grade_candidate = grade;
        if grade {
            surface_stage[0] = Some(self.previous_surface);
        }
        let fb_mask = if self.surfaces.is_empty() {
            (0..4u32)
                .filter(|&stage| {
                    mask >> stage & 1 != 0
                        && self.empty_textures.contains(&stage_ids[stage as usize])
                })
                .fold(0, |bits, stage| bits | 1 << stage)
        } else {
            0
        };
        if fb_mask != 0 && self.skip_framebuffer_samples {
            // INFERRED: a post process draw sampling a frame buffer surface (MEASURED: record format 0x12 linear 640x480, no
            // texel data in the stream). Without a target copy its colour is unknown, so it is left out.
            *self
                .report
                .pixel_shader_notes
                .entry("draws skipped: samples a frame buffer surface (render to texture not replayed)")
                .or_insert(0) += 1;
            return;
        }
        if fb_mask != 0 {
            *self
                .report
                .pixel_shader_notes
                .entry("draws that sample a copy of the render target")
                .or_insert(0) += 1;
        }
        let (base, stride) = self.bound.unwrap_or((0, 0));
        let use_program = self.vsh_start.is_some() && (self.arrays_active || stride != 0);
        if !use_program && self.bound.is_none() {
            *self.report.skipped_draws.entry((primitive, 0)).or_insert(0) += 1;
            return;
        }
        if !use_program && !stride_supported(stride) {
            *self
                .report
                .skipped_draws
                .entry((primitive, stride))
                .or_insert(0) += 1;
            return;
        }
        let lines = matches!(primitive, 2..=4) && !self.knob("no_lines");
        let assemble = |count: u32| {
            if lines {
                line_list(primitive, count)
            } else {
                triangulate(primitive, count)
            }
        };
        let (vertex_ids, order): (Vec<u32>, Vec<u32>) = if let Some((start, count)) = run {
            let Some(order) = assemble(count) else {
                *self
                    .report
                    .skipped_draws
                    .entry((primitive, stride))
                    .or_insert(0) += 1;
                return;
            };
            ((start..start + count).collect(), order)
        } else {
            let (address, count) = indexed.unwrap_or_default();
            let ids: Vec<u32> = (0..count)
                .map(|i| u32::from(self.memory.read16(address.wrapping_add(i * 2))))
                .collect();
            let Some(order) = assemble(count) else {
                *self
                    .report
                    .skipped_draws
                    .entry((primitive, stride))
                    .or_insert(0) += 1;
                return;
            };
            // `order` indexes the index list, map through it.
            let mapped = order
                .iter()
                .map(|&position| ids[position as usize])
                .collect::<Vec<_>>();
            (mapped.clone(), (0..mapped.len() as u32).collect())
        };
        let vertex_base = (self.vertices.len() / Vertex::SIZE) as u32;
        let first_index = self.indices.len() as u32;
        let mut index_count = order.len() as u32;
        let mut clear_colour = None;
        let mut overwrite_quad = false;
        let mut behind_eye = 0usize;
        let mut pixel_box = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        if self.debug_vertices > 0 {
            eprintln!(
                "draw primitive {primitive} stride {stride} ids {} state {:?} texture {:x} have {} scissor {:?} shader {:x} fb_mask {}",
                vertex_ids.len(),
                self.state,
                self.texture_id,
                self.textures.contains_key(&self.texture_id),
                self.scissor,
                self.pixel_shader,
                fb_mask
            );
        }
        let gpu_started = std::time::Instant::now();
        let gpu_indices = if use_program {
            self.gpu_shade(base, stride, &vertex_ids, &order, if lines { 2 } else { 3 })
        } else {
            None
        };
        self.timings.gpu_prep_ns += gpu_started.elapsed().as_nanos() as u64;
        if let Some(count) = gpu_indices {
            index_count = count;
            self.report.vsh_draws += 1;
        } else if use_program {
            let shade_started = std::time::Instant::now();
            let (shaded, indices) =
                self.shade_vertices(base, stride, &vertex_ids, &order, if lines { 2 } else { 3 });
            self.timings.shade_ns += shade_started.elapsed().as_nanos() as u64;
            self.timings.shaded_vertices += shaded.len() as u64;
            for vertex in &shaded {
                vertex.write(&mut self.vertices);
            }
            index_count = indices.len() as u32;
            behind_eye = indices
                .chunks_exact(if lines { usize::MAX } else { 3 })
                .filter(|triangle| {
                    triangle
                        .iter()
                        .any(|&index| shaded[index as usize].position[3] < 0.0)
                })
                .count();
            self.report.vsh_triangles_behind_eye += behind_eye;
            for vertex in &shaded {
                let w = vertex.position[3];
                if w > 0.0 {
                    let x = (vertex.position[0] / w * 0.5 + 0.5) * self.width as f32;
                    let y = (0.5 - vertex.position[1] / w * 0.5) * self.height as f32;
                    pixel_box = [
                        pixel_box[0].min(x),
                        pixel_box[1].min(y),
                        pixel_box[2].max(x),
                        pixel_box[3].max(y),
                    ];
                }
            }
            clear_colour = clear_like_colour(&self.state, &shaded, indices.len());
            overwrite_quad = shaded.len() == 4 && indices.len() == 6;
            if self.knob("colour_report") && !shaded.is_empty() {
                let mut mean = [0.0f32; 4];
                for vertex in &shaded {
                    for (sum, value) in mean.iter_mut().zip(vertex.colour) {
                        *sum += value / shaded.len() as f32;
                    }
                }
                let start = self.vsh_start.unwrap_or(0);
                eprintln!(
                    "colour_report start {start} shader {:x} verts {} mean {:.3} {:.3} {:.3} {:.3} tex {:x}",
                    self.pixel_shader,
                    shaded.len(),
                    mean[0],
                    mean[1],
                    mean[2],
                    mean[3],
                    self.texture_id
                );
                if self.knob("constant_dump") {
                    for register in (0..12).chain([
                        24, 28, 29, 30, 31, 32, 33, 34, 35, 36, 58, 59, 60, 64, 65, 66, 67, 68, 70,
                    ]) {
                        eprintln!("    c[{register}] = {:?}", self.vsh_constants[register]);
                    }
                }
                if self.knob("program_dump") {
                    eprint!("{}", vsh::describe(&self.program_memory, start));
                }
            }
            if self.debug_vertices > 0 && !shaded.is_empty() {
                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                let mut colour = [0.0f32; 4];
                for vertex in &shaded {
                    let w = vertex.position[3];
                    for axis in 0..2 {
                        let ndc = vertex.position[axis] / w;
                        lo[axis] = lo[axis].min(ndc);
                        hi[axis] = hi[axis].max(ndc);
                    }
                    for (sum, value) in colour.iter_mut().zip(vertex.colour) {
                        *sum += value / shaded.len() as f32;
                    }
                }
                if self.debug_vertices > 0 {
                    for vertex in shaded.iter().take(6) {
                        eprintln!(
                            "    clip {:?} colour {:?} uv {:?}",
                            vertex.position, vertex.colour, vertex.uv
                        );
                    }
                    eprintln!("    indices {:?}", indices);
                }
                eprintln!(
                    "  shaded {} vertices {} triangles ndc x {:?}..{:?} y {:?}..{:?} mean colour {:?}",
                    shaded.len(),
                    indices.len() / 3,
                    lo[0],
                    hi[0],
                    lo[1],
                    hi[1],
                    colour
                );
            }
            self.indices
                .extend(indices.iter().map(|&i| vertex_base + i));
            self.report.vsh_draws += 1;
        } else {
            for id in &vertex_ids {
                let address = base.wrapping_add(id.wrapping_mul(stride));
                let bytes: Vec<u8> = (0..stride)
                    .map(|offset| self.memory.read8(address.wrapping_add(offset)))
                    .collect();
                let vertex = decode_vertex(&bytes, stride).expect("stride checked");
                vertex.write(&mut self.vertices);
            }
            self.indices.extend(order.iter().map(|&i| vertex_base + i));
        }
        if self.texture_id != 0 && !self.textures.contains_key(&self.texture_id) {
            *self
                .report
                .missing_textures
                .entry(self.texture_id)
                .or_insert(0) += 1;
        }
        *self
            .report
            .draws_by_pixel_shader
            .entry(self.pixel_shader)
            .or_insert(0) += 1;
        let mut draw_state = self.state;
        if !self.surfaces.is_empty()
            && !self
                .surface_depth
                .get(&self.current_surface)
                .copied()
                .unwrap_or(true)
        {
            draw_state.depth_test = false;
            draw_state.depth_write = false;
        }
        if self.knob("trace_draws") {
            eprintln!(
                "draw shader {:x} target {:x} depth {} blend {} {:?}->{:?} {:?} mask {:x} surf {:?} tex {:x?} clip {use_program} verts {} fb {} behind_eye {behind_eye} box {:.0?}",
                self.pixel_shader,
                if self.surfaces.is_empty() {
                    0
                } else {
                    self.current_surface
                },
                draw_state.depth_test,
                draw_state.blend_enable,
                draw_state.source,
                draw_state.destination,
                draw_state.equation,
                draw_state.colour_mask,
                surface_stage,
                stage_ids,
                vertex_ids.len(),
                fb_mask,
                pixel_box
            );
        }
        // only a quad (4 vertices, 2 triangles, One/Zero, no depth test) is the grade pass
        let grade = grade
            && overwrite_quad
            && !draw_state.depth_test
            && draw_state.blend_enable
            && draw_state.source == Factor::One
            && draw_state.destination == Factor::Zero;
        if grade_candidate && !grade {
            surface_stage[0] = None;
        }
        self.ops.push(Op::Draw(Box::new(DrawRecord {
            state: draw_state,
            texture: self.texture_id,
            nearest: self.nearest,
            extra_textures: self.extra_textures,
            extra_nearest: self.extra_nearest,
            fb_mask,
            scissor: self.scissor,
            lines,
            first_index,
            index_count,
            clip: use_program,
            clear_colour,
            pixel: self.effective_pixel(),
            target: if self.surfaces.is_empty() {
                LEGACY_SURFACE
            } else {
                self.current_surface
            },
            surface_stage,
            grade,
            fog_colour: self.fog_colour(),
            address: std::array::from_fn(|stage| {
                (self.use_lib_address
                    && !self.knob("no_lib_address")
                    && self.lib_regs.contains_key(&0x1B08))
                .then(|| self.lib_regs.get(&(0x1B08 + 0x40 * stage as u32)).copied())
                .flatten()
            }),
        })));
    }

    /// The bound pixel program with the combiner constants the library pushed after `SetPixelShader` (T2140, `lib` 0xA60 + 4 i
    /// factor 0 of stage i, 0xA80 + 4 i factor 1, 0x1E20 and 0x1E24 the final combiner constants, ARGB words). The definition in
    /// guest memory only holds the constants at bind time; `SetPixelShaderConstant` changes reach the draw through these pushes.
    /// Control: `--opt no_lib_combiner`.
    fn effective_pixel(&self) -> Option<PixelProgram> {
        let mut program = self.pixel?;
        if self.knob("no_lib_combiner") || self.knob("no_lib") {
            return Some(program);
        }
        let get = |method: u32| self.lib_regs.get(&method).copied();
        let unique0 = program.flags & 0x10 != 0;
        let unique1 = program.flags & 0x100 != 0;
        for stage in 0..MAX_STAGES {
            let offset = 4 * stage as u32;
            if let Some(word) = get(0xA60 + offset).filter(|_| unique0 || stage == 0) {
                program.stages[stage].constant0 = argb(word);
            }
            if let Some(word) = get(0xA80 + offset).filter(|_| unique1 || stage == 0) {
                program.stages[stage].constant1 = argb(word);
            }
        }
        if !unique0 {
            let first = program.stages[0].constant0;
            program
                .stages
                .iter_mut()
                .for_each(|stage| stage.constant0 = first);
        }
        if !unique1 {
            let first = program.stages[0].constant1;
            program
                .stages
                .iter_mut()
                .for_each(|stage| stage.constant1 = first);
        }
        if let Some(word) = get(0x1E20) {
            program.final_constant0 = argb(word);
        }
        if let Some(word) = get(0x1E24) {
            program.final_constant1 = argb(word);
        }
        Some(program)
    }

    /// The library's fog state as the stream last pushed it (`lib` 0x2A4 enable, 0x29C mode, 0x9C0 and 0x9C4 parameters), `None`
    /// when fog is off, never pushed, or switched off with the `no_fog` knob.
    fn fog_state(&self) -> Option<FogState> {
        if self.knob("no_fog") || self.lib_regs.get(&0x2A4) != Some(&1) {
            return None;
        }
        let mode = match self.lib_regs.get(&0x29C)? {
            0x2601 => 0,
            0x800 => 1,
            0x801 => 2,
            0x804 => 3,
            0x802 => 4,
            0x803 => 5,
            _ => return None,
        };
        Some(FogState {
            mode,
            parameter0: f32::from_bits(*self.lib_regs.get(&0x9C0)?),
            parameter1: f32::from_bits(*self.lib_regs.get(&0x9C4)?),
        })
    }

    /// The fog colour (`lib` 0x2A8, ABGR) as the combiner's fog register, alpha 0.
    fn fog_colour(&self) -> [f32; 4] {
        match self.lib_regs.get(&0x2A8) {
            Some(word) if !self.knob("no_fog") => [
                (word & 0xFF) as f32 / 255.0,
                ((word >> 8) & 0xFF) as f32 / 255.0,
                ((word >> 16) & 0xFF) as f32 / 255.0,
                0.0,
            ],
            _ => [0.0; 4],
        }
    }

    /// Run the bound vertex program over the distinct vertices of a draw. Returns the clip space vertices (one per distinct
    /// id) and the triangle list indices into them; triangles with a vertex that has no valid clip position are dropped.
    fn shade_vertices(
        &mut self,
        base: u32,
        stride: u32,
        vertex_ids: &[u32],
        order: &[u32],
        group: usize,
    ) -> (Vec<Vertex>, Vec<u32>) {
        let cpu_clip = self.knob("cpu_clip");
        let start = self.vsh_start.unwrap_or(0);
        let stage_ids = [
            self.texture_id,
            self.extra_textures[0],
            self.extra_textures[1],
            self.extra_textures[2],
        ];
        let linear_sizes: Vec<Option<(f32, f32)>> = stage_ids
            .iter()
            .map(|id| {
                self.textures
                    .get(id)
                    .and_then(|image| {
                        image
                            .linear
                            .then_some((image.width as f32, image.height as f32))
                    })
                    .or_else(|| {
                        self.fb_surfaces
                            .get(id)
                            .filter(|(_, _, linear)| *linear)
                            .map(|&(width, height, _)| (width as f32, height as f32))
                    })
            })
            .collect();
        let normalise = |sizes: &[Option<(f32, f32)>], stage: usize, c: [f32; 4]| match sizes[stage]
        {
            Some((width, height)) => [c[0] / width, c[1] / height],
            None => [c[0], c[1]],
        };
        let fog = self.fog_state();
        let program = vsh::decode_from(&self.program_memory, start);
        // Input registers the program reads: only those are fetched from guest memory (a vertex fetch is several page lookups).
        // Debug dumps of the unread inputs show their default value.
        let mut used_inputs = 0u32;
        for inst in &program {
            for source in [inst.a, inst.b, inst.c] {
                if source.mux == 2 {
                    used_inputs |= 1 << inst.input;
                }
            }
        }
        if self.debug_vertices > 0
            || self.knob("input_report")
            || self.knobs.contains_key("vsh_trace_tex")
        {
            used_inputs = u32::MAX;
        }
        let mut local: HashMap<
            u32,
            u32,
            std::hash::BuildHasherDefault<tsfp_engine_render::mem::PageHasher>,
        > = HashMap::default();
        let mut shaded: Vec<Option<Vertex>> = Vec::new();
        let mut positions: Vec<u32> = Vec::with_capacity(vertex_ids.len());
        for &id in vertex_ids {
            if let Some(&slot) = local.get(&id) {
                positions.push(slot);
                continue;
            }
            let address = base.wrapping_add(id.wrapping_mul(stride));
            let mut inputs = [[0.0, 0.0, 0.0, 1.0]; INPUT_COUNT];
            let fetch_started = std::time::Instant::now();
            if self.arrays_active {
                for (register, &(offset, format)) in self.arrays.iter().enumerate() {
                    if used_inputs >> register & 1 == 0 {
                        continue;
                    }
                    inputs[register] = fetch_attribute(
                        &self.memory,
                        offset.wrapping_add(id.wrapping_mul(format >> 8)),
                        format & 0xFF,
                    );
                }
            } else {
                for (register, entry) in self.declaration.iter().enumerate() {
                    if used_inputs >> register & 1 == 0 {
                        continue;
                    }
                    inputs[register] = fetch_attribute(
                        &self.memory,
                        address.wrapping_add(entry.offset),
                        entry.format,
                    );
                }
            }
            self.timings.fetch_ns += fetch_started.elapsed().as_nanos() as u64;
            if self.knob("input_report") && local.is_empty() {
                eprintln!(
                    "input_report id {id} arrays_active {} v2 {:?} v3 {:?} v0 {:?} arrays2 {:x?} declaration2 {:?}",
                    self.arrays_active,
                    inputs[2],
                    inputs[3],
                    inputs[0],
                    self.arrays[2],
                    self.declaration.get(2).map(|e| (e.offset, e.format))
                );
            }
            let traced = self
                .knobs
                .get("vsh_trace_tex")
                .is_some_and(|tex| *tex == format!("{:x}", self.texture_id));
            let interp_started = std::time::Instant::now();
            let outputs = if traced {
                let mut lines = Vec::new();
                let outputs = vsh::run_traced(
                    &self.program_memory,
                    start,
                    &inputs,
                    &self.vsh_constants,
                    &mut lines,
                );
                eprintln!(
                    "vsh_trace id {id} inputs v0 {:?} v1 {:?} v2 {:?} v3 {:?}",
                    inputs[0], inputs[1], inputs[2], inputs[3]
                );
                for line in lines {
                    eprintln!("  {line}");
                }
                outputs
            } else {
                vsh::run_decoded(&program, start, &inputs, &self.vsh_constants)
            };
            self.timings.interp_ns += interp_started.elapsed().as_nanos() as u64;
            self.timings.interp_vertices += 1;
            self.report.vsh_vertices += 1;
            if self.debug_vertices > 0 {
                self.debug_vertices -= 1;
                eprintln!(
                    "vsh start {start} id {id} v0 {:?} v1 {:?} v2 {:?} v3 {:?} -> pos {:?} d0 {:?} t0 {:?}",
                    inputs[0],
                    inputs[1],
                    inputs[2],
                    inputs[3],
                    outputs.position(),
                    outputs.diffuse(),
                    outputs.texcoord(0)
                );
            }
            let clip_position = if self.knob("host_raster") {
                vsh::to_clip_host_raster(
                    outputs.position(),
                    (self.width as f32, self.height as f32),
                    16_777_215.0,
                )
            } else {
                vsh::to_clip_with(
                    outputs.position(),
                    (self.width as f32, self.height as f32),
                    &self.vsh_constants,
                    !(self.knob("no_depth_clamp") || cpu_clip),
                )
            };
            let vertex = clip_position
                .filter(|position| position[3] > 0.0 || !self.knob("drop_behind_eye"))
                .map(|position| {
                    let colour = outputs.diffuse().map(|value| value.clamp(0.0, 1.0));
                    Vertex {
                        position,
                        colour,
                        specular: outputs.specular().map(|value| value.clamp(0.0, 1.0)),
                        uv: normalise(&linear_sizes, 0, outputs.texcoord(0)),
                        uv_extra: [1, 2, 3]
                            .map(|stage| normalise(&linear_sizes, stage, outputs.texcoord(stage))),
                        fog: fog.map_or(0.0, |state| state.amount(outputs.registers[5][0])),
                    }
                });
            shaded.push(vertex);
            let slot = (shaded.len() - 1) as u32;
            local.insert(id, slot);
            positions.push(slot);
        }
        let mut indices = Vec::with_capacity(order.len());
        let mut extra: Vec<Vertex> = Vec::new();
        let shaded_count = shaded.len() as u32;
        for triangle in order.chunks_exact(group) {
            let slots: Vec<u32> = triangle.iter().map(|&p| positions[p as usize]).collect();
            if slots.iter().all(|&slot| shaded[slot as usize].is_some()) {
                if group == 2 {
                    indices.extend(slots);
                } else if cpu_clip {
                    let corners = [0, 1, 2].map(|k| shaded[slots[k] as usize].expect("checked"));
                    let polygon = clip_polygon(corners.to_vec());
                    if polygon.len() >= 3 {
                        let first = shaded_count + extra.len() as u32;
                        extra.extend_from_slice(&polygon);
                        for k in 1..polygon.len() as u32 - 1 {
                            indices.extend([first, first + k, first + k + 1]);
                        }
                    } else {
                        self.report.vsh_triangles_dropped += 1;
                    }
                } else {
                    indices.extend(slots);
                }
            } else {
                self.report.vsh_triangles_dropped += 1;
            }
        }
        // Invalid vertices keep a placeholder so the slot numbers stay valid.
        let filler = Vertex {
            position: [0.0, 0.0, 0.0, 1.0],
            colour: [0.0; 4],
            specular: [0.0; 4],
            uv: [0.0; 2],
            uv_extra: [[0.0; 2]; 3],
            fog: 0.0,
        };
        (
            shaded
                .into_iter()
                .map(|v| v.unwrap_or(filler))
                .chain(extra)
                .collect(),
            indices,
        )
    }

    /// `LoadVertexProgram`: copy the program the stream's memory image holds at `program` (u16 count at +2, 16 bytes per
    /// instruction from +4, the layout `d3d8_upload_vertex_program` reads) into program memory slot `address`.
    fn load_program(&mut self, program: u32, address: u32) {
        let count = u32::from(self.memory.read16(program.wrapping_add(2)));
        for index in 0..count {
            let slot = (address + index) as usize;
            if slot >= vsh::PROGRAM_SLOTS {
                break;
            }
            let at = program.wrapping_add(4 + index * 16);
            self.program_memory.slots[slot] = [
                self.memory.read32(at),
                self.memory.read32(at.wrapping_add(4)),
                self.memory.read32(at.wrapping_add(8)),
                self.memory.read32(at.wrapping_add(12)),
            ];
        }
        *self
            .report
            .vsh_programs
            .entry((address, count))
            .or_insert(0) += 1;
    }

    /// `SelectVertexDeclaration`: read the 16 entry block (+4 offset, +8 `size << 4 | type`) and start the program at `address`.
    fn select_declaration(&mut self, declaration: u32, address: u32) {
        for (index, entry) in self.declaration.iter_mut().enumerate() {
            let at = declaration.wrapping_add(index as u32 * 0x10);
            entry.offset = self.memory.read32(at.wrapping_add(4));
            entry.format = self.memory.read32(at.wrapping_add(8));
        }
        self.vsh_start = Some(address as usize);
    }

    /// Decode the texture record `record` (D3D pixel container: data +4, format +12, size +16) from the replay image into
    /// `textures` keyed by the record address. Redecodes only when the header words or any texel snapshot changed.
    /// True when texel memory in `address..address + bytes` was written after `epoch` (or any wrapping write happened).
    fn texels_written_since(&self, address: u32, bytes: u32, epoch: u64) -> bool {
        if self.texel_epoch == epoch {
            return false;
        }
        if self.texel_pages.get(&u32::MAX).is_some_and(|&e| e > epoch) {
            return true;
        }
        if bytes == 0 {
            return false;
        }
        let last = address.wrapping_add(bytes - 1);
        if last < address {
            return true;
        }
        ((address >> 12)..=(last >> 12))
            .any(|page| self.texel_pages.get(&page).is_some_and(|&e| e > epoch))
    }

    fn decode_texture(&mut self, record: u32) {
        let started = std::time::Instant::now();
        if self.decode_texture_inner(record) {
            self.timings.decodes += 1;
            self.timings.decode_ns += started.elapsed().as_nanos() as u64;
        }
    }

    fn decode_texture_inner(&mut self, record: u32) -> bool {
        if record == 0 {
            return false;
        }
        // Only the data, format and size words decide the image; words 0 (refcount) and 2 (lock) change without touching it
        // (MEASURED: the full header differed on 69.7k of 69.9k decodes of identical texels).
        let header = [
            0,
            self.memory.read32(record.wrapping_add(4)),
            0,
            self.memory.read32(record.wrapping_add(12)),
            self.memory.read32(record.wrapping_add(16)),
        ];
        let previous = self.texture_versions.get(&record).copied();
        if let Some((epoch, seen, address, bytes, _)) = previous {
            if seen == header && !self.texels_written_since(address, bytes, epoch) {
                return false;
            }
        }
        let (data, format, size) = (header[1], header[3], header[4]);
        let code = (format >> 8) & 0xFF;
        let Some((surface_width, surface_height, bytes)) = texture::base_level(format, size) else {
            self.texture_versions
                .insert(record, (self.texel_epoch, header, 0, 0, 0));
            *self.report.texture_failed.entry(code).or_insert(0) += 1;
            return false;
        };
        let address = texture::data_address(data);
        let mut texels = vec![0u8; bytes as usize];
        self.memory.read_bytes(address, &mut texels);
        // A write hit this texture's pages (4 KiB granularity, neighbours share pages) but the texels may be the same: the content
        // hash decides, a hash is a lot cheaper than the decode (T2136f).
        let hash = hash_bytes(&texels);
        self.texture_versions
            .insert(record, (self.texel_epoch, header, address, bytes, hash));
        if previous.is_some_and(|(_, seen, old_address, old_bytes, old_hash)| {
            seen == header && (old_address, old_bytes, old_hash) == (address, bytes, hash)
        }) {
            return false;
        }
        match previous {
            None => self.timings.decode_why[0] += 1,
            Some((_, seen, ..)) if seen != header => self.timings.decode_why[1] += 1,
            Some(_) => self.timings.decode_why[2] += 1,
        }
        if !self.seen_texel_hashes.insert((header[3], header[4], hash)) {
            self.timings.decode_repeat += 1;
        }
        match texture::decode(format, size, &texels) {
            Some(decoded) if texels.iter().any(|&b| b != 0) => {
                *self.report.texture_formats.entry(code).or_insert(0) += 1;
                self.empty_textures.remove(&record);
                if let Some(directory) = self.knobs.get("dump_textures") {
                    let name = format!("{directory}/tex_{record:08x}_f{code:02x}.png");
                    let png = crate::png::encode_rgba(decoded.width, decoded.height, &decoded.rgba);
                    let _ = std::fs::write(name, png);
                }
                let image = TextureImage {
                    width: decoded.width,
                    height: decoded.height,
                    rgba: decoded.rgba,
                    linear: texture::is_linear(format),
                    mips: ((format >> 16) & 0xF).max(1),
                };
                // Every write to texture memory re-decodes the bound textures; an unchanged image keeps its generation so the
                // GPU view built from it stays valid (T2136c).
                let unchanged = self.textures.get(&record).is_some_and(|old| {
                    (old.width, old.height, old.linear, old.mips)
                        == (image.width, image.height, image.linear, image.mips)
                        && old.rgba == image.rgba
                });
                if !unchanged {
                    *self.texture_generation.entry(record).or_insert(0) += 1;
                    self.textures.insert(record, image);
                }
            }
            _ => {
                // 0x100: the stream holds no texel data at the data word (never snapshotted).
                *self.report.texture_failed.entry(0x100).or_insert(0) += 1;
                // An image that arrived as a `Texture` event (synthetic scenes) or an earlier decode stays bound, only a record
                // with no image at all is a frame buffer surface candidate.
                if !self.textures.contains_key(&record) {
                    self.empty_textures.insert(record);
                    self.fb_surfaces.insert(
                        record,
                        (surface_width, surface_height, texture::is_linear(format)),
                    );
                }
            }
        }
        true
    }

    /// A pushed clear with the colour write mask applied, by the rules of [`crate::clear_policy`].
    fn clear_with_mask(&self, clear: ClearRequest) -> ClearRequest {
        let size = self.surfaces.get(&self.current_surface);
        let back_buffer = size.is_none_or(|&size| size == (self.width, self.height));
        let drawn = self
            .ops
            .iter()
            .any(|op| matches!(op, Op::Draw(draw) if draw.target == self.current_surface));
        let knob = |name: &str| self.knob(name);
        clear_policy::decide(clear, back_buffer, drawn, knob)
    }

    fn pipeline(&mut self, key: PipelineKey) -> &wgpu::RenderPipeline {
        if !self.pipelines.contains_key(&key) {
            let started = std::time::Instant::now();
            let pipeline =
                build_pipeline(&self.gpu.device, &self.module, &self.pipeline_layout, key);
            self.timings
                .note_pipeline(started.elapsed().as_nanos() as u64);
            self.pipelines.insert(key, pipeline);
        }
        &self.pipelines[&key]
    }

    /// Build pipelines on `threads` worker threads from now on (0 turns it off again). A draw whose pipeline is still being built
    /// is skipped (counted in [`PhaseTimings::draws_deferred`]), the frame goes out without it. Off by default: the offline replay
    /// and the golden hashes need every draw.
    pub fn set_async_pipelines(&mut self, threads: usize) {
        self.async_pipelines = (threads > 0).then(|| {
            AsyncPipelines::new(
                self.gpu.device.clone(),
                self.module.clone(),
                self.pipeline_layout.clone(),
                threads,
            )
        });
    }

    /// Loading-boundary request using the complete immutable raster state. Default off;
    /// requires `prewarm_pipelines` and async workers. True means ready, not merely queued.
    /// Does not draw, change selected state, or touch surfaces. No disk cache: keys are
    /// device-local and shader/layout-specific. Producer must provide a captured state.
    pub fn prewarm_pipeline(&mut self, state: &PipelineState, lines: bool) -> bool {
        if !self.knob("prewarm_pipelines") || self.async_pipelines.is_none() {
            return false;
        }
        let key = PipelineKey::of(
            state,
            !self.knob("no_colour_mask"),
            lines,
            self.knob("polygon_offset"),
        );
        self.pipeline_ready(key)
    }

    /// Nonblocking completion observation. Includes polling delay; never GPU execution time.
    pub fn poll_gpu(&mut self) {
        self.gpu.device.poll(wgpu::Maintain::Poll);
        self.timings.gpu_latency_ns = self.latency.0.load(Ordering::SeqCst);
        self.timings.gpu_latency_max_ns = self.latency.1.load(Ordering::SeqCst);
        self.timings.gpu_latency_frames = self.latency.2.load(Ordering::SeqCst);
    }

    /// Outstanding callback observations, not a GPU timestamp query.
    pub fn gpu_work_pending(&self) -> bool {
        self.in_flight.load(Ordering::SeqCst) != 0
    }

    fn track_submission(&mut self) {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        crate::frame_layer::submitted(&self.gpu.queue);
        let counter = self.in_flight.clone();
        let latency = self.latency.clone();
        let submitted = std::time::Instant::now();
        self.gpu.queue.on_submitted_work_done(move || {
            let ns = submitted.elapsed().as_nanos() as u64;
            latency.0.fetch_add(ns, Ordering::SeqCst);
            latency.1.fetch_max(ns, Ordering::SeqCst);
            latency.2.fetch_add(1, Ordering::SeqCst);
            counter.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// True when the pipeline for `key` can be used now. With async pipelines a missing one is requested and `false` returned.
    fn pipeline_ready(&mut self, key: PipelineKey) -> bool {
        let Some(workers) = &mut self.async_pipelines else {
            self.pipeline(key);
            return true;
        };
        for (finished, pipeline, nanoseconds) in workers.finished() {
            self.timings.note_pipeline(nanoseconds);
            self.pipelines.insert(finished, pipeline);
        }
        if self.pipelines.contains_key(&key) {
            return true;
        }
        workers.request(key);
        false
    }

    fn record_sample(&mut self, before: &PhaseTimings) {
        if self.samples.len() < MAX_FRAME_SAMPLES {
            self.samples
                .push(FrameSample::between(before, &self.timings));
        }
    }

    /// A stage with no texture bound: legacy streams keep white (the modulate stand-in of the HUD draws), streams with
    /// `SetSurface` events sample transparent black (MEASURED better on the loading screens, slice 5).
    fn null_black(&self) -> bool {
        self.knob("null_texture_black")
            || (!self.surfaces.is_empty() && !self.knob("null_texture_white"))
    }

    fn upload_texture(
        &self,
        image: Option<&TextureImage>,
        batch: Option<&mut TextureUploads>,
    ) -> wgpu::TextureView {
        let black = self.null_black();
        let white = if black { [0u8; 4] } else { [255u8; 4] };
        let (width, height, data): (u32, u32, &[u8]) = match image {
            Some(image) => (image.width, image.height, &image.rgba),
            None => (1, 1, &white),
        };
        // Slice 8: a box filtered mip chain made from the base level (the stream only carries level 0), for the sampler's LOD. Linear
        // (rectangle) textures have no mips on the NV2A.
        let declared = image.map_or(1, |image| image.mips);
        let with_mips = image.is_some_and(|image| !image.linear && image.mips > 1)
            && !self.knob("no_mips")
            || self.knob("all_mips") && image.is_some_and(|image| !image.linear);
        let (mip_level_count, chain) = if with_mips {
            mip_chain(
                width,
                height,
                data,
                if self.knob("all_mips") {
                    u32::MAX
                } else {
                    declared
                },
            )
        } else {
            (1, data.to_vec())
        };
        let descriptor = wgpu::TextureDescriptor {
            label: Some("tsfp-stage0"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOUR_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        // Padding can dominate narrow images. Bound staging memory; overflow uses the control path.
        let padded_bytes: u64 = (0..mip_level_count)
            .map(|level| {
                u64::from(((width >> level).max(1) * 4).div_ceil(256) * 256)
                    * u64::from((height >> level).max(1))
            })
            .sum();
        let texture = if let Some(batch) =
            batch.filter(|batch| batch.bytes.len() as u64 + padded_bytes <= 64 * 1024 * 1024)
        {
            let texture = self.gpu.device.create_texture(&descriptor);
            batch.append(&texture, width, height, mip_level_count, &chain);
            texture
        } else {
            self.gpu.device.create_texture_with_data(
                &self.gpu.queue,
                &descriptor,
                wgpu::util::TextureDataOrder::LayerMajor,
                &chain,
            )
        };
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// The (u, v) address mode codes of a stage's sampler: NV2A 1 wrap, 2 mirror, 3 clamp (and the unmodelled border and
    /// clamp to edge variants, all clamped). Linear (rectangle) textures and surfaces clamp. Without `lib` address words the
    /// slice 4 inference applies (repeat for swizzled textures when `wrap_textures` is on).
    fn address_modes(&self, draw: &DrawRecord, stage: usize, id: u32) -> (u8, u8) {
        let linear = self.textures.get(&id).is_some_and(|image| image.linear)
            || draw.surface_stage[stage].is_some();
        if linear {
            return (3, 3);
        }
        match draw.address[stage] {
            Some(word) => (
                ((word & 0xF) as u8).max(1),
                (((word >> 8) & 0xF) as u8).max(1),
            ),
            None if self.wrap_textures => (1, 1),
            None => (3, 3),
        }
    }

    /// Encode the frame's ops, read the image back and append it to [`Replayer::frames`].
    /// T2294: in a suppressed (skip-draw) present, omit draws into a back buffer (a surface an earlier frame displayed) that is
    /// never read back. A target counts as read once any draw samples it (surface stage, feedback copy or frame buffer record),
    /// and it stays read for the rest of the run, so feedback surfaces keep every draw; render-to-texture surfaces that were
    /// never displayed are never pruned, even before their first sampling. Pruning also needs the last `CLEAR_STREAK` frames to start
    /// that surface with a full colour clear (INFERRED: then a shown frame does not depend on the content of the skipped one). Clears and target switches
    /// are kept. Knob `no_skip_unread` turns it off.
    fn prune_unread_draws(&mut self, ops: &mut Vec<Op>) {
        let mut legacy_read = false;
        for op in ops.iter() {
            if let Op::Draw(draw) = op {
                self.ever_sampled
                    .extend(draw.surface_stage.iter().flatten());
                if draw.fb_mask != 0 {
                    self.ever_sampled.insert(draw.target);
                    legacy_read = true;
                }
            }
        }
        if legacy_read {
            self.ever_sampled.insert(LEGACY_SURFACE);
        }
        // Per surface: did its first op of this frame clear every colour channel?
        let mut first: HashMap<u32, bool> = HashMap::new();
        let mut target = self.frame_start_surface;
        for op in ops.iter() {
            match op {
                Op::Target(key) => target = *key,
                Op::Clear(clear) => {
                    first.entry(target).or_insert(
                        clear.colour.is_some() && clear.channels == 0xF && clear.write_mask == 0xF,
                    );
                }
                Op::ViewLayer(key) => {
                    first.entry(*key).or_insert(false);
                }
                Op::Draw(draw) => {
                    first.entry(draw.target).or_insert(false);
                }
            }
        }
        for (key, cleared) in first {
            let streak = self.clear_streak.entry(key).or_insert(0);
            *streak = if cleared { streak.saturating_add(1) } else { 0 };
        }
        if !self.suppress_presentation || self.knob("no_skip_unread") {
            return;
        }
        let before = ops.len();
        let (sampled, displayed, streaks) =
            (&self.ever_sampled, &self.ever_displayed, &self.clear_streak);
        ops.retain(|op| match op {
            Op::Draw(draw) => {
                !displayed.contains(&draw.target)
                    || sampled.contains(&draw.target)
                    || streaks
                        .get(&draw.target)
                        .is_none_or(|streak| *streak < CLEAR_STREAK)
            }
            _ => true,
        });
        self.unread_draws_skipped += (before - ops.len()) as u64;
    }

    pub fn present(&mut self) {
        self.flush_draw();
        if !self.rendering {
            self.frame_start_surface = self.current_surface;
            self.ops.clear();
            self.vertices.clear();
            self.indices.clear();
            if let Some(gpu) = &mut self.gpu_vsh {
                gpu.clear_frame();
            }
            return;
        }
        let (width, height) = (self.width, self.height);
        let (out_width, out_height) = (self.out_width, self.out_height);
        let frame_started = std::time::Instant::now();
        let phase_base = self.timings.phases();
        let timings_before = self.timings;
        let mut mark;
        let mut ops = std::mem::take(&mut self.ops);
        // INFERRED frame boundary fix: the host marks a present after the engine already queued the NEXT frame's first full
        // screen overwrite quad (MEASURED: present 585 of one capture ends with one, colour 0, blend One/Zero, which wipes the
        // finished scene). That last quad is applied as a clear at the start of the next frame instead.
        if let Some(colour) = self.carry_clear.take() {
            ops.insert(
                0,
                Op::Clear(ClearRequest {
                    colour: Some(colour),
                    depth: true,
                    channels: 0xF,
                    write_mask: 0xF,
                }),
            );
        }
        if let Some(Op::Draw(draw)) = ops.last() {
            if let Some(colour) = draw.clear_colour.filter(|_| !draw.grade) {
                self.carry_clear = Some(colour);
                ops.pop();
            }
        }
        self.prune_unread_draws(&mut ops);
        let vertices = std::mem::take(&mut self.vertices);
        let indices = std::mem::take(&mut self.indices);
        let device = self.gpu.device.clone();

        // Surfaces: one per Data word the frame touches (render to texture, kept across frames), or the single per frame
        // target of streams without `Surface` events.
        let surface_mode = !self.surfaces.is_empty();
        let start_key = if surface_mode {
            self.frame_start_surface
        } else {
            LEGACY_SURFACE
        };
        let shown: Option<u32> = self
            .knobs
            .get("show_surface")
            .and_then(|value| value.parse().ok())
            .filter(|key| self.surfaces.contains_key(key));
        let graded = ops.iter().rev().find_map(|op| match op {
            Op::Draw(draw) if draw.grade => Some(draw.target),
            _ => None,
        });
        let display_key = if !surface_mode {
            LEGACY_SURFACE
        } else if let Some(key) = shown {
            // Debug: present this surface (Data word, decimal) instead of the back buffer.
            key
        } else if let Some(target) = graded {
            target
        } else if self.surfaces.contains_key(&self.display_surface) {
            self.display_surface
        } else {
            start_key
        };
        self.ever_displayed.insert(display_key);
        if !surface_mode {
            self.surface_gpu.remove(&LEGACY_SURFACE);
        }
        let mut used = vec![start_key, display_key];
        for op in &ops {
            match op {
                Op::Target(key) | Op::ViewLayer(key) => used.push(*key),
                Op::Draw(draw) => {
                    used.push(draw.target);
                    used.extend(draw.surface_stage.iter().flatten());
                }
                Op::Clear(_) => {}
            }
        }
        let mut fresh: HashSet<u32> = HashSet::new();
        for key in used {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.surface_gpu.entry(key) {
                let surface = make_surface_gpu(
                    &self.gpu.device,
                    self.out_width,
                    self.out_height,
                    &self.surface_counter,
                    "tsfp-target",
                );
                entry.insert(surface);
                fresh.insert(key);
            }
        }
        let gpu_jobs = self.gpu_vsh.as_ref().is_some_and(|gpu| gpu.has_jobs());
        let vertex_buffer = (!vertices.is_empty()).then(|| {
            persistent_buffer(
                &device,
                &self.gpu.queue,
                &mut self.vertex_buffer,
                "tsfp-vertices",
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
                &vertices,
            )
        });
        let index_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let index_buffer = (!index_bytes.is_empty()).then(|| {
            persistent_buffer(
                &device,
                &self.gpu.queue,
                &mut self.index_buffer,
                "tsfp-indices",
                wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
                &index_bytes,
            )
        });

        // Per draw resources are created up front so the pass can borrow them.
        let mut texture_batch = self.knob("batch_textures").then(TextureUploads::default);
        let mut views: HashMap<u32, wgpu::TextureView> = HashMap::new();
        mark = std::time::Instant::now();
        self.prepare_texture_views(&ops, &mut views, texture_batch.as_mut());
        let texture_ns = mark.elapsed().as_nanos() as u64;
        self.timings.textures_ns += texture_ns;
        self.timings.textures_frame_max_ns = self.timings.textures_frame_max_ns.max(texture_ns);
        mark = std::time::Instant::now();
        // Samplers by (nearest, u mode, v mode).
        let mut samplers = std::mem::take(&mut self.sampler_cache);
        let address = |mode: u8| match mode {
            1 => wgpu::AddressMode::Repeat,
            2 => wgpu::AddressMode::MirrorRepeat,
            _ => wgpu::AddressMode::ClampToEdge,
        };
        for op in &ops {
            if let Op::Draw(draw) = op {
                let ids = draw.stage_ids();
                let nearest = [
                    draw.nearest,
                    draw.extra_nearest[0],
                    draw.extra_nearest[1],
                    draw.extra_nearest[2],
                ];
                for stage in 0..4 {
                    let (u, v) = self.address_modes(draw, stage, ids[stage]);
                    samplers.entry((nearest[stage], u, v)).or_insert_with(|| {
                        let filter = if nearest[stage] {
                            wgpu::FilterMode::Nearest
                        } else {
                            wgpu::FilterMode::Linear
                        };
                        device.create_sampler(&wgpu::SamplerDescriptor {
                            mag_filter: filter,
                            min_filter: filter,
                            mipmap_filter: filter,
                            address_mode_u: address(u),
                            address_mode_v: address(v),
                            ..Default::default()
                        })
                    });
                }
            }
        }
        // Stages that sample the surface the draw renders into (a feedback loop, or the legacy frame buffer record): the target
        // is copied to a texture just before the draw. The owner draw copies, with `fb_snapshot` later draws of the frame may
        // reuse an earlier copy.
        let copy_masks: Vec<(u32, u32)> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Draw(draw) => {
                    let ids = draw.stage_ids();
                    let mut mask = draw.fb_mask;
                    let mut record = if draw.fb_mask == 0 {
                        0
                    } else {
                        ids[draw.fb_mask.trailing_zeros() as usize]
                    };
                    for stage in 0..4 {
                        if draw.surface_stage[stage] == Some(draw.target) {
                            mask |= 1 << stage;
                            record = draw.target;
                        }
                    }
                    Some((mask, record))
                }
                _ => None,
            })
            .collect();
        let mut first_owner: Option<usize> = None;
        let mut record_owner: HashMap<u32, usize> = HashMap::new();
        let snapshot_owner: Vec<Option<usize>> = copy_masks
            .iter()
            .enumerate()
            .map(|(index, &(mask, record))| {
                if mask == 0 {
                    None
                } else {
                    Some(match self.fb_snapshot {
                        FbSnapshot::PerDraw => index,
                        FbSnapshot::Once => *first_owner.get_or_insert(index),
                        FbSnapshot::PerRecord => *record_owner.entry(record).or_insert(index),
                    })
                }
            })
            .collect();
        let snapshots: Vec<Option<(wgpu::Texture, wgpu::TextureView)>> = snapshot_owner
            .iter()
            .enumerate()
            .map(|(index, owner)| {
                (*owner == Some(index)).then(|| {
                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("tsfp-fb-copy"),
                        size: wgpu::Extent3d {
                            width: out_width,
                            height: out_height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: COLOUR_FORMAT,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    let view = texture.create_view(&Default::default());
                    (texture, view)
                })
            })
            .collect();
        let honour_mask = !self.knob("no_colour_mask");
        let draw_total = ops.iter().filter(|op| matches!(op, Op::Draw(_))).count();
        // One persistent parameter buffer for all draws of the frame, one write, dynamic offsets (T2136f).
        let needed = (draw_total * PARAM_STRIDE).max(PARAM_STRIDE) as u64;
        if self
            .param_buffer
            .as_ref()
            .is_none_or(|(_, size)| *size < needed)
        {
            let size = needed.next_power_of_two().max(1 << 16);
            self.param_buffer = Some((
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("tsfp-params"),
                    size,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                size,
            ));
            self.param_generation += 1;
            self.bind_cache.clear();
        }
        let param_buffer = self.param_buffer.as_ref().expect("param buffer").0.clone();
        let mut param_block: Vec<u8> = Vec::with_capacity(draw_total * PARAM_STRIDE);
        let mut bind_groups = Vec::new();
        let mut keys = Vec::new();
        for op in &ops {
            if let Op::Draw(draw) = op {
                let params = self.draw_uniform_bytes(draw);
                param_block.extend_from_slice(&params);
                let ids = draw.stage_ids();
                let nearest = [
                    draw.nearest,
                    draw.extra_nearest[0],
                    draw.extra_nearest[1],
                    draw.extra_nearest[2],
                ];
                let draw_index = bind_groups.len();
                let copy_mask = copy_masks[draw_index].0;
                // The cache key of this draw's bind group; a draw that reads a per frame snapshot copy builds its own.
                let mut cacheable = true;
                let mut stages = [(StageBinding::Null(false), (false, 0u8, 0u8)); 4];
                for stage in 0..4usize {
                    let (u, v) = self.address_modes(draw, stage, ids[stage]);
                    let snapshot = snapshot_owner[draw_index]
                        .and_then(|owner| snapshots[owner].as_ref())
                        .filter(|_| copy_mask >> stage & 1 != 0);
                    let surface =
                        draw.surface_stage[stage].and_then(|key| self.surface_gpu.get(&key));
                    let binding = match (snapshot, surface) {
                        (Some(_), _) => {
                            cacheable = false;
                            StageBinding::Null(false)
                        }
                        (None, Some(surface)) => StageBinding::Surface(surface.id),
                        (None, None) if self.textures.contains_key(&ids[stage]) => {
                            StageBinding::Texture(
                                ids[stage],
                                self.texture_generation
                                    .get(&ids[stage])
                                    .copied()
                                    .unwrap_or(0),
                            )
                        }
                        (None, None) => StageBinding::Null(self.null_black()),
                    };
                    stages[stage] = (binding, (nearest[stage], u, v));
                }
                let key = BindKey {
                    params: self.param_generation,
                    stages,
                };
                let group = match self.bind_cache.get(&key).filter(|_| cacheable) {
                    Some(group) => {
                        self.timings.bind_cache_hits += 1;
                        group.clone()
                    }
                    None => {
                        let mut entries = vec![wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &param_buffer,
                                offset: 0,
                                size: wgpu::BufferSize::new(PARAM_STRIDE as u64),
                            }),
                        }];
                        for stage in 0..4usize {
                            let snapshot = snapshot_owner[draw_index]
                                .and_then(|owner| snapshots[owner].as_ref())
                                .filter(|_| copy_mask >> stage & 1 != 0);
                            let surface = draw.surface_stage[stage]
                                .and_then(|key| self.surface_gpu.get(&key))
                                .map(|surface| &surface.view);
                            entries.push(wgpu::BindGroupEntry {
                                binding: 1 + 2 * stage as u32,
                                resource: wgpu::BindingResource::TextureView(
                                    match (snapshot, surface) {
                                        (Some((_, view)), _) => view,
                                        (None, Some(view)) => view,
                                        (None, None) => &views[&ids[stage]],
                                    },
                                ),
                            });
                            let (u, v) = self.address_modes(draw, stage, ids[stage]);
                            entries.push(wgpu::BindGroupEntry {
                                binding: 2 + 2 * stage as u32,
                                resource: wgpu::BindingResource::Sampler(
                                    &samplers[&(nearest[stage], u, v)],
                                ),
                            });
                        }
                        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("tsfp-draw"),
                            layout: &self.layout,
                            entries: &entries,
                        });
                        self.timings.bind_groups_created += 1;
                        if cacheable {
                            if self.bind_cache.len() >= 4096 {
                                self.bind_cache.clear();
                            }
                            self.bind_cache.insert(key, group.clone());
                        }
                        group
                    }
                };
                bind_groups.push(group);
                keys.push(PipelineKey::of(
                    &draw.state,
                    honour_mask,
                    draw.lines,
                    self.knob("polygon_offset"),
                ));
            }
        }
        let ready: Vec<bool> = keys.iter().map(|key| self.pipeline_ready(*key)).collect();
        self.timings.draws_deferred += ready.iter().filter(|ready| !**ready).count() as u64;
        if !param_block.is_empty() {
            self.gpu.queue.write_buffer(&param_buffer, 0, &param_block);
        }
        self.sampler_cache = samplers;

        let clear_groups = self.prepare_clear_groups(&device, &ops);

        self.timings.bind_groups_ns += mark.elapsed().as_nanos() as u64;
        self.timings.draws += bind_groups.len() as u64;
        mark = std::time::Instant::now();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tsfp-frame"),
        });
        if let Some(batch) = &texture_batch {
            batch.encode(&device, &mut encoder);
            if !batch.bytes.is_empty() {
                self.timings.texture_upload_batches += 1;
                self.timings.texture_staging_bytes += batch.bytes.len() as u64;
            }
        }
        if gpu_jobs {
            if let (Some(gpu), Some(vertex_buffer), Some(index_buffer)) =
                (&mut self.gpu_vsh, &vertex_buffer, &index_buffer)
            {
                gpu.encode(&mut encoder, vertex_buffer, index_buffer);
            }
        }
        let use_native_grade = !self.knob("replay_grade");
        if use_native_grade && self.native_grade.is_none() {
            self.native_grade = Some(crate::grade::NativeGrade::new(&device, COLOUR_FORMAT));
        }
        let mut frame_grade: Option<crate::grade::GradeModel> = None;
        let (mut native_grades, mut replayed_grades) = (0u64, 0u64);
        {
            let surfaces = &self.surface_gpu;
            let begin = |encoder: &mut wgpu::CommandEncoder,
                         key: u32,
                         clear: Option<ClearRequest>,
                         fresh_target: bool| {
                let colour_load = match clear.and_then(|c| c.colour) {
                    Some(argb) => {
                        let [r, g, b, a] = crate::nv2a::argb_to_rgba(argb).map(f64::from);
                        wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a })
                    }
                    None if fresh_target => wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    None => wgpu::LoadOp::Load,
                };
                let depth_load = if fresh_target || clear.is_some_and(|c| c.depth) {
                    wgpu::LoadOp::Clear(1.0)
                } else {
                    wgpu::LoadOp::Load
                };
                let surface = &surfaces[&key];
                encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("tsfp-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &surface.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: colour_load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &surface.depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: depth_load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    })
                    .forget_lifetime()
            };
            let mut target = start_key;
            let mut draw_number = 0usize;
            let mut current: Option<wgpu::RenderPass<'static>> = None;
            for (op_index, op) in ops.iter().enumerate() {
                match op {
                    Op::ViewLayer(key) => {
                        if !self.suppress_presentation && crate::frame_layer::installed() {
                            drop(current.take());
                            if fresh.remove(key) {
                                drop(begin(&mut encoder, *key, None, true));
                            }
                            crate::frame_layer::encode_view_pass(crate::frame_layer::LayerTarget {
                                device: &device,
                                queue: &self.gpu.queue,
                                encoder: &mut encoder,
                                target: &surfaces[key].texture,
                                frame: self.layer_frames,
                                present: self.readback_present,
                            });
                        }
                    }
                    Op::Target(key) => {
                        drop(current.take());
                        target = *key;
                    }
                    Op::Clear(clear) => {
                        drop(current.take());
                        if let Some(group) = &clear_groups[op_index] {
                            // masked colour clear: the depth part (if any) clears with the pass, the colour part is a masked draw
                            let depth_only = ClearRequest {
                                colour: None,
                                ..*clear
                            };
                            let mut pass = begin(
                                &mut encoder,
                                target,
                                Some(depth_only),
                                fresh.remove(&target),
                            );
                            pass.set_pipeline(&self.clear_pipelines[&clear.channels]);
                            pass.set_bind_group(0, group, &[]);
                            pass.draw(0..3, 0..1);
                            current = Some(pass);
                        } else {
                            current = Some(begin(
                                &mut encoder,
                                target,
                                Some(*clear),
                                fresh.remove(&target),
                            ));
                        }
                    }
                    Op::Draw(draw) => {
                        if scissor_rect(draw.scissor, width, height).is_none() {
                            draw_number += 1;
                            continue;
                        }
                        if !ready[draw_number] {
                            draw_number += 1; // pipeline still being built on a worker thread: this draw is skipped
                            continue;
                        }
                        if self.knob("trace_passes") && draw.target != start_key {
                            eprintln!(
                                "pass draw {draw_number} record target {:x} pass target {:x} start {:x} indices {}..+{} snapshot {} surf {:x?}",
                                draw.target,
                                target,
                                start_key,
                                draw.first_index,
                                draw.index_count,
                                snapshots[draw_number].is_some(),
                                draw.surface_stage
                            );
                        }
                        // T2336: in display-rate mode the clean grade source (before any view-pass layer) is kept for the
                        // shell, which redoes the grade per display update over display-rate view-pass draws.
                        let mut pregrade = None;
                        if draw.grade && use_native_grade && crate::frame_layer::display_rate() {
                            if let Some(source) =
                                draw.surface_stage[0].and_then(|key| self.surface_gpu.get(&key))
                            {
                                drop(current.take());
                                let copy = device.create_texture(&wgpu::TextureDescriptor {
                                    label: Some("tsfp-pregrade"),
                                    size: source.texture.size(),
                                    mip_level_count: 1,
                                    sample_count: 1,
                                    dimension: wgpu::TextureDimension::D2,
                                    format: source.texture.format(),
                                    usage: wgpu::TextureUsages::COPY_DST
                                        | wgpu::TextureUsages::COPY_SRC,
                                    view_formats: &[],
                                });
                                encoder.copy_texture_to_texture(
                                    source.texture.as_image_copy(),
                                    copy.as_image_copy(),
                                    copy.size(),
                                );
                                pregrade = Some(copy);
                            }
                        }
                        if draw.grade
                            && !self.suppress_presentation
                            && crate::frame_layer::installed()
                        {
                            // T2312: view-pass layer draws (the native weapon reticle) go into the grade's source
                            // surface before the grade samples it. Fallback only: explicit view boundaries run earlier.
                            if let Some(source) =
                                draw.surface_stage[0].and_then(|key| self.surface_gpu.get(&key))
                            {
                                drop(current.take());
                                crate::frame_layer::encode_before_grade(
                                    crate::frame_layer::LayerTarget {
                                        device: &device,
                                        queue: &self.gpu.queue,
                                        encoder: &mut encoder,
                                        target: &source.texture,
                                        frame: self.layer_frames,
                                        present: self.readback_present,
                                    },
                                );
                            }
                        }
                        // T2347: the closing grade is drawn by the native pass from its typed model when the combiner is the
                        // engine's grade; any other closing pass stays the replayed draw.
                        if draw.grade {
                            let model = draw
                                .pixel
                                .as_ref()
                                .and_then(crate::grade::GradeModel::from_program);
                            let source = draw.surface_stage[0].and_then(|key| surfaces.get(&key));
                            match (use_native_grade, model, source, &self.native_grade) {
                                (true, Some(model), Some(source), Some(native))
                                    if draw.surface_stage[0] != Some(target) =>
                                {
                                    drop(current.take());
                                    if fresh.remove(&target) {
                                        drop(begin(&mut encoder, target, None, true));
                                    }
                                    let (x, y, w, h) = scissor_rect(draw.scissor, width, height)
                                        .expect("draw scissor was checked above");
                                    let scissor = (
                                        x * out_width / width,
                                        y * out_height / height,
                                        w * out_width / width,
                                        h * out_height / height,
                                    );
                                    native.encode(
                                        &device,
                                        &mut encoder,
                                        &source.view,
                                        &surfaces[&target].view,
                                        &model,
                                        Some(scissor),
                                    );
                                    if let Some(pregrade) = pregrade.take() {
                                        crate::frame_layer::latch_display_grade(
                                            crate::frame_layer::DisplayGrade {
                                                device: device.clone(),
                                                present: self.readback_present,
                                                pregrade,
                                                model,
                                                scissor,
                                            },
                                        );
                                    }
                                    frame_grade = Some(model);
                                    native_grades += 1;
                                    draw_number += 1;
                                    continue;
                                }
                                _ => {
                                    frame_grade = model.or(frame_grade);
                                    replayed_grades += 1;
                                }
                            }
                        }
                        if let Some((snapshot, _)) = &snapshots[draw_number] {
                            // Render to texture, feedback: end the pass, copy the target, continue with Load ops.
                            drop(current.take());
                            if fresh.remove(&target) {
                                drop(begin(&mut encoder, target, None, true));
                            }
                            encoder.copy_texture_to_texture(
                                wgpu::TexelCopyTextureInfo {
                                    texture: &self.surface_gpu[&target].texture,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                wgpu::TexelCopyTextureInfo {
                                    texture: snapshot,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                wgpu::Extent3d {
                                    width: out_width,
                                    height: out_height,
                                    depth_or_array_layers: 1,
                                },
                            );
                        }
                        if current.is_none() {
                            current =
                                Some(begin(&mut encoder, target, None, fresh.remove(&target)));
                        }
                        let pass = current.as_mut().expect("pass open");
                        let key = keys[draw_number];
                        pass.set_pipeline(&self.pipelines[&key]);
                        pass.set_bind_group(
                            0,
                            &bind_groups[draw_number],
                            &[(draw_number * PARAM_STRIDE) as u32],
                        );
                        pass.set_vertex_buffer(
                            0,
                            vertex_buffer
                                .as_ref()
                                .expect("draws imply vertices")
                                .slice(..),
                        );
                        pass.set_index_buffer(
                            index_buffer
                                .as_ref()
                                .expect("draws imply indices")
                                .slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        let [r, g, b, a] =
                            crate::nv2a::argb_to_rgba(draw.state.blend_colour).map(f64::from);
                        pass.set_blend_constant(wgpu::Color { r, g, b, a });
                        let (x, y, w, h) = scissor_rect(draw.scissor, width, height)
                            .expect("draw scissor was checked above");
                        let (x, y, w, h) = (
                            x * out_width / width,
                            y * out_height / height,
                            w * out_width / width,
                            h * out_height / height,
                        );
                        pass.set_scissor_rect(x, y, w, h);
                        pass.draw_indexed(
                            draw.first_index..draw.first_index + draw.index_count,
                            0,
                            0..1,
                        );
                        draw_number += 1;
                    }
                }
            }
            drop(current.take());
            if fresh.remove(&display_key) {
                drop(begin(&mut encoder, display_key, None, true));
            }
        }
        if frame_grade.is_some() {
            self.last_grade = frame_grade;
        }
        self.native_grades += native_grades;
        self.replayed_grades += replayed_grades;
        self.timings.encode_ns += mark.elapsed().as_nanos() as u64;
        mark = std::time::Instant::now();
        let mut shown = self.surface_gpu[&display_key].texture.clone();
        let (mut out_width, mut out_height) = (out_width, out_height);
        let persistent_present = self.knob("persistent_present");
        if let Some((window_w, window_h)) = self.window_size {
            let device = &self.gpu.device;
            let presenter = self
                .presenter
                .get_or_insert_with(|| crate::present::Presenter::new(device, COLOUR_FORMAT));
            let target = self
                .window_target
                .get_or_insert_with(|| {
                    device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("tsfp-window"),
                        size: wgpu::Extent3d {
                            width: window_w,
                            height: window_h,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: COLOUR_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::COPY_SRC
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                })
                .clone();
            let present_started = std::time::Instant::now();
            let encode = if persistent_present {
                crate::present::Presenter::encode_cached
            } else {
                crate::present::Presenter::encode
            };
            encode(
                presenter,
                device,
                &mut encoder,
                &shown,
                &target,
                width as f32 / height as f32,
            );
            self.timings.present_encode_ns += present_started.elapsed().as_nanos() as u64;
            self.timings.present_bind_cache_hits = presenter.cache_hits.get();
            self.timings.present_bindings_created = presenter.bindings_created.get();
            shown = target;
            (out_width, out_height) = (window_w, window_h);
        }
        if !self.suppress_presentation && crate::frame_layer::installed() {
            shown = self.compose_frame_layer(&mut encoder, shown, self.window_size.is_some());
        }
        self.finish_present(
            &device,
            encoder,
            shown,
            (out_width, out_height),
            (frame_started, mark, phase_base),
            timings_before,
        );
    }
    /// T2309: let the installed frame layer draw over the finished frame, inside this frame's encoder (after the scene and
    /// post passes, before the present copy). A guest surface is copied to a private composite first (`own_target` false),
    /// so the guest never reads the layer's pixels back; the window present target is drawn on directly.
    fn compose_frame_layer(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        shown: wgpu::Texture,
        own_target: bool,
    ) -> wgpu::Texture {
        let device = self.gpu.device.clone();
        let target = if own_target {
            shown
        } else {
            let size = shown.size();
            let reuse = self
                .layer_composite
                .as_ref()
                .is_some_and(|texture| texture.size() == size);
            if !reuse {
                self.layer_composite = Some(device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("tsfp-frame-layer"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: COLOUR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
                }));
            }
            let composite = self.layer_composite.clone().expect("created above");
            encoder.copy_texture_to_texture(shown.as_image_copy(), composite.as_image_copy(), size);
            composite
        };
        let drew = crate::frame_layer::encode(crate::frame_layer::LayerTarget {
            device: &device,
            queue: &self.gpu.queue,
            encoder,
            target: &target,
            frame: self.layer_frames,
            present: self.readback_present,
        });
        self.layer_frames += 1;
        self.layer_drawn += u64::from(drew);
        target
    }
    /// Pack the fixed-stride uniform ABI in shader field order, including padding.
    fn draw_uniform_bytes(&self, draw: &DrawRecord) -> Vec<u8> {
        let mut params = Vec::with_capacity(PARAM_STRIDE);
        for value in [
            self.width as f32,
            self.height as f32,
            f32::from(draw.state.alpha_ref) / 255.0,
        ] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        let alpha_func = if draw.state.alpha_test {
            draw.state.alpha_func.code()
        } else {
            7
        };
        let textured = u32::from(draw.texture != 0 && self.textures.contains_key(&draw.texture));
        // The combiner stand-in decoded from the bound pixel shader definition (None: the modulate stand-in).
        let pixel = draw.pixel;
        for value in [
            alpha_func,
            textured,
            u32::from(draw.clip),
            u32::from(pixel.is_some()),
            // flag bit 0 (experiment `rtt_alpha_one`): a stage 0 texture that is a render target reads alpha 1
            u32::from(self.knob("rtt_alpha_one") && draw.surface_stage[0].is_some())
                | u32::from(self.knob("rtt_probe") && draw.surface_stage[0].is_some()) << 1,
        ] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        match pixel {
            Some(program) => params.extend_from_slice(&program.uniform_bytes()),
            None => params.resize(params.len() + 432, 0),
        }
        for value in draw.fog_colour {
            params.extend_from_slice(&value.to_le_bytes());
        }
        // Per stage texture coordinate scale (slice 8): a stage that samples a render target smaller than the replay surface
        // (the 160x120 bloom surface of the scope) sees its image in the top left corner of the replay sized surface, the
        // draws land at 1:1 pixels there, so its coordinates 0..1 span only that corner.
        for stage in 0..4usize {
            let scale = if draw.grade && stage == 0 {
                // linear surface texture: the pass addresses it in pixels
                [1.0 / self.width as f32, 1.0 / self.height as f32]
            } else {
                draw.surface_stage[stage]
                    .and_then(|key| self.surfaces.get(&key))
                    .filter(|_| !self.knob("no_surface_uv_scale"))
                    .map_or([1.0, 1.0], |&(surface_width, surface_height)| {
                        [
                            surface_width as f32 / self.width as f32,
                            surface_height as f32 / self.height as f32,
                        ]
                    })
            };
            for value in scale {
                params.extend_from_slice(&value.to_le_bytes());
            }
        }
        params.resize(PARAM_STRIDE, 0);
        params
    }

    /// Prepare masked clears in operation order, retaining one slot for every operation.
    fn prepare_clear_groups(
        &mut self,
        device: &wgpu::Device,
        ops: &[Op],
    ) -> Vec<Option<wgpu::BindGroup>> {
        // Masked colour clears: one pipeline per channel mask and a uniform with the clear colour per clear.
        let mut clear_groups: Vec<Option<wgpu::BindGroup>> = Vec::new();
        for op in ops {
            let mut group = None;
            if let Op::Clear(ClearRequest {
                colour: Some(argb),
                channels,
                ..
            }) = op
            {
                if *channels != 0xF {
                    if !self.clear_pipelines.contains_key(channels) {
                        let clear_started = std::time::Instant::now();
                        let pipeline =
                            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                                label: Some("tsfp-clear-pipeline"),
                                layout: Some(&self.clear_pipeline_layout),
                                vertex: wgpu::VertexState {
                                    module: &self.clear_module,
                                    entry_point: Some("vs_main"),
                                    compilation_options: Default::default(),
                                    buffers: &[],
                                },
                                fragment: Some(wgpu::FragmentState {
                                    module: &self.clear_module,
                                    entry_point: Some("fs_main"),
                                    compilation_options: Default::default(),
                                    targets: &[Some(wgpu::ColorTargetState {
                                        format: COLOUR_FORMAT,
                                        blend: None,
                                        write_mask: wgpu::ColorWrites::from_bits_truncate(
                                            u32::from(*channels),
                                        ),
                                    })],
                                }),
                                primitive: wgpu::PrimitiveState::default(),
                                depth_stencil: Some(wgpu::DepthStencilState {
                                    format: DEPTH_FORMAT,
                                    depth_write_enabled: false,
                                    depth_compare: wgpu::CompareFunction::Always,
                                    stencil: Default::default(),
                                    bias: Default::default(),
                                }),
                                multisample: Default::default(),
                                multiview: None,
                                cache: None,
                            });
                        self.clear_pipelines.insert(*channels, pipeline);
                        self.timings.pipelines_ns += clear_started.elapsed().as_nanos() as u64;
                        self.timings.pipelines_created += 1;
                    }
                    let [r, g, b, a] = crate::nv2a::argb_to_rgba(*argb);
                    let mut bytes = Vec::with_capacity(16);
                    for value in [r, g, b, a] {
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("tsfp-clear-colour"),
                        contents: &bytes,
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                    group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("tsfp-clear"),
                        layout: &self.clear_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform.as_entire_binding(),
                        }],
                    }));
                }
            }
            clear_groups.push(group);
        }

        clear_groups
    }

    /// Resolve each stage once per frame, preserving generation-based cache invalidation.
    fn prepare_texture_views(
        &mut self,
        ops: &[Op],
        views: &mut HashMap<u32, wgpu::TextureView>,
        mut texture_batch: Option<&mut TextureUploads>,
    ) {
        for op in ops {
            if let Op::Draw(draw) = op {
                for id in draw.stage_ids() {
                    if !views.contains_key(&id) && !self.textures.contains_key(&id) {
                        let black = self.null_black();
                        let view = self.null_views.borrow_mut()[usize::from(black)]
                            .get_or_insert_with(|| self.upload_texture(None, None))
                            .clone();
                        views.insert(id, view);
                    }
                    if let std::collections::hash_map::Entry::Vacant(entry) = views.entry(id) {
                        let generation = self.texture_generation.get(&id).copied().unwrap_or(0);
                        let cached = self
                            .view_cache
                            .borrow()
                            .get(&id)
                            .filter(|(built, _)| *built == generation)
                            .map(|(_, view)| view.clone());
                        let view =
                            if let (Some(view), true) = (cached, self.textures.contains_key(&id)) {
                                self.timings.texture_cache_hits += 1;
                                view
                            } else {
                                let view = self.upload_texture(
                                    self.textures.get(&id),
                                    texture_batch.as_deref_mut(),
                                );
                                if self.textures.contains_key(&id) {
                                    self.timings.texture_uploads += 1;
                                    self.view_cache
                                        .borrow_mut()
                                        .insert(id, (generation, view.clone()));
                                }
                                view
                            };
                        entry.insert(view);
                    }
                }
            }
        }
    }

    /// Submit the encoded image and complete the selected publication/readback path.
    /// Shared-ring locking and callback ordering stay local to each path.
    fn finish_present(
        &mut self,
        device: &wgpu::Device,
        mut encoder: wgpu::CommandEncoder,
        shown: wgpu::Texture,
        (out_width, out_height): (u32, u32),
        (frame_started, mark, phase_base): (std::time::Instant, std::time::Instant, [u64; 4]),
        timings_before: PhaseTimings,
    ) {
        self.display_texture = Some(shown.clone());
        if let Some(ring) = &self.shared_frames {
            // Hold the lock through submission: acquire cannot observe an unsubmitted copy.
            let mut ring = ring.lock().unwrap();
            if !self.suppress_presentation {
                ring.encode(device, &mut encoder, &shown, self.readback_present);
            }
            let finish_started = std::time::Instant::now();
            let commands = encoder.finish();
            self.timings.finish_ns += finish_started.elapsed().as_nanos() as u64;
            let queue_started = std::time::Instant::now();
            self.gpu.queue.submit([commands]);
            self.timings.queue_submit_ns += queue_started.elapsed().as_nanos() as u64;
            drop(ring);
            self.track_submission();
            self.poll_gpu();
            device.poll(wgpu::Maintain::Poll);
            self.timings.submit_ns += mark.elapsed().as_nanos() as u64;
            self.timings
                .end_frame(frame_started.elapsed().as_nanos() as u64, phase_base);
            self.record_sample(&timings_before);
            self.frame_start_surface = self.current_surface;
            // T2293: count the image handed to the shared ring as drawn (no CPU pixels), as the unread path below does.
            if !self.suppress_presentation {
                self.frames.push(Frame {
                    width: out_width,
                    height: out_height,
                    rgba: Vec::new(),
                });
            }
            return;
        }
        if self.knob("pipelined_readback") && self.readback {
            let started = std::time::Instant::now();
            let slot = if self.suppress_presentation {
                None
            } else {
                self.readback_ring
                    .encode(device, &mut encoder, &shown, (out_width, out_height))
            };
            let finish_started = std::time::Instant::now();
            let commands = encoder.finish();
            self.timings.finish_ns += finish_started.elapsed().as_nanos() as u64;
            let queue_started = std::time::Instant::now();
            self.gpu.queue.submit([commands]);
            self.timings.queue_submit_ns += queue_started.elapsed().as_nanos() as u64;
            self.track_submission();
            if let Some(slot) = slot {
                self.readback_ring.map(slot, self.readback_present);
            }
            device.poll(wgpu::Maintain::Poll);
            self.timings.readback_ns += started.elapsed().as_nanos() as u64;
            self.timings.submit_ns += mark.elapsed().as_nanos() as u64;
            self.timings
                .end_frame(frame_started.elapsed().as_nanos() as u64, phase_base);
            self.record_sample(&timings_before);
            self.frame_start_surface = self.current_surface;
            return;
        }

        if !self.readback || self.suppress_presentation {
            let finish_started = std::time::Instant::now();
            let commands = encoder.finish();
            self.timings.finish_ns += finish_started.elapsed().as_nanos() as u64;
            let queue_started = std::time::Instant::now();
            self.gpu.queue.submit([commands]);
            self.timings.queue_submit_ns += queue_started.elapsed().as_nanos() as u64;
            // Not read back: nothing waits for the GPU, so bound the frames in flight to 2 instead (a CPU running far ahead of a
            // slow GPU would otherwise queue unbounded work and memory).
            self.track_submission();
            device.poll(wgpu::Maintain::Poll);
            if self.in_flight.load(Ordering::SeqCst) > MAX_FRAMES_IN_FLIGHT {
                let wait_started = std::time::Instant::now();
                device.poll(wgpu::Maintain::Wait);
                self.timings.wait_ns += wait_started.elapsed().as_nanos() as u64;
                self.timings.waits += 1;
            }
            self.poll_gpu();
            self.timings.submit_ns += mark.elapsed().as_nanos() as u64;
            self.timings
                .end_frame(frame_started.elapsed().as_nanos() as u64, phase_base);
            self.record_sample(&timings_before);
            self.frame_start_surface = self.current_surface;
            if !self.suppress_presentation {
                self.frames.push(Frame {
                    width: out_width,
                    height: out_height,
                    rgba: Vec::new(),
                });
            }
            return;
        }
        let readback_started = std::time::Instant::now();
        let row_bytes = out_width * 4;
        let padded = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tsfp-readback"),
            size: u64::from(padded * out_height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &shown,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(out_height),
                },
            },
            wgpu::Extent3d {
                width: out_width,
                height: out_height,
                depth_or_array_layers: 1,
            },
        );
        let finish_started = std::time::Instant::now();
        let commands = encoder.finish();
        self.timings.finish_ns += finish_started.elapsed().as_nanos() as u64;
        let queue_started = std::time::Instant::now();
        self.gpu.queue.submit([commands]);
        self.timings.queue_submit_ns += queue_started.elapsed().as_nanos() as u64;
        self.track_submission();
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let wait_started = std::time::Instant::now();
        device.poll(wgpu::Maintain::Wait);
        self.timings.readback_wait_ns += wait_started.elapsed().as_nanos() as u64;
        self.timings.readback_waits += 1;
        self.poll_gpu();
        let callback_started = std::time::Instant::now();
        receiver
            .recv()
            .expect("map callback")
            .expect("readback map");
        self.timings.map_callback_ns += callback_started.elapsed().as_nanos() as u64;
        let copy_started = std::time::Instant::now();
        let mapped = slice.get_mapped_range();
        let mut rgba = Vec::with_capacity((row_bytes * out_height) as usize);
        for row in mapped.chunks_exact(padded as usize) {
            rgba.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        readback.unmap();
        self.timings.row_copy_ns += copy_started.elapsed().as_nanos() as u64;
        // `host_display` (the slice 8 display transfer) is a documented no-op since T2140: the brightness gap it fitted was the missing
        // closing grade pass, see `grade` in `draw` and docs/t-wgpu-renderer.md Slice 9.
        self.timings.submit_ns += mark.elapsed().as_nanos() as u64;
        self.timings.readback_ns += readback_started.elapsed().as_nanos() as u64;
        self.timings
            .end_frame(frame_started.elapsed().as_nanos() as u64, phase_base);
        self.record_sample(&timings_before);
        self.frame_start_surface = self.current_surface;
        self.frames.push(Frame {
            width: out_width,
            height: out_height,
            rgba,
        });
    }
}

/// A buffer that lives across frames: created (power of two sized) when `data` does not fit, then written with `write_buffer`.
fn persistent_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slot: &mut Option<(wgpu::Buffer, u64)>,
    label: &'static str,
    usage: wgpu::BufferUsages,
    data: &[u8],
) -> wgpu::Buffer {
    let padded = (data.len() as u64).next_multiple_of(4);
    if slot.as_ref().is_none_or(|(_, size)| *size < padded) {
        let size = padded.next_power_of_two().max(1 << 16);
        *slot = Some((
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            size,
        ));
    }
    let buffer = slot.as_ref().expect("buffer").0.clone();
    if padded as usize == data.len() {
        queue.write_buffer(&buffer, 0, data);
    } else {
        let mut copy = data.to_vec();
        copy.resize(padded as usize, 0);
        queue.write_buffer(&buffer, 0, &copy);
    }
    buffer
}

/// A fast 64 bit content hash (multiply-rotate over 8 byte words), enough to tell a texture's texels changed.
fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0x9E37_79B9_7F4A_7C15u64 ^ bytes.len() as u64;
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().expect("8 bytes"));
        hash = (hash.rotate_left(23) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    for &byte in chunks.remainder() {
        hash = (hash.rotate_left(23) ^ u64::from(byte)).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    hash ^ (hash >> 29)
}

/// All mip levels of an RGBA8 image (box filter, 2x2 averages, odd sizes round down, minimum 1) appended to the base level, and the
/// level count, in the layout `create_texture_with_data` expects.
fn mip_chain(width: u32, height: u32, base: &[u8], max_levels: u32) -> (u32, Vec<u8>) {
    let mut chain = base.to_vec();
    let (mut level_width, mut level_height) = (width as usize, height as usize);
    let mut previous: Vec<u8> = base.to_vec();
    let mut levels = 1u32;
    while (level_width > 1 || level_height > 1) && levels < max_levels {
        let next_width = (level_width / 2).max(1);
        let next_height = (level_height / 2).max(1);
        let mut next = Vec::with_capacity(next_width * next_height * 4);
        for y in 0..next_height {
            for x in 0..next_width {
                let x0 = (x * 2).min(level_width - 1);
                let x1 = (x * 2 + 1).min(level_width - 1);
                let y0 = (y * 2).min(level_height - 1);
                let y1 = (y * 2 + 1).min(level_height - 1);
                for channel in 0..4 {
                    let at = |px: usize, py: usize| {
                        u32::from(previous[(py * level_width + px) * 4 + channel])
                    };
                    let sum = at(x0, y0) + at(x1, y0) + at(x0, y1) + at(x1, y1);
                    next.push(((sum + 2) / 4) as u8);
                }
            }
        }
        chain.extend_from_slice(&next);
        previous = next;
        level_width = next_width;
        level_height = next_height;
        levels += 1;
    }
    (levels, chain)
}

/// Linear interpolation of every vertex attribute (clip space, so the GPU's perspective correct interpolation stays exact).
fn lerp_vertex(a: &Vertex, b: &Vertex, t: f32) -> Vertex {
    let mix = |x: f32, y: f32| x + (y - x) * t;
    Vertex {
        position: std::array::from_fn(|i| mix(a.position[i], b.position[i])),
        colour: std::array::from_fn(|i| mix(a.colour[i], b.colour[i])),
        specular: std::array::from_fn(|i| mix(a.specular[i], b.specular[i])),
        uv: std::array::from_fn(|i| mix(a.uv[i], b.uv[i])),
        fog: mix(a.fog, b.fog),
        uv_extra: std::array::from_fn(|k| {
            std::array::from_fn(|i| mix(a.uv_extra[k][i], b.uv_extra[k][i]))
        }),
    }
}

/// Clips a polygon of clip space vertices against `w >= 1e-4` and `z >= 0` (Sutherland Hodgman), then clamps z to `w` (the NV2A
/// clamps depth, it does not clip it at the far end). Slice 5 experiment `cpu_clip`.
fn clip_polygon(polygon: Vec<Vertex>) -> Vec<Vertex> {
    let planes: [fn(&Vertex) -> f32; 2] = [|v| v.position[3] - 1e-4, |v| v.position[2]];
    let mut polygon = polygon;
    for plane in planes {
        let mut output = Vec::with_capacity(polygon.len() + 1);
        for index in 0..polygon.len() {
            let (a, b) = (&polygon[index], &polygon[(index + 1) % polygon.len()]);
            let (da, db) = (plane(a), plane(b));
            if da >= 0.0 {
                output.push(*a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                output.push(lerp_vertex(a, b, da / (da - db)));
            }
        }
        polygon = output;
        if polygon.len() < 3 {
            return Vec::new();
        }
    }
    for vertex in &mut polygon {
        vertex.position[2] = vertex.position[2].min(vertex.position[3]);
    }
    polygon
}

fn blend_factor(factor: Factor) -> wgpu::BlendFactor {
    use wgpu::BlendFactor as B;
    match factor {
        Factor::Zero => B::Zero,
        Factor::One => B::One,
        Factor::SrcColour => B::Src,
        Factor::OneMinusSrcColour => B::OneMinusSrc,
        Factor::SrcAlpha => B::SrcAlpha,
        Factor::OneMinusSrcAlpha => B::OneMinusSrcAlpha,
        Factor::DstAlpha => B::DstAlpha,
        Factor::OneMinusDstAlpha => B::OneMinusDstAlpha,
        Factor::DstColour => B::Dst,
        Factor::OneMinusDstColour => B::OneMinusDst,
        Factor::SrcAlphaSaturate => B::SrcAlphaSaturated,
        Factor::ConstantColour | Factor::ConstantAlpha => B::Constant,
        Factor::OneMinusConstantColour | Factor::OneMinusConstantAlpha => B::OneMinusConstant,
    }
}

fn compare_function(compare: Compare) -> wgpu::CompareFunction {
    use wgpu::CompareFunction as C;
    match compare {
        Compare::Never => C::Never,
        Compare::Less => C::Less,
        Compare::Equal => C::Equal,
        Compare::LessEqual => C::LessEqual,
        Compare::Greater => C::Greater,
        Compare::NotEqual => C::NotEqual,
        Compare::GreaterEqual => C::GreaterEqual,
        Compare::Always => C::Always,
    }
}

fn build_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    key: PipelineKey,
) -> wgpu::RenderPipeline {
    let operation = match key.equation {
        Equation::Add => wgpu::BlendOperation::Add,
        Equation::Subtract => wgpu::BlendOperation::Subtract,
        Equation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
        Equation::Min => wgpu::BlendOperation::Min,
        Equation::Max => wgpu::BlendOperation::Max,
    };
    let component = wgpu::BlendComponent {
        src_factor: blend_factor(key.source),
        dst_factor: blend_factor(key.destination),
        operation,
    };
    let attributes = [
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
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 40,
            shader_location: 3,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 48,
            shader_location: 4,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 56,
            shader_location: 5,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32,
            offset: 64,
            shader_location: 6,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 68,
            shader_location: 7,
        },
    ];
    // The screen y axis points down, so the clip space winding flips: NV2A front_face CW is
    // clockwise on screen.
    let front_face = if key.front_clockwise {
        wgpu::FrontFace::Cw
    } else {
        wgpu::FrontFace::Ccw
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tsfp-hud-pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: Vertex::SIZE as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: COLOUR_FORMAT,
                blend: key.blend_enable.then_some(wgpu::BlendState {
                    color: component,
                    alpha: component,
                }),
                write_mask: wgpu::ColorWrites::from_bits_truncate(u32::from(key.write_mask)),
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: if key.lines {
                wgpu::PrimitiveTopology::LineList
            } else {
                wgpu::PrimitiveTopology::TriangleList
            },
            front_face,
            cull_mode: match key.cull {
                None => None,
                Some(CullFace::Back) => Some(wgpu::Face::Back),
                Some(CullFace::Front) => Some(wgpu::Face::Front),
                // Both faces culled draws nothing: a Back cull with a degenerate front face is
                // not expressible, so the quad pass is skipped by a never-passing depth test.
                Some(CullFace::FrontAndBack) => Some(wgpu::Face::Back),
            },
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: key.depth_test && key.depth_write,
            depth_compare: if key.depth_test {
                if key.cull == Some(CullFace::FrontAndBack) {
                    wgpu::CompareFunction::Never
                } else {
                    compare_function(key.depth_func)
                }
            } else if key.cull == Some(CullFace::FrontAndBack) {
                wgpu::CompareFunction::Never
            } else {
                wgpu::CompareFunction::Always
            },
            stencil: Default::default(),
            bias: wgpu::DepthBiasState {
                constant: key.depth_bias_constant,
                slope_scale: f32::from_bits(key.depth_bias_slope_bits),
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Vertex constants at the start of a replay. The two Direct3D reserved viewport constants (`c[-38]` scale and `c[-37]` offset,
/// hardware registers 58 and 59, read by the viewport transform instructions the runtime appends to the title's programs) are
/// written by the lifted D3D library straight into the push buffer, so no `RenderDevice` call carries them (MEASURED: the
/// recorded streams only set registers 0xB, 0x3C and 0x40). They are synthesised from the replay size (INFERRED: a 24 bit depth
/// range, the pixel centre offset is not modelled). `vsh::to_clip` inverts them, so only their consistency matters.
fn default_vsh_constants(width: u32, height: u32) -> [Vec4; CONSTANT_COUNT] {
    let mut constants = [[0.0; 4]; CONSTANT_COUNT];
    constants[58] = [
        width as f32 / 2.0,
        -(height as f32) / 2.0,
        16_777_215.0,
        0.0,
    ];
    constants[59] = [width as f32 / 2.0, height as f32 / 2.0, 0.0, 0.0];
    constants
}

/// ARGB colour of a draw that is a full screen opaque overwrite: two triangles over four vertices that span the whole clip
/// volume, one flat colour, blend off or One/Zero, no depth test. INFERRED to be the engine's frame clear.
fn clear_like_colour(state: &PipelineState, vertices: &[Vertex], indices: usize) -> Option<u32> {
    let overwrite =
        !state.blend_enable || (state.source == Factor::One && state.destination == Factor::Zero);
    if !overwrite || state.depth_test || vertices.len() != 4 || indices != 6 {
        return None;
    }
    let first = vertices[0].colour;
    let flat = vertices.iter().all(|vertex| vertex.colour == first);
    let covers = vertices.iter().all(|vertex| {
        let w = vertex.position[3];
        (vertex.position[0] / w).abs() > 0.99 && (vertex.position[1] / w).abs() > 0.99
    });
    if !flat || !covers {
        return None;
    }
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    Some((byte(first[3]) << 24) | (byte(first[0]) << 16) | (byte(first[1]) << 8) | byte(first[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_chain_averages_two_by_two_blocks_and_stops_at_the_declared_level_count() {
        // 4x2 image: left half 0, right half 200 in every channel.
        let mut base = Vec::new();
        for _row in 0..2 {
            for x in 0..4 {
                base.extend_from_slice(&[if x < 2 { 0u8 } else { 200u8 }; 4]);
            }
        }
        let (levels, chain) = mip_chain(4, 2, &base, u32::MAX);
        assert_eq!(levels, 3, "4x2, 2x1, 1x1");
        assert_eq!(chain.len(), (8 + 2 + 1) * 4);
        assert_eq!(&chain[32..36], &[0, 0, 0, 0], "level 1 left");
        assert_eq!(&chain[36..40], &[200, 200, 200, 200], "level 1 right");
        assert_eq!(&chain[40..44], &[100, 100, 100, 100], "level 2 is the mean");
        let (limited, short) = mip_chain(4, 2, &base, 2);
        assert_eq!(limited, 2, "the declared level count bounds the chain");
        assert_eq!(short.len(), (8 + 2) * 4);
    }
}

#[cfg(test)]
mod cold_resource_tests {
    use super::*;
    use crate::context::Preference;

    #[test]
    fn prewarm_is_opt_in_and_does_not_draw_or_upload() {
        let gpu = GpuContext::new(&Preference::Software).expect("lavapipe adapter");
        let mut replay = Replayer::new(gpu, 16, 16);
        let state = PipelineState::default();
        assert!(!replay.prewarm_pipeline(&state, false));
        replay.knobs.insert("prewarm_pipelines".into(), "1".into());
        assert!(!replay.prewarm_pipeline(&state, false));
        replay.set_async_pipelines(1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !replay.prewarm_pipeline(&state, false) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(replay.pipelines.len(), 1);
        assert_eq!(replay.timings.draws, 0);
        assert_eq!(replay.timings.texture_uploads, 0);
        assert!(replay.display_texture.is_none());
        // Every topology/state change has a separate key.
        assert!(!replay.prewarm_pipeline(&state, true));
    }

    #[test]
    #[ignore = "bounded host-free cold upload microbenchmark; run explicitly"]
    fn sixty_six_texture_upload_microbenchmark() {
        let gpu = GpuContext::new(&Preference::Software).expect("lavapipe adapter");
        let replay = Replayer::new(gpu, 16, 16);
        let image = TextureImage {
            width: 256,
            height: 256,
            rgba: vec![127; 256 * 256 * 4],
            linear: false,
            mips: 9,
        };
        for batched in [false, true, false, true] {
            replay.gpu.device.poll(wgpu::Maintain::Wait);
            let started = std::time::Instant::now();
            let mut batch = batched.then(TextureUploads::default);
            let views: Vec<_> = (0..66)
                .map(|_| replay.upload_texture(Some(&image), batch.as_mut()))
                .collect();
            let mut encoder = replay
                .gpu
                .device
                .create_command_encoder(&Default::default());
            if let Some(batch) = &batch {
                batch.encode(&replay.gpu.device, &mut encoder);
            }
            let encode_us = started.elapsed().as_micros();
            replay.gpu.queue.submit([encoder.finish()]);
            replay.gpu.device.poll(wgpu::Maintain::Wait);
            println!(
                "T2228 66x256x256 mip9 batched={batched} encode_us={encode_us} total_us={} staging_bytes={}",
                started.elapsed().as_micros(),
                batch.as_ref().map_or(0, |batch| batch.bytes.len())
            );
            assert_eq!(views.len(), 66);
        }
    }
}

/// Texture record to the decoded generation and reusable GPU view.
type TextureViewCache = HashMap<u32, (u64, wgpu::TextureView)>;

/// Allocate colour/depth together and assign the next stable surface identity.
fn make_surface_gpu(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    surface_counter: &std::cell::Cell<u64>,
    label: &'static str,
) -> SurfaceGpu {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOUR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tsfp-depth"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    surface_counter.set(surface_counter.get() + 1);
    SurfaceGpu {
        id: surface_counter.get(),
        view: texture.create_view(&Default::default()),
        depth_view: depth.create_view(&Default::default()),
        texture,
    }
}

/// Keep per-frame diagnostic storage bounded across long live sessions.
const MAX_FRAME_SAMPLES: usize = 65_536;
/// Readback-free rendering retains the original two-frame CPU lead limit.
const MAX_FRAMES_IN_FLIGHT: usize = 2;

/// Deferred primitive with either a vertex run or an indexed run.
type PendingDraw = (u32, Option<(u32, u32)>, Option<(u32, u32)>);
