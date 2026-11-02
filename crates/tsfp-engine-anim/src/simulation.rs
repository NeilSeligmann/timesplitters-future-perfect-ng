// SPDX-License-Identifier: GPL-3.0-or-later
//! Native fixed-tick pose producer for the reconstructed 60B30 instance contract.
//! Clock commands come from the native owner; rendering never invokes this producer.
use crate::display::{
    ClipSource, DisplayInstance, Error, Skeleton, TickPose, build_palette, validate,
};
use crate::instance::{Instance, evaluate_instance};
use crate::keybracket::{bracket_weight, key_bracket};
use crate::posemath::Sample;

/// Explicit fixed owner command. Saved base pose uses the original prefix count;
/// the display path never saves a base or consumes root displacement.
#[derive(Clone, Copy, Debug)]
pub struct FixedCommand {
    pub owner_time: f32,
    pub time_step: f32,
    pub save_base: bool,
    pub saved_count: usize,
    /// Required when an outgoing blend layer is active; independent of incoming pause.
    pub blend_owner_time: Option<f32>,
}

/// Owned committed state and display history. No guest offsets or call trampolines.
#[derive(Clone, Debug)]
pub struct FixedInstance {
    display: DisplayInstance,
    skeleton: Skeleton,
}

impl FixedInstance {
    pub fn new(pose: TickPose, skeleton: Skeleton) -> Result<Self, Error> {
        Ok(Self {
            display: DisplayInstance::new(pose, skeleton.clone())?,
            skeleton,
        })
    }
    pub fn from_display(display: DisplayInstance) -> Self {
        let skeleton = display.skeleton().clone();
        Self { display, skeleton }
    }
    pub fn display(&self) -> &DisplayInstance {
        &self.display
    }
    pub fn into_display(self) -> DisplayInstance {
        self.display
    }

    /// Evaluate and commit one complete fixed command transactionally. Returns
    /// root displacement for the native object owner to consume once. Missing
    /// streamed samples preserve clocks, roots, base, palette and history.
    pub fn tick(
        &mut self,
        command: FixedCommand,
        source: &impl ClipSource,
    ) -> Result<[f32; 3], Error> {
        let mut pose = self.display.ticks().1.clone();
        pose.owner_time = command.owner_time;
        pose.time_step = command.time_step;
        validate(&pose, &self.skeleton)?;
        if command.saved_count > pose.local.len() {
            return Err(Error::InvalidState);
        }
        match (&mut pose.blend_layer, command.blend_owner_time) {
            (Some(layer), Some(time)) if time.is_finite() => {
                pose.base = crate::display::sample_local(
                    source,
                    &layer.track,
                    time,
                    0.0,
                    0.0,
                    &pose.base,
                    [0.0; 9],
                )?;
                let view = source.view(layer.track.clip).ok_or(Error::MissingClip)?;
                layer.track.layer_time = crate::advance::advance_time(
                    view.duration,
                    time,
                    layer.track.offset,
                    layer.track.layer_time,
                )
                .ok_or(Error::InvalidState)?;
                layer.owner_time = time;
            }
            (None, None) => {}
            _ => return Err(Error::InvalidState),
        }
        let view = source.view(pose.track.clip).ok_or(Error::MissingClip)?;
        if view.nodes > pose.local.len() || !view.duration.is_finite() {
            return Err(Error::InvalidState);
        }
        let time = crate::advance::advance_time(
            view.duration,
            pose.owner_time,
            pose.track.offset,
            pose.track.layer_time,
        )
        .ok_or(Error::InvalidState)?;
        let (low, high) = key_bracket(view.keys, view.duration, time).ok_or(Error::InvalidState)?;
        bracket_weight(view.keys, low, high, time).ok_or(Error::InvalidState)?;
        let samples: Vec<(Sample, Sample)> = (0..view.nodes)
            .map(|node| {
                let a = source
                    .sample(pose.track.clip, node, low)
                    .ok_or(Error::MissingSample)?;
                let b = source
                    .sample(pose.track.clip, node, high)
                    .ok_or(Error::MissingSample)?;
                if !a
                    .position()
                    .into_iter()
                    .chain(a.quaternion())
                    .chain(b.position())
                    .chain(b.quaternion())
                    .all(|v| v.is_finite())
                {
                    return Err(Error::InvalidState);
                }
                Ok((a, b))
            })
            .collect::<Result<_, _>>()?;
        let mut instance = Instance {
            layer_time: pose.track.layer_time,
            root: pose.root,
            matrices: &mut pose.local,
            base: &mut pose.base,
            saved_count: command.saved_count,
        };
        evaluate_instance(
            &view,
            pose.owner_time,
            pose.track.offset,
            pose.track.policy,
            pose.track.snap,
            command.save_base,
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
        pose.track.layer_time = instance.layer_time;
        pose.root = instance.root;
        pose.full_blend = command.save_base;
        pose.palette = build_palette(
            &self.skeleton,
            &pose.local,
            &pose.base,
            &pose.palette,
            pose.full_blend,
        );
        let delta = [pose.root[6], pose.root[7], pose.root[8]];
        self.display.publish(pose)?;
        Ok(delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{Sampling, Track};
    use crate::instance::ClipView;
    use crate::posemix::TreeRow;
    struct Source;
    impl ClipSource for Source {
        fn view(&self, _: u32) -> Option<ClipView<'_>> {
            Some(ClipView {
                keys: &[0, 60],
                duration: 1.0,
                nodes: 1,
            })
        }
        fn sample(&self, _: u32, _: usize, key: i32) -> Option<Sample> {
            Some(Sample {
                bits: [key as f32 * 60.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0].map(f32::to_bits),
            })
        }
    }
    fn instance() -> FixedInstance {
        let id = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        FixedInstance::new(
            TickPose {
                generation: 1,
                owner_time: 0.0,
                time_step: 1.0 / 60.0,
                track: Track {
                    clip: 7,
                    offset: 0.0,
                    layer_time: 0.0,
                    policy: (2, 0),
                    snap: false,
                },
                blend_layer: None,
                root: [0.0; 9],
                local: vec![id],
                base: vec![id],
                palette: vec![id],
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
    fn native_owner_consumes_root_once_and_display_sampling_is_read_only() {
        let mut actor = instance();
        let command = |time| FixedCommand {
            owner_time: time,
            time_step: 1.0 / 60.0,
            save_base: false,
            saved_count: 0,
            blend_owner_time: None,
        };
        assert_eq!(
            actor.tick(command(0.25), &Source).unwrap(),
            [15.0, 0.0, 0.0]
        );
        assert_eq!(actor.tick(command(0.5), &Source).unwrap(), [15.0, 0.0, 0.0]);
        let before = actor.display().ticks().1.clone();
        for alpha in [0.0, 0.25, 0.5, 0.75, 1.0] {
            actor
                .display()
                .sample(&Source, Sampling::Fractional, alpha)
                .unwrap();
            assert_eq!(actor.display().ticks().1, &before);
        }
        assert_eq!(&before.root[..6], &[30.0, 0.0, 0.0, 30.0, 0.0, 0.0]);
    }
    #[test]
    fn missing_samples_are_atomic_for_entire_fixed_state() {
        struct Missing;
        impl ClipSource for Missing {
            fn view(&self, id: u32) -> Option<ClipView<'_>> {
                Source.view(id)
            }
            fn sample(&self, _: u32, _: usize, _: i32) -> Option<Sample> {
                None
            }
        }
        let mut actor = instance();
        let before = actor.display().ticks().1.clone();
        assert_eq!(
            actor.tick(
                FixedCommand {
                    owner_time: 0.5,
                    time_step: 1.0 / 60.0,
                    save_base: true,
                    saved_count: 1,
                    blend_owner_time: None,
                },
                &Missing
            ),
            Err(Error::MissingSample)
        );
        assert_eq!(actor.display().ticks().1, &before);
        assert_eq!(actor.display().ticks().0, &before);
    }
    #[test]
    fn save_base_forces_incoming_weight_for_fractional_sampling_too() {
        let mut original = instance();
        let pose = original.display().ticks().1.clone();
        let mut skeleton = original.skeleton.clone();
        skeleton.weights[0].1 = 0.5;
        original = FixedInstance::new(pose, skeleton).unwrap();
        let command = FixedCommand {
            owner_time: 0.5,
            time_step: 1.0 / 60.0,
            save_base: true,
            saved_count: 1,
            blend_owner_time: None,
        };
        original.tick(command, &Source).unwrap();
        let pose = original.display().ticks().1;
        assert!(pose.full_blend);
        assert_eq!(pose.base[0], pose.local[0]);
        assert_eq!(pose.palette[0][12], 30.0);
        let sampled = original
            .display()
            .sample(&Source, Sampling::Fractional, 0.5)
            .unwrap();
        assert!((sampled[0][12] - 30.5).abs() < 1e-5);
    }
    #[test]
    fn outgoing_clock_is_independent_and_required_when_blending() {
        let mut actor = instance();
        let mut pose = actor.display().ticks().1.clone();
        pose.blend_layer = Some(crate::display::BlendLayer {
            track: pose.track.clone(),
            owner_time: 0.0,
            time_step: 1.0 / 60.0,
        });
        let mut skeleton = actor.skeleton.clone();
        skeleton.weights[0].1 = 0.5;
        actor = FixedInstance::new(pose.clone(), skeleton).unwrap();
        let mut command = FixedCommand {
            owner_time: 0.25,
            time_step: 0.0,
            save_base: false,
            saved_count: 0,
            blend_owner_time: None,
        };
        assert_eq!(actor.tick(command, &Source), Err(Error::InvalidState));
        assert_eq!(actor.display().ticks().1, &pose);
        command.blend_owner_time = Some(0.5);
        actor.tick(command, &Source).unwrap();
        let state = actor.display().ticks().1;
        assert_eq!(state.owner_time, 0.25);
        assert_eq!(state.blend_layer.as_ref().unwrap().owner_time, 0.5);
        assert_eq!(state.local[0][12], 15.0);
        assert_eq!(state.base[0][12], 30.0);
        let palette = actor
            .display()
            .sample(&Source, Sampling::Fractional, 0.5)
            .unwrap();
        assert!(palette[0][12] > state.palette[0][12]);
    }
}
