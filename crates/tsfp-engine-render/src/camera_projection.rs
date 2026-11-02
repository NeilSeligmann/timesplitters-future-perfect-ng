// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable camera projection setup for 0xC77C0. The guest adapter preserves
//! the original register, stack and cross-system call contract.
use crate::c2r::rt::*;

/// Values read from the viewport and global content aspect before the matrix call.
#[derive(Clone, Copy, Debug)]
pub struct ProjectionInputs {
    pub viewport_width: i32,
    pub viewport_height: i32,
    pub content_scale: f32,
    pub ratio_cap: Option<f32>,
}

impl ProjectionInputs {
    /// Preserve the original f32 operation order, including nonfinite results.
    pub fn aspect_ratio(self) -> f32 {
        let scaled_width = (self.viewport_width as f32) * self.content_scale;
        let ratio = scaled_width / (self.viewport_height as f32);
        match self.ratio_cap {
            // Original unordered SSE comparison leaves NaN unchanged.
            Some(cap) if ratio > cap => cap,
            _ => ratio,
        }
    }
}

/// Typed extension point for T2096. The original preset uses the ratio above;
/// a later Hor+ preset can supply its own ratio without changing guest layout.
#[derive(Clone, Copy, Debug)]
pub struct ProjectionExtents {
    pub vertical: f32,
    pub horizontal: f32,
}

/// Presentation-owned aspect supplied to the camera. `Original` preserves the
/// guest stack scale; HorPlus derives it from a typed full-picture aspect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProjectionPreset {
    Original,
    HorPlus { content_aspect: f32 },
}

/// Camera projection settings after reading the original FOV argument. The
/// presentation system may select a preset without owning the guest layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionSettings {
    pub vertical_fov_degrees: f32,
    pub preset: ProjectionPreset,
}

impl ProjectionPreset {
    pub fn hor_plus(content_aspect: f32) -> Option<Self> {
        (content_aspect.is_finite() && content_aspect > 0.0)
            .then_some(Self::HorPlus { content_aspect })
    }
}

pub fn extents(vertical_tangent: f32, ratio: f32) -> ProjectionExtents {
    ProjectionExtents {
        vertical: vertical_tangent,
        horizontal: vertical_tangent * ratio,
    }
}

/// Guest ABI adapter. 0x151CF0 remains a cross-system matrix writer call.
pub fn run<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, r: &mut ExtRegisters) {
    run_with_preset(m, guest, r, ProjectionPreset::Original);
}

pub fn run_with_preset<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
    preset: ProjectionPreset,
) {
    run_with_preset_and_matrix(m, guest, r, preset, false);
}

/// T2330: optional typed perspective writer, retaining the projection adapter.
/// Default entry keeps its independently selectable matrix callee.
pub fn run_with_preset_and_matrix<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
    preset: ProjectionPreset,
    native_matrix: bool,
) {
    let viewport = m.read32(0x7e71c8);
    let inputs = ProjectionInputs {
        viewport_width: m.read32(viewport.wrapping_add(0xc)) as i32,
        viewport_height: m.read32(viewport.wrapping_add(0x20)) as i32,
        content_scale: match preset {
            ProjectionPreset::Original => f32::from_bits(m.read32(r.esp.wrapping_add(0xc))),
            ProjectionPreset::HorPlus { content_aspect } => content_aspect / (4.0 / 3.0),
        },
        ratio_cap: if m.read32(0x7de458) & 0x4000_0000 != 0 {
            Some(f32::from_bits(m.read32(0x4760cc)))
        } else {
            None
        },
    };
    let ratio = inputs.aspect_ratio();
    r.xmm[1].set_f(0, inputs.viewport_height as f32);
    m.write32(r.esp.wrapping_add(0xc), ratio.to_bits());
    r.xmm[0].set_f(0, ratio);
    if let Some(cap) = inputs.ratio_cap {
        r.xmm[1] = Xmm::scalar(cap.to_bits());
    }

    let matrix_arg_2 = m.read32(r.esp.wrapping_add(8));
    let matrix_arg_1 = m.read32(r.esp.wrapping_add(4));
    let fov_scale =
        f32::from_bits(m.read32(r.esp.wrapping_add(0x10))) * f32::from_bits(m.read32(0x475d0c));
    r.eax = matrix_arg_2;
    r.ecx = matrix_arg_1;
    r.xmm[0] = Xmm::scalar(fov_scale.to_bits());
    r.edx = m.read32(r.esp.wrapping_add(0xc));
    r.push(m, matrix_arg_2);
    r.eax = m.read32(0x7b0cc0);
    r.push(m, matrix_arg_1);
    r.push(m, matrix_arg_1);
    r.ecx = m.read32(r.eax.wrapping_add(0x204));
    m.write32(r.esp, fov_scale.to_bits());
    r.push(m, r.edx);
    r.push(m, r.ecx);
    if native_matrix {
        let input = crate::camera::Perspective {
            vertical_fov_radians: fov_scale,
            aspect_ratio: ratio,
            near: f32::from_bits(matrix_arg_1),
            far: f32::from_bits(matrix_arg_2),
        };
        let matrix = input.matrix(
            f32::from_bits(m.read32(0x475cd4)),
            f32::from_bits(m.read32(0x4760d0)),
            |value| fp_pc(&r.fpu, value),
        );
        for (i, value) in matrix.into_iter().enumerate() {
            m.write32(r.ecx + i as u32 * 4, value.to_bits());
        }
        let depth = input.far / (input.far - input.near);
        r.eax = r.ecx;
        r.xmm[0] = Xmm::default();
        r.xmm[1] = Xmm::scalar(depth.to_bits());
        r.xmm[2] = Xmm::scalar((0.0 - depth * input.near).to_bits());
        r.xmm[3] = Xmm::scalar(f32::from_bits(m.read32(0x4760d0)).to_bits());
    } else {
        r.call(guest, 0x151cf0);
    }

    let settings = ProjectionSettings {
        vertical_fov_degrees: f32::from_bits(m.read32(r.esp.wrapping_add(0x24))),
        preset,
    };
    r.fpu.push(settings.vertical_fov_degrees as f64);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * f32::from_bits(m.read32(0x475cd4)) as f64,
        ),
    );
    r.edx = m.read32(0x7b0cc0);
    r.esp = r.esp.wrapping_add(0x14);
    r.fpu.set_st(
        0,
        fp_pc(
            &r.fpu,
            r.fpu.st(0) * f32::from_bits(m.read32(0x475d0c)) as f64,
        ),
    );
    r.fpu.set_st(0, tan(r.fpu.st(0)));
    r.fpu.push(1.0);
    r.fpu.pop();
    m.write32(r.edx.wrapping_add(0x37c), (r.fpu.st(0) as f32).to_bits());
    r.fpu.pop();
    r.eax = m.read32(0x7b0cc0);
    let vertical = f32::from_bits(m.read32(r.eax.wrapping_add(0x37c)));
    r.xmm[0] = Xmm::scalar(vertical.to_bits());
    let result = extents(vertical, f32::from_bits(m.read32(r.esp.wrapping_add(0xc))));
    r.xmm[0].set_f(0, result.horizontal);
    m.write32(r.eax.wrapping_add(0x380), result.horizontal.to_bits());
    if native_matrix {
        let destination = m.read32(r.eax.wrapping_add(0x204));
        crate::camera::record_projection(
            r.eax,
            crate::camera::CameraLens {
                vertical_fov_degrees: settings.vertical_fov_degrees,
                aspect_ratio: ratio,
                near: f32::from_bits(matrix_arg_1),
                far: f32::from_bits(matrix_arg_2),
                projection: crate::camera::CameraProjection {
                    matrix: core::array::from_fn(|i| {
                        f32::from_bits(m.read32(destination + i as u32 * 4))
                    }),
                    tangent_extents: [result.vertical, result.horizontal],
                },
                policy: crate::camera::ProjectionPolicy {
                    radians_per_degree: f32::from_bits(m.read32(0x475d0c)),
                    half: f32::from_bits(m.read32(0x475cd4)),
                    homogeneous_axis: f32::from_bits(m.read32(0x4760d0)),
                },
            },
        );
    }
}
