// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded producer-side semantic observations for offline renderer comparison.
//!
//! This format records only Rust-owned values already present at the scene
//! producer. It is deliberately an observation format, not a DrawList codec:
//! missing material/model mappings are explicit and cannot become equality.

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::sync::{Mutex, OnceLock};

use tsfp_engine_render::{
    native_scene::RoomDrawPacket,
    scene::light_state::{PublishedSceneLights, SceneLightBinding},
};
use tsfp_engine_render::scene::render_prep::PreparedSceneFrame;

const MAGIC: &[u8; 8] = b"TSFPSC05";
const MAX_PACKETS: usize = 4_096;
const MAX_EFFECTS: usize = 65_536;
const MAX_VIEWS: usize = 16;
const MAX_OBJECTS_PER_VIEW: usize = 0x226;
const MAX_ROOMS: usize = 200;
const MAX_LIGHT_BLOCKS: usize = 65_536;
const DEFAULT_CAPTURE_FRAMES: usize = 256;
const MAX_CAPTURE_FRAMES: usize = 4_096;

struct CaptureState {
    output: Option<BufWriter<File>>,
    frames: usize,
    frame_limit: usize,
    limit_reported: bool,
}

static CAPTURE: OnceLock<Mutex<CaptureState>> = OnceLock::new();

pub fn enabled() -> bool {
    std::env::var_os("TSFP_SEMANTIC_SCENE_CAPTURE").is_some()
}

fn put_f32(record: &mut Vec<u8>, value: f32) {
    record.extend_from_slice(&value.to_bits().to_le_bytes());
}

fn append_camera_handoff(record: &mut Vec<u8>, camera: CameraHandoffObservation) {
    record.extend_from_slice(b"CAMR");
    record.extend_from_slice(&camera.camera_valid.to_le_bytes());
    record.extend_from_slice(&camera.viewer.to_le_bytes());
    record.extend_from_slice(&camera.room_count.to_le_bytes());
    record.extend_from_slice(&camera.max_depth.to_le_bytes());
    for value in camera
        .sample_position
        .iter()
        .chain(camera.sample_matrix.iter())
        .chain(camera.viewport.iter())
        .chain(camera.owner_eye.iter())
        .chain(camera.owner_view.iter())
        .chain(camera.owner_projection.iter())
        .chain(camera.owner_tangent_extents.iter())
    {
        put_f32(record, *value);
    }
    for value in camera
        .rectangle
        .iter()
        .chain(camera.owner_visible_rooms.iter())
    {
        record.extend_from_slice(&value.to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Completeness {
    pub cameras: bool,
    pub ordered_room_draws: bool,
    pub lights: bool,
    /// Every captured draw's typed light binding resolves to one published block.
    pub draw_light_bindings: bool,
    pub effects: bool,
    /// The current producer has no complete typed material/model resource map.
    pub materials_and_models: bool,
}

/// Why a producer observation did not become a prepared frame. These values
/// are diagnostic metadata only; rejected observations never reach a renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FrameDisposition {
    Prepared = 0,
    MissingCamera = 1,
    InvalidView = 2,
    CameraMismatch = 3,
    StalePublication = 4,
    RoomPlanMismatch = 5,
    ObjectInputsRequired = 6,
    OtherRejected = 7,
}

/// Camera candidates and target metadata at the producer boundary. Keeping
/// both camera sides makes even rejected handoffs useful without choosing a
/// winner; accepted observations remain raw evidence, not completeness proof.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraHandoffObservation {
    pub camera_valid: u32,
    pub viewer: u32,
    pub room_count: u32,
    pub max_depth: i32,
    pub sample_position: [f32; 3],
    pub sample_matrix: [f32; 16],
    pub viewport: [f32; 4],
    pub rectangle: [i32; 4],
    pub owner_eye: [f32; 3],
    pub owner_view: [f32; 16],
    pub owner_projection: [f32; 16],
    pub owner_tangent_extents: [f32; 2],
    pub owner_visible_rooms: [i32; 2],
}

impl CameraHandoffObservation {
    pub fn from_native_view(view: &tsfp_render_prep::live::NativeSceneView) -> Self {
        Self {
            camera_valid: view.camera_valid,
            viewer: view.viewer,
            room_count: view.room_count,
            max_depth: view.max_depth,
            sample_position: view.position,
            sample_matrix: view.matrix,
            viewport: view.viewport,
            rectangle: view.rectangle,
            owner_eye: view.camera.eye,
            owner_view: view.camera.view,
            owner_projection: view.camera.projection,
            owner_tangent_extents: view.camera.tangent_extents,
            owner_visible_rooms: view.camera.visible_rooms,
        }
    }
}

/// Preserve a rejected inlet observation without submitting it to a renderer.
pub fn capture_live_rejection(
    owner: &tsfp_engine_render::native_scene::NativeSceneOwner,
    view: &tsfp_render_prep::live::NativeSceneView,
    error: &tsfp_render_prep::live::LiveError,
) {
    use tsfp_render_prep::live::LiveError;
    if !enabled() {
        return;
    }
    let disposition = match error {
        LiveError::MissingCamera => FrameDisposition::MissingCamera,
        LiveError::InvalidView => FrameDisposition::InvalidView,
        LiveError::CameraMismatch => FrameDisposition::CameraMismatch,
        LiveError::StalePublication => FrameDisposition::StalePublication,
        LiveError::RoomPlanMismatch => FrameDisposition::RoomPlanMismatch,
        LiveError::ObjectInputsRequired => FrameDisposition::ObjectInputsRequired,
        _ => FrameDisposition::OtherRejected,
    };
    let camera = CameraHandoffObservation::from_native_view(view);
    match owner.prepare_original_frame(&[]) {
        Ok(frame) => capture_rejected(&frame, owner.room_draw_packets(), disposition, camera),
        Err(capture_error) => eprintln!(
            "rejected render-prep sample {error:?}; semantic observation unavailable: {capture_error:?}"
        ),
    }
}

impl Completeness {
    pub fn draw_list_complete(self) -> bool {
        self.cameras
            && self.ordered_room_draws
            && self.lights
            && self.draw_light_bindings
            && self.effects
            && self.materials_and_models
    }

    fn bits(self) -> u32 {
        u32::from(self.cameras)
            | (u32::from(self.ordered_room_draws) << 1)
            | (u32::from(self.lights) << 2)
            | (u32::from(self.effects) << 3)
            | (u32::from(self.materials_and_models) << 4)
            | (u32::from(self.draw_light_bindings) << 5)
    }
}

fn light_block_indices(
    lights: Option<&PublishedSceneLights>,
    bindings: &[Option<SceneLightBinding>],
) -> (bool, Vec<u32>) {
    let Some(lights) = lights.filter(|lights| lights.blocks.len() <= MAX_LIGHT_BLOCKS) else {
        return (false, vec![0; bindings.len()]);
    };
    let mut complete = !bindings.is_empty();
    let indices = bindings
        .iter()
        .map(|binding| {
            let Some(binding) = binding else {
                complete = false;
                return 0;
            };
            let Some(block) = lights.block_for_binding(*binding) else {
                complete = false;
                return 0;
            };
            lights
                .blocks
                .iter()
                .position(|candidate| std::ptr::eq(candidate, block))
                .and_then(|index| u32::try_from(index + 1).ok())
                .unwrap_or_else(|| {
                    complete = false;
                    0
                })
        })
        .collect();
    (complete, indices)
}

/// Append one producer frame. Integers are little endian; the length prefix
/// lets readers skip future record versions safely. Payload bytes and guest
/// pointers are never copied into this sidecar.
pub fn write_frame(
    out: &mut impl Write,
    frame: &PreparedSceneFrame,
    packets: &[RoomDrawPacket],
) -> io::Result<Completeness> {
    write_frame_observation(out, frame, packets, FrameDisposition::Prepared, None)
}

/// Append a prepared frame together with the raw camera and target handoff
/// copied at the live producer boundary. Camera completeness still depends on
/// a typed prepared view; this record only preserves its source evidence.
pub fn write_frame_with_camera(
    out: &mut impl Write,
    frame: &PreparedSceneFrame,
    packets: &[RoomDrawPacket],
    camera: CameraHandoffObservation,
) -> io::Result<Completeness> {
    write_frame_observation(out, frame, packets, FrameDisposition::Prepared, Some(camera))
}

/// Serialize an explicitly rejected producer sample. The camera candidates
/// are retained as observations and camera completeness remains false.
pub fn write_rejected_frame(
    out: &mut impl Write,
    frame: &PreparedSceneFrame,
    packets: &[RoomDrawPacket],
    disposition: FrameDisposition,
    camera: CameraHandoffObservation,
) -> io::Result<Completeness> {
    if disposition == FrameDisposition::Prepared {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "rejected frame must carry a rejected disposition",
        ));
    }
    write_frame_observation(out, frame, packets, disposition, Some(camera))
}

fn write_frame_observation(
    out: &mut impl Write,
    frame: &PreparedSceneFrame,
    packets: &[RoomDrawPacket],
    disposition: FrameDisposition,
    camera_handoff: Option<CameraHandoffObservation>,
) -> io::Result<Completeness> {
    if packets.len() > MAX_PACKETS
        || frame.views.len() > MAX_VIEWS
        || frame.effects.len() > MAX_EFFECTS
        || frame
            .scene_lights
            .as_ref()
            .is_some_and(|lights| lights.blocks.len() > MAX_LIGHT_BLOCKS)
        || frame.observed_objects.len() > MAX_OBJECTS_PER_VIEW
        || frame.views.iter().any(|view| {
            view.objects.len() > MAX_OBJECTS_PER_VIEW
                || view
                    .objects
                    .iter()
                    .any(|object| object.rooms.len() > MAX_ROOMS)
        })
        || frame
            .room_plan
            .as_ref()
            .is_some_and(|plan| plan.rooms.len() > MAX_ROOMS)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "semantic scene frame exceeds a bounded collection limit",
        ));
    }
    let (draw_light_bindings, draw_light_block_indices) = light_block_indices(
        frame.scene_lights.as_ref(),
        &packets
            .iter()
            .map(|packet| packet.scene_light_binding)
            .collect::<Vec<_>>(),
    );
    let complete = Completeness {
        cameras: disposition == FrameDisposition::Prepared
            && !frame.views.is_empty()
            && frame
                .views
                .iter()
                .all(|view| view.camera.is_some() && view.target.is_some()),
        // The current bridge does not report an expected draw count, so even
        // a clean packet list cannot prove that the producer did not omit a draw.
        ordered_room_draws: false,
        // Block identity fields are private and the serialized capture cannot
        // yet join a light block to a view/draw without guessing by array order.
        lights: false,
        draw_light_bindings,
        effects: frame.effects_tick == Some(frame.tick),
        materials_and_models: false,
    };

    let mut record = Vec::with_capacity(48 + packets.len() * 64);
    record.extend_from_slice(&frame.tick.to_le_bytes());
    record.extend_from_slice(&frame.alpha.to_bits().to_le_bytes());
    record.extend_from_slice(&complete.bits().to_le_bytes());
    record.extend_from_slice(&(frame.views.len() as u32).to_le_bytes());
    record.extend_from_slice(&(frame.observed_objects.len() as u32).to_le_bytes());
    record.extend_from_slice(&(frame.effects.len() as u32).to_le_bytes());
    record.extend_from_slice(&(packets.len() as u32).to_le_bytes());
    record.extend_from_slice(&(disposition as u32).to_le_bytes());
    if let Some(camera) = camera_handoff {
        append_camera_handoff(&mut record, camera);
    }
    if let Some(plan) = &frame.room_plan {
        record.extend_from_slice(&1u32.to_le_bytes());
        record.extend_from_slice(&plan.tick.to_le_bytes());
        record.extend_from_slice(&plan.draw_max_depth.to_le_bytes());
        record.extend_from_slice(&(plan.rooms.len() as u32).to_le_bytes());
        record.extend(plan.rooms.iter().copied());
    } else {
        record.extend_from_slice(&0u32.to_le_bytes());
    }
    for view in &frame.views {
        record.extend_from_slice(&view.tick.to_le_bytes());
        record.extend_from_slice(&u32::from(view.camera.is_some()).to_le_bytes());
        record.extend_from_slice(&u32::from(view.target.is_some()).to_le_bytes());
        record.extend_from_slice(&(view.objects.len() as u32).to_le_bytes());
        if let Some(camera) = &view.camera {
            for value in camera.eye {
                put_f32(&mut record, value);
            }
            for value in camera.view.iter().chain(camera.projection.matrix.iter()) {
                put_f32(&mut record, *value);
            }
            for value in camera.projection.tangent_extents {
                put_f32(&mut record, value);
            }
            for room in camera.visible_rooms {
                record.extend_from_slice(&room.to_le_bytes());
            }
        }
        if let Some(target) = &view.target {
            record.extend_from_slice(&target.viewer.to_le_bytes());
            for value in [
                target.viewport.x_offset,
                target.viewport.x_scale,
                target.viewport.y_offset,
                target.viewport.y_scale,
            ] {
                put_f32(&mut record, value);
            }
            for value in [
                target.rectangle.left,
                target.rectangle.right,
                target.rectangle.top,
                target.rectangle.bottom,
            ] {
                record.extend_from_slice(&value.to_le_bytes());
            }
        }
        for object in &view.objects {
            record.extend_from_slice(&object.id.slot.to_le_bytes());
            record.extend_from_slice(&object.id.generation.to_le_bytes());
            for value in object.placement.position {
                put_f32(&mut record, value);
            }
            for value in object.placement.orientation.iter().flatten() {
                put_f32(&mut record, *value);
            }
            record.extend_from_slice(&(object.rooms.len() as u32).to_le_bytes());
            for room in &object.rooms {
                record.extend_from_slice(&room.to_le_bytes());
            }
        }
    }
    if let Some(lights) = &frame.scene_lights {
        record.extend_from_slice(&lights.epoch.0.to_le_bytes());
        record.extend_from_slice(&(lights.blocks.len() as u32).to_le_bytes());
        for (index, block) in lights.blocks.iter().enumerate() {
            record.extend_from_slice(&((index + 1) as u32).to_le_bytes());
            for slot in block.slots {
                record.extend_from_slice(&u32::from(slot.enabled).to_le_bytes());
                for value in slot
                    .position
                    .0
                    .into_iter()
                    .chain(slot.color.0)
                    .chain(slot.parameters)
                {
                    put_f32(&mut record, value);
                }
            }
            for value in block.fourth_color.0 {
                put_f32(&mut record, value);
            }
        }
    }
    for effect in &frame.effects {
        record.extend_from_slice(&effect.handle.slot().to_le_bytes());
        record.extend_from_slice(&effect.handle.generation().to_le_bytes());
        record.extend_from_slice(&effect.effect_type.0.to_le_bytes());
        for value in effect
            .position
            .into_iter()
            .chain([effect.age, effect.lifetime])
            .chain(effect.visual.tint_rgba)
            .chain(effect.visual.scale)
            .chain([effect.visual.rotation_radians])
        {
            put_f32(&mut record, value);
        }
    }
    for (packet, light_block_index) in packets.iter().zip(draw_light_block_indices) {
        record.extend_from_slice(&light_block_index.to_le_bytes());
        for value in [
            packet.frame_sequence as u32,
            (packet.frame_sequence >> 32) as u32,
            packet.room,
            packet.indexed,
            packet.primitive,
            packet.first,
            packet.count,
            packet.vertex_stride,
            packet.vertex_first,
            packet.vertex_bytes,
            packet.index_bytes,
            packet.vertex_declaration,
            packet.vertex_shader_handle,
            packet.vertex_shader_index,
        ] {
            record.extend_from_slice(&value.to_le_bytes());
        }
        for value in [
            packet.material_key,
            packet.state_key,
            packet.payload_hash,
            packet.vertex_program_hash,
            packet.vertex_constants_hash,
            packet.pixel_shader_hash,
        ] {
            record.extend_from_slice(&value.to_le_bytes());
        }
        for value in packet
            .texture_formats
            .iter()
            .chain(packet.texture_sizes.iter())
            .chain(packet.texture_stage_state.iter())
            .chain(packet.viewport_words.iter())
            .chain(packet.viewport_target.iter())
            .chain(packet.scissor_words.iter())
            .chain(packet.render_state.iter())
        {
            record.extend_from_slice(&value.to_le_bytes());
        }
        for value in [
            packet.scissor_count,
            packet.scissor_exclusive,
            packet.blend_enable,
            packet.blend_source,
            packet.blend_destination,
            packet.blend_operation,
            packet.depth_enable,
            packet.depth_write,
            packet.depth_function,
            packet.cull_mode,
            packet.stencil_enable,
            packet.stencil_function,
            packet.stencil_reference,
            packet.stencil_mask,
            packet.stencil_write_mask,
            packet.stencil_fail,
            packet.stencil_depth_fail,
            packet.stencil_pass,
        ] {
            record.extend_from_slice(&value.to_le_bytes());
        }
        record.extend_from_slice(&u32::from(packet.payload_dropped).to_le_bytes());
    }
    out.write_all(MAGIC)?;
    out.write_all(&(record.len() as u64).to_le_bytes())?;
    out.write_all(&record)?;
    Ok(complete)
}

/// Submit to the ordinary audit sink and, when requested, append the same
/// producer frame to a sidecar. `create_new` prevents clobbering an earlier
/// recording. The path should live under the caller's `tmp/` recording tree.
pub fn submit(
    sink: &mut impl tsfp_engine_render::native_scene::SceneFrameSink,
    frame: &PreparedSceneFrame,
    packets: &[RoomDrawPacket],
    view: &tsfp_render_prep::live::NativeSceneView,
) {
    sink.submit(frame);
    capture_observation(
        frame,
        packets,
        FrameDisposition::Prepared,
        Some(CameraHandoffObservation::from_native_view(view)),
    );
}

/// Record the Rust-owned state associated with a producer sample rejected by
/// the live inlet. This does not submit the incomplete frame to the audit or
/// native render sinks.
pub fn capture_rejected(
    frame: &PreparedSceneFrame,
    packets: &[RoomDrawPacket],
    disposition: FrameDisposition,
    camera: CameraHandoffObservation,
) {
    capture_observation(frame, packets, disposition, Some(camera));
}

fn capture_observation(
    frame: &PreparedSceneFrame,
    packets: &[RoomDrawPacket],
    disposition: FrameDisposition,
    camera: Option<CameraHandoffObservation>,
) {
    let Ok(path) = std::env::var("TSFP_SEMANTIC_SCENE_CAPTURE") else {
        return;
    };
    let capture = CAPTURE.get_or_init(|| {
        let opened = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map(BufWriter::new)
            .map_err(|error| {
                eprintln!("tsfp-scene-capture: unable to create sidecar at {path}: {error}");
                error
            })
            .ok();
        let frame_limit = std::env::var("TSFP_SEMANTIC_SCENE_CAPTURE_FRAMES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(DEFAULT_CAPTURE_FRAMES)
            .clamp(1, MAX_CAPTURE_FRAMES);
        Mutex::new(CaptureState {
            output: opened,
            frames: 0,
            frame_limit,
            limit_reported: false,
        })
    });
    match capture.lock() {
        Ok(mut state) => {
            if state.frames >= state.frame_limit {
                if !state.limit_reported {
                    state.limit_reported = true;
                    eprintln!(
                        "tsfp-scene-capture: frame limit {} reached; later observations omitted",
                        state.frame_limit
                    );
                }
                return;
            }
            if let Some(output) = state.output.as_mut() {
                let result = match camera {
                    Some(camera) if disposition == FrameDisposition::Prepared => {
                        write_frame_with_camera(output, frame, packets, camera)
                    }
                    Some(camera) => {
                        write_rejected_frame(output, frame, packets, disposition, camera)
                    }
                    None => write_frame_observation(output, frame, packets, disposition, None),
                }
                .and_then(|_| output.flush());
                if let Err(error) = result {
                    state.output = None;
                    eprintln!(
                        "tsfp-scene-capture: frame {} write failed: {error}",
                        frame.tick
                    );
                } else {
                    state.frames += 1;
                }
            } else {
                eprintln!("tsfp-scene-capture: unable to create sidecar at {path}");
            }
        }
        Err(_) => eprintln!("tsfp-scene-capture: sidecar lock poisoned"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        light_block_indices, write_frame_with_camera, write_rejected_frame,
        CameraHandoffObservation, Completeness, FrameDisposition,
    };
    use tsfp_engine_render::scene::render_prep::{PreparedSceneFrame, Sampling};
    use tsfp_engine_render::scene::light_state::{
        Color3, FrameResourceEpoch, SceneLightBinding, SceneLightInputs, SceneLightState, ViewPoint,
        WorldPoint,
    };

    #[test]
    fn draw_list_requires_every_semantic_category() {
        let complete = Completeness {
            cameras: true,
            ordered_room_draws: true,
            lights: true,
            draw_light_bindings: true,
            effects: true,
            materials_and_models: true,
        };
        assert!(complete.draw_list_complete());
        for incomplete in [
            Completeness {
                cameras: false,
                ..complete
            },
            Completeness {
                ordered_room_draws: false,
                ..complete
            },
            Completeness {
                lights: false,
                ..complete
            },
            Completeness {
                draw_light_bindings: false,
                ..complete
            },
            Completeness {
                effects: false,
                ..complete
            },
            Completeness {
                materials_and_models: false,
                ..complete
            },
        ] {
            assert!(!incomplete.draw_list_complete());
        }
    }

    #[test]
    fn draw_light_bindings_resolve_to_the_published_block_and_fail_closed() {
        let mut lights = SceneLightState::default();
        let record = lights
            .setup_record(SceneLightInputs {
                colors: [Color3([1.0, 0.0, 0.0]); 4],
                world_points: [WorldPoint([1.0, 2.0, 3.0]); 3],
            })
            .unwrap();
        let view = lights.setup_view().unwrap();
        let epoch = FrameResourceEpoch(7);
        lights
            .prepare_view(record, view, epoch, |point| {
                ViewPoint([point.0[0], point.0[1], point.0[2], 1.0])
            })
            .unwrap();
        lights.set_publication_enabled(true);
        let binding = SceneLightBinding::new(record, view);
        let published = lights.publish_frame(epoch, &[binding]).unwrap();

        assert_eq!(light_block_indices(Some(published), &[Some(binding)]), (true, vec![1]));
        assert_eq!(light_block_indices(Some(published), &[None]), (false, vec![0]));
        assert_eq!(light_block_indices(None, &[Some(binding)]), (false, vec![0]));
    }

    #[test]
    fn rejected_frame_is_retained_as_incomplete_and_mutation_sensitive() {
        let frame = PreparedSceneFrame {
            tick: 7,
            alpha: 0.0,
            sampling: Sampling::Fixed,
            resource_epoch: None,
            room_plan: None,
            observed_objects: Vec::new(),
            observed_views: Vec::new(),
            effects_tick: Some(7),
            effects: Vec::new(),
            scene_lights: None,
            views: Vec::new(),
        };
        let mut camera = CameraHandoffObservation {
            camera_valid: 1,
            viewer: 0,
            room_count: 2,
            max_depth: 7,
            sample_position: [1.0, 2.0, 3.0],
            sample_matrix: [4.0; 16],
            viewport: [10.0, 20.0, 640.0, 480.0],
            rectangle: [10, 20, 650, 500],
            owner_eye: [5.0, 6.0, 7.0],
            owner_view: [8.0; 16],
            owner_projection: [9.0; 16],
            owner_tangent_extents: [1.0, 2.0],
            owner_visible_rooms: [3, 4],
        };
        let mut original_bytes = Vec::new();
        let complete = write_rejected_frame(
            &mut original_bytes,
            &frame,
            &[],
            FrameDisposition::CameraMismatch,
            camera,
        )
        .unwrap();
        camera.owner_projection[0] = 9.25;
        let mut mutated_bytes = Vec::new();
        write_rejected_frame(
            &mut mutated_bytes,
            &frame,
            &[],
            FrameDisposition::CameraMismatch,
            camera,
        )
        .unwrap();

        assert!(!complete.cameras);
        assert_eq!(&original_bytes[..8], b"TSFPSC05");
        assert_eq!(&original_bytes[48..52], &3u32.to_le_bytes());
        assert_eq!(&original_bytes[52..56], b"CAMR");
        assert_eq!(&original_bytes[64..68], &2u32.to_le_bytes());
        assert_eq!(&original_bytes[68..72], &7i32.to_le_bytes());
        assert_ne!(original_bytes, mutated_bytes);
        assert_eq!(FrameDisposition::CameraMismatch as u32, 3);
    }

    #[test]
    fn accepted_frame_records_camera_handoff_but_does_not_claim_completeness() {
        let frame = PreparedSceneFrame {
            tick: 7,
            alpha: 0.0,
            sampling: Sampling::Fixed,
            resource_epoch: None,
            room_plan: None,
            observed_objects: Vec::new(),
            observed_views: Vec::new(),
            effects_tick: Some(7),
            effects: Vec::new(),
            scene_lights: None,
            views: Vec::new(),
        };
        let mut camera = CameraHandoffObservation {
            camera_valid: 1,
            viewer: 2,
            room_count: 3,
            max_depth: 4,
            sample_position: [1.0, 2.0, 3.0],
            sample_matrix: [4.0; 16],
            viewport: [5.0, 6.0, 7.0, 8.0],
            rectangle: [9, 10, 11, 12],
            owner_eye: [13.0, 14.0, 15.0],
            owner_view: [16.0; 16],
            owner_projection: [17.0; 16],
            owner_tangent_extents: [18.0, 19.0],
            owner_visible_rooms: [20, 21],
        };
        let mut original_bytes = Vec::new();
        let complete = write_frame_with_camera(&mut original_bytes, &frame, &[], camera).unwrap();
        camera.rectangle[0] += 1;
        let mut mutated_bytes = Vec::new();
        write_frame_with_camera(&mut mutated_bytes, &frame, &[], camera).unwrap();

        assert!(!complete.cameras);
        assert!(!complete.draw_list_complete());
        assert_eq!(&original_bytes[..8], b"TSFPSC05");
        assert_eq!(&original_bytes[52..56], b"CAMR");
        assert_eq!(&original_bytes[56..60], &1u32.to_le_bytes());
        assert_ne!(original_bytes, mutated_bytes);
    }
}
