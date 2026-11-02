//! Owned pure-game state sketch, deliberately separate from repr(C) guest records.
use std::sync::atomic::{AtomicU64, Ordering};

use tsfp_input::player::{PlayerInput, Vec2};
use tsfp_input::{
    look::{Integration, LookIntegration, LookPolicy, LookProfile, LookState},
    player::AssistMask,
};
use tsfp_types::TsfpVec3;

static NEXT_PLAYER_STATE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlayerStateId(u64);

impl PlayerStateId {
    fn next() -> Self {
        let id = NEXT_PLAYER_STATE_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .expect("Rust player-state identity space exhausted");
        Self(id)
    }
}

/// Generational handles prevent stale references after object-pool reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterId {
    pub slot: u32,
    pub generation: u32,
}

/// Rust-owned identity for one published player input value. This is a native snapshot
/// revision, not a recovered guest input generation or simulation tick counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlayerInputRevision(u64);

impl PlayerInputRevision {
    const INITIAL: Self = Self(0);

    fn next(self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("Rust player input revision space exhausted"),
        )
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Immutable copy of the command published to one Rust-owned player state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerInputSnapshot {
    owner: PlayerStateId,
    revision: PlayerInputRevision,
    input: PlayerInput,
}

impl PlayerInputSnapshot {
    pub const fn revision(self) -> PlayerInputRevision {
        self.revision
    }

    pub const fn input(self) -> PlayerInput {
        self.input
    }
}

/// Native movement/look values selected from the same player input publication.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStepInput {
    pub revision: PlayerInputRevision,
    pub move_axis: Vec2,
    pub look_applied: bool,
}

#[derive(Debug)]
pub struct PlayerState {
    pub pose: TsfpVec3,
    pub facing: f32,
    input_owner: PlayerStateId,
    input_revision: PlayerInputRevision,
    input_snapshot: Option<PlayerInputSnapshot>,
    look_state: Option<LookState>,
}

impl PlayerState {
    pub fn new(pose: TsfpVec3, facing: f32) -> Self {
        Self {
            pose,
            facing,
            input_owner: PlayerStateId::next(),
            input_revision: PlayerInputRevision::INITIAL,
            input_snapshot: None,
            look_state: None,
        }
    }

    /// Replace the typed input for the current Rust simulation step. `None` clears a
    /// disconnected or inactive slot; callers publish again at the next step boundary.
    /// The `PlayerInput::step` value belongs to the Rust input/replay contract and is not
    /// evidence of an original-game input generation counter.
    pub fn set_input_for_step(&mut self, input: Option<PlayerInput>) -> PlayerInputRevision {
        self.input_revision = self.input_revision.next();
        self.input_snapshot = input.map(|input| PlayerInputSnapshot {
            owner: self.input_owner,
            revision: self.input_revision,
            input,
        });
        self.input_revision
    }

    /// Capture the owned input and revision so multiple native consumers can share one value.
    pub fn input_snapshot(&self) -> Option<PlayerInputSnapshot> {
        self.input_snapshot
    }

    /// Reject a retained snapshot after this player's input has been replaced or cleared.
    pub fn input_snapshot_is_current(&self, snapshot: PlayerInputSnapshot) -> bool {
        self.input_snapshot.is_some_and(|current| {
            snapshot.owner == self.input_owner && current.revision == snapshot.revision
        })
    }

    pub fn input_for_step(&self) -> Option<PlayerInput> {
        self.input_snapshot.map(|snapshot| snapshot.input)
    }

    /// Return the raw movement axes in the caller-selected current-step sample.
    /// Simulation owns axis shaping and tick cadence; this accessor does not infer
    /// original input-generation or catch-up semantics from `PlayerInput::step`.
    pub fn move_axis_for_step(&self) -> Option<Vec2> {
        self.input_snapshot
            .and_then(|snapshot| self.move_axis_for_snapshot(snapshot))
    }

    /// Read movement axes from one previously captured snapshot. A replacement or clear
    /// invalidates the snapshot, so sibling consumers cannot silently use different inputs.
    pub fn move_axis_for_snapshot(&self, snapshot: PlayerInputSnapshot) -> Option<Vec2> {
        self.input_snapshot_is_current(snapshot)
            .then_some(snapshot.input.move_axis)
    }

    /// Install caller-owned look state. Its flags, profile and slew history must come from
    /// the native player owner; this does not decode a guest record or invent defaults.
    pub fn set_look_state(&mut self, state: LookState) {
        self.facing = state.yaw;
        self.look_state = Some(state);
    }

    pub fn look_state(&self) -> Option<&LookState> {
        self.look_state.as_ref()
    }

    /// Feed the selected current-step input through the existing readable Rust look policy.
    /// The caller owns controller selection and simulation timing. This method neither
    /// consumes the input sample nor assigns its `step` field to an original tick sequence.
    pub fn step_look_from_input<P: LookPolicy>(
        &mut self,
        integration: &mut LookIntegration<P>,
        profile: LookProfile,
        allowed: AssistMask,
        config: Integration,
    ) -> bool {
        let Some(snapshot) = self.input_snapshot else {
            return false;
        };
        self.step_look_from_snapshot(snapshot, integration, profile, allowed, config)
    }

    /// Apply look from a shared caller-captured input snapshot only while it is current.
    /// This lets look and movement consumers observe the same native publication revision.
    pub fn step_look_from_snapshot<P: LookPolicy>(
        &mut self,
        snapshot: PlayerInputSnapshot,
        integration: &mut LookIntegration<P>,
        profile: LookProfile,
        allowed: AssistMask,
        config: Integration,
    ) -> bool {
        if !self.input_snapshot_is_current(snapshot) {
            return false;
        }
        let Some(state) = self.look_state.as_mut() else {
            return false;
        };
        let applied = integration.step(
            state,
            snapshot.input.look,
            snapshot.input.move_axis,
            profile,
            allowed,
            config,
        );
        if applied {
            self.facing = state.yaw;
        }
        applied
    }

    /// Consume movement and look from one current publication for a caller-owned step.
    /// Movement remains raw axes; the caller owns shaping and tick assignment.
    pub fn step_input_for_step<P: LookPolicy>(
        &mut self,
        integration: &mut LookIntegration<P>,
        profile: LookProfile,
        allowed: AssistMask,
        config: Integration,
    ) -> Option<PlayerStepInput> {
        let snapshot = self.input_snapshot?;
        let look_applied =
            self.step_look_from_snapshot(snapshot, integration, profile, allowed, config);
        Some(PlayerStepInput {
            revision: snapshot.revision,
            move_axis: snapshot.input.move_axis,
            look_applied,
        })
    }
}
/// Source mappings: NPC pointer table 0x7A3580, *player global 0x7B0C48.
/// The NPC count must come from a recovered live producer, never a guessed capacity.
#[derive(Debug, Default)]
pub struct WorldState {
    pub npc_order: Vec<CharacterId>,
    pub player: Option<PlayerState>,
}
/// Hybrid mappings remain authoritative until a complete ownership unit migrates.
/// Decoders must preserve aliases and fail on unsupported/unresolved fields.
pub trait StateBridge {
    type Error;
    fn import_world(&mut self) -> Result<WorldState, Self::Error>;
    fn export_world(&mut self, world: &WorldState) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{PlayerInputRevision, PlayerState};
    use tsfp_input::player::{AssistMask, Look, PlayerInput, SourceKind, Vec2};
    use tsfp_types::TsfpVec3;

    fn input(step: u64, slot: u8) -> PlayerInput {
        PlayerInput {
            step,
            slot,
            source: SourceKind::Pad(7),
            held: 1,
            pressed: 1,
            released: 0,
            move_axis: Vec2 { x: 0.25, y: 0.75 },
            look: Look::Stick(Vec2 { x: -0.5, y: 0.125 }),
            triggers: [0.0, 1.0],
            menu: Default::default(),
            assists: AssistMask::PROFILE,
        }
    }

    #[test]
    fn player_state_replaces_and_clears_its_step_input() {
        let mut player = PlayerState::new(
            TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );
        assert_eq!(player.input_for_step(), None);

        let first = input(4, 1);
        let first_revision = player.set_input_for_step(Some(first));
        assert_eq!(player.input_for_step(), Some(first));
        let first_snapshot = player.input_snapshot().unwrap();
        assert_eq!(first_snapshot.revision(), first_revision);
        assert_eq!(first_snapshot.input(), first);
        assert!(player.input_snapshot_is_current(first_snapshot));

        let next = input(5, 1);
        let next_revision = player.set_input_for_step(Some(next));
        assert_eq!(player.input_for_step(), Some(next));
        let next_snapshot = player.input_snapshot().unwrap();
        assert_ne!(first_revision, next_revision);
        assert_eq!(first_snapshot.input(), first);
        assert!(!player.input_snapshot_is_current(first_snapshot));
        assert!(player.input_snapshot_is_current(next_snapshot));

        let clear_revision = player.set_input_for_step(None);
        assert_ne!(next_revision, clear_revision);
        assert_eq!(player.input_for_step(), None);
        assert_eq!(player.input_snapshot(), None);
        assert!(!player.input_snapshot_is_current(next_snapshot));
    }

    #[test]
    #[should_panic(expected = "Rust player input revision space exhausted")]
    fn player_input_revision_exhaustion_does_not_reuse_snapshot_identity() {
        PlayerInputRevision(u64::MAX).next();
    }

    #[test]
    fn player_state_rejects_another_players_snapshot_at_the_same_revision() {
        let mut first = PlayerState::new(
            TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );
        let mut second = PlayerState::new(
            TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );
        first.set_input_for_step(Some(input(1, 0)));
        second.set_input_for_step(Some(input(2, 1)));
        let first_snapshot = first.input_snapshot().unwrap();
        let second_snapshot = second.input_snapshot().unwrap();

        assert_eq!(first_snapshot.revision(), second_snapshot.revision());
        assert!(first.input_snapshot_is_current(first_snapshot));
        assert!(!second.input_snapshot_is_current(first_snapshot));
        assert_eq!(second.move_axis_for_snapshot(first_snapshot), None);
    }

    #[test]
    fn player_state_exposes_raw_movement_from_only_the_selected_step_sample() {
        let mut player = PlayerState::new(
            TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );
        assert_eq!(player.move_axis_for_step(), None);

        let mut selected = input(40, 3);
        selected.move_axis = Vec2 { x: -0.25, y: 0.75 };
        player.set_input_for_step(Some(selected));
        assert_eq!(player.move_axis_for_step(), Some(selected.move_axis));

        let mut next = input(2, 0);
        next.move_axis = Vec2 { x: 0.5, y: -1.0 };
        player.set_input_for_step(Some(next));
        assert_eq!(player.move_axis_for_step(), Some(next.move_axis));

        player.set_input_for_step(None);
        assert_eq!(player.move_axis_for_step(), None);
    }

    #[test]
    fn player_state_routes_selected_step_input_through_native_look_integration() {
        use tsfp_input::look::{LookIntegration, OriginalLook};
        use tsfp_input::player::Look;

        let mut player = PlayerState::new(
            TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            1.0,
        );
        player.set_look_state(tsfp_input::look::LookState {
            yaw: 1.0,
            pitch: 59.0,
            ramp: 1.0,
            state: 2,
            integration_allowed: true,
            ..Default::default()
        });
        let mut selected = input(4, 1);
        selected.look = Look::Delta {
            yaw_deg: 4.0,
            pitch_deg: 4.0,
        };
        player.set_input_for_step(Some(selected));
        let snapshot = player.input_snapshot().expect("published input snapshot");
        assert_eq!(
            player.move_axis_for_snapshot(snapshot),
            Some(selected.move_axis)
        );

        let mut integration = LookIntegration::<OriginalLook> {
            enabled: true,
            ..Default::default()
        };
        let profile = tsfp_input::look::LookProfile {
            flags: 0xff,
            crosshair: 2,
            turn_speed: 0.5,
        };

        assert!(player.step_look_from_snapshot(
            snapshot,
            &mut integration,
            profile,
            AssistMask::NONE,
            tsfp_input::look::Integration::default(),
        ));
        let state = player.look_state().expect("installed native look state");
        assert_eq!(player.facing, state.yaw);
        assert_ne!((state.yaw, state.pitch), (1.0, 59.0));
        assert_eq!(player.input_for_step(), Some(selected));

        let before_stale_sample = *player.look_state().unwrap();
        player.set_input_for_step(Some(input(5, 1)));
        assert_eq!(player.move_axis_for_snapshot(snapshot), None);
        assert!(!player.step_look_from_snapshot(
            snapshot,
            &mut integration,
            profile,
            AssistMask::NONE,
            tsfp_input::look::Integration::default(),
        ));
        assert_eq!(*player.look_state().unwrap(), before_stale_sample);
    }

    #[test]
    fn player_state_step_input_pairs_movement_and_look_from_one_revision() {
        use tsfp_input::look::{LookIntegration, OriginalLook};

        let mut player = PlayerState::new(
            TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );
        player.set_look_state(tsfp_input::look::LookState {
            yaw: 0.0,
            state: 2,
            integration_allowed: true,
            ..Default::default()
        });
        let mut selected = input(71, 2);
        selected.move_axis = Vec2 { x: -0.75, y: 0.5 };
        player.set_input_for_step(Some(selected));
        let revision = player.input_snapshot().unwrap().revision();
        let mut integration = LookIntegration::<OriginalLook> {
            enabled: true,
            ..Default::default()
        };
        let profile = tsfp_input::look::LookProfile {
            flags: 0xff,
            crosshair: 2,
            turn_speed: 0.5,
        };

        let consumed = player
            .step_input_for_step(
                &mut integration,
                profile,
                AssistMask::NONE,
                tsfp_input::look::Integration::default(),
            )
            .expect("one current input publication");

        assert_eq!(consumed.revision, revision);
        assert_eq!(consumed.move_axis, selected.move_axis);
        assert!(consumed.look_applied);
        assert_eq!(player.facing, player.look_state().unwrap().yaw);
        assert_eq!(player.input_for_step(), Some(selected));
    }

    #[test]
    fn player_state_step_input_returns_none_when_input_is_cleared() {
        use tsfp_input::look::{LookIntegration, OriginalLook};

        let mut player = PlayerState::new(
            TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
        );
        let mut integration = LookIntegration::<OriginalLook>::default();
        assert!(
            player
                .step_input_for_step(
                    &mut integration,
                    tsfp_input::look::LookProfile {
                        flags: 0,
                        crosshair: 0,
                        turn_speed: 0.0,
                    },
                    AssistMask::NONE,
                    tsfp_input::look::Integration::default(),
                )
                .is_none()
        );
    }

    #[test]
    fn player_state_leaves_look_unchanged_without_selected_input_or_enabled_state() {
        use tsfp_input::look::{LookIntegration, OriginalLook};

        let mut player = PlayerState::new(
            TsfpVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            7.0,
        );
        player.set_look_state(tsfp_input::look::LookState {
            yaw: 7.0,
            state: 2,
            integration_allowed: true,
            ..Default::default()
        });
        let before = *player.look_state().expect("installed native look state");
        let mut integration = LookIntegration::<OriginalLook>::default();
        let profile = tsfp_input::look::LookProfile {
            flags: 0,
            crosshair: 0,
            turn_speed: 0.0,
        };
        let config = tsfp_input::look::Integration::default();

        assert!(!player.step_look_from_input(&mut integration, profile, AssistMask::NONE, config,));
        player.set_input_for_step(Some(input(1, 0)));
        assert!(!player.step_look_from_input(&mut integration, profile, AssistMask::NONE, config,));
        assert_eq!(
            *player.look_state().expect("installed native look state"),
            before
        );
        assert_eq!(player.facing, 7.0);
    }
}

/// Proposed pure-runtime tick; not a measured original-title frequency.
pub const SIMULATION_HZ: u32 = 60;

/// Verified integer leaf primitives shared with the hybrid archive.
pub mod leaf;

/// T1895 wave 4c second-half engine/core ports (Tier 0).
pub mod w4c_b;

/// T2409 typed owner for delivered vblank count.
pub mod frame_state;

/// T3409 one-shot display-clock deadline for the loading-worker startup seam.
pub mod vblank_clock;

/// T1670 native owner for the original 75-entry simulation callback table.
pub mod game_timer;
/// T1907 shared initializers (Tier 0).
pub mod initializers;
/// T1907 launch configuration ports and explicit guest record views (Tier 0).
pub mod launch;
