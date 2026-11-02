// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed payload pose, shot admission, and shot-start ordering contracts (T2386/T2411).
//!
//! These records are opt-in Rust-owned state. They do not create a scene object, own its
//! lifetime, load resources, or dispatch a projectile. See docs/t2386-weapon-payload-pose.md.

use crate::object_pool::ObjectId;
use crate::weapon_hands::WeaponRecordId;
use crate::weapon_hands_inventory::{AMMO_TYPES, ENTRY_COUNT, Inventory};
use crate::weapon_inventory::WeaponPlayerHandle;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadRotationMode {
    Yaw,
    Euler,
}

/// Resolved, typed inputs at the C1950 payload-creation boundary. The definition table lookup and
/// template object lifetime belong to neighboring owners; no guest pointer crosses this seam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C1950PayloadInputs {
    pub definition_angles_degrees: [f32; 3],
    pub staged_yaw_degrees: f32,
    pub template: Option<C1950PayloadTemplate>,
}

/// The two template fields consumed by the documented C1950 payload writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C1950PayloadTemplate {
    pub flags: u32,
    pub angles_degrees: [f32; 3],
}

/// The creation-time component triple consumed by the supported object transform reader.
/// Components are pitch/yaw/roll in degrees, while `staged_yaw_degrees` is the separate +0xC4
/// animation accumulator. It is deliberately not folded into the consumed +0x70 yaw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponPayloadPose {
    pub pitch_yaw_roll_degrees: [f32; 3],
    pub staged_yaw_degrees: f32,
    pub rotation_mode: PayloadRotationMode,
}

impl WeaponPayloadPose {
    /// Build the constructor intermediate from the already-decoded definition variant.
    /// Ordered equality preserves signed-zero behavior; NaN components select Euler mode.
    pub fn from_definition(angles_degrees: [f32; 3], staged_yaw_degrees: f32) -> Self {
        let [pitch, _, roll] = angles_degrees;
        let rotation_mode = if pitch == 0.0 && roll == 0.0 {
            PayloadRotationMode::Yaw
        } else {
            PayloadRotationMode::Euler
        };
        Self {
            pitch_yaw_roll_degrees: angles_degrees,
            staged_yaw_degrees,
            rotation_mode,
        }
    }

    /// Apply the ordered constructor and C1950 template stages to already-resolved values.
    /// C1950 forces Euler mode even when the optional template is absent or its angle flag is clear.
    pub fn from_c1950_inputs(inputs: C1950PayloadInputs) -> Self {
        let mut pose =
            Self::from_definition(inputs.definition_angles_degrees, inputs.staged_yaw_degrees);
        let template = inputs.template.unwrap_or(C1950PayloadTemplate {
            flags: 0,
            angles_degrees: [0.0; 3],
        });
        pose.apply_c1950_template(template.flags, template.angles_degrees);
        pose
    }

    /// Apply C1950's post-constructor writes. It always forces Euler mode; flag `0x2` additionally
    /// replaces the consumed angle triple. Staged yaw remains the constructor's independent value.
    pub fn apply_c1950_template(&mut self, template_flags: u32, angles_degrees: [f32; 3]) -> bool {
        self.rotation_mode = PayloadRotationMode::Euler;
        if template_flags & 0x2 == 0 {
            return false;
        }
        self.pitch_yaw_roll_degrees = angles_degrees;
        true
    }
}

/// Error from binding a creation-time weapon payload pose to a lifecycle-owned object slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponPayloadLifecycleError {
    SlotOutOfRange,
    SlotOccupied,
    AlreadyAttached,
    NotAttached,
    StaleObjectId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BoundWeaponPayload {
    object: ObjectId,
    pose: WeaponPayloadPose,
}

/// Owns the weapon payload pose attached to each native object-pool slot.
///
/// The pool slot is only an index. Every operation checks its generation-bearing `ObjectId`, so
/// a recycled slot cannot inherit or mutate the previous object's pose. This table owns payload
/// pose state only; it does not allocate pool objects, resolve resources, or publish render data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponPayloadLifecycle {
    slots: Vec<Option<BoundWeaponPayload>>,
}

impl WeaponPayloadLifecycle {
    pub fn with_capacity(object_slots: usize) -> Self {
        Self {
            slots: vec![None; object_slots],
        }
    }

    /// Attach pose state to a newly-created object identity. Reattachment and slot reuse require
    /// an explicit successful `retire` first.
    pub fn attach(
        &mut self,
        object: ObjectId,
        pose: WeaponPayloadPose,
    ) -> Result<(), WeaponPayloadLifecycleError> {
        let slot = self
            .slots
            .get_mut(object.slot as usize)
            .ok_or(WeaponPayloadLifecycleError::SlotOutOfRange)?;
        match slot {
            None => {
                *slot = Some(BoundWeaponPayload { object, pose });
                Ok(())
            }
            Some(existing) if existing.object == object => {
                Err(WeaponPayloadLifecycleError::AlreadyAttached)
            }
            Some(_) => Err(WeaponPayloadLifecycleError::SlotOccupied),
        }
    }

    /// Read pose state only when both slot and generation match the live identity.
    pub fn pose(&self, object: ObjectId) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
        let bound = self
            .slots
            .get(object.slot as usize)
            .ok_or(WeaponPayloadLifecycleError::SlotOutOfRange)?
            .as_ref()
            .ok_or(WeaponPayloadLifecycleError::NotAttached)?;
        if bound.object != object {
            return Err(WeaponPayloadLifecycleError::StaleObjectId);
        }
        Ok(bound.pose)
    }

    /// Replace the attached pose while preserving the owning object generation.
    pub fn set_pose(
        &mut self,
        object: ObjectId,
        pose: WeaponPayloadPose,
    ) -> Result<(), WeaponPayloadLifecycleError> {
        let bound = self
            .slots
            .get_mut(object.slot as usize)
            .ok_or(WeaponPayloadLifecycleError::SlotOutOfRange)?
            .as_mut()
            .ok_or(WeaponPayloadLifecycleError::NotAttached)?;
        if bound.object != object {
            return Err(WeaponPayloadLifecycleError::StaleObjectId);
        }
        bound.pose = pose;
        Ok(())
    }

    /// Retire the exact object generation and its payload state.
    pub fn retire(
        &mut self,
        object: ObjectId,
    ) -> Result<WeaponPayloadPose, WeaponPayloadLifecycleError> {
        let slot = self
            .slots
            .get_mut(object.slot as usize)
            .ok_or(WeaponPayloadLifecycleError::SlotOutOfRange)?;
        match slot {
            None => Err(WeaponPayloadLifecycleError::NotAttached),
            Some(bound) if bound.object != object => {
                Err(WeaponPayloadLifecycleError::StaleObjectId)
            }
            Some(_) => Ok(slot.take().expect("validated payload slot").pose),
        }
    }
}

/// Typed ammo-table index for the shot path. Unlike hand reload metadata, this accepts type zero:
/// the bounded 0x1BC650 contract does not establish a nonzero sentinel for this read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotAmmoType(u8);

impl ShotAmmoType {
    pub const fn new(value: u8) -> Option<Self> {
        if (value as usize) < AMMO_TYPES {
            Some(Self(value))
        } else {
            None
        }
    }
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Data resolved by the neighbouring ammo/resource-record owner for each inventory entry.
/// `resource_flags` is the result of the documented 0x810C0-derived field helper; no guest
/// pointer or callback ABI crosses into the Rust-owned firing decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShotAmmoSource {
    pub weapon: Option<WeaponRecordId>,
    pub resource_flags: u32,
    pub ammo_type: Option<ShotAmmoType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotSelection {
    pub weapon: WeaponRecordId,
    pub ammo_type: Option<ShotAmmoType>,
    pub reserve_consumed: bool,
    /// Emission may be suppressed after reserve consumption by actor gates.
    pub emission_allowed: bool,
}

/// Typed owner context carried from shot-readiness evaluation into fresh projectile creation.
/// The actor identity is always present; player inventory identity is separate because the
/// original projectile descriptor stores actor and owner-extension inputs in different fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponProjectileOwner {
    actor: ObjectId,
    player_extension: Option<WeaponPlayerHandle>,
}

impl WeaponProjectileOwner {
    pub const fn new(actor: ObjectId, player_extension: Option<WeaponPlayerHandle>) -> Self {
        Self {
            actor,
            player_extension,
        }
    }

    pub const fn actor(self) -> ObjectId {
        self.actor
    }

    pub const fn player_extension(self) -> Option<WeaponPlayerHandle> {
        self.player_extension
    }

    /// Rust-owned attribution follows the owner extension when present, otherwise the actor.
    /// This models the measured source choice without exposing either guest pointer or treating
    /// the original descriptor's opaque owner-derived dword as a Rust memory address.
    pub const fn attribution(self) -> WeaponProjectileAttribution {
        match self.player_extension {
            Some(owner) => WeaponProjectileAttribution::PlayerExtension(owner),
            None => WeaponProjectileAttribution::Actor(self.actor),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponProjectileAttribution {
    PlayerExtension(WeaponPlayerHandle),
    Actor(ObjectId),
}

/// State words read from the exact actor object passed to the original readiness routine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponOwnerReadinessState {
    pub state: u32,
    pub transition_state: u32,
}

/// Typed access to Rust-owned actor state and transition services used by weapon readiness.
/// Implementations must resolve these queries through the supplied generation-checked actor.
pub trait WeaponReadinessSource {
    fn owner_state(&mut self, actor: ObjectId) -> Option<WeaponOwnerReadinessState>;
    fn transition_active(&mut self, actor: ObjectId, transition: u32) -> bool;
}

/// Result of checking shot readiness, retaining the same typed owner for the later spawn edge.
/// Its fields are private so shot startup cannot pair a readiness result with another actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponShotReadiness {
    owner: WeaponProjectileOwner,
    ready: bool,
}

impl WeaponShotReadiness {
    pub const fn owner(self) -> WeaponProjectileOwner {
        self.owner
    }

    pub const fn is_ready(self) -> bool {
        self.ready
    }
}

/// Evaluate the measured 0x1B9A30 policy against Rust-owned state. Transition checks keep their
/// observed order and short-circuit behavior: 0x21, 0x14, then 0x22.
pub fn evaluate_shot_readiness<R: WeaponReadinessSource>(
    owner: WeaponProjectileOwner,
    source: &mut R,
) -> WeaponShotReadiness {
    let actor = owner.actor();
    let Some(state) = source.owner_state(actor) else {
        return WeaponShotReadiness {
            owner,
            ready: false,
        };
    };

    let transition_matches = match state.state {
        9 | 14 | 19 => state.transition_state == state.state,
        18 => state.transition_state == 18,
        _ => true,
    };
    if !transition_matches {
        return WeaponShotReadiness {
            owner,
            ready: false,
        };
    }

    let ready = [0x21, 0x14, 0x22]
        .into_iter()
        .all(|transition| !source.transition_active(actor, transition));
    WeaponShotReadiness { owner, ready }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotAdmission {
    NoEligibleAmmo,
    Selected(ShotSelection),
}

/// Side effects used by the native 0x1C0200 shot-start coordinator. Implementations mutate
/// Rust-owned weapon/presentation state; guest-memory mirroring belongs only at the hybrid edge.
/// `ProjectileResult` is intentionally opaque: the original wrapper does not branch on it, and a
/// null descriptor can mean reuse rather than allocation failure.
pub trait ShotStartActions {
    type ProjectileResult;

    fn reset_shot_record(&mut self);
    fn apply_recoil_and_weapon_state(&mut self);
    fn handle_weapon_one_special(&mut self);
    fn prepare_shot_effects(&mut self);
    fn spawn_projectile_loop(&mut self, owner: WeaponProjectileOwner) -> Self::ProjectileResult;
    fn advance_refire_state(&mut self);
    fn take_shot_ammo(&mut self);
    fn commit_fired_tail(&mut self, weapon: WeaponRecordId);
}

/// Result of the bounded native shot-start ordering. The ordinary path retains the projectile
/// return for diagnostics but always reaches ammo and fired-tail commits, matching the measured
/// caller order. This does not assert what a returned null descriptor means inside the allocator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShotStartOutcome<P> {
    ReadinessRejected,
    WeaponOneEarlyReturn,
    Ordinary { projectile_result: P },
}

/// Run the typed 0x1C0200 sequence after the caller evaluates readiness. Readiness rejection has
/// no shot-record writes. Weapon 0x1B follows its measured early path after record/state updates;
/// ordinary shots perform presentation and projectile work before ammo, without rollback on a
/// null/opaque projectile result. The A/B host adapter decides whether to mirror resulting state.
pub fn run_shot_start<A: ShotStartActions>(
    readiness: WeaponShotReadiness,
    weapon: WeaponRecordId,
    actions: &mut A,
) -> ShotStartOutcome<A::ProjectileResult> {
    if !readiness.is_ready() {
        return ShotStartOutcome::ReadinessRejected;
    }

    actions.reset_shot_record();
    actions.apply_recoil_and_weapon_state();
    if weapon.0 == 0x1b {
        actions.handle_weapon_one_special();
        actions.commit_fired_tail(weapon);
        return ShotStartOutcome::WeaponOneEarlyReturn;
    }

    actions.prepare_shot_effects();
    let projectile_result = actions.spawn_projectile_loop(readiness.owner());
    actions.advance_refire_state();
    actions.take_shot_ammo();
    actions.commit_fired_tail(weapon);
    ShotStartOutcome::Ordinary { projectile_result }
}

/// 0x1BC650's bounded inventory edge. The first owned entry with a record, resource bit 0x400,
/// and positive signed reserve wins. Its reserve is decremented before the actor emission gates.
/// Game type 10 selects record 22 without scanning or consuming reserve. Matrix construction and
/// the shared 0x1B7680 projectile/reuse contract remain neighbouring responsibilities.
pub fn admit_weapon_shot(
    inventory: &mut Inventory,
    sources: &[ShotAmmoSource; ENTRY_COUNT],
    game_type: u8,
    actor_flags: u32,
    actor_low16: u16,
) -> ShotAdmission {
    let selected = if game_type == 10 {
        ShotSelection {
            weapon: WeaponRecordId(22),
            ammo_type: None,
            reserve_consumed: false,
            emission_allowed: false,
        }
    } else {
        let mut found = None;
        for (entry, source) in sources.iter().enumerate() {
            if inventory.owned[entry] == 0 || source.resource_flags & 0x400 == 0 {
                continue;
            }
            let Some(weapon) = source.weapon else {
                continue;
            };
            let Some(ammo) = source.ammo_type else {
                continue;
            };
            let reserve = inventory.ammo[ammo.index()] as i16;
            if reserve <= 0 {
                continue;
            }
            let after = reserve.wrapping_sub(1);
            inventory.ammo[ammo.index()] = if after < 0 { 0 } else { after as u16 };
            found = Some(ShotSelection {
                weapon,
                ammo_type: Some(ammo),
                reserve_consumed: true,
                emission_allowed: false,
            });
            break;
        }
        let Some(found) = found else {
            return ShotAdmission::NoEligibleAmmo;
        };
        found
    };
    ShotAdmission::Selected(ShotSelection {
        emission_allowed: actor_flags & 0x100 == 0 && actor_low16 == 0,
        ..selected
    })
}

#[cfg(test)]
mod shot_start_order_tests {
    use super::{
        ShotStartActions, ShotStartOutcome, WeaponOwnerReadinessState, WeaponProjectileOwner,
        WeaponReadinessSource, WeaponShotReadiness, evaluate_shot_readiness, run_shot_start,
    };
    use crate::object_pool::ObjectId;
    use crate::weapon_hands::WeaponRecordId;
    use crate::weapon_inventory::{WeaponInventory, WeaponInventoryStore, WeaponPlayerId};
    use alloc::vec::Vec;

    #[derive(Default)]
    struct Actions {
        calls: Vec<&'static str>,
        projectile_result: Option<u32>,
        projectile_owner: Option<WeaponProjectileOwner>,
    }

    struct Readiness {
        state: Option<WeaponOwnerReadinessState>,
        blocked: [u32; 3],
        actors: Vec<ObjectId>,
        transitions: Vec<u32>,
    }

    impl WeaponReadinessSource for Readiness {
        fn owner_state(&mut self, actor: ObjectId) -> Option<WeaponOwnerReadinessState> {
            self.actors.push(actor);
            self.state
        }

        fn transition_active(&mut self, actor: ObjectId, transition: u32) -> bool {
            self.actors.push(actor);
            self.transitions.push(transition);
            self.blocked.contains(&transition)
        }
    }

    impl ShotStartActions for Actions {
        type ProjectileResult = Option<u32>;

        fn reset_shot_record(&mut self) {
            self.calls.push("record-reset");
        }
        fn apply_recoil_and_weapon_state(&mut self) {
            self.calls.push("recoil-state");
        }
        fn handle_weapon_one_special(&mut self) {
            self.calls.push("weapon-one-special");
        }
        fn prepare_shot_effects(&mut self) {
            self.calls.push("effects");
        }
        fn spawn_projectile_loop(
            &mut self,
            owner: WeaponProjectileOwner,
        ) -> Self::ProjectileResult {
            self.calls.push("projectiles");
            self.projectile_owner = Some(owner);
            self.projectile_result
        }
        fn advance_refire_state(&mut self) {
            self.calls.push("refire");
        }
        fn take_shot_ammo(&mut self) {
            self.calls.push("ammo");
        }
        fn commit_fired_tail(&mut self, _weapon: WeaponRecordId) {
            self.calls.push("fired-tail");
        }
    }

    fn ready_owner() -> WeaponProjectileOwner {
        let mut players = WeaponInventoryStore::<1>::default();
        let player = players
            .begin_lifetime(WeaponPlayerId::new(0), WeaponInventory::default())
            .unwrap();
        WeaponProjectileOwner::new(
            ObjectId {
                slot: 7,
                generation: 12,
            },
            Some(player),
        )
    }

    fn ready(owner: WeaponProjectileOwner) -> WeaponShotReadiness {
        let mut source = Readiness {
            state: Some(WeaponOwnerReadinessState {
                state: 2,
                transition_state: 0,
            }),
            blocked: [0; 3],
            actors: Vec::new(),
            transitions: Vec::new(),
        };
        evaluate_shot_readiness(owner, &mut source)
    }

    #[test]
    fn readiness_rejection_leaves_shot_state_untouched() {
        let mut source = Readiness {
            state: None,
            blocked: [0; 3],
            actors: Vec::new(),
            transitions: Vec::new(),
        };
        let readiness = evaluate_shot_readiness(ready_owner(), &mut source);
        let mut actions = Actions::default();
        assert_eq!(
            run_shot_start(readiness, WeaponRecordId(3), &mut actions),
            ShotStartOutcome::ReadinessRejected
        );
        assert!(actions.calls.is_empty());
        assert!(actions.projectile_owner.is_none());
    }

    #[test]
    fn ordinary_path_commits_ammo_after_projectiles_even_for_null_result() {
        let mut actions = Actions {
            projectile_result: None,
            ..Actions::default()
        };
        let owner = ready_owner();
        assert_eq!(
            run_shot_start(ready(owner), WeaponRecordId(3), &mut actions),
            ShotStartOutcome::Ordinary {
                projectile_result: None,
            }
        );
        assert_eq!(actions.projectile_owner, Some(owner));
        assert_eq!(
            actions.calls,
            [
                "record-reset",
                "recoil-state",
                "effects",
                "projectiles",
                "refire",
                "ammo",
                "fired-tail",
            ]
        );
    }

    #[test]
    fn weapon_one_takes_its_early_return_after_record_updates() {
        let mut actions = Actions::default();
        assert_eq!(
            run_shot_start(ready(ready_owner()), WeaponRecordId(0x1b), &mut actions),
            ShotStartOutcome::WeaponOneEarlyReturn
        );
        assert_eq!(
            actions.calls,
            [
                "record-reset",
                "recoil-state",
                "weapon-one-special",
                "fired-tail"
            ]
        );
    }

    #[test]
    fn readiness_queries_exact_actor_in_order_and_short_circuits() {
        let owner = ready_owner();
        let actor = owner.actor();
        let mut source = Readiness {
            state: Some(WeaponOwnerReadinessState {
                state: 9,
                transition_state: 8,
            }),
            blocked: [0; 3],
            actors: Vec::new(),
            transitions: Vec::new(),
        };
        assert!(!evaluate_shot_readiness(owner, &mut source).is_ready());
        assert_eq!(source.actors, [actor]);
        assert!(source.transitions.is_empty());

        source.state = Some(WeaponOwnerReadinessState {
            state: 4,
            transition_state: 0,
        });
        source.actors.clear();
        source.blocked[0] = 0x14;
        assert!(!evaluate_shot_readiness(owner, &mut source).is_ready());
        assert_eq!(source.transitions, [0x21, 0x14]);
        assert!(source.actors.iter().all(|queried| *queried == actor));
    }

    #[test]
    fn readiness_uses_the_measured_state_pairs_and_queries_all_clear_transitions() {
        let owner = ready_owner();
        for (state, transition_state, expected) in [
            (9, 9, true),
            (14, 14, true),
            (19, 19, true),
            (18, 18, true),
            (9, 14, false),
            (14, 19, false),
            (19, 18, false),
            (18, 9, false),
            (2, 0, true),
        ] {
            let mut source = Readiness {
                state: Some(WeaponOwnerReadinessState {
                    state,
                    transition_state,
                }),
                blocked: [0; 3],
                actors: Vec::new(),
                transitions: Vec::new(),
            };
            assert_eq!(
                evaluate_shot_readiness(owner, &mut source).is_ready(),
                expected,
                "state={state} transition_state={transition_state}"
            );
            if expected {
                assert_eq!(source.transitions, [0x21, 0x14, 0x22]);
                assert!(
                    source
                        .actors
                        .iter()
                        .all(|queried| *queried == owner.actor())
                );
            } else {
                assert!(source.transitions.is_empty());
            }
        }
    }

    #[test]
    fn projectile_attribution_keeps_extension_and_actor_identities_distinct() {
        use super::WeaponProjectileAttribution;

        let owner = ready_owner();
        assert_eq!(owner.actor().slot, 7);
        assert_eq!(
            owner.attribution(),
            WeaponProjectileAttribution::PlayerExtension(owner.player_extension().unwrap())
        );
        let fallback = WeaponProjectileOwner::new(owner.actor(), None);
        assert_eq!(
            fallback.attribution(),
            WeaponProjectileAttribution::Actor(owner.actor())
        );
    }
}
