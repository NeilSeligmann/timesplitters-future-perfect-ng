//! Pose matrix mixing loops of the animation instance (T1668): `0x60540`, `0x605D0` and `0x604D0`. MEASURED against the ORIGINAL
//! under Unicorn with the real matrix helpers (`tools/t1668_posemix_gold.py`, `tests/posemix_golden.rs`, 300 cases each).
//!
//! The context (register `esi` for the first two): `[ctx]` model, `[ctx+4]` source matrices (indexed by part), `[ctx+8]` output matrices
//! (indexed by weight entry). The model gives the node count (`+0x20`), the weight entries (`[[model+0x10]+0xC]`, 8 bytes: part
//! index, weight f32), the base pose (`+0x2C`, indexed by part) and the part tree table (`+0x14`, 20 bytes per part: `+4` parent,
//! `+8` first child, `+0xC` next sibling, -1 for none).
use crate::blend::matrix_blend;
use crate::posemath::{Mat4, mat_mul};

/// One weight entry: part index and blend weight.
pub type WeightEntry = (u32, f32);

/// `0x60540` (`game_animation_blend_pose_bone_matrices_by_weight_loop`): for every entry whose weight is not exactly 1.0 (NaN
/// included) `out[i] = blend(base[part], pose[part], weight)` (`1526C0`), with weight forced to 1.0 when `full` (the stack flag).
/// Entries of weight 1.0 leave `out[i]` alone.
pub fn blend_by_weight(
    entries: &[WeightEntry],
    base: &[Mat4],
    pose: &[Mat4],
    out: &mut [Mat4],
    full: bool,
) {
    for (i, &(part, weight)) in entries.iter().enumerate() {
        if weight == 1.0 {
            continue;
        }
        let t = if full { 1.0 } else { weight };
        out[i] = matrix_blend(&base[part as usize], &pose[part as usize], t);
    }
}

/// `0x605D0` (`..._compose_pose_matrices_with_parent_or_copy_loop`): weight exactly 1.0 copies `pose[part]` to `out[i]` (`1531D0`);
/// any other weight multiplies `out[i] = out[i] * pose[parent(part)]` (`153270`) when the part has a parent (`-1` none).
pub fn compose_or_copy(entries: &[WeightEntry], parents: &[i32], pose: &[Mat4], out: &mut [Mat4]) {
    for (i, &(part, weight)) in entries.iter().enumerate() {
        if weight == 1.0 {
            out[i] = pose[part as usize];
        } else if parents[part as usize] != -1 {
            out[i] = mat_mul(&out[i], &pose[parents[part as usize] as usize]);
        }
    }
}

/// One row of the part tree table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeRow {
    pub parent: i32,
    pub child: i32,
    pub sibling: i32,
}

/// `0x604D0`: walks a sibling chain from `node`, `mats[n] = mats[n] * mats[parent(n)]` for nodes with a parent (parents are final before
/// their children, so local matrices become world matrices), recursing into the first child before moving to the next sibling.
pub fn compose_tree(tree: &[TreeRow], mats: &mut [Mat4], node: i32) {
    let mut current = node;
    loop {
        // an index outside the table ends the walk (the original would read past it; the port stays inside its slices)
        if current < 0 || current as usize >= tree.len().min(mats.len()) {
            break;
        }
        let row = tree[current as usize];
        if row.parent >= 0 && (row.parent as usize) < mats.len() {
            mats[current as usize] = mat_mul(&mats[current as usize], &mats[row.parent as usize]);
        }
        if row.child >= 0 {
            compose_tree(tree, mats, row.child);
        }
        if row.sibling < 0 {
            break;
        }
        current = row.sibling;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_composition_makes_world_matrices() {
        let identity: Mat4 = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        let mut shift = identity;
        shift[12] = 1.0;
        let tree = [
            TreeRow {
                parent: -1,
                child: 1,
                sibling: -1,
            },
            TreeRow {
                parent: 0,
                child: 2,
                sibling: -1,
            },
            TreeRow {
                parent: 1,
                child: -1,
                sibling: -1,
            },
        ];
        let mut mats = [shift; 3];
        compose_tree(&tree, &mut mats, 0);
        assert_eq!([mats[0][12], mats[1][12], mats[2][12]], [1.0, 2.0, 3.0]);
    }

    #[test]
    fn weight_one_copies_and_others_multiply() {
        let identity: Mat4 = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        let mut moved = identity;
        moved[13] = 2.0;
        let pose = [identity, moved];
        let mut out = [[9.0; 16]; 2];
        compose_or_copy(&[(1, 1.0), (0, 0.5)], &[-1, 0], &pose, &mut out);
        assert_eq!(out[0], moved);
        assert_eq!(out[1], [9.0; 16], "no parent: untouched");
    }
}
