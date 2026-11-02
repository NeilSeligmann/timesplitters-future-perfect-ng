//! Native render-facing placement slice of object 0xA1B80 (T2241).
//! Owns no simulation state, guest pointers, frame masks or bump allocation.
use crate::display::{ClipSource, DisplayInstance, Error, Sampling};
use crate::posemath::{Mat4, mat_mul};
use std::num::NonZeroUsize;

/// The object contract selects this explicitly, rather than passing raw flags.
#[derive(Clone, Copy, Debug)]
pub enum Nodes<'a> {
    /// No animation matrix array: one correction * object matrix.
    Rigid,
    /// Animated model without the transform-palette flag: copy object per node.
    Repeated(NonZeroUsize),
    /// Animated model with the flag: bone * object per node.
    Skinned(&'a [Mat4]),
}

/// Object placement for one view, before optional model correction.
#[derive(Clone, Copy, Debug)]
pub enum Space {
    /// Object flag +0x31 is nonzero: object transform is already in view space.
    View(Mat4),
    /// Otherwise 0xA1B80 multiplies object * camera before placing nodes.
    World { object: Mat4, camera: Mat4 },
}

/// Typed inputs published by the object/view owner. The optional correction is
/// the output of 0x8E630, not an invented inverse bind or additive aim layer.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub space: Space,
    pub correction: Option<Mat4>,
}

/// The four per-view states selected by object 0xA14B0. Reflection is
/// applied on the right of the retained orientation, before world translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reflection {
    None,
    X { plane: f32 },
    Y { plane: f32 },
    Z { plane: f32 },
}

/// Resolved orientation mode of object 0xA14B0. Angles are radians;
/// the native object producer owns conversion from runtime data units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orientation {
    Identity,
    Yaw(f32),
    Euler([f32; 3]),
    /// Mode 3 uses Euler only when its attached parent has mode 2.
    Attached {
        angles: [f32; 3],
        parent_is_euler: bool,
    },
}

/// Native owner of the retained orientation (original object +0x274).
/// Rebuilds atomically when inputs change; repeated views never mutate it.
#[derive(Clone, Debug)]
pub struct OrientationState {
    inputs: Option<(Orientation, bool)>,
    matrix: Mat4,
}

impl Default for OrientationState {
    fn default() -> Self {
        Self {
            inputs: None,
            matrix: identity_matrix(),
        }
    }
}

impl OrientationState {
    pub fn matrix(&self) -> &Mat4 {
        &self.matrix
    }

    pub fn update(&mut self, mode: Orientation, flip_x: bool) -> Result<(), Error> {
        if self.inputs == Some((mode, flip_x)) {
            return Ok(());
        }
        let angles = match mode {
            Orientation::Identity => [0.0; 3],
            Orientation::Yaw(y) => [0.0, y, 0.0],
            Orientation::Euler(angles) => angles,
            Orientation::Attached {
                angles,
                parent_is_euler,
            } => {
                if parent_is_euler {
                    angles
                } else {
                    [0.0, angles[1], 0.0]
                }
            }
        };
        if !angles.iter().all(|v| v.is_finite()) {
            return Err(Error::InvalidState);
        }
        let [x, y, z] = angles;
        let (sx, cx) = x.sin_cos();
        let (sy, cy) = y.sin_cos();
        let (sz, cz) = z.sin_cos();
        // 0x151650 row-major Euler layout, also reducing to 0x153440 for yaw.
        let mut matrix = [
            sy * sx * sz + cz * cy,
            sz * cx,
            -cz * sy + cy * sx * sz,
            0.0,
            -sz * cy + sy * sx * cz,
            cz * cx,
            cy * sx * cz + sz * sy,
            0.0,
            sy * cx,
            -sx,
            cy * cx,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ];
        // Mode 0 returns identity before the persistent flip branch.
        if flip_x && mode != Orientation::Identity {
            let mut mirror = identity_matrix();
            mirror[0] = -1.0;
            matrix = mat_mul(&mirror, &matrix);
        }
        self.matrix = matrix;
        self.inputs = Some((mode, flip_x));
        Ok(())
    }
}

/// Committed native object inputs. View reflection and camera remain per-view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectTick {
    pub orientation: Orientation,
    pub flip_x: bool,
    pub position: [f32; 3],
}

/// Owns animation and object placement together. No guest adapter is involved.
/// Clock stepping, events, modifiers and root-motion consumption belong to the
/// fixed simulation producer, which supplies the complete evaluated tick inputs.
#[derive(Clone, Debug)]
pub struct AnimatedObject {
    animation: DisplayInstance,
    orientation: OrientationState,
    tick: ObjectTick,
}

impl AnimatedObject {
    pub fn new(animation: DisplayInstance, tick: ObjectTick) -> Result<Self, Error> {
        let orientation = Self::prepare(tick)?;
        Ok(Self {
            animation,
            orientation,
            tick,
        })
    }

    fn prepare(tick: ObjectTick) -> Result<OrientationState, Error> {
        if !tick.position.iter().all(|value| value.is_finite()) {
            return Err(Error::InvalidState);
        }
        let mut orientation = OrientationState::default();
        orientation.update(tick.orientation, tick.flip_x)?;
        Ok(orientation)
    }

    /// Publish a simulation tick transactionally: missing streamed animation or
    /// invalid placement leaves both pose snapshots and object inputs unchanged.
    pub fn publish(
        &mut self,
        pose: crate::display::TickPose,
        tick: ObjectTick,
        source: &impl ClipSource,
    ) -> Result<(), Error> {
        let orientation = Self::prepare(tick)?;
        self.animation.evaluate_and_publish(pose, source)?;
        self.orientation = orientation;
        self.tick = tick;
        Ok(())
    }

    /// Commit the native fixed producer's complete result without evaluating it
    /// a second time. The original root displacement/base-save result survives
    /// exactly, while rendering only borrows its previous/current snapshots.
    pub fn publish_committed(
        &mut self,
        pose: crate::display::TickPose,
        tick: ObjectTick,
    ) -> Result<(), Error> {
        let orientation = Self::prepare(tick)?;
        self.animation.publish(pose)?;
        self.orientation = orientation;
        self.tick = tick;
        Ok(())
    }

    /// Native simulation command. Clocks, root displacement, base save and
    /// pose evaluation execute once; display sampling only borrows the result.
    pub fn advance_fixed(
        &mut self,
        command: crate::simulation::FixedCommand,
        tick: ObjectTick,
        source: &impl ClipSource,
    ) -> Result<[f32; 3], Error> {
        let orientation = Self::prepare(tick)?;
        let mut fixed = crate::simulation::FixedInstance::from_display(self.animation.clone());
        let delta = fixed.tick(command, source)?;
        self.animation = fixed.into_display();
        self.orientation = orientation;
        self.tick = tick;
        Ok(delta)
    }

    pub fn animation(&self) -> &DisplayInstance {
        &self.animation
    }

    pub fn tick(&self) -> ObjectTick {
        self.tick
    }

    /// Rebuild a palette for this display/view without advancing simulation.
    /// Position is committed simulation placement; camera interpolation is the
    /// camera owner's contract, not an invented object movement prediction.
    pub fn sample_for_view(
        &self,
        source: &impl ClipSource,
        sampling: Sampling,
        alpha: f32,
        reflection: Reflection,
        camera: Option<Mat4>,
        correction: ModelCorrection,
        use_palette: bool,
    ) -> Result<Vec<Mat4>, Error> {
        let object = ObjectTransform {
            orientation: *self.orientation.matrix(),
            position: self.tick.position,
            reflection,
            include_position: true,
        };
        let placement = Placement::from_object(&object, camera, correction)?;
        self.animation
            .sample_for_view(source, sampling, alpha, &placement, use_palette)
    }
}

/// Rust-owned placement inputs for the final stage of object 0xA14B0.
/// Orientation construction/cache invalidation belongs to the object owner.
#[derive(Clone, Copy, Debug)]
pub struct ObjectTransform {
    pub orientation: Mat4,
    pub position: [f32; 3],
    pub reflection: Reflection,
    pub include_position: bool,
}

impl ObjectTransform {
    pub fn matrix(&self) -> Result<Mat4, Error> {
        let mut matrix = self.orientation;
        let mut position = self.position;
        let reflected = match self.reflection {
            Reflection::None => None,
            Reflection::X { plane } => Some((0, plane)),
            Reflection::Y { plane } => Some((1, plane)),
            Reflection::Z { plane } => Some((2, plane)),
        };
        if !matrix.iter().all(|v| v.is_finite())
            || (self.include_position && !position.iter().all(|v| v.is_finite()))
            || reflected.is_some_and(|(_, plane)| !plane.is_finite())
        {
            return Err(Error::InvalidState);
        }
        if let Some((axis, plane)) = reflected {
            let mut mirror = identity_matrix();
            mirror[axis * 5] = -1.0;
            matrix = mat_mul(&matrix, &mirror);
            // Preserve original subtraction order, rather than replacing it
            // with 2 * plane - position (different floating-point rounding).
            position[axis] = plane - (position[axis] - plane);
        }
        if self.include_position {
            matrix[12..15].copy_from_slice(&position);
            if !position.iter().all(|v| v.is_finite()) {
                return Err(Error::InvalidState);
            }
        }
        Ok(matrix)
    }
}

/// Typed reconstruction of model record 0x8E630: absent correction skips
/// multiplication; enabled correction is translation, never inverse bind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModelCorrection {
    None,
    Translation([f32; 3]),
}

impl ModelCorrection {
    pub fn matrix(self) -> Result<Option<Mat4>, Error> {
        match self {
            Self::None => Ok(None),
            Self::Translation(position) => {
                if !position.iter().all(|v| v.is_finite()) {
                    return Err(Error::InvalidState);
                }
                let mut matrix = identity_matrix();
                matrix[12..15].copy_from_slice(&position);
                Ok(Some(matrix))
            }
        }
    }
}

fn identity_matrix() -> Mat4 {
    std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 })
}

impl Placement {
    /// Assemble typed object/model inputs for a view. A missing camera means
    /// the owner supplied view-space placement (original object byte +0x31).
    pub fn from_object(
        object: &ObjectTransform,
        camera: Option<Mat4>,
        correction: ModelCorrection,
    ) -> Result<Self, Error> {
        let object = object.matrix()?;
        let space = match camera {
            Some(camera) => {
                if !camera.iter().all(|v| v.is_finite()) {
                    return Err(Error::InvalidState);
                }
                Space::World { object, camera }
            }
            None => Space::View(object),
        };
        Ok(Self {
            space,
            correction: correction.matrix()?,
        })
    }

    /// Immutable render consumer of node matrices. Always rebuilds for this
    /// display sample; the original per-tick processed-view mask is not reused.
    pub fn place(&self, nodes: Nodes<'_>) -> Result<Vec<Mat4>, Error> {
        let object = match &self.space {
            Space::View(matrix) => *matrix,
            Space::World { object, camera } => mat_mul(object, camera),
        };
        let mut result = match nodes {
            Nodes::Rigid => vec![object],
            Nodes::Repeated(count) => vec![object; count.get()],
            Nodes::Skinned(palette) => {
                if palette.is_empty() {
                    return Err(Error::InvalidState);
                }
                palette.iter().map(|bone| mat_mul(bone, &object)).collect()
            }
        };
        if let Some(correction) = &self.correction {
            for matrix in &mut result {
                // 0xA1D60: correction is the LEFT operand, after bone placement.
                *matrix = mat_mul(correction, matrix);
            }
        }
        Ok(result)
    }
}

impl DisplayInstance {
    /// Samples owned animation state and consumes its palette at the native
    /// object/render boundary. The owner decides whether this model uses its
    /// animated palette or repeats the object transform (original flag +0x28/8).
    pub fn sample_for_view(
        &self,
        source: &impl ClipSource,
        sampling: Sampling,
        alpha: f32,
        placement: &Placement,
        use_palette: bool,
    ) -> Result<Vec<Mat4>, Error> {
        let palette = self.sample(source, sampling, alpha)?;
        if use_palette {
            placement.place(Nodes::Skinned(&palette))
        } else {
            let count = NonZeroUsize::new(palette.len()).ok_or(Error::InvalidState)?;
            placement.place(Nodes::Repeated(count))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{Skeleton, TickPose, Track};
    use crate::instance::ClipView;
    use crate::posemath::Sample;
    use crate::posemix::TreeRow;

    fn identity() -> Mat4 {
        std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 })
    }
    fn translation(x: f32, y: f32) -> Mat4 {
        let mut matrix = identity();
        matrix[12] = x;
        matrix[13] = y;
        matrix
    }
    fn scale(x: f32, y: f32) -> Mat4 {
        let mut matrix = identity();
        matrix[0] = x;
        matrix[5] = y;
        matrix
    }

    #[test]
    fn world_placement_multiplies_object_then_camera() {
        let placement = Placement {
            space: Space::World {
                object: translation(3.0, 4.0),
                camera: scale(2.0, 3.0),
            },
            correction: None,
        };
        assert_eq!(
            placement.place(Nodes::Rigid).unwrap(),
            vec![translation(6.0, 12.0).map_with_scale(2.0, 3.0)]
        );
    }

    // Keep expected matrix construction independent of the multiplication routine.
    trait ExpectedScale {
        fn map_with_scale(self, x: f32, y: f32) -> Self;
    }
    impl ExpectedScale for Mat4 {
        fn map_with_scale(mut self, x: f32, y: f32) -> Self {
            self[0] = x;
            self[5] = y;
            self
        }
    }

    #[test]
    fn correction_is_left_operand_after_bone_and_object() {
        let palette = [translation(1.0, 2.0), translation(3.0, 4.0)];
        let before = palette;
        let placement = Placement {
            space: Space::View(scale(2.0, 3.0)),
            correction: Some(translation(5.0, 7.0)),
        };
        let result = placement.place(Nodes::Skinned(&palette)).unwrap();
        assert_eq!(result[0], translation(12.0, 27.0).map_with_scale(2.0, 3.0));
        assert_eq!(result[1], translation(16.0, 33.0).map_with_scale(2.0, 3.0));
        assert_eq!(palette, before);
    }

    #[test]
    fn repeated_and_rigid_modes_do_not_invent_bones() {
        let placement = Placement {
            space: Space::View(translation(2.0, 4.0)),
            correction: Some(scale(3.0, 5.0)),
        };
        let expected = translation(2.0, 4.0).map_with_scale(3.0, 5.0);
        assert_eq!(placement.place(Nodes::Rigid).unwrap(), vec![expected]);
        assert_eq!(
            placement
                .place(Nodes::Repeated(NonZeroUsize::new(3).unwrap()))
                .unwrap(),
            vec![expected; 3]
        );
        assert_eq!(
            placement.place(Nodes::Skinned(&[])),
            Err(Error::InvalidState)
        );
    }

    #[test]
    fn reflections_use_per_view_planes_and_right_multiplication() {
        // Rotation sends local X to world Y: reflecting world X must negate
        // column X, not row X. This catches a reversed multiplication order.
        let mut orientation = identity();
        orientation[0] = 0.0;
        orientation[1] = 1.0;
        orientation[4] = -1.0;
        orientation[5] = 0.0;
        for (reflection, axis) in [
            (Reflection::X { plane: 10.0 }, 0),
            (Reflection::Y { plane: 10.0 }, 1),
            (Reflection::Z { plane: 10.0 }, 2),
        ] {
            let object = ObjectTransform {
                orientation,
                position: [2.0, 3.0, 4.0],
                reflection,
                include_position: true,
            };
            let result = object.matrix().unwrap();
            let mut expected = orientation;
            for row in 0..4 {
                expected[row * 4 + axis] *= -1.0;
            }
            let mut position = object.position;
            position[axis] = 20.0 - position[axis];
            expected[12..15].copy_from_slice(&position);
            assert_eq!(result, expected);
            assert_eq!(object.orientation, orientation);
        }
    }

    #[test]
    fn orientation_only_reflects_without_inserting_world_position() {
        let object = ObjectTransform {
            orientation: translation(7.0, 8.0),
            position: [f32::NAN; 3],
            reflection: Reflection::X { plane: 10.0 },
            include_position: false,
        };
        let matrix = object.matrix().unwrap();
        assert_eq!(matrix[0], -1.0);
        assert_eq!(&matrix[12..15], &[-7.0, 8.0, 0.0]);
    }

    #[test]
    fn typed_model_correction_flows_through_palette_placement() {
        let object = ObjectTransform {
            orientation: scale(2.0, 3.0),
            position: [4.0, 5.0, 6.0],
            reflection: Reflection::None,
            include_position: true,
        };
        let placement =
            Placement::from_object(&object, None, ModelCorrection::Translation([1.0, 2.0, 3.0]))
                .unwrap();
        let result = placement.place(Nodes::Rigid).unwrap();
        assert_eq!(&result[0][12..15], &[6.0, 11.0, 9.0]);
        assert_eq!(ModelCorrection::None.matrix().unwrap(), None);
        assert_eq!(
            ModelCorrection::Translation([f32::NAN; 3]).matrix(),
            Err(Error::InvalidState)
        );
    }

    #[test]
    fn transform_rejects_reflection_overflow_and_invalid_camera() {
        let object = ObjectTransform {
            orientation: identity(),
            position: [-f32::MAX, 0.0, 0.0],
            reflection: Reflection::X { plane: f32::MAX },
            include_position: true,
        };
        assert_eq!(object.matrix(), Err(Error::InvalidState));
        let object = ObjectTransform {
            position: [0.0; 3],
            reflection: Reflection::None,
            ..object
        };
        assert!(
            Placement::from_object(&object, Some([f32::NAN; 16]), ModelCorrection::None).is_err()
        );
    }

    #[test]
    fn native_orientation_resolves_parent_and_persistent_flip() {
        let mut state = OrientationState::default();
        let half = std::f32::consts::FRAC_PI_2;
        state.update(Orientation::Yaw(half), false).unwrap();
        let yaw = *state.matrix();
        assert!((yaw[2] + 1.0).abs() < 1e-6);
        assert!((yaw[8] - 1.0).abs() < 1e-6);
        state
            .update(
                Orientation::Attached {
                    angles: [half, half, half],
                    parent_is_euler: false,
                },
                false,
            )
            .unwrap();
        assert_eq!(*state.matrix(), yaw);
        state
            .update(
                Orientation::Attached {
                    angles: [half, half, half],
                    parent_is_euler: true,
                },
                false,
            )
            .unwrap();
        let euler = *state.matrix();
        assert!((euler[0] - 1.0).abs() < 1e-6);
        assert!((euler[9] + 1.0).abs() < 1e-6);
        state.update(Orientation::Euler([half; 3]), true).unwrap();
        let flipped = *state.matrix();
        for i in 0..16 {
            assert!((flipped[i] - if i < 4 { -euler[i] } else { euler[i] }).abs() < 1e-6);
        }
        state.update(Orientation::Identity, true).unwrap();
        assert_eq!(*state.matrix(), identity());
    }

    #[test]
    fn native_orientation_rejects_invalid_update_atomically() {
        let mut state = OrientationState::default();
        state.update(Orientation::Yaw(0.5), true).unwrap();
        let before = *state.matrix();
        assert_eq!(
            state.update(Orientation::Euler([f32::NAN; 3]), false),
            Err(Error::InvalidState)
        );
        assert_eq!(*state.matrix(), before);
        state.update(Orientation::Yaw(0.5), true).unwrap();
        assert_eq!(*state.matrix(), before);
        // Rebuilding a changed mode does not accumulate the persistent flip.
        state.update(Orientation::Yaw(0.0), true).unwrap();
        assert_eq!(state.matrix()[0], -1.0);
    }

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
                bits: [0.0, key as f32, 0.0, 0.0, 0.0, 0.0, 1.0].map(f32::to_bits),
            })
        }
    }
    fn instance() -> DisplayInstance {
        DisplayInstance::new(
            TickPose {
                generation: 1,
                owner_time: 0.0,
                time_step: 1.0 / 60.0,
                track: Track {
                    clip: 1,
                    offset: 0.0,
                    layer_time: 0.0,
                    policy: (0, 0),
                    snap: false,
                },
                blend_layer: None,
                root: [0.0; 9],
                local: vec![identity()],
                base: vec![identity()],
                palette: vec![identity()],
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
    fn fractional_render_rebuilds_each_view_without_advancing_ticks() {
        let instance = instance();
        let before = (instance.ticks().0.clone(), instance.ticks().1.clone());
        for alpha in [0.25, 0.5, 0.75, 1.0] {
            for view_y in [10.0, 20.0] {
                let placement = Placement {
                    space: Space::View(translation(0.0, view_y)),
                    correction: None,
                };
                let palette = instance
                    .sample_for_view(&Source, Sampling::Fractional, alpha, &placement, true)
                    .unwrap();
                assert!((palette[0][13] - (view_y + alpha)).abs() < 1e-6);
            }
        }
        assert_eq!(instance.ticks(), (&before.0, &before.1));
    }

    fn object_tick(y: f32) -> ObjectTick {
        ObjectTick {
            orientation: Orientation::Identity,
            flip_x: false,
            position: [0.0, y, 0.0],
        }
    }

    #[test]
    fn object_publication_is_atomic_across_pose_and_transform() {
        struct Missing;
        impl ClipSource for Missing {
            fn view(&self, _: u32) -> Option<ClipView<'_>> {
                None
            }
            fn sample(&self, _: u32, _: usize, _: i32) -> Option<Sample> {
                None
            }
        }
        let mut object = AnimatedObject::new(instance(), object_tick(10.0)).unwrap();
        let before = object.animation().ticks().1.clone();
        assert_eq!(
            object.publish(before.clone(), object_tick(20.0), &Missing),
            Err(Error::MissingClip)
        );
        assert_eq!(object.tick(), object_tick(10.0));
        assert_eq!(object.animation().ticks(), (&before, &before));
        assert_eq!(
            object.publish(before.clone(), object_tick(f32::NAN), &Source),
            Err(Error::InvalidState)
        );
        assert_eq!(object.animation().ticks(), (&before, &before));
        object
            .publish(before.clone(), object_tick(20.0), &Source)
            .unwrap();
        assert_eq!(object.tick(), object_tick(20.0));
        assert_eq!(object.animation().ticks().0, &before);
        assert_eq!(object.animation().ticks().1.root, before.root);
    }

    #[test]
    fn object_owner_connects_fixed_publication_to_immutable_fractional_views() {
        let mut object = AnimatedObject::new(instance(), object_tick(10.0)).unwrap();
        let pose = object.animation().ticks().1.clone();
        object.publish(pose, object_tick(20.0), &Source).unwrap();
        let before = (
            object.animation().ticks().0.clone(),
            object.animation().ticks().1.clone(),
        );
        for alpha in [0.25, 0.5, 0.75, 1.0] {
            for view in [5.0, 10.0] {
                let result = object
                    .sample_for_view(
                        &Source,
                        Sampling::Fractional,
                        alpha,
                        Reflection::None,
                        Some(translation(0.0, view)),
                        ModelCorrection::None,
                        true,
                    )
                    .unwrap();
                assert!((result[0][13] - (20.0 + view + alpha)).abs() < 1e-6);
            }
        }
        let fixed = object
            .sample_for_view(
                &Source,
                Sampling::Fixed,
                0.75,
                Reflection::None,
                None,
                ModelCorrection::None,
                true,
            )
            .unwrap();
        assert_eq!(fixed[0][13], 20.0);
        assert_eq!(object.animation().ticks(), (&before.0, &before.1));
        assert_eq!(object.tick(), object_tick(20.0));
    }

    #[test]
    fn fixed_and_repeated_paths_retain_original_placement() {
        let instance = instance();
        let placement = Placement {
            space: Space::View(translation(0.0, 10.0)),
            correction: None,
        };
        assert_eq!(
            instance
                .sample_for_view(&Source, Sampling::Fixed, 0.75, &placement, true)
                .unwrap(),
            vec![translation(0.0, 10.0)]
        );
        assert_eq!(
            instance
                .sample_for_view(&Source, Sampling::Fractional, 0.75, &placement, false)
                .unwrap(),
            vec![translation(0.0, 10.0)]
        );
        assert_eq!(
            instance.sample_for_view(&Source, Sampling::Fractional, f32::NAN, &placement, true),
            Err(Error::InvalidFraction)
        );
    }
}
