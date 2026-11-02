//! Typed composition of the bounded route-point search and actor-owned state.
//!
//! This module models only the point-follow search/position stage. The
//! behavior route cursor, secondary occupancy claims, and arrival handoff
//! remain separate until their ordering is established by REA.

use crate::{
    movement_approach::Position3,
    navigation_scoring::NodeId,
    rea_routes::PointFollowSearchResult,
    rea_tick_state::{
        NavigationGraphGeneration, PointFollowEffect, ReaActorIdentity, ReaActorStoreError,
        ReaTickActorStore,
    },
};

/// World queries required by the measured `0x26EA00` point-follow stage.
///
/// The adapter classifies the signed search return with
/// `classify_point_follow_search_result` before returning it here. For a
/// selected node, the adapter resolves the movement target through the
/// measured target resolver; that target may be the primary node or its linked
/// actor. Only the copied position crosses this seam. Cached-record results
/// resolve only the record position requested by that result.
pub trait PointFollowWorld {
    fn search_primary_node(&mut self, current_node: NodeId) -> PointFollowSearchResult;
    fn selected_node_movement_target(&mut self, node: NodeId) -> Option<Position3>;
    fn cached_record_position(&mut self) -> Option<Position3>;
}

/// Resolves one typed point-follow result and commits its bounded effect to
/// the matching Rust-owned actor generation.
///
/// Generation and graph checks happen before querying the world and again at
/// commit. The function does not advance the behavior route cursor, alter
/// secondary occupancy, or infer an arrival transition.
pub fn follow_owned_primary_point<W: PointFollowWorld>(
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    graph: NavigationGraphGeneration,
    current_node: NodeId,
    world: &mut W,
) -> Result<PointFollowEffect, ReaActorStoreError> {
    let actor = store
        .state(identity)
        .ok_or(ReaActorStoreError::StaleIdentity)?;
    if store.navigation_graph_generation() != Some(graph)
        || actor.actor_navigation().graph_generation() != Some(graph)
    {
        return Err(ReaActorStoreError::StaleNavigationGraph);
    }

    let result = world.search_primary_node(current_node);
    let (selected_position, cached_position) = match result {
        PointFollowSearchResult::SelectedNode(node) => {
            (world.selected_node_movement_target(node), None)
        }
        PointFollowSearchResult::CachedRecord => (None, world.cached_record_position()),
        PointFollowSearchResult::AlertAbort | PointFollowSearchResult::OtherNegative(_) => {
            (None, None)
        }
    };

    store.apply_point_follow_result(identity, graph, result, selected_position, cached_position)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{PointFollowWorld, follow_owned_primary_point};
    use crate::{
        awareness::AwarenessCache,
        movement_approach::Position3,
        navigation_scoring::NodeId,
        rea_routes::PointFollowSearchResult,
        rea_tick_state::{
            BehaviorMode, BehaviorPair, LocalState, NavigationGraphGeneration, PointFollowEffect,
            ReaActorIdentity, ReaActorStoreError, ReaTickActorStore, ReaTickSnapshot,
            TransitionProgress,
        },
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum WorldCall {
        Search(NodeId),
        SelectedNodeTarget(NodeId),
        CachedPosition,
    }

    struct TestWorld {
        result: PointFollowSearchResult,
        selected_target: Option<Position3>,
        cached_position: Option<Position3>,
        calls: Vec<WorldCall>,
    }

    impl TestWorld {
        fn new(result: PointFollowSearchResult) -> Self {
            Self {
                result,
                selected_target: None,
                cached_position: None,
                calls: Vec::new(),
            }
        }
    }

    impl PointFollowWorld for TestWorld {
        fn search_primary_node(&mut self, current_node: NodeId) -> PointFollowSearchResult {
            self.calls.push(WorldCall::Search(current_node));
            self.result
        }

        fn selected_node_movement_target(&mut self, node: NodeId) -> Option<Position3> {
            self.calls.push(WorldCall::SelectedNodeTarget(node));
            self.selected_target
        }

        fn cached_record_position(&mut self) -> Option<Position3> {
            self.calls.push(WorldCall::CachedPosition);
            self.cached_position
        }
    }

    fn snapshot() -> ReaTickSnapshot {
        ReaTickSnapshot {
            mode: BehaviorMode::Other,
            mode10_active: BehaviorPair::default(),
            other_active: BehaviorPair::default(),
            previous_behavior: BehaviorPair::default(),
            pending_behavior: BehaviorPair::default(),
            local_state: LocalState::default(),
            previous_local_state: LocalState::default(),
            pending_local_state: None,
            transition_progress: TransitionProgress::default(),
            awareness_cache: AwarenessCache::from_raw(0),
        }
    }

    fn actor_store() -> (
        ReaTickActorStore,
        NavigationGraphGeneration,
        ReaActorIdentity,
    ) {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let actor = store.spawn(3, snapshot()).unwrap();
        (store, graph, actor)
    }

    fn position(x: f32, y: f32, z: f32) -> Position3 {
        Position3 { x, y, z }
    }

    #[test]
    fn owned_point_follow_selected_node_commits_one_generation_owned_step() {
        let (mut store, graph, actor) = actor_store();
        let mut world = TestWorld::new(PointFollowSearchResult::SelectedNode(NodeId(12)));
        // The typed resolver may copy coordinates from either the primary
        // node or its linked actor, as selected by the measured adapter.
        world.selected_target = Some(position(4.0, 5.0, 6.0));

        assert_eq!(
            follow_owned_primary_point(&mut store, actor, graph, NodeId(7), &mut world),
            Ok(PointFollowEffect::SelectedNodeCommitted(NodeId(12)))
        );
        assert_eq!(
            world.calls,
            [
                WorldCall::Search(NodeId(7)),
                WorldCall::SelectedNodeTarget(NodeId(12))
            ]
        );
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), Some(NodeId(12)));
        assert_eq!(navigation.movement_target(), Some(position(4.0, 5.0, 6.0)));
    }

    #[test]
    fn owned_point_follow_cached_record_keeps_the_measured_scalar() {
        let (mut store, graph, actor) = actor_store();
        let mut world = TestWorld::new(PointFollowSearchResult::CachedRecord);
        world.cached_position = Some(position(8.0, 9.0, 10.0));

        assert_eq!(
            follow_owned_primary_point(&mut store, actor, graph, NodeId(7), &mut world),
            Ok(PointFollowEffect::CachedRecordTargetCommitted {
                point_follow_scalar: 1.4
            })
        );
        assert_eq!(
            world.calls,
            [WorldCall::Search(NodeId(7)), WorldCall::CachedPosition]
        );
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), None);
        assert_eq!(navigation.movement_target(), Some(position(8.0, 9.0, 10.0)));
    }

    #[test]
    fn owned_point_follow_abort_and_unclassified_results_are_no_ops() {
        let (mut store, graph, actor) = actor_store();
        let original = position(1.0, 2.0, 3.0);
        store.set_navigation_next_node(actor, NodeId(5)).unwrap();
        store
            .set_navigation_movement_target(actor, original)
            .unwrap();
        for (result, expected) in [
            (
                PointFollowSearchResult::AlertAbort,
                PointFollowEffect::AlertAbort,
            ),
            (
                PointFollowSearchResult::OtherNegative(-3),
                PointFollowEffect::UnclassifiedNegative(-3),
            ),
        ] {
            let mut world = TestWorld::new(result);
            assert_eq!(
                follow_owned_primary_point(&mut store, actor, graph, NodeId(7), &mut world),
                Ok(expected)
            );
            assert_eq!(world.calls, [WorldCall::Search(NodeId(7))]);
        }
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), Some(NodeId(5)));
        assert_eq!(navigation.movement_target(), Some(original));
    }

    #[test]
    fn owned_point_follow_stale_actor_or_graph_skips_world_queries() {
        let (mut store, graph, actor) = actor_store();
        let mut world = TestWorld::new(PointFollowSearchResult::AlertAbort);
        let stale_actor = ReaActorIdentity {
            slot: actor.slot,
            generation: actor.generation + 1,
        };
        assert_eq!(
            follow_owned_primary_point(&mut store, stale_actor, graph, NodeId(7), &mut world),
            Err(ReaActorStoreError::StaleIdentity)
        );
        assert!(world.calls.is_empty());

        let replacement = store.replace_navigation_graph().unwrap();
        assert_ne!(graph, replacement);
        assert_eq!(
            follow_owned_primary_point(&mut store, actor, graph, NodeId(7), &mut world),
            Err(ReaActorStoreError::StaleNavigationGraph)
        );
        assert!(world.calls.is_empty());
    }
}
