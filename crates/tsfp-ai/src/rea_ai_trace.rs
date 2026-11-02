// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact AI state trace encoding shared with the headless replay observer.

use crate::movement_approach::Position3;
use crate::navigation_scoring::NodeId;
use crate::rea_tick_state::{ReaActorIdentity, ReaTickState};

pub const STATE_WORDS: usize = 24;

/// Serialize the Rust-owned actor snapshot and its bounded navigation projection.
/// Absent optionals use `u32::MAX`; the final word records which navigation
/// groups have typed values so defaults cannot masquerade as owned state.
pub fn snapshot_words(identity: ReaActorIdentity, state: &ReaTickState) -> [u32; STATE_WORDS + 2] {
    let snapshot = state.snapshot();
    let navigation = state.actor_navigation();
    let route = navigation.route();
    let target = navigation.movement_target();
    let target_bits = target.map_or([u32::MAX; 3], position_bits);
    let owned = u32::from(navigation.route_owned())
        | (u32::from(navigation.next_node().is_some()) << 1)
        | (u32::from(target.is_some()) << 2);
    [
        u32::from(identity.slot),
        identity.generation,
        match snapshot.mode {
            crate::rea_tick_state::BehaviorMode::Mode10 => 1,
            crate::rea_tick_state::BehaviorMode::Other => 0,
        },
        snapshot.mode10_active.low,
        snapshot.mode10_active.high,
        snapshot.other_active.low,
        snapshot.other_active.high,
        snapshot.previous_behavior.low,
        snapshot.previous_behavior.high,
        snapshot.pending_behavior.low,
        snapshot.pending_behavior.high,
        snapshot.local_state.0,
        snapshot.previous_local_state.0,
        snapshot
            .pending_local_state
            .map_or(u32::MAX, |value| value.0),
        snapshot.transition_progress.0,
        snapshot.awareness_cache.raw_result(),
        route.current_node.0,
        route.goal_node.0,
        route.intermediate_node.map_or(u32::MAX, |node| node.0),
        route.reverse_trace_within,
        route.reverse_trace_other,
        navigation
            .next_node()
            .map_or(u32::MAX, |node: NodeId| node.0),
        target_bits[0],
        target_bits[1],
        target_bits[2],
        owned,
    ]
}

fn position_bits(position: Position3) -> [u32; 3] {
    [
        position.x.to_bits(),
        position.y.to_bits(),
        position.z.to_bits(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{STATE_WORDS, snapshot_words};
    use crate::awareness::AwarenessCache;
    use crate::movement_approach::Position3;
    use crate::navigation_scoring::NodeId;
    use crate::rea_tick_state::{
        BehaviorMode, BehaviorPair, LocalState, ReaActorIdentity, ReaTickSnapshot, ReaTickState,
        TransitionProgress,
    };

    fn state() -> ReaTickState {
        ReaTickState::new(ReaTickSnapshot {
            mode: BehaviorMode::Other,
            mode10_active: BehaviorPair { low: 1, high: 2 },
            other_active: BehaviorPair { low: 3, high: 4 },
            previous_behavior: BehaviorPair { low: 5, high: 6 },
            pending_behavior: BehaviorPair { low: 7, high: 8 },
            local_state: LocalState(9),
            previous_local_state: LocalState(10),
            pending_local_state: None,
            transition_progress: TransitionProgress(11),
            awareness_cache: AwarenessCache::from_raw(12),
        })
    }

    #[test]
    fn trace_encodes_route_projection_and_ownership_without_zero_defaults() {
        let identity = ReaActorIdentity {
            slot: 2,
            generation: 7,
        };
        let mut state = state();
        {
            let route = state.navigation_route_mut();
            route.current_node = NodeId(13);
            route.goal_node = NodeId(14);
            route.intermediate_node = Some(NodeId(15));
            route.reverse_trace_within = 16;
            route.reverse_trace_other = 17;
        }
        state.actor_navigation_mut().set_next_node(NodeId(18));
        state.actor_navigation_mut().set_movement_target(Position3 {
            x: 1.0,
            y: -2.0,
            z: 3.5,
        });

        let words = snapshot_words(identity, &state);
        assert_eq!(words.len(), STATE_WORDS + 2);
        assert_eq!(
            &words[16..25],
            &[
                13,
                14,
                15,
                16,
                17,
                18,
                1.0f32.to_bits(),
                (-2.0f32).to_bits(),
                3.5f32.to_bits()
            ]
        );
        assert_eq!(words[25], 0b111);
    }

    #[test]
    fn trace_marks_unprojected_navigation_as_unowned() {
        let words = snapshot_words(
            ReaActorIdentity {
                slot: 0,
                generation: 1,
            },
            &state(),
        );
        assert_eq!(
            &words[16..25],
            &[0, 0, u32::MAX, 0, 0, u32::MAX, u32::MAX, u32::MAX, u32::MAX]
        );
        assert_eq!(words[25], 0);
    }
}
