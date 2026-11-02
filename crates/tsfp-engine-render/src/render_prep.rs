// SPDX-License-Identifier: GPL-3.0-or-later
//! Immutable typed render preparation (T2238). Owner APIs accept native IDs,
//! never guest offsets. Hybrid decoding/submission is separate in scene.rs.
use super::spatial::{Bounds, Plane, SpatialTree};
use crate::visual_effects::EffectEntry;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    Character,
    Player,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneObject {
    pub id: ObjectId,
    pub model: ModelId,
    pub visible: bool,
    pub scissor_rejected: bool,
    pub model_hidden: bool,
    pub deferred: bool,
    pub overlay: Overlay,
    pub scissor: [f32; 4],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectCommand {
    Scissor([i32; 4]),
    CharacterOverlay(ObjectId),
    PlayerOverlay(ObjectId),
    Defer(ObjectId),
    DrawModel(ModelId),
}

/// Stable native model asset identity. The T2244 compatibility adapter may
/// temporarily wrap a guest address, but native scene owners supply asset IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModelId(pub u64);
/// Immutable original-preset decisions of 0xA2740. Render flags and deferred
/// queues are view-owned output, never stored on the simulation object.
pub fn prepare_object(object: &SceneObject) -> Vec<ObjectCommand> {
    let mut commands = Vec::new();
    if !object.visible || object.scissor_rejected || object.model_hidden {
        return commands;
    }
    commands.push(ObjectCommand::Scissor(object.scissor.map(truncate)));
    match object.overlay {
        Overlay::None => {}
        Overlay::Character => commands.push(ObjectCommand::CharacterOverlay(object.id)),
        Overlay::Player => commands.push(ObjectCommand::PlayerOverlay(object.id)),
    }
    commands.push(if object.deferred {
        ObjectCommand::Defer(object.id)
    } else {
        ObjectCommand::DrawModel(object.model)
    });
    commands
}
/// CVTTSS2SI indefinite result on NaN and out-of-range, independent of std.
fn truncate(value: f32) -> i32 {
    if !(-2147483648.0..2147483648.0).contains(&value) {
        i32::MIN
    } else {
        value as i32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelNode {
    pub mask: u32,
    pub child: Option<usize>,
    pub sibling: Option<usize>,
    pub stop_children: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelCommand {
    Enter,
    Draw(usize),
    Leave,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    Cycle,
}
/// Original 0x8F5E0 preorder: selected node, child unless type 3, sibling.
/// A per-path cycle check permits repeated nodes reached through different paths.
/// Out-of-range links are ignored like the signed byte indices in the original.
pub fn model_commands(
    nodes: &[ModelNode],
    root: usize,
    mask: u32,
) -> Result<Vec<ModelCommand>, ModelError> {
    enum Step {
        Enter(usize),
        Leave(usize),
    }
    let mut pending = Vec::new();
    let mut path = alloc::vec![false; nodes.len()];
    let mut draws = Vec::new();
    pending.push(Step::Enter(root));
    while let Some(step) = pending.pop() {
        match step {
            Step::Leave(index) => {
                path[index] = false;
                draws.push(ModelCommand::Leave);
            }
            Step::Enter(index) => {
                let Some(node) = nodes.get(index) else {
                    continue;
                };
                if path[index] {
                    return Err(ModelError::Cycle);
                }
                path[index] = true;
                draws.push(ModelCommand::Enter);
                if node.mask == 0 || node.mask & mask != 0 {
                    draws.push(ModelCommand::Draw(index));
                }
                pending.push(Step::Leave(index));
                if let Some(sibling) = node.sibling {
                    pending.push(Step::Enter(sibling));
                }
                if !node.stop_children {
                    if let Some(child) = node.child {
                        pending.push(Step::Enter(child));
                    }
                }
            }
        }
    }
    Ok(draws)
}

/// Draw-only view, useful for prefetching resources before submission.
pub fn model_nodes(nodes: &[ModelNode], root: usize, mask: u32) -> Result<Vec<usize>, ModelError> {
    Ok(model_commands(nodes, root, mask)?
        .into_iter()
        .filter_map(|command| match command {
            ModelCommand::Draw(index) => Some(index),
            _ => None,
        })
        .collect())
}

/// A stable scene identity includes the lifetime generation; retired slots
/// never interpolate into a newly spawned object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectId {
    pub slot: u32,
    pub generation: u64,
}
/// Owner identity for the transform that positions a scene draw. A renderable
/// object may be controlled by a player or character owner, so draw identity
/// and transform identity are deliberately separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransformNamespace {
    Object,
    Player,
    Character,
    Camera,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransformIdentity {
    pub namespace: TransformNamespace,
    pub slot: u64,
    pub generation: u64,
}
/// Transform owners attached to a draw at the two ends of a render interval.
/// A changed owner is a discontinuity: its current sample is used at every
/// interpolation alpha because the two owners' poses are not comparable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransformBinding {
    pub object: ObjectId,
    pub previous: TransformIdentity,
    pub current: TransformIdentity,
}
/// Per-camera transform binding for producers that compose a different world
/// placement for each transform view, such as mirrored split-screen views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewTransformBinding {
    pub object: ObjectId,
    pub viewer: u32,
    pub transform_view: u32,
    pub previous: TransformIdentity,
    pub current: TransformIdentity,
}
impl From<ObjectId> for TransformIdentity {
    fn from(id: ObjectId) -> Self {
        Self {
            namespace: TransformNamespace::Object,
            slot: u64::from(id.slot),
            generation: id.generation,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub position: [f32; 3],
    pub orientation: [[f32; 3]; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectSnapshot {
    pub id: ObjectId,
    pub draw: SceneObject,
    pub placement: Placement,
    pub bounds: Bounds,
    /// Owner-supplied conservative bounds for every display sample of this tick.
    /// Includes rotation/deformation; renderer never guesses a motion envelope.
    pub motion_bounds: Bounds,
    pub rooms: Vec<u16>,
    /// Spawn, teleport, respawn or a camera/owner cut: snap all presentation.
    pub discontinuity: bool,
}

/// Complete owner-authored row required to admit one renderable into a scene
/// snapshot. This is deliberately separate from the bounded guest object-pool
/// mirror: only the owning scene system can provide stable lifetime identity,
/// traversal order, room membership, transform bindings and conservative
/// display-sample bounds together.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneObjectAdmission {
    pub snapshot: ObjectSnapshot,
    pub transform_bindings: Vec<TransformBinding>,
    pub view_transform_bindings: Vec<ViewTransformBinding>,
}

/// Producer-side contract for a complete tick. Implementations must enumerate
/// the owner's full render membership in original submission order, including
/// non-pool objects, and return no partial result when collection fails.
pub trait SceneSnapshotSource {
    type Error;

    fn tick(&self) -> u64;
    fn collect_ordered(&self) -> Result<Vec<SceneObjectAdmission>, Self::Error>;
}

/// Validate and materialize one complete producer result. Binding rows are
/// checked against admitted object identities so stale or foreign transforms
/// cannot silently attach to another draw.
pub fn collect_scene_snapshot<S: SceneSnapshotSource>(
    source: &S,
) -> Result<
    (
        SceneSnapshot,
        Vec<TransformBinding>,
        Vec<ViewTransformBinding>,
    ),
    SceneCollectError<S::Error>,
> {
    let tick = source.tick();
    let admissions = source
        .collect_ordered()
        .map_err(SceneCollectError::Source)?;
    let mut builder = SceneSnapshotBuilder::new(tick);
    let mut objects = BTreeSet::new();
    let mut transforms = Vec::new();
    let mut view_transforms = Vec::new();
    for admission in admissions {
        let id = admission.snapshot.id;
        builder
            .push(admission.snapshot)
            .map_err(SceneCollectError::Scene)?;
        objects.insert(id);
        if admission
            .transform_bindings
            .iter()
            .any(|binding| binding.object != id)
            || admission
                .view_transform_bindings
                .iter()
                .any(|binding| binding.object != id)
        {
            return Err(SceneCollectError::BindingIdentityMismatch);
        }
        transforms.extend(admission.transform_bindings);
        view_transforms.extend(admission.view_transform_bindings);
    }
    if transforms
        .iter()
        .any(|binding| !objects.contains(&binding.object))
        || view_transforms
            .iter()
            .any(|binding| !objects.contains(&binding.object))
    {
        return Err(SceneCollectError::BindingIdentityMismatch);
    }
    Ok((builder.finish(), transforms, view_transforms))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneCollectError<E> {
    Source(E),
    Scene(SceneError),
    BindingIdentityMismatch,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SceneSnapshot {
    pub tick: u64,
    /// Original traversal order, supplied by the owning object system.
    pub objects: Vec<ObjectSnapshot>,
}

/// Incremental owner for one complete scene tick. Rows are retained in the
/// order supplied by the object/room traversal; invalid or duplicate rows are
/// rejected before they can enter the finished snapshot.
pub struct SceneSnapshotBuilder {
    snapshot: SceneSnapshot,
    identities: BTreeSet<ObjectId>,
}

impl SceneSnapshotBuilder {
    pub fn new(tick: u64) -> Self {
        Self {
            snapshot: SceneSnapshot {
                tick,
                objects: Vec::new(),
            },
            identities: BTreeSet::new(),
        }
    }

    pub fn push(&mut self, object: ObjectSnapshot) -> Result<(), SceneError> {
        if object.id != object.draw.id {
            return Err(SceneError::ObjectIdentityMismatch);
        }
        if !self.identities.insert(object.id) {
            return Err(SceneError::DuplicateObject);
        }
        self.snapshot.objects.push(object);
        Ok(())
    }

    /// Number of owner-ordered objects collected so far. This is useful for
    /// producer diagnostics without exposing a partially built snapshot.
    pub fn len(&self) -> usize {
        self.snapshot.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshot.objects.is_empty()
    }

    /// Finish collection only after the owner has visited its complete scene.
    /// The caller publishes this value to `SceneTickHistory` after collection
    /// returns successfully; dropping the builder discards a partial scene.
    pub fn finish(self) -> SceneSnapshot {
        self.snapshot
    }
}

/// Camera/visibility owns these typed values, not globals or a mutable query.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub query: Bounds,
    pub planes: Vec<Plane>,
    pub visible_rooms: BTreeSet<u16>,
}
impl View {
    /// Join T2330's sampled camera room endpoints with T2238's owned portal
    /// visibility result. The caller supplies the matching spatial query and
    /// frustum planes; this adapter only maps visible room identities.
    pub fn from_camera_visibility(
        camera: &crate::camera::CameraPresentation,
        visibility: &super::visibility::VisibilityOutput,
        query: Bounds,
        planes: Vec<Plane>,
        sampling: Sampling,
    ) -> Result<Self, SceneError> {
        let mut visible_rooms = BTreeSet::new();
        let camera_rooms = if sampling == Sampling::Fixed {
            &camera.visible_rooms[1..]
        } else {
            &camera.visible_rooms[..]
        };
        for &room in camera_rooms {
            // Negative camera room IDs are the original absent-room sentinel.
            if room < 0 {
                continue;
            }
            visible_rooms.insert(u16::try_from(room).map_err(|_| SceneError::RoomIdOutOfRange)?);
        }
        for (room, coverage) in visibility.rooms.iter().enumerate() {
            if coverage.depth >= 0 && coverage.rect.nonempty() {
                visible_rooms
                    .insert(u16::try_from(room).map_err(|_| SceneError::RoomIdOutOfRange)?);
            }
        }
        Ok(Self {
            query,
            planes,
            visible_rooms,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sampling {
    /// Original preset uses the current fixed-tick state; alpha is irrelevant.
    Fixed,
    /// Optional display interpolation. 0 = previous, 1 = current; a cut snaps.
    BetweenTicks,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedObject {
    pub id: ObjectId,
    pub placement: Placement,
    /// Current room membership for Fixed/discontinuous samples; the stable
    /// union of both endpoints for continuous BetweenTicks samples. Empty
    /// retains the owner's unbounded-room sentinel.
    pub rooms: Vec<u16>,
    pub commands: Vec<ObjectCommand>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedView {
    pub tick: u64,
    /// Rust-issued identity shared with scene-light preparation for this view.
    /// `None` means this prepared view is not bound to scene-light data.
    pub scene_light_view: Option<super::light_state::ViewId>,
    /// Query, frustum and room set used to prepare this view.
    pub view: View,
    /// Camera and portal results are present for `prepare_camera_frame` views;
    /// the low-level `prepare_frame` API leaves these absent.
    pub camera: Option<crate::camera::CameraPresentation>,
    /// Paired portal-walk input and result for camera-aware views.
    pub visibility: Option<super::visibility::VisibilityPresentation>,
    /// Viewer identity and screen-space target for camera-aware views.
    pub target: Option<ViewTarget>,
    /// Typed room-depth collection paired with this camera's prepared objects.
    pub room_order: Option<super::scene_collect::RoomOrder>,
    pub objects: Vec<PreparedObject>,
}
/// One camera/view input to a display frame. Each request may use a different
/// visibility index and room/plane set while sharing the same scene endpoints.
pub struct ViewRequest<'a> {
    pub index: &'a SpatialTree,
    pub view: &'a View,
}
/// One complete camera inlet for native display-frame preparation. Room IDs
/// come from the sampled camera and paired portal visibility result; query
/// bounds and frustum planes remain explicit owner inputs. The camera path
/// scans every object in the current snapshot and does not require a spatial
/// index that could omit scene members.
pub struct CameraViewRequest<'a> {
    pub camera: &'a crate::camera::CameraPresentation,
    pub visibility: &'a super::visibility::VisibilityPresentation,
    /// Optional stable identity used to join this sampled camera to typed
    /// scene-light blocks prepared for the same view.
    pub scene_light_view: Option<super::light_state::ViewId>,
    /// Native room-depth collection for this render target.
    pub room_order: &'a super::scene_collect::RoomOrder,
    pub query: Bounds,
    pub planes: Vec<Plane>,
}
/// Render-target metadata owned by the camera/visibility system for one view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTarget {
    pub viewer: u32,
    pub viewport: super::visibility::ProjectionViewport,
    pub rectangle: super::visibility::Rect,
}

/// Bounded camera observation copied by the live owner bridge. These values
/// identify the active screen rectangle and camera for native consumers; they
/// are not a reconstructed projection or frustum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeViewInput {
    pub viewer: u32,
    pub position: [f32; 3],
    pub matrix: [f32; 16],
    pub viewport: [f32; 4],
    pub rectangle: [i32; 4],
}

/// Complete native render-preparation result for one display frame. Every view
/// is sampled from the same immutable tick pair and alpha.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSceneFrame {
    pub tick: u64,
    pub alpha: f32,
    pub sampling: Sampling,
    /// Epoch issued by `NativeSceneOwner` for this sampled camera/resource set.
    /// Low-level `ScenePair` frames leave this unset.
    pub resource_epoch: Option<super::light_state::FrameResourceEpoch>,
    /// Ordered room identities copied from the owner. The native comparison
    /// sink can draw one proxy per room in this order; this carries no retail
    /// mesh or material data.
    pub room_plan: Option<PreparedRoomPlan>,
    /// Lifecycle identities and camera observations copied from the owner
    /// boundary. They are not model assets or fully prepared native views.
    pub observed_objects: Vec<ObjectId>,
    pub observed_views: Vec<NativeViewInput>,
    /// Fixed-tick Rust-owned effect snapshot. Sinks may audit this typed data;
    /// the current renderer intentionally does not draw it.
    pub effects_tick: Option<u64>,
    pub effects: Vec<EffectEntry>,
    /// Opt-in Rust-owned light snapshot. `None` leaves the current renderer
    /// path unchanged until the native light consumer passes its adoption gate.
    pub scene_lights: Option<super::light_state::PublishedSceneLights>,
    pub views: Vec<PreparedView>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedRoomPlan {
    pub tick: u64,
    pub draw_max_depth: i32,
    pub rooms: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneError {
    InvalidFraction,
    InvalidTick,
    FrameResourceEpochExhausted,
    InvalidRoomPlan,
    InvalidRoomDrawPacket,
    MissingSnapshotHistory,
    DuplicateObject,
    DuplicateTransformBinding(ObjectId),
    ObjectIdentityMismatch,
    UnknownTransformBinding(ObjectId),
    RoomIdOutOfRange,
    InvalidViewer,
    CameraEyeMismatch,
    CameraMatrixMismatch,
    InvalidTree,
    HistoryTickMismatch,
    MissingTransform(ObjectId),
    DuplicateViewTransformBinding(ObjectId, u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformSamples {
    pub previous: Placement,
    pub current: Placement,
    pub discontinuity: bool,
}

/// Adapter boundary for an owner that stores authoritative object transforms.
/// Kept local to render prep so the no_std lane build needs no state-crate link.
pub trait TransformHistorySource {
    fn tick(&self) -> Option<u64>;
    fn transform_samples(&self, id: TransformIdentity) -> Option<TransformSamples>;

    /// View-specific owners can override this when their world transform
    /// depends on the camera viewer. Tick-wide sources share one sample.
    fn transform_samples_for_view(
        &self,
        id: TransformIdentity,
        _transform_view: u32,
    ) -> Option<TransformSamples> {
        self.transform_samples(id)
    }
}

/// Validated immutable state for one render interval. Construct once after a
/// simulation tick and reuse it for each display-rate view or camera. The
/// previous-object lookup and duplicate checks are shared across those calls.
pub struct ScenePair<'a> {
    previous: &'a SceneSnapshot,
    current: &'a SceneSnapshot,
    previous_objects: BTreeMap<ObjectId, &'a ObjectSnapshot>,
    transforms: BTreeMap<ObjectId, TransformSamples>,
    view_transforms: BTreeMap<(ObjectId, u32), TransformSamples>,
}

/// Owner of the two most recent fixed-tick scene snapshots. Publish once when
/// simulation state changes; keep the same pair for any number of display-rate
/// samples. Same-tick refreshes replace the current endpoint without rotating
/// history. A missed tick preserves its real tick labels and marks current
/// objects as cuts, so interpolation cannot bridge an unobserved interval.
#[derive(Default)]
pub struct SceneTickHistory {
    previous: Option<SceneSnapshot>,
    current: Option<SceneSnapshot>,
}

impl SceneTickHistory {
    /// Validate and atomically publish one complete owner-built snapshot.
    /// Callers should finish collection before invoking this method; an
    /// incomplete traversal must never replace the reusable display interval.
    pub fn publish(&mut self, mut snapshot: SceneSnapshot) -> Result<(), SceneError> {
        // Reuse the pair's identity and duplicate validation before changing
        // owned history, so an invalid publication leaves the old pair intact.
        ScenePair::new(&snapshot, &snapshot)?;

        let Some(current) = self.current.as_ref() else {
            self.previous = Some(snapshot.clone());
            self.current = Some(snapshot);
            return Ok(());
        };
        if snapshot.tick < current.tick {
            return Err(SceneError::InvalidTick);
        }
        if snapshot.tick == current.tick {
            self.current = Some(snapshot);
            return Ok(());
        }
        if current.tick.checked_add(1) == Some(snapshot.tick) {
            self.previous = self.current.take();
            self.current = Some(snapshot);
            return Ok(());
        }

        // Preserve the actual previous endpoint, but snap every object across
        // the gap because skipped simulation state cannot be interpolated.
        for object in &mut snapshot.objects {
            object.discontinuity = true;
        }
        self.previous = self.current.take();
        self.current = Some(snapshot);
        Ok(())
    }

    pub fn current_tick(&self) -> Option<u64> {
        self.current.as_ref().map(|snapshot| snapshot.tick)
    }

    pub fn object_count(&self) -> Option<usize> {
        self.current.as_ref().map(|snapshot| snapshot.objects.len())
    }

    pub fn pair(&self) -> Result<ScenePair<'_>, SceneError> {
        ScenePair::new(
            self.previous
                .as_ref()
                .ok_or(SceneError::MissingSnapshotHistory)?,
            self.current
                .as_ref()
                .ok_or(SceneError::MissingSnapshotHistory)?,
        )
    }

    pub fn pair_with_transform_history(
        &self,
        history: &impl TransformHistorySource,
    ) -> Result<ScenePair<'_>, SceneError> {
        ScenePair::with_transform_history(
            self.previous
                .as_ref()
                .ok_or(SceneError::MissingSnapshotHistory)?,
            self.current
                .as_ref()
                .ok_or(SceneError::MissingSnapshotHistory)?,
            history,
        )
    }
}

impl<'a> ScenePair<'a> {
    pub fn new(
        previous: &'a SceneSnapshot,
        current: &'a SceneSnapshot,
    ) -> Result<Self, SceneError> {
        Self::build(previous, current, None, &[], &[])
    }

    /// Build a reusable render interval directly from T2240 typed transforms.
    /// Object draw metadata remains in the supplied snapshots, which are only
    /// borrowed; transform samples are indexed once and read for each view.
    pub fn with_transform_history(
        previous: &'a SceneSnapshot,
        current: &'a SceneSnapshot,
        history: &impl TransformHistorySource,
    ) -> Result<Self, SceneError> {
        Self::build(previous, current, Some(history), &[], &[])
    }

    /// Build a render interval with explicit owner mappings for scene draws
    /// whose transform is owned by a player, character or camera. Unbound
    /// objects retain the stable ObjectId-to-Object mapping.
    pub fn with_transform_history_and_bindings(
        previous: &'a SceneSnapshot,
        current: &'a SceneSnapshot,
        history: &impl TransformHistorySource,
        bindings: &[(ObjectId, TransformIdentity)],
    ) -> Result<Self, SceneError> {
        let endpoint_bindings: Vec<_> = bindings
            .iter()
            .map(|&(object, identity)| TransformBinding {
                object,
                previous: identity,
                current: identity,
            })
            .collect();
        Self::with_transform_history_and_endpoint_bindings(
            previous,
            current,
            history,
            &endpoint_bindings,
        )
    }

    /// Build a pair with optional viewer-specific endpoint bindings in
    /// addition to the tick-wide object/player bindings. A view binding takes
    /// precedence for that object and visibility viewer only.
    pub fn with_transform_history_and_view_bindings(
        previous: &'a SceneSnapshot,
        current: &'a SceneSnapshot,
        history: &impl TransformHistorySource,
        bindings: &[TransformBinding],
        view_bindings: &[ViewTransformBinding],
    ) -> Result<Self, SceneError> {
        Self::build(previous, current, Some(history), bindings, view_bindings)
    }

    /// Build a render interval with explicit transform owners at both ticks.
    /// If a draw changes owner, preparation snaps it to the new owner's
    /// current transform instead of blending unrelated histories.
    pub fn with_transform_history_and_endpoint_bindings(
        previous: &'a SceneSnapshot,
        current: &'a SceneSnapshot,
        history: &impl TransformHistorySource,
        bindings: &[TransformBinding],
    ) -> Result<Self, SceneError> {
        Self::build(previous, current, Some(history), bindings, &[])
    }

    fn build(
        previous: &'a SceneSnapshot,
        current: &'a SceneSnapshot,
        history: Option<&dyn TransformHistorySource>,
        bindings: &[TransformBinding],
        view_bindings: &[ViewTransformBinding],
    ) -> Result<Self, SceneError> {
        if previous.tick > current.tick {
            return Err(SceneError::InvalidTick);
        }
        if history.is_some_and(|history| history.tick() != Some(current.tick)) {
            return Err(SceneError::HistoryTickMismatch);
        }
        let mut previous_objects = BTreeMap::new();
        for object in &previous.objects {
            if object.id != object.draw.id {
                return Err(SceneError::ObjectIdentityMismatch);
            }
            if previous_objects.insert(object.id, object).is_some() {
                return Err(SceneError::DuplicateObject);
            }
        }
        let mut identities = BTreeSet::new();
        for object in &current.objects {
            if object.id != object.draw.id {
                return Err(SceneError::ObjectIdentityMismatch);
            }
            if !identities.insert(object.id) {
                return Err(SceneError::DuplicateObject);
            }
        }
        let mut transform_bindings = BTreeMap::new();
        for &binding in bindings {
            if !identities.contains(&binding.object) {
                return Err(SceneError::UnknownTransformBinding(binding.object));
            }
            if transform_bindings.insert(binding.object, binding).is_some() {
                return Err(SceneError::DuplicateTransformBinding(binding.object));
            }
        }
        let mut transforms = BTreeMap::new();
        let mut view_transforms = BTreeMap::new();
        if let Some(history) = history {
            for object in &current.objects {
                let binding = transform_bindings.get(&object.id).copied();
                let transform_id = binding
                    .map(|binding| binding.current)
                    .unwrap_or_else(|| object.id.into());
                let samples = history
                    .transform_samples(transform_id)
                    .ok_or(SceneError::MissingTransform(object.id))?;
                let samples = if binding.is_some_and(|binding| binding.previous != binding.current)
                {
                    TransformSamples {
                        previous: samples.current,
                        current: samples.current,
                        discontinuity: true,
                    }
                } else {
                    samples
                };
                transforms.insert(object.id, samples);
            }
            for &binding in view_bindings {
                if !identities.contains(&binding.object) {
                    return Err(SceneError::UnknownTransformBinding(binding.object));
                }
                if binding.viewer >= 16 {
                    return Err(SceneError::InvalidViewer);
                }
                let key = (binding.object, binding.viewer);
                if view_transforms.contains_key(&key) {
                    return Err(SceneError::DuplicateViewTransformBinding(
                        binding.object,
                        binding.viewer,
                    ));
                }
                let samples = history
                    .transform_samples_for_view(binding.current, binding.transform_view)
                    .ok_or(SceneError::MissingTransform(binding.object))?;
                let samples = if binding.previous != binding.current {
                    TransformSamples {
                        previous: samples.current,
                        current: samples.current,
                        discontinuity: true,
                    }
                } else {
                    samples
                };
                view_transforms.insert(key, samples);
            }
        }
        Ok(Self {
            previous,
            current,
            previous_objects,
            transforms,
            view_transforms,
        })
    }

    /// Prepare an independent view from the shared tick pair. The immutable
    /// pair can be called repeatedly with different alpha values and cameras.
    pub fn prepare_view(
        &self,
        index: &SpatialTree,
        view: &View,
        alpha: f32,
        sampling: Sampling,
        sampler: &impl PlacementSampler,
    ) -> Result<PreparedView, SceneError> {
        self.prepare_view_with_context(
            Some(index),
            view,
            None,
            None,
            None,
            None,
            None,
            alpha,
            sampling,
            sampler,
        )
    }

    fn prepare_view_with_context(
        &self,
        index: Option<&SpatialTree>,
        view: &View,
        camera: Option<crate::camera::CameraPresentation>,
        visibility: Option<super::visibility::VisibilityPresentation>,
        target: Option<ViewTarget>,
        scene_light_view: Option<super::light_state::ViewId>,
        room_order: Option<&super::scene_collect::RoomOrder>,
        alpha: f32,
        sampling: Sampling,
        sampler: &impl PlacementSampler,
    ) -> Result<PreparedView, SceneError> {
        if target.is_some_and(|target| target.viewer >= 16) {
            return Err(SceneError::InvalidViewer);
        }
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(SceneError::InvalidFraction);
        }
        if sampling == Sampling::BetweenTicks && !self.has_interpolable_interval() {
            return Err(SceneError::InvalidTick);
        }
        let candidates = if let Some(index) = index {
            let mut candidates = BTreeSet::new();
            index
                .query(view.query, |items| {
                    candidates.extend(items.into_iter().map(usize::from));
                    0
                })
                .map_err(|_| SceneError::InvalidTree)?;
            if candidates
                .iter()
                .any(|index| *index >= self.current.objects.len())
            {
                return Err(SceneError::InvalidTree);
            }
            Some(candidates)
        } else {
            None
        };
        let mut objects = Vec::new();
        // An explicit spatial tree prunes candidates; it never reorders draws.
        // Camera-aware full-scene preparation uses all current snapshot entries.
        for (position, object) in self.current.objects.iter().enumerate() {
            if index.is_some() && position > u16::MAX as usize {
                return Err(SceneError::InvalidTree);
            }
            // Camera-aware display preparation scans every object in the
            // snapshot when no explicit acceleration tree is supplied.
            let candidate = candidates
                .as_ref()
                .map_or(true, |candidates| candidates.contains(&position));
            // Current-tick spatial indices can omit an object crossing the view
            // between ticks. The owner-provided sweep keeps interpolation conservative.
            if !candidate
                && !(sampling == Sampling::BetweenTicks
                    && object.motion_bounds.overlaps(view.query))
            {
                continue;
            }
            let old = self.previous_objects.get(&object.id).copied();
            let transform = target
                .and_then(|target| {
                    self.view_transforms
                        .get(&(object.id, target.viewer))
                        .copied()
                })
                .or_else(|| self.transforms.get(&object.id).copied());
            let discontinuity =
                object.discontinuity || transform.is_some_and(|samples| samples.discontinuity);
            let current_placement = transform.map_or(object.placement, |samples| samples.current);
            let current_room_visible = object.rooms.is_empty()
                || object
                    .rooms
                    .iter()
                    .any(|room| view.visible_rooms.contains(room));
            let previous_room_visible = sampling == Sampling::BetweenTicks
                && !discontinuity
                && old.is_some_and(|old| {
                    old.rooms.is_empty()
                        || old
                            .rooms
                            .iter()
                            .any(|room| view.visible_rooms.contains(room))
                });
            if !current_room_visible && !previous_room_visible {
                continue;
            }
            let mut rooms = object.rooms.clone();
            if sampling == Sampling::BetweenTicks && !discontinuity {
                if let Some(old) = old {
                    if rooms.is_empty() || old.rooms.is_empty() {
                        // Empty is the owner's unbounded-room sentinel, so a
                        // union with either unbounded endpoint stays unbounded.
                        rooms.clear();
                    } else {
                        for &room in &old.rooms {
                            if !rooms.contains(&room) {
                                rooms.push(room);
                            }
                        }
                    }
                }
            }
            let (placement, bounds) = match (sampling, old) {
                (Sampling::BetweenTicks, Some(old)) if !discontinuity => {
                    let previous_placement =
                        transform.map_or(old.placement, |samples| samples.previous);
                    (
                        sampler.sample(previous_placement, current_placement, alpha),
                        object.motion_bounds,
                    )
                }
                _ => (current_placement, object.bounds),
            };
            if !bounds.overlaps(view.query) {
                continue;
            }
            // An AABB is rejected only if all its corners are outside one plane.
            if view.planes.iter().any(|plane| {
                (0..8).all(|corner| {
                    let point = core::array::from_fn(|axis| {
                        if corner & (1 << axis) == 0 {
                            bounds.min[axis]
                        } else {
                            bounds.max[axis]
                        }
                    });
                    plane.distance(point) < 0.0
                })
            }) {
                continue;
            }
            let commands = prepare_object(&object.draw);
            if !commands.is_empty() {
                objects.push(PreparedObject {
                    id: object.id,
                    placement,
                    rooms,
                    commands,
                });
            }
        }
        Ok(PreparedView {
            tick: self.current.tick,
            scene_light_view,
            view: view.clone(),
            camera,
            visibility,
            target,
            room_order: room_order.cloned(),
            objects,
        })
    }

    /// Prepare every camera in one display frame from the shared tick pair.
    /// Any invalid view rejects the entire frame, so callers cannot submit a
    /// partially prepared set of cameras. Repeated calls remain independent.
    pub fn prepare_frame(
        &self,
        requests: &[ViewRequest<'_>],
        alpha: f32,
        sampling: Sampling,
        sampler: &impl PlacementSampler,
    ) -> Result<PreparedSceneFrame, SceneError> {
        self.validate_frame_sample(alpha, sampling)?;
        let mut views = Vec::with_capacity(requests.len());
        for request in requests {
            views.push(self.prepare_view(request.index, request.view, alpha, sampling, sampler)?);
        }
        Ok(PreparedSceneFrame {
            tick: self.current.tick,
            alpha,
            sampling,
            resource_epoch: None,
            room_plan: None,
            observed_objects: Vec::new(),
            observed_views: Vec::new(),
            effects_tick: None,
            effects: Vec::new(),
            scene_lights: None,
            views,
        })
    }

    /// Join camera endpoints and portal visibility for every view, then
    /// prepare the complete scene frame from the same immutable tick pair.
    /// Camera views scan every current snapshot object in owner order before
    /// applying bounds, room and frustum culling; they do not rely on a
    /// potentially incomplete caller-provided spatial index.
    /// Errors discard the local partial result, so consumers only receive a
    /// complete set of prepared views. Call this for each display sample.
    pub fn prepare_camera_frame(
        &self,
        requests: &[CameraViewRequest<'_>],
        alpha: f32,
        sampling: Sampling,
        sampler: &impl PlacementSampler,
    ) -> Result<PreparedSceneFrame, SceneError> {
        self.validate_frame_sample(alpha, sampling)?;
        let mut views = Vec::with_capacity(requests.len());
        for request in requests {
            let visibility_view = request.visibility.view();
            if request.camera.eye != visibility_view.position {
                return Err(SceneError::CameraEyeMismatch);
            }
            if request.camera.view != visibility_view.matrix {
                return Err(SceneError::CameraMatrixMismatch);
            }
            let visibility_output = request.visibility.output();
            let target = ViewTarget {
                viewer: visibility_view.viewer,
                viewport: visibility_view.viewport,
                rectangle: visibility_view.full_rect,
            };
            let view = View::from_camera_visibility(
                request.camera,
                visibility_output,
                request.query,
                request.planes.clone(),
                sampling,
            )?;
            views.push(self.prepare_view_with_context(
                None,
                &view,
                Some(*request.camera),
                Some(request.visibility.clone()),
                Some(target),
                request.scene_light_view,
                Some(request.room_order),
                alpha,
                sampling,
                sampler,
            )?);
        }
        Ok(PreparedSceneFrame {
            tick: self.current.tick,
            alpha,
            sampling,
            resource_epoch: None,
            room_plan: None,
            observed_objects: Vec::new(),
            observed_views: Vec::new(),
            effects_tick: None,
            effects: Vec::new(),
            scene_lights: None,
            views,
        })
    }

    /// Prepare the default high-refresh presentation from a fixed-tick pair.
    /// Each call samples the same immutable snapshots at its display alpha,
    /// using linear position and shortest-arc rigid orientation interpolation.
    pub fn prepare_display_frame(
        &self,
        requests: &[CameraViewRequest<'_>],
        alpha: f32,
    ) -> Result<PreparedSceneFrame, SceneError> {
        self.prepare_camera_frame(
            requests,
            alpha,
            Sampling::BetweenTicks,
            &SlerpPlacementSampler,
        )
    }

    /// Prepare the original-preset frame from the current simulation endpoint.
    /// Alpha is reported as zero for the frame-trace contract; `Sampling::Fixed`
    /// intentionally ignores it and never reads previous placements or rooms.
    /// Camera and visibility requests must likewise contain current-tick values.
    pub fn prepare_original_frame(
        &self,
        requests: &[CameraViewRequest<'_>],
    ) -> Result<PreparedSceneFrame, SceneError> {
        self.prepare_camera_frame(requests, 0.0, Sampling::Fixed, &SlerpPlacementSampler)
    }

    fn validate_frame_sample(&self, alpha: f32, sampling: Sampling) -> Result<(), SceneError> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(SceneError::InvalidFraction);
        }
        if sampling == Sampling::BetweenTicks && !self.has_interpolable_interval() {
            return Err(SceneError::InvalidTick);
        }
        Ok(())
    }

    fn has_interpolable_interval(&self) -> bool {
        self.previous.tick.checked_add(1) == Some(self.current.tick)
            || self
                .current
                .objects
                .iter()
                .all(|object| object.discontinuity)
    }
}

/// Presentation owner supplies its orientation sampler (animation/camera have
/// their own basis contracts). It never receives mutable simulation state.
pub trait PlacementSampler {
    fn sample(&self, previous: Placement, current: Placement, alpha: f32) -> Placement;
}

/// Native rigid-transform presentation sampler. Position is linearly
/// interpolated and the local-to-world orientation basis is converted to a
/// quaternion, shortest-arc slerped, and converted back. The owner contract is
/// that orientation inputs are finite orthonormal rotation bases.
pub struct SlerpPlacementSampler;
impl PlacementSampler for SlerpPlacementSampler {
    fn sample(&self, previous: Placement, current: Placement, alpha: f32) -> Placement {
        if alpha == 0.0 {
            return previous;
        }
        if alpha == 1.0 {
            return current;
        }
        Placement {
            position: core::array::from_fn(|axis| {
                previous.position[axis] + (current.position[axis] - previous.position[axis]) * alpha
            }),
            orientation: slerp_basis(previous.orientation, current.orientation, alpha),
        }
    }
}

use crate::fmath::{atan2_f64, sin_f64, sqrt_f64};

fn basis_quaternion(basis: [[f32; 3]; 3]) -> [f32; 4] {
    let trace = basis[0][0] + basis[1][1] + basis[2][2];
    if trace > 0.0 {
        let scale = sqrt_f64(f64::from(trace + 1.0)) as f32 * 2.0;
        [
            (basis[1][2] - basis[2][1]) / scale,
            (basis[2][0] - basis[0][2]) / scale,
            (basis[0][1] - basis[1][0]) / scale,
            scale * 0.25,
        ]
    } else if basis[0][0] > basis[1][1] && basis[0][0] > basis[2][2] {
        let scale = sqrt_f64(f64::from(1.0 + basis[0][0] - basis[1][1] - basis[2][2])) as f32 * 2.0;
        [
            scale * 0.25,
            (basis[1][0] + basis[0][1]) / scale,
            (basis[2][0] + basis[0][2]) / scale,
            (basis[1][2] - basis[2][1]) / scale,
        ]
    } else if basis[1][1] > basis[2][2] {
        let scale = sqrt_f64(f64::from(1.0 + basis[1][1] - basis[0][0] - basis[2][2])) as f32 * 2.0;
        [
            (basis[1][0] + basis[0][1]) / scale,
            scale * 0.25,
            (basis[2][1] + basis[1][2]) / scale,
            (basis[2][0] - basis[0][2]) / scale,
        ]
    } else {
        let scale = sqrt_f64(f64::from(1.0 + basis[2][2] - basis[0][0] - basis[1][1])) as f32 * 2.0;
        [
            (basis[2][0] + basis[0][2]) / scale,
            (basis[2][1] + basis[1][2]) / scale,
            scale * 0.25,
            (basis[0][1] - basis[1][0]) / scale,
        ]
    }
}

fn quaternion_basis([x, y, z, w]: [f32; 4]) -> [[f32; 3]; 3] {
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
        ],
        [
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
        ],
        [
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

fn normalize_quaternion(q: [f32; 4]) -> [f32; 4] {
    let norm_squared = (q[0] * q[0] + q[1] * q[1]) + (q[2] * q[2] + q[3] * q[3]);
    let inverse_norm = (1.0 / sqrt_f64(f64::from(norm_squared))) as f32;
    q.map(|value| value * inverse_norm)
}

fn slerp_basis(previous: [[f32; 3]; 3], current: [[f32; 3]; 3], alpha: f32) -> [[f32; 3]; 3] {
    let a = normalize_quaternion(basis_quaternion(previous));
    let mut b = normalize_quaternion(basis_quaternion(current));
    let mut dot = ((a[0] * b[0] + a[1] * b[1]) + a[2] * b[2]) + a[3] * b[3];
    if dot < 0.0 {
        b = b.map(|value| -value);
        dot = -dot;
    }
    let dot = dot.min(1.0);
    let q = if dot > 0.9995 {
        normalize_quaternion(core::array::from_fn(|i| a[i] + (b[i] - a[i]) * alpha))
    } else {
        let sine = sqrt_f64(f64::from(1.0 - dot * dot)) as f32;
        let theta = atan2_f64(f64::from(sine), f64::from(dot)) as f32;
        let denominator = sin_f64(f64::from(theta)) as f32;
        let previous_weight = (sin_f64(f64::from((1.0 - alpha) * theta)) as f32) / denominator;
        let current_weight = (sin_f64(f64::from(alpha * theta)) as f32) / denominator;
        normalize_quaternion(core::array::from_fn(|i| {
            a[i] * previous_weight + b[i] * current_weight
        }))
    };
    quaternion_basis(q)
}
/// Pure typed native scene preparation. Query traversal, room membership,
/// per-plane culling and object submission operate on immutable snapshots.
/// The fixed preset reproduces current-tick input at alpha 0. Optional
/// interpolation uses both states, never writes or advances the fixed tick.
/// `index` leaves refer to stable entries in `current.objects`, not addresses.
pub fn prepare_view(
    previous: &SceneSnapshot,
    current: &SceneSnapshot,
    index: &SpatialTree,
    view: &View,
    alpha: f32,
    sampling: Sampling,
    sampler: &impl PlacementSampler,
) -> Result<PreparedView, SceneError> {
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(SceneError::InvalidFraction);
    }
    ScenePair::new(previous, current)?.prepare_view(index, view, alpha, sampling, sampler)
}

// This source is also included into the no_std `tsfp-ports` archive. Its
// integration fixtures rely on the engine-render crate's `tsfp_state` and
// `tsfp_ports` dependencies, so the owner crate opts in with a distinct feature.
#[cfg(all(test, feature = "render-prep-tests"))]
mod tests {
    use super::*;
    use alloc::vec;

    struct FixtureScene(Vec<SceneObjectAdmission>);
    impl SceneSnapshotSource for FixtureScene {
        type Error = ();
        fn tick(&self) -> u64 {
            12
        }
        fn collect_ordered(&self) -> Result<Vec<SceneObjectAdmission>, Self::Error> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn owned_scene_source_preserves_order_and_validates_transform_owner() {
        let id = ObjectId {
            slot: 5,
            generation: 2,
        };
        let bounds = Bounds {
            min: [-1.0; 3],
            max: [1.0; 3],
        };
        let admission = SceneObjectAdmission {
            snapshot: ObjectSnapshot {
                id,
                draw: SceneObject {
                    id,
                    model: ModelId(9),
                    visible: true,
                    scissor_rejected: false,
                    model_hidden: false,
                    deferred: false,
                    overlay: Overlay::None,
                    scissor: [0.0; 4],
                },
                placement: Placement {
                    position: [0.0; 3],
                    orientation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                },
                bounds,
                motion_bounds: bounds,
                rooms: vec![3],
                discontinuity: false,
            },
            transform_bindings: vec![TransformBinding {
                object: id,
                previous: id.into(),
                current: id.into(),
            }],
            view_transform_bindings: vec![],
        };
        let (snapshot, transforms, _) =
            collect_scene_snapshot(&FixtureScene(vec![admission])).unwrap();
        assert_eq!(snapshot.tick, 12);
        assert_eq!(snapshot.objects[0].id, id);
        assert_eq!(transforms[0].object, id);
    }

    #[test]
    fn scene_tick_history_reuses_pairs_reentrantly_and_snaps_gaps() {
        let id = ObjectId {
            slot: 7,
            generation: 3,
        };
        let bounds = Bounds {
            min: [-100.0; 3],
            max: [100.0; 3],
        };
        let snapshot = |tick, x| SceneSnapshot {
            tick,
            objects: vec![ObjectSnapshot {
                id,
                draw: SceneObject {
                    id,
                    model: ModelId(23),
                    visible: true,
                    scissor_rejected: false,
                    model_hidden: false,
                    deferred: false,
                    overlay: Overlay::None,
                    scissor: [0.0, 0.0, 640.0, 480.0],
                },
                placement: Placement {
                    position: [x, 0.0, 0.0],
                    orientation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                },
                bounds,
                motion_bounds: bounds,
                rooms: vec![],
                discontinuity: false,
            }],
        };
        let tree = SpatialTree {
            bounds,
            nodes: vec![
                super::super::spatial::Node::Leaf(vec![]),
                super::super::spatial::Node::Leaf(vec![0]),
            ],
            split_scale: 0.0,
        };
        let view = View {
            query: bounds,
            planes: vec![],
            visible_rooms: Default::default(),
        };
        let sample_x = |pair: &ScenePair<'_>, alpha| {
            pair.prepare_view(
                &tree,
                &view,
                alpha,
                Sampling::BetweenTicks,
                &SlerpPlacementSampler,
            )
            .unwrap()
            .objects[0]
                .placement
                .position[0]
        };

        let mut history = SceneTickHistory::default();
        assert_eq!(
            history.pair().err(),
            Some(SceneError::MissingSnapshotHistory)
        );
        history.publish(snapshot(20, 0.0)).unwrap();
        assert_eq!(history.object_count(), Some(1));
        let initial = history.pair().unwrap();
        assert_eq!(
            initial
                .prepare_view(&tree, &view, 0.0, Sampling::Fixed, &SlerpPlacementSampler)
                .unwrap()
                .objects[0]
                .placement
                .position[0],
            0.0
        );

        history.publish(snapshot(21, 10.0)).unwrap();
        let pair = history.pair().unwrap();
        assert_eq!(sample_x(&pair, 0.25), 2.5);
        assert_eq!(sample_x(&pair, 0.5), 5.0);
        assert_eq!(sample_x(&pair, 0.75), 7.5);
        assert_eq!(history.current_tick(), Some(21));

        // A refresh in the same simulation tick updates only the current
        // endpoint; the preceding tick remains the interpolation origin.
        history.publish(snapshot(21, 20.0)).unwrap();
        assert_eq!(sample_x(&history.pair().unwrap(), 0.5), 10.0);

        // A missed tick has no valid interpolation history and must snap.
        history.publish(snapshot(24, 50.0)).unwrap();
        let gap_pair = history.pair().unwrap();
        assert_eq!(gap_pair.previous.tick, 21);
        assert_eq!(gap_pair.current.tick, 24);
        assert_eq!(sample_x(&gap_pair, 0.25), 50.0);
        assert_eq!(sample_x(&gap_pair, 0.75), 50.0);
        assert_eq!(
            history.publish(snapshot(23, 40.0)),
            Err(SceneError::InvalidTick)
        );
        assert_eq!(history.current_tick(), Some(24));
    }

    #[test]
    fn snapshot_builder_preserves_owner_order_and_rejects_bad_rows_atomically() {
        let bounds = Bounds {
            min: [-1.0; 3],
            max: [1.0; 3],
        };
        let object = |id: ObjectId, model| ObjectSnapshot {
            id,
            draw: SceneObject {
                id,
                model: ModelId(model),
                visible: true,
                scissor_rejected: false,
                model_hidden: false,
                deferred: false,
                overlay: Overlay::None,
                scissor: [0.0, 0.0, 640.0, 480.0],
            },
            placement: Placement {
                position: [0.0; 3],
                orientation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            bounds,
            motion_bounds: bounds,
            rooms: vec![],
            discontinuity: false,
        };
        let first_id = ObjectId {
            slot: 4,
            generation: 2,
        };
        let second_id = ObjectId {
            slot: 9,
            generation: 5,
        };
        let mut builder = SceneSnapshotBuilder::new(31);
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        builder.push(object(first_id, 100)).unwrap();
        assert_eq!(builder.len(), 1);
        assert_eq!(
            builder.push(object(first_id, 101)),
            Err(SceneError::DuplicateObject)
        );
        let mut mismatched = object(second_id, 200);
        mismatched.draw.id = first_id;
        assert_eq!(
            builder.push(mismatched),
            Err(SceneError::ObjectIdentityMismatch)
        );
        builder.push(object(second_id, 200)).unwrap();

        let snapshot = builder.finish();
        assert_eq!(snapshot.tick, 31);
        assert_eq!(
            snapshot
                .objects
                .iter()
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            [first_id, second_id]
        );
        assert_eq!(snapshot.objects[0].draw.model, ModelId(100));
        assert_eq!(snapshot.objects[1].draw.model, ModelId(200));
    }

    #[test]
    fn published_transform_packets_feed_reentrant_per_view_scene_preparation() {
        use super::super::visibility::{ProjectionViewport, Rect};
        use crate::transform_history::{PublishedTransformEntry, PublishedTransformSet};
        use tsfp_ports::object_transform::{
            TransformHistoryRead, TransformHistoryRequest, TransformHistorySnapshot,
            TransformId as PublishedId, WorldTransform,
        };

        let object_id = ObjectId {
            slot: 3,
            generation: 1,
        };
        let transform_identity = TransformIdentity {
            namespace: TransformNamespace::Object,
            slot: 701,
            generation: 9002,
        };
        let make_read = |reference, view, previous_x, current_x| {
            let mut previous = WorldTransform::IDENTITY;
            previous.matrix[12] = previous_x;
            let mut current = WorldTransform::IDENTITY;
            current.matrix[12] = current_x;
            TransformHistoryRead {
                request: TransformHistoryRequest {
                    reference,
                    view,
                    translate: 1,
                },
                found: 1,
                history: TransformHistorySnapshot {
                    id: PublishedId {
                        slot: 7,
                        generation: 42,
                        view,
                    },
                    tick: 2,
                    previous,
                    current,
                    discontinuity: 0,
                },
            }
        };
        let entries = [
            PublishedTransformEntry::from_read(transform_identity, make_read(0x2000, 1, 0.0, 10.0))
                .unwrap()
                .unwrap(),
            PublishedTransformEntry::from_read(
                transform_identity,
                make_read(0x2000, 4, 20.0, 60.0),
            )
            .unwrap()
            .unwrap(),
        ];
        let missing = TransformHistoryRead {
            request: TransformHistoryRequest {
                reference: 0xdead,
                view: 1,
                translate: 1,
            },
            found: 0,
            history: TransformHistorySnapshot {
                id: PublishedId {
                    slot: 0,
                    generation: 0,
                    view: 0,
                },
                tick: 0,
                previous: WorldTransform::IDENTITY,
                current: WorldTransform::IDENTITY,
                discontinuity: 1,
            },
        };
        assert!(
            PublishedTransformEntry::from_read(transform_identity, missing)
                .unwrap()
                .is_none()
        );
        let source = PublishedTransformSet::new(2, 1, true, &entries).unwrap();
        let bounds = Bounds {
            min: [-1.0; 3],
            max: [1.0; 3],
        };
        let draw = SceneObject {
            id: object_id,
            model: ModelId(9),
            visible: true,
            scissor_rejected: false,
            model_hidden: false,
            deferred: false,
            overlay: Overlay::None,
            scissor: [0.0, 0.0, 10.0, 10.0],
        };
        let snapshot = |tick| SceneSnapshot {
            tick,
            objects: vec![ObjectSnapshot {
                id: object_id,
                draw: draw.clone(),
                placement: Placement {
                    position: [99.0, 0.0, 0.0],
                    orientation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                },
                bounds,
                motion_bounds: bounds,
                rooms: vec![],
                discontinuity: false,
            }],
        };
        let previous = snapshot(1);
        let current = snapshot(2);
        let pair = ScenePair::with_transform_history_and_view_bindings(
            &previous,
            &current,
            &source,
            &[TransformBinding {
                object: object_id,
                previous: transform_identity,
                current: transform_identity,
            }],
            &[ViewTransformBinding {
                object: object_id,
                viewer: 2,
                transform_view: 4,
                previous: transform_identity,
                current: transform_identity,
            }],
        )
        .unwrap();
        let view = View {
            query: Bounds {
                min: [-100.0; 3],
                max: [100.0; 3],
            },
            planes: vec![],
            visible_rooms: Default::default(),
        };
        let target = ViewTarget {
            viewer: 2,
            viewport: ProjectionViewport {
                x_offset: 0.0,
                x_scale: 10.0,
                y_offset: 0.0,
                y_scale: 10.0,
            },
            rectangle: Rect {
                left: 0,
                right: 10,
                top: 0,
                bottom: 10,
            },
        };
        let sampler = SlerpPlacementSampler;
        let view_two = pair
            .prepare_view_with_context(
                None,
                &view,
                None,
                None,
                Some(target),
                None,
                None,
                0.5,
                Sampling::BetweenTicks,
                &sampler,
            )
            .unwrap();
        let view_one = pair
            .prepare_view_with_context(
                None,
                &view,
                None,
                None,
                Some(ViewTarget {
                    viewer: 1,
                    ..target
                }),
                None,
                None,
                0.5,
                Sampling::BetweenTicks,
                &sampler,
            )
            .unwrap();
        assert_eq!(view_two.objects[0].placement.position[0], 40.0);
        assert_eq!(view_one.objects[0].placement.position[0], 5.0);
        assert_eq!(previous.objects[0].placement.position[0], 99.0);
        assert_eq!(current.objects[0].placement.position[0], 99.0);
    }

    struct ViewHistory<'a> {
        base: &'a tsfp_state::transforms::TransformHistory,
        samples: alloc::collections::BTreeMap<(TransformIdentity, u32), TransformSamples>,
    }
    impl TransformHistorySource for ViewHistory<'_> {
        fn tick(&self) -> Option<u64> {
            TransformHistorySource::tick(self.base)
        }

        fn transform_samples(&self, id: TransformIdentity) -> Option<TransformSamples> {
            TransformHistorySource::transform_samples(self.base, id)
        }

        fn transform_samples_for_view(
            &self,
            id: TransformIdentity,
            transform_view: u32,
        ) -> Option<TransformSamples> {
            self.samples
                .get(&(id, transform_view))
                .copied()
                .or_else(|| self.transform_samples(id))
        }
    }

    #[test]
    fn camera_visibility_uses_current_room_for_fixed_and_both_endpoints_for_interpolation() {
        use super::super::visibility::{Rect, RoomCoverage, VisibilityOutput};

        let empty = Rect {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
        };
        let visible = Rect {
            left: 0,
            right: 10,
            top: 0,
            bottom: 10,
        };
        let camera = crate::camera::CameraPresentation {
            eye: [1., 2., 3.],
            view: [0.; 16],
            projection: crate::camera::CameraProjection {
                matrix: [0.; 16],
                tangent_extents: [1., 1.],
            },
            visible_rooms: [2, 3],
        };
        let visibility = VisibilityOutput {
            rooms: vec![
                RoomCoverage {
                    depth: 0,
                    rect: visible,
                },
                RoomCoverage {
                    depth: 1,
                    rect: visible,
                },
                RoomCoverage {
                    depth: 2,
                    rect: empty,
                },
                RoomCoverage {
                    depth: -1,
                    rect: empty,
                },
            ],
            portals: vec![],
            portal_visits: vec![],
            max_depth: 2,
            active_rooms: vec![false; 4],
        };
        let query = Bounds {
            min: [-1.; 3],
            max: [1.; 3],
        };

        let fixed =
            View::from_camera_visibility(&camera, &visibility, query, vec![], Sampling::Fixed)
                .unwrap();
        let interpolated = View::from_camera_visibility(
            &camera,
            &visibility,
            query,
            vec![],
            Sampling::BetweenTicks,
        )
        .unwrap();

        assert_eq!(fixed.visible_rooms, [0, 1, 3].into_iter().collect());
        assert_eq!(
            interpolated.visible_rooms,
            [0, 1, 2, 3].into_iter().collect()
        );
        assert_eq!(fixed.query, query);
        assert_eq!(interpolated.query, query);
    }

    #[test]
    fn camera_visibility_rejects_room_ids_outside_scene_object_contract() {
        let camera = crate::camera::CameraPresentation {
            eye: [0.; 3],
            view: [0.; 16],
            projection: crate::camera::CameraProjection {
                matrix: [0.; 16],
                tangent_extents: [1.; 2],
            },
            visible_rooms: [-1, i32::from(u16::MAX) + 1],
        };
        let visibility = super::super::visibility::VisibilityOutput {
            rooms: vec![],
            portals: vec![],
            portal_visits: vec![],
            max_depth: -1,
            active_rooms: vec![],
        };
        assert_eq!(
            View::from_camera_visibility(
                &camera,
                &visibility,
                Bounds {
                    min: [0.; 3],
                    max: [1.; 3],
                },
                vec![],
                Sampling::Fixed,
            )
            .err(),
            Some(SceneError::RoomIdOutOfRange)
        );
    }

    #[test]
    fn camera_frame_prepares_all_views_from_endpoint_rooms() {
        use super::super::visibility::{
            Rect, Room, RoomCoverage, VisibilityOutput, VisibilityPresentation, VisibilityScene,
            VisibilityView,
        };
        use tsfp_state::transforms::{
            Discontinuity, TickTransform, TransformHistory, TransformId, TransformOwner,
            TransformSample, WorldTransform,
        };

        let bounds = Bounds {
            min: [-1.; 3],
            max: [1.; 3],
        };
        let object = |slot, room| {
            let id = ObjectId {
                slot,
                generation: 1,
            };
            let draw = SceneObject {
                id,
                model: ModelId(u64::from(slot)),
                visible: true,
                scissor_rejected: false,
                model_hidden: false,
                deferred: false,
                overlay: Overlay::None,
                scissor: [0., 0., 10., 10.],
            };
            ObjectSnapshot {
                id,
                draw,
                placement: Placement {
                    position: [0.; 3],
                    orientation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                },
                bounds,
                motion_bounds: bounds,
                rooms: vec![room],
                discontinuity: false,
            }
        };
        let mut unbounded = object(2, 2);
        unbounded.rooms.clear();
        let previous = SceneSnapshot {
            tick: 20,
            objects: vec![object(1, 1), unbounded],
        };
        let mut current = SceneSnapshot {
            tick: 21,
            objects: previous.objects.clone(),
        };
        current.objects[0].placement.position = [4., 0., 0.];
        current.objects[0].placement.orientation = rotation_z(core::f32::consts::FRAC_PI_2);
        current.objects[0].rooms = vec![3];
        current.objects[1].placement.position = [0.5, 0., 0.];
        current.objects[1].rooms = vec![2];
        current.objects.push(object(3, 2));
        let player_transform = TransformId {
            owner: TransformOwner::Player,
            slot: 7,
            generation: 1,
        };
        let scene_object_transform = TransformId {
            owner: TransformOwner::Object,
            slot: 1,
            generation: 1,
        };
        let object2_transform = TransformId {
            owner: TransformOwner::Object,
            slot: 2,
            generation: 1,
        };
        let object3_transform = TransformId {
            owner: TransformOwner::Object,
            slot: 3,
            generation: 1,
        };
        let identity = || [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
        let sample = |position, orientation| TransformSample {
            world: WorldTransform {
                position,
                orientation,
            },
            bones: None,
        };
        let mut history = TransformHistory::default();
        history.set_enabled(true);
        history
            .capture_tick(
                20,
                [
                    TickTransform {
                        id: player_transform,
                        sample: sample([0.; 3], identity()),
                        discontinuity: Discontinuity::default(),
                    },
                    TickTransform {
                        id: scene_object_transform,
                        sample: sample([6., 0., 0.], identity()),
                        discontinuity: Discontinuity::default(),
                    },
                    TickTransform {
                        id: object2_transform,
                        sample: sample([0.; 3], identity()),
                        discontinuity: Discontinuity::default(),
                    },
                ],
            )
            .unwrap();
        history
            .capture_tick(
                21,
                [
                    TickTransform {
                        id: player_transform,
                        sample: sample([4., 0., 0.], rotation_z(core::f32::consts::FRAC_PI_2)),
                        discontinuity: Discontinuity::default(),
                    },
                    TickTransform {
                        id: scene_object_transform,
                        sample: sample([12., 0., 0.], identity()),
                        discontinuity: Discontinuity::default(),
                    },
                    TickTransform {
                        id: object2_transform,
                        sample: sample([0.5, 0., 0.], identity()),
                        discontinuity: Discontinuity::default(),
                    },
                    TickTransform {
                        id: object3_transform,
                        sample: sample([0.; 3], identity()),
                        discontinuity: Discontinuity::default(),
                    },
                ],
            )
            .unwrap();
        let player_binding = TransformIdentity {
            namespace: TransformNamespace::Player,
            slot: player_transform.slot,
            generation: player_transform.generation,
        };
        let pair = ScenePair::with_transform_history_and_bindings(
            &previous,
            &current,
            &history,
            &[(current.objects[0].id, player_binding)],
        )
        .unwrap();
        let camera = crate::camera::CameraPresentation {
            eye: [0.; 3],
            view: [0.; 16],
            projection: crate::camera::CameraProjection {
                matrix: [0.; 16],
                tangent_extents: [1.; 2],
            },
            visible_rooms: [1, 2],
        };
        let initial_visibility = VisibilityOutput {
            rooms: vec![RoomCoverage {
                depth: -1,
                rect: Rect {
                    left: 0,
                    right: 0,
                    top: 0,
                    bottom: 0,
                },
            }],
            portals: vec![],
            portal_visits: vec![],
            max_depth: -1,
            active_rooms: vec![false],
        };
        let visibility_scene = VisibilityScene {
            rooms: vec![Room {
                portals: vec![],
                crossing_suppressed: false,
            }],
            portals: vec![],
        };
        let visibility_view = VisibilityView {
            position: [0.; 3],
            matrix: [0.; 16],
            viewport: super::super::visibility::ProjectionViewport {
                x_offset: 0.,
                x_scale: 10.,
                y_offset: 0.,
                y_scale: 10.,
            },
            full_rect: Rect {
                left: 0,
                right: 10,
                top: 0,
                bottom: 10,
            },
            viewer: 2,
            near_portal_distance: 0.,
            clip_groups: vec![],
            mapmaker: false,
        };
        let visibility = VisibilityPresentation::calculate(
            &visibility_scene,
            visibility_view.clone(),
            0,
            0,
            visibility_view.full_rect,
            &initial_visibility,
        )
        .unwrap();
        let target = ViewTarget {
            viewer: 2,
            viewport: visibility.view().viewport,
            rectangle: visibility.view().full_rect,
        };
        let room_order = super::super::scene_collect::collect_rooms(
            &[
                super::super::scene_collect::DrawRoom {
                    room: 1,
                    depth: 0,
                    after_ordinary: false,
                },
                super::super::scene_collect::DrawRoom {
                    room: 2,
                    depth: 1,
                    after_ordinary: false,
                },
                super::super::scene_collect::DrawRoom {
                    room: 3,
                    depth: 2,
                    after_ordinary: true,
                },
            ],
            2,
        )
        .unwrap();
        let request = [CameraViewRequest {
            camera: &camera,
            visibility: &visibility,
            scene_light_view: Some(
                crate::scene::light_state::SceneLightState::default()
                    .setup_view()
                    .unwrap(),
            ),
            room_order: &room_order,
            query: bounds,
            planes: vec![],
        }];

        let fixed = pair
            .prepare_camera_frame(&request, 0., Sampling::Fixed, &Sampler)
            .unwrap();
        assert_eq!(fixed.views[0].scene_light_view, request[0].scene_light_view);
        let original = pair.prepare_original_frame(&request).unwrap();
        let interpolated = pair
            .prepare_camera_frame(&request, 0.5, Sampling::BetweenTicks, &Sampler)
            .unwrap();

        assert_eq!(original.alpha, 0.0);
        assert_eq!(original.sampling, Sampling::Fixed);
        assert_eq!(
            original.views[0]
                .objects
                .iter()
                .find(|object| object.id.slot == 2)
                .unwrap()
                .placement
                .position,
            [0.5, 0., 0.]
        );
        assert_eq!(
            original.views[0].view.visible_rooms,
            [0, 2].into_iter().collect()
        );
        assert_eq!(original.views[0].room_order, Some(room_order.clone()));
        assert_eq!(
            fixed.views[0]
                .objects
                .iter()
                .map(|object| object.id.slot)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        assert_eq!(fixed.views[0].camera, Some(camera));
        assert_eq!(fixed.views[0].visibility, Some(visibility.clone()));
        assert_eq!(fixed.views[0].target, Some(target));
        assert_eq!(fixed.views[0].room_order, Some(room_order.clone()));
        assert_eq!(fixed.views[0].objects[0].rooms, [2]);
        assert_eq!(
            fixed.views[0].view.visible_rooms,
            [0, 2].into_iter().collect()
        );
        assert_eq!(
            interpolated.views[0]
                .objects
                .iter()
                .map(|object| object.id.slot)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(interpolated.views[0].camera, Some(camera));
        assert_eq!(interpolated.views[0].visibility, Some(visibility.clone()));
        assert_eq!(interpolated.views[0].target, Some(target));
        assert_eq!(interpolated.views[0].room_order, Some(room_order.clone()));
        assert_eq!(interpolated.views[0].objects[0].rooms, [3, 1]);
        assert!(interpolated.views[0].objects[1].rooms.is_empty());
        assert_eq!(
            interpolated.views[0].view.visible_rooms,
            [0, 1, 2].into_iter().collect()
        );

        let display = pair.prepare_display_frame(&request, 0.5).unwrap();
        assert_eq!(display.sampling, Sampling::BetweenTicks);
        assert_eq!(display.alpha, 0.5);
        let first_object = &display.views[0].objects[0];
        assert_eq!(first_object.placement.position, [2., 0., 0.]);
        let half = core::f32::consts::FRAC_1_SQRT_2;
        for (actual, expected) in [
            (first_object.placement.orientation[0][0], half),
            (first_object.placement.orientation[0][1], half),
            (first_object.placement.orientation[1][0], -half),
            (first_object.placement.orientation[1][1], half),
        ] {
            assert!((actual - expected).abs() < 1e-5);
        }
        let early = pair.prepare_display_frame(&request, 0.25).unwrap();
        let late = pair.prepare_display_frame(&request, 0.75).unwrap();
        assert_eq!(early.views[0].objects[0].placement.position, [1., 0., 0.]);
        assert_eq!(late.views[0].objects[0].placement.position, [3., 0., 0.]);
        for (alpha, frame) in [(0.25, &early), (0.75, &late)] {
            let angle = core::f32::consts::FRAC_PI_2 * alpha;
            let (sine, cosine) = angle.sin_cos();
            let orientation = frame.views[0].objects[0].placement.orientation;
            assert!((orientation[0][0] - cosine).abs() < 1e-5);
            assert!((orientation[0][1] - sine).abs() < 1e-5);
            assert!((orientation[1][0] + sine).abs() < 1e-5);
            assert!((orientation[1][1] - cosine).abs() < 1e-5);
        }
        let mut owner_change_current = current.clone();
        owner_change_current.objects[0].rooms = vec![1];
        let switched_owner_pair = ScenePair::with_transform_history_and_endpoint_bindings(
            &previous,
            &owner_change_current,
            &history,
            &[TransformBinding {
                object: owner_change_current.objects[0].id,
                previous: player_binding,
                current: owner_change_current.objects[0].id.into(),
            }],
        )
        .unwrap();
        for alpha in [0.25, 0.5, 0.75] {
            let frame = switched_owner_pair
                .prepare_display_frame(&request, alpha)
                .unwrap();
            assert_eq!(
                frame.views[0]
                    .objects
                    .iter()
                    .find(|object| object.id.slot == 1)
                    .unwrap()
                    .placement
                    .position,
                [12., 0., 0.]
            );
        }
        let object_identity: TransformIdentity = current.objects[0].id.into();
        let placement = |x| Placement {
            position: [x, 0., 0.],
            orientation: identity(),
        };
        let view_history = ViewHistory {
            base: &history,
            samples: [
                (
                    (object_identity, 1),
                    TransformSamples {
                        previous: placement(8.),
                        current: placement(16.),
                        discontinuity: false,
                    },
                ),
                (
                    (object_identity, 4),
                    TransformSamples {
                        previous: placement(30.),
                        current: placement(50.),
                        discontinuity: false,
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };
        let other_visibility = VisibilityPresentation::calculate(
            &visibility_scene,
            VisibilityView {
                viewer: 3,
                ..visibility_view.clone()
            },
            0,
            0,
            visibility.view().full_rect,
            &initial_visibility,
        )
        .unwrap();
        let split_camera_requests = [
            CameraViewRequest {
                camera: &camera,
                visibility: &visibility,
                scene_light_view: None,
                room_order: &room_order,
                query: bounds,
                planes: vec![],
            },
            CameraViewRequest {
                camera: &camera,
                visibility: &other_visibility,
                scene_light_view: None,
                room_order: &room_order,
                query: bounds,
                planes: vec![],
            },
        ];
        let split_pair = ScenePair::with_transform_history_and_view_bindings(
            &previous,
            &current,
            &view_history,
            &[TransformBinding {
                object: current.objects[0].id,
                previous: player_binding,
                current: player_binding,
            }],
            &[
                ViewTransformBinding {
                    object: current.objects[0].id,
                    viewer: 2,
                    transform_view: 1,
                    previous: object_identity,
                    current: object_identity,
                },
                ViewTransformBinding {
                    object: current.objects[0].id,
                    viewer: 3,
                    transform_view: 4,
                    previous: object_identity,
                    current: object_identity,
                },
            ],
        )
        .unwrap();
        let split_frame = split_pair
            .prepare_display_frame(&split_camera_requests, 0.5)
            .unwrap();
        for (view_index, expected_x) in [(0, 12.), (1, 40.)] {
            assert_eq!(
                split_frame.views[view_index]
                    .objects
                    .iter()
                    .find(|object| object.id.slot == 1)
                    .unwrap()
                    .placement
                    .position,
                [expected_x, 0., 0.]
            );
        }
        assert_eq!(previous.objects[0].placement.position, [0.; 3]);
        assert_eq!(current.objects[0].placement.position, [4., 0., 0.]);
        let mismatched_camera = crate::camera::CameraPresentation {
            eye: [1., 0., 0.],
            ..camera
        };
        let mismatched_request = [CameraViewRequest {
            camera: &mismatched_camera,
            visibility: &visibility,
            scene_light_view: None,
            room_order: &room_order,
            query: bounds,
            planes: vec![],
        }];
        assert_eq!(
            pair.prepare_display_frame(&mismatched_request, 0.5).err(),
            Some(SceneError::CameraEyeMismatch)
        );
        let mismatched_camera = crate::camera::CameraPresentation {
            view: [1.; 16],
            ..camera
        };
        let mismatched_request = [CameraViewRequest {
            camera: &mismatched_camera,
            visibility: &visibility,
            scene_light_view: None,
            room_order: &room_order,
            query: bounds,
            planes: vec![],
        }];
        assert_eq!(
            pair.prepare_display_frame(&mismatched_request, 0.5).err(),
            Some(SceneError::CameraMatrixMismatch)
        );

        assert_eq!(
            VisibilityPresentation::calculate(
                &visibility_scene,
                VisibilityView {
                    viewer: 16,
                    ..visibility.view().clone()
                },
                0,
                0,
                visibility.view().full_rect,
                &initial_visibility,
            )
            .err(),
            Some(super::super::visibility::VisibilityError::InvalidViewer)
        );
    }

    #[test]
    fn hidden_objects_do_not_emit_scissor_or_overlay() {
        let object = SceneObject {
            id: ObjectId {
                slot: 2,
                generation: 1,
            },
            model: ModelId(3),
            visible: false,
            scissor_rejected: false,
            model_hidden: false,
            deferred: true,
            overlay: Overlay::Player,
            scissor: [1.9, 2., 5., 6.],
        };
        assert!(prepare_object(&object).is_empty());
        let mut visible = object;
        visible.visible = true;
        assert_eq!(
            prepare_object(&visible),
            vec![
                ObjectCommand::Scissor([1, 2, 5, 6]),
                ObjectCommand::PlayerOverlay(ObjectId {
                    slot: 2,
                    generation: 1
                }),
                ObjectCommand::Defer(ObjectId {
                    slot: 2,
                    generation: 1
                })
            ]
        );
        assert!(!object.visible);
    }
    #[test]
    fn model_preorder_masks_type_three_and_cycle_guard() {
        let nodes = vec![
            ModelNode {
                mask: 0,
                child: Some(1),
                sibling: Some(3),
                stop_children: false,
            },
            ModelNode {
                mask: 4,
                child: Some(2),
                sibling: None,
                stop_children: true,
            },
            ModelNode {
                mask: 0,
                child: None,
                sibling: None,
                stop_children: false,
            },
            ModelNode {
                mask: 8,
                child: None,
                sibling: None,
                stop_children: false,
            },
        ];
        assert_eq!(model_nodes(&nodes, 0, 4).unwrap(), [0, 1]);
        assert_eq!(model_nodes(&nodes, 0, 8).unwrap(), [0, 3]);
        let mut broken = nodes;
        broken[3].sibling = Some(0);
        assert_eq!(model_nodes(&broken, 0, 8), Err(ModelError::Cycle));
    }
    struct Sampler;
    impl PlacementSampler for Sampler {
        fn sample(&self, previous: Placement, current: Placement, alpha: f32) -> Placement {
            Placement {
                position: core::array::from_fn(|i| {
                    previous.position[i] + (current.position[i] - previous.position[i]) * alpha
                }),
                orientation: current.orientation,
            }
        }
    }
    fn rotation_z(radians: f32) -> [[f32; 3]; 3] {
        let (sine, cosine) = radians.sin_cos();
        [[cosine, sine, 0.], [-sine, cosine, 0.], [0., 0., 1.]]
    }

    #[test]
    fn rigid_placement_sampler_lerps_position_and_slerps_orientation() {
        let previous = Placement {
            position: [0., 2., -4.],
            orientation: rotation_z(0.),
        };
        let current = Placement {
            position: [4., 6., 2.],
            orientation: rotation_z(core::f32::consts::FRAC_PI_2),
        };
        let sampler = SlerpPlacementSampler;
        assert_eq!(sampler.sample(previous, current, 0.), previous);
        assert_eq!(sampler.sample(previous, current, 1.), current);

        let midpoint = sampler.sample(previous, current, 0.5);
        let half = core::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(midpoint.position, [2., 4., -1.]);
        for (actual, expected) in [
            (midpoint.orientation[0][0], half),
            (midpoint.orientation[0][1], half),
            (midpoint.orientation[1][0], -half),
            (midpoint.orientation[1][1], half),
            (midpoint.orientation[2][2], 1.),
        ] {
            assert!((actual - expected).abs() < 1e-5);
        }
    }

    #[test]
    fn rigid_placement_sampler_uses_shortest_rotation_arc() {
        let previous = Placement {
            position: [0.; 3],
            orientation: rotation_z(170f32.to_radians()),
        };
        let current = Placement {
            position: [0.; 3],
            orientation: rotation_z((-170f32).to_radians()),
        };
        let midpoint = SlerpPlacementSampler.sample(previous, current, 0.5);
        assert!((midpoint.orientation[0][0] + 1.0).abs() < 1e-5);
        assert!(midpoint.orientation[0][1].abs() < 1e-5);
        assert!(midpoint.orientation[1][0].abs() < 1e-5);
        assert!((midpoint.orientation[1][1] + 1.0).abs() < 1e-5);
    }

    #[test]
    fn views_are_reentrant_fixed_is_current_and_discontinuities_snap() {
        let bounds = Bounds {
            min: [0.; 3],
            max: [2.; 3],
        };
        let placement = Placement {
            position: [0.; 3],
            orientation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        };
        let draw = SceneObject {
            id: ObjectId {
                slot: 7,
                generation: 1,
            },
            model: ModelId(2),
            visible: true,
            scissor_rejected: false,
            model_hidden: false,
            deferred: false,
            overlay: Overlay::None,
            scissor: [0., 0., 640., 480.],
        };
        let object = ObjectSnapshot {
            id: ObjectId {
                slot: 7,
                generation: 1,
            },
            draw,
            placement,
            bounds,
            motion_bounds: bounds,
            rooms: vec![2],
            discontinuity: false,
        };
        let previous = SceneSnapshot {
            tick: 1,
            objects: vec![object],
        };
        let mut current = previous.clone();
        current.tick = 2;
        current.objects[0].placement.position = [2.; 3];
        let mut mismatched_identity = current.clone();
        mismatched_identity.objects[0].draw.id = ObjectId {
            slot: 8,
            generation: 1,
        };
        assert_eq!(
            ScenePair::new(&previous, &mismatched_identity).err(),
            Some(SceneError::ObjectIdentityMismatch)
        );
        use tsfp_state::transforms::{
            Discontinuity, TickTransform, TransformHistory, TransformId, TransformOwner,
            TransformSample, WorldTransform,
        };
        let transform_id = TransformId {
            owner: TransformOwner::Object,
            slot: 7,
            generation: 1,
        };
        let transform = |x| TransformSample {
            world: WorldTransform {
                position: [x; 3],
                orientation: placement.orientation,
            },
            bones: None,
        };
        let mut history = TransformHistory::default();
        history.set_enabled(true);
        history
            .capture_tick(
                1,
                [TickTransform {
                    id: transform_id,
                    sample: transform(0.),
                    discontinuity: Discontinuity::default(),
                }],
            )
            .unwrap();
        history
            .capture_tick(
                2,
                [TickTransform {
                    id: transform_id,
                    sample: transform(2.),
                    discontinuity: Discontinuity::default(),
                }],
            )
            .unwrap();
        let mut cut_history = TransformHistory::default();
        cut_history.set_enabled(true);
        cut_history
            .capture_tick(
                1,
                [TickTransform {
                    id: transform_id,
                    sample: transform(0.),
                    discontinuity: Discontinuity::default(),
                }],
            )
            .unwrap();
        cut_history
            .capture_tick(
                2,
                [TickTransform {
                    id: transform_id,
                    sample: transform(2.),
                    discontinuity: Discontinuity {
                        teleported: true,
                        ..Discontinuity::default()
                    },
                }],
            )
            .unwrap();
        let mut missing = TransformHistory::default();
        missing.set_enabled(true);
        missing.capture_tick(2, []).unwrap();
        let tree = SpatialTree {
            bounds,
            nodes: vec![
                super::super::spatial::Node::Leaf(vec![]),
                super::super::spatial::Node::Leaf(vec![0]),
            ],
            split_scale: 0.,
        };
        let view = View {
            query: bounds,
            planes: vec![],
            visible_rooms: [2].into_iter().collect(),
        };
        let transform_pair = ScenePair::with_transform_history(&previous, &current, &history)
            .expect("matching transform history");
        assert_eq!(
            transform_pair
                .prepare_view(&tree, &view, 0.5, Sampling::BetweenTicks, &Sampler)
                .unwrap()
                .objects[0]
                .placement
                .position,
            [1.; 3]
        );
        assert_eq!(
            transform_pair
                .prepare_view(&tree, &view, 0., Sampling::Fixed, &Sampler)
                .unwrap()
                .objects[0]
                .placement
                .position,
            [2.; 3]
        );
        let player_transform_id = TransformId {
            owner: TransformOwner::Player,
            slot: 77,
            generation: 4,
        };
        let mut player_history = TransformHistory::default();
        player_history.set_enabled(true);
        player_history
            .capture_tick(
                1,
                [TickTransform {
                    id: player_transform_id,
                    sample: transform(10.),
                    discontinuity: Discontinuity::default(),
                }],
            )
            .unwrap();
        player_history
            .capture_tick(
                2,
                [TickTransform {
                    id: player_transform_id,
                    sample: transform(12.),
                    discontinuity: Discontinuity::default(),
                }],
            )
            .unwrap();
        let player_binding = TransformIdentity {
            namespace: TransformNamespace::Player,
            slot: 77,
            generation: 4,
        };
        let player_pair = ScenePair::with_transform_history_and_bindings(
            &previous,
            &current,
            &player_history,
            &[(previous.objects[0].id, player_binding)],
        )
        .expect("player-owned transform is bound to the scene draw");
        assert_eq!(
            player_pair
                .prepare_view(&tree, &view, 0.5, Sampling::BetweenTicks, &Sampler)
                .unwrap()
                .objects[0]
                .placement
                .position,
            [11.; 3]
        );
        assert_eq!(
            player_pair
                .prepare_view(&tree, &view, 0., Sampling::Fixed, &Sampler)
                .unwrap()
                .objects[0]
                .placement
                .position,
            [12.; 3]
        );
        let object_binding: TransformIdentity = current.objects[0].id.into();
        let switched_owner_pair = ScenePair::with_transform_history_and_endpoint_bindings(
            &previous,
            &current,
            &history,
            &[TransformBinding {
                object: current.objects[0].id,
                previous: player_binding,
                current: object_binding,
            }],
        )
        .expect("new owner has current transform history");
        assert_eq!(
            switched_owner_pair
                .prepare_view(&tree, &view, 0.5, Sampling::BetweenTicks, &Sampler)
                .unwrap()
                .objects[0]
                .placement
                .position,
            [2.; 3]
        );
        assert_eq!(
            ScenePair::with_transform_history_and_bindings(
                &previous,
                &current,
                &player_history,
                &[(
                    ObjectId {
                        slot: 99,
                        generation: 4
                    },
                    player_binding
                )],
            )
            .err(),
            Some(SceneError::UnknownTransformBinding(ObjectId {
                slot: 99,
                generation: 4,
            }))
        );
        assert_eq!(
            ScenePair::with_transform_history_and_bindings(
                &previous,
                &current,
                &player_history,
                &[
                    (previous.objects[0].id, player_binding),
                    (previous.objects[0].id, player_binding),
                ],
            )
            .err(),
            Some(SceneError::DuplicateTransformBinding(
                previous.objects[0].id
            ))
        );
        let cut_pair = ScenePair::with_transform_history(&previous, &current, &cut_history)
            .expect("matching cut history");
        assert_eq!(
            cut_pair
                .prepare_view(&tree, &view, 0.5, Sampling::BetweenTicks, &Sampler)
                .unwrap()
                .objects[0]
                .placement
                .position,
            [2.; 3]
        );
        assert!(matches!(
            ScenePair::with_transform_history(&previous, &current, &missing),
            Err(SceneError::MissingTransform(_))
        ));
        let mut mismatched_tick = TransformHistory::default();
        mismatched_tick.set_enabled(true);
        mismatched_tick.capture_tick(1, []).unwrap();
        assert_eq!(
            ScenePair::with_transform_history(&previous, &current, &mismatched_tick).err(),
            Some(SceneError::HistoryTickMismatch)
        );
        let retired_id = ObjectId {
            slot: 7,
            generation: 0,
        };
        let active_id = ObjectId {
            slot: 7,
            generation: 1,
        };
        let mut retired_snapshot = previous.clone();
        retired_snapshot.objects[0].id = retired_id;
        retired_snapshot.objects[0].draw.id = retired_id;
        let mut active_snapshot = current.clone();
        active_snapshot.objects[0].id = active_id;
        active_snapshot.objects[0].draw.id = active_id;
        active_snapshot.objects[0].placement.position = [20.; 3];
        let mut recycled_history = TransformHistory::default();
        recycled_history.set_enabled(true);
        recycled_history
            .capture_tick(
                1,
                [TickTransform {
                    id: TransformId {
                        owner: TransformOwner::Object,
                        slot: 7,
                        generation: 0,
                    },
                    sample: transform(0.),
                    discontinuity: Discontinuity::default(),
                }],
            )
            .unwrap();
        recycled_history
            .capture_tick(
                2,
                [TickTransform {
                    id: transform_id,
                    sample: transform(20.),
                    discontinuity: Discontinuity::default(),
                }],
            )
            .unwrap();
        let recycled_pair = ScenePair::with_transform_history(
            &retired_snapshot,
            &active_snapshot,
            &recycled_history,
        )
        .expect("reused slot has a distinct current lifetime");
        assert_eq!(
            recycled_pair
                .prepare_view(&tree, &view, 0.5, Sampling::BetweenTicks, &Sampler)
                .unwrap()
                .objects[0]
                .placement
                .position,
            [20.; 3]
        );
        let original = current.clone();
        for _ in 0..4 {
            let frame = prepare_view(
                &previous,
                &current,
                &tree,
                &view,
                0.,
                Sampling::Fixed,
                &Sampler,
            )
            .unwrap();
            assert_eq!(frame.objects[0].placement.position, [2.; 3]);
            let frame = prepare_view(
                &previous,
                &current,
                &tree,
                &view,
                0.5,
                Sampling::BetweenTicks,
                &Sampler,
            )
            .unwrap();
            assert_eq!(frame.objects[0].placement.position, [1.; 3]);
        }
        let pair = ScenePair::new(&previous, &current).unwrap();
        let same_tick = SceneSnapshot {
            tick: current.tick,
            objects: current.objects.clone(),
        };
        let fixed_pair = ScenePair::new(&same_tick, &current).unwrap();
        assert_eq!(
            fixed_pair
                .prepare_view(&tree, &view, 0.5, Sampling::Fixed, &Sampler)
                .unwrap()
                .objects[0]
                .placement
                .position,
            [2.; 3]
        );
        assert_eq!(
            fixed_pair
                .prepare_view(&tree, &view, 0.5, Sampling::BetweenTicks, &Sampler)
                .err(),
            Some(SceneError::InvalidTick)
        );
        let quarter = pair
            .prepare_view(&tree, &view, 0.25, Sampling::BetweenTicks, &Sampler)
            .unwrap();
        let mut other_view = view.clone();
        other_view.visible_rooms.clear();
        let hidden = pair
            .prepare_view(&tree, &other_view, 0.5, Sampling::BetweenTicks, &Sampler)
            .unwrap();
        let three_quarters = pair
            .prepare_view(&tree, &view, 0.75, Sampling::BetweenTicks, &Sampler)
            .unwrap();
        assert_eq!(quarter.objects[0].placement.position, [0.5; 3]);
        assert!(hidden.objects.is_empty());
        assert_eq!(three_quarters.objects[0].placement.position, [1.5; 3]);
        assert_eq!(current, original);
        current.objects[0].discontinuity = true;
        assert_eq!(
            prepare_view(
                &previous,
                &current,
                &tree,
                &view,
                0.5,
                Sampling::BetweenTicks,
                &Sampler
            )
            .unwrap()
            .objects[0]
                .placement
                .position,
            [2.; 3]
        );
        let mut hidden_room = view.clone();
        hidden_room.visible_rooms.clear();
        assert!(
            prepare_view(
                &previous,
                &current,
                &tree,
                &hidden_room,
                0.,
                Sampling::Fixed,
                &Sampler
            )
            .unwrap()
            .objects
            .is_empty()
        );

        // An object crossing from a visible room into a hidden room remains
        // present between ticks. The fixed preset still uses current room IDs.
        current.objects[0].rooms = vec![3];
        let mut crossing_view = view.clone();
        crossing_view.visible_rooms = [2].into_iter().collect();
        assert!(
            prepare_view(
                &previous,
                &current,
                &tree,
                &crossing_view,
                0.5,
                Sampling::Fixed,
                &Sampler
            )
            .unwrap()
            .objects
            .is_empty()
        );
        current.objects[0].discontinuity = false;
        assert_eq!(
            prepare_view(
                &previous,
                &current,
                &tree,
                &crossing_view,
                0.5,
                Sampling::BetweenTicks,
                &Sampler
            )
            .unwrap()
            .objects
            .len(),
            1
        );
        current.objects[0].discontinuity = true;
        assert!(
            prepare_view(
                &previous,
                &current,
                &tree,
                &crossing_view,
                0.5,
                Sampling::BetweenTicks,
                &Sampler
            )
            .unwrap()
            .objects
            .is_empty()
        );

        let mut outside = view;
        outside.planes.push(Plane {
            normal: [-1., 0., 0.],
            offset: -1.,
        });
        assert!(
            prepare_view(
                &previous,
                &current,
                &tree,
                &outside,
                0.,
                Sampling::Fixed,
                &Sampler
            )
            .unwrap()
            .objects
            .is_empty()
        );
    }

    #[test]
    fn frame_aggregator_samples_all_views_from_the_same_pair_and_alpha() {
        let bounds = Bounds {
            min: [0.; 3],
            max: [2.; 3],
        };
        let placement = |x| Placement {
            position: [x, 0., 0.],
            orientation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        };
        let draw = SceneObject {
            id: ObjectId {
                slot: 1,
                generation: 1,
            },
            model: ModelId(3),
            visible: true,
            scissor_rejected: false,
            model_hidden: false,
            deferred: false,
            overlay: Overlay::None,
            scissor: [0., 0., 640., 480.],
        };
        let object = |x| ObjectSnapshot {
            id: draw.id,
            draw,
            placement: placement(x),
            bounds,
            motion_bounds: bounds,
            rooms: vec![2],
            discontinuity: false,
        };
        let previous = SceneSnapshot {
            tick: 10,
            objects: vec![object(1.)],
        };
        let current = SceneSnapshot {
            tick: 11,
            objects: vec![object(5.)],
        };
        let pair = ScenePair::new(&previous, &current).unwrap();
        let tree = SpatialTree {
            bounds,
            nodes: vec![
                super::super::spatial::Node::Leaf(vec![]),
                super::super::spatial::Node::Leaf(vec![0]),
            ],
            split_scale: 0.,
        };
        let left_view = View {
            query: bounds,
            planes: vec![],
            visible_rooms: [2].into_iter().collect(),
        };
        let mut right_view = left_view.clone();
        right_view.visible_rooms.clear();
        let requests = [
            ViewRequest {
                index: &tree,
                view: &left_view,
            },
            ViewRequest {
                index: &tree,
                view: &right_view,
            },
        ];

        let first = pair
            .prepare_frame(&requests, 0.25, Sampling::BetweenTicks, &Sampler)
            .unwrap();
        let second = pair
            .prepare_frame(&requests, 0.75, Sampling::BetweenTicks, &Sampler)
            .unwrap();

        assert_eq!(first.tick, 11);
        assert_eq!(first.alpha, 0.25);
        assert_eq!(first.sampling, Sampling::BetweenTicks);
        assert_eq!(first.views.len(), 2);
        assert_eq!(first.views[0].objects[0].placement.position[0], 2.0);
        assert!(first.views[1].objects.is_empty());
        assert_eq!(second.views[0].objects[0].placement.position[0], 4.0);
        assert!(second.views[1].objects.is_empty());
        assert_eq!(previous.objects[0].placement.position[0], 1.0);
        assert_eq!(current.objects[0].placement.position[0], 5.0);
    }

    #[test]
    fn frame_aggregator_rejects_a_partial_frame() {
        let bounds = Bounds {
            min: [0.; 3],
            max: [2.; 3],
        };
        let placement = Placement {
            position: [1., 0., 0.],
            orientation: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        };
        let draw = SceneObject {
            id: ObjectId {
                slot: 1,
                generation: 1,
            },
            model: ModelId(3),
            visible: true,
            scissor_rejected: false,
            model_hidden: false,
            deferred: false,
            overlay: Overlay::None,
            scissor: [0., 0., 640., 480.],
        };
        let object = ObjectSnapshot {
            id: draw.id,
            draw,
            placement,
            bounds,
            motion_bounds: bounds,
            rooms: vec![2],
            discontinuity: false,
        };
        let previous = SceneSnapshot {
            tick: 10,
            objects: vec![object.clone()],
        };
        let current = SceneSnapshot {
            tick: 11,
            objects: vec![object],
        };
        let pair = ScenePair::new(&previous, &current).unwrap();
        let valid_index = SpatialTree {
            bounds,
            nodes: vec![
                super::super::spatial::Node::Leaf(vec![]),
                super::super::spatial::Node::Leaf(vec![0]),
            ],
            split_scale: 0.,
        };
        let invalid_index = SpatialTree {
            bounds,
            nodes: vec![
                super::super::spatial::Node::Leaf(vec![]),
                super::super::spatial::Node::Leaf(vec![1]),
            ],
            split_scale: 0.,
        };
        let valid_view = View {
            query: bounds,
            planes: vec![],
            visible_rooms: [2].into_iter().collect(),
        };
        let invalid_view = valid_view.clone();
        let requests = [
            ViewRequest {
                index: &valid_index,
                view: &valid_view,
            },
            ViewRequest {
                index: &invalid_index,
                view: &invalid_view,
            },
        ];

        assert_eq!(
            pair.prepare_frame(&requests, 0.5, Sampling::BetweenTicks, &Sampler)
                .err(),
            Some(SceneError::InvalidTree)
        );
    }
}
