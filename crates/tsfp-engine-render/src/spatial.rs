// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 native compact spatial traversal. No guest memory or simulation writes.
//! Split arithmetic and lower-before-upper order follow original 0x1571C0.
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}
impl Bounds {
    /// Original 0x47560 treats unordered comparisons as overlapping.
    pub fn overlaps(self, other: Self) -> bool {
        (0..3).all(|axis| !(self.min[axis] > other.max[axis] || other.min[axis] > self.max[axis]))
    }
}

/// An immutable scene-owner tree. Node 0 is unused, root is 1; branch children
/// are 2*i and 2*i+1. Leaf values are scene-owner IDs, never pointers.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialTree {
    pub bounds: Bounds,
    pub nodes: Vec<Node>,
    /// The original data's quantisation scale, supplied by the data loader.
    pub split_scale: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Split { axis: usize, fraction: u16 },
    Leaf(Vec<u16>),
}

/// Typed decoder boundary. Native owners implement this over owned records;
/// the hybrid adapter is a separate TEMPORARY-SHIM.
pub trait TreeSource {
    type Leaf;
    fn node(&self, index: u32) -> Option<TreeNode<Self::Leaf>>;
    fn split_scale(&self) -> f32;
}
#[derive(Clone, Debug, PartialEq)]
pub enum TreeNode<L> {
    Split { axis: usize, fraction: u16 },
    Leaf(L),
}
impl TreeSource for SpatialTree {
    type Leaf = Vec<u16>;
    fn node(&self, index: u32) -> Option<TreeNode<Self::Leaf>> {
        Some(match self.nodes.get(index as usize)? {
            Node::Split { axis, fraction } => TreeNode::Split {
                axis: *axis,
                fraction: *fraction,
            },
            Node::Leaf(items) => TreeNode::Leaf(items.clone()),
        })
    }
    fn split_scale(&self) -> f32 {
        self.split_scale
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkError {
    MissingNode,
    InvalidAxis,
    IndexOverflow,
}

/// A callback may return any original result word; exactly 1 stops traversal.
/// When no child overlaps, original 0x1571C0 returns 0. Otherwise the last leaf's
/// result is retained. No sorting/deduplication changes the callback order.
pub fn walk<T: TreeSource>(
    tree: &T,
    query: Bounds,
    cell: Bounds,
    root: u32,
    mut visit: impl FnMut(T::Leaf) -> u32,
) -> Result<u32, WalkError> {
    fn descend<T: TreeSource>(
        tree: &T,
        query: Bounds,
        cell: Bounds,
        index: u32,
        visit: &mut impl FnMut(T::Leaf) -> u32,
    ) -> Result<u32, WalkError> {
        match tree.node(index).ok_or(WalkError::MissingNode)? {
            TreeNode::Leaf(items) => Ok(visit(items)),
            TreeNode::Split { axis, fraction } => {
                if axis >= 3 {
                    return Err(WalkError::InvalidAxis);
                }
                let split = (cell.max[axis] - cell.min[axis])
                    * (f32::from(fraction) * tree.split_scale())
                    + cell.min[axis];
                let lower = index.checked_mul(2).ok_or(WalkError::IndexOverflow)?;
                let upper = lower.checked_add(1).ok_or(WalkError::IndexOverflow)?;
                let mut result = 0;
                // UCOMISS unordered paths skip each child, unlike the outer AABB test.
                if split >= query.min[axis] {
                    let mut child = cell;
                    child.max[axis] = split;
                    result = descend(tree, query, child, lower, visit)?;
                    if result == 1 {
                        return Ok(1);
                    }
                }
                if query.max[axis] >= split {
                    let mut child = cell;
                    child.min[axis] = split;
                    result = descend(tree, query, child, upper, visit)?;
                }
                Ok(result)
            }
        }
    }
    descend(tree, query, cell, root, &mut visit)
}
impl SpatialTree {
    pub fn query(
        &self,
        query: Bounds,
        visit: impl FnMut(Vec<u16>) -> u32,
    ) -> Result<u32, WalkError> {
        if !query.overlaps(self.bounds) {
            return Ok(0);
        }
        walk(self, query, self.bounds, 1, visit)
    }
}

/// Native plane record. Original 0x8BDD0 uses z*c + x*a + y*b + d,
/// with each f32 operation rounded separately. Unordered distances are accepted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: [f32; 3],
    pub offset: f32,
}
impl Plane {
    pub fn distance(self, point: [f32; 3]) -> f32 {
        ((self.normal[2] * point[2] + self.normal[0] * point[0]) + point[1] * self.normal[1])
            + self.offset
    }
}
pub fn points_inside(points: &[[f32; 3]], planes: &[Plane]) -> bool {
    points
        .iter()
        .all(|point| planes.iter().all(|plane| !(0.0 > plane.distance(*point))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    fn tree() -> SpatialTree {
        SpatialTree {
            bounds: Bounds {
                min: [0.; 3],
                max: [10.; 3],
            },
            split_scale: 1. / 16383.,
            nodes: vec![
                Node::Leaf(vec![]),
                Node::Split {
                    axis: 0,
                    fraction: 8192,
                },
                Node::Leaf(vec![4, 2]),
                Node::Leaf(vec![9]),
            ],
        }
    }
    #[test]
    fn ordered_boundary_overlap_early_stop_and_reentrant() {
        let t = tree();
        let unchanged = t.clone();
        for _ in 0..3 {
            let mut seen = vec![];
            assert_eq!(
                t.query(t.bounds, |items| {
                    seen.extend(items);
                    2
                })
                .unwrap(),
                2
            );
            assert_eq!(seen, [4, 2, 9]);
        }
        let mut seen = vec![];
        assert_eq!(
            t.query(t.bounds, |items| {
                seen.extend(items);
                1
            })
            .unwrap(),
            1
        );
        assert_eq!(seen, [4, 2]);
        assert_eq!(t, unchanged);
        let outside = Bounds {
            min: [11.; 3],
            max: [12.; 3],
        };
        assert_eq!(t.query(outside, |_| panic!("outside callback")).unwrap(), 0);
    }
    #[test]
    fn unordered_split_skips_but_outer_overlap_accepts() {
        let mut t = tree();
        t.split_scale = f32::NAN;
        assert_eq!(t.query(t.bounds, |_| panic!("unordered child")).unwrap(), 0);
        assert!(
            Bounds {
                min: [f32::NAN; 3],
                max: [f32::NAN; 3]
            }
            .overlaps(t.bounds)
        );
    }
}
