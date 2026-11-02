//! Clip time interpolation (T1875): the key bracket `0x584A0` and the two key interpolation core of `0x5AAA0`.
//!
//! * [`key_bracket`] = `0x584A0` (custom convention: `ebx` clip, `xmm2` time, two `i32*` outputs on the stack): the
//!   pair of key indices around a time. Key `i` happens at `f32(key_frames[i]) * (1/60)` seconds (frames at 60 Hz).
//!   `time <= 0` or a single key gives `(0, 0)`, `time >= duration` gives `(K-1, K-1)` with the duration the loader
//!   `0x57F30` stores at `clip+0x18` ([`clip_duration`] = `(frame count - 1) / 60`), an exact key hit gives the same index
//!   twice, otherwise a binary search with the original's rounding of the midpoint. A NaN time loops forever in the
//!   original; here it is `None`, as is a bracket that would read past the key table.
//! * [`bracket_weight`] = the weight of `0x5AAA0` (`0x5AB3B..0x5AB6C`): `(t - t0) / (t1 - t0)`, 0 when the keys coincide.
//! * [`interpolate_node`] = `0x5AAA0` between the bracket and the chain call (`0x5ABBD..0x5AD09`): both keys sampled with
//!   `0x59320` (node as given, the original passes 0), quaternions slerped (`0x150960`) into the rotation matrix
//!   (`0x1506B0`), translation `(p1 - p0) * w + p0` per component into cells 12..14. `0x60670` (time advance of the
//!   state at `obj + 0xD4`), `0x60920` (gate of the optional weight snap) and the part chain `0x582A0` are outside.
//!
//! Validation: goldens of the ORIGINAL under Unicorn, `tools/t1875_interp_gold.py`, tests `tests/interp_golden.rs`
//! (fabricated key tables and real clips of the owner disc, numbers only).
use crate::blend::{matrix_to_quaternion, slerp};
use crate::pose::{Mat4, PoseError, quat_matrix, sample};
use tsfp_format_anim::Clip;

// the pure bracket helpers live in keybracket.rs (core only, shared with the no_std tsfp-ports crate)
pub use crate::keybracket::{
    FRAME_SECONDS, bracket_weight, bracket_weight_by, half, key_bracket, key_bracket_by, key_time,
};
/// `0x475CD4`: weights below this snap to 0, others to 1 when the snap gate is on.
const SNAP_THRESHOLD: f32 = 0.5;

/// `clip+0x18` as `0x57F30` computes it: `(frame count - 1.0) / 60.0` (frame count = header dword 3, signed).
pub fn clip_duration(clip: &Clip) -> f32 {
    (clip.header[3] as i32 as f32 - 1.0f32) / 60.0f32
}

/// Result of [`interpolate_node`]: the local matrix and the pieces it came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interpolated {
    pub low: i32,
    pub high: i32,
    pub weight: f32,
    pub matrix: Mat4,
}

/// `0x5AAA0` core for one node at `time`. `snap` mirrors the optional gate (`[0x7DE458] & 0x20000000` and `0x60920`):
/// a weight below 0.5 becomes 0, otherwise 1.
pub fn interpolate_node(
    clip: &Clip,
    node: usize,
    time: f32,
    scale: f32,
    snap: bool,
) -> Result<Option<Interpolated>, PoseError> {
    let Some((low, high)) = key_bracket(&clip.key_frames, clip_duration(clip), time) else {
        return Ok(None);
    };
    let Some(mut weight) = bracket_weight(&clip.key_frames, low, high, time) else {
        return Ok(None);
    };
    if snap {
        weight = if SNAP_THRESHOLD > weight { 0.0 } else { 1.0 };
    }
    let first = sample(clip, node, low, scale)?;
    let second = sample(clip, node, high, scale)?;
    let mut matrix = quat_matrix(slerp(first.quaternion(), second.quaternion(), weight));
    let (a, b) = (first.position(), second.position());
    for axis in 0..3 {
        matrix[12 + axis] = (b[axis] - a[axis]) * weight + a[axis];
    }
    Ok(Some(Interpolated {
        low,
        high,
        weight,
        matrix,
    }))
}

/// Rotation half of `0x1526C0` (`0x152540` on both inputs, `0x150960`, then `0x1506B0` into the destination): the
/// sixteen cells the original writes before it reads the inputs' translation rows again. A caller working on guest
/// memory must write this first and re-read the translation cells (`a` or `b` may alias the destination, then the
/// original sees the zeroed row: MEASURED by the regenerated T1872 `pose_blend` goldens).
pub fn blend_rotation(a: &Mat4, b: &Mat4, t: f32) -> Mat4 {
    let from_b = matrix_to_quaternion(b);
    let from_a = matrix_to_quaternion(a);
    quat_matrix(slerp(from_a, from_b, t))
}

/// Translation component of `0x1526C0` (`0x152716..0x152762`): `(a - b) * (1 - t) + b`.
pub fn blend_translation(a: f32, b: f32, t: f32) -> f32 {
    (a - b) * (1.0f32 - t) + b
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [u32; 5] = [0, 10, 20, 40, 60];

    #[test]
    fn bracket_edges_and_exact_hits() {
        let duration = 59.0f32 / 60.0;
        assert_eq!(key_bracket(&KEYS, duration, 0.0), Some((0, 0)));
        assert_eq!(key_bracket(&KEYS, duration, -3.0), Some((0, 0)));
        assert_eq!(key_bracket(&[7], duration, 5.0), Some((0, 0)));
        assert_eq!(key_bracket(&KEYS, duration, 5.0), Some((4, 4)));
        assert_eq!(key_bracket(&KEYS, 2.0, 40.0 / 60.0), Some((3, 3)));
        assert_eq!(key_bracket(&KEYS, 2.0, 15.0 / 60.0), Some((1, 2)));
        assert_eq!(key_bracket(&KEYS, 2.0, 50.0 / 60.0), Some((3, 4)));
        assert_eq!(key_bracket(&KEYS, 2.0, f32::NAN), None);
        assert_eq!(key_bracket(&[], 2.0, 1.0), None);
    }

    #[test]
    fn indexed_bracket_reads_past_the_old_adapter_prefix_without_copying() {
        let mut reads = 0;
        let (low, high) = key_bracket_by(7_200, 120.0, 256.0 / 60.0, |index| {
            reads += 1;
            Some(index as u32)
        })
        .unwrap();
        assert_eq!((low, high), (256, 256));
        assert_eq!(
            bracket_weight_by(low, high, 256.0 / 60.0, |index| { Some(index as u32) }),
            Some(0.0)
        );
        assert!(reads < 32, "binary search should not copy all 7,200 rows");
    }

    #[test]
    fn weight_is_linear_in_key_time() {
        assert_eq!(bracket_weight(&KEYS, 1, 1, 0.3), Some(0.0));
        let w = bracket_weight(&KEYS, 1, 2, 15.0 / 60.0).unwrap();
        assert!((w - 0.5).abs() < 1e-6);
        assert_eq!(bracket_weight(&KEYS, 1, 9, 0.3), None);
    }

    #[test]
    fn midpoint_rounds_to_zero() {
        assert_eq!((half(5), half(4), half(0), half(-3)), (2, 2, 0, -1));
    }
}
