//! Full animation instance evaluation `0x60B30` (T1668). MEASURED against the ORIGINAL under Unicorn with the snap gate `0x60920`,
//! the sampler `0x59320` and the post steps `0x60540`, `0x604D0`, `0x605D0` stubbed (their calls are recorded and compared;
//! `tools/t1668_instance_gold.py`, `tests/instance_golden.rs`, 449 cases). Real: bracket `0x585B0`, slerp, matrices, copy.
//!
//! `0x60B30(owner, index, instance, save_base)`:
//! 1. time: the layer at `instance+0x10` (`+4` slot, `+8` time) takes `owner+8 - owner[index * 0x2C + 0x34]`, clamped to the clip duration and
//!    wrapped up when negative ([`crate::advance::advance_time`], inlined in the original).
//! 2. bracket the time (`0x585B0`), weight as in `0x5AAA0`; with the snap gate the weight becomes 0 (below 0.5) or 1.
//! 3. for every clip node `n`: sample both keys, slerp, rotation matrix into `instance.matrices[n]` (`instance+4`). Translation of node 0 is
//!    the lerped position kept in `instance+0x20..0x28` when the clip tracks root motion (`policy0 & 0x82` or `policy1 & 0x1800`), else
//!    only its Y component survives (cells 12 and 14 zero). Other nodes get the full lerped translation.
//! 4. `save_base != 0`: the first `model+0x1C` evaluated matrices are copied to the model's base pose (`model+0x2C`, the `from` side of the
//!    weight blend `0x60540`).
//! 5. post steps in order: `0x60540(save_base)`, `0x604D0(instance, matrices, 0)`, `0x605D0()` ([`PostStep`]).
//! 6. root motion bookkeeping like the `0x5AAA0` tail: `delta (+0x38..0x40) = position (+0x20) - previous (+0x2C)`, previous = position;
//!    else delta zero.
use crate::advance::advance_time;
use crate::blend::slerp;
use crate::keybracket::{bracket_weight, key_bracket};
use crate::posemath::{Mat4, Sample, quat_matrix};
use crate::tail::tracks_root_motion;

/// The steps the original calls after the matrices are built (implemented by `posemix`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostStep {
    BlendByWeight { full: bool },
    ComposeTree,
    ComposeOrCopy,
}

/// Clip data the evaluation reads.
pub struct ClipView<'a> {
    pub keys: &'a [u32],
    pub duration: f32,
    pub nodes: usize,
}

/// The parts of the instance memory the routine writes.
#[derive(Debug, PartialEq)]
pub struct Instance<'a> {
    /// `instance+0x18`: layer time.
    pub layer_time: f32,
    /// Dwords `+0x20..+0x44`: position (3), previous position (3), delta (3).
    pub root: [f32; 9],
    /// `[instance+4]` matrices.
    pub matrices: &'a mut [Mat4],
    /// `[model+0x2C]` base pose.
    pub base: &'a mut [Mat4],
    /// `[model+0x1C]` number of matrices saved to the base pose.
    pub saved_count: usize,
}

/// `0x60B30`. `policy` = the clip's policy words; `sampler(node, key)` = `0x59320` at scale 1.0. `None` when the bracket does not exist.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_instance(
    clip: &ClipView,
    owner_time: f32,
    track_offset: f32,
    policy: (u32, u32),
    snap: bool,
    save_base: bool,
    sampler: impl FnMut(usize, i32) -> Sample,
    instance: &mut Instance,
) -> Option<[PostStep; 3]> {
    let layer_time = advance_time(clip.duration, owner_time, track_offset, instance.layer_time);
    let bracket = layer_time.and_then(|time| {
        let (low, high) = key_bracket(clip.keys, clip.duration, time)?;
        let weight = bracket_weight(clip.keys, low, high, time)?;
        Some((low, high, weight))
    });
    evaluate_instance_with_bracket(
        clip.nodes, layer_time, bracket, policy, snap, save_base, sampler, instance,
    )
}

/// Evaluates a prepared key bracket while retaining `0x60B30`'s time write
/// before a missing-bracket return. Guest adapters use this after searching
/// their complete key table through [`crate::keybracket::key_bracket_by`].
#[allow(clippy::too_many_arguments)]
pub fn evaluate_instance_with_bracket(
    nodes: usize,
    layer_time: Option<f32>,
    bracket: Option<(i32, i32, f32)>,
    policy: (u32, u32),
    snap: bool,
    save_base: bool,
    mut sampler: impl FnMut(usize, i32) -> Sample,
    instance: &mut Instance,
) -> Option<[PostStep; 3]> {
    instance.layer_time = layer_time?;
    let (low, high, mut weight) = bracket?;
    if snap {
        weight = if 0.5 > weight { 0.0 } else { 1.0 };
    }
    let root_motion = tracks_root_motion(policy.0, policy.1);
    for node in 0..nodes {
        let first = sampler(node, low);
        let second = sampler(node, high);
        let mut matrix = quat_matrix(slerp(first.quaternion(), second.quaternion(), weight));
        let (a, b) = (first.position(), second.position());
        let lerp = [0, 1, 2].map(|axis| (b[axis] - a[axis]) * weight + a[axis]);
        if node == 0 {
            if root_motion {
                instance.root[0..3].copy_from_slice(&lerp);
                matrix[12..15].copy_from_slice(&lerp);
            } else {
                matrix[12] = 0.0;
                matrix[13] = lerp[1];
                matrix[14] = 0.0;
            }
        } else {
            matrix[12..15].copy_from_slice(&lerp);
        }
        instance.matrices[node] = matrix;
    }
    if save_base {
        for i in 0..instance.saved_count {
            instance.base[i] = instance.matrices[i];
        }
    }
    if root_motion {
        for axis in 0..3 {
            instance.root[6 + axis] = instance.root[axis] - instance.root[3 + axis];
        }
        let position = [instance.root[0], instance.root[1], instance.root[2]];
        instance.root[3..6].copy_from_slice(&position);
    } else {
        instance.root[6..9].fill(0.0);
    }
    Some([
        PostStep::BlendByWeight { full: save_base },
        PostStep::ComposeTree,
        PostStep::ComposeOrCopy,
    ])
}
