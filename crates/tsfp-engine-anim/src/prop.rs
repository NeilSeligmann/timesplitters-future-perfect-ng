//! Single node evaluators for props, `0x5F260` and `0x5F040` (T1668). MEASURED against the ORIGINAL under Unicorn with the time step
//! `0x5ADC0` and the key sampler `0x59320` stubbed (`tools/t1668_prop_gold.py`, `tests/prop_golden.rs`, 516 cases; the bracket `0x584A0`,
//! slerp and matrix routines are the real code).
//!
//! Both do the work of the `0x5AAA0` core for node 0 of the clip of a layer state at its current time: key bracket, two samples, slerp of
//! the quaternions, rotation matrix, position lerp ([`crate::interp::interpolate_node`] does the same on a parsed clip).
//! * `0x5F260(state, matrix)`: the layer state is stepped (`0x5ADC0(state, 1.0, 0)`), the matrix gets the rotation and the lerped position
//!   in cells 12..14 ([`bare_pose`]).
//! * `0x5F040(body)`: steps the layer at `body+0xD4` (the model first gets `+0x70 = [model+0xC4]`), writes the rotation to `[body+0x10]`, copies
//!   it to all `N` matrices (`N = [[body+4]+4]`), zeroes the translation row of the first one and stores the lerped position in the model
//!   (`model+0x64..0x6C`, `model = [body]`): the prop moves, its matrices do not ([`prop_body_pose`]). `0x5F430(obj)` wraps it and sets flags.
use crate::blend::slerp;
use crate::keybracket::{bracket_weight, key_bracket};
use crate::posemath::{Mat4, Sample, quat_matrix};

/// Rotation matrix (translation row zero) and lerped position of node 0 at `time`; `None` when the bracket does not exist (NaN time,
/// empty key table). `sampler(key)` is `0x59320` for node 0 at scale 1.0.
pub fn bare_pose(
    keys: &[u32],
    duration: f32,
    time: f32,
    mut sampler: impl FnMut(i32) -> Sample,
) -> Option<(Mat4, [f32; 3])> {
    let (low, high) = key_bracket(keys, duration, time)?;
    let weight = bracket_weight(keys, low, high, time)?;
    let first = sampler(low);
    let second = sampler(high);
    let matrix = quat_matrix(slerp(first.quaternion(), second.quaternion(), weight));
    let (a, b) = (first.position(), second.position());
    let position = [0, 1, 2].map(|axis| (b[axis] - a[axis]) * weight + a[axis]);
    Some((matrix, position))
}

/// `0x5F260`: the matrix carries the position in cells 12..14.
pub fn state_matrix(
    keys: &[u32],
    duration: f32,
    time: f32,
    sampler: impl FnMut(i32) -> Sample,
) -> Option<Mat4> {
    let (mut matrix, position) = bare_pose(keys, duration, time, sampler)?;
    matrix[12..15].copy_from_slice(&position);
    Some(matrix)
}

/// `0x5F040`: fills `out` (the `N` matrices) with the rotation (translation rows zero) and returns the position destined for the model.
pub fn prop_body_pose(
    keys: &[u32],
    duration: f32,
    time: f32,
    out: &mut [Mat4],
    sampler: impl FnMut(i32) -> Sample,
) -> Option<[f32; 3]> {
    let (mut matrix, position) = bare_pose(keys, duration, time, sampler)?;
    matrix[12..15].fill(0.0);
    out.fill(matrix);
    Some(position)
}
