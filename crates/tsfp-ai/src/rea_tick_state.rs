// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned state for the REA tick's current behavior, local-state, route,
//! and secondary-claim slices. Guest-memory adapters must translate at the
//! boundary; this module deliberately contains no guest addresses or memory
//! access.

use alloc::{collections::BTreeMap, vec::Vec};

use crate::{
    awareness::{AwarenessCache, NavigationClaimState, PrimaryTargetId, SecondaryTargetClaim},
    movement_approach::Position3,
    navigation_scoring::{ActorId, NodeId},
    rea_ai_types::{GroupMask, LinkActorState},
    rea_controller_target_commit::{
        ControllerTargetEffect, ControllerTargetHandoff, SelectedGoalEffect,
    },
    rea_group_alert::{ACTOR_ALERTABLE, BEHAVIOR_RAISE_FULL, BEHAVIOR_SKIP_CHECK, LOCAL_ALERTED},
    rea_routes::{PointFollowSearchResult, RouteActor},
    rea_target_handoff::TargetObjectId,
};

/// The behavior-mask bank selected by the current actor mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorMode {
    /// Mode 10 reads and commits the active pair at `+0x10/+0x14`.
    Mode10,
    /// Other modes read and commit the active pair at `+0x18/+0x1c`.
    Other,
}

/// The low and high words passed to the behavior-request handler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BehaviorPair {
    pub low: u32,
    pub high: u32,
}

/// A local state number. It is kept distinct from behavior masks and route kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalState(pub u32);

/// Opaque compatibility token copied from the actor extension's cached route
/// record slot. Its pointer/handle meaning, validity sentinel, and graph
/// revision are unresolved; Rust preserves the bits without dereferencing or
/// treating them as a generation-qualified node ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CachedNavigationRecord(pub u32);

/// Opaque DWORD stored at actor extension +0x9D8.
///
/// The bounded controller resets it on a changed local-state commit, but its
/// representation, update source, units, and other writers are unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransitionProgress(pub u32);

/// The two values retained from the measured `0xD8E30` actor-tick prelude.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActorTickTimers {
    /// Extension `+0xAD4`.
    pub primary: f32,
    /// Extension `+0xB40`.
    pub secondary: f32,
}

/// Computes the shared timer ratio used by the measured actor-tick prelude.
/// The guest converts the denominator DWORD to signed `i32` before the f32
/// division, so retain that conversion at this typed seam.
pub fn actor_tick_time_step(shared_time: f32, denominator: i32) -> f32 {
    shared_time / denominator as f32
}

/// Typed guard inputs measured at the `0xD8E30` actor update boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorTickAdmission {
    /// Current mode returned by the global mode query.
    pub mode: u32,
    /// Whether `0x1CEBA0()` returned nonzero.
    pub global_tick_blocked: bool,
    /// Whether `0x3568C0(extension_slot)` accepted this slot.
    pub slot_connected: bool,
    /// Slot stored at extension `+8`.
    pub slot: u32,
    /// Global minimum slot loaded from `0x790950`.
    pub minimum_slot: u32,
    /// Whether `0x3533F0()` selected the additional actor permission check.
    pub actor_permission_check_required: bool,
    /// Whether `0x358560(actor)` permits this actor when checked.
    pub actor_permission_granted: bool,
    /// Actor extension byte `+0x2c`, bit 2.
    pub actor_flag_4_set: bool,
}

/// First measured guard that prevents the `0x15EAE0(actor)` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorTickAdmissionResult {
    RejectedMode9,
    RejectedGlobalGate,
    RejectedDisconnectedSlot,
    RejectedBelowMinimumSlot,
    RejectedActorPermission,
    RejectedActorFlag4,
    Admitted,
}

/// Decays positive timers by the shared actor time-step ratio supplied by the caller.
pub fn decay_actor_tick_timers(timers: &mut ActorTickTimers, time_step: f32) {
    if timers.primary > 0.0 {
        timers.primary -= time_step;
    }
    if timers.secondary > 0.0 {
        timers.secondary -= time_step;
    }
}

/// Applies the measured `0xD8E30` guard and timer ordering to shared Rust state.
pub fn run_actor_tick_prelude(
    input: ActorTickAdmission,
    timers: &mut ActorTickTimers,
    time_step: f32,
) -> ActorTickAdmissionResult {
    if input.mode == 9 {
        return ActorTickAdmissionResult::RejectedMode9;
    }
    if input.global_tick_blocked {
        return ActorTickAdmissionResult::RejectedGlobalGate;
    }
    if !input.slot_connected {
        return ActorTickAdmissionResult::RejectedDisconnectedSlot;
    }
    decay_actor_tick_timers(timers, time_step);
    if input.slot < input.minimum_slot {
        return ActorTickAdmissionResult::RejectedBelowMinimumSlot;
    }
    if input.actor_permission_check_required && !input.actor_permission_granted {
        return ActorTickAdmissionResult::RejectedActorPermission;
    }
    if input.actor_flag_4_set {
        return ActorTickAdmissionResult::RejectedActorFlag4;
    }
    ActorTickAdmissionResult::Admitted
}

/// Result of queuing a local state, applied by the next state-tick prologue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalStateChange {
    pub previous: LocalState,
    pub current: LocalState,
}

/// Complete result of consuming a queued local-state request at a tick
/// prologue. `changed` distinguishes request consumption from a state change:
/// an equal request is still cleared, but does not update previous state or
/// reset transition progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalStateCommit {
    pub requested: LocalState,
    pub previous: LocalState,
    pub current: LocalState,
    pub changed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalStateCommitEffect {
    ResetTransitionProgress,
    SavePreviousState,
    PublishCurrentState,
    ClearPendingRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalStateCommitOrder {
    StateMachinePrologue,
    ControllerEntry,
}

/// Result of committing a behavior request. World callbacks and navigation
/// cleanup remain at the typed host seam and are not performed by this model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BehaviorChange {
    pub mode: BehaviorMode,
    pub previous: BehaviorPair,
    pub current: BehaviorPair,
}

/// Rust-owned fields used by the measured first-alert recipient branch.
/// Descriptor checks and target resolution happen before this value reaches
/// the actor store; it contains no borrowed guest pointers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwarenessRecipientState {
    pub actor: ActorId,
    pub actor_flags: u32,
    pub local_flags: u32,
    pub listen_flags: u32,
    pub behavior_flags: u32,
    pub class_selector: i16,
    pub group_mask: GroupMask,
    pub alert_target: Option<NodeId>,
}

/// The group-membership callback used between the recipient's alert writes
/// and its final group-mask intersection.
pub trait AwarenessGroupMembership {
    /// Notification may update the owned recipient before the caller commits
    /// the post-callback group-mask intersection, preserving callback order.
    fn leave_groups(&mut self, recipient: &mut AwarenessRecipientState, removed: GroupMask);
}

/// World services called by the measured group-alert recipient branches.
/// Mutable actor state passes through each callback so typed side effects are
/// visible to the following ordered operation.
pub trait AwarenessRecipientWorld: AwarenessGroupMembership {
    fn class_listens(&mut self, selector: i16) -> bool;
    fn candidate_check(&mut self, recipient: &mut AwarenessRecipientState) -> bool;
    fn actor_busy(&mut self, recipient: &mut AwarenessRecipientState) -> bool;
    fn raise(&mut self, recipient: &mut AwarenessRecipientState, code: u32, three_args: bool);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwarenessRecipientEffect {
    Skipped,
    NewlyAlerted { removed_groups: GroupMask },
    MarkedAlerted,
    Raised { code: u32, three_args: bool },
    SuppressedBusy,
}

/// Rust-owned navigation state attached to one actor lifetime. Route IDs,
/// next-node selection, movement target, link projection, and secondary claim
/// handles share one explicit owner; guest-memory adapters translate only at
/// the boundary. Graph replacement invalidates every field that refers to the
/// old loaded graph.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActorNavigation {
    graph_generation: Option<NavigationGraphGeneration>,
    route: RouteActor,
    route_owned: bool,
    cached_route_record: Option<CachedNavigationRecord>,
    state_six_route: StateSixRoute,
    next_node: Option<NodeId>,
    movement_target: Option<Position3>,
    link: Option<LinkActorState>,
    claims: NavigationClaimState,
}

/// Typed fields updated by the measured 0x26D8B0 route-entry function.
/// Kept separate from `rea_routes::RouteActor`, which tracks the loaded route
/// program cursor and goal rather than this extension-local request state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StateSixRoute {
    pub state: u32,
    pub requested_node: i32,
    pub target_node: i32,
    pub counter_9ac: u32,
    pub counter_9a8: u32,
    pub progress_b30: f32,
    pub mode_9d0: u32,
}

impl StateSixRoute {
    fn prepare_entry(
        &mut self,
        current_state: LocalState,
        requested_node: i32,
        player_node: i32,
    ) -> bool {
        if current_state.0 != 1 {
            return false;
        }
        self.state = current_state.0;
        self.requested_node = requested_node;
        self.target_node = if requested_node < 0 {
            player_node
        } else {
            requested_node
        };
        self.counter_9ac = 0;
        self.counter_9a8 = 0;
        self.progress_b30 = 0.0;
        self.mode_9d0 = 6;
        true
    }
}

impl ActorNavigation {
    pub const fn graph_generation(&self) -> Option<NavigationGraphGeneration> {
        self.graph_generation
    }

    /// The outer boundary has imported the cached-record bits for this actor
    /// generation. `None` means no snapshot is owned yet, not a guest null.
    pub const fn cached_route_record(&self) -> Option<CachedNavigationRecord> {
        self.cached_route_record
    }

    pub fn seed_cached_route_record_if_unowned(
        &mut self,
        record: CachedNavigationRecord,
    ) -> CachedNavigationRecord {
        *self.cached_route_record.get_or_insert(record)
    }

    pub fn set_cached_route_record(&mut self, record: CachedNavigationRecord) {
        self.cached_route_record = Some(record);
    }

    pub const fn route(&self) -> RouteActor {
        self.route
    }

    pub fn route_mut(&mut self) -> &mut RouteActor {
        self.route_owned = true;
        &mut self.route
    }

    pub const fn route_owned(&self) -> bool {
        self.route_owned
    }

    /// Imports one measured compatibility snapshot only until a Rust route
    /// handler has taken ownership of this actor's route state.
    pub fn seed_route_if_unowned(&mut self, route: RouteActor) -> RouteActor {
        if !self.route_owned {
            self.route = route;
            self.route_owned = true;
        }
        self.route
    }

    pub const fn state_six_route(&self) -> StateSixRoute {
        self.state_six_route
    }

    pub fn state_six_route_mut(&mut self) -> &mut StateSixRoute {
        &mut self.state_six_route
    }

    pub const fn next_node(&self) -> Option<NodeId> {
        self.next_node
    }

    pub fn set_next_node(&mut self, node: NodeId) {
        self.next_node = Some(node);
    }

    pub const fn movement_target(&self) -> Option<Position3> {
        self.movement_target
    }

    pub fn set_movement_target(&mut self, target: Position3) {
        self.movement_target = Some(target);
    }

    pub const fn link(&self) -> Option<LinkActorState> {
        self.link
    }

    pub fn set_link(&mut self, link: LinkActorState) {
        self.link = Some(link);
    }

    pub fn clear_link(&mut self) {
        self.link = None;
    }

    pub const fn claims(&self) -> &NavigationClaimState {
        &self.claims
    }

    pub fn claims_mut(&mut self) -> &mut NavigationClaimState {
        &mut self.claims
    }

    fn replace_graph(&mut self, generation: NavigationGraphGeneration) {
        *self = Self {
            graph_generation: Some(generation),
            ..Self::default()
        };
    }
}

/// Ordered effects around the measured behavior commit. The callbacks and
/// navigation services execute at the runtime seam; this plan keeps their
/// ordering explicit in the Rust-owned transition model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorCommitEffect {
    ClearHandoverExtensionWords,
    SavePreviousPair,
    InstallCurrentPair,
    ResetNavigationProgress,
    DispatchPreviousMessage3ea,
    DispatchCurrentMessage3e9,
}

/// Synchronous messages emitted while handing an actor from one behavior pair
/// to another. The callback receives mutable Rust-owned state so requests made
/// by the old behavior are visible to the current behavior and to commit finish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorHandoverMessage {
    Previous,
    Current,
}

/// Raw extension words cleared by the measured behavior handover prelude.
/// Their individual meanings and types are unresolved, so preserve bits only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BehaviorHandoverWords {
    pub at_9cc: u32,
    pub at_b30: u32,
    pub at_9e0: u32,
    pub at_9dc: u32,
}

impl BehaviorHandoverMessage {
    pub const fn id(self) -> u32 {
        match self {
            Self::Previous => 0x3ea,
            Self::Current => 0x3e9,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BehaviorClaimCleanup {
    /// Clear the secondary target and its referenced primary occupancy.
    pub clear_secondary: bool,
    /// Release and clear the secondary claim record.
    pub release_secondary_claim: bool,
}

impl BehaviorChange {
    /// Effects for the measured pre-callback commit and ordered callbacks.
    /// Cleanup is evaluated separately after callbacks because they may mutate
    /// the active pair and secondary pointers.
    pub const fn ordered_effects(self) -> [BehaviorCommitEffect; 6] {
        [
            BehaviorCommitEffect::ClearHandoverExtensionWords,
            BehaviorCommitEffect::SavePreviousPair,
            BehaviorCommitEffect::InstallCurrentPair,
            BehaviorCommitEffect::ResetNavigationProgress,
            BehaviorCommitEffect::DispatchPreviousMessage3ea,
            BehaviorCommitEffect::DispatchCurrentMessage3e9,
        ]
    }

    /// Evaluates the mode-10 claim cleanup guard against state observed after
    /// callbacks. `current_after_callbacks` is passed explicitly so this model
    /// does not assume those callbacks are pure.
    pub const fn cleanup_after_callbacks(
        self,
        current_after_callbacks: BehaviorPair,
        has_secondary: bool,
        has_secondary_claim: bool,
    ) -> BehaviorClaimCleanup {
        let leaving_navigation_mode = matches!(self.mode, BehaviorMode::Mode10)
            && self.previous.low & 0x70000 != 0
            && current_after_callbacks.low & 0x4070060 == 0;
        BehaviorClaimCleanup {
            clear_secondary: leaving_navigation_mode && has_secondary,
            release_secondary_claim: leaving_navigation_mode
                && self.previous.low & 0x60000 != 0
                && has_secondary_claim,
        }
    }
}

/// Typed boundary snapshot used to initialize the actor-owned state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReaTickSnapshot {
    pub mode: BehaviorMode,
    pub mode10_active: BehaviorPair,
    pub other_active: BehaviorPair,
    /// Selected pair saved at extension `+0xAB0/+0xAB4` after its class tick.
    pub previous_behavior: BehaviorPair,
    pub pending_behavior: BehaviorPair,
    pub local_state: LocalState,
    pub previous_local_state: LocalState,
    pub pending_local_state: Option<LocalState>,
    pub transition_progress: TransitionProgress,
    /// Full DWORD cache at actor-extension `+0xADC`; no Boolean meaning is assumed.
    pub awareness_cache: AwarenessCache,
}

/// State owned by one REA actor for the measured request, state-queue, route,
/// and secondary-claim slices.
///
/// The two active mask banks are separate because the original selects them by
/// mode. Pending requests are one replaceable pair, not an accumulating mask.
#[derive(Clone, Debug, PartialEq)]
pub struct ReaTickState {
    mode: BehaviorMode,
    mode10_active: BehaviorPair,
    other_active: BehaviorPair,
    previous_behavior: BehaviorPair,
    pending_behavior: BehaviorPair,
    behavior_owned: bool,
    behavior_handover_words: BehaviorHandoverWords,
    local_state_owned: bool,
    local_state: LocalState,
    previous_local_state: LocalState,
    pending_local_state: Option<LocalState>,
    transition_progress: TransitionProgress,
    /// Rust-owned values for the measured `0xD8E30` actor-tick timer prelude.
    /// Their source value is adopted only through the explicit setter seam.
    actor_tick_timers: ActorTickTimers,
    awareness_cache: AwarenessCache,
    awareness_cache_owned: bool,
    /// Cursor for the selected behavior program (`extension + 0xAA0`). This is
    /// separate from `ActorNavigation`'s graph-node IDs; route reconstruction
    /// does not rebase this program cursor. `None` means the compatibility
    /// seam has not adopted the guest value.
    behavior_route_cursor: Option<u32>,
    /// Target object selected by the ordered target-handoff path.
    handoff_target: Option<TargetObjectId>,
    /// Opaque current/pending/previous DWORD transaction from the controller.
    controller_target_handoff: Option<ControllerTargetHandoff>,
    /// Recipient fields consumed by the measured group-alert path.
    awareness_recipient: Option<AwarenessRecipientState>,
    /// Actor object position, kept separate from the navigation extension's
    /// movement target.
    world_position: Option<Position3>,
    navigation: ActorNavigation,
}

impl ReaTickState {
    /// Creates state from a typed boundary snapshot.
    pub fn new(snapshot: ReaTickSnapshot) -> Self {
        Self {
            mode: snapshot.mode,
            mode10_active: snapshot.mode10_active,
            other_active: snapshot.other_active,
            previous_behavior: snapshot.previous_behavior,
            pending_behavior: snapshot.pending_behavior,
            behavior_owned: false,
            behavior_handover_words: BehaviorHandoverWords::default(),
            local_state_owned: false,
            local_state: snapshot.local_state,
            previous_local_state: snapshot.previous_local_state,
            pending_local_state: snapshot.pending_local_state,
            transition_progress: snapshot.transition_progress,
            actor_tick_timers: ActorTickTimers::default(),
            awareness_cache: snapshot.awareness_cache,
            awareness_cache_owned: false,
            behavior_route_cursor: None,
            handoff_target: None,
            controller_target_handoff: None,
            awareness_recipient: None,
            world_position: None,
            navigation: ActorNavigation::default(),
        }
    }

    pub const fn mode(&self) -> BehaviorMode {
        self.mode
    }

    pub const fn actor_tick_timers(&self) -> ActorTickTimers {
        self.actor_tick_timers
    }

    pub fn set_actor_tick_timers(&mut self, timers: ActorTickTimers) {
        self.actor_tick_timers = timers;
    }

    pub fn decay_actor_tick_timers(&mut self, time_step: f32) {
        decay_actor_tick_timers(&mut self.actor_tick_timers, time_step);
    }

    /// Applies the measured per-actor admission/timer prelude to Rust-owned
    /// timer state. The caller still owns guest-facing guard inputs and the
    /// `0x15EAE0` dispatch boundary.
    pub fn run_actor_tick_prelude(
        &mut self,
        admission: ActorTickAdmission,
        time_step: f32,
    ) -> ActorTickAdmissionResult {
        run_actor_tick_prelude(admission, &mut self.actor_tick_timers, time_step)
    }

    pub const fn active_behavior(&self) -> BehaviorPair {
        match self.mode {
            BehaviorMode::Mode10 => self.mode10_active,
            BehaviorMode::Other => self.other_active,
        }
    }

    pub const fn previous_behavior(&self) -> BehaviorPair {
        self.previous_behavior
    }

    pub const fn pending_behavior(&self) -> BehaviorPair {
        self.pending_behavior
    }

    pub const fn awareness_cache(&self) -> AwarenessCache {
        self.awareness_cache
    }

    pub const fn owns_awareness_cache(&self) -> bool {
        self.awareness_cache_owned
    }

    pub const fn awareness_recipient(&self) -> Option<AwarenessRecipientState> {
        self.awareness_recipient
    }

    pub fn set_awareness_recipient(&mut self, recipient: AwarenessRecipientState) {
        self.awareness_recipient = Some(recipient);
    }

    /// Takes ownership of the complete cached result without reducing it to a Boolean.
    pub fn set_awareness_cache(&mut self, cache: AwarenessCache) {
        self.awareness_cache = cache;
        self.awareness_cache_owned = true;
    }

    pub const fn owns_behavior_pairs(&self) -> bool {
        self.behavior_owned
    }

    pub const fn behavior_handover_words(&self) -> BehaviorHandoverWords {
        self.behavior_handover_words
    }

    /// Imports the measured raw words at the compatibility boundary. The
    /// values remain opaque until their independent contracts are resolved.
    pub fn set_behavior_handover_words(&mut self, words: BehaviorHandoverWords) {
        self.behavior_handover_words = words;
    }

    pub const fn local_state(&self) -> LocalState {
        self.local_state
    }

    pub const fn previous_local_state(&self) -> LocalState {
        self.previous_local_state
    }

    pub const fn pending_local_state(&self) -> Option<LocalState> {
        self.pending_local_state
    }

    pub const fn owns_local_state(&self) -> bool {
        self.local_state_owned
    }

    pub const fn transition_progress(&self) -> TransitionProgress {
        self.transition_progress
    }

    pub const fn behavior_route_cursor(&self) -> Option<u32> {
        self.behavior_route_cursor
    }

    pub fn set_behavior_route_cursor(&mut self, cursor: u32) {
        self.behavior_route_cursor = Some(cursor);
    }

    pub fn clear_behavior_route_cursor(&mut self) {
        self.behavior_route_cursor = None;
    }

    pub const fn handoff_target(&self) -> Option<TargetObjectId> {
        self.handoff_target
    }

    pub fn set_handoff_target(&mut self, target: TargetObjectId) {
        self.handoff_target = Some(target);
    }

    pub const fn controller_target_handoff(&self) -> Option<ControllerTargetHandoff> {
        self.controller_target_handoff
    }

    pub fn set_controller_target_handoff(&mut self, handoff: ControllerTargetHandoff) {
        self.controller_target_handoff = Some(handoff);
    }

    /// Applies the four bounded selected-goal setter branches documented in
    /// the REA record. Only an installed target resets shared progress; other
    /// goal writers remain outside this model.
    pub fn apply_measured_selected_goal(&mut self, target: u32) -> Option<SelectedGoalEffect> {
        let handoff = self.controller_target_handoff.as_mut()?;
        let effect = handoff.apply_measured_selected_goal(target);
        if matches!(effect, SelectedGoalEffect::SelectedGoalInstalled { .. }) {
            self.transition_progress = TransitionProgress(0);
        }
        Some(effect)
    }

    /// Applies the bounded controller target request to this actor's state.
    /// A changed target also clears the shared raw progress field at `+0x9D8`.
    pub fn apply_controller_target_request(&mut self) -> Option<ControllerTargetEffect> {
        let handoff = self.controller_target_handoff.as_mut()?;
        let effect = handoff.apply_pending_request();
        if matches!(effect, ControllerTargetEffect::TargetChanged { .. }) {
            self.transition_progress = TransitionProgress(0);
        }
        Some(effect)
    }

    pub const fn world_position(&self) -> Option<Position3> {
        self.world_position
    }

    pub fn set_world_position(&mut self, position: Position3) {
        self.world_position = Some(position);
    }

    /// Actor-owned typed route progress. Loaded node IDs stay distinct from
    /// serialized keys at the route-world boundary.
    pub const fn navigation_route(&self) -> RouteActor {
        self.navigation.route()
    }

    pub fn navigation_route_mut(&mut self) -> &mut RouteActor {
        self.navigation.route_mut()
    }

    pub const fn navigation_route_owned(&self) -> bool {
        self.navigation.route_owned()
    }

    pub fn seed_navigation_route_if_unowned(&mut self, route: RouteActor) -> RouteActor {
        self.navigation.seed_route_if_unowned(route)
    }

    pub const fn cached_navigation_record(&self) -> Option<CachedNavigationRecord> {
        self.navigation.cached_route_record()
    }

    pub fn seed_cached_navigation_record_if_unowned(
        &mut self,
        record: CachedNavigationRecord,
    ) -> CachedNavigationRecord {
        self.navigation.seed_cached_route_record_if_unowned(record)
    }

    pub fn set_cached_navigation_record(&mut self, record: CachedNavigationRecord) {
        self.navigation.set_cached_route_record(record);
    }

    /// Applies the measured state-6 route request to this actor lifetime.
    /// Current local state is authoritative; the queue is consumed later by
    /// the state's existing prologue seam.
    pub fn enter_navigation_state_six(&mut self, requested_node: i32, player_node: i32) -> bool {
        if !self.navigation.state_six_route_mut().prepare_entry(
            self.local_state,
            requested_node,
            player_node,
        ) {
            return false;
        }
        self.queue_local_state(LocalState(6));
        true
    }

    pub const fn actor_navigation(&self) -> &ActorNavigation {
        &self.navigation
    }

    pub fn actor_navigation_mut(&mut self) -> &mut ActorNavigation {
        &mut self.navigation
    }

    /// Returns the link-follow fields only after a typed boundary has
    /// projected them; absence is not interpreted as an all-zero actor.
    pub const fn navigation_link(&self) -> Option<LinkActorState> {
        self.navigation.link()
    }

    /// Installs a typed link-follow snapshot for this actor lifetime.
    pub fn set_navigation_link(&mut self, link: LinkActorState) {
        self.navigation.set_link(link);
    }

    /// Clears the typed projection when its source boundary delegates.
    pub fn clear_navigation_link(&mut self) {
        self.navigation.clear_link();
    }

    fn invalidate_navigation_graph(&mut self, generation: NavigationGraphGeneration) {
        self.navigation.replace_graph(generation);
    }

    /// Applies the measured current-node refresh to this actor's owned route.
    pub fn refresh_navigation_current_node(&mut self, current_node: NodeId) -> bool {
        self.navigation
            .route_mut()
            .refresh_current_node(current_node)
    }

    /// Current/saved secondary handles and Boolean primary occupancy for this
    /// actor lifetime; graph-reload validity is not inferred here.
    pub const fn navigation_claims(&self) -> &NavigationClaimState {
        self.navigation.claims()
    }

    pub fn navigation_claims_mut(&mut self) -> &mut NavigationClaimState {
        self.navigation.claims_mut()
    }

    /// Installs a typed secondary claim and marks its referenced primary
    /// occupied in this actor's Rust-owned claim state.
    pub fn acquire_navigation_claim(&mut self, claim: SecondaryTargetClaim) {
        self.navigation.claims_mut().acquire(claim);
    }

    /// Saves the current typed secondary handle for state-exit processing.
    pub fn save_navigation_claim(&mut self) {
        self.navigation.claims_mut().save_current();
    }

    /// Applies the bounded state-11 exit guard. The measured exit releases the
    /// saved primary occupancy but leaves current and saved handles installed.
    pub fn navigation_state11_exit(&mut self, exit_guard: bool) {
        self.navigation.claims_mut().state11_exit(exit_guard);
    }

    /// Applies the bounded state-12 timeout transition. The saved secondary
    /// snapshot remains installed after its referenced primary occupancy is
    /// released, matching the measured lifted state machine.
    pub fn navigation_state12_timeout(&mut self, timed_out: bool) {
        self.navigation.claims_mut().state12_timeout(timed_out);
    }

    /// Commits mode-10 claim cleanup using the active masks observed after
    /// callbacks. The returned plan describes which typed pointers were
    /// present at the boundary; mutation order remains owned by the claim
    /// model. Callers must run this after the ordered behavior callbacks.
    pub fn cleanup_navigation_after_behavior_callbacks(
        &mut self,
        change: BehaviorChange,
        current_after_callbacks: BehaviorPair,
    ) -> BehaviorClaimCleanup {
        let cleanup = change.cleanup_after_callbacks(
            current_after_callbacks,
            self.navigation.claims().current.is_some(),
            self.navigation.claims().saved.is_some(),
        );
        if cleanup.clear_secondary || cleanup.release_secondary_claim {
            self.navigation
                .claims_mut()
                .mode10_cleanup(change.previous.low, current_after_callbacks.low);
        }
        cleanup
    }

    /// Queues the next local state; later requests replace earlier requests.
    pub fn queue_local_state(&mut self, state: LocalState) {
        self.pending_local_state = Some(state);
        self.local_state_owned = true;
    }

    /// Applies a queued request at the common `0x2a1f30`/`0x2a4120`
    /// state-machine prologue. A changed request saves previous state,
    /// publishes current state, resets transition progress, then clears the
    /// pending request.
    pub fn apply_local_state_prologue(&mut self) -> Option<LocalStateCommit> {
        self.apply_local_state_commit_observing(
            LocalStateCommitOrder::StateMachinePrologue,
            |_, _| {},
        )
    }

    /// Applies the modeled `0x26EFC0` controller-entry commit after the
    /// retained `0x15E120(extension, 1)` pre-hook returns. This method does not
    /// call or interpret that pre-hook.
    pub fn apply_controller_local_state_commit(&mut self) -> Option<LocalStateCommit> {
        self.apply_local_state_commit_observing(LocalStateCommitOrder::ControllerEntry, |_, _| {})
    }

    fn apply_local_state_commit_observing<F>(
        &mut self,
        order: LocalStateCommitOrder,
        mut after_write: F,
    ) -> Option<LocalStateCommit>
    where
        F: FnMut(LocalStateCommitEffect, &Self),
    {
        let requested = self.pending_local_state?;
        let previous = self.local_state;
        let changed = requested != previous;
        if changed {
            match order {
                LocalStateCommitOrder::StateMachinePrologue => {
                    self.previous_local_state = previous;
                    after_write(LocalStateCommitEffect::SavePreviousState, self);
                    self.local_state = requested;
                    after_write(LocalStateCommitEffect::PublishCurrentState, self);
                    self.transition_progress = TransitionProgress(0);
                    after_write(LocalStateCommitEffect::ResetTransitionProgress, self);
                }
                LocalStateCommitOrder::ControllerEntry => {
                    self.transition_progress = TransitionProgress(0);
                    after_write(LocalStateCommitEffect::ResetTransitionProgress, self);
                    self.previous_local_state = previous;
                    after_write(LocalStateCommitEffect::SavePreviousState, self);
                    self.local_state = requested;
                    after_write(LocalStateCommitEffect::PublishCurrentState, self);
                }
            }
        }
        self.pending_local_state = None;
        after_write(LocalStateCommitEffect::ClearPendingRequest, self);

        Some(LocalStateCommit {
            requested,
            previous,
            current: self.local_state,
            changed,
        })
    }

    /// Applies a queued local-state request and reports only actual state
    /// transitions for callers that do not need the consumed-request detail.
    pub fn apply_local_state_request(&mut self) -> Option<LocalStateChange> {
        let commit = self.apply_local_state_prologue()?;
        commit.changed.then_some(LocalStateChange {
            previous: commit.previous,
            current: commit.current,
        })
    }

    /// Queues a replaceable behavior pair. The documented `(1, 0)` request is
    /// ignored while bit 0 is absent from the selected active low word.
    pub fn request_behavior(&mut self, requested: BehaviorPair) -> bool {
        if requested == (BehaviorPair { low: 1, high: 0 }) && self.active_behavior().low & 1 == 0 {
            return false;
        }
        self.pending_behavior = requested;
        self.behavior_owned = true;
        true
    }

    /// Refreshes lifted-owned fields while retaining Rust-owned state. The
    /// inactive behavior bank remains a compatibility input because other
    /// lifted paths can still update it. Local-state fields stop importing once
    /// a typed Rust request claims that state.
    pub fn refresh_compatibility_fields(&mut self, snapshot: ReaTickSnapshot) {
        self.mode = snapshot.mode;
        if !self.behavior_owned {
            self.mode10_active = snapshot.mode10_active;
            self.other_active = snapshot.other_active;
            self.previous_behavior = snapshot.previous_behavior;
            self.pending_behavior = snapshot.pending_behavior;
        } else {
            match self.mode {
                BehaviorMode::Mode10 => self.other_active = snapshot.other_active,
                BehaviorMode::Other => self.mode10_active = snapshot.mode10_active,
            }
        }
        if !self.local_state_owned {
            self.local_state = snapshot.local_state;
            self.previous_local_state = snapshot.previous_local_state;
            self.pending_local_state = snapshot.pending_local_state;
            self.transition_progress = snapshot.transition_progress;
        }
        if !self.awareness_cache_owned {
            self.awareness_cache = snapshot.awareness_cache;
        }
    }

    /// Reconciles commit inputs that still come from lifted writers before Rust
    /// applies the measured behavior transition at the typed seam.
    pub fn reconcile_lifted_commit(&mut self, snapshot: ReaTickSnapshot) {
        self.mode = snapshot.mode;
        match self.mode {
            BehaviorMode::Mode10 => self.mode10_active = snapshot.mode10_active,
            BehaviorMode::Other => self.other_active = snapshot.other_active,
        }
        self.previous_behavior = snapshot.previous_behavior;
        self.pending_behavior = snapshot.pending_behavior;
    }

    /// Describes the pending pair change without mutating active state.
    pub fn pending_behavior_change(&self) -> Option<BehaviorChange> {
        let previous = self.active_behavior();
        let current = self.pending_behavior;
        if previous == current {
            return None;
        }

        Some(BehaviorChange {
            mode: self.mode,
            previous,
            current,
        })
    }

    /// Applies a prepared transition when the pending pair has not changed.
    /// Returns false if the snapshot became stale.
    ///
    /// The remaining measured transition effects are intentionally separate:
    /// local-flag and navigation-progress reset, ordered callbacks, mode-10
    /// actor-array maintenance, and callback-ordered claim cleanup.
    pub fn apply_behavior_change(&mut self, change: BehaviorChange) -> bool {
        if change.mode != self.mode
            || change.previous != self.active_behavior()
            || change.current != self.pending_behavior
        {
            return false;
        }

        self.finish_behavior_tick(change.mode, change.previous)
    }

    /// Completes the selected class tick after its callback returns. The
    /// callback observes `selected` in the active bank and may replace the
    /// single pending pair; the final pending pair is what the original then
    /// installs in the mode-selected bank.
    pub fn finish_behavior_tick(&mut self, mode: BehaviorMode, selected: BehaviorPair) -> bool {
        self.finish_behavior_tick_with_pending(mode, selected, self.pending_behavior)
    }

    /// Completes a callback transaction using the final pending pair observed
    /// at the typed callback boundary. The explicit value covers lifted
    /// producers that replace the guest projection without entering the Rust
    /// request handler; that projection remains a TEMPORARY-SHIM until those
    /// producers are ported.
    pub fn finish_behavior_tick_with_pending(
        &mut self,
        mode: BehaviorMode,
        selected: BehaviorPair,
        pending_after_callbacks: BehaviorPair,
    ) -> bool {
        if mode != self.mode || selected != self.active_behavior() {
            return false;
        }

        self.pending_behavior = pending_after_callbacks;
        self.previous_behavior = selected;
        match mode {
            BehaviorMode::Mode10 => self.mode10_active = pending_after_callbacks,
            BehaviorMode::Other => self.other_active = pending_after_callbacks,
        }
        self.behavior_owned = true;
        true
    }

    /// Runs the measured synchronous behavior handover against Rust-owned
    /// state. The old pair is visible during message 0x3ea, the newly installed
    /// pair during 0x3e9, and a request queued by either callback is committed
    /// after both return. This models only the bounded 0x15c700/0x27b8b0 seam;
    /// it does not define scheduler re-entry or callback completion policy.
    pub fn run_behavior_handover(
        &mut self,
        mut dispatch: impl FnMut(&mut Self, BehaviorHandoverMessage),
    ) -> bool {
        let Some(change) = self.pending_behavior_change() else {
            return false;
        };
        let old_pair = change.previous;
        let installed_pair = change.current;

        self.previous_behavior = old_pair;
        self.set_active_behavior(installed_pair);
        self.behavior_handover_words = BehaviorHandoverWords::default();
        self.transition_progress = TransitionProgress(0);
        if self.local_state != LocalState(0) {
            self.pending_local_state = Some(LocalState(0));
        }
        self.behavior_owned = true;

        self.set_active_behavior(old_pair);
        dispatch(self, BehaviorHandoverMessage::Previous);
        self.set_active_behavior(installed_pair);
        dispatch(self, BehaviorHandoverMessage::Current);

        let pending_after_callbacks = self.pending_behavior;
        self.previous_behavior = old_pair;
        self.pending_behavior = pending_after_callbacks;
        self.set_active_behavior(pending_after_callbacks);
        self.behavior_owned = true;
        true
    }

    fn set_active_behavior(&mut self, pair: BehaviorPair) {
        match self.mode {
            BehaviorMode::Mode10 => self.mode10_active = pair,
            BehaviorMode::Other => self.other_active = pair,
        }
    }

    /// Completes the typed behavior-pair state transition after the retained
    /// commit's measured callback sequence has run. The commit adapter remains
    /// responsible for its side effects until those services are ported.
    pub fn commit_pending_behavior(&mut self) -> bool {
        let Some(change) = self.pending_behavior_change() else {
            return false;
        };
        self.apply_behavior_change(change)
    }

    /// Returns the typed state boundary used by an owner-state trace.
    pub const fn snapshot(&self) -> ReaTickSnapshot {
        ReaTickSnapshot {
            mode: self.mode,
            mode10_active: self.mode10_active,
            other_active: self.other_active,
            previous_behavior: self.previous_behavior,
            pending_behavior: self.pending_behavior,
            local_state: self.local_state,
            previous_local_state: self.previous_local_state,
            pending_local_state: self.pending_local_state,
            transition_progress: self.transition_progress,
            awareness_cache: self.awareness_cache,
        }
    }
}

/// Stable identity supplied by the actor-pool lifecycle seam. Reusing a pool
/// slot requires a new generation before its state can be attached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReaActorIdentity {
    pub slot: u16,
    pub generation: u32,
}

/// A request referenced an actor lifetime that is no longer live in the
/// Rust-owned store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaActorStoreError {
    StaleIdentity,
    StaleNavigationGraph,
    NavigationGraphGenerationExhausted,
}

/// Rust-owned identity of one loaded navigation graph. This generation is a
/// native lifetime boundary; it is not inferred from an original pointer or
/// claimed as an observed Xbox counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigationGraphGeneration(pub u32);

/// Secondary claim snapshot tagged at the graph boundary that resolved it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphScopedSecondaryClaim {
    pub graph: NavigationGraphGeneration,
    pub claim: SecondaryTargetClaim,
}

/// A fully resolved navigation result tied to the loaded graph that produced
/// it. The caller constructs this only after route lookup and target
/// resolution both succeed; the store commits its node and movement target as
/// one native state transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedNavigationStep {
    pub graph: NavigationGraphGeneration,
    pub next_node: NodeId,
    pub movement_target: Position3,
}

/// Resulting state effect of the measured point-follow search sentinels.
/// This records the typed Rust owner transition; the compatibility caller
/// projects the cached-path scalar at its boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointFollowEffect {
    SelectedNodeCommitted(NodeId),
    CachedRecordTargetCommitted { point_follow_scalar: f32 },
    AlertAbort,
    UnclassifiedNegative(i32),
    SelectedNodePositionUnavailable(NodeId),
    CachedRecordUnavailable,
}

pub const CACHED_RECORD_POINT_FOLLOW_SCALAR: f32 = 1.4;

/// Rust-owned per-actor state indexed by a typed pool slot and generation.
/// The caller must provide the generation on allocation and prune using the
/// active identity set at the lifecycle boundary; this type does not infer
/// lifetime from guest memory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReaTickActorStore {
    actors: Vec<(ReaActorIdentity, ReaTickState)>,
    last_generation: Vec<(u16, u32)>,
    /// Shared Boolean occupancy of primary navigation records. Actor-local
    /// claim state retains the current/saved secondary handles; this map is the
    /// world-level value observed by other actors during selection.
    primary_occupancy: BTreeMap<PrimaryTargetId, bool>,
    navigation_graph: Option<NavigationGraphGeneration>,
}

impl ReaTickActorStore {
    pub const fn new_empty() -> Self {
        Self {
            actors: Vec::new(),
            last_generation: Vec::new(),
            primary_occupancy: BTreeMap::new(),
            navigation_graph: None,
        }
    }

    pub fn len(&self) -> usize {
        self.actors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    /// Reads the shared Boolean on a primary record. This is deliberately not
    /// a reference count or an owner token; the original field is one bit.
    pub fn primary_is_occupied(&self, primary: PrimaryTargetId) -> bool {
        self.primary_occupancy
            .get(&primary)
            .copied()
            .unwrap_or(false)
    }

    /// Recomputes the native Boolean projection from the live actor-owned claim
    /// states. This is a Rust lifetime policy; REA has not established how the
    /// original tears down overlapping actor claims.
    fn refresh_primary_occupancy(&mut self, primary: PrimaryTargetId) {
        let occupied = self
            .actors
            .iter()
            .any(|(_, state)| state.navigation_claims().primary_is_claimed(primary));
        self.primary_occupancy.insert(primary, occupied);
    }

    pub const fn navigation_graph_generation(&self) -> Option<NavigationGraphGeneration> {
        self.navigation_graph
    }

    /// Replaces the Rust-owned graph identity and invalidates every projection
    /// that contains IDs or claims into the prior graph. The original loader's
    /// repeated-load allocation and claim-remap behavior is unresolved, so this
    /// is an explicit native ownership policy, not a claim about lifted code.
    pub fn replace_navigation_graph(
        &mut self,
    ) -> Result<NavigationGraphGeneration, ReaActorStoreError> {
        let next = match self.navigation_graph {
            Some(current) => NavigationGraphGeneration(
                current
                    .0
                    .checked_add(1)
                    .ok_or(ReaActorStoreError::NavigationGraphGenerationExhausted)?,
            ),
            None => NavigationGraphGeneration(1),
        };
        self.primary_occupancy.clear();
        for (_, state) in &mut self.actors {
            state.invalidate_navigation_graph(next);
        }
        self.navigation_graph = Some(next);
        Ok(next)
    }

    /// Starts one actor lifetime and mints its generation from this explicit
    /// lifecycle event. A slot cannot be spawned twice until its current
    /// identity is retired or pruned from an authoritative live-set snapshot.
    /// Generation exhaustion fails closed instead of reusing an identity.
    pub fn spawn(&mut self, slot: u16, snapshot: ReaTickSnapshot) -> Option<ReaActorIdentity> {
        if self
            .actors
            .iter()
            .any(|(identity, _)| identity.slot == slot)
        {
            return None;
        }
        let generation = if let Some((_, last)) = self
            .last_generation
            .iter_mut()
            .find(|(known_slot, _)| *known_slot == slot)
        {
            *last = last.checked_add(1)?;
            *last
        } else {
            self.last_generation.push((slot, 1));
            1
        };
        let identity = ReaActorIdentity { slot, generation };
        let mut state = ReaTickState::new(snapshot);
        if let Some(graph) = self.navigation_graph {
            state.navigation.replace_graph(graph);
        }
        self.actors.push((identity, state));
        Some(identity)
    }

    /// Starts one actor lifetime in the first available typed slot. The slot is
    /// selected by this Rust owner; a caller-provided guest address or reusable
    /// pool index never supplies the generation.
    pub fn spawn_next(&mut self, snapshot: ReaTickSnapshot) -> Option<ReaActorIdentity> {
        (0..=u16::MAX).find_map(|slot| self.spawn(slot, snapshot))
    }

    /// Uses a constructor-published slot when its object/extension tuple was
    /// validated; other discovery paths retain the native Rust slot allocator.
    pub fn spawn_with_observed_slot(
        &mut self,
        observed_slot: Option<u16>,
        snapshot: ReaTickSnapshot,
    ) -> Option<ReaActorIdentity> {
        match observed_slot {
            Some(slot) => self.spawn(slot, snapshot),
            None => self.spawn_next(snapshot),
        }
    }

    pub fn state(&self, identity: ReaActorIdentity) -> Option<&ReaTickState> {
        self.actors
            .iter()
            .find(|(candidate, _)| *candidate == identity)
            .map(|(_, state)| state)
    }

    pub(crate) fn state_mut(&mut self, identity: ReaActorIdentity) -> Option<&mut ReaTickState> {
        self.actors
            .iter_mut()
            .find(|(candidate, _)| *candidate == identity)
            .map(|(_, state)| state)
    }

    pub fn owns_behavior_pairs(
        &self,
        identity: ReaActorIdentity,
    ) -> Result<bool, ReaActorStoreError> {
        self.state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::owns_behavior_pairs)
    }

    pub fn set_controller_target_handoff(
        &mut self,
        identity: ReaActorIdentity,
        handoff: ControllerTargetHandoff,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.set_controller_target_handoff(handoff))
    }

    pub fn controller_target_handoff(
        &self,
        identity: ReaActorIdentity,
    ) -> Result<Option<ControllerTargetHandoff>, ReaActorStoreError> {
        self.state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::controller_target_handoff)
    }

    pub fn apply_controller_target_request(
        &mut self,
        identity: ReaActorIdentity,
    ) -> Result<Option<ControllerTargetEffect>, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::apply_controller_target_request)
    }

    /// Applies the bounded selected-goal setter effect to the exact actor
    /// generation that owns the controller target state.
    pub fn apply_measured_selected_goal(
        &mut self,
        identity: ReaActorIdentity,
        target: u32,
    ) -> Result<Option<SelectedGoalEffect>, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.apply_measured_selected_goal(target))
    }

    /// Installs the typed recipient fields read by the group-alert adapter for
    /// this exact actor lifetime.
    pub fn set_awareness_recipient(
        &mut self,
        identity: ReaActorIdentity,
        recipient: AwarenessRecipientState,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.set_awareness_recipient(recipient))
    }

    pub fn awareness_recipient(
        &self,
        identity: ReaActorIdentity,
    ) -> Result<Option<AwarenessRecipientState>, ReaActorStoreError> {
        self.state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::awareness_recipient)
    }

    /// Applies one ordered awareness callback to the recipient state owned by
    /// this exact actor generation. A live actor without recipient state is
    /// distinct from a stale identity.
    pub fn with_awareness_recipient_mut<R>(
        &mut self,
        identity: ReaActorIdentity,
        update: impl FnOnce(&mut AwarenessRecipientState) -> R,
    ) -> Result<Option<R>, ReaActorStoreError> {
        let state = self
            .state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?;
        Ok(state.awareness_recipient.as_mut().map(update))
    }

    /// Applies the measured recipient decision tree from `0x267FF0` to
    /// Rust-owned state. The resolved target is supplied by the caller because
    /// source-specific target selection remains at the seam. Calls into the
    /// world stay in source order, with callback-visible actor writes.
    pub fn process_group_alert_recipient(
        &mut self,
        identity: ReaActorIdentity,
        excluded_actor: ActorId,
        incoming_groups: GroupMask,
        target: NodeId,
        world: &mut impl AwarenessRecipientWorld,
    ) -> Result<AwarenessRecipientEffect, ReaActorStoreError> {
        let state = self
            .state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?;
        let Some(recipient) = state.awareness_recipient.as_mut() else {
            return Ok(AwarenessRecipientEffect::Skipped);
        };
        if recipient.actor == excluded_actor {
            return Ok(AwarenessRecipientEffect::Skipped);
        }

        let alertable = recipient.actor_flags & ACTOR_ALERTABLE != 0;
        let alerted = recipient.local_flags & LOCAL_ALERTED != 0;
        if alertable && !alerted {
            recipient.alert_target = Some(target);
            recipient.local_flags |= LOCAL_ALERTED;
            let removed_groups = GroupMask(!incoming_groups.0 & recipient.group_mask.0);
            if removed_groups.0 != 0 {
                world.leave_groups(recipient, removed_groups);
                recipient.group_mask = GroupMask(recipient.group_mask.0 & incoming_groups.0);
            }
            return Ok(AwarenessRecipientEffect::NewlyAlerted { removed_groups });
        }

        if !alertable || recipient.behavior_flags & BEHAVIOR_SKIP_CHECK != 0 || !alerted {
            if recipient.listen_flags & 1 != 0 && world.class_listens(recipient.class_selector) {
                recipient.local_flags |= LOCAL_ALERTED;
                return Ok(AwarenessRecipientEffect::MarkedAlerted);
            }
            return Ok(AwarenessRecipientEffect::Skipped);
        }

        let code = if !world.candidate_check(recipient) {
            if world.actor_busy(recipient) {
                return Ok(AwarenessRecipientEffect::SuppressedBusy);
            }
            8
        } else if recipient.behavior_flags & BEHAVIOR_RAISE_FULL == 0 {
            if world.actor_busy(recipient) {
                return Ok(AwarenessRecipientEffect::SuppressedBusy);
            }
            4
        } else {
            world.raise(recipient, 0x200, false);
            return Ok(AwarenessRecipientEffect::Raised {
                code: 0x200,
                three_args: false,
            });
        };
        world.raise(recipient, code, true);
        Ok(AwarenessRecipientEffect::Raised {
            code,
            three_args: true,
        })
    }

    /// Queues a local-state request for exactly one live actor generation.
    pub fn queue_local_state(
        &mut self,
        identity: ReaActorIdentity,
        requested: LocalState,
    ) -> Result<(), ReaActorStoreError> {
        let state = self
            .state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?;
        state.queue_local_state(requested);
        Ok(())
    }

    /// Applies the typed state-6 route request to one live actor generation.
    /// The caller invokes guest route services only after this store operation
    /// releases its mutable borrow/lock.
    pub fn enter_navigation_state_six(
        &mut self,
        identity: ReaActorIdentity,
        requested_node: i32,
        player_node: i32,
    ) -> Result<bool, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.enter_navigation_state_six(requested_node, player_node))
    }

    /// Consumes that actor's queued local-state request at its tick prologue.
    /// Stale generations are rejected separately from a live actor with no
    /// queued request.
    pub fn apply_local_state_prologue(
        &mut self,
        identity: ReaActorIdentity,
    ) -> Result<Option<LocalStateCommit>, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::apply_local_state_prologue)
    }

    /// Applies the bounded controller-entry state commit to one live actor.
    /// The retained pre-hook and state-specific work remain outside this seam.
    pub fn apply_controller_local_state_commit(
        &mut self,
        identity: ReaActorIdentity,
    ) -> Result<Option<LocalStateCommit>, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::apply_controller_local_state_commit)
    }

    /// Applies measured target-handoff effects to one live Rust-owned actor.
    /// The caller supplies the typed handoff decision and any successful goal
    /// selection result; a stale actor generation is rejected before effects
    /// can change actor-owned state.
    pub fn apply_target_handoff(
        &mut self,
        identity: ReaActorIdentity,
        decision: &crate::rea_target_handoff::HandoffDecision,
        goal_selection: Option<crate::rea_target_selector::TargetSelectorOutcome>,
    ) -> Result<crate::rea_target_handoff::HandoffCommit, ReaActorStoreError> {
        let state = self
            .state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?;
        Ok(crate::rea_target_handoff::apply_target_handoff(
            state,
            decision,
            goal_selection,
        ))
    }

    /// Applies the measured local-state prologue at a compatibility boundary.
    /// Lifted-owned fields are refreshed before consuming the request; once a
    /// typed caller owns local state, stale guest fields cannot replace it.
    pub fn apply_compatibility_local_state_prologue(
        &mut self,
        identity: ReaActorIdentity,
        snapshot: ReaTickSnapshot,
    ) -> Result<Option<LocalStateCommit>, ReaActorStoreError> {
        let state = self
            .state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?;
        state.refresh_compatibility_fields(snapshot);
        Ok(state.apply_local_state_prologue())
    }

    /// Queues a behavior request for exactly one live actor generation.
    ///
    /// `false` means the measured `(1, 0)` guard rejected the request; a stale
    /// actor identity is reported separately. Callback execution remains the
    /// caller's responsibility at the typed runtime seam.
    pub fn request_behavior(
        &mut self,
        identity: ReaActorIdentity,
        requested: BehaviorPair,
    ) -> Result<bool, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.request_behavior(requested))
    }

    /// Returns the pending transition for a live actor, if its active pair
    /// differs from the requested pair.
    pub fn pending_behavior_change(
        &self,
        identity: ReaActorIdentity,
    ) -> Result<Option<BehaviorChange>, ReaActorStoreError> {
        self.state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::pending_behavior_change)
    }

    /// Applies a transition after its typed pre-commit services have run.
    /// Returns `false` when the captured transition no longer matches the
    /// actor's current mode, active pair, or pending request.
    pub fn apply_behavior_change(
        &mut self,
        identity: ReaActorIdentity,
        change: BehaviorChange,
    ) -> Result<bool, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.apply_behavior_change(change))
    }

    /// Completes the selected behavior tick after its callback returns. Any
    /// callback replacement of the single pending pair is installed, while a
    /// stale selected pair or actor generation is rejected.
    pub fn finish_behavior_tick(
        &mut self,
        identity: ReaActorIdentity,
        mode: BehaviorMode,
        selected: BehaviorPair,
    ) -> Result<bool, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.finish_behavior_tick(mode, selected))
    }

    /// Completes a selected behavior tick with the final request written by
    /// its ordered callback sequence. Stale generations and selected pairs are
    /// rejected before the owned pair is changed.
    pub fn finish_behavior_tick_with_pending(
        &mut self,
        identity: ReaActorIdentity,
        mode: BehaviorMode,
        selected: BehaviorPair,
        pending_after_callbacks: BehaviorPair,
    ) -> Result<bool, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| {
                state.finish_behavior_tick_with_pending(mode, selected, pending_after_callbacks)
            })
    }

    /// Imports lifted fields at a measured commit boundary while preserving
    /// any state already owned by the typed actor.
    pub fn reconcile_lifted_commit(
        &mut self,
        identity: ReaActorIdentity,
        snapshot: ReaTickSnapshot,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.reconcile_lifted_commit(snapshot))
    }

    /// Imports lifted-owned fields while retaining any behavior state already
    /// accepted by the Rust request seam.
    pub fn refresh_compatibility_snapshot(
        &mut self,
        identity: ReaActorIdentity,
        snapshot: ReaTickSnapshot,
    ) -> bool {
        let Some(state) = self.state_mut(identity) else {
            return false;
        };
        state.refresh_compatibility_fields(snapshot);
        true
    }

    /// Refreshes route state only for the matching live actor generation.
    pub fn refresh_navigation_current_node(
        &mut self,
        identity: ReaActorIdentity,
        current_node: NodeId,
    ) -> Result<bool, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.refresh_navigation_current_node(current_node))
    }

    /// Commits a completely resolved next-node/target pair only while both
    /// the world and actor still refer to the graph that produced it. This is
    /// a native atomicity policy; REA has not established the lifted claim or
    /// route rollback behavior when point-follow resolution fails.
    pub fn commit_resolved_navigation_step(
        &mut self,
        identity: ReaActorIdentity,
        step: ResolvedNavigationStep,
    ) -> Result<(), ReaActorStoreError> {
        if self.navigation_graph != Some(step.graph) {
            return Err(ReaActorStoreError::StaleNavigationGraph);
        }
        let state = self
            .state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?;
        if state.actor_navigation().graph_generation() != Some(step.graph) {
            return Err(ReaActorStoreError::StaleNavigationGraph);
        }
        let navigation = state.actor_navigation_mut();
        navigation.set_next_node(step.next_node);
        navigation.set_movement_target(step.movement_target);
        Ok(())
    }

    /// Applies the bounded point-follow search result to Rust-owned navigation
    /// state. The measured -2 path aborts without changing navigation; -1
    /// updates only the movement target and returns the measured scalar effect;
    /// nonnegative results commit node and target together. Other negatives
    /// remain unclassified and do not mutate state.
    pub fn apply_point_follow_result(
        &mut self,
        identity: ReaActorIdentity,
        graph: NavigationGraphGeneration,
        result: PointFollowSearchResult,
        selected_position: Option<Position3>,
        cached_position: Option<Position3>,
    ) -> Result<PointFollowEffect, ReaActorStoreError> {
        match result {
            PointFollowSearchResult::SelectedNode(node) => {
                let Some(movement_target) = selected_position else {
                    return Ok(PointFollowEffect::SelectedNodePositionUnavailable(node));
                };
                self.commit_resolved_navigation_step(
                    identity,
                    ResolvedNavigationStep {
                        graph,
                        next_node: node,
                        movement_target,
                    },
                )?;
                Ok(PointFollowEffect::SelectedNodeCommitted(node))
            }
            PointFollowSearchResult::CachedRecord => {
                let Some(movement_target) = cached_position else {
                    return Ok(PointFollowEffect::CachedRecordUnavailable);
                };
                if self.navigation_graph != Some(graph) {
                    return Err(ReaActorStoreError::StaleNavigationGraph);
                }
                let state = self
                    .state_mut(identity)
                    .ok_or(ReaActorStoreError::StaleIdentity)?;
                if state.actor_navigation().graph_generation() != Some(graph) {
                    return Err(ReaActorStoreError::StaleNavigationGraph);
                }
                state
                    .actor_navigation_mut()
                    .set_movement_target(movement_target);
                Ok(PointFollowEffect::CachedRecordTargetCommitted {
                    point_follow_scalar: CACHED_RECORD_POINT_FOLLOW_SCALAR,
                })
            }
            PointFollowSearchResult::AlertAbort => Ok(PointFollowEffect::AlertAbort),
            PointFollowSearchResult::OtherNegative(value) => {
                Ok(PointFollowEffect::UnclassifiedNegative(value))
            }
        }
    }

    /// Stores the selected next primary node on the matching actor lifetime.
    pub fn set_navigation_next_node(
        &mut self,
        identity: ReaActorIdentity,
        next_node: NodeId,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?
            .actor_navigation_mut()
            .set_next_node(next_node);
        Ok(())
    }

    /// Reads the generation-owned route progress used by the route-advance
    /// adapter. Stale guest extension handles cannot borrow a replacement
    /// actor's route.
    pub fn navigation_route(
        &self,
        identity: ReaActorIdentity,
    ) -> Result<RouteActor, ReaActorStoreError> {
        self.state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::navigation_route)
    }

    /// Imports the measured guest snapshot once, until a Rust route handler
    /// mutates the actor-owned route state.
    pub fn seed_navigation_route_if_unowned(
        &mut self,
        identity: ReaActorIdentity,
        snapshot: RouteActor,
    ) -> Result<RouteActor, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.seed_navigation_route_if_unowned(snapshot))
    }

    /// Commits route progress to the matching actor generation.
    pub fn set_navigation_route(
        &mut self,
        identity: ReaActorIdentity,
        route: RouteActor,
    ) -> Result<(), ReaActorStoreError> {
        *self
            .state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?
            .navigation_route_mut() = route;
        Ok(())
    }

    /// Imports or commits the cached route-record token without interpreting
    /// it as a node ID or claiming that the original paired it with a graph
    /// generation.
    pub fn seed_cached_navigation_record_if_unowned(
        &mut self,
        identity: ReaActorIdentity,
        record: CachedNavigationRecord,
    ) -> Result<CachedNavigationRecord, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.seed_cached_navigation_record_if_unowned(record))
    }

    pub fn set_cached_navigation_record(
        &mut self,
        identity: ReaActorIdentity,
        record: CachedNavigationRecord,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?
            .set_cached_navigation_record(record);
        Ok(())
    }

    /// Stores the native movement target independently of the actor object's
    /// world position and of any linked actor position.
    pub fn set_navigation_movement_target(
        &mut self,
        identity: ReaActorIdentity,
        target: Position3,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?
            .actor_navigation_mut()
            .set_movement_target(target);
        Ok(())
    }

    /// Reads the Rust-owned route-link state for one live actor generation.
    pub fn navigation_link(
        &self,
        identity: ReaActorIdentity,
    ) -> Result<Option<LinkActorState>, ReaActorStoreError> {
        self.state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::navigation_link)
    }

    /// Installs a route-link snapshot only on its matching actor generation.
    pub fn set_navigation_link(
        &mut self,
        identity: ReaActorIdentity,
        link: LinkActorState,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.set_navigation_link(link))
    }

    /// Drops the route-link projection at an explicit delegation boundary.
    pub fn clear_navigation_link(
        &mut self,
        identity: ReaActorIdentity,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::clear_navigation_link)
    }

    /// Reads the selected behavior-route cursor for one live actor generation.
    pub fn behavior_route_cursor(
        &self,
        identity: ReaActorIdentity,
    ) -> Result<Option<u32>, ReaActorStoreError> {
        self.state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::behavior_route_cursor)
    }

    /// Installs the cursor adopted by a selected Rust route handler.
    pub fn set_behavior_route_cursor(
        &mut self,
        identity: ReaActorIdentity,
        cursor: u32,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.set_behavior_route_cursor(cursor))
    }

    /// Drops the cursor at the full-tick/delegation boundary while unported
    /// executor writers still run through the guest compatibility seam.
    pub fn clear_behavior_route_cursor(
        &mut self,
        identity: ReaActorIdentity,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::clear_behavior_route_cursor)
    }

    /// Acquires a secondary target only for the matching live actor generation.
    pub fn acquire_navigation_claim(
        &mut self,
        identity: ReaActorIdentity,
        claim: GraphScopedSecondaryClaim,
    ) -> Result<(), ReaActorStoreError> {
        if self.state(identity).is_none() {
            return Err(ReaActorStoreError::StaleIdentity);
        }
        if self.navigation_graph != Some(claim.graph) {
            return Err(ReaActorStoreError::StaleNavigationGraph);
        }
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?
            .acquire_navigation_claim(claim.claim);
        self.refresh_primary_occupancy(claim.claim.primary);
        Ok(())
    }

    /// Saves the current secondary target only for the matching actor lifetime.
    pub fn save_navigation_claim(
        &mut self,
        identity: ReaActorIdentity,
    ) -> Result<(), ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(ReaTickState::save_navigation_claim)
    }

    /// Applies the measured state-11 release boundary to one live actor.
    pub fn navigation_state11_exit(
        &mut self,
        identity: ReaActorIdentity,
        exit_guard: bool,
    ) -> Result<(), ReaActorStoreError> {
        let state = self
            .state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?;
        let released = if exit_guard {
            state.navigation_claims().saved.map(|claim| claim.primary)
        } else {
            None
        };
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?
            .navigation_state11_exit(exit_guard);
        if let Some(primary) = released {
            self.refresh_primary_occupancy(primary);
        }
        Ok(())
    }

    /// Applies the bounded state-12 timeout transition for one live actor.
    pub fn navigation_state12_timeout(
        &mut self,
        identity: ReaActorIdentity,
        timed_out: bool,
    ) -> Result<(), ReaActorStoreError> {
        let state = self
            .state(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?;
        let released = if timed_out {
            state.navigation_claims().saved.map(|claim| claim.primary)
        } else {
            None
        };
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?
            .navigation_state12_timeout(timed_out);
        if let Some(primary) = released {
            self.refresh_primary_occupancy(primary);
        }
        Ok(())
    }

    /// Applies post-callback mode-10 claim cleanup to one live actor
    /// generation. Stale identities cannot release another actor's claim.
    pub fn cleanup_navigation_after_behavior_callbacks(
        &mut self,
        identity: ReaActorIdentity,
        change: BehaviorChange,
        current_after_callbacks: BehaviorPair,
    ) -> Result<BehaviorClaimCleanup, ReaActorStoreError> {
        let (current_primary, saved_primary) = {
            let claims = self
                .state(identity)
                .ok_or(ReaActorStoreError::StaleIdentity)?
                .navigation_claims();
            (
                claims.current.map(|claim| claim.primary),
                claims.saved.map(|claim| claim.primary),
            )
        };
        let cleanup = self
            .state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)?
            .cleanup_navigation_after_behavior_callbacks(change, current_after_callbacks);
        if cleanup.clear_secondary {
            if let Some(primary) = current_primary {
                self.refresh_primary_occupancy(primary);
            }
        }
        if cleanup.release_secondary_claim {
            if let Some(primary) = saved_primary {
                self.refresh_primary_occupancy(primary);
            }
        }
        Ok(cleanup)
    }

    pub fn retire(&mut self, identity: ReaActorIdentity) -> bool {
        let Some(index) = self
            .actors
            .iter()
            .position(|(candidate, _)| *candidate == identity)
        else {
            return false;
        };
        let affected: Vec<_> = self.actors[index]
            .1
            .navigation_claims()
            .occupied_primaries()
            .collect();
        self.actors.swap_remove(index);
        for primary in affected {
            self.refresh_primary_occupancy(primary);
        }
        true
    }

    /// Drops actors absent from the authoritative pool-lifecycle snapshot.
    pub fn retain_live(&mut self, live: &[ReaActorIdentity]) {
        let affected: Vec<_> = self
            .primary_occupancy
            .iter()
            .filter_map(|(primary, occupied)| occupied.then_some(*primary))
            .collect();
        self.actors.retain(|(identity, _)| live.contains(identity));
        for primary in affected {
            self.refresh_primary_occupancy(primary);
        }
    }

    /// Stable typed rows for diagnostics and contract tests.
    pub fn trace_rows(&self) -> impl Iterator<Item = (ReaActorIdentity, ReaTickSnapshot)> + '_ {
        self.actors
            .iter()
            .map(|(identity, state)| (*identity, state.snapshot()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(mode: BehaviorMode, active: BehaviorPair) -> ReaTickState {
        ReaTickState::new(ReaTickSnapshot {
            mode,
            mode10_active: if mode == BehaviorMode::Mode10 {
                active
            } else {
                BehaviorPair { low: 0x40, high: 4 }
            },
            other_active: if mode == BehaviorMode::Other {
                active
            } else {
                BehaviorPair { low: 0x20, high: 3 }
            },
            previous_behavior: BehaviorPair { low: 0, high: 0 },
            pending_behavior: BehaviorPair { low: 0, high: 0 },
            local_state: LocalState(4),
            previous_local_state: LocalState(3),
            pending_local_state: None,
            transition_progress: TransitionProgress(0x4020_0000),
            awareness_cache: AwarenessCache::from_raw(0),
        })
    }

    #[test]
    fn actor_tick_timers_are_owned_per_actor_and_decay_in_place() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.set_actor_tick_timers(ActorTickTimers {
            primary: 0.75,
            secondary: 0.25,
        });

        actor.decay_actor_tick_timers(0.5);

        assert_eq!(
            actor.actor_tick_timers(),
            ActorTickTimers {
                primary: 0.25,
                secondary: -0.25,
            }
        );
    }

    #[test]
    fn actor_tick_prelude_mutates_rust_owned_timers_at_the_measured_boundary() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.set_actor_tick_timers(ActorTickTimers {
            primary: 0.75,
            secondary: 0.25,
        });
        let mut admission = ActorTickAdmission {
            mode: 7,
            global_tick_blocked: false,
            slot_connected: true,
            slot: 0,
            minimum_slot: 1,
            actor_permission_check_required: false,
            actor_permission_granted: false,
            actor_flag_4_set: false,
        };

        assert_eq!(
            actor.run_actor_tick_prelude(admission, 0.5),
            ActorTickAdmissionResult::RejectedBelowMinimumSlot
        );
        assert_eq!(actor.actor_tick_timers().primary, 0.25);

        admission.slot_connected = false;
        assert_eq!(
            actor.run_actor_tick_prelude(admission, 0.5),
            ActorTickAdmissionResult::RejectedDisconnectedSlot
        );
        assert_eq!(actor.actor_tick_timers().primary, 0.25);
    }

    #[test]
    fn awareness_cache_keeps_the_full_dword_after_rust_takes_ownership() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.refresh_compatibility_fields(ReaTickSnapshot {
            awareness_cache: AwarenessCache::from_raw(0x100),
            ..actor.snapshot()
        });
        assert_eq!(actor.awareness_cache().raw_result(), 0x100);
        assert!(!actor.owns_awareness_cache());

        actor.set_awareness_cache(AwarenessCache::from_raw(0x1234_5600));
        actor.refresh_compatibility_fields(ReaTickSnapshot {
            awareness_cache: AwarenessCache::from_raw(0xFF),
            ..actor.snapshot()
        });
        assert_eq!(actor.awareness_cache().raw_result(), 0x1234_5600);
        assert!(actor.owns_awareness_cache());
    }

    #[test]
    fn controller_target_commit_resets_shared_progress_only_when_target_changes() {
        let mut changed = state(BehaviorMode::Other, BehaviorPair::default());
        changed.set_controller_target_handoff(ControllerTargetHandoff::new(0x120, 0x240, 0x80));
        assert_eq!(
            changed.apply_controller_target_request(),
            Some(ControllerTargetEffect::TargetChanged {
                previous: 0x120,
                current: 0x240,
            })
        );
        assert_eq!(changed.transition_progress(), TransitionProgress(0));
        assert_eq!(
            changed.controller_target_handoff(),
            Some(ControllerTargetHandoff::new(0x240, -1, 0x120))
        );

        let mut equal = state(BehaviorMode::Other, BehaviorPair::default());
        equal.set_controller_target_handoff(ControllerTargetHandoff::new(0x120, 0x120, 0x80));
        assert_eq!(
            equal.apply_controller_target_request(),
            Some(ControllerTargetEffect::EqualRequestRetired { target: 0x120 })
        );
        assert_eq!(equal.transition_progress(), TransitionProgress(0x4020_0000));
        assert_eq!(
            equal.controller_target_handoff(),
            Some(ControllerTargetHandoff::new(0x120, -1, 0x80))
        );

        let mut negative = state(BehaviorMode::Other, BehaviorPair::default());
        negative.set_controller_target_handoff(ControllerTargetHandoff::new(0x120, -2, 0x80));
        assert_eq!(
            negative.apply_controller_target_request(),
            Some(ControllerTargetEffect::NegativePendingSkipped { pending: -2 })
        );
        assert_eq!(
            negative.transition_progress(),
            TransitionProgress(0x4020_0000)
        );
        assert_eq!(
            negative.controller_target_handoff(),
            Some(ControllerTargetHandoff::new(0x120, -2, 0x80))
        );
    }

    #[test]
    fn measured_selected_goal_lifecycle_resets_progress_once() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.set_controller_target_handoff(ControllerTargetHandoff::new(0x240, -1, 0x120));

        assert_eq!(
            actor.apply_measured_selected_goal(0x360),
            Some(SelectedGoalEffect::SelectedGoalInstalled {
                previous: 0x240,
                current: 0x360,
            })
        );
        assert_eq!(actor.transition_progress(), TransitionProgress(0));
        assert_eq!(
            actor.controller_target_handoff(),
            Some(ControllerTargetHandoff::new(0x360, 0x360, 0x240))
        );

        assert_eq!(
            actor.apply_controller_target_request(),
            Some(ControllerTargetEffect::EqualRequestRetired { target: 0x360 })
        );
        assert_eq!(actor.transition_progress(), TransitionProgress(0));
        assert_eq!(
            actor.controller_target_handoff(),
            Some(ControllerTargetHandoff::new(0x360, -1, 0x240))
        );
    }

    #[test]
    fn controller_target_state_is_scoped_to_the_actor_generation() {
        let mut store = ReaTickActorStore::new_empty();
        let snapshot = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let actor = store.spawn(7, snapshot).unwrap();
        store
            .set_controller_target_handoff(actor, ControllerTargetHandoff::new(0x120, 0x240, 0x80))
            .unwrap();
        assert_eq!(
            store.controller_target_handoff(actor).unwrap(),
            Some(ControllerTargetHandoff::new(0x120, 0x240, 0x80))
        );

        assert!(store.retire(actor));
        let replacement = store.spawn(7, snapshot).unwrap();
        assert_ne!(replacement.generation, actor.generation);
        assert_eq!(
            store.controller_target_handoff(actor),
            Err(ReaActorStoreError::StaleIdentity)
        );
        assert_eq!(store.controller_target_handoff(replacement).unwrap(), None);
    }

    #[test]
    fn selected_goal_update_uses_the_exact_actor_generation() {
        let mut store = ReaTickActorStore::new_empty();
        let snapshot = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let actor = store.spawn(7, snapshot).unwrap();
        store
            .set_controller_target_handoff(actor, ControllerTargetHandoff::new(0x240, -1, 0x120))
            .unwrap();

        assert_eq!(
            store.apply_measured_selected_goal(actor, 0x360),
            Ok(Some(SelectedGoalEffect::SelectedGoalInstalled {
                previous: 0x240,
                current: 0x360,
            }))
        );
        assert_eq!(
            store.controller_target_handoff(actor).unwrap(),
            Some(ControllerTargetHandoff::new(0x360, 0x360, 0x240))
        );

        assert!(store.retire(actor));
        let replacement = store.spawn(7, snapshot).unwrap();
        assert_eq!(
            store.apply_measured_selected_goal(actor, 0x480),
            Err(ReaActorStoreError::StaleIdentity)
        );
        assert_eq!(store.controller_target_handoff(replacement).unwrap(), None);
    }

    #[test]
    fn selected_goal_store_commit_resets_shared_progress_and_retires_marker() {
        let mut store = ReaTickActorStore::new_empty();
        let snapshot = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let actor = store.spawn(7, snapshot).unwrap();
        store
            .set_controller_target_handoff(actor, ControllerTargetHandoff::new(0x240, -1, 0x120))
            .unwrap();

        assert_eq!(
            store.apply_measured_selected_goal(actor, 0x360),
            Ok(Some(SelectedGoalEffect::SelectedGoalInstalled {
                previous: 0x240,
                current: 0x360,
            }))
        );
        let committed = store.state(actor).unwrap();
        assert_eq!(committed.transition_progress(), TransitionProgress(0));
        assert_eq!(
            committed.controller_target_handoff(),
            Some(ControllerTargetHandoff::new(0x360, 0x360, 0x240))
        );

        assert_eq!(
            store.apply_controller_target_request(actor),
            Ok(Some(ControllerTargetEffect::EqualRequestRetired {
                target: 0x360
            }))
        );
        let retired = store.state(actor).unwrap();
        assert_eq!(retired.transition_progress(), TransitionProgress(0));
        assert_eq!(
            retired.controller_target_handoff(),
            Some(ControllerTargetHandoff::new(0x360, -1, 0x240))
        );
    }

    #[test]
    fn local_state_queue_replaces_and_resets_transition_progress_only_on_change() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.queue_local_state(LocalState(5));
        actor.queue_local_state(LocalState(6));
        assert_eq!(actor.pending_local_state(), Some(LocalState(6)));
        assert_eq!(
            actor.apply_local_state_request(),
            Some(LocalStateChange {
                previous: LocalState(4),
                current: LocalState(6),
            })
        );
        assert_eq!(actor.local_state(), LocalState(6));
        assert_eq!(actor.previous_local_state(), LocalState(4));
        assert_eq!(actor.transition_progress(), TransitionProgress(0));
        assert_eq!(actor.pending_local_state(), None);

        let mut unchanged = state(BehaviorMode::Other, BehaviorPair::default());
        unchanged.queue_local_state(LocalState(4));
        assert_eq!(unchanged.apply_local_state_request(), None);
        assert_eq!(
            unchanged.transition_progress(),
            TransitionProgress(0x4020_0000)
        );
    }

    #[test]
    fn transition_progress_preserves_opaque_bits_until_a_changed_state_commit() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        let opaque_bits = 0x7fc0_1234;
        actor.refresh_compatibility_fields(ReaTickSnapshot {
            transition_progress: TransitionProgress(opaque_bits),
            ..actor.snapshot()
        });
        assert_eq!(actor.transition_progress(), TransitionProgress(opaque_bits));

        actor.queue_local_state(LocalState(4));
        assert!(!actor.apply_local_state_prologue().unwrap().changed);
        assert_eq!(actor.transition_progress(), TransitionProgress(opaque_bits));

        actor.queue_local_state(LocalState(5));
        assert!(actor.apply_local_state_prologue().unwrap().changed);
        assert_eq!(actor.transition_progress(), TransitionProgress(0));
    }

    #[test]
    fn local_state_prologue_consumes_equal_request_without_state_effects() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.queue_local_state(LocalState(4));

        assert_eq!(
            actor.apply_local_state_prologue(),
            Some(LocalStateCommit {
                requested: LocalState(4),
                previous: LocalState(4),
                current: LocalState(4),
                changed: false,
            })
        );
        assert_eq!(actor.pending_local_state(), None);
        assert_eq!(actor.previous_local_state(), LocalState(3));
        assert_eq!(actor.transition_progress(), TransitionProgress(0x4020_0000));

        actor.queue_local_state(LocalState(7));
        assert_eq!(
            actor.apply_local_state_prologue(),
            Some(LocalStateCommit {
                requested: LocalState(7),
                previous: LocalState(4),
                current: LocalState(7),
                changed: true,
            })
        );
        assert_eq!(actor.previous_local_state(), LocalState(4));
        assert_eq!(actor.transition_progress(), TransitionProgress(0));
    }

    #[test]
    fn controller_local_state_commit_publishes_fields_in_measured_order() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.queue_local_state(LocalState(7));
        let mut observed = std::vec::Vec::new();

        let commit = actor
            .apply_local_state_commit_observing(
                LocalStateCommitOrder::ControllerEntry,
                |effect, state| {
                    observed.push((
                        effect,
                        state.previous_local_state(),
                        state.local_state(),
                        state.pending_local_state(),
                        state.transition_progress(),
                    ));
                },
            )
            .unwrap();

        assert!(commit.changed);
        assert_eq!(
            observed,
            std::vec![
                (
                    LocalStateCommitEffect::ResetTransitionProgress,
                    LocalState(3),
                    LocalState(4),
                    Some(LocalState(7)),
                    TransitionProgress(0),
                ),
                (
                    LocalStateCommitEffect::SavePreviousState,
                    LocalState(4),
                    LocalState(4),
                    Some(LocalState(7)),
                    TransitionProgress(0),
                ),
                (
                    LocalStateCommitEffect::PublishCurrentState,
                    LocalState(4),
                    LocalState(7),
                    Some(LocalState(7)),
                    TransitionProgress(0),
                ),
                (
                    LocalStateCommitEffect::ClearPendingRequest,
                    LocalState(4),
                    LocalState(7),
                    None,
                    TransitionProgress(0),
                ),
            ]
        );

        let mut equal = state(BehaviorMode::Other, BehaviorPair::default());
        equal.queue_local_state(LocalState(4));
        let mut equal_effects = std::vec::Vec::new();
        assert!(
            !equal
                .apply_local_state_commit_observing(
                    LocalStateCommitOrder::ControllerEntry,
                    |effect, _| equal_effects.push(effect),
                )
                .unwrap()
                .changed
        );
        assert_eq!(equal_effects, [LocalStateCommitEffect::ClearPendingRequest]);
    }

    #[test]
    fn state_machine_local_state_prologue_uses_its_separate_measured_order() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.queue_local_state(LocalState(7));
        let mut observed = std::vec::Vec::new();

        actor
            .apply_local_state_commit_observing(
                LocalStateCommitOrder::StateMachinePrologue,
                |effect, _| observed.push(effect),
            )
            .unwrap();

        assert_eq!(
            observed,
            [
                LocalStateCommitEffect::SavePreviousState,
                LocalStateCommitEffect::PublishCurrentState,
                LocalStateCommitEffect::ResetTransitionProgress,
                LocalStateCommitEffect::ClearPendingRequest,
            ]
        );
    }

    #[test]
    fn behavior_request_uses_selected_bank_and_protected_request() {
        let mut mode10 = state(BehaviorMode::Mode10, BehaviorPair { low: 0x40, high: 0 });
        assert!(!mode10.request_behavior(BehaviorPair { low: 1, high: 0 }));
        assert_eq!(mode10.pending_behavior(), BehaviorPair::default());
        assert!(mode10.request_behavior(BehaviorPair { low: 0x20, high: 2 }));
        assert_eq!(mode10.active_behavior().low, 0x40);
        assert_eq!(mode10.pending_behavior().low, 0x20);

        let mut other = state(BehaviorMode::Other, BehaviorPair { low: 1, high: 0 });
        assert!(other.request_behavior(BehaviorPair { low: 1, high: 0 }));
    }

    #[test]
    fn behavior_commit_replaces_pair_and_preserves_mode_banks() {
        let mut actor = state(BehaviorMode::Mode10, BehaviorPair { low: 0x40, high: 5 });
        let requested = BehaviorPair {
            low: 0x10000,
            high: 7,
        };
        assert!(actor.request_behavior(requested));
        let change = actor.pending_behavior_change().unwrap();
        assert_eq!(
            change,
            BehaviorChange {
                mode: BehaviorMode::Mode10,
                previous: BehaviorPair { low: 0x40, high: 5 },
                current: requested,
            }
        );
        assert!(actor.apply_behavior_change(change));
        assert_eq!(actor.active_behavior(), requested);
        assert_eq!(
            actor.previous_behavior(),
            BehaviorPair { low: 0x40, high: 5 }
        );
        assert_eq!(actor.pending_behavior_change(), None);
        assert_eq!(actor.other_active, BehaviorPair { low: 0x20, high: 3 });
    }

    #[test]
    fn behavior_handover_callbacks_observe_old_then_new_rust_state() {
        let old = BehaviorPair { low: 0x40, high: 5 };
        let installed = BehaviorPair {
            low: 0x10000,
            high: 7,
        };
        let requested_during_callback = BehaviorPair {
            low: 0x200,
            high: 9,
        };
        let mut actor = state(BehaviorMode::Mode10, old);
        actor.set_behavior_handover_words(BehaviorHandoverWords {
            at_9cc: 0x11,
            at_b30: 0x22,
            at_9e0: 0x33,
            at_9dc: 0x44,
        });
        assert!(actor.request_behavior(installed));
        let mut observed = std::vec::Vec::new();

        assert!(actor.run_behavior_handover(|state, message| {
            assert_eq!(
                state.behavior_handover_words(),
                BehaviorHandoverWords::default()
            );
            observed.push((message, state.active_behavior()));
            if message == BehaviorHandoverMessage::Previous {
                assert!(state.request_behavior(requested_during_callback));
            }
        }));

        assert_eq!(
            observed,
            [
                (BehaviorHandoverMessage::Previous, old),
                (BehaviorHandoverMessage::Current, installed),
            ]
        );
        assert_eq!(actor.previous_behavior(), old);
        assert_eq!(actor.active_behavior(), requested_during_callback);
        assert_eq!(actor.pending_behavior_change(), None);
        assert_eq!(actor.transition_progress(), TransitionProgress(0));
        assert_eq!(
            actor.behavior_handover_words(),
            BehaviorHandoverWords::default()
        );
        assert_eq!(actor.pending_local_state(), Some(LocalState(0)));
    }

    #[test]
    fn compatibility_refresh_preserves_selected_pair_and_imports_inactive_bank() {
        let initial = BehaviorPair { low: 0x40, high: 5 };
        let requested = BehaviorPair {
            low: 0x10000,
            high: 7,
        };
        let mut actor = state(BehaviorMode::Mode10, initial);
        assert!(actor.request_behavior(requested));

        let mut stale = actor.snapshot();
        stale.mode10_active = BehaviorPair {
            low: 0xdead,
            high: 1,
        };
        stale.other_active = BehaviorPair {
            low: 0xbeef,
            high: 2,
        };
        stale.previous_behavior = BehaviorPair {
            low: 0xabcd,
            high: 3,
        };
        stale.pending_behavior = BehaviorPair {
            low: 0xaaaa,
            high: 4,
        };
        stale.local_state = LocalState(8);
        actor.refresh_compatibility_fields(stale);

        assert_eq!(actor.active_behavior(), initial);
        assert_eq!(actor.pending_behavior(), requested);
        assert_eq!(
            actor.other_active,
            BehaviorPair {
                low: 0xbeef,
                high: 2
            }
        );
        assert_eq!(actor.local_state(), LocalState(8));
        assert!(actor.commit_pending_behavior());
        assert_eq!(actor.active_behavior(), requested);
        assert_eq!(actor.previous_behavior(), initial);

        stale.mode10_active = BehaviorPair {
            low: 0x3333,
            high: 6,
        };
        stale.previous_behavior = BehaviorPair {
            low: 0x2222,
            high: 5,
        };
        actor.refresh_compatibility_fields(stale);
        assert_eq!(actor.active_behavior(), requested);
        assert_eq!(actor.previous_behavior(), initial);
        assert_eq!(
            actor.other_active,
            BehaviorPair {
                low: 0xbeef,
                high: 2
            }
        );
    }

    #[test]
    fn local_state_owner_ignores_stale_compatibility_snapshot() {
        let mut actor = state(BehaviorMode::Other, BehaviorPair::default());
        actor.queue_local_state(LocalState(9));
        let mut stale = actor.snapshot();
        stale.local_state = LocalState(1);
        stale.previous_local_state = LocalState(2);
        stale.pending_local_state = Some(LocalState(3));
        stale.transition_progress = TransitionProgress(0x4140_0000);

        actor.refresh_compatibility_fields(stale);

        assert!(actor.owns_local_state());
        assert_eq!(actor.local_state(), LocalState(4));
        assert_eq!(actor.previous_local_state(), LocalState(3));
        assert_eq!(actor.pending_local_state(), Some(LocalState(9)));
        assert_eq!(actor.transition_progress(), TransitionProgress(0x4020_0000));
        assert_eq!(
            actor.apply_local_state_prologue(),
            Some(LocalStateCommit {
                requested: LocalState(9),
                previous: LocalState(4),
                current: LocalState(9),
                changed: true,
            })
        );

        actor.refresh_compatibility_fields(stale);
        assert_eq!(actor.local_state(), LocalState(9));
        assert_eq!(actor.previous_local_state(), LocalState(4));
        assert_eq!(actor.pending_local_state(), None);
        assert_eq!(actor.transition_progress(), TransitionProgress(0));
    }

    #[test]
    fn completed_behavior_tick_installs_latest_callback_request() {
        let initial = BehaviorPair { low: 0x40, high: 5 };
        let first = BehaviorPair { low: 0x20, high: 6 };
        let queued_by_callback = BehaviorPair { low: 0x80, high: 7 };
        let mut actor = state(BehaviorMode::Mode10, initial);
        assert!(actor.request_behavior(first));
        let in_flight = actor.pending_behavior_change().unwrap();

        assert!(actor.request_behavior(queued_by_callback));
        assert_eq!(actor.active_behavior(), initial);
        assert!(actor.finish_behavior_tick(in_flight.mode, in_flight.previous));
        assert_eq!(actor.active_behavior(), queued_by_callback);
        assert_eq!(actor.pending_behavior(), queued_by_callback);
        assert_eq!(actor.previous_behavior(), initial);
    }

    #[test]
    fn behavior_commit_orders_callbacks_before_mode10_cleanup() {
        let change = BehaviorChange {
            mode: BehaviorMode::Mode10,
            previous: BehaviorPair {
                low: 0x60000,
                high: 0,
            },
            current: BehaviorPair { low: 4, high: 0 },
        };
        assert_eq!(
            change.ordered_effects(),
            [
                BehaviorCommitEffect::ClearHandoverExtensionWords,
                BehaviorCommitEffect::SavePreviousPair,
                BehaviorCommitEffect::InstallCurrentPair,
                BehaviorCommitEffect::ResetNavigationProgress,
                BehaviorCommitEffect::DispatchPreviousMessage3ea,
                BehaviorCommitEffect::DispatchCurrentMessage3e9,
            ]
        );
        assert_eq!(
            change.cleanup_after_callbacks(change.current, true, true),
            BehaviorClaimCleanup {
                clear_secondary: true,
                release_secondary_claim: true,
            }
        );
        assert_eq!(
            change.cleanup_after_callbacks(
                BehaviorPair {
                    low: 0x10000,
                    high: 0
                },
                true,
                true,
            ),
            BehaviorClaimCleanup {
                clear_secondary: false,
                release_secondary_claim: false,
            }
        );
    }

    #[test]
    fn actor_store_mints_generations_for_explicit_lifetimes() {
        let mut store = ReaTickActorStore::default();
        let initial = state(BehaviorMode::Other, BehaviorPair { low: 4, high: 1 });
        let first = store.spawn(3, initial.snapshot()).unwrap();
        assert_eq!(first.generation, 1);
        assert!(store.spawn(3, initial.snapshot()).is_none());

        store.queue_local_state(first, LocalState(8)).unwrap();
        assert_eq!(
            store.state(first).unwrap().pending_local_state(),
            Some(LocalState(8))
        );
        assert_eq!(
            store.apply_local_state_prologue(first),
            Ok(Some(LocalStateCommit {
                requested: LocalState(8),
                previous: LocalState(4),
                current: LocalState(8),
                changed: true,
            }))
        );
        assert_eq!(
            store.state(first).unwrap().transition_progress(),
            TransitionProgress(0)
        );
        assert!(store.retire(first));
        let reused = store.spawn(3, initial.snapshot()).unwrap();
        assert_eq!(reused.generation, 2);
        assert_eq!(
            store.queue_local_state(first, LocalState(9)),
            Err(ReaActorStoreError::StaleIdentity)
        );
        assert_eq!(
            store.apply_local_state_prologue(first),
            Err(ReaActorStoreError::StaleIdentity)
        );
        assert_eq!(
            store.state(reused).unwrap().active_behavior(),
            initial.active_behavior()
        );
        assert_eq!(store.state(reused).unwrap().local_state(), LocalState(4));
        assert_eq!(store.state(reused).unwrap().pending_local_state(), None);
        assert_eq!(store.trace_rows().count(), 1);

        store.retain_live(&[]);
        assert!(store.is_empty());
        let third = store.spawn(3, initial.snapshot()).unwrap();
        assert_eq!(third.generation, 3);
        assert!(store.retire(third));
        assert!(!store.retire(third));
    }

    #[test]
    fn actor_store_applies_target_handoff_only_to_the_live_generation() {
        let mut store = ReaTickActorStore::default();
        let initial = state(BehaviorMode::Other, BehaviorPair::default());
        let first = store.spawn(4, initial.snapshot()).unwrap();
        let target = crate::rea_target_handoff::TargetObjectId(0x42);
        let decision = crate::rea_target_handoff::HandoffDecision {
            branch: crate::rea_target_handoff::HandoffBranch::TypeZeroTargetHandoff,
            effects: alloc::vec![
                crate::rea_target_handoff::HandoffEffect::StoreTarget(target),
                crate::rea_target_handoff::HandoffEffect::RequestBehavior(
                    crate::rea_target_handoff::HANDOFF_BEHAVIOR,
                ),
                crate::rea_target_handoff::HandoffEffect::RequestLocalState(1),
            ],
        };

        assert_eq!(
            store.apply_target_handoff(first, &decision, None),
            Ok(crate::rea_target_handoff::HandoffCommit {
                goal_selection_committed: false,
                local_state_queued: true,
                behavior_queued: true,
                target_stored: true,
                world_position_copied: false,
            })
        );
        let owned = store.state(first).unwrap();
        assert_eq!(owned.handoff_target(), Some(target));
        assert_eq!(
            owned.pending_behavior(),
            crate::rea_target_handoff::HANDOFF_BEHAVIOR
        );
        assert_eq!(owned.pending_local_state(), Some(LocalState(1)));

        assert!(store.retire(first));
        let reused = store.spawn(4, initial.snapshot()).unwrap();
        assert_eq!(
            store.apply_target_handoff(first, &decision, None),
            Err(ReaActorStoreError::StaleIdentity)
        );
        let reused_state = store.state(reused).unwrap();
        assert_eq!(reused_state.handoff_target(), None);
        assert_eq!(reused_state.pending_behavior(), BehaviorPair::default());
        assert_eq!(reused_state.pending_local_state(), None);
    }

    #[test]
    fn actor_store_owns_state12_and_post_callback_mode10_claim_transitions() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let snapshot = state(
            BehaviorMode::Mode10,
            BehaviorPair {
                low: 0x60000,
                high: 0,
            },
        )
        .snapshot();
        let actor = store.spawn(6, snapshot).unwrap();
        let current = SecondaryTargetClaim {
            handle: crate::awareness::TargetHandle {
                object: crate::navigation_scoring::ObjectId(20),
                generation: 1,
            },
            own_key: crate::rea_ai_types::NodeKey(9002),
            primary: crate::awareness::PrimaryTargetId(2),
        };
        store
            .acquire_navigation_claim(
                actor,
                GraphScopedSecondaryClaim {
                    graph,
                    claim: current,
                },
            )
            .unwrap();
        assert!(store.primary_is_occupied(current.primary));
        store.save_navigation_claim(actor).unwrap();
        store.navigation_state12_timeout(actor, false).unwrap();
        assert_eq!(
            store.state(actor).unwrap().navigation_claims().current,
            Some(current)
        );
        assert!(store.primary_is_occupied(current.primary));
        store.navigation_state12_timeout(actor, true).unwrap();
        let claims = store.state(actor).unwrap().navigation_claims();
        assert_eq!(claims.current, None);
        assert_eq!(claims.saved, Some(current));
        assert!(!claims.primary_is_claimed(current.primary));
        assert!(!store.primary_is_occupied(current.primary));

        let saved = SecondaryTargetClaim {
            handle: crate::awareness::TargetHandle {
                object: crate::navigation_scoring::ObjectId(21),
                generation: 1,
            },
            own_key: crate::rea_ai_types::NodeKey(9003),
            primary: crate::awareness::PrimaryTargetId(3),
        };
        store
            .acquire_navigation_claim(
                actor,
                GraphScopedSecondaryClaim {
                    graph,
                    claim: current,
                },
            )
            .unwrap();
        store.save_navigation_claim(actor).unwrap();
        store
            .acquire_navigation_claim(
                actor,
                GraphScopedSecondaryClaim {
                    graph,
                    claim: saved,
                },
            )
            .unwrap();
        assert!(store.primary_is_occupied(current.primary));
        assert!(store.primary_is_occupied(saved.primary));
        let change = BehaviorChange {
            mode: BehaviorMode::Mode10,
            previous: BehaviorPair {
                low: 0x60000,
                high: 0,
            },
            current: BehaviorPair { low: 4, high: 0 },
        };
        assert_eq!(
            store
                .cleanup_navigation_after_behavior_callbacks(
                    actor,
                    change,
                    BehaviorPair {
                        low: 0x10000,
                        high: 0,
                    },
                )
                .unwrap(),
            BehaviorClaimCleanup {
                clear_secondary: false,
                release_secondary_claim: false,
            }
        );
        let claims = store.state(actor).unwrap().navigation_claims();
        assert_eq!(claims.current, Some(saved));
        assert_eq!(claims.saved, Some(current));
        assert!(claims.primary_is_claimed(current.primary));
        assert!(claims.primary_is_claimed(saved.primary));

        assert_eq!(
            store
                .cleanup_navigation_after_behavior_callbacks(
                    actor,
                    change,
                    BehaviorPair::default(),
                )
                .unwrap(),
            BehaviorClaimCleanup {
                clear_secondary: true,
                release_secondary_claim: true,
            }
        );
        let claims = store.state(actor).unwrap().navigation_claims();
        assert_eq!(claims.current, None);
        assert_eq!(claims.saved, None);
        assert!(!claims.primary_is_claimed(current.primary));
        assert!(!claims.primary_is_claimed(saved.primary));
        assert!(!store.primary_is_occupied(current.primary));
        assert!(!store.primary_is_occupied(saved.primary));

        assert!(store.retire(actor));
        let replacement = store.spawn(6, snapshot).unwrap();
        assert_ne!(replacement, actor);
        assert_eq!(
            store.navigation_state12_timeout(actor, true),
            Err(ReaActorStoreError::StaleIdentity)
        );
    }

    #[test]
    fn actor_store_finishes_behavior_tick_on_the_captured_generation() {
        let mut store = ReaTickActorStore::default();
        let initial = state(BehaviorMode::Other, BehaviorPair { low: 4, high: 1 });
        let actor = store.spawn(3, initial.snapshot()).unwrap();
        let first_request = BehaviorPair { low: 0x80, high: 2 };
        let callback_request = BehaviorPair {
            low: 0x100,
            high: 5,
        };

        assert_eq!(store.request_behavior(actor, first_request), Ok(true));
        let captured = store.pending_behavior_change(actor).unwrap().unwrap();
        assert_eq!(captured.previous, BehaviorPair { low: 4, high: 1 });
        assert_eq!(captured.current, first_request);

        // A callback replaces the one pending pair while the old active pair
        // remains selected. The tick installs that final pending pair only
        // after callback return.
        assert_eq!(store.request_behavior(actor, callback_request), Ok(true));
        assert_eq!(
            store.finish_behavior_tick(actor, captured.mode, captured.previous),
            Ok(true)
        );
        assert_eq!(
            store.state(actor).unwrap().active_behavior(),
            callback_request
        );
        assert_eq!(
            store.state(actor).unwrap().previous_behavior(),
            captured.previous
        );
        assert_eq!(
            store.state(actor).unwrap().pending_behavior(),
            callback_request
        );

        let next_request = BehaviorPair {
            low: 0x200,
            high: 6,
        };
        assert_eq!(store.request_behavior(actor, next_request), Ok(true));
        let next = store.pending_behavior_change(actor).unwrap().unwrap();
        assert_eq!(store.apply_behavior_change(actor, next), Ok(true));
        assert_eq!(store.state(actor).unwrap().active_behavior(), next_request);

        assert!(store.retire(actor));
        let reused = store.spawn(3, initial.snapshot()).unwrap();
        assert_ne!(actor.generation, reused.generation);
        assert_eq!(
            store.finish_behavior_tick(actor, captured.mode, captured.previous),
            Err(ReaActorStoreError::StaleIdentity)
        );
        assert_eq!(
            store.state(reused).unwrap().active_behavior(),
            initial.active_behavior()
        );
    }

    #[test]
    fn actor_store_prefers_observed_slots_and_reuses_with_new_generation() {
        let mut store = ReaTickActorStore::default();
        let initial = state(BehaviorMode::Other, BehaviorPair { low: 4, high: 1 });
        let first = store
            .spawn_with_observed_slot(Some(7), initial.snapshot())
            .unwrap();
        let second = store
            .spawn_with_observed_slot(None, initial.snapshot())
            .unwrap();
        assert_eq!(first.slot, 7);
        assert_eq!(first.generation, 1);
        assert_eq!(second.slot, 0);
        assert_eq!(second.generation, 1);
        assert!(store.retire(first));
        let reused = store
            .spawn_with_observed_slot(Some(7), initial.snapshot())
            .unwrap();
        assert_eq!(reused.slot, first.slot);
        assert_eq!(reused.generation, 2);
    }

    #[test]
    fn actor_store_owns_state_six_route_entry_by_generation() {
        let mut initial = state(BehaviorMode::Other, BehaviorPair { low: 4, high: 0 });
        initial.local_state = LocalState(1);
        let mut store = ReaTickActorStore::new_empty();
        let identity = store.spawn(6, initial.snapshot()).unwrap();

        assert_eq!(store.enter_navigation_state_six(identity, -1, 42), Ok(true));
        let owned = store.state(identity).unwrap();
        assert_eq!(owned.pending_local_state(), Some(LocalState(6)));
        assert_eq!(
            owned.actor_navigation().state_six_route(),
            StateSixRoute {
                state: 1,
                requested_node: -1,
                target_node: 42,
                counter_9ac: 0,
                counter_9a8: 0,
                progress_b30: 0.0,
                mode_9d0: 6,
            }
        );
        assert!(store.retire(identity));
        assert_eq!(
            store.enter_navigation_state_six(identity, 7, 42),
            Err(ReaActorStoreError::StaleIdentity)
        );

        let mut inactive = state(BehaviorMode::Other, BehaviorPair { low: 4, high: 0 });
        inactive.local_state = LocalState(2);
        let inactive_identity = store.spawn(6, inactive.snapshot()).unwrap();
        assert_eq!(
            store.enter_navigation_state_six(inactive_identity, -1, 42),
            Ok(false)
        );
        assert_eq!(
            store
                .state(inactive_identity)
                .unwrap()
                .actor_navigation()
                .state_six_route(),
            StateSixRoute::default()
        );
    }

    #[test]
    fn actor_store_owns_route_and_claim_state_per_generation() {
        use crate::{
            awareness::{PrimaryTargetId, SecondaryTargetClaim, TargetHandle},
            navigation_scoring::{NodeId, ObjectId},
            rea_ai_types::NodeKey,
        };

        let mut store = ReaTickActorStore::new_empty();
        let initial = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let first = store.spawn(7, initial).unwrap();
        let neighbor = store.spawn(8, initial).unwrap();
        {
            let actor = store.state_mut(first).unwrap();
            actor.navigation_route_mut().goal_node = NodeId(42);
            actor.actor_navigation_mut().set_next_node(NodeId(43));
            actor.actor_navigation_mut().set_movement_target(Position3 {
                x: 7.0,
                y: 8.0,
                z: 9.0,
            });
            actor.navigation_claims_mut().acquire(SecondaryTargetClaim {
                handle: TargetHandle {
                    object: ObjectId(5),
                    generation: 3,
                },
                own_key: NodeKey(9001),
                primary: PrimaryTargetId(4),
            });
        }

        let first_state = store.state(first).unwrap();
        assert_eq!(first_state.navigation_route().goal_node, NodeId(42));
        assert_eq!(first_state.actor_navigation().next_node(), Some(NodeId(43)));
        assert_eq!(
            first_state.actor_navigation().movement_target(),
            Some(Position3 {
                x: 7.0,
                y: 8.0,
                z: 9.0,
            })
        );
        assert!(
            first_state
                .navigation_claims()
                .primary_is_claimed(PrimaryTargetId(4))
        );
        let neighbor_state = store.state(neighbor).unwrap();
        assert_eq!(neighbor_state.navigation_route().goal_node, NodeId(0));
        assert!(neighbor_state.navigation_claims().current.is_none());

        assert!(store.retire(first));
        let replacement = store.spawn(7, initial).unwrap();
        assert_ne!(first, replacement);
        assert!(store.state(first).is_none());
        let replacement_state = store.state(replacement).unwrap();
        assert_eq!(replacement_state.navigation_route(), RouteActor::default());
        assert_eq!(replacement_state.actor_navigation().next_node(), None);
        assert_eq!(replacement_state.actor_navigation().movement_target(), None);
        assert!(replacement_state.navigation_claims().current.is_none());
    }

    #[test]
    fn graph_replacement_invalidates_cached_route_record_and_other_projections() {
        use crate::{
            awareness::{PrimaryTargetId, SecondaryTargetClaim, TargetHandle},
            navigation_scoring::{NodeId, ObjectId},
            rea_ai_types::NodeKey,
            rea_movement::Position3,
        };

        let mut store = ReaTickActorStore::new_empty();
        let first_graph = store.replace_navigation_graph().unwrap();
        let initial = state(BehaviorMode::Other, BehaviorPair { low: 4, high: 1 }).snapshot();
        let actor = store.spawn(12, initial).unwrap();
        store
            .set_cached_navigation_record(actor, CachedNavigationRecord(0xdead_beef))
            .unwrap();
        assert_eq!(
            store
                .state(actor)
                .unwrap()
                .actor_navigation()
                .graph_generation(),
            Some(first_graph)
        );
        let claim = SecondaryTargetClaim {
            handle: TargetHandle {
                object: ObjectId(90),
                generation: 3,
            },
            own_key: NodeKey(71),
            primary: PrimaryTargetId(6),
        };
        store
            .acquire_navigation_claim(
                actor,
                GraphScopedSecondaryClaim {
                    graph: first_graph,
                    claim,
                },
            )
            .unwrap();
        store.save_navigation_claim(actor).unwrap();
        store
            .state_mut(actor)
            .unwrap()
            .navigation_route_mut()
            .goal_node = NodeId(33);
        let link = LinkActorState {
            position: Position3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            current_node: NodeId(33),
            goal_node: NodeId(44),
            movement_target: Position3 {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
        };
        store.set_navigation_link(actor, link).unwrap();
        store.set_navigation_next_node(actor, NodeId(45)).unwrap();
        store
            .set_navigation_movement_target(
                actor,
                Position3 {
                    x: 7.0,
                    y: 8.0,
                    z: 9.0,
                },
            )
            .unwrap();

        let second_graph = store.replace_navigation_graph().unwrap();

        assert_eq!(second_graph, NavigationGraphGeneration(first_graph.0 + 1));
        assert_eq!(store.navigation_graph_generation(), Some(second_graph));
        assert!(!store.primary_is_occupied(claim.primary));
        assert_eq!(
            store
                .state(actor)
                .unwrap()
                .actor_navigation()
                .cached_route_record(),
            None
        );
        assert_eq!(
            store.state(actor).unwrap().navigation_route(),
            RouteActor::default()
        );
        assert_eq!(store.navigation_link(actor), Ok(None));
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.graph_generation(), Some(second_graph));
        assert_eq!(navigation.next_node(), None);
        assert_eq!(navigation.movement_target(), None);
        assert!(
            store
                .state(actor)
                .unwrap()
                .navigation_claims()
                .current
                .is_none()
        );
        assert!(
            store
                .state(actor)
                .unwrap()
                .navigation_claims()
                .saved
                .is_none()
        );
        assert_eq!(
            store.state(actor).unwrap().active_behavior(),
            initial.other_active
        );
        assert_eq!(
            store.acquire_navigation_claim(
                actor,
                GraphScopedSecondaryClaim {
                    graph: first_graph,
                    claim,
                },
            ),
            Err(ReaActorStoreError::StaleNavigationGraph)
        );
    }

    #[test]
    fn resolved_navigation_step_commits_together_and_rejects_stale_graph() {
        let mut store = ReaTickActorStore::new_empty();
        let first_graph = store.replace_navigation_graph().unwrap();
        let actor = store
            .spawn(
                5,
                state(BehaviorMode::Other, BehaviorPair { low: 2, high: 0 }).snapshot(),
            )
            .unwrap();
        let first_step = ResolvedNavigationStep {
            graph: first_graph,
            next_node: NodeId(13),
            movement_target: Position3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
        };

        store
            .commit_resolved_navigation_step(actor, first_step)
            .unwrap();
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), Some(NodeId(13)));
        assert_eq!(
            navigation.movement_target(),
            Some(first_step.movement_target)
        );

        let second_graph = store.replace_navigation_graph().unwrap();
        let second_step = ResolvedNavigationStep {
            graph: second_graph,
            next_node: NodeId(21),
            movement_target: Position3 {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
        };
        store
            .commit_resolved_navigation_step(actor, second_step)
            .unwrap();

        assert_eq!(
            store.commit_resolved_navigation_step(actor, first_step),
            Err(ReaActorStoreError::StaleNavigationGraph)
        );
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), Some(second_step.next_node));
        assert_eq!(
            navigation.movement_target(),
            Some(second_step.movement_target)
        );
    }

    #[test]
    fn point_follow_results_apply_only_the_measured_navigation_effects() {
        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let actor = store
            .spawn(
                6,
                state(BehaviorMode::Other, BehaviorPair { low: 2, high: 0 }).snapshot(),
            )
            .unwrap();
        let old_target = Position3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        store.set_navigation_next_node(actor, NodeId(7)).unwrap();
        store
            .set_navigation_movement_target(actor, old_target)
            .unwrap();

        assert_eq!(
            store
                .apply_point_follow_result(
                    actor,
                    graph,
                    PointFollowSearchResult::AlertAbort,
                    None,
                    None,
                )
                .unwrap(),
            PointFollowEffect::AlertAbort
        );
        assert_eq!(
            store
                .apply_point_follow_result(
                    actor,
                    graph,
                    PointFollowSearchResult::OtherNegative(-3),
                    None,
                    None,
                )
                .unwrap(),
            PointFollowEffect::UnclassifiedNegative(-3)
        );
        assert_eq!(
            store
                .apply_point_follow_result(
                    actor,
                    graph,
                    PointFollowSearchResult::SelectedNode(NodeId(12)),
                    None,
                    None,
                )
                .unwrap(),
            PointFollowEffect::SelectedNodePositionUnavailable(NodeId(12))
        );
        assert_eq!(
            store
                .apply_point_follow_result(
                    actor,
                    graph,
                    PointFollowSearchResult::CachedRecord,
                    None,
                    None,
                )
                .unwrap(),
            PointFollowEffect::CachedRecordUnavailable
        );
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), Some(NodeId(7)));
        assert_eq!(navigation.movement_target(), Some(old_target));

        let cached_target = Position3 {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        };
        assert_eq!(
            store
                .apply_point_follow_result(
                    actor,
                    graph,
                    PointFollowSearchResult::CachedRecord,
                    None,
                    Some(cached_target),
                )
                .unwrap(),
            PointFollowEffect::CachedRecordTargetCommitted {
                point_follow_scalar: CACHED_RECORD_POINT_FOLLOW_SCALAR,
            }
        );
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), Some(NodeId(7)));
        assert_eq!(navigation.movement_target(), Some(cached_target));

        let selected_target = Position3 {
            x: 7.0,
            y: 8.0,
            z: 9.0,
        };
        assert_eq!(
            store
                .apply_point_follow_result(
                    actor,
                    graph,
                    PointFollowSearchResult::SelectedNode(NodeId(13)),
                    Some(selected_target),
                    None,
                )
                .unwrap(),
            PointFollowEffect::SelectedNodeCommitted(NodeId(13))
        );
        let navigation = store.state(actor).unwrap().actor_navigation();
        assert_eq!(navigation.next_node(), Some(NodeId(13)));
        assert_eq!(navigation.movement_target(), Some(selected_target));
    }

    #[test]
    fn navigation_link_state_is_generation_scoped_and_absent_until_projected() {
        // This module is shared with `tsfp-ports`, whose crate root exposes
        // movement_approach directly but not the tsfp-ai-only rea_movement shim.
        use crate::movement_approach::Position3;

        let mut store = ReaTickActorStore::new_empty();
        let initial = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let actor = store.spawn(21, initial).unwrap();
        let neighbor = store.spawn(22, initial).unwrap();
        let link = LinkActorState {
            position: Position3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            current_node: NodeId(8),
            goal_node: NodeId(13),
            movement_target: Position3 {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
        };

        assert_eq!(store.navigation_link(actor), Ok(None));
        assert_eq!(store.navigation_link(neighbor), Ok(None));
        store.set_navigation_link(actor, link).unwrap();
        assert_eq!(store.navigation_link(actor), Ok(Some(link)));
        assert_eq!(store.navigation_link(neighbor), Ok(None));

        assert!(store.retire(actor));
        let replacement = store.spawn(21, initial).unwrap();
        assert_ne!(actor.generation, replacement.generation);
        assert_eq!(
            store.set_navigation_link(actor, link),
            Err(ReaActorStoreError::StaleIdentity)
        );
        assert_eq!(store.navigation_link(replacement), Ok(None));
    }

    #[test]
    fn actor_navigation_refresh_and_state11_claim_release_are_generation_scoped() {
        use crate::{
            awareness::{PrimaryTargetId, SecondaryTargetClaim, TargetHandle},
            navigation_scoring::{NodeId, ObjectId},
            rea_ai_types::NodeKey,
        };

        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let initial = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let actor = store.spawn(7, initial).unwrap();
        let neighbor = store.spawn(8, initial).unwrap();

        assert!(
            store
                .refresh_navigation_current_node(actor, NodeId(12))
                .unwrap()
        );
        {
            let route = store.state_mut(actor).unwrap().navigation_route_mut();
            route.reverse_trace_within = 4;
            route.reverse_trace_other = 9;
        }
        assert!(
            !store
                .refresh_navigation_current_node(actor, NodeId(12))
                .unwrap()
        );
        {
            let route = store.state(actor).unwrap().navigation_route();
            assert_eq!(route.reverse_trace_within, 4);
            assert_eq!(route.reverse_trace_other, 9);
        }
        assert!(
            store
                .refresh_navigation_current_node(actor, NodeId(13))
                .unwrap()
        );
        {
            let route = store.state(actor).unwrap().navigation_route();
            assert_eq!(route.reverse_trace_within, 0);
            assert_eq!(route.reverse_trace_other, 0);
        }

        let claim = SecondaryTargetClaim {
            handle: TargetHandle {
                object: ObjectId(5),
                generation: 3,
            },
            own_key: NodeKey(9001),
            primary: PrimaryTargetId(4),
        };
        store
            .acquire_navigation_claim(actor, GraphScopedSecondaryClaim { graph, claim })
            .unwrap();
        assert!(store.primary_is_occupied(claim.primary));
        store.save_navigation_claim(actor).unwrap();
        store.navigation_state11_exit(actor, false).unwrap();
        assert!(store.primary_is_occupied(claim.primary));
        assert!(
            store
                .state(actor)
                .unwrap()
                .navigation_claims()
                .primary_is_claimed(PrimaryTargetId(4))
        );
        store.navigation_state11_exit(actor, true).unwrap();
        assert!(!store.primary_is_occupied(claim.primary));
        let claims = store.state(actor).unwrap().navigation_claims();
        assert!(!claims.primary_is_claimed(PrimaryTargetId(4)));
        assert_eq!(claims.current, Some(claim));
        assert_eq!(claims.saved, Some(claim));
        assert_eq!(
            store.state(neighbor).unwrap().navigation_route(),
            RouteActor::default()
        );
        assert!(
            store
                .state(neighbor)
                .unwrap()
                .navigation_claims()
                .current
                .is_none()
        );

        assert!(store.retire(actor));
        let replacement = store.spawn(7, initial).unwrap();
        assert_ne!(actor, replacement);
        assert_eq!(
            store.refresh_navigation_current_node(actor, NodeId(14)),
            Err(ReaActorStoreError::StaleIdentity)
        );
    }

    #[test]
    fn behavior_route_cursor_is_generation_scoped_and_explicitly_released() {
        let snapshot = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let mut store = ReaTickActorStore::new_empty();
        let actor = store.spawn(3, snapshot).unwrap();
        assert_eq!(store.behavior_route_cursor(actor), Ok(None));
        store.set_behavior_route_cursor(actor, 7).unwrap();
        assert_eq!(store.behavior_route_cursor(actor), Ok(Some(7)));
        store.clear_behavior_route_cursor(actor).unwrap();
        assert_eq!(store.behavior_route_cursor(actor), Ok(None));

        assert!(store.retire(actor));
        let replacement = store.spawn(3, snapshot).unwrap();
        assert_ne!(replacement, actor);
        assert_eq!(store.behavior_route_cursor(replacement), Ok(None));
    }

    #[test]
    fn behavior_program_cursor_is_separate_from_graph_node_route_state() {
        let snapshot = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let mut store = ReaTickActorStore::new_empty();
        let actor = store.spawn(3, snapshot).unwrap();
        let route = crate::rea_routes::RouteActor {
            current_node: crate::navigation_scoring::NodeId(17),
            goal_node: crate::navigation_scoring::NodeId(23),
            intermediate_node: Some(crate::navigation_scoring::NodeId(19)),
            reverse_trace_within: 2,
            reverse_trace_other: 4,
        };

        store.set_navigation_route(actor, route).unwrap();
        assert_eq!(store.behavior_route_cursor(actor), Ok(None));
        store.set_behavior_route_cursor(actor, 7).unwrap();
        assert_eq!(store.navigation_route(actor), Ok(route));
        assert_eq!(store.behavior_route_cursor(actor), Ok(Some(7)));
    }

    #[test]
    fn actor_claim_retirement_and_pruning_recompute_shared_occupancy() {
        use crate::{
            awareness::{PrimaryTargetId, SecondaryTargetClaim, TargetHandle},
            navigation_scoring::ObjectId,
            rea_ai_types::NodeKey,
        };

        let mut store = ReaTickActorStore::new_empty();
        let graph = store.replace_navigation_graph().unwrap();
        let snapshot = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let first = store.spawn(1, snapshot).unwrap();
        let second = store.spawn(2, snapshot).unwrap();
        let shared = PrimaryTargetId(77);
        let claim = |object, generation| SecondaryTargetClaim {
            handle: TargetHandle {
                object: ObjectId(object),
                generation,
            },
            own_key: NodeKey(object as i32),
            primary: shared,
        };

        for (identity, object) in [(first, 10), (second, 11)] {
            store
                .acquire_navigation_claim(
                    identity,
                    GraphScopedSecondaryClaim {
                        graph,
                        claim: claim(object, 1),
                    },
                )
                .unwrap();
            store.save_navigation_claim(identity).unwrap();
        }
        assert!(store.primary_is_occupied(shared));

        // Releasing or retiring one owner's claim must not clear another live
        // actor's Boolean occupancy.
        store.navigation_state11_exit(first, true).unwrap();
        assert!(store.primary_is_occupied(shared));
        assert!(store.retire(first));
        assert!(store.primary_is_occupied(shared));
        assert!(store.retire(second));
        assert!(!store.primary_is_occupied(shared));

        let pruned = store.spawn(3, snapshot).unwrap();
        let unique = SecondaryTargetClaim {
            handle: TargetHandle {
                object: ObjectId(12),
                generation: 1,
            },
            own_key: NodeKey(12),
            primary: PrimaryTargetId(88),
        };
        store
            .acquire_navigation_claim(
                pruned,
                GraphScopedSecondaryClaim {
                    graph,
                    claim: unique,
                },
            )
            .unwrap();
        store.retain_live(&[]);
        assert!(!store.primary_is_occupied(unique.primary));
    }

    #[test]
    fn group_alert_owns_recipient_state_and_orders_leave_callback_before_mask_commit() {
        #[derive(Default)]
        struct LeaveProbe {
            observed: Option<(ActorId, GroupMask, u32, Option<NodeId>)>,
        }

        impl AwarenessGroupMembership for LeaveProbe {
            fn leave_groups(
                &mut self,
                recipient: &mut AwarenessRecipientState,
                removed: GroupMask,
            ) {
                self.observed = Some((
                    recipient.actor,
                    removed,
                    recipient.local_flags,
                    recipient.alert_target,
                ));
                // Model a callback-side group mutation. The final intersection
                // must read the current Rust-owned value after this callback.
                recipient.group_mask = GroupMask(0b0101);
            }
        }

        impl AwarenessRecipientWorld for LeaveProbe {
            fn class_listens(&mut self, _selector: i16) -> bool {
                false
            }

            fn candidate_check(&mut self, _recipient: &mut AwarenessRecipientState) -> bool {
                false
            }

            fn actor_busy(&mut self, _recipient: &mut AwarenessRecipientState) -> bool {
                false
            }

            fn raise(
                &mut self,
                _recipient: &mut AwarenessRecipientState,
                _code: u32,
                _three_args: bool,
            ) {
            }
        }

        let snapshot = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let mut store = ReaTickActorStore::new_empty();
        let actor = store.spawn(5, snapshot).unwrap();
        store
            .set_awareness_recipient(
                actor,
                AwarenessRecipientState {
                    actor: ActorId(0x1234),
                    actor_flags: ACTOR_ALERTABLE,
                    local_flags: 0,
                    listen_flags: 0,
                    behavior_flags: 0,
                    class_selector: 0,
                    group_mask: GroupMask(0b1011),
                    alert_target: None,
                },
            )
            .unwrap();

        let mut world = LeaveProbe::default();
        assert_eq!(
            store.process_group_alert_recipient(
                actor,
                ActorId(0x9999),
                GroupMask(0b1100),
                NodeId(41),
                &mut world,
            ),
            Ok(AwarenessRecipientEffect::NewlyAlerted {
                removed_groups: GroupMask(0b0011),
            })
        );
        assert_eq!(
            world.observed,
            Some((
                ActorId(0x1234),
                GroupMask(0b0011),
                LOCAL_ALERTED,
                Some(NodeId(41)),
            ))
        );
        assert_eq!(
            store.awareness_recipient(actor),
            Ok(Some(AwarenessRecipientState {
                actor: ActorId(0x1234),
                actor_flags: ACTOR_ALERTABLE,
                local_flags: LOCAL_ALERTED,
                listen_flags: 0,
                behavior_flags: 0,
                class_selector: 0,
                group_mask: GroupMask(0b0100),
                alert_target: Some(NodeId(41)),
            }))
        );

        assert!(store.retire(actor));
        let replacement = store.spawn(5, snapshot).unwrap();
        assert_ne!(replacement, actor);
        assert_eq!(
            store.process_group_alert_recipient(
                actor,
                ActorId(0),
                GroupMask(u32::MAX),
                NodeId(9),
                &mut world,
            ),
            Err(ReaActorStoreError::StaleIdentity)
        );
        assert_eq!(store.awareness_recipient(replacement), Ok(None));
    }

    #[test]
    fn group_alert_owned_existing_recipient_preserves_dispatch_and_raise_order() {
        #[derive(Default)]
        struct RecipientProbe {
            listens: bool,
            candidate: bool,
            busy: bool,
            events: Vec<&'static str>,
            raised: Vec<(u32, bool)>,
            class_selector: Option<i16>,
        }

        impl AwarenessGroupMembership for RecipientProbe {
            fn leave_groups(
                &mut self,
                _recipient: &mut AwarenessRecipientState,
                _removed: GroupMask,
            ) {
                self.events.push("leave_groups");
            }
        }

        impl AwarenessRecipientWorld for RecipientProbe {
            fn class_listens(&mut self, selector: i16) -> bool {
                self.events.push("class_listens");
                self.class_selector = Some(selector);
                self.listens
            }

            fn candidate_check(&mut self, _recipient: &mut AwarenessRecipientState) -> bool {
                self.events.push("candidate_check");
                self.candidate
            }

            fn actor_busy(&mut self, _recipient: &mut AwarenessRecipientState) -> bool {
                self.events.push("actor_busy");
                self.busy
            }

            fn raise(
                &mut self,
                _recipient: &mut AwarenessRecipientState,
                code: u32,
                three_args: bool,
            ) {
                self.events.push("raise");
                self.raised.push((code, three_args));
            }
        }

        let snapshot = state(BehaviorMode::Other, BehaviorPair::default()).snapshot();
        let mut store = ReaTickActorStore::new_empty();
        let actor = store.spawn(9, snapshot).unwrap();
        let mut recipient = AwarenessRecipientState {
            actor: ActorId(0x3456),
            actor_flags: ACTOR_ALERTABLE,
            local_flags: LOCAL_ALERTED,
            listen_flags: 0,
            behavior_flags: 0,
            class_selector: 7,
            group_mask: GroupMask(0b11),
            alert_target: Some(NodeId(3)),
        };
        store.set_awareness_recipient(actor, recipient).unwrap();
        let mut world = RecipientProbe::default();

        assert_eq!(
            store.process_group_alert_recipient(
                actor,
                ActorId(0),
                GroupMask(0b11),
                NodeId(4),
                &mut world,
            ),
            Ok(AwarenessRecipientEffect::Raised {
                code: 8,
                three_args: true,
            })
        );
        assert_eq!(world.events, ["candidate_check", "actor_busy", "raise"]);
        assert_eq!(world.raised, [(8, true)]);

        world.events.clear();
        world.raised.clear();
        world.candidate = true;
        world.busy = false;
        assert_eq!(
            store.process_group_alert_recipient(
                actor,
                ActorId(0),
                GroupMask(0b11),
                NodeId(4),
                &mut world,
            ),
            Ok(AwarenessRecipientEffect::Raised {
                code: 4,
                three_args: true,
            })
        );
        assert_eq!(world.events, ["candidate_check", "actor_busy", "raise"]);
        assert_eq!(world.raised, [(4, true)]);

        world.events.clear();
        world.raised.clear();
        world.busy = true;
        assert_eq!(
            store.process_group_alert_recipient(
                actor,
                ActorId(0),
                GroupMask(0b11),
                NodeId(4),
                &mut world,
            ),
            Ok(AwarenessRecipientEffect::SuppressedBusy)
        );
        assert_eq!(world.events, ["candidate_check", "actor_busy"]);
        assert!(world.raised.is_empty());

        recipient.behavior_flags = BEHAVIOR_RAISE_FULL;
        store.set_awareness_recipient(actor, recipient).unwrap();
        world.events.clear();
        world.raised.clear();
        world.candidate = true;
        world.busy = true;
        assert_eq!(
            store.process_group_alert_recipient(
                actor,
                ActorId(0),
                GroupMask(0b11),
                NodeId(4),
                &mut world,
            ),
            Ok(AwarenessRecipientEffect::Raised {
                code: 0x200,
                three_args: false,
            })
        );
        assert_eq!(world.events, ["candidate_check", "raise"]);
        assert_eq!(world.raised, [(0x200, false)]);

        recipient.actor_flags = 0;
        recipient.local_flags = 0;
        recipient.listen_flags = 1;
        store.set_awareness_recipient(actor, recipient).unwrap();
        world.events.clear();
        world.listens = true;
        assert_eq!(
            store.process_group_alert_recipient(
                actor,
                ActorId(0),
                GroupMask(0b11),
                NodeId(4),
                &mut world,
            ),
            Ok(AwarenessRecipientEffect::MarkedAlerted)
        );
        assert_eq!(world.events, ["class_listens"]);
        assert_eq!(world.class_selector, Some(7));
        assert_eq!(
            store
                .awareness_recipient(actor)
                .unwrap()
                .unwrap()
                .local_flags
                & LOCAL_ALERTED,
            LOCAL_ALERTED
        );
    }
}
