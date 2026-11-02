// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable camera producer. Only this legacy edge marshals guest state/services.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../../tsfp-gameplay/src/cutscene_original_core.rs"]
mod model;
#[link(name = "m")]
unsafe extern "C" {
    fn atan2(y: f64, x: f64) -> f64;
}
fn angle(y: f64, x: f64) -> f64 {
    unsafe { atan2(y, x) }
}
use model::{CameraKey, Extent, LensCalibration, original_projection_ratio, select_keys};
fn float<M: GuestMemory>(m: &M, at: u32) -> f32 {
    f32::from_bits(m.read32(at))
}
fn call<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    c: &mut C,
    r: &mut ExtRegisters,
    va: u32,
    args: &[u32],
) {
    for &arg in args.iter().rev() {
        r.esp = r.esp.wrapping_sub(4);
        m.write32(r.esp, arg);
    }
    let ebp = r.ebp;
    c.call(va, r);
    r.ebp = ebp;
    r.esp = r.esp.wrapping_add(args.len() as u32 * 4);
}
fn key<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    c: &mut C,
    r: &mut ExtRegisters,
    index: i32,
    at: u32,
) -> CameraKey {
    call(m, c, r, 0x60740, &[index as u32, at, at + 16, at + 28]);
    CameraKey {
        rotation: core::array::from_fn(|i| float(m, at + i as u32 * 4)),
        position: core::array::from_fn(|i| float(m, at + 16 + i as u32 * 4)),
        lens: float(m, at + 28),
    }
}
/// Original 0xC95E0: animation lookup, quaternion services and camera projection
/// remain neighbouring contracts; selection/interpolation/optics are typed Rust.
// portreg: va=0x000c95e0 tier=1
// portreg: replaces=tsfp-ports/c2r/t2049_exec_009
pub fn apply_original<M: GuestMemory, C: GuestCall>(m: &mut M, c: &mut C, r: &mut ExtRegisters) {
    let entry = *r;
    r.esp = r.esp.wrapping_sub(160);
    let scratch = r.esp;
    call(m, c, r, 0x70330, &[1]);
    let record = r.eax;
    if record != 0 && m.read32(0x6d780c) != 0 {
        let time = float(m, record + 8);
        let rate = float(m, 0x4d1994);
        let count = m.read32(record) as i32;
        let midpoint = float(m, 0x475cd4);
        if let Some(mut selection) = select_keys(time, rate, count, false, midpoint) {
            let first = key(m, c, r, selection.index, scratch);
            let next = key(m, c, r, selection.index.wrapping_add(1), scratch + 32);
            call(m, c, r, 0x60920, &[record]);
            if r.eax != 0 {
                selection.fraction = if midpoint > selection.fraction {
                    0.0
                } else {
                    1.0
                };
            }
            call(
                m,
                c,
                r,
                0x150960,
                &[
                    scratch,
                    scratch + 32,
                    selection.fraction.to_bits(),
                    scratch + 64,
                ],
            );
            call(m, c, r, 0x1506b0, &[scratch + 80, scratch + 64]);
            let matrix: [u32; 12] = core::array::from_fn(|i| m.read32(scratch + 80 + i as u32 * 4));
            let position: [f32; 3] = core::array::from_fn(|i| {
                (next.position[i] - first.position[i]) * selection.fraction + first.position[i]
            });
            let lens = (next.lens - first.lens) * selection.fraction + first.lens;
            let camera = m.read32(0x7b0cc0);
            let matrix_out = m.read32(camera + 0x208);
            for (i, value) in matrix.iter().enumerate() {
                m.write32(matrix_out + i as u32 * 4, *value);
            }
            let translation_scale = float(m, 0x47d630);
            for (i, value) in position.iter().enumerate() {
                m.write32(
                    matrix_out + 48 + i as u32 * 4,
                    (value * translation_scale).to_bits(),
                );
            }
            m.write32(matrix_out + 60, 1.0f32.to_bits());
            let calibration = LensCalibration {
                lens_scale: float(m, 0x47d62c),
                numerator: float(m, 0x4765e0),
                aperture_scale: float(m, 0x47d628),
                aperture: float(m, 0x47d61c),
                angle_units: float(m, 0x478638),
                wide_scale: float(m, 0x47d624),
            };
            call(m, c, r, 0x3c420, &[]);
            let fov = calibration.original_fov_with(lens, r.eax as u8 != 0, angle);
            let ratio = original_projection_ratio(
                Extent {
                    width: m.read32(0x4c0458) as i32,
                    height: m.read32(0x4c045c) as i32,
                },
                Extent {
                    width: m.read32(0x4e8cf0) as i32,
                    height: m.read32(0x4e8cf4) as i32,
                },
            );
            call(
                m,
                c,
                r,
                0xc77c0,
                &[
                    m.read32(0x4dcb08),
                    m.read32(0x4dcb0c),
                    ratio.to_bits(),
                    fov.to_bits(),
                ],
            );
            m.write32(camera + 0x384, fov.to_bits());
            let position_scale = float(m, 0x475d1c);
            for (i, value) in position.iter().enumerate() {
                let value = (value * position_scale).to_bits();
                m.write32(camera + 0x27c + i as u32 * 4, value);
                m.write32(camera + 0x288 + i as u32 * 4, value);
                m.write32(camera + 0x334 + i as u32 * 4, matrix[i]);
                m.write32(camera + 0x340 + i as u32 * 4, matrix[i]);
                m.write32(camera + 0x364 + i as u32 * 4, matrix[i + 4]);
            }
            call(
                m,
                c,
                r,
                0xc3c50,
                &[camera + 0x27c, 1.0f32.to_bits(), 0.5f32.to_bits()],
            );
            let old = m.read32(camera + 0x2a4);
            let tag = if r.eax != 0 {
                r.eax
            } else if old != 0 {
                old
            } else {
                1
            };
            m.write32(camera + 0x2a0, tag);
            m.write32(camera + 0x2a4, tag);
        }
    }
    r.esp = entry.esp;
    r.ebx = entry.ebx;
    r.ebp = entry.ebp;
    r.esi = entry.esi;
    r.edi = entry.edi;
}
