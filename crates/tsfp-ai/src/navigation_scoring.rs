// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable native boundary for REA's ordered navigation candidate scorer (0x261DE0).
//!
//! Inputs are snapshots owned by the Rust navigation world. The caller retains the
//! group order and candidate order from the loaded world; this function has no guest
//! memory, claims, occupancy updates, RNG, or behavior-state side effects.
//! Verification tier: Tier 1 differential against lifted `0x261DE0` in
//! `tests/navigation_scoring_diff.rs` (4,096 seeded, fabricated but valid worlds).

pub use crate::rea_ai_types::ActorId;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavPosition {
    pub x: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavNode {
    pub id: NodeId,
    pub position: NavPosition,
}

/// A loaded group in original order. Candidate IDs are valid loaded node IDs.
#[derive(Clone, Copy, Debug)]
pub struct CandidateGroup<'a> {
    pub node_ids: &'a [NodeId],
}

/// The fields used by the original qualifying-other-goal scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OtherActorGoal {
    pub extension: ActorId,
    pub object: ObjectId,
    pub flags: u8,
    pub goal: Option<NodeId>,
}

/// Typed snapshot at the scorer boundary. `target_position` and `target_object`
/// come from the actor's current target object; absent target returns no selection.
#[derive(Clone, Copy, Debug)]
pub struct ScoringActor {
    pub extension: ActorId,
    pub object: ObjectId,
    pub target_object: Option<ObjectId>,
    pub current_node: NodeId,
    pub position: NavPosition,
    pub target_position: Option<NavPosition>,
}

#[derive(Clone, Copy, Debug)]
pub struct ScoringWorld<'a> {
    pub groups: &'a [CandidateGroup<'a>],
    pub nodes: &'a [NavNode],
    pub other_actors: &'a [OtherActorGoal],
}

fn node_position(nodes: &[NavNode], id: NodeId) -> NavPosition {
    nodes
        .iter()
        .find(|node| node.id == id)
        .expect("valid navigation-world node ID")
        .position
}

fn distance_squared_xz(a: NavPosition, b: NavPosition) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    let dx_squared = dx * dx;
    let dz_squared = dz * dz;
    dx_squared + dz_squared
}

/// Select the best goal node using the original 0x261DE0 score and traversal order.
///
/// The caller supplies valid node/group snapshots. Other-goal qualification is
/// preserved here: skip this extension, skip actors whose object equals this
/// actor's target object, skip flag bit 8, and require a nonnegative goal node.
/// Invalid IDs are outside the documented loaded-world domain and panic at lookup,
/// rather than acquiring invented null/invalid-node behavior.
pub fn select_best_goal(actor: ScoringActor, world: ScoringWorld<'_>) -> Option<NodeId> {
    let (Some(target_object), Some(target_position)) = (actor.target_object, actor.target_position)
    else {
        return None;
    };

    let mut other_goals = Vec::new();
    for other in world.other_actors {
        if other.extension == actor.extension
            || other.object == target_object
            || other.flags & 8 != 0
        {
            continue;
        }
        if let Some(goal) = other.goal {
            other_goals.push(node_position(world.nodes, goal));
        }
    }

    let mut best_score = -f32::MAX;
    let mut best_node = None;
    for group in world.groups {
        for &candidate_id in group.node_ids {
            if candidate_id == actor.current_node {
                continue;
            }

            let candidate = node_position(world.nodes, candidate_id);
            let mut target_distance = distance_squared_xz(target_position, candidate);
            let mut actor_distance = distance_squared_xz(actor.position, candidate);

            if target_distance < 100.0 {
                target_distance *= target_distance;
            }
            if actor_distance > 400.0 {
                actor_distance *= actor_distance;
            }

            let mut spacing = f32::MAX;
            for &other_goal in &other_goals {
                let distance = distance_squared_xz(other_goal, candidate);
                if distance < spacing {
                    spacing = distance;
                }
            }
            if spacing == f32::MAX {
                spacing = 0.0;
            }

            // Preserve the original SSE operation order and strict winner update.
            let score = (target_distance - 0.5 * actor_distance) + spacing;
            if score > best_score {
                best_score = score;
                best_node = Some(candidate_id);
            }
        }
    }
    best_node
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u32, x: f32, z: f32) -> NavNode {
        NavNode {
            id: NodeId(id),
            position: NavPosition { x, z },
        }
    }

    #[test]
    fn thresholds_order_current_node_and_ties_follow_documented_contract() {
        let nodes = [node(1, 0.0, 0.0), node(2, 5.0, 0.0), node(3, 10.0, 0.0)];
        let group = CandidateGroup {
            node_ids: &[NodeId(1), NodeId(2), NodeId(3)],
        };
        let actor = ScoringActor {
            extension: ActorId(10),
            object: ObjectId(11),
            target_object: Some(ObjectId(12)),
            current_node: NodeId(1),
            position: NavPosition { x: 0.0, z: 0.0 },
            target_position: Some(NavPosition { x: 0.0, z: 0.0 }),
        };
        assert_eq!(
            select_best_goal(
                actor,
                ScoringWorld {
                    groups: &[group],
                    nodes: &nodes,
                    other_actors: &[]
                }
            ),
            Some(NodeId(2))
        );
        let tied = [node(2, 10.0, 0.0), node(3, -10.0, 0.0)];
        let group = CandidateGroup {
            node_ids: &[NodeId(2), NodeId(3)],
        };
        assert_eq!(
            select_best_goal(
                actor,
                ScoringWorld {
                    groups: &[group],
                    nodes: &tied,
                    other_actors: &[]
                }
            ),
            Some(NodeId(2))
        );
    }

    #[test]
    fn other_goal_filters_and_spacing_affect_only_qualifying_goals() {
        let nodes = [
            node(1, 0.0, 0.0),
            node(2, 10.0, 0.0),
            node(3, 20.0, 0.0),
            node(4, 10.0, 0.0),
        ];
        let group = CandidateGroup {
            node_ids: &[NodeId(2), NodeId(3)],
        };
        let actor = ScoringActor {
            extension: ActorId(10),
            object: ObjectId(11),
            target_object: Some(ObjectId(12)),
            current_node: NodeId(1),
            position: NavPosition::default(),
            target_position: Some(NavPosition::default()),
        };
        let others = [
            OtherActorGoal {
                extension: ActorId(10),
                object: ObjectId(13),
                flags: 0,
                goal: Some(NodeId(2)),
            },
            OtherActorGoal {
                extension: ActorId(20),
                object: ObjectId(12),
                flags: 0,
                goal: Some(NodeId(2)),
            },
            OtherActorGoal {
                extension: ActorId(21),
                object: ObjectId(14),
                flags: 8,
                goal: Some(NodeId(2)),
            },
            OtherActorGoal {
                extension: ActorId(22),
                object: ObjectId(15),
                flags: 0,
                goal: Some(NodeId(4)),
            },
        ];
        assert_eq!(
            select_best_goal(
                actor,
                ScoringWorld {
                    groups: &[group],
                    nodes: &nodes,
                    other_actors: &others
                }
            ),
            Some(NodeId(3))
        );
    }
}
