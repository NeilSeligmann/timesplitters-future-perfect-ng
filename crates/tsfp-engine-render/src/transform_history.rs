// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 attachment to the typed end-of-tick store from T2240.
use crate::scene::render_prep::{
    Placement, TransformHistorySource, TransformIdentity, TransformNamespace, TransformSamples,
};
use tsfp_ports::object_transform::{TransformHistoryRead, TransformHistorySnapshot};
use tsfp_state::transforms::{TransformHistory, TransformId, TransformOwner};

/// One copied T2334 bank entry, paired with the render owner's stable identity.
/// The identity mapping is explicit because the compatibility bank's slot and
/// generation are not proof of a scene object's lifetime.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PublishedTransformEntry {
    pub identity: TransformIdentity,
    pub translate: bool,
    pub history: TransformHistorySnapshot,
}

impl PublishedTransformEntry {
    /// Pair a copied host-bank result with the render owner's stable identity.
    /// Missing compatibility keys do not produce entries and therefore cannot
    /// accidentally interpolate the ABI's initialized fallback packet.
    pub fn from_read(
        identity: TransformIdentity,
        read: TransformHistoryRead,
    ) -> Result<Option<Self>, PublishedTransformReadError> {
        if read.found > 1 {
            return Err(PublishedTransformReadError::InvalidFoundFlag);
        }
        if read.found == 0 {
            return Ok(None);
        }
        if read.request.view != read.history.id.view {
            return Err(PublishedTransformReadError::ViewMismatch);
        }
        Ok(Some(Self {
            identity,
            translate: read.request.translate != 0,
            history: read.history,
        }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishedTransformReadError {
    InvalidFoundFlag,
    ViewMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishedTransformSetError {
    TickMismatch,
    DuplicateEntry,
}

/// Immutable render-prep adapter over endpoint packets copied from T2334's
/// host archive. It owns no bank lock and can be reused across display samples.
pub struct PublishedTransformSet<'a> {
    tick: u64,
    default_view: u32,
    translate: bool,
    entries: &'a [PublishedTransformEntry],
}

impl<'a> PublishedTransformSet<'a> {
    pub fn new(
        tick: u64,
        default_view: u32,
        translate: bool,
        entries: &'a [PublishedTransformEntry],
    ) -> Result<Self, PublishedTransformSetError> {
        for (index, entry) in entries.iter().enumerate() {
            if entry.history.tick != tick {
                return Err(PublishedTransformSetError::TickMismatch);
            }
            if entries[..index].iter().any(|prior| {
                prior.identity == entry.identity
                    && prior.translate == entry.translate
                    && prior.history.id.view == entry.history.id.view
            }) {
                return Err(PublishedTransformSetError::DuplicateEntry);
            }
        }
        Ok(Self {
            tick,
            default_view,
            translate,
            entries,
        })
    }

    fn samples(&self, identity: TransformIdentity, view: u32) -> Option<TransformSamples> {
        let entry = self.entries.iter().find(|entry| {
            entry.identity == identity
                && entry.translate == self.translate
                && entry.history.id.view == view
        })?;
        Some(TransformSamples {
            previous: placement(entry.history.previous),
            current: placement(entry.history.current),
            discontinuity: entry.history.discontinuity != 0,
        })
    }
}

fn placement(world: tsfp_ports::object_transform::WorldTransform) -> Placement {
    Placement {
        position: [world.matrix[12], world.matrix[13], world.matrix[14]],
        orientation: core::array::from_fn(|row| {
            core::array::from_fn(|column| world.matrix[row * 4 + column])
        }),
    }
}

impl TransformHistorySource for PublishedTransformSet<'_> {
    fn tick(&self) -> Option<u64> {
        Some(self.tick)
    }

    fn transform_samples(&self, id: TransformIdentity) -> Option<TransformSamples> {
        self.samples(id, self.default_view)
    }

    fn transform_samples_for_view(
        &self,
        id: TransformIdentity,
        transform_view: u32,
    ) -> Option<TransformSamples> {
        self.samples(id, transform_view)
    }
}

impl TransformHistorySource for TransformHistory {
    fn tick(&self) -> Option<u64> {
        TransformHistory::tick(self)
    }

    fn transform_samples(&self, id: TransformIdentity) -> Option<TransformSamples> {
        let transform_id = TransformId {
            owner: match id.namespace {
                TransformNamespace::Object => TransformOwner::Object,
                TransformNamespace::Player => TransformOwner::Player,
                TransformNamespace::Character => TransformOwner::Character,
                TransformNamespace::Camera => TransformOwner::Camera,
            },
            slot: id.slot,
            generation: id.generation,
        };
        let pair = self.get(transform_id)?;
        Some(TransformSamples {
            previous: Placement {
                position: pair.previous.world.position,
                orientation: pair.previous.world.orientation,
            },
            current: Placement {
                position: pair.current.world.position,
                orientation: pair.current.world.orientation,
            },
            discontinuity: pair.discontinuity.any(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ports::object_transform::{
        TransformHistoryRead, TransformHistoryRequest, TransformId as PublishedId, WorldTransform,
    };

    fn packet(
        slot: u32,
        generation: u64,
        view: u32,
        previous_x: f32,
        current_x: f32,
    ) -> TransformHistorySnapshot {
        let mut previous = WorldTransform::IDENTITY;
        previous.matrix[12] = previous_x;
        let mut current = WorldTransform::IDENTITY;
        current.matrix[12] = current_x;
        TransformHistorySnapshot {
            id: PublishedId {
                slot,
                generation,
                view,
            },
            tick: 12,
            previous,
            current,
            discontinuity: 0,
        }
    }

    #[test]
    fn copied_t2334_endpoints_are_selected_by_explicit_identity_and_view() {
        let identity = TransformIdentity {
            namespace: TransformNamespace::Object,
            slot: 7,
            generation: 42,
        };
        let entries = [
            PublishedTransformEntry {
                identity,
                translate: true,
                history: packet(7, 42, 1, 2.0, 10.0),
            },
            PublishedTransformEntry {
                identity,
                translate: true,
                history: packet(7, 42, 4, 20.0, 60.0),
            },
            PublishedTransformEntry {
                identity,
                translate: false,
                history: packet(7, 42, 1, 200.0, 600.0),
            },
        ];
        let set = PublishedTransformSet::new(12, 1, true, &entries).unwrap();
        assert_eq!(set.tick(), Some(12));
        assert_eq!(
            set.transform_samples(identity).unwrap().previous.position[0],
            2.0
        );
        assert_eq!(
            set.transform_samples_for_view(identity, 4)
                .unwrap()
                .current
                .position[0],
            60.0
        );
        assert!(set.transform_samples_for_view(identity, 2).is_none());
    }

    #[test]
    fn copied_t2334_set_accepts_explicit_owner_mapping_and_rejects_bad_sets() {
        let identity = TransformIdentity {
            namespace: TransformNamespace::Object,
            slot: 701,
            generation: 9002,
        };
        let mut stale = packet(7, 42, 1, 0.0, 1.0);
        stale.tick = 11;
        let entries = [PublishedTransformEntry {
            identity,
            translate: true,
            history: stale,
        }];
        assert_eq!(
            PublishedTransformSet::new(12, 1, true, &entries).err(),
            Some(PublishedTransformSetError::TickMismatch)
        );
        let entries = [PublishedTransformEntry {
            identity,
            translate: true,
            history: packet(7, 42, 1, 0.0, 1.0),
        }];
        let set = PublishedTransformSet::new(12, 1, true, &entries).unwrap();
        assert_eq!(
            set.transform_samples(identity).unwrap().current.position[0],
            1.0
        );
        let duplicate_entries = [entries[0], entries[0]];
        assert_eq!(
            PublishedTransformSet::new(12, 1, true, &duplicate_entries).err(),
            Some(PublishedTransformSetError::DuplicateEntry)
        );
    }

    #[test]
    fn copied_read_rejects_malformed_flags_and_view_mismatch() {
        let identity = TransformIdentity {
            namespace: TransformNamespace::Object,
            slot: 701,
            generation: 9002,
        };
        let read = || TransformHistoryRead {
            request: TransformHistoryRequest {
                reference: 0x2000,
                view: 1,
                translate: 1,
            },
            found: 1,
            history: packet(7, 42, 1, 2.0, 10.0),
        };
        assert!(
            PublishedTransformEntry::from_read(identity, read())
                .unwrap()
                .is_some()
        );
        let mut missing = read();
        missing.found = 0;
        assert_eq!(
            PublishedTransformEntry::from_read(identity, missing),
            Ok(None)
        );
        let mut invalid = read();
        invalid.found = 2;
        assert_eq!(
            PublishedTransformEntry::from_read(identity, invalid),
            Err(PublishedTransformReadError::InvalidFoundFlag)
        );
        let mut mismatched = read();
        mismatched.history.id.view = 4;
        assert_eq!(
            PublishedTransformEntry::from_read(identity, mismatched),
            Err(PublishedTransformReadError::ViewMismatch)
        );
    }

    #[test]
    fn copied_t2334_set_drives_scene_pair_display_sampling() {
        use crate::scene::{
            render_prep::{
                ModelId, ObjectCommand, ObjectId, ObjectSnapshot, Placement, Sampling, SceneObject,
                ScenePair, SceneSnapshot, SlerpPlacementSampler,
            },
            spatial::{Bounds, Node, SpatialTree},
        };
        use alloc::vec;

        let transform = TransformIdentity {
            namespace: TransformNamespace::Object,
            slot: 701,
            generation: 9002,
        };
        let object = ObjectId {
            slot: 3,
            generation: 9,
        };
        let bounds = Bounds {
            min: [-20.0; 3],
            max: [20.0; 3],
        };
        let snapshot = |tick| SceneSnapshot {
            tick,
            objects: vec![ObjectSnapshot {
                id: object,
                draw: SceneObject {
                    id: object,
                    model: ModelId(4),
                    visible: true,
                    scissor_rejected: false,
                    model_hidden: false,
                    deferred: false,
                    overlay: crate::scene::render_prep::Overlay::None,
                    scissor: [0.0, 0.0, 10.0, 10.0],
                },
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
        let previous = snapshot(11);
        let current = snapshot(12);
        let read = TransformHistoryRead {
            request: TransformHistoryRequest {
                reference: 0x2000,
                view: 1,
                translate: 1,
            },
            found: 1,
            history: packet(7, 42, 1, 3.0, 9.0),
        };
        let entry = PublishedTransformEntry::from_read(transform, read)
            .unwrap()
            .unwrap();
        let entries = [entry];
        let set = PublishedTransformSet::new(12, 1, true, &entries).unwrap();
        let pair = ScenePair::with_transform_history_and_bindings(
            &previous,
            &current,
            &set,
            &[(object, transform)],
        )
        .unwrap();
        let tree = SpatialTree {
            bounds,
            nodes: vec![Node::Leaf(vec![]), Node::Leaf(vec![0])],
            split_scale: 0.0,
        };
        let view = crate::scene::render_prep::View {
            query: bounds,
            planes: vec![],
            visible_rooms: Default::default(),
        };
        let prepared = pair
            .prepare_view(
                &tree,
                &view,
                0.5,
                Sampling::BetweenTicks,
                &SlerpPlacementSampler,
            )
            .unwrap();
        assert_eq!(prepared.tick, 12);
        assert_eq!(prepared.objects.len(), 1);
        assert_eq!(prepared.objects[0].placement.position, [6.0, 0.0, 0.0]);
        assert!(
            prepared.objects[0]
                .commands
                .contains(&ObjectCommand::DrawModel(ModelId(4)))
        );
    }
}
