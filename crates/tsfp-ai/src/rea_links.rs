// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned route-link following at the documented 0x263ED0 boundary.
//!
//! This slice keeps movement targets, primary node IDs, route cursor state, and
//! animation root samples separate. It covers only the measured entry gates and
//! delegates animation samples/navigation advancement to explicit host methods.
//! Tier 1 differential coverage is in `tests/t2395_rea.rs`.

use crate::{
    navigation_scoring::NodeId,
    rea_movement::{LinkFollowDecision, LinkFollowInput, Position3, link_follow_decision},
    rea_routes::{RouteActor, RouteProgram, RouteWorld, advance_route},
    rea_tick_state::{ReaActorIdentity, ReaActorStoreError, ReaTickActorStore},
};

pub use crate::rea_ai_types::LinkActorState;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RootEndpointSamples {
    pub first_z: f32,
    pub last_z: f32,
}

/// Environment services retained at the navigation and animation edges.
pub trait LinkWorld: RouteWorld {
    fn animation_matches(&mut self, actor: &LinkActorState) -> bool;
    fn sample_root_endpoints(&mut self, actor: &LinkActorState) -> RootEndpointSamples;
    fn primary_position(&mut self, node: NodeId) -> Position3;
    fn advance_navigation_link(&mut self, actor: &mut LinkActorState, goal: NodeId);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkFollowResult {
    Returned,
    AskedNavigationToAdvance,
    AdvancedRoute,
    TargetedActor,
    TargetedGoal,
}

/// Applies the established link-follow decisions to Rust-owned route and actor
/// state. Route entries and primary-node IDs are valid loaded-world data.
pub fn follow_route_link<W: LinkWorld>(
    behavior_route_selector: i32,
    actor: &mut LinkActorState,
    route_actor: &mut RouteActor,
    route: &mut RouteProgram,
    world: &mut W,
) -> LinkFollowResult {
    if crate::rea_link_gate::returns_before_route(behavior_route_selector) {
        return LinkFollowResult::Returned;
    }
    let kind = route.entries[route.cursor as usize].kind;
    if kind == 0 {
        advance_route(route_actor, route, world);
        actor.goal_node = route_actor.goal_node;
        return LinkFollowResult::AdvancedRoute;
    }

    if kind == 1 {
        let samples = world.sample_root_endpoints(actor);
        let goal = world.primary_position(actor.goal_node);
        let decision = link_follow_decision(LinkFollowInput {
            route_kind: kind,
            behavior_route_selector,
            animation_matches: false,
            actor_position: actor.position,
            goal_position: goal,
            root_first_z: samples.first_z,
            root_last_z: samples.last_z,
        });
        if decision == LinkFollowDecision::AskNavigationToAdvance {
            world.advance_navigation_link(actor, actor.goal_node);
            LinkFollowResult::AskedNavigationToAdvance
        } else {
            actor.current_node = actor.goal_node;
            route_actor.current_node = actor.goal_node;
            advance_route(route_actor, route, world);
            actor.goal_node = route_actor.goal_node;
            LinkFollowResult::AdvancedRoute
        }
    } else if world.animation_matches(actor) {
        actor.movement_target = world.primary_position(actor.goal_node);
        LinkFollowResult::TargetedGoal
    } else {
        actor.movement_target = actor.position;
        LinkFollowResult::TargetedActor
    }
}

/// Runs link following directly against one generation-owned actor. The route
/// program supplies loaded route data; its cursor, route progress, link state,
/// and movement target are committed to the actor store. World and animation
/// reads remain explicit services at the boundary.
pub fn follow_owned_route_link<W: LinkWorld>(
    behavior_route_selector: i32,
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    route: &mut RouteProgram,
    world: &mut W,
) -> Result<Option<LinkFollowResult>, ReaActorStoreError> {
    let Some(mut actor) = store.navigation_link(identity)? else {
        return Ok(None);
    };

    let mut route_actor = store.navigation_route(identity)?;
    route.cursor = store
        .behavior_route_cursor(identity)?
        .unwrap_or(route.cursor);
    let result = follow_route_link(
        behavior_route_selector,
        &mut actor,
        &mut route_actor,
        route,
        world,
    );

    store.set_navigation_route(identity, route_actor)?;
    store.set_behavior_route_cursor(identity, route.cursor)?;
    store.set_navigation_link(identity, actor)?;
    store.set_navigation_movement_target(identity, actor.movement_target)?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::{
        LinkActorState, LinkFollowResult, LinkWorld, RootEndpointSamples, follow_owned_route_link,
    };
    use crate::{
        awareness::AwarenessCache,
        movement_approach::Position3,
        navigation_scoring::NodeId,
        rea_ai_types::NodeKey,
        rea_routes::{RouteEntry, RouteProgram},
        rea_tick_state::{
            BehaviorMode, BehaviorPair, LocalState, ReaActorStoreError, ReaTickActorStore,
            ReaTickSnapshot, TransitionProgress,
        },
    };

    #[derive(Default)]
    struct World;

    impl crate::rea_routes::RouteWorld for World {
        fn resolve_key(&mut self, _key: NodeKey) -> Option<NodeId> {
            None
        }

        fn player_current_node(&mut self) -> NodeId {
            NodeId(0)
        }

        fn find_intermediate(&mut self, _current: NodeId, _target: NodeId) -> i32 {
            -1
        }

        fn abort_route(&mut self) {}
    }

    impl LinkWorld for World {
        fn animation_matches(&mut self, _actor: &LinkActorState) -> bool {
            false
        }

        fn sample_root_endpoints(&mut self, _actor: &LinkActorState) -> RootEndpointSamples {
            RootEndpointSamples::default()
        }

        fn primary_position(&mut self, _node: NodeId) -> Position3 {
            Position3::default()
        }

        fn advance_navigation_link(&mut self, _actor: &mut LinkActorState, _goal: NodeId) {}
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

    #[test]
    fn owned_link_follow_commits_movement_to_the_actor_generation() {
        let mut store = ReaTickActorStore::new_empty();
        let identity = store.spawn(3, snapshot()).unwrap();
        let actor_position = Position3 {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        };
        store
            .set_navigation_link(
                identity,
                LinkActorState {
                    position: actor_position,
                    current_node: NodeId(1),
                    goal_node: NodeId(2),
                    movement_target: Position3::default(),
                },
            )
            .unwrap();

        let mut route = RouteProgram {
            entries: vec![
                RouteEntry {
                    kind: 2,
                    payload_a: 0,
                    payload_b: 0,
                },
                RouteEntry::default(),
            ],
            cursor: 0,
        };
        let result =
            follow_owned_route_link(0, &mut store, identity, &mut route, &mut World).unwrap();

        assert_eq!(result, Some(LinkFollowResult::TargetedActor));
        assert_eq!(store.behavior_route_cursor(identity), Ok(Some(0)));
        assert_eq!(
            store
                .navigation_link(identity)
                .unwrap()
                .unwrap()
                .movement_target,
            actor_position
        );
        assert_eq!(
            store
                .state(identity)
                .unwrap()
                .actor_navigation()
                .movement_target(),
            Some(actor_position)
        );
        assert_eq!(
            follow_owned_route_link(
                0,
                &mut store,
                crate::rea_tick_state::ReaActorIdentity {
                    slot: 3,
                    generation: 0,
                },
                &mut route,
                &mut World
            ),
            Err(ReaActorStoreError::StaleIdentity)
        );
    }

    #[test]
    fn owned_link_follow_without_a_link_snapshot_does_not_mutate_cursor() {
        let mut store = ReaTickActorStore::new_empty();
        let identity = store.spawn(9, snapshot()).unwrap();
        let mut route = RouteProgram {
            entries: vec![RouteEntry::default(), RouteEntry::default()],
            cursor: 0,
        };

        assert_eq!(
            follow_owned_route_link(0, &mut store, identity, &mut route, &mut World).unwrap(),
            None
        );
        assert_eq!(store.behavior_route_cursor(identity), Ok(None));
    }
}
