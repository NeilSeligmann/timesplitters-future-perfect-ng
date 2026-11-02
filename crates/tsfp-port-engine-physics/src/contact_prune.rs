//! Readable contact-list pruning and geometric coalescing model for `0x167540`.
//!
//! The carrier dossier measures the record fields and the same-owner-A,
//! point/normal similarity predicate. The kind-3 backface and class-pair
//! interpretation below follows the bounded function-name analysis and stays
//! **INFERRED** until exercised against owner data. This module is an opt-in
//! reconstruction; it is not wired into the lifted pass.

/// Contact data needed by the prune/merge operation. `next` is an index into
/// the Rust-owned pass arena; endpoint classes and body-B origin are imported
/// typed inputs, not read through guest offsets here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactNode {
    pub next: Option<usize>,
    pub owner_a: u32,
    pub owner_b: u32,
    pub owner_a_class: u32,
    pub owner_b_class: u32,
    pub kind: u16,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub body_b_origin: [f32; 3],
}

/// Runtime thresholds supplied from the game's typed contact policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactPrunePolicy {
    pub backface_epsilon: f32,
    pub merge_distance_squared: f32,
    pub merge_normal_dot_min: f32,
    pub merge_normal_dot_max: f32,
}

/// Prune inferred backface/class cases, then merge later geometrically similar
/// contacts with the same owner A. Returns the possibly changed list head.
///
/// Verification tier: **Tier 0 only**, FABRICATED typed records. Do not select
/// this in the game host until a Tier 1 lifted differential and route gate pass.
pub fn prune_and_merge(
    nodes: &mut [ContactNode],
    mut head: Option<usize>,
    policy: ContactPrunePolicy,
) -> Option<usize> {
    let mut previous = None;
    let mut current = head;

    while let Some(index) = current {
        let node = nodes[index];
        let next = node.next;
        if should_prune(&node, policy.backface_epsilon) {
            unlink(nodes, previous, index, next, &mut head);
            current = next;
            continue;
        }

        let mut merged_normal = nodes[index].normal;
        let mut kept_previous = Some(index);
        let mut candidate = next;
        while let Some(candidate_index) = candidate {
            let candidate_node = nodes[candidate_index];
            let candidate_next = candidate_node.next;
            if mergeable(&nodes[index], &candidate_node, policy) {
                merged_normal = normalize(add(merged_normal, candidate_node.normal));
                nodes[index].normal = merged_normal;
                unlink(
                    nodes,
                    kept_previous,
                    candidate_index,
                    candidate_next,
                    &mut head,
                );
            } else {
                kept_previous = Some(candidate_index);
            }
            candidate = candidate_next;
        }
        nodes[index].normal = merged_normal;
        previous = Some(index);
        current = nodes[index].next;
    }
    head
}

fn should_prune(node: &ContactNode, backface_epsilon: f32) -> bool {
    let excluded_class_pair =
        node.owner_a_class == 0x0040_0000 && matches!(node.owner_b_class, 0x40 | 0x20);
    let away_from_body_b =
        node.kind == 3 && dot(node.normal, sub(node.point, node.body_b_origin)) > backface_epsilon;
    excluded_class_pair || away_from_body_b
}

fn mergeable(a: &ContactNode, b: &ContactNode, policy: ContactPrunePolicy) -> bool {
    if a.owner_a != b.owner_a {
        return false;
    }
    let delta = sub(a.point, b.point);
    let distance_squared = dot(delta, delta);
    let normal_dot = dot(a.normal, b.normal);
    distance_squared < policy.merge_distance_squared
        && normal_dot > policy.merge_normal_dot_min
        && normal_dot < policy.merge_normal_dot_max
}

fn unlink(
    nodes: &mut [ContactNode],
    previous: Option<usize>,
    removed: usize,
    next: Option<usize>,
    head: &mut Option<usize>,
) {
    if let Some(previous) = previous {
        nodes[previous].next = next;
    } else {
        *head = next;
    }
    nodes[removed].next = None;
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    // `f32::sqrt` is only available with std on this toolchain. This inferred
    // path stays no_std by using Newton iteration; zero vectors retain the
    // lifted-style IEEE result (NaNs) instead of inventing a direction.
    let length_squared = dot(value, value);
    let inverse_length = 1.0 / sqrt_no_std(length_squared);
    [
        value[0] * inverse_length,
        value[1] * inverse_length,
        value[2] * inverse_length,
    ]
}

fn sqrt_no_std(value: f32) -> f32 {
    if value == 0.0 || value.is_infinite() {
        return value;
    }
    if value.is_nan() || value < 0.0 {
        return f32::NAN;
    }

    let mut estimate = f32::from_bits((value.to_bits() >> 1) + 0x1fc0_0000);
    for _ in 0..5 {
        estimate = 0.5 * (estimate + value / estimate);
    }
    estimate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(next: Option<usize>, owner_a: u32, point_x: f32) -> ContactNode {
        ContactNode {
            next,
            owner_a,
            owner_b: 20,
            owner_a_class: 0x40,
            owner_b_class: 0x40,
            kind: 1,
            point: [point_x, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
            body_b_origin: [10.0, 0.0, 0.0],
        }
    }

    fn policy() -> ContactPrunePolicy {
        ContactPrunePolicy {
            backface_epsilon: 0.0,
            merge_distance_squared: 1.0,
            merge_normal_dot_min: 0.0,
            merge_normal_dot_max: 1.1,
        }
    }

    #[test]
    fn merges_close_same_owner_a_and_keeps_list_order() {
        let mut nodes = [
            node(Some(1), 10, 0.0),
            node(Some(2), 10, 0.5),
            node(None, 11, 0.75),
        ];
        let head = prune_and_merge(&mut nodes, Some(0), policy());
        assert_eq!(head, Some(0));
        assert_eq!(nodes[0].next, Some(2));
        assert_eq!(nodes[1].next, None);
        assert_eq!(nodes[0].normal, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn leaves_same_geometry_when_owner_a_differs() {
        let mut nodes = [node(Some(1), 10, 0.0), node(None, 11, 0.25)];
        assert_eq!(prune_and_merge(&mut nodes, Some(0), policy()), Some(0));
        assert_eq!(nodes[0].next, Some(1));
    }

    #[test]
    fn merges_same_owner_a_even_when_owner_b_differs() {
        let mut first = node(Some(1), 10, 0.0);
        let mut second = node(None, 10, 0.25);
        first.owner_b = 20;
        second.owner_b = 99;

        assert_eq!(
            prune_and_merge(&mut [first, second], Some(0), policy()),
            Some(0)
        );
    }

    #[test]
    fn merge_distance_threshold_is_strict() {
        let mut nodes = [node(Some(1), 10, 0.0), node(None, 10, 1.0)];
        assert_eq!(prune_and_merge(&mut nodes, Some(0), policy()), Some(0));
        assert_eq!(nodes[0].next, Some(1));
    }

    #[test]
    fn merge_normal_threshold_is_strict() {
        let first = node(Some(1), 10, 0.0);
        let mut second = node(None, 10, 0.25);
        second.normal = [0.0, 1.0, 0.0];
        assert_eq!(
            prune_and_merge(&mut [first, second], Some(0), policy()),
            Some(0)
        );
        assert_eq!(first.next, Some(1));
    }

    #[test]
    fn removes_inferred_kind_three_backface() {
        let mut first = node(Some(1), 10, 11.0);
        first.kind = 3;
        let mut nodes = [first, node(None, 11, 0.0)];
        assert_eq!(prune_and_merge(&mut nodes, Some(0), policy()), Some(1));
        assert_eq!(nodes[0].next, None);
    }

    #[test]
    fn removes_inferred_special_class_pair() {
        let mut first = node(Some(1), 10, 0.0);
        first.owner_a_class = 0x0040_0000;
        first.owner_b_class = 0x20;
        let mut nodes = [first, node(None, 11, 0.0)];
        assert_eq!(prune_and_merge(&mut nodes, Some(0), policy()), Some(1));
    }
}
