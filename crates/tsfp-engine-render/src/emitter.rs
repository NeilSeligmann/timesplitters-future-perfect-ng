// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle motion step of the effects system (T1898): the per frame update of one particle of
//! an emitter. See [`Renderer::step_particle`].
//!
//! Evidence class: differential versus the lifted original, see docs/t-render-slice.md.
use crate::{
    Renderer,
    device::{HostArg, HostFunction, RenderDevice, RenderHost},
    mem::{GuestAccess, truncate_to_i32},
    memory::GuestMemory,
    particle::random_unit_float,
    visual_effects::ParticleState,
};
use alloc::vec;

/// Original addresses of the functions ported in this module (T1898, see docs/t-render-slice.md).
pub const ROOTS: [u32; 1] = [0x10d070];

const SINE_TABLE: u32 = 0x007A_38A0;
const CONST_ONE: u32 = 0x0047_5C78;
const CONST_HALF: u32 = 0x0047_5CD4;
const CONST_TABLE_SCALE: u32 = 0x0047_5CE0;
const CONST_SWIRL_TIME_SCALE: u32 = 0x004E_6158;
const CONST_SWIRL_LIFT_SCALE: u32 = 0x0047_5D10;
const CONST_SWIRL_AMPLITUDE_SCALE: u32 = 0x004E_6154;
const CONST_SWIRL_LIFT: u32 = 0x004E_615C;
const CONST_GROUND_EPSILON: u32 = 0x0047_5D1C;
const CONST_NORMALISE_FLOOR: u32 = 0x0047_65F4;

/// Particle record fields (offsets), INFERRED from the step.
mod field {
    pub const POSITION: u32 = 0x00;
    pub const VELOCITY: u32 = 0x0C;
    pub const SPEED_SCALE: u32 = 0x18;
    pub const SWIRL_BASE: u32 = 0x1C;
    pub const SWIRL_RATE: u32 = 0x20;
    pub const LIFETIME: u32 = 0x24;
    pub const AGE: u32 = 0x28;
    pub const SWIRL_FRACTION: u32 = 0x30;
    pub const AXIS_A: u32 = 0x34;
    pub const AXIS_B: u32 = 0x40;
    pub const SPIN: u32 = 0x4C;
}

impl<M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'_, M, D, H> {
    fn float(&self, address: u32) -> f32 {
        self.mem.read_f32(address)
    }

    /// The sine table lookup of the swirl: `table[index & 0x7FF]`.
    fn sine(&self, index: i32) -> f32 {
        self.float(SINE_TABLE.wrapping_add(((index & 0x7FF) as u32) << 2))
    }

    /// Normalise by the lifted reciprocal square root. The original uses `rsqrtss`, the lifted
    /// C models it as the exact `1 / sqrt`, which this keeps (INFERRED, hardware `rsqrtss`
    /// is a 12 bit estimate, so this is a Tier 3 fidelity question).
    fn reciprocal_root(length_squared: f32) -> f32 {
        1.0f32 / crate::render_math::sqrt32(length_squared)
    }

    /// 0x10D070 game_particle_emitter_step_flag_0x20000_swirl_emission_with_sine_wobble_axis_rotation_and_trail_spawn.
    /// Register convention: EBX emitter, ESI particle, stack `dt`. Applies the swirl (definition
    /// flag 8), the emitter drift, integrates the position, adds randomised drag, optionally
    /// bounces on the ground plane at `emitter + 0x84` and rotates the orientation axes.
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // negated forms keep the NaN branches of `comiss`
    pub fn step_particle(&mut self, emitter: u32, particle: u32, dt: f32) {
        let definition = self.mem.read32(
            self.mem
                .read32(emitter.wrapping_add(0xC))
                .wrapping_add(0x4C),
        );
        let emitter_flags = |renderer: &Self| renderer.mem.read32(emitter.wrapping_add(4));
        let p = |offset: u32| particle.wrapping_add(offset);
        let d = |offset: u32| definition.wrapping_add(offset);
        let drag_scale = self.float(emitter.wrapping_add(0x6C)) * dt;
        let velocity = [
            self.float(p(field::VELOCITY)),
            self.float(p(field::VELOCITY + 4)),
            self.float(p(field::VELOCITY + 8)),
        ];

        // The documented no-swirl/no-drag/no-collision case is a native owned-state update:
        // the original still consumes three random values in its drag helper, then integrates
        // position and increments age. Keep every other flag/parameter combination on the full
        // guest-backed reference implementation below.
        let mode = self.mem.read32(d(0xC));
        if self.mem.read8(definition) & 0x18 == 0
            && emitter_flags(self) & 0x2000_400C == 0
            && mode != 5
            && mode != 6
            && self.float(d(0x88)) == 0.0
            && self.float(d(0x98)) == 0.0
            && self.float(p(field::SPEED_SCALE)) == 0.0
            && velocity.iter().all(|value| {
                value.is_finite() && value.to_bits() != (-0.0f32).to_bits()
            })
            && dt.is_finite()
            && drag_scale.is_finite()
            && self.float(CONST_HALF) == 0.5
            && self.float(CONST_ONE) == 1.0
        {
            for _ in 0..3 {
                let _ = random_unit_float(self.mem);
            }
            let mut owned = ParticleState {
                position: [
                    self.float(p(field::POSITION)),
                    self.float(p(field::POSITION + 4)),
                    self.float(p(field::POSITION + 8)),
                ],
                velocity,
                age: self.float(p(field::AGE)),
                lifetime: self.float(p(field::LIFETIME)),
            };
            owned.advance_linear(dt);
            for axis in 0..3 {
                self.mem
                    .write_f32(p(field::POSITION + 4 * axis as u32), owned.position[axis]);
            }
            self.mem.write_f32(p(field::AGE), owned.age);
            return;
        }

        let one = self.float(CONST_ONE);
        let half = self.float(CONST_HALF);

        if self.mem.read8(definition) & 8 != 0 {
            let denominator = self.float(d(8)) + self.float(d(4));
            let age = self.float(p(field::AGE));
            let swirl_phase = self.float(p(field::SWIRL_RATE)) * self.float(p(field::AGE))
                + self.float(p(field::SWIRL_BASE));
            let table_scale = self.float(CONST_TABLE_SCALE);
            let mut time = age / denominator;
            time += self.float(p(field::SWIRL_BASE));
            time *= self.float(CONST_SWIRL_TIME_SCALE);
            let mut amplitude =
                self.float(d(0x64)) * self.float(p(field::SWIRL_FRACTION)) + self.float(d(0x60));
            let amplitude_phase = time;
            let lift_phase = time * self.float(CONST_SWIRL_LIFT_SCALE);
            let phase_index = truncate_to_i32(swirl_phase * table_scale + half);
            let lift_index = truncate_to_i32(lift_phase * table_scale + half);
            let amplitude_index = truncate_to_i32(amplitude_phase * table_scale + half);
            amplitude *= amplitude;
            amplitude *= self.float(CONST_SWIRL_AMPLITUDE_SCALE);
            amplitude *= self.sine(amplitude_index);
            amplitude *= self.float(p(field::SPEED_SCALE));
            let velocity_x = self.sine(phase_index) * amplitude + self.float(p(field::VELOCITY));
            self.mem.write_f32(p(field::VELOCITY), velocity_x);
            let mut velocity_y = self.sine(lift_index) + one;
            velocity_y *= half;
            velocity_y *= self.float(CONST_SWIRL_LIFT);
            velocity_y += self.float(p(field::VELOCITY + 4));
            self.mem.write_f32(p(field::VELOCITY + 4), velocity_y);
            let velocity_z = self.sine(phase_index.wrapping_add(0x200)) * amplitude
                + self.float(p(field::VELOCITY + 8));
            self.mem.write_f32(p(field::VELOCITY + 8), velocity_z);
        }
        if emitter_flags(self) & 0x2000_4000 != 0 {
            for axis in 0..3u32 {
                let drift = self.float(emitter.wrapping_add(0x98 + 4 * axis));
                let velocity = self.float(p(field::VELOCITY + 4 * axis)) + drift;
                self.mem.write_f32(p(field::VELOCITY + 4 * axis), velocity);
            }
        }
        for axis in 0..3u32 {
            let moved = dt * self.float(p(field::VELOCITY + 4 * axis))
                + self.float(p(field::POSITION + 4 * axis));
            self.mem.write_f32(p(field::POSITION + 4 * axis), moved);
        }
        self.randomised_drag(definition, particle, drag_scale, dt);
        if emitter_flags(self) & 0x4000 != 0 {
            for axis in 0..3u32 {
                let velocity = self.float(p(field::VELOCITY + 4 * axis))
                    - self.float(emitter.wrapping_add(0x98 + 4 * axis));
                self.mem.write_f32(p(field::VELOCITY + 4 * axis), velocity);
            }
        }
        if (mode == 5 || mode == 6) && emitter_flags(self) & 0x8000 == 0 {
            self.rotate_orientation_axes(particle, dt);
        }
        if emitter_flags(self) & 4 == 0 {
            return;
        }
        let ground = self.float(emitter.wrapping_add(0x84)) + self.float(CONST_GROUND_EPSILON);
        if !(ground > self.float(p(4))) || !(dt > 0.0) {
            return;
        }
        self.mem.write_f32(p(4), ground);
        let restitution = |renderer: &Self| one - renderer.float(d(0x8C));
        let rebound = 0.0f32 - restitution(self) * self.float(p(field::VELOCITY + 4));
        self.mem.write_f32(p(field::VELOCITY + 4), rebound);
        let side_x = restitution(self) * self.float(p(field::VELOCITY));
        self.mem.write_f32(p(field::VELOCITY), side_x);
        let side_z = restitution(self) * self.float(p(field::VELOCITY + 8));
        self.mem.write_f32(p(field::VELOCITY + 8), side_z);
        if mode == 5 || mode == 6 {
            let vertical = self.float(p(field::VELOCITY + 4)).abs();
            if self.float(CONST_GROUND_EPSILON) > vertical {
                self.settle_orientation(particle, one);
            }
        }
        if self.mem.read8(definition) & 0x10 != 0 {
            let age = self.float(p(field::AGE));
            let eased = (self.float(p(field::LIFETIME)) - age) * half + age;
            self.mem.write_f32(p(field::AGE), eased);
        }
        if emitter_flags(self) & 8 != 0 {
            self.host
                .call(HostFunction::ParticleTrail, &[emitter, particle]);
        }
    }

    /// Three randomised drag updates, x87 (53 bit) arithmetic: each axis velocity becomes
    /// `(1 - scale * damping) * (velocity + (random - 0.5) * jitter * scale)`. The age advances
    /// by `dt` between the second and third update.
    fn randomised_drag(&mut self, definition: u32, particle: u32, drag_scale: f32, dt: f32) {
        let jitter = f64::from(self.float(definition.wrapping_add(0x98)));
        let damping = f64::from(drag_scale) * f64::from(self.float(definition.wrapping_add(0x88)));
        let half = f64::from(self.float(CONST_HALF));
        let one = f64::from(self.float(CONST_ONE));
        let keep = one - damping;
        let velocity = |axis: u32| particle.wrapping_add(field::VELOCITY + 4 * axis);
        // x axis, the y velocity is lowered by the gravity share first
        let random = random_unit_float(self.mem);
        let gravity = self.float(particle.wrapping_add(field::SPEED_SCALE)) * drag_scale;
        let lowered = self.float(velocity(1)) - gravity;
        let mut value = (random - half) * jitter;
        value *= f64::from(drag_scale);
        value += f64::from(self.float(velocity(0)));
        self.mem.write_f32(velocity(0), value as f32);
        self.mem.write_f32(velocity(1), lowered);
        self.mem.write_f32(velocity(0), (keep * value) as f32);
        // y axis
        let random = random_unit_float(self.mem);
        let mut value = (random - half) * jitter;
        value *= f64::from(drag_scale);
        value += f64::from(self.float(velocity(1)));
        self.mem.write_f32(velocity(1), value as f32);
        self.mem.write_f32(velocity(1), (keep * value) as f32);
        // z axis and the age
        let random = random_unit_float(self.mem);
        let aged = self.float(particle.wrapping_add(field::AGE)) + dt;
        let mut value = (random - half) * jitter;
        value *= f64::from(drag_scale);
        value += f64::from(self.float(velocity(2)));
        self.mem.write_f32(velocity(2), value as f32);
        self.mem.write_f32(particle.wrapping_add(field::AGE), aged);
        self.mem.write_f32(velocity(2), (keep * value) as f32);
    }

    /// Spin: rotate the two orientation axes about the spin axis `P+0x4C..` by an angle
    /// proportional to its length. The helpers build the rotation and apply it.
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // negated form keeps the NaN branch of `comiss`
    fn rotate_orientation_axes(&mut self, particle: u32, dt: f32) {
        let spin = |renderer: &Self, axis: u32| {
            renderer.float(particle.wrapping_add(field::SPIN + 4 * axis))
        };
        let length_squared = spin(self, 0) * spin(self, 0)
            + spin(self, 1) * spin(self, 1)
            + spin(self, 2) * spin(self, 2);
        if !(length_squared > 0.0) {
            return;
        }
        let length = crate::render_math::sqrt64(f64::from(length_squared)) as f32;
        let inverse = self.float(CONST_ONE) / length;
        let angle = (f64::from(length) * f64::from(dt)) as f32;
        let axis = [
            inverse * spin(self, 0),
            inverse * spin(self, 1),
            inverse * spin(self, 2),
        ]
        .map(f32::to_bits);
        let mut buffers = [vec![0u32; 16], axis.to_vec()];
        self.host.call_buffers(
            HostFunction::RotationFromAxisAngle,
            &[
                HostArg::Buffer(0),
                HostArg::Buffer(1),
                HostArg::Word(angle.to_bits()),
            ],
            &mut buffers,
        );
        for target in [field::AXIS_A, field::AXIS_B] {
            self.host.call_buffers(
                HostFunction::TransformVector,
                &[
                    HostArg::Buffer(0),
                    HostArg::Word(particle.wrapping_add(target)),
                ],
                &mut buffers[..1],
            );
        }
    }

    /// The resting case of the bounce: the orientation axes lose their sideways parts and are
    /// renormalised, the spin stops.
    fn settle_orientation(&mut self, particle: u32, one: f32) {
        let p = |offset: u32| particle.wrapping_add(offset);
        let floor = self.float(CONST_NORMALISE_FLOOR);
        for offset in [0x34, 0x44, 0x3C, 0x48] {
            self.mem.write_f32(p(offset), 0.0);
        }
        let length_squared = self.float(p(0x40)) * self.float(p(0x40));
        if length_squared > floor {
            let inverse = Self::reciprocal_root(length_squared) * one;
            for offset in [0x40, 0x44, 0x48] {
                let scaled = self.float(p(offset)) * inverse;
                self.mem.write_f32(p(offset), scaled);
            }
        }
        let (a, b, c) = (
            self.float(p(0x34)),
            self.float(p(0x38)),
            self.float(p(0x3C)),
        );
        let length_squared = a * a + b * b + c * c;
        if length_squared > floor {
            let inverse = Self::reciprocal_root(length_squared) * one;
            let scaled = [inverse * a, inverse * b, inverse * c];
            for (offset, value) in [0x34, 0x38, 0x3C].into_iter().zip(scaled) {
                self.mem.write_f32(p(offset), value);
            }
        }
        for offset in [0x4C, 0x50, 0x54, 0x10] {
            self.mem.write_f32(p(offset), 0.0);
        }
    }
}
