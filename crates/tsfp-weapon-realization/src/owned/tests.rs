// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use tsfp_weapon_owner::weapon_hands::AmmoType;
use tsfp_weapon_owner::weapon_inventory::{WeaponInventory, WeaponPlayerId};

fn fixture() -> (
    WeaponInventoryStore<1>,
    WeaponPlayerHandle,
    OwnedTransition,
    WeaponAmmoRecord,
) {
    let record = WeaponAmmoRecord {
        id: WeaponRecordId(2),
        primary: AmmoType::new(1),
        primary_capacity: 6,
        ..WeaponAmmoRecord::default()
    };
    let mut inventory = WeaponInventory::default();
    inventory.reserve[1] = 10;
    inventory.hands.hand_mut(HandSide::Left).record = Some(record.id);
    inventory.hands.hand_mut(HandSide::Left).loaded[1] = 4;
    let mut store = WeaponInventoryStore::default();
    let handle = store
        .begin_lifetime(WeaponPlayerId::new(0), inventory)
        .unwrap();
    (
        store,
        handle,
        OwnedTransition::new(handle, Side::Left),
        record,
    )
}

fn active(begin: Begin) -> Ticket {
    match begin {
        Begin::Active(ticket) => ticket,
        _ => panic!("expected active"),
    }
}

#[test]
fn existing_owner_unloads_and_reloads_without_overwriting_intervening_reserve() {
    let (mut store, handle, mut transition, old) = fixture();
    let next = WeaponAmmoRecord {
        id: WeaponRecordId(3),
        ..old
    };
    let ticket = active(
        transition
            .begin(&store, Record::Present(3), Some(old), Some(next), false)
            .unwrap(),
    );
    transition
        .commit(&mut store, ticket, Stage::Started, Mutation::SkipRelease)
        .unwrap();
    transition
        .commit(
            &mut store,
            ticket,
            Stage::Released,
            Mutation::UnloadPrevious,
        )
        .unwrap();
    assert_eq!(store.get_handle(handle).unwrap().reserve[1], 14);
    let publication = transition
        .commit(&mut store, ticket, Stage::Unloaded, Mutation::PublishRecord)
        .unwrap();
    assert!(publication.clear_realization_flag);
    // Represents a legal reserve-only callback between scoped owner accesses.
    store.get_handle_mut(handle).unwrap().reserve[1] += 3;
    for (stage, mutation) in [
        (Stage::RecordPublished, Mutation::ClaimCreate),
        (Stage::Creating, Mutation::PublishChild(Child(u32::MAX))),
        (Stage::ChildPublished, Mutation::ClaimInitialize),
        (Stage::Initializing, Mutation::Initialized),
        (Stage::Initialized, Mutation::ClaimAttach),
        (Stage::Attaching, Mutation::Attached),
        (Stage::Attached, Mutation::Reload),
        (Stage::Reloaded, Mutation::Finish),
    ] {
        transition
            .commit(&mut store, ticket, stage, mutation)
            .unwrap();
    }
    let owner = store.get_handle(handle).unwrap();
    assert_eq!(owner.reserve[1], 11);
    assert_eq!(owner.hands.hand(HandSide::Left).loaded[1], 6);
    assert_eq!(owner.hands.hand(HandSide::Left).child, Some(u32::MAX));
    assert_eq!(
        transition.commit(&mut store, ticket, Stage::Attached, Mutation::Reload),
        Err(OwnerError::WrongStage)
    );
    assert_eq!(store.get_handle(handle).unwrap().reserve[1], 11);
}

#[test]
fn retired_lifetime_and_superseded_transition_cannot_mutate_owner() {
    let (mut store, handle, mut transition, old) = fixture();
    let ticket = active(
        transition
            .begin(&store, Record::Absent, Some(old), None, false)
            .unwrap(),
    );
    transition.invalidate();
    assert_eq!(
        transition.commit(&mut store, ticket, Stage::Started, Mutation::SkipRelease),
        Err(OwnerError::Stale)
    );
    let ticket = active(
        transition
            .begin(&store, Record::Absent, Some(old), None, false)
            .unwrap(),
    );
    store.retire_handle(handle).unwrap();
    let replacement = store
        .begin_lifetime(handle.player(), WeaponInventory::default())
        .unwrap();
    assert_eq!(
        transition.commit(&mut store, ticket, Stage::Started, Mutation::SkipRelease),
        Err(OwnerError::Stale)
    );
    assert_eq!(
        store.get_handle(replacement).unwrap(),
        &WeaponInventory::default()
    );
}

#[test]
fn missing_definition_declines_before_state_change_and_absence_skips_reload() {
    let (mut store, handle, mut transition, old) = fixture();
    let before = store.get_handle(handle).unwrap().clone();
    assert_eq!(
        transition.begin(&store, Record::Absent, None, None, false),
        Err(OwnerError::Unsupported)
    );
    assert_eq!(store.get_handle(handle).unwrap(), &before);
    let ticket = active(
        transition
            .begin(&store, Record::Absent, Some(old), None, false)
            .unwrap(),
    );
    for (stage, mutation) in [
        (Stage::Started, Mutation::SkipRelease),
        (Stage::Released, Mutation::UnloadPrevious),
        (Stage::Unloaded, Mutation::PublishRecord),
        (Stage::RecordPublished, Mutation::Finish),
    ] {
        transition
            .commit(&mut store, ticket, stage, mutation)
            .unwrap();
    }
    let owner = store.get_handle(handle).unwrap();
    assert_eq!(owner.hands.hand(HandSide::Left).record, None);
    assert_eq!(owner.reserve[1], 14);
    assert_eq!(owner.hands.hand(HandSide::Left).loaded[1], 0);
}
