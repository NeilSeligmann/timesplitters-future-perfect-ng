//! Native animation lifecycle and immutable render batches (T2241).
//! Object keys are native lifetime identities, never guest addresses.
use std::collections::{BTreeMap, BTreeSet};

use crate::display::{ClipSource, Error, Sampling, TickPose};
use crate::posemath::Mat4;
use crate::presentation::{AnimatedObject, ModelCorrection, ObjectTick, Reflection};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneError {
    DuplicateObject,
    MissingObject,
    InvalidTick,
    Animation(Error),
}

impl From<Error> for SceneError {
    fn from(value: Error) -> Self {
        Self::Animation(value)
    }
}

/// The simulation owner provides one evaluated command per object per tick.
pub struct Update {
    pub object: ObjectId,
    pub pose: TickPose,
    pub placement: ObjectTick,
}

/// The native owner supplies fixed clocks and placement, not a guest-backed pose.
pub struct SimulationUpdate {
    pub object: ObjectId,
    pub command: crate::simulation::FixedCommand,
    pub placement: ObjectTick,
}

/// Model and view owners resolve these inputs, rather than exposing raw flags.
#[derive(Clone, Copy, Debug)]
pub struct ViewObject {
    pub object: ObjectId,
    pub reflection: Reflection,
    pub camera: Option<Mat4>,
    pub correction: ModelCorrection,
    pub use_palette: bool,
}

#[derive(Debug, PartialEq)]
pub struct Palette {
    pub object: ObjectId,
    pub nodes: Vec<Mat4>,
}

/// Contains only successfully sampled matrices. The renderer can stage a whole
/// view before making any GPU changes; stream misses cannot produce half a view.
#[derive(Debug, PartialEq)]
pub struct ViewBatch {
    pub tick: u64,
    pub palettes: Vec<Palette>,
}

#[derive(Default, Clone, Debug)]
pub struct AnimationScene {
    objects: BTreeMap<ObjectId, AnimatedObject>,
    tick: Option<u64>,
    sampling: Sampling,
}

impl AnimationScene {
    /// Fixed is the default. This display policy never changes simulation inputs.
    pub fn set_sampling(&mut self, sampling: Sampling) {
        self.sampling = sampling;
    }
    pub fn sampling(&self) -> Sampling {
        self.sampling
    }
    pub fn tick(&self) -> Option<u64> {
        self.tick
    }
    pub fn object(&self, id: ObjectId) -> Option<&AnimatedObject> {
        self.objects.get(&id)
    }

    /// Native lifetimes are allocated by the object owner. An occupied identity
    /// must be removed before replacement; never silently overwrite its history.
    pub fn insert(&mut self, id: ObjectId, object: AnimatedObject) -> Result<(), SceneError> {
        if self.objects.contains_key(&id) {
            return Err(SceneError::DuplicateObject);
        }
        self.objects.insert(id, object);
        Ok(())
    }
    pub fn remove(&mut self, id: ObjectId) -> Option<AnimatedObject> {
        self.objects.remove(&id)
    }
    /// Level teardown drops all previous/current histories and the fixed tick.
    pub fn clear(&mut self) {
        self.objects.clear();
        self.tick = None;
    }

    /// Publish a complete fixed tick atomically. Every owned object must appear
    /// exactly once; skipped/repeated ticks would make fractional clocks stale.
    pub fn publish_tick(
        &mut self,
        tick: u64,
        updates: Vec<Update>,
        source: &impl ClipSource,
    ) -> Result<(), SceneError> {
        self.publish_tick_inner(
            tick,
            updates.into_iter().map(|u| (u.object, u)).collect(),
            |object, update| object.publish(update.pose, update.placement, source),
        )
        .map(|_| ())
    }

    /// Commit complete results from native fixed producers. Their root/base/pose
    /// results are retained exactly, with no second simulation evaluation.
    pub fn publish_committed_tick(
        &mut self,
        tick: u64,
        updates: Vec<Update>,
    ) -> Result<(), SceneError> {
        self.publish_tick_inner(
            tick,
            updates.into_iter().map(|u| (u.object, u)).collect(),
            |object, update| object.publish_committed(update.pose, update.placement),
        )
        .map(|_| ())
    }

    /// Advance all native instances atomically at one fixed tick and return root
    /// displacements for object handlers. A failed object's data prevents the
    /// entire tick from committing; no root/event consumer runs during staging.
    pub fn simulate_tick(
        &mut self,
        tick: u64,
        updates: Vec<SimulationUpdate>,
        source: &impl ClipSource,
    ) -> Result<Vec<(ObjectId, [f32; 3])>, SceneError> {
        self.publish_tick_inner(
            tick,
            updates.into_iter().map(|u| (u.object, u)).collect(),
            |object, update| object.advance_fixed(update.command, update.placement, source),
        )
    }

    fn publish_tick_inner<U, O>(
        &mut self,
        tick: u64,
        updates: Vec<(ObjectId, U)>,
        mut publish: impl FnMut(&mut AnimatedObject, U) -> Result<O, Error>,
    ) -> Result<Vec<(ObjectId, O)>, SceneError> {
        if self
            .tick
            .is_some_and(|previous| previous.checked_add(1) != Some(tick))
        {
            return Err(SceneError::InvalidTick);
        }
        let mut seen = BTreeSet::new();
        let mut pending = Vec::with_capacity(updates.len());
        let mut outputs = Vec::with_capacity(updates.len());
        for (id, update) in updates {
            if !seen.insert(id) {
                return Err(SceneError::DuplicateObject);
            }
            let mut object = self
                .objects
                .get(&id)
                .ok_or(SceneError::MissingObject)?
                .clone();
            let output = publish(&mut object, update)?;
            outputs.push((id, output));
            pending.push((id, object));
        }
        if pending.len() != self.objects.len() {
            return Err(SceneError::MissingObject);
        }
        for (id, object) in pending {
            self.objects.insert(id, object);
        }
        self.tick = Some(tick);
        Ok(outputs)
    }

    /// Immutable per-view display handler. Culled objects may be omitted; output
    /// order follows draw order rather than sorting by lifetime identity.
    pub fn sample_view(
        &self,
        objects: &[ViewObject],
        alpha: f32,
        source: &impl ClipSource,
    ) -> Result<ViewBatch, SceneError> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(Error::InvalidFraction.into());
        }
        let tick = self.tick.ok_or(SceneError::InvalidTick)?;
        let mut palettes = Vec::with_capacity(objects.len());
        for view in objects {
            let object = self
                .objects
                .get(&view.object)
                .ok_or(SceneError::MissingObject)?;
            let nodes = object.sample_for_view(
                source,
                self.sampling,
                alpha,
                view.reflection,
                view.camera,
                view.correction,
                view.use_palette,
            )?;
            palettes.push(Palette {
                object: view.object,
                nodes,
            });
        }
        Ok(ViewBatch { tick, palettes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{DisplayInstance, Skeleton, Track};
    use crate::instance::ClipView;
    use crate::posemath::Sample;
    use crate::posemix::TreeRow;
    use crate::presentation::Orientation;

    struct Source;
    impl ClipSource for Source {
        fn view(&self, clip: u32) -> Option<ClipView<'_>> {
            (clip == 7).then_some(ClipView {
                keys: &[0, 1],
                duration: 1.0 / 60.0,
                nodes: 1,
            })
        }
        fn sample(&self, _: u32, _: usize, key: i32) -> Option<Sample> {
            Some(Sample {
                bits: [0.0, key as f32, 0.0, 0.0, 0.0, 0.0, 1.0].map(f32::to_bits),
            })
        }
    }
    fn pose() -> TickPose {
        let identity = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        TickPose {
            generation: 1,
            owner_time: 0.0,
            time_step: 1.0 / 60.0,
            track: Track {
                clip: 7,
                offset: 0.0,
                layer_time: 0.0,
                policy: (0, 0),
                snap: false,
            },
            blend_layer: None,
            root: [42.0; 9],
            local: vec![identity],
            base: vec![identity],
            palette: vec![identity],
            full_blend: false,
        }
    }
    fn placement() -> ObjectTick {
        ObjectTick {
            orientation: Orientation::Identity,
            flip_x: false,
            position: [0.0; 3],
        }
    }
    fn object() -> AnimatedObject {
        AnimatedObject::new(
            DisplayInstance::new(
                pose(),
                Skeleton {
                    tree: vec![TreeRow {
                        parent: -1,
                        child: -1,
                        sibling: -1,
                    }],
                    weights: vec![(0, 1.0)],
                },
            )
            .unwrap(),
            placement(),
        )
        .unwrap()
    }
    fn update(id: u64) -> Update {
        Update {
            object: ObjectId(id),
            pose: pose(),
            placement: placement(),
        }
    }
    fn view(id: u64) -> ViewObject {
        ViewObject {
            object: ObjectId(id),
            reflection: Reflection::None,
            camera: None,
            correction: ModelCorrection::None,
            use_palette: true,
        }
    }
    #[test]
    fn full_scene_failure_rolls_back_and_tick_can_retry() {
        let mut scene = AnimationScene::default();
        scene.insert(ObjectId(1), object()).unwrap();
        scene.insert(ObjectId(2), object()).unwrap();
        scene
            .publish_tick(10, vec![update(1), update(2)], &Source)
            .unwrap();
        let before = scene
            .object(ObjectId(1))
            .unwrap()
            .animation()
            .ticks()
            .1
            .clone();
        let mut first = update(1);
        first.pose.owner_time = 0.005;
        let mut missing = update(2);
        missing.pose.track.clip = 8;
        assert_eq!(
            scene.publish_tick(11, vec![first, missing], &Source),
            Err(SceneError::Animation(Error::MissingClip))
        );
        assert_eq!(scene.tick(), Some(10));
        assert_eq!(
            scene.object(ObjectId(1)).unwrap().animation().ticks().1,
            &before
        );
        scene
            .publish_tick(11, vec![update(1), update(2)], &Source)
            .unwrap();
        assert_eq!(
            scene.publish_tick(11, vec![update(1), update(2)], &Source),
            Err(SceneError::InvalidTick)
        );
        assert_eq!(
            scene.publish_tick(13, vec![update(1), update(2)], &Source),
            Err(SceneError::InvalidTick)
        );
    }
    #[test]
    fn lifetime_and_complete_tick_contracts() {
        let mut scene = AnimationScene::default();
        scene.insert(ObjectId(1), object()).unwrap();
        assert_eq!(
            scene.insert(ObjectId(1), object()),
            Err(SceneError::DuplicateObject)
        );
        assert_eq!(
            scene.publish_tick(0, vec![], &Source),
            Err(SceneError::MissingObject)
        );
        assert_eq!(
            scene.publish_tick(0, vec![update(1), update(1)], &Source),
            Err(SceneError::DuplicateObject)
        );
        assert_eq!(
            scene.publish_tick(0, vec![update(2)], &Source),
            Err(SceneError::MissingObject)
        );
        assert_eq!(scene.tick(), None);
        scene.publish_tick(0, vec![update(1)], &Source).unwrap();
        scene.remove(ObjectId(1)).unwrap();
        assert_eq!(
            scene.sample_view(&[view(1)], 0.5, &Source),
            Err(SceneError::MissingObject)
        );
        scene.clear();
        assert_eq!(
            scene.sample_view(&[], 0.5, &Source),
            Err(SceneError::InvalidTick)
        );
        scene.insert(ObjectId(2), object()).unwrap();
        scene.publish_tick(100, vec![update(2)], &Source).unwrap();
    }
    #[test]
    fn fractional_scene_samples_are_read_only_and_fixed_ticks_equal() {
        let mut scene = AnimationScene::default();
        scene.insert(ObjectId(1), object()).unwrap();
        scene.insert(ObjectId(2), object()).unwrap();
        scene
            .publish_tick(0, vec![update(1), update(2)], &Source)
            .unwrap();
        let mut fractional = scene.clone();
        fractional.set_sampling(Sampling::Fractional);
        for tick in 1..=60 {
            for alpha in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let batch = fractional
                    .sample_view(&[view(2), view(1)], alpha, &Source)
                    .unwrap();
                assert_eq!(batch.palettes[0].object, ObjectId(2));
                assert!((batch.palettes[0].nodes[0][13] - alpha).abs() < 1e-6);
                let fixed = scene
                    .sample_view(&[view(2), view(1)], alpha, &Source)
                    .unwrap();
                assert_eq!(fixed.palettes[0].nodes[0][13], 0.0);
            }
            scene
                .publish_tick(tick, vec![update(1), update(2)], &Source)
                .unwrap();
            fractional
                .publish_tick(tick, vec![update(1), update(2)], &Source)
                .unwrap();
            for id in [ObjectId(1), ObjectId(2)] {
                assert_eq!(
                    scene.object(id).unwrap().animation().ticks(),
                    fractional.object(id).unwrap().animation().ticks()
                );
                assert_eq!(
                    scene.object(id).unwrap().tick(),
                    fractional.object(id).unwrap().tick()
                );
            }
        }
    }
    #[test]
    fn native_simulation_tick_commits_all_objects_or_none() {
        let mut scene = AnimationScene::default();
        scene.insert(ObjectId(1), object()).unwrap();
        scene.insert(ObjectId(2), object()).unwrap();
        let command = |time| crate::simulation::FixedCommand {
            owner_time: time,
            time_step: 1.0 / 60.0,
            save_base: false,
            saved_count: 0,
            blend_owner_time: None,
        };
        let updates = |bad| {
            vec![
                SimulationUpdate {
                    object: ObjectId(1),
                    command: command(0.005),
                    placement: placement(),
                },
                SimulationUpdate {
                    object: ObjectId(2),
                    command: command(if bad { f32::NAN } else { 0.01 }),
                    placement: placement(),
                },
            ]
        };
        let before = scene
            .object(ObjectId(1))
            .unwrap()
            .animation()
            .ticks()
            .1
            .clone();
        assert!(scene.simulate_tick(0, updates(true), &Source).is_err());
        assert_eq!(scene.tick(), None);
        assert_eq!(
            scene.object(ObjectId(1)).unwrap().animation().ticks().1,
            &before
        );
        assert_eq!(
            scene.simulate_tick(0, updates(false), &Source).unwrap(),
            vec![(ObjectId(1), [0.0; 3]), (ObjectId(2), [0.0; 3])]
        );
        let committed = scene
            .object(ObjectId(1))
            .unwrap()
            .animation()
            .ticks()
            .1
            .clone();
        assert_eq!(committed.owner_time, 0.005);
        assert_eq!(
            &committed.root[3..6],
            &before.root[3..6],
            "non-root policy preserves previous"
        );
        assert!(scene.simulate_tick(0, updates(false), &Source).is_err());
        assert_eq!(
            scene.object(ObjectId(1)).unwrap().animation().ticks().1,
            &committed
        );
    }

    #[test]
    fn committed_publication_preserves_producer_root_and_palette_exactly() {
        let mut scene = AnimationScene::default();
        scene.insert(ObjectId(1), object()).unwrap();
        let mut committed = pose();
        committed.root = [1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 0.5, 0.25, 0.125];
        committed.local[0][12] = 17.0;
        committed.palette[0][12] = 23.0;
        scene
            .publish_committed_tick(
                0,
                vec![Update {
                    object: ObjectId(1),
                    pose: committed.clone(),
                    placement: placement(),
                }],
            )
            .unwrap();
        assert_eq!(
            scene.object(ObjectId(1)).unwrap().animation().ticks().1,
            &committed
        );
        assert_eq!(
            scene
                .sample_view(&[view(1)], 0.75, &Source)
                .unwrap()
                .palettes[0]
                .nodes[0][12],
            23.0
        );
    }
}
