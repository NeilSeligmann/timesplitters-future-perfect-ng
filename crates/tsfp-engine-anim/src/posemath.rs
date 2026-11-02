// SPDX-License-Identifier: GPL-3.0-or-later
//! `core` only pose math split out of `pose.rs` (T1918) so the `no_std` `tsfp-ports` crate can reuse it: matrices, samples, `0x153270`, `0x1506B0`.
pub type Mat4 = [f32; 16];

const ONE: f32 = f32::from_bits(0x3F80_0000); // 0x475C78
const TWO: f32 = f32::from_bits(0x4000_0000); // 0x475D10

/// One decoded sample as raw f32 bits (position x y z, quaternion x y z w).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sample {
    pub bits: [u32; 7],
}

impl Sample {
    pub fn position(&self) -> [f32; 3] {
        [0, 1, 2].map(|i| f32::from_bits(self.bits[i]))
    }
    pub fn quaternion(&self) -> [f32; 4] {
        [3, 4, 5, 6].map(|i| f32::from_bits(self.bits[i]))
    }
}

/// `0x153270`: `a * b`, row major, products accumulated left to right (all inputs read before the result).
pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0f32; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut acc = a[row * 4] * b[col];
            acc += a[row * 4 + 1] * b[4 + col];
            acc += a[row * 4 + 2] * b[8 + col];
            acc += a[row * 4 + 3] * b[12 + col];
            out[row * 4 + col] = acc;
        }
    }
    out
}

/// `0x1506B0`: quaternion (x, y, z, w; scaled by `2 / |q|^2`) to a rotation matrix, translation row zero.
pub fn quat_matrix(q: [f32; 4]) -> Mat4 {
    let [x, y, z, w] = q;
    let norm = (x * x + y * y + z * z) + w * w;
    let inverse = TWO / norm;
    let (zs, xs, ys) = (z * inverse, x * inverse, y * inverse);
    let (yw, zw, xx, zz, xy, yy) = (ys * w, zs * w, x * xs, z * zs, x * ys, y * ys);
    let (yz, xw, xz) = (y * zs, xs * w, x * zs);
    [
        ONE - (zz + yy),
        xy + zw,
        xz - yw,
        0.0,
        xy - zw,
        ONE - (zz + xx),
        yz + xw,
        0.0,
        xz + yw,
        yz - xw,
        ONE - (yy + xx),
        0.0,
        0.0,
        0.0,
        0.0,
        ONE,
    ]
}

/// The local matrix of a sample as `0x976E0` fills it: rotation from the quaternion, translation row from the
/// position.
pub fn local_matrix(sample: &Sample) -> Mat4 {
    let mut matrix = quat_matrix(sample.quaternion());
    matrix[12..15].copy_from_slice(&sample.position());
    matrix
}
