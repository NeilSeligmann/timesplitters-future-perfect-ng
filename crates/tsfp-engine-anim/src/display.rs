//! Native display sampling over owned animation state (T2241).
//! No simulation commands, events, root-motion consumption or guest writes occur here.
use crate::instance::{ClipView, Instance, evaluate_instance};
use crate::posemath::{Mat4, Sample};
use crate::posemix::{TreeRow, WeightEntry, blend_by_weight, compose_or_copy};

/// Read-only data boundary. Implementations decode disc clips or read a native cache.
/// Missing streamed data is an error, never a fabricated pose.
pub trait ClipSource {
    fn view(&self, clip: u32) -> Option<ClipView<'_>>;
    fn sample(&self, clip: u32, node: usize, key: i32) -> Option<Sample>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub clip: u32,
    pub offset: f32,
    pub layer_time: f32,
    pub policy: (u32, u32),
    pub snap: bool,
}

/// Optional outgoing clip supplying the base side of the per-bone blend.
#[derive(Debug, Clone, PartialEq)]
pub struct BlendLayer {
    pub track: Track,
    pub owner_time: f32,
    pub time_step: f32,
}

/// Owned snapshot published exactly once by the 60 Hz simulation.
#[derive(Debug, Clone, PartialEq)]
pub struct TickPose {
    pub generation: u64,
    pub owner_time: f32,
    /// Owner-time advance per simulation tick (zero while paused).
    pub time_step: f32,
    pub track: Track,
    pub blend_layer: Option<BlendLayer>,
    pub root: [f32; 9],
    pub local: Vec<Mat4>,
    pub base: Vec<Mat4>,
    pub palette: Vec<Mat4>,
    /// Fixed owner forces full incoming pose for this tick (60540 save-base path).
    pub full_blend: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Skeleton {
    pub tree: Vec<TreeRow>,
    pub weights: Vec<WeightEntry>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sampling {
    #[default]
    Fixed,
    Fractional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidFraction,
    InvalidState,
    MissingClip,
    MissingSample,
}

/// Previous/current state is owned here; rendering borrows it immutably.
#[derive(Debug, Clone)]
pub struct DisplayInstance {
    previous: TickPose,
    current: TickPose,
    skeleton: Skeleton,
}

impl DisplayInstance {
    pub fn new(pose: TickPose, skeleton: Skeleton) -> Result<Self, Error> {
        validate(&pose, &skeleton)?;
        Ok(Self {
            previous: pose.clone(),
            current: pose,
            skeleton,
        })
    }

    /// A generation change denotes a clip restart, teleport or model replacement.
    pub fn publish(&mut self, pose: TickPose) -> Result<(), Error> {
        validate(&pose, &self.skeleton)?;
        self.previous = std::mem::replace(&mut self.current, pose);
        Ok(())
    }

    /// Native fixed-tick producer: evaluate the supplied committed clocks, then
    /// publish atomically. The owner advances clocks/events/root motion itself;
    /// evaluation retains its root history and never consumes simulation deltas.
    /// On missing streamed data, the previous/current pair remains unchanged.
    pub fn evaluate_and_publish(
        &mut self,
        mut pose: TickPose,
        source: &impl ClipSource,
    ) -> Result<(), Error> {
        validate(&pose, &self.skeleton)?;
        pose.local = sample_local(
            source,
            &pose.track,
            pose.owner_time,
            0.0,
            0.0,
            &pose.local,
            pose.root,
        )?;
        if let Some(layer) = &pose.blend_layer {
            pose.base = sample_local(
                source,
                &layer.track,
                layer.owner_time,
                0.0,
                0.0,
                &pose.base,
                [0.0; 9],
            )?;
        }
        pose.palette = build_palette(
            &self.skeleton,
            &pose.local,
            &pose.base,
            &pose.palette,
            pose.full_blend,
        );
        self.publish(pose)
    }

    pub(crate) fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    pub fn ticks(&self) -> (&TickPose, &TickPose) {
        (&self.previous, &self.current)
    }

    /// Sample t_k + alpha * time_step. Fixed mode and alpha=0 return the exact
    /// committed palette. A discontinuity holds the new palette for this tick.
    /// The simulation supplies time_step=1/60 * playback speed, or zero for pause.
    pub fn sample(
        &self,
        source: &impl ClipSource,
        mode: Sampling,
        alpha: f32,
    ) -> Result<Vec<Mat4>, Error> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(Error::InvalidFraction);
        }
        let state = &self.current;
        if mode == Sampling::Fixed
            || alpha == 0.0
            || (state.time_step == 0.0
                && state
                    .blend_layer
                    .as_ref()
                    .is_none_or(|layer| layer.time_step == 0.0))
            || state.generation != self.previous.generation
            || state.track.clip != self.previous.track.clip
        {
            return Ok(state.palette.clone());
        }
        let local = sample_local(
            source,
            &state.track,
            state.owner_time,
            state.time_step,
            alpha,
            &state.local,
            state.root,
        )?;
        let base = match &state.blend_layer {
            Some(layer) => sample_local(
                source,
                &layer.track,
                layer.owner_time,
                layer.time_step,
                alpha,
                &state.base,
                [0.0; 9],
            )?,
            None => state.base.clone(),
        };
        Ok(build_palette(
            &self.skeleton,
            &local,
            &base,
            &state.palette,
            state.full_blend,
        ))
    }
}

/// Keep local matrices local in snapshots; compose only temporary render work.
pub(crate) fn build_palette(
    skeleton: &Skeleton,
    local: &[Mat4],
    base: &[Mat4],
    committed: &[Mat4],
    full: bool,
) -> Vec<Mat4> {
    let mut palette = committed.to_vec();
    blend_by_weight(&skeleton.weights, base, local, &mut palette, full);
    let mut composed = local.to_vec();
    for (node, row) in skeleton.tree.iter().enumerate() {
        if row.parent >= 0 {
            composed[node] =
                crate::posemath::mat_mul(&composed[node], &composed[row.parent as usize]);
        }
    }
    let parents: Vec<_> = skeleton.tree.iter().map(|row| row.parent).collect();
    compose_or_copy(&skeleton.weights, &parents, &composed, &mut palette);
    palette
}

pub(crate) fn sample_local(
    source: &impl ClipSource,
    track: &Track,
    owner_time: f32,
    time_step: f32,
    alpha: f32,
    committed: &[Mat4],
    root: [f32; 9],
) -> Result<Vec<Mat4>, Error> {
    let view = source.view(track.clip).ok_or(Error::MissingClip)?;
    if view.nodes > committed.len() || view.keys.is_empty() || !view.duration.is_finite() {
        return Err(Error::InvalidState);
    }
    let owner_time = owner_time + alpha * time_step;
    if !owner_time.is_finite() {
        return Err(Error::InvalidState);
    }
    let time =
        crate::advance::advance_time(view.duration, owner_time, track.offset, track.layer_time)
            .ok_or(Error::InvalidState)?;
    let (low, high) = crate::keybracket::key_bracket(view.keys, view.duration, time)
        .ok_or(Error::InvalidState)?;
    let samples: Vec<_> = (0..view.nodes)
        .map(|node| {
            Ok((
                source
                    .sample(track.clip, node, low)
                    .ok_or(Error::MissingSample)?,
                source
                    .sample(track.clip, node, high)
                    .ok_or(Error::MissingSample)?,
            ))
        })
        .collect::<Result<_, Error>>()?;
    let mut local = committed.to_vec();
    let mut base = Vec::new();
    let mut instance = Instance {
        layer_time: track.layer_time,
        root,
        matrices: &mut local,
        base: &mut base,
        saved_count: 0,
    };
    evaluate_instance(
        &view,
        owner_time,
        track.offset,
        track.policy,
        track.snap,
        false,
        |node, key| {
            if key == low {
                samples[node].0
            } else {
                samples[node].1
            }
        },
        &mut instance,
    )
    .ok_or(Error::InvalidState)?;
    Ok(local)
}

/// Parsed disc clips owned by the native data loader. No guest pointers or
/// original data are embedded in the executable.
#[derive(Default, Clone)]
pub struct DecodedClips(pub std::collections::BTreeMap<u32, tsfp_format_anim::Clip>);
impl ClipSource for DecodedClips {
    fn view(&self, id: u32) -> Option<ClipView<'_>> {
        let clip = self.0.get(&id)?;
        Some(ClipView {
            keys: &clip.key_frames,
            duration: crate::interp::clip_duration(clip),
            nodes: clip.nodes.len(),
        })
    }
    fn sample(&self, id: u32, node: usize, key: i32) -> Option<Sample> {
        crate::pose::sample(self.0.get(&id)?, node, key, 1.0).ok()
    }
}

pub(crate) fn validate(pose: &TickPose, skeleton: &Skeleton) -> Result<(), Error> {
    let n = pose.local.len();
    if !pose.owner_time.is_finite()
        || !pose.time_step.is_finite()
        || !pose.track.offset.is_finite()
        || !pose.track.layer_time.is_finite()
        || pose.base.len() != n
        || skeleton.tree.len() != n
        || pose.palette.len() != skeleton.weights.len()
        || skeleton
            .weights
            .iter()
            .any(|&(part, weight)| part as usize >= n || !weight.is_finite())
    {
        return Err(Error::InvalidState);
    }
    if let Some(layer) = &pose.blend_layer {
        if !layer.owner_time.is_finite()
            || !layer.time_step.is_finite()
            || !layer.track.offset.is_finite()
            || !layer.track.layer_time.is_finite()
        {
            return Err(Error::InvalidState);
        }
    }
    // Parent-before-child plus forward child/sibling links make the recursive
    // original tree walk bounded and ensure composed parents precede children.
    for (i, row) in skeleton.tree.iter().enumerate() {
        if row.parent < -1
            || row.parent >= i as i32
            || [row.child, row.sibling]
                .iter()
                .any(|&link| link < -1 || (link >= 0 && (link as usize <= i || link as usize >= n)))
        {
            return Err(Error::InvalidState);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Source;
    impl ClipSource for Source {
        fn view(&self, _: u32) -> Option<ClipView<'_>> {
            Some(ClipView {
                keys: &[0, 1],
                duration: 1.0 / 60.0,
                nodes: 1,
            })
        }
        fn sample(&self, _: u32, _: usize, key: i32) -> Option<Sample> {
            Some(Sample {
                bits: [0.0f32, key as f32, 0.0, 0.0, 0.0, 0.0, 1.0].map(f32::to_bits),
            })
        }
    }
    fn fixture() -> DisplayInstance {
        let identity = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        DisplayInstance::new(
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
            },
            Skeleton {
                tree: vec![TreeRow {
                    parent: -1,
                    child: -1,
                    sibling: -1,
                }],
                weights: vec![(0, 1.0)],
            },
        )
        .unwrap()
    }
    #[test]
    fn fractional_pose_does_not_consume_simulation_state() {
        let instance = fixture();
        let before = instance.current.clone();
        for alpha in [0.25, 0.5, 0.75, 1.0] {
            let pose = instance
                .sample(&Source, Sampling::Fractional, alpha)
                .unwrap();
            assert!((pose[0][13] - alpha).abs() < 1e-6);
            assert_eq!(instance.current, before);
            assert_eq!(instance.previous, before);
        }
        assert_eq!(
            instance.sample(&Source, Sampling::Fixed, 0.5).unwrap(),
            before.palette
        );
        assert_eq!(
            instance.sample(&Source, Sampling::Fractional, 0.0).unwrap(),
            before.palette
        );
    }
    #[test]
    fn pause_and_restart_hold_committed_palette() {
        let mut instance = fixture();
        let mut next = instance.current.clone();
        next.time_step = 0.0;
        instance.publish(next.clone()).unwrap();
        assert_eq!(
            instance.sample(&Source, Sampling::Fractional, 0.5).unwrap(),
            next.palette
        );
        next.time_step = 1.0 / 60.0;
        next.generation += 1;
        instance.publish(next.clone()).unwrap();
        assert_eq!(
            instance.sample(&Source, Sampling::Fractional, 0.5).unwrap(),
            next.palette
        );
    }
    #[test]
    fn weight_blend_replays_from_base() {
        let mut instance = fixture();
        instance.skeleton.weights[0].1 = 0.5;
        let pose = instance.sample(&Source, Sampling::Fractional, 0.5).unwrap();
        assert!((pose[0][13] - 0.25).abs() < 1e-6);
    }
    #[test]
    fn outgoing_layer_has_its_own_clock_and_is_not_consumed() {
        let mut instance = fixture();
        instance.skeleton.weights[0].1 = 0.5;
        instance.current.blend_layer = Some(BlendLayer {
            track: instance.current.track.clone(),
            owner_time: 0.0,
            time_step: 0.5 / 60.0,
        });
        let before = instance.current.clone();
        let pose = instance.sample(&Source, Sampling::Fractional, 0.5).unwrap();
        assert!((pose[0][13] - 0.375).abs() < 1e-6);
        assert_eq!(instance.current, before);
    }
    #[test]
    fn paused_incoming_does_not_freeze_running_outgoing_layer() {
        let mut instance = fixture();
        instance.skeleton.weights[0].1 = 0.5;
        let mut pose = instance.current.clone();
        pose.time_step = 0.0;
        pose.blend_layer = Some(BlendLayer {
            track: pose.track.clone(),
            owner_time: 0.0,
            time_step: 1.0 / 60.0,
        });
        instance.evaluate_and_publish(pose, &Source).unwrap();
        let before = (instance.previous.clone(), instance.current.clone());
        for alpha in [0.25, 0.5, 0.75, 1.0] {
            let palette = instance
                .sample(&Source, Sampling::Fractional, alpha)
                .unwrap();
            assert!((palette[0][13] - 0.5 * alpha).abs() < 1e-6);
        }
        assert_eq!(instance.ticks(), (&before.0, &before.1));
        assert_eq!(
            instance.sample(&Source, Sampling::Fixed, 0.5).unwrap(),
            before.1.palette
        );
        assert_eq!(
            instance.sample(&Source, Sampling::Fractional, 0.0).unwrap(),
            before.1.palette
        );
    }

    #[test]
    fn both_paused_layers_return_committed_palette_without_stream_reads() {
        struct Unavailable;
        impl ClipSource for Unavailable {
            fn view(&self, _: u32) -> Option<ClipView<'_>> {
                panic!("paused layers must not read clips")
            }
            fn sample(&self, _: u32, _: usize, _: i32) -> Option<Sample> {
                panic!("paused layers must not read samples")
            }
        }
        let mut instance = fixture();
        let mut pose = instance.current.clone();
        pose.time_step = 0.0;
        pose.blend_layer = Some(BlendLayer {
            track: pose.track.clone(),
            owner_time: 0.0,
            time_step: 0.0,
        });
        instance.evaluate_and_publish(pose, &Source).unwrap();
        assert_eq!(
            instance
                .sample(&Unavailable, Sampling::Fractional, 0.75)
                .unwrap(),
            instance.current.palette
        );
    }

    #[test]
    fn rotation_samples_shortest_arc_and_root_delta_stays_in_simulation() {
        struct Rotate;
        impl ClipSource for Rotate {
            fn view(&self, _: u32) -> Option<ClipView<'_>> {
                Source.view(7)
            }
            fn sample(&self, _: u32, _: usize, key: i32) -> Option<Sample> {
                let angle = key as f32 * std::f32::consts::FRAC_PI_4;
                Some(Sample {
                    bits: [key as f32, 0.0, 0.0, 0.0, 0.0, angle.sin(), angle.cos()]
                        .map(f32::to_bits),
                })
            }
        }
        let mut instance = fixture();
        instance.current.track.policy = (0x82, 0);
        let before = instance.current.clone();
        let pose = instance.sample(&Rotate, Sampling::Fractional, 0.5).unwrap();
        assert!((pose[0][0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        assert!((pose[0][12] - 0.5).abs() < 1e-6);
        assert_eq!(instance.current, before);
    }
    #[test]
    fn native_tick_publishes_committed_pose_and_preserves_owner_state() {
        let mut instance = fixture();
        let before = instance.current.clone();
        let mut next = before.clone();
        next.owner_time = 0.5 / 60.0;
        next.time_step = 0.0; // A paused owner can still commit a changed pose.
        instance.evaluate_and_publish(next, &Source).unwrap();
        assert_eq!(instance.previous, before);
        assert_eq!(instance.current.root, before.root);
        assert!((instance.current.local[0][13] - 0.5).abs() < 1e-6);
        assert_eq!(instance.current.palette, instance.current.local);
        assert_eq!(
            instance.sample(&Source, Sampling::Fixed, 0.5).unwrap(),
            instance.current.palette
        );
    }

    #[test]
    fn failed_native_tick_does_not_publish_partial_layer() {
        struct Incomplete;
        impl ClipSource for Incomplete {
            fn view(&self, clip: u32) -> Option<ClipView<'_>> {
                (clip == 7).then(|| Source.view(clip).unwrap())
            }
            fn sample(&self, clip: u32, node: usize, key: i32) -> Option<Sample> {
                Source.sample(clip, node, key)
            }
        }
        let mut instance = fixture();
        let before = instance.ticks();
        let before = (before.0.clone(), before.1.clone());
        let mut next = before.1.clone();
        next.owner_time = 0.5 / 60.0;
        let mut track = next.track.clone();
        track.clip = 8;
        next.blend_layer = Some(BlendLayer {
            track,
            owner_time: 0.0,
            time_step: 0.0,
        });
        assert_eq!(
            instance.evaluate_and_publish(next, &Incomplete),
            Err(Error::MissingClip)
        );
        assert_eq!(instance.ticks(), (&before.0, &before.1));
    }

    #[test]
    fn rejects_bad_fraction_missing_data_and_cycle() {
        let instance = fixture();
        for alpha in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert_eq!(
                instance.sample(&Source, Sampling::Fractional, alpha),
                Err(Error::InvalidFraction)
            );
        }
        struct Missing;
        impl ClipSource for Missing {
            fn view(&self, _: u32) -> Option<ClipView<'_>> {
                Source.view(7)
            }
            fn sample(&self, _: u32, _: usize, _: i32) -> Option<Sample> {
                None
            }
        }
        assert_eq!(
            instance.sample(&Missing, Sampling::Fractional, 0.5),
            Err(Error::MissingSample)
        );
        let mut skeleton = instance.skeleton.clone();
        skeleton.tree[0].child = 0;
        assert!(matches!(
            DisplayInstance::new(instance.current.clone(), skeleton),
            Err(Error::InvalidState)
        ));
    }
}
