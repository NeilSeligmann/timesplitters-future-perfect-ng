//! Pose bounds query `0x60F80` (T1668, rule 10 batch 5): the axis aligned bounds of the node translations of an animation at its current time,
//! evaluated into temporary arena matrices. Tier 1 via `tests/bounds_golden.rs` (seeded worlds against the original under Unicorn, the sampler,
//! snap gate, arena allocator and tree walk stubbed).
//!
//! `0x60F80(owner, index, instance, out_min, out_max)`: same time rule, bracket, weight and per node pose as [`crate::instance::evaluate_instance`]
//! (including the root motion rule for node 0 translation) but into `0x3E80` bytes taken from the temporary arena (`0x3DFA0`) and without touching the
//! root fields or the base pose. After `0x604D0` composes the tree, the minimum and maximum of every matrix translation (cells 12..14) are written
//! (start values `+-f32::MAX`), the arena block is released (`0x3E1E0`, `instance+4 = 0`) and the layer time is restored to its value before the call.
use crate::advance::advance_time;
use crate::blend::slerp;
use crate::keybracket::{bracket_weight, key_bracket};
use crate::posemath::{Mat4, Sample, quat_matrix};
use crate::tail::tracks_root_motion;

/// Inputs that are not memory of the instance.
pub struct BoundsClip<'a> {
    pub keys: &'a [u32],
    pub duration: f32,
    pub nodes: usize,
}

/// Returns `(min, max)`; `layer_time` is restored; `scratch` is the temporary matrix block (at least `clip.nodes` entries). `compose_tree` is `0x604D0` over the temporary matrices. `None` when the bracket is missing.
#[allow(clippy::too_many_arguments)]
pub fn pose_bounds(
    clip: &BoundsClip,
    owner_time: f32,
    track_offset: f32,
    policy: (u32, u32),
    snap: bool,
    layer_time: &mut f32,
    mut sampler: impl FnMut(usize, i32) -> Sample,
    scratch: &mut [Mat4],
    compose_tree: impl FnOnce(&mut [Mat4]),
) -> Option<([f32; 3], [f32; 3])> {
    let saved_time = *layer_time;
    if clip.duration > 0.0 {
        *layer_time = advance_time(clip.duration, owner_time, track_offset, *layer_time)?;
    }
    let (low, high) = key_bracket(clip.keys, clip.duration, *layer_time)?;
    let mut weight = bracket_weight(clip.keys, low, high, *layer_time)?;
    if snap {
        weight = if 0.5 > weight { 0.0 } else { 1.0 };
    }
    let root_motion = tracks_root_motion(policy.0, policy.1);
    for node in 0..clip.nodes {
        let (first, second) = (sampler(node, low), sampler(node, high));
        let mut matrix = quat_matrix(slerp(first.quaternion(), second.quaternion(), weight));
        let (a, b) = (first.position(), second.position());
        let lerp = [0, 1, 2].map(|axis| (b[axis] - a[axis]) * weight + a[axis]);
        if node > 0 || root_motion {
            matrix[12..15].copy_from_slice(&lerp);
        } else {
            matrix[12] = 0.0;
            matrix[13] = lerp[1];
            matrix[14] = 0.0;
        }
        scratch[node] = matrix;
    }
    let matrices = &mut scratch[..clip.nodes];
    compose_tree(matrices);
    let (mut min, mut max) = ([f32::MAX; 3], [-f32::MAX; 3]);
    for matrix in matrices.iter() {
        for axis in 0..3 {
            let cell = matrix[12 + axis];
            if min[axis] > cell {
                min[axis] = cell;
            }
            if cell > max[axis] {
                max[axis] = cell;
            }
        }
    }
    *layer_time = saved_time;
    Some((min, max))
}
