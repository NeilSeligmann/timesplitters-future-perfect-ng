// SPDX-License-Identifier: GPL-3.0-or-later
//! T2385's opt-in host-boundary adoption of merged Rust AI owners.
//!
//! The live actor still resides in the legacy guest image, so this adapter is a
//! host-boundary snapshot/writeback. The behavior itself calls the shared typed
//! owners from shared `tsfp-ai` source files. Admitted boundaries are the
//! 0x266250 XZ approach, route peeks/advancement, route-link kinds 0/1 and
//! ordinary target selection, kind-12 0x26E150 reset, selected 0x161730
//! perception paths, the ordered 0x25FAF0 awareness source loop, and mechanism
//! handle/record resolution plus the 0x2A0500 mechanism states 0..14.
//! Larger awareness, perception, executor and mechanism boundaries still
//! decline to their retained lane implementation.

#[path = "../../tsfp-ai/src/rea_ai_trace.rs"]
mod ai_trace;

use crate::rea_awareness_sources::{
    AlertSource, AwarenessAlertWorld, AwarenessSourceRecord, process_awareness_sources,
};
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    awareness::AwarenessCache,
    memory::GuestMemory,
    movement_approach::{Position3, approach_xz},
    navigation_scoring::{
        ActorId, CandidateGroup, NavNode, NavPosition, NodeId, ObjectId, OtherActorGoal,
        ScoringActor, ScoringWorld, select_best_goal,
    },
    perception_geometry::{
        ActorGeometry, AngularAdmissionQuery, DirectionProvider, Vec3, angular_admitted,
    },
    rea_controller_target_commit::{
        ControllerTargetEffect, ControllerTargetHandoff, SelectedGoalEffect,
    },
    rea_group_target::{
        GROUP_BUCKETS, GROUP_MASK_LIMIT, Position3 as GroupPosition3, RecentCandidate,
        nearest_candidate,
    },
    rea_link_decision::{LinkFollowDecision, LinkFollowInput, link_follow_decision},
    rea_routes::{RouteActor, RouteEntry, RouteProgram, RouteWorld, advance_route_index},
    rea_tick_state::{
        AwarenessRecipientState, BehaviorChange, BehaviorMode, BehaviorPair,
        CachedNavigationRecord, LocalState, NavigationGraphGeneration, ReaActorIdentity,
        ReaTickActorStore, ReaTickSnapshot, TransitionProgress,
    },
};
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[cfg(all(not(test), not(feature = "test-diagnostics")))]
unsafe extern "C" {
    fn host_rust_ports_nested_call_args(va: u32, phase: u32, arg0: u32, eax: u32);
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static ENABLED_FUNCTIONS: AtomicU64 = AtomicU64::new(0);

struct LiveActorState {
    store: ReaTickActorStore,
    /// Last observed tuple of measured route-table roots. This is an adapter
    /// invalidation key, not proof that the guest allocator replaced a graph.
    published_navigation_roots: Option<[u32; 4]>,
    /// Guest extension addresses are boundary handles only. Rust identities
    /// come from explicit actor-list append events.
    bindings: Vec<(u32, ReaActorIdentity)>,
    /// Object identity at append/movement boundaries is an inferred guard
    /// against applying one extension generation to a replacement actor.
    actor_objects: Vec<(u32, u32)>,
    ambiguous_actor_extensions: Vec<u32>,
}

impl Default for LiveActorState {
    fn default() -> Self {
        Self {
            store: ReaTickActorStore::default(),
            published_navigation_roots: None,
            bindings: Vec::new(),
            actor_objects: Vec::new(),
            ambiguous_actor_extensions: Vec::new(),
        }
    }
}

struct LockedLiveActorState {
    held: AtomicBool,
    state: UnsafeCell<LiveActorState>,
}

// All mutable access is serialized by `lock`.
unsafe impl Sync for LockedLiveActorState {}

impl LockedLiveActorState {
    const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
            state: UnsafeCell::new(LiveActorState {
                store: ReaTickActorStore::new_empty(),
                published_navigation_roots: None,
                bindings: Vec::new(),
                actor_objects: Vec::new(),
                ambiguous_actor_extensions: Vec::new(),
            }),
        }
    }

    fn lock(&self) -> LiveActorStateGuard<'_> {
        while self
            .held
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }
        LiveActorStateGuard { owner: self }
    }
}

struct LiveActorStateGuard<'a> {
    owner: &'a LockedLiveActorState,
}

impl Deref for LiveActorStateGuard<'_> {
    type Target = LiveActorState;

    fn deref(&self) -> &Self::Target {
        // SAFETY: this guard owns the lock until drop.
        unsafe { &*self.owner.state.get() }
    }
}

impl DerefMut for LiveActorStateGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: this guard owns the lock until drop.
        unsafe { &mut *self.owner.state.get() }
    }
}

impl Drop for LiveActorStateGuard<'_> {
    fn drop(&mut self) {
        self.owner.held.store(false, Ordering::Release);
    }
}

static LIVE_ACTORS: LockedLiveActorState = LockedLiveActorState::new();
const GAME_MODE_BYTE: u32 = 0x007d_e455;
const CHARACTER_EXTENSION_COUNT: u32 = 0x0074_c37c;
const CHARACTER_EXTENSION_ARRAY: u32 = 0x007a_3580;
/// CharacterSet's 32-slot capacity is INFERRED from its typed owner. Reject a
/// larger count instead of walking beyond the bounded table.
const CHARACTER_EXTENSION_CAPACITY: u32 = 32;

#[derive(Clone, Copy, Debug)]
pub struct BehaviorCommitToken {
    extension: u32,
    identity: ReaActorIdentity,
    mode: BehaviorMode,
    selected: BehaviorPair,
    change: BehaviorChange,
}

#[derive(Clone, Copy, Debug)]
pub struct ActorControllerToken {
    extension: u32,
    identity: ReaActorIdentity,
    target_before: u32,
    navigation_before: NavigationCompatibilitySnapshot,
    navigation_graph_generation_before: Option<NavigationGraphGeneration>,
}

/// Compatibility fields around the measured actor-controller boundary.
/// Route cursor is managed separately by the selected per-call cursor seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NavigationCompatibilitySnapshot {
    current_node: u32,
    cached_route_record: u32,
    next_node: u32,
    goal_node: u32,
    reverse_trace_within: u32,
    reverse_trace_other: u32,
    movement_target: [u32; 3],
}

fn navigation_compatibility_snapshot<M: GuestMemory>(
    memory: &M,
    extension: u32,
) -> NavigationCompatibilitySnapshot {
    NavigationCompatibilitySnapshot {
        current_node: memory.read32(extension.wrapping_add(0x678)),
        cached_route_record: memory.read32(extension.wrapping_add(0x67c)),
        next_node: memory.read32(extension.wrapping_add(0x680)),
        goal_node: memory.read32(extension.wrapping_add(0x684)),
        reverse_trace_within: memory.read32(extension.wrapping_add(0x9a8)),
        reverse_trace_other: memory.read32(extension.wrapping_add(0x9ac)),
        movement_target: [0x30, 0x34, 0x38]
            .map(|offset| memory.read32(extension.wrapping_add(offset))),
    }
}

/// Import the measured route/movement write set after `0x26EFC0` returns.
/// This outer-boundary association is INFERRED: the bounded REA contract
/// measures the ordered `0x26E150 -> 0x26EA00` consumers and their fields, but
/// does not close every writer that may run inside the controller callback.
fn import_navigation_writeback(
    store: &mut ReaTickActorStore,
    identity: ReaActorIdentity,
    before: NavigationCompatibilitySnapshot,
    after: NavigationCompatibilitySnapshot,
) {
    if let Ok(mut route) = store.navigation_route(identity) {
        let mut changed = false;
        if before.current_node != after.current_node {
            route.current_node = NodeId(after.current_node);
            changed = true;
        }
        if before.goal_node != after.goal_node {
            route.goal_node = NodeId(after.goal_node);
            changed = true;
        }
        if before.reverse_trace_within != after.reverse_trace_within {
            route.reverse_trace_within = after.reverse_trace_within;
            changed = true;
        }
        if before.reverse_trace_other != after.reverse_trace_other {
            route.reverse_trace_other = after.reverse_trace_other;
            changed = true;
        }
        if changed {
            let _ = store.set_navigation_route(identity, route);
        }
    }
    if before.next_node != after.next_node {
        let _ = store.set_navigation_next_node(identity, NodeId(after.next_node));
    }
    if before.cached_route_record != after.cached_route_record {
        let _ = store.set_cached_navigation_record(
            identity,
            CachedNavigationRecord(after.cached_route_record),
        );
    }
    if before.movement_target != after.movement_target {
        let _ = store.set_navigation_movement_target(
            identity,
            Position3 {
                x: f32::from_bits(after.movement_target[0]),
                y: f32::from_bits(after.movement_target[1]),
                z: f32::from_bits(after.movement_target[2]),
            },
        );
    }
}

fn actor_snapshot<M: GuestMemory>(memory: &M, extension: u32) -> ReaTickSnapshot {
    let pair = |offset| BehaviorPair {
        low: memory.read32(extension.wrapping_add(offset)),
        high: memory.read32(extension.wrapping_add(offset + 4)),
    };
    let pending_local = memory.read32(extension.wrapping_add(0x9d0)) as i32;
    ReaTickSnapshot {
        mode: if memory.read8(GAME_MODE_BYTE) == 10 {
            BehaviorMode::Mode10
        } else {
            BehaviorMode::Other
        },
        mode10_active: pair(0x10),
        other_active: pair(0x18),
        previous_behavior: pair(0xab0),
        pending_behavior: pair(0xaa8),
        local_state: LocalState(memory.read32(extension.wrapping_add(0x20))),
        previous_local_state: LocalState(memory.read32(extension.wrapping_add(0x9d4))),
        pending_local_state: (pending_local >= 0).then_some(LocalState(pending_local as u32)),
        transition_progress: TransitionProgress(memory.read32(extension.wrapping_add(0x9d8))),
        awareness_cache: AwarenessCache::from_raw(memory.read32(extension.wrapping_add(0xadc))),
    }
}

/// Apply the measured queued local-state transition before the lifted consumer
/// runs. Guest writes are a TEMPORARY-SHIM so the remaining lifted body sees
/// the consumed queue and continues from the Rust-owned typed result.
fn apply_local_state_prologue<M: GuestMemory>(memory: &mut M, regs: &ExtRegisters) {
    let actor = memory.read32(regs.esp.wrapping_add(4));
    if actor == 0 {
        return;
    }
    let extension = memory.read32(actor.wrapping_add(0x7c));
    if extension == 0 {
        return;
    }
    bind_actor_extension(memory, extension);
    let snapshot = actor_snapshot(memory, extension);
    let mut live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return;
    };
    let Ok(Some(commit)) = live
        .store
        .apply_compatibility_local_state_prologue(identity, snapshot)
    else {
        return;
    };

    if commit.changed {
        memory.write32(extension.wrapping_add(0x9d4), commit.previous.0);
        memory.write32(extension.wrapping_add(0x20), commit.current.0);
        memory.write32(extension.wrapping_add(0x9d8), 0);
    }
    memory.write32(extension.wrapping_add(0x9d0), u32::MAX);
}

/// Bind actors by validated constructor slot; generations remain Rust-owned.
pub fn actor_spawned<M: GuestMemory>(memory: &M, object: u32) {
    if !enabled() || object == 0 {
        return;
    }
    let extension = memory.read32(object.wrapping_add(0x7c));
    if extension == 0 {
        return;
    }
    bind_actor_extension(memory, extension);
    let _ = record_actor_object(extension, object);
}

/// Remember the concrete object seen at a measured boundary. Reusing one
/// extension for a different object is not enough evidence to retire or
/// recreate its actor generation, so mark it ambiguous and let lifted code
/// handle selected movement leaves until the explicit unlink edge resolves it.
fn record_actor_object(extension: u32, object: u32) -> bool {
    let mut live = LIVE_ACTORS.lock();
    if !live.bindings.iter().any(|(bound, _)| *bound == extension) {
        return false;
    }
    if live.ambiguous_actor_extensions.contains(&extension) {
        return false;
    }
    match live
        .actor_objects
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, known)| *known)
    {
        Some(known) if known == object => true,
        Some(_) => {
            live.ambiguous_actor_extensions.push(extension);
            false
        }
        None => {
            live.actor_objects.push((extension, object));
            true
        }
    }
}

fn bind_actor_extension<M: GuestMemory>(memory: &M, extension: u32) {
    if extension == 0 {
        return;
    }
    let snapshot = actor_snapshot(memory, extension);
    let mut live = LIVE_ACTORS.lock();
    if live.bindings.iter().any(|(bound, _)| *bound == extension) {
        return;
    }
    let observed_slot = tsfp_ai_spawn_binding::published_slot_for_extension(memory, extension);
    if let Some(identity) = live.store.spawn_with_observed_slot(observed_slot, snapshot) {
        live.bindings.push((extension, identity));
    }
}

/// Bind every extension in the measured CharacterSet roster before a selected
/// actor movement leaf runs. The roster stores extension handles directly;
/// this only adds missing identities. Retirement stays on the explicit
/// CharacterSet unlink hook because callback-time roster mutation and allocator
/// reclamation order are still open in REA.
fn bind_active_actor_roster<M: GuestMemory>(memory: &M) -> bool {
    let count = memory.read32(CHARACTER_EXTENSION_COUNT);
    if count > CHARACTER_EXTENSION_CAPACITY {
        return false;
    }
    for index in 0..count {
        let extension = memory.read32(CHARACTER_EXTENSION_ARRAY + index * 4);
        if extension != 0 {
            bind_actor_extension(memory, extension);
        }
    }
    true
}

/// TEMPORARY-SHIM (removal tracked by T2407): keep the selected behavior-program
/// cursor (`extension + 0xAA0`) typed across Rust helpers within one full actor
/// tick. Unsupported executor paths and the outer tick boundary relinquish it
/// to lifted writers until their slice is ported.
fn behavior_route_cursor<M: GuestMemory>(memory: &M, extension: u32) -> u32 {
    let guest_cursor = memory.read32(extension.wrapping_add(0xaa0));
    let mut live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return guest_cursor;
    };
    match live.store.behavior_route_cursor(identity) {
        Ok(Some(cursor)) => cursor,
        Ok(None) => {
            let _ = live.store.set_behavior_route_cursor(identity, guest_cursor);
            guest_cursor
        }
        Err(_) => guest_cursor,
    }
}

fn set_behavior_route_cursor(extension: u32, cursor: u32) {
    let mut live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return;
    };
    let _ = live.store.set_behavior_route_cursor(identity, cursor);
}

fn try_set_behavior_route_cursor(extension: u32, cursor: u32) -> bool {
    let mut live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return false;
    };
    live.store
        .set_behavior_route_cursor(identity, cursor)
        .is_ok()
}

fn clear_behavior_route_cursor(extension: u32) {
    let mut live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return;
    };
    let _ = live.store.clear_behavior_route_cursor(identity);
}

/// Bind the actor passed to the measured per-actor controller. T1660 now
/// measures `[0x7A3580]` / `[0x74C37C]` as the counted CharacterSet extension
/// roster; this seam still takes identity from the caller's object and
/// `object+0x7c`, without retaining that guest pointer across dispatch.
pub fn actor_controller_started<M: GuestMemory>(
    memory: &mut M,
    actor: u32,
) -> Option<ActorControllerToken> {
    if !enabled() || actor == 0 {
        return None;
    }
    let extension = memory.read32(actor.wrapping_add(0x7c));
    if extension == 0 {
        return None;
    }
    bind_actor_extension(memory, extension);
    let mut live = LIVE_ACTORS.lock();
    let identity = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)?;
    // TEMPORARY-SHIM: lifted target producers still write the pending DWORD.
    // Import that request at the measured controller entry, commit it through
    // the generation-owned Rust state, and project only the measured writes
    // before the retained controller body reads the fields.
    let handoff = ControllerTargetHandoff::new(
        memory.read32(extension.wrapping_add(0x20)),
        memory.read32(extension.wrapping_add(0x9d0)) as i32,
        memory.read32(extension.wrapping_add(0x9d4)),
    );
    let navigation_before = navigation_compatibility_snapshot(memory, extension);
    let navigation_graph_generation_before = live.store.navigation_graph_generation();
    let route_snapshot = RouteActor {
        current_node: NodeId(navigation_before.current_node),
        goal_node: NodeId(navigation_before.goal_node),
        intermediate_node: None,
        reverse_trace_within: navigation_before.reverse_trace_within,
        reverse_trace_other: navigation_before.reverse_trace_other,
    };
    let _ = live
        .store
        .seed_navigation_route_if_unowned(identity, route_snapshot);
    let seed_cached_route_record = live
        .store
        .state(identity)
        .is_some_and(|state| state.actor_navigation().cached_route_record().is_none());
    if seed_cached_route_record {
        let _ = live.store.seed_cached_navigation_record_if_unowned(
            identity,
            CachedNavigationRecord(navigation_before.cached_route_record),
        );
    }
    let seed_next_node = live
        .store
        .state(identity)
        .is_some_and(|state| state.actor_navigation().next_node().is_none());
    if seed_next_node {
        let _ = live
            .store
            .set_navigation_next_node(identity, NodeId(navigation_before.next_node));
    }
    let seed_movement_target = live
        .store
        .state(identity)
        .is_some_and(|state| state.actor_navigation().movement_target().is_none());
    if seed_movement_target {
        let _ = live.store.set_navigation_movement_target(
            identity,
            Position3 {
                x: f32::from_bits(navigation_before.movement_target[0]),
                y: f32::from_bits(navigation_before.movement_target[1]),
                z: f32::from_bits(navigation_before.movement_target[2]),
            },
        );
    }
    if live
        .store
        .set_controller_target_handoff(identity, handoff)
        .is_ok()
    {
        if let Ok(Some(effect)) = live.store.apply_controller_target_request(identity) {
            match effect {
                ControllerTargetEffect::NegativePendingSkipped { .. } => {}
                ControllerTargetEffect::EqualRequestRetired { .. } => {
                    memory.write32(extension.wrapping_add(0x9d0), u32::MAX);
                }
                ControllerTargetEffect::TargetChanged { previous, current } => {
                    memory.write32(extension.wrapping_add(0x9d8), 0);
                    memory.write32(extension.wrapping_add(0x9d4), previous);
                    memory.write32(extension.wrapping_add(0x20), current);
                    memory.write32(extension.wrapping_add(0x9d0), u32::MAX);
                }
            }
        }
    }
    // The full controller still delegates unported executor branches. Start
    // this invocation from the measured guest value, then share typed cursor
    // updates only across selected helpers in this tick.
    let _ = live.store.clear_behavior_route_cursor(identity);
    Some(ActorControllerToken {
        extension,
        identity,
        target_before: memory.read32(extension.wrapping_add(0x20)),
        navigation_before,
        navigation_graph_generation_before,
    })
}

/// `actor_controller_started` at the nested `GuestCall` seam (`LaneGuest::call`), which is
/// entered before the x86 return slot is pushed: the actor (argument one) is `[esp]`.
/// T3478: reading `[esp + 4]` took the caller's previous argument slot, which a lifted
/// callee (0x15CD00 under 0x27D590) may reuse as scratch, and faulted on Story level 3.
pub fn actor_controller_started_at_nested_call<M: GuestMemory>(
    memory: &mut M,
    esp: u32,
) -> Option<ActorControllerToken> {
    let actor = memory.read32(esp);
    actor_controller_started(memory, actor)
}

/// Publish the compatibility snapshot after the measured controller sequence
/// returns. Revalidate before reading the guest extension: a callback may
/// synchronously unregister its actor and invalidate this token.
pub fn actor_controller_finished<M: GuestMemory>(memory: &M, token: Option<ActorControllerToken>) {
    let Some(token) = token else { return };
    let mut live = LIVE_ACTORS.lock();
    if !live
        .bindings
        .iter()
        .any(|(bound, identity)| *bound == token.extension && *identity == token.identity)
    {
        return;
    }
    let snapshot = actor_snapshot(memory, token.extension);
    // TEMPORARY-SHIM (removed with T2407's lifted controller): the four
    // measured selected-goal branches leave this exact write footprint.
    // Import only that footprint into the actor generation; unrelated
    // target writers remain outside the typed model.
    let current = memory.read32(token.extension.wrapping_add(0x20));
    let selected_goal_writeback = current != token.target_before
        && memory.read32(token.extension.wrapping_add(0x9d4)) == token.target_before
        && memory.read32(token.extension.wrapping_add(0x9d0)) == current
        && memory.read32(token.extension.wrapping_add(0x9d8)) == 0;
    if selected_goal_writeback
        && live
            .store
            .controller_target_handoff(token.identity)
            .is_ok_and(|handoff| {
                handoff.is_some_and(|handoff| handoff.current() == token.target_before)
            })
        && matches!(
            live.store
                .apply_measured_selected_goal(token.identity, current),
            Ok(Some(SelectedGoalEffect::SelectedGoalInstalled { .. }))
        )
    {
        // The lifted setter already projected the measured writes. The
        // typed store now owns the same result for subsequent Rust leaves.
    }
    // TEMPORARY-SHIM (removal tracked by T2407): REA measures the outer
    // controller's ordered route executor and movement/arrival stages. Publish
    // their changed compatibility fields only after the whole call returns,
    // so nested lifted readers retain the original call order.
    if live.store.navigation_graph_generation() == token.navigation_graph_generation_before {
        let navigation_after = navigation_compatibility_snapshot(memory, token.extension);
        import_navigation_writeback(
            &mut live.store,
            token.identity,
            token.navigation_before,
            navigation_after,
        );
    }
    // TEMPORARY-SHIM: lifted code still owns local-state and unported
    // behavior transitions. Import them at the full controller boundary.
    live.store
        .refresh_compatibility_snapshot(token.identity, snapshot);
    let _ = live.store.clear_behavior_route_cursor(token.identity);
}

/// Bind the measured live extension roster and exact actor passed to an inner
/// movement leaf when level loading bypassed the explicit append callback.
fn actor_movement_started<M: GuestMemory>(memory: &M, actor: u32) -> bool {
    if !enabled() || actor == 0 {
        return false;
    }
    let _ = bind_active_actor_roster(memory);
    let extension = memory.read32(actor.wrapping_add(0x7c));
    if extension == 0 {
        return false;
    }
    bind_actor_extension(memory, extension);
    record_actor_object(extension, actor)
}

/// Establish a Rust graph generation only after one of the measured published
/// route-table roots is live. Root presence is measured; using it to mint the
/// initial native generation is an INFERRED ownership policy. The measured
/// 0x9A260 clear path advances the generation on later teardown. This token
/// does not claim ownership of the guest-backed graph allocations.
fn published_navigation_graph_generation<M: GuestMemory>(
    memory: &M,
) -> Option<NavigationGraphGeneration> {
    let mut roots = [0; NAVIGATION_GRAPH_ROOTS.len()];
    for (index, root) in NAVIGATION_GRAPH_ROOTS.into_iter().enumerate() {
        if !memory.is_readable(root, 4) {
            return None;
        }
        roots[index] = memory.read32(root);
    }
    let published = roots.iter().any(|root| *root != 0);
    let mut live = LIVE_ACTORS.lock();
    if !published {
        if live.published_navigation_roots.take().is_some() {
            // Root absence is only observed at this adapter boundary. Treat
            // it as a native invalidation policy; it does not establish the
            // lifted loader's allocation or claim-cleanup behavior.
            let _ = live.store.replace_navigation_graph();
        }
        return None;
    }

    if live
        .published_navigation_roots
        .is_some_and(|previous| previous != roots)
    {
        let generation = live.store.replace_navigation_graph().ok()?;
        live.published_navigation_roots = Some(roots);
        return Some(generation);
    }
    live.published_navigation_roots = Some(roots);
    if let Some(generation) = live.store.navigation_graph_generation() {
        return Some(generation);
    }
    live.store.replace_navigation_graph().ok()
}

fn store_navigation_movement_target(extension: u32, target: Position3) {
    let mut live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return;
    };
    let _ = live.store.set_navigation_movement_target(identity, target);
}

/// Unregister the Rust AI state after `0x15DDA0` removes its extension from the
/// active actor list. This retires only the Rust generation; it does not claim
/// that the keyed extension allocation is freed.
pub fn actor_unregistered(extension: u32) {
    if extension == 0 {
        return;
    }
    let mut live = LIVE_ACTORS.lock();
    if let Some(index) = live
        .bindings
        .iter()
        .position(|(bound, _)| *bound == extension)
    {
        let (_, identity) = live.bindings.swap_remove(index);
        live.store.retire(identity);
    }
    live.actor_objects.retain(|(bound, _)| *bound != extension);
    live.ambiguous_actor_extensions
        .retain(|bound| *bound != extension);
}

/// Captures the active selector before retained 0x15c700 dispatches callbacks.
/// The callback observes this active pair; the pending pair is installed only
/// after the callback returns, including any replacement queued during it.
pub fn actor_behavior_commit_started<M: GuestMemory>(
    memory: &M,
    extension: u32,
) -> Option<BehaviorCommitToken> {
    if !enabled() || extension == 0 {
        return None;
    }
    let mut live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return None;
    };
    // TEMPORARY-SHIM: several lifted producers still write the pending pair
    // directly. Import the measured pre-callback snapshot at 0x15c700 so the
    // typed owner can advance even when no Rust 0x17570 request claimed it.
    live.store
        .reconcile_lifted_commit(identity, actor_snapshot(memory, extension))
        .ok()?;
    let change = live.store.pending_behavior_change(identity).ok()??;
    Some(BehaviorCommitToken {
        extension,
        identity,
        mode: change.mode,
        selected: change.previous,
        change,
    })
}

/// Imports fields still owned by the lifted callback/claim and local-state
/// sequence. Active and pending behavior pairs remain Rust-owned, including
/// any later request queued by a callback.
pub fn actor_behavior_committed<M: GuestMemory>(memory: &M, token: BehaviorCommitToken) {
    actor_behavior_tick_finished(memory, token.extension, Some(token));
}

/// Completes the 0x15c700 typed boundary after its retained callback sequence.
/// A changed behavior request is committed from the final pending pair; every
/// call also refreshes still lifted-owned local-state and compatibility fields
/// into the actor's Rust snapshot. The latter remains a TEMPORARY-SHIM until
/// the local-state handlers and callback writers are ported.
pub fn actor_behavior_tick_finished<M: GuestMemory>(
    memory: &M,
    extension: u32,
    token: Option<BehaviorCommitToken>,
) {
    if !enabled() || extension == 0 {
        return;
    }
    let mut live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return;
    };
    // A retained callback may synchronously unregister this actor and even
    // reuse its extension address before the lifted tick returns. Validate
    // the captured Rust identity before reading any compatibility fields.
    if let Some(token) = token.as_ref() {
        if token.extension != extension || token.identity != identity {
            return;
        }
    }
    let snapshot = actor_snapshot(memory, extension);
    if let Some(token) = token {
        let current_after_callbacks = match token.mode {
            BehaviorMode::Mode10 => snapshot.mode10_active,
            BehaviorMode::Other => snapshot.other_active,
        };
        if matches!(
            live.store.finish_behavior_tick_with_pending(
                token.identity,
                token.mode,
                token.selected,
                snapshot.pending_behavior,
            ),
            Ok(true)
        ) {
            // The lifted callback path has already projected these effects to
            // guest memory. Mirror its measured post-callback cleanup in the
            // typed actor state so later Rust navigation reads observe the
            // same claim lifetime.
            let _ = live.store.cleanup_navigation_after_behavior_callbacks(
                token.identity,
                token.change,
                current_after_callbacks,
            );
        }
    }
    let _ = live
        .store
        .refresh_compatibility_snapshot(identity, snapshot);
}

fn run_behavior_request<M: GuestMemory>(memory: &mut M, regs: &ExtRegisters) -> bool {
    let extension = memory.read32(regs.esp.wrapping_add(4));
    let request = BehaviorPair {
        low: memory.read32(regs.esp.wrapping_add(8)),
        high: memory.read32(regs.esp.wrapping_add(12)),
    };
    let accepted = {
        let mut live = LIVE_ACTORS.lock();
        let Some(identity) = live
            .bindings
            .iter()
            .find(|(bound, _)| *bound == extension)
            .map(|(_, identity)| *identity)
        else {
            return false;
        };
        match live.store.request_behavior(identity, request) {
            Ok(accepted) => accepted,
            Err(_) => return false,
        }
    };
    if accepted {
        // TEMPORARY-SHIM (remove under T2407): lifted callers still read these
        // words until their state-machine slices move to the typed actor model.
        // Keep the projection behind the narrow request seam; Rust owns the pair.
        memory.write32(extension.wrapping_add(0xaa8), request.low);
        memory.write32(extension.wrapping_add(0xaac), request.high);
    }
    true
}

/// Stable typed state rows for the `tools.rust_trace` adapter.
pub fn actor_state_trace_rows() -> Vec<(ReaActorIdentity, ReaTickSnapshot)> {
    LIVE_ACTORS.lock().store.trace_rows().collect()
}

/// FFI view for the headless `tools.rust_trace` observer.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_rea_ai_trace_snapshot(extension: u32, output: *mut u32) -> i32 {
    if output.is_null() {
        return -2;
    }
    if !enabled() {
        return 0;
    }
    let live = LIVE_ACTORS.lock();
    let Some(identity) = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)
    else {
        return -1;
    };
    let Some(state) = live.store.state(identity) else {
        return -1;
    };
    let words = ai_trace::snapshot_words(identity, state);
    // SAFETY: the C observer supplies writable storage for this fixed-size row.
    unsafe { core::ptr::copy_nonoverlapping(words.as_ptr(), output, words.len()) };
    1
}

#[cfg(test)]
fn reset_actor_state() {
    *LIVE_ACTORS.lock() = LiveActorState::default();
}

/// Runtime adapter entry points whose behavior has a documented shared Rust owner.
/// The optional per-address switch exists for live A/B diagnosis; unlisted paths
/// continue through the retained lane implementation.
const ALL_ADOPTED_FUNCTIONS: u64 = (1u64 << 52) - 1;
/// Number of supported adapters, derived from the mask.
pub const ADAPTER_COUNT: u32 = ALL_ADOPTED_FUNCTIONS.count_ones();
/// Bit 16 (`0x15D500`), default-on only after its clean 180 s r5 replay (T2385).
const NAV_LINK_BIT: u64 = 1u64 << 16;
const NAVIGATION_GRAPH_ROOTS: [u32; 4] = [0x715f2c, 0x715f30, 0x715f34, 0x715f38];
// 0x26E100, 0x161730 and 0x12F00 faulted on the owner route only with the old
// nested stack ABI; after the call-frame fix each ran a fault-free 180 s r5
// hold replay alone and together (T2385, 2026-10-17), so all are default-on.
// T2422: the three point-table leaves (bits 13 to 15) are default-on under `--rust-rea-ai` after a
// clean 180 s r5 replay (`--rust-rea-ai-disable=VA` for A/B); see docs/t2422-ai-point-table-leaves.md.
// The T2434 particle-pool adapter (bit 32) stays default-on after its clean replay.
// T2437 adapters remain opt-in until the requested 180 s r5 gate is clean.
// T2441 weapon-readiness adapter remains opt-in pending the main-host replay.
// T3421: 0x12f00 is adopted after the matched Story level1 snapshot-resume
// gate was Tier 2 EQUAL and fault-free for 180 seconds. Keep the per-address
// switch for A/B diagnosis.
const DEFAULT_ADOPTED_FUNCTIONS: u64 = 0x1_ffff_ffff;

pub const fn adapter_count() -> u32 {
    ALL_ADOPTED_FUNCTIONS.count_ones()
}

pub const fn default_adapter_count() -> u32 {
    DEFAULT_ADOPTED_FUNCTIONS.count_ones()
}

const fn function_bit(va: u32) -> Option<u64> {
    match va {
        0x266250 => Some(1u64 << 0),
        0x261de0 => Some(1u64 << 1),
        0x26d690 => Some(1u64 << 2),
        0x26e100 => Some(1u64 << 3),
        0x26de30 => Some(1u64 << 4),
        0x26e150 => Some(1u64 << 5),
        0x161730 => Some(1u64 << 6),
        0x263ed0 => Some(1u64 << 7),
        0x25faf0 => Some(1u64 << 8),
        0x12f00 => Some(1u64 << 9),
        0x177920 => Some(1u64 << 10),
        0x2a0500 => Some(1u64 << 11),
        0x267ff0 => Some(1u64 << 12),
        0x9cc40 => Some(1u64 << 13),
        0x59d20 => Some(1u64 << 14),
        0x9a420 => Some(1u64 << 15),
        0x15d500 => Some(NAV_LINK_BIT),
        // T2425: awareness and route leaves (bits 17 to 20).
        0xa0640 => Some(1u64 << 17),
        0x349e20 => Some(1u64 << 18),
        0x3797b0 => Some(1u64 << 19),
        0x15d470 => Some(1u64 << 20),
        // Character and spawn table leaves (bits 21 to 26).
        0x12fc0 => Some(1u64 << 21),
        0x1c5210 => Some(1u64 << 22),
        0x1c5250 => Some(1u64 << 23),
        0x2aefb0 => Some(1u64 << 24),
        0x2b2390 => Some(1u64 << 25),
        0x350e70 => Some(1u64 << 26),
        // T2430: pair field 8, type-id class, message 0x3EB and weapon-record action tests (bits 27 to 31).
        0x12fe0 => Some(1u64 << 27),
        0xd6ed0 => Some(1u64 << 28),
        0x27ce70 => Some(1u64 << 29),
        0x26cc80 => Some(1u64 << 30),
        0x26cd50 => Some(1u64 << 31),
        // T2434: particle pool step (bit 32).
        0x288030 => Some(1u64 << 32),
        // T2437: behavior action guard (bit 33), followed by its state-6/state-8 gate (bit 34).
        0x26dbe0 => Some(1u64 << 33),
        0x26dd80 => Some(1u64 << 34),
        // T2407: typed state-6 entry, reached from the measured follow-up gate.
        0x26d8b0 => Some(1u64 << 50),
        // T2407: measured navigation-root invalidation, opt-in pending the AI gate.
        0x9a260 => Some(1u64 << 51),
        // T2440 weapon/player frontier adapters remain opt-in until the main-host replay gate.
        0x26d390 => Some(1u64 << 35),
        0x26ce20 => Some(1u64 << 36),
        0x59d50 => Some(1u64 << 37),
        // T1682 weapon adapters remain opt-in pending the main-host replay gate (bits 38 to 42).
        0x1b6db0 => Some(1u64 << 38),
        // T1678 Q1 shares 0x1B6BF0 with T1682's next-owned selector; use the
        // measured full-cycle owner at that single runtime boundary (bit 39).
        0x1b6bf0 => Some(1u64 << 39),
        0x1b9a30 => Some(1u64 << 40),
        0x1b63d0 => Some(1u64 << 41),
        0x1b7cc0 => Some(1u64 << 42),
        // T1678 ordered group lookup and row-subrecord lookup (opt-in pending replay).
        0x1b6bb0 => Some(1u64 << 43),
        0x1b99f0 => Some(1u64 << 44),
        // T2407 behavior request remains switchable pending the subsystem adoption gate.
        0x17570 => Some(1u64 << 45),
        // T1682 Q8: mode-aware fallback ammo-cost predicate.
        0x1b6c90 => Some(1u64 << 46),
        // T2407 local-state prologues remain opt-in pending subsystem adoption.
        0x2a1f30 => Some(1u64 << 47),
        0x2a4120 => Some(1u64 << 48),
        // T2365: measured capacity predicate; opt-in pending replay adoption.
        0xa4c70 => Some(1u64 << 49),
        _ => None,
    }
}

struct RuntimeRouteWorld<'a, M, G> {
    memory: &'a mut M,
    guest: &'a mut G,
    regs: &'a mut ExtRegisters,
    actor: u32,
    extension: u32,
    intermediate_result: Option<i32>,
}

impl<M: GuestMemory, G: GuestCall> RuntimeRouteWorld<'_, M, G> {
    fn service(&mut self, va: u32, args: &[u32]) -> u32 {
        let saved = *self.regs;
        call_cdecl(self.guest, self.memory, self.regs, va, args);
        let result = self.regs.eax;
        *self.regs = saved;
        result
    }
}

impl<M: GuestMemory, G: GuestCall> RouteWorld for RuntimeRouteWorld<'_, M, G> {
    fn resolve_key(&mut self, key: crate::rea_ai_types::NodeKey) -> Option<NodeId> {
        let node = self.service(0x9cc40, &[key.0 as u32]);
        if (node as i32) < 0 {
            None
        } else {
            // The lifted helper performs a second identical lookup on the
            // successful branch before storing the result.
            Some(NodeId(self.service(0x9cc40, &[key.0 as u32])))
        }
    }

    fn player_current_node(&mut self) -> NodeId {
        let player = self.memory.read32(0x7b0c48);
        let object = self.memory.read32(player.wrapping_add(0x14));
        let extension = self.memory.read32(object.wrapping_add(0x7c));
        NodeId(self.memory.read32(extension.wrapping_add(0x678)))
    }

    fn find_intermediate(&mut self, current: NodeId, target: NodeId) -> i32 {
        // T1662 measures kind-16/17 as a search from current node to the
        // peeked target through 0x1596F0. The neighboring arrival call supplies
        // (actor, &extension.current_node, 0); reuse that observed ABI after
        // projecting the typed endpoints. This call shape is INFERRED for the
        // route-advance caller until a covered owner-route trace confirms it.
        self.memory
            .write32(self.extension.wrapping_add(0x678), current.0);
        self.memory
            .write32(self.extension.wrapping_add(0x684), target.0);
        let result = self.service(
            0x1596f0,
            &[self.actor, self.extension.wrapping_add(0x678), 0],
        ) as i32;
        self.intermediate_result = Some(result);
        result
    }

    fn abort_route(&mut self) {
        // The measured negative-result path calls 0x15D490 with the actor.
        self.service(0x15d490, &[self.actor]);
    }
}

fn run_route_advance<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    // Original 0x26DE30 ABI is cdecl(actor, route). Build the smallest valid
    // route snapshot needed by the shared owner, including the peek row for a
    // kind-16/17 intermediate search. The 0x1596F0 ABI reuse is INFERRED from
    // its neighboring call until an owner-route trace covers this caller.
    let actor = memory.read32(regs.esp.wrapping_add(4));
    if !actor_movement_started(memory, actor) {
        return false;
    }
    let Some(graph) = published_navigation_graph_generation(memory) else {
        return false;
    };
    let route_address = memory.read32(regs.esp.wrapping_add(8));
    let extension = memory.read32(actor.wrapping_add(0x7c));
    let (identity, graph) = {
        let live = LIVE_ACTORS.lock();
        let Some(identity) = live
            .bindings
            .iter()
            .find(|(bound, _)| *bound == extension)
            .map(|(_, identity)| *identity)
        else {
            return false;
        };
        let Some(state) = live.store.state(identity) else {
            return false;
        };
        if state.actor_navigation().graph_generation() != Some(graph) {
            return false;
        }
        (identity, graph)
    };
    let cursor = behavior_route_cursor(memory, extension);
    let control_index = cursor.wrapping_add(1);
    let control = route_address.wrapping_add(control_index.wrapping_mul(12));
    let control_kind = memory.read32(control);
    let selected_index = advance_route_index(cursor, control_kind, memory.read32(control + 4));
    let selected_address = route_address.wrapping_add(selected_index.wrapping_mul(12));
    let selected_kind = memory.read32(selected_address);

    let mut last_index = control_index.max(selected_index);
    if matches!(selected_kind, 16 | 17) {
        // The measured intermediate search peeks after the selected route row.
        // Include the control row and its selected target in this bounded
        // snapshot; the loaded route remains the authoritative table.
        let peek_control_index = selected_index.wrapping_add(1);
        let peek_control = route_address.wrapping_add(peek_control_index.wrapping_mul(12));
        let peek_index = crate::rea_route_peek::peek_next_index(
            selected_index,
            memory.read32(peek_control),
            memory.read32(peek_control.wrapping_add(4)),
        );
        last_index = last_index.max(peek_control_index).max(peek_index);
    }
    let mut entries = Vec::with_capacity(last_index as usize + 1);
    for index in 0..=last_index {
        let address = route_address.wrapping_add(index.wrapping_mul(12));
        entries.push(RouteEntry {
            kind: memory.read32(address),
            payload_a: memory.read32(address.wrapping_add(4)),
            payload_b: memory.read32(address.wrapping_add(8)),
        });
    }
    let mut route = RouteProgram { entries, cursor };
    let route_snapshot = RouteActor {
        current_node: NodeId(memory.read32(extension.wrapping_add(0x678))),
        goal_node: NodeId(memory.read32(extension.wrapping_add(0x684))),
        intermediate_node: None,
        reverse_trace_within: memory.read32(extension.wrapping_add(0x9a8)),
        reverse_trace_other: memory.read32(extension.wrapping_add(0x9ac)),
    };
    let mut route_actor = {
        let mut live = LIVE_ACTORS.lock();
        if live.store.navigation_graph_generation() != Some(graph)
            || live
                .store
                .state(identity)
                .is_none_or(|state| state.actor_navigation().graph_generation() != Some(graph))
            || live
                .store
                .seed_navigation_route_if_unowned(identity, route_snapshot)
                .is_err()
        {
            return false;
        }
        match live.store.navigation_route(identity) {
            Ok(route) => route,
            Err(_) => return false,
        }
    };
    let mut world = RuntimeRouteWorld {
        memory,
        guest,
        regs,
        actor,
        extension,
        intermediate_result: None,
    };
    crate::rea_routes::advance_route(&mut route_actor, &mut route, &mut world);
    let intermediate_result = world.intermediate_result;
    let cursor = route.cursor;
    let goal_node = route_actor.goal_node.0;
    let current_node = route_actor.current_node.0;
    let reverse_trace_within = route_actor.reverse_trace_within;
    let reverse_trace_other = route_actor.reverse_trace_other;
    drop(world);

    let _ = LIVE_ACTORS
        .lock()
        .store
        .set_navigation_route(identity, route_actor);
    memory.write32(extension.wrapping_add(0xaa0), cursor);
    set_behavior_route_cursor(extension, cursor);
    // Compatibility projection for lifted route readers remains a
    // TEMPORARY-SHIM until their typed route consumers are ported.
    memory.write32(extension.wrapping_add(0x678), current_node);
    memory.write32(extension.wrapping_add(0x684), goal_node);
    memory.write32(extension.wrapping_add(0x9a8), reverse_trace_within);
    memory.write32(extension.wrapping_add(0x9ac), reverse_trace_other);
    if intermediate_result.is_some_and(|result| result >= 0)
        && let Some(intermediate) = route_actor.intermediate_node
    {
        memory.write32(extension.wrapping_add(0x9dc), intermediate.0);
    }
    regs.eax = goal_node;
    true
}

/// Port the measured root-clearing leaf while invalidating Rust-owned IDs and
/// claims that refer to the graph being unpublished. Allocation release and
/// actor-pointer cleanup remain unresolved in the REA record.
fn run_navigation_graph_root_invalidation<M: GuestMemory>(
    memory: &mut M,
    regs: &mut ExtRegisters,
) -> bool {
    let saved = *regs;
    if memory.read32(0x6b7aa8) == 0 {
        let had_published_roots = NAVIGATION_GRAPH_ROOTS
            .iter()
            .any(|root| memory.read32(*root) != 0);
        for root in NAVIGATION_GRAPH_ROOTS {
            memory.write32(root, 0);
        }
        let mut live = LIVE_ACTORS.lock();
        if had_published_roots || live.published_navigation_roots.is_some() {
            let _ = live.store.replace_navigation_graph();
        }
        live.published_navigation_roots = None;
    }
    *regs = saved;
    true
}

struct RuntimeAwarenessWorld<'a, M, G> {
    memory: &'a mut M,
    guest: &'a mut G,
    regs: &'a mut ExtRegisters,
    current_record: u32,
}

impl<M: GuestMemory, G: GuestCall> RuntimeAwarenessWorld<'_, M, G> {
    fn service(&mut self, va: u32, args: &[u32]) {
        let saved = *self.regs;
        call_cdecl(self.guest, self.memory, self.regs, va, args);
        *self.regs = saved;
    }
}

impl<M: GuestMemory, G: GuestCall> AwarenessAlertWorld for RuntimeAwarenessWorld<'_, M, G> {
    fn source_count(&self) -> usize {
        let count = self.memory.read32(0x78b520) as i32;
        usize::try_from(count.max(0)).unwrap_or(0)
    }

    fn source_record(&self, index: usize) -> Option<AwarenessSourceRecord> {
        let address = 0x78b2e0u32.wrapping_add((index as u32).wrapping_mul(0x20));
        Some(AwarenessSourceRecord {
            alert_mask: crate::rea_ai_types::GroupMask(self.memory.read32(address + 4)),
            filter_mask: crate::rea_ai_types::GroupMask(self.memory.read32(address + 8)),
            target: match self.memory.read32(address + 0x10) {
                0 => None,
                value => Some(crate::navigation_scoring::ObjectId(value)),
            },
            state: self.memory.read32(address + 0x14) as i32,
            elapsed: f32::from_bits(self.memory.read32(address + 0x18)),
        })
    }

    fn write_source_record(&mut self, index: usize, record: AwarenessSourceRecord) {
        let address = 0x78b2e0u32.wrapping_add((index as u32).wrapping_mul(0x20));
        self.current_record = address;
        self.memory
            .write32(address + 0x10, record.target.map_or(0, |object| object.0));
        self.memory
            .write32(address + 0x18, record.elapsed.to_bits());
        self.memory.write32(address + 0x14, record.state as u32);
    }

    fn set_group_alert_value(&mut self, value: u32) {
        self.service(0x349e20, &[value]);
    }

    fn dispatch_alert(&mut self, source: AlertSource) {
        if matches!(source, AlertSource::RecordOne { .. }) {
            self.service(0x267ff0, &[1, self.current_record, 0]);
        }
    }

    fn register_spawn_entry(&mut self, index: usize) {
        self.service(0xa0640, &[index as u32]);
    }

    fn increment_alert_count(&mut self) {
        let count = self.memory.read32(0x7a2b80);
        self.memory.write32(0x7a2b80, count.wrapping_add(1));
    }
}

fn run_mechanism_handle_resolve<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) -> bool {
    let argument_address = regs.esp.wrapping_add(4);
    if !memory.is_readable(argument_address, 4) {
        return false;
    }
    let handle_address = memory.read32(argument_address);
    if !memory.is_readable(handle_address, 4) {
        return false;
    }
    let raw_handle = memory.read32(handle_address);
    if raw_handle != 0 {
        let slot = crate::rea_mechanism::MechanismHandle(raw_handle).slot();
        // The instruction is `ADD EAX, [0x72f5e8]`: this global contains the
        // pool pointer; it is not itself the pool's address.
        let pool = memory.read32(0x72f5e8);
        let entry = pool.wrapping_add((slot as u32).wrapping_mul(0x2e8));
        if !memory.is_readable(entry.wrapping_add(0x2c), 4) {
            return false;
        }
        let identity = memory.read32(entry.wrapping_add(0x2c));
        let handle = crate::rea_mechanism::MechanismHandle(raw_handle);
        let resolved = crate::rea_mechanism::mechanism_handle_matches(handle, identity);
        // Match the lifted 0x12F00 register effects: EDX receives the caller's
        // handle-slot pointer and ECX receives the full token, including zero.
        regs.edx = handle_address;
        regs.ecx = raw_handle;
        // Every return path leaves the modeled status flags with PF and ZF set
        // and CF and SF clear (CMP-equal on a match, XOR EAX,EAX otherwise).
        regs.eflags = (regs.eflags & !0x8c5) | 0x44;
        if !resolved {
            memory.write32(handle_address, 0);
        }
        regs.eax = if resolved { entry } else { 0 };
    } else {
        // The original still loads both registers for an inert zero handle.
        regs.edx = handle_address;
        regs.ecx = 0;
        regs.eax = 0;
        regs.eflags = (regs.eflags & !0x8c5) | 0x44;
    }
    true
}

fn run_mechanism_record_resolve<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) -> bool {
    let object = memory.read32(regs.esp.wrapping_add(4));
    let extension = memory.read32(object.wrapping_add(0x7c));
    regs.eax = extension.wrapping_add(0x568);
    true
}

struct RuntimeMechanismWorld<'a, M, G> {
    memory: &'a mut M,
    guest: &'a mut G,
    regs: &'a mut ExtRegisters,
    actor: u32,
    extension: u32,
}

impl<M: GuestMemory, G: GuestCall> RuntimeMechanismWorld<'_, M, G> {
    fn service(&mut self, va: u32, args: &[u32]) -> u32 {
        let saved = *self.regs;
        call_cdecl(self.guest, self.memory, self.regs, va, args);
        let result = self.regs.eax;
        *self.regs = saved;
        result
    }
}

impl<M: GuestMemory, G: GuestCall> crate::rea_mechanism::MechanismWorld
    for RuntimeMechanismWorld<'_, M, G>
{
    fn interact(
        &mut self,
        mechanism: &crate::rea_mechanism::MechanismView<'_>,
        actor: crate::navigation_scoring::ActorId,
        state: u32,
        argument: f32,
    ) -> i32 {
        if state == 1 {
            // The original state-1 path resolves this borrowed subrecord before
            // the interaction call, although it does not consume the result.
            self.service(0x177920, &[mechanism.object.0]);
        }
        self.service(
            0x17e010,
            &[mechanism.object.0, actor.0, state, argument.to_bits()],
        ) as i32
    }

    fn prepare_interaction(&mut self) {
        self.service(0x29fe60, &[]);
    }

    fn actor_predicate(&mut self, actor: crate::navigation_scoring::ActorId) -> bool {
        self.service(0x29fe20, &[actor.0]) != 0
    }

    fn current_target(&mut self) -> Option<crate::navigation_scoring::ObjectId> {
        match self.memory.read32(self.extension.wrapping_add(4)) {
            0 => None,
            value => Some(crate::navigation_scoring::ObjectId(value)),
        }
    }

    fn mechanism_ready(&mut self, mechanism: crate::navigation_scoring::ObjectId) -> bool {
        self.service(0x177e90, &[mechanism.0]) != 0
    }

    fn mechanism_begin(&mut self, mechanism: crate::navigation_scoring::ObjectId) {
        self.service(0x177f20, &[mechanism.0]);
    }

    fn mechanism_exit_bit(&mut self, mechanism: crate::navigation_scoring::ObjectId) -> bool {
        self.service(0x177950, &[mechanism.0]) & 0x10 != 0
    }

    fn mechanism_link_field(
        &mut self,
        mechanism: crate::navigation_scoring::ObjectId,
    ) -> Option<u32> {
        // 0x179F70(mechanism, &out) writes a record pointer through a guest
        // out-slot; keep it in a scratch word below the live stack.
        let saved = *self.regs;
        let out = saved.esp.wrapping_sub(0x40);
        self.regs.esp = out;
        call_cdecl(
            self.guest,
            self.memory,
            self.regs,
            0x179f70,
            &[mechanism.0, out],
        );
        let found = self.regs.eax != 0;
        let record = self.memory.read32(out);
        *self.regs = saved;
        found.then(|| self.memory.read32(record.wrapping_add(0x150)))
    }

    fn speed_ramp_inputs(
        &mut self,
        mechanism: crate::navigation_scoring::ObjectId,
    ) -> crate::rea_mechanism::SpeedRampInputs {
        let extension = self.memory.read32(mechanism.0.wrapping_add(0x7c));
        crate::rea_mechanism::SpeedRampInputs {
            sign_only: self.memory.read8(extension.wrapping_add(0x58)) & 1 != 0,
            reference: f32::from_bits(self.memory.read32(extension.wrapping_add(0x52c))),
        }
    }

    fn frame_step(&mut self) -> f32 {
        frame_step(self.memory)
    }

    fn mechanism_probe(
        &mut self,
        actor: crate::navigation_scoring::ActorId,
        mechanism: crate::navigation_scoring::ObjectId,
    ) -> bool {
        let extension = self.memory.read32(mechanism.0.wrapping_add(0x7c));
        let record = extension.wrapping_add(0x568);
        let key = self.service(0x1596f0, &[actor.0, record.wrapping_add(0x48), 0]);
        self.service(0x177fb0, &[mechanism.0, key]) != 0
    }

    fn classification(&mut self, actor: crate::navigation_scoring::ActorId) -> u32 {
        self.service(0x17a0a0, &[actor.0.wrapping_add(0x7c)])
    }

    fn choose_state_eight(
        &mut self,
        actor: crate::navigation_scoring::ActorId,
        mechanism: crate::navigation_scoring::ObjectId,
    ) -> bool {
        let record_extension = self.memory.read32(mechanism.0.wrapping_add(0x7c));
        self.memory.write32(
            record_extension.wrapping_add(0x568 + 0x394),
            8.0f32.to_bits(),
        );
        self.service(0x29fe20, &[actor.0]) != 0
    }

    fn mechanism_count(&mut self, mechanism: crate::navigation_scoring::ObjectId) -> i32 {
        self.service(0x17a200, &[mechanism.0]) as i32
    }

    fn project_position(&mut self, _position: Position3) -> (NodeId, i16) {
        let node = self.service(0x9bcf0, &[self.actor.wrapping_add(0x64), 0]);
        self.memory
            .write32(self.extension.wrapping_add(0x678), node);
        let record = self.service(0x9a420, &[node]);
        let cell = self.memory.read16(record.wrapping_add(0xa)) as i16;
        let cell_address = self.actor.wrapping_add(0x34);
        self.memory.write8(cell_address, cell as u8);
        self.memory
            .write8(cell_address.wrapping_add(1), ((cell as u16) >> 8) as u8);
        (NodeId(node), cell)
    }

    fn controls_same_actor_extension(
        &mut self,
        _actor: crate::navigation_scoring::ActorId,
    ) -> bool {
        let saved = *self.regs;
        self.regs.edi = self.actor;
        self.guest.call(0x277fe0, self.regs);
        let same = self.regs.eax == self.extension;
        *self.regs = saved;
        same
    }

    fn queue_behavior(
        &mut self,
        _actor: crate::navigation_scoring::ActorId,
        request: crate::rea_mechanism::BehaviorRequest,
    ) {
        if request.mask == 4 {
            self.memory.write32(self.extension.wrapping_add(4), 0);
        }
        self.service(0x17570, &[self.extension, request.mask, 0]);
    }

    fn refresh_actor(&mut self, actor: crate::navigation_scoring::ActorId) {
        let saved = *self.regs;
        call_cdecl(self.guest, self.memory, self.regs, 0x15f0d0, &[actor.0]);
        *self.regs = saved;
    }
}

/// `1.0 / frame_rate * delta`, the per-frame step shared by the prologue and state 4.
fn frame_step<M: GuestMemory>(memory: &mut M) -> f32 {
    let frame_rate = (memory.read32(0x4e8cec) as i32) as f32;
    f32::from_bits(memory.read32(0x475c78)) / frame_rate * f32::from_bits(memory.read32(0x4e794c))
}

fn run_mechanism_tick<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    let actor = memory.read32(regs.esp.wrapping_add(4));
    let extension = memory.read32(actor.wrapping_add(0x7c));
    let current_state = memory.read32(extension.wrapping_add(0x20));
    let pending_state = memory.read32(extension.wrapping_add(0x9d0));
    let effective_state = if (pending_state as i32) >= 0 && pending_state != current_state {
        pending_state
    } else {
        current_state
    };
    if !matches!(effective_state, 0..=14) {
        return false;
    }

    // Keep the common original tick prologue ahead of the selected Rust state
    // owner. Unsupported local states decline before any mutation so the full
    // lifted controller can still run its own prologue exactly once.
    call_preserving_registers(memory, guest, regs, 0x15e120, &[extension, 1]);
    tsfp_ai_selector_transition::commit_guest_selector(memory, extension);
    if (memory.read8(extension.wrapping_add(0x2c)) as i8) >= 0 {
        memory.write32(extension.wrapping_add(0xa90), 0);
    } else {
        let elapsed = f32::from_bits(memory.read32(extension.wrapping_add(0xa90)));
        let step = frame_step(memory);
        memory.write32(extension.wrapping_add(0xa90), (step + elapsed).to_bits());
    }
    call_preserving_registers(memory, guest, regs, 0x15d470, &[actor]);

    let local_before = memory.read32(extension.wrapping_add(0x20));
    let previous_before = memory.read32(extension.wrapping_add(0x9d4));
    let pending_before = memory.read32(extension.wrapping_add(0x9d0));
    let elapsed_before = memory.read32(extension.wrapping_add(0x9d8));
    let handle_address = extension.wrapping_add(0xc08);
    let raw_handle = memory.read32(handle_address);
    let mut pool = crate::rea_mechanism::MechanismPool::default();
    if raw_handle != 0 {
        let handle = crate::rea_mechanism::MechanismHandle(raw_handle);
        let slot = handle.slot();
        let pool_base = memory.read32(0x72f5e8);
        let object = pool_base.wrapping_add((slot as u32).wrapping_mul(0x2e8));
        let record_extension = memory.read32(object.wrapping_add(0x7c));
        let record = record_extension.wrapping_add(0x568);
        pool.slots.resize_with(slot + 1, || None);
        pool.slots[slot] = Some(crate::rea_mechanism::MechanismRecord {
            identity: memory.read32(object.wrapping_add(0x2c)),
            object: crate::navigation_scoring::ObjectId(object),
            speed: f32::from_bits(memory.read32(record.wrapping_add(0x394))),
            flags: memory.read32(record.wrapping_add(0x390)),
        });
    }

    let target_before = memory.read32(extension.wrapping_add(4));
    let timer_before = memory.read32(extension.wrapping_add(0x9e0));
    let flags_before = pool
        .slots
        .iter()
        .flatten()
        .next()
        .map_or(0, |record| record.flags);
    let mut state = crate::rea_mechanism::ActorInteractionState {
        local_state: memory.read32(extension.wrapping_add(0x20)),
        previous_state: memory.read32(extension.wrapping_add(0x9d4)),
        pending_state: match memory.read32(extension.wrapping_add(0x9d0)) {
            u32::MAX => None,
            value => Some(value),
        },
        elapsed: f32::from_bits(memory.read32(extension.wrapping_add(0x9d8))),
        timer: f32::from_bits(memory.read32(extension.wrapping_add(0x9e0))),
        mechanism: (raw_handle != 0).then_some(crate::rea_mechanism::MechanismHandle(raw_handle)),
        target: match memory.read32(extension.wrapping_add(4)) {
            0 => None,
            value => Some(crate::navigation_scoring::ObjectId(value)),
        },
        current_node: Some(NodeId(memory.read32(extension.wrapping_add(0x678)))),
        cell: Some(memory.read16(actor.wrapping_add(0x34)) as i16),
        behavior_request: None,
    };
    let position = Position3 {
        x: f32::from_bits(memory.read32(actor.wrapping_add(0x64))),
        y: f32::from_bits(memory.read32(actor.wrapping_add(0x68))),
        z: f32::from_bits(memory.read32(actor.wrapping_add(0x6c))),
    };
    let mut world = RuntimeMechanismWorld {
        memory,
        guest,
        regs,
        actor,
        extension,
    };
    crate::rea_mechanism::tick_selected_state(
        crate::navigation_scoring::ActorId(actor),
        position,
        &mut state,
        &mut pool,
        &mut world,
    );

    if state.local_state != local_before {
        world
            .memory
            .write32(extension.wrapping_add(0x20), state.local_state);
        world
            .memory
            .write32(extension.wrapping_add(0x9d4), state.previous_state);
        world.memory.write32(
            extension.wrapping_add(0x9d0),
            state.pending_state.unwrap_or(u32::MAX),
        );
        world
            .memory
            .write32(extension.wrapping_add(0x9d8), state.elapsed.to_bits());
    } else if state.previous_state != previous_before
        || state.pending_state.unwrap_or(u32::MAX) != pending_before
        || state.elapsed.to_bits() != elapsed_before
    {
        world
            .memory
            .write32(extension.wrapping_add(0x9d4), state.previous_state);
        world.memory.write32(
            extension.wrapping_add(0x9d0),
            state.pending_state.unwrap_or(u32::MAX),
        );
        world
            .memory
            .write32(extension.wrapping_add(0x9d8), state.elapsed.to_bits());
    }
    if state.timer.to_bits() != timer_before {
        world
            .memory
            .write32(extension.wrapping_add(0x9e0), state.timer.to_bits());
    }
    if raw_handle != 0 && state.mechanism.is_none() {
        world.memory.write32(handle_address, 0);
    }
    let target_after = state.target.map_or(0, |value| value.0);
    if target_after != target_before {
        world
            .memory
            .write32(extension.wrapping_add(4), target_after);
    }
    if let Some(record) = pool.slots.iter().flatten().next() {
        if record.flags != flags_before {
            let record_extension = world.memory.read32(record.object.0.wrapping_add(0x7c));
            world
                .memory
                .write32(record_extension.wrapping_add(0x568 + 0x390), record.flags);
        }
    }
    true
}

const POINT_PRIMARY_TABLE: u32 = 0x715f2c;
const POINT_SECONDARY_TABLE: u32 = 0x715f34;
const POINT_TERTIARY_TABLE: u32 = 0x715f38;
const POINT_PRIMARY_COUNT: u32 = 0x7b6664;
const POINT_SECONDARY_COUNT: u32 = 0x7b65c0;
const LEVEL_WORD: u32 = 0x0079_094c;

/// 0x9CC40 cdecl(key): record id for `key` or -1. Runs the shared `point_tables` search over the
/// guest tables; the secondary-table fallthrough (0x9A3C0) is searched natively.
fn run_key_to_record_id<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) -> bool {
    let key = memory.read32(regs.esp.wrapping_add(4)) as i32;
    let primary = memory.read32(POINT_PRIMARY_TABLE);
    let secondary = memory.read32(POINT_SECONDARY_TABLE);
    let primary_count = memory.read32(POINT_PRIMARY_COUNT) as i32;
    let secondary_count = memory.read32(POINT_SECONDARY_COUNT) as i32;
    let found = crate::point_tables::key_to_record_id(
        key,
        primary_count,
        |index| memory.read32(crate::point_tables::record_address(primary, index as u32)) as i32,
        secondary_count,
        |index| memory.read32(crate::point_tables::record_address(secondary, index as u32)) as i32,
    );
    regs.eax = found.map_or(u32::MAX, |id| id as u32);
    true
}

/// 0x59D20 cdecl(owner, wanted): true when either slot of `*(owner+4)` equals `wanted`.
fn run_owner_has_either_slot<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) -> bool {
    let owner = memory.read32(regs.esp.wrapping_add(4));
    let wanted = memory.read32(regs.esp.wrapping_add(8));
    let record = memory.read32(owner.wrapping_add(4));
    regs.eax = u32::from(crate::point_tables::owner_has_either_slot(
        memory.read32(record.wrapping_add(0xd4)),
        memory.read32(record.wrapping_add(0x12c)),
        wanted,
    ));
    true
}

/// 0x9A420 cdecl(id): pointer to the id's point record when 0x9A330 accepts it, else 0.
/// Id -1 selects no record; the original's 0x9A330(0) call is a pure null reject, so it is skipped.
fn run_select_valid_record<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    use crate::point_tables::{RecordSlot, record_address, select_record};
    let id = memory.read32(regs.esp.wrapping_add(4)) as i32;
    let level = memory.read32(LEVEL_WORD);
    let Some(slot) = select_record(id, level) else {
        regs.eax = 0;
        return true;
    };
    let record = match slot {
        RecordSlot::Primary(index) => record_address(memory.read32(POINT_PRIMARY_TABLE), index),
        RecordSlot::Secondary(index) => record_address(memory.read32(POINT_SECONDARY_TABLE), index),
        RecordSlot::Tertiary(index) => record_address(memory.read32(POINT_TERTIARY_TABLE), index),
    };
    let saved = *regs;
    call_cdecl(guest, memory, regs, 0x9a330, &[record]);
    let valid = regs.eax != 0;
    *regs = saved;
    regs.eax = if valid { record } else { 0 };
    true
}

fn run_route_peek_kind<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) -> bool {
    let actor = memory.read32(regs.esp.wrapping_add(4));
    let route = memory.read32(regs.esp.wrapping_add(8));
    let extension = memory.read32(actor.wrapping_add(0x7c));
    let cursor = behavior_route_cursor(memory, extension);
    let next = route.wrapping_add(cursor.wrapping_add(1).wrapping_mul(12));
    let selected = crate::rea_route_peek::peek_next_index(
        cursor,
        memory.read32(next),
        memory.read32(next.wrapping_add(4)),
    );
    regs.eax = memory.read32(route.wrapping_add(selected.wrapping_mul(12)));
    true
}

struct RuntimeDirections<'a, M, G> {
    memory: &'a mut M,
    guest: &'a mut G,
    regs: &'a mut ExtRegisters,
    frame_esp: u32,
    delegated: Option<bool>,
}

impl<M: GuestMemory, G: GuestCall> DirectionProvider for RuntimeDirections<'_, M, G> {
    fn has_delegated_direction(&mut self, selector: i16) -> bool {
        if let Some(admitted) = self.delegated.take() {
            return admitted;
        }
        let saved = *self.regs;
        self.regs.esp = self.frame_esp;
        call_cdecl(
            self.guest,
            self.memory,
            self.regs,
            0xd6ea0,
            &[selector as i32 as u32],
        );
        let admitted = self.regs.eax != 0;
        *self.regs = saved;
        admitted
    }

    fn delegated_direction(&mut self, actor: crate::rea_ai_types::ActorId, _selector: i16) -> Vec3 {
        let saved = *self.regs;
        self.regs.esp = self.frame_esp;
        let output = self.frame_esp.wrapping_add(0x20);
        call_cdecl(
            self.guest,
            self.memory,
            self.regs,
            0x3797b0,
            &[actor.0, output],
        );
        let direction = Vec3 {
            x: f32::from_bits(self.memory.read32(output)),
            y: f32::from_bits(self.memory.read32(output.wrapping_add(4))),
            z: f32::from_bits(self.memory.read32(output.wrapping_add(8))),
        };
        *self.regs = saved;
        direction
    }

    fn matrix_direction_xz(
        &mut self,
        _actor: crate::rea_ai_types::ActorId,
        _route: crate::perception_geometry::MatrixDirectionRoute,
        _origin: Vec3,
        _yaw_radians: Option<f32>,
    ) -> crate::perception_geometry::Vec2 {
        unreachable!("runtime adapter declines matrix-direction branches")
    }

    fn yaw_table_value(&mut self, index: u16) -> f32 {
        f32::from_bits(
            self.memory
                .read32(0x7a38a0u32.wrapping_add(u32::from(index) * 4)),
        )
    }
}

fn run_yaw_table_perception<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    let actor = memory.read32(regs.esp.wrapping_add(4));
    let target = memory.read32(regs.esp.wrapping_add(8));
    let extension = memory.read32(actor.wrapping_add(0x7c));
    let selector = memory.read16(extension.wrapping_add(0x4a)) as i16;
    let kind = memory.read16(actor.wrapping_add(0x94));
    let flags = memory.read16(actor.wrapping_add(0x10));
    // Preserve the original direction-source check. The readable owner uses
    // the lifted helpers only for opaque vector construction; its geometry and
    // ordered predicate remain the shared Rust implementation.
    let original = *regs;
    let frame_esp = original.esp.wrapping_sub(0x100);
    regs.esp = frame_esp;
    call_cdecl(guest, memory, regs, 0xd6ea0, &[selector as i32 as u32]);
    let delegated = regs.eax != 0;
    *regs = original;
    if !delegated && (selector != 0 || kind == 0x607 || flags != 0) {
        return false;
    }

    let height_index = memory.read16(extension.wrapping_add(0x48)) as i16;
    let height_address =
        0x53a3e4u32.wrapping_add((i32::from(height_index).wrapping_mul(0x30)) as u32);
    let actor_position = Vec3 {
        x: f32::from_bits(memory.read32(actor.wrapping_add(0x64))),
        y: f32::from_bits(memory.read32(actor.wrapping_add(0x68))),
        z: f32::from_bits(memory.read32(actor.wrapping_add(0x6c))),
    };
    let query = AngularAdmissionQuery {
        actor: ActorGeometry {
            id: crate::rea_ai_types::ActorId(actor),
            position: actor_position,
            yaw: f32::from_bits(memory.read32(actor.wrapping_add(0x70))),
            kind,
            flags: memory.read32(actor.wrapping_add(0x10)),
            direction_selector: selector,
            vertical_extent: f32::from_bits(memory.read32(height_address)),
        },
        target: Vec3 {
            x: f32::from_bits(memory.read32(target)),
            y: f32::from_bits(memory.read32(target.wrapping_add(4))),
            z: f32::from_bits(memory.read32(target.wrapping_add(8))),
        },
        threshold: f32::from_bits(memory.read32(regs.esp.wrapping_add(12))),
        x87_control: regs.fpu.control,
    };
    let admitted = angular_admitted(
        query,
        &mut RuntimeDirections {
            memory,
            guest,
            regs,
            frame_esp,
            delegated: Some(delegated),
        },
    );
    regs.eax = u32::from(admitted);
    true
}

fn run_navigation_scorer<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) {
    let actor_object = memory.read32(regs.esp.wrapping_add(4));
    let extension = memory.read32(actor_object.wrapping_add(0x7c));
    let target_object = memory.read32(extension.wrapping_add(4));
    let current_node = NodeId(memory.read32(extension.wrapping_add(0x678)));
    let actor = ScoringActor {
        extension: ActorId(extension),
        object: ObjectId(actor_object),
        target_object: (target_object != 0).then_some(ObjectId(target_object)),
        current_node,
        position: NavPosition {
            x: f32::from_bits(memory.read32(actor_object.wrapping_add(0x64))),
            z: f32::from_bits(memory.read32(actor_object.wrapping_add(0x6c))),
        },
        target_position: (target_object != 0).then(|| NavPosition {
            x: f32::from_bits(memory.read32(target_object.wrapping_add(0x64))),
            z: f32::from_bits(memory.read32(target_object.wrapping_add(0x6c))),
        }),
    };

    if target_object == 0 {
        regs.eax = select_best_goal(
            actor,
            ScoringWorld {
                groups: &[],
                nodes: &[],
                other_actors: &[],
            },
        )
        .map_or(u32::MAX, |node| node.0);
        return;
    }

    let config = regs.ecx;
    let mut group_nodes = Vec::<Vec<NodeId>>::new();
    for slot in 0..4u32 {
        let group_id = memory.read32(config.wrapping_add(0x14 + slot * 4)) as i32;
        if slot != 0 && group_id < 0 {
            break;
        }
        let header = memory.read32(0x7b6770u32.wrapping_add((group_id as u32).wrapping_mul(4)));
        let count = memory.read32(header) as i32;
        let mut candidates = Vec::new();
        if count > 0 {
            for index in 0..count as u32 {
                candidates.push(NodeId(
                    memory.read32(header.wrapping_add(24).wrapping_add(index.wrapping_mul(4))),
                ));
            }
        }
        group_nodes.push(candidates);
    }

    let other_count = memory.read32(0x74c37c) as i32;
    let mut other_actors = Vec::new();
    if other_count > 0 {
        for index in 0..other_count as u32 {
            let extension = memory.read32(0x7a3580u32.wrapping_add(index.wrapping_mul(4)));
            let goal = memory.read32(extension.wrapping_add(0x684)) as i32;
            other_actors.push(OtherActorGoal {
                extension: ActorId(extension),
                object: ObjectId(memory.read32(extension)),
                flags: memory.read8(extension.wrapping_add(0x24)),
                goal: (goal >= 0).then_some(NodeId(goal as u32)),
            });
        }
    }

    let mut nodes = Vec::<NavNode>::new();
    let other_goals = other_actors.iter().filter_map(|other| {
        (other.extension != actor.extension
            && other.object != ObjectId(target_object)
            && other.flags & 8 == 0)
            .then_some(other.goal)
            .flatten()
    });
    for id in group_nodes
        .iter()
        .flat_map(|group| group.iter().copied())
        .filter(|id| *id != current_node)
        .chain(other_goals)
    {
        if nodes.iter().any(|node| node.id == id) {
            continue;
        }
        let saved = *regs;
        regs.esp = saved.esp.wrapping_sub(0x100);
        call_cdecl(guest, memory, regs, 0x9a420, &[id.0]);
        let record = regs.eax;
        let position = NavPosition {
            x: f32::from_bits(memory.read32(record.wrapping_add(0x0c))),
            z: f32::from_bits(memory.read32(record.wrapping_add(0x14))),
        };
        *regs = saved;
        nodes.push(NavNode { id, position });
    }

    let groups = group_nodes
        .iter()
        .map(|node_ids| CandidateGroup { node_ids })
        .collect::<Vec<_>>();
    regs.eax = select_best_goal(
        actor,
        ScoringWorld {
            groups: &groups,
            nodes: &nodes,
            other_actors: &other_actors,
        },
    )
    .map_or(u32::MAX, |node| node.0);
}

const GROUP_TABLE: u32 = 0x007a_2d88;
const GROUP_STRIDE: u32 = 0x60;
const GROUP_HEADER_TIMER: u32 = 0x4170_0000;

/// Refresh one generation-owned recipient at the group-alert seam, then apply
/// an ordered native mutation. Guest reads/writes remain a TEMPORARY-SHIM for
/// fields whose other producers still live in lifted code.
fn update_bound_awareness_recipient<M: GuestMemory>(
    memory: &M,
    extension: u32,
    update: impl FnOnce(&mut AwarenessRecipientState),
) -> Option<AwarenessRecipientState> {
    if extension == 0 {
        return None;
    }
    let descriptor = memory.read32(extension.wrapping_add(0xbf0));
    let mut snapshot = AwarenessRecipientState {
        actor: crate::rea_ai_types::ActorId(memory.read32(extension.wrapping_add(8))),
        actor_flags: memory.read32(extension.wrapping_add(0x10)),
        local_flags: memory.read32(extension.wrapping_add(0x28)),
        listen_flags: memory.read32(extension.wrapping_add(0x14)),
        behavior_flags: memory.read32(descriptor.wrapping_add(0x54)),
        class_selector: memory.read16(extension.wrapping_add(0x4a)) as i16,
        group_mask: crate::rea_ai_types::GroupMask(memory.read32(extension.wrapping_add(0xbec))),
        alert_target: None,
    };

    let mut live = LIVE_ACTORS.lock();
    let identity = live
        .bindings
        .iter()
        .find(|(bound, _)| *bound == extension)
        .map(|(_, identity)| *identity)?;
    snapshot.alert_target = live
        .store
        .awareness_recipient(identity)
        .ok()
        .flatten()
        .and_then(|recipient| recipient.alert_target);
    live.store
        .set_awareness_recipient(identity, snapshot)
        .ok()?;
    live.store
        .with_awareness_recipient_mut(identity, update)
        .ok()
        .flatten()?;
    live.store.awareness_recipient(identity).ok().flatten()
}

struct RuntimeGroupAlert<'a, M, G> {
    memory: &'a mut M,
    guest: &'a mut G,
    regs: &'a mut ExtRegisters,
}

impl<M: GuestMemory, G: GuestCall> RuntimeGroupAlert<'_, M, G> {
    fn group(group: u32) -> u32 {
        GROUP_TABLE.wrapping_add(group.wrapping_mul(GROUP_STRIDE))
    }
    fn extension(&self, group: u32, index: usize) -> u32 {
        let list = Self::group(group).wrapping_add(8);
        let object = self.memory.read32(list.wrapping_add(index as u32 * 4));
        self.memory.read32(object.wrapping_add(0x7c))
    }
    fn service(&mut self, va: u32, args: &[u32]) -> u32 {
        let saved = *self.regs;
        call_cdecl(self.guest, self.memory, self.regs, va, args);
        let result = self.regs.eax;
        *self.regs = saved;
        result
    }
}

impl<M: GuestMemory, G: GuestCall> crate::rea_group_alert::GroupAlertWorld
    for RuntimeGroupAlert<'_, M, G>
{
    fn member_count(&self, group: u32) -> usize {
        let count = self.memory.read32(Self::group(group).wrapping_sub(8)) as i32;
        usize::try_from(count.max(0)).unwrap_or(0)
    }
    fn mark_group_alerted(&mut self, group: u32) {
        let base = Self::group(group);
        self.memory.write32(base.wrapping_sub(4), 1);
        self.memory.write32(base, 1);
        self.memory
            .write32(base.wrapping_add(4), GROUP_HEADER_TIMER);
    }
    fn member(&self, group: u32, index: usize) -> crate::rea_group_alert::MemberView {
        let e = self.extension(group, index);
        if let Some(recipient) = update_bound_awareness_recipient(self.memory, e, |_| {}) {
            return crate::rea_group_alert::MemberView {
                id: recipient.actor.0,
                actor_flags: recipient.actor_flags,
                local_flags: recipient.local_flags,
                listen_flags: recipient.listen_flags,
                behavior_flags: recipient.behavior_flags,
                group_mask: recipient.group_mask.0,
                class_selector: recipient.class_selector,
            };
        }
        let behavior = self.memory.read32(e.wrapping_add(0xbf0));
        crate::rea_group_alert::MemberView {
            id: self.memory.read32(e.wrapping_add(8)),
            actor_flags: self.memory.read32(e.wrapping_add(0x10)),
            local_flags: self.memory.read32(e.wrapping_add(0x28)),
            listen_flags: self.memory.read32(e.wrapping_add(0x14)),
            behavior_flags: self.memory.read32(behavior.wrapping_add(0x54)),
            group_mask: self.memory.read32(e.wrapping_add(0xbec)),
            class_selector: self.memory.read16(e.wrapping_add(0x4a)) as i16,
        }
    }
    fn linked_object_node(&self, group: u32, index: usize) -> u32 {
        let e = self.extension(group, index);
        linked_object_node(self.memory, self.memory.read32(e.wrapping_add(4)))
    }
    fn set_member_target(&mut self, group: u32, index: usize, node: u32) {
        let e = self.extension(group, index);
        let _ = update_bound_awareness_recipient(self.memory, e, |recipient| {
            recipient.alert_target = Some(NodeId(node));
        });
        // TEMPORARY-SHIM: lifted consumers still read the selected target.
        self.memory.write32(e.wrapping_add(0xac8), node);
    }
    fn set_member_local_flags(&mut self, group: u32, index: usize, flags: u32) {
        let e = self.extension(group, index);
        let _ = update_bound_awareness_recipient(self.memory, e, |recipient| {
            recipient.local_flags = flags;
        });
        // TEMPORARY-SHIM: lifted consumers still read the recipient flags.
        self.memory.write32(e.wrapping_add(0x28), flags);
    }
    fn set_member_group_mask(&mut self, group: u32, index: usize, mask: u32) {
        let e = self.extension(group, index);
        let _ = update_bound_awareness_recipient(self.memory, e, |recipient| {
            recipient.group_mask.0 = mask;
        });
        // TEMPORARY-SHIM: other group members and lifted writers read this mask.
        self.memory.write32(e.wrapping_add(0xbec), mask);
    }
    fn class_listens(&mut self, selector: i16) -> bool {
        self.service(0xd6dd0, &[i32::from(selector) as u32]) != 0
    }
    fn candidate_check(&mut self, group: u32, index: usize) -> bool {
        let e = self.extension(group, index);
        self.service(0x15e120, &[e, 0]) != 0
    }
    fn actor_busy(&mut self, group: u32, index: usize) -> bool {
        let e = self.extension(group, index);
        let actor = self.memory.read32(e);
        self.service(0x59e00, &[actor, 0x20, 0]) != 0
    }
    fn raise(&mut self, group: u32, index: usize, code: u32, three_args: bool) {
        let e = self.extension(group, index);
        if three_args {
            self.service(0x17570, &[e, code, 0]);
        } else {
            self.service(0x17570, &[e, code]);
        }
    }
    fn leave_groups(&mut self, group: u32, index: usize, groups: u32) {
        let e = self.extension(group, index);
        let actor = self.memory.read32(e);
        self.service(0x160cf0, &[actor, groups]);
    }
}

/// Node an alert source object points at: `+0x678` of its extension for kind-8
/// objects, otherwise the object's own `+0x110`.
fn linked_object_node<M: GuestMemory>(memory: &M, object: u32) -> u32 {
    if memory.read32(object.wrapping_add(0x20)) == 8 {
        let extension = memory.read32(object.wrapping_add(0x7c));
        memory.read32(extension.wrapping_add(0x678))
    } else {
        memory.read32(object.wrapping_add(0x110))
    }
}

/// Original `0x267FF0(kind, subject, node)`: resolve the alert source, then
/// sweep the groups (`rea_group_alert`). Kinds 0 and 2 first run the awareness
/// source loop (`0x25FAF0`, itself an adopted adapter).
fn run_group_alert<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    use crate::rea_group_alert::{TargetChoice, alert_group_members};
    let kind = memory.read32(regs.esp.wrapping_add(4));
    let subject = memory.read32(regs.esp.wrapping_add(8));
    let extra = memory.read32(regs.esp.wrapping_add(12));
    if memory.read8(GAME_MODE_BYTE) != 10 || memory.read32(0x007d_e458) & 0x4000_0000 != 0 {
        return true;
    }
    let original = *regs;
    let (mask, excluded, target) = match kind {
        0 => {
            let extension = memory.read32(subject.wrapping_add(0x7c));
            let behavior = memory.read32(extension.wrapping_add(0xbf0));
            if memory.read8(behavior.wrapping_add(0x54)) & 1 != 0 {
                return true;
            }
            let mask = memory.read32(extension.wrapping_add(0xbec));
            let excluded = memory.read32(extension.wrapping_add(8));
            call_preserving_registers(memory, guest, regs, 0x25faf0, &[mask]);
            (mask, excluded, TargetChoice::Fixed(extra))
        }
        1 => (
            memory.read32(subject.wrapping_add(4)),
            u32::MAX,
            TargetChoice::FromMemberObject,
        ),
        2 => {
            let mask = memory.read32(subject.wrapping_add(0xc));
            let key = memory.read32(subject.wrapping_add(0x10));
            // The original resolves the key before the source loop; the node
            // itself is unused because members take their own object's node.
            call_preserving_registers(memory, guest, regs, 0x9cc40, &[key]);
            call_preserving_registers(memory, guest, regs, 0x25faf0, &[mask]);
            (mask, u32::MAX, TargetChoice::FromMemberObject)
        }
        _ => (u32::MAX, u32::MAX, TargetChoice::FromMemberObject),
    };
    let mut world = RuntimeGroupAlert {
        memory,
        guest,
        regs,
    };
    alert_group_members(&mut world, mask, excluded, target);
    *regs = original;
    true
}

/// Nav link advance world over guest memory (`rea_nav_link`). The actor
/// extension is `object + 0x7c`; node tables are read through `0x715F2C`.
struct RuntimeNavLink<'a, M, G> {
    memory: &'a mut M,
    guest: &'a mut G,
    regs: &'a mut ExtRegisters,
    object: u32,
}

impl<M: GuestMemory, G: GuestCall> RuntimeNavLink<'_, M, G> {
    fn ext(&self, offset: u32) -> u32 {
        self.memory
            .read32(self.object.wrapping_add(0x7c))
            .wrapping_add(offset)
    }
    fn read(&self, offset: u32) -> u32 {
        self.memory.read32(self.ext(offset))
    }
    fn write(&mut self, offset: u32, value: u32) {
        let address = self.ext(offset);
        self.memory.write32(address, value);
    }
    fn node_record(&self, node: i32) -> u32 {
        self.memory
            .read32(0x715f2c)
            .wrapping_add((node as u32).wrapping_shl(5))
    }
    fn position_at(&self, base: u32) -> [u32; 3] {
        [0xc, 0x10, 0x14].map(|offset| self.memory.read32(base.wrapping_add(offset)))
    }
    fn service(&mut self, va: u32, args: &[u32]) -> u32 {
        let saved = *self.regs;
        call_cdecl(self.guest, self.memory, self.regs, va, args);
        let result = self.regs.eax;
        *self.regs = saved;
        result
    }
}

impl<M: GuestMemory, G: GuestCall> crate::rea_nav_link::NavLinkWorld for RuntimeNavLink<'_, M, G> {
    fn constants(&self) -> crate::rea_nav_link::NavLinkConstants {
        crate::rea_nav_link::NavLinkConstants {
            hold_radius_tight: f32::from_bits(self.memory.read32(0x475d24)),
            hold_radius_loose: f32::from_bits(self.memory.read32(0x47d568)),
            arrive_distance_sq: f32::from_bits(self.memory.read32(0x47dd90)),
        }
    }
    fn clear_local_flags(&mut self, mask: u32) {
        let flags = self.read(0x28);
        self.write(0x28, flags & !mask);
    }
    fn local_flags_2c(&self) -> u32 {
        self.read(0x2c)
    }
    fn held_record(&self) -> u32 {
        self.read(0xacc)
    }
    fn set_held_record(&mut self, record: u32) {
        self.write(0xacc, record);
    }
    fn goal(&self) -> i32 {
        self.read(0x684) as i32
    }
    fn set_goal(&mut self, goal: i32) {
        self.write(0x684, goal as u32);
    }
    fn pending(&self) -> (u32, u32) {
        (self.read(0x9a8), self.read(0x9ac))
    }
    fn set_pending(&mut self, first: u32, second: u32) {
        self.write(0x9ac, second);
        self.write(0x9a8, first);
    }
    fn current(&self) -> i32 {
        self.read(0x678) as i32
    }
    fn set_current(&mut self, node: i32) {
        self.write(0x678, node as u32);
    }
    fn link(&self) -> i32 {
        self.read(0x680) as i32
    }
    fn set_link(&mut self, link: i32) {
        self.write(0x680, link as u32);
    }
    fn hold_distance_sq(&self) -> f32 {
        f32::from_bits(self.read(0x9b0))
    }
    fn set_target(&mut self, position: [u32; 3]) {
        for (offset, bits) in [0x30, 0x34, 0x38].into_iter().zip(position) {
            self.write(offset, bits);
        }
    }
    fn actor_position(&self) -> [f32; 3] {
        [0x64, 0x68, 0x6c]
            .map(|offset| f32::from_bits(self.memory.read32(self.object.wrapping_add(offset))))
    }
    fn enter_follow_state(&mut self) {
        let state = self.read(0x24);
        self.write(0x24, state & 0xffff_ffe9 | 1);
        let value = self.read(0xb14);
        self.write(0xad0, value);
    }
    fn decrement_pending_first(&mut self) {
        let value = self.read(0x9a8).wrapping_sub(1);
        self.write(0x9a8, value);
    }
    fn node_position(&self, node: i32) -> [u32; 3] {
        self.position_at(self.node_record(node))
    }
    fn node_key(&self, node: i32) -> i32 {
        self.memory.read32(self.node_record(node)) as i32
    }
    fn record_key(&self, record: u32) -> i16 {
        self.memory.read16(record.wrapping_add(8)) as i16
    }
    fn record_position(&self, record: u32) -> [u32; 3] {
        self.position_at(record)
    }
    fn player_node(&self) -> i32 {
        let player = self.memory.read32(0x7356d8);
        self.memory.read32(player.wrapping_add(0x678)) as i32
    }
    fn first_actor_node(&self) -> i32 {
        let list = self.memory.read32(0x7b0c48);
        let object = self.memory.read32(list.wrapping_add(0x14));
        let extension = self.memory.read32(object.wrapping_add(0x7c));
        self.memory.read32(extension.wrapping_add(0x678)) as i32
    }
    fn find_record(&mut self, id: i32) -> u32 {
        self.service(0x9a420, &[id as u32])
    }
    fn resolve_key(&mut self, key: i16) -> i32 {
        self.service(0x9cc40, &[i32::from(key) as u32]) as i32
    }
    fn trace_link(&mut self, trace: bool) -> i32 {
        let (object, current) = (self.object, self.ext(0x678));
        self.service(0x1596f0, &[object, current, u32::from(trace)]) as i32
    }
    fn set_trace_gate(&mut self, enabled: bool) {
        self.service(0x160b00, &[u32::from(enabled)]);
    }
    fn abort_link(&mut self) {
        let object = self.object;
        self.service(0x15d490, &[object]);
    }
    fn refresh_target(&mut self) {
        let object = self.object;
        self.service(0x15a050, &[object]);
    }
    fn node_entry_id(&mut self, node: i32) -> i32 {
        self.service(0x9a4c0, &[node as u32]) as i32
    }
    fn commit_link(&mut self, from: i32, link: i32) -> bool {
        let object = self.object;
        self.service(0x15a180, &[object, from as u32, link as u32]) != 0
    }
    fn finish_step(&mut self) {
        let object = self.object;
        self.service(0x15be70, &[object]);
    }
}

/// Spawn register world of `0xA0640` over guest memory.
struct RuntimeSpawnRegister<'a, M, G> {
    memory: &'a mut M,
    guest: &'a mut G,
    regs: &'a mut ExtRegisters,
}

impl<M: GuestMemory, G: GuestCall> RuntimeSpawnRegister<'_, M, G> {
    fn service(&mut self, va: u32, args: &[u32]) -> u32 {
        let saved = *self.regs;
        call_cdecl(self.guest, self.memory, self.regs, va, args);
        let result = self.regs.eax;
        *self.regs = saved;
        result
    }
}

impl<M: GuestMemory, G: GuestCall> crate::rea_awareness_leaves::SpawnRegisterWorld
    for RuntimeSpawnRegister<'_, M, G>
{
    fn level_record(&mut self) -> u32 {
        let level = crate::state_wire::get_level(self.memory);
        self.service(0x6a850, &[level])
    }
    fn entry_table(&self, record: u32) -> u32 {
        self.memory.read32(record.wrapping_add(0xbc))
    }
    fn entry_count(&self, record: u32) -> i32 {
        self.memory.read32(record.wrapping_add(0xc0)) as i32
    }
    fn read32(&self, address: u32) -> u32 {
        self.memory.read32(address)
    }
    fn counters(&self) -> u32 {
        self.memory.read32(0x72f5ec)
    }
    fn group_index_of(&mut self, key: u32) -> u32 {
        self.service(0x25fa10, &[key])
    }
    fn register_entry(&mut self, entry: u32) {
        self.service(0xa05c0, &[entry]);
    }
}

/// Original `0xA0640(index)` cdecl, no result.
fn run_register_spawn_entries<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    let index = memory.read32(regs.esp.wrapping_add(4));
    let original = *regs;
    let mut world = RuntimeSpawnRegister {
        memory,
        guest,
        regs,
    };
    crate::rea_awareness_leaves::register_spawn_entries(&mut world, index);
    *regs = original;
    true
}

/// Original `0x3797B0(actor, out)` cdecl, writes three floats to `out`.
fn run_view_direction<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) -> bool {
    let actor = memory.read32(regs.esp.wrapping_add(4));
    let out = memory.read32(regs.esp.wrapping_add(8));
    let source = memory.read32(actor.wrapping_add(0x7c)).wrapping_add(0xaa0);
    let source = memory.read32(source);
    let view = (source != 0).then(|| crate::rea_awareness_leaves::ViewAngles {
        pitch: f32::from_bits(memory.read32(source.wrapping_add(0x20))),
        yaw_offset: f32::from_bits(memory.read32(source.wrapping_add(0x24))),
    });
    let yaw = f32::from_bits(memory.read32(actor.wrapping_add(0x70)));
    let direction = crate::rea_awareness_leaves::view_direction(yaw, view, |index| {
        memory.read32(0x7a38a0u32.wrapping_add((index as u32) * 4))
    });
    for (slot, bits) in direction.into_iter().enumerate() {
        memory.write32(out.wrapping_add(slot as u32 * 4), bits);
    }
    true
}

/// Read-only table view over guest memory for the character leaves.
struct GuestTables<'a, M>(&'a M);

impl<M: GuestMemory> crate::rea_character_leaves::TableView for GuestTables<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn read16(&self, address: u32) -> u16 {
        self.0.read16(address)
    }
    fn read32(&self, address: u32) -> u32 {
        self.0.read32(address)
    }
}

/// Character and spawn table leaves (cdecl, result in EAX; the originals also clobber ECX/EDX,
/// which callers do not rely on, so only EAX is written).
fn run_character_leaf<M: GuestMemory>(va: u32, memory: &M, regs: &ExtRegisters) -> Option<u32> {
    use crate::rea_character_leaves as leaves;
    let arg = |index: u32| memory.read32(regs.esp.wrapping_add(4 + 4 * index));
    let tables = GuestTables(memory);
    Some(match va {
        0x12fc0 => leaves::pair_entry_field_4(&tables, arg(0), arg(1)),
        0x1c5210 => leaves::group_index_outside(&tables, arg(0), arg(1)),
        0x1c5250 => leaves::spawn_group_count(),
        0x2aefb0 => leaves::behaviour_table_entry(&tables, arg(0), arg(1)),
        0x2b2390 => leaves::spawn_table_has_no_flagged_entry(&tables),
        0x350e70 => leaves::type_flag_mask_test(&tables, arg(0), arg(1)),
        0x12fe0 => leaves::pair_entry_field_8(&tables, arg(0), arg(1)),
        0xd6ed0 => leaves::is_ordinary_type_id(arg(0)),
        0x26cc80 => leaves::weapon_action_slot_matches(&tables, arg(0)),
        0x26cd50 => leaves::weapon_action_owner_has(&tables, arg(0)),
        _ => return None,
    })
}

/// Original `0x1B6BF0(ext, previous_entry)`: snapshot its 70 ordered candidate
/// rows, then use the readable Rust selector. Caller-side ammo mutation and shot
/// emission remain outside this leaf.
fn run_weapon_cycle<M: GuestMemory>(memory: &M, regs: &mut ExtRegisters) -> bool {
    use crate::rea_weapon_cycle::{WEAPON_CYCLE_SLOTS, WeaponCycleEntry, next_selectable};
    use crate::weapon_hands_runtime::{
        commit_cycle_selection_from_guest, next_cycle_entry_for_player,
    };
    use crate::weapon_inventory::WeaponPlayerId;

    const ORDER: u32 = 0x004f_c888;
    const ENTRY_FLAGS: u32 = 0x004f_a128;
    const OWNED: u32 = 0x54c;
    let extension = memory.read32(regs.esp.wrapping_add(4));
    let previous = memory.read32(regs.esp.wrapping_add(8));
    let table_entries: [WeaponCycleEntry; WEAPON_CYCLE_SLOTS] = core::array::from_fn(|slot| {
        let id = memory.read32(ORDER.wrapping_add((slot as u32).wrapping_mul(4)));
        WeaponCycleEntry {
            id,
            selection_flag_0x10: memory.read8(ENTRY_FLAGS.wrapping_add(id.wrapping_mul(0x3c)))
                & 0x10
                != 0,
            owned: false,
        }
    });
    let player = {
        let index = memory.read32(extension.wrapping_add(8)) as i32;
        (0..crate::weapon_hands_runtime::WEAPON_PLAYER_SLOTS as i32)
            .contains(&index)
            .then_some(WeaponPlayerId::new(index as u16))
    };
    let native_selection = player.and_then(|player| {
        next_cycle_entry_for_player(player, &table_entries, previous)
            .map(|selected| (player, selected))
    });
    let selected = native_selection.map_or_else(
        || {
            let guest_entries: [WeaponCycleEntry; WEAPON_CYCLE_SLOTS] =
                core::array::from_fn(|slot| {
                    let mut entry = table_entries[slot];
                    entry.owned =
                        memory.read8(extension.wrapping_add(OWNED).wrapping_add(entry.id)) != 0;
                    entry
                });
            next_selectable(&guest_entries, previous)
        },
        |(_, selected)| selected,
    );
    regs.eax = selected;
    if native_selection.is_none() {
        let _ = commit_cycle_selection_from_guest(memory, extension, selected);
    }
    true
}

/// Original `0x1B6BB0(group in ECX)`: return the first matching cycle entry in EAX.
fn run_weapon_group_lookup<M: GuestMemory>(memory: &M, regs: &mut ExtRegisters) -> bool {
    use crate::rea_weapon_group::{WEAPON_CYCLE_SLOTS, first_group_entry};

    const ORDER: u32 = 0x004f_c888;
    const ENTRY_GROUP: u32 = 0x004f_a160;
    let ordered_ids: [u32; WEAPON_CYCLE_SLOTS] =
        core::array::from_fn(|slot| memory.read32(ORDER.wrapping_add(slot as u32 * 4)));
    let groups_by_slot: [u32; WEAPON_CYCLE_SLOTS] = core::array::from_fn(|slot| {
        memory.read32(ENTRY_GROUP.wrapping_add(ordered_ids[slot].wrapping_mul(0x3c)))
    });
    regs.eax = first_group_entry(&ordered_ids, &groups_by_slot, regs.ecx) as u32;
    true
}

/// Original `0x1B99F0(slot, row_id)`: return row `+0x1c`, or zero on a guard miss.
fn run_weapon_subrecord<M: GuestMemory>(memory: &M, regs: &mut ExtRegisters) -> bool {
    use crate::rea_weapon_subrecord as subrecord;

    let slot = memory.read32(regs.esp.wrapping_add(4)) as i32;
    // The second caller argument is pushed by one observed caller but ignored
    // by the original; the lookup is entirely slot-driven.
    let mapped_row = if (0..subrecord::WEAPON_ROW_COUNT).contains(&slot) {
        memory.read32(
            0x004f_a138u32.wrapping_add((slot as u32).wrapping_mul(subrecord::WEAPON_ROW_STRIDE)),
        ) as i32
    } else {
        -1
    };
    let value = if (0..subrecord::AMMO_SUBRECORD_COUNT).contains(&mapped_row) {
        let base = memory.read32(0x004f_9bac);
        Some(
            memory.read32(
                base.wrapping_add((mapped_row as u32).wrapping_mul(0x29c))
                    .wrapping_add(subrecord::AMMO_SUBRECORD_OFFSET),
            ),
        )
    } else {
        None
    };
    regs.eax = subrecord::resolve_row_field(slot, mapped_row, value);
    true
}

/// Original `0x288030()`: step the 500-entry particle pool through the typed core (EAX ends at the
/// loop's final entry pointer, the original has no result).
fn run_particle_pool<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) -> bool {
    use crate::rea_particle_pool as pool;
    let base = memory.read32(pool::POOL_POINTER);
    let float = |m: &M, a: u32| f32::from_bits(m.read32(a));
    let mut entries = Vec::with_capacity(pool::ENTRY_COUNT);
    for i in 0..pool::ENTRY_COUNT as u32 {
        let e = base.wrapping_add(i * pool::ENTRY_STRIDE);
        entries.push(pool::Particle {
            position: [float(memory, e), float(memory, e + 4), float(memory, e + 8)],
            velocity: [
                float(memory, e + 12),
                float(memory, e + 16),
                float(memory, e + 20),
            ],
            age: float(memory, e + 24),
        });
    }
    let dt = pool::time_step(
        memory.read32(pool::STEP_DIVISOR) as i32,
        memory.read32(pool::STEP_FACTOR) as i32,
    );
    let live = pool::step_pool(&mut entries, dt);
    for (i, p) in entries.iter().enumerate() {
        let e = base.wrapping_add(i as u32 * pool::ENTRY_STRIDE);
        for (k, v) in p
            .position
            .iter()
            .chain(&p.velocity)
            .chain([&p.age])
            .enumerate()
        {
            memory.write32(e + 4 * k as u32, v.to_bits());
        }
    }
    memory.write32(base.wrapping_add(pool::LIVE_COUNT_OFFSET), live);
    regs.eax = base.wrapping_add(0xc + pool::ENTRY_COUNT as u32 * pool::ENTRY_STRIDE);
    true
}

/// Original `0x15D500(actor, requested_node)` cdecl, result in EAX.
fn run_nav_link<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    let object = memory.read32(regs.esp.wrapping_add(4));
    let requested = memory.read32(regs.esp.wrapping_add(8)) as i32;
    let original = *regs;
    let mut world = RuntimeNavLink {
        memory,
        guest,
        regs,
        object,
    };
    let reached = crate::rea_nav_link::advance_nav_link(&mut world, requested);
    *regs = original;
    regs.eax = u32::from(reached);
    true
}

struct RuntimeActionGuard<'a, M, G> {
    memory: &'a mut M,
    guest: &'a mut G,
    regs: &'a mut ExtRegisters,
    actor: u32,
    extension: u32,
}

impl<M: GuestMemory, G: GuestCall> crate::rea_behavior_guard::ActionGuardHost
    for RuntimeActionGuard<'_, M, G>
{
    fn status(&mut self) -> i32 {
        let saved = *self.regs;
        call_cdecl(
            self.guest,
            self.memory,
            self.regs,
            0x15fe00,
            &[self.actor, 1],
        );
        let result = self.regs.eax as i32;
        *self.regs = saved;
        result
    }

    fn timer_threshold(&mut self) -> f32 {
        f32::from_bits(self.memory.read32(0x475c78))
    }

    fn masked_action_exists(&mut self) -> bool {
        let mask = self.memory.read32(self.extension.wrapping_add(0xbec));
        let saved = *self.regs;
        call_cdecl(self.guest, self.memory, self.regs, 0x160ec0, &[mask]);
        let result = self.regs.eax != 0;
        *self.regs = saved;
        result
    }

    fn alternate_action_available(&mut self) -> bool {
        self.memory.read32(self.extension.wrapping_add(0xadc)) != 0
    }

    fn movement_suppressed(&mut self) -> bool {
        let behavior = self.memory.read32(self.extension.wrapping_add(0xbf0));
        self.memory.read32(behavior.wrapping_add(0x54)) & 0x20 != 0
    }

    fn set_mode_bit(&mut self, extension: u32, bit: u32) {
        call_cdecl(
            self.guest,
            self.memory,
            self.regs,
            0x17570,
            &[extension, bit, 0],
        );
    }

    fn play_animation(&mut self, animation: u32) {
        call_cdecl(
            self.guest,
            self.memory,
            self.regs,
            0x5f460,
            &[self.actor, animation, 0x3eaa7efa, 0x3f800000],
        );
    }

    fn target_and_animation(&mut self) -> (i32, i32, i32, u32) {
        let owner_index = self.memory.read32(self.extension.wrapping_add(0x204)) as i32;
        let owner_is_special = owner_index == -1
            || self
                .memory
                .read32(0x4f9bacu32.wrapping_add((owner_index as u32).wrapping_mul(0x29c) + 0xc))
                & 1
                != 0;
        let animation = self
            .memory
            .read32(0x520398u32.wrapping_add(u32::from(owner_is_special) * 4));
        let owner_object = self.memory.read32(self.actor.wrapping_add(4));
        let owner_node = if owner_object == 0 {
            self.memory.read32(0x7a3580u32.wrapping_add(0x678)) as i32
        } else {
            let owner_extension = self.memory.read32(owner_object.wrapping_add(0x7c));
            self.memory.read32(owner_extension.wrapping_add(0x678)) as i32
        };
        let player = self.memory.read32(0x7b0c48u32.wrapping_add(0x14));
        let player_extension = self.memory.read32(player.wrapping_add(0x7c));
        let player_node = self.memory.read32(player_extension.wrapping_add(0x678)) as i32;
        (owner_node, player_node, player_node, animation)
    }
}

/// Original `0x26DBE0(actor)` cdecl. Local state is read and queued through the
/// generation-owned actor store; other fields still use the guest boundary.
fn run_action_guard<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    let actor = memory.read32(regs.esp.wrapping_add(4));
    run_action_guard_for_actor(memory, guest, regs, actor)
}

/// Shared typed guard entry used by its cdecl root and the measured
/// `0x26DD80` caller edge. The latter has the actor in EBX, not on this
/// adapter's stack, so it must not synthesize guest stack storage.
fn run_action_guard_for_actor<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
    actor: u32,
) -> bool {
    use crate::rea_behavior_guard::{ActionGuardState, apply_action_guard};

    if actor == 0 {
        return false;
    }
    let extension = memory.read32(actor.wrapping_add(0x7c));
    if extension == 0 {
        return false;
    }
    bind_actor_extension(memory, extension);
    let (identity, local_state, queued_state) = {
        let mut live = LIVE_ACTORS.lock();
        let Some(identity) = live
            .bindings
            .iter()
            .find(|(bound, _)| *bound == extension)
            .map(|(_, identity)| *identity)
        else {
            return false;
        };
        if !live
            .store
            .refresh_compatibility_snapshot(identity, actor_snapshot(memory, extension))
        {
            return false;
        }
        let Some(owner) = live.store.state(identity) else {
            return false;
        };
        (
            identity,
            owner.local_state().0,
            owner
                .pending_local_state()
                .map_or(u32::MAX, |state| state.0),
        )
    };
    let mut state = ActionGuardState {
        local_flags: memory.read32(extension.wrapping_add(0x28)),
        local_state,
        queued_state,
        action_timer: f32::from_bits(memory.read32(extension.wrapping_add(0xb30))),
        target_node: memory.read32(extension.wrapping_add(0x684)) as i32,
        pending_target_node: memory.read32(extension.wrapping_add(0xac8)) as i32,
        route_progress_a: memory.read32(extension.wrapping_add(0x9ac)),
        route_progress_b: memory.read32(extension.wrapping_add(0x9a8)),
    };

    let before = state;
    let saved = *regs;
    let consumed = {
        let mut host = RuntimeActionGuard {
            memory,
            guest,
            regs,
            actor,
            extension,
        };
        apply_action_guard(&mut state, extension, &mut host)
    };
    *regs = saved;
    if state.local_flags != before.local_flags {
        memory.write32(extension.wrapping_add(0x28), state.local_flags);
    }
    let queued_state_owned =
        if state.queued_state != before.queued_state && (state.queued_state as i32) >= 0 {
            LIVE_ACTORS
                .lock()
                .store
                .queue_local_state(identity, LocalState(state.queued_state))
                .is_ok()
        } else {
            false
        };
    if queued_state_owned {
        memory.write32(extension.wrapping_add(0x9d0), state.queued_state);
    }
    if state.action_timer != before.action_timer {
        memory.write32(extension.wrapping_add(0xb30), state.action_timer.to_bits());
    }
    if state.target_node != before.target_node {
        memory.write32(extension.wrapping_add(0x684), state.target_node as u32);
    }
    if state.pending_target_node != before.pending_target_node {
        memory.write32(
            extension.wrapping_add(0xac8),
            state.pending_target_node as u32,
        );
    }
    if state.route_progress_a != before.route_progress_a {
        memory.write32(extension.wrapping_add(0x9ac), state.route_progress_a);
    }
    if state.route_progress_b != before.route_progress_b {
        memory.write32(extension.wrapping_add(0x9a8), state.route_progress_b);
    }
    regs.eax = u32::from(consumed);
    true
}

/// Original `0x26DD80` uses EBX for the actor and returns no meaningful value.
/// Actor and mode records remain guest-owned at this TEMPORARY-SHIM boundary.
fn run_state6_followup<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    use crate::rea_behavior_followup::{FollowupAction, FollowupEvidence, decide_state6_followup};

    let callee_saved = (regs.ebx, regs.ebp, regs.esi, regs.edi, regs.esp);
    let actor = regs.ebx;
    let extension = memory.read32(actor.wrapping_add(0x7c));
    let action_guard_blocked =
        if function_enabled(0x26dbe0) && run_action_guard_for_actor(memory, guest, regs, actor) {
            regs.eax != 0
        } else {
            call_result(guest, memory, regs, 0x26dbe0, &[actor]) != 0
        };
    let mut evidence = FollowupEvidence {
        action_guard_blocked,
        actor_level: memory.read32(extension.wrapping_add(8)),
        current_level: memory.read32(0x7a3500),
        pair_entry_present: false,
        direction_gate_passed: false,
        state8_pair_selected: false,
        state6_pair_selected: false,
        state6_target: 0,
    };

    if !evidence.action_guard_blocked && evidence.actor_level != evidence.current_level {
        let level = evidence.current_level;
        let level_record = 0x0073_56d8u32.wrapping_add(level.wrapping_mul(0xea0));
        let pair_table = memory.read32(level_record.wrapping_add(8));
        let pair_entry = call_result(
            guest,
            memory,
            regs,
            0x15bd00,
            &[evidence.actor_level, pair_table],
        );
        evidence.pair_entry_present = pair_entry != 0 && memory.read8(pair_entry) != 0;

        if evidence.pair_entry_present {
            let direction_threshold = memory.read32(extension.wrapping_add(0x9bc));
            evidence.direction_gate_passed = call_result(
                guest,
                memory,
                regs,
                0x161630,
                &[actor, level_record.wrapping_add(0x64), direction_threshold],
            ) != 0;
        }
        if evidence.direction_gate_passed {
            evidence.state8_pair_selected =
                call_result(guest, memory, regs, 0x174e0, &[level_record, 0x7120c, 0]) != 0;
            if !evidence.state8_pair_selected {
                evidence.state6_pair_selected =
                    call_result(guest, memory, regs, 0x174e0, &[level_record, 0x400001, 0]) != 0;
                if evidence.state6_pair_selected {
                    evidence.state6_target = memory.read32(level_record);
                }
            }
        }
    }

    let action = decide_state6_followup(evidence);
    match action {
        Some(FollowupAction::SetModeBit8) => {
            call_cdecl(guest, memory, regs, 0x17570, &[extension, 8, 0]);
        }
        Some(FollowupAction::EnterState6 { target }) => {
            call_cdecl(guest, memory, regs, 0x26d8b0, &[actor, target]);
        }
        None => {}
    }
    regs.ebx = callee_saved.0;
    regs.ebp = callee_saved.1;
    regs.esi = callee_saved.2;
    regs.edi = callee_saved.3;
    regs.esp = callee_saved.4;
    true
}

/// Original `0x26D8B0(actor, target)` entry. Rust owns the measured state and
/// route fields for a bound actor; guest memory is only the compatibility
/// projection and `0x267FF0` remains an explicit route service.
fn run_enter_state_six<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    let saved = *regs;
    let actor = memory.read32(regs.esp.wrapping_add(4));
    let target = memory.read32(regs.esp.wrapping_add(8));
    if actor == 0 || target == 0 {
        return false;
    }
    let extension = memory.read32(actor.wrapping_add(0x7c));
    let target_extension = memory.read32(target.wrapping_add(0x7c));
    if extension == 0 || target_extension == 0 {
        return false;
    }
    let identity = {
        LIVE_ACTORS
            .lock()
            .bindings
            .iter()
            .find(|(bound, _)| *bound == extension)
            .map(|(_, identity)| *identity)
    };
    let Some(identity) = identity else {
        return false;
    };

    let requested_node = memory.read32(target_extension.wrapping_add(0x678)) as i32;
    let player_node = if requested_node < 0 {
        let player = memory.read32(0x7b0c48u32.wrapping_add(0x14));
        let player_extension = memory.read32(player.wrapping_add(0x7c));
        memory.read32(player_extension.wrapping_add(0x678)) as i32
    } else {
        0
    };
    let route = {
        let mut live = LIVE_ACTORS.lock();
        let Some(state) = live.store.state(identity) else {
            return false;
        };
        // If the retained prologue still owns the guest field, decline when
        // it has moved since actor registration. Never seed this transition
        // from an arbitrary call-site snapshot.
        if !state.owns_local_state()
            && state.local_state().0 != memory.read32(extension.wrapping_add(0x20))
        {
            return false;
        }
        if !matches!(
            live.store
                .enter_navigation_state_six(identity, requested_node, player_node),
            Ok(true)
        ) {
            return false;
        }
        live.store
            .state(identity)
            .unwrap()
            .actor_navigation()
            .state_six_route()
    };

    memory.write32(extension.wrapping_add(0xac8), route.requested_node as u32);
    memory.write32(extension.wrapping_add(0x684), route.target_node as u32);
    memory.write32(extension.wrapping_add(0x9a8), route.counter_9a8);
    memory.write32(extension.wrapping_add(0x9ac), route.counter_9ac);
    memory.write32(extension.wrapping_add(0xb30), route.progress_b30.to_bits());
    memory.write32(extension.wrapping_add(0x9d0), route.mode_9d0);
    call_preserving_registers(
        memory,
        guest,
        regs,
        0x267ff0,
        &[0, actor, requested_node as u32],
    );
    *regs = saved;
    true
}

fn call_preserving_registers<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
    va: u32,
    args: &[u32],
) {
    let saved = *regs;
    call_cdecl(guest, memory, regs, va, args);
    *regs = saved;
}

fn run_route_link_kind_zero_or_one<M: GuestMemory, G: GuestCall>(
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
    actor: u32,
    extension: u32,
    route: u32,
    kind: u32,
) -> bool {
    if kind == 0 {
        call_preserving_registers(memory, guest, regs, 0x26de30, &[actor, route]);
        return true;
    } else if kind != 1 {
        return false;
    }

    // 0x263ED0 samples root Z at the first and last integer keys. Keep the
    // output areas uninitialized just like the guest stack: unsupported or
    // unavailable sample formats do not acquire invented zero semantics.
    let (original, frame) = (*regs, regs.esp.wrapping_sub(0x100));
    let (first_output, last_output) = (frame.wrapping_add(0x20), frame.wrapping_add(0x40));
    let selected_clip = |m: &M| {
        let offset = (m.read16(0x7e1808) as i16 as i32 * 4) as u32;
        m.read32(m.read32(0x7e3954).wrapping_add(offset))
    };
    let clip = selected_clip(memory);
    let scale = memory.read32(extension.wrapping_add(0xb08));
    regs.esp = frame;
    call_preserving_registers(
        memory,
        guest,
        regs,
        0x59320,
        &[clip, 0, 0, first_output, scale],
    );
    let clip = selected_clip(memory);
    let last_key = memory.read32(clip.wrapping_add(0x10)).wrapping_sub(1);
    let scale = memory.read32(extension.wrapping_add(0xb08));
    call_preserving_registers(
        memory,
        guest,
        regs,
        0x59320,
        &[clip, 0, last_key, last_output, scale],
    );
    let first_z = f32::from_bits(memory.read32(first_output.wrapping_add(8)));
    let last_z = f32::from_bits(memory.read32(last_output.wrapping_add(8)));
    *regs = original;

    let goal_id = memory.read32(extension.wrapping_add(0x684));
    let primary = memory.read32(0x715f2c);
    let goal_record = primary.wrapping_add(goal_id.wrapping_mul(32));
    let actor_position = Position3 {
        x: f32::from_bits(memory.read32(actor.wrapping_add(0x64))),
        y: f32::from_bits(memory.read32(actor.wrapping_add(0x68))),
        z: f32::from_bits(memory.read32(actor.wrapping_add(0x6c))),
    };
    let goal_position = Position3 {
        x: f32::from_bits(memory.read32(goal_record.wrapping_add(0x0c))),
        y: f32::from_bits(memory.read32(goal_record.wrapping_add(0x10))),
        z: f32::from_bits(memory.read32(goal_record.wrapping_add(0x14))),
    };
    match link_follow_decision(LinkFollowInput {
        route_kind: kind,
        behavior_route_selector: memory.read32(
            memory
                .read32(extension.wrapping_add(0xbf0))
                .wrapping_add(0x68),
        ) as i32,
        animation_matches: false,
        actor_position,
        goal_position,
        root_first_z: first_z,
        root_last_z: last_z,
    }) {
        LinkFollowDecision::AskNavigationToAdvance => {
            call_preserving_registers(memory, guest, regs, 0x15d500, &[actor, goal_id])
        }
        LinkFollowDecision::AdvanceCurrentToGoal => {
            memory.write32(extension.wrapping_add(0x678), goal_id);
            call_preserving_registers(memory, guest, regs, 0x26de30, &[actor, route]);
        }
        _ => unreachable!("kind-1 owner returns one of its two measured decisions"),
    }
    true
}

pub fn configure(enabled: bool) {
    ENABLED_FUNCTIONS.store(
        if enabled {
            DEFAULT_ADOPTED_FUNCTIONS
        } else {
            0
        },
        Ordering::Release,
    );
    ENABLED.store(enabled, Ordering::Release);
}

/// Enables or disables one adopted runtime boundary for A/B diagnosis.
/// Unknown addresses are rejected so a typo cannot silently leave an adapter on.
pub fn configure_function(va: u32, enabled: bool) -> bool {
    let Some(bit) = function_bit(va) else {
        return false;
    };
    if enabled {
        ENABLED_FUNCTIONS.fetch_or(bit, Ordering::AcqRel);
    } else {
        ENABLED_FUNCTIONS.fetch_and(!bit, Ordering::AcqRel);
    }
    true
}

fn function_enabled(va: u32) -> bool {
    function_bit(va).is_some_and(|bit| ENABLED_FUNCTIONS.load(Ordering::Acquire) & bit != 0)
}

fn call_result<M: GuestMemory, G: GuestCall>(
    guest: &mut G,
    memory: &mut M,
    regs: &mut ExtRegisters,
    va: u32,
    args: &[u32],
) -> u32 {
    call_cdecl(guest, memory, regs, va, args);
    regs.eax
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// `0x26D390`: snapshot the selected recent-actor row and the group member
/// windows, then apply the shared nearest-candidate owner. The guest snapshot
/// and writeback are a TEMPORARY-SHIM pending T2050 Rust-owned actor/group state.
fn run_recent_group_target<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) -> bool {
    const RECENT_ROWS: u32 = 0x007a_2900;
    const RECENT_ROW_STRIDE: u32 = 0x2c;
    const RECENT_SELECTOR: u32 = 0x007a_295c;
    const RECENT_CAPACITY: i32 = 10;
    const GROUP_WINDOWS: u32 = 0x007a_2d94;
    const GROUP_WINDOW_STRIDE: u32 = 0x60;
    const GROUP_MEMBER_CAPACITY: i32 = 22;

    let actor = regs.eax;
    if actor == 0 {
        return false;
    }
    let extension = memory.read32(actor.wrapping_add(0x7c));
    if extension == 0 {
        return false;
    }

    let row = if memory.read32(RECENT_SELECTOR) == 0 {
        1
    } else {
        0
    };
    let recent_base = RECENT_ROWS.wrapping_add(row * RECENT_ROW_STRIDE);
    let count = memory.read32(recent_base) as i32;
    if count <= 0 {
        return true;
    }
    if count > RECENT_CAPACITY {
        return false;
    }

    let source = GroupPosition3 {
        x: f32::from_bits(memory.read32(actor.wrapping_add(0x64))),
        y: f32::from_bits(memory.read32(actor.wrapping_add(0x68))),
        z: f32::from_bits(memory.read32(actor.wrapping_add(0x6c))),
    };
    let mut candidates = Vec::with_capacity(count as usize);
    for index in 0..count as u32 {
        let candidate = memory.read32(
            recent_base
                .wrapping_add(4)
                .wrapping_add(index.wrapping_mul(4)),
        );
        if candidate == 0 {
            return false;
        }
        candidates.push(RecentCandidate {
            group_mask: memory.read32(candidate.wrapping_add(0x114)),
            position: GroupPosition3 {
                x: f32::from_bits(memory.read32(candidate.wrapping_add(0x64))),
                y: f32::from_bits(memory.read32(candidate.wrapping_add(0x68))),
                z: f32::from_bits(memory.read32(candidate.wrapping_add(0x6c))),
            },
            target: memory.read32(candidate.wrapping_add(0x110)) as i32,
        });
    }

    let mut groups = Vec::with_capacity(GROUP_BUCKETS);
    for bucket in 0..GROUP_BUCKETS {
        let bit = 1u32 << (bucket & 31);
        let needed = candidates.iter().any(|candidate| {
            candidate.group_mask < GROUP_MASK_LIMIT && candidate.group_mask & bit != 0
        });
        if !needed {
            groups.push(Vec::new());
            continue;
        }
        let window = GROUP_WINDOWS.wrapping_add((bucket as u32).wrapping_mul(GROUP_WINDOW_STRIDE));
        let member_count = memory.read32(window.wrapping_sub(0x14)) as i32;
        if member_count <= 0 {
            groups.push(Vec::new());
            continue;
        }
        if member_count > GROUP_MEMBER_CAPACITY {
            return false;
        }
        let mut members = Vec::with_capacity(member_count as usize);
        for index in 0..member_count as u32 {
            members.push(memory.read32(window.wrapping_sub(4).wrapping_add(index.wrapping_mul(4))));
        }
        groups.push(members);
    }

    let initial_max_distance = f32::from_bits(memory.read32(0x0047_5d5c));
    let Some(selected) =
        nearest_candidate(actor, source, initial_max_distance, &candidates, &groups)
    else {
        return true;
    };
    let target = candidates[selected].target;
    if target != -1 {
        memory.write32(extension.wrapping_add(0xac8), target as u32);
        let flags = memory.read32(extension.wrapping_add(0x28));
        memory.write32(extension.wrapping_add(0x28), flags | 0x400);
    }
    true
}

fn run_weapon_ammo_fallback<M: GuestMemory>(memory: &M, regs: &mut ExtRegisters) -> bool {
    let owner = memory.read32(regs.esp.wrapping_add(4));
    let state = memory.read32(regs.esp.wrapping_add(8));
    let mode = memory.read32(regs.esp.wrapping_add(12));
    let definition_index = memory.read32(state.wrapping_add(0x30)) as i32;
    if definition_index < 0 {
        regs.eax = 0;
        return true;
    }

    let definition = memory
        .read32(0x4f9bac)
        .wrapping_add((definition_index as u32).wrapping_mul(0x29c));
    let mode_row = owner
        .wrapping_add(mode.wrapping_mul(0x7c))
        .wrapping_add(0x68);
    let marker = memory.read32(mode_row) as i32;
    let limit_row = if marker >= 0 {
        mode_row
    } else {
        definition.wrapping_add(0x68)
    };
    let limit_offset = if mode == 0 { 0x24 } else { 0x28 };
    let ammo_kind = crate::weapon_ammo_fallback::selected_ammo_kind(
        mode,
        memory.read32(definition.wrapping_add(0x14)),
        memory.read32(definition.wrapping_add(0x1c)),
    );
    let owner_rounds = memory.read16(
        owner
            .wrapping_add(ammo_kind.wrapping_mul(2))
            .wrapping_add(0x61e),
    ) as i16;
    let state_rounds = memory.read16(
        state
            .wrapping_add(ammo_kind.wrapping_mul(2))
            .wrapping_add(0x38),
    ) as i16;
    regs.eax = u32::from(crate::weapon_ammo_fallback::ammo_total_covers_cost(
        crate::weapon_ammo_fallback::AmmoFallbackFacts {
            definition_index,
            mode_row_marker: marker,
            mode_limit: memory.read32(limit_row.wrapping_add(limit_offset)) as i32,
            definition_limit: memory.read32(definition.wrapping_add(0x68)) as i32,
            owner_rounds,
            state_rounds,
        },
    ));
    true
}

/// Returns true only for a boundary handled by the readable owner.
pub fn run<M: GuestMemory, G: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    // T3421: the mechanism handle adapter can be entered as a lane root, without
    // crossing LaneGuest::call. Record its stack argument at this dispatch seam.
    let handle_trace = va == 0x12f00;
    let handle_arg = if handle_trace {
        memory.read32(regs.esp.wrapping_add(4))
    } else {
        0
    };
    if handle_trace {
        #[cfg(all(not(test), not(feature = "test-diagnostics")))]
        unsafe {
            host_rust_ports_nested_call_args(va, 0, handle_arg, 0)
        };
    }
    if !enabled() || !function_enabled(va) {
        return false;
    }

    if matches!(va, 0x2a1f30 | 0x2a4120) {
        // This hook only consumes the shared typed prologue; the selected
        // lifted routine continues with its remaining, still-unported body.
        apply_local_state_prologue(memory, regs);
        return false;
    }

    if va == 0x9a260 {
        return run_navigation_graph_root_invalidation(memory, regs);
    }

    if va == 0x17570 {
        return run_behavior_request(memory, regs);
    }

    if va == 0x9cc40 {
        return run_key_to_record_id(memory, regs);
    }

    if va == 0x26dbe0 {
        return run_action_guard(memory, guest, regs);
    }

    if va == 0x26dd80 {
        return run_state6_followup(memory, guest, regs);
    }

    if va == 0x26d8b0 {
        return run_enter_state_six(memory, guest, regs);
    }

    if va == 0x1b6db0 {
        crate::weapons::b6db0(memory, guest, regs);
        return true;
    }

    if va == 0x1b6c90 {
        return run_weapon_ammo_fallback(memory, regs);
    }

    if va == 0xa4c70 {
        let requested = memory.read32(regs.esp.wrapping_add(4));
        let ledger = crate::rea_object_pool_ledger::PoolLedger {
            slot_count: memory.read32(0x7b4fac),
            retired_count: memory.read32(0x7b65b8),
            capacity: memory.read32(0x72f5e4),
        };
        regs.eax = u32::from(ledger.can_reserve(requested));
        return true;
    }

    if va == 0x1b63d0 {
        let object = memory.read32(regs.esp.wrapping_add(4));
        let target = memory.read32(regs.esp.wrapping_add(8));
        let current = memory.read32(object.wrapping_add(0xe8));
        let pending = memory.read32(object.wrapping_add(0xf8));
        regs.eax = u32::from(crate::weapon_transition::is_pending(
            current, pending, target,
        ));
        return true;
    }

    if va == 0x1b7cc0 {
        let record = memory.read32(memory.read32(regs.esp.wrapping_add(4)).wrapping_add(0x30));
        if record == u32::MAX {
            regs.eax = 0;
            return true;
        }
        let player = memory.read32(0x7b0c7c);
        let player_index = memory.read32(player.wrapping_add(4)) as i32;
        if player_index < 0 {
            regs.eax = 0;
            return true;
        }
        let extension = memory.read32(player.wrapping_add(0xe84));
        if extension == 0 {
            regs.eax = 0;
            return true;
        }
        let table = memory.read32(0x4f9bac);
        let definition = memory.read32(table.wrapping_add(record.wrapping_mul(0x29c)));
        regs.eax = u32::from(crate::weapon_context::has_weapon_context(
            crate::weapon_context::WeaponContextFacts {
                weapon_record: record,
                player_index,
                player_extension: extension,
                definition_index: definition,
            },
        ));
        return true;
    }

    if va == 0x1b9a30 {
        let object = regs.edx;
        let current = memory.read32(object.wrapping_add(0xe8));
        let pending = memory.read32(object.wrapping_add(0xf8));
        let mut probes = [false; 3];
        let state = crate::weapon_readiness::WeaponState { current, pending };
        // Preserve original short-circuit order: state mismatch precedes all
        // transition predicates; the first pending target rejects readiness.
        let stable = match current {
            9 | 14 | 19 => pending == current,
            18 => pending == 18,
            _ => true,
        };
        if stable {
            for (index, target) in crate::weapon_readiness::TRANSITION_TARGETS
                .into_iter()
                .enumerate()
            {
                probes[index] = crate::weapon_transition::is_pending(current, pending, target);
                if probes[index] {
                    break;
                }
            }
        }
        regs.eax = u32::from(crate::weapon_readiness::is_ready(state, probes));
        return true;
    }

    if va == 0x59d20 {
        return run_owner_has_either_slot(memory, regs);
    }

    if va == 0x9a420 {
        return run_select_valid_record(memory, guest, regs);
    }

    if va == 0x12f00 {
        let handled = run_mechanism_handle_resolve(memory, regs);
        if handle_trace && handled {
            #[cfg(all(not(test), not(feature = "test-diagnostics")))]
            unsafe {
                host_rust_ports_nested_call_args(va, 1, handle_arg, regs.eax)
            };
        }
        return handled;
    }

    if va == 0x177920 {
        return run_mechanism_record_resolve(memory, regs);
    }

    if va == 0x2a0500 && run_mechanism_tick(memory, guest, regs) {
        return true;
    }

    if va == 0x288030 {
        return run_particle_pool(memory, regs);
    }

    if va == 0x1b6bf0 {
        return run_weapon_cycle(memory, regs);
    }

    if va == 0x1b6bb0 {
        return run_weapon_group_lookup(memory, regs);
    }

    if va == 0x1b99f0 {
        return run_weapon_subrecord(memory, regs);
    }

    if va == 0x27ce70 {
        let (actor, message) = (
            memory.read32(regs.esp.wrapping_add(4)),
            memory.read32(regs.esp.wrapping_add(8)),
        );
        let mut writes = Vec::new();
        let result = crate::rea_character_leaves::handle_message_3eb(
            &GuestTables(&*memory),
            |address, value| writes.push((address, value)),
            actor,
            message,
        );
        for (address, value) in writes {
            memory.write32(address, value);
        }
        regs.eax = result;
        return true;
    }

    if va == 0x26d390 {
        return run_recent_group_target(memory, regs);
    }

    if va == 0x26ce20 {
        let bits = crate::rea_character_leaves::weapon_action_yaw_offset_bits(
            &GuestTables(&*memory),
            regs.edi,
        );
        // Original MOVSS writes the scalar lane. Callers consume XMM0's low float only.
        regs.xmm[0].lanes[0] = bits;
        return true;
    }

    if va == 0x59d50 {
        let actor = memory.read32(regs.esp.wrapping_add(4));
        let bits = crate::rea_weapon_turn::actor_turn_rate_bits(&GuestTables(&*memory), actor);
        regs.fpu.return_st0(f32::from_bits(bits));
        return true;
    }

    if let Some(result) = run_character_leaf(va, memory, regs) {
        regs.eax = result;
        return true;
    }

    if va == 0xa0640 {
        return run_register_spawn_entries(memory, guest, regs);
    }

    if va == 0x349e20 {
        let value = memory.read32(regs.esp.wrapping_add(4));
        crate::rea_awareness_leaves::set_group_alert_value(
            |cell, v| memory.write32(cell, v),
            value,
        );
        regs.eax = value;
        return true;
    }

    if va == 0x3797b0 {
        return run_view_direction(memory, regs);
    }

    if va == 0x15d470 {
        let actor = memory.read32(regs.esp.wrapping_add(4));
        let original = *regs;
        crate::rea_awareness_leaves::route_nav_update(
            |actor, first, second| {
                call_cdecl(guest, memory, regs, 0x159e50, &[actor, first, second]);
            },
            actor,
        );
        let result = regs.eax;
        *regs = original;
        regs.eax = result;
        return true;
    }

    if va == 0x15d500 {
        return run_nav_link(memory, guest, regs);
    }

    if va == 0x267ff0 {
        return run_group_alert(memory, guest, regs);
    }

    if va == 0x25faf0 {
        let incoming = crate::rea_ai_types::GroupMask(memory.read32(regs.esp.wrapping_add(4)));
        let saved = *regs;
        let mut world = RuntimeAwarenessWorld {
            memory,
            guest,
            regs,
            current_record: 0,
        };
        process_awareness_sources(incoming, &mut world);
        *regs = saved;
        return true;
    }

    if va == 0x261de0 {
        run_navigation_scorer(memory, guest, regs);
        return true;
    }

    if va == 0x161730 && run_yaw_table_perception(memory, guest, regs) {
        return true;
    }

    if va == 0x26e150 {
        let actor = regs.eax;
        let route = regs.ebx;
        let extension = memory.read32(actor.wrapping_add(0x7c));
        let local_flags = memory.read32(extension.wrapping_add(0x28));
        let cursor_address = extension.wrapping_add(0xaa0);
        let cursor = behavior_route_cursor(memory, extension);
        let entry = route.wrapping_add(cursor.wrapping_mul(12));
        let kind = memory.read32(entry);
        if crate::rea_executor_gate::resets_route_to_first(local_flags, kind) {
            // The selected leaf receives the live actor directly. If level
            // loading skipped the append hook, attach the exact actor at this
            // movement boundary before consuming the Rust-owned reset.
            if !actor_movement_started(memory, actor) {
                return false;
            }
            if !try_set_behavior_route_cursor(extension, 1) {
                clear_behavior_route_cursor(extension);
                return false;
            }
            memory.write32(cursor_address, 1);
            return true;
        }
        clear_behavior_route_cursor(extension);
    }

    if va == 0x263ed0 {
        // EDI is the actor object and the documented -2 selector returns
        // before touching EBX's route entry array.
        let actor = regs.edi;
        if !actor_movement_started(memory, actor) {
            return false;
        }
        let extension = memory.read32(actor.wrapping_add(0x7c));
        let behavior = memory.read32(extension.wrapping_add(0xbf0));
        let selector = memory.read32(behavior.wrapping_add(0x68)) as i32;
        if crate::rea_link_gate::returns_before_route(selector) {
            return true;
        }
        let route = regs.ebx;
        let cursor = behavior_route_cursor(memory, extension);
        let entry = route.wrapping_add(cursor.wrapping_mul(12));
        let kind = memory.read32(entry);
        if run_route_link_kind_zero_or_one(memory, guest, regs, actor, extension, route, kind) {
            return true;
        }

        let actor_position = Position3 {
            x: f32::from_bits(memory.read32(actor.wrapping_add(0x64))),
            y: f32::from_bits(memory.read32(actor.wrapping_add(0x68))),
            z: f32::from_bits(memory.read32(actor.wrapping_add(0x6c))),
        };
        let goal_id = memory.read32(extension.wrapping_add(0x684));
        let primary = memory.read32(0x715f2c);
        let goal_record = primary.wrapping_add(goal_id.wrapping_mul(32));
        let goal_position = Position3 {
            x: f32::from_bits(memory.read32(goal_record.wrapping_add(0x0c))),
            y: f32::from_bits(memory.read32(goal_record.wrapping_add(0x10))),
            z: f32::from_bits(memory.read32(goal_record.wrapping_add(0x14))),
        };
        let original = *regs;
        regs.esp = original.esp.wrapping_sub(0x100);
        call_cdecl(guest, memory, regs, 0x59d20, &[actor, 0xa25]);
        let animation_matches = regs.eax != 0;
        *regs = original;
        let decision = link_follow_decision(LinkFollowInput {
            route_kind: kind,
            behavior_route_selector: selector,
            animation_matches,
            actor_position,
            goal_position,
            root_first_z: 0.0,
            root_last_z: 0.0,
        });
        let target = match decision {
            LinkFollowDecision::TargetGoalPosition => goal_position,
            LinkFollowDecision::TargetActorPosition => actor_position,
            _ => unreachable!("ordinary non-kind-0/1 path selects a movement target"),
        };
        store_navigation_movement_target(extension, target);
        // TEMPORARY-SHIM: lifted readers still consume the original extension
        // target while their remaining movement paths are ported.
        memory.write32(extension.wrapping_add(0x30), target.x.to_bits());
        memory.write32(extension.wrapping_add(0x34), target.y.to_bits());
        memory.write32(extension.wrapping_add(0x38), target.z.to_bits());
        return true;
    }

    if va == 0x26d690 {
        // The cited ABI is cdecl(actor, route) -> selected-entry pointer.
        let actor = memory.read32(regs.esp.wrapping_add(4));
        let route = memory.read32(regs.esp.wrapping_add(8));
        let extension = memory.read32(actor.wrapping_add(0x7c));
        let cursor = behavior_route_cursor(memory, extension);
        let next_index = cursor.wrapping_add(1);
        let next = route.wrapping_add(next_index.wrapping_mul(12));
        let selected = crate::rea_route_peek::peek_next_index(
            cursor,
            memory.read32(next),
            memory.read32(next.wrapping_add(4)),
        );
        regs.eax = route.wrapping_add(selected.wrapping_mul(12));
        return true;
    }

    if va == 0x26e100 {
        return run_route_peek_kind(memory, regs);
    }

    if va == 0x26de30 {
        return run_route_advance(memory, guest, regs);
    }

    if va != 0x266250 {
        return false;
    }

    // Level-loaded actors can predate selected append callbacks. Bind the
    // actor passed to this leaf directly; do not infer lifecycle from the
    // disputed global table shape at 0x7A3580.
    let actor = memory.read32(regs.esp.wrapping_add(4));
    // This generic movement leaf also handles actors without an AI extension;
    // the actor-generation guard only gates leaves that consume owned AI state.
    let _ = actor_movement_started(memory, actor);

    // The static contract fixes the dead zone at 0.01. If the live constant is
    // changed, retain the old implementation rather than silently changing it.
    if memory.read32(0x475d1c) != 0x3c23_d70a {
        return false;
    }

    let target = memory.read32(regs.esp.wrapping_add(8));
    let fraction = f32::from_bits(memory.read32(regs.esp.wrapping_add(12)));
    let x = actor.wrapping_add(0x64);
    let z = actor.wrapping_add(0x6c);
    let mut position = Position3 {
        x: f32::from_bits(memory.read32(x)),
        y: f32::from_bits(memory.read32(actor.wrapping_add(0x68))),
        z: f32::from_bits(memory.read32(z)),
    };
    let destination = Position3 {
        x: f32::from_bits(memory.read32(target.wrapping_add(0x0c))),
        y: f32::from_bits(memory.read32(target.wrapping_add(0x10))),
        z: f32::from_bits(memory.read32(target.wrapping_add(0x14))),
    };
    if approach_xz(&mut position, destination, fraction) {
        memory.write32(x, position.x.to_bits());
        memory.write32(z, position.z.to_bits());
    }
    regs.eax = actor;
    true
}
/// Dispatch an adopted function from `GuestCall::call`.
///
/// `GuestCall::call` is entered with the caller's arguments already pushed but
/// before the x86 `CALL` return slot is present. The lifted trampoline creates
/// that slot before entering the callee. Mirror that here so the adapter sees
/// its normal callee-entry ABI (`[ESP + 4]` is argument one), then mirror `RET`
/// on success. The cdecl caller remains responsible for removing its arguments.
pub(crate) fn run_nested<M: GuestMemory, G: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut G,
    regs: &mut ExtRegisters,
) -> bool {
    let caller = *regs;
    regs.esp = caller.esp.wrapping_sub(4);
    memory.write32(regs.esp, 0);
    if !run(va, memory, guest, regs) {
        *regs = caller;
        return false;
    }

    // Adopted adapters implement cdecl returns. Preserve the nonvolatile x86
    // registers just as a lifted callee does; EAX remains the return value.
    regs.esp = caller.esp;
    regs.ebx = caller.ebx;
    regs.ebp = caller.ebp;
    regs.esi = caller.esi;
    regs.edi = caller.edi;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{Fpu, Xmm};
    use crate::memory::GuestMemory;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Clone, Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn is_readable(&self, address: u32, len: u32) -> bool {
            address
                .checked_add(len)
                .is_some_and(|end| end <= 0x0400_0000)
        }
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    impl Memory {
        fn word(&mut self, address: u32, value: u32) {
            for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address.wrapping_add(index as u32), byte);
            }
        }
        fn float(&mut self, address: u32, value: f32) {
            self.word(address, value.to_bits());
        }
        fn game_data(&self) -> BTreeMap<u32, u8> {
            self.0
                .iter()
                .filter(|(a, _)| !(0x8ff00..0x90100).contains(*a))
                .map(|(a, v)| (*a, *v))
                .collect()
        }
    }

    #[derive(Default)]
    struct WriteTrackingMemory {
        memory: Memory,
        writes: Vec<u32>,
    }
    impl GuestMemory for WriteTrackingMemory {
        fn is_readable(&self, address: u32, len: u32) -> bool {
            self.memory.is_readable(address, len)
        }
        fn read8(&self, address: u32) -> u8 {
            self.memory.read8(address)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.writes.push(address);
            self.memory.write8(address, value);
        }
    }

    fn publish_navigation_graph(memory: &mut Memory) {
        for (index, root) in super::NAVIGATION_GRAPH_ROOTS.into_iter().enumerate() {
            memory.word(root, 0x5000 + (index as u32 * 0x100));
        }
    }

    struct NoExtensionReads<'a> {
        inner: &'a Memory,
        blocked: core::ops::Range<u32>,
    }
    impl GuestMemory for NoExtensionReads<'_> {
        fn is_readable(&self, address: u32, len: u32) -> bool {
            self.inner.is_readable(address, len)
        }
        fn read8(&self, address: u32) -> u8 {
            assert!(
                !self.blocked.contains(&address),
                "retired actor extension was read at {address:#x}"
            );
            self.inner.read8(address)
        }
        fn write8(&mut self, _address: u32, _value: u8) {
            unreachable!("controller finish is read-only")
        }
    }

    #[derive(Clone, Default)]
    struct TestGuest {
        calls: Vec<u32>,
        delegated_direction: bool,
        animation_match: bool,
        service_results: BTreeMap<u32, u32>,
        service_result_sequences: BTreeMap<u32, Vec<u32>>,
        service_result_counts: BTreeMap<u32, usize>,
        resolved_route_node: u32,
        node_records: Vec<u32>,
        next_node_record: usize,
    }
    impl GuestCall for TestGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.calls.push(va);
            if va == 0xd6ea0 {
                regs.eax = u32::from(self.delegated_direction);
            } else if va == 0x59d20 {
                regs.eax = u32::from(self.animation_match);
            } else if va == 0x9a420 {
                regs.eax = self
                    .node_records
                    .get(self.next_node_record)
                    .copied()
                    .unwrap_or(0);
                self.next_node_record += 1;
            } else if va == 0x9cc40 {
                regs.eax = self.resolved_route_node;
            } else if let Some(sequence) = self.service_result_sequences.get(&va) {
                let count = self.service_result_counts.entry(va).or_default();
                regs.eax = sequence
                    .get(*count)
                    .or_else(|| sequence.last())
                    .copied()
                    .unwrap_or(0);
                *count += 1;
            } else if let Some(result) = self.service_results.get(&va) {
                regs.eax = *result;
            }
        }
    }

    fn run_without_services<M: GuestMemory>(
        va: u32,
        memory: &mut M,
        regs: &mut ExtRegisters,
    ) -> bool {
        super::run(va, memory, &mut TestGuest::default(), regs)
    }

    #[test]
    fn route_link_clip_table_is_indirect_for_nonzero_selector() {
        struct CheckedMemory(Memory);
        impl GuestMemory for CheckedMemory {
            fn is_readable(&self, address: u32, len: u32) -> bool {
                self.0.is_readable(address, len)
            }
            fn read8(&self, address: u32) -> u8 {
                assert_ne!(address, 0x45, "invalid clip 0x35 was dereferenced");
                self.0.read8(address)
            }
            fn write8(&mut self, address: u32, value: u8) {
                self.0.write8(address, value);
            }
        }
        let mut memory = CheckedMemory(Memory::default());
        let mut regs = regs();
        let frame = regs.esp - 0x100;
        let extension = 0xa100;
        memory.0.word(0x7e1808, 1);
        memory.0.word(0x7e3954, 0x13000);
        memory.0.word(0x7e3958, 0x35); // observed erroneous direct-slot value
        memory.0.word(0x13004, 0x14000);
        memory.0.word(0x14010, 7);
        let mut guest = TestGuest::default();
        assert!(run_route_link_kind_zero_or_one(
            &mut memory,
            &mut guest,
            &mut regs,
            0xa000,
            extension,
            0xc000,
            1
        ));
        assert_eq!(&guest.calls[..2], &[0x59320, 0x59320]);
        assert_eq!(memory.0.read32(frame - 20), 0x14000);
        assert_eq!(memory.0.read32(frame - 12), 6);
    }

    fn stored_behavior_route_cursor(extension: u32) -> Option<u32> {
        let live = super::LIVE_ACTORS.lock();
        let identity = live
            .bindings
            .iter()
            .find(|(bound, _)| *bound == extension)
            .map(|(_, identity)| *identity)?;
        live.store.behavior_route_cursor(identity).ok().flatten()
    }

    fn regs() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x9000,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x27f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }

    #[test]
    fn local_state_prologue_adapters_consume_the_measured_request_then_delegate() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);
        for va in [0x2a1f30, 0x2a4120] {
            reset_actor_state();
            let mut memory = Memory::default();
            let actor = 0x2000;
            let extension = 0x3000;
            let stack = 0x1000;
            memory.word(actor + 0x7c, extension);
            memory.word(extension + 0x20, 3);
            memory.word(extension + 0x9d0, 5);
            memory.word(extension + 0x9d4, 1);
            memory.word(extension + 0x9d8, 0x4020_0000);
            memory.word(stack + 4, actor);
            actor_spawned(&memory, actor);
            let mut regs = regs();
            regs.esp = stack;

            assert!(!function_enabled(va));
            assert!(!run(va, &mut memory, &mut TestGuest::default(), &mut regs));
            assert_eq!(memory.read32(extension + 0x9d0), 5);
            assert!(configure_function(va, true));
            // False means the Rust hook only consumed the shared prologue;
            // the rest of the lifted routine still executes.
            assert!(!run(va, &mut memory, &mut TestGuest::default(), &mut regs));
            assert_eq!(memory.read32(extension + 0x9d0), u32::MAX);
            assert_eq!(memory.read32(extension + 0x20), 5);
            assert_eq!(memory.read32(extension + 0x9d4), 3);
            assert_eq!(memory.read32(extension + 0x9d8), 0);
            let live = LIVE_ACTORS.lock();
            let identity = live.bindings[0].1;
            let state = live.store.state(identity).unwrap();
            assert_eq!(state.local_state(), LocalState(5));
            assert!(!state.owns_local_state());
            drop(live);
        }
        configure(false);
        reset_actor_state();
    }

    #[test]
    fn local_state_prologue_projects_changed_fields_in_measured_order() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);
        assert!(configure_function(0x2a1f30, true));

        let actor = 0x2000;
        let extension = 0x3000;
        let stack = 0x1000;
        let mut memory = WriteTrackingMemory::default();
        memory.memory.word(actor + 0x7c, extension);
        memory.memory.word(extension + 0x20, 3);
        memory.memory.word(extension + 0x9d0, u32::MAX);
        memory.memory.word(extension + 0x9d4, 1);
        memory.memory.word(extension + 0x9d8, 0x4020_0000);
        memory.memory.word(stack + 4, actor);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;
        LIVE_ACTORS
            .lock()
            .store
            .queue_local_state(identity, LocalState(7))
            .unwrap();

        let mut regs = regs();
        regs.esp = stack;
        assert!(!run(
            0x2a1f30,
            &mut memory,
            &mut TestGuest::default(),
            &mut regs,
        ));

        let measured_fields = [
            extension + 0x9d4,
            extension + 0x20,
            extension + 0x9d8,
            extension + 0x9d0,
        ];
        let writes: Vec<u32> = memory
            .writes
            .iter()
            .copied()
            .filter(|address| measured_fields.contains(address))
            .collect();
        assert_eq!(writes, measured_fields);
        assert_eq!(memory.read32(extension + 0x20), 7);
        assert_eq!(memory.read32(extension + 0x9d4), 3);
        assert_eq!(memory.read32(extension + 0x9d8), 0);
        assert_eq!(memory.read32(extension + 0x9d0), u32::MAX);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn local_state_prologue_preserves_typed_requests_over_guest_projection() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);
        assert!(configure_function(0x2a4120, true));
        let mut memory = Memory::default();
        let actor = 0x2000;
        let extension = 0x3000;
        let stack = 0x1000;
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x20, 3);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 0x9d4, 1);
        memory.word(extension + 0x9d8, 0x4020_0000);
        memory.word(stack + 4, actor);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;
        LIVE_ACTORS
            .lock()
            .store
            .queue_local_state(identity, LocalState(8))
            .unwrap();
        memory.word(extension + 0x9d0, 6);
        let mut regs = regs();
        regs.esp = stack;

        assert!(!run(
            0x2a4120,
            &mut memory,
            &mut TestGuest::default(),
            &mut regs,
        ));
        assert_eq!(memory.read32(extension + 0x9d0), u32::MAX);
        assert_eq!(memory.read32(extension + 0x20), 8);
        assert_eq!(memory.read32(extension + 0x9d4), 3);
        assert_eq!(memory.read32(extension + 0x9d8), 0);
        let live = LIVE_ACTORS.lock();
        let state = live.store.state(identity).unwrap();
        assert_eq!(state.local_state(), LocalState(8));
        assert!(state.owns_local_state());
        drop(live);
        configure(false);
        reset_actor_state();
    }

    #[test]
    fn action_guard_local_state_request_is_owned_by_actor_generation() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);
        assert!(!function_enabled(0x26dbe0));
        assert!(configure_function(0x26dbe0, true));
        assert!(configure_function(0x2a4120, true));

        let actor = 0x2000;
        let extension = 0x3000;
        let stack = 0x1000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x20, 1);
        memory.word(extension + 0x28, 0);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 0x9d4, 0);
        memory.word(extension + 0x9d8, 0x7fc0_1234);
        memory.word(extension + 0xb30, 0x3f80_0000);
        memory.word(stack + 4, actor);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;
        let mut regs = regs();
        regs.esp = stack;
        let mut guest = TestGuest {
            service_results: [(0x15fe00, 1)].into(),
            ..TestGuest::default()
        };

        assert!(run(0x26dbe0, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 1);
        assert_eq!(memory.read32(extension + 0x9d0), 3);
        {
            let live = LIVE_ACTORS.lock();
            let state = live.store.state(identity).unwrap();
            assert_eq!(state.local_state(), LocalState(1));
            assert_eq!(state.pending_local_state(), Some(LocalState(3)));
            assert!(state.owns_local_state());
        }

        // The next local-state prologue consumes the typed request and projects
        // its result for the retained state-machine body.
        assert!(!run(0x2a4120, &mut memory, &mut guest, &mut regs));
        assert_eq!(memory.read32(extension + 0x20), 3);
        assert_eq!(memory.read32(extension + 0x9d4), 1);
        assert_eq!(memory.read32(extension + 0x9d0), u32::MAX);
        assert_eq!(memory.read32(extension + 0x9d8), 0);
        let live = LIVE_ACTORS.lock();
        let state = live.store.state(identity).unwrap();
        assert_eq!(state.local_state(), LocalState(3));
        assert_eq!(state.pending_local_state(), None);
        assert!(state.owns_local_state());
        drop(live);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn state6_followup_uses_selected_guard_against_actor_owned_local_state() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);
        assert!(configure_function(0x26dd80, true));
        assert!(configure_function(0x26dbe0, true));

        let actor = 0x2000;
        let extension = 0x3000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 8, 4);
        memory.word(extension + 0x20, 1);
        memory.word(extension + 0x28, 0);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 0x9d4, 0);
        memory.word(extension + 0x9d8, 0x7fc0_1234);
        memory.word(extension + 0xb30, 0x3f80_0000);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;

        let mut regs = regs();
        regs.ebx = actor;
        let mut guest = TestGuest {
            service_results: [(0x15fe00, 1)].into(),
            ..TestGuest::default()
        };

        assert!(run(0x26dd80, &mut memory, &mut guest, &mut regs));
        assert_eq!(memory.read32(extension + 0x9d0), 3);
        assert!(!guest.calls.contains(&0x26dbe0));
        let live = LIVE_ACTORS.lock();
        let state = live.store.state(identity).unwrap();
        assert_eq!(state.local_state(), LocalState(1));
        assert_eq!(state.pending_local_state(), Some(LocalState(3)));
        assert!(state.owns_local_state());
        drop(live);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn state6_entry_updates_actor_owned_route_and_projects_before_dispatch() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);
        assert!(configure_function(0x26d8b0, true));

        let actor = 0x2000;
        let extension = 0x3000;
        let target = 0x4000;
        let target_extension = 0x5000;
        let player = 0x6000;
        let player_extension = 0x7000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x20, 1);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 0x9a8, 8);
        memory.word(extension + 0x9ac, 9);
        memory.word(extension + 0xb30, 0x4020_0000);
        memory.word(target + 0x7c, target_extension);
        memory.word(target_extension + 0x678, u32::MAX);
        memory.word(0x7b0c48 + 0x14, player);
        memory.word(player + 0x7c, player_extension);
        memory.word(player_extension + 0x678, 42);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;

        let mut regs = regs();
        memory.word(regs.esp + 4, actor);
        memory.word(regs.esp + 8, target);
        let saved = regs;
        let mut guest = TestGuest::default();
        assert!(run(0x26d8b0, &mut memory, &mut guest, &mut regs));

        assert_eq!(memory.read32(extension + 0xac8), u32::MAX);
        assert_eq!(memory.read32(extension + 0x684), 42);
        assert_eq!(memory.read32(extension + 0x9a8), 0);
        assert_eq!(memory.read32(extension + 0x9ac), 0);
        assert_eq!(memory.read32(extension + 0xb30), 0);
        assert_eq!(memory.read32(extension + 0x9d0), 6);
        assert_eq!(guest.calls, [0x267ff0]);
        assert_eq!(regs, saved);
        let live = LIVE_ACTORS.lock();
        let owner = live.store.state(identity).unwrap();
        assert_eq!(owner.local_state(), LocalState(1));
        assert_eq!(owner.pending_local_state(), Some(LocalState(6)));
        assert_eq!(
            owner.actor_navigation().state_six_route().requested_node,
            -1
        );
        assert_eq!(owner.actor_navigation().state_six_route().target_node, 42);
        drop(live);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn route_advance_keeps_route_progress_on_the_bound_actor_generation() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x2000;
        let extension = 0x3000;
        let route = 0x4000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x678, 5);
        memory.word(extension + 0x684, 9);
        memory.word(extension + 0x9a8, 7);
        memory.word(extension + 0x9ac, 8);
        memory.word(route + 12, 4);
        memory.word(route + 16, 77);
        publish_navigation_graph(&mut memory);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;

        let mut regs = regs();
        memory.word(regs.esp + 4, actor);
        memory.word(regs.esp + 8, route);
        let mut guest = TestGuest {
            resolved_route_node: 44,
            ..TestGuest::default()
        };
        assert!(run(0x26de30, &mut memory, &mut guest, &mut regs));
        assert_eq!(memory.read32(extension + 0x678), 5);
        assert_eq!(memory.read32(extension + 0x684), 44);
        assert_eq!(memory.read32(extension + 0x9a8), 0);
        assert_eq!(memory.read32(extension + 0x9ac), 0);
        let owned = LIVE_ACTORS.lock().store.navigation_route(identity).unwrap();
        assert_eq!(owned.current_node, NodeId(5));
        assert_eq!(owned.goal_node, NodeId(44));
        let live = LIVE_ACTORS.lock();
        let graph = live.store.navigation_graph_generation().unwrap();
        assert_eq!(
            live.store
                .state(identity)
                .unwrap()
                .actor_navigation()
                .graph_generation(),
            Some(graph)
        );
        drop(live);

        // A second route advance consumes the saved Rust state. A compatibility
        // write to the guest view cannot silently replace its current node.
        memory.word(extension + 0x678, 99);
        memory.word(extension + 0x684, 123);
        memory.word(extension + 0x9a8, 6);
        memory.word(extension + 0x9ac, 7);
        memory.word(route + 24, 4);
        memory.word(route + 28, 88);
        guest.resolved_route_node = 55;
        assert!(run(0x26de30, &mut memory, &mut guest, &mut regs));
        assert_eq!(memory.read32(extension + 0x678), 5);
        assert_eq!(memory.read32(extension + 0x684), 55);
        let owned = LIVE_ACTORS.lock().store.navigation_route(identity).unwrap();
        assert_eq!(owned.current_node, NodeId(5));
        assert_eq!(owned.goal_node, NodeId(55));
        assert_eq!(guest.calls, [0x9cc40, 0x9cc40, 0x9cc40, 0x9cc40]);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn route_advance_falls_back_without_a_rust_owned_graph_generation() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x2000;
        let extension = 0x3000;
        let route = 0x4000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x678, 5);
        memory.word(extension + 0x684, 9);
        memory.word(route + 12, 4);
        memory.word(route + 16, 77);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;

        let mut regs = regs();
        memory.word(regs.esp + 4, actor);
        memory.word(regs.esp + 8, route);
        regs.eax = 0x1234;
        let saved_regs = regs;
        let mut guest = TestGuest::default();

        assert!(!run(0x26de30, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs, saved_regs);
        assert_eq!(memory.read32(extension + 0x678), 5);
        assert_eq!(memory.read32(extension + 0x684), 9);
        assert!(guest.calls.is_empty());
        let live = LIVE_ACTORS.lock();
        let state = live.store.state(identity).unwrap();
        assert!(!state.actor_navigation().route_owned());
        assert_eq!(state.actor_navigation().graph_generation(), None);
        drop(live);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn route_advance_owns_kind16_intermediate_search_and_projects_result() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x2000;
        let extension = 0x3000;
        let route = 0x4000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x678, 5);
        memory.word(extension + 0x684, 9);
        memory.word(extension + 0x9a8, 7);
        memory.word(extension + 0x9ac, 8);
        memory.word(route + 12, 16);
        memory.word(route + 16, 123);
        memory.word(route + 24, 4);
        memory.word(route + 32, 77);
        publish_navigation_graph(&mut memory);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;

        let mut regs = regs();
        memory.word(regs.esp + 4, actor);
        memory.word(regs.esp + 8, route);
        let mut guest = TestGuest {
            resolved_route_node: 44,
            service_results: BTreeMap::from([(0x1596f0, 55)]),
            ..TestGuest::default()
        };
        assert!(run(0x26de30, &mut memory, &mut guest, &mut regs));

        let owned = LIVE_ACTORS.lock().store.navigation_route(identity).unwrap();
        assert_eq!(owned.current_node, NodeId(5));
        assert_eq!(owned.goal_node, NodeId(44));
        assert_eq!(owned.intermediate_node, Some(NodeId(55)));
        assert_eq!(memory.read32(extension + 0x9dc), 55);
        assert_eq!(memory.read32(extension + 0x684), 44);
        assert_eq!(guest.calls, [0x9cc40, 0x9cc40, 0x1596f0, 0x9cc40, 0x9cc40]);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn route_advance_aborts_negative_kind16_search_without_committing_goal() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x2000;
        let extension = 0x3000;
        let route = 0x4000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x678, 5);
        memory.word(extension + 0x684, 9);
        memory.word(extension + 0x9a8, 7);
        memory.word(extension + 0x9ac, 8);
        memory.word(extension + 0x9dc, 0x1234);
        memory.word(route + 12, 16);
        memory.word(route + 16, 123);
        memory.word(route + 24, 4);
        memory.word(route + 32, 77);
        publish_navigation_graph(&mut memory);
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;

        let mut regs = regs();
        memory.word(regs.esp + 4, actor);
        memory.word(regs.esp + 8, route);
        let mut guest = TestGuest {
            resolved_route_node: 44,
            service_results: BTreeMap::from([(0x1596f0, u32::MAX)]),
            ..TestGuest::default()
        };
        assert!(run(0x26de30, &mut memory, &mut guest, &mut regs));

        let owned = LIVE_ACTORS.lock().store.navigation_route(identity).unwrap();
        assert_eq!(owned.current_node, NodeId(5));
        assert_eq!(owned.goal_node, NodeId(9));
        assert_eq!(owned.reverse_trace_within, 7);
        assert_eq!(owned.reverse_trace_other, 8);
        assert_eq!(memory.read32(extension + 0x9dc), 0x1234);
        assert_eq!(guest.calls, [0x9cc40, 0x9cc40, 0x1596f0, 0x15d490]);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn route_advance_graph_root_clear_invalidates_only_on_the_measured_clear_path() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);
        assert!(!function_enabled(0x9a260));
        assert!(configure_function(0x9a260, true));

        let actor = 0x2000;
        let extension = 0x3000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        for (index, root) in super::NAVIGATION_GRAPH_ROOTS.into_iter().enumerate() {
            memory.word(root, 0x5000 + (index as u32 * 0x100));
        }
        let first_generation = LIVE_ACTORS.lock().store.replace_navigation_graph().unwrap();
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;
        LIVE_ACTORS
            .lock()
            .store
            .set_navigation_next_node(identity, NodeId(77))
            .unwrap();

        let mut regs = regs();
        let saved = regs;
        let mut guest = TestGuest::default();

        // The measured guard leaves all roots and the Rust graph untouched.
        memory.word(0x6b7aa8, 1);
        assert!(run(0x9a260, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs, saved);
        assert!(guest.calls.is_empty());
        for root in super::NAVIGATION_GRAPH_ROOTS {
            assert_ne!(memory.read32(root), 0);
        }
        assert_eq!(
            LIVE_ACTORS.lock().store.navigation_graph_generation(),
            Some(first_generation)
        );

        // With the guard clear, the four published roots are cleared and the
        // typed owner invalidates every graph-relative actor projection.
        memory.word(0x6b7aa8, 0);
        assert!(run(0x9a260, &mut memory, &mut guest, &mut regs));
        for root in super::NAVIGATION_GRAPH_ROOTS {
            assert_eq!(memory.read32(root), 0);
        }
        let next_generation = LIVE_ACTORS
            .lock()
            .store
            .navigation_graph_generation()
            .unwrap();
        assert_eq!(next_generation.0, first_generation.0 + 1);
        let live = LIVE_ACTORS.lock();
        let state = live.store.state(identity).unwrap();
        assert_eq!(
            state.actor_navigation().graph_generation(),
            Some(next_generation)
        );
        assert_eq!(state.actor_navigation().next_node(), None);
        drop(live);

        // Repeated clears over already-empty roots do not invent generations.
        assert!(run(0x9a260, &mut memory, &mut guest, &mut regs));
        assert_eq!(
            LIVE_ACTORS.lock().store.navigation_graph_generation(),
            Some(next_generation)
        );

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn changed_published_route_roots_invalidate_rust_route_projection() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x2000;
        let extension = 0x3000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        publish_navigation_graph(&mut memory);
        let first_generation = super::published_navigation_graph_generation(&memory).unwrap();
        actor_spawned(&memory, actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;
        LIVE_ACTORS
            .lock()
            .store
            .set_navigation_next_node(identity, NodeId(77))
            .unwrap();

        let replacement_root = super::NAVIGATION_GRAPH_ROOTS[0];
        memory.word(replacement_root, 0x7000);
        let second_generation = super::published_navigation_graph_generation(&memory).unwrap();
        assert_eq!(second_generation.0, first_generation.0 + 1);
        let live = LIVE_ACTORS.lock();
        let state = live.store.state(identity).unwrap();
        assert_eq!(
            state.actor_navigation().graph_generation(),
            Some(second_generation)
        );
        assert_eq!(state.actor_navigation().next_node(), None);
        drop(live);

        // The same published tuple is stable and does not mint generations.
        assert_eq!(
            super::published_navigation_graph_generation(&memory),
            Some(second_generation)
        );

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn nested_guest_call_supplies_return_slot_and_preserves_callee_saved_registers() {
        let _guard = TEST_LOCK.lock().unwrap();
        configure(true);
        let mut memory = Memory::default();
        let caller_esp = 0x1000;
        let actor = 0x2000;
        let route = 0x3000;
        let extension = 0x2800;
        memory.word(caller_esp, actor);
        memory.word(caller_esp + 4, route);
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0xaa0, 0);
        memory.word(actor + 0x7c + 0xaa0, 99);
        memory.word(route + 12, 0);

        let mut regs = ExtRegisters {
            eax: 0xaaaa,
            ecx: 0xbbbb,
            edx: 0xcccc,
            esp: caller_esp,
            ebx: 0x1111,
            ebp: 0x2222,
            esi: 0x3333,
            edi: 0x4444,
            eflags: 0x202,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };
        assert!(run_nested(
            0x26d690,
            &mut memory,
            &mut TestGuest::default(),
            &mut regs,
        ));
        assert_eq!(regs.eax, route + 12);
        assert_eq!(regs.esp, caller_esp);
        assert_eq!(
            (regs.ebx, regs.ebp, regs.esi, regs.edi),
            (0x1111, 0x2222, 0x3333, 0x4444)
        );
        assert_eq!(memory.read32(caller_esp - 4), 0);
        configure(false);
    }

    #[test]
    fn default_off_switch_routes_typed_motion_and_route_leaf_owners() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert!(!enabled());
        configure(false);
        assert!(!enabled());
        let mut memory = Memory::default();
        let esp = 0x1000;
        let actor = 0x2000;
        let target = 0x3000;
        memory.word(0x475d1c, 0x3c23_d70a);
        memory.word(esp + 4, actor);
        memory.word(esp + 8, target);
        memory.float(esp + 12, 0.25);
        memory.float(actor + 0x64, 12.0);
        memory.float(actor + 0x68, 99.0);
        memory.float(actor + 0x6c, -8.0);
        memory.float(target + 0x0c, 4.0);
        memory.float(target + 0x10, 0.0);
        memory.float(target + 0x14, 0.0);
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };
        regs.esp = esp;

        assert!(!run_without_services(0x266250, &mut memory, &mut regs));
        assert!(!run_without_services(0x26e100, &mut memory, &mut regs));
        assert!(!run_without_services(0x26de30, &mut memory, &mut regs));
        assert!(!run_without_services(0x177920, &mut memory, &mut regs));
        configure(true);
        assert!(run_without_services(0x266250, &mut memory, &mut regs));
        assert_eq!(f32::from_bits(memory.read32(actor + 0x64)), 10.0);
        assert_eq!(f32::from_bits(memory.read32(actor + 0x68)), 99.0);
        assert_eq!(f32::from_bits(memory.read32(actor + 0x6c)), -6.0);

        // T2370's absent-target scorer branch is complete without building a
        // navigation world; populated targets retain the lane implementation.
        let scorer_actor = 0xc000;
        let scorer_extension = 0xc100;
        memory.word(scorer_actor + 0x7c, scorer_extension);
        memory.word(esp + 4, scorer_actor);
        memory.word(scorer_extension + 4, 0);
        memory.word(scorer_extension + 0x678, 9);
        assert!(run_without_services(0x261de0, &mut memory, &mut regs));
        assert_eq!(regs.eax, u32::MAX);
        let scorer_target = 0xd000;
        let scorer_config = 0xe000;
        let scorer_group = 0xf000;
        let scorer_node = 0x11000;
        memory.word(scorer_extension + 4, scorer_target);
        memory.word(scorer_extension + 0x678, 1);
        memory.word(scorer_config + 0x14, 2);
        memory.word(scorer_config + 0x18, u32::MAX);
        memory.word(0x7b6770 + 2 * 4, scorer_group);
        memory.word(scorer_group, 2);
        memory.word(scorer_group + 24, 1);
        memory.word(scorer_group + 28, 2);
        memory.word(0x74c37c, 0);
        memory.float(scorer_actor + 0x64, 0.0);
        memory.float(scorer_actor + 0x6c, 0.0);
        memory.float(scorer_target + 0x64, 10.0);
        memory.float(scorer_target + 0x6c, 0.0);
        memory.float(scorer_node + 0x0c, 5.0);
        memory.float(scorer_node + 0x14, 0.0);
        regs.ecx = scorer_config;
        let mut guest = TestGuest {
            node_records: vec![scorer_node],
            ..TestGuest::default()
        };
        assert!(super::run(0x261de0, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 2);
        assert_eq!(guest.calls, [0x9a420]);
        memory.word(0x475d1c, 0x3c23_d709);
        assert!(!run_without_services(0x266250, &mut memory, &mut regs));
        memory.word(esp + 4, actor);
        let route = 0x4000;
        let cursor = 3;
        memory.word(esp + 8, route);
        let route_extension = 0x2800;
        memory.word(actor + 0x7c, route_extension);
        memory.word(route_extension + 0xaa0, cursor);
        // Conflicting inline data guards the object+0x7c pointer seam.
        memory.word(actor + 0x7c + 0xaa0, 0);
        super::actor_spawned(&memory, actor);
        let next = route + (cursor + 1) * 12;
        memory.word(next, 13);
        memory.word(next + 4, 2);
        assert!(run_without_services(0x26d690, &mut memory, &mut regs));
        assert_eq!(regs.eax, route + (cursor - 2 + 1) * 12);
        assert_eq!(memory.read32(route_extension + 0xaa0), cursor);
        assert_eq!(stored_behavior_route_cursor(route_extension), Some(cursor));

        // Within the tick, the typed cursor remains authoritative even if the
        // guest compatibility field is changed before another selected leaf.
        memory.word(route_extension + 0xaa0, 99);

        for (kind, payload, index) in [(12, 0, 1), (4, 0, cursor + 1)] {
            memory.word(next, kind);
            memory.word(next + 4, payload);
            assert!(run_without_services(0x26d690, &mut memory, &mut regs));
            assert_eq!(regs.eax, route + index * 12);
            assert_eq!(stored_behavior_route_cursor(route_extension), Some(cursor));
        }

        // 0x26E100 is default-on since the corrected r5 replay.
        assert!(function_enabled(0x26e100));
        memory.word(next, 13);
        memory.word(next + 4, 2);
        memory.word(route + (cursor - 1) * 12, 17);
        assert!(run_without_services(0x26e100, &mut memory, &mut regs));
        assert_eq!(regs.eax, 17);
        assert_eq!(stored_behavior_route_cursor(route_extension), Some(cursor));
        assert!(configure_function(0x26e100, false));

        // The shared route program owns ordinary advancement and the
        // kind-16/17 intermediate search. The latter's 0x1596F0 call shape is
        // INFERRED; adjacent Tier 0 tests cover positive/negative results.
        memory.word(next, 4);
        memory.word(next + 4, 77);
        memory.word(route_extension + 0x678, 5);
        memory.word(route_extension + 0x684, 9);
        memory.word(route_extension + 0x9a8, 7);
        memory.word(route_extension + 0x9ac, 8);
        publish_navigation_graph(&mut memory);
        let mut route_guest = TestGuest {
            resolved_route_node: 44,
            ..TestGuest::default()
        };
        assert!(super::run(
            0x26de30,
            &mut memory,
            &mut route_guest,
            &mut regs
        ));
        assert_eq!(route_guest.calls, [0x9cc40, 0x9cc40]);
        assert_eq!(regs.eax, 44);
        assert_eq!(memory.read32(route_extension + 0xaa0), cursor + 1);
        assert_eq!(memory.read32(route_extension + 0x684), 44);
        assert_eq!(memory.read32(route_extension + 0x9a8), 0);
        assert_eq!(memory.read32(route_extension + 0x9ac), 0);
        memory.word(route_extension + 0xaa0, cursor);
        super::set_behavior_route_cursor(route_extension, cursor);
        memory.word(next, 16);
        memory.word(next + 12, 4);
        memory.word(next + 16, 0);
        memory.word(next + 20, 77);
        route_guest.service_results.insert(0x1596f0, 55);
        assert!(super::run(
            0x26de30,
            &mut memory,
            &mut route_guest,
            &mut regs
        ));
        assert_eq!(
            route_guest.calls,
            [
                0x9cc40, 0x9cc40, 0x9cc40, 0x9cc40, 0x1596f0, 0x9cc40, 0x9cc40
            ]
        );
        assert_eq!(memory.read32(route_extension + 0x9dc), 55);
        assert_eq!(
            stored_behavior_route_cursor(route_extension),
            Some(cursor + 1)
        );
        assert_eq!(memory.read32(route_extension + 0xaa0), cursor + 1);
        super::actor_unregistered(route_extension);

        // T2393's measured fallback direction route reads the retained yaw
        // table. Other direction-source branches must stay on the lane path.
        let geometry_actor = 0x5000;
        let geometry_ext = 0x6000;
        let geometry_target = 0x7000;
        memory.word(esp + 4, geometry_actor);
        memory.word(esp + 8, geometry_target);
        memory.float(esp + 12, 0.5);
        memory.word(geometry_actor + 0x7c, geometry_ext);
        memory.word(geometry_ext + 0x48, 0);
        memory.word(geometry_ext + 0x4a, 0);
        memory.float(geometry_actor + 0x64, 0.0);
        memory.float(geometry_actor + 0x68, 0.0);
        memory.float(geometry_actor + 0x6c, 0.0);
        memory.float(geometry_actor + 0x70, 0.0);
        memory.word(geometry_actor + 0x94, 0);
        memory.word(geometry_actor + 0x10, 0);
        memory.float(0x53a3e4, 1.0);
        memory.float(geometry_target, 10.0);
        memory.float(geometry_target + 4, 0.0);
        memory.float(geometry_target + 8, 0.0);
        memory.float(0x7a38a0, 1.0);
        memory.float(0x7a38a0 + 0x200 * 4, 0.0);
        assert!(function_enabled(0x161730));
        assert!(run_without_services(0x161730, &mut memory, &mut regs));
        assert_eq!(regs.eax, 1);
        memory.word(geometry_ext + 0x4a, 1);
        assert!(!run_without_services(0x161730, &mut memory, &mut regs));

        // T2385's executor owner handles the complete kind-12 reset branch.
        let executor_actor = 0x8000;
        let executor_extension = 0x8100;
        let executor_route = 0x9000;
        memory.word(executor_actor + 0x7c, executor_extension);
        let executor_cursor = executor_extension + 0xaa0;
        memory.word(executor_extension + 0x28, 0);
        memory.word(executor_cursor, 3);
        memory.word(executor_route + 3 * 12, 12);
        regs.eax = executor_actor;
        regs.ebx = executor_route;
        assert!(run_without_services(0x26e150, &mut memory, &mut regs));
        assert_eq!(memory.read32(executor_cursor), 1);
        let executor_identity = {
            let live = LIVE_ACTORS.lock();
            live.bindings
                .iter()
                .find(|(bound, _)| *bound == executor_extension)
                .map(|(_, identity)| *identity)
                .unwrap()
        };
        assert_eq!(
            LIVE_ACTORS
                .lock()
                .store
                .behavior_route_cursor(executor_identity),
            Ok(Some(1))
        );
        memory.word(executor_extension + 0x28, 0x0010_0000);
        memory.word(executor_cursor, 3);
        assert!(!run_without_services(0x26e150, &mut memory, &mut regs));
        assert_eq!(memory.read32(executor_cursor), 3);
        assert_eq!(
            LIVE_ACTORS
                .lock()
                .store
                .behavior_route_cursor(executor_identity),
            Ok(None)
        );

        // T2395's link gate must return before dereferencing EBX's route.
        let link_actor = 0xa000;
        let link_extension = 0xa100;
        let link_behavior = 0xb000;
        memory.word(link_actor + 0x7c, link_extension);
        memory.word(link_extension + 0xbf0, link_behavior);
        memory.word(link_behavior + 0x68, (-2i32) as u32);
        regs.edi = link_actor;
        regs.ebx = 0; // invalid route is safe only on the documented early return
        assert!(run_without_services(0x263ed0, &mut memory, &mut regs));
        memory.word(link_behavior + 0x68, 0);
        let link_route = 0xc000;
        let primary_table = 0xd000;
        regs.ebx = link_route;
        memory.word(link_route, 0);
        let mut guest = TestGuest::default();
        assert!(super::run(0x263ed0, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x26de30]);
        memory.word(link_route, 4);
        memory.word(link_extension + 0x684, 2);
        memory.float(link_actor + 0x64, 1.0);
        memory.float(link_actor + 0x68, 2.0);
        memory.float(link_actor + 0x6c, 3.0);
        memory.word(0x715f2c, primary_table);
        memory.float(primary_table + 2 * 32 + 0x0c, 20.0);
        memory.float(primary_table + 2 * 32 + 0x10, 21.0);
        memory.float(primary_table + 2 * 32 + 0x14, 3.0);
        let mut guest = TestGuest::default();
        assert!(super::run(0x263ed0, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x59d20]);
        assert_eq!(f32::from_bits(memory.read32(link_extension + 0x30)), 1.0);
        assert_eq!(f32::from_bits(memory.read32(link_extension + 0x34)), 2.0);
        assert_eq!(f32::from_bits(memory.read32(link_extension + 0x38)), 3.0);
        {
            let live = super::LIVE_ACTORS.lock();
            let identity = live
                .bindings
                .iter()
                .find(|(bound, _)| *bound == link_extension)
                .map(|(_, identity)| *identity)
                .expect("link follower binds the active actor generation");
            assert_eq!(
                live.store
                    .state(identity)
                    .unwrap()
                    .actor_navigation()
                    .movement_target(),
                Some(Position3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                })
            );
        }
        let mut guest = TestGuest {
            animation_match: true,
            ..TestGuest::default()
        };
        assert!(super::run(0x263ed0, &mut memory, &mut guest, &mut regs));
        assert_eq!(f32::from_bits(memory.read32(link_extension + 0x30)), 20.0);
        assert_eq!(f32::from_bits(memory.read32(link_extension + 0x34)), 21.0);
        assert_eq!(f32::from_bits(memory.read32(link_extension + 0x38)), 3.0);

        // Route-link kinds 0/1 now share the typed decision owner. Kind 0
        // delegates route advancement; kind 1 samples only endpoint root Z,
        // then either asks navigation to advance or advances the current ID.
        memory.word(link_route, 0);
        let mut guest = TestGuest::default();
        assert!(super::run(0x263ed0, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x26de30]);

        memory.word(link_route, 1);
        memory.word(0x7e3954, 0x13000);
        memory.word(0x13000, 0x14000);
        memory.word(0x14010, 3);
        memory.float(link_extension + 0xb08, 1.0);
        let first_output = esp - 0x100 + 0x20;
        let last_output = esp - 0x100 + 0x40;
        memory.float(first_output + 8, 0.0);
        memory.float(last_output + 8, 1.0);
        let mut guest = TestGuest::default();
        assert!(super::run(0x263ed0, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x59320, 0x59320, 0x15d500]);
        assert_eq!(regs.esp, esp);

        memory.float(primary_table + 2 * 32 + 0x0c, 1.5);
        memory.float(last_output + 8, 2.0);
        let mut guest = TestGuest::default();
        assert!(super::run(0x263ed0, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x59320, 0x59320, 0x26de30]);
        assert_eq!(memory.read32(link_extension + 0x678), 2);

        // T2375's source loop preserves reset, nested alert, registration and
        // count-update order while the reentrant selector-1 call is delegated.
        let source = 0x78b2e0;
        memory.word(esp + 4, 1);
        memory.word(0x78b520, 1);
        memory.word(source + 4, 2);
        memory.word(source + 8, 1);
        memory.word(source + 0x10, 0xde00);
        memory.word(source + 0x14, 0);
        memory.float(source + 0x18, 4.5);
        memory.word(0x7a2b80, 7);
        let registers_before = regs;
        let mut guest = TestGuest::default();
        assert!(super::run(0x25faf0, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x349e20, 0x267ff0, 0xa0640]);
        assert_eq!(memory.read32(source + 4), 2);
        assert_eq!(memory.read32(source + 8), 1);
        assert_eq!(memory.read32(source + 0x10), 0);
        assert_eq!(memory.read32(source + 0x14), 1);
        assert_eq!(memory.read32(source + 0x18), 0.0f32.to_bits());
        assert_eq!(memory.read32(0x7a2b80), 8);
        assert_eq!(regs, registers_before);

        // T2395's mechanism owner resolves valid identity and clears only a
        // stale handle, while a zero handle remains untouched.
        assert!(configure_function(0x12f00, true));
        let handle_address = 0xd100;
        let handle = (2u32 << 22) | 0x12345;
        let pool = 0x0010_0000;
        let pool_entry = pool + 2 * 0x2e8;
        memory.word(0x72f5e8, pool);
        memory.word(esp + 4, handle_address);
        memory.word(handle_address, handle);
        memory.word(pool_entry + 0x2c, handle);
        regs.eflags = 0x0000_0fff;
        regs.ecx = 0xcccc_cccc;
        regs.edx = 0xdddd_dddd;
        assert!(run_without_services(0x12f00, &mut memory, &mut regs));
        assert_eq!(regs.eax, pool_entry);
        assert_eq!(regs.edx, handle_address);
        assert_eq!(regs.ecx, handle);
        assert_eq!(regs.eflags & 0x8c5, 0x44);
        assert_eq!(memory.read32(handle_address), handle);
        memory.word(pool_entry + 0x2c, handle ^ 1);
        assert!(run_without_services(0x12f00, &mut memory, &mut regs));
        assert_eq!(regs.eax, 0);
        assert_eq!(regs.edx, handle_address);
        assert_eq!(regs.ecx, handle);
        assert_eq!(regs.eflags & 0x8c5, 0x44);
        assert_eq!(memory.read32(handle_address), 0);
        memory.word(handle_address, 0);
        assert!(run_without_services(0x12f00, &mut memory, &mut regs));
        assert_eq!(regs.eax, 0);
        assert_eq!(regs.edx, handle_address);
        assert_eq!(regs.ecx, 0);
        assert_eq!(regs.eflags & 0x8c5, 0x44);

        // A live boundary must decline a wild caller-owned handle pointer
        // before asking the guest-memory implementation to dereference it.
        let wild_handle_address = 0xc1b5_b33c;
        memory.word(esp + 4, wild_handle_address);
        let registers_before = regs;
        assert!(!run_without_services(0x12f00, &mut memory, &mut regs));
        assert_eq!(regs, registers_before);

        // The borrowed mechanism subrecord helper is a direct Rust boundary:
        // it returns the documented record address without taking ownership.
        let mechanism_object = 0xd200;
        let mechanism_extension = 0xd300;
        memory.word(esp + 4, mechanism_object);
        memory.word(mechanism_object + 0x7c, mechanism_extension);
        assert!(run_without_services(0x177920, &mut memory, &mut regs));
        assert_eq!(regs.eax, mechanism_extension + 0x568);
        assert!(run_without_services(0x12f00, &mut memory, &mut regs));
        assert_eq!(regs.eax, 0);
        assert_eq!(memory.read32(handle_address), 0);

        // The delegated-vector source stays on its original guest helpers,
        // while the shared Rust owner computes the geometry and strict gate.
        let delegated_output = esp - 0x100 + 0x20;
        memory.word(esp + 4, geometry_actor);
        memory.word(esp + 8, geometry_target);
        memory.word(geometry_ext + 0x4a, 0x4a);
        memory.float(delegated_output, 1.0);
        memory.float(delegated_output + 4, 0.0);
        memory.float(delegated_output + 8, 0.0);
        let mut guest = TestGuest {
            delegated_direction: true,
            ..TestGuest::default()
        };
        assert!(super::run(0x161730, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 1);
        assert_eq!(guest.calls, [0xd6ea0, 0x3797b0]);
        configure(false);
    }

    #[test]
    fn per_function_exclusion_preserves_other_adapters() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut memory = Memory::default();
        let esp = 0x1000;
        let object = 0x4000;
        let extension = 0x5000;
        memory.word(esp + 4, object);
        memory.word(object + 0x7c, extension);
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };

        configure(true);
        for va in [0x26e100, 0x161730] {
            assert!(function_enabled(va));
            assert!(configure_function(va, false));
            assert!(!run_without_services(va, &mut memory, &mut regs));
        }
        assert!(function_enabled(0x12f00));
        assert!(configure_function(0x12f00, true));
        assert!(function_enabled(0x12f00));
        assert!(run_without_services(0x12f00, &mut memory, &mut regs));
        assert!(configure_function(0x12f00, false));
        assert!(!run_without_services(0x12f00, &mut memory, &mut regs));
        assert!(configure_function(0x266250, false));
        assert!(!run_without_services(0x266250, &mut memory, &mut regs));
        assert!(run_without_services(0x177920, &mut memory, &mut regs));
        assert_eq!(regs.eax, extension + 0x568);
        assert!(!configure_function(0xdeadbeef, false));
        configure(false);
    }

    #[test]
    fn mechanism_tick_adopts_selected_states_and_preserves_service_order() {
        let _guard = TEST_LOCK.lock().unwrap();
        let actor = 0x18000;
        let extension = 0x19000;
        let esp = 0x1000;
        let mut memory = Memory::default();
        memory.word(esp + 4, actor);
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x20, 0);
        memory.word(extension + 0x9d0, u32::MAX);
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };
        configure(false);
        let mut guest = TestGuest::default();
        assert!(!super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert!(guest.calls.is_empty());
        configure(true);

        memory.word(extension + 0xc08, 0);
        memory.word(0x4e8cec, 60);
        memory.float(0x475c78, 1.0);
        memory.float(0x4e794c, 1.0);
        // A missing state-0 handle takes the owner's selected-state transition
        // to 14 after the shared eligibility/pending/timestep/nav prologue.
        let mut guest = TestGuest::default();
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x15e120, 0x15d470]);
        assert_eq!(memory.read32(extension + 0x20), 14);
        assert_eq!(memory.read32(extension + 0x9d4), 0);
        assert_eq!(memory.read32(extension + 0x9d0), 14);
        assert_eq!(memory.read32(extension + 0x9d8), 0);

        // State 0 calls the interaction and classification owners in order;
        // class 1 enters local state 11 without touching the mechanism record.
        let handle = (2u32 << 22) | 0x12345;
        let mechanism_pool = 0x0010_0000;
        let mechanism = mechanism_pool + 2 * 0x2e8;
        memory.word(0x72f5e8, mechanism_pool);
        let mechanism_extension = 0x1a000;
        memory.word(extension + 0x20, 0);
        memory.word(extension + 0x9d4, 0);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 0xc08, handle);
        memory.word(mechanism + 0x2c, handle);
        memory.word(mechanism + 0x7c, mechanism_extension);
        memory.float(mechanism_extension + 0x568 + 0x394, 3.5);
        let mut guest = TestGuest {
            service_results: [(0x17e010, 2), (0x17a0a0, 1)].into(),
            ..TestGuest::default()
        };
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x15e120, 0x15d470, 0x17e010, 0x17a0a0]);
        assert_eq!(memory.read32(extension + 0x20), 11);
        assert_eq!(memory.read32(extension + 0x9d4), 0);
        assert_eq!(memory.read32(extension + 0x9d0), 11);
        assert_eq!(
            f32::from_bits(memory.read32(mechanism_extension + 0x568 + 0x394)),
            3.5
        );

        // Classification zero writes the borrowed record's literal 8.0 before
        // consulting the state-8 predicate and mechanism count.
        memory.word(extension + 0x20, 0);
        memory.word(extension + 0x9d4, 0);
        memory.word(extension + 0x9d0, u32::MAX);
        let mut guest = TestGuest {
            service_results: [(0x17e010, 2), (0x17a0a0, 0), (0x29fe20, 1), (0x17a200, 1)].into(),
            ..TestGuest::default()
        };
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(
            guest.calls,
            [0x15e120, 0x15d470, 0x17e010, 0x17a0a0, 0x29fe20, 0x17a200]
        );
        assert_eq!(memory.read32(extension + 0x20), 8);
        assert_eq!(
            f32::from_bits(memory.read32(mechanism_extension + 0x568 + 0x394)),
            8.0
        );

        // State 1 resolves the borrowed record before interaction and selects
        // local state 4 only when the interaction result is 2.
        memory.word(extension + 0x20, 1);
        memory.word(extension + 0x9d4, 0);
        memory.word(extension + 0x9d0, u32::MAX);
        let mut guest = TestGuest {
            service_results: [(0x17e010, 2)].into(),
            ..TestGuest::default()
        };
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x15e120, 0x15d470, 0x177920, 0x17e010]);
        assert_eq!(memory.read32(extension + 0x20), 4);

        // State 14 uses the typed projection and queue boundary in source
        // order, preserving the actor target when the same extension controls it.
        memory.word(extension + 0x20, 14);
        // State 1 above queued state 4. This fixture directly selects state 14,
        // so clear that pending transition just as the next tick prologue does.
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 4, 0x1b000);
        memory.word(actor + 0x34, 0xffff);
        memory.float(actor + 0x64, 1.0);
        memory.float(actor + 0x68, 2.0);
        memory.float(actor + 0x6c, 3.0);
        let node_record = 0x1c000;
        memory.write8(node_record + 0xa, 0xfe);
        memory.write8(node_record + 0xb, 0xff);
        let mut guest = TestGuest {
            service_results: [(0x9bcf0, 7), (0x277fe0, extension)].into(),
            node_records: vec![node_record],
            ..TestGuest::default()
        };
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(
            guest.calls,
            [
                0x15e120, 0x15d470, 0x9bcf0, 0x9a420, 0x277fe0, 0x17570, 0x15f0d0
            ]
        );
        assert_eq!(memory.read32(extension + 0x678), 7);
        assert_eq!(memory.read16(actor + 0x34), 0xfffe);
        assert_eq!(memory.read32(extension + 4), 0x1b000);

        // When a different extension owns control, state 14 clears the target
        // before queuing behavior mask 4 and refreshing the actor.
        memory.word(extension + 4, 0x1b000);
        let node_record = 0x1d000;
        memory.write8(node_record + 0xa, 3);
        memory.write8(node_record + 0xb, 0);
        let mut guest = TestGuest {
            service_results: [(0x9bcf0, 8), (0x277fe0, 0)].into(),
            node_records: vec![node_record],
            ..TestGuest::default()
        };
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(memory.read32(extension + 0x678), 8);
        assert_eq!(memory.read16(actor + 0x34), 3);
        assert_eq!(memory.read32(extension + 4), 0);
        assert_eq!(guest.calls.last(), Some(&0x15f0d0));

        // T2385 extension (Tier 0): state 3 gates on 0x177E90, begins via 0x177F20,
        // sets record flag bit 1 and selects state 4 on interaction result 2.
        memory.word(extension + 0x20, 3);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(mechanism_extension + 0x568 + 0x390, 0);
        let mut guest = TestGuest {
            service_results: [(0x177e90, 1), (0x17e010, 2)].into(),
            ..TestGuest::default()
        };
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(
            guest.calls,
            [0x15e120, 0x15d470, 0x177e90, 0x177f20, 0x17e010]
        );
        assert_eq!(memory.read32(extension + 0x20), 4);
        assert_eq!(memory.read32(mechanism_extension + 0x568 + 0x390), 2);

        // State 9 with no target, a set predicate, clear exit bit and no link
        // record requests state 13 after the ordered service calls.
        memory.word(extension + 0x20, 9);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 4, 0);
        let mut guest = TestGuest {
            service_results: [(0x29fe20, 1)].into(),
            ..TestGuest::default()
        };
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(
            guest.calls,
            [
                0x15e120, 0x15d470, 0x29fe60, 0x17e010, 0x29fe20, 0x177950, 0x179f70
            ]
        );
        assert_eq!(memory.read32(extension + 0x20), 13);

        // State 4 is Rust-owned too: interaction result 2 with flag bit 1 clear
        // requests state 6, after 0x29FE60 and the Rust speed ramp (no 0x172CD0 call).
        memory.word(extension + 0x20, 4);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(mechanism_extension + 0x568 + 0x390, 0);
        let mut guest = TestGuest {
            service_results: [(0x17e010, 2)].into(),
            ..TestGuest::default()
        };
        assert!(super::run(0x2a0500, &mut memory, &mut guest, &mut regs));
        assert_eq!(guest.calls, [0x15e120, 0x15d470, 0x29fe60, 0x17e010]);
        assert_eq!(memory.read32(extension + 0x20), 6);
        configure(false);
    }

    #[test]
    fn route_reset_falls_through_when_extension_is_seen_on_a_replacement_actor() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let first_actor = 0x2000;
        let replacement_actor = 0x3000;
        let extension = 0x4000;
        let route = 0x5000;
        let mut memory = Memory::default();
        memory.word(first_actor + 0x7c, extension);
        memory.word(replacement_actor + 0x7c, extension);
        memory.word(extension + 0x28, 0);
        memory.word(extension + 0xaa0, 2);
        memory.word(route + 2 * 12, 12);

        actor_spawned(&memory, first_actor);
        let identity = LIVE_ACTORS.lock().bindings[0].1;
        LIVE_ACTORS
            .lock()
            .store
            .set_behavior_route_cursor(identity, 2)
            .unwrap();
        actor_spawned(&memory, replacement_actor);

        let mut registers = regs();
        registers.eax = replacement_actor;
        registers.ebx = route;
        assert!(!run_without_services(0x26e150, &mut memory, &mut registers));
        assert_eq!(memory.read32(extension + 0xaa0), 2);
        assert_eq!(
            LIVE_ACTORS.lock().store.behavior_route_cursor(identity),
            Ok(Some(2))
        );
        assert!(
            LIVE_ACTORS
                .lock()
                .ambiguous_actor_extensions
                .contains(&extension)
        );

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn live_actor_store_tracks_registration_and_unregistration() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let object = 0x1000;
        let extension = 0x0073_56d8 + 3 * 0xea0;
        let mut memory = Memory::default();
        memory.word(object + 0x7c, extension);
        memory.word(GAME_MODE_BYTE, 10);
        memory.word(extension + 0x10, 0x40);
        memory.word(extension + 0x14, 1);
        memory.word(extension + 0x18, 0x80);
        memory.word(extension + 0x1c, 2);
        memory.word(extension, object);
        memory.word(extension + 8, 3);
        memory.word(extension + 0xaa8, 5);
        memory.word(extension + 0xaac, 6);
        memory.word(extension + 0x20, 7);
        memory.word(extension + 0x9d4, 8);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.write32(extension + 0x9d8, 0x7fc0_1234);

        actor_spawned(&memory, object);
        actor_spawned(&memory, object);
        let rows = actor_state_trace_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0.slot, 3);
        assert_eq!(rows[0].0.generation, 1);
        assert_eq!(rows[0].1.mode, BehaviorMode::Mode10);
        assert_eq!(rows[0].1.mode10_active, BehaviorPair { low: 0x40, high: 1 });
        assert_eq!(rows[0].1.other_active, BehaviorPair { low: 0x80, high: 2 });
        assert_eq!(rows[0].1.pending_behavior, BehaviorPair { low: 5, high: 6 });
        assert_eq!(rows[0].1.local_state, LocalState(7));
        assert_eq!(rows[0].1.pending_local_state, None);
        assert_eq!(
            rows[0].1.transition_progress,
            TransitionProgress(0x7fc0_1234)
        );

        actor_unregistered(extension);
        assert!(actor_state_trace_rows().is_empty());
        actor_spawned(&memory, object);
        let reused = actor_state_trace_rows();
        assert_eq!(reused.len(), 1);
        assert_eq!(reused[0].0.slot, 3);
        assert_eq!(reused[0].0.generation, 2);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn movement_roster_binds_preloaded_extensions_and_keeps_generation_on_reuse() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let first = 0x2000;
        let second = 0x3000;
        let third = 0x4000;
        let mut memory = Memory::default();
        memory.word(GAME_MODE_BYTE, 10);
        memory.word(CHARACTER_EXTENSION_COUNT, 4);
        memory.word(CHARACTER_EXTENSION_ARRAY, first);
        memory.word(CHARACTER_EXTENSION_ARRAY + 4, second);
        memory.word(CHARACTER_EXTENSION_ARRAY + 8, 0);
        memory.word(CHARACTER_EXTENSION_ARRAY + 12, first);

        assert!(bind_active_actor_roster(&memory));
        let first_bindings = LIVE_ACTORS.lock().bindings.clone();
        assert_eq!(first_bindings.len(), 2);
        let first_identity = first_bindings
            .iter()
            .find(|(extension, _)| *extension == first)
            .unwrap()
            .1;
        let second_identity = first_bindings
            .iter()
            .find(|(extension, _)| *extension == second)
            .unwrap()
            .1;
        assert_eq!((first_identity.slot, first_identity.generation), (0, 1));
        assert_eq!((second_identity.slot, second_identity.generation), (1, 1));

        // The explicit CharacterSet unlink hook retires the removed actor;
        // the next roster pass preserves the survivor and advances the reused
        // Rust slot generation for the newly listed extension.
        actor_unregistered(first);
        memory.word(CHARACTER_EXTENSION_COUNT, 2);
        memory.word(CHARACTER_EXTENSION_ARRAY, second);
        memory.word(CHARACTER_EXTENSION_ARRAY + 4, third);
        assert!(bind_active_actor_roster(&memory));
        let current = LIVE_ACTORS.lock().bindings.clone();
        assert_eq!(current.len(), 2);
        assert!(
            current.iter().any(|(extension, identity)| {
                *extension == second && *identity == second_identity
            })
        );
        assert!(current.iter().any(|(extension, identity)| {
            *extension == third && (identity.slot, identity.generation) == (0, 2)
        }));

        memory.word(CHARACTER_EXTENSION_COUNT, CHARACTER_EXTENSION_CAPACITY + 1);
        assert!(!bind_active_actor_roster(&memory));
        assert_eq!(LIVE_ACTORS.lock().bindings.len(), 2);

        configure(false);
        reset_actor_state();
    }

    /// T3478: the nested `0x26EFC0` seam reads the actor from `[esp]` (no return slot yet),
    /// never from the caller's previous argument slot at `[esp + 4]`.
    #[test]
    fn nested_controller_seam_reads_actor_from_first_argument_slot() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x1000;
        let extension = 0x2000;
        let scratch_actor = 0x3000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        // A lifted callee reused its own argument slot as scratch.
        memory.word(scratch_actor + 0x7c, 0x3100);
        let esp = 0x8000;
        memory.word(esp, actor);
        memory.word(esp + 4, scratch_actor);

        let token = actor_controller_started_at_nested_call(&mut memory, esp).unwrap();
        assert_eq!(token.extension, extension);
        let bound: Vec<u32> = LIVE_ACTORS
            .lock()
            .bindings
            .iter()
            .map(|(bound, _)| *bound)
            .collect();
        assert_eq!(bound, vec![extension]);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn controller_target_request_is_committed_before_lifted_controller_dispatch() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x1000;
        let extension = 0x2000;
        let mut memory = WriteTrackingMemory::default();
        memory.memory.word(actor + 0x7c, extension);
        memory.memory.word(extension + 0x20, 0x120);
        memory.memory.word(extension + 0x9d0, 0x240);
        memory.memory.word(extension + 0x9d4, 0x80);
        memory.memory.word(extension + 0x9d8, 0x3f00_0000);
        memory.writes.clear();

        let token = actor_controller_started(&mut memory, actor).unwrap();
        assert_eq!(
            memory
                .writes
                .chunks_exact(4)
                .map(|bytes| bytes[0])
                .collect::<Vec<_>>(),
            vec![
                extension + 0x9d8,
                extension + 0x9d4,
                extension + 0x20,
                extension + 0x9d0
            ]
        );
        assert_eq!(memory.memory.read32(extension + 0x20), 0x240);
        assert_eq!(memory.memory.read32(extension + 0x9d0), u32::MAX);
        assert_eq!(memory.memory.read32(extension + 0x9d4), 0x120);
        assert_eq!(memory.memory.read32(extension + 0x9d8), 0);

        let handoff = LIVE_ACTORS
            .lock()
            .store
            .controller_target_handoff(token.identity)
            .unwrap()
            .unwrap();
        assert_eq!(handoff.current(), 0x240);
        assert_eq!(handoff.pending(), -1);
        assert_eq!(handoff.previous(), 0x120);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn controller_boundary_preserves_cached_route_record_as_opaque_state() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x1100;
        let extension = 0x2200;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x67c, 0);

        let token = actor_controller_started(&mut memory, actor).unwrap();
        assert_eq!(
            LIVE_ACTORS
                .lock()
                .store
                .state(token.identity)
                .unwrap()
                .actor_navigation()
                .cached_route_record(),
            Some(CachedNavigationRecord(0))
        );

        // Preserve a changed compatibility-view token after the bounded
        // controller call without assigning it node or generation semantics.
        memory.word(extension + 0x67c, 0x8bad_f00d);
        actor_controller_finished(&memory, Some(token));
        assert_eq!(
            LIVE_ACTORS
                .lock()
                .store
                .state(token.identity)
                .unwrap()
                .actor_navigation()
                .cached_route_record(),
            Some(CachedNavigationRecord(0x8bad_f00d))
        );

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn controller_return_imports_only_measured_selected_goal_writeback() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x1000;
        let extension = 0x2000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x20, 0x120);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 0x9d4, 0x80);
        memory.word(extension + 0x9d8, 0x3f00_0000);

        let token = actor_controller_started(&mut memory, actor).unwrap();
        assert_eq!(token.target_before, 0x120);

        // One of the measured selected-goal branches writes the same four
        // fields before the controller returns.
        memory.word(extension + 0x20, 0x360);
        memory.word(extension + 0x9d0, 0x360);
        memory.word(extension + 0x9d4, 0x120);
        memory.word(extension + 0x9d8, 0);
        actor_controller_finished(&memory, Some(token));

        let live = LIVE_ACTORS.lock();
        let handoff = live
            .store
            .controller_target_handoff(token.identity)
            .unwrap()
            .unwrap();
        assert_eq!(handoff.current(), 0x360);
        assert_eq!(handoff.pending(), 0x360);
        assert_eq!(handoff.previous(), 0x120);
        assert_eq!(
            live.store
                .state(token.identity)
                .unwrap()
                .transition_progress(),
            TransitionProgress(0)
        );
        drop(live);

        // A later controller entry retires the equal pending marker without
        // resetting progress a second time.
        memory.word(extension + 0x9d8, 0x55);
        let next = actor_controller_started(&mut memory, actor).unwrap();
        assert_eq!(memory.read32(extension + 0x9d0), u32::MAX);
        assert_eq!(memory.read32(extension + 0x9d8), 0x55);
        actor_controller_finished(&memory, Some(next));
        assert_eq!(
            LIVE_ACTORS
                .lock()
                .store
                .state(token.identity)
                .unwrap()
                .transition_progress(),
            TransitionProgress(0x55)
        );

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn controller_return_imports_changed_route_and_movement_fields() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x1000;
        let extension = 0x2000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x678, 2);
        memory.word(extension + 0x680, 3);
        memory.word(extension + 0x684, 4);
        memory.word(extension + 0x9a8, 5);
        memory.word(extension + 0x9ac, 6);
        memory.float(extension + 0x30, 1.0);
        memory.float(extension + 0x34, 2.0);
        memory.float(extension + 0x38, 3.0);

        let token = actor_controller_started(&mut memory, actor).unwrap();
        assert_eq!(
            LIVE_ACTORS.lock().store.navigation_route(token.identity),
            Ok(RouteActor {
                current_node: NodeId(2),
                goal_node: NodeId(4),
                intermediate_node: None,
                reverse_trace_within: 5,
                reverse_trace_other: 6,
            })
        );

        // The measured route/movement stages update their compatibility fields
        // before the outer controller returns.
        memory.word(extension + 0x678, 7);
        memory.word(extension + 0x680, 8);
        memory.word(extension + 0x684, 9);
        memory.word(extension + 0x9a8, 10);
        memory.word(extension + 0x9ac, 11);
        memory.float(extension + 0x30, 12.0);
        memory.float(extension + 0x34, 13.0);
        memory.float(extension + 0x38, 14.0);
        actor_controller_finished(&memory, Some(token));

        let live = LIVE_ACTORS.lock();
        assert_eq!(
            live.store.navigation_route(token.identity),
            Ok(RouteActor {
                current_node: NodeId(7),
                goal_node: NodeId(9),
                intermediate_node: None,
                reverse_trace_within: 10,
                reverse_trace_other: 11,
            })
        );
        let navigation = live.store.state(token.identity).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), Some(NodeId(8)));
        assert_eq!(
            navigation.movement_target(),
            Some(Position3 {
                x: 12.0,
                y: 13.0,
                z: 14.0,
            })
        );
        drop(live);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn controller_return_skips_route_writeback_after_graph_replacement() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x1000;
        let extension = 0x2000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x678, 2);
        memory.word(extension + 0x680, 3);
        memory.word(extension + 0x684, 4);
        memory.float(extension + 0x30, 1.0);
        memory.float(extension + 0x34, 2.0);
        memory.float(extension + 0x38, 3.0);
        let token = actor_controller_started(&mut memory, actor).unwrap();

        let replacement = LIVE_ACTORS.lock().store.replace_navigation_graph().unwrap();
        memory.word(extension + 0x678, 7);
        memory.word(extension + 0x680, 8);
        memory.word(extension + 0x684, 9);
        memory.float(extension + 0x30, 12.0);
        memory.float(extension + 0x34, 13.0);
        memory.float(extension + 0x38, 14.0);
        actor_controller_finished(&memory, Some(token));

        let live = LIVE_ACTORS.lock();
        let state = live.store.state(token.identity).unwrap();
        assert_eq!(
            state.actor_navigation().graph_generation(),
            Some(replacement)
        );
        assert!(!state.navigation_route_owned());
        assert_eq!(state.actor_navigation().next_node(), None);
        assert_eq!(state.actor_navigation().movement_target(), None);
        drop(live);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn controller_return_does_not_import_an_unmatched_target_writeback() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let actor = 0x1000;
        let extension = 0x2000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, extension);
        memory.word(extension + 0x20, 0x120);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 0x9d4, 0x80);

        let token = actor_controller_started(&mut memory, actor).unwrap();
        memory.word(extension + 0x20, 0x360);
        memory.word(extension + 0x9d4, 0x120);
        memory.word(extension + 0x9d0, u32::MAX);
        actor_controller_finished(&memory, Some(token));

        let handoff = LIVE_ACTORS
            .lock()
            .store
            .controller_target_handoff(token.identity)
            .unwrap()
            .unwrap();
        assert_eq!(handoff.current(), 0x120);
        assert_eq!(handoff.pending(), -1);
        assert_eq!(handoff.previous(), 0x80);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn controller_boundary_uses_actor_handle_and_rejects_stale_generation() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let first = 0x2000;
        let actor = 0x1000;
        let target = 0x4000;
        let mut memory = Memory::default();
        memory.word(actor + 0x7c, first);
        memory.word(GAME_MODE_BYTE, 10);
        memory.word(first + 0x10, 0x40);
        memory.word(0x475d1c, 0x3c23_d70a);
        memory.word(actor + 0x64, 0x3f800000);
        memory.word(actor + 0x68, 0x40000000);
        memory.word(actor + 0x6c, 0x40400000);
        memory.word(target + 0x0c, 0x40800000);
        memory.word(target + 0x10, 0x40a00000);
        memory.word(target + 0x14, 0x40c00000);
        let mut regs = regs();
        memory.word(regs.esp + 4, actor);
        memory.word(regs.esp + 8, target);
        memory.float(regs.esp + 12, 0.5);

        assert!(run_without_services(0x266250, &mut memory, &mut regs));
        assert_eq!(regs.eax, actor);
        let initial = actor_state_trace_rows();
        assert_eq!(initial.len(), 1);
        let first_identity = initial[0].0;
        assert_eq!(first_identity.generation, 1);
        let first_controller = actor_controller_started(&mut memory, actor).unwrap();
        assert_eq!(first_controller.identity, first_identity);

        // The movement leaf is inside the controller, so its return must not
        // publish a partial actor snapshot.
        memory.word(first + 0x14, 1);
        memory.word(first + 0x18, 2);
        let during_controller = actor_state_trace_rows()
            .into_iter()
            .find(|(identity, _)| *identity == first_identity)
            .unwrap()
            .1;
        assert_eq!(during_controller.mode10_active.high, 0);
        assert_eq!(during_controller.other_active.low, 0);

        // `0x26EFC0` returns after executor, arrival, and post-stage decisions;
        // refresh the compatibility projection only at that actor's boundary.
        actor_controller_finished(&memory, Some(first_controller));
        let refreshed = actor_state_trace_rows()
            .into_iter()
            .find(|(identity, _)| *identity == first_identity)
            .unwrap()
            .1;
        assert_eq!(refreshed.mode10_active.high, 1);
        assert_eq!(refreshed.other_active.low, 2);

        // Later lifted writes become visible at the next full controller edge.
        memory.word(first + 0x14, 4);
        memory.word(first + 0x18, 3);
        let second_controller = actor_controller_started(&mut memory, actor).unwrap();
        actor_controller_finished(&memory, Some(second_controller));
        let completed = actor_state_trace_rows()
            .into_iter()
            .find(|(identity, _)| *identity == first_identity)
            .unwrap()
            .1;
        assert_eq!(completed.mode10_active.high, 4);
        assert_eq!(completed.other_active.low, 3);

        // A stale invocation cannot publish into a later generation that
        // reused the same guest extension address.
        actor_unregistered(first);
        let new_generation = actor_controller_started(&mut memory, actor).unwrap();
        assert_eq!(new_generation.identity.slot, first_identity.slot);
        assert_eq!(
            new_generation.identity.generation,
            first_identity.generation + 1
        );
        memory.word(first + 0x14, 7);
        memory.word(first + 0x18, 8);
        let retired_memory = NoExtensionReads {
            inner: &memory,
            blocked: first..first + 0x1000,
        };
        actor_controller_finished(&retired_memory, Some(first_controller));
        let before_new_return = actor_state_trace_rows()[0].1;
        assert_eq!(before_new_return.mode10_active.high, 4);
        assert_eq!(before_new_return.other_active.low, 3);
        actor_controller_finished(&memory, Some(new_generation));
        let after_new_return = actor_state_trace_rows()[0].1;
        assert_eq!(after_new_return.mode10_active.high, 7);
        assert_eq!(after_new_return.other_active.low, 8);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn approach_adapter_matches_lifted_memory_and_registers_including_eax() {
        let _guard = TEST_LOCK.lock().unwrap();
        configure(true);
        assert!(configure_function(0x266250, true));
        let actor = 0x2000;
        let target = 0x4000;
        let mut seed = 0x2662_5001u32;
        for _ in 0..3000 {
            let mut next = || {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                seed
            };
            let mut memory = Memory::default();
            memory.word(0x475d1c, 0x3c23_d70a);
            for address in [
                actor + 0x64,
                actor + 0x68,
                actor + 0x6c,
                target + 0x0c,
                target + 0x10,
                target + 0x14,
            ] {
                memory.word(address, next());
            }
            let mut rust_regs = regs();
            rust_regs.esp = 0x9000;
            let fraction = next();
            memory.word(0x900c, fraction);
            rust_regs.eax = next();
            rust_regs.ecx = next();
            rust_regs.edx = next();
            rust_regs.ebx = next();
            rust_regs.ebp = next();
            rust_regs.esi = next();
            rust_regs.edi = next();
            let mut lifted_regs = rust_regs;
            memory.word(0x9004, actor);
            memory.word(0x9008, target);
            let mut lifted_memory = memory.clone();

            assert!(run_without_services(0x266250, &mut memory, &mut rust_regs));
            tsfp_port_engine_object_2::c2r::objchar_wired7::game_object_nudge_xz_toward_point_when_outside_deadzone(
                &mut lifted_memory,
                &mut lifted_regs,
            );
            assert_eq!(
                rust_regs.eax, lifted_regs.eax,
                "return value at seed {seed:#x}"
            );
            assert_eq!(
                rust_regs.esp, lifted_regs.esp,
                "stack pointer at seed {seed:#x}"
            );
            assert_eq!(rust_regs.ebx, lifted_regs.ebx, "EBX at seed {seed:#x}");
            assert_eq!(rust_regs.ebp, lifted_regs.ebp, "EBP at seed {seed:#x}");
            assert_eq!(rust_regs.esi, lifted_regs.esi, "ESI at seed {seed:#x}");
            assert_eq!(rust_regs.edi, lifted_regs.edi, "EDI at seed {seed:#x}");
            for offset in [0x64, 0x6c] {
                assert_eq!(
                    memory.read32(actor + offset),
                    lifted_memory.read32(actor + offset),
                    "actor position at {offset:#x}, seed {seed:#x}"
                );
            }
        }
        configure(false);
    }

    #[test]
    fn behavior_request_and_commit_keep_typed_pair_authoritative() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);
        assert!(configure_function(0x17570, true));

        let object = 0x1000;
        let extension = 0x2000;
        let mut memory = Memory::default();
        memory.word(object + 0x7c, extension);
        memory.word(GAME_MODE_BYTE, 10);
        memory.word(extension + 0x10, 0x40);
        memory.word(extension + 0x14, 1);
        memory.word(extension + 0x18, 0x80);
        memory.word(extension + 0x1c, 2);
        memory.word(extension + 0x20, 3);
        memory.word(extension + 0x9d0, u32::MAX);
        memory.word(extension + 0xadc, 0x100);
        actor_spawned(&memory, object);

        let mut registers = regs();
        registers.esp = 0x8000;
        memory.word(registers.esp + 4, extension);
        memory.word(registers.esp + 8, 0x21);
        memory.word(registers.esp + 12, 7);
        assert!(run(
            0x17570,
            &mut memory,
            &mut TestGuest::default(),
            &mut registers
        ));
        assert_eq!(memory.read32(extension + 0xaa8), 0x21);
        assert_eq!(memory.read32(extension + 0xaac), 7);
        assert_eq!(
            actor_state_trace_rows()[0].1.pending_behavior,
            BehaviorPair { low: 0x21, high: 7 }
        );

        // The protected (1, 0) request leaves both Rust state and the
        // compatibility projection untouched when active low bit zero is clear.
        memory.word(registers.esp + 8, 1);
        memory.word(registers.esp + 12, 0);
        assert!(run(
            0x17570,
            &mut memory,
            &mut TestGuest::default(),
            &mut registers
        ));
        assert_eq!(memory.read32(extension + 0xaa8), 0x21);
        assert_eq!(memory.read32(extension + 0xaac), 7);

        // The retained selected tick runs on the old active pair. Its callback
        // can replace the single pending pair before 0x15c700 installs it.
        let commit = actor_behavior_commit_started(&memory, extension).unwrap();
        assert_eq!(
            actor_state_trace_rows()[0].1.mode10_active,
            BehaviorPair { low: 0x40, high: 1 }
        );

        // This guarded request distinguishes the old selected pair (even low
        // bit, reject) from the pending pair (odd low bit, accept).
        memory.word(registers.esp + 8, 1);
        memory.word(registers.esp + 12, 0);
        assert!(run(
            0x17570,
            &mut memory,
            &mut TestGuest::default(),
            &mut registers
        ));
        assert_eq!(memory.read32(extension + 0xaa8), 0x21);
        assert_eq!(memory.read32(extension + 0xaac), 7);
        assert_eq!(
            actor_state_trace_rows()[0].1.mode10_active,
            BehaviorPair { low: 0x40, high: 1 }
        );

        // A later callback request replaces that pending pair. The retained
        // 0x15c700 then publishes the final pair and records the selected one.
        memory.word(registers.esp + 8, 0x30);
        memory.word(registers.esp + 12, 9);
        assert!(run(
            0x17570,
            &mut memory,
            &mut TestGuest::default(),
            &mut registers
        ));
        memory.word(extension + 0x10, 0x30);
        memory.word(extension + 0x14, 9);
        memory.word(extension + 0xab0, 0x40);
        memory.word(extension + 0xab4, 1);
        actor_behavior_committed(&memory, commit);
        let state = actor_state_trace_rows()[0].1;
        assert_eq!(state.mode10_active, BehaviorPair { low: 0x30, high: 9 });
        assert_eq!(state.previous_behavior, BehaviorPair { low: 0x40, high: 1 });
        assert_eq!(state.pending_behavior, BehaviorPair { low: 0x30, high: 9 });
        assert_eq!(state.awareness_cache.raw_result(), 0x100);
        let mut trace = [0; 26];
        assert_eq!(
            tsfp_rust_rea_ai_trace_snapshot(extension, trace.as_mut_ptr()),
            1
        );
        assert_eq!(trace[2], 1);
        assert_eq!(&trace[3..5], &[0x30, 9]);
        assert_eq!(&trace[7..9], &[0x40, 1]);
        assert_eq!(&trace[9..11], &[0x30, 9]);
        assert_eq!(trace[15], 0x100);

        // A lifted producer can still write the compatibility pending words
        // directly; import that pair before the next Rust-owned commit.
        memory.word(extension + 0xaa8, 0x60);
        memory.word(extension + 0xaac, 11);
        let next_commit = actor_behavior_commit_started(&memory, extension).unwrap();
        let state = actor_state_trace_rows()[0].1;
        assert_eq!(state.mode10_active, BehaviorPair { low: 0x30, high: 9 });
        assert_eq!(state.previous_behavior, BehaviorPair { low: 0x40, high: 1 });
        memory.word(extension + 0x10, 0x60);
        memory.word(extension + 0x14, 11);
        memory.word(extension + 0xab0, 0x30);
        memory.word(extension + 0xab4, 9);
        actor_behavior_committed(&memory, next_commit);
        let state = actor_state_trace_rows()[0].1;
        assert_eq!(
            state.mode10_active,
            BehaviorPair {
                low: 0x60,
                high: 11
            }
        );
        assert_eq!(state.previous_behavior, BehaviorPair { low: 0x30, high: 9 });

        configure(false);
        assert_eq!(
            tsfp_rust_rea_ai_trace_snapshot(extension, trace.as_mut_ptr()),
            0
        );
        reset_actor_state();
    }

    #[test]
    fn behavior_commit_adopts_lifted_transition_and_callback_replacement() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let object = 0x1100;
        let extension = 0x2100;
        let mut memory = Memory::default();
        memory.word(object + 0x7c, extension);
        memory.word(GAME_MODE_BYTE, 10);
        memory.word(extension + 0x10, 0x40);
        memory.word(extension + 0x14, 1);
        memory.word(extension + 0x18, 0x80);
        memory.word(extension + 0x1c, 2);
        memory.word(extension + 0x20, 5);
        memory.word(extension + 0x9d0, 7);
        memory.word(extension + 0x9d4, 4);
        memory.word(extension + 0x9d8, 3);
        memory.word(extension + 0xaa8, 0x30);
        memory.word(extension + 0xaac, 9);
        memory.word(extension + 0xab0, 0x20);
        memory.word(extension + 0xab4, 6);
        actor_spawned(&memory, object);

        // The measured behavior root may receive a pair from a lifted writer
        // without a preceding Rust 0x17570 request.
        let token = actor_behavior_commit_started(&memory, extension).unwrap();
        assert_eq!(token.selected, BehaviorPair { low: 0x40, high: 1 });
        assert_eq!(
            actor_state_trace_rows()[0].1.mode10_active,
            BehaviorPair { low: 0x40, high: 1 }
        );

        // The retained callback publishes the selected transition and replaces
        // the pending pair. Rust commits the final callback-visible request.
        memory.word(extension + 0x10, 0x30);
        memory.word(extension + 0x14, 9);
        memory.word(extension + 0xaa8, 0x50);
        memory.word(extension + 0xaac, 11);
        memory.word(extension + 0xab0, 0x40);
        memory.word(extension + 0xab4, 1);
        memory.word(extension + 0x20, 6);
        memory.word(extension + 0x9d0, 8);
        memory.word(extension + 0x9d4, 5);
        memory.word(extension + 0x9d8, 19);
        actor_behavior_tick_finished(&memory, extension, Some(token));

        let snapshot = actor_state_trace_rows()[0].1;
        assert_eq!(
            snapshot.mode10_active,
            BehaviorPair {
                low: 0x50,
                high: 11
            }
        );
        assert_eq!(
            snapshot.previous_behavior,
            BehaviorPair { low: 0x40, high: 1 }
        );
        assert_eq!(
            snapshot.pending_behavior,
            BehaviorPair {
                low: 0x50,
                high: 11
            }
        );
        assert_eq!(snapshot.local_state, LocalState(6));
        assert_eq!(snapshot.previous_local_state, LocalState(5));
        assert_eq!(snapshot.pending_local_state, Some(LocalState(8)));
        assert_eq!(snapshot.transition_progress, TransitionProgress(19));

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn behavior_tick_finish_rejects_retired_or_replaced_generation_before_guest_read() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let first_object = 0x1300;
        let replacement_object = 0x1400;
        let extension = 0x2300;
        let mut memory = Memory::default();
        memory.word(first_object + 0x7c, extension);
        memory.word(GAME_MODE_BYTE, 10);
        memory.word(extension + 0x10, 0x40);
        memory.word(extension + 0x14, 1);
        memory.word(extension + 0x20, 5);
        memory.word(extension + 0x9d0, u32::MAX);
        actor_spawned(&memory, first_object);
        let old_token = actor_behavior_commit_started(&memory, extension).unwrap();

        actor_unregistered(extension);
        let retired_guard = NoExtensionReads {
            inner: &memory,
            blocked: extension..extension + 0x1000,
        };
        actor_behavior_tick_finished(&retired_guard, extension, Some(old_token));

        // Reusing the same guest address creates a new Rust generation. The
        // old callback token must not refresh the replacement from guest data.
        memory.word(replacement_object + 0x7c, extension);
        memory.word(extension + 0x10, 0x80);
        memory.word(extension + 0x14, 2);
        actor_spawned(&memory, replacement_object);
        let replacement_before = actor_state_trace_rows()[0].1;
        let replacement_guard = NoExtensionReads {
            inner: &memory,
            blocked: extension..extension + 0x1000,
        };
        actor_behavior_tick_finished(&replacement_guard, extension, Some(old_token));
        assert_eq!(actor_state_trace_rows()[0].1, replacement_before);

        configure(false);
        reset_actor_state();
    }

    #[test]
    fn behavior_commit_mirrors_post_callback_claim_cleanup_to_owned_state() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let object = 0x1200;
        let extension = 0x2200;
        let mut memory = Memory::default();
        memory.word(object + 0x7c, extension);
        memory.word(GAME_MODE_BYTE, 10);
        memory.word(extension + 0x10, 0x40000);
        memory.word(extension + 0xaa8, 0x10000);
        actor_spawned(&memory, object);

        let identity = LIVE_ACTORS.lock().bindings[0].1;
        let claim = crate::awareness::SecondaryTargetClaim {
            handle: crate::awareness::TargetHandle {
                object: ObjectId(0x55),
                generation: 1,
            },
            own_key: crate::rea_ai_types::NodeKey(0x77),
            primary: crate::awareness::PrimaryTargetId(0x99),
        };
        {
            let mut live = LIVE_ACTORS.lock();
            let graph = live.store.replace_navigation_graph().unwrap();
            live.store
                .acquire_navigation_claim(
                    identity,
                    crate::rea_tick_state::GraphScopedSecondaryClaim { graph, claim },
                )
                .unwrap();
            live.store.save_navigation_claim(identity).unwrap();
            assert!(live.store.primary_is_occupied(claim.primary));
        }

        let token = actor_behavior_commit_started(&memory, extension).unwrap();
        assert_eq!(token.change.previous.low, 0x40000);

        // The retained callbacks change the selected active mask before the
        // cleanup guard runs. The typed cleanup must use this final callback
        // value, rather than the initially requested pair.
        memory.word(extension + 0x10, 4);
        memory.word(extension + 0xaa8, 0x20);
        actor_behavior_tick_finished(&memory, extension, Some(token));

        let live = LIVE_ACTORS.lock();
        let state = live.store.state(identity).unwrap();
        assert_eq!(state.navigation_claims().current, None);
        assert_eq!(state.navigation_claims().saved, None);
        assert!(!live.store.primary_is_occupied(claim.primary));

        configure(false);
        drop(live);
        reset_actor_state();
    }

    fn point_regs(esp: u32) -> ExtRegisters {
        ExtRegisters {
            eax: 0xdead,
            ecx: 0,
            edx: 0,
            esp,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }

    #[test]
    fn adopted_function_mask_is_u64_with_unique_single_bits() {
        let _guard = TEST_LOCK.lock().unwrap();
        let vas = [
            0x266250u32,
            0x261de0,
            0x26d690,
            0x26e100,
            0x26de30,
            0x26e150,
            0x161730,
            0x263ed0,
            0x25faf0,
            0x12f00,
            0x177920,
            0x2a0500,
            0x267ff0,
            0x9cc40,
            0x59d20,
            0x9a420,
            0x15d500,
            0xa0640,
            0x349e20,
            0x3797b0,
            0x15d470,
            0x12fc0,
            0x1c5210,
            0x1c5250,
            0x2aefb0,
            0x2b2390,
            0x350e70,
            0x12fe0,
            0xd6ed0,
            0x27ce70,
            0x26cc80,
            0x26cd50,
            0x288030,
            0x26dbe0,
            0x26dd80,
            0x26d390,
            0x26ce20,
            0x59d50,
            0x1b6db0,
            0x1b6bf0,
            0x1b9a30,
            0x1b63d0,
            0x1b7cc0,
            0x1b6bb0,
            0x1b99f0,
            0x17570,
            0x1b6c90,
            0x2a1f30,
            0x2a4120,
            0xa4c70,
            0x26d8b0,
            0x9a260,
        ];
        let mut seen = 0u64;
        for (expected_bit, va) in vas.into_iter().enumerate() {
            let bit = function_bit(va).expect("known address");
            assert_eq!(
                bit,
                1u64 << expected_bit,
                "{va:#x} keeps its documented bit"
            );
            assert_eq!(seen & bit, 0);
            seen |= bit;
        }
        assert_eq!(seen, ALL_ADOPTED_FUNCTIONS);
        assert_eq!(function_bit(0x1b9a30), Some(1u64 << 40));
        assert_eq!(function_bit(0x1b63d0), Some(1u64 << 41));
        assert_eq!(function_bit(0x1b7cc0), Some(1u64 << 42));
        // bits above 31 survive the atomic and the per-address switch
        ENABLED_FUNCTIONS.store(0, Ordering::Release);
        ENABLED_FUNCTIONS.fetch_or(1u64 << 42, Ordering::AcqRel);
        assert_eq!(ENABLED_FUNCTIONS.load(Ordering::Acquire), 1u64 << 42);
        configure(true);
        assert_eq!(
            ENABLED_FUNCTIONS.load(Ordering::Acquire),
            DEFAULT_ADOPTED_FUNCTIONS
        );
        assert!(function_enabled(0x12f00));
        assert!(!function_enabled(0x26d390));
        assert!(configure_function(0x26d390, true));
        assert!(function_enabled(0x26d390));
        assert!(configure_function(0x26d390, false));
        assert!(configure_function(0x26cd50, false));
        assert!(!function_enabled(0x26cd50) && function_enabled(0x26cc80));
        assert!(!configure_function(0x1234, false));
        configure(false);
    }

    #[test]
    fn weapon_readiness_adapter_uses_typed_transition_predicate_and_state_gate() {
        let _guard = TEST_LOCK.lock().unwrap();
        let object = 0x31000;
        let stack = 0x90000;
        let mut memory = Memory::default();
        memory.word(object + 0xe8, 9);
        memory.word(object + 0xf8, 9);
        let mut guest = TestGuest::default();
        let mut regs = point_regs(stack);
        regs.edx = object;
        configure(true);
        configure_function(0x1b9a30, true);
        assert!(run(0x1b9a30, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 1);
        assert!(guest.calls.is_empty());

        guest.calls.clear();
        memory.word(object + 0xf8, 8);
        assert!(run(0x1b9a30, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 0);
        assert!(guest.calls.is_empty());

        guest.calls.clear();
        memory.word(object + 0xe8, 0x21);
        memory.word(object + 0xf8, 0x14);
        assert!(run(0x1b9a30, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 0);
        assert!(guest.calls.is_empty());
        configure(false);
    }

    #[test]
    fn weapon_transition_adapter_reads_object_and_target_from_cdecl_stack() {
        let _guard = TEST_LOCK.lock().unwrap();
        let object = 0x32000;
        let stack = 0x90000;
        let mut memory = Memory::default();
        memory.word(object + 0xe8, 0x21);
        memory.word(object + 0xf8, 0x14);
        memory.word(stack + 4, object);
        memory.word(stack + 8, 0x21);
        let mut guest = TestGuest::default();
        let mut regs = point_regs(stack);
        configure(true);
        configure_function(0x1b63d0, true);
        assert!(run(0x1b63d0, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 1);
        assert!(guest.calls.is_empty());

        memory.word(stack + 8, 0x14);
        assert!(run(0x1b63d0, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 0);
        configure(false);
    }

    #[test]
    fn state6_followup_tier1_weak_differential_vs_lifted_c2r_3000_fabricated_seeds() {
        let _guard = TEST_LOCK.lock().unwrap();
        let actor = 0x20000;
        let extension = 0x21000;
        let level_record = 0x22000;
        let pair_entry = 0x23000;
        let stack = 0x90000;
        let mut seed = 0x2434_26dd_80u64;

        configure(true);
        configure_function(0x26dd80, true);
        for case in 0..3000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let current_level = ((seed >> 5) as u32) & 0x1f;
            let actor_level = if seed & 1 == 0 {
                current_level
            } else {
                current_level.wrapping_add(1)
            };
            let guard_result = u32::from(seed & 2 != 0);
            let pair_present = seed & 4 != 0;
            let direction_passed = seed & 8 != 0;
            let state8_selected = seed & 16 != 0;
            let state6_selected = seed & 32 != 0;
            let target = (seed >> 32) as u32;

            let mut memory = Memory::default();
            memory.word(actor + 0x7c, extension);
            memory.word(extension + 8, actor_level);
            memory.word(extension + 0x9bc, (seed >> 24) as u32);
            memory.word(0x7a3500, current_level);
            let level_address = 0x0073_56d8u32.wrapping_add(current_level.wrapping_mul(0xea0));
            memory.word(level_address + 8, level_record);
            memory.word(level_address, target);
            memory.write8(pair_entry, u8::from(pair_present));

            let mut base_guest = TestGuest::default();
            base_guest.service_results.insert(0x26dbe0, guard_result);
            base_guest.service_results.insert(0x15bd00, pair_entry);
            base_guest
                .service_results
                .insert(0x161630, u32::from(direction_passed));
            base_guest.service_result_sequences.insert(
                0x174e0,
                vec![u32::from(state8_selected), u32::from(state6_selected)],
            );

            let mut lifted_memory = memory.clone();
            let mut lifted_guest = base_guest.clone();
            let mut rust_guest = base_guest;
            let mut lifted_regs = point_regs(stack);
            lifted_regs.ebx = actor;
            lifted_regs.esi = 0xaaaa_1111;
            lifted_regs.edi = 0xbbbb_2222;
            let mut rust_regs = lifted_regs;

            crate::c2r::t1926_characters::game_actor_try_enter_state_6_or_set_flag_8_when_player_pair_entry_nonzero(
                &mut lifted_memory,
                &mut lifted_guest,
                &mut lifted_regs,
            );
            assert!(super::run(
                0x26dd80,
                &mut memory,
                &mut rust_guest,
                &mut rust_regs,
            ));

            assert_eq!(rust_guest.calls, lifted_guest.calls, "seed {case}: calls");
            assert_eq!(
                memory.game_data(),
                lifted_memory.game_data(),
                "seed {case}: memory"
            );
            assert_eq!(
                (
                    rust_regs.ebx,
                    rust_regs.ebp,
                    rust_regs.esi,
                    rust_regs.edi,
                    rust_regs.esp
                ),
                (
                    lifted_regs.ebx,
                    lifted_regs.ebp,
                    lifted_regs.esi,
                    lifted_regs.edi,
                    lifted_regs.esp
                ),
                "seed {case}: preserved registers",
            );
        }
        configure(false);
    }

    #[test]
    fn action_guard_tier1_weak_differential_vs_lifted_c2r_3000_fabricated_seeds() {
        let _guard = TEST_LOCK.lock().unwrap();
        configure(true);
        configure_function(0x26dbe0, true);
        let mut seed = 0x2430_26db_e0u64;
        for _ in 0..3000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            // Each fabricated seed is a fresh actor snapshot. Reuse of the
            // same guest address must not carry the prior sample's typed
            // local-state owner into this independent differential case.
            reset_actor_state();
            let mut memory = Memory::default();
            let stack = 0x90000;
            let actor = 0x20000;
            let extension = 0x21000;
            let owner = 0x22000;
            let owner_extension = 0x23000;
            let player = 0x24000;
            let player_extension = 0x25000;
            let behavior = 0x26000;
            let record_index = match seed % 4 {
                0 => u32::MAX,
                n => (n - 1) as u32,
            };
            let status = ((seed >> 8) % 4) as u32;
            let flags = if seed & 0x10 != 0 {
                0x8000_0000
            } else {
                (seed >> 20) as u32 & 0x7fff_ffff
            };
            let timer = match seed % 5 {
                0 => 1.0,
                1 => f32::from_bits(1.0f32.to_bits() + 1),
                _ => ((seed >> 12) % 400) as f32 / 100.0,
            };
            memory.word(stack, 0xfeedface);
            memory.word(stack + 4, actor);
            memory.word(actor + 4, owner);
            memory.word(actor + 0x7c, extension);
            memory.word(owner + 0x7c, owner_extension);
            memory.word(
                owner_extension + 0x678,
                if seed & 0x40 != 0 {
                    u32::MAX
                } else {
                    (seed >> 24) as u32
                },
            );
            memory.word(0x7b0c48 + 0x14, player);
            memory.word(player + 0x7c, player_extension);
            memory.word(player_extension + 0x678, (seed >> 32) as u32);
            memory.word(0x7a3580 + 0x678, (seed >> 28) as u32);
            memory.word(extension + 0x204, record_index);
            memory.word(extension + 0x28, flags);
            memory.word(extension + 0x20, ((seed >> 4) % 7) as u32);
            memory.word(extension + 0x9d0, u32::MAX);
            memory.float(extension + 0xb30, timer);
            memory.float(0x475c78, 1.0);
            memory.word(extension + 0x684, (seed >> 16) as u32);
            memory.word(extension + 0xac8, (seed >> 18) as u32);
            memory.word(extension + 0x9ac, 0x1111);
            memory.word(extension + 0x9a8, 0x2222);
            memory.word(extension + 0xbec, (seed >> 5) as u32);
            memory.word(extension + 0xadc, if seed & 0x80 != 0 { 1 } else { 0 });
            memory.word(extension + 0xbf0, behavior);
            memory.word(behavior + 0x54, if seed & 0x100 != 0 { 0x20 } else { 0 });
            if record_index != u32::MAX {
                memory.word(
                    0x4f9bac + record_index * 0x29c + 0xc,
                    if seed & 0x200 != 0 { 1 } else { 0 },
                );
            }
            memory.word(0x520398, 0xaaaa);
            memory.word(0x52039c, 0xbbbb);

            let mut lifted_memory = memory.clone();
            let mut lifted_guest = TestGuest::default();
            lifted_guest.service_results.insert(0x15fe00, status);
            lifted_guest
                .service_results
                .insert(0x160ec0, u32::from(seed & 0x400 != 0));
            let mut rust_guest = lifted_guest.clone();
            let mut lifted_regs = point_regs(stack);
            let mut rust_regs = lifted_regs;
            crate::c2r::t1926_characters::game_char_action_by_weapon_record_4f9bac_26dbe0(
                &mut lifted_memory,
                &mut lifted_guest,
                &mut lifted_regs,
            );
            assert!(super::run(
                0x26dbe0,
                &mut memory,
                &mut rust_guest,
                &mut rust_regs
            ));
            assert_eq!(rust_regs.eax, lifted_regs.eax, "seed {seed:#x} return");
            // Compare stable transition fields. Both +0x684 and +0xac8 alias
            // stack scratch in the lifted call convention under this fixture.
            for offset in [0x28, 0x20, 0x9d0, 0xb30, 0x9ac, 0x9a8] {
                for byte in 0..4 {
                    let address = extension + offset + byte;
                    assert_eq!(
                        memory.read8(address),
                        lifted_memory.read8(address),
                        "seed {seed:#x} state at extension+{offset:#x}+{byte}"
                    );
                }
            }
            // On a branch that requests a target, check typed resolution directly.
            if rust_guest.calls.contains(&0x5f460) {
                let owner = memory.read32(owner_extension + 0x678) as i32;
                let player_node = memory.read32(player_extension + 0x678) as i32;
                let owner_node = if owner < 0 { player_node } else { owner };
                assert_eq!(
                    memory.read32(extension + 0x684) as i32,
                    owner_node,
                    "seed {seed:#x} typed target selection"
                );
                assert_eq!(
                    memory.read32(extension + 0xac8) as i32,
                    owner,
                    "seed {seed:#x} pending owner target"
                );
            }
            assert_eq!(rust_guest.calls, lifted_guest.calls, "seed {seed:#x} calls");
        }
        configure(false);
    }

    #[test]
    fn point_table_leaves_are_switchable_and_use_the_typed_owner() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut memory = Memory::default();
        let esp = 0x1000;
        // primary table: keys 1001, 1003, 1005 at 0x2000, 32-byte stride
        memory.word(0x715f2c, 0x2000);
        memory.word(0x7b6664, 3);
        for (index, key) in [1001u32, 1003, 1005].into_iter().enumerate() {
            memory.word(0x2000 + index as u32 * 32, key);
        }
        memory.word(0x715f34, 0x3000);
        memory.word(0x7b65c0, 2);
        memory.word(0x3000, 0x2329);
        memory.word(0x3020, 0x232b);
        memory.word(esp + 4, 1003);
        memory.word(esp + 8, 7);
        // owner slot test
        memory.word(0x4000 + 4, 0x5000);
        memory.word(0x5000 + 0x12c, 7);

        configure(true);
        for va in [0x9cc40, 0x59d20, 0x9a420] {
            assert!(function_enabled(va), "{va:#x} default-on under the switch");
            assert!(configure_function(va, false));
            assert!(!run_without_services(va, &mut memory, &mut point_regs(esp)));
            assert!(configure_function(va, true));
        }
        let mut regs = point_regs(esp);
        assert!(run_without_services(0x9cc40, &mut memory, &mut regs));
        assert_eq!(regs.eax, 1);
        memory.word(esp + 4, 0x232b);
        assert!(run_without_services(0x9cc40, &mut memory, &mut regs));
        assert_eq!(regs.eax, 0x1f40 + 1);
        memory.word(esp + 4, 1004);
        assert!(run_without_services(0x9cc40, &mut memory, &mut regs));
        assert_eq!(regs.eax, u32::MAX);

        memory.word(esp + 4, 0x4000);
        assert!(run_without_services(0x59d20, &mut memory, &mut regs));
        assert_eq!(regs.eax, 1);
        memory.word(esp + 8, 8);
        assert!(run_without_services(0x59d20, &mut memory, &mut regs));
        assert_eq!(regs.eax, 0);

        // 0x9A420: id -1 selects nothing, a valid id yields the record address
        memory.word(esp + 4, u32::MAX);
        assert!(run_without_services(0x9a420, &mut memory, &mut regs));
        assert_eq!(regs.eax, 0);
        let mut guest = TestGuest::default();
        guest.service_results.insert(0x9a330, 1);
        memory.word(esp + 4, 2);
        assert!(super::run(0x9a420, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 0x2000 + 2 * 32);
        assert_eq!(guest.calls, [0x9a330]);
        configure(false);
    }

    fn group_target_fixture(seed: &mut u32) -> (Memory, u32, usize) {
        fn next(seed: &mut u32) -> u32 {
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *seed
        }
        fn position(seed: &mut u32) -> f32 {
            ((next(seed) % 2001) as i32 - 1000) as f32 / 10.0
        }
        let mut memory = Memory::default();
        let actor = 0x2000;
        let extension = 0x3000;
        memory.word(actor + 0x7c, extension);
        memory.word(0x475d5c, 10_000f32.to_bits());
        memory.word(0x7a295c, 0);
        let recent_base = 0x7a2900 + 0x2c;
        let count = (next(seed) % 11) as usize;
        memory.word(recent_base, count as u32);
        memory.float(actor + 0x64, position(seed));
        memory.float(actor + 0x68, position(seed));
        memory.float(actor + 0x6c, position(seed));
        memory.word(extension + 0x28, next(seed) & 0x3ff);
        memory.word(extension + 0xac8, next(seed));

        let group = (next(seed) % 20) as usize;
        let group_mask = 1u32 << group;
        let mut has_valid = false;
        for index in 0..count {
            let candidate = 0x4000 + index as u32 * 0x400;
            memory.word(recent_base + 4 + index as u32 * 4, candidate);
            let choice = next(seed) % 8;
            let mask = match choice {
                0 => u32::MAX,
                1 => 1 << 20,
                _ => group_mask,
            };
            has_valid |= mask == group_mask;
            memory.word(candidate + 0x114, mask);
            memory.word(
                candidate + 0x110,
                if next(seed) % 7 == 0 {
                    u32::MAX
                } else {
                    0x7000 + index as u32
                },
            );
            memory.float(candidate + 0x64, position(seed));
            memory.float(candidate + 0x68, position(seed));
            memory.float(candidate + 0x6c, position(seed));
        }
        if has_valid {
            let window = 0x7a2d94 + group as u32 * 0x60;
            memory.word(window - 0x14, 4);
            memory.word(window - 4, actor);
        }
        (memory, actor, count)
    }

    #[test]
    fn group_target_adapter_is_opt_in_and_differential_against_lifted_reference() {
        let _guard = TEST_LOCK.lock().unwrap();
        configure(true);
        assert!(!function_enabled(0x26d390));
        assert!(configure_function(0x26d390, true));
        assert!(function_enabled(0x26d390));
        let mut seed = 0x2434_0001;
        for _ in 0..3000 {
            let (mut rust_memory, actor, _) = group_target_fixture(&mut seed);
            rust_memory.word(0x475d5c, 10_000f32.to_bits());
            let mut lifted_memory = rust_memory.clone();
            let esp = 0x100000;
            let mut rust_regs = point_regs(esp);
            rust_regs.eax = actor;
            let mut lifted_regs = rust_regs;
            assert!(super::run(
                0x26d390,
                &mut rust_memory,
                &mut TestGuest::default(),
                &mut rust_regs,
            ));
            tsfp_port_ai_behavior::ai_behavior::game_char_pick_nearest_group_member_and_set_alert_target_0xac8(
                &mut lifted_memory,
                &mut lifted_regs,
            );
            let extension = rust_memory.read32(actor + 0x7c);
            assert_eq!(
                rust_memory.read32(extension + 0xac8),
                lifted_memory.read32(extension + 0xac8),
                "Tier 1 weak lifted-reference target mismatch at seed {seed:#x}"
            );
            assert_eq!(
                rust_memory.read32(extension + 0x28),
                lifted_memory.read32(extension + 0x28),
                "Tier 1 weak lifted-reference flags mismatch at seed {seed:#x}"
            );
        }
        configure(false);
    }

    #[test]
    fn group_target_keeps_source_order_wrap_and_no_target_write_rules() {
        let _guard = TEST_LOCK.lock().unwrap();
        configure(true);
        assert!(configure_function(0x26d390, true));
        let mut memory = Memory::default();
        let actor = 0x2000;
        let extension = 0x3000;
        let first = 0x4000;
        let second = 0x4400;
        memory.word(actor + 0x7c, extension);
        memory.float(actor + 0x64, 0.0);
        memory.float(actor + 0x68, 0.0);
        memory.float(actor + 0x6c, 0.0);
        memory.word(extension + 0x28, 0x8000);
        memory.word(extension + 0xac8, 0x9999);
        memory.word(0x7a295c, 0);
        memory.word(0x7a292c, 2);
        memory.word(0x7a2930, first);
        memory.word(0x7a2934, second);
        memory.word(0x475d5c, 10_000f32.to_bits());
        memory.word(0x7a2d80, 4);
        memory.word(0x7a2d90, actor);
        for candidate in [first, second] {
            memory.word(candidate + 0x114, 1);
            memory.float(candidate + 0x64, 0.0);
            memory.float(candidate + 0x68, 0.0);
            memory.float(candidate + 0x6c, 0.0);
        }
        // A nearest -1 target prevents falling through to an equal-distance candidate.
        memory.word(first + 0x110, u32::MAX);
        memory.word(second + 0x110, 0x7777);
        let mut regs = point_regs(0x100000);
        regs.eax = actor;
        assert!(super::run(
            0x26d390,
            &mut memory,
            &mut TestGuest::default(),
            &mut regs
        ));
        assert_eq!(memory.read32(extension + 0xac8), 0x9999);
        assert_eq!(memory.read32(extension + 0x28), 0x8000);

        // Group one uses the next 0x60-byte membership window; count is a byte length.
        memory.word(0x7a2d80, 0);
        memory.word(0x7a2de0, 4);
        memory.word(0x7a2df0, actor);
        memory.word(first + 0x114, 2);
        memory.word(first + 0x110, 0x8888);
        memory.word(0x7a292c, 1);
        assert!(super::run(
            0x26d390,
            &mut memory,
            &mut TestGuest::default(),
            &mut regs
        ));
        assert_eq!(memory.read32(extension + 0xac8), 0x8888);
        assert_eq!(memory.read32(extension + 0x28), 0x8400);

        // A mask outside the measured 20-bit domain is ignored even when its
        // matching wrapped bucket contains the source actor.
        memory.word(extension + 0xac8, 0x9999);
        memory.word(extension + 0x28, 0x8000);
        memory.word(first + 0x114, 1 << 20);
        memory.word(first + 0x110, 0xaaaa);
        memory.word(0x7a292c, 1);
        let bucket20 = 0x7a2d94 + 20 * 0x60;
        memory.word(bucket20 - 0x14, 4);
        memory.word(bucket20 - 4, actor);
        assert!(super::run(
            0x26d390,
            &mut memory,
            &mut TestGuest::default(),
            &mut regs
        ));
        assert_eq!(memory.read32(extension + 0xac8), 0x9999);
        assert_eq!(memory.read32(extension + 0x28), 0x8000);

        // All three position components participate in squared distance.
        memory.word(first + 0x114, 1);
        memory.word(first + 0x110, 0x7771);
        memory.word(second + 0x114, 1);
        memory.word(second + 0x110, 0x7772);
        memory.float(first + 0x64, 1.0);
        memory.float(first + 0x68, 0.0);
        memory.float(first + 0x6c, 0.0);
        memory.float(second + 0x64, 0.0);
        memory.float(second + 0x68, 2.0);
        memory.float(second + 0x6c, 0.0);
        memory.word(0x7a2d80, 1);
        memory.word(0x7a292c, 2);
        memory.word(0x7a3080, 0);
        memory.word(extension + 0xac8, 0x9999);
        assert!(super::run(
            0x26d390,
            &mut memory,
            &mut TestGuest::default(),
            &mut regs
        ));
        assert_eq!(memory.read32(extension + 0xac8), 0x7771);
        configure(false);
    }

    #[test]
    fn bound_group_alert_recipient_mutations_update_typed_state_and_projection() {
        use crate::rea_group_alert::GroupAlertWorld;

        let _guard = TEST_LOCK.lock().unwrap();
        reset_actor_state();
        configure(true);

        let object = 0x1100;
        let extension = 0x2100;
        let descriptor = 0x3100;
        let mut memory = Memory::default();
        memory.word(object + 0x7c, extension);
        memory.word(extension + 8, 0x40);
        memory.word(extension + 0x10, 0x11);
        memory.word(extension + 0x14, 0x33);
        memory.word(extension + 0x28, 0x22);
        memory.word(extension + 0x4a, (-7i16) as u16 as u32);
        memory.word(extension + 0xbec, 0x55);
        memory.word(extension + 0xbf0, descriptor);
        memory.word(descriptor + 0x54, 0x44);
        // GROUP_TABLE is the group's signed-count word at base + 8; the
        // first member slot is therefore GROUP_TABLE + 8 (base + 0x10).
        memory.word(super::GROUP_TABLE + 8, object);
        super::actor_spawned(&memory, object);

        let mut guest = TestGuest::default();
        let mut regs = regs();
        {
            let mut world = super::RuntimeGroupAlert {
                memory: &mut memory,
                guest: &mut guest,
                regs: &mut regs,
            };
            let member = world.member(0, 0);
            assert_eq!(member.id, 0x40);
            assert_eq!(member.actor_flags, 0x11);
            assert_eq!(member.local_flags, 0x22);
            assert_eq!(member.listen_flags, 0x33);
            assert_eq!(member.behavior_flags, 0x44);
            assert_eq!(member.class_selector, -7);
            assert_eq!(member.group_mask, 0x55);

            world.set_member_target(0, 0, 0x7777);
            world.set_member_local_flags(0, 0, 0x66);
            world.set_member_group_mask(0, 0, 0xaa);
        }

        let live = LIVE_ACTORS.lock();
        let identity = live
            .bindings
            .iter()
            .find(|(bound, _)| *bound == extension)
            .map(|(_, identity)| *identity)
            .unwrap();
        let recipient = live.store.awareness_recipient(identity).unwrap().unwrap();
        assert_eq!(recipient.local_flags, 0x66);
        assert_eq!(recipient.group_mask.0, 0xaa);
        assert_eq!(recipient.alert_target, Some(NodeId(0x7777)));
        assert_eq!(memory.read32(extension + 0xac8), 0x7777);
        assert_eq!(memory.read32(extension + 0x28), 0x66);
        assert_eq!(memory.read32(extension + 0xbec), 0xaa);

        drop(live);
        configure(false);
        reset_actor_state();
    }
}
