// SPDX-License-Identifier: GPL-3.0-or-later
//! T2394's typed player-motion boundary, derived from the finite T1675 REA contracts.
//!
//! This models the established value flow only. It does not choose D5C70 branches,
//! resolve collision, infer additive writers, or reinterpret any component as force
//! or physical velocity. The hybrid guest path remains the default until a separate
//! selection change is qualified.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionCandidate(pub [f32; 3]);

/// Candidate branches already selected by their upstream guards.
///
/// Branch selection is intentionally an input: the retained source contracts do
/// not establish which branch ran in the sampled motion, and this type does not
/// claim to reconstruct the surrounding geometry or animation services.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CandidatePath {
    /// State-2's ordinary basis/accumulator/additive construction.
    Ordinary {
        movement_mode: u32,
        scalar_q3: f32,
        scalar_q6: f32,
        basis_q4_xz: [f32; 2],
        basis_q5_xz: [f32; 2],
    },
    /// Geometry-derived path. The caller supplies the established normalized X/Z
    /// components after its geometry guard; this path does not add F4/F8/FC.
    Geometry {
        signed_drive: f32,
        scale: f32,
        divisor: f32,
        normalized_xz: [f32; 2],
        half_unit: f32,
    },
    /// The animation-copy gate returned the current player pose into Q1.
    AnimationCopy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionFrame {
    pub player: PlayerId,
    pub actor: ActorId,
    pub fixed_update: u64,
    pub player_state: u32,
    /// P+A8/AC/B0: the source position copied by the known pose publisher.
    pub player_position: [f32; 3],
    /// P+120/+124: accumulated planar quantity in the D2B80 path.
    pub accumulated_planar: [f32; 2],
    /// P+F4/F8/FC: additive triplet consumed by the ordinary candidate path.
    pub additive_position: [f32; 3],
    /// P+FC4/FC8: stored movement intent, kept distinct from the quantities above.
    pub stored_intent: [f32; 2],
    /// Optional body velocity, present only when the actor has an attached body.
    pub body_velocity: Option<[f32; 3]>,
}

impl MotionFrame {
    /// Apply the bounded D2B80 recurrence to the two accumulated planar words.
    ///
    /// The yaw projection, injection coefficient, and damping coefficient are
    /// explicit inputs so the operation cannot silently turn stored intent into
    /// velocity or assume a tick rate. Each component keeps the source's ordered
    /// binary32 multiply/add/multiply sequence.
    /// Verification tier: Tier 1, 2,500 fabricated D2B80 differential cases.
    pub fn accumulate_planar_input(
        &mut self,
        first_input_scalar: f32,
        yaw_projection_xz: [f32; 2],
        injection_coefficient: f32,
        damping_coefficient: f32,
    ) {
        for axis in 0..2 {
            let projected_input = yaw_projection_xz[axis] * first_input_scalar;
            let injected = projected_input * injection_coefficient;
            let accumulated = self.accumulated_planar[axis] + injected;
            self.accumulated_planar[axis] = accumulated * damping_coefficient;
        }
    }

    /// Build the candidate before the documented Y/collision/result adjustments.
    ///
    /// `additive_position` is read only on the ordinary path. The geometry branch
    /// and animation-copy branch intentionally bypass those additions.
    /// Verification tier: Tier 1 for the ordinary branch (2,500 lifted cases);
    /// Tier 0 contract tests for geometry and animation-copy branches.
    pub fn candidate_before_adjustments(&self, path: CandidatePath) -> MotionCandidate {
        match path {
            CandidatePath::AnimationCopy => MotionCandidate(self.player_position),
            CandidatePath::Geometry {
                signed_drive,
                scale,
                divisor,
                normalized_xz,
                half_unit,
            } => {
                let scaled_drive = signed_drive * scale;
                let drive = scaled_drive / divisor;
                MotionCandidate([
                    self.player_position[0] + drive * normalized_xz[0],
                    self.player_position[1] - half_unit,
                    self.player_position[2] + drive * normalized_xz[1],
                ])
            }
            CandidatePath::Ordinary {
                movement_mode,
                scalar_q3,
                scalar_q6,
                basis_q4_xz,
                basis_q5_xz,
            } => {
                let first_xz = if movement_mode == 1 {
                    self.accumulated_planar
                } else {
                    [basis_q5_xz[0] * scalar_q6, basis_q5_xz[1] * scalar_q6]
                };
                let second_xz = [basis_q4_xz[0] * scalar_q3, basis_q4_xz[1] * scalar_q3];
                MotionCandidate([
                    ((self.player_position[0] + first_xz[0]) + second_xz[0])
                        + self.additive_position[0],
                    self.player_position[1] + self.additive_position[1],
                    ((self.player_position[2] + first_xz[1]) + second_xz[1])
                        + self.additive_position[2],
                ])
            }
        }
    }

    /// D5EB0's post-dispatch multiplication of the separate additive triplet.
    /// Verification tier: Tier 0, finite-value contract test only.
    pub fn decay_additive_after_dispatch(&mut self, coefficient: f32) {
        for component in &mut self.additive_position {
            *component *= coefficient;
        }
    }

    /// Produce the actor pose publication after the candidate and its adjustments
    /// have been committed by their owner. State 0x400 skips the three stores.
    /// Verification tier: Tier 1, 2,500 fabricated D0CA0 differential cases.
    pub fn actor_pose_publication(&self) -> Option<ActorPosePublication> {
        (self.player_state != 0x400).then_some(ActorPosePublication {
            actor: self.actor,
            fixed_update: self.fixed_update,
            position: self.player_position,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorPosePublication {
    pub actor: ActorId,
    pub fixed_update: u64,
    pub position: [f32; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> MotionFrame {
        MotionFrame {
            player: PlayerId(1),
            actor: ActorId(2),
            fixed_update: 9,
            player_state: 2,
            player_position: [10.0, 20.0, 30.0],
            accumulated_planar: [1.0, -2.0],
            additive_position: [0.5, 1.5, -0.25],
            stored_intent: [0.0; 2],
            body_velocity: None,
        }
    }

    #[test]
    fn candidate_keeps_accumulator_intent_and_additive_sources_separate() {
        let f = frame();
        let ordinary = f.candidate_before_adjustments(CandidatePath::Ordinary {
            movement_mode: 1,
            scalar_q3: 3.0,
            scalar_q6: 4.0,
            basis_q4_xz: [0.25, 0.5],
            basis_q5_xz: [0.75, -0.5],
        });
        assert_eq!(ordinary.0, [12.25, 21.5, 29.25]);
        let geometry = f.candidate_before_adjustments(CandidatePath::Geometry {
            signed_drive: 2.0,
            scale: 3.0,
            divisor: 2.0,
            normalized_xz: [0.6, -0.8],
            half_unit: 0.5,
        });
        assert_eq!(geometry.0, [11.8, 19.5, 27.6]);
        assert_eq!(
            f.candidate_before_adjustments(CandidatePath::AnimationCopy)
                .0,
            f.player_position
        );
        assert_eq!(f.stored_intent, [0.0; 2]);
        assert_eq!(f.body_velocity, None);
    }

    #[test]
    fn damping_is_per_call_and_pose_publication_is_state_gated() {
        let mut f = frame();
        f.accumulate_planar_input(0.0, [0.4, -0.8], 0.01, 0.97);
        assert_eq!(f.accumulated_planar, [0.97, -1.94]);
        f.decay_additive_after_dispatch(0.9);
        assert_eq!(f.additive_position, [0.45, 1.349_999_9, -0.225]);
        let publication = f.actor_pose_publication().unwrap();
        assert_eq!(publication.actor, ActorId(2));
        assert_eq!(publication.fixed_update, 9);
        f.player_state = 0x400;
        assert_eq!(f.actor_pose_publication(), None);
    }
}
