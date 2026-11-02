// SPDX-License-Identifier: GPL-3.0-or-later
use super::projection::{Sink, WriteProjection};
use super::*;
use crate::{Child, Completion, EffectError, Effects, Side, realize};
use core::cell::RefCell;
use tsfp_weapon_owner::weapon_hands::{AmmoType, HandSide, WeaponRecordId};
use tsfp_weapon_owner::weapon_inventory::{WeaponPlayerHandle, WeaponPlayerId};
use tsfp_weapon_realization_protocol::OwnerAccess as ProtocolOwnerAccess;

struct State {
    store: WeaponInventoryStore<1>,
    transition: OwnedTransition,
}
struct Access<'a>(&'a RefCell<State>);
impl ProtocolOwnerAccess<WeaponInventoryStore<1>, OwnedTransition> for Access<'_> {
    type Error = OwnerError;

    fn with<R>(
        &mut self,
        operation: impl FnOnce(&mut WeaponInventoryStore<1>, &mut OwnedTransition) -> R,
    ) -> Result<R, OwnerError> {
        let mut state = self.0.try_borrow_mut().map_err(|_| OwnerError::Busy)?;
        let State { store, transition } = &mut *state;
        Ok(operation(store, transition))
    }
}

#[derive(Default)]
struct Visible {
    record: Option<WeaponRecordId>,
    child: Option<u32>,
    flag: bool,
    publications: usize,
}
struct Project<'a>(&'a RefCell<Visible>, Side);
impl Sink for Project<'_> {
    fn child(&mut self, side: Side, child: Option<u32>) {
        assert_eq!(side, self.1);
        let mut visible = self.0.borrow_mut();
        visible.child = child;
        visible.publications += 1;
    }
    fn record(&mut self, side: Side, record: Option<WeaponRecordId>) {
        assert_eq!(side, self.1);
        let mut visible = self.0.borrow_mut();
        visible.record = record;
        visible.publications += 1;
    }
    fn clear_realization_flag(&mut self) {
        let mut visible = self.0.borrow_mut();
        assert_eq!(visible.record, Some(WeaponRecordId(3)));
        visible.flag = false;
        visible.publications += 1;
    }
    fn ammo(&mut self, side: Side, loaded: &[i16], saved: &[i16], reserve: &[i16]) {
        assert_eq!(side, self.1);
        assert_eq!(loaded.len(), reserve.len());
        assert_eq!(saved.len(), reserve.len());
        let mut visible = self.0.borrow_mut();
        // Callback increments reserve after unload; reload must publish the
        // authoritative updated reserve, not restore a pre-callback snapshot.
        if visible.record == Some(WeaponRecordId(2)) {
            assert_eq!((loaded[1], reserve[1]), (0, 14));
        } else {
            assert_eq!((loaded[1], reserve[1]), (6, 11));
        }
        visible.publications += 1;
    }
}

struct Reentry<'a> {
    state: &'a RefCell<State>,
    visible: &'a RefCell<Visible>,
    handle: WeaponPlayerHandle,
    retire_during_create: bool,
    calls: usize,
}
impl Effects for Reentry<'_> {
    fn release_child(&mut self, _: Ticket, child: Child) -> Result<(), EffectError> {
        assert_eq!(child, Child(99));
        assert_eq!(self.visible.borrow().child, Some(99));
        assert!(self.state.try_borrow_mut().is_ok());
        self.calls += 1;
        Ok(())
    }
    fn create_child(&mut self, _: Ticket) -> Result<Child, EffectError> {
        assert_eq!(self.visible.borrow().record, Some(WeaponRecordId(3)));
        assert!(!self.visible.borrow().flag);
        let mut state = self.state.borrow_mut(); // would panic if owner access escaped
        if self.retire_during_create {
            state.store.retire_handle(self.handle).unwrap();
        } else {
            state.store.get_handle_mut(self.handle).unwrap().reserve[1] += 3;
        }
        self.calls += 1;
        Ok(Child(123))
    }
    fn initialize_child(&mut self, _: Ticket, child: Child) -> Result<(), EffectError> {
        assert_eq!(self.visible.borrow().child, Some(child.0));
        assert!(self.state.try_borrow_mut().is_ok());
        self.calls += 1;
        Ok(())
    }
    fn attach_child(&mut self, _: Ticket, _: Child) -> Result<(), EffectError> {
        assert!(self.state.try_borrow_mut().is_ok());
        self.calls += 1;
        Ok(())
    }
}

fn run_case(retire: bool, side: Side) {
    let old = WeaponAmmoRecord {
        id: WeaponRecordId(2),
        primary: AmmoType::new(1),
        primary_capacity: 6,
        ..Default::default()
    };
    let next = WeaponAmmoRecord {
        id: WeaponRecordId(3),
        ..old
    };
    let mut inventory = WeaponInventory::default();
    inventory.reserve[1] = 10;
    let hand = inventory.hands.hand_mut(match side {
        Side::Left => HandSide::Left,
        Side::Right => HandSide::Right,
    });
    hand.record = Some(old.id);
    hand.child = Some(99);
    hand.loaded[1] = 4;
    let mut store = WeaponInventoryStore::default();
    let handle = store
        .begin_lifetime(WeaponPlayerId::new(0), inventory)
        .unwrap();
    let state = RefCell::new(State {
        store,
        transition: OwnedTransition::new(handle, side),
    });
    let visible = RefCell::new(Visible {
        flag: true,
        record: Some(old.id),
        child: Some(99),
        ..Default::default()
    });
    let mut owner = ScopedOwner {
        access: Access(&state),
        projection: WriteProjection(Project(&visible, side)),
        definitions: Definitions {
            previous: Some(old),
            next: Some(next),
            restrict_primary_to_one: false,
        },
    };
    let mut effects = Reentry {
        state: &state,
        visible: &visible,
        handle,
        retire_during_create: retire,
        calls: 0,
    };
    let result = realize(&mut owner, &mut effects, Record::Present(3));
    if retire {
        let failure = result.unwrap_err();
        assert_eq!(failure.cause, crate::Cause::Owner(OwnerError::Stale));
        assert_eq!(
            failure.created_child.unwrap(),
            crate::ChildReceipt {
                child: Child(123),
                published: false
            }
        );
        assert_eq!(effects.calls, 2);
        assert_eq!(visible.borrow().child, None);
    } else {
        assert!(matches!(result, Ok(Completion::Replaced { .. })));
        assert_eq!(effects.calls, 4);
        assert_eq!(
            state.borrow().store.get_handle(handle).unwrap().reserve[1],
            11
        );
        assert_eq!(visible.borrow().child, Some(123));
    }
    let count = visible.borrow().publications;
    // Rejection never performs compatibility writes.
    let result = owner
        .access
        .with(|store, transition| {
            let ticket = match transition.begin(store, Record::Absent, Some(next), None, false)? {
                Begin::Active(t) => t,
                _ => unreachable!(),
            };
            transition.invalidate();
            Ok::<_, OwnerError>(ticket)
        })
        .unwrap();
    if let Ok(ticket) = result {
        assert_eq!(
            owner.commit(ticket, Stage::Started, Mutation::SkipRelease),
            Err(OwnerError::Stale)
        );
    }
    assert_eq!(visible.borrow().publications, count);
}

#[test]
fn callbacks_reenter_actual_store_and_observe_ordered_publication() {
    run_case(false, Side::Left);
}

#[test]
fn callback_retirement_stops_publication_and_preserves_raw_receipt() {
    run_case(true, Side::Left);
}

#[test]
fn right_hand_publication_uses_selected_owner_not_sibling() {
    run_case(false, Side::Right);
}
