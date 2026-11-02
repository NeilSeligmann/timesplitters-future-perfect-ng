//! T1873: the matrix blend `0x1526C0(a, b, out, t)` with its callees, MEASURED bit exact against the original
//! under Unicorn (`tools/t1873_blend_gold.py`, `tests/blend_golden.rs`).
//!
//! * [`matrix_to_quaternion`] is `0x152540(out, matrix)`: the usual trace / largest diagonal conversion. The square
//!   roots run through x87 at 53 bits (control word `0x27F`), so they are computed in f64 and rounded to f32 twice
//!   exactly where the original stores them.
//! * [`slerp`] is `0x150960(p, q, t, out)`: shortest arc quaternion interpolation. `acos` is the CRT helper
//!   `0x3C9CC4` (`atan2(sqrt((1 + x)(1 - x)), x)` on the x87 stack), the sines are `fsin`, `rsqrtss` is `1 / sqrt`
//!   in f32. QEMU evaluates `fsin` and `fpatan` with the host double libm, so [`f64::sin`] and [`f64::atan2`] are
//!   used (a glibc dependency: results of the golden come from glibc, other libms may differ in the last bit).
//! * [`matrix_blend`] is `0x1526C0`: both rotations to quaternions, slerp, back to a matrix, translation row
//!   interpolated linearly. INFERRED: the arguments are `(a, b, out, t)`, T1872 labelled them `(out, a, b, t)`.
use crate::fmath::{atan2_f64, sin_f64, sqrt_f64};
use crate::posemath::{Mat4, quat_matrix};

use core::sync::atomic::{AtomicBool, Ordering};

/// x87 precision control of the running guest (T2061). The goldens above ran with control word `0x27F` (53 bit mantissa), but the live
/// game runs with `0x3F` (24 bit): there every `fadd` / `fsub` / `fmul` / `fsqrt` of the original `slerp` rounds to f32 (the lifted
/// `RECOMP_FP_PC`), while `fsin` and `fpatan` stay unrounded and the CRT `acos` helper `0x3C9CC4` switches to 53 bit itself. The lanes call
/// [`set_single_precision`] with the control word of the call before running a port that reaches [`slerp`].
static SINGLE_PRECISION: AtomicBool = AtomicBool::new(false);

/// `true` when bits 8..9 of the x87 control word are 0 (24 bit mantissa).
pub fn set_single_precision(control_word: u16) {
    SINGLE_PRECISION.store(control_word & 0x300 == 0, Ordering::Relaxed);
}

const ONE: f32 = 1.0;
const HALF: f32 = 0.5;
const EPSILON: f32 = f32::from_bits(0x3400_0000); // 0x4785B0, 2^-23
const HALF_PI: f32 = f32::from_bits(0x3FC9_0FDB); // 0x475CE4

/// `0x152540(out, matrix)`: rotation matrix (row major, cells 0, 5, 10 diagonal) to a quaternion `[x, y, z, w]`.
pub fn matrix_to_quaternion(m: &Mat4) -> [f32; 4] {
    let trace = m[5] + m[0] + m[10];
    if trace > 0.0 {
        let root = sqrt_f64(f64::from(trace) + 1.0);
        let inverse = HALF / (root as f32);
        return [
            (m[6] - m[9]) * inverse,
            (m[8] - m[2]) * inverse,
            (m[1] - m[4]) * inverse,
            (root * 0.5) as f32,
        ];
    }
    let mut i = 0;
    if m[5] > m[0] {
        i = 1;
    }
    if m[10] > m[5 * i] {
        i = 2;
    }
    let next = [1usize, 2, 0];
    let (j, k) = (next[i], next[next[i]]);
    let root = sqrt_f64(f64::from(m[5 * i]) - (f64::from(m[5 * k]) + f64::from(m[5 * j])) + 1.0);
    let inverse = HALF / (root as f32);
    let mut q = [0.0f32; 4];
    q[i] = (root * 0.5) as f32;
    q[j] = (m[4 * j + i] + m[4 * i + j]) * inverse;
    q[k] = (m[4 * k + i] + m[4 * i + k]) * inverse;
    q[3] = (m[4 * j + k] - m[4 * k + j]) * inverse;
    q
}

/// `acos` of the CRT helper `0x3C9CC4` for `|x| < 1`, the f32 the caller stores. Other inputs never reach it (the
/// callers guard with `1 + x` and `1 - x` above `2^-23`).
fn crt_acos(x: f32, single: bool) -> f32 {
    if single {
        // the helper runs under the caller's control word: `fadd`, `fsub`, `fmulp` and `fsqrt` each round to f32, `fpatan` does not
        let product = (ONE + x) * (ONE - x);
        return atan2_f64(f64::from(sqrt_f64(f64::from(product)) as f32), f64::from(x)) as f32;
    }
    let x = f64::from(x);
    let product = (1.0 + x) * (1.0 - x);
    atan2_f64(sqrt_f64(product), x) as f32
}

/// `0x150960(p, q, t, out)`: `t == 0` returns `p`, `t == 1` returns `q` (before any sign flip), otherwise the shortest
/// arc between them (`q` is negated when it lies in the opposite hemisphere).
pub fn slerp(p: [f32; 4], q: [f32; 4], t: f32) -> [f32; 4] {
    if t == 0.0 {
        return p;
    }
    if t == ONE {
        return q;
    }
    let difference = ((((q[3] - p[3]) * (q[3] - p[3])) + ((q[2] - p[2]) * (q[2] - p[2])))
        + ((q[1] - p[1]) * (q[1] - p[1])))
        + ((q[0] - p[0]) * (q[0] - p[0]));
    let sum = (((q[3] + p[3]) * (q[3] + p[3])) + ((q[2] + p[2]) * (q[2] + p[2])))
        + ((q[1] + p[1]) * (q[1] + p[1]))
        + ((q[0] + p[0]) * (q[0] + p[0]));
    let single = SINGLE_PRECISION.load(Ordering::Relaxed);
    let b = if difference > sum {
        [0.0 - q[0], 0.0 - q[1], 0.0 - q[2], 0.0 - q[3]]
    } else {
        q
    };
    let dot = ((p[3] * b[3] + p[0] * b[0]) + p[1] * b[1]) + b[2] * p[2];
    let (w_p, w_b) = if dot + ONE > EPSILON {
        if ONE - dot > EPSILON {
            let theta = crt_acos(dot, single);
            let inverse_sine = ONE / sqrt_f64(f64::from(ONE - dot * dot)) as f32;
            let weight = |angle: f64| (sin_f64(angle) * f64::from(inverse_sine)) as f32;
            let (first, second) = if single {
                // 24 bit control word: `1 - t` and the products with `theta` are rounded to f32 before `fsin`
                (
                    weight(f64::from((ONE - t) * theta)),
                    weight(f64::from(t * theta)),
                )
            } else {
                (
                    weight((1.0 - f64::from(t)) * f64::from(theta)),
                    weight(f64::from(t) * f64::from(theta)),
                )
            };
            (first, second)
        } else {
            (ONE - t, t)
        }
    } else {
        let half_pi = f64::from(HALF_PI);
        let (first, second) = if single {
            (
                sin_f64(f64::from((ONE - t) * HALF_PI)),
                sin_f64(f64::from(t * HALF_PI)),
            )
        } else {
            (
                sin_f64((1.0 - f64::from(t)) * half_pi),
                sin_f64(f64::from(t) * half_pi),
            )
        };
        if single {
            return [0, 1, 2, 3]
                .map(|c| ((first * f64::from(p[c])) as f32) - ((second * f64::from(b[c])) as f32));
        }
        return [0, 1, 2, 3].map(|c| (first * f64::from(p[c]) - second * f64::from(b[c])) as f32);
    };
    [
        p[0] * w_p + w_b * b[0],
        p[1] * w_p + w_b * b[1],
        b[2] * w_b + w_p * p[2],
        p[3] * w_p + w_b * b[3],
    ]
}

/// `0x1526C0(a, b, out, t)`: blend of two pose matrices (rotation by slerp of their quaternions, translation row
/// linear), all sixteen cells of the result are written.
pub fn matrix_blend(a: &Mat4, b: &Mat4, t: f32) -> Mat4 {
    let from_b = matrix_to_quaternion(b);
    let from_a = matrix_to_quaternion(a);
    let blended = slerp(from_a, from_b, t);
    let mut out = quat_matrix(blended);
    let weight = ONE - t;
    for cell in 12..15 {
        out[cell] = (a[cell] - b[cell]) * weight + b[cell];
    }
    out
}
