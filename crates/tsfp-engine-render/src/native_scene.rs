// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 lifecycle membership to native render snapshot producer.
use alloc::{collections::BTreeSet, sync::Arc};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::scene::light_state::{
    FrameResourceEpoch, PublishedSceneLights, RenderRecordId, SceneLightBinding, SceneLightError,
    SceneLightInputs, SceneLightRecordUpdate, SceneLightState, ViewId, ViewLightBlock,
};
pub use crate::scene::render_prep::NativeViewInput;
use crate::scene::render_prep::*;
use crate::visual_effects::{EffectEntry, VisualEffectsState};

fn valid_object_ids(objects: &[ObjectId]) -> bool {
    let mut occupied = [false; tsfp_state::scene_membership::OBJECT_SLOT_CAP];
    for object in objects {
        let Some(slot) = occupied.get_mut(object.slot as usize) else {
            return false;
        };
        if object.generation == 0 || *slot {
            return false;
        }
        *slot = true;
    }
    true
}

fn valid_view_inputs(views: &[NativeViewInput]) -> bool {
    let mut viewers = 0u16;
    for view in views {
        if view.viewer >= 16 {
            return false;
        }
        let bit = 1u16 << view.viewer;
        if viewers & bit != 0
            || !view
                .position
                .iter()
                .chain(view.matrix.iter())
                .chain(view.viewport.iter())
                .all(|value| value.is_finite())
        {
            return false;
        }
        viewers |= bit;
    }
    true
}

/// Match the retained 0x151FC0 point-to-vec4 convention using the exact view
/// matrix copied into this prepared camera. The accumulation order follows the
/// readable lifted contract: z, then y, then x, then translation.
/// Owned observation of one retained D3D draw in the original room pass.
/// `payload` concatenates vertex, index, stage texture, vertex-program,
/// vertex-constant and pixel-shader spans in that order; the lengths identify
/// each range. Guest addresses remain source identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomDrawPacket {
    pub frame_sequence: u64,
    pub room: u32,
    pub indexed: u32,
    pub primitive: u32,
    pub first: u32,
    pub count: u32,
    pub vertex_buffer: u32,
    pub vertex_buffer_bytes: u32,
    pub vertex_stride: u32,
    /// First source vertex represented by the owned vertex span.
    pub vertex_first: u32,
    pub index_buffer: u32,
    pub index_buffer_bytes: u32,
    pub texture_headers: [u32; 4],
    pub texture_data: [u32; 4],
    pub texture_formats: [u32; 4],
    /// D3D pixel-container size words (log2 dimensions and mip count).
    pub texture_sizes: [u32; 4],
    /// Four 32-dword texture-stage shadow rows copied at the draw hook.
    pub texture_stage_state: [u32; 128],
    /// Draw-time viewport rectangle and target dimensions/scales, as stored by D3D8.
    pub viewport_words: [u32; 4],
    pub viewport_target: [u32; 4],
    /// Last explicit D3D scissors call observed before this draw; zero count means none was captured.
    pub scissor_words: [u32; 4],
    pub scissor_count: u32,
    pub scissor_exclusive: u32,
    /// Original D3D vertex declaration/shader selectors, retained as source identities.
    pub vertex_declaration: u32,
    pub vertex_shader_handle: u32,
    pub vertex_shader_index: u32,
    /// Trailing payload ranges, after vertex/index/texture spans, in this order.
    pub vertex_program_bytes: u32,
    pub vertex_constants_bytes: u32,
    pub pixel_shader_bytes: u32,
    pub vertex_program_hash: u64,
    pub vertex_constants_hash: u64,
    pub pixel_shader_hash: u64,
    /// Common D3D8 state values copied from the draw-time render-state shadow.
    pub blend_enable: u32,
    pub blend_source: u32,
    pub blend_destination: u32,
    pub blend_operation: u32,
    pub depth_enable: u32,
    pub depth_write: u32,
    pub depth_function: u32,
    pub cull_mode: u32,
    pub stencil_enable: u32,
    pub stencil_function: u32,
    pub stencil_reference: u32,
    pub stencil_mask: u32,
    pub stencil_write_mask: u32,
    pub stencil_fail: u32,
    pub stencil_depth_fail: u32,
    pub stencil_pass: u32,
    pub vertex_bytes: u32,
    pub index_bytes: u32,
    pub texture_bytes: [u32; 4],
    pub payload_dropped: bool,
    pub render_state: [u32; 256],
    pub payload_hash: u64,
    pub payload: std::sync::Arc<[u8]>,
    pub material_key: u64,
    pub state_key: u64,
    /// Rust-owned render identity for native lighting lookup. The observed
    /// guest draw bridge never fills this field; a native producer must bind
    /// it explicitly from its own record/view ownership.
    pub scene_light_binding: Option<SceneLightBinding>,
}

/// Runtime owner for the native scene interval. Simulation owns the pool and
/// membership inputs; presentation borrows a pair from this object any number
/// of times between fixed ticks. Keeping the inputs and published snapshots
/// together prevents render-rate calls from advancing scene history.
pub struct NativeSceneOwner {
    pub membership: tsfp_state::scene_membership::SceneMembership,
    pub pool: tsfp_state::object_pool::ObjectPool,
    /// Independently typed scene-light setup/view state. Access stays behind
    /// this owner so lifecycle changes invalidate its current publication.
    /// Publication is opt-in; the existing guest producer does not consume it.
    light_state: SceneLightState,
    /// Rust-owned effect inputs advanced by the fixed-tick producer. Scene
    /// publication copies only explicitly typed presentation records.
    pub visual_effects: VisualEffectsState,
    history: SceneTickHistory,
    effect_tick: Option<u64>,
    effect_entries: Vec<EffectEntry>,
    room_plan: Option<PreparedRoomPlan>,
    room_draw_packets: Vec<RoomDrawPacket>,
    observed_objects: Vec<ObjectId>,
    observed_views: Vec<NativeViewInput>,
    record_identities: Vec<Option<ObjectId>>,
    slot_generations: Vec<u64>,
    slot_records: Vec<Option<usize>>,
    next_frame_resource_epoch: AtomicU64,
}

impl Default for NativeSceneOwner {
    fn default() -> Self {
        Self {
            membership: tsfp_state::scene_membership::SceneMembership::default(),
            pool: tsfp_state::object_pool::ObjectPool::EMPTY,
            light_state: SceneLightState::default(),
            visual_effects: VisualEffectsState::default(),
            history: SceneTickHistory::default(),
            effect_tick: None,
            effect_entries: Vec::new(),
            room_plan: None,
            room_draw_packets: Vec::new(),
            observed_objects: Vec::new(),
            observed_views: Vec::new(),
            record_identities: vec![None; tsfp_state::object_pool::REC_CAP],
            slot_generations: Vec::new(),
            slot_records: Vec::new(),
            next_frame_resource_epoch: AtomicU64::new(0),
        }
    }
}

fn prepare_scene_lights_in_state(
    state: &mut SceneLightState,
    frame: &PreparedSceneFrame,
    selections: &[SceneLightBinding],
) -> Result<(), SceneLightError> {
    let epoch = frame
        .resource_epoch
        .ok_or(SceneLightError::MissingFrameResourceEpoch)?;
    if !state.publication_enabled() {
        return Err(SceneLightError::PublicationDisabled);
    }

    let mut seen = BTreeSet::new();
    for &selection in selections {
        let record = selection.record();
        let view = selection.view();
        if !seen.insert((record, view)) {
            return Err(SceneLightError::DuplicatePublicationEntry);
        }
        state.validate_record_view(record, view)?;
        let prepared = frame
            .views
            .iter()
            .find(|prepared| prepared.scene_light_view == Some(view))
            .ok_or(SceneLightError::ViewNotInPreparedFrame)?;
        if prepared.camera.is_none() {
            return Err(SceneLightError::MissingPreparedCamera);
        }
    }

    for &selection in selections {
        let view = selection.view();
        let prepared = frame
            .views
            .iter()
            .find(|prepared| prepared.scene_light_view == Some(view))
            .expect("selected view was validated before preparation");
        let matrix = prepared
            .camera
            .as_ref()
            .expect("selected camera was validated before preparation")
            .view;
        state.prepare_view(selection.record(), view, epoch, |point| {
            crate::scene::light_state::transform_world_point_to_view(matrix, point)
        })?;
    }
    state.publish_frame(epoch, selections)?;
    Ok(())
}

impl NativeSceneOwner {
    /// Refresh the owned ObjectPool mirror from the completed guest-side tick.
    /// Lifecycle IDs and render components remain explicit native inputs; this
    /// method only updates pool rows used to validate room membership.
    pub fn refresh_pool_from_guest<M: tsfp_state::GuestMemory>(&mut self, memory: &M) {
        self.pool.refresh_from_guest(memory);
    }

    /// Complete one simulation tick from its guest-owned ObjectPool image.
    /// The mirror is refreshed before publication, and history is only
    /// advanced if the full lifecycle/component join succeeds. Callers must
    /// update native lifecycle membership and render components for this tick
    /// before invoking this method.
    pub fn finish_guest_tick<M: tsfp_state::GuestMemory>(
        &mut self,
        tick: u64,
        memory: &M,
    ) -> Result<(), SceneError> {
        self.refresh_pool_from_guest(memory);
        self.finish_tick(tick)
    }

    /// Publish the completed fixed tick. It is safe to call this again for the
    /// same tick after a producer refreshes its typed rows; SceneTickHistory
    /// keeps the previous completed tick stable.
    pub fn finish_tick(&mut self, tick: u64) -> Result<(), SceneError> {
        publish_membership_tick(&mut self.history, tick, &self.membership, &self.pool)?;
        self.publish_effect_entries(tick);
        Ok(())
    }

    /// Reconcile authoritative owner rows against the complete, ordered pool
    /// membership audit and publish them atomically. `rows` are supplied by
    /// the lifecycle/transform/visibility owners; pool room membership only
    /// establishes which record indexes and order must be represented. No
    /// guest address or record index is promoted to a native ObjectId.
    pub fn finish_tick_with_rows(
        &mut self,
        tick: u64,
        rows: &[tsfp_state::scene_membership::SceneMember],
    ) -> Result<(), SceneError> {
        let audit = self
            .pool
            .audit_membership()
            .map_err(|_| SceneError::ObjectIdentityMismatch)?;
        let mut expected = Vec::with_capacity(audit.total_members);
        for room in &audit.rooms {
            for &record in &room.records {
                if !expected.contains(&record) {
                    expected.push(record);
                }
            }
        }
        // A draw identity can be present in several rooms. The membership
        // table stores that identity once, in first-observed room order;
        // compare against the first occurrence of each pool record only.
        let unique_expected: Vec<_> =
            expected.into_iter().fold(Vec::new(), |mut unique, record| {
                if !unique.contains(&record) {
                    unique.push(record);
                }
                unique
            });
        let supplied: Vec<_> = rows
            .iter()
            .map(|row| {
                self.membership
                    .record_index(row.object)
                    .ok_or(SceneError::ObjectIdentityMismatch)
            })
            .collect::<Result<_, _>>()?;
        if unique_expected != supplied {
            return Err(SceneError::ObjectIdentityMismatch);
        }
        // Existing membership validation checks model keys, record bindings,
        // and exact room sets. Keep it as the final independent audit.
        let complete = self
            .membership
            .collect_complete(&self.pool)
            .map_err(|_| SceneError::ObjectIdentityMismatch)?;
        if complete != rows {
            return Err(SceneError::ObjectIdentityMismatch);
        }
        let snapshot = snapshot_from_membership(tick, &self.membership, &self.pool)
            .map_err(|_| SceneError::ObjectIdentityMismatch)?;
        self.history.publish(snapshot)?;
        self.publish_effect_entries(tick);
        Ok(())
    }

    fn publish_effect_entries(&mut self, tick: u64) {
        self.effect_entries.clear();
        self.effect_entries
            .extend(self.visual_effects.iter_effects());
        self.effect_tick = Some(tick);
    }

    /// Typed fixed-tick snapshot currently owned by this scene interval.
    pub fn effect_entries(&self) -> &[EffectEntry] {
        &self.effect_entries
    }

    pub fn effect_tick(&self) -> Option<u64> {
        self.effect_tick
    }

    /// Begin an owned typed scene-light record lifetime.
    pub fn setup_scene_light_record(
        &mut self,
        inputs: SceneLightInputs,
    ) -> Result<RenderRecordId, SceneLightError> {
        self.light_state.setup_record(inputs)
    }

    /// Replace source values for one live scene-light record. This invalidates
    /// prepared views and any publication made from the previous values.
    pub fn update_scene_light_record(
        &mut self,
        record: RenderRecordId,
        inputs: SceneLightInputs,
    ) -> Result<(), SceneLightError> {
        self.light_state.update_record(record, inputs)
    }

    /// Atomically replace several source records before publishing a frame.
    /// Invalid batches preserve the prior publication; accepted batches clear
    /// all affected view blocks and the old publication together.
    pub fn update_scene_light_records(
        &mut self,
        updates: &[SceneLightRecordUpdate],
    ) -> Result<(), SceneLightError> {
        self.light_state.update_records(updates)
    }

    /// End one owned scene-light record lifetime.
    pub fn teardown_scene_light_record(
        &mut self,
        record: RenderRecordId,
    ) -> Result<(), SceneLightError> {
        self.light_state.teardown_record(record)
    }

    /// Begin a view identity used to qualify this owner's sampled camera.
    pub fn setup_scene_light_view(&mut self) -> Result<ViewId, SceneLightError> {
        self.light_state.setup_view()
    }

    /// End a view lifetime and discard its prepared blocks.
    pub fn teardown_scene_light_view(&mut self, view: ViewId) -> Result<(), SceneLightError> {
        self.light_state.teardown_view(view)
    }

    /// Enable or disable opt-in scene-light publication. Disabling clears the
    /// current publication; a later enable requires a fresh frame publication.
    pub fn set_scene_light_publication_enabled(&mut self, enabled: bool) {
        self.light_state.set_publication_enabled(enabled);
    }

    pub fn scene_light_publication_enabled(&self) -> bool {
        self.light_state.publication_enabled()
    }

    /// Read an owned block only while its record and view identities are live.
    pub fn scene_light_view_block(
        &self,
        record: RenderRecordId,
        view: ViewId,
    ) -> Result<Option<&ViewLightBlock>, SceneLightError> {
        self.light_state.view_block(record, view)
    }

    pub fn published_scene_lights(&self) -> Option<&PublishedSceneLights> {
        self.light_state.published_frame()
    }

    /// Reuse a single immutable endpoint pair for repeated display samples.
    pub fn pair(&self) -> Result<ScenePair<'_>, SceneError> {
        self.history.pair()
    }

    /// Prepare the original fixed-tick scene through the authoritative owner.
    /// This keeps current camera/visibility requests and scene endpoints on one
    /// call path; callers submit the returned typed commands to their renderer.
    pub fn prepare_original_frame(
        &self,
        requests: &[CameraViewRequest<'_>],
    ) -> Result<PreparedSceneFrame, SceneError> {
        let frame = self.pair()?.prepare_original_frame(requests)?;
        self.attach_owner_inputs(frame)
    }

    /// Prepare one display-rate sample without advancing or republishing scene
    /// history. The caller may invoke this several times between fixed ticks.
    pub fn prepare_display_frame(
        &self,
        requests: &[CameraViewRequest<'_>],
        alpha: f32,
    ) -> Result<PreparedSceneFrame, SceneError> {
        let frame = self.pair()?.prepare_display_frame(requests, alpha)?;
        self.attach_owner_inputs(frame)
    }

    /// Prepare a display sample using copied T2334 endpoint histories while
    /// retaining the lifecycle identity mapping owned by this scene interval.
    pub fn prepare_display_frame_with_transforms(
        &self,
        requests: &[CameraViewRequest<'_>],
        alpha: f32,
        transforms: &impl TransformHistorySource,
    ) -> Result<PreparedSceneFrame, SceneError> {
        let frame = self
            .history
            .pair_with_transform_history(transforms)?
            .prepare_display_frame(requests, alpha)?;
        self.attach_owner_inputs(frame)
    }

    fn attach_owner_inputs(
        &self,
        mut frame: PreparedSceneFrame,
    ) -> Result<PreparedSceneFrame, SceneError> {
        let epoch = self
            .next_frame_resource_epoch
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| SceneError::FrameResourceEpochExhausted)?
            + 1;
        frame.resource_epoch = Some(FrameResourceEpoch(epoch));
        frame.room_plan = self.room_plan.clone();
        frame.observed_objects.clone_from(&self.observed_objects);
        frame.observed_views.clone_from(&self.observed_views);
        frame.effects_tick = self.effect_tick;
        frame.effects.clone_from(&self.effect_entries);
        Ok(frame)
    }

    /// Attach the current immutable light publication only when its resource
    /// epoch matches and every selected view is present in this prepared frame.
    /// Ordinary frame preparation deliberately leaves `scene_lights` unset.
    pub fn attach_scene_lights(
        &self,
        frame: &mut PreparedSceneFrame,
    ) -> Result<(), SceneLightError> {
        let epoch = frame
            .resource_epoch
            .ok_or(SceneLightError::MissingFrameResourceEpoch)?;
        let published = self
            .light_state
            .published_frame()
            .ok_or(SceneLightError::NoPublishedFrame)?;
        if published.epoch != epoch {
            return Err(SceneLightError::FrameEpochMismatch);
        }
        if published.blocks.iter().any(|block| {
            !frame
                .views
                .iter()
                .any(|view| view.scene_light_view == Some(block.view))
        }) {
            return Err(SceneLightError::ViewNotInPreparedFrame);
        }
        frame.scene_lights = Some(published.clone());
        Ok(())
    }

    /// Validate and publish prepared Rust-owned light blocks for renderer
    /// submissions. A successful publication is copied onto subsequent
    /// prepared scene frames until its source data changes or it is replaced.
    pub fn publish_scene_lights(
        &mut self,
        frame: &PreparedSceneFrame,
        selections: &[SceneLightBinding],
    ) -> Result<&PublishedSceneLights, SceneLightError> {
        let epoch = frame
            .resource_epoch
            .ok_or(SceneLightError::MissingFrameResourceEpoch)?;
        if selections.iter().any(|selection| {
            !frame
                .views
                .iter()
                .any(|prepared| prepared.scene_light_view == Some(selection.view()))
        }) {
            return Err(SceneLightError::ViewNotInPreparedFrame);
        }
        self.light_state.publish_frame(epoch, selections)
    }

    /// Prepare one light block from the camera actually stored in this
    /// owner-issued sampled frame. This binds world-to-view conversion, view
    /// identity, and resource epoch to one prepared camera sample.
    pub fn prepare_scene_light_view(
        &mut self,
        frame: &PreparedSceneFrame,
        record: crate::scene::light_state::RenderRecordId,
        view: ViewId,
    ) -> Result<&crate::scene::light_state::ViewLightBlock, SceneLightError> {
        let prepared = frame
            .views
            .iter()
            .find(|prepared| prepared.scene_light_view == Some(view))
            .ok_or(SceneLightError::ViewNotInPreparedFrame)?;
        let epoch = frame
            .resource_epoch
            .ok_or(SceneLightError::MissingFrameResourceEpoch)?;
        let camera = prepared
            .camera
            .as_ref()
            .ok_or(SceneLightError::MissingPreparedCamera)?;
        let matrix = camera.view;
        self.light_state.prepare_view(record, view, epoch, |point| {
            crate::scene::light_state::transform_world_point_to_view(matrix, point)
        })
    }

    /// Prepare and publish the selected light blocks from one sampled camera
    /// frame. All identities, camera samples and the opt-in switch are checked
    /// before any per-view block is changed, so invalid input leaves the prior
    /// publication intact.
    pub fn prepare_and_publish_scene_lights(
        &mut self,
        frame: &PreparedSceneFrame,
        selections: &[SceneLightBinding],
    ) -> Result<&PublishedSceneLights, SceneLightError> {
        prepare_scene_lights_in_state(&mut self.light_state, frame, selections)?;
        self.published_scene_lights()
            .ok_or(SceneLightError::NoPublishedFrame)
    }

    /// Prepare, publish, and attach selected native light blocks to one frame.
    ///
    /// This keeps the default-off producer flow on one owner-issued frame and
    /// prevents callers from accidentally submitting the publication without
    /// attaching it to that frame.
    pub fn prepare_scene_lights_for_frame(
        &mut self,
        frame: &mut PreparedSceneFrame,
        selections: &[SceneLightBinding],
    ) -> Result<(), SceneLightError> {
        self.prepare_and_publish_scene_lights(frame, selections)?;
        self.attach_scene_lights(frame)
    }

    /// Replace source values and publish their selected view blocks as one
    /// producer transaction. Failed validation leaves both owner state and the
    /// frame's previous attachment unchanged.
    pub fn update_and_prepare_scene_lights_for_frame(
        &mut self,
        frame: &mut PreparedSceneFrame,
        updates: &[SceneLightRecordUpdate],
        selections: &[SceneLightBinding],
    ) -> Result<(), SceneLightError> {
        let mut candidate = self.light_state.clone();
        candidate.update_records(updates)?;
        prepare_scene_lights_in_state(&mut candidate, frame, selections)?;
        let publication = candidate
            .published_frame()
            .ok_or(SceneLightError::NoPublishedFrame)?
            .clone();
        frame.scene_lights = Some(publication);
        self.light_state = candidate;
        Ok(())
    }

    /// Submit a prepared scene to a replaceable renderer contract. This keeps
    /// repeated display samples on the same published tick pair and gives the
    /// eventual host caller a single typed admission point.
    pub fn submit_display_frame<S: SceneFrameSink>(
        &self,
        requests: &[CameraViewRequest<'_>],
        alpha: f32,
        sink: &mut S,
    ) -> Result<(), SceneError> {
        let frame = self.prepare_display_frame(requests, alpha)?;
        sink.submit(&frame);
        Ok(())
    }

    /// Submit an owned display-frame lease to a renderer that queues work.
    /// The sink must retain the lease until its queued consumer completes.
    pub fn submit_display_frame_leased<S: SceneFrameLeaseSink>(
        &self,
        requests: &[CameraViewRequest<'_>],
        alpha: f32,
        sink: &mut S,
    ) -> Result<(), SceneError> {
        let frame = self.prepare_display_frame(requests, alpha)?;
        sink.submit_lease(PreparedSceneLease::new(frame));
        Ok(())
    }

    /// Publish the live collector's ordered room output. This does not mint
    /// object identities or provide room meshes; the opt-in comparison sink
    /// may draw room-order proxies while the legacy renderer stays underneath.
    pub fn publish_room_plan(
        &mut self,
        tick: u64,
        rooms: &[u8],
        draw_max_depth: i32,
    ) -> Result<(), SceneError> {
        if !(-1..80).contains(&draw_max_depth)
            || rooms.len() > 200
            || rooms.iter().any(|room| *room == 0 || *room > 200)
        {
            return Err(SceneError::InvalidRoomPlan);
        }
        for (index, room) in rooms.iter().enumerate() {
            if rooms[..index].contains(room) {
                return Err(SceneError::InvalidRoomPlan);
            }
        }
        if self
            .room_plan
            .as_ref()
            .is_some_and(|current| tick <= current.tick)
        {
            return Err(SceneError::InvalidTick);
        }
        self.room_plan = Some(PreparedRoomPlan {
            tick,
            draw_max_depth,
            rooms: rooms.to_vec(),
        });
        self.room_draw_packets.clear();
        Ok(())
    }

    /// Replace the current room plan's captured draw metadata after validating
    /// the explicit frame sequence and room membership. The retained reference
    /// path remains the renderer; these packets are audit data only.
    pub fn publish_room_draw_packets(
        &mut self,
        tick: u64,
        packets: &[RoomDrawPacket],
    ) -> Result<(), SceneError> {
        let Some(plan) = self.room_plan.as_ref() else {
            return Err(SceneError::InvalidRoomDrawPacket);
        };
        if plan.tick != tick
            || packets.iter().any(|packet| {
                packet.frame_sequence != tick
                    || packet.room == 0
                    || packet.room > 200
                    || !plan.rooms.contains(&(packet.room as u8))
                    || packet.indexed > 1
            })
        {
            return Err(SceneError::InvalidRoomDrawPacket);
        }
        self.room_draw_packets.clear();
        self.room_draw_packets.extend(packets.iter().cloned());
        Ok(())
    }

    /// Bind retained draw packets to published light blocks using identities
    /// supplied by the native scene producer. Guest room/object IDs are never
    /// promoted to render-record or view identities here.
    pub fn bind_room_draw_scene_lights(
        &mut self,
        epoch: FrameResourceEpoch,
        bindings: &[Option<SceneLightBinding>],
    ) -> Result<(), SceneLightError> {
        if bindings.len() != self.room_draw_packets.len() {
            return Err(SceneLightError::InvalidDrawBinding);
        }
        if bindings.iter().all(Option::is_none) {
            for packet in &mut self.room_draw_packets {
                packet.scene_light_binding = None;
            }
            return Ok(());
        }
        let published = self
            .light_state
            .published_frame()
            .ok_or(SceneLightError::NoPublishedFrame)?;
        if published.epoch != epoch {
            return Err(SceneLightError::FrameEpochMismatch);
        }
        if bindings
            .iter()
            .flatten()
            .any(|&binding| published.block_for_binding(binding).is_none())
        {
            return Err(SceneLightError::InvalidDrawBinding);
        }
        for (packet, binding) in self.room_draw_packets.iter_mut().zip(bindings) {
            packet.scene_light_binding = *binding;
        }
        Ok(())
    }

    pub fn room_draw_packets(&self) -> &[RoomDrawPacket] {
        &self.room_draw_packets
    }

    pub fn room_plan(&self) -> Option<&PreparedRoomPlan> {
        self.room_plan.as_ref()
    }

    /// Copy generation-qualified ObjectPool membership and the current camera
    /// inputs for one collector tick. These remain source observations until
    /// the lifecycle owner supplies model and typed render components.
    pub fn publish_bridge_inputs(
        &mut self,
        tick: u64,
        objects: &[ObjectId],
        views: &[NativeViewInput],
    ) -> Result<(), SceneError> {
        if self.room_plan.as_ref().map(|plan| plan.tick) != Some(tick) {
            return Err(SceneError::InvalidTick);
        }
        if !valid_object_ids(objects) {
            return Err(SceneError::ObjectIdentityMismatch);
        }
        if !valid_view_inputs(views) {
            return Err(SceneError::InvalidViewer);
        }
        self.observed_objects.clear();
        self.observed_objects.extend_from_slice(objects);
        self.observed_views.clear();
        self.observed_views.extend_from_slice(views);
        Ok(())
    }

    /// Join ObjectPool record observations to Rust-owned lifecycle IDs. The
    /// record index is only an internal lookup key; allocated slots and their
    /// generation counters form the native identity.
    pub fn publish_pool_record_inputs(
        &mut self,
        tick: u64,
        record_indices: &[u32],
        views: &[NativeViewInput],
    ) -> Result<(), SceneError> {
        if self.room_plan.as_ref().map(|plan| plan.tick) != Some(tick) {
            return Err(SceneError::ObjectIdentityMismatch);
        }
        let mut seen_records = [false; tsfp_state::object_pool::REC_CAP];
        for &record in record_indices {
            let index = record as usize;
            let Some(seen) = seen_records.get_mut(index) else {
                return Err(SceneError::ObjectIdentityMismatch);
            };
            if index >= self.record_identities.len() || *seen {
                return Err(SceneError::ObjectIdentityMismatch);
            }
            *seen = true;
        }
        self.validate_bridge_views(views)?;
        let mut objects = Vec::with_capacity(record_indices.len());
        for &record in record_indices {
            let record = record as usize;
            let id = if let Some(id) = self.record_identities[record] {
                id
            } else {
                let slot = self
                    .slot_records
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(self.slot_records.len());
                if slot >= tsfp_state::scene_membership::OBJECT_SLOT_CAP {
                    return Err(SceneError::ObjectIdentityMismatch);
                }
                if slot == self.slot_records.len() {
                    self.slot_records.push(None);
                    self.slot_generations.push(0);
                }
                self.slot_generations[slot] = self.slot_generations[slot]
                    .checked_add(1)
                    .ok_or(SceneError::ObjectIdentityMismatch)?;
                let id = ObjectId {
                    slot: slot as u32,
                    generation: self.slot_generations[slot],
                };
                self.slot_records[slot] = Some(record);
                self.record_identities[record] = Some(id);
                id
            };
            objects.push(id);
        }
        self.publish_bridge_inputs(tick, &objects, views)
    }

    /// Retire the identity associated with a pool record only after the
    /// original teardown adapter has completed. Reuse increments generation
    /// when the record is observed again.
    pub fn retire_pool_record(&mut self, record_index: u32) -> bool {
        let Some(identity) = self.record_identities.get_mut(record_index as usize) else {
            return false;
        };
        let Some(id) = identity.take() else {
            return false;
        };
        if let Some(slot) = self.slot_records.get_mut(id.slot as usize) {
            *slot = None;
        }
        self.observed_objects.retain(|object| *object != id);
        true
    }

    fn validate_bridge_views(&self, views: &[NativeViewInput]) -> Result<(), SceneError> {
        if !valid_view_inputs(views) {
            return Err(SceneError::InvalidViewer);
        }
        Ok(())
    }

    pub fn observed_objects(&self) -> &[ObjectId] {
        &self.observed_objects
    }

    pub fn observed_views(&self) -> &[NativeViewInput] {
        &self.observed_views
    }

    /// Begin a lifecycle instance in the authoritative native owner.
    pub fn spawn(
        &mut self,
        slot: u32,
    ) -> Result<tsfp_state::scene_membership::ObjectId, tsfp_state::scene_membership::MembershipError>
    {
        self.membership.spawn(slot)
    }

    /// Attach one ordered room observation to a lifecycle instance.
    pub fn attach_room(
        &mut self,
        id: tsfp_state::scene_membership::ObjectId,
        record_index: usize,
        model_asset: u64,
        model_key: u16,
        room: u16,
        visible: bool,
        deferred: bool,
    ) -> Result<(), tsfp_state::scene_membership::MembershipError> {
        let already_bound = self
            .membership
            .ordered()
            .iter()
            .any(|member| member.object == id);
        if let Some(bound) = self.membership.record_index(id) {
            if bound != record_index {
                return Err(tsfp_state::scene_membership::MembershipError::PoolIdentityMismatch);
            }
        }
        if !already_bound {
            // Validate/attach first so an invalid room or asset does not leave
            // a live lifecycle identity tied to an unusable pool row.
            self.membership
                .attach(id, model_asset, model_key, room, visible, deferred)?;
            self.membership.bind_record(id, record_index)?;
            return Ok(());
        }
        self.membership
            .attach(id, model_asset, model_key, room, visible, deferred)
    }

    /// Supply typed presentation state after simulation updates this object.
    pub fn set_render_state(
        &mut self,
        id: tsfp_state::scene_membership::ObjectId,
        state: tsfp_state::scene_membership::RenderComponents,
    ) -> Result<(), tsfp_state::scene_membership::MembershipError> {
        self.membership.set_render_state(id, state)
    }

    pub fn retire(
        &mut self,
        id: tsfp_state::scene_membership::ObjectId,
    ) -> Result<(), tsfp_state::scene_membership::MembershipError> {
        self.membership.retire(id)
    }

    pub fn current_tick(&self) -> Option<u64> {
        self.history.current_tick()
    }

    pub fn object_count(&self) -> Option<usize> {
        self.history.object_count()
    }

    /// Return the validated ObjectPool room order for the current owned tick.
    /// The caller must join record indexes to native lifecycle IDs and typed
    /// presentation data; guest record addresses never become draw identities.
    pub fn observed_room_membership(
        &self,
    ) -> Result<
        tsfp_state::object_pool::MembershipAudit,
        tsfp_state::object_pool::MembershipAuditError,
    > {
        self.pool.audit_membership()
    }
}

/// Native renderer boundary. Implementations consume typed prepared commands;
/// they must not recover guest pointers from the scene packet.
pub trait SceneFrameSink {
    fn submit(&mut self, frame: &PreparedSceneFrame);
}

/// Shared immutable CPU data retained for one renderer submission.
///
/// The frame owns its typed scene-light snapshot, camera samples, and other
/// prepared values. A queued renderer stores this lease with its in-flight
/// submission and releases it only after completion. Backend allocations
/// still belong to the renderer and must be retired with the same completion.
#[derive(Clone, Debug)]
pub struct PreparedSceneLease(Arc<PreparedSceneFrame>);

impl PreparedSceneLease {
    pub fn new(frame: PreparedSceneFrame) -> Self {
        Self(Arc::new(frame))
    }

    pub fn frame(&self) -> &PreparedSceneFrame {
        &self.0
    }
}

/// Ownership-transfer boundary for renderers that queue asynchronous work.
/// Implementations retain the lease until every queued consumer has completed.
pub trait SceneFrameLeaseSink {
    fn submit_lease(&mut self, lease: PreparedSceneLease);
}

/// Production audit sink used by the Bevy host while native frame output is
/// opt-in. It accounts prepared values and refuses mismatched tick/order data;
/// it intentionally issues no draw calls.
#[derive(Default)]
pub struct SceneFrameAuditSink {
    frames: u64,
    rooms: u64,
    objects: u64,
    views: u64,
    effects: u64,
    order_errors: u64,
    last_tick: Option<u64>,
    last_order_hash: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneFrameAuditSummary {
    pub frames: u64,
    pub rooms: u64,
    pub objects: u64,
    pub views: u64,
    /// Typed effect entries observed across submitted display samples.
    pub effects: u64,
    pub order_errors: u64,
    pub last_tick: Option<u64>,
    pub order_hash: Option<u64>,
}

impl SceneFrameAuditSink {
    pub fn take_summary(&mut self) -> SceneFrameAuditSummary {
        let summary = SceneFrameAuditSummary {
            frames: self.frames,
            rooms: self.rooms,
            objects: self.objects,
            views: self.views,
            effects: self.effects,
            order_errors: self.order_errors,
            last_tick: self.last_tick,
            order_hash: self.last_order_hash,
        };
        self.frames = 0;
        self.rooms = 0;
        self.objects = 0;
        self.views = 0;
        self.effects = 0;
        self.order_errors = 0;
        summary
    }

    /// Count and validate the owner inputs separately from prepared draw
    /// commands. Membership IDs and camera observations do not imply geometry
    /// admission or renderer submission.
    pub fn observe_owner_inputs(&mut self, objects: &[ObjectId], views: &[NativeViewInput]) {
        self.objects += objects.len() as u64;
        self.views += views.len() as u64;
        if !valid_object_ids(objects) || !valid_view_inputs(views) {
            self.order_errors += 1;
        }
    }
}

impl SceneFrameSink for SceneFrameAuditSink {
    fn submit(&mut self, frame: &PreparedSceneFrame) {
        let previous_tick = self.last_tick;
        self.frames += 1;
        self.last_tick = Some(frame.tick);
        self.effects += frame.effects.len() as u64;
        if !frame.effects.is_empty()
            && (frame.effects_tick != Some(frame.tick)
                || frame.effects.iter().enumerate().any(|(index, effect)| {
                    frame.effects[..index]
                        .iter()
                        .any(|prior| prior.handle == effect.handle)
                        || !effect
                            .position
                            .iter()
                            .chain(effect.visual.tint_rgba.iter())
                            .chain(effect.visual.scale.iter())
                            .chain([
                                &effect.age,
                                &effect.lifetime,
                                &effect.visual.rotation_radians,
                            ])
                            .all(|value| value.is_finite())
                }))
        {
            self.order_errors += 1;
        }
        let Some(plan) = frame.room_plan.as_ref() else {
            self.order_errors += 1;
            return;
        };
        let mut hash = 0xcbf29ce484222325u64;
        let mut valid = plan.tick == frame.tick
            && (-1..80).contains(&plan.draw_max_depth)
            && plan.rooms.len() <= 200;
        for (index, room) in plan.rooms.iter().enumerate() {
            valid &= (1..=200).contains(room) && !plan.rooms[..index].contains(room);
            hash = (hash ^ u64::from(*room)).wrapping_mul(0x100000001b3);
        }
        if previous_tick == Some(frame.tick)
            && self
                .last_order_hash
                .is_some_and(|previous| previous != hash)
        {
            valid = false;
        }
        if !valid || frame.views.iter().any(|view| view.tick != frame.tick) {
            self.order_errors += 1;
        }
        self.rooms += plan.rooms.len() as u64;
        self.views += frame.views.len() as u64;
        self.objects += frame
            .views
            .iter()
            .map(|view| view.objects.len() as u64)
            .sum::<u64>();
        self.last_order_hash = Some(hash);
    }
}

/// Atomically collect and publish one tick into the reusable native scene
/// interval. Publication happens only after the full pool/membership join
/// succeeds, so a missing record or render component cannot replace the last
/// complete pair. Repeated display-rate preparation should borrow `history`
/// through `pair()`/`pair_with_transform_history()` after this call.
pub fn publish_membership_tick(
    history: &mut SceneTickHistory,
    tick: u64,
    membership: &tsfp_state::scene_membership::SceneMembership,
    pool: &tsfp_state::object_pool::ObjectPool,
) -> Result<(), SceneError> {
    let snapshot = snapshot_from_membership(tick, membership, pool)
        .map_err(|_| SceneError::ObjectIdentityMismatch)?;
    history.publish(snapshot)
}
/// Join lifecycle membership from `tsfp-state` with the full owned pool and
/// explicit presentation components into one native snapshot. It fails closed
/// if any member has no exact record binding or typed render data.
pub fn snapshot_from_membership(
    tick: u64,
    membership: &tsfp_state::scene_membership::SceneMembership,
    pool: &tsfp_state::object_pool::ObjectPool,
) -> Result<SceneSnapshot, tsfp_state::scene_membership::MembershipError> {
    let members = membership.collect_complete(pool)?;
    let mut builder = SceneSnapshotBuilder::new(tick);
    for member in members {
        let components = membership
            .render_state(member.object)
            .ok_or(tsfp_state::scene_membership::MembershipError::MissingRenderState)?;
        let placement = Placement {
            position: components.placement.position,
            orientation: components.placement.orientation,
        };
        let bounds = crate::spatial::Bounds {
            min: components.bounds[0],
            max: components.bounds[1],
        };
        let motion_bounds = crate::spatial::Bounds {
            min: components.motion_bounds[0],
            max: components.motion_bounds[1],
        };
        let overlay = match components.overlay {
            0 => Overlay::None,
            1 => Overlay::Character,
            2 => Overlay::Player,
            _ => return Err(tsfp_state::scene_membership::MembershipError::MissingRenderState),
        };
        builder
            .push(ObjectSnapshot {
                id: ObjectId {
                    slot: member.object.slot,
                    generation: member.object.generation,
                },
                draw: SceneObject {
                    id: ObjectId {
                        slot: member.object.slot,
                        generation: member.object.generation,
                    },
                    model: ModelId(member.model_asset),
                    visible: member.visible,
                    scissor_rejected: components.scissor_rejected,
                    model_hidden: components.model_hidden,
                    deferred: member.deferred,
                    overlay,
                    scissor: components.scissor,
                },
                placement,
                bounds,
                motion_bounds,
                rooms: member.rooms,
                discontinuity: components.discontinuity,
            })
            .map_err(|_| tsfp_state::scene_membership::MembershipError::PoolIdentityMismatch)?;
    }
    Ok(builder.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::light_state::{Color3, WorldPoint};
    use crate::scene::render_prep::ObjectId;
    use std::collections::BTreeMap;
    use tsfp_state::{
        GuestMemory,
        generated_views::TsfpObjectView,
        object_pool::*,
        scene_membership::{MembershipError, RenderComponents, SceneMembership},
    };

    #[derive(Default)]
    struct CaptureSink(usize);
    impl SceneFrameSink for CaptureSink {
        fn submit(&mut self, frame: &PreparedSceneFrame) {
            self.0 += 1;
            assert_eq!(frame.tick, 1);
        }
    }

    #[derive(Default)]
    struct PendingLeaseSink(Option<PreparedSceneLease>);
    impl SceneFrameLeaseSink for PendingLeaseSink {
        fn submit_lease(&mut self, lease: PreparedSceneLease) {
            assert!(self.0.replace(lease).is_none());
        }
    }

    #[test]
    fn native_scene_owner_retains_one_pair_for_display_rate_sampling() {
        let mut owner = NativeSceneOwner::default();
        owner.finish_tick(1).unwrap();
        assert_eq!(owner.current_tick(), Some(1));
        assert_eq!(owner.object_count(), Some(0));
        let first = owner.prepare_display_frame(&[], 0.0).unwrap();
        let repeated = owner.prepare_display_frame(&[], 0.75).unwrap();
        assert_eq!(first.tick, repeated.tick);
        assert_eq!(first.views, repeated.views);
        owner.finish_tick(2).unwrap();
        assert_eq!(owner.current_tick(), Some(2));
        assert!(owner.pair().is_ok());
    }

    #[test]
    fn sink_receives_repeated_display_samples_without_advancing_tick() {
        let mut owner = NativeSceneOwner::default();
        owner.finish_tick(1).unwrap();
        let mut sink = CaptureSink::default();
        owner.submit_display_frame(&[], 0.0, &mut sink).unwrap();
        owner.submit_display_frame(&[], 0.75, &mut sink).unwrap();
        assert_eq!(sink.0, 2);
        assert_eq!(owner.current_tick(), Some(1));
    }

    #[test]
    fn queued_sink_owns_prepared_frame_until_completion() {
        let mut owner = NativeSceneOwner::default();
        owner.finish_tick(1).unwrap();
        let mut sink = PendingLeaseSink::default();
        owner
            .submit_display_frame_leased(&[], 0.25, &mut sink)
            .unwrap();
        let retained_epoch = sink.0.as_ref().unwrap().frame().resource_epoch;
        assert_eq!(sink.0.as_ref().unwrap().frame().tick, 1);
        assert!(retained_epoch.is_some());

        owner.finish_tick(2).unwrap();
        let next = owner.prepare_display_frame(&[], 0.75).unwrap();
        assert_eq!(sink.0.as_ref().unwrap().frame().tick, 1);
        assert_ne!(
            sink.0.as_ref().unwrap().frame().resource_epoch,
            next.resource_epoch
        );

        // The sink retires its lease at the completion boundary.
        drop(sink.0.take());
        assert!(sink.0.is_none());
    }

    #[test]
    fn owner_publishes_live_room_plan_and_sink_accounts_ordered_frames() {
        let mut owner = NativeSceneOwner::default();
        owner.publish_room_plan(1, &[7, 3, 11], 2).unwrap();
        owner.finish_tick(1).unwrap();
        assert_eq!(owner.room_plan().unwrap().rooms, [7, 3, 11]);

        let mut sink = SceneFrameAuditSink::default();
        owner.submit_display_frame(&[], 0.0, &mut sink).unwrap();
        owner.submit_display_frame(&[], 0.5, &mut sink).unwrap();
        let summary = sink.take_summary();
        assert_eq!(summary.frames, 2);
        assert_eq!(summary.rooms, 6);
        assert_eq!(summary.objects, 0);
        assert_eq!(summary.views, 0);
        assert_eq!(summary.effects, 0);
        assert_eq!(summary.order_errors, 0);
        assert_eq!(summary.last_tick, Some(1));
        assert!(summary.order_hash.is_some());
        let mut reordered = owner.prepare_display_frame(&[], 0.75).unwrap();
        reordered.room_plan.as_mut().unwrap().rooms.reverse();
        sink.submit(&reordered);
        assert_eq!(sink.take_summary().order_errors, 1);
        assert_eq!(owner.current_tick(), Some(1));
    }

    #[test]
    fn fixed_tick_publishes_one_typed_effect_for_scene_audit_only() {
        let mut owner = NativeSceneOwner::default();
        owner.publish_room_plan(3, &[7], 1).unwrap();
        owner.visual_effects.spawn_effect(
            crate::visual_effects::ParticleState {
                position: [1.0, 2.0, 3.0],
                velocity: [0.0; 3],
                age: 0.5,
                lifetime: 4.0,
            },
            crate::visual_effects::EffectTypeId(9),
            crate::visual_effects::EffectVisualParams {
                tint_rgba: [0.25, 0.5, 0.75, 1.0],
                scale: [1.0, 1.0, 1.0],
                rotation_radians: 0.0,
            },
        );

        owner.finish_tick(3).unwrap();
        assert_eq!(owner.effect_tick(), Some(3));
        assert_eq!(owner.effect_entries().len(), 1);
        let frame = owner.prepare_display_frame(&[], 0.0).unwrap();
        assert_eq!(frame.effects_tick, Some(3));
        assert_eq!(frame.effects.len(), 1);
        assert_eq!(frame.effects[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(frame.effects[0].effect_type.0, 9);
        assert_eq!(frame.effects[0].lifetime, 4.0);
        assert_eq!(frame.effects[0].visual.scale, [1.0; 3]);

        let mut audit = SceneFrameAuditSink::default();
        audit.submit(&frame);
        let summary = audit.take_summary();
        assert_eq!(summary.effects, 1);
        assert_eq!(summary.order_errors, 0);
        assert!(owner.room_draw_packets().is_empty());
    }

    #[test]
    fn owner_accepts_only_typed_draw_packets_from_the_published_room_scope() {
        let mut owner = NativeSceneOwner::default();
        owner.publish_room_plan(7, &[3, 9], 2).unwrap();
        let packet = RoomDrawPacket {
            frame_sequence: 7,
            room: 9,
            indexed: 1,
            primitive: 5,
            first: 0,
            count: 12,
            vertex_buffer: 0x1000,
            vertex_buffer_bytes: 4096,
            vertex_stride: 24,
            vertex_first: 0,
            index_buffer: 0x2000,
            index_buffer_bytes: 24,
            texture_headers: [0x3000, 0, 0, 0],
            texture_data: [0x4000, 0, 0, 0],
            texture_formats: [0x112, 0, 0, 0],
            texture_sizes: [0x10001, 0, 0, 0],
            texture_stage_state: [0; 128],
            viewport_words: [0; 4],
            viewport_target: [0; 4],
            scissor_words: [0; 4],
            scissor_count: 0,
            scissor_exclusive: 0,
            vertex_declaration: 0xD,
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
            cull_mode: 0,
            stencil_enable: 0,
            stencil_function: 0,
            stencil_reference: 0,
            stencil_mask: 0,
            stencil_write_mask: 0,
            stencil_fail: 0,
            stencil_depth_fail: 0,
            stencil_pass: 0,
            vertex_bytes: 0,
            index_bytes: 0,
            texture_bytes: [0; 4],
            payload_dropped: false,
            render_state: [0; 256],
            payload_hash: 0,
            payload: std::sync::Arc::from([]),
            material_key: 11,
            state_key: 13,
            scene_light_binding: None,
        };
        owner
            .publish_room_draw_packets(7, &[packet.clone()])
            .unwrap();
        assert_eq!(owner.room_draw_packets(), &[packet.clone()]);

        let wrong_room = RoomDrawPacket {
            room: 4,
            ..packet.clone()
        };
        assert_eq!(
            owner.publish_room_draw_packets(7, &[wrong_room]),
            Err(SceneError::InvalidRoomDrawPacket)
        );
        assert_eq!(owner.room_draw_packets(), &[packet]);

        let record = owner
            .setup_scene_light_record(SceneLightInputs {
                colors: [Color3([1.0, 0.0, 0.0]); 4],
                world_points: [WorldPoint([1.0, 2.0, 3.0]); 3],
            })
            .unwrap();
        let view = owner.setup_scene_light_view().unwrap();
        let epoch = FrameResourceEpoch(3);
        let unpublished_record = owner
            .setup_scene_light_record(SceneLightInputs {
                colors: [Color3([0.0; 3]); 4],
                world_points: [WorldPoint([0.0; 3]); 3],
            })
            .unwrap();
        owner.set_scene_light_publication_enabled(true);
        owner
            .light_state
            .prepare_view(record, view, epoch, |point| {
                crate::scene::light_state::ViewPoint([point.0[0], point.0[1], point.0[2], 1.0])
            })
            .unwrap();
        owner
            .light_state
            .publish_frame(epoch, &[SceneLightBinding::new(record, view)])
            .unwrap();

        assert_eq!(
            owner
                .bind_room_draw_scene_lights(epoch, &[Some(SceneLightBinding::new(record, view))],),
            Ok(())
        );
        assert_eq!(
            owner.room_draw_packets()[0].scene_light_binding,
            Some(SceneLightBinding::new(record, view))
        );
        assert_eq!(
            owner.bind_room_draw_scene_lights(
                epoch,
                &[Some(SceneLightBinding::new(unpublished_record, view))],
            ),
            Err(SceneLightError::InvalidDrawBinding)
        );
        assert_eq!(
            owner.room_draw_packets()[0].scene_light_binding,
            Some(SceneLightBinding::new(record, view)),
            "rejected binding must leave the prior draw association intact"
        );
    }

    #[test]
    fn owner_accepts_generation_qualified_membership_and_camera_view_inputs() {
        let mut owner = NativeSceneOwner::default();
        owner.publish_room_plan(1, &[7], 0).unwrap();
        let objects = [
            ObjectId {
                slot: 3,
                generation: 1,
            },
            ObjectId {
                slot: 8,
                generation: 4,
            },
        ];
        let views = [NativeViewInput {
            viewer: 0,
            position: [1.0, 2.0, 3.0],
            matrix: [0.0; 16],
            viewport: [0.0, 640.0, 0.0, 480.0],
            rectangle: [0, 0, 640, 480],
        }];
        owner.publish_bridge_inputs(1, &objects, &views).unwrap();
        owner.finish_tick(1).unwrap();
        assert_eq!(owner.observed_objects(), objects);
        assert_eq!(owner.observed_views(), views);
        let prepared = owner.prepare_display_frame(&[], 0.0).unwrap();
        assert_eq!(prepared.room_plan.as_ref().unwrap().rooms, [7]);
        assert_eq!(prepared.observed_objects, objects);
        assert_eq!(prepared.observed_views, views);

        let mut sink = SceneFrameAuditSink::default();
        sink.observe_owner_inputs(owner.observed_objects(), owner.observed_views());
        owner.submit_display_frame(&[], 0.0, &mut sink).unwrap();
        let summary = sink.take_summary();
        assert_eq!(summary.objects, 2);
        assert_eq!(summary.views, 1);
        assert_eq!(summary.order_errors, 0);

        let duplicate_slot = [
            objects[0],
            ObjectId {
                slot: 3,
                generation: 2,
            },
        ];
        assert_eq!(
            owner.publish_bridge_inputs(1, &duplicate_slot, &views),
            Err(SceneError::ObjectIdentityMismatch)
        );
    }

    #[test]
    fn pool_record_retirement_reuses_native_slot_with_new_generation() {
        let mut owner = NativeSceneOwner::default();
        owner.publish_room_plan(10, &[2], 0).unwrap();
        owner.publish_pool_record_inputs(10, &[17], &[]).unwrap();
        let retired = owner.observed_objects()[0];
        assert_eq!(retired.slot, 0);
        assert_eq!(retired.generation, 1);
        assert!(owner.retire_pool_record(17));
        assert!(owner.observed_objects().is_empty());

        owner.publish_room_plan(11, &[2], 0).unwrap();
        owner.publish_pool_record_inputs(11, &[17], &[]).unwrap();
        assert_eq!(
            owner.observed_objects(),
            [ObjectId {
                slot: retired.slot,
                generation: retired.generation + 1,
            }]
        );
    }

    #[test]
    fn owner_rejects_invalid_or_reordered_room_plan_publications() {
        let mut owner = NativeSceneOwner::default();
        assert_eq!(
            owner.publish_room_plan(1, &[4, 4], 0),
            Err(SceneError::InvalidRoomPlan)
        );
        owner.publish_room_plan(1, &[4, 9], 0).unwrap();
        assert_eq!(
            owner.publish_room_plan(1, &[9, 4], 0),
            Err(SceneError::InvalidTick)
        );
        assert_eq!(owner.room_plan().unwrap().rooms, [4, 9]);
    }

    #[test]
    fn native_scene_owner_refreshes_pool_after_guest_tick() {
        let records = 0x4300_0000;
        let mut image = Image(BTreeMap::new());
        image.write32(SCALARS[S_CAPACITY].0, 1);
        image.write32(SCALARS[S_RECORDS_BASE].0, records);
        image.write32(SCALARS[S_USED_RECORDS].0, 1);
        image.write32(records + TsfpObjectView::OFFSET_MODEL_ID, 0x1234);

        let mut owner = NativeSceneOwner::default();
        owner.refresh_pool_from_guest(&image);

        assert_eq!(owner.pool.owned_records(), 1);
        assert_eq!(
            owner.pool.owned_record(0).unwrap()[(TsfpObjectView::OFFSET_MODEL_ID / 4) as usize],
            0x1234
        );
    }

    #[test]
    fn guest_tick_publication_keeps_last_complete_pair_on_join_failure() {
        let image = Image(BTreeMap::new());
        let mut owner = NativeSceneOwner::default();
        owner.finish_guest_tick(4, &image).unwrap();
        assert_eq!(owner.current_tick(), Some(4));

        // An explicit lifecycle member without a matching guest pool row is
        // rejected; refreshing guest-owned pool state cannot publish a partial
        // replacement over the last complete interval.
        let id = owner.spawn(3).unwrap();
        owner.attach_room(id, 0, 0xabc, 7, 1, true, false).unwrap();
        assert!(owner.finish_guest_tick(5, &image).is_err());
        assert_eq!(owner.current_tick(), Some(4));
    }

    #[test]
    fn pool_membership_audit_preserves_room_order_and_rejects_incomplete_rows() {
        let records = 0x4300_0000;
        let arena = 0x4400_0000;
        let mut image = Image(BTreeMap::new());
        image.write32(SCALARS[S_CAPACITY].0, 2);
        image.write32(SCALARS[S_RECORDS_BASE].0, records);
        image.write32(SCALARS[S_USED_RECORDS].0, 2);
        image.write32(SCALARS[S_ARENA_BASE].0, arena);
        image.write32(SCALARS[S_ARENA_USED].0, 3);
        for room in [2usize, 5] {
            image.write32(cell_addr(CAPS, room), 3);
        }
        image.write32(cell_addr(COUNTS, 2), 2);
        image.write32(cell_addr(MEMBERS, 2), arena);
        image.write32(cell_addr(COUNTS, 5), 1);
        image.write32(cell_addr(MEMBERS, 5), arena + 8);
        image.write32(arena, records + RECORD_SIZE as u32);
        image.write32(arena + 4, records);
        image.write32(arena + 8, records);
        let pool = ObjectPool::load(&image);

        let audit = pool.audit_membership().unwrap();
        assert_eq!(audit.total_members, 3);
        assert_eq!(audit.rooms[0].room, 2);
        assert_eq!(audit.rooms[0].records, [1, 0]);
        assert_eq!(audit.rooms[1].room, 5);
        assert_eq!(audit.rooms[1].records, [0]);

        let mut incomplete = pool.clone();
        incomplete.arena_len = 2;
        assert_eq!(
            incomplete.audit_membership(),
            Err(MembershipAuditError::MissingArenaMember {
                room: 5,
                ordinal: 0
            })
        );
        let mut duplicate = pool;
        duplicate.arena[1] = records + RECORD_SIZE as u32;
        assert_eq!(
            duplicate.audit_membership(),
            Err(MembershipAuditError::DuplicateRecord { room: 2, record: 1 })
        );
    }

    struct Image(BTreeMap<u32, u8>);
    impl GuestMemory for Image {
        fn read8(&self, address: u32) -> u8 {
            *self.0.get(&address).unwrap_or(&0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn public_owner_lifecycle_api_publishes_multi_room_scene() {
        let records = 0x4300_0000;
        let arena = 0x4400_0000;
        let mut image = Image(BTreeMap::new());
        image.write32(SCALARS[S_CAPACITY].0, 1);
        image.write32(SCALARS[S_RECORDS_BASE].0, records);
        image.write32(SCALARS[S_USED_RECORDS].0, 1);
        image.write32(SCALARS[S_ARENA_BASE].0, arena);
        image.write32(SCALARS[S_ARENA_USED].0, 2);
        for room in [2usize, 4] {
            image.write32(cell_addr(CAPS, room), 1);
            image.write32(cell_addr(COUNTS, room), 1);
        }
        image.write32(cell_addr(MEMBERS, 2), arena);
        image.write32(cell_addr(MEMBERS, 4), arena + 4);
        image.write32(arena, records);
        image.write32(arena + 4, records);
        image.write32(records + TsfpObjectView::OFFSET_MODEL_ID, 9);
        let mut owner = NativeSceneOwner::default();
        owner.pool = ObjectPool::load(&image);
        let id = owner.spawn(7).unwrap();
        owner.attach_room(id, 0, 0x900, 9, 2, true, false).unwrap();
        owner.attach_room(id, 0, 0x900, 9, 4, false, true).unwrap();
        let components = RenderComponents {
            placement: tsfp_state::transforms::WorldTransform {
                position: [2.0, 0.0, 0.0],
                orientation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            bounds: [[-1.0; 3], [1.0; 3]],
            motion_bounds: [[-2.0; 3], [2.0; 3]],
            scissor: [0.0, 0.0, 640.0, 480.0],
            scissor_rejected: false,
            model_hidden: false,
            overlay: 0,
            discontinuity: false,
        };
        owner.set_render_state(id, components).unwrap();
        let rows = owner.membership.ordered().to_vec();
        owner.finish_tick_with_rows(10, &rows).unwrap();
        let pair = owner.pair().unwrap();
        let fixed = pair
            .prepare_frame(&[], 0.0, Sampling::Fixed, &SlerpPlacementSampler)
            .unwrap();
        let interpolated = pair
            .prepare_frame(&[], 0.75, Sampling::Fixed, &SlerpPlacementSampler)
            .unwrap();
        assert_eq!(fixed.tick, 10);
        assert_eq!(fixed.views, interpolated.views);
        assert_eq!(owner.membership.ordered()[0].rooms, [2, 4]);
        owner.retire(id).unwrap();
        owner.finish_tick(11).unwrap();
        assert_eq!(owner.current_tick(), Some(11));
        assert_eq!(owner.object_count(), Some(0));
    }

    #[test]
    fn failed_first_room_attach_does_not_bind_record() {
        let mut owner = NativeSceneOwner::default();
        let id = owner.spawn(3).unwrap();
        assert!(
            owner
                .attach_room(id, 77, 0, 0, CELLS as u16, true, false)
                .is_err()
        );
        assert_eq!(owner.membership.record_index(id), None);
        owner.attach_room(id, 0, 0x900, 9, 2, true, false).unwrap();
        assert_eq!(owner.membership.record_index(id), Some(0));
    }

    #[test]
    fn lifecycle_membership_publishes_reusable_scene_history() {
        let records = 0x4300_0000;
        let arena = 0x4400_0000;
        let mut image = Image(BTreeMap::new());
        image.write32(SCALARS[S_CAPACITY].0, 1);
        image.write32(SCALARS[S_RECORDS_BASE].0, records);
        image.write32(SCALARS[S_USED_RECORDS].0, 1);
        image.write32(SCALARS[S_ARENA_BASE].0, arena);
        image.write32(SCALARS[S_ARENA_USED].0, 2);
        image.write32(cell_addr(COUNTS, 2), 1);
        image.write32(cell_addr(MEMBERS, 2), arena);
        image.write32(cell_addr(COUNTS, 4), 1);
        image.write32(cell_addr(MEMBERS, 4), arena + 4);
        image.write32(arena, records);
        image.write32(arena + 4, records);
        image.write32(records + TsfpObjectView::OFFSET_MODEL_ID, 9);
        let mut pool = ObjectPool::load(&image);

        let mut owner = SceneMembership::default();
        let id = owner.spawn(6).unwrap();
        owner.attach(id, 0x900, 9, 2, true, false).unwrap();
        owner.attach(id, 0x900, 9, 4, false, true).unwrap();
        owner.bind_record(id, 0).unwrap();
        owner
            .set_render_state(
                id,
                RenderComponents {
                    placement: tsfp_state::transforms::WorldTransform {
                        position: [2.0, 0.0, 0.0],
                        orientation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    },
                    bounds: [[-1.0; 3], [1.0; 3]],
                    motion_bounds: [[-2.0; 3], [2.0; 3]],
                    scissor: [0.0, 0.0, 640.0, 480.0],
                    scissor_rejected: false,
                    model_hidden: false,
                    overlay: 0,
                    discontinuity: false,
                },
            )
            .unwrap();

        let mut history = SceneTickHistory::default();
        publish_membership_tick(&mut history, 44, &owner, &pool).unwrap();
        assert_eq!(history.current_tick(), Some(44));
        assert_eq!(history.object_count(), Some(1));
        let pair = history.pair().unwrap();
        let first = pair
            .prepare_frame(&[], 0.0, Sampling::Fixed, &SlerpPlacementSampler)
            .unwrap();
        let second = pair
            .prepare_frame(&[], 1.0, Sampling::Fixed, &SlerpPlacementSampler)
            .unwrap();
        assert_eq!(first.tick, second.tick);
        assert_eq!(first.views, second.views);

        let mut refreshed = owner.clone();
        let mut moved = refreshed.render_state(id).unwrap();
        moved.placement.position[0] = 4.0;
        refreshed.set_render_state(id, moved).unwrap();
        publish_membership_tick(&mut history, 44, &refreshed, &pool).unwrap();
        let pair = history.pair().unwrap();
        let again = pair
            .prepare_frame(&[], 0.5, Sampling::Fixed, &SlerpPlacementSampler)
            .unwrap();
        assert_eq!(again.tick, 44);

        owner.retire(id).unwrap();
        let recycled = owner.spawn(6).unwrap();
        owner.attach(recycled, 0x900, 9, 2, true, false).unwrap();
        owner.bind_record(recycled, 1).unwrap();
        assert_eq!(
            snapshot_from_membership(45, &owner, &pool),
            Err(MembershipError::MissingPoolRecord)
        );
        assert_eq!(
            publish_membership_tick(&mut history, 45, &owner, &pool),
            Err(SceneError::ObjectIdentityMismatch)
        );
        assert_eq!(history.current_tick(), Some(44));
        let _ = &mut pool;
    }
}
