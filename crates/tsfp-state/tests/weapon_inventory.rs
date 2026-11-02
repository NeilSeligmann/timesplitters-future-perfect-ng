// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED typed state fixtures; these tests exercise the measured Rust contract, not a live route.
use tsfp_state::object_pool::ObjectId;
use tsfp_state::scene_membership::SceneMembership;
use tsfp_state::weapon_hands::{AmmoType, HandSide, WeaponAmmoRecord, WeaponRecordId};
use tsfp_state::weapon_inventory::{
    AMMO_TYPE_COUNT, AmmoAutoSelectionContext, AmmoPickupCallbackProgress, AmmoReservePolicy,
    ENTRY_COUNT, GrantSavedRounds, InventoryEntryDefinition, PickupAmmoPair,
    ProfileWeaponInventory, ReloadSessionAdvance, ReloadSessionState, WeaponInventory,
    WeaponInventoryStore, WeaponPickupAmmo, WeaponPlayerId, WeaponSubsystemState,
};
use tsfp_state::weapon_payload::{
    C1950PayloadInputs, C1950PayloadTemplate, PayloadRotationMode, ShotAdmission, ShotAmmoSource,
    ShotAmmoType, ShotSelection, WeaponPayloadLifecycleError, WeaponPayloadPose,
};

fn ammo(value: u8) -> AmmoType {
    AmmoType::new(value).unwrap()
}

fn record(id: u16, primary: u8, secondary: u8) -> WeaponAmmoRecord {
    WeaponAmmoRecord {
        id: WeaponRecordId(id),
        primary: Some(ammo(primary)),
        secondary: Some(ammo(secondary)),
        primary_capacity: 0,
        secondary_capacity: 0,
        primary_single_round: false,
    }
}

#[test]
fn ammo_reserve_policy_applies_only_the_measured_optional_frame_scale() {
    let fixed = AmmoReservePolicy::from_row_fields(240, 0x02);
    assert!(!fixed.is_frame_scaled());
    assert_eq!(fixed.maximum(3, 0.5), 240);

    let scaled = AmmoReservePolicy::from_row_fields(240, 0x01);
    assert!(scaled.is_frame_scaled());
    assert_eq!(scaled.maximum(3, 0.5), 360);
}

#[test]
fn owned_handoff_keeps_request_realisation_and_equipped_once_separate() {
    let mut state = WeaponInventory {
        desired_entry: Some(4),
        ..WeaponInventory::default()
    };
    state.hands.realised_entry = Some(3);

    assert!(!state.request_handoff(4, 5));
    assert!(state.request_handoff(7, 5));
    assert_eq!(state.desired_entry, Some(5));
    assert_eq!(state.hands.realised_entry, Some(4));
    assert!(
        state
            .hands
            .controllers
            .iter()
            .all(|controller| controller.state == 3 && controller.timer == 0)
    );
    assert!(!state.hands.equipped_once[5]);

    assert!(state.complete_handoff(5));
    assert_eq!(state.hands.realised_entry, Some(5));
    assert!(!state.hands.equipped_once[5]);
    assert!(state.mark_equipped_once(5));
    assert!(state.hands.equipped_once[5]);
    assert!(!state.complete_handoff(ENTRY_COUNT));
    assert!(!state.mark_equipped_once(ENTRY_COUNT));
}

#[test]
fn owned_handoff_replaces_pending_selection_and_restarts_controllers() {
    let mut state = WeaponInventory {
        desired_entry: Some(4),
        ..WeaponInventory::default()
    };

    assert!(state.request_handoff(7, 5));
    assert_eq!(state.desired_entry, Some(5));
    assert_eq!(state.hands.realised_entry, Some(4));

    // A second changed request arrives before the state-3 consumer completes.
    // The owner keeps one desired slot and restages all three measured controllers.
    state.hands.controllers[0].state = 1;
    state.hands.controllers[0].timer = 25;
    assert!(state.request_handoff(8, 9));

    assert_eq!(state.desired_entry, Some(9));
    assert_eq!(state.hands.realised_entry, Some(5));
    assert!(
        state
            .hands
            .controllers
            .iter()
            .all(|controller| controller.state == 3 && controller.timer == 0)
    );
}

#[test]
fn state3_holster_commits_ammo_and_desired_entry_only_for_valid_controller() {
    let active_record = record(3, 2, 4);
    let mut owner = WeaponInventory {
        desired_entry: Some(5),
        ..WeaponInventory::default()
    };
    owner.hands.realised_entry = Some(4);
    owner.hands.hands[0].record = Some(active_record.id);
    owner.hands.hands[0].loaded[2] = 7;
    owner.hands.hands[0].loaded[4] = 3;
    owner.hands.controllers[1].state = 2;
    owner.hands.controllers[1].hand = Some(HandSide::Right.index());
    let caps = [i16::MAX; 44];
    let before = owner.clone();

    assert!(!owner.complete_state3_holster(HandSide::Right, 1, active_record, &caps));
    assert_eq!(owner, before);
    assert!(!owner.complete_state3_holster(HandSide::Right, 3, active_record, &caps));
    assert_eq!(owner, before);

    owner.hands.controllers[1].state = 3;
    let mut wrong_hand = owner.clone();
    assert!(!wrong_hand.complete_state3_holster(HandSide::Left, 1, active_record, &caps));
    assert_eq!(wrong_hand, owner);
    assert!(owner.complete_state3_holster(HandSide::Right, 1, active_record, &caps));
    assert_eq!(owner.hands.realised_entry, Some(5));
    assert_eq!(owner.hands.hands[0].saved_rounds[2], 7);
    assert_eq!(owner.hands.hands[0].saved_rounds[4], 3);
    assert_eq!(owner.hands.hands[0].loaded[2], 0);
    assert_eq!(owner.hands.hands[0].loaded[4], 0);
    assert_eq!(owner.reserve[2], 7);
    assert_eq!(owner.reserve[4], 3);
    assert_eq!(owner.hands.controllers[1].state, 3);
}

#[test]
fn state3_holster_fails_closed_for_invalid_desired_entry() {
    let mut owner = WeaponInventory {
        desired_entry: Some(u8::MAX),
        ..WeaponInventory::default()
    };
    owner.hands.controllers[0].state = 3;
    owner.hands.hands[0].loaded[2] = 9;
    let before = owner.clone();

    assert!(!owner.complete_state3_holster(HandSide::Right, 0, record(3, 2, 4), &[i16::MAX; 44],));
    assert_eq!(owner, before);
}

#[test]
fn player_slot_store_owns_state_independently_and_retires_reused_slots() {
    let mut players = WeaponInventoryStore::<2>::default();
    let first = WeaponPlayerId::new(0);
    let second = WeaponPlayerId::new(1);

    players.get_or_default(first).unwrap().reserve[3] = 17;
    players.get_or_default(second).unwrap().reserve[3] = 29;
    assert_eq!(players.get(first).unwrap().reserve[3], 17);
    assert_eq!(players.get(second).unwrap().reserve[3], 29);

    let retired = players.retire(first).unwrap();
    assert_eq!(retired.reserve[3], 17);
    assert!(players.get(first).is_none());
    assert_eq!(players.get_or_default(first).unwrap().reserve[3], 0);
    assert_eq!(players.get(second).unwrap().reserve[3], 29);
    assert!(players.get(WeaponPlayerId::new(2)).is_none());
}

#[test]
fn profile_restore_copies_only_profile_owned_selection_and_ammo_for_live_generation() {
    let mut inventory = WeaponInventory {
        desired_entry: Some(2),
        default_throwable: Some(9),
        collected_latch: true,
        ..WeaponInventory::default()
    };
    inventory.hands.hands[HandSide::Right.index()].record = Some(WeaponRecordId(17));
    inventory.hands.hands[HandSide::Right.index()].saved_rounds[4] = 6;
    inventory.hands.hands[HandSide::Right.index()].reload_timer = 21;
    inventory.hands.hands[HandSide::Left.index()].record = Some(WeaponRecordId(23));
    inventory.hands.hands[HandSide::Left.index()].saved_rounds[5] = 8;
    inventory.hands.realised_entry = Some(1);
    inventory.hands.equipped_once[2] = true;
    inventory.hands.controllers[1].state = 3;
    inventory.hands.controllers[1].timer = 44;

    let mut owned = [false; ENTRY_COUNT];
    owned[7] = true;
    let mut reserve = [0; 44];
    reserve[11] = -3;
    let mut loaded_rounds = [[0; 44]; 2];
    loaded_rounds[HandSide::Right.index()][4] = 12;
    loaded_rounds[HandSide::Left.index()][5] = -2;
    let profile = ProfileWeaponInventory {
        desired_entry: Some(7),
        owned,
        reserve,
        loaded_rounds,
    };

    let mut players = WeaponInventoryStore::<1>::default();
    let player = WeaponPlayerId::new(0);
    let old = players.begin_lifetime(player, inventory).unwrap();
    assert!(players.restore_profile_for_lifetime(old, profile));
    let restored = players.get_handle(old).unwrap();
    assert_eq!(restored.desired_entry, Some(7));
    assert!(restored.owned[7]);
    assert!(!restored.owned[2]);
    assert_eq!(restored.reserve[11], -3);
    assert_eq!(restored.hands.hand(HandSide::Right).loaded[4], 12);
    assert_eq!(restored.hands.hand(HandSide::Left).loaded[5], -2);

    // Restore is a direct profile copy, not reload or hand reconstruction.
    assert_eq!(restored.default_throwable, Some(9));
    assert!(restored.collected_latch);
    assert_eq!(
        restored.hands.hand(HandSide::Right).record,
        Some(WeaponRecordId(17))
    );
    assert_eq!(
        restored.hands.hand(HandSide::Left).record,
        Some(WeaponRecordId(23))
    );
    assert_eq!(restored.hands.hand(HandSide::Right).saved_rounds[4], 6);
    assert_eq!(restored.hands.hand(HandSide::Right).reload_timer, 21);
    assert_eq!(restored.hands.realised_entry, Some(1));
    assert!(restored.hands.equipped_once[2]);
    assert_eq!(restored.hands.controllers[1].state, 3);
    assert_eq!(restored.hands.controllers[1].timer, 44);

    players.retire_handle(old).unwrap();
    let current = players
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();
    assert!(!players.restore_profile_for_lifetime(old, profile));
    assert_eq!(
        players.get_handle(current).unwrap(),
        &WeaponInventory::default()
    );
}

#[test]
fn subsystem_player_lifetime_can_start_from_typed_profile_values() {
    let mut owned = [false; ENTRY_COUNT];
    owned[7] = true;
    let mut reserve = [0; 44];
    reserve[11] = 13;
    let mut loaded_rounds = [[0; 44]; 2];
    loaded_rounds[HandSide::Right.index()][4] = 5;
    loaded_rounds[HandSide::Left.index()][5] = 6;
    let profile = ProfileWeaponInventory {
        desired_entry: Some(7),
        owned,
        reserve,
        loaded_rounds,
    };

    let mut subsystem = WeaponSubsystemState::<1>::default();
    let player = WeaponPlayerId::new(0);
    let handle = subsystem
        .begin_player_lifetime_from_profile(player, profile)
        .unwrap();
    let inventory = subsystem.players.get_handle(handle).unwrap();

    assert_eq!(inventory.desired_entry, Some(7));
    assert!(inventory.owned[7]);
    assert_eq!(inventory.reserve[11], 13);
    assert_eq!(inventory.hands.hand(HandSide::Right).loaded[4], 5);
    assert_eq!(inventory.hands.hand(HandSide::Left).loaded[5], 6);
    assert_eq!(inventory.hands.hand(HandSide::Right).record, None);
    assert_eq!(inventory.hands.hand(HandSide::Left).record, None);
    assert_eq!(inventory.hands.hand(HandSide::Right).saved_rounds[4], 0);
    assert!(
        subsystem
            .begin_player_lifetime_from_profile(player, profile)
            .is_none()
    );
}

#[test]
fn subsystem_profile_restore_replaces_the_previous_player_generation() {
    let mut subsystem = WeaponSubsystemState::<1>::default();
    let player = WeaponPlayerId::new(0);
    let previous = subsystem
        .begin_player_lifetime(player, WeaponInventory::default())
        .unwrap();
    let mut owned = [false; ENTRY_COUNT];
    owned[4] = true;
    let profile = ProfileWeaponInventory {
        desired_entry: Some(4),
        owned,
        reserve: [0; AMMO_TYPE_COUNT],
        loaded_rounds: [[0; AMMO_TYPE_COUNT]; 2],
    };

    let current = subsystem
        .replace_player_lifetime_from_profile(player, profile)
        .unwrap();

    assert_ne!(previous, current);
    assert!(subsystem.players.get_handle(previous).is_none());
    assert_eq!(
        subsystem.players.get_handle(current).unwrap().desired_entry,
        Some(4)
    );
    assert!(subsystem.retire_player_lifetime(previous).is_none());
}

#[test]
fn subsystem_owner_groups_player_inventory_and_generation_checked_payloads() {
    let mut subsystem = WeaponSubsystemState::<2>::with_object_capacity(2);
    let player = WeaponPlayerId::new(1);
    let handle = subsystem
        .players
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();
    subsystem.players.get_handle_mut(handle).unwrap().reserve[4] = 12;

    let object = ObjectId {
        slot: 1,
        generation: 7,
    };
    let pose = WeaponPayloadPose::from_definition([12.0, 24.0, -3.0], 45.0);
    subsystem.attach_payload(object, pose).unwrap();

    assert_eq!(subsystem.players.get_handle(handle).unwrap().reserve[4], 12);
    assert_eq!(subsystem.payload_pose(object).unwrap(), pose);
    assert_eq!(
        subsystem.payload_pose(ObjectId {
            generation: 8,
            ..object
        }),
        Err(WeaponPayloadLifecycleError::StaleObjectId)
    );
    assert_eq!(subsystem.retire_payload(object).unwrap(), pose);
    assert_eq!(
        subsystem.payload_pose(object),
        Err(WeaponPayloadLifecycleError::NotAttached)
    );
    assert!(subsystem.players.retire_handle(handle).is_some());
}

#[test]
fn c1950_creation_attaches_the_ordered_final_pose_atomically() {
    let mut subsystem = WeaponSubsystemState::<1>::with_object_capacity(2);
    let base_object = ObjectId {
        slot: 0,
        generation: 3,
    };
    let base_pose = subsystem
        .create_c1950_payload(
            base_object,
            C1950PayloadInputs {
                definition_angles_degrees: [-0.0, 32.0, 0.0],
                staged_yaw_degrees: 57.0,
                template: None,
            },
        )
        .unwrap();
    assert_eq!(base_pose.rotation_mode, PayloadRotationMode::Euler);
    assert_eq!(base_pose.pitch_yaw_roll_degrees, [-0.0, 32.0, 0.0]);
    assert_eq!(base_pose.staged_yaw_degrees, 57.0);
    assert_eq!(subsystem.payload_pose(base_object), Ok(base_pose));

    let templated_object = ObjectId {
        slot: 1,
        generation: 4,
    };
    let templated_pose = subsystem
        .create_c1950_payload(
            templated_object,
            C1950PayloadInputs {
                definition_angles_degrees: [0.0, 11.0, 0.0],
                staged_yaw_degrees: 73.0,
                template: Some(C1950PayloadTemplate {
                    flags: 0x2,
                    angles_degrees: [8.0, -19.0, 27.0],
                }),
            },
        )
        .unwrap();
    assert_eq!(templated_pose.rotation_mode, PayloadRotationMode::Euler);
    assert_eq!(templated_pose.pitch_yaw_roll_degrees, [8.0, -19.0, 27.0]);
    assert_eq!(templated_pose.staged_yaw_degrees, 73.0);

    let replacement_generation = ObjectId {
        generation: 5,
        ..templated_object
    };
    assert_eq!(
        subsystem.create_c1950_payload(
            replacement_generation,
            C1950PayloadInputs {
                definition_angles_degrees: [1.0, 2.0, 3.0],
                staged_yaw_degrees: 4.0,
                template: None,
            },
        ),
        Err(WeaponPayloadLifecycleError::SlotOccupied)
    );
    assert_eq!(subsystem.payload_pose(templated_object), Ok(templated_pose));
}

#[test]
fn stale_player_handle_cannot_mutate_or_retire_a_reused_slot() {
    let mut players = WeaponInventoryStore::<1>::default();
    let player = WeaponPlayerId::new(0);
    let first = players
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();
    players.get_handle_mut(first).unwrap().reserve[3] = 17;
    assert_eq!(players.retire_handle(first).unwrap().reserve[3], 17);

    let mut replacement_inventory = WeaponInventory::default();
    replacement_inventory.reserve[3] = 29;
    let replacement = players
        .begin_lifetime(player, replacement_inventory)
        .unwrap();

    assert_ne!(replacement.generation(), first.generation());
    assert!(players.get_handle(first).is_none());
    assert!(players.retire_handle(first).is_none());
    assert_eq!(players.get_handle(replacement).unwrap().reserve[3], 29);
}

#[test]
fn native_equipped_once_commit_requires_the_current_player_lifetime() {
    let mut subsystem = WeaponSubsystemState::<1>::default();
    let player = WeaponPlayerId::new(0);
    let first = subsystem
        .players
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();

    assert!(subsystem.mark_equipped_once_for_lifetime(first, 4));
    assert!(!subsystem.mark_equipped_once_for_lifetime(first, ENTRY_COUNT));
    assert!(
        subsystem
            .players
            .get_handle(first)
            .unwrap()
            .hands
            .equipped_once[4]
    );

    subsystem.players.retire_handle(first).unwrap();
    let replacement = subsystem
        .players
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();
    assert!(!subsystem.mark_equipped_once_for_lifetime(first, 5));
    assert!(
        !subsystem
            .players
            .get_handle(replacement)
            .unwrap()
            .hands
            .equipped_once[5]
    );
}

#[test]
fn owned_shot_consumes_before_actor_gate_and_game_type_ten_bypasses_reserve() {
    let ammo_type = ShotAmmoType::new(7).unwrap();
    let mut sources = [ShotAmmoSource::default(); ENTRY_COUNT];
    sources[3] = ShotAmmoSource {
        weapon: Some(WeaponRecordId(77)),
        resource_flags: 0x400,
        ammo_type: Some(ammo_type),
    };
    let mut state = WeaponInventory::default();
    state.owned[3] = true;
    state.reserve[7] = 1;

    assert_eq!(
        state.admit_shot(&sources, 0, 0x100, 0),
        ShotAdmission::Selected(ShotSelection {
            weapon: WeaponRecordId(77),
            ammo_type: Some(ammo_type),
            reserve_consumed: true,
            emission_allowed: false,
        })
    );
    assert_eq!(state.reserve[7], 0);

    state.reserve[7] = 5;
    assert_eq!(
        state.admit_shot(&sources, 10, 0, 0),
        ShotAdmission::Selected(ShotSelection {
            weapon: WeaponRecordId(22),
            ammo_type: None,
            reserve_consumed: false,
            emission_allowed: true,
        })
    );
    assert_eq!(state.reserve[7], 5);
}

#[test]
fn pickup_pairs_apply_in_order_and_skip_invalid_values() {
    let mut state = WeaponInventory::default();
    let mut maximums = [i16::MAX; 44];
    maximums[1] = 7;
    let outcomes = state.collect_ammo_pickup(
        [
            PickupAmmoPair {
                ammo_type: 1,
                amount: 4,
            },
            PickupAmmoPair {
                ammo_type: 1,
                amount: 4,
            },
        ],
        [None; 2],
        &maximums,
        false,
    );
    assert_eq!(outcomes[0].unwrap().reserve_delta, 4);
    assert_eq!(outcomes[1].unwrap().reserve_delta, 3);
    assert_eq!(state.reserve[1], 7);

    let skipped = state.collect_ammo_pickup(
        [
            PickupAmmoPair {
                ammo_type: 0,
                amount: 5,
            },
            PickupAmmoPair {
                ammo_type: 44,
                amount: 5,
            },
        ],
        [None; 2],
        &maximums,
        true,
    );
    assert_eq!(skipped, [None; 2]);
}

#[test]
fn ordinary_ammo_pickup_full_capacity_returns_before_consumption() {
    let definitions = [None; ENTRY_COUNT];
    let mut full = WeaponInventory::default();
    full.reserve[1] = 4;
    let mut maximums = [i16::MAX; AMMO_TYPE_COUNT];
    // The one-round amount is fully clamped against the existing reserve.
    maximums[1] = 4;
    let full_capacity = full.collect_ammo_pickup_acquire_and_select(
        [
            PickupAmmoPair {
                ammo_type: 1,
                amount: 1,
            },
            PickupAmmoPair::default(),
        ],
        [None; 2],
        &maximums,
        false,
        &definitions,
        [None; 2],
    );
    assert_eq!(
        full_capacity.callback_progress(false),
        AmmoPickupCallbackProgress::ReturnZeroWithoutConsumption
    );
    assert_eq!(
        full_capacity.callback_progress(true),
        AmmoPickupCallbackProgress::ContinueFlaggedAcquisition
    );

    let mut room = WeaponInventory::default();
    maximums[1] = 10;
    let reserve_changed = room.collect_ammo_pickup_acquire_and_select(
        [
            PickupAmmoPair {
                ammo_type: 1,
                amount: 1,
            },
            PickupAmmoPair::default(),
        ],
        [None; 2],
        &maximums,
        false,
        &definitions,
        [None; 2],
    );
    assert_eq!(
        reserve_changed.callback_progress(false),
        AmmoPickupCallbackProgress::ContinueOrdinaryAcquisition
    );
}

#[test]
fn pickup_pair_validates_signed_source_amount_before_short_narrowing() {
    let mut state = WeaponInventory::default();
    let maximums = [i16::MAX; 44];

    let skipped = state.collect_ammo_pickup(
        [
            PickupAmmoPair {
                ammo_type: 4,
                amount: 0x8000_0000,
            },
            PickupAmmoPair {
                ammo_type: 4,
                amount: u32::MAX,
            },
        ],
        [None; 2],
        &maximums,
        true,
    );
    assert_eq!(skipped, [None; 2]);
    assert_eq!(state.reserve[4], 0);

    let narrowed = state.collect_ammo_pickup(
        [
            PickupAmmoPair {
                ammo_type: 4,
                amount: 0x8000,
            },
            PickupAmmoPair::default(),
        ],
        [None; 2],
        &maximums,
        false,
    );
    let mutation = narrowed[0].unwrap();
    assert_eq!(mutation.reserve_delta, i16::MIN as i32);
    assert_eq!(state.reserve[4], i16::MIN);
}

#[test]
fn pickup_clamps_reserve_and_requests_acquisition_when_the_reserve_changes() {
    let mut state = WeaponInventory::default();
    state.reserve[3] = 7;
    let current = record(12, 3, 3);
    state.hands.hand_mut(HandSide::Right).record = Some(current.id);
    state.hands.hand_mut(HandSide::Right).loaded[3] = 4;
    state.hands.hand_mut(HandSide::Left).record = Some(WeaponRecordId(99));
    state.hands.hand_mut(HandSide::Left).loaded[3] = 100;
    let mut maximums = [i16::MAX; 44];
    maximums[3] = 10;

    let outcome = state.collect_ammo_pickup(
        [
            PickupAmmoPair {
                ammo_type: 3,
                amount: 5,
            },
            PickupAmmoPair::default(),
        ],
        [Some(current), None],
        &maximums,
        false,
    )[0]
    .unwrap();

    assert_eq!(outcome.loaded_at_pickup, 4);
    assert_eq!(outcome.reserve_before, 7);
    assert_eq!(outcome.reserve_after, 6);
    assert_eq!(outcome.reserve_delta, -1);
    assert!(outcome.acquisition_check_requested);
}

#[test]
fn loaded_ammo_counts_a_matching_hand_once_for_duplicate_record_channels() {
    let mut state = WeaponInventory::default();
    let right = record(1, 5, 5);
    let left = record(2, 5, 6);
    state.hands.hand_mut(HandSide::Right).record = Some(right.id);
    state.hands.hand_mut(HandSide::Right).loaded[5] = 3;
    state.hands.hand_mut(HandSide::Left).record = Some(left.id);
    state.hands.hand_mut(HandSide::Left).loaded[5] = 4;
    let mut maximums = [i16::MAX; 44];
    maximums[5] = 10;

    let result = state.collect_ammo_pickup(
        [
            PickupAmmoPair {
                ammo_type: 5,
                amount: 8,
            },
            PickupAmmoPair::default(),
        ],
        [Some(right), Some(left)],
        &maximums,
        false,
    )[0]
    .unwrap();

    assert_eq!(result.loaded_at_pickup, 7);
    assert_eq!(result.reserve_after, 3);
}

#[test]
fn pickup_narrows_amount_to_signed_word_before_capacity_check() {
    let mut state = WeaponInventory::default();
    state.reserve[1] = i16::MAX;
    let mut maximums = [i16::MAX; 44];
    maximums[1] = i16::MAX;
    let outcome = state.collect_ammo_pickup(
        [
            PickupAmmoPair {
                ammo_type: 1,
                amount: 65_537,
            },
            PickupAmmoPair::default(),
        ],
        [None; 2],
        &maximums,
        false,
    )[0]
    .unwrap();
    assert_eq!(outcome.reserve_after, i16::MIN);
    assert_eq!(outcome.reserve_delta, i16::MIN as i32 - i16::MAX as i32);
}

#[test]
fn pickup_flag_requests_acquisition_even_when_reserve_does_not_change() {
    let mut state = WeaponInventory::default();
    let mut maximums = [i16::MAX; 44];
    maximums[1] = 0;
    let outcome = state.collect_ammo_pickup(
        [
            PickupAmmoPair {
                ammo_type: 1,
                amount: 1,
            },
            PickupAmmoPair::default(),
        ],
        [None; 2],
        &maximums,
        true,
    )[0]
    .unwrap();
    assert_eq!(outcome.reserve_delta, 0);
    assert!(outcome.acquisition_check_requested);
}

#[test]
fn owned_inventory_composes_reload_saved_rounds_and_holster_state() {
    let mut state = WeaponInventory::default();
    let weapon = WeaponAmmoRecord {
        id: WeaponRecordId(4),
        primary: Some(ammo(1)),
        secondary: None,
        primary_capacity: 5,
        secondary_capacity: 0,
        primary_single_round: false,
    };
    state.reserve[1] = 8;
    let hand = state.hands.hand_mut(HandSide::Right);
    hand.record = Some(weapon.id);
    hand.loaded[1] = 1;
    hand.saved_rounds[1] = 3;

    assert_eq!(state.reloadable_count(HandSide::Right, weapon), 4);
    assert_eq!(
        state.transfer_reload(HandSide::Right, weapon, true, false),
        3
    );
    assert_eq!(state.reserve[1], 5);
    assert_eq!(state.hands.hand(HandSide::Right).loaded[1], 4);

    let mut maximums = [i16::MAX; 44];
    maximums[1] = 6;
    state.holster(HandSide::Right, weapon, &maximums, 12);
    assert_eq!(state.reserve[1], 6);
    assert_eq!(state.hands.hand(HandSide::Right).loaded[1], 0);
    assert_eq!(state.hands.hand(HandSide::Right).saved_rounds[1], 4);
    assert_eq!(state.hands.realised_entry, Some(12));

    assert_eq!(state.restore_saved(HandSide::Right, weapon, false), 4);
    assert_eq!(state.reserve[1], 2);
    assert_eq!(state.hands.hand(HandSide::Right).loaded[1], 4);
}

#[test]
fn reload_session_cancel_before_transfer_preserves_state() {
    let player = WeaponPlayerId::new(0);
    let mut store = WeaponInventoryStore::<1>::default();
    let handle = store
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();
    let record = WeaponAmmoRecord {
        id: WeaponRecordId(8),
        primary: Some(ammo(1)),
        secondary: Some(ammo(2)),
        primary_capacity: 5,
        secondary_capacity: 4,
        primary_single_round: false,
    };
    {
        let owner = store.get_handle_mut(handle).unwrap();
        owner.reserve[1] = 8;
        owner.reserve[2] = 6;
        let hand = owner.hands.hand_mut(HandSide::Right);
        hand.loaded[1] = 1;
        hand.loaded[2] = 0;
    }
    let mut session = store
        .begin_reload_session(handle, HandSide::Right, 0, false, false)
        .unwrap();

    assert_eq!(
        store.cancel_reload_session(handle, &mut session),
        Some(ReloadSessionAdvance::Terminal {
            state: ReloadSessionState::Cancelled,
            total_moved: 0,
        })
    );
    assert_eq!(
        store.advance_reload_session(handle, &mut session, record),
        Some(ReloadSessionAdvance::Terminal {
            state: ReloadSessionState::Cancelled,
            total_moved: 0,
        })
    );
    let owner = store.get_handle(handle).unwrap();
    assert_eq!(owner.reserve[1], 8);
    assert_eq!(owner.hands.hand(HandSide::Right).loaded[1], 1);
    assert_eq!(owner.reserve[2], 6);
    assert_eq!(owner.hands.hand(HandSide::Right).loaded[2], 0);
}

#[test]
fn reload_session_uses_active_record_and_completes_both_channels_synchronously() {
    let player = WeaponPlayerId::new(0);
    let mut store = WeaponInventoryStore::<1>::default();
    let handle = store
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();
    let record = WeaponAmmoRecord {
        id: WeaponRecordId(9),
        primary: Some(ammo(1)),
        secondary: Some(ammo(2)),
        primary_capacity: 3,
        secondary_capacity: 2,
        primary_single_round: false,
    };
    {
        let owner = store.get_handle_mut(handle).unwrap();
        owner.reserve[1] = 5;
        owner.reserve[2] = 4;
        owner.hands.controllers[1].flags = 7;
        owner.hands.controllers[1].attachment_target = Some(42);
        owner.hands.controllers[1].timer = 6;
        owner.hands.hand_mut(HandSide::Left).record = Some(WeaponRecordId(8));
    }
    let mut session = store
        .begin_reload_session(handle, HandSide::Left, 1, false, false)
        .unwrap();
    store
        .get_handle_mut(handle)
        .unwrap()
        .hands
        .hand_mut(HandSide::Left)
        .record = Some(record.id);
    assert!(matches!(
        store.advance_reload_session(handle, &mut session, record),
        Some(ReloadSessionAdvance::TransferProcessed {
            state: ReloadSessionState::Complete,
            total_moved: 5,
            ..
        })
    ));
    assert!(matches!(
        store.advance_reload_session(handle, &mut session, record),
        Some(ReloadSessionAdvance::Terminal {
            state: ReloadSessionState::Complete,
            total_moved: 5,
        })
    ));
    let owner = store.get_handle(handle).unwrap();
    assert_eq!(owner.reserve[1], 2);
    assert_eq!(owner.reserve[2], 2);
    assert_eq!(owner.hands.hand(HandSide::Left).loaded[1], 3);
    assert_eq!(owner.hands.hand(HandSide::Left).loaded[2], 2);
    assert_eq!(owner.hands.controllers[1].flags, 0);
    assert_eq!(owner.hands.controllers[1].attachment_target, None);
    assert_eq!(owner.hands.controllers[1].timer, 1000);
}

#[test]
fn stale_lifetime_cannot_advance_its_reload_session_on_a_reused_slot() {
    let player = WeaponPlayerId::new(0);
    let mut store = WeaponInventoryStore::<1>::default();
    let stale = store
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();
    let record = WeaponAmmoRecord {
        id: WeaponRecordId(10),
        primary: Some(ammo(1)),
        secondary: None,
        primary_capacity: 5,
        secondary_capacity: 0,
        primary_single_round: false,
    };
    let mut session = store
        .begin_reload_session(stale, HandSide::Left, 0, false, false)
        .unwrap();
    assert!(store.retire_handle(stale).is_some());
    let replacement = store
        .begin_lifetime(player, WeaponInventory::default())
        .unwrap();
    assert!(
        store
            .advance_reload_session(stale, &mut session, record)
            .is_none()
    );
    assert_eq!(session.owner(), stale);
    assert_eq!(store.get_handle(replacement).unwrap().reserve[1], 0);
    assert_eq!(session.state(), ReloadSessionState::Pending);
}

#[test]
fn x1_skips_dual_left_members_and_x2_grant_owns_the_group() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[4] = Some(InventoryEntryDefinition {
        group: 2,
        flags: 0,
        record_present: true,
        primary_ammo: Some(ammo(1)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    definitions[5] = Some(InventoryEntryDefinition {
        group: 2,
        flags: 0x08,
        record_present: true,
        primary_ammo: Some(ammo(2)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    definitions[6] = Some(InventoryEntryDefinition {
        group: 2,
        flags: 0x0c,
        record_present: true,
        primary_ammo: Some(ammo(3)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    definitions[7] = Some(InventoryEntryDefinition {
        group: 2,
        flags: 0,
        record_present: true,
        primary_ammo: Some(ammo(4)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });

    let mut state = WeaponInventory::default();
    let grant = state.grant_entry(&definitions, 4);
    assert!(grant.acquired);
    assert!(state.owned[4]);
    assert!(state.owned[5]);
    assert!(!state.owned[6]);
    assert!(!state.owned[7]);

    let grant = state.grant_entry(&definitions, 6);
    assert!(grant.acquired);
    assert!(state.owned[6]);
    assert!(state.owned[5]);
    assert!(state.owned[7]);
}

#[test]
fn ammo_acquisition_scans_matching_entries_and_reports_group_grants() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x10,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    definitions[9] = Some(InventoryEntryDefinition {
        group: 6,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: Some(ammo(10)),
        grant_saved_rounds: None,
    });
    definitions[10] = Some(InventoryEntryDefinition {
        group: 6,
        flags: 0x08,
        record_present: true,
        primary_ammo: Some(ammo(11)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    definitions[11] = Some(InventoryEntryDefinition {
        group: 7,
        flags: 0x08,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    definitions[12] = Some(InventoryEntryDefinition {
        group: 8,
        flags: 0x010,
        record_present: false,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });

    let mut state = WeaponInventory::default();
    let result = state.acquire_matching_ammo(&definitions, ammo(9));
    assert!(result.acquired);
    assert!(result.newly_owned[8]); // 0x10 is one of the two eligibility bits in mask 0x210
    assert!(result.newly_owned[9]);
    assert!(result.newly_owned[10]);
    assert!(!result.newly_owned[11]); // outside the 0x210 eligibility mask
    assert!(!result.newly_owned[12]); // no main weapon record
    assert!(!state.owned[1]); // entry zero is never scanned
    assert!(state.collected_latch); // 0x1BFCA0 ORs mask 8 after each delegated grant
}

#[test]
fn collection_delegate_sets_latch_when_entry_was_already_owned() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });

    let mut state = WeaponInventory::default();
    state.owned[8] = true;
    let result = state.collect_entry(&definitions, 8);

    assert!(!result.acquired);
    assert!(!result.newly_owned[8]);
    assert!(state.collected_latch);
}

#[test]
fn first_collection_initializes_both_owned_hand_saved_round_pairs_once() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: Some(ammo(10)),
        grant_saved_rounds: Some(GrantSavedRounds {
            primary: 6,
            secondary: 2,
        }),
    });

    let mut state = WeaponInventory::default();
    let result = state.collect_entry(&definitions, 8);
    assert!(result.acquired);
    assert!(!result.saved_rounds_unresolved);
    for side in [HandSide::Left, HandSide::Right] {
        let hand = state.hands.hand(side);
        assert_eq!(hand.saved_rounds[9], 6);
        assert_eq!(hand.saved_rounds[10], 2);
    }

    state.hands.hand_mut(HandSide::Left).saved_rounds[9] = 4;
    state.hands.hand_mut(HandSide::Right).saved_rounds[10] = 1;
    let repeated = state.collect_entry(&definitions, 8);
    assert!(!repeated.acquired);
    assert!(!repeated.saved_rounds_unresolved);
    assert_eq!(state.hands.hand(HandSide::Left).saved_rounds[9], 4);
    assert_eq!(state.hands.hand(HandSide::Right).saved_rounds[10], 1);
}

#[test]
fn first_collection_reports_missing_saved_round_record_facts() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });

    let mut state = WeaponInventory::default();
    let result = state.collect_entry(&definitions, 8);
    assert!(result.acquired);
    assert!(result.saved_rounds_unresolved);
}

#[test]
fn collect_and_select_keeps_desired_publication_separate_from_hand_realisation() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: Some(ammo(10)),
        grant_saved_rounds: Some(GrantSavedRounds {
            primary: 6,
            secondary: 2,
        }),
    });

    let mut state = WeaponInventory {
        desired_entry: Some(3),
        ..WeaponInventory::default()
    };
    state.hands.realised_entry = Some(2);
    let controllers_before = state.hands.controllers;

    let outcome = state.collect_entry_and_select(&definitions, 8, true, Some(9));

    assert!(!outcome.previously_owned);
    assert!(outcome.acquisition.acquired);
    assert!(outcome.selection_requested);
    assert_eq!(outcome.selected_entry, Some(9));
    assert!(!outcome.selection_unresolved);
    assert!(state.collected_latch);
    assert!(state.owned[8]);
    assert_eq!(state.desired_entry, Some(9));
    assert_eq!(state.hands.realised_entry, Some(2));
    assert_eq!(state.hands.controllers, controllers_before);
    for side in [HandSide::Left, HandSide::Right] {
        let hand = state.hands.hand(side);
        assert_eq!(hand.saved_rounds[9], 6);
        assert_eq!(hand.saved_rounds[10], 2);
    }
}

#[test]
fn weapon_pickup_commits_ammo_before_duplicate_acceptance_and_grant() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: Some(GrantSavedRounds {
            primary: 6,
            secondary: 0,
        }),
    });

    let mut first_acquisition = WeaponInventory::default();
    let fresh =
        first_acquisition.collect_weapon_pickup(8, [None; 2], [None; 2], &definitions, false, None);
    assert!(fresh.inventory_accepted);
    assert!(fresh.grant_called);
    assert!(fresh.grant.unwrap().acquisition.acquired);
    assert!(first_acquisition.owned[8]);

    let mut duplicate = WeaponInventory::default();
    duplicate.owned[8] = true;
    duplicate.reserve[9] = 4;
    duplicate.reserve[10] = 10;
    duplicate.hands.realised_entry = Some(2);
    let accepted = duplicate.collect_weapon_pickup(
        8,
        [
            Some(WeaponPickupAmmo {
                ammo_type: ammo(9),
                amount: 3,
                reserve_maximum: 5,
            }),
            Some(WeaponPickupAmmo {
                ammo_type: ammo(10),
                amount: 1,
                reserve_maximum: 10,
            }),
        ],
        [None; 2],
        &definitions,
        true,
        Some(9),
    );
    assert!(accepted.previously_owned);
    assert!(accepted.inventory_accepted);
    assert!(accepted.reserve_increased);
    assert_eq!(accepted.ammo[0].unwrap().reserve_delta, 1);
    assert_eq!(accepted.ammo[1].unwrap().reserve_delta, 0);
    assert!(accepted.grant_called);
    assert!(!accepted.grant.unwrap().acquisition.acquired);
    assert!(duplicate.collected_latch);
    assert_eq!(duplicate.reserve[9], 5);
    assert_eq!(duplicate.reserve[10], 10);
    assert_eq!(duplicate.desired_entry, Some(9));
    assert_eq!(duplicate.hands.realised_entry, Some(2));

    let mut full_duplicate = WeaponInventory::default();
    full_duplicate.owned[8] = true;
    full_duplicate.reserve[9] = 5;
    let refused = full_duplicate.collect_weapon_pickup(
        8,
        [
            Some(WeaponPickupAmmo {
                ammo_type: ammo(9),
                amount: 1,
                reserve_maximum: 5,
            }),
            None,
        ],
        [None; 2],
        &definitions,
        false,
        None,
    );
    assert!(!refused.inventory_accepted);
    assert!(!refused.grant_called);
    assert_eq!(refused.ammo[0].unwrap().reserve_delta, 0);
    assert!(!full_duplicate.collected_latch);
    assert_eq!(full_duplicate.reserve[9], 5);
}

#[test]
fn weapon_pickup_repeated_ammo_pairs_observe_the_prior_reserve_commit() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });

    let mut state = WeaponInventory::default();
    state.owned[8] = true;
    state.reserve[9] = 2;
    let outcome = state.collect_weapon_pickup(
        8,
        [
            Some(WeaponPickupAmmo {
                ammo_type: ammo(9),
                amount: 3,
                reserve_maximum: 5,
            }),
            Some(WeaponPickupAmmo {
                ammo_type: ammo(9),
                amount: 2,
                reserve_maximum: 5,
            }),
        ],
        [None; 2],
        &definitions,
        false,
        None,
    );

    assert!(outcome.inventory_accepted);
    assert!(outcome.reserve_increased);
    assert_eq!(outcome.ammo[0].unwrap().reserve_before, 2);
    assert_eq!(outcome.ammo[0].unwrap().reserve_after, 5);
    assert_eq!(outcome.ammo[0].unwrap().reserve_delta, 3);
    assert_eq!(outcome.ammo[1].unwrap().reserve_before, 5);
    assert_eq!(outcome.ammo[1].unwrap().reserve_after, 5);
    assert_eq!(outcome.ammo[1].unwrap().reserve_delta, 0);
    assert!(outcome.grant_called);
    assert!(state.collected_latch);
    assert_eq!(state.reserve[9], 5);
}

#[test]
fn duplicate_weapon_pickup_uses_accumulated_delta_and_native_zero_gate() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition::default());
    let pairs = [
        Some(WeaponPickupAmmo {
            ammo_type: ammo(9),
            amount: 5,
            reserve_maximum: 100,
        }),
        Some(WeaponPickupAmmo {
            ammo_type: ammo(9),
            amount: -5,
            reserve_maximum: 100,
        }),
    ];

    let mut refused_state = WeaponInventory::default();
    refused_state.owned[8] = true;
    refused_state.reserve[9] = 20;
    let refused = refused_state.collect_weapon_pickup_with_duplicate_predicate(
        8,
        pairs,
        [None; 2],
        &definitions,
        false,
        false,
        None,
    );
    assert_eq!(refused.ammo[0].unwrap().reserve_delta, 5);
    assert_eq!(refused.ammo[1].unwrap().reserve_delta, -5);
    assert!(refused.reserve_increased);
    assert_eq!(refused.reserve_delta_total, 0);
    assert!(refused.duplicate_predicate_consulted);
    assert!(!refused.duplicate_predicate_passed);
    assert!(!refused.inventory_accepted);
    assert!(!refused.grant_called);
    assert_eq!(refused_state.reserve[9], 20);

    let mut accepted_state = WeaponInventory::default();
    accepted_state.owned[8] = true;
    accepted_state.reserve[9] = 20;
    let accepted = accepted_state.collect_weapon_pickup_with_duplicate_predicate(
        8,
        pairs,
        [None; 2],
        &definitions,
        true,
        false,
        None,
    );
    assert_eq!(accepted.reserve_delta_total, 0);
    assert!(accepted.duplicate_predicate_consulted);
    assert!(accepted.duplicate_predicate_passed);
    assert!(accepted.inventory_accepted);
    assert!(accepted.grant_called);
}

#[test]
fn subsystem_pickup_uses_live_player_generation_and_ordered_reserve_commits() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });

    let player = WeaponPlayerId::new(0);
    let mut inventory = WeaponInventory::default();
    inventory.owned[8] = true;
    inventory.reserve[9] = 2;
    let mut subsystem = WeaponSubsystemState::<1>::default();
    let old = subsystem.begin_player_lifetime(player, inventory).unwrap();

    let outcome = subsystem
        .collect_weapon_pickup_for_lifetime(
            old,
            8,
            [
                Some(WeaponPickupAmmo {
                    ammo_type: ammo(9),
                    amount: 3,
                    reserve_maximum: 5,
                }),
                Some(WeaponPickupAmmo {
                    ammo_type: ammo(9),
                    amount: 2,
                    reserve_maximum: 5,
                }),
            ],
            [None; 2],
            &definitions,
            false,
            false,
            None,
        )
        .unwrap();
    assert!(outcome.previously_owned);
    assert!(outcome.inventory_accepted);
    assert_eq!(outcome.ammo[0].unwrap().reserve_delta, 3);
    assert_eq!(outcome.ammo[1].unwrap().reserve_delta, 0);
    assert!(outcome.grant_called);
    assert!(!outcome.grant.unwrap().acquisition.acquired);
    assert_eq!(subsystem.players.get_handle(old).unwrap().reserve[9], 5);

    assert!(subsystem.retire_player_lifetime(old).is_some());
    let current = subsystem
        .begin_player_lifetime(player, WeaponInventory::default())
        .unwrap();
    assert!(
        subsystem
            .collect_weapon_pickup_for_lifetime(
                old,
                8,
                [None; 2],
                [None; 2],
                &definitions,
                false,
                false,
                None,
            )
            .is_none()
    );
    assert_eq!(
        subsystem.players.get_handle(current).unwrap(),
        &WeaponInventory::default()
    );
}

#[test]
fn world_pickup_requires_live_scene_and_player_generations() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });

    let player = WeaponPlayerId::new(0);
    let mut inventory = WeaponInventory::default();
    inventory.owned[8] = true;
    inventory.reserve[9] = 1;
    let mut subsystem = WeaponSubsystemState::<1>::default();
    let player = subsystem.begin_player_lifetime(player, inventory).unwrap();
    let mut scene = SceneMembership::default();
    let stale_object = scene.spawn(4).unwrap();
    scene.retire(stale_object).unwrap();
    let live_object = scene.spawn(4).unwrap();
    assert_ne!(stale_object.generation, live_object.generation);

    let ammo_pairs = [
        Some(WeaponPickupAmmo {
            ammo_type: ammo(9),
            amount: 4,
            reserve_maximum: 5,
        }),
        None,
    ];
    assert!(
        subsystem
            .collect_weapon_pickup_for_object(
                &scene,
                stale_object,
                player,
                8,
                ammo_pairs,
                [None; 2],
                &definitions,
                false,
                false,
                None,
            )
            .is_none()
    );
    assert_eq!(subsystem.players.get_handle(player).unwrap().reserve[9], 1);

    let outcome = subsystem
        .collect_weapon_pickup_for_object(
            &scene,
            live_object,
            player,
            8,
            ammo_pairs,
            [None; 2],
            &definitions,
            false,
            false,
            None,
        )
        .unwrap();
    assert!(outcome.inventory_accepted);
    assert_eq!(subsystem.players.get_handle(player).unwrap().reserve[9], 5);
}

#[test]
fn active_payload_pose_requires_the_current_scene_generation() {
    let mut scene = SceneMembership::default();
    let mut weapons = WeaponSubsystemState::<1>::with_object_capacity(16);
    let first = scene.spawn(9).unwrap();
    let first_pose = WeaponPayloadPose {
        pitch_yaw_roll_degrees: [0.0, 10.0, 0.0],
        staged_yaw_degrees: 15.0,
        rotation_mode: PayloadRotationMode::Yaw,
    };
    weapons.attach_payload(first, first_pose).unwrap();
    assert_eq!(weapons.active_payload_pose(&scene, first), Ok(first_pose));

    scene.retire(first).unwrap();
    weapons.retire_payload(first).unwrap();
    let reused = scene.spawn(9).unwrap();
    let reused_pose = WeaponPayloadPose {
        pitch_yaw_roll_degrees: [0.0, 20.0, 0.0],
        staged_yaw_degrees: 25.0,
        rotation_mode: PayloadRotationMode::Yaw,
    };
    weapons.attach_payload(reused, reused_pose).unwrap();
    assert_eq!(
        weapons.active_payload_pose(&scene, first),
        Err(WeaponPayloadLifecycleError::StaleObjectId)
    );
    assert_eq!(weapons.active_payload_pose(&scene, reused), Ok(reused_pose));
    assert_eq!(
        weapons.payload_pose(first),
        Err(WeaponPayloadLifecycleError::StaleObjectId)
    );
}

#[test]
fn weapon_pickup_inventory_commit_does_not_claim_world_retirement_order() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    let mut inventory = WeaponInventory::default();
    inventory.owned[8] = true;
    inventory.reserve[9] = 1;
    let mut subsystem = WeaponSubsystemState::<1>::default();
    let player = subsystem
        .begin_player_lifetime(WeaponPlayerId::new(0), inventory)
        .unwrap();
    let mut scene = SceneMembership::default();
    let accepted_object = scene.spawn(4).unwrap();
    let pair = Some(WeaponPickupAmmo {
        ammo_type: ammo(9),
        amount: 4,
        reserve_maximum: 5,
    });

    let accepted = subsystem
        .collect_weapon_pickup_for_object(
            &scene,
            accepted_object,
            player,
            8,
            [pair, None],
            [None; 2],
            &definitions,
            false,
            false,
            None,
        )
        .unwrap();
    assert!(accepted.inventory_accepted);
    assert!(accepted.grant_called);
    assert!(scene.is_active(accepted_object));
    assert_eq!(subsystem.players.get_handle(player).unwrap().reserve[9], 5);

    let refused_object = scene.spawn(5).unwrap();
    let inventory = subsystem.players.get_handle_mut(player).unwrap();
    inventory.reserve[9] = 5;
    let refused = subsystem
        .collect_weapon_pickup_for_object(
            &scene,
            refused_object,
            player,
            8,
            [pair, None],
            [None; 2],
            &definitions,
            false,
            false,
            None,
        )
        .unwrap();
    assert!(!refused.inventory_accepted);
    assert!(!refused.grant_called);
    assert!(scene.is_active(refused_object));
    scene.retire(accepted_object).unwrap();
    scene.retire(refused_object).unwrap();
}

#[test]
fn collect_selection_reports_unresolved_normalization_without_faking_desired_state() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    let mut state = WeaponInventory {
        desired_entry: Some(3),
        ..WeaponInventory::default()
    };

    let outcome = state.collect_entry_and_select(&definitions, 8, true, Some(ENTRY_COUNT));

    assert!(outcome.selection_requested);
    assert_eq!(outcome.selected_entry, None);
    assert!(outcome.selection_unresolved);
    assert_eq!(state.desired_entry, Some(3));
    assert!(state.owned[8]);
    assert!(outcome.acquisition.acquired);
}

#[test]
fn pickup_core_applies_reserve_and_owned_acquisition_in_source_order() {
    let mut definitions = [None; ENTRY_COUNT];
    definitions[8] = Some(InventoryEntryDefinition {
        group: 5,
        flags: 0x200,
        record_present: true,
        primary_ammo: Some(ammo(9)),
        secondary_ammo: None,
        grant_saved_rounds: None,
    });
    let mut state = WeaponInventory::default();
    let mut maximums = [i16::MAX; 44];
    maximums[9] = 5;

    let batch = state.collect_ammo_pickup_and_acquire(
        [
            PickupAmmoPair {
                ammo_type: 9,
                amount: 5,
            },
            PickupAmmoPair {
                ammo_type: 9,
                amount: 2,
            },
        ],
        [None; 2],
        &maximums,
        false,
        &definitions,
    );

    assert_eq!(state.reserve[9], 5);
    assert!(batch.pickups[0].unwrap().acquisition_check_requested);
    assert_eq!(batch.pickups[0].unwrap().reserve_delta, 5);
    assert!(batch.acquisitions[0].acquired);
    assert!(batch.acquisitions[0].newly_owned[8]);
    assert!(!batch.pickups[1].unwrap().acquisition_check_requested);
    assert!(!batch.acquisitions[1].acquired);
    assert!(state.owned[8]);
}

#[test]
fn reserve_add_wraps_the_word_before_loaded_rounds_cap() {
    let mut state = WeaponInventory::default();
    state.reserve[3] = 4;

    let added = state.add_ammo_reserve(ammo(3), 9, 4, 10);

    assert_eq!(added, 2);
    assert_eq!(state.reserve[3], 6);

    state.reserve[3] = i16::MAX;
    let wrapped = state.add_ammo_reserve(ammo(3), 1, 0, i16::MAX as i32);
    assert_eq!(wrapped, -65_535);
    assert_eq!(state.reserve[3], i16::MIN);
}

#[test]
fn pickup_core_auto_selects_owned_matching_entries_in_resolved_priority_order() {
    let mut definitions = [None; ENTRY_COUNT];
    for entry in [2, 4, 6] {
        definitions[entry] = Some(InventoryEntryDefinition {
            group: entry as u32,
            flags: 0x200,
            record_present: true,
            primary_ammo: Some(ammo(9)),
            secondary_ammo: None,
            grant_saved_rounds: None,
        });
    }
    let mut context = AmmoAutoSelectionContext {
        profile_available: true,
        player_index: 0,
        weapon_change_mode: 2,
        ..AmmoAutoSelectionContext::default()
    };
    context.priorities[1] = Some(20);
    context.priorities[2] = Some(10);
    context.priorities[3] = Some(15);
    context.priorities[4] = Some(12);
    context.priorities[5] = Some(11);
    context.priorities[6] = Some(12);
    context.matching_loaded_rounds_zero[2] = true;
    context.matching_loaded_rounds_zero[4] = true;
    context.matching_loaded_rounds_zero[6] = true;
    context.normalized_entries[2] = Some(3);
    context.normalized_entries[4] = Some(5);
    context.normalized_entries[6] = Some(7);

    let mut state = WeaponInventory {
        desired_entry: Some(1),
        ..WeaponInventory::default()
    };
    state.owned[2] = true;
    state.owned[4] = true;
    state.owned[6] = true;
    let mut maximums = [i16::MAX; 44];
    maximums[9] = 0;

    let batch = state.collect_ammo_pickup_acquire_and_select(
        [
            PickupAmmoPair {
                ammo_type: 9,
                amount: 1,
            },
            PickupAmmoPair::default(),
        ],
        [None; 2],
        &maximums,
        true,
        &definitions,
        [Some(context), None],
    );

    assert_eq!(batch.pickups[0].unwrap().reserve_delta, 0);
    assert!(!batch.acquisitions[0].acquired);
    assert!(batch.auto_selections[0].selected_entries[2]);
    assert!(batch.auto_selections[0].selected_entries[4]);
    assert!(!batch.auto_selections[0].selected_entries[6]);
    assert_eq!(batch.auto_selections[0].resulting_desired_entry, Some(5));
    assert_eq!(state.desired_entry, Some(5));
}
