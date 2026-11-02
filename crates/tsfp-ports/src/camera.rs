// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/player_wired
//! T2330 native camera basis. The handler owns typed values; only `run_basis`
//! knows the temporary hybrid guest layout. See docs/t2330-camera.md.
use crate::abi::ExtRegisters;
use crate::c2r::rt::{cos, fp_pc, sin, sqrtf};
use crate::memory::GuestMemory;

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-input/src/view.rs"]
mod view;
pub use view::ViewAngles;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraBasis {
    pub forward: [f32; 3],
    pub up: [f32; 3],
    pub right: [f32; 3],
}

/// Runtime constants remain data, rather than guessed literals.
#[derive(Clone, Copy, Debug)]
pub struct BasisPolicy {
    pub radians_per_degree: f32,
    pub normalization_threshold: f32,
    pub normalization_length: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraState {
    pub look: ViewAngles,
    pub bob: ViewAngles,
    /// Attached objects may supply their own basis, suppressing the player writer.
    pub externally_controlled: bool,
    pub basis: CameraBasis,
}

impl CameraState {
    /// Original fixed-step presentation consumer. `round` preserves the caller's
    /// x87 precision policy; it does not advance look, gameplay or shake timers.
    pub fn update_basis(&mut self, policy: BasisPolicy, round: impl Fn(f64) -> f64) -> bool {
        if self.externally_controlled {
            return false;
        }
        let yaw = round(
            round(self.bob.yaw as f64 + self.look.yaw as f64) * policy.radians_per_degree as f64,
        );
        let pitch = round(
            round(self.bob.pitch as f64 + self.look.pitch as f64)
                * policy.radians_per_degree as f64,
        );
        // Original stores cos(pitch) to f32 before multiplying the yaw terms.
        let pitch_cos = cos(pitch) as f32;
        let forward = [
            round(sin(yaw) * pitch_cos as f64) as f32,
            sin(pitch) as f32,
            round(cos(yaw) * pitch_cos as f64) as f32,
        ];
        // Preserve the original cross-product operation order (including zero
        // products for unordered inputs), and its near-pole normalization gate.
        let right = normalize(
            [
                forward[2] - forward[1] * 0.0,
                forward[0] * 0.0 - forward[2] * 0.0,
                forward[1] * 0.0 - forward[0],
            ],
            policy,
        );
        let up = normalize(
            [
                right[2] * forward[1] - right[1] * forward[2],
                right[0] * forward[2] - right[2] * forward[0],
                right[1] * forward[0] - right[0] * forward[1],
            ],
            policy,
        );
        self.basis = CameraBasis { forward, up, right };
        true
    }
}

fn normalize(mut vector: [f32; 3], policy: BasisPolicy) -> [f32; 3] {
    let squared = (vector[0] * vector[0] + vector[1] * vector[1]) + vector[2] * vector[2];
    if squared > policy.normalization_threshold {
        let scale = (1.0 / sqrtf(squared)) * policy.normalization_length;
        for value in &mut vector {
            *value *= scale;
        }
    }
    vector
}

/// Per-root A/B selector. Remains opt-in until a qualifying Tier 2 capture.
fn enabled(name: &core::ffi::CStr) -> bool {
    unsafe extern "C" {
        fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
    }
    // SAFETY: getenv returns a valid terminated string or null; read only byte 0.
    unsafe {
        let value = getenv(name.as_ptr());
        !value.is_null() && value.read() == b'1' as core::ffi::c_char
    }
}
pub fn selected() -> bool {
    enabled(c"TSFP_NATIVE_CAMERA_BASIS")
}
pub fn motion_selected() -> bool {
    enabled(c"TSFP_NATIVE_CAMERA_MOTION")
}
pub fn projection_selected() -> bool {
    enabled(c"TSFP_NATIVE_CAMERA_PROJECTION")
}

pub fn run<M: GuestMemory>(va: u32, m: &mut M, r: &mut ExtRegisters) -> bool {
    if va != 0x25a0c0 || !selected() {
        return false;
    }
    run_basis(m, r);
    true
}

/// TEMPORARY-SHIM: projects existing PlayerSet state, then mirrors native results.
/// Removal is part of T2330's complete camera ownership acceptance. No callee is
/// invoked. Scratch registers are caller-clobbered; ESP and saved registers stay intact.
pub fn run_basis<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    let player = m.read32(0x7b0c7c);
    let attached = crate::player_wire::get32(m, player.wrapping_add(8));
    let externally_controlled =
        attached != 0 && crate::player_wire::get32(m, attached.wrapping_add(0x21c)) & 0x10000 != 0;
    if externally_controlled {
        return;
    }
    let read = |offset| f32::from_bits(crate::player_wire::get32(m, player.wrapping_add(offset)));
    let mut state = CameraState {
        look: ViewAngles {
            yaw: read(0x100),
            pitch: read(0x104),
        },
        bob: ViewAngles {
            yaw: read(0x140),
            pitch: read(0x144),
        },
        externally_controlled,
        ..CameraState::default()
    };
    let policy = BasisPolicy {
        radians_per_degree: f32::from_bits(m.read32(0x475d0c)),
        normalization_threshold: f32::from_bits(m.read32(0x4765f4)),
        normalization_length: f32::from_bits(m.read32(0x475c78)),
    };
    state.update_basis(policy, |value| fp_pc(&r.fpu, value));
    for (offset, values) in [
        (0xc0, state.basis.forward),
        (0xdc, state.basis.up),
        (0xe8, state.basis.right),
    ] {
        for (index, value) in values.into_iter().enumerate() {
            crate::leaf_wire::set_player_word(
                m,
                player.wrapping_add(offset + index as u32 * 4),
                value.to_bits(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{Fpu, Xmm};
    #[derive(Clone)]
    struct Memory(std::vec::Vec<u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0[address as usize]
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }
    fn registers() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x700000,
            ebx: 17,
            ebp: 23,
            esi: 31,
            edi: 47,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }
    #[test]
    fn basis_matches_translated_root_across_angles_bob_precision_and_attachment() {
        let mut original = Memory(std::vec![0; 0x800000]);
        original.write32(0x7b0c7c, 0x20000);
        original.write32(0x475d0c, (core::f32::consts::PI / 180.0).to_bits());
        original.write32(0x475c78, 1.0f32.to_bits());
        original.write32(0x4765f4, 0.000001f32.to_bits());
        let mut native = original.clone();
        let mut seed = 2330u32;
        for case in 0..4096 {
            for offset in [0x100, 0x104, 0x140, 0x144] {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let value = if case % 17 == 0 {
                    90.0
                } else {
                    (seed as i32 as f32 / i32::MAX as f32) * 360.0
                };
                original.write32(0x20000 + offset, value.to_bits());
                native.write32(0x20000 + offset, value.to_bits());
            }
            let attached = if case % 3 == 0 { 0 } else { 0x30000 };
            for memory in [&mut original, &mut native] {
                memory.write32(0x20008, attached);
                memory.write32(0x3021c, if case % 5 == 0 { 0x10000 } else { 0 });
            }
            let mut a = registers();
            if case % 2 == 0 {
                a.fpu.control = 0x7f;
            }
            let mut b = a;
            crate::c2r::player_wired::game_player_build_view_direction_and_basis_vectors_from_angles_and_bob(&mut original, &mut a);
            run_basis(&mut native, &mut b);
            for offset in [0xc0, 0xc4, 0xc8, 0xdc, 0xe0, 0xe4, 0xe8, 0xec, 0xf0] {
                assert_eq!(
                    original.read32(0x20000 + offset),
                    native.read32(0x20000 + offset),
                    "case {case}, offset {offset:x}"
                );
            }
            assert_eq!(
                (a.esp, a.ebx, a.ebp, a.esi, a.edi),
                (b.esp, b.ebx, b.ebp, b.esi, b.edi)
            );
            assert_eq!(a.fpu.top, b.fpu.top);
        }
    }
}

/// Completed presentation data supplied by the camera/renderer attachment.
/// Shake is an already resolved pose, not a request to run collision or timers
/// again at the display rate. FOV is the effective zoomed vertical FOV.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraSnapshot {
    pub tick: u64,
    pub position: [f32; 3],
    pub angles: ViewAngles,
    pub bob: ViewAngles,
    pub shake_angles: ViewAngles,
    pub shake_position: [f32; 3],
    pub vertical_fov_degrees: f32,
    pub aspect_ratio: f32,
}

/// Per-camera history belongs to the caller (one instance per player/view).
/// Cuts/teleports replace both endpoints explicitly; no shared global camera.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraHistory {
    previous: Option<CameraSnapshot>,
    current: Option<CameraSnapshot>,
}
fn positive_degrees(value: f32) -> f32 {
    let remainder = value % 360.0;
    if remainder < 0.0 {
        remainder + 360.0
    } else {
        remainder
    }
}

impl CameraHistory {
    /// Returns false on a duplicate/out-of-order tick, preserving both endpoints.
    pub fn publish(&mut self, next: CameraSnapshot, cut: bool) -> bool {
        if !cut
            && self
                .current
                .is_some_and(|current| next.tick <= current.tick)
        {
            return false;
        }
        self.previous = if cut {
            Some(next)
        } else {
            self.current.or(Some(next))
        };
        self.current = Some(next);
        true
    }
    /// A fixed 60 Hz endpoint is copied exactly at alpha 0/1. Interpolation is
    /// read-only and never advances original simulation state. Invalid alpha is
    /// rejected; finite alpha outside the tick interval is clamped.
    pub fn sample(&self, alpha: f32) -> Option<CameraSnapshot> {
        if !alpha.is_finite() {
            return None;
        }
        let previous = self.previous?;
        let current = self.current?;
        let alpha = alpha.clamp(0.0, 1.0);
        if alpha == 0.0 {
            return Some(previous);
        }
        if alpha == 1.0 {
            return Some(current);
        }
        let linear = |a: f32, b: f32| a + (b - a) * alpha;
        let angles = |a: ViewAngles, b: ViewAngles| ViewAngles {
            yaw: a.yaw + (positive_degrees(b.yaw - a.yaw + 180.0) - 180.0) * alpha,
            pitch: linear(a.pitch, b.pitch),
        };
        Some(CameraSnapshot {
            tick: current.tick,
            position: core::array::from_fn(|i| linear(previous.position[i], current.position[i])),
            angles: angles(previous.angles, current.angles),
            bob: ViewAngles {
                yaw: linear(previous.bob.yaw, current.bob.yaw),
                pitch: linear(previous.bob.pitch, current.bob.pitch),
            },
            shake_angles: ViewAngles {
                yaw: linear(previous.shake_angles.yaw, current.shake_angles.yaw),
                pitch: linear(previous.shake_angles.pitch, current.shake_angles.pitch),
            },
            shake_position: core::array::from_fn(|i| {
                linear(previous.shake_position[i], current.shake_position[i])
            }),
            vertical_fov_degrees: linear(
                previous.vertical_fov_degrees,
                current.vertical_fov_degrees,
            ),
            aspect_ratio: linear(previous.aspect_ratio, current.aspect_ratio),
        })
    }
}

impl CameraSnapshot {
    /// Sampled native basis; original fixed endpoints use the same basis handler.
    pub fn basis(self, policy: BasisPolicy) -> CameraBasis {
        let mut camera = CameraState {
            look: ViewAngles {
                yaw: self.angles.yaw + self.shake_angles.yaw,
                pitch: self.angles.pitch + self.shake_angles.pitch,
            },
            bob: self.bob,
            ..CameraState::default()
        };
        camera.update_basis(policy, |value| value);
        camera.basis
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;
    #[test]
    fn history_endpoints_wrap_cuts_and_sampling_do_not_tick() {
        let first = CameraSnapshot {
            tick: 10,
            angles: ViewAngles {
                yaw: 359.0,
                pitch: -20.0,
            },
            vertical_fov_degrees: 55.0,
            ..CameraSnapshot::default()
        };
        let second = CameraSnapshot {
            tick: 11,
            angles: ViewAngles {
                yaw: 1.0,
                pitch: 20.0,
            },
            vertical_fov_degrees: 30.25,
            position: [10.0; 3],
            ..first
        };
        let mut history = CameraHistory::default();
        assert_eq!(history.sample(0.5), None);
        assert!(history.publish(first, false));
        assert_eq!(history.sample(0.5), Some(first));
        assert!(history.publish(second, false));
        let saved = history;
        assert_eq!(history.sample(0.0), Some(first));
        assert_eq!(history.sample(1.0), Some(second));
        let middle = history.sample(0.5).unwrap();
        assert_eq!(
            middle.angles,
            ViewAngles {
                yaw: 360.0,
                pitch: 0.0
            }
        );
        assert_eq!(middle.position, [5.0; 3]);
        assert_eq!(middle.vertical_fov_degrees, 42.625);
        assert_eq!(history, saved);
        assert!(!history.publish(first, false));
        assert_eq!(history.sample(f32::NAN), None);
        assert!(history.publish(first, true));
        assert_eq!(history.sample(0.5), Some(first));
    }
}

/// Typed original perspective contract (0x151CF0). FOV is already in radians,
/// as supplied by 0xC77C0 after runtime degree conversion. Row-major guest
/// matrix uses negative Z projection and the runtime homogeneous-axis constant.
#[derive(Clone, Copy, Debug)]
pub struct Perspective {
    pub vertical_fov_radians: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}
impl Perspective {
    pub fn matrix(self, half: f32, homogeneous_axis: f32, round: impl Fn(f64) -> f64) -> [f32; 16] {
        let half_angle = round(self.vertical_fov_radians as f64 * half as f64);
        let cotangent = round(cos(half_angle) / sin(half_angle));
        let depth = self.far / (self.far - self.near);
        let mut matrix = [0.0; 16];
        matrix[0] = round(cotangent / self.aspect_ratio as f64) as f32;
        matrix[5] = cotangent as f32;
        matrix[10] = 0.0 - depth;
        matrix[11] = homogeneous_axis;
        matrix[14] = 0.0 - depth * self.near;
        matrix
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use crate::abi::{Fpu, Xmm};
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    #[test]
    fn typed_perspective_matches_translated_matrix_writer() {
        let mut seed = 2330u32;
        for case in 0..2048 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let input = Perspective {
                vertical_fov_radians: (20.0 + (seed % 120) as f32)
                    * (core::f32::consts::PI / 180.0),
                aspect_ratio: 0.5 + (seed % 300) as f32 / 100.0,
                near: 0.1 + (seed % 20) as f32,
                far: 100.0 + (seed % 10000) as f32,
            };
            let mut memory = Memory::default();
            for (address, value) in [
                (0x1004, 0x2000),
                (0x1008, input.aspect_ratio.to_bits()),
                (0x100c, input.vertical_fov_radians.to_bits()),
                (0x1010, input.near.to_bits()),
                (0x1014, input.far.to_bits()),
                (0x475cd4, 0.5f32.to_bits()),
                (0x4760d0, (-1.0f32).to_bits()),
            ] {
                memory.write32(address, value);
            }
            let mut regs = ExtRegisters {
                eax: 0,
                ecx: 0,
                edx: 0,
                esp: 0x1000,
                ebx: 0,
                ebp: 0,
                esi: 0,
                edi: 0,
                eflags: 0,
                fpu: Fpu {
                    stack: [0.0; 8],
                    top: 0,
                    control: if case % 2 == 0 { 0x7f } else { 0x37f },
                },
                xmm: [Xmm::default(); 5],
                xmm_hi: [Xmm::default(); 3],
            };
            let expected = input.matrix(0.5, -1.0, |value| fp_pc(&regs.fpu, value));
            crate::c2r::t1937_math::game_matrix4x4_make_perspective_fov(&mut memory, &mut regs);
            for (index, value) in expected.into_iter().enumerate() {
                assert_eq!(
                    value.to_bits(),
                    memory.read32(0x2000 + 4 * index as u32),
                    "case {case} matrix[{index}]"
                );
            }
        }
    }
}

/// Shared original shake phases are separate from each camera's timer: the
/// original uses three global phases, not one phase bank per camera.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShakePhases(pub [f32; 3]);
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShakeState {
    pub remaining: f32,
    pub displacement_amplitude: f32,
    pub up_amplitude: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct ShakePolicy {
    pub rates: [f32; 3],
    pub radians_per_degree: f32,
    pub table_scale: f32,
    pub table_bias: f32,
    pub vertical_scale: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShakeOffsets {
    /// Desired local offsets; collision resolution is a separate required edge.
    pub world_displacement: [f32; 3],
    pub up_delta: [f32; 3],
}
impl ShakeState {
    /// 0xC78C0 policy: a positive-duration request replaces an idle shake or a
    /// strictly stronger displacement amplitude. Duration alone never extends it.
    pub fn request(&mut self, duration: f32, displacement: f32, up: f32) -> bool {
        if !(duration > 0.0)
            || (!(0.0 >= self.remaining) && !(displacement > self.displacement_amplitude))
        {
            return false;
        }
        self.remaining = duration;
        self.displacement_amplitude = displacement;
        self.up_amplitude = up;
        true
    }
    /// Timer/phase/sine stage of 0xC79C0. Called exactly once at its original
    /// fixed-step call site. Disabled camera flags suppress the entire stage.
    /// The caller must resolve the resulting pose against room geometry before
    /// publishing a snapshot; these offsets are not a collision result.
    pub fn tick(
        &mut self,
        phases: &mut ShakePhases,
        frame_scale: f32,
        tick_divisor: i32,
        disabled: bool,
        policy: ShakePolicy,
        sine_table: &[f32; 2048],
    ) -> Option<ShakeOffsets> {
        if disabled {
            *self = Self::default();
            return None;
        }
        if self.remaining > 0.0 {
            self.remaining -= frame_scale / tick_divisor as f32;
            for (phase, rate) in phases.0.iter_mut().zip(policy.rates) {
                *phase = (frame_scale * rate) / tick_divisor as f32 + *phase;
            }
            if self.remaining < 0.0 {
                self.remaining = 0.0;
            }
        }
        let samples: [f32; 3] = core::array::from_fn(|axis| {
            let index = ((phases.0[axis] * policy.radians_per_degree) * policy.table_scale)
                + policy.table_bias;
            // SSE cvttss2si returns INT_MIN on unordered/out-of-range input.
            let index = if index >= -2147483648.0 && index < 2147483648.0 {
                index as i32
            } else {
                i32::MIN
            };
            sine_table[(index as u32 & 0x7ff) as usize] * self.remaining
        });
        Some(ShakeOffsets {
            world_displacement: [
                self.displacement_amplitude * samples[0],
                (self.displacement_amplitude * samples[1]) * policy.vertical_scale,
                self.displacement_amplitude * samples[2],
            ],
            up_delta: core::array::from_fn(|axis| self.up_amplitude * samples[axis]),
        })
    }
}

#[cfg(test)]
mod shake_tests {
    use super::*;
    #[test]
    fn requests_preserve_stronger_shakes_and_tick_crossing_clamps_without_resampling() {
        let mut state = ShakeState::default();
        assert!(state.request(0.01, 2.0, 3.0));
        assert!(!state.request(10.0, 2.0, 9.0));
        assert!(!state.request(f32::NAN, 3.0, 9.0));
        assert!(state.request(0.01, 3.0, 4.0));
        let mut phases = ShakePhases([10.0, 20.0, 30.0]);
        let saved = phases;
        let policy = ShakePolicy {
            rates: [3.0, 2.0, 1.0],
            radians_per_degree: 1.0,
            table_scale: 1.0,
            table_bias: 0.5,
            vertical_scale: -1.0,
        };
        assert_eq!(
            state.tick(&mut phases, 1.0, 60, true, policy, &[1.0; 2048]),
            None
        );
        assert_eq!(phases, saved);
        assert_eq!(state, ShakeState::default());
        assert!(state.request(0.01, 3.0, 4.0));
        let offsets = state
            .tick(&mut phases, 1.0, 60, false, policy, &[1.0; 2048])
            .unwrap();
        assert_eq!(state.remaining, 0.0);
        assert_eq!(offsets.world_displacement[0], 0.0);
        assert_eq!(offsets.up_delta, [0.0; 3]);
        let saved = phases;
        state.tick(&mut phases, 1.0, 60, false, policy, &[1.0; 2048]);
        assert_eq!(phases, saved);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProjectionPolicy {
    pub radians_per_degree: f32,
    pub half: f32,
    pub homogeneous_axis: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraProjection {
    pub matrix: [f32; 16],
    pub tangent_extents: [f32; 2],
}
impl CameraSnapshot {
    /// Native presentation projection from the effective zoom FOV. The matrix
    /// and tangent paths preserve their different original conversion order.
    pub fn projection(
        self,
        near: f32,
        far: f32,
        policy: ProjectionPolicy,
        round: impl Fn(f64) -> f64,
    ) -> CameraProjection {
        let input = Perspective {
            vertical_fov_radians: self.vertical_fov_degrees * policy.radians_per_degree,
            aspect_ratio: self.aspect_ratio,
            near,
            far,
        };
        let half_angle = round(
            round(self.vertical_fov_degrees as f64 * policy.half as f64)
                * policy.radians_per_degree as f64,
        );
        let vertical = crate::c2r::rt::tan(half_angle) as f32;
        CameraProjection {
            matrix: input.matrix(policy.half, policy.homogeneous_axis, round),
            tangent_extents: [vertical, vertical * self.aspect_ratio],
        }
    }
}

/// Original 0xCB3B0 selection over typed camera mode data. Constants are loaded
/// from runtime input, including the special mode's default FOV.
pub fn reference_fov(camera_kind: u32, variant: u32, ordinary: f32, special: f32) -> f32 {
    if camera_kind == 0 && variant == 1 {
        special
    } else {
        ordinary
    }
}

/// Readable 0x1519E0 view-matrix contract, including its explicit forward sign.
/// This is independent of guest matrix buffers and can consume a sampled pose.
#[derive(Clone, Copy, Debug)]
pub struct LookAt {
    pub eye: [f32; 3],
    pub direction: [f32; 3],
    pub up: [f32; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct ViewPolicy {
    pub forward_sign: f32,
    pub unit_length: f32,
}
impl LookAt {
    pub fn matrix(self, policy: ViewPolicy, round: impl Fn(f64) -> f64) -> [f32; 16] {
        let normalize = |vector: [f32; 3], length: f32, reverse: bool| {
            let squares = vector.map(|value| value * value);
            let squared = if reverse {
                (squares[2] + squares[1]) + squares[0]
            } else {
                (squares[0] + squares[1]) + squares[2]
            };
            let scale = if squared > 0.0 {
                round(length as f64 / round(crate::c2r::rt::sqrt(squared as f64))) as f32
            } else {
                squared
            };
            vector.map(|value| value * scale)
        };
        let direction = normalize(self.direction, policy.forward_sign, false);
        let right = normalize(
            [
                direction[2] * self.up[1] - direction[1] * self.up[2],
                direction[0] * self.up[2] - direction[2] * self.up[0],
                direction[1] * self.up[0] - direction[0] * self.up[1],
            ],
            policy.unit_length,
            true,
        );
        let up = normalize(
            [
                right[2] * direction[1] - right[1] * direction[2],
                right[0] * direction[2] - right[2] * direction[0],
                right[1] * direction[0] - right[0] * direction[1],
            ],
            policy.unit_length,
            false,
        );
        let negative_dot = |axis: [f32; 3]| {
            0.0 - ((axis[0] * self.eye[0] + axis[1] * self.eye[1]) + axis[2] * self.eye[2])
        };
        [
            right[0],
            up[0],
            direction[0],
            0.0,
            right[1],
            up[1],
            direction[1],
            0.0,
            right[2],
            up[2],
            direction[2],
            0.0,
            negative_dot(right),
            negative_dot(up),
            negative_dot(direction),
            policy.unit_length,
        ]
    }
}

#[cfg(test)]
mod view_matrix_tests {
    use super::*;
    use crate::abi::{Fpu, Xmm};
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    #[test]
    fn typed_view_matrix_matches_translated_writer() {
        let mut seed = 2330u32;
        for case in 0..2048 {
            let mut vector = || {
                core::array::from_fn(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    seed as i32 as f32 / i32::MAX as f32
                })
            };
            let input = LookAt {
                eye: vector(),
                direction: vector(),
                up: vector(),
            };
            let mut memory = Memory::default();
            memory.write32(0x1004, 0x2000);
            for (index, value) in input
                .eye
                .into_iter()
                .chain(input.direction)
                .chain(input.up)
                .enumerate()
            {
                memory.write32(0x1008 + index as u32 * 4, value.to_bits());
            }
            memory.write32(0x4760d0, (-1.0f32).to_bits());
            memory.write32(0x475c78, 1.0f32.to_bits());
            let mut regs = ExtRegisters {
                eax: 0,
                ecx: 0,
                edx: 0,
                esp: 0x1000,
                ebx: 0,
                ebp: 0,
                esi: 0,
                edi: 0,
                eflags: 0,
                fpu: Fpu {
                    stack: [0.0; 8],
                    top: 0,
                    control: if case % 2 == 0 { 0x7f } else { 0x37f },
                },
                xmm: [Xmm::default(); 5],
                xmm_hi: [Xmm::default(); 3],
            };
            let matrix = input.matrix(
                ViewPolicy {
                    forward_sign: -1.0,
                    unit_length: 1.0,
                },
                |value| fp_pc(&regs.fpu, value),
            );
            crate::c2r::t1937_math::game_matrix4x4_make_look_at_from_eye_direction_and_up(
                &mut memory,
                &mut regs,
            );
            for (index, value) in matrix.into_iter().enumerate() {
                assert_eq!(
                    value.to_bits(),
                    memory.read32(0x2000 + 4 * index as u32),
                    "case {case} matrix[{index}]"
                );
            }
        }
    }
}

/// Final camera pose, with an explicit up vector (shake is not Euler roll).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraPose {
    pub eye: [f32; 3],
    pub direction: [f32; 3],
    pub up: [f32; 3],
    pub room: i32,
    pub previous_room: i32,
}

/// Collision contract crossed by 0xC79C0, implemented by the neighbouring
/// scene system. No default/no-op implementation is supplied.
pub trait CameraRooms {
    fn raycast(&mut self, room: i32, origin: [f32; 3], displacement: [f32; 3]) -> Option<[f32; 3]>;
    fn trace_segment(&mut self, room: i32, origin: [f32; 3], displacement: [f32; 3]) -> i32;
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraMotion {
    pub shake: ShakeState,
    pub recoil_distance: f32,
    pub recoil_decay: f32,
    pub pitch_degrees: f32,
    pub pitch_decay: f32,
    pub kick_distance: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct MotionPolicy {
    pub shake: ShakePolicy,
    pub basis: BasisPolicy,
    pub decay_cutoff: f32,
    pub probe_extension: f32,
    pub minimum_hit_distance: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct MotionTick {
    pub frame_scale: f32,
    pub tick_divisor: i32,
    pub elapsed_frames: i32,
    pub kick_direction: [f32; 3],
    pub base_direction: [f32; 3],
    pub disabled: bool,
}
impl CameraMotion {
    /// Readable full shake/recoil/room-resolution step of 0xC79C0. The caller
    /// owns pose/state and runs this once per original simulation call.
    pub fn tick(
        &mut self,
        pose: &mut CameraPose,
        phases: &mut ShakePhases,
        tick: MotionTick,
        policy: MotionPolicy,
        table: &[f32; 2048],
        round: impl Fn(f64) -> f64,
        rooms: &mut impl CameraRooms,
    ) {
        if tick.disabled {
            *self = Self::default();
            return;
        }
        let offsets = self
            .shake
            .tick(
                phases,
                tick.frame_scale,
                tick.tick_divisor,
                false,
                policy.shake,
                table,
            )
            .expect("enabled shake stage always supplies offsets");
        let displacement: [f32; 3] = core::array::from_fn(|i| {
            pose.up[i] = offsets.up_delta[i] + pose.up[i];
            // Preserve the original multiply/subtract/add order.
            (offsets.world_displacement[i] - tick.base_direction[i] * self.recoil_distance)
                + self.kick_distance * tick.kick_direction[i]
        });
        let yaw = crate::c2r::rt::atan2(pose.direction[0] as f64, pose.direction[2] as f64) as f32;
        pose.direction = rotate_y(pose.direction, -yaw);
        pose.direction = rotate_x(
            pose.direction,
            0.0 - self.pitch_degrees * policy.shake.radians_per_degree,
        );
        pose.direction = rotate_y(pose.direction, yaw);
        for _ in 0..tick.elapsed_frames.max(0) {
            self.recoil_distance *= self.recoil_decay;
            self.pitch_degrees *= self.pitch_decay;
            if self.recoil_distance < policy.decay_cutoff {
                self.recoil_distance = 0.0;
            }
            if self.pitch_degrees < policy.decay_cutoff {
                self.pitch_degrees = 0.0;
            }
        }
        pose.previous_room = pose.room;
        // Original unordered comparisons treat NaN as zero in this gate.
        if !displacement
            .iter()
            .any(|value| *value < 0.0 || *value > 0.0)
        {
            return;
        }
        let squared = (displacement[2] * displacement[2] + displacement[1] * displacement[1])
            + displacement[0] * displacement[0];
        let probe_direction = if squared > policy.basis.normalization_threshold {
            let scale = (1.0 / sqrtf(squared)) * policy.basis.normalization_length;
            displacement.map(|value| value * scale)
        } else {
            displacement
        };
        let extension = probe_direction.map(|value| value * policy.probe_extension);
        let mut resolved = displacement;
        if pose.previous_room <= 200 {
            let probe = core::array::from_fn(|i| displacement[i] + extension[i]);
            if let Some(hit) = rooms.raycast(pose.previous_room, pose.eye, probe) {
                let delta: [f64; 3] =
                    core::array::from_fn(|i| round(hit[i] as f64 - pose.eye[i] as f64));
                // x87 evaluates the Z/Y/X squared sum, not the SSE order.
                let squared = round(
                    round(round(delta[2] * delta[2]) + round(delta[1] * delta[1]))
                        + round(delta[0] * delta[0]),
                );
                let distance = round(crate::c2r::rt::sqrt(squared));
                if !(distance > policy.minimum_hit_distance as f64) {
                    return;
                }
                resolved = core::array::from_fn(|i| (hit[i] - extension[i]) - pose.eye[i]);
            }
        }
        pose.room = rooms.trace_segment(pose.previous_room, pose.eye, resolved);
        for (value, delta) in pose.eye.iter_mut().zip(resolved) {
            *value += delta;
        }
    }
}

fn transform_rotation(vector: [f32; 3], rows: [[f32; 3]; 3]) -> [f32; 3] {
    core::array::from_fn(|i| {
        ((rows[2][i] * vector[2] + rows[1][i] * vector[1]) + rows[0][i] * vector[0]) + 0.0
    })
}
fn rotate_y(vector: [f32; 3], radians: f32) -> [f32; 3] {
    let (sine, cosine) = (sin(radians as f64) as f32, cos(radians as f64) as f32);
    transform_rotation(
        vector,
        [[cosine, 0.0, -sine], [0.0, 1.0, 0.0], [sine, 0.0, cosine]],
    )
}
fn rotate_x(vector: [f32; 3], radians: f32) -> [f32; 3] {
    let (sine, cosine) = (sin(radians as f64) as f32, cos(radians as f64) as f32);
    transform_rotation(
        vector,
        [[1.0, 0.0, 0.0], [0.0, cosine, sine], [0.0, -sine, cosine]],
    )
}

/// TEMPORARY-SHIM: hybrid ABI projection and neighbouring room-query bridge.
/// Camera logic runs exclusively on typed owned values; room helpers retain
/// their independent implementation selection. This is never a default until
/// qualifying real-camera Tier 2, including a distinct scratch/base direction.
pub fn run_motion<M: GuestMemory, C: crate::abi::GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    let camera = m.read32(0x7b0cc0);
    let float = |address| f32::from_bits(m.read32(address));
    let vector = |address| core::array::from_fn(|i| float(address + i as u32 * 4));
    let mut pose = CameraPose {
        eye: vector(camera + 0x27c),
        direction: vector(r.esi),
        up: vector(camera + 0x364),
        room: m.read32(camera + 0x2a0) as i32,
        previous_room: m.read32(camera + 0x2a4) as i32,
    };
    let mut motion = CameraMotion {
        shake: ShakeState {
            remaining: float(camera + 0x398),
            displacement_amplitude: float(camera + 0x39c),
            up_amplitude: float(camera + 0x3a0),
        },
        recoil_distance: float(camera + 0x3a4),
        recoil_decay: float(camera + 0x3a8),
        pitch_degrees: float(camera + 0x3ac),
        pitch_decay: float(camera + 0x3b0),
        kick_distance: float(camera + 0x3b8),
    };
    let mut phases = ShakePhases(vector(0x4dd350));
    let policy = MotionPolicy {
        shake: ShakePolicy {
            rates: [float(0x47d618), float(0x47d5a4), float(0x47d614)],
            radians_per_degree: float(0x475d0c),
            table_scale: float(0x475ce0),
            table_bias: float(0x475cd4),
            vertical_scale: float(0x475d10),
        },
        basis: BasisPolicy {
            radians_per_degree: float(0x475d0c),
            normalization_threshold: float(0x4765f4),
            normalization_length: float(0x475c78),
        },
        decay_cutoff: float(0x475d1c),
        probe_extension: float(0x475cd4),
        minimum_hit_distance: float(0x4788e8),
    };
    let tick = MotionTick {
        frame_scale: float(0x4e794c),
        tick_divisor: m.read32(0x4e8cec) as i32,
        elapsed_frames: m.read32(0x4e7944) as i32,
        kick_direction: vector(0x7330a0),
        base_direction: vector(camera + 0x334),
        disabled: m.read8(camera + 0x21c) & 0x80 != 0,
    };
    let table = core::array::from_fn(|i| float(0x7a38a0 + i as u32 * 4));
    let fpu = r.fpu;
    let direction_pointer = r.esi;
    {
        let mut rooms = HybridRooms {
            memory: m,
            guest,
            registers: r,
        };
        motion.tick(
            &mut pose,
            &mut phases,
            tick,
            policy,
            &table,
            |value| fp_pc(&fpu, value),
            &mut rooms,
        );
    }
    record_motion(
        camera,
        pose,
        ViewPolicy {
            forward_sign: f32::from_bits(m.read32(0x4760d0)),
            unit_length: policy.basis.normalization_length,
        },
        &fpu,
    );
    for (address, values) in [
        (camera + 0x27c, pose.eye),
        (direction_pointer, pose.direction),
        (camera + 0x364, pose.up),
        (0x4dd350, phases.0),
    ] {
        for (i, value) in values.into_iter().enumerate() {
            m.write32(address + i as u32 * 4, value.to_bits());
        }
    }
    m.write32(camera + 0x2a0, pose.room as u32);
    m.write32(camera + 0x2a4, pose.previous_room as u32);
    for (offset, value) in [
        (0x398, motion.shake.remaining),
        (0x39c, motion.shake.displacement_amplitude),
        (0x3a0, motion.shake.up_amplitude),
        (0x3a4, motion.recoil_distance),
        (0x3a8, motion.recoil_decay),
        (0x3ac, motion.pitch_degrees),
        (0x3b0, motion.pitch_decay),
        (0x3b8, motion.kick_distance),
    ] {
        m.write32(camera + offset, value.to_bits());
    }
}
struct HybridRooms<'a, M, C> {
    memory: &'a mut M,
    guest: &'a mut C,
    registers: &'a mut ExtRegisters,
}
impl<M: GuestMemory, C: crate::abi::GuestCall> HybridRooms<'_, M, C> {
    fn query(&mut self, va: u32, room: i32, origin: [f32; 3], delta: [f32; 3]) -> (u32, [f32; 3]) {
        use crate::c2r::rt::RegsExt;
        let entry_esp = self.registers.esp;
        let base = entry_esp.wrapping_sub(0x88);
        self.registers.esp = base;
        for (offset, values) in [(0, origin), (12, delta)] {
            for (i, value) in values.into_iter().enumerate() {
                self.memory
                    .write32(base + offset + i as u32 * 4, value.to_bits());
            }
        }
        self.registers.esp -= 0x40;
        if va == 0x88c60 {
            for argument in [
                0,
                0,
                (-7i32) as u32, // original collision inclusion mask
                0,
                base + 36,
                base + 24,
                0,
                base + 12,
                base,
                room as u32,
            ] {
                self.registers.push(self.memory, argument);
            }
        } else {
            for argument in [0, 0, 0, 0, room as u32, base + 12, base] {
                self.registers.push(self.memory, argument);
            }
        }
        self.registers.call(self.guest, va);
        let result = self.registers.eax;
        let hit =
            core::array::from_fn(|i| f32::from_bits(self.memory.read32(base + 24 + i as u32 * 4)));
        self.registers.esp = entry_esp;
        (result, hit)
    }
}
impl<M: GuestMemory, C: crate::abi::GuestCall> CameraRooms for HybridRooms<'_, M, C> {
    fn raycast(&mut self, room: i32, origin: [f32; 3], displacement: [f32; 3]) -> Option<[f32; 3]> {
        let (result, hit) = self.query(0x88c60, room, origin, displacement);
        (result != 0).then_some(hit)
    }
    fn trace_segment(&mut self, room: i32, origin: [f32; 3], displacement: [f32; 3]) -> i32 {
        self.query(0x88430, room, origin, displacement).0 as i32
    }
}

/// Renderer-ready fixed-step packet. Matrices are cached at publication so
/// alpha 0/1 preserve the original tick's precision and operation ordering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraFrame {
    pub tick: u64,
    pub pose: CameraPose,
    pub vertical_fov_degrees: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
    pub view: [f32; 16],
    pub projection: CameraProjection,
}
#[derive(Clone, Copy, Debug)]
pub struct CameraPolicies {
    pub basis: BasisPolicy,
    pub motion: MotionPolicy,
    pub view: ViewPolicy,
    pub projection: ProjectionPolicy,
}
#[derive(Clone, Copy, Debug)]
pub struct CameraTick {
    pub tick: u64,
    pub look: ViewAngles,
    pub bob: ViewAngles,
    pub eye: [f32; 3],
    pub up: [f32; 3],
    pub room: i32,
    /// Effective current zoom FOV, supplied by the weapon/zoom provider.
    pub vertical_fov_degrees: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
    pub motion: MotionTick,
    pub cut: bool,
}
/// Native per-view owner. Input devices update `ViewAngles` through T2194's
/// shared handler; they never emulate a pad or write this camera's guest image.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraRig {
    pub state: CameraState,
    pub motion: CameraMotion,
    previous: Option<CameraFrame>,
    current: Option<CameraFrame>,
}
impl CameraRig {
    /// Player-look fixed step. Unsupported external camera ownership declines
    /// before changing state; cutscene/attached controllers supply their own rig.
    pub fn tick(
        &mut self,
        input: CameraTick,
        phases: &mut ShakePhases,
        policies: CameraPolicies,
        sine_table: &[f32; 2048],
        round: impl Fn(f64) -> f64,
        rooms: &mut impl CameraRooms,
    ) -> Option<CameraFrame> {
        if self.state.externally_controlled
            || (!input.cut && self.current.is_some_and(|frame| input.tick <= frame.tick))
        {
            return None;
        }
        self.state.look = input.look;
        self.state.bob = input.bob;
        self.state.update_basis(policies.basis, &round);
        let mut pose = CameraPose {
            eye: input.eye,
            direction: self.state.basis.forward,
            up: input.up,
            room: input.room,
            previous_room: input.room,
        };
        if input.motion.elapsed_frames > 0 {
            let motion_tick = MotionTick {
                base_direction: self.state.basis.forward,
                ..input.motion
            };
            self.motion.tick(
                &mut pose,
                phases,
                motion_tick,
                policies.motion,
                sine_table,
                &round,
                rooms,
            );
        }
        let snapshot = CameraSnapshot {
            vertical_fov_degrees: input.vertical_fov_degrees,
            aspect_ratio: input.aspect_ratio,
            ..CameraSnapshot::default()
        };
        let frame = CameraFrame {
            tick: input.tick,
            pose,
            vertical_fov_degrees: input.vertical_fov_degrees,
            aspect_ratio: input.aspect_ratio,
            near: input.near,
            far: input.far,
            view: LookAt {
                eye: pose.eye,
                direction: pose.direction,
                up: pose.up,
            }
            .matrix(policies.view, &round),
            projection: snapshot.projection(input.near, input.far, policies.projection, &round),
        };
        self.previous = if input.cut {
            Some(frame)
        } else {
            self.current.or(Some(frame))
        };
        self.current = Some(frame);
        Some(frame)
    }
    /// Display-rate sampling never updates timers, input or room queries. The
    /// union of endpoint rooms lets the renderer avoid prematurely culling a
    /// portal crossing while displaying the interpolated eye position.
    pub fn sample(&self, alpha: f32, projection: ProjectionPolicy) -> Option<CameraPresentation> {
        if !alpha.is_finite() {
            return None;
        }
        let previous = self.previous?;
        let current = self.current?;
        let alpha = alpha.clamp(0.0, 1.0);
        if previous.tick == current.tick {
            return Some(CameraPresentation::endpoint(current));
        }
        if alpha == 0.0 {
            return Some(CameraPresentation::endpoint(previous));
        }
        if alpha == 1.0 {
            return Some(CameraPresentation::endpoint(current));
        }
        let linear = |a: f32, b: f32| a + (b - a) * alpha;
        let eye = core::array::from_fn(|i| linear(previous.pose.eye[i], current.pose.eye[i]));
        let a = rotation_quaternion(previous.view);
        let mut b = rotation_quaternion(current.view);
        if (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]) < 0.0 {
            b = b.map(|value| -value);
        }
        let mut q: [f32; 4] = core::array::from_fn(|i| linear(a[i], b[i]));
        let scale = 1.0 / sqrtf(q.iter().map(|value| value * value).sum());
        q = q.map(|value| value * scale);
        let mut view = quaternion_rotation(q);
        for i in 0..3 {
            view[12 + i] = 0.0 - ((view[i] * eye[0] + view[4 + i] * eye[1]) + view[8 + i] * eye[2]);
        }
        let snapshot = CameraSnapshot {
            vertical_fov_degrees: linear(
                previous.vertical_fov_degrees,
                current.vertical_fov_degrees,
            ),
            aspect_ratio: linear(previous.aspect_ratio, current.aspect_ratio),
            ..CameraSnapshot::default()
        };
        Some(CameraPresentation {
            eye,
            view,
            projection: snapshot.projection(
                linear(previous.near, current.near),
                linear(previous.far, current.far),
                projection,
                |value| value,
            ),
            visible_rooms: [previous.pose.room, current.pose.room],
        })
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPresentation {
    pub eye: [f32; 3],
    pub view: [f32; 16],
    pub projection: CameraProjection,
    pub visible_rooms: [i32; 2],
}
impl CameraPresentation {
    fn endpoint(frame: CameraFrame) -> Self {
        Self {
            eye: frame.pose.eye,
            view: frame.view,
            projection: frame.projection,
            visible_rooms: [frame.pose.room; 2],
        }
    }
}
fn rotation_quaternion(m: [f32; 16]) -> [f32; 4] {
    let trace = m[0] + m[5] + m[10];
    if trace > 0.0 {
        let scale = sqrtf(trace + 1.0) * 2.0;
        [
            (m[9] - m[6]) / scale,
            (m[2] - m[8]) / scale,
            (m[4] - m[1]) / scale,
            scale * 0.25,
        ]
    } else if m[0] > m[5] && m[0] > m[10] {
        let scale = sqrtf(1.0 + m[0] - m[5] - m[10]) * 2.0;
        [
            scale * 0.25,
            (m[1] + m[4]) / scale,
            (m[2] + m[8]) / scale,
            (m[9] - m[6]) / scale,
        ]
    } else if m[5] > m[10] {
        let scale = sqrtf(1.0 + m[5] - m[0] - m[10]) * 2.0;
        [
            (m[1] + m[4]) / scale,
            scale * 0.25,
            (m[6] + m[9]) / scale,
            (m[2] - m[8]) / scale,
        ]
    } else {
        let scale = sqrtf(1.0 + m[10] - m[0] - m[5]) * 2.0;
        [
            (m[2] + m[8]) / scale,
            (m[6] + m[9]) / scale,
            scale * 0.25,
            (m[4] - m[1]) / scale,
        ]
    }
}
fn quaternion_rotation(q: [f32; 4]) -> [f32; 16] {
    let [x, y, z, w] = q;
    [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - z * w),
        2.0 * (x * z + y * w),
        0.0,
        2.0 * (x * y + z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - x * w),
        0.0,
        2.0 * (x * z - y * w),
        2.0 * (y * z + x * w),
        1.0 - 2.0 * (x * x + y * y),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

/// Shared T2334 rigid-placement presentation. Reflections are factored out
/// before quaternion interpolation; a parity change is a discontinuity.
pub fn sample_object_placement(
    previous: crate::scene::render_prep::Placement,
    current: crate::scene::render_prep::Placement,
    alpha: f32,
) -> crate::scene::render_prep::Placement {
    if alpha == 0.0 {
        return previous;
    }
    if alpha == 1.0 {
        return current;
    }
    let determinant = |m: [[f32; 3]; 3]| {
        (m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]))
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let reflected = determinant(current.orientation) < 0.0;
    if reflected != (determinant(previous.orientation) < 0.0) {
        return current;
    }
    let matrix = |placement: crate::scene::render_prep::Placement| {
        let mut m = [0.; 16];
        m[15] = 1.;
        for i in 0..3 {
            for j in 0..3 {
                m[i * 4 + j] =
                    placement.orientation[i][j] * if reflected && i == 0 { -1. } else { 1. };
            }
        }
        m
    };
    let a = rotation_quaternion(matrix(previous));
    let mut b = rotation_quaternion(matrix(current));
    if a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>() < 0. {
        b = b.map(|v| -v);
    }
    let mut q: [f32; 4] = core::array::from_fn(|i| a[i] + (b[i] - a[i]) * alpha);
    let scale = 1. / sqrtf(q.iter().map(|v| v * v).sum());
    q = q.map(|v| v * scale);
    let m = quaternion_rotation(q);
    crate::scene::render_prep::Placement {
        position: core::array::from_fn(|i| {
            previous.position[i] + (current.position[i] - previous.position[i]) * alpha
        }),
        orientation: core::array::from_fn(|i| {
            core::array::from_fn(|j| m[i * 4 + j] * if reflected && i == 0 { -1. } else { 1. })
        }),
    }
}

#[cfg(test)]
mod rig_tests {
    use super::*;
    struct Rooms(usize);
    impl CameraRooms for Rooms {
        fn raycast(&mut self, _: i32, _: [f32; 3], _: [f32; 3]) -> Option<[f32; 3]> {
            self.0 += 1;
            None
        }
        fn trace_segment(&mut self, room: i32, _: [f32; 3], _: [f32; 3]) -> i32 {
            self.0 += 1;
            room
        }
    }
    fn policies() -> CameraPolicies {
        let basis = BasisPolicy {
            radians_per_degree: core::f32::consts::PI / 180.0,
            normalization_threshold: 0.000001,
            normalization_length: 1.0,
        };
        CameraPolicies {
            basis,
            motion: MotionPolicy {
                basis,
                shake: ShakePolicy {
                    rates: [73.0, 89.0, 101.0],
                    radians_per_degree: basis.radians_per_degree,
                    table_scale: 325.9493,
                    table_bias: 0.5,
                    vertical_scale: 0.3,
                },
                decay_cutoff: 0.001,
                probe_extension: 0.5,
                minimum_hit_distance: 0.1,
            },
            view: ViewPolicy {
                forward_sign: -1.0,
                unit_length: 1.0,
            },
            projection: ProjectionPolicy {
                radians_per_degree: basis.radians_per_degree,
                half: 0.5,
                homogeneous_axis: -1.0,
            },
        }
    }
    #[test]
    fn native_rig_publishes_exact_endpoints_and_read_only_short_arc_presentation() {
        let policy = policies();
        let mut rig = CameraRig::default();
        let mut phases = ShakePhases::default();
        let mut rooms = Rooms(0);
        let first = CameraTick {
            tick: 10,
            look: ViewAngles {
                yaw: 359.0,
                pitch: 0.0,
            },
            bob: ViewAngles::default(),
            eye: [0.0; 3],
            up: [0.0, 1.0, 0.0],
            room: 4,
            vertical_fov_degrees: 55.0,
            aspect_ratio: 4.0 / 3.0,
            near: 0.1,
            far: 1000.0,
            motion: MotionTick {
                frame_scale: 1.0,
                tick_divisor: 60,
                elapsed_frames: 1,
                kick_direction: [0.0; 3],
                base_direction: [0.0; 3],
                disabled: false,
            },
            cut: false,
        };
        let a = rig
            .tick(
                first,
                &mut phases,
                policy,
                &[0.0; 2048],
                |value| value,
                &mut rooms,
            )
            .unwrap();
        let second = CameraTick {
            tick: 11,
            look: ViewAngles {
                yaw: 1.0,
                pitch: 0.0,
            },
            eye: [10.0, 0.0, 0.0],
            room: 5,
            vertical_fov_degrees: 30.25,
            ..first
        };
        let b = rig
            .tick(
                second,
                &mut phases,
                policy,
                &[0.0; 2048],
                |value| value,
                &mut rooms,
            )
            .unwrap();
        let saved = rig;
        assert_eq!(
            rig.sample(0.0, policy.projection),
            Some(CameraPresentation::endpoint(a))
        );
        assert_eq!(
            rig.sample(1.0, policy.projection),
            Some(CameraPresentation::endpoint(b))
        );
        let middle = rig.sample(0.5, policy.projection).unwrap();
        assert_eq!(middle.eye, [5.0, 0.0, 0.0]);
        assert_eq!(middle.visible_rooms, [4, 5]);
        assert!((middle.view[0] + 1.0).abs() < 0.00001);
        assert!((middle.view[10] + 1.0).abs() < 0.00001);
        assert_eq!(rig, saved);
        assert_eq!(rooms.0, 0);
        assert_eq!(
            rig.tick(
                first,
                &mut phases,
                policy,
                &[0.0; 2048],
                |value| value,
                &mut rooms
            ),
            None
        );
        assert_eq!(rig, saved);
        assert_eq!(rig.sample(f32::NAN, policy.projection), None);
        assert!(
            rig.tick(
                CameraTick { cut: true, ..first },
                &mut phases,
                policy,
                &[0.0; 2048],
                |value| value,
                &mut rooms
            )
            .is_some()
        );
        assert_eq!(
            rig.sample(0.5, policy.projection),
            rig.sample(1.0, policy.projection)
        );
    }
    #[test]
    fn quaternion_conversion_retains_all_four_rotation_branches() {
        for q in [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [0.5; 4],
        ] {
            let matrix = quaternion_rotation(q);
            let reconstructed = quaternion_rotation(rotation_quaternion(matrix));
            for (a, b) in matrix.into_iter().zip(reconstructed) {
                assert!((a - b).abs() < 0.00001);
            }
        }
    }
}

/// Transitional camera identity mapping. The renderer receives a generation
/// checked native handle, never an address into the guest camera record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraHandle {
    slot: u8,
    generation: u64,
}
/// Token returned when a native cutscene temporarily selects a camera. Completion restores the
/// prior selection only if this handoff still owns the active-camera slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CutsceneCameraHandoff {
    previous: Option<CameraHandle>,
    selected: CameraHandle,
    epoch: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct CameraLens {
    pub vertical_fov_degrees: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
    pub projection: CameraProjection,
    pub policy: ProjectionPolicy,
}
#[derive(Clone, Copy)]
struct CameraSlot {
    reference: u32,
    generation: u64,
    lens: Option<CameraLens>,
    previous: Option<CameraFrame>,
    current: Option<CameraFrame>,
}
impl CameraSlot {
    const EMPTY: Self = Self {
        reference: 0,
        generation: 0,
        lens: None,
        previous: None,
        current: None,
    };
}
struct CameraBank {
    slots: [CameraSlot; 8],
    next_slot: usize,
    latest: Option<CameraHandle>,
    active: Option<CameraHandle>,
    activation_epoch: u64,
}
impl CameraBank {
    const fn new() -> Self {
        Self {
            slots: [CameraSlot::EMPTY; 8],
            next_slot: 0,
            latest: None,
            active: None,
            activation_epoch: 0,
        }
    }
    fn bind(&mut self, reference: u32) -> CameraHandle {
        let index = self
            .slots
            .iter()
            .position(|slot| slot.reference == reference)
            .unwrap_or_else(|| {
                let index = self.next_slot;
                self.next_slot = (index + 1) % self.slots.len();
                self.slots[index] = CameraSlot {
                    reference,
                    generation: self.slots[index].generation.wrapping_add(1),
                    ..CameraSlot::EMPTY
                };
                index
            });
        CameraHandle {
            slot: index as u8,
            generation: self.slots[index].generation,
        }
    }
    fn slot(&self, handle: CameraHandle) -> Option<&CameraSlot> {
        self.slots
            .get(handle.slot as usize)
            .filter(|slot| slot.generation == handle.generation && slot.reference != 0)
    }
    fn select_active(&mut self, handle: CameraHandle) -> Option<CameraHandle> {
        self.slot(handle)?;
        let previous = self.active.filter(|active| self.slot(*active).is_some());
        self.active = Some(handle);
        self.activation_epoch = self.activation_epoch.wrapping_add(1);
        previous
    }
    fn begin_cutscene_handoff(&mut self, handle: CameraHandle) -> Option<CutsceneCameraHandoff> {
        self.slot(handle)?;
        let previous = self.active.filter(|active| self.slot(*active).is_some());
        self.active = Some(handle);
        self.activation_epoch = self.activation_epoch.wrapping_add(1);
        Some(CutsceneCameraHandoff {
            previous,
            selected: handle,
            epoch: self.activation_epoch,
        })
    }
    fn finish_cutscene_handoff(&mut self, handoff: CutsceneCameraHandoff) -> bool {
        if self.activation_epoch != handoff.epoch
            || self.active != Some(handoff.selected)
            || self.slot(handoff.selected).is_none()
        {
            return false;
        }
        self.active = handoff
            .previous
            .filter(|previous| self.slot(*previous).is_some());
        self.activation_epoch = self.activation_epoch.wrapping_add(1);
        true
    }
}
/// Short native-state critical section, with no guest calls or interpolation
/// under the lock. Readers copy a coherent endpoint pair before sampling.
struct CameraStore {
    held: core::sync::atomic::AtomicBool,
    bank: core::cell::UnsafeCell<CameraBank>,
}
// SAFETY: every access to bank goes through the acquire/release guard below;
// no references to its contents escape the critical section.
unsafe impl Sync for CameraStore {}
impl CameraStore {
    const fn new() -> Self {
        Self {
            held: core::sync::atomic::AtomicBool::new(false),
            bank: core::cell::UnsafeCell::new(CameraBank::new()),
        }
    }
    fn with<R>(&self, action: impl FnOnce(&mut CameraBank) -> R) -> R {
        use core::sync::atomic::Ordering::{Acquire, Release};
        while self
            .held
            .compare_exchange_weak(false, true, Acquire, core::sync::atomic::Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        struct Unlock<'a>(&'a core::sync::atomic::AtomicBool);
        impl Drop for Unlock<'_> {
            fn drop(&mut self) {
                self.0.store(false, Release);
            }
        }
        let _unlock = Unlock(&self.held);
        // SAFETY: this guard is the sole accessor; action cannot return a bank
        // borrow because its lifetime is not part of the generic return type.
        action(unsafe { &mut *self.bank.get() })
    }
}
static CAMERAS: CameraStore = CameraStore::new();

/// Projection-root publisher. The reference is only an ABI-adapter mapping key;
/// the stored lens and all frame/history values are Rust-owned typed state.
pub fn record_projection(reference: u32, lens: CameraLens) {
    if reference == 0 {
        return;
    }
    CAMERAS.with(|bank| {
        let handle = bank.bind(reference);
        bank.slots[handle.slot as usize].lens = Some(lens);
    });
}
fn record_motion(reference: u32, pose: CameraPose, view_policy: ViewPolicy, fpu: &crate::abi::Fpu) {
    let Some((handle, lens)) = CAMERAS.with(|bank| {
        let handle = bank.bind(reference);
        bank.slot(handle)?.lens.map(|lens| (handle, lens))
    }) else {
        return;
    };
    let view = LookAt {
        eye: pose.eye,
        direction: pose.direction,
        up: pose.up,
    }
    .matrix(view_policy, |value| fp_pc(fpu, value));
    if !view
        .iter()
        .chain(lens.projection.matrix.iter())
        .all(|value| value.is_finite())
    {
        return;
    }
    CAMERAS.with(|bank| {
        if bank.slot(handle).is_none() {
            return;
        }
        let slot = &mut bank.slots[handle.slot as usize];
        // This ordinal counts actual publications, not a guest/gameplay counter.
        let tick = slot.current.map_or(0, |frame| frame.tick.wrapping_add(1));
        let frame = CameraFrame {
            tick,
            pose,
            vertical_fov_degrees: lens.vertical_fov_degrees,
            aspect_ratio: lens.aspect_ratio,
            near: lens.near,
            far: lens.far,
            view,
            projection: lens.projection,
        };
        slot.previous = slot.current.or(Some(frame));
        slot.current = Some(frame);
        bank.latest = Some(handle);
    });
}
/// Native presentation endpoint lookup. One handle per view; a reused legacy
/// mapping slot invalidates old handles instead of returning another camera.
pub fn latest_camera() -> Option<CameraHandle> {
    CAMERAS.with(|bank| bank.latest)
}
/// The explicitly selected native camera. Frame publication alone never changes this selection.
pub fn active_camera() -> Option<CameraHandle> {
    CAMERAS.with(|bank| bank.active.filter(|handle| bank.slot(*handle).is_some()))
}
/// Select a generation-checked camera handle and return the previous valid selection.
pub fn select_active_camera(handle: CameraHandle) -> Option<CameraHandle> {
    CAMERAS.with(|bank| bank.select_active(handle))
}
/// Select a cutscene camera and retain the previous active view in an owned completion token.
pub fn begin_cutscene_camera_handoff(handle: CameraHandle) -> Option<CutsceneCameraHandoff> {
    CAMERAS.with(|bank| bank.begin_cutscene_handoff(handle))
}
/// Restore a cutscene handoff's prior selection if it still owns the active slot.
pub fn finish_cutscene_camera_handoff(handoff: CutsceneCameraHandoff) -> bool {
    CAMERAS.with(|bank| bank.finish_cutscene_handoff(handoff))
}
pub fn camera_for_reference(reference: u32) -> Option<CameraHandle> {
    CAMERAS.with(|bank| {
        bank.slots
            .iter()
            .enumerate()
            .find(|(_, slot)| slot.reference == reference && reference != 0)
            .map(|(slot, value)| CameraHandle {
                slot: slot as u8,
                generation: value.generation,
            })
    })
}
pub fn camera_lens(handle: CameraHandle) -> Option<CameraLens> {
    CAMERAS.with(|bank| bank.slot(handle)?.lens)
}
pub fn sample_camera(handle: CameraHandle, alpha: f32) -> Option<CameraPresentation> {
    let (previous, current, policy) = CAMERAS.with(|bank| {
        let slot = bank.slot(handle)?;
        Some((slot.previous?, slot.current?, slot.lens?.policy))
    })?;
    CameraRig {
        previous: Some(previous),
        current: Some(current),
        ..CameraRig::default()
    }
    .sample(alpha, policy)
}
/// A scene cut or lifecycle reset changes owned history only. The caller uses
/// this when reusing a camera in a new mission or moving it without continuity.
pub fn cut_camera(handle: CameraHandle) -> bool {
    CAMERAS.with(|bank| {
        if bank.slot(handle).is_none() {
            return false;
        }
        let slot = &mut bank.slots[handle.slot as usize];
        slot.previous = slot.current;
        true
    })
}

/// C ABI exported only by the no_std host archive, avoiding duplicate stores
/// when a renderer also links the std ports rlib. The receiver owns the copy.
///
/// # Safety
/// `output` points to one aligned writable CameraPresentation. `reference` is
/// an opaque hybrid mapping key; no guest pointer is dereferenced by sampling.
#[cfg(not(feature = "std"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_camera_sample(
    reference: u32,
    alpha: f32,
    output: *mut CameraPresentation,
) -> u32 {
    if output.is_null() {
        return 0;
    }
    let Some(sample) =
        camera_for_reference(reference).and_then(|handle| sample_camera(handle, alpha))
    else {
        return 0;
    };
    // SAFETY: the caller provides the writable, aligned output stated above.
    unsafe { output.write(sample) };
    1
}

#[cfg(test)]
mod store_tests {
    use super::*;
    #[test]
    fn reused_camera_mapping_invalidates_stale_renderer_handles() {
        let store = CameraStore::new();
        store.with(|bank| {
            let old = bank.bind(1);
            for reference in 2..=9 {
                bank.bind(reference);
            }
            assert!(bank.slot(old).is_none());
            let current = bank.bind(9);
            assert!(bank.slot(current).is_some());
            assert_ne!(old.generation, current.generation);
        });
    }
    #[test]
    fn active_selection_is_explicit_and_independent_of_frame_publication_order() {
        let store = CameraStore::new();
        store.with(|bank| {
            let gameplay = bank.bind(0x100);
            let cutscene = bank.bind(0x200);
            bank.latest = Some(cutscene);
            assert_eq!(bank.active, None);
            assert_eq!(bank.select_active(gameplay), None);
            assert_eq!(bank.active, Some(gameplay));
            assert_eq!(bank.latest, Some(cutscene));
            assert_eq!(bank.select_active(cutscene), Some(gameplay));
            assert_eq!(bank.active, Some(cutscene));
        });
    }
    #[test]
    fn cutscene_handoff_restores_the_previous_camera_when_still_current() {
        let store = CameraStore::new();
        store.with(|bank| {
            let gameplay = bank.bind(0x100);
            let cutscene = bank.bind(0x200);
            bank.select_active(gameplay);
            let handoff = bank.begin_cutscene_handoff(cutscene).unwrap();
            assert_eq!(bank.active, Some(cutscene));
            assert!(bank.finish_cutscene_handoff(handoff));
            assert_eq!(bank.active, Some(gameplay));
        });
    }
    #[test]
    fn stale_cutscene_handoff_cannot_replace_a_newer_selection() {
        let store = CameraStore::new();
        store.with(|bank| {
            let gameplay = bank.bind(0x100);
            let cutscene = bank.bind(0x200);
            let replacement = bank.bind(0x300);
            bank.select_active(gameplay);
            let handoff = bank.begin_cutscene_handoff(cutscene).unwrap();
            bank.select_active(replacement);
            assert!(!bank.finish_cutscene_handoff(handoff));
            assert_eq!(bank.active, Some(replacement));
        });
    }
    #[test]
    fn camera_store_serializes_concurrent_native_publishers() {
        let store = std::sync::Arc::new(CameraStore::new());
        let workers: std::vec::Vec<_> = (1..=4)
            .map(|reference| {
                let store = store.clone();
                std::thread::spawn(move || {
                    for iteration in 0..1000 {
                        store.with(|bank| {
                            let handle = bank.bind(reference);
                            let slot = &mut bank.slots[handle.slot as usize];
                            slot.previous = slot.current;
                            let value = (reference * 10000 + iteration) as f32;
                            slot.current = Some(CameraFrame {
                                tick: iteration as u64,
                                pose: CameraPose {
                                    eye: [value; 3],
                                    ..CameraPose::default()
                                },
                                vertical_fov_degrees: value,
                                aspect_ratio: 1.0,
                                near: 0.1,
                                far: 1000.0,
                                view: [value; 16],
                                projection: CameraProjection {
                                    matrix: [value; 16],
                                    tangent_extents: [value; 2],
                                },
                            });
                        });
                    }
                })
            })
            .collect();
        for _ in 0..4000 {
            store.with(|bank| {
                for slot in bank.slots {
                    if let Some(frame) = slot.current {
                        assert_eq!(frame.pose.eye, [frame.vertical_fov_degrees; 3]);
                        assert_eq!(frame.view, [frame.vertical_fov_degrees; 16]);
                        if let Some(previous) = slot.previous {
                            assert_eq!(frame.tick, previous.tick + 1);
                        }
                    }
                }
            });
        }
        for worker in workers {
            worker.join().unwrap();
        }
    }
}
