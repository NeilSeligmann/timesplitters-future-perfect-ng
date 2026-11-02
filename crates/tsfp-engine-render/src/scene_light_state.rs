// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned light publication state from the bounded T1700 REA contract.
//!
//! This is an opt-in data owner. It does not replace or feed the current
//! retained scene producer. The caller supplies the sampled camera transform
//! for each view, so view-space values stay tied to that view and epoch.
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};

/// A render-record identity, distinct from an object identity or guest address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RenderRecordId(u64);

/// A camera/view identity, distinct from its render record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewId(u64);

/// Explicit identity pair used to select one published render-record light
/// block for one prepared view and to bind that pair to a native draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SceneLightBinding {
    record: RenderRecordId,
    view: ViewId,
}

impl SceneLightBinding {
    pub const fn new(record: RenderRecordId, view: ViewId) -> Self {
        Self { record, view }
    }

    pub const fn record(self) -> RenderRecordId {
        self.record
    }

    pub const fn view(self) -> ViewId {
        self.view
    }
}

/// Resource/frame generation for a prepared view block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameResourceEpoch(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color3(pub [f32; 3]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPoint(pub [f32; 3]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPoint(pub [f32; 4]);

/// Transform a world point with the sampled camera's column-major view matrix.
/// The evaluation order matches the retained 0x151FC0 point-to-vec4 helper;
/// NativeSceneOwner supplies the matrix from the selected PreparedView.
pub fn transform_world_point_to_view(matrix: [f32; 16], point: WorldPoint) -> ViewPoint {
    let [x, y, z] = point.0;
    let component = |axis: usize| {
        ((matrix[axis + 8] * z + matrix[axis + 4] * y) + matrix[axis] * x) + matrix[axis + 12]
    };
    ViewPoint(core::array::from_fn(component))
}

/// Four owned color vectors and three owned world-space points from the REA
/// producer contract. The fourth color is kept separate from the three slots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneLightInputs {
    pub colors: [Color3; 4],
    pub world_points: [WorldPoint; 3],
}

/// One owned source replacement in an atomic native scene-light update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneLightRecordUpdate {
    record: RenderRecordId,
    inputs: SceneLightInputs,
}

impl SceneLightRecordUpdate {
    pub const fn new(record: RenderRecordId, inputs: SceneLightInputs) -> Self {
        Self { record, inputs }
    }
}

/// One packed 0x28-byte light slot, represented without guest memory offsets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewLightSlot {
    pub position: ViewPoint,
    pub color: Color3,
    /// Whether the original producer's color-length branch enabled this slot.
    /// This typed flag is derived from the color predicate; it is not an
    /// additional value read from the original 0x28-byte payload.
    pub enabled: bool,
    /// The final three words in the original slot; their semantic names are
    /// not established by the REA analysis.
    pub parameters: [f32; 3],
}

/// Three view-space light slots plus the producer's separate fourth color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewLightBlock {
    pub record: RenderRecordId,
    pub view: ViewId,
    pub epoch: FrameResourceEpoch,
    pub slots: [ViewLightSlot; 3],
    pub fourth_color: Color3,
}

/// Immutable owned light data selected for one renderer submission epoch.
/// Blocks stay in caller-provided order, which lets scene preparation preserve
/// its established record/view traversal without exposing guest addresses.
#[derive(Clone, Debug, PartialEq)]
pub struct PublishedSceneLights {
    pub epoch: FrameResourceEpoch,
    pub blocks: Vec<ViewLightBlock>,
}

impl PublishedSceneLights {
    /// Resolve one renderer attachment by its typed record and view identity.
    /// Reject an ambiguous or stale block rather than choosing by vector order.
    pub fn block_for(&self, record: RenderRecordId, view: ViewId) -> Option<&ViewLightBlock> {
        let mut matches = self
            .blocks
            .iter()
            .filter(|block| block.record == record && block.view == view);
        let block = matches.next()?;
        if block.epoch != self.epoch || matches.next().is_some() {
            return None;
        }
        Some(block)
    }

    /// Resolve a draw's named record/view association.
    pub fn block_for_binding(&self, binding: SceneLightBinding) -> Option<&ViewLightBlock> {
        self.block_for(binding.record(), binding.view())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneLightError {
    IdentityExhausted,
    InvalidRenderRecord,
    InvalidView,
    MissingPreparedViewLight,
    MissingFrameResourceEpoch,
    MissingPreparedCamera,
    FrameEpochMismatch,
    DuplicatePublicationEntry,
    DuplicateRecordUpdate,
    PublicationDisabled,
    NoPublishedFrame,
    ViewNotInPreparedFrame,
    InvalidDrawBinding,
}

#[derive(Clone)]
struct RecordState {
    inputs: SceneLightInputs,
    view_blocks: BTreeMap<ViewId, ViewLightBlock>,
}

/// Owns scene-light setup, per-view publication and teardown. IDs are
/// monotonically issued and validated against live entries, so retirement
/// cannot make an old record or view valid again.
#[derive(Clone, Default)]
pub struct SceneLightState {
    next_record: u64,
    next_view: u64,
    records: BTreeMap<RenderRecordId, RecordState>,
    views: BTreeSet<ViewId>,
    publication_enabled: bool,
    published: Option<PublishedSceneLights>,
}

impl SceneLightState {
    /// Check a live record/view pair without requiring a prepared block.
    /// Producers use this before building a block for a view's first frame.
    pub(crate) fn validate_record_view(
        &self,
        record: RenderRecordId,
        view: ViewId,
    ) -> Result<(), SceneLightError> {
        if !self.records.contains_key(&record) {
            return Err(SceneLightError::InvalidRenderRecord);
        }
        if !self.views.contains(&view) {
            return Err(SceneLightError::InvalidView);
        }
        Ok(())
    }

    /// Begin one render-record light lifetime with owned source values.
    pub fn setup_record(
        &mut self,
        inputs: SceneLightInputs,
    ) -> Result<RenderRecordId, SceneLightError> {
        let id = RenderRecordId(next_id(&mut self.next_record)?);
        self.published = None;
        self.records.insert(
            id,
            RecordState {
                inputs,
                view_blocks: BTreeMap::new(),
            },
        );
        Ok(id)
    }

    /// Replace source values for a live record and discard blocks made from
    /// the prior values. New view blocks require an explicit publication.
    pub fn update_record(
        &mut self,
        record: RenderRecordId,
        inputs: SceneLightInputs,
    ) -> Result<(), SceneLightError> {
        self.update_records(&[SceneLightRecordUpdate::new(record, inputs)])
    }

    /// Replace multiple live records as one source update. Duplicate or stale
    /// identities reject the whole batch without disturbing prepared blocks or
    /// the last complete publication.
    pub fn update_records(
        &mut self,
        updates: &[SceneLightRecordUpdate],
    ) -> Result<(), SceneLightError> {
        let mut seen = BTreeSet::new();
        for update in updates {
            if !seen.insert(update.record) {
                return Err(SceneLightError::DuplicateRecordUpdate);
            }
            if !self.records.contains_key(&update.record) {
                return Err(SceneLightError::InvalidRenderRecord);
            }
        }
        if updates.is_empty() {
            return Ok(());
        }

        self.published = None;
        for update in updates {
            let state = self
                .records
                .get_mut(&update.record)
                .expect("records checked before batch mutation");
            state.inputs = update.inputs;
            state.view_blocks.clear();
        }
        Ok(())
    }

    /// End a render-record lifetime and release its inputs and view blocks.
    pub fn teardown_record(&mut self, record: RenderRecordId) -> Result<(), SceneLightError> {
        if self.records.remove(&record).is_none() {
            return Err(SceneLightError::InvalidRenderRecord);
        }
        self.published = None;
        Ok(())
    }

    /// Start a view identity used to qualify sampled camera output.
    pub fn setup_view(&mut self) -> Result<ViewId, SceneLightError> {
        let id = ViewId(next_id(&mut self.next_view)?);
        self.published = None;
        self.views.insert(id);
        Ok(id)
    }

    /// End a view lifetime and drop its published blocks from every record.
    pub fn teardown_view(&mut self, view: ViewId) -> Result<(), SceneLightError> {
        if !self.views.remove(&view) {
            return Err(SceneLightError::InvalidView);
        }
        self.published = None;
        for record in self.records.values_mut() {
            record.view_blocks.remove(&view);
        }
        Ok(())
    }

    /// Build and own a per-view block from this record's colors and world
    /// points. `transform` is the matching sampled camera's point transform;
    /// it is called only for enabled slots, as in the original helper path.
    pub fn prepare_view(
        &mut self,
        record: RenderRecordId,
        view: ViewId,
        epoch: FrameResourceEpoch,
        mut transform: impl FnMut(WorldPoint) -> ViewPoint,
    ) -> Result<&ViewLightBlock, SceneLightError> {
        if !self.views.contains(&view) {
            return Err(SceneLightError::InvalidView);
        }
        if !self.records.contains_key(&record) {
            return Err(SceneLightError::InvalidRenderRecord);
        }
        self.published = None;
        let state = self.records.get_mut(&record).expect("record checked above");
        let inputs = state.inputs;
        let slots = core::array::from_fn(|index| {
            let color = inputs.colors[index];
            let [x, y, z] = color.0;
            // Preserve the original scalar SSE grouping and ordered-positive
            // test. Zero, negative/unordered results and NaN disable the slot.
            let enabled = (x * x + y * y) + z * z > 0.0;
            if enabled {
                ViewLightSlot {
                    position: transform(inputs.world_points[index]),
                    color,
                    enabled: true,
                    parameters: [0.5, 0.0, 0.0],
                }
            } else {
                ViewLightSlot {
                    position: ViewPoint([0.0, 0.0, 0.0, 0.5]),
                    color: Color3([0.0, 0.0, 0.0]),
                    enabled: false,
                    parameters: [1.0, 0.0, 1.0],
                }
            }
        });
        let block = ViewLightBlock {
            record,
            view,
            epoch,
            slots,
            fourth_color: inputs.colors[3],
        };
        state.view_blocks.insert(view, block);
        Ok(state.view_blocks.get(&view).expect("block inserted"))
    }

    /// Read a block only while both its render record and view are live.
    pub fn view_block(
        &self,
        record: RenderRecordId,
        view: ViewId,
    ) -> Result<Option<&ViewLightBlock>, SceneLightError> {
        if !self.views.contains(&view) {
            return Err(SceneLightError::InvalidView);
        }
        let state = self
            .records
            .get(&record)
            .ok_or(SceneLightError::InvalidRenderRecord)?;
        Ok(state.view_blocks.get(&view))
    }

    /// Toggle the renderer-facing publication seam. It defaults to disabled;
    /// turning it off also drops the current snapshot so re-enabling requires
    /// an explicit publication from current source data.
    pub fn set_publication_enabled(&mut self, enabled: bool) {
        self.publication_enabled = enabled;
        if !enabled {
            self.published = None;
        }
    }

    /// Whether the renderer-facing publication seam is enabled.
    pub fn publication_enabled(&self) -> bool {
        self.publication_enabled
    }

    /// Publish a complete immutable renderer snapshot from already-prepared
    /// per-view blocks. Validation completes before replacing the previous
    /// publication, so a rejected partial request cannot leak into a frame.
    pub fn publish_frame(
        &mut self,
        epoch: FrameResourceEpoch,
        selections: &[SceneLightBinding],
    ) -> Result<&PublishedSceneLights, SceneLightError> {
        if !self.publication_enabled {
            return Err(SceneLightError::PublicationDisabled);
        }
        let mut seen = BTreeSet::new();
        let mut blocks = Vec::with_capacity(selections.len());
        for &selection in selections {
            let record = selection.record();
            let view = selection.view();
            if !seen.insert((record, view)) {
                return Err(SceneLightError::DuplicatePublicationEntry);
            }
            if !self.views.contains(&view) {
                return Err(SceneLightError::InvalidView);
            }
            let state = self
                .records
                .get(&record)
                .ok_or(SceneLightError::InvalidRenderRecord)?;
            let block = state
                .view_blocks
                .get(&view)
                .ok_or(SceneLightError::MissingPreparedViewLight)?;
            if block.epoch != epoch {
                return Err(SceneLightError::FrameEpochMismatch);
            }
            blocks.push(*block);
        }
        self.published = Some(PublishedSceneLights { epoch, blocks });
        Ok(self.published.as_ref().expect("publication inserted"))
    }

    /// Borrow the current publication for attachment to a prepared scene.
    /// Any source or per-view mutation clears it until the caller republishes.
    pub fn published_frame(&self) -> Option<&PublishedSceneLights> {
        self.published.as_ref()
    }
}

fn next_id(counter: &mut u64) -> Result<u64, SceneLightError> {
    let id = counter
        .checked_add(1)
        .ok_or(SceneLightError::IdentityExhausted)?;
    *counter = id;
    Ok(id)
}
