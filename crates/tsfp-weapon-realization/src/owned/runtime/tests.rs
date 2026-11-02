// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::{Begin, Mutation, Record, Stage, Ticket};
use tsfp_weapon_owner::weapon_inventory::{WeaponInventory, WeaponPlayerId};
use tsfp_weapon_realization_protocol::OwnerAccess as ProtocolOwnerAccess;

fn fixture() -> (LockedInventoryStore<1>, WeaponPlayerHandle) {
    let owner = LockedInventoryStore::new();
    let handle = owner.with(|state| {
        state
            .begin_player_lifetime(WeaponPlayerId::new(0), WeaponInventory::default())
            .unwrap()
    });
    (owner, handle)
}

fn begin(access: &mut RuntimeAccess<'_, 1>) -> Ticket {
    match access
        .with(|store, transition| transition.begin(store, Record::Present(0x29), None, None, false))
        .unwrap()
        .unwrap()
    {
        Begin::Active(ticket) => ticket,
        _ => panic!("expected changed target"),
    }
}

#[test]
fn runtime_read_preserves_ticket_and_hands_have_separate_metadata() {
    let (owner, handle) = fixture();
    let mut left = owner.access(handle, Side::Left);
    let ticket = begin(&mut left);
    assert_eq!(
        owner.read(|state| state.players.get_handle(handle).unwrap().reserve[0]),
        Some(0)
    );
    let mut right = owner.access(handle, Side::Right);
    let right_ticket = begin(&mut right);
    assert_ne!(ticket, right_ticket);
    assert!(
        left.with(|store, transition| transition.commit(
            store,
            ticket,
            Stage::Started,
            Mutation::SkipRelease
        ))
        .unwrap()
        .is_ok()
    );
    assert!(
        right
            .with(|store, transition| transition.commit(
                store,
                right_ticket,
                Stage::Started,
                Mutation::SkipRelease
            ))
            .unwrap()
            .is_ok()
    );
}

#[test]
fn switch_requests_coalesce_in_one_generation_bound_selection_slot() {
    let (owner, handle) = fixture();
    assert_eq!(
        owner.stage_selection(handle, 3),
        Ok(Some(SelectionHandoff {
            handle,
            previous: None,
            desired: 3,
        }))
    );
    assert_eq!(
        owner.stage_selection(handle, 5),
        Ok(Some(SelectionHandoff {
            handle,
            previous: Some(3),
            desired: 5,
        }))
    );
    assert_eq!(owner.pending_selection(handle).unwrap().desired, 5);
    assert_eq!(owner.stage_selection(handle, 5), Ok(None));
    assert_eq!(owner.pending_selection(handle).unwrap().desired, 5);
    assert_eq!(
        owner.read(|state| state.players.get_handle(handle).unwrap().desired_entry),
        Some(Some(5))
    );
}

#[test]
fn switch_handoff_does_not_follow_a_reused_player_slot() {
    let (owner, old_handle) = fixture();
    owner.stage_selection(old_handle, 7).unwrap();
    owner.with(|state| {
        state.players.retire_handle(old_handle).unwrap();
        state
            .begin_player_lifetime(old_handle.player(), WeaponInventory::default())
            .unwrap()
    });

    assert_eq!(owner.pending_selection(old_handle), None);
    assert_eq!(owner.stage_selection(old_handle, 9), Err(OwnerError::Stale));
}

#[test]
fn runtime_mutation_invalidates_old_ticket_without_losing_callback_state() {
    let (owner, handle) = fixture();
    let mut access = owner.access(handle, Side::Left);
    let obsolete = begin(&mut access);
    owner.with(|state| state.players.get_handle_mut(handle).unwrap().reserve[1] = 37);
    let fresh = begin(&mut access);
    assert_ne!(obsolete, fresh);
    assert_eq!(
        access
            .with(|store, transition| transition.commit(
                store,
                obsolete,
                Stage::Started,
                Mutation::SkipRelease
            ))
            .unwrap(),
        Err(OwnerError::Stale)
    );
    assert!(
        access
            .with(|store, transition| transition.commit(
                store,
                fresh,
                Stage::Started,
                Mutation::SkipRelease
            ))
            .unwrap()
            .is_ok()
    );
    assert_eq!(
        owner.read(|state| state.players.get_handle(handle).unwrap().reserve[1]),
        Some(37)
    );
}

#[test]
fn runtime_retirement_and_slot_reuse_reject_old_handle() {
    let (owner, handle) = fixture();
    let mut access = owner.access(handle, Side::Left);
    let obsolete = begin(&mut access);
    let replacement = owner.with(|state| {
        state.players.retire_handle(handle).unwrap();
        state
            .begin_player_lifetime(handle.player(), WeaponInventory::default())
            .unwrap()
    });
    assert_ne!(handle, replacement);
    assert_eq!(
        access.with(|_, _| panic!("stale closure must not run")),
        Err::<(), _>(OwnerError::Stale)
    );
    let mut replacement_access = owner.access(replacement, Side::Left);
    let fresh = begin(&mut replacement_access);
    assert_ne!(obsolete, fresh);
    assert_eq!(
        replacement_access
            .with(|store, transition| transition.commit(
                store,
                obsolete,
                Stage::Started,
                Mutation::SkipRelease
            ))
            .unwrap(),
        Err(OwnerError::Stale)
    );
}

#[test]
fn runtime_contention_declines_without_invalidation_and_unwind_unlocks() {
    let (owner, handle) = fixture();
    let mut access = owner.access(handle, Side::Left);
    let ticket = begin(&mut access);
    owner
        .read(|_| {
            assert_eq!(
                access.with(|_, _| panic!("busy closure must not run")),
                Err::<(), _>(OwnerError::Busy)
            );
            assert!(
                owner
                    .try_with(|_| panic!("busy mutation must not run"))
                    .is_none()
            );
        })
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        owner.read(|_| panic!("test unwinding read"));
    }));
    assert!(result.is_err());
    assert!(
        access
            .with(|store, transition| transition.commit(
                store,
                ticket,
                Stage::Started,
                Mutation::SkipRelease
            ))
            .unwrap()
            .is_ok()
    );
}
