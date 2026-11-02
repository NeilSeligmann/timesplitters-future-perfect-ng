// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed action entry and the documented direct route-executor cases.
//!
//! `execute_route_entry` covers the documented override and kinds 1-9, 12, 16,
//! 17 and default paths. Animation sampling, navigation and world effects stay
//! explicit host boundaries.
//!
//! Verification tiers are recorded in `docs/t2385-rea-rust-slice.md`.

pub use crate::rea_executor_gate::{TURN_LOCK_MASK, resets_route_to_first};

use crate::{
    awareness::NodeKey,
    navigation_scoring::NodeId,
    rea_point_follow::{PointFollowWorld, follow_owned_primary_point},
    rea_routes::{RouteActor, RouteEntry, RouteProgram, RouteWorld, advance_route},
    rea_tick_state::{
        LocalState, NavigationGraphGeneration, PointFollowEffect, ReaActorIdentity,
        ReaActorStoreError, ReaTickActorStore,
    },
};

pub const PRIMARY_ROUTE_ANIM: u32 = 0x5202b8;
pub const TRANSITION_ROUTE_ANIM: u32 = 0x5202c8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutorResult {
    Held,
    Requested(u32),
    RandomRequest,
    TurnToward(u32),
    Advanced,
    AdvancedThenRandom,
    ResetToFirst,
    DefaultReturn,
}

/// Inputs measured by the navigation/world boundary for the bounded early
/// arrival checks in `0x26EA00`. Distances are supplied explicitly so the
/// actor store never reaches into guest or world state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarlyRouteArrivalObservation {
    pub vertical_delta: f32,
    pub horizontal_distance_squared: f32,
}

/// Result of applying the measured early arrival portion of `0x26EA00`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EarlyRouteArrivalEffect {
    NoAdvance,
    Advanced {
        reached_goal: NodeId,
        selected_entry: RouteEntry,
    },
}

/// Outcome of the bounded post-target movement threshold in `0x26EA00`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MovementThresholdEffect {
    NotReached {
        next_node: NodeId,
        threshold_squared: f32,
    },
    Advanced {
        previous_node: NodeId,
        current_node: NodeId,
        threshold_squared: f32,
        reverse_trace_within: u32,
    },
}

/// Combined selected-node point-follow and movement-threshold result.
/// `movement_threshold` is absent for cached-record, abort, unclassified, or
/// unresolved point-follow results, which return before the measured distance
/// commit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnedRoutePointMovementEffect {
    pub point_follow: PointFollowEffect,
    pub movement_threshold: Option<MovementThresholdEffect>,
}

/// Ordered bounded effects from route entry through the selected point-follow
/// path. This does not represent the whole controller tick or occupancy calls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnedRouteMovementTickEffect {
    pub route_entry: ExecutorResult,
    pub point_follow: PointFollowEffect,
    pub early_arrival: Option<EarlyRouteArrivalEffect>,
    pub movement_threshold: Option<MovementThresholdEffect>,
}

/// Result of the ordered, actor-owned route-entry and early-arrival stages.
/// `arrival` is absent when the caller's preceding 0x26EA00 guards did not
/// select the measured early-arrival branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedRouteTickEffect {
    pub route_entry: ExecutorResult,
    pub early_arrival: Option<EarlyRouteArrivalEffect>,
}

pub trait RouteAnimationHost: RouteWorld {
    fn actor_has_either_animation(&mut self, animation: u32) -> bool;
    fn actor_has_primary_animation(&mut self, animation: u32) -> bool;
    fn route_animation_matches(&mut self) -> bool;
    fn end_animation_matches(&mut self) -> bool;
    fn animation_finished(&mut self) -> bool;
    fn frame_event_crossed(&mut self, kind: u32) -> bool;
    fn request_animation(&mut self, animation: u32);
    fn request_random_route_animation(&mut self, route: &RouteProgram);
    fn set_heading_from_current_node(&mut self);
    fn turn_toward_key(&mut self, key: u32);
    fn angle_to_intermediate(&mut self, node: NodeId) -> f32;
    fn key_for_primary_node(&mut self, node: NodeId) -> NodeKey;
}

/// A readable Rust-owned slice of 0x26E150. The override check is ordered before
/// kind dispatch. This function does not advance through kind 1: the later
/// 0x26EA00 arrival stage owns that decision in the caller.
pub fn execute_route_entry<H: RouteAnimationHost>(
    local_flags: u32,
    actor: &mut RouteActor,
    route: &mut RouteProgram,
    host: &mut H,
) -> ExecutorResult {
    if local_flags & TURN_LOCK_MASK != 0 {
        if !host.actor_has_either_animation(0x520498) {
            host.request_animation(0x520498);
            return ExecutorResult::Requested(0x520498);
        }
        return ExecutorResult::Held;
    }

    let kind = route.entries[route.cursor as usize].kind;
    if resets_route_to_first(local_flags, kind) {
        route.cursor = 1;
        return ExecutorResult::ResetToFirst;
    }
    match kind {
        1 => {
            if host.actor_has_either_animation(TRANSITION_ROUTE_ANIM) {
                if !host.animation_finished() {
                    return ExecutorResult::Held;
                }
                host.request_animation(PRIMARY_ROUTE_ANIM);
                ExecutorResult::Requested(PRIMARY_ROUTE_ANIM)
            } else if host.actor_has_either_animation(PRIMARY_ROUTE_ANIM) {
                ExecutorResult::Held
            } else {
                host.request_animation(PRIMARY_ROUTE_ANIM);
                ExecutorResult::Requested(PRIMARY_ROUTE_ANIM)
            }
        }
        4 => {
            // Guard order follows the measured 26CBE0/F pair.
            if !host.route_animation_matches() || host.animation_finished() {
                host.request_random_route_animation(route);
                host.set_heading_from_current_node();
                ExecutorResult::RandomRequest
            } else {
                host.set_heading_from_current_node();
                ExecutorResult::Held
            }
        }
        5..=9 => execute_grouped_route_kind(kind, actor, route, host),
        2 | 3 | 16 => execute_turn_route_kind(kind, actor, route, host),
        17 => execute_grouped_route_kind(kind, actor, route, host),
        _ => ExecutorResult::DefaultReturn,
    }
}

/// Executes the bounded route-entry stage only for a route already owned by
/// the selected actor generation. The typed actor navigation state must match
/// the supplied loaded navigation graph; the behavior cursor remains
/// actor-owned. The loaded `RouteProgram` contributes route-table data and
/// receives the resulting cursor for its caller.
///
/// `None` means the adapter has not installed both owned route state and a
/// behavior cursor, so the lifted caller remains responsible for this tick.
/// A graph mismatch fails closed before host queries. This native generation
/// guard does not establish the original graph teardown ordering. The function
/// does not connect the lifted adapter or establish whole-tick ordering.
pub fn execute_owned_route_entry<H: RouteAnimationHost>(
    local_flags: u32,
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    graph: NavigationGraphGeneration,
    route: &mut RouteProgram,
    host: &mut H,
) -> Result<Option<ExecutorResult>, ReaActorStoreError> {
    let state = store
        .state(identity)
        .ok_or(ReaActorStoreError::StaleIdentity)?;
    if store.navigation_graph_generation() != Some(graph)
        || state.actor_navigation().graph_generation() != Some(graph)
    {
        return Err(ReaActorStoreError::StaleNavigationGraph);
    }
    if !state.navigation_route_owned() {
        return Ok(None);
    }
    let Some(cursor) = state.behavior_route_cursor() else {
        return Ok(None);
    };

    let mut actor = state.navigation_route();
    route.cursor = cursor;
    let result = execute_route_entry(local_flags, &mut actor, route, host);

    store.set_navigation_route(identity, actor)?;
    store.set_behavior_route_cursor(identity, route.cursor)?;
    Ok(Some(result))
}

/// Composes the measured route-entry stage before the caller-selected early
/// arrival branch, retaining one actor-generation-owned route across both.
/// The caller supplies `Some` only after the preceding 0x26EA00 guards admit
/// that branch. This is a bounded route substep, not the full AI tick: search,
/// ordinary movement, cached-record failure, later thresholds, secondary
/// claims, and natural call ordering remain outside this contract.
pub fn execute_owned_route_tick<H: RouteAnimationHost>(
    local_flags: u32,
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    graph: NavigationGraphGeneration,
    route: &mut RouteProgram,
    early_arrival: Option<EarlyRouteArrivalObservation>,
    host: &mut H,
) -> Result<Option<OwnedRouteTickEffect>, ReaActorStoreError> {
    let Some(route_entry) =
        execute_owned_route_entry(local_flags, store, identity, graph, route, host)?
    else {
        return Ok(None);
    };

    let early_arrival = match early_arrival {
        Some(observation) => {
            advance_owned_route_on_early_arrival(store, identity, graph, route, observation, host)?
        }
        None => None,
    };

    Ok(Some(OwnedRouteTickEffect {
        route_entry,
        early_arrival,
    }))
}

/// Applies only the bounded early-arrival branches of `0x26EA00` to an
/// actor-owned route. The caller supplies world-space distance measurements
/// and invokes this after route-entry processing and the measured turn/search
/// guards. The actor-owned cursor is copied to the loaded route before the
/// local-state ownership check, so the lifted caller receives the selected
/// cursor when it retains this stage. Local state must already be Rust-owned
/// before native arrival decisions run. Local states 5 through 7 and unmatched
/// kinds fall through without changing actor state.
/// This does not model ordinary movement, cached-record failure, the later
/// current-node threshold, or whole-tick caller ordering.
pub fn advance_owned_route_on_early_arrival<H: RouteAnimationHost>(
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    graph: NavigationGraphGeneration,
    route: &mut RouteProgram,
    observation: EarlyRouteArrivalObservation,
    host: &mut H,
) -> Result<Option<EarlyRouteArrivalEffect>, ReaActorStoreError> {
    let state = store
        .state(identity)
        .ok_or(ReaActorStoreError::StaleIdentity)?;
    if store.navigation_graph_generation() != Some(graph)
        || state.actor_navigation().graph_generation() != Some(graph)
    {
        return Err(ReaActorStoreError::StaleNavigationGraph);
    }
    if !state.navigation_route_owned() {
        return Ok(None);
    }
    let Some(cursor) = state.behavior_route_cursor() else {
        return Ok(None);
    };
    route.cursor = cursor;
    if !state.owns_local_state() {
        return Ok(None);
    }
    if !early_route_arrival_matches(route, state.local_state(), observation) {
        return Ok(Some(EarlyRouteArrivalEffect::NoAdvance));
    }

    let mut actor = state.navigation_route();
    let reached_goal = actor.goal_node;
    actor.current_node = reached_goal;
    let selected_entry = advance_route(&mut actor, route, host);
    store.set_navigation_route(identity, actor)?;
    store.set_behavior_route_cursor(identity, route.cursor)?;
    Ok(Some(EarlyRouteArrivalEffect::Advanced {
        reached_goal,
        selected_entry,
    }))
}

/// Applies the bounded distance threshold after `0x26EA00` has selected a
/// next node and movement target. The caller supplies the measured squared
/// distance and the actor extension's local flags; this operation updates
/// only the Rust-owned current node and within-route counter for the matching
/// actor generation.
///
/// The base threshold is strictly `distance_squared < 0.01` when local flag
/// `0x100` is set, otherwise strictly `< 0.2025`. When local state is 1 and
/// the selected next node is the route goal, the peeked route kind selects
/// the measured larger thresholds (6.25 for 2/3/16; 2.25 for 5-7/17).
/// This is a bounded movement commit, not the complete movement stage: route
/// search, target resolution, early-arrival ordering, other local states,
/// occupancy, and the lifted runtime adapter remain separate.
pub fn advance_owned_route_on_movement_threshold(
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    graph: NavigationGraphGeneration,
    route: &mut RouteProgram,
    local_flags: u32,
    distance_squared: f32,
) -> Result<Option<MovementThresholdEffect>, ReaActorStoreError> {
    let state = store
        .state(identity)
        .ok_or(ReaActorStoreError::StaleIdentity)?;
    if store.navigation_graph_generation() != Some(graph)
        || state.actor_navigation().graph_generation() != Some(graph)
    {
        return Err(ReaActorStoreError::StaleNavigationGraph);
    }
    if !state.navigation_route_owned() || !state.owns_local_state() {
        return Ok(None);
    }
    let Some(cursor) = state.behavior_route_cursor() else {
        return Ok(None);
    };
    let Some(next_node) = state.actor_navigation().next_node() else {
        return Ok(None);
    };

    route.cursor = cursor;
    let mut actor = state.navigation_route();
    let mut threshold_squared = if local_flags & 0x100 != 0 {
        0.01
    } else {
        0.2025
    };
    if state.local_state() == LocalState(1) && next_node == actor.goal_node {
        let (_, peeked) = route.peek_next();
        match peeked.kind {
            2 | 3 | 16 => threshold_squared = 6.25,
            5..=7 | 17 => threshold_squared = 2.25,
            _ => {}
        }
    }

    if !(distance_squared < threshold_squared) {
        return Ok(Some(MovementThresholdEffect::NotReached {
            next_node,
            threshold_squared,
        }));
    }

    let previous_node = actor.current_node;
    actor.current_node = next_node;
    actor.reverse_trace_within = actor.reverse_trace_within.wrapping_sub(1);
    let reverse_trace_within = actor.reverse_trace_within;
    store.set_navigation_route(identity, actor)?;
    Ok(Some(MovementThresholdEffect::Advanced {
        previous_node,
        current_node: next_node,
        threshold_squared,
        reverse_trace_within,
    }))
}

/// Runs the bounded ordinary point-follow movement path for one actor-owned
/// route. The actor's typed current node feeds the world search, its resolved
/// target is committed to typed navigation state, and the measured distance
/// threshold then advances the owned route when selected-node search succeeds.
/// Early-arrival guards, route search/goal selection, occupancy, and the lifted
/// runtime adapter remain separate boundaries.
pub fn follow_owned_route_point_and_commit_threshold<W: PointFollowWorld>(
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    graph: NavigationGraphGeneration,
    route: &mut RouteProgram,
    local_flags: u32,
    distance_squared: f32,
    world: &mut W,
) -> Result<Option<OwnedRoutePointMovementEffect>, ReaActorStoreError> {
    let state = store
        .state(identity)
        .ok_or(ReaActorStoreError::StaleIdentity)?;
    if store.navigation_graph_generation() != Some(graph)
        || state.actor_navigation().graph_generation() != Some(graph)
    {
        return Err(ReaActorStoreError::StaleNavigationGraph);
    }
    if !state.navigation_route_owned() || !state.owns_local_state() {
        return Ok(None);
    }

    let current_node = state.actor_navigation().route().current_node;
    let point_follow = follow_owned_primary_point(store, identity, graph, current_node, world)?;
    let movement_threshold = match point_follow {
        PointFollowEffect::SelectedNodeCommitted(_) => advance_owned_route_on_movement_threshold(
            store,
            identity,
            graph,
            route,
            local_flags,
            distance_squared,
        )?,
        _ => None,
    };
    Ok(Some(OwnedRoutePointMovementEffect {
        point_follow,
        movement_threshold,
    }))
}

/// Composes the measured caller order for route entry, point search, and the
/// selected movement commit. A resolved point is installed before early
/// arrival checks; when those checks advance the route, the later distance
/// threshold is skipped. An unselected or unresolved point returns without
/// arrival or threshold changes. The caller supplies world observations and
/// keeps occupancy, callbacks, other controller states, and the lifted
/// adapter outside this bounded Rust-owned slice.
pub fn execute_owned_route_movement_tick<H: RouteAnimationHost, W: PointFollowWorld>(
    local_flags: u32,
    distance_squared: f32,
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    graph: NavigationGraphGeneration,
    route: &mut RouteProgram,
    early_arrival: Option<EarlyRouteArrivalObservation>,
    host: &mut H,
    world: &mut W,
) -> Result<Option<OwnedRouteMovementTickEffect>, ReaActorStoreError> {
    let Some(route_entry) =
        execute_owned_route_entry(local_flags, store, identity, graph, route, host)?
    else {
        return Ok(None);
    };

    let current_node = store
        .state(identity)
        .ok_or(ReaActorStoreError::StaleIdentity)?
        .actor_navigation()
        .route()
        .current_node;
    let point_follow = follow_owned_primary_point(store, identity, graph, current_node, world)?;

    let mut early_effect = None;
    let mut movement_threshold = None;
    if matches!(point_follow, PointFollowEffect::SelectedNodeCommitted(_)) {
        if let Some(observation) = early_arrival {
            early_effect = advance_owned_route_on_early_arrival(
                store,
                identity,
                graph,
                route,
                observation,
                host,
            )?;
        }
        if !matches!(early_effect, Some(EarlyRouteArrivalEffect::Advanced { .. })) {
            movement_threshold = advance_owned_route_on_movement_threshold(
                store,
                identity,
                graph,
                route,
                local_flags,
                distance_squared,
            )?;
        }
    }

    Ok(Some(OwnedRouteMovementTickEffect {
        route_entry,
        point_follow,
        early_arrival: early_effect,
        movement_threshold,
    }))
}

fn early_route_arrival_matches(
    route: &RouteProgram,
    local_state: LocalState,
    observation: EarlyRouteArrivalObservation,
) -> bool {
    if matches!(local_state.0, 5..=7) {
        return false;
    }
    match route.entries[route.cursor as usize].kind {
        0 => true,
        1 if observation.vertical_delta.abs() < 0.5 => {
            let (_, peeked) = route.peek_next();
            let max_xz_distance_squared = match peeked.kind {
                1 => 0.2025,
                2 | 3 | 16 => 9.0,
                5..=9 | 17 => 3.24,
                _ => return false,
            };
            observation.horizontal_distance_squared < max_xz_distance_squared
        }
        _ => false,
    }
}

/// Turn route kinds share the measured primary-animation and event gates.
/// Intermediate IDs and sampled angles are supplied by typed navigation and
/// animation boundaries; valid route state carries an intermediate node for 16.
fn execute_turn_route_kind<H: RouteAnimationHost>(
    kind: u32,
    actor: &mut RouteActor,
    route: &mut RouteProgram,
    host: &mut H,
) -> ExecutorResult {
    if [0x5202f8, 0x520308, 0x5202e8]
        .into_iter()
        .any(|animation| host.actor_has_primary_animation(animation))
    {
        let key = if kind == 16 {
            let intermediate = actor
                .intermediate_node
                .expect("kind 16 route requires its measured intermediate node");
            host.key_for_primary_node(intermediate).0 as u32
        } else {
            route.peek_next().1.payload_b
        };
        host.turn_toward_key(key);
        return ExecutorResult::TurnToward(key);
    }

    if !host.frame_event_crossed(kind) {
        return ExecutorResult::Held;
    }
    match kind {
        2 => {
            host.request_animation(0x5202f8);
            ExecutorResult::Requested(0x5202f8)
        }
        3 => {
            host.request_animation(0x520308);
            ExecutorResult::Requested(0x520308)
        }
        16 => {
            let intermediate = actor
                .intermediate_node
                .expect("kind 16 route requires its measured intermediate node");
            let angle = host.angle_to_intermediate(intermediate);
            if angle.abs() < 45.0 {
                advance_route(actor, route, host);
                ExecutorResult::Advanced
            } else if angle.abs() > 135.0 {
                host.request_animation(0x5202e8);
                ExecutorResult::Requested(0x5202e8)
            } else if angle > 0.0 {
                host.request_animation(0x5202f8);
                ExecutorResult::Requested(0x5202f8)
            } else {
                host.request_animation(0x520308);
                ExecutorResult::Requested(0x520308)
            }
        }
        _ => unreachable!(),
    }
}

/// The measured ordered animation guards for route kinds 5 through 9.
/// Kind 17 shares the outer group and uses its measured intermediate-angle
/// dispatch after a completed route animation.
fn execute_grouped_route_kind<H: RouteAnimationHost>(
    kind: u32,
    actor: &mut RouteActor,
    route: &mut RouteProgram,
    host: &mut H,
) -> ExecutorResult {
    let none_of_group_animations = !host.actor_has_either_animation(0x5202d8)
        && !host.actor_has_either_animation(TRANSITION_ROUTE_ANIM)
        && !host.route_animation_matches()
        && !host.end_animation_matches();
    if none_of_group_animations {
        if host.actor_has_either_animation(PRIMARY_ROUTE_ANIM) {
            if host.frame_event_crossed(kind) {
                host.request_animation(0x5202d8);
                return ExecutorResult::Requested(0x5202d8);
            }
            return ExecutorResult::Held;
        }
        host.request_random_route_animation(route);
        return ExecutorResult::RandomRequest;
    }

    if host.actor_has_either_animation(0x5202d8) && host.animation_finished() {
        host.request_random_route_animation(route);
        return ExecutorResult::RandomRequest;
    }

    if host.route_animation_matches() && host.animation_finished() {
        return match kind {
            5 => {
                host.request_animation(0x520318);
                ExecutorResult::Requested(0x520318)
            }
            6 => {
                host.request_animation(0x520328);
                ExecutorResult::Requested(0x520328)
            }
            7 => {
                host.request_animation(0x520338);
                ExecutorResult::Requested(0x520338)
            }
            8 => {
                let (_, next) = route.peek_next();
                if next.kind == 1 {
                    host.request_animation(TRANSITION_ROUTE_ANIM);
                    ExecutorResult::Requested(TRANSITION_ROUTE_ANIM)
                } else {
                    advance_route(actor, route, host);
                    host.request_random_route_animation(route);
                    ExecutorResult::AdvancedThenRandom
                }
            }
            17 => {
                let intermediate = actor
                    .intermediate_node
                    .expect("kind 17 route requires its measured intermediate node");
                let angle = host.angle_to_intermediate(intermediate);
                let animation = if angle.abs() < 45.0 {
                    TRANSITION_ROUTE_ANIM
                } else if angle > 0.0 {
                    0x520328
                } else {
                    0x520338
                };
                host.request_animation(animation);
                ExecutorResult::Requested(animation)
            }
            // 9's nested dispatch is the measured default return.
            _ => ExecutorResult::DefaultReturn,
        };
    }

    if host.end_animation_matches() {
        let (_, next) = route.peek_next();
        if next.kind == 1 {
            if host.animation_finished() {
                host.request_animation(TRANSITION_ROUTE_ANIM);
                return ExecutorResult::Requested(TRANSITION_ROUTE_ANIM);
            }
            host.turn_toward_key(next.payload_b);
            return ExecutorResult::TurnToward(next.payload_b);
        }
        if host.animation_finished() {
            advance_route(actor, route, host);
            host.request_random_route_animation(route);
            return ExecutorResult::AdvancedThenRandom;
        }
    }
    ExecutorResult::Held
}

#[cfg(test)]
mod owned_tests {
    use super::{
        EarlyRouteArrivalEffect, EarlyRouteArrivalObservation, ExecutorResult,
        MovementThresholdEffect, OwnedRouteMovementTickEffect, OwnedRoutePointMovementEffect,
        OwnedRouteTickEffect, RouteAnimationHost, advance_owned_route_on_early_arrival,
        advance_owned_route_on_movement_threshold, execute_owned_route_entry,
        execute_owned_route_movement_tick, execute_owned_route_tick,
        follow_owned_route_point_and_commit_threshold,
    };
    use crate::{
        awareness::{AwarenessCache, NodeKey},
        movement_approach::Position3,
        navigation_scoring::NodeId,
        rea_point_follow::PointFollowWorld,
        rea_routes::{PointFollowSearchResult, RouteActor, RouteEntry, RouteProgram, RouteWorld},
        rea_tick_state::{
            BehaviorMode, BehaviorPair, LocalState, NavigationGraphGeneration, PointFollowEffect,
            ReaActorIdentity, ReaActorStoreError, ReaTickActorStore, ReaTickSnapshot,
            TransitionProgress,
        },
    };

    struct PointWorld {
        result: PointFollowSearchResult,
        target: Option<Position3>,
        cached: Option<Position3>,
        calls: alloc::vec::Vec<&'static str>,
    }

    impl PointFollowWorld for PointWorld {
        fn search_primary_node(&mut self, _current_node: NodeId) -> PointFollowSearchResult {
            self.calls.push("search");
            self.result
        }

        fn selected_node_movement_target(&mut self, _node: NodeId) -> Option<Position3> {
            self.calls.push("selected-target");
            self.target
        }

        fn cached_record_position(&mut self) -> Option<Position3> {
            self.calls.push("cached-position");
            self.cached
        }
    }

    struct Host {
        crossed_events: u32,
        events: alloc::vec::Vec<&'static str>,
    }

    impl RouteWorld for Host {
        fn resolve_key(&mut self, key: NodeKey) -> Option<NodeId> {
            self.events.push("resolve");
            Some(NodeId(key.0 as u32))
        }

        fn player_current_node(&mut self) -> NodeId {
            NodeId(0)
        }

        fn find_intermediate(&mut self, _current: NodeId, _target: NodeId) -> i32 {
            -1
        }

        fn abort_route(&mut self) {}
    }

    impl RouteAnimationHost for Host {
        fn actor_has_either_animation(&mut self, _animation: u32) -> bool {
            false
        }

        fn actor_has_primary_animation(&mut self, _animation: u32) -> bool {
            false
        }

        fn route_animation_matches(&mut self) -> bool {
            false
        }

        fn end_animation_matches(&mut self) -> bool {
            false
        }

        fn animation_finished(&mut self) -> bool {
            false
        }

        fn frame_event_crossed(&mut self, _kind: u32) -> bool {
            self.crossed_events += 1;
            true
        }

        fn request_animation(&mut self, _animation: u32) {
            self.events.push("request");
        }

        fn request_random_route_animation(&mut self, _route: &RouteProgram) {}

        fn set_heading_from_current_node(&mut self) {}

        fn turn_toward_key(&mut self, _key: u32) {}

        fn angle_to_intermediate(&mut self, _node: NodeId) -> f32 {
            0.0
        }

        fn key_for_primary_node(&mut self, node: NodeId) -> NodeKey {
            NodeKey(node.0 as i32)
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

    fn own_local_state(
        store: &mut ReaTickActorStore,
        identity: ReaActorIdentity,
        state: LocalState,
    ) {
        store.queue_local_state(identity, state).unwrap();
        store.apply_local_state_prologue(identity).unwrap();
    }

    fn movement_threshold_setup(
        local_state: u32,
        current_node: u32,
        goal_node: u32,
        next_node: u32,
        peeked_kind: u32,
        reverse_trace_within: u32,
    ) -> (
        ReaTickActorStore,
        ReaActorIdentity,
        NavigationGraphGeneration,
        RouteProgram,
    ) {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(1, snapshot()).unwrap();
        own_local_state(&mut store, identity, LocalState(local_state));
        store
            .set_navigation_route(
                identity,
                RouteActor {
                    current_node: NodeId(current_node),
                    goal_node: NodeId(goal_node),
                    intermediate_node: None,
                    reverse_trace_within,
                    reverse_trace_other: 3,
                },
            )
            .unwrap();
        store.set_behavior_route_cursor(identity, 1).unwrap();
        store
            .set_navigation_next_node(identity, NodeId(next_node))
            .unwrap();
        let route = RouteProgram {
            entries: alloc::vec![
                RouteEntry::default(),
                RouteEntry {
                    kind: 1,
                    payload_a: 0,
                    payload_b: 0,
                },
                RouteEntry {
                    kind: peeked_kind,
                    payload_a: 0,
                    payload_b: 0,
                },
            ],
            cursor: 0,
        };
        (store, identity, graph, route)
    }

    #[test]
    fn movement_threshold_uses_strict_default_distance_and_decrements_owned_trace() {
        let (mut store, identity, graph, mut route) = movement_threshold_setup(2, 3, 9, 4, 1, 5);

        assert_eq!(
            advance_owned_route_on_movement_threshold(
                &mut store, identity, graph, &mut route, 0, 0.2025
            ),
            Ok(Some(MovementThresholdEffect::NotReached {
                next_node: NodeId(4),
                threshold_squared: 0.2025,
            }))
        );
        assert_eq!(
            advance_owned_route_on_movement_threshold(
                &mut store, identity, graph, &mut route, 0, 0.2
            ),
            Ok(Some(MovementThresholdEffect::Advanced {
                previous_node: NodeId(3),
                current_node: NodeId(4),
                threshold_squared: 0.2025,
                reverse_trace_within: 4,
            }))
        );
        let actor = store.navigation_route(identity).unwrap();
        assert_eq!(actor.current_node, NodeId(4));
        assert_eq!(actor.reverse_trace_within, 4);
        assert_eq!(actor.reverse_trace_other, 3);
    }

    #[test]
    fn owned_route_point_follow_commits_target_before_threshold_advance() {
        let (mut store, identity, graph, mut route) = movement_threshold_setup(2, 3, 9, 4, 1, 5);
        let target = Position3 {
            x: 8.0,
            y: 2.0,
            z: 6.0,
        };
        let mut world = PointWorld {
            result: PointFollowSearchResult::SelectedNode(NodeId(4)),
            target: Some(target),
            cached: None,
            calls: alloc::vec::Vec::new(),
        };

        assert_eq!(
            follow_owned_route_point_and_commit_threshold(
                &mut store, identity, graph, &mut route, 0, 0.1, &mut world,
            ),
            Ok(Some(OwnedRoutePointMovementEffect {
                point_follow: PointFollowEffect::SelectedNodeCommitted(NodeId(4)),
                movement_threshold: Some(MovementThresholdEffect::Advanced {
                    previous_node: NodeId(3),
                    current_node: NodeId(4),
                    threshold_squared: 0.2025,
                    reverse_trace_within: 4,
                }),
            }))
        );
        assert_eq!(world.calls, ["search", "selected-target"]);
        let actor = store.state(identity).unwrap().actor_navigation();
        assert_eq!(actor.route().current_node, NodeId(4));
        assert_eq!(actor.movement_target(), Some(target));
    }

    #[test]
    fn owned_movement_tick_searches_before_early_arrival_and_skips_later_threshold() {
        let (mut store, identity, graph, mut route) = movement_threshold_setup(1, 3, 9, 4, 1, 5);
        let target = Position3 {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec::Vec::new(),
        };
        let mut world = PointWorld {
            result: PointFollowSearchResult::SelectedNode(NodeId(4)),
            target: Some(target),
            cached: None,
            calls: alloc::vec::Vec::new(),
        };

        assert_eq!(
            execute_owned_route_movement_tick(
                0,
                0.0,
                &mut store,
                identity,
                graph,
                &mut route,
                Some(EarlyRouteArrivalObservation {
                    vertical_delta: 0.0,
                    horizontal_distance_squared: 0.1,
                }),
                &mut host,
                &mut world,
            ),
            Ok(Some(OwnedRouteMovementTickEffect {
                route_entry: ExecutorResult::Requested(super::PRIMARY_ROUTE_ANIM),
                point_follow: PointFollowEffect::SelectedNodeCommitted(NodeId(4)),
                early_arrival: Some(EarlyRouteArrivalEffect::Advanced {
                    reached_goal: NodeId(9),
                    selected_entry: route.entries[2],
                }),
                movement_threshold: None,
            }))
        );
        assert_eq!(world.calls, ["search", "selected-target"]);
        assert_eq!(host.events, ["request", "resolve"]);
        let navigation = store.state(identity).unwrap().actor_navigation();
        assert_eq!(navigation.route().current_node, NodeId(9));
        assert_eq!(navigation.next_node(), Some(NodeId(4)));
        assert_eq!(navigation.movement_target(), Some(target));
        assert_eq!(store.behavior_route_cursor(identity), Ok(Some(2)));
    }

    #[test]
    fn cached_point_follow_returns_before_movement_threshold() {
        let (mut store, identity, graph, mut route) = movement_threshold_setup(2, 3, 9, 4, 1, 5);
        let cached = Position3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let mut world = PointWorld {
            result: PointFollowSearchResult::CachedRecord,
            target: None,
            cached: Some(cached),
            calls: alloc::vec::Vec::new(),
        };

        assert_eq!(
            follow_owned_route_point_and_commit_threshold(
                &mut store, identity, graph, &mut route, 0, 0.0, &mut world,
            ),
            Ok(Some(OwnedRoutePointMovementEffect {
                point_follow: PointFollowEffect::CachedRecordTargetCommitted {
                    point_follow_scalar: 1.4,
                },
                movement_threshold: None,
            }))
        );
        assert_eq!(world.calls, ["search", "cached-position"]);
        assert_eq!(
            store
                .state(identity)
                .unwrap()
                .actor_navigation()
                .route()
                .current_node,
            NodeId(3)
        );
    }

    #[test]
    fn movement_threshold_uses_local_flag_and_goal_route_kind_overrides() {
        let (mut flagged, flagged_id, flagged_graph, mut flagged_route) =
            movement_threshold_setup(2, 3, 9, 4, 1, 2);
        assert_eq!(
            advance_owned_route_on_movement_threshold(
                &mut flagged,
                flagged_id,
                flagged_graph,
                &mut flagged_route,
                0x100,
                0.01,
            ),
            Ok(Some(MovementThresholdEffect::NotReached {
                next_node: NodeId(4),
                threshold_squared: 0.01,
            }))
        );

        let (mut goal, goal_id, goal_graph, mut goal_route) =
            movement_threshold_setup(1, 3, 7, 7, 2, 2);
        assert_eq!(
            advance_owned_route_on_movement_threshold(
                &mut goal,
                goal_id,
                goal_graph,
                &mut goal_route,
                0x100,
                6.25,
            ),
            Ok(Some(MovementThresholdEffect::NotReached {
                next_node: NodeId(7),
                threshold_squared: 6.25,
            }))
        );
        assert_eq!(
            advance_owned_route_on_movement_threshold(
                &mut goal,
                goal_id,
                goal_graph,
                &mut goal_route,
                0,
                6.24,
            ),
            Ok(Some(MovementThresholdEffect::Advanced {
                previous_node: NodeId(3),
                current_node: NodeId(7),
                threshold_squared: 6.25,
                reverse_trace_within: 1,
            }))
        );
    }

    #[test]
    fn owned_route_tick_runs_entry_before_the_guard_selected_arrival_stage() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(1, snapshot()).unwrap();
        own_local_state(&mut store, identity, LocalState(1));
        store
            .set_navigation_route(
                identity,
                RouteActor {
                    current_node: NodeId(3),
                    goal_node: NodeId(7),
                    intermediate_node: None,
                    reverse_trace_within: 4,
                    reverse_trace_other: 5,
                },
            )
            .unwrap();
        store.set_behavior_route_cursor(identity, 1).unwrap();
        let mut route = RouteProgram {
            entries: alloc::vec![
                RouteEntry::default(),
                RouteEntry {
                    kind: 1,
                    payload_a: 0,
                    payload_b: 0,
                },
                RouteEntry {
                    kind: 2,
                    payload_a: 12,
                    payload_b: 0,
                },
            ],
            cursor: 0,
        };
        let observation = EarlyRouteArrivalObservation {
            vertical_delta: 0.25,
            horizontal_distance_squared: 0.1,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec![],
        };

        assert_eq!(
            execute_owned_route_tick(
                0,
                &mut store,
                identity,
                graph,
                &mut route,
                Some(observation),
                &mut host,
            ),
            Ok(Some(OwnedRouteTickEffect {
                route_entry: ExecutorResult::Requested(super::PRIMARY_ROUTE_ANIM),
                early_arrival: Some(EarlyRouteArrivalEffect::Advanced {
                    reached_goal: NodeId(7),
                    selected_entry: route.entries[2],
                }),
            }))
        );
        assert_eq!(host.events, ["request", "resolve"]);
        assert_eq!(
            store.navigation_route(identity).unwrap().current_node,
            NodeId(7)
        );
        assert_eq!(
            store.navigation_route(identity).unwrap().goal_node,
            NodeId(12)
        );
        assert_eq!(store.behavior_route_cursor(identity), Ok(Some(2)));
    }

    #[test]
    fn owned_route_entry_commits_cursor_and_goal_to_the_actor_generation() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(1, snapshot()).unwrap();
        store
            .set_navigation_route(
                identity,
                RouteActor {
                    current_node: NodeId(3),
                    goal_node: NodeId(4),
                    intermediate_node: Some(NodeId(5)),
                    reverse_trace_within: 6,
                    reverse_trace_other: 7,
                },
            )
            .unwrap();
        store.set_behavior_route_cursor(identity, 1).unwrap();

        let mut route = RouteProgram {
            entries: alloc::vec![
                RouteEntry::default(),
                RouteEntry {
                    kind: 16,
                    payload_a: 0,
                    payload_b: 0,
                },
                RouteEntry {
                    kind: 0,
                    payload_a: 77,
                    payload_b: 0,
                },
            ],
            cursor: 0,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec![],
        };

        assert_eq!(
            execute_owned_route_entry(0, &mut store, identity, graph, &mut route, &mut host),
            Ok(Some(ExecutorResult::Advanced))
        );
        assert_eq!(host.crossed_events, 1);
        assert_eq!(route.cursor, 2);
        assert_eq!(store.behavior_route_cursor(identity), Ok(Some(2)));
        assert_eq!(
            store.navigation_route(identity).unwrap().goal_node,
            NodeId(77)
        );
    }

    #[test]
    fn owned_kind12_route_entry_resets_cursor_without_changing_navigation_state() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(1, snapshot()).unwrap();
        let route_state = RouteActor {
            current_node: NodeId(3),
            goal_node: NodeId(9),
            intermediate_node: Some(NodeId(7)),
            reverse_trace_within: 4,
            reverse_trace_other: 5,
        };
        store.set_navigation_route(identity, route_state).unwrap();
        store.set_behavior_route_cursor(identity, 6).unwrap();

        let mut route = RouteProgram {
            entries: alloc::vec![
                RouteEntry::default(),
                RouteEntry::default(),
                RouteEntry::default(),
                RouteEntry::default(),
                RouteEntry::default(),
                RouteEntry::default(),
                RouteEntry {
                    kind: 12,
                    payload_a: 0,
                    payload_b: 0,
                },
            ],
            cursor: 0,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec![],
        };

        assert_eq!(
            execute_owned_route_entry(0, &mut store, identity, graph, &mut route, &mut host),
            Ok(Some(ExecutorResult::ResetToFirst))
        );
        assert_eq!(route.cursor, 1);
        assert_eq!(store.behavior_route_cursor(identity), Ok(Some(1)));
        assert_eq!(store.navigation_route(identity), Ok(route_state));
        assert_eq!(host.crossed_events, 0);
        assert!(host.events.is_empty());
    }

    #[test]
    fn owned_route_entry_leaves_unowned_actor_to_the_lifted_caller() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(1, snapshot()).unwrap();
        let mut route = RouteProgram {
            entries: alloc::vec![RouteEntry::default()],
            cursor: 0,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec![],
        };

        assert_eq!(
            execute_owned_route_entry(0, &mut store, identity, graph, &mut route, &mut host),
            Ok(None)
        );
        assert_eq!(host.crossed_events, 0);
        assert_eq!(store.behavior_route_cursor(identity), Ok(None));

        let stale = crate::rea_tick_state::ReaActorIdentity {
            slot: identity.slot,
            generation: identity.generation.wrapping_add(1),
        };
        assert_eq!(
            execute_owned_route_entry(0, &mut store, stale, graph, &mut route, &mut host),
            Err(ReaActorStoreError::StaleIdentity)
        );
    }

    #[test]
    fn owned_route_entry_rejects_a_replaced_navigation_graph_before_host_calls() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(1, snapshot()).unwrap();
        store
            .set_navigation_route(identity, RouteActor::default())
            .unwrap();
        store.set_behavior_route_cursor(identity, 0).unwrap();
        let mut route = RouteProgram {
            entries: alloc::vec![RouteEntry::default()],
            cursor: 0,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec![],
        };
        let current_graph = store.replace_navigation_graph().unwrap();

        assert_ne!(graph, current_graph);
        assert_eq!(
            execute_owned_route_entry(0, &mut store, identity, graph, &mut route, &mut host),
            Err(ReaActorStoreError::StaleNavigationGraph)
        );
        assert_eq!(host.crossed_events, 0);
        assert_eq!(route.cursor, 0);
        assert_eq!(
            execute_owned_route_entry(
                0,
                &mut store,
                identity,
                current_graph,
                &mut route,
                &mut host,
            ),
            Ok(None)
        );
        assert_eq!(host.crossed_events, 0);
    }

    #[test]
    fn owned_route_early_arrival_commits_reached_goal_and_next_route_state() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(2, snapshot()).unwrap();
        own_local_state(&mut store, identity, LocalState(1));
        store
            .set_navigation_route(
                identity,
                RouteActor {
                    current_node: NodeId(3),
                    goal_node: NodeId(7),
                    intermediate_node: None,
                    reverse_trace_within: 5,
                    reverse_trace_other: 6,
                },
            )
            .unwrap();
        store.set_behavior_route_cursor(identity, 1).unwrap();
        let mut route = RouteProgram {
            entries: alloc::vec![
                RouteEntry::default(),
                RouteEntry {
                    kind: 0,
                    payload_a: 0,
                    payload_b: 0,
                },
                RouteEntry {
                    kind: 2,
                    payload_a: 12,
                    payload_b: 0,
                },
            ],
            cursor: 0,
        };
        let observation = EarlyRouteArrivalObservation {
            vertical_delta: 90.0,
            horizontal_distance_squared: 900.0,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec![],
        };

        assert_eq!(
            advance_owned_route_on_early_arrival(
                &mut store,
                identity,
                graph,
                &mut route,
                observation,
                &mut host,
            ),
            Ok(Some(EarlyRouteArrivalEffect::Advanced {
                reached_goal: NodeId(7),
                selected_entry: route.entries[2],
            }))
        );
        let actor_route = store.navigation_route(identity).unwrap();
        assert_eq!(actor_route.current_node, NodeId(7));
        assert_eq!(actor_route.goal_node, NodeId(12));
        assert_eq!(actor_route.reverse_trace_within, 0);
        assert_eq!(actor_route.reverse_trace_other, 0);
        assert_eq!(route.cursor, 2);
        assert_eq!(store.behavior_route_cursor(identity), Ok(Some(2)));
    }

    #[test]
    fn owned_route_proximity_uses_strict_peeked_kind_threshold_and_skips_states_5_to_7() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(3, snapshot()).unwrap();
        own_local_state(&mut store, identity, LocalState(1));
        store
            .set_navigation_route(
                identity,
                RouteActor {
                    current_node: NodeId(3),
                    goal_node: NodeId(7),
                    intermediate_node: None,
                    reverse_trace_within: 5,
                    reverse_trace_other: 6,
                },
            )
            .unwrap();
        store.set_behavior_route_cursor(identity, 1).unwrap();
        let mut route = RouteProgram {
            entries: alloc::vec![
                RouteEntry::default(),
                RouteEntry {
                    kind: 1,
                    payload_a: 0,
                    payload_b: 0,
                },
                RouteEntry {
                    kind: 2,
                    payload_a: 12,
                    payload_b: 0,
                },
            ],
            cursor: 0,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec![],
        };
        let outside = EarlyRouteArrivalObservation {
            vertical_delta: 0.0,
            horizontal_distance_squared: 9.0,
        };
        assert_eq!(
            advance_owned_route_on_early_arrival(
                &mut store, identity, graph, &mut route, outside, &mut host,
            ),
            Ok(Some(EarlyRouteArrivalEffect::NoAdvance))
        );
        assert_eq!(
            store.navigation_route(identity).unwrap().current_node,
            NodeId(3)
        );

        own_local_state(&mut store, identity, LocalState(5));
        let inside_but_skipped = EarlyRouteArrivalObservation {
            vertical_delta: 0.0,
            horizontal_distance_squared: 1.0,
        };
        assert_eq!(
            advance_owned_route_on_early_arrival(
                &mut store,
                identity,
                graph,
                &mut route,
                inside_but_skipped,
                &mut host,
            ),
            Ok(Some(EarlyRouteArrivalEffect::NoAdvance))
        );
        assert_eq!(
            store.navigation_route(identity).unwrap().current_node,
            NodeId(3)
        );
        assert_eq!(route.cursor, 1);
    }

    #[test]
    fn owned_route_early_arrival_defers_when_local_state_is_not_rust_owned() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let identity = store.spawn(4, snapshot()).unwrap();
        store
            .set_navigation_route(
                identity,
                RouteActor {
                    current_node: NodeId(3),
                    goal_node: NodeId(7),
                    intermediate_node: None,
                    reverse_trace_within: 5,
                    reverse_trace_other: 6,
                },
            )
            .unwrap();
        store.set_behavior_route_cursor(identity, 1).unwrap();
        let mut route = RouteProgram {
            entries: alloc::vec![
                RouteEntry::default(),
                RouteEntry {
                    kind: 0,
                    payload_a: 0,
                    payload_b: 0,
                },
            ],
            cursor: 0,
        };
        let mut host = Host {
            crossed_events: 0,
            events: alloc::vec![],
        };

        assert_eq!(
            advance_owned_route_on_early_arrival(
                &mut store,
                identity,
                graph,
                &mut route,
                EarlyRouteArrivalObservation {
                    vertical_delta: 0.0,
                    horizontal_distance_squared: 0.0,
                },
                &mut host,
            ),
            Ok(None)
        );
        assert_eq!(
            store.navigation_route(identity).unwrap().current_node,
            NodeId(3)
        );
        assert_eq!(route.cursor, 1);
        assert_eq!(host.crossed_events, 0);
        assert!(host.events.is_empty());
    }
}
