// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238's opt-in native scene comparison layer.
//!
//! Captured room packets are rendered for bounded flat, textured alpha/blend,
//! and translated vertex-program classes. Other materials remain on the
//! legacy renderer. No synthetic room geometry is added to the comparison
//! frame. An optional per-class filter supports live captures over legacy.
use std::collections::{BTreeSet, HashMap};
use std::num::NonZeroU64;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tsfp_engine_render::native_scene::{
    NativeViewInput, PreparedSceneLease, RoomDrawPacket, SceneFrameLeaseSink, SceneFrameSink,
};
use tsfp_engine_render::scene::light_state::{
    FrameResourceEpoch, PublishedSceneLights, SceneLightBinding, ViewLightBlock,
};
use tsfp_engine_render::scene::render_prep::{ObjectId, PreparedSceneFrame};
use wgpu::util::DeviceExt;

const LAYER_KEY: &str = "native-scene";
// The diagnostic room pass belongs under both the HUD and the menu layer.
const LAYER_ORDER: i32 = -10;
const MAX_ROOMS: usize = 200;
const MAX_OBJECTS: usize = 0x226;
const MAX_VIEWS: usize = 16;

/// Raw scene-light field selected by the opt-in transport diagnostic.
/// Position previews map view-space XYZ to clamped RGB only for inspection;
/// enabled-mask previews show the measured producer branch without assigning
/// lighting semantics to the slot. `EnabledSlotColorTransport` sums enabled
/// slot RGB for a combined payload view; it is not the game's lighting equation.
/// Parameter previews expose the final three words without naming their meaning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum SceneLightPreviewSource {
    #[default]
    Slot0 = 0,
    Slot1 = 1,
    Slot2 = 2,
    FourthColor = 3,
    Slot0Position = 4,
    Slot1Position = 5,
    Slot2Position = 6,
    Slot0Enabled = 7,
    Slot1Enabled = 8,
    Slot2Enabled = 9,
    EnabledSlotColorTransport = 10,
    Slot0Parameters = 11,
    Slot1Parameters = 12,
    Slot2Parameters = 13,
}

impl SceneLightPreviewSource {
    fn from_raw(value: u64) -> Self {
        match value {
            1 => Self::Slot1,
            2 => Self::Slot2,
            3 => Self::FourthColor,
            4 => Self::Slot0Position,
            5 => Self::Slot1Position,
            6 => Self::Slot2Position,
            7 => Self::Slot0Enabled,
            8 => Self::Slot1Enabled,
            9 => Self::Slot2Enabled,
            10 => Self::EnabledSlotColorTransport,
            11 => Self::Slot0Parameters,
            12 => Self::Slot1Parameters,
            13 => Self::Slot2Parameters,
            _ => Self::Slot0,
        }
    }

    fn fragment_entry_point(self) -> &'static str {
        match self {
            Self::Slot0 => "fs_slot_0",
            Self::Slot1 => "fs_slot_1",
            Self::Slot2 => "fs_slot_2",
            Self::FourthColor => "fs_fourth_color",
            Self::Slot0Position => "fs_slot_0_position",
            Self::Slot1Position => "fs_slot_1_position",
            Self::Slot2Position => "fs_slot_2_position",
            Self::Slot0Enabled => "fs_slot_0_enabled",
            Self::Slot1Enabled => "fs_slot_1_enabled",
            Self::Slot2Enabled => "fs_slot_2_enabled",
            Self::EnabledSlotColorTransport => "fs_enabled_slot_color_transport",
            Self::Slot0Parameters => "fs_slot_0_parameters",
            Self::Slot1Parameters => "fs_slot_1_parameters",
            Self::Slot2Parameters => "fs_slot_2_parameters",
        }
    }

    fn raw_preview_rgb(self, block: &GpuSceneLightBlock) -> [f32; 3] {
        let value = match self {
            Self::Slot0 => block.slots[0].color,
            Self::Slot1 => block.slots[1].color,
            Self::Slot2 => block.slots[2].color,
            Self::FourthColor => block.fourth_color,
            Self::Slot0Position => block.slots[0].position,
            Self::Slot1Position => block.slots[1].position,
            Self::Slot2Position => block.slots[2].position,
            Self::Slot0Enabled => return enabled_preview_rgb(block.slots[0].enabled),
            Self::Slot1Enabled => return enabled_preview_rgb(block.slots[1].enabled),
            Self::Slot2Enabled => return enabled_preview_rgb(block.slots[2].enabled),
            Self::Slot0Parameters => return block.slots[0].parameters,
            Self::Slot1Parameters => return block.slots[1].parameters,
            Self::Slot2Parameters => return block.slots[2].parameters,
            Self::EnabledSlotColorTransport => {
                return block.slots.iter().filter(|slot| slot.enabled != 0).fold(
                    [0.0; 3],
                    |mut sum, slot| {
                        for (channel, value) in sum.iter_mut().zip(slot.color) {
                            *channel += value;
                        }
                        sum
                    },
                );
            }
        };
        [value[0], value[1], value[2]]
    }
}

fn enabled_preview_rgb(enabled: u32) -> [f32; 3] {
    let value = if enabled == 0 { 0.0 } else { 1.0 };
    [value; 3]
}

/// One storage-buffer slot for the measured 0x28-byte light payload.
///
/// The original ten words preserve view position, RGB color and opaque
/// parameters. A typed enabled word follows them in renderer padding and is
/// derived from the producer's measured color-length branch. The 16-byte
/// alignment keeps each storage slot at 48 bytes.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuSceneLightSlot {
    position: [f32; 4],
    color: [f32; 4],
    parameters: [f32; 3],
    enabled: u32,
}

/// 16-byte-aligned renderer payload for one published record/view block.
///
/// This is a data-layout adapter only; the optional diagnostic shows one raw
/// RGB field without assigning it lighting semantics.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuSceneLightBlock {
    slots: [GpuSceneLightSlot; 3],
    fourth_color: [f32; 4],
}

/// Renderer-side payload paired with its typed CPU lookup identity.
/// Record/view keys stay out of the GPU bytes but travel with the block so a
/// native draw can resolve the matching light data without relying on order.
#[derive(Clone, Copy, Debug, PartialEq)]
struct GpuSceneLightEntry {
    binding: SceneLightBinding,
    payload: GpuSceneLightBlock,
}

impl From<&ViewLightBlock> for GpuSceneLightBlock {
    fn from(block: &ViewLightBlock) -> Self {
        Self {
            slots: core::array::from_fn(|index| {
                let slot = block.slots[index];
                GpuSceneLightSlot {
                    position: slot.position.0,
                    color: [slot.color.0[0], slot.color.0[1], slot.color.0[2], 0.0],
                    parameters: slot.parameters,
                    enabled: u32::from(slot.enabled),
                }
            }),
            fourth_color: [
                block.fourth_color.0[0],
                block.fourth_color.0[1],
                block.fourth_color.0[2],
                0.0,
            ],
        }
    }
}

/// CPU reference for the default-off shader diagnostic. It displays the
/// selected raw field as clamped RGB and carries source alpha through
/// unchanged; positions and combined enabled colors inspect transport only.
fn scene_light_preview_color(
    blocks: &[GpuSceneLightBlock],
    block_index: u32,
    alpha: f32,
    source: SceneLightPreviewSource,
) -> Option<[f32; 4]> {
    let color = source.raw_preview_rgb(blocks.get(block_index as usize)?);
    Some([
        color[0].clamp(0.0, 1.0),
        color[1].clamp(0.0, 1.0),
        color[2].clamp(0.0, 1.0),
        alpha,
    ])
}

#[derive(Clone, Default)]
struct ScenePacket {
    objects: Vec<ObjectId>,
    views: Vec<NativeViewInput>,
    draw_packets: Vec<RoomDrawPacket>,
    scene_lights: Option<PublishedSceneLights>,
    gpu_scene_lights: Vec<GpuSceneLightEntry>,
    frame_lease: Option<PreparedSceneLease>,
}

impl ScenePacket {
    fn scene_lights(&self) -> Option<&PublishedSceneLights> {
        self.frame_lease
            .as_ref()
            .and_then(|lease| lease.frame().scene_lights.as_ref())
            .or(self.scene_lights.as_ref())
    }

    fn gpu_scene_light_for(&self, binding: SceneLightBinding) -> Option<&GpuSceneLightBlock> {
        self.scene_lights()?.block_for_binding(binding)?;
        let mut entries = self
            .gpu_scene_lights
            .iter()
            .filter(|entry| entry.binding == binding);
        let entry = entries.next()?;
        if entries.next().is_some() {
            return None;
        }
        Some(&entry.payload)
    }

    fn gpu_scene_light_for_draw(
        &self,
        binding: Option<SceneLightBinding>,
    ) -> Option<&GpuSceneLightBlock> {
        self.gpu_scene_light_for(binding?)
    }
}

fn native_draw_has_scene_light_match(
    packet: &ScenePacket,
    binding: Option<SceneLightBinding>,
    native_draw_accepted: bool,
) -> bool {
    native_draw_accepted && packet.gpu_scene_light_for_draw(binding).is_some()
}

/// Compact renderer upload for accepted native draws with explicit typed
/// light bindings. Draw indices refer to the source packet; block indices
/// address the deduplicated storage payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GpuSceneLightDrawBinding {
    draw_index: usize,
    block_index: u32,
}

#[derive(Clone, Default)]
struct GpuSceneLightUpload {
    identities: Vec<SceneLightBinding>,
    blocks: Vec<GpuSceneLightBlock>,
    draw_bindings: Vec<GpuSceneLightDrawBinding>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GpuSceneLightBinding {
    epoch: FrameResourceEpoch,
    binding: SceneLightBinding,
    block_index: u32,
}

/// GPU storage and its typed draw lookup live together until queue completion.
/// The mapping is retained alongside the buffer so a later shader binding can
/// resolve the same source draw without rebuilding identities from vector order.
struct GpuSceneLightFrame {
    _buffer: wgpu::Buffer,
    debug_bind_group: Option<Arc<wgpu::BindGroup>>,
    epoch: FrameResourceEpoch,
    identities: Vec<SceneLightBinding>,
    draw_bindings: Vec<GpuSceneLightDrawBinding>,
}

impl GpuSceneLightFrame {
    fn new(
        buffer: wgpu::Buffer,
        debug_bind_group: Option<Arc<wgpu::BindGroup>>,
        epoch: FrameResourceEpoch,
        upload: GpuSceneLightUpload,
    ) -> Option<Self> {
        if !scene_light_upload_is_well_formed(&upload) {
            return None;
        }
        let frame = Self {
            _buffer: buffer,
            debug_bind_group,
            epoch,
            identities: upload.identities,
            draw_bindings: upload.draw_bindings,
        };
        frame
            .draw_bindings
            .iter()
            .all(|binding| frame.binding_for_draw(epoch, binding.draw_index).is_some())
            .then_some(frame)
    }

    fn binding_for_draw(
        &self,
        expected_epoch: FrameResourceEpoch,
        draw_index: usize,
    ) -> Option<GpuSceneLightBinding> {
        resolve_scene_light_draw_binding(
            expected_epoch,
            self.epoch,
            &self.identities,
            &self.draw_bindings,
            draw_index,
        )
    }
}

fn resolve_scene_light_draw_binding(
    expected_epoch: FrameResourceEpoch,
    epoch: FrameResourceEpoch,
    identities: &[SceneLightBinding],
    draw_bindings: &[GpuSceneLightDrawBinding],
    draw_index: usize,
) -> Option<GpuSceneLightBinding> {
    if expected_epoch != epoch {
        return None;
    }
    let mut bindings = draw_bindings
        .iter()
        .filter(|binding| binding.draw_index == draw_index);
    let draw_binding = bindings.next()?;
    if bindings.next().is_some() {
        return None;
    }
    let binding = *identities.get(draw_binding.block_index as usize)?;
    Some(GpuSceneLightBinding {
        epoch,
        binding,
        block_index: draw_binding.block_index,
    })
}

/// Validate the CPU-side draw map before it is paired with an in-flight GPU
/// buffer. A malformed map must fail closed in release builds too.
fn scene_light_upload_is_well_formed(upload: &GpuSceneLightUpload) -> bool {
    if upload.identities.len() != upload.blocks.len() {
        return false;
    }
    let mut identities = BTreeSet::new();
    if upload
        .identities
        .iter()
        .any(|identity| !identities.insert(*identity))
    {
        return false;
    }
    let mut draws = BTreeSet::new();
    upload.draw_bindings.iter().all(|binding| {
        draws.insert(binding.draw_index)
            && upload
                .identities
                .get(binding.block_index as usize)
                .is_some()
    })
}

impl GpuSceneLightUpload {
    fn bind_draw(
        &mut self,
        packet: &ScenePacket,
        draw_index: usize,
        binding: Option<SceneLightBinding>,
    ) -> Option<u32> {
        let Some(binding) = binding else {
            return None;
        };
        let Some(payload) = packet.gpu_scene_light_for_draw(Some(binding)).copied() else {
            return None;
        };
        let block_index = if let Some(index) = self
            .identities
            .iter()
            .position(|identity| *identity == binding)
        {
            index
        } else {
            self.identities.push(binding);
            self.blocks.push(payload);
            self.identities.len() - 1
        };
        let Ok(block_index) = u32::try_from(block_index) else {
            return None;
        };
        self.draw_bindings.push(GpuSceneLightDrawBinding {
            draw_index,
            block_index,
        });
        Some(block_index)
    }
}

#[derive(Default)]
struct Shared {
    packet: Mutex<Option<ScenePacket>>,
    pending_draw_packets: Mutex<Vec<RoomDrawPacket>>,
    pending_scene_leases: Mutex<Vec<PreparedSceneLease>>,
    pending_scene_light_frames: Mutex<Vec<GpuSceneLightFrame>>,
    scene_light_preview_enabled: AtomicBool,
    scene_light_preview_source: AtomicU64,
    has_packet: AtomicBool,
    submitted: AtomicU64,
    encoded: AtomicU64,
    room_draws: AtomicU64,
    object_draws: AtomicU64,
    view_draws: AtomicU64,
    scene_light_blocks: AtomicU64,
    scene_light_draw_matches: AtomicU64,
    scene_light_gpu_buffers: AtomicU64,
    scene_light_gpu_blocks: AtomicU64,
    scene_light_gpu_draw_bindings: AtomicU64,
    native_packet_draws: AtomicU64,
    legacy_packet_draws: AtomicU64,
    flat_native: AtomicU64,
    flat_legacy: AtomicU64,
    textured_native: AtomicU64,
    textured_legacy: AtomicU64,
    other_legacy: AtomicU64,
    shader_native: AtomicU64,
    shader_legacy: AtomicU64,
    alpha_blend_native: AtomicU64,
    alpha_blend_legacy: AtomicU64,
    alpha_test_native: AtomicU64,
    alpha_test_legacy: AtomicU64,
    cull_native: AtomicU64,
    cull_legacy: AtomicU64,
    layout_native: AtomicU64,
    layout_legacy: AtomicU64,
    combiner_native: AtomicU64,
    combiner_legacy: AtomicU64,
    depth_native: AtomicU64,
    depth_legacy: AtomicU64,
    stencil_native: AtomicU64,
    stencil_legacy: AtomicU64,
    rejected: AtomicU64,
    encode_cpu_us: AtomicU64,
    encode_cpu_us_max: AtomicU64,
    encode_samples: AtomicU64,
}

struct EncodeTimer<'a> {
    total_us: &'a AtomicU64,
    max_us: &'a AtomicU64,
    samples: &'a AtomicU64,
    started: Instant,
}

impl Shared {
    fn retain_scene_lease(&self, lease: PreparedSceneLease) {
        self.pending_scene_leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(lease);
    }

    fn take_pending_scene_leases(&self) -> Vec<PreparedSceneLease> {
        std::mem::take(
            &mut *self
                .pending_scene_leases
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    fn retain_scene_light_frame(&self, frame: GpuSceneLightFrame) {
        self.pending_scene_light_frames
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(frame);
    }

    fn take_pending_scene_light_frames(&self) -> Vec<GpuSceneLightFrame> {
        std::mem::take(
            &mut *self
                .pending_scene_light_frames
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }
}

impl<'a> EncodeTimer<'a> {
    fn new(shared: &'a Shared) -> Self {
        Self {
            total_us: &shared.encode_cpu_us,
            max_us: &shared.encode_cpu_us_max,
            samples: &shared.encode_samples,
            started: Instant::now(),
        }
    }
}

impl Drop for EncodeTimer<'_> {
    fn drop(&mut self) {
        let elapsed_us = self.started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        self.total_us.fetch_add(elapsed_us, Ordering::Relaxed);
        self.max_us.fetch_max(elapsed_us, Ordering::Relaxed);
        self.samples.fetch_add(1, Ordering::Relaxed);
    }
}

/// Production `SceneFrameSink` for the `TSFP_NATIVE_RENDER_PREP=2` comparison
/// mode. It copies the borrowed owner packet for the renderer thread and never
/// retains guest addresses or model asset IDs as resources.
pub struct NativeSceneWgpuSink {
    shared: Arc<Shared>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PacketFilter {
    #[default]
    All,
    Flat,
    Textured,
    VertexShader,
    Alpha,
}

impl PacketFilter {
    fn from_env() -> Self {
        match std::env::var("TSFP_NATIVE_RENDER_PREP_CLASS").as_deref() {
            Ok("flat") => Self::Flat,
            Ok("textured") => Self::Textured,
            Ok("vertex-shader") => Self::VertexShader,
            Ok("alpha") => Self::Alpha,
            _ => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Flat => "flat",
            Self::Textured => "textured",
            Self::VertexShader => "vertex-shader",
            Self::Alpha => "alpha",
        }
    }

    fn accepts(self, packet: &RoomDrawPacket) -> bool {
        let class = packet_material_class(packet);
        match self {
            Self::All => true,
            Self::Flat => class == RoomMaterialClass::FlatVertexColour,
            Self::Textured => {
                class == RoomMaterialClass::TexturedModulate
                    && native_alpha_state(packet)
                        .is_some_and(|state| state.blend.is_none() && !state.test)
            }
            Self::VertexShader => class == RoomMaterialClass::VertexShader,
            Self::Alpha => {
                class == RoomMaterialClass::TexturedModulate
                    && native_alpha_state(packet)
                        .is_some_and(|state| state.blend.is_some() || state.test)
            }
        }
    }
}

/// Counts from actual layer encoding, separate from owner input counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeSceneWgpuStats {
    pub submitted: u64,
    pub encoded: u64,
    pub room_draws: u64,
    pub object_draws: u64,
    pub view_draws: u64,
    /// Typed scene-light blocks retained at renderer ingress.
    pub scene_light_blocks: u64,
    /// Accepted draw observations with a resolvable typed light binding.
    /// This measures lookup independently of the selected diagnostic source.
    pub scene_light_draw_matches: u64,
    /// Storage buffers allocated for matched native draws and retained through
    /// queue completion. The default-off diagnostic can read raw color fields.
    pub scene_light_gpu_buffers: u64,
    /// Deduplicated blocks copied into those frame-scoped storage buffers.
    pub scene_light_gpu_blocks: u64,
    /// Native draws mapped to a block index in those buffers.
    pub scene_light_gpu_draw_bindings: u64,
    pub native_packet_draws: u64,
    pub legacy_packet_draws: u64,
    pub flat_native: u64,
    pub flat_legacy: u64,
    pub textured_native: u64,
    pub textured_legacy: u64,
    pub other_legacy: u64,
    pub shader_native: u64,
    pub shader_legacy: u64,
    pub alpha_blend_native: u64,
    pub alpha_blend_legacy: u64,
    pub alpha_test_native: u64,
    pub alpha_test_legacy: u64,
    pub cull_native: u64,
    pub cull_legacy: u64,
    pub layout_native: u64,
    pub layout_legacy: u64,
    pub combiner_native: u64,
    pub combiner_legacy: u64,
    pub depth_native: u64,
    pub depth_legacy: u64,
    pub stencil_native: u64,
    pub stencil_legacy: u64,
    pub rejected: u64,
    /// CPU time spent encoding this diagnostic layer, excluding GPU execution.
    pub encode_cpu_us_total: u64,
    pub encode_cpu_us_max: u64,
    pub encode_samples: u64,
}

impl NativeSceneWgpuSink {
    /// Install the layer in the same wgpu frame compositor that draws the
    /// retained scene. It loads the legacy image and alpha blends over it.
    pub fn install() -> Self {
        let shared = Arc::new(Shared::default());
        let packet_filter = PacketFilter::from_env();
        eprintln!(
            "tsfp-wgpu: native room comparison class_filter={}",
            packet_filter.label()
        );
        crate::frame_layer::install(
            LAYER_KEY,
            LAYER_ORDER,
            Box::new(NativeSceneLayer {
                shared: shared.clone(),
                pipeline: None,
                room_pipelines: HashMap::new(),
                room_light_preview_pipelines: HashMap::new(),
                scene_light_bind_group_layout: None,
                textured_pipeline: None,
                shader_pipelines: HashMap::new(),
                shader_programs: HashMap::new(),
                packet_filter,
                rendered_present: AtomicU64::new(0),
            }),
        );
        Self { shared }
    }

    /// Enable a diagnostic view that displays one raw color field for
    /// explicitly bound native flat draws. This is default-off and is not the
    /// original lighting model or an adopted renderer feature.
    pub fn set_scene_light_preview_enabled(&self, enabled: bool) {
        self.shared
            .scene_light_preview_enabled
            .store(enabled, Ordering::Release);
    }

    /// Select which raw field or derived enabled bit the default-off
    /// diagnostic displays.
    pub fn set_scene_light_preview_source(&self, source: SceneLightPreviewSource) {
        self.shared
            .scene_light_preview_source
            .store(source as u64, Ordering::Release);
    }

    pub fn take_stats(&self) -> NativeSceneWgpuStats {
        NativeSceneWgpuStats {
            submitted: self.shared.submitted.swap(0, Ordering::Relaxed),
            encoded: self.shared.encoded.swap(0, Ordering::Relaxed),
            room_draws: self.shared.room_draws.swap(0, Ordering::Relaxed),
            object_draws: self.shared.object_draws.swap(0, Ordering::Relaxed),
            view_draws: self.shared.view_draws.swap(0, Ordering::Relaxed),
            scene_light_blocks: self.shared.scene_light_blocks.swap(0, Ordering::Relaxed),
            scene_light_draw_matches: self
                .shared
                .scene_light_draw_matches
                .swap(0, Ordering::Relaxed),
            scene_light_gpu_buffers: self
                .shared
                .scene_light_gpu_buffers
                .swap(0, Ordering::Relaxed),
            scene_light_gpu_blocks: self
                .shared
                .scene_light_gpu_blocks
                .swap(0, Ordering::Relaxed),
            scene_light_gpu_draw_bindings: self
                .shared
                .scene_light_gpu_draw_bindings
                .swap(0, Ordering::Relaxed),
            native_packet_draws: self.shared.native_packet_draws.swap(0, Ordering::Relaxed),
            legacy_packet_draws: self.shared.legacy_packet_draws.swap(0, Ordering::Relaxed),
            flat_native: self.shared.flat_native.swap(0, Ordering::Relaxed),
            flat_legacy: self.shared.flat_legacy.swap(0, Ordering::Relaxed),
            textured_native: self.shared.textured_native.swap(0, Ordering::Relaxed),
            textured_legacy: self.shared.textured_legacy.swap(0, Ordering::Relaxed),
            other_legacy: self.shared.other_legacy.swap(0, Ordering::Relaxed),
            shader_native: self.shared.shader_native.swap(0, Ordering::Relaxed),
            shader_legacy: self.shared.shader_legacy.swap(0, Ordering::Relaxed),
            alpha_blend_native: self.shared.alpha_blend_native.swap(0, Ordering::Relaxed),
            alpha_blend_legacy: self.shared.alpha_blend_legacy.swap(0, Ordering::Relaxed),
            alpha_test_native: self.shared.alpha_test_native.swap(0, Ordering::Relaxed),
            alpha_test_legacy: self.shared.alpha_test_legacy.swap(0, Ordering::Relaxed),
            cull_native: self.shared.cull_native.swap(0, Ordering::Relaxed),
            cull_legacy: self.shared.cull_legacy.swap(0, Ordering::Relaxed),
            layout_native: self.shared.layout_native.swap(0, Ordering::Relaxed),
            layout_legacy: self.shared.layout_legacy.swap(0, Ordering::Relaxed),
            combiner_native: self.shared.combiner_native.swap(0, Ordering::Relaxed),
            combiner_legacy: self.shared.combiner_legacy.swap(0, Ordering::Relaxed),
            depth_native: self.shared.depth_native.swap(0, Ordering::Relaxed),
            depth_legacy: self.shared.depth_legacy.swap(0, Ordering::Relaxed),
            stencil_native: self.shared.stencil_native.swap(0, Ordering::Relaxed),
            stencil_legacy: self.shared.stencil_legacy.swap(0, Ordering::Relaxed),
            rejected: self.shared.rejected.swap(0, Ordering::Relaxed),
            encode_cpu_us_total: self.shared.encode_cpu_us.swap(0, Ordering::Relaxed),
            encode_cpu_us_max: self.shared.encode_cpu_us_max.swap(0, Ordering::Relaxed),
            encode_samples: self.shared.encode_samples.swap(0, Ordering::Relaxed),
        }
    }

    /// Attach copied draw observations to the next owner frame submission.
    pub fn submit_room_draw_packets(&self, packets: Vec<RoomDrawPacket>) {
        *self
            .shared
            .pending_draw_packets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = packets;
    }

    /// Clear a sampled comparison frame when the guest did not publish a new
    /// room collection. Replaying the previous packet over unrelated frames
    /// can draw stale geometry on movie/UI content.
    pub fn clear_frame(&self) {
        self.shared
            .pending_draw_packets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        let mut guard = self
            .shared
            .packet
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = None;
        self.shared.has_packet.store(false, Ordering::Release);
    }
}

impl NativeSceneWgpuSink {
    fn submit_frame(&mut self, frame: &PreparedSceneFrame, lease: Option<PreparedSceneLease>) {
        let Some(plan) = frame.room_plan.as_ref() else {
            self.shared.rejected.fetch_add(1, Ordering::Relaxed);
            self.clear_frame();
            return;
        };
        let valid = plan.tick == frame.tick
            && (-1..80).contains(&plan.draw_max_depth)
            && plan.rooms.len() <= MAX_ROOMS
            && plan
                .rooms
                .iter()
                .enumerate()
                .all(|(i, room)| (1..=200).contains(room) && !plan.rooms[..i].contains(room))
            && frame.observed_objects.len() <= MAX_OBJECTS
            && frame.observed_objects.iter().enumerate().all(|(i, id)| {
                id.generation != 0
                    && !frame.observed_objects[..i].contains(id)
                    && !frame.observed_objects[..i]
                        .iter()
                        .any(|prior| prior.slot == id.slot)
            })
            && frame.observed_views.len() <= MAX_VIEWS
            && frame.observed_views.iter().enumerate().all(|(i, view)| {
                view.viewer < 16
                    && !frame.observed_views[..i]
                        .iter()
                        .any(|prior| prior.viewer == view.viewer)
                    && view
                        .position
                        .iter()
                        .chain(view.matrix.iter())
                        .chain(view.viewport.iter())
                        .all(|value| value.is_finite())
            })
            && scene_lights_match_frame(frame);
        if !valid {
            self.shared.rejected.fetch_add(1, Ordering::Relaxed);
            self.clear_frame();
            return;
        }
        let draw_packets = std::mem::take(
            &mut *self
                .shared
                .pending_draw_packets
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        let mut packet = ScenePacket {
            objects: frame.observed_objects.clone(),
            views: frame.observed_views.clone(),
            draw_packets,
            scene_lights: lease
                .is_none()
                .then(|| frame.scene_lights.clone())
                .flatten(),
            gpu_scene_lights: Vec::new(),
            frame_lease: lease,
        };
        packet.gpu_scene_lights = packet
            .scene_lights()
            .map(|lights| {
                lights
                    .blocks
                    .iter()
                    .map(|block| GpuSceneLightEntry {
                        binding: SceneLightBinding::new(block.record, block.view),
                        payload: GpuSceneLightBlock::from(block),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let scene_light_blocks = packet.gpu_scene_lights.len() as u64;
        let mut guard = self
            .shared
            .packet
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(packet);
        self.shared.has_packet.store(true, Ordering::Release);
        self.shared.submitted.fetch_add(1, Ordering::Relaxed);
        self.shared
            .scene_light_blocks
            .fetch_add(scene_light_blocks, Ordering::Relaxed);
    }
}

impl SceneFrameSink for NativeSceneWgpuSink {
    fn submit(&mut self, frame: &PreparedSceneFrame) {
        self.submit_frame(frame, None);
    }
}

impl SceneFrameLeaseSink for NativeSceneWgpuSink {
    fn submit_lease(&mut self, lease: PreparedSceneLease) {
        let retained = lease.clone();
        self.submit_frame(lease.frame(), Some(retained));
    }
}

/// The owner publishes an immutable snapshot for one prepared frame. Check the
/// epoch and identities again at renderer ingress so malformed packets cannot
/// cross into the WGPU thread. The native scene comparison pass currently
/// retains this typed model but does not use it to shade geometry.
fn scene_lights_match_frame(frame: &PreparedSceneFrame) -> bool {
    let Some(lights) = frame.scene_lights.as_ref() else {
        return true;
    };
    let Some(epoch) = frame.resource_epoch else {
        return false;
    };
    if lights.epoch != epoch || lights.blocks.iter().any(|block| block.epoch != epoch) {
        return false;
    }
    let mut identities = BTreeSet::new();
    lights
        .blocks
        .iter()
        .all(|block| identities.insert(SceneLightBinding::new(block.record, block.view)))
}

struct NativeSceneLayer {
    shared: Arc<Shared>,
    pipeline: Option<wgpu::RenderPipeline>,
    room_pipelines: HashMap<RoomPipelineKey, wgpu::RenderPipeline>,
    room_light_preview_pipelines:
        HashMap<(RoomPipelineKey, SceneLightPreviewSource), wgpu::RenderPipeline>,
    scene_light_bind_group_layout: Option<wgpu::BindGroupLayout>,
    textured_pipeline: Option<TexturedPipeline>,
    shader_pipelines: HashMap<RoomCullMode, wgpu::RenderPipeline>,
    shader_programs: HashMap<u64, ShaderComputePipeline>,
    packet_filter: PacketFilter,
    rendered_present: AtomicU64,
}

struct ShaderComputePipeline {
    bind_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::ComputePipeline,
}

struct EncodedShaderDraw {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
    scissor: Option<[u32; 4]>,
    cull: RoomCullMode,
}

#[derive(Clone, Copy)]
enum NativeRoomDrawOrder {
    Flat(usize),
    Textured(usize),
    Shader(usize),
}

struct TexturedPipeline {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: HashMap<TexturedPipelineKey, wgpu::RenderPipeline>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct TexturedPipelineKey {
    state: RoomPipelineKey,
    blend: Option<RoomBlendState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RoomBlendState {
    source: RoomBlendFactor,
    destination: RoomBlendFactor,
    operation: RoomBlendOperation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RoomBlendFactor {
    Zero,
    One,
    SourceColour,
    OneMinusSourceColour,
    SourceAlpha,
    OneMinusSourceAlpha,
    DestinationAlpha,
    OneMinusDestinationAlpha,
    DestinationColour,
    OneMinusDestinationColour,
    SourceAlphaSaturated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RoomBlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

struct TexturedRoomDraw {
    vertices: Vec<[f32; 9]>,
    indices: Vec<u32>,
    texture: crate::texture::Decoded,
    address_u: wgpu::AddressMode,
    address_v: wgpu::AddressMode,
    mag_filter: wgpu::FilterMode,
    min_filter: wgpu::FilterMode,
    mip_filter: wgpu::FilterMode,
    mip_levels: u32,
    scissor: Option<[u32; 4]>,
    cull: RoomCullMode,
    depth: RoomDepthState,
    stencil: RoomStencilState,
    stencil_reference: u32,
    alpha_test: bool,
    alpha_function: u32,
    alpha_reference: u32,
    blend: Option<RoomBlendState>,
    combiner_scale: f32,
}

struct EncodedTexturedDraw {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    index_count: u32,
    scissor: Option<[u32; 4]>,
    pipeline: TexturedPipelineKey,
    stencil_reference: u32,
}

#[derive(Clone)]
struct IndexedDrawRange {
    indices: Range<u32>,
    vertices: Range<u32>,
    scene_light_block_index: Option<u32>,
    scissor: Option<[u32; 4]>,
    cull: RoomCullMode,
    depth: RoomDepthState,
    stencil: RoomStencilState,
    stencil_reference: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
struct RoomDepthState {
    enabled: bool,
    write: bool,
    compare: RoomDepthCompare,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum RoomDepthCompare {
    Never,
    Less,
    Equal,
    #[default]
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RoomPipelineKey {
    cull: RoomCullMode,
    depth: RoomDepthState,
    stencil: RoomStencilState,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
struct RoomStencilState {
    enabled: bool,
    compare: RoomDepthCompare,
    fail: RoomStencilOperation,
    depth_fail: RoomStencilOperation,
    pass: RoomStencilOperation,
    read_mask: u32,
    write_mask: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum RoomStencilOperation {
    #[default]
    Keep,
    Zero,
    Replace,
    IncrementClamp,
    DecrementClamp,
    Invert,
    IncrementWrap,
    DecrementWrap,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RoomVertex {
    position: [f32; 3],
    colour: [f32; 4],
    scene_light_block_index: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum RoomCullMode {
    #[default]
    None,
    Clockwise,
    CounterClockwise,
}

fn native_cull_mode(value: u32) -> Option<RoomCullMode> {
    Some(match value {
        0 | 1 => RoomCullMode::None,
        2 => RoomCullMode::Clockwise,
        3 => RoomCullMode::CounterClockwise,
        _ => return None,
    })
}

fn native_depth_state(packet: &RoomDrawPacket) -> Option<RoomDepthState> {
    let enabled = match packet.depth_enable {
        0 => false,
        1 => true,
        _ => return None,
    };
    let write = match packet.depth_write {
        0 => false,
        1 => true,
        _ => return None,
    };
    let compare = match packet.depth_function {
        1 => RoomDepthCompare::Never,
        2 => RoomDepthCompare::Less,
        3 => RoomDepthCompare::Equal,
        4 => RoomDepthCompare::LessEqual,
        5 => RoomDepthCompare::Greater,
        6 => RoomDepthCompare::NotEqual,
        7 => RoomDepthCompare::GreaterEqual,
        8 => RoomDepthCompare::Always,
        _ if !enabled => RoomDepthCompare::LessEqual,
        _ => return None,
    };
    Some(RoomDepthState {
        enabled,
        write: enabled && write,
        compare,
    })
}

fn native_stencil_state(packet: &RoomDrawPacket) -> Option<RoomStencilState> {
    if packet.stencil_enable == 0 {
        return Some(RoomStencilState::default());
    }
    if packet.stencil_enable != 1 {
        return None;
    }
    let compare = match packet.stencil_function {
        1 => RoomDepthCompare::Never,
        2 => RoomDepthCompare::Less,
        3 => RoomDepthCompare::Equal,
        4 => RoomDepthCompare::LessEqual,
        5 => RoomDepthCompare::Greater,
        6 => RoomDepthCompare::NotEqual,
        7 => RoomDepthCompare::GreaterEqual,
        8 => RoomDepthCompare::Always,
        _ => return None,
    };
    let operation = |value| {
        Some(match value {
            1 => RoomStencilOperation::Keep,
            2 => RoomStencilOperation::Zero,
            3 => RoomStencilOperation::Replace,
            4 => RoomStencilOperation::IncrementClamp,
            5 => RoomStencilOperation::DecrementClamp,
            6 => RoomStencilOperation::Invert,
            7 => RoomStencilOperation::IncrementWrap,
            8 => RoomStencilOperation::DecrementWrap,
            _ => return None,
        })
    };
    Some(RoomStencilState {
        enabled: true,
        compare,
        fail: operation(packet.stencil_fail)?,
        depth_fail: operation(packet.stencil_depth_fail)?,
        pass: operation(packet.stencil_pass)?,
        read_mask: packet.stencil_mask,
        write_mask: packet.stencil_write_mask,
    })
}

impl crate::frame_layer::FrameLayer for NativeSceneLayer {
    fn encode(&mut self, target: crate::frame_layer::LayerTarget<'_>) -> bool {
        if !self.shared.has_packet.load(Ordering::Acquire) {
            return false;
        }
        let _timer = EncodeTimer::new(&self.shared);
        let packet = self
            .shared
            .packet
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let Some(packet) = packet else {
            return false;
        };
        if packet.draw_packets.is_empty() {
            return false;
        }
        let extent = target.target.size();
        if extent.width == 0 || extent.height == 0 {
            return false;
        }
        if let Some(lease) = packet.frame_lease.as_ref() {
            self.shared.retain_scene_lease(lease.clone());
        }

        let mut vertices: Vec<[f32; 6]> = Vec::new();
        let view_rect = packet
            .views
            .first()
            .map(|view| view.rectangle)
            .map(|rect| clamp_rect(rect, extent.width, extent.height))
            .unwrap_or([0.0, 0.0, extent.width as f32, extent.height as f32]);

        let view_start = vertices.len() as u32;
        if self.packet_filter == PacketFilter::All {
            for view in &packet.views {
                let rect = clamp_rect(view.rectangle, extent.width, extent.height);
                add_rect_outline(
                    &mut vertices,
                    rect,
                    [0.05, 0.95, 0.9, 0.78],
                    extent.width,
                    extent.height,
                );
            }
        }
        let view_range = range_since(view_start, &vertices);

        let mut native_room_vertices = Vec::<RoomVertex>::new();
        let mut native_indices = Vec::<u32>::new();
        let mut native_ranges = Vec::new();
        let mut textured_draws = Vec::new();
        let mut shader_draws = Vec::new();
        let mut room_draw_order = Vec::new();
        let mut native_packet_count = 0u64;
        let mut flat_native = 0u64;
        let mut flat_legacy = 0u64;
        let mut textured_native = 0u64;
        let mut textured_legacy = 0u64;
        let mut other_legacy = 0u64;
        let mut shader_native = 0u64;
        let mut shader_legacy = 0u64;
        let mut alpha_blend_native = 0u64;
        let mut alpha_blend_legacy = 0u64;
        let mut alpha_test_native = 0u64;
        let mut alpha_test_legacy = 0u64;
        let mut cull_native = 0u64;
        let mut cull_legacy = 0u64;
        let mut layout_native = 0u64;
        let mut layout_legacy = 0u64;
        let mut combiner_native = 0u64;
        let mut combiner_legacy = 0u64;
        let mut depth_native = 0u64;
        let mut depth_legacy = 0u64;
        let mut stencil_native = 0u64;
        let mut stencil_legacy = 0u64;
        let mut scene_light_draw_matches = 0u64;
        let mut scene_light_upload = GpuSceneLightUpload::default();
        for (draw_index, draw) in packet.draw_packets.iter().enumerate() {
            if !self.packet_filter.accepts(draw) {
                continue;
            }
            let native_draws_before = native_packet_count;
            let depth_native_before = depth_native;
            let stencil_native_before = stencil_native;
            let mut flat_range_index = None;
            if let Some(range) = append_simple_room_draw(
                &mut native_room_vertices,
                &mut native_indices,
                draw,
                extent.width,
                extent.height,
            ) {
                native_packet_count += 1;
                flat_native += 1;
                cull_native += (range.cull != RoomCullMode::None) as u64;
                depth_native += range.depth.enabled as u64;
                stencil_native += range.stencil.enabled as u64;
                layout_native += 1;
                let index = native_ranges.len();
                native_ranges.push(range);
                flat_range_index = Some(index);
                room_draw_order.push(NativeRoomDrawOrder::Flat(index));
            } else if let Some(textured) =
                append_textured_room_draw(draw, extent.width, extent.height)
            {
                native_packet_count += 1;
                textured_native += 1;
                cull_native += (textured.cull != RoomCullMode::None) as u64;
                alpha_blend_native += textured.blend.is_some() as u64;
                alpha_test_native += textured.alpha_test as u64;
                combiner_native += 1;
                let index = textured_draws.len();
                textured_draws.push(textured);
                room_draw_order.push(NativeRoomDrawOrder::Textured(index));
            } else if let Ok(shader) = append_shader_room_draw(
                draw,
                extent.width,
                extent.height,
                target.device,
                target.encoder,
                &mut self.shader_programs,
            ) {
                native_packet_count += 1;
                shader_native += 1;
                cull_native += (shader.cull != RoomCullMode::None) as u64;
                layout_native += 1;
                let index = shader_draws.len();
                shader_draws.push(shader);
                room_draw_order.push(NativeRoomDrawOrder::Shader(index));
            } else if packet_material_class(draw) == RoomMaterialClass::FlatVertexColour {
                flat_legacy += 1;
                cull_legacy += native_cull_mode(draw.cull_mode)
                    .is_some_and(|mode| mode != RoomCullMode::None)
                    as u64;
                layout_legacy += 1;
                alpha_blend_legacy += (draw.blend_enable != 0) as u64;
                alpha_test_legacy += (draw.render_state[15] != 0) as u64;
            } else if packet_material_class(draw) == RoomMaterialClass::TexturedModulate {
                textured_legacy += 1;
                cull_legacy += (draw.cull_mode > 1) as u64;
                layout_legacy += 1;
                combiner_legacy += 1;
                alpha_blend_legacy += (draw.blend_enable != 0) as u64;
                alpha_test_legacy += (draw.render_state[15] != 0) as u64;
            } else if packet_material_class(draw) == RoomMaterialClass::VertexShader {
                shader_legacy += 1;
                cull_legacy += (draw.cull_mode > 1) as u64;
                layout_legacy += 1;
                alpha_blend_legacy += (draw.blend_enable != 0) as u64;
                alpha_test_legacy += (draw.render_state[15] != 0) as u64;
            } else {
                other_legacy += 1;
                cull_legacy += (draw.cull_mode > 1) as u64;
                layout_legacy += 1;
                alpha_blend_legacy += (draw.blend_enable != 0) as u64;
                alpha_test_legacy += (draw.render_state[15] != 0) as u64;
            }
            if draw.depth_enable != 0 && depth_native == depth_native_before {
                depth_legacy += 1;
            }
            if draw.stencil_enable != 0 && stencil_native == stencil_native_before {
                stencil_legacy += 1;
            }
            if native_packet_count != native_draws_before {
                scene_light_draw_matches +=
                    native_draw_has_scene_light_match(&packet, draw.scene_light_binding, true)
                        as u64;
                let block_index =
                    scene_light_upload.bind_draw(&packet, draw_index, draw.scene_light_binding);
                if let (Some(range_index), Some(block_index)) = (flat_range_index, block_index) {
                    let range = &mut native_ranges[range_index];
                    range.scene_light_block_index = Some(block_index);
                    for vertex_index in range.vertices.clone() {
                        native_room_vertices[vertex_index as usize].scene_light_block_index =
                            block_index;
                    }
                }
            }
        }
        let legacy_packet_count = flat_legacy + textured_legacy + other_legacy + shader_legacy;

        let object_start = vertices.len() as u32;
        if self.packet_filter == PacketFilter::All {
            add_object_markers(
                &mut vertices,
                &packet.objects,
                view_rect,
                extent.width,
                extent.height,
            );
        }
        let object_range = range_since(object_start, &vertices);
        if vertices.is_empty()
            && native_room_vertices.is_empty()
            && textured_draws.is_empty()
            && shader_draws.is_empty()
        {
            return false;
        }

        let scene_light_preview_enabled = self
            .shared
            .scene_light_preview_enabled
            .load(Ordering::Acquire);
        let scene_light_preview_source = SceneLightPreviewSource::from_raw(
            self.shared
                .scene_light_preview_source
                .load(Ordering::Acquire),
        );
        let mut scene_light_preview_bind_group = None;
        if scene_light_preview_enabled && !scene_light_upload.blocks.is_empty() {
            if self.scene_light_bind_group_layout.is_none() {
                self.scene_light_bind_group_layout =
                    Some(create_scene_light_bind_group_layout(target.device));
            }
        }
        if !scene_light_upload.blocks.is_empty() {
            let light_bytes = bytemuck::cast_slice(&scene_light_upload.blocks);
            let limits = target.device.limits();
            if light_bytes.len() as u64 <= limits.max_buffer_size
                && light_bytes.len() as u64 <= limits.max_storage_buffer_binding_size as u64
            {
                if let Some(epoch) = packet.scene_lights().map(|lights| lights.epoch) {
                    let block_count = scene_light_upload.blocks.len() as u64;
                    let draw_binding_count = scene_light_upload.draw_bindings.len() as u64;
                    let buffer =
                        target
                            .device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("T3403 native scene-light storage blocks"),
                                contents: light_bytes,
                                usage: wgpu::BufferUsages::STORAGE,
                            });
                    let debug_bind_group = self
                        .scene_light_bind_group_layout
                        .as_ref()
                        .filter(|_| scene_light_preview_enabled)
                        .map(|layout| {
                            Arc::new(target.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("T3403 scene-light diagnostic shader input"),
                                layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: buffer.as_entire_binding(),
                                }],
                            }))
                        });
                    if let Some(frame) =
                        GpuSceneLightFrame::new(buffer, debug_bind_group, epoch, scene_light_upload)
                    {
                        scene_light_preview_bind_group = frame.debug_bind_group.clone();
                        self.shared.retain_scene_light_frame(frame);
                        self.shared
                            .scene_light_gpu_buffers
                            .fetch_add(1, Ordering::Relaxed);
                        self.shared
                            .scene_light_gpu_blocks
                            .fetch_add(block_count, Ordering::Relaxed);
                        self.shared
                            .scene_light_gpu_draw_bindings
                            .fetch_add(draw_binding_count, Ordering::Relaxed);
                    }
                }
            }
        }

        if self.pipeline.is_none() {
            self.pipeline = Some(create_pipeline(target.device));
        }
        for range in &native_ranges {
            let key = RoomPipelineKey {
                cull: range.cull,
                depth: range.depth,
                stencil: range.stencil,
            };
            self.room_pipelines
                .entry(key)
                .or_insert_with(|| create_room_pipeline(target.device, key));
            if scene_light_preview_bind_group.is_some() && range.scene_light_block_index.is_some() {
                let layout = self
                    .scene_light_bind_group_layout
                    .as_ref()
                    .expect("preview layout was created for scene-light frame");
                self.room_light_preview_pipelines
                    .entry((key, scene_light_preview_source))
                    .or_insert_with(|| {
                        create_room_light_preview_pipeline(
                            target.device,
                            key,
                            layout,
                            scene_light_preview_source,
                        )
                    });
            }
        }
        let mut encoded_textured = Vec::with_capacity(textured_draws.len());
        if !textured_draws.is_empty() {
            let pipeline = self
                .textured_pipeline
                .get_or_insert_with(|| create_textured_pipeline(target.device));
            for draw in &textured_draws {
                let key = TexturedPipelineKey {
                    state: RoomPipelineKey {
                        cull: draw.cull,
                        depth: draw.depth,
                        stencil: draw.stencil,
                    },
                    blend: draw.blend,
                };
                if !pipeline.pipelines.contains_key(&key) {
                    let native = create_textured_room_pipeline(target.device, pipeline, key);
                    pipeline.pipelines.insert(key, native);
                }
                encoded_textured.push(encode_textured_draw(
                    target.device,
                    target.queue,
                    &pipeline.layout,
                    key,
                    draw.stencil_reference,
                    draw,
                ));
            }
        }
        let vertex_buffer = (!vertices.is_empty()).then(|| {
            target
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("T2238 native room packet and diagnostic vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let index_buffer = (!native_indices.is_empty()).then(|| {
            target
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("T2238 captured room packet indices"),
                    contents: bytemuck::cast_slice(&native_indices),
                    usage: wgpu::BufferUsages::INDEX,
                })
        });
        let native_room_vertex_buffer = (!native_room_vertices.is_empty()).then(|| {
            target
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("T2238 captured room vertices with depth"),
                    contents: bytemuck::cast_slice(&native_room_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let view = target
            .target
            .create_view(&wgpu::TextureViewDescriptor::default());
        let needs_room_depth = !room_draw_order.is_empty();
        let depth_texture = needs_room_depth.then(|| {
            target.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("T2238 captured room depth and stencil attachment"),
                size: wgpu::Extent3d {
                    width: extent.width,
                    height: extent.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        });
        let depth_view = depth_texture
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
        if let Some(vertex_buffer) = vertex_buffer.as_ref() {
            let mut pass = target
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("T2238 native scene comparison overlay"),
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
            pass.set_pipeline(self.pipeline.as_ref().expect("pipeline initialized"));
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            if let Some(index_buffer) = index_buffer.as_ref() {
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            }
            if view_range.start < view_range.end {
                pass.draw(view_range.clone(), 0..1);
            }
            if object_range.start < object_range.end {
                pass.draw(object_range.clone(), 0..1);
            }
        }
        if !shader_draws.is_empty() {
            for draw in &shader_draws {
                self.shader_pipelines
                    .entry(draw.cull)
                    .or_insert_with(|| create_shader_render_pipeline(target.device, draw.cull));
            }
        }
        if !room_draw_order.is_empty() {
            let textured_pipeline = self.textured_pipeline.as_ref();
            let mut pass = target
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("T2238 native ordered room packet pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view.as_ref().expect("room depth attachment exists"),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0),
                            store: wgpu::StoreOp::Store,
                        }),
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            for ordered in &room_draw_order {
                match *ordered {
                    NativeRoomDrawOrder::Flat(index) => {
                        let range = &native_ranges[index];
                        let key = RoomPipelineKey {
                            cull: range.cull,
                            depth: range.depth,
                            stencil: range.stencil,
                        };
                        if range.scene_light_block_index.is_some()
                            && scene_light_preview_bind_group.is_some()
                        {
                            pass.set_pipeline(
                                self.room_light_preview_pipelines
                                    .get(&(key, scene_light_preview_source))
                                    .expect("scene-light preview pipeline initialized"),
                            );
                            pass.set_bind_group(
                                0,
                                scene_light_preview_bind_group
                                    .as_deref()
                                    .expect("scene-light preview bind group exists"),
                                &[],
                            );
                        } else {
                            pass.set_pipeline(
                                self.room_pipelines
                                    .get(&key)
                                    .expect("room cull/depth pipeline initialized"),
                            );
                        }
                        pass.set_vertex_buffer(
                            0,
                            native_room_vertex_buffer
                                .as_ref()
                                .expect("room vertex buffer exists")
                                .slice(..),
                        );
                        pass.set_index_buffer(
                            index_buffer
                                .as_ref()
                                .expect("room index buffer exists")
                                .slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        if let Some([x, y, width, height]) = range.scissor {
                            pass.set_scissor_rect(x, y, width, height);
                        } else {
                            pass.set_scissor_rect(0, 0, extent.width, extent.height);
                        }
                        pass.set_stencil_reference(range.stencil_reference);
                        pass.draw_indexed(range.indices.clone(), 0, 0..1);
                    }
                    NativeRoomDrawOrder::Textured(index) => {
                        let draw = &encoded_textured[index];
                        let pipeline = textured_pipeline.expect("textured pipeline initialized");
                        pass.set_pipeline(
                            pipeline
                                .pipelines
                                .get(&draw.pipeline)
                                .expect("textured material/state pipeline initialized"),
                        );
                        pass.set_vertex_buffer(0, draw.vertex.slice(..));
                        pass.set_index_buffer(draw.index.slice(..), wgpu::IndexFormat::Uint32);
                        pass.set_bind_group(0, &draw.bind_group, &[]);
                        if let Some([x, y, width, height]) = draw.scissor {
                            pass.set_scissor_rect(x, y, width, height);
                        } else {
                            pass.set_scissor_rect(0, 0, extent.width, extent.height);
                        }
                        pass.set_stencil_reference(draw.stencil_reference);
                        pass.draw_indexed(0..draw.index_count, 0, 0..1);
                    }
                    NativeRoomDrawOrder::Shader(index) => {
                        let draw = &shader_draws[index];
                        pass.set_pipeline(
                            self.shader_pipelines
                                .get(&draw.cull)
                                .expect("shader cull pipeline initialized"),
                        );
                        pass.set_vertex_buffer(0, draw.vertex.slice(..));
                        pass.set_index_buffer(draw.index.slice(..), wgpu::IndexFormat::Uint32);
                        if let Some([x, y, width, height]) = draw.scissor {
                            pass.set_scissor_rect(x, y, width, height);
                        } else {
                            pass.set_scissor_rect(0, 0, extent.width, extent.height);
                        }
                        pass.draw_indexed(0..draw.index_count, 0, 0..1);
                    }
                }
            }
        }

        self.shared.encoded.fetch_add(1, Ordering::Relaxed);
        self.shared.room_draws.fetch_add(
            (native_ranges.len() + textured_draws.len() + shader_draws.len()) as u64,
            Ordering::Relaxed,
        );
        self.shared
            .native_packet_draws
            .fetch_add(native_packet_count, Ordering::Relaxed);
        self.shared
            .scene_light_draw_matches
            .fetch_add(scene_light_draw_matches, Ordering::Relaxed);
        self.shared
            .legacy_packet_draws
            .fetch_add(legacy_packet_count, Ordering::Relaxed);
        self.shared
            .flat_native
            .fetch_add(flat_native, Ordering::Relaxed);
        self.shared
            .flat_legacy
            .fetch_add(flat_legacy, Ordering::Relaxed);
        self.shared
            .textured_native
            .fetch_add(textured_native, Ordering::Relaxed);
        self.shared
            .textured_legacy
            .fetch_add(textured_legacy, Ordering::Relaxed);
        self.shared
            .other_legacy
            .fetch_add(other_legacy, Ordering::Relaxed);
        self.shared
            .shader_native
            .fetch_add(shader_native, Ordering::Relaxed);
        self.shared
            .shader_legacy
            .fetch_add(shader_legacy, Ordering::Relaxed);
        self.shared
            .alpha_blend_native
            .fetch_add(alpha_blend_native, Ordering::Relaxed);
        self.shared
            .alpha_blend_legacy
            .fetch_add(alpha_blend_legacy, Ordering::Relaxed);
        self.shared
            .alpha_test_native
            .fetch_add(alpha_test_native, Ordering::Relaxed);
        self.shared
            .alpha_test_legacy
            .fetch_add(alpha_test_legacy, Ordering::Relaxed);
        self.shared
            .cull_native
            .fetch_add(cull_native, Ordering::Relaxed);
        self.shared
            .cull_legacy
            .fetch_add(cull_legacy, Ordering::Relaxed);
        self.shared
            .layout_native
            .fetch_add(layout_native, Ordering::Relaxed);
        self.shared
            .layout_legacy
            .fetch_add(layout_legacy, Ordering::Relaxed);
        self.shared
            .combiner_native
            .fetch_add(combiner_native, Ordering::Relaxed);
        self.shared
            .combiner_legacy
            .fetch_add(combiner_legacy, Ordering::Relaxed);
        self.shared
            .depth_native
            .fetch_add(depth_native, Ordering::Relaxed);
        self.shared
            .depth_legacy
            .fetch_add(depth_legacy, Ordering::Relaxed);
        self.shared
            .stencil_native
            .fetch_add(stencil_native, Ordering::Relaxed);
        self.shared
            .stencil_legacy
            .fetch_add(stencil_legacy, Ordering::Relaxed);
        self.shared.object_draws.fetch_add(
            if object_range.start < object_range.end {
                packet.objects.len() as u64
            } else {
                0
            },
            Ordering::Relaxed,
        );
        self.shared
            .view_draws
            .fetch_add(packet.views.len() as u64, Ordering::Relaxed);
        if target.present != 0 {
            self.rendered_present
                .store(target.present, Ordering::Release);
        }
        true
    }

    fn submitted(&mut self, queue: &wgpu::Queue) {
        let leases = self.shared.take_pending_scene_leases();
        let light_frames = self.shared.take_pending_scene_light_frames();
        if !leases.is_empty() || !light_frames.is_empty() {
            queue.on_submitted_work_done(move || drop((leases, light_frames)));
        }
    }

    fn wants_display(&self) -> bool {
        true
    }

    fn encode_display(&mut self, target: crate::frame_layer::LayerTarget<'_>) -> bool {
        // When the live renderer has already composed this guest present, its
        // clean scene copy contains the native layer. Skip exactly the first
        // display redraw for that present; later display redraws start from the
        // clean scene and need to apply the layer again.
        if target.present != 0
            && self
                .rendered_present
                .compare_exchange(target.present, 0, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            return false;
        }
        self.encode(target)
    }
}

fn create_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    create_pipeline_with_cull(device, RoomCullMode::None)
}

fn create_pipeline_with_cull(device: &wgpu::Device, cull: RoomCullMode) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("T2238 native scene and diagnostic shader"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) colour: vec4<f32>,
};
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
};
@vertex fn vs_main(input: VertexIn) -> VertexOut {
    var output: VertexOut;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.colour = input.colour;
    return output;
}

@fragment fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.colour;
}
"#
            .into(),
        ),
    });
    let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("T2238 native scene comparison pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 24,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }],
        },
        primitive: wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Cw,
            cull_mode: match cull {
                RoomCullMode::None => None,
                RoomCullMode::Clockwise => Some(wgpu::Face::Front),
                RoomCullMode::CounterClockwise => Some(wgpu::Face::Back),
            },
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}

fn create_room_pipeline(device: &wgpu::Device, key: RoomPipelineKey) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("T2238 native room draw with captured depth"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
};
@vertex fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) colour: vec4<f32>,
) -> VertexOut {
    var output: VertexOut;
    output.position = vec4<f32>(position, 1.0);
    output.colour = colour;
    return output;
}

@fragment fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.colour;
}
"#
            .into(),
        ),
    });
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
    let depth_stencil = room_depth_stencil_state(key);
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("T2238 captured room layout/cull/depth pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<RoomVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }],
        },
        primitive: wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Cw,
            cull_mode: match key.cull {
                RoomCullMode::None => None,
                RoomCullMode::Clockwise => Some(wgpu::Face::Front),
                RoomCullMode::CounterClockwise => Some(wgpu::Face::Back),
            },
            ..Default::default()
        },
        depth_stencil: Some(depth_stencil),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}

fn create_scene_light_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("T3403 scene-light diagnostic storage layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(std::mem::size_of::<GpuSceneLightBlock>() as u64),
            },
            count: None,
        }],
    })
}

const SCENE_LIGHT_PREVIEW_WGSL: &str = r#"
struct SceneLightSlot {
    position: vec4<f32>,
    color: vec4<f32>,
    parameters: vec3<f32>,
    enabled: u32,
};
struct SceneLightBlock {
    slots: array<SceneLightSlot, 3>,
    fourth_color: vec4<f32>,
};
@group(0) @binding(0) var<storage, read> scene_light_blocks: array<SceneLightBlock>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
    @location(1) @interpolate(flat) light_block_index: u32,
};
@vertex fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) colour: vec4<f32>,
    @location(2) light_block_index: u32,
) -> VertexOut {
    var output: VertexOut;
    output.position = vec4<f32>(position, 1.0);
    output.colour = colour;
    output.light_block_index = light_block_index;
    return output;
}

fn selected_color(input: VertexOut, color: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), input.colour.a);
}

fn selected_enabled(input: VertexOut, enabled: u32) -> vec4<f32> {
    let channel = select(0.0, 1.0, enabled != 0u);
    return selected_color(input, vec3<f32>(channel));
}

@fragment fn fs_slot_0(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[0].color.rgb);
    }
    return input.colour;
}

@fragment fn fs_slot_1(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[1].color.rgb);
    }
    return input.colour;
}

@fragment fn fs_slot_2(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[2].color.rgb);
    }
    return input.colour;
}

@fragment fn fs_fourth_color(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].fourth_color.rgb);
    }
    return input.colour;
}

@fragment fn fs_slot_0_position(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[0].position.xyz);
    }
    return input.colour;
}

@fragment fn fs_slot_1_position(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[1].position.xyz);
    }
    return input.colour;
}

@fragment fn fs_slot_2_position(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[2].position.xyz);
    }
    return input.colour;
}

@fragment fn fs_slot_0_enabled(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_enabled(input, scene_light_blocks[input.light_block_index].slots[0].enabled);
    }
    return input.colour;
}

@fragment fn fs_slot_1_enabled(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_enabled(input, scene_light_blocks[input.light_block_index].slots[1].enabled);
    }
    return input.colour;
}

@fragment fn fs_slot_2_enabled(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_enabled(input, scene_light_blocks[input.light_block_index].slots[2].enabled);
    }
    return input.colour;
}

@fragment fn fs_slot_0_parameters(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[0].parameters);
    }
    return input.colour;
}

@fragment fn fs_slot_1_parameters(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[1].parameters);
    }
    return input.colour;
}

@fragment fn fs_slot_2_parameters(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        return selected_color(input, scene_light_blocks[input.light_block_index].slots[2].parameters);
    }
    return input.colour;
}

@fragment fn fs_enabled_slot_color_transport(input: VertexOut) -> @location(0) vec4<f32> {
    if (input.light_block_index < arrayLength(&scene_light_blocks)) {
        let block = scene_light_blocks[input.light_block_index];
        var color = vec3<f32>(0.0);
        for (var slot_index = 0u; slot_index < 3u; slot_index += 1u) {
            let slot = block.slots[slot_index];
            if (slot.enabled != 0u) {
                color += slot.color.rgb;
            }
        }
        return selected_color(input, color);
    }
    return input.colour;
}
"#;

fn create_room_light_preview_pipeline(
    device: &wgpu::Device,
    key: RoomPipelineKey,
    light_layout: &wgpu::BindGroupLayout,
    source: SceneLightPreviewSource,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("T3403 raw scene-light slot preview shader"),
        source: wgpu::ShaderSource::Wgsl(SCENE_LIGHT_PREVIEW_WGSL.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("T3403 scene-light preview pipeline layout"),
        bind_group_layouts: &[light_layout],
        push_constant_ranges: &[],
    });
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4, 2 => Uint32];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("T3403 opt-in scene-light raw slot preview"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<RoomVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }],
        },
        primitive: wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Cw,
            cull_mode: match key.cull {
                RoomCullMode::None => None,
                RoomCullMode::Clockwise => Some(wgpu::Face::Front),
                RoomCullMode::CounterClockwise => Some(wgpu::Face::Back),
            },
            ..Default::default()
        },
        depth_stencil: Some(room_depth_stencil_state(key)),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(source.fragment_entry_point()),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}

fn room_depth_stencil_state(key: RoomPipelineKey) -> wgpu::DepthStencilState {
    let compare = match key.depth.compare {
        RoomDepthCompare::Never => wgpu::CompareFunction::Never,
        RoomDepthCompare::Less => wgpu::CompareFunction::Less,
        RoomDepthCompare::Equal => wgpu::CompareFunction::Equal,
        RoomDepthCompare::LessEqual => wgpu::CompareFunction::LessEqual,
        RoomDepthCompare::Greater => wgpu::CompareFunction::Greater,
        RoomDepthCompare::NotEqual => wgpu::CompareFunction::NotEqual,
        RoomDepthCompare::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        RoomDepthCompare::Always => wgpu::CompareFunction::Always,
    };
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        depth_write_enabled: key.depth.write,
        depth_compare: if key.depth.enabled {
            compare
        } else {
            wgpu::CompareFunction::Always
        },
        stencil: wgpu::StencilState {
            front: stencil_face_state(key.stencil),
            back: stencil_face_state(key.stencil),
            read_mask: key.stencil.read_mask,
            write_mask: key.stencil.write_mask,
        },
        bias: Default::default(),
    }
}

fn stencil_face_state(state: RoomStencilState) -> wgpu::StencilFaceState {
    let operation = |operation| match operation {
        RoomStencilOperation::Keep => wgpu::StencilOperation::Keep,
        RoomStencilOperation::Zero => wgpu::StencilOperation::Zero,
        RoomStencilOperation::Replace => wgpu::StencilOperation::Replace,
        RoomStencilOperation::IncrementClamp => wgpu::StencilOperation::IncrementClamp,
        RoomStencilOperation::DecrementClamp => wgpu::StencilOperation::DecrementClamp,
        RoomStencilOperation::Invert => wgpu::StencilOperation::Invert,
        RoomStencilOperation::IncrementWrap => wgpu::StencilOperation::IncrementWrap,
        RoomStencilOperation::DecrementWrap => wgpu::StencilOperation::DecrementWrap,
    };
    let compare = match state.compare {
        RoomDepthCompare::Never => wgpu::CompareFunction::Never,
        RoomDepthCompare::Less => wgpu::CompareFunction::Less,
        RoomDepthCompare::Equal => wgpu::CompareFunction::Equal,
        RoomDepthCompare::LessEqual => wgpu::CompareFunction::LessEqual,
        RoomDepthCompare::Greater => wgpu::CompareFunction::Greater,
        RoomDepthCompare::NotEqual => wgpu::CompareFunction::NotEqual,
        RoomDepthCompare::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        RoomDepthCompare::Always => wgpu::CompareFunction::Always,
    };
    wgpu::StencilFaceState {
        compare: if state.enabled {
            compare
        } else {
            wgpu::CompareFunction::Always
        },
        fail_op: operation(state.fail),
        depth_fail_op: operation(state.depth_fail),
        pass_op: operation(state.pass),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativePixelDiff {
    pub changed_pixels: u64,
    pub max_channel_delta: u8,
    pub total_channel_delta: u64,
}

/// Compare straight RGBA8 pixels from the legacy and native draw outputs.
pub fn pixel_diff_rgba8(reference: &[u8], native: &[u8]) -> Option<NativePixelDiff> {
    if reference.len() != native.len() || reference.len() % 4 != 0 {
        return None;
    }
    let mut diff = NativePixelDiff::default();
    for (reference, native) in reference.chunks_exact(4).zip(native.chunks_exact(4)) {
        let mut maximum = 0;
        for (a, b) in reference.iter().zip(native) {
            let delta = a.abs_diff(*b);
            maximum = maximum.max(delta);
            diff.total_channel_delta += u64::from(delta);
        }
        if maximum != 0 {
            diff.changed_pixels += 1;
        }
        diff.max_channel_delta = diff.max_channel_delta.max(maximum);
    }
    Some(diff)
}

/// Decode the captured screen-space XYZ+diffuse layouts (FVF 0x9/16-byte and
/// FVF 0xD/24-byte with unused UVs) with the admitted cull/depth/stencil state.
fn append_simple_room_draw(
    vertices: &mut Vec<RoomVertex>,
    indices: &mut Vec<u32>,
    packet: &RoomDrawPacket,
    width: u32,
    height: u32,
) -> Option<IndexedDrawRange> {
    let cull = native_cull_mode(packet.cull_mode)?;
    let depth = native_depth_state(packet)?;
    let stencil = native_stencil_state(packet)?;
    if packet.payload_dropped
        || packet.indexed > 1
        || packet.count < 3
        || packet.vertex_shader_handle != 0
        || packet.vertex_program_bytes != 0
        || packet.vertex_constants_bytes != 0
        || packet.pixel_shader_bytes != 0
        || packet.texture_headers.iter().any(|texture| *texture != 0)
        || packet.blend_enable != 0
        || packet.render_state[15] != 0
        || !matches!(
            (packet.vertex_declaration, packet.vertex_stride),
            (9, 16) | (13, 24)
        )
        || !matches!(packet.texture_stage_state[1], 0 | 1)
        || !matches!(packet.texture_stage_state[4], 0 | 1)
        || packet.scissor_count > 1
        || (packet.scissor_count == 1 && packet.scissor_exclusive != 0)
        || width == 0
        || height == 0
    {
        return None;
    }
    if packet.viewport_target[0] != 0
        && (packet.viewport_target[0] != width || packet.viewport_target[1] != height)
    {
        return None;
    }
    if packet.viewport_words.iter().any(|word| *word != 0) {
        let viewport = packet.viewport_words.map(f32::from_bits);
        if viewport[0] != 0.0
            || viewport[1] != 0.0
            || viewport[2] != width as f32
            || viewport[3] != height as f32
        {
            return None;
        }
    }
    let scissor = if packet.scissor_count == 1 {
        let [left, top, right, bottom] = packet.scissor_words;
        if right <= left || bottom <= top || right > width || bottom > height {
            return None;
        }
        Some([left, top, right - left, bottom - top])
    } else {
        None
    };
    let stride = packet.vertex_stride as usize;
    let vertex_bytes = packet.vertex_bytes as usize;
    let index_bytes = packet.index_bytes as usize;
    let needed = vertex_bytes.checked_add(index_bytes)?;
    if packet.payload.len() < needed
        || vertex_bytes == 0
        || vertex_bytes % stride != 0
        || (packet.indexed == 1 && index_bytes != packet.count as usize * 2)
        || (packet.indexed == 0 && index_bytes != 0)
    {
        return None;
    }
    let vertex_count = vertex_bytes / stride;
    let mut decoded = Vec::with_capacity(vertex_count);
    for bytes in packet.payload[..vertex_bytes].chunks_exact(stride) {
        let vertex = crate::vertex::decode_vertex(bytes, packet.vertex_stride)?;
        if !vertex.position[..3].iter().all(|value| value.is_finite())
            || !vertex.colour.iter().all(|value| value.is_finite())
        {
            return None;
        }
        decoded.push(RoomVertex {
            position: [
                vertex.position[0] * (2.0 / width as f32) - 1.0,
                1.0 - vertex.position[1] * (2.0 / height as f32),
                vertex.position[2].clamp(0.0, 1.0),
            ],
            colour: vertex.colour,
            scene_light_block_index: u32::MAX,
        });
    }
    let draw_base = vertices.len() as u32;
    let local_indices = crate::vertex::triangulate(packet.primitive, packet.count)?;
    let first_index = indices.len() as u32;
    if packet.indexed == 0 {
        if vertex_count < packet.count as usize {
            return None;
        }
        indices.extend(local_indices.into_iter().map(|index| draw_base + index));
    } else {
        let raw_indices = packet.payload[vertex_bytes..needed].chunks_exact(2);
        let draw_indices: Vec<u32> = raw_indices
            .map(|bytes| u32::from(u16::from_le_bytes([bytes[0], bytes[1]])))
            .collect();
        let local: Option<Vec<u32>> = local_indices
            .into_iter()
            .map(|index| {
                let source = *draw_indices.get(index as usize)?;
                let relative = source.checked_sub(packet.vertex_first)?;
                (relative < vertex_count as u32).then_some(draw_base + relative)
            })
            .collect();
        indices.extend(local?);
    }
    vertices.extend(decoded);
    Some(IndexedDrawRange {
        indices: first_index..indices.len() as u32,
        vertices: draw_base..vertices.len() as u32,
        scene_light_block_index: None,
        scissor,
        cull,
        depth,
        stencil,
        stencil_reference: packet.stencil_reference,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoomMaterialClass {
    FlatVertexColour,
    TexturedModulate,
    VertexShader,
    Other,
}

#[derive(Clone, Copy)]
struct NativeAlphaState {
    test: bool,
    function: u32,
    reference: u32,
    blend: Option<RoomBlendState>,
}

fn native_blend_state_values(
    enabled: u32,
    source: u32,
    destination: u32,
    operation: u32,
) -> Option<Option<RoomBlendState>> {
    if enabled == 0 {
        return Some(None);
    }
    if enabled != 1 {
        return None;
    }
    let factor = |value| {
        Some(match value {
            1 => RoomBlendFactor::Zero,
            2 => RoomBlendFactor::One,
            3 => RoomBlendFactor::SourceColour,
            4 => RoomBlendFactor::OneMinusSourceColour,
            5 => RoomBlendFactor::SourceAlpha,
            6 => RoomBlendFactor::OneMinusSourceAlpha,
            7 => RoomBlendFactor::DestinationAlpha,
            8 => RoomBlendFactor::OneMinusDestinationAlpha,
            9 => RoomBlendFactor::DestinationColour,
            10 => RoomBlendFactor::OneMinusDestinationColour,
            11 => RoomBlendFactor::SourceAlphaSaturated,
            _ => return None,
        })
    };
    let operation = match operation {
        1 => RoomBlendOperation::Add,
        2 => RoomBlendOperation::Subtract,
        3 => RoomBlendOperation::ReverseSubtract,
        4 => RoomBlendOperation::Min,
        5 => RoomBlendOperation::Max,
        _ => return None,
    };
    Some(Some(RoomBlendState {
        source: factor(source)?,
        destination: factor(destination)?,
        operation,
    }))
}

fn native_blend_state(packet: &RoomDrawPacket) -> Option<Option<RoomBlendState>> {
    native_blend_state_values(
        packet.blend_enable,
        packet.blend_source,
        packet.blend_destination,
        packet.blend_operation,
    )
}

fn native_alpha_state(packet: &RoomDrawPacket) -> Option<NativeAlphaState> {
    let blend = native_blend_state(packet)?;
    let test = match packet.render_state[15] {
        0 => false,
        1 => true,
        _ => return None,
    };
    let function = packet.render_state[25];
    let reference = packet.render_state[24];
    if (test && !(1..=8).contains(&function)) || reference > 255 {
        return None;
    }
    Some(NativeAlphaState {
        test,
        function: if test { function } else { 8 },
        reference,
        blend,
    })
}

fn wgpu_room_blend(state: RoomBlendState) -> wgpu::BlendState {
    let factor = |value| match value {
        RoomBlendFactor::Zero => wgpu::BlendFactor::Zero,
        RoomBlendFactor::One => wgpu::BlendFactor::One,
        RoomBlendFactor::SourceColour => wgpu::BlendFactor::Src,
        RoomBlendFactor::OneMinusSourceColour => wgpu::BlendFactor::OneMinusSrc,
        RoomBlendFactor::SourceAlpha => wgpu::BlendFactor::SrcAlpha,
        RoomBlendFactor::OneMinusSourceAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        RoomBlendFactor::DestinationAlpha => wgpu::BlendFactor::DstAlpha,
        RoomBlendFactor::OneMinusDestinationAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        RoomBlendFactor::DestinationColour => wgpu::BlendFactor::Dst,
        RoomBlendFactor::OneMinusDestinationColour => wgpu::BlendFactor::OneMinusDst,
        RoomBlendFactor::SourceAlphaSaturated => wgpu::BlendFactor::SrcAlphaSaturated,
    };
    let operation = match state.operation {
        RoomBlendOperation::Add => wgpu::BlendOperation::Add,
        RoomBlendOperation::Subtract => wgpu::BlendOperation::Subtract,
        RoomBlendOperation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
        RoomBlendOperation::Min => wgpu::BlendOperation::Min,
        RoomBlendOperation::Max => wgpu::BlendOperation::Max,
    };
    let component = wgpu::BlendComponent {
        src_factor: factor(state.source),
        dst_factor: factor(state.destination),
        operation,
    };
    wgpu::BlendState {
        color: component,
        alpha: component,
    }
}

fn packet_material_class(packet: &RoomDrawPacket) -> RoomMaterialClass {
    if packet.vertex_shader_handle != 0 && packet.vertex_program_bytes != 0 {
        RoomMaterialClass::VertexShader
    } else if packet.texture_headers.iter().all(|texture| *texture == 0)
        && packet.pixel_shader_bytes == 0
        && matches!(
            (packet.vertex_declaration, packet.vertex_stride),
            (9, 16) | (13, 24)
        )
    {
        RoomMaterialClass::FlatVertexColour
    } else if packet.texture_headers[0] != 0
        && packet.texture_headers[1..]
            .iter()
            .all(|texture| *texture == 0)
        && packet.pixel_shader_bytes == 240
        && packet.vertex_stride == 24
    {
        RoomMaterialClass::TexturedModulate
    } else {
        RoomMaterialClass::Other
    }
}

/// Submit one captured NV2A program through the existing WGSL translator and
/// compute path. The current bounded input contract is the common FVF 0xD
/// layout: NV2A v1=position, v2=diffuse, v3=UV. Unknown declarations,
/// registers, combiner state, or incomplete captures remain on the replayer.
fn append_shader_room_draw(
    packet: &RoomDrawPacket,
    width: u32,
    height: u32,
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    programs: &mut HashMap<u64, ShaderComputePipeline>,
) -> Result<EncodedShaderDraw, &'static str> {
    let cull = native_cull_mode(packet.cull_mode).ok_or("unsupported cull mode")?;
    if packet_material_class(packet) != RoomMaterialClass::VertexShader
        || packet.payload_dropped
        || packet.indexed > 1
        || packet.count < 3
        || packet.vertex_declaration != 13
        || packet.vertex_stride != 24
        || packet.pixel_shader_bytes != 0
        || packet.vertex_shader_handle == 0
        || packet.vertex_program_bytes != 136 * 16
        || packet.vertex_constants_bytes != 192 * 16
        || packet.blend_enable != 0
        || packet.depth_enable != 0
        || packet.stencil_enable != 0
        || packet.render_state[15] != 0
        || packet.scissor_count > 1
        || (packet.scissor_count == 1 && packet.scissor_exclusive != 0)
        || packet.texture_headers.iter().any(|header| *header != 0)
        || width == 0
        || height == 0
    {
        return Err("unsupported packet state or vertex layout");
    }
    if packet.viewport_target[0] != 0
        && (packet.viewport_target[0] != width || packet.viewport_target[1] != height)
    {
        return Err("viewport target does not match room target");
    }
    if packet.viewport_words.iter().any(|word| *word != 0) {
        let viewport = packet.viewport_words.map(f32::from_bits);
        if viewport != [0.0, 0.0, width as f32, height as f32] {
            return Err("viewport rectangle does not match room target");
        }
    }
    let scissor = if packet.scissor_count == 1 {
        let [left, top, right, bottom] = packet.scissor_words;
        if right <= left || bottom <= top || right > width || bottom > height {
            return Err("scissor rectangle is invalid for room target");
        }
        Some([left, top, right - left, bottom - top])
    } else {
        None
    };
    let vertex_bytes = packet.vertex_bytes as usize;
    let index_bytes = packet.index_bytes as usize;
    if vertex_bytes == 0
        || vertex_bytes % 24 != 0
        || (packet.indexed == 1 && index_bytes != packet.count as usize * 2)
        || (packet.indexed == 0 && index_bytes != 0)
    {
        return Err("vertex or index span is invalid");
    }
    let vertex_count = vertex_bytes / 24;
    let index_end = vertex_bytes
        .checked_add(index_bytes)
        .ok_or("index span overflows payload extent")?;
    let texture_bytes = packet
        .texture_bytes
        .iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(*bytes as usize))
        .ok_or("texture spans overflow payload extent")?;
    let program_start = index_end
        .checked_add(texture_bytes)
        .ok_or("program span overflows payload extent")?;
    let constants_start = program_start
        .checked_add(packet.vertex_program_bytes as usize)
        .ok_or("constant span overflows payload extent")?;
    let payload_end = constants_start
        .checked_add(packet.vertex_constants_bytes as usize)
        .ok_or("payload extent overflows address space")?;
    if payload_end != packet.payload.len() || packet.vertex_shader_index as usize >= 136 {
        return Err("shader payload extent or program index is invalid");
    }

    let program_bytes = &packet.payload[program_start..constants_start];
    let mut memory = crate::vsh::ProgramMemory::default();
    for (slot, words) in memory.slots.iter_mut().enumerate() {
        for (word, bytes) in words
            .iter_mut()
            .zip(program_bytes[slot * 16..slot * 16 + 16].chunks_exact(4))
        {
            *word = u32::from_le_bytes(bytes.try_into().map_err(|_| "shader word is truncated")?);
        }
    }
    let translation = crate::vsh_wgsl::translate(&memory, packet.vertex_shader_index as usize);
    if translation
        .inputs
        .iter()
        .any(|register| !matches!(*register, 1..=3))
    {
        return Err("shader reads an unsupported vertex register");
    }
    if translation
        .inputs
        .iter()
        .any(|register| !matches!(register, 1..=3))
    {
        return Err("shader reads an unsupported vertex register");
    }

    let mut inputs = Vec::<f32>::with_capacity(vertex_count * translation.inputs.len() * 4);
    for bytes in packet.payload[..vertex_bytes].chunks_exact(24) {
        let vertex =
            crate::vertex::decode_vertex(bytes, 24).ok_or("vertex payload is truncated")?;
        for &register in &translation.inputs {
            inputs.extend(match register {
                1 => vertex.position,
                2 => vertex.colour,
                3 => [vertex.uv[0], vertex.uv[1], 0.0, 1.0],
                _ => return Err("shader reads an unsupported vertex register"),
            });
        }
    }
    if inputs.is_empty() {
        inputs.extend([0.0; 4]);
    }
    let constants: Vec<f32> = packet.payload[constants_start..payload_end]
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("four byte chunks")))
        .collect();
    let key = if packet.vertex_program_hash != 0 {
        packet.vertex_program_hash
    } else {
        crate::vsh_wgsl::program_key(&memory, packet.vertex_shader_index as usize)
    };
    if !programs.contains_key(&key) {
        let entries = [
            storage_entry(0, true),
            storage_entry(1, true),
            storage_entry(2, false),
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            storage_entry(4, false),
        ];
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("T2238 translated room VSH bindings"),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("T2238 translated room VSH layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let pipeline =
            crate::replay::gpu_vsh::build_compute(device, &pipeline_layout, &translation.wgsl)
                .ok_or("translated shader failed wgpu validation")?;
        programs.insert(
            key,
            ShaderComputePipeline {
                bind_layout,
                pipeline_layout,
                pipeline,
            },
        );
    }
    let pipeline = programs
        .get(&key)
        .ok_or("translated shader pipeline is absent")?;
    let input_buffer = storage_buffer(
        device,
        "T2238 translated room inputs",
        &float_bytes(&inputs),
    );
    let constant_buffer = storage_buffer(
        device,
        "T2238 translated room constants",
        &float_bytes(&constants),
    );
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("T2238 translated room vertices"),
        size: (vertex_count * crate::vertex::Vertex::SIZE).max(84) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        mapped_at_creation: false,
    });
    let dummy_indices = storage_buffer(device, "T2238 translated room dummy indices", &[0; 16]);
    let job = crate::vsh_wgsl::Job {
        vertex_count: vertex_count as u32,
        screen: [width as f32, height as f32],
        fog_mode: u32::MAX,
        ..Default::default()
    };
    let job_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("T2238 translated room VSH job"),
        contents: &job.bytes(),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("T2238 translated room VSH resources"),
        layout: &pipeline.bind_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: constant_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: vertex_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: job_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: dummy_indices.as_entire_binding(),
            },
        ],
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("T2238 native NV2A vertex program"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            (vertex_count as u32).div_ceil(crate::vsh_wgsl::WORKGROUP),
            1,
            1,
        );
    }
    let topology = crate::vertex::triangulate(packet.primitive, packet.count)
        .ok_or("unsupported primitive topology")?;
    let indices = if packet.indexed == 0 {
        if vertex_count < packet.count as usize {
            return Err("non-indexed range exceeds owned vertices");
        }
        topology
    } else {
        topology
            .into_iter()
            .map(|index| {
                let at = vertex_bytes + index as usize * 2;
                let source = u16::from_le_bytes(packet.payload.get(at..at + 2)?.try_into().ok()?);
                let relative = u32::from(source).checked_sub(packet.vertex_first)?;
                (relative < vertex_count as u32).then_some(relative)
            })
            .collect::<Option<Vec<_>>>()
            .ok_or("index references a vertex outside the owned span")?
    };
    if indices.is_empty() {
        return Err("primitive produced no triangle indices");
    }
    let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("T2238 translated room indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let _ = &pipeline.pipeline_layout;
    Ok(EncodedShaderDraw {
        vertex: vertex_buffer,
        index,
        index_count: indices.len() as u32,
        scissor,
        cull,
    })
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_buffer(device: &wgpu::Device, label: &str, contents: &[u8]) -> wgpu::Buffer {
    let bytes = if contents.is_empty() {
        &[0; 16][..]
    } else {
        contents
    };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::STORAGE,
    })
}

fn float_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn create_shader_render_pipeline(
    device: &wgpu::Device,
    cull: RoomCullMode,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("T2238 translated NV2A vertex output"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) colour: vec4<f32> };
@vertex fn vs_main(@location(0) position: vec4<f32>, @location(1) colour: vec4<f32>) -> VertexOut {
    var out: VertexOut; out.position = position; out.colour = colour; return out;
}
@fragment fn fs_main(input: VertexOut) -> @location(0) vec4<f32> { return input.colour; }
"#
            .into(),
        ),
    });
    let attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("T2238 native translated vertex draw"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: crate::vertex::Vertex::SIZE as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }],
        },
        primitive: wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Cw,
            cull_mode: match cull {
                RoomCullMode::None => None,
                RoomCullMode::Clockwise => Some(wgpu::Face::Front),
                RoomCullMode::CounterClockwise => Some(wgpu::Face::Back),
            },
            ..Default::default()
        },
        depth_stencil: Some(room_depth_stencil_state(RoomPipelineKey {
            cull,
            depth: RoomDepthState {
                enabled: false,
                write: false,
                compare: RoomDepthCompare::Always,
            },
            stencil: RoomStencilState::default(),
        })),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}

/// Exact one-stage NV2A combiner shape observed for diffuse * texture 0 * 2.
/// The title binds it as a pixel shader; D3D texture blend-stage words do not
/// define this material when a shader is active.
fn diffuse_texture_combiner(scale: u32) -> crate::combiner::PixelProgram {
    let mut words = [0u32; 60];
    words[0x00 / 4] = 0xD4D8_1010;
    words[0x20 / 4] = 0x0000_000C;
    words[0x24 / 4] = 0x0000_1C80;
    let output = match scale {
        1 => 0x0000_00C0,
        2 => 0x0001_00C0,
        _ => unreachable!("only captured scale variants are admitted"),
    };
    words[0x68 / 4] = output;
    words[0x88 / 4] = 0xC4C8_0000;
    words[0xB4 / 4] = output;
    words[0xD4 / 4] = 0x0001_1101;
    words[0xD8 / 4] = 1;
    crate::combiner::PixelProgram::from_words(&words)
        .expect("the fixed expected combiner definition has one stage")
}

fn native_diffuse_texture_scale(words: &[u32]) -> Option<f32> {
    let program = crate::combiner::PixelProgram::from_words(words)?;
    if program.count != 1 || program.texture_mask() != 1 || program.uses_other_textures() {
        return None;
    }
    [1u32, 2]
        .into_iter()
        .find_map(|scale| (program == diffuse_texture_combiner(scale)).then_some(scale as f32))
}

/// Admit one screen-space XYZ+diffuse+UV packet using the captured stage-0
/// texture and the exact original diffuse-times-texture combiner. Any missing
/// bytes, unknown decoder format, unsupported sampler, or other D3D state is
/// left on the retained renderer.
fn append_textured_room_draw(
    packet: &RoomDrawPacket,
    width: u32,
    height: u32,
) -> Option<TexturedRoomDraw> {
    let alpha = native_alpha_state(packet)?;
    let cull = native_cull_mode(packet.cull_mode)?;
    let depth = native_depth_state(packet)?;
    let stencil = native_stencil_state(packet)?;
    if packet_material_class(packet) != RoomMaterialClass::TexturedModulate
        || packet.payload_dropped
        || packet.indexed > 1
        || packet.count < 3
        || packet.vertex_declaration != 13
        || packet.vertex_shader_handle != 0
        || packet.vertex_program_bytes != 0
        || packet.vertex_constants_bytes != 0
        || packet.scissor_count > 1
        || (packet.scissor_count == 1 && packet.scissor_exclusive != 0)
        || packet.vertex_stride != 24
        || width == 0
        || height == 0
        || packet.texture_bytes[1..].iter().any(|bytes| *bytes != 0)
    {
        return None;
    }
    if packet.viewport_target[0] != 0
        && (packet.viewport_target[0] != width || packet.viewport_target[1] != height)
    {
        return None;
    }
    if packet.viewport_words.iter().any(|word| *word != 0) {
        let viewport = packet.viewport_words.map(f32::from_bits);
        if viewport[0] != 0.0
            || viewport[1] != 0.0
            || viewport[2] != width as f32
            || viewport[3] != height as f32
        {
            return None;
        }
    }
    let scissor = if packet.scissor_count == 1 {
        let [left, top, right, bottom] = packet.scissor_words;
        if right <= left || bottom <= top || right > width || bottom > height {
            return None;
        }
        Some([left, top, right - left, bottom - top])
    } else {
        None
    };

    let texture_state = &packet.texture_stage_state[..32];
    let address_u = native_address_mode(texture_state[13])?;
    let address_v = native_address_mode(texture_state[14])?;
    let mag_filter = native_filter(texture_state[16])?;
    let min_filter = native_filter(texture_state[17])?;
    let mip_filter = match texture_state[18] {
        0 => wgpu::FilterMode::Nearest,
        1 => wgpu::FilterMode::Nearest,
        2 => wgpu::FilterMode::Linear,
        _ => return None,
    };
    if texture_state[19] > 1 || texture_state[28] != 0 {
        return None;
    }

    let vertex_bytes = packet.vertex_bytes as usize;
    let index_bytes = packet.index_bytes as usize;
    let vertex_count = vertex_bytes.checked_div(24)?;
    if vertex_bytes == 0
        || vertex_bytes % 24 != 0
        || (packet.indexed == 1 && index_bytes != packet.count as usize * 2)
        || (packet.indexed == 0 && index_bytes != 0)
    {
        return None;
    }
    let index_end = vertex_bytes.checked_add(index_bytes)?;
    let base_texture_length =
        crate::texture::data_length(packet.texture_formats[0], packet.texture_sizes[0])?;
    if packet.texture_bytes[0] < base_texture_length {
        return None;
    }
    // Captures may include source mip levels. The existing pure-Rust decoder
    // consumes the base level; match the replay path by building its declared
    // box-filtered mip chain from that decoded image.
    let texture_end = index_end.checked_add(packet.texture_bytes[0] as usize)?;
    let shader_end = texture_end.checked_add(packet.pixel_shader_bytes as usize)?;
    if shader_end != packet.payload.len() || packet.pixel_shader_bytes != 240 {
        return None;
    }
    let shader_words: Vec<u32> = packet.payload[texture_end..shader_end]
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();
    let combiner_scale = native_diffuse_texture_scale(&shader_words)?;
    let texture = crate::texture::decode(
        packet.texture_formats[0],
        packet.texture_sizes[0],
        &packet.payload[index_end..index_end + base_texture_length as usize],
    )?;

    let mut vertices = Vec::with_capacity(vertex_count);
    for bytes in packet.payload[..vertex_bytes].chunks_exact(24) {
        let vertex = crate::vertex::decode_vertex(bytes, 24)?;
        if !vertex.position[..3].iter().all(|value| value.is_finite())
            || !vertex.colour.iter().all(|value| value.is_finite())
            || !vertex.uv.iter().all(|value| value.is_finite())
        {
            return None;
        }
        vertices.push([
            vertex.position[0] * (2.0 / width as f32) - 1.0,
            1.0 - vertex.position[1] * (2.0 / height as f32),
            vertex.position[2].clamp(0.0, 1.0),
            vertex.colour[0],
            vertex.colour[1],
            vertex.colour[2],
            vertex.colour[3],
            vertex.uv[0],
            vertex.uv[1],
        ]);
    }
    let topology = crate::vertex::triangulate(packet.primitive, packet.count)?;
    let indices = if packet.indexed == 0 {
        if vertex_count < packet.count as usize {
            return None;
        }
        topology
    } else {
        if index_end > packet.payload.len() {
            return None;
        }
        let source_indices: Vec<u16> = packet.payload[vertex_bytes..index_end]
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        topology
            .into_iter()
            .map(|index| {
                let source = *source_indices.get(index as usize)?;
                let relative = u32::from(source).checked_sub(packet.vertex_first)?;
                (relative < vertex_count as u32).then_some(relative)
            })
            .collect::<Option<Vec<_>>>()?
    };

    Some(TexturedRoomDraw {
        vertices,
        indices,
        texture,
        address_u,
        address_v,
        mag_filter,
        min_filter,
        mip_filter,
        mip_levels: if crate::texture::is_linear(packet.texture_formats[0]) {
            1
        } else {
            ((packet.texture_formats[0] >> 16) & 0xF).max(1)
        },
        scissor,
        cull,
        depth,
        stencil,
        stencil_reference: packet.stencil_reference,
        alpha_test: alpha.test,
        alpha_function: alpha.function,
        alpha_reference: alpha.reference,
        blend: alpha.blend,
        combiner_scale,
    })
}

fn native_address_mode(value: u32) -> Option<wgpu::AddressMode> {
    Some(match value {
        1 => wgpu::AddressMode::Repeat,
        2 => wgpu::AddressMode::MirrorRepeat,
        3 => wgpu::AddressMode::ClampToEdge,
        _ => return None,
    })
}

fn native_filter(value: u32) -> Option<wgpu::FilterMode> {
    Some(match value {
        1 => wgpu::FilterMode::Nearest,
        2 => wgpu::FilterMode::Linear,
        _ => return None,
    })
}

fn create_textured_pipeline(device: &wgpu::Device) -> TexturedPipeline {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("T2238 textured room material bindings"),
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
        label: Some("T2238 textured room pipeline layout"),
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("T2238 diffuse times texture combiner"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
    @location(1) uv: vec2<f32>,
}
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@group(0) @binding(2) var<uniform> alpha_test: vec4<f32>;
@vertex fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) colour: vec4<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 1.0);
    out.colour = colour;
    out.uv = uv;
    return out;
}
@fragment fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let result = min(input.colour * textureSample(image, image_sampler, input.uv) * alpha_test.w, vec4<f32>(1.0));
    var alpha_passed = true;
    if (alpha_test.x > 0.5) {
        if (alpha_test.y < 1.5) { alpha_passed = false; }
        else if (alpha_test.y < 2.5) { alpha_passed = result.a < alpha_test.z; }
        else if (alpha_test.y < 3.5) { alpha_passed = result.a == alpha_test.z; }
        else if (alpha_test.y < 4.5) { alpha_passed = result.a <= alpha_test.z; }
        else if (alpha_test.y < 5.5) { alpha_passed = result.a > alpha_test.z; }
        else if (alpha_test.y < 6.5) { alpha_passed = result.a != alpha_test.z; }
        else if (alpha_test.y < 7.5) { alpha_passed = result.a >= alpha_test.z; }
    }
    if (!alpha_passed) { discard; }
    return result;
}
"#
            .into(),
        ),
    });
    TexturedPipeline {
        layout,
        pipeline_layout,
        shader,
        pipelines: HashMap::new(),
    }
}

fn create_textured_room_pipeline(
    device: &wgpu::Device,
    textured: &TexturedPipeline,
    key: TexturedPipelineKey,
) -> wgpu::RenderPipeline {
    let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4, 2 => Float32x2];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("T2238 captured textured room material/state pipeline"),
        layout: Some(&textured.pipeline_layout),
        vertex: wgpu::VertexState {
            module: &textured.shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 36,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            }],
        },
        primitive: wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Cw,
            cull_mode: match key.state.cull {
                RoomCullMode::None => None,
                RoomCullMode::Clockwise => Some(wgpu::Face::Front),
                RoomCullMode::CounterClockwise => Some(wgpu::Face::Back),
            },
            ..Default::default()
        },
        depth_stencil: Some(room_depth_stencil_state(key.state)),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &textured.shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: key.blend.map(wgpu_room_blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}

fn encode_textured_draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    pipeline: TexturedPipelineKey,
    stencil_reference: u32,
    draw: &TexturedRoomDraw,
) -> EncodedTexturedDraw {
    let (mip_levels, mip_chain) = native_mip_chain(
        draw.texture.width,
        draw.texture.height,
        &draw.texture.rgba,
        draw.mip_levels,
    );
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("T2238 decoded NV2A room texture"),
        size: wgpu::Extent3d {
            width: draw.texture.width,
            height: draw.texture.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut offset = 0usize;
    for level in 0..mip_levels {
        let width = (draw.texture.width >> level).max(1);
        let height = (draw.texture.height >> level).max(1);
        let bytes = (width * height * 4) as usize;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &mip_chain[offset..offset + bytes],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        offset += bytes;
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: draw.address_u,
        address_mode_v: draw.address_v,
        mag_filter: draw.mag_filter,
        min_filter: draw.min_filter,
        mipmap_filter: draw.mip_filter,
        lod_max_clamp: (mip_levels - 1) as f32,
        ..Default::default()
    });
    let alpha = [
        if draw.alpha_test { 1.0 } else { 0.0 },
        draw.alpha_function as f32,
        draw.alpha_reference as f32 / 255.0,
        draw.combiner_scale,
    ];
    let alpha_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("T2238 native room alpha-test state"),
        contents: bytemuck::cast_slice(&alpha),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("T2238 textured room draw bindings"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: alpha_buffer.as_entire_binding(),
            },
        ],
    });
    let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("T2238 owned textured room vertices"),
        contents: bytemuck::cast_slice(&draw.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("T2238 owned textured room indices"),
        contents: bytemuck::cast_slice(&draw.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    EncodedTexturedDraw {
        vertex,
        index,
        bind_group,
        index_count: draw.indices.len() as u32,
        scissor: draw.scissor,
        pipeline,
        stencil_reference,
    }
}

fn native_mip_chain(width: u32, height: u32, base: &[u8], requested_levels: u32) -> (u32, Vec<u8>) {
    let maximum_levels = 32 - width.max(height).leading_zeros();
    let levels = requested_levels.clamp(1, maximum_levels);
    let mut chain = base.to_vec();
    let (mut previous_width, mut previous_height) = (width as usize, height as usize);
    let mut previous = base.to_vec();
    for _ in 1..levels {
        let next_width = (previous_width / 2).max(1);
        let next_height = (previous_height / 2).max(1);
        let mut next = Vec::with_capacity(next_width * next_height * 4);
        for y in 0..next_height {
            for x in 0..next_width {
                for channel in 0..4 {
                    let x0 = (2 * x).min(previous_width - 1);
                    let x1 = (2 * x + 1).min(previous_width - 1);
                    let y0 = (2 * y).min(previous_height - 1);
                    let y1 = (2 * y + 1).min(previous_height - 1);
                    let at = |px: usize, py: usize| {
                        u32::from(previous[(py * previous_width + px) * 4 + channel])
                    };
                    let value = at(x0, y0) + at(x1, y0) + at(x0, y1) + at(x1, y1);
                    next.push(((value + 2) / 4) as u8);
                }
            }
        }
        chain.extend_from_slice(&next);
        previous = next;
        previous_width = next_width;
        previous_height = next_height;
    }
    (levels, chain)
}

fn add_object_markers(
    vertices: &mut Vec<[f32; 6]>,
    objects: &[ObjectId],
    rect: [f32; 4],
    width: u32,
    height: u32,
) {
    if objects.is_empty() {
        return;
    }
    let start_x = rect[0] + rect[2] * 0.025;
    let start_y = rect[1] + rect[3] * 0.035;
    let marker = (rect[2] * 0.008).clamp(2.0, 8.0);
    let gap = marker * 1.35;
    for (index, id) in objects.iter().enumerate() {
        let col = index % 48;
        let row = index / 48;
        let x = start_x + col as f32 * gap;
        let y = start_y + row as f32 * gap;
        let colour = [
            (id.slot.wrapping_mul(29) % 191) as f32 / 255.0 + 0.2,
            (id.generation.wrapping_mul(43) % 191) as f32 / 255.0 + 0.2,
            (id.slot.wrapping_add(id.generation as u32).wrapping_mul(17) % 191) as f32 / 255.0
                + 0.2,
            0.82,
        ];
        add_rect(
            vertices,
            [x, y, x + marker, y + marker],
            colour,
            width,
            height,
        );
    }
}

fn add_rect_outline(
    vertices: &mut Vec<[f32; 6]>,
    rect: [f32; 4],
    colour: [f32; 4],
    width: u32,
    height: u32,
) {
    let thickness = (rect[2] - rect[0])
        .min(rect[3] - rect[1])
        .mul_add(0.003, 1.0)
        .clamp(1.0, 4.0);
    add_rect(
        vertices,
        [rect[0], rect[1], rect[2], rect[1] + thickness],
        colour,
        width,
        height,
    );
    add_rect(
        vertices,
        [rect[0], rect[3] - thickness, rect[2], rect[3]],
        colour,
        width,
        height,
    );
    add_rect(
        vertices,
        [rect[0], rect[1], rect[0] + thickness, rect[3]],
        colour,
        width,
        height,
    );
    add_rect(
        vertices,
        [rect[2] - thickness, rect[1], rect[2], rect[3]],
        colour,
        width,
        height,
    );
}

fn add_rect(
    vertices: &mut Vec<[f32; 6]>,
    rect: [f32; 4],
    colour: [f32; 4],
    width: u32,
    height: u32,
) {
    let a = [rect[0], rect[1]];
    let b = [rect[2], rect[1]];
    let c = [rect[2], rect[3]];
    let d = [rect[0], rect[3]];
    add_triangle(vertices, a, b, c, colour, width, height);
    add_triangle(vertices, a, c, d, colour, width, height);
}

fn add_triangle(
    vertices: &mut Vec<[f32; 6]>,
    a: [f32; 2],
    b: [f32; 2],
    c: [f32; 2],
    colour: [f32; 4],
    width: u32,
    height: u32,
) {
    for point in [a, b, c] {
        let ndc = pixel_to_ndc(point[0], point[1], width, height);
        vertices.push([ndc[0], ndc[1], colour[0], colour[1], colour[2], colour[3]]);
    }
}

fn pixel_to_ndc(x: f32, y: f32, width: u32, height: u32) -> [f32; 2] {
    [2.0 * x / width as f32 - 1.0, 1.0 - 2.0 * y / height as f32]
}

fn clamp_rect(rect: [i32; 4], width: u32, height: u32) -> [f32; 4] {
    let max_x = width as f32;
    let max_y = height as f32;
    let left = (rect[0] as f32).clamp(0.0, max_x);
    let top = (rect[1] as f32).clamp(0.0, max_y);
    let right = (rect[2] as f32).clamp(left, max_x);
    let bottom = (rect[3] as f32).clamp(top, max_y);
    if right - left < 2.0 || bottom - top < 2.0 {
        [0.0, 0.0, max_x, max_y]
    } else {
        [left, top, right, bottom]
    }
}

fn range_since(start: u32, vertices: &[[f32; 6]]) -> Range<u32> {
    start..vertices.len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_layer::FrameLayer;
    use tsfp_engine_render::scene::{
        light_state::{
            Color3, FrameResourceEpoch, SceneLightInputs, SceneLightState, ViewPoint, WorldPoint,
        },
        render_prep::{PreparedRoomPlan, Sampling},
    };

    fn prepared_test_frame() -> PreparedSceneFrame {
        PreparedSceneFrame {
            tick: 9,
            alpha: 0.0,
            sampling: Sampling::Fixed,
            resource_epoch: None,
            room_plan: Some(PreparedRoomPlan {
                tick: 9,
                draw_max_depth: 1,
                rooms: vec![7, 3],
            }),
            observed_objects: vec![ObjectId {
                slot: 4,
                generation: 2,
            }],
            observed_views: vec![NativeViewInput {
                viewer: 0,
                position: [0.0; 3],
                matrix: [0.0; 16],
                viewport: [0.0, 0.0, 1.0, 1.0],
                rectangle: [0, 0, 640, 480],
            }],
            effects_tick: None,
            effects: Vec::new(),
            scene_lights: None,
            views: Vec::new(),
        }
    }

    fn fabricated_scene_lights(epoch: FrameResourceEpoch) -> PublishedSceneLights {
        let mut state = SceneLightState::default();
        let first_record = state
            .setup_record(SceneLightInputs {
                colors: [
                    Color3([1.0, 0.5, 0.25]),
                    Color3([0.0; 3]),
                    Color3([0.0; 3]),
                    Color3([0.1, 0.2, 0.3]),
                ],
                world_points: [WorldPoint([1.0, 2.0, 3.0]); 3],
            })
            .unwrap();
        let second_record = state
            .setup_record(SceneLightInputs {
                colors: [
                    Color3([0.25, 0.5, 1.0]),
                    Color3([0.0; 3]),
                    Color3([0.0; 3]),
                    Color3([0.3, 0.2, 0.1]),
                ],
                world_points: [WorldPoint([4.0, 5.0, 6.0]); 3],
            })
            .unwrap();
        let first_view = state.setup_view().unwrap();
        let second_view = state.setup_view().unwrap();
        state
            .prepare_view(first_record, first_view, epoch, |WorldPoint(point)| {
                ViewPoint([point[0], point[1], point[2], 1.0])
            })
            .unwrap();
        state
            .prepare_view(second_record, second_view, epoch, |WorldPoint(point)| {
                ViewPoint([point[0] + 10.0, point[1], point[2], 1.0])
            })
            .unwrap();
        state.set_publication_enabled(true);
        state
            .publish_frame(
                epoch,
                &[
                    SceneLightBinding::new(first_record, first_view),
                    SceneLightBinding::new(second_record, second_view),
                ],
            )
            .unwrap()
            .clone()
    }

    #[test]
    fn d3d_blend_state_maps_supported_factors_and_operations() {
        let factors = [
            (1, RoomBlendFactor::Zero),
            (2, RoomBlendFactor::One),
            (3, RoomBlendFactor::SourceColour),
            (4, RoomBlendFactor::OneMinusSourceColour),
            (5, RoomBlendFactor::SourceAlpha),
            (6, RoomBlendFactor::OneMinusSourceAlpha),
            (7, RoomBlendFactor::DestinationAlpha),
            (8, RoomBlendFactor::OneMinusDestinationAlpha),
            (9, RoomBlendFactor::DestinationColour),
            (10, RoomBlendFactor::OneMinusDestinationColour),
            (11, RoomBlendFactor::SourceAlphaSaturated),
        ];
        for (value, expected) in factors {
            assert_eq!(
                native_blend_state_values(1, value, 2, 1),
                Some(Some(RoomBlendState {
                    source: expected,
                    destination: RoomBlendFactor::One,
                    operation: RoomBlendOperation::Add,
                })),
                "D3D blend factor {value}"
            );
        }
        let operations = [
            (1, RoomBlendOperation::Add),
            (2, RoomBlendOperation::Subtract),
            (3, RoomBlendOperation::ReverseSubtract),
            (4, RoomBlendOperation::Min),
            (5, RoomBlendOperation::Max),
        ];
        for (value, expected) in operations {
            assert_eq!(
                native_blend_state_values(1, 2, 1, value),
                Some(Some(RoomBlendState {
                    source: RoomBlendFactor::One,
                    destination: RoomBlendFactor::Zero,
                    operation: expected,
                })),
                "D3D blend operation {value}"
            );
        }
        assert_eq!(native_blend_state_values(0, 99, 99, 99), Some(None));
        assert_eq!(native_blend_state_values(1, 12, 6, 1), None);
        assert_eq!(native_blend_state_values(1, 5, 6, 6), None);
    }

    #[test]
    fn production_sink_copies_owner_rooms_objects_and_views() {
        let shared = Arc::new(Shared::default());
        let mut sink = NativeSceneWgpuSink {
            shared: shared.clone(),
        };
        let frame = prepared_test_frame();

        sink.submit(&frame);
        let packet = shared.packet.lock().unwrap().clone().unwrap();
        assert_eq!(packet.objects, frame.observed_objects);
        assert_eq!(packet.views, frame.observed_views);
        assert!(packet.scene_lights.is_none());
        assert_eq!(shared.submitted.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn production_sink_retains_only_matching_epoch_light_snapshots() {
        let shared = Arc::new(Shared::default());
        let mut sink = NativeSceneWgpuSink {
            shared: shared.clone(),
        };
        let epoch = FrameResourceEpoch(8);
        let mut frame = prepared_test_frame();
        frame.resource_epoch = Some(epoch);
        frame.scene_lights = Some(fabricated_scene_lights(epoch));

        sink.submit(&frame);
        let packet = shared.packet.lock().unwrap().clone().unwrap();
        assert_eq!(packet.scene_lights, frame.scene_lights);
        assert_eq!(packet.gpu_scene_lights.len(), 2);
        let lights = packet.scene_lights().unwrap();
        let first = &lights.blocks[0];
        let second = &lights.blocks[1];
        let first_binding = SceneLightBinding::new(first.record, first.view);
        let second_binding = SceneLightBinding::new(second.record, second.view);
        let cross_binding = SceneLightBinding::new(first.record, second.view);
        assert_eq!(
            packet.gpu_scene_light_for(first_binding),
            Some(&packet.gpu_scene_lights[0].payload)
        );
        assert_eq!(
            packet.gpu_scene_light_for_draw(Some(first_binding)),
            Some(&packet.gpu_scene_lights[0].payload)
        );
        assert!(native_draw_has_scene_light_match(
            &packet,
            Some(first_binding),
            true,
        ));
        assert!(!native_draw_has_scene_light_match(
            &packet,
            Some(first_binding),
            false,
        ));
        assert!(!native_draw_has_scene_light_match(
            &packet,
            Some(cross_binding),
            true,
        ));
        assert!(packet.gpu_scene_light_for_draw(None).is_none());
        assert!(
            packet
                .gpu_scene_light_for_draw(Some(cross_binding))
                .is_none()
        );
        assert_eq!(
            packet.gpu_scene_light_for(second_binding),
            Some(&packet.gpu_scene_lights[1].payload)
        );
        assert_eq!(
            packet.gpu_scene_lights[1].payload.slots[0].position,
            [14.0, 5.0, 6.0, 1.0]
        );
        assert!(packet.gpu_scene_light_for(cross_binding).is_none());
        let mut reordered = packet.clone();
        reordered.gpu_scene_lights.reverse();
        assert_eq!(
            reordered.gpu_scene_light_for(first_binding),
            Some(&reordered.gpu_scene_lights[1].payload)
        );
        let gpu_lights = packet.gpu_scene_lights[0].payload;
        assert_eq!(gpu_lights.slots[0].position, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(gpu_lights.slots[0].color, [1.0, 0.5, 0.25, 0.0]);
        assert_eq!(gpu_lights.slots[0].parameters, [0.5, 0.0, 0.0]);
        assert_eq!(gpu_lights.slots[0].enabled, 1);
        assert_eq!(gpu_lights.slots[1].position, [0.0, 0.0, 0.0, 0.5]);
        assert_eq!(gpu_lights.slots[1].parameters, [1.0, 0.0, 1.0]);
        assert_eq!(gpu_lights.slots[1].enabled, 0);
        assert_eq!(gpu_lights.fourth_color, [0.1, 0.2, 0.3, 0.0]);
        assert_eq!(std::mem::size_of::<GpuSceneLightSlot>(), 48);
        assert_eq!(std::mem::align_of::<GpuSceneLightSlot>(), 16);
        assert_eq!(std::mem::size_of::<GpuSceneLightBlock>(), 160);
        assert_eq!(std::mem::align_of::<GpuSceneLightBlock>(), 16);
        assert_eq!(bytemuck::bytes_of(&gpu_lights).len(), 160);
        assert_eq!(sink.take_stats().scene_light_blocks, 2);

        let mut stale = frame.clone();
        stale.scene_lights.as_mut().unwrap().epoch = FrameResourceEpoch(7);
        sink.submit(&stale);
        assert!(shared.packet.lock().unwrap().is_none());
        assert_eq!(shared.rejected.load(Ordering::Relaxed), 1);

        let mut duplicate = frame;
        let block = duplicate.scene_lights.as_ref().unwrap().blocks[0].clone();
        duplicate.scene_lights.as_mut().unwrap().blocks.push(block);
        sink.submit(&duplicate);
        assert!(shared.packet.lock().unwrap().is_none());
        assert_eq!(shared.rejected.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn scene_light_upload_deduplicates_payloads_and_keeps_draw_indices() {
        let shared = Arc::new(Shared::default());
        let mut sink = NativeSceneWgpuSink {
            shared: shared.clone(),
        };
        let epoch = FrameResourceEpoch(10);
        let mut frame = prepared_test_frame();
        frame.resource_epoch = Some(epoch);
        frame.scene_lights = Some(fabricated_scene_lights(epoch));
        sink.submit(&frame);
        let packet = shared.packet.lock().unwrap().clone().unwrap();
        let lights = packet.scene_lights().unwrap();
        let first = &lights.blocks[0];
        let second = &lights.blocks[1];
        let first_binding = SceneLightBinding::new(first.record, first.view);
        let second_binding = SceneLightBinding::new(second.record, second.view);
        let cross_binding = SceneLightBinding::new(first.record, second.view);

        let mut upload = GpuSceneLightUpload::default();
        assert_eq!(upload.bind_draw(&packet, 4, Some(first_binding)), Some(0));
        assert_eq!(upload.bind_draw(&packet, 7, Some(first_binding)), Some(0));
        assert_eq!(upload.bind_draw(&packet, 9, Some(second_binding)), Some(1));
        assert_eq!(upload.bind_draw(&packet, 11, Some(cross_binding)), None);
        assert_eq!(upload.bind_draw(&packet, 12, None), None);

        assert_eq!(upload.blocks.len(), 2);
        assert_eq!(upload.identities, [first_binding, second_binding]);
        assert_eq!(upload.blocks[0].slots[0].enabled, 1);
        assert_eq!(upload.blocks[0].slots[1].enabled, 0);
        assert_eq!(upload.blocks[0].slots[2].enabled, 0);
        assert_eq!(
            upload.draw_bindings,
            [
                GpuSceneLightDrawBinding {
                    draw_index: 4,
                    block_index: 0
                },
                GpuSceneLightDrawBinding {
                    draw_index: 7,
                    block_index: 0
                },
                GpuSceneLightDrawBinding {
                    draw_index: 9,
                    block_index: 1
                },
            ]
        );
        assert_eq!(
            resolve_scene_light_draw_binding(
                epoch,
                epoch,
                &upload.identities,
                &upload.draw_bindings,
                4,
            ),
            Some(GpuSceneLightBinding {
                epoch,
                binding: first_binding,
                block_index: 0,
            })
        );
        assert_eq!(
            resolve_scene_light_draw_binding(
                epoch,
                epoch,
                &upload.identities,
                &upload.draw_bindings,
                9,
            ),
            Some(GpuSceneLightBinding {
                epoch,
                binding: second_binding,
                block_index: 1,
            })
        );
        let mut diagnostic_block = upload.blocks[0];
        diagnostic_block.slots[1].color = [0.2, 0.3, 0.4, 0.0];
        diagnostic_block.slots[2].color = [0.6, 0.7, 1.2, 0.0];
        diagnostic_block.slots[0].position = [-1.0, 0.25, 1.5, 1.0];
        diagnostic_block.slots[1].position = [0.35, 0.55, 0.75, 1.0];
        diagnostic_block.slots[2].position = [1.0, 0.4, -0.5, 1.0];
        diagnostic_block.slots[0].parameters = [0.15, 0.45, 0.75];
        diagnostic_block.slots[1].parameters = [0.5, 0.0, 0.0];
        diagnostic_block.slots[2].parameters = [1.2, -0.2, 0.35];
        let diagnostic_blocks = [diagnostic_block];
        assert_eq!(
            scene_light_preview_color(&diagnostic_blocks, 0, 0.4, SceneLightPreviewSource::Slot0),
            Some([1.0, 0.5, 0.25, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(&diagnostic_blocks, 0, 0.4, SceneLightPreviewSource::Slot1),
            Some([0.2, 0.3, 0.4, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(&diagnostic_blocks, 0, 0.4, SceneLightPreviewSource::Slot2),
            Some([0.6, 0.7, 1.0, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::FourthColor
            ),
            Some([0.1, 0.2, 0.3, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot0Position
            ),
            Some([0.0, 0.25, 1.0, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot1Position
            ),
            Some([0.35, 0.55, 0.75, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot2Position
            ),
            Some([1.0, 0.4, 0.0, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot0Parameters
            ),
            Some([0.15, 0.45, 0.75, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot1Parameters
            ),
            Some([0.5, 0.0, 0.0, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot2Parameters
            ),
            Some([1.0, 0.0, 0.35, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot0Enabled
            ),
            Some([1.0, 1.0, 1.0, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot1Enabled
            ),
            Some([0.0, 0.0, 0.0, 0.4])
        );
        diagnostic_block.slots[1].enabled = 1;
        let enabled_color_blocks = [diagnostic_block];
        assert_eq!(
            scene_light_preview_color(
                &enabled_color_blocks,
                0,
                0.4,
                SceneLightPreviewSource::EnabledSlotColorTransport
            ),
            Some([1.0, 0.8, 0.65, 0.4])
        );
        assert_eq!(
            scene_light_preview_color(
                &diagnostic_blocks,
                0,
                0.4,
                SceneLightPreviewSource::Slot2Enabled
            ),
            Some([0.0, 0.0, 0.0, 0.4])
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(7),
            SceneLightPreviewSource::Slot0Enabled
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(8),
            SceneLightPreviewSource::Slot1Enabled
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(9),
            SceneLightPreviewSource::Slot2Enabled
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(10),
            SceneLightPreviewSource::EnabledSlotColorTransport
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(11),
            SceneLightPreviewSource::Slot0Parameters
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(12),
            SceneLightPreviewSource::Slot1Parameters
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(13),
            SceneLightPreviewSource::Slot2Parameters
        );
        assert_eq!(
            scene_light_preview_color(&diagnostic_blocks, 2, 0.4, SceneLightPreviewSource::Slot0),
            None
        );
        assert!(
            resolve_scene_light_draw_binding(
                epoch,
                epoch,
                &upload.identities,
                &upload.draw_bindings,
                12,
            )
            .is_none()
        );
        assert!(scene_light_upload_is_well_formed(&upload));
        assert!(
            resolve_scene_light_draw_binding(
                FrameResourceEpoch(epoch.0 + 1),
                epoch,
                &upload.identities,
                &upload.draw_bindings,
                4,
            )
            .is_none()
        );

        let mut duplicate_identity = upload.clone();
        duplicate_identity.identities[1] = duplicate_identity.identities[0];
        assert!(!scene_light_upload_is_well_formed(&duplicate_identity));
        let mut missing_payload = upload.clone();
        missing_payload.blocks.pop();
        assert!(!scene_light_upload_is_well_formed(&missing_payload));
        let mut duplicate_draw = upload.clone();
        duplicate_draw
            .draw_bindings
            .push(duplicate_draw.draw_bindings[0]);
        assert!(!scene_light_upload_is_well_formed(&duplicate_draw));
        let mut invalid_block_index = upload.clone();
        invalid_block_index.draw_bindings[0].block_index = u32::MAX;
        assert!(!scene_light_upload_is_well_formed(&invalid_block_index));

        let mut ambiguous = upload.draw_bindings.clone();
        ambiguous.push(GpuSceneLightDrawBinding {
            draw_index: 4,
            block_index: 1,
        });
        assert!(
            resolve_scene_light_draw_binding(epoch, epoch, &upload.identities, &ambiguous, 4)
                .is_none()
        );
        let invalid_block = [GpuSceneLightDrawBinding {
            draw_index: 4,
            block_index: u32::MAX,
        }];
        assert!(
            resolve_scene_light_draw_binding(epoch, epoch, &upload.identities, &invalid_block, 4,)
                .is_none()
        );
        assert_eq!(bytemuck::cast_slice::<_, u8>(&upload.blocks).len(), 320);
        assert_eq!(upload.blocks[0].slots[0].position, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(upload.blocks[1].slots[0].position, [14.0, 5.0, 6.0, 1.0]);
    }

    #[test]
    fn scene_light_preview_shader_parses_and_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(SCENE_LIGHT_PREVIEW_WGSL)
            .expect("scene-light preview WGSL parses");
        let mut validator = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        validator
            .validate(&module)
            .expect("scene-light preview WGSL validates");
        assert_eq!(std::mem::size_of::<GpuSceneLightSlot>(), 48);
        assert_eq!(std::mem::size_of::<GpuSceneLightBlock>(), 160);
        for source in [
            SceneLightPreviewSource::Slot0,
            SceneLightPreviewSource::Slot1,
            SceneLightPreviewSource::Slot2,
            SceneLightPreviewSource::FourthColor,
            SceneLightPreviewSource::Slot0Position,
            SceneLightPreviewSource::Slot1Position,
            SceneLightPreviewSource::Slot2Position,
            SceneLightPreviewSource::Slot0Enabled,
            SceneLightPreviewSource::Slot1Enabled,
            SceneLightPreviewSource::Slot2Enabled,
            SceneLightPreviewSource::EnabledSlotColorTransport,
            SceneLightPreviewSource::Slot0Parameters,
            SceneLightPreviewSource::Slot1Parameters,
            SceneLightPreviewSource::Slot2Parameters,
        ] {
            assert!(module.entry_points.iter().any(|entry| {
                entry.name == source.fragment_entry_point()
                    && entry.stage == wgpu::naga::ShaderStage::Fragment
            }));
        }
    }

    #[test]
    fn scene_light_gpu_payload_matches_wgsl_storage_layout() {
        use wgpu::naga::{ArraySize, TypeInner};

        let module = wgpu::naga::front::wgsl::parse_str(SCENE_LIGHT_PREVIEW_WGSL)
            .expect("scene-light preview WGSL parses");
        let slot = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("SceneLightSlot"))
            .map(|(_, ty)| ty)
            .expect("WGSL SceneLightSlot type exists");
        let TypeInner::Struct { members, span } = &slot.inner else {
            panic!("WGSL SceneLightSlot is a struct");
        };
        assert_eq!(*span, std::mem::size_of::<GpuSceneLightSlot>() as u32);
        assert_eq!(
            members
                .iter()
                .map(|member| (member.name.as_deref(), member.offset))
                .collect::<Vec<_>>(),
            [
                (Some("position"), 0),
                (Some("color"), 16),
                (Some("parameters"), 32),
                (Some("enabled"), 44)
            ]
        );
        assert_eq!(std::mem::offset_of!(GpuSceneLightSlot, position), 0);
        assert_eq!(std::mem::offset_of!(GpuSceneLightSlot, color), 16);
        assert_eq!(std::mem::offset_of!(GpuSceneLightSlot, parameters), 32);
        assert_eq!(std::mem::offset_of!(GpuSceneLightSlot, enabled), 44);

        let block = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("SceneLightBlock"))
            .map(|(_, ty)| ty)
            .expect("WGSL SceneLightBlock type exists");
        let TypeInner::Struct { members, span } = &block.inner else {
            panic!("WGSL SceneLightBlock is a struct");
        };
        assert_eq!(*span, std::mem::size_of::<GpuSceneLightBlock>() as u32);
        assert_eq!(
            members
                .iter()
                .map(|member| (member.name.as_deref(), member.offset))
                .collect::<Vec<_>>(),
            [(Some("slots"), 0), (Some("fourth_color"), 144)]
        );
        assert_eq!(std::mem::offset_of!(GpuSceneLightBlock, slots), 0);
        assert_eq!(std::mem::offset_of!(GpuSceneLightBlock, fourth_color), 144);

        let slots = &module.types[members[0].ty];
        let TypeInner::Array { base, size, stride } = &slots.inner else {
            panic!("WGSL scene-light slots are an array");
        };
        assert!(matches!(size, ArraySize::Constant(length) if length.get() == 3));
        assert_eq!(*stride, std::mem::size_of::<GpuSceneLightSlot>() as u32);
        assert_eq!(module.types[*base].name.as_deref(), Some("SceneLightSlot"));

        let storage = module
            .global_variables
            .iter()
            .find(|(_, global)| global.name.as_deref() == Some("scene_light_blocks"))
            .map(|(_, global)| global)
            .expect("WGSL scene-light storage binding exists");
        let TypeInner::Array { base, size, stride } = &module.types[storage.ty].inner else {
            panic!("WGSL scene-light storage is a runtime array");
        };
        assert!(matches!(size, ArraySize::Dynamic));
        assert_eq!(*stride, std::mem::size_of::<GpuSceneLightBlock>() as u32);
        assert_eq!(module.types[*base].name.as_deref(), Some("SceneLightBlock"));
    }

    #[test]
    fn scene_light_preview_switch_is_explicit_and_default_off() {
        let sink = NativeSceneWgpuSink {
            shared: Arc::new(Shared::default()),
        };
        assert!(
            !sink
                .shared
                .scene_light_preview_enabled
                .load(Ordering::Acquire)
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(
                sink.shared
                    .scene_light_preview_source
                    .load(Ordering::Acquire)
            ),
            SceneLightPreviewSource::Slot0
        );
        sink.set_scene_light_preview_source(SceneLightPreviewSource::FourthColor);
        assert_eq!(
            SceneLightPreviewSource::from_raw(
                sink.shared
                    .scene_light_preview_source
                    .load(Ordering::Acquire)
            ),
            SceneLightPreviewSource::FourthColor
        );
        assert_eq!(
            SceneLightPreviewSource::from_raw(SceneLightPreviewSource::Slot2Position as u64),
            SceneLightPreviewSource::Slot2Position
        );
        sink.set_scene_light_preview_enabled(true);
        assert!(
            sink.shared
                .scene_light_preview_enabled
                .load(Ordering::Acquire)
        );
    }

    #[test]
    fn production_sink_keeps_frame_lease_through_queue_completion() {
        let shared = Arc::new(Shared::default());
        let mut sink = NativeSceneWgpuSink {
            shared: shared.clone(),
        };
        let epoch = FrameResourceEpoch(9);
        let mut frame = prepared_test_frame();
        frame.resource_epoch = Some(epoch);
        frame.scene_lights = Some(fabricated_scene_lights(epoch));
        let expected_lights = frame.scene_lights.clone();

        sink.submit_lease(PreparedSceneLease::new(frame));
        let packet = shared.packet.lock().unwrap().clone().unwrap();
        assert!(packet.scene_lights.is_none());
        assert_eq!(packet.scene_lights(), expected_lights.as_ref());
        assert_eq!(packet.gpu_scene_lights.len(), 2);
        assert_eq!(
            packet.gpu_scene_lights[0].payload.fourth_color,
            [0.1, 0.2, 0.3, 0.0]
        );
        assert_eq!(sink.take_stats().scene_light_blocks, 2);
        let lease = packet.frame_lease.clone().unwrap();

        // Encoding transfers another lease reference to the in-flight list.
        shared.retain_scene_lease(lease);
        sink.clear_frame();
        assert!(shared.packet.lock().unwrap().is_none());
        let completed = shared.take_pending_scene_leases();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].frame().tick, 9);
        assert_eq!(completed[0].frame().scene_lights, expected_lights);
        assert!(shared.take_pending_scene_leases().is_empty());
        drop(completed);
    }

    #[test]
    fn native_ordered_room_pass_matches_legacy_pixels() {
        use crate::context::{GpuContext, Preference};
        use crate::replay::Replayer;
        use crate::stream::Stream;

        let gpu = match GpuContext::new(&Preference::Software) {
            Ok(gpu) => gpu,
            Err(error) => {
                println!("SKIPPED (no software adapter): {error}");
                return;
            }
        };
        let device = gpu.device.clone();
        let queue = gpu.queue.clone();
        let (width, height) = (640u32, 480u32);
        let mut source_vertices = Vec::new();
        for (x, y) in [
            (100.0f32, 100.0f32),
            (400.0, 100.0),
            (400.0, 400.0),
            (100.0, 400.0),
        ] {
            source_vertices.extend_from_slice(&x.to_le_bytes());
            source_vertices.extend_from_slice(&y.to_le_bytes());
            source_vertices.extend_from_slice(&0.5f32.to_le_bytes());
            source_vertices.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        }
        let packet = RoomDrawPacket {
            frame_sequence: 1,
            room: 7,
            indexed: 0,
            primitive: 8,
            first: 0,
            count: 4,
            vertex_buffer: 0x4000_2000,
            vertex_buffer_bytes: source_vertices.len() as u32,
            vertex_stride: 16,
            vertex_first: 0,
            index_buffer: 0,
            index_buffer_bytes: 0,
            texture_headers: [0; 4],
            texture_data: [0; 4],
            texture_formats: [0; 4],
            texture_sizes: [0; 4],
            texture_stage_state: [0; 128],
            viewport_words: [0; 4],
            viewport_target: [0; 4],
            scissor_words: [0; 4],
            scissor_count: 0,
            scissor_exclusive: 0,
            vertex_declaration: 9,
            vertex_shader_handle: 0,
            vertex_shader_index: 0,
            vertex_program_bytes: 0,
            vertex_constants_bytes: 0,
            pixel_shader_bytes: 0,
            vertex_program_hash: 0,
            vertex_constants_hash: 0,
            pixel_shader_hash: 0,
            blend_enable: 0,
            blend_source: 0,
            blend_destination: 0,
            blend_operation: 0,
            depth_enable: 0,
            depth_write: 0,
            depth_function: 0,
            cull_mode: 1,
            stencil_enable: 0,
            stencil_function: 0,
            stencil_reference: 0,
            stencil_mask: 0,
            stencil_write_mask: 0,
            stencil_fail: 0,
            stencil_depth_fail: 0,
            stencil_pass: 0,
            vertex_bytes: source_vertices.len() as u32,
            index_bytes: 0,
            texture_bytes: [0; 4],
            payload_dropped: false,
            render_state: [0; 256],
            payload_hash: 0,
            payload: Arc::from(source_vertices.clone()),
            material_key: 1,
            state_key: 1,
            scene_light_binding: None,
        };
        assert!(PacketFilter::Flat.accepts(&packet));
        assert!(!PacketFilter::Textured.accepts(&packet));
        assert!(!PacketFilter::VertexShader.accepts(&packet));
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut expanded = Vec::new();
        for source in source_vertices.chunks_exact(16) {
            expanded.extend_from_slice(source);
            expanded.extend_from_slice(&[0; 8]);
        }
        let mut layout_packet = packet.clone();
        layout_packet.vertex_declaration = 13;
        layout_packet.vertex_stride = 24;
        layout_packet.vertex_bytes = expanded.len() as u32;
        layout_packet.cull_mode = 3; // captured D3DCULL_CCW
        layout_packet.depth_enable = 1;
        layout_packet.depth_write = 1;
        layout_packet.depth_function = 4; // D3DCMP_LESSEQUAL
        layout_packet.stencil_enable = 1;
        layout_packet.stencil_function = 8; // D3DCMP_ALWAYS
        layout_packet.stencil_reference = 1;
        layout_packet.stencil_mask = u32::MAX;
        layout_packet.stencil_write_mask = u32::MAX;
        layout_packet.stencil_fail = 1; // D3DSTENCILOP_KEEP
        layout_packet.stencil_depth_fail = 1;
        layout_packet.stencil_pass = 3; // D3DSTENCILOP_REPLACE
        layout_packet.payload = Arc::from(expanded);
        assert_eq!(
            packet_material_class(&layout_packet),
            RoomMaterialClass::FlatVertexColour
        );
        let range =
            append_simple_room_draw(&mut vertices, &mut indices, &layout_packet, width, height)
                .expect("captured XYZ+diffuse+UV layout with CCW culling is native eligible");
        assert_eq!(range.cull, RoomCullMode::CounterClockwise);
        assert!(range.stencil.enabled);
        // Exercise the production ordered room-pass encoder with an owned
        // packet rather than drawing a test-built range directly.
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("T2238 native pixel-diff target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let shared = Arc::new(Shared::default());
        *shared.packet.lock().unwrap() = Some(ScenePacket {
            draw_packets: vec![layout_packet],
            ..ScenePacket::default()
        });
        shared.has_packet.store(true, Ordering::Release);
        let mut layer = NativeSceneLayer {
            shared,
            pipeline: None,
            room_pipelines: HashMap::new(),
            room_light_preview_pipelines: HashMap::new(),
            scene_light_bind_group_layout: None,
            textured_pipeline: None,
            shader_pipelines: HashMap::new(),
            shader_programs: HashMap::new(),
            packet_filter: PacketFilter::All,
            rendered_present: AtomicU64::new(0),
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let view = target.create_view(&Default::default());
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("T2238 isolated room-pass pixel-diff clear"),
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
        }
        assert!(layer.encode(crate::frame_layer::LayerTarget {
            device: &device,
            queue: &queue,
            encoder: &mut encoder,
            target: &target,
            frame: 1,
            present: 1,
        }));
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("T2238 native pixel-diff readback"),
            size: u64::from(width * height * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
        device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let native_rgba = slice.get_mapped_range().to_vec();
        readback.unmap();

        let hex = source_vertices
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let legacy_stream = Stream::from_text(&format!(
            "mem 40002000 {hex}\nhost 19240 40002000 10\ncall DrawVertices 8 0 4\npresent\n"
        ))
        .unwrap();
        let mut legacy = Replayer::new(gpu, width, height);
        legacy.run(&legacy_stream);
        let diff =
            pixel_diff_rgba8(&legacy.frames[0].rgba, &native_rgba).expect("matching extents");
        assert_eq!(diff.changed_pixels, 0, "native-vs-legacy diff: {diff:?}");
        assert_eq!(diff.max_channel_delta, 0);

        // The next material class runs the captured program through the same
        // translator used by Replayer's GPU VSH path, then compares its frame
        // with the legacy CPU interpreter on the equivalent synthetic stream.
        fn mov(input: u32, output: u32, last: bool) -> [u32; 4] {
            [
                0,
                (1 << 21) | (input << 9) | 0x1b,
                2 << 26,
                (output << 3) | (1 << 11) | (0xF << 12) | u32::from(last),
            ]
        }
        let instructions = [mov(1, 0, false), mov(2, 3, false), mov(3, 9, true)];
        let mut program_bytes = vec![0u8; 136 * 16];
        for (slot, instruction) in instructions.iter().enumerate() {
            for (word, bytes) in instruction
                .iter()
                .zip(program_bytes[slot * 16..slot * 16 + 16].chunks_exact_mut(4))
            {
                bytes.copy_from_slice(&word.to_le_bytes());
            }
        }
        let constants = vec![0u8; 192 * 16];
        let mut shader_memory = crate::vsh::ProgramMemory::default();
        shader_memory.slots[..instructions.len()].copy_from_slice(&instructions);
        let mut shader_vertices = Vec::new();
        for (x, y, colour) in [
            (100.0f32, 100.0f32, 0xFFFF_0000u32),
            (400.0, 100.0, 0xFF00_FF00),
            (100.0, 400.0, 0xFF00_00FF),
        ] {
            for value in [x, y, 0.5] {
                shader_vertices.extend_from_slice(&value.to_le_bytes());
            }
            shader_vertices.extend_from_slice(&colour.to_le_bytes());
            shader_vertices.extend_from_slice(&0.0f32.to_le_bytes());
            shader_vertices.extend_from_slice(&0.0f32.to_le_bytes());
        }
        let mut shader_packet = packet.clone();
        shader_packet.primitive = 5;
        shader_packet.count = 3;
        shader_packet.vertex_stride = 24;
        shader_packet.vertex_declaration = 13;
        shader_packet.cull_mode = 3;
        shader_packet.vertex_shader_handle = 1;
        shader_packet.vertex_shader_index = 0;
        shader_packet.vertex_program_bytes = program_bytes.len() as u32;
        shader_packet.vertex_constants_bytes = constants.len() as u32;
        shader_packet.vertex_program_hash = crate::vsh_wgsl::program_key(&shader_memory, 0);
        shader_packet.vertex_bytes = shader_vertices.len() as u32;
        shader_packet.vertex_buffer_bytes = shader_vertices.len() as u32;
        shader_packet.payload = Arc::from(
            shader_vertices
                .iter()
                .chain(program_bytes.iter())
                .chain(constants.iter())
                .copied()
                .collect::<Vec<_>>(),
        );
        assert!(PacketFilter::VertexShader.accepts(&shader_packet));
        assert!(!PacketFilter::Flat.accepts(&shader_packet));
        assert_eq!(crate::vsh_wgsl::extent(&shader_memory, 0).len(), 3);
        assert_eq!(
            crate::vsh_wgsl::translate(&shader_memory, 0).inputs,
            vec![1, 2, 3]
        );
        assert_eq!(
            crate::vertex::triangulate(shader_packet.primitive, shader_packet.count),
            Some(vec![0, 1, 2])
        );
        let shader_shared = Arc::new(Shared::default());
        *shader_shared.packet.lock().unwrap() = Some(ScenePacket {
            draw_packets: vec![shader_packet.clone()],
            ..ScenePacket::default()
        });
        shader_shared.has_packet.store(true, Ordering::Release);
        let mut shader_layer = NativeSceneLayer {
            shared: shader_shared,
            pipeline: None,
            room_pipelines: HashMap::new(),
            room_light_preview_pipelines: HashMap::new(),
            scene_light_bind_group_layout: None,
            textured_pipeline: None,
            shader_pipelines: HashMap::new(),
            shader_programs: HashMap::new(),
            packet_filter: PacketFilter::All,
            rendered_present: AtomicU64::new(0),
        };
        let shader_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("T2238 production translated VSH pixel-diff target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut shader_encoder = device.create_command_encoder(&Default::default());
        {
            let view = shader_target.create_view(&Default::default());
            let _pass = shader_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("T2238 production translated VSH pixel-diff clear"),
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
        }
        assert!(shader_layer.encode(crate::frame_layer::LayerTarget {
            device: &device,
            queue: &queue,
            encoder: &mut shader_encoder,
            target: &shader_target,
            frame: 1,
            present: 1,
        }));
        let shader_readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("T2238 production translated VSH readback"),
            size: u64::from(width * height * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        shader_encoder.copy_texture_to_buffer(
            shader_target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &shader_readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            shader_target.size(),
        );
        queue.submit([shader_encoder.finish()]);
        let shader_slice = shader_readback.slice(..);
        let (shader_tx, shader_rx) = std::sync::mpsc::channel();
        shader_slice.map_async(wgpu::MapMode::Read, move |result| {
            shader_tx.send(result).unwrap()
        });
        device.poll(wgpu::Maintain::Wait);
        shader_rx.recv().unwrap().unwrap();
        let shader_rgba = shader_slice.get_mapped_range().to_vec();
        shader_readback.unmap();

        let declaration = {
            let mut words = vec![0u32; 64];
            for entry in 0..16 {
                words[entry * 4 + 2] = 2;
            }
            words[5] = 0;
            words[6] = 0x32;
            words[9] = 12;
            words[10] = 0x40;
            words[13] = 16;
            words[14] = 0x22;
            words
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let shader_program = format!(
            "0300{}",
            program_bytes[..48]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let shader_vertex_hex = shader_vertices
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let shader_stream = Stream::from_text(&format!(
            "mem 40000002 {shader_program}\ncall LoadVertexProgram 40000000 0\nmem 40001000 {declaration}\ncall SelectVertexDeclaration 40001000 0\nmem 40002000 {shader_vertex_hex}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\npresent\n"
        )).unwrap();
        let legacy_gpu = GpuContext::new(&Preference::Software)
            .expect("software adapter for legacy VSH reference");
        let mut shader_legacy = Replayer::new(legacy_gpu, width, height);
        shader_legacy.set_gpu_vsh(Some(false));
        shader_legacy.run(&shader_stream);
        let shader_diff = pixel_diff_rgba8(&shader_legacy.frames[0].rgba, &shader_rgba)
            .expect("matching translated VSH frame extents");
        assert_eq!(
            shader_diff.changed_pixels, 0,
            "production translated VSH versus legacy: {shader_diff:?}"
        );
        assert_eq!(shader_diff.max_channel_delta, 0);
    }

    #[test]
    fn production_textured_room_pass_matches_legacy_pixels() {
        use crate::context::{GpuContext, Preference};
        use crate::replay::Replayer;
        use crate::stream::Stream;

        let gpu = match GpuContext::new(&Preference::Software) {
            Ok(gpu) => gpu,
            Err(error) => {
                println!("SKIPPED (no software adapter): {error}");
                return;
            }
        };
        let device = gpu.device.clone();
        let queue = gpu.queue.clone();
        let (width, height) = (640u32, 480u32);
        let format = (6 << 8) | (1 << 20) | (1 << 24) | 0x29;
        // Mix failing and passing alpha-test samples. The pixel comparison below mirrors
        // the captured D3D alpha/blend state in the Replayer pushbuffer.
        let texel_words = [0x40FF_0000u32, 0x8000_FF00, 0xFFFF_00FF, 0x0000_FFFF];
        let texture_bytes: Vec<u8> = texel_words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        let mut vertex_bytes = Vec::new();
        for (x, y, u, v) in [
            (100.0f32, 100.0f32, 0.25f32, 0.25f32),
            (400.0, 100.0, 0.75, 0.25),
            (100.0, 400.0, 0.25, 0.75),
        ] {
            for value in [x, y, 0.5] {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
            vertex_bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
            vertex_bytes.extend_from_slice(&u.to_le_bytes());
            vertex_bytes.extend_from_slice(&v.to_le_bytes());
        }
        let mut shader_words = diffuse_texture_words();
        shader_words[0x68 / 4] = 0x0000_00C0;
        shader_words[0xB4 / 4] = 0x0000_00C0;
        let shader_bytes: Vec<u8> = shader_words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        let mut payload = vertex_bytes.clone();
        payload.extend_from_slice(&texture_bytes);
        payload.extend_from_slice(&shader_bytes);
        let mut texture_stage_state = [0; 128];
        texture_stage_state[13] = 3; // clamp U
        texture_stage_state[14] = 3; // clamp V
        texture_stage_state[16] = 1; // point magnification
        texture_stage_state[17] = 1; // point minification
        texture_stage_state[18] = 0; // no mip filter
        texture_stage_state[19] = 1; // default anisotropy
        let mut packet = RoomDrawPacket {
            frame_sequence: 1,
            room: 7,
            indexed: 0,
            primitive: 5,
            first: 0,
            count: 3,
            vertex_buffer: 0x4000_2000,
            vertex_buffer_bytes: vertex_bytes.len() as u32,
            vertex_stride: 24,
            vertex_first: 0,
            index_buffer: 0,
            index_buffer_bytes: 0,
            texture_headers: [0x4200_0000, 0, 0, 0],
            texture_data: [0x8000_1000, 0, 0, 0],
            texture_formats: [format, 0, 0, 0],
            texture_sizes: [0; 4],
            texture_stage_state,
            viewport_words: [0; 4],
            viewport_target: [0; 4],
            scissor_words: [0; 4],
            scissor_count: 0,
            scissor_exclusive: 0,
            vertex_declaration: 13,
            vertex_shader_handle: 0,
            vertex_shader_index: 0,
            vertex_program_bytes: 0,
            vertex_constants_bytes: 0,
            pixel_shader_bytes: 240,
            vertex_program_hash: 0,
            vertex_constants_hash: 0,
            pixel_shader_hash: 0,
            blend_enable: 0,
            blend_source: 0,
            blend_destination: 0,
            blend_operation: 0,
            depth_enable: 0,
            depth_write: 0,
            depth_function: 0,
            cull_mode: 3,
            stencil_enable: 0,
            stencil_function: 0,
            stencil_reference: 0,
            stencil_mask: 0,
            stencil_write_mask: 0,
            stencil_fail: 0,
            stencil_depth_fail: 0,
            stencil_pass: 0,
            vertex_bytes: vertex_bytes.len() as u32,
            index_bytes: 0,
            texture_bytes: [texture_bytes.len() as u32, 0, 0, 0],
            payload_dropped: false,
            render_state: [0; 256],
            payload_hash: 0,
            payload: Arc::from(payload),
            material_key: 2,
            state_key: 2,
            scene_light_binding: None,
        };
        // Exercise non-opaque source alpha as well as GREATER alpha-test rejection.
        packet.blend_enable = 1;
        packet.blend_source = 5;
        packet.blend_destination = 6;
        packet.blend_operation = 1;
        packet.render_state[15] = 1;
        packet.render_state[24] = 127;
        packet.render_state[25] = 5;
        assert_eq!(
            packet_material_class(&packet),
            RoomMaterialClass::TexturedModulate
        );
        assert!(PacketFilter::Alpha.accepts(&packet));
        assert!(!PacketFilter::Textured.accepts(&packet));
        let mut unblended = packet.clone();
        unblended.blend_enable = 0;
        unblended.render_state[15] = 0;
        assert!(PacketFilter::Textured.accepts(&unblended));
        assert!(!PacketFilter::Alpha.accepts(&unblended));
        let native = append_textured_room_draw(&packet, width, height)
            .expect("owned swizzled texture and supported combiner are eligible");
        assert_eq!(native.combiner_scale, 1.0);
        let mut depth_stencil_packet = packet.clone();
        depth_stencil_packet.depth_enable = 1;
        depth_stencil_packet.depth_write = 1;
        depth_stencil_packet.depth_function = 4;
        depth_stencil_packet.stencil_enable = 1;
        depth_stencil_packet.stencil_function = 4;
        depth_stencil_packet.stencil_reference = 5;
        depth_stencil_packet.stencil_mask = 0xff;
        depth_stencil_packet.stencil_write_mask = 0xff;
        depth_stencil_packet.stencil_fail = 1;
        depth_stencil_packet.stencil_depth_fail = 1;
        depth_stencil_packet.stencil_pass = 3;
        let stateful = append_textured_room_draw(&depth_stencil_packet, width, height)
            .expect("captured textured depth/stencil state remains eligible");
        assert!(stateful.depth.enabled && stateful.depth.write);
        assert!(stateful.stencil.enabled);
        assert_eq!(stateful.stencil_reference, 5);
        let mut scaled_packet = packet.clone();
        let scaled_words = diffuse_texture_words();
        let scaled_bytes: Vec<u8> = scaled_words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        let combiner_start =
            scaled_packet.vertex_bytes as usize + scaled_packet.texture_bytes[0] as usize;
        Arc::make_mut(&mut scaled_packet.payload)
            [combiner_start..combiner_start + scaled_bytes.len()]
            .copy_from_slice(&scaled_bytes);
        let scaled = append_textured_room_draw(&scaled_packet, width, height)
            .expect("captured 2x combiner variant remains native eligible");
        assert_eq!(scaled.combiner_scale, 2.0);
        let shared = Arc::new(Shared::default());
        *shared.packet.lock().unwrap() = Some(ScenePacket {
            draw_packets: vec![packet.clone()],
            ..ScenePacket::default()
        });
        shared.has_packet.store(true, Ordering::Release);
        let mut layer = NativeSceneLayer {
            shared,
            pipeline: None,
            room_pipelines: HashMap::new(),
            room_light_preview_pipelines: HashMap::new(),
            scene_light_bind_group_layout: None,
            textured_pipeline: None,
            shader_pipelines: HashMap::new(),
            shader_programs: HashMap::new(),
            packet_filter: PacketFilter::All,
            rendered_present: AtomicU64::new(0),
        };
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("T2238 production textured room-pass pixel-diff target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let view = target.create_view(&Default::default());
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("T2238 production textured room-pass pixel-diff clear"),
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
        }
        assert!(layer.encode(crate::frame_layer::LayerTarget {
            device: &device,
            queue: &queue,
            encoder: &mut encoder,
            target: &target,
            frame: 1,
            present: 1,
        }));
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("T2238 textured native pixel-diff readback"),
            size: u64::from(width * height * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
        device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let native_rgba = slice.get_mapped_range().to_vec();
        readback.unmap();

        let hex = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let record_words = [0x0004_0001u32, 0x8000_1000, 0, format, 0];
        let record: Vec<u8> = record_words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        let legacy_stream = Stream::from_text(&format!(
            "mem 42000000 {}\nmem 80001000 {}\nmem 40003000 {}\ncall SetPixelShader 40003000\ncall SetTexture 0 42000000\ncall DeferTextureStageWord 0 0 1\nlib 1b08 303\ncall PushMethod 40300 1\ncall PushMethod 4033c 204\ncall PushMethod 40340 7f\ncall PushMethod 40304 1\ncall PushMethod 40344 302\ncall PushMethod 40348 303\ncall PushMethod 40350 8006\nhost 19240 40002000 18\nmem 40002000 {}\ncall DrawVertices 5 0 3\npresent\n",
            hex(&record),
            hex(&texture_bytes),
            hex(&shader_bytes),
            hex(&vertex_bytes),
        ))
        .unwrap();
        let mut legacy = Replayer::new(gpu, width, height);
        legacy.run(&legacy_stream);
        let diff = pixel_diff_rgba8(&legacy.frames[0].rgba, &native_rgba)
            .expect("matching legacy/native image extents");
        assert_eq!(
            diff.changed_pixels, 0,
            "textured native-vs-legacy diff: {diff:?}"
        );
        assert_eq!(diff.max_channel_delta, 0);
    }

    fn diffuse_texture_words() -> [u32; 60] {
        let mut words = [0u32; 60];
        words[0x00 / 4] = 0xD4D8_1010;
        words[0x20 / 4] = 0x0000_000C;
        words[0x24 / 4] = 0x0000_1C80;
        words[0x68 / 4] = 0x0001_00C0;
        words[0x88 / 4] = 0xC4C8_0000;
        words[0xB4 / 4] = 0x0001_00C0;
        words[0xD4 / 4] = 0x0001_1101;
        words[0xD8 / 4] = 1;
        words
    }
}
