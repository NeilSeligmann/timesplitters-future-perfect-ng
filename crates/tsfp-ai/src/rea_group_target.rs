//! Typed nearest-member target selection extracted from XBE 0x26D390.

use alloc::vec::Vec;

pub const GROUP_BUCKETS: usize = 80;
pub const GROUP_MASK_LIMIT: u32 = 1 << 20;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecentCandidate {
    pub group_mask: u32,
    pub position: Position3,
    /// Candidate actor `+0x110`; `-1` means selection has no writeback.
    pub target: i32,
}

/// Select the closest recent candidate matching the source actor's group list.
/// Equal distances retain the first candidate; candidates must strictly beat
/// the caller-supplied initial threshold.
pub fn nearest_candidate(
    source_actor: u32,
    source: Position3,
    initial_max_distance: f32,
    candidates: &[RecentCandidate],
    groups: &[Vec<u32>],
) -> Option<usize> {
    assert_eq!(groups.len(), GROUP_BUCKETS);
    let mut best_distance = initial_max_distance;
    let mut best = None;
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        if candidate.group_mask >= GROUP_MASK_LIMIT {
            continue;
        }
        for (bucket_index, members) in groups.iter().enumerate() {
            let bit = 1u32 << (bucket_index & 31);
            if candidate.group_mask & bit == 0 || !members.contains(&source_actor) {
                continue;
            }
            let dx = candidate.position.x - source.x;
            let dy = candidate.position.y - source.y;
            let dz = candidate.position.z - source.z;
            let distance = dx * dx + dy * dy + dz * dz;
            if distance < best_distance {
                best_distance = distance;
                best = Some(candidate_index);
            }
        }
    }
    best
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupTargetCandidate {
    /// Rust-owned identity corresponding to the compared object pointer.
    pub identity: u32,
    /// Raw object components read at `+0x64`, `+0x68`, and `+0x6C`.
    pub position: [f32; 3],
    /// Raw candidate field `+0x110`; `None` represents the observed `-1` sentinel.
    pub target: Option<u32>,
}

/// Select the nearest eligible member in the caller-supplied traversal order.
///
/// The original skips the current object and updates only when squared distance
/// is strictly smaller, preserving the first member on ties. The chosen member's
/// target is returned only after selection, so a nearest member with no target
/// yields `None` rather than falling through to the next member.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn nearest_group_target_26d390(
    actor_identity: u32,
    actor_position: [f32; 3],
    traversal_order: &[GroupTargetCandidate],
) -> Option<u32> {
    let mut best_distance = f32::MAX;
    let mut best = None;
    for candidate in traversal_order {
        if candidate.identity == actor_identity {
            continue;
        }
        let d0 = candidate.position[0] - actor_position[0];
        let d1 = candidate.position[1] - actor_position[1];
        let d2 = candidate.position[2] - actor_position[2];
        let distance = d0 * d0 + d1 * d1 + d2 * d2;
        if distance < best_distance {
            best_distance = distance;
            best = Some(candidate);
        }
    }
    best.and_then(|candidate| candidate.target)
}

#[cfg(test)]
mod tests {
    use super::{GroupTargetCandidate, nearest_group_target_26d390};

    fn candidate(identity: u32, position: [f32; 3], target: Option<u32>) -> GroupTargetCandidate {
        GroupTargetCandidate {
            identity,
            position,
            target,
        }
    }

    #[test]
    fn skips_self_and_keeps_first_member_on_equal_distance() {
        let members = [
            candidate(7, [0.0; 3], Some(700)),
            candidate(8, [1.0, 0.0, 0.0], Some(801)),
            candidate(9, [-1.0, 0.0, 0.0], Some(902)),
        ];
        assert_eq!(
            nearest_group_target_26d390(7, [0.0; 3], &members),
            Some(801)
        );
    }

    #[test]
    fn nearest_member_without_target_does_not_fall_through() {
        let members = [
            candidate(8, [1.0, 0.0, 0.0], None),
            candidate(9, [2.0, 0.0, 0.0], Some(902)),
        ];
        assert_eq!(nearest_group_target_26d390(7, [0.0; 3], &members), None);
    }
}
