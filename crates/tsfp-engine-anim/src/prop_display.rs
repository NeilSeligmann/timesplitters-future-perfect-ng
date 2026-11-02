//! Native prop presentation over the reconstructed 5F040/5F260 contract.
//! The owner supplies committed clocks; rendering never steps simulation.
use crate::display::{ClipSource, Error, Sampling};
use crate::posemath::Mat4;

/// 5F040 moves the model separately; 5F260 puts movement in the matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Model,
    Matrix,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropTick {
    pub generation: u64,
    pub clip: u32,
    pub time: f32,
    pub time_step: f32,
}

/// Model-space translation for Model placement, zero for Matrix placement.
/// The model's world transform remains the object owner's responsibility.
#[derive(Clone, Debug, PartialEq)]
pub struct PropPose {
    pub position: [f32; 3],
    pub matrices: Vec<Mat4>,
}

#[derive(Clone, Debug)]
pub struct PropDisplay {
    previous: PropTick,
    current: PropTick,
    committed: PropPose,
    placement: Placement,
    matrix_count: usize,
}

impl PropDisplay {
    /// Evaluate the initial committed pose from runtime clip data, not an
    /// identity placeholder. The model owner supplies its output matrix count.
    pub fn new(
        tick: PropTick,
        placement: Placement,
        matrix_count: usize,
        source: &impl ClipSource,
    ) -> Result<Self, Error> {
        let committed = evaluate(&tick, placement, matrix_count, source, 0.0)?;
        Ok(Self {
            previous: tick.clone(),
            current: tick,
            committed,
            placement,
            matrix_count,
        })
    }

    /// Publish only after all samples succeed. Clock advancement, event
    /// delivery and model movement are separate fixed-tick owner operations.
    pub fn publish(&mut self, tick: PropTick, source: &impl ClipSource) -> Result<(), Error> {
        let pose = evaluate(&tick, self.placement, self.matrix_count, source, 0.0)?;
        self.previous = std::mem::replace(&mut self.current, tick);
        self.committed = pose;
        Ok(())
    }

    pub fn ticks(&self) -> (&PropTick, &PropTick) {
        (&self.previous, &self.current)
    }

    pub fn committed(&self) -> &PropPose {
        &self.committed
    }

    pub fn sample(
        &self,
        source: &impl ClipSource,
        mode: Sampling,
        alpha: f32,
    ) -> Result<PropPose, Error> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(Error::InvalidFraction);
        }
        if mode == Sampling::Fixed
            || alpha == 0.0
            || self.current.time_step == 0.0
            || self.previous.generation != self.current.generation
            || self.previous.clip != self.current.clip
        {
            return Ok(self.committed.clone());
        }
        evaluate(
            &self.current,
            self.placement,
            self.matrix_count,
            source,
            alpha,
        )
    }
}

fn evaluate(
    tick: &PropTick,
    placement: Placement,
    count: usize,
    source: &impl ClipSource,
    alpha: f32,
) -> Result<PropPose, Error> {
    let time = tick.time + alpha * tick.time_step;
    if count == 0 || !tick.time.is_finite() || !tick.time_step.is_finite() || !time.is_finite() {
        return Err(Error::InvalidState);
    }
    let view = source.view(tick.clip).ok_or(Error::MissingClip)?;
    if view.nodes == 0 || !view.duration.is_finite() {
        return Err(Error::InvalidState);
    }
    // Retain the instance clock's established upper-clamp/negative-wrap rule.
    let time =
        crate::advance::advance_time(view.duration, time, 0.0, tick.time).ok_or(Error::InvalidState)?;
    let (low, high) = crate::keybracket::key_bracket(view.keys, view.duration, time)
        .ok_or(Error::InvalidState)?;
    let first = source
        .sample(tick.clip, 0, low)
        .ok_or(Error::MissingSample)?;
    let second = source
        .sample(tick.clip, 0, high)
        .ok_or(Error::MissingSample)?;
    let sampler = |key| if key == low { first } else { second };
    match placement {
        Placement::Model => {
            let mut matrices = vec![[0.0; 16]; count];
            let position =
                crate::prop::prop_body_pose(view.keys, view.duration, time, &mut matrices, sampler)
                    .ok_or(Error::InvalidState)?;
            Ok(PropPose { position, matrices })
        }
        Placement::Matrix => {
            let matrix = crate::prop::state_matrix(view.keys, view.duration, time, sampler)
                .ok_or(Error::InvalidState)?;
            Ok(PropPose {
                position: [0.0; 3],
                matrices: vec![matrix; count],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{instance::ClipView, posemath::Sample};
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
            let angle = key as f32 * std::f32::consts::FRAC_PI_4;
            Some(Sample {
                bits: [key as f32, 0.0, 0.0, 0.0, 0.0, angle.sin(), angle.cos()].map(f32::to_bits),
            })
        }
    }
    fn tick() -> PropTick {
        PropTick {
            generation: 1,
            clip: 7,
            time: 0.0,
            time_step: 1.0 / 60.0,
        }
    }

    #[test]
    fn model_position_and_bone_rotation_remain_separate() {
        let display = PropDisplay::new(tick(), Placement::Model, 3, &Source).unwrap();
        let before = display.ticks();
        let before = (before.0.clone(), before.1.clone());
        for alpha in [0.25, 0.5, 0.75, 1.0] {
            let pose = display
                .sample(&Source, Sampling::Fractional, alpha)
                .unwrap();
            assert!((pose.position[0] - alpha).abs() < 1e-6);
            assert_eq!(pose.matrices[0][12..15], [0.0; 3]);
            assert!(pose.matrices.windows(2).all(|pair| pair[0] == pair[1]));
        }
        let pose = display.sample(&Source, Sampling::Fractional, 0.5).unwrap();
        assert!((pose.matrices[0][0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        assert_eq!(display.ticks(), (&before.0, &before.1));
        assert_eq!(display.committed().position, [0.0; 3]);
    }

    #[test]
    fn matrix_placement_does_not_double_apply_translation() {
        let display = PropDisplay::new(tick(), Placement::Matrix, 1, &Source).unwrap();
        let pose = display.sample(&Source, Sampling::Fractional, 0.5).unwrap();
        assert_eq!(pose.position, [0.0; 3]);
        assert!((pose.matrices[0][12] - 0.5).abs() < 1e-6);
        for (mode, alpha) in [(Sampling::Fixed, 0.5), (Sampling::Fractional, 0.0)] {
            assert_eq!(
                display.sample(&Source, mode, alpha).unwrap(),
                *display.committed()
            );
        }
    }

    #[test]
    fn publish_is_atomic_and_restart_holds() {
        let mut display = PropDisplay::new(tick(), Placement::Model, 1, &Source).unwrap();
        let mut next = tick();
        next.clip = 8;
        assert_eq!(display.publish(next, &Source), Err(Error::MissingClip));
        assert_eq!(display.ticks(), (&tick(), &tick()));
        let mut next = tick();
        next.time = 0.5 / 60.0;
        next.generation += 1;
        display.publish(next, &Source).unwrap();
        assert_eq!(
            display.sample(&Source, Sampling::Fractional, 0.75).unwrap(),
            *display.committed()
        );
        assert!((display.committed().position[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rejects_invalid_inputs_and_clamps_clip_end() {
        assert!(matches!(
            PropDisplay::new(tick(), Placement::Model, 0, &Source),
            Err(Error::InvalidState)
        ));
        let display = PropDisplay::new(tick(), Placement::Model, 1, &Source).unwrap();
        assert_eq!(
            display.sample(&Source, Sampling::Fractional, f32::NAN),
            Err(Error::InvalidFraction)
        );
        let mut end = tick();
        end.time = 0.75 / 60.0;
        let display = PropDisplay::new(end, Placement::Model, 1, &Source).unwrap();
        assert_eq!(
            display
                .sample(&Source, Sampling::Fractional, 1.0)
                .unwrap()
                .position,
            [1.0, 0.0, 0.0]
        );
    }
}
