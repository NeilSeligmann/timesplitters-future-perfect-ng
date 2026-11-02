//! Typed state and effect boundary for a native Story gameplay world (T2494).
//!
//! The host owns this value and refreshes its snapshot from native gameplay state. Event
//! execution reads only these typed inputs and appends typed effects; this module has no guest
//! memory access or lifted callback fallback.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use tsfp_input::player::PlayerInput;

use super::callbacks::WorldOp;
use super::record::Link;
use super::reset_relocation::{
    PlacementFlags, PlayerSlot, ResetDisposition, StoryResetEvent, StoryResetState,
};
use super::slots::{MissionEvent, MissionOutcome};
use super::world::{
    CallbackTarget, ConditionQuery, IdVariant, PropRequest, SpawnRequest, VoiceRequest, World,
};
use tsfp_state::story_flow::StoryFlow;

#[derive(Debug, Clone, PartialEq)]
pub struct StoryPlayerState {
    pub character_id: u32,
    pub following_link: u32,
    pub resource_memberships: BTreeSet<u32>,
    pub first_player_flag: bool,
    pub health: f32,
    pub down: bool,
}

/// Complete typed inputs published by the Rust gameplay owners for one Story tick.
/// Every domain is explicit: an empty collection is meaningful only when its native owner
/// supplied it, and no host-memory or guest-memory fallback is available here.
#[derive(Debug, Clone, PartialEq)]
pub struct StoryGameplayInputs {
    pub players: Vec<StoryPlayerState>,
    pub actor_health: BTreeMap<u32, f32>,
    pub tags: BTreeSet<u32>,
    pub tagged_child_states: BTreeMap<u32, StoryChildState>,
    pub settled_props: BTreeSet<u32>,
    pub ready_objects: BTreeSet<u32>,
    pub live_objects: BTreeSet<u32>,
    pub finished_voices: BTreeSet<u32>,
    pub polled_links: Vec<Link>,
    pub reset_state: Option<StoryResetState>,
}

impl StoryGameplayInputs {
    /// Require each domain at construction, including domains whose authoritative value is empty.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        players: Vec<StoryPlayerState>,
        actor_health: BTreeMap<u32, f32>,
        tags: BTreeSet<u32>,
        tagged_child_states: BTreeMap<u32, StoryChildState>,
        settled_props: BTreeSet<u32>,
        ready_objects: BTreeSet<u32>,
        live_objects: BTreeSet<u32>,
        finished_voices: BTreeSet<u32>,
        polled_links: Vec<Link>,
        reset_state: Option<StoryResetState>,
    ) -> Self {
        Self {
            players,
            actor_health,
            tags,
            tagged_child_states,
            settled_props,
            ready_objects,
            live_objects,
            finished_voices,
            polled_links,
            reset_state,
        }
    }

    /// Assemble one complete tick snapshot from the native player and actor owners.
    ///
    /// The remaining domains are explicit caller inputs because they belong to other gameplay
    /// owners. This factory performs no guest-memory reads and cannot silently supply defaults.
    pub fn from_player_actor_owners(
        players: &player_owner::StoryPlayerStore,
        actors: &actor_owner::StoryActorStore,
        finished_voices: BTreeSet<u32>,
        polled_links: Vec<Link>,
        reset_state: Option<StoryResetState>,
    ) -> Self {
        let mut world = NativeStoryWorld::default();
        world.players = players.snapshot();
        actors.publish_into(&mut world);
        Self::new(
            world.players,
            world.actor_health,
            world.tags,
            world.tagged_child_states,
            world.settled_props,
            world.ready_objects,
            world.live_objects,
            finished_voices,
            polled_links,
            reset_state,
        )
    }

    pub fn publish_into(&self, world: &mut NativeStoryWorld) {
        world.players.clone_from(&self.players);
        world.actor_health.clone_from(&self.actor_health);
        world.tags.clone_from(&self.tags);
        world
            .tagged_child_states
            .clone_from(&self.tagged_child_states);
        world.settled_props.clone_from(&self.settled_props);
        world.ready_objects.clone_from(&self.ready_objects);
        world.live_objects.clone_from(&self.live_objects);
        world.finished_voices.clone_from(&self.finished_voices);
        world.polled_links.clone_from(&self.polled_links);
        world.reset_state.clone_from(&self.reset_state);
    }
}

/// Cloneable publication point shared by native gameplay owners and the Story tick.
/// A writer replaces one complete snapshot atomically; readers see the prior complete snapshot
/// or the new one, never a mixture of domains from separate ticks.
#[derive(Debug, Clone, Default)]
pub struct StoryGameplayInputsStore(Arc<RwLock<StoryGameplayInputsStoreState>>);

#[derive(Debug, Default)]
struct StoryGameplayInputsStoreState {
    revision: u64,
    session: u64,
    inputs: Option<StoryGameplayInputs>,
    players: player_owner::StoryPlayerStore,
    actors: actor_owner::StoryActorStore,
}

/// Capability held by the native gameplay owner for one installed Story mission.
/// Replacing a mission invalidates the previous capability before new inputs are accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryGameplayInputSession(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryGameplayInputsStoreError {
    Poisoned,
    RevisionExhausted,
    StaleSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryGameplayOwnerStoreError {
    Inputs(StoryGameplayInputsStoreError),
    Player(player_owner::StoryPlayerStoreError),
    Actor(actor_owner::StoryActorStoreError),
}

impl StoryGameplayInputsStore {
    /// Start a fresh mission publication epoch and discard the previous complete snapshot.
    pub fn begin_session(
        &self,
    ) -> Result<StoryGameplayInputSession, StoryGameplayInputsStoreError> {
        let mut current = self
            .0
            .write()
            .map_err(|_| StoryGameplayInputsStoreError::Poisoned)?;
        let revision = current
            .revision
            .checked_add(1)
            .ok_or(StoryGameplayInputsStoreError::RevisionExhausted)?;
        let session = current
            .session
            .checked_add(1)
            .ok_or(StoryGameplayInputsStoreError::RevisionExhausted)?;
        current.revision = revision;
        current.session = session;
        current.inputs = None;
        current.players.clear();
        current.actors.clear();
        Ok(StoryGameplayInputSession(session))
    }

    /// Bind one Rust-owned player to this mission's typed Story input state.
    pub fn bind_player(
        &self,
        session: StoryGameplayInputSession,
        slot: u32,
        state: StoryPlayerState,
    ) -> Result<player_owner::StoryPlayerHandle, StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        current
            .players
            .bind(slot, state)
            .map_err(StoryGameplayOwnerStoreError::Player)
    }

    /// Explicitly associate one logical input-router channel with a current Story player.
    /// The mapping is Rust-owned and generation checked; it does not infer guest player identity.
    pub fn bind_player_input_channel(
        &self,
        session: StoryGameplayInputSession,
        channel: u8,
        handle: player_owner::StoryPlayerHandle,
    ) -> Result<bool, StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(current.players.bind_input_channel(channel, handle))
    }

    /// Publish the four logical input-router channels for a native simulation step.
    /// Callers choose the step boundary and player mapping explicitly. `PlayerInput::slot` is
    /// retained as source metadata; the channel array index selects the bound player.
    pub fn publish_player_inputs_for_step(
        &self,
        session: StoryGameplayInputSession,
        inputs: [Option<PlayerInput>; 4],
    ) -> Result<(), StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        current.players.publish_input_channels_for_step(inputs);
        Ok(())
    }

    /// Replace one current Rust-owned player's complete Story facts.
    pub fn update_player(
        &self,
        session: StoryGameplayInputSession,
        handle: player_owner::StoryPlayerHandle,
        state: StoryPlayerState,
    ) -> Result<bool, StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(current.players.update(handle, state))
    }

    /// Publish one caller-routed native input value for a live Story player and simulation step.
    /// Session and player-generation checks reject delayed writes from replaced missions/players.
    pub fn set_player_input_for_step(
        &self,
        session: StoryGameplayInputSession,
        handle: player_owner::StoryPlayerHandle,
        input: Option<PlayerInput>,
    ) -> Result<bool, StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(current.players.set_input_for_step(handle, input))
    }

    /// Read a current step input through the same mission capability used to publish it.
    /// `Some(None)` is a live player with no input; outer `None` is a stale/unbound player.
    pub fn player_input_for_step(
        &self,
        session: StoryGameplayInputSession,
        handle: player_owner::StoryPlayerHandle,
    ) -> Result<Option<Option<PlayerInput>>, StoryGameplayOwnerStoreError> {
        let current = self.0.read().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(current.players.input_for_step(handle))
    }

    /// Resolve a caller-selected router channel under one mission-capability read.
    /// `None` means no current player is bound; a returned `None` input is a live player with no
    /// sample for this step. The returned generation handle is the only player identity here.
    pub fn player_input_for_channel(
        &self,
        session: StoryGameplayInputSession,
        channel: u8,
    ) -> Result<
        Option<(player_owner::StoryPlayerHandle, Option<PlayerInput>)>,
        StoryGameplayOwnerStoreError,
    > {
        let current = self.0.read().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(current.players.input_for_channel(channel))
    }

    /// Remove a player only when its slot-generation handle is still current.
    pub fn unbind_player(
        &self,
        session: StoryGameplayInputSession,
        handle: player_owner::StoryPlayerHandle,
    ) -> Result<Option<StoryPlayerState>, StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(current.players.unbind(handle))
    }

    /// Bind one Rust-owned actor/object to this mission's typed Story input state.
    pub fn bind_actor(
        &self,
        session: StoryGameplayInputSession,
        actor_key: u32,
        state: actor_owner::StoryActorState,
    ) -> Result<actor_owner::StoryActorHandle, StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        current
            .actors
            .bind(actor_key, state)
            .map_err(StoryGameplayOwnerStoreError::Actor)
    }

    /// Replace one current Rust-owned actor's complete Story facts.
    pub fn update_actor(
        &self,
        session: StoryGameplayInputSession,
        handle: actor_owner::StoryActorHandle,
        state: actor_owner::StoryActorState,
    ) -> Result<bool, StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(current.actors.update(handle, state))
    }

    /// Remove an actor only when its key-generation handle is still current.
    pub fn unbind_actor(
        &self,
        session: StoryGameplayInputSession,
        handle: actor_owner::StoryActorHandle,
    ) -> Result<Option<actor_owner::StoryActorState>, StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        Ok(current.actors.unbind(handle))
    }

    /// Publish player and actor facts atomically with the separately owned voice, link, and reset
    /// domains. The complete snapshot becomes visible only after every input has been assembled.
    pub fn publish_owned_player_actor_snapshot(
        &self,
        session: StoryGameplayInputSession,
        finished_voices: BTreeSet<u32>,
        polled_links: Vec<Link>,
        reset_state: Option<StoryResetState>,
    ) -> Result<(), StoryGameplayOwnerStoreError> {
        let mut current = self.0.write().map_err(|_| {
            StoryGameplayOwnerStoreError::Inputs(StoryGameplayInputsStoreError::Poisoned)
        })?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        let revision =
            current
                .revision
                .checked_add(1)
                .ok_or(StoryGameplayOwnerStoreError::Inputs(
                    StoryGameplayInputsStoreError::RevisionExhausted,
                ))?;
        let mut world = NativeStoryWorld::default();
        world.players = current.players.snapshot();
        current.actors.publish_into(&mut world);
        current.inputs = Some(StoryGameplayInputs::new(
            world.players,
            world.actor_health,
            world.tags,
            world.tagged_child_states,
            world.settled_props,
            world.ready_objects,
            world.live_objects,
            finished_voices,
            polled_links,
            reset_state,
        ));
        current.revision = revision;
        Ok(())
    }

    /// Publish a complete input snapshot assembled from the Rust-owned player and actor owners.
    ///
    /// Voice, link, and reset inputs remain explicit because they have separate owners. This is
    /// the intended gameplay attachment: call once after each owner has published its fixed tick.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_from_player_actor_owners(
        &self,
        session: StoryGameplayInputSession,
        players: &player_owner::StoryPlayerStore,
        actors: &actor_owner::StoryActorStore,
        finished_voices: BTreeSet<u32>,
        polled_links: Vec<Link>,
        reset_state: Option<StoryResetState>,
    ) -> Result<(), StoryGameplayInputsStoreError> {
        self.publish(
            session,
            StoryGameplayInputs::from_player_actor_owners(
                players,
                actors,
                finished_voices,
                polled_links,
                reset_state,
            ),
        )
    }

    pub fn publish(
        &self,
        session: StoryGameplayInputSession,
        inputs: StoryGameplayInputs,
    ) -> Result<(), StoryGameplayInputsStoreError> {
        let mut current = self
            .0
            .write()
            .map_err(|_| StoryGameplayInputsStoreError::Poisoned)?;
        if current.session != session.0 || current.session == 0 {
            return Err(StoryGameplayInputsStoreError::StaleSession);
        }
        current.revision = current
            .revision
            .checked_add(1)
            .ok_or(StoryGameplayInputsStoreError::RevisionExhausted)?;
        current.inputs = Some(inputs);
        Ok(())
    }

    pub fn snapshot(&self) -> Result<Option<StoryGameplayInputs>, StoryGameplayInputsStoreError> {
        self.0
            .read()
            .map(|current| current.inputs.clone())
            .map_err(|_| StoryGameplayInputsStoreError::Poisoned)
    }

    /// Return the latest complete snapshot only when it is newer than `after_revision`.
    /// A consumer can use this to decline a tick when its producer has not published fresh
    /// state, instead of replaying a stale world snapshot.
    pub fn snapshot_after(
        &self,
        after_revision: u64,
    ) -> Result<Option<(u64, StoryGameplayInputs)>, StoryGameplayInputsStoreError> {
        let current = self
            .0
            .read()
            .map_err(|_| StoryGameplayInputsStoreError::Poisoned)?;
        if current.revision <= after_revision {
            return Ok(None);
        }
        Ok(current
            .inputs
            .clone()
            .map(|inputs| (current.revision, inputs)))
    }
}

/// Measured child state values consumed by Story condition kind 36.
/// Unknown words are retained as data but never treated as a successful predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryChildState {
    Word2,
    Word3,
    Word5,
    Word7,
    Other(u32),
}

impl StoryChildState {
    pub const fn from_word(word: u32) -> Self {
        match word {
            2 => Self::Word2,
            3 => Self::Word3,
            5 => Self::Word5,
            7 => Self::Word7,
            other => Self::Other(other),
        }
    }

    const fn satisfies_state_condition(self) -> bool {
        matches!(self, Self::Word2 | Self::Word3 | Self::Word5 | Self::Word7)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StoryWorldEffect {
    PlayVoice {
        handle: u32,
        request: VoiceRequest,
    },
    Callback {
        callback: u32,
        target: CallbackTarget,
    },
    /// Decoded callback operation retained as typed gameplay intent for its native owner.
    CallbackOp {
        op: WorldOp,
        target: CallbackTarget,
    },
    /// T3415: committed typed player relocation, reported to the adjacent player owner.
    ResetPlayer {
        event: StoryResetEvent,
        disposition: ResetDisposition,
    },
    /// Result emitted by the typed Story objective aggregator.
    MissionEvent(MissionEvent),
    /// Typed Story session and objective snapshot paired with a terminal result request.
    MissionOutcome(StoryMissionResult),
    /// The result path requested the adjacent match/session owner to end the match.
    EndMatch,
    /// Decoded static objective-slot operation retained for the owning gameplay system.
    SlotOp {
        slot: u32,
        op: WorldOp,
    },
    SetObjective {
        actor_key: u32,
        index: u32,
    },
    ControlProp(PropRequest),
    Spawn {
        handle: u32,
        request: SpawnRequest,
    },
    DestroyObject {
        handle: u32,
    },
}

/// Complete typed context for a Story result consumer. The flow snapshot carries the active
/// selector and staged continuation alongside the objective result; consumers never need to
/// reconstruct those guards from guest memory after the result screen starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryMissionResult {
    pub flow: StoryFlow,
    pub outcome: MissionOutcome,
}

/// Native world snapshot plus an effect queue consumed by the engine's game-world owner.
/// Missing query data remains unsupported through `World`'s default methods; it is never
/// silently treated as a successful query here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NativeStoryWorld {
    pub players: Vec<StoryPlayerState>,
    pub actor_health: BTreeMap<u32, f32>,
    pub tags: BTreeSet<u32>,
    /// Typed child states for objects resolved by Story tag key. Missing keys stay unsupported.
    pub tagged_child_states: BTreeMap<u32, StoryChildState>,
    pub settled_props: BTreeSet<u32>,
    pub ready_objects: BTreeSet<u32>,
    pub live_objects: BTreeSet<u32>,
    pub finished_voices: BTreeSet<u32>,
    pub polled_links: Vec<Link>,
    pub effects: Vec<StoryWorldEffect>,
    /// Rust-owned inputs for the opt-in reset callbacks. `None` keeps the lifted callback path.
    pub reset_state: Option<StoryResetState>,
    next_voice_handle: u32,
    next_object_handle: u32,
}

impl NativeStoryWorld {
    pub fn new(players: Vec<StoryPlayerState>) -> Self {
        Self {
            players,
            next_voice_handle: 1,
            next_object_handle: 1,
            ..Self::default()
        }
    }

    /// Transfer pending native-world effects to the adjacent gameplay owner.
    ///
    /// Taking the queue makes ownership explicit: an effect is delivered once and cannot be
    /// replayed accidentally by a later Story tick.
    pub fn take_effects(&mut self) -> Vec<StoryWorldEffect> {
        std::mem::take(&mut self.effects)
    }

    fn allocate_voice_handle(&mut self) -> u32 {
        let handle = self.next_voice_handle.max(1);
        self.next_voice_handle = handle.wrapping_add(1).max(1);
        handle
    }

    fn allocate_object_handle(&mut self) -> u32 {
        let handle = self.next_object_handle.max(1);
        self.next_object_handle = handle.wrapping_add(1).max(1);
        handle
    }
}

impl World for NativeStoryWorld {
    fn play_voice(&mut self, request: &VoiceRequest) -> Option<u32> {
        let handle = self.allocate_voice_handle();
        self.effects.push(StoryWorldEffect::PlayVoice {
            handle,
            request: request.clone(),
        });
        Some(handle)
    }

    fn voice_finished(&mut self, handle: u32) -> bool {
        self.finished_voices.contains(&handle)
    }

    fn run_callback(&mut self, callback: u32, target: CallbackTarget) {
        self.effects
            .push(StoryWorldEffect::Callback { callback, target });
    }

    fn apply_callback(&mut self, op: &WorldOp, target: CallbackTarget) -> bool {
        if let WorldOp::ResetInputDriven {
            player,
            character,
            mode,
        } = op
        {
            if !super::reset_relocation::enabled() {
                return false;
            }
            let Some(state) = self.reset_state.as_mut() else {
                return false;
            };
            let event = StoryResetEvent {
                spawn_key: super::reset_relocation::SpawnKey(*character),
                player_slot: if *player == 0 {
                    PlayerSlot::First
                } else if *player == 1 {
                    PlayerSlot::Second
                } else {
                    return false;
                },
                placement_flags: PlacementFlags(*mode),
            };
            let disposition = match event.player_slot {
                PlayerSlot::First => super::reset_relocation::reset_first_player(event, state),
                PlayerSlot::Second => super::reset_relocation::reset_second_player(event, state),
            };
            let Some(disposition) = disposition else {
                return false;
            };
            self.effects
                .push(StoryWorldEffect::ResetPlayer { event, disposition });
            return true;
        }
        self.effects.push(StoryWorldEffect::CallbackOp {
            op: op.clone(),
            target,
        });
        true
    }

    fn apply_slot_op(&mut self, slot: u32, op: &WorldOp) -> bool {
        self.effects.push(StoryWorldEffect::SlotOp {
            slot,
            op: op.clone(),
        });
        true
    }

    fn set_objective(&mut self, actor_key: u32, index: u32) {
        self.effects
            .push(StoryWorldEffect::SetObjective { actor_key, index });
    }

    fn control_prop(&mut self, request: &PropRequest) {
        self.effects
            .push(StoryWorldEffect::ControlProp(request.clone()));
    }

    fn prop_settled(&mut self, key: u32) -> bool {
        self.settled_props.contains(&key)
    }

    fn spawn_object(&mut self, request: &SpawnRequest) -> Option<u32> {
        let handle = self.allocate_object_handle();
        self.live_objects.insert(handle);
        self.effects.push(StoryWorldEffect::Spawn {
            handle,
            request: request.clone(),
        });
        Some(handle)
    }

    fn object_ready(&mut self, handle: u32) -> bool {
        self.ready_objects.contains(&handle)
    }

    fn destroy_object(&mut self, handle: u32) {
        self.live_objects.remove(&handle);
        self.ready_objects.remove(&handle);
        self.effects
            .push(StoryWorldEffect::DestroyObject { handle });
    }

    fn object_exists(&self, handle: u32) -> bool {
        self.live_objects.contains(&handle)
    }

    fn player_count(&self) -> usize {
        self.players.len()
    }

    fn player_matches(&self, player: usize, variant: IdVariant, ids: &[u32]) -> bool {
        let Some(player) = self.players.get(player) else {
            return false;
        };
        match variant {
            IdVariant::CharacterId => ids.contains(&player.character_id),
            IdVariant::FollowingLink => ids.contains(&player.following_link),
            IdVariant::ResourceMembership => ids
                .iter()
                .any(|id| player.resource_memberships.contains(id)),
        }
    }

    fn first_player_flag(&self) -> bool {
        self.players
            .first()
            .is_some_and(|player| player.first_player_flag)
    }

    fn tag_exists(&self, key: u32) -> bool {
        self.tags.contains(&key)
    }

    fn condition_query(&mut self, query: &ConditionQuery) -> Option<bool> {
        match *query {
            ConditionQuery::ChildState { key } => {
                // The typed owner resolves the tag to the live child's state word. No entry means
                // the object or child is absent, which the measured kind-36 predicate reports false.
                Some(
                    self.tagged_child_states
                        .get(&key)
                        .is_some_and(|state| state.satisfies_state_condition()),
                )
            }
            ConditionQuery::Scene { .. }
            | ConditionQuery::TableField { .. }
            | ConditionQuery::FewerThanTwoOfClass => None,
        }
    }

    fn poll_link(&mut self, link: &Link) -> bool {
        self.polled_links.contains(link)
    }

    fn actor_health(&self, actor: u32) -> Option<f32> {
        self.actor_health.get(&actor).copied()
    }

    fn player_health(&self, player: usize) -> Option<f32> {
        self.players.get(player).map(|player| player.health)
    }

    fn players_all_down(&self) -> bool {
        !self.players.is_empty() && self.players.iter().all(|player| player.down)
    }

    fn mission_event(&mut self, event: MissionEvent) {
        self.effects.push(StoryWorldEffect::MissionEvent(event));
    }

    fn end_match(&mut self) {
        self.effects.push(StoryWorldEffect::EndMatch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> StoryPlayerState {
        StoryPlayerState {
            character_id: 41,
            following_link: 9,
            resource_memberships: BTreeSet::from([3, 7]),
            first_player_flag: true,
            health: 75.0,
            down: false,
        }
    }

    #[test]
    fn gameplay_input_store_scopes_snapshots_to_one_mission_session() {
        let store = StoryGameplayInputsStore::default();
        assert_eq!(store.snapshot().unwrap(), None);
        let first_session = store.begin_session().unwrap();

        let inputs = StoryGameplayInputs::new(
            vec![player()],
            BTreeMap::from([(41, 75.0)]),
            BTreeSet::from([9]),
            BTreeMap::from([(9, StoryChildState::Word3)]),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::from([41]),
            BTreeSet::new(),
            Vec::new(),
            None,
        );
        store.publish(first_session, inputs.clone()).unwrap();
        assert_eq!(store.snapshot().unwrap(), Some(inputs));

        let second_session = store.begin_session().unwrap();
        assert_eq!(store.snapshot().unwrap(), None);
        assert_eq!(
            store.publish(
                first_session,
                StoryGameplayInputs::new(
                    Vec::new(),
                    BTreeMap::new(),
                    BTreeSet::new(),
                    BTreeMap::new(),
                    BTreeSet::new(),
                    BTreeSet::new(),
                    BTreeSet::new(),
                    BTreeSet::new(),
                    Vec::new(),
                    None,
                )
            ),
            Err(StoryGameplayInputsStoreError::StaleSession)
        );
        store
            .publish(
                second_session,
                StoryGameplayInputs::new(
                    Vec::new(),
                    BTreeMap::new(),
                    BTreeSet::new(),
                    BTreeMap::new(),
                    BTreeSet::new(),
                    BTreeSet::new(),
                    BTreeSet::new(),
                    BTreeSet::new(),
                    Vec::new(),
                    None,
                ),
            )
            .unwrap();
    }

    #[test]
    fn mission_owner_store_publishes_typed_player_actor_state_and_invalidates_old_handles() {
        let store = StoryGameplayInputsStore::default();
        let first = store.begin_session().unwrap();
        let player_handle = store.bind_player(first, 0, player()).unwrap();
        let actor_handle = store
            .bind_actor(
                first,
                41,
                actor_owner::StoryActorState {
                    health: Some(62.5),
                    tags: BTreeSet::from([9]),
                    tagged_child_states: BTreeMap::from([(9, StoryChildState::Word5)]),
                    settled_prop: true,
                    ready: true,
                    live: true,
                },
            )
            .unwrap();

        store
            .publish_owned_player_actor_snapshot(first, BTreeSet::from([7]), Vec::new(), None)
            .unwrap();
        let snapshot = store.snapshot().unwrap().unwrap();
        assert_eq!(snapshot.players, vec![player()]);
        assert_eq!(snapshot.actor_health, BTreeMap::from([(41, 62.5)]));
        assert_eq!(snapshot.tags, BTreeSet::from([9]));
        assert_eq!(
            snapshot.tagged_child_states,
            BTreeMap::from([(9, StoryChildState::Word5)])
        );
        assert_eq!(snapshot.settled_props, BTreeSet::from([41]));
        assert_eq!(snapshot.ready_objects, BTreeSet::from([41]));
        assert_eq!(snapshot.live_objects, BTreeSet::from([41]));
        assert_eq!(snapshot.finished_voices, BTreeSet::from([7]));

        let second = store.begin_session().unwrap();
        assert_eq!(store.snapshot().unwrap(), None);
        assert_eq!(
            store.update_player(first, player_handle, player()),
            Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession
            ))
        );
        assert_eq!(
            store.update_actor(first, actor_handle, actor_owner::StoryActorState::default()),
            Err(StoryGameplayOwnerStoreError::Inputs(
                StoryGameplayInputsStoreError::StaleSession
            ))
        );
        let new_player_handle = store.bind_player(second, 0, player()).unwrap();
        let new_actor_handle = store
            .bind_actor(second, 41, actor_owner::StoryActorState::default())
            .unwrap();
        assert!(new_player_handle.generation() > player_handle.generation());
        assert!(new_actor_handle.generation() > actor_handle.generation());
        assert!(
            !store
                .update_player(second, player_handle, player())
                .unwrap()
        );
        assert!(
            !store
                .update_actor(
                    second,
                    actor_handle,
                    actor_owner::StoryActorState::default()
                )
                .unwrap()
        );
    }

    #[test]
    fn typed_player_queries_use_explicit_ids_and_memberships() {
        let world = NativeStoryWorld::new(vec![player()]);
        assert!(world.player_matches(0, IdVariant::CharacterId, &[41]));
        assert!(world.player_matches(0, IdVariant::FollowingLink, &[9]));
        assert!(world.player_matches(0, IdVariant::ResourceMembership, &[7]));
        assert!(!world.player_matches(1, IdVariant::CharacterId, &[41]));
        assert_eq!(world.player_health(0), Some(75.0));
        assert_eq!(world.player_health(1), None);
        assert_eq!(world.actor_health(41), None);
        assert!(world.first_player_flag());
    }

    #[test]
    fn mission_result_and_end_request_are_forwarded_once_in_order() {
        let mut world = NativeStoryWorld::default();
        world.mission_event(MissionEvent::ObjectiveFailedPending);
        assert_eq!(
            world.take_effects(),
            vec![StoryWorldEffect::MissionEvent(
                MissionEvent::ObjectiveFailedPending
            )]
        );

        world.mission_event(MissionEvent::Complete);
        world.end_match();
        assert_eq!(
            world.take_effects(),
            vec![
                StoryWorldEffect::MissionEvent(MissionEvent::Complete),
                StoryWorldEffect::EndMatch,
            ]
        );
        assert!(world.take_effects().is_empty());
    }

    #[test]
    fn child_state_condition_uses_only_the_typed_tag_snapshot() {
        let mut world = NativeStoryWorld::default();
        for (key, state) in [(10, 2), (11, 3), (12, 5), (13, 7), (14, 4)] {
            world
                .tagged_child_states
                .insert(key, StoryChildState::from_word(state));
        }
        for (key, expected) in [
            (10, true),
            (11, true),
            (12, true),
            (13, true),
            (14, false),
            (99, false),
        ] {
            assert_eq!(
                world.condition_query(&ConditionQuery::ChildState { key }),
                Some(expected),
                "tag key {key}"
            );
        }
        assert_eq!(
            world.condition_query(&ConditionQuery::Scene {
                kind: 1,
                args: [0; 4],
            }),
            None,
            "unsupported scene state must remain explicit"
        );
    }

    #[test]
    fn typed_spawn_lifecycle_emits_effects_and_checks_owned_handles() {
        let mut world = NativeStoryWorld::default();
        let request = SpawnRequest {
            group_key: 1,
            object_type: 2,
            tag: 3,
            variant: 4,
            scale: 1.0,
            position: [0.0; 3],
            orientation: [0.0; 3],
            callback: 5,
            tick_callback: 6,
            flags: 7,
        };
        let handle = world.spawn_object(&request).unwrap();
        assert!(world.object_exists(handle));
        assert!(!world.object_ready(handle));
        world.ready_objects.insert(handle);
        assert!(world.object_ready(handle));
        world.destroy_object(handle);
        assert!(!world.object_exists(handle));
        assert_eq!(world.effects.len(), 2);
        assert!(
            matches!(world.effects[0], StoryWorldEffect::Spawn { handle: h, .. } if h == handle)
        );
        assert_eq!(world.effects[1], StoryWorldEffect::DestroyObject { handle });
    }

    #[test]
    fn decoded_callbacks_stay_typed_until_the_native_owner_consumes_them() {
        let mut world = NativeStoryWorld::default();
        let target = CallbackTarget::Instance(4);
        let op = WorldOp::NpcGoal { character: 41 };

        assert!(world.apply_callback(&op, target));
        assert!(world.apply_slot_op(2, &op));
        assert_eq!(
            world.take_effects(),
            vec![
                StoryWorldEffect::CallbackOp {
                    op: op.clone(),
                    target,
                },
                StoryWorldEffect::SlotOp { slot: 2, op },
            ]
        );
        assert!(world.take_effects().is_empty());
    }
}
#[path = "actor_owner.rs"]
pub mod actor_owner;
#[path = "player_owner.rs"]
pub mod player_owner;
