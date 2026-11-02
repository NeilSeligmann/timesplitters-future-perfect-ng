// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::owned::runtime::LockedInventoryStore;
use crate::{Child, Completion, EffectError, Effects, Side, realize};
use core::cell::RefCell;
use tsfp_weapon_owner::weapon_hands::HandSide;
use tsfp_weapon_owner::weapon_inventory::{WeaponInventory, WeaponPlayerId};

struct Memory<'a>(&'a RefCell<[u8; 0xbb0]>, bool);
impl ExtensionAccess for Memory<'_> {
    fn with<R>(&mut self, operation: impl FnOnce(&mut [u8]) -> R) -> Result<R, OwnerError> {
        let mut memory = self.0.try_borrow_mut().map_err(|_| OwnerError::Busy)?;
        let len = if self.1 { 0xbb0 } else { 0xbaf };
        Ok(operation(&mut memory[..len]))
    }
}

#[test]
fn inaccessible_memory_leaves_transition_retryable_without_mutating_owner() {
    let store = LockedInventoryStore::<1>::new();
    let handle = store.with(|state| {
        state
            .begin_player_lifetime(WeaponPlayerId::new(0), WeaponInventory::default())
            .unwrap()
    });
    let bytes = RefCell::new([0xa5; 0xbb0]);
    let mut owner = MappedOwner {
        access: store.access(handle, Side::Left),
        memory: Memory(&bytes, false),
        definitions: Definitions::default(),
    };
    assert_eq!(
        owner.begin(Record::Present(0x29)),
        Err(OwnerError::Unsupported)
    );
    owner.memory.1 = true;
    let Begin::Active(ticket) = owner.begin(Record::Present(0x29)).unwrap() else {
        panic!()
    };
    let held = bytes.borrow_mut();
    assert_eq!(
        owner.commit(ticket, Stage::Started, Mutation::SkipRelease),
        Err(OwnerError::Busy)
    );
    drop(held);
    // Failed acquisition did not advance protocol state.
    owner
        .commit(ticket, Stage::Started, Mutation::SkipRelease)
        .unwrap();
    assert_eq!(*bytes.borrow(), [0xa5; 0xbb0]);
    assert_eq!(
        store.read(|s| s
            .players
            .get_handle(handle)
            .unwrap()
            .hands
            .hand(HandSide::Left)
            .record),
        Some(None)
    );
}

struct Release<'a> {
    bytes: &'a RefCell<[u8; 0xbb0]>,
    store: &'a LockedInventoryStore<1>,
    calls: usize,
}
impl Effects for Release<'_> {
    fn release_child(&mut self, _: Ticket, child: Child) -> Result<(), EffectError> {
        assert_eq!(child, Child(99));
        // Both exclusive borrows have ended before retained presentation runs.
        assert!(self.store.read(|_| ()).is_some());
        self.bytes.borrow_mut()[0x800] = 0x7e;
        self.calls += 1;
        Ok(())
    }
    fn create_child(&mut self, _: Ticket) -> Result<Child, EffectError> {
        panic!("special record")
    }
    fn initialize_child(&mut self, _: Ticket, _: Child) -> Result<(), EffectError> {
        panic!("special record")
    }
    fn attach_child(&mut self, _: Ticket, _: Child) -> Result<(), EffectError> {
        panic!("special record")
    }
}

#[test]
fn runner_drops_mapping_before_callback_and_publishes_owned_special_record() {
    for (side, hand, opposite, base) in [
        (Side::Left, HandSide::Left, HandSide::Right, 0xa4),
        (Side::Right, HandSide::Right, HandSide::Left, 0x1d4),
    ] {
        let store = LockedInventoryStore::<1>::new();
        let mut inventory = WeaponInventory::default();
        inventory.hands.hand_mut(hand).child = Some(99);
        inventory.hands.hand_mut(opposite).child = Some(77);
        let handle = store.with(|state| {
            state
                .begin_player_lifetime(WeaponPlayerId::new(0), inventory)
                .unwrap()
        });
        let bytes = RefCell::new([0xa5; 0xbb0]);
        let mut owner = MappedOwner {
            access: store.access(handle, side),
            memory: Memory(&bytes, true),
            definitions: Definitions::default(),
        };
        let mut effects = Release {
            bytes: &bytes,
            store: &store,
            calls: 0,
        };
        assert!(matches!(
            realize(&mut owner, &mut effects, Record::Present(0x29)),
            Ok(Completion::Replaced { .. })
        ));
        assert_eq!(effects.calls, 1);
        assert_eq!(
            store.read(|s| {
                let hands = &s.players.get_handle(handle).unwrap().hands;
                (hands.hand(hand).child, hands.hand(opposite).child)
            }),
            Some((None, Some(77)))
        );
        let bytes = bytes.borrow();
        assert_eq!(&bytes[base + 0x30..base + 0x34], &0x29u32.to_le_bytes());
        assert_eq!(&bytes[base + 0xe8..base + 0xec], &u32::MAX.to_le_bytes());
        let other_base = if base == 0xa4 { 0x1d4 } else { 0xa4 };
        assert_eq!(&bytes[other_base..other_base + 0x130], &[0xa5; 0x130]);
        assert_eq!(&bytes[0xbac..0xbb0], &[0; 4]);
        assert_eq!(bytes[0x800], 0x7e); // no whole-extension snapshot writeback
    }
}
