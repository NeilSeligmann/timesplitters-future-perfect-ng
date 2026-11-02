// SPDX-License-Identifier: GPL-3.0-or-later
//! Short, callback-free mutations of the existing Rust inventory owner.
//!
//! Keep this metadata beside the store under the same lock. Acquire that lock
//! for one begin/commit only; release it before executing presentation effects.
//! All ownership-changing paths must invalidate this metadata before changing
//! a hand. Production hook registration and compatibility publications remain
//! the caller's responsibility; this module does not enable a runtime path.
use crate::{
    Begin, Child, HandSnapshot, Mutation, OwnerError, OwnerIdentity, Record, Side, Stage, Ticket,
    TransitionGuard,
};
use tsfp_weapon_owner::weapon_hands::{HandSide, WeaponAmmoRecord, WeaponRecordId};
use tsfp_weapon_owner::weapon_inventory::{WeaponInventoryStore, WeaponPlayerHandle};

pub mod runtime;
pub mod scoped;

/// Precise publication obligation, to be consumed before the next callback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Publication {
    /// The original replacement clears the adjacent extension realization flag.
    pub clear_realization_flag: bool,
}

/// No hand/reserve copies: only transition metadata and immutable definitions.
pub struct OwnedTransition {
    handle: WeaponPlayerHandle,
    side: HandSide,
    guard: TransitionGuard,
    previous: Option<WeaponAmmoRecord>,
    next: Option<WeaponAmmoRecord>,
    restrict_primary_to_one: bool,
}

impl OwnedTransition {
    pub fn new(handle: WeaponPlayerHandle, side: Side) -> Self {
        Self {
            handle,
            side: match side {
                Side::Left => HandSide::Left,
                Side::Right => HandSide::Right,
            },
            guard: TransitionGuard::new(OwnerIdentity {
                player: handle.player().slot() as u16,
                generation: u64::from(handle.generation()),
                side,
            }),
            previous: None,
            next: None,
            restrict_primary_to_one: false,
        }
    }

    /// Call before a same-lifetime hand replacement outside this transition.
    pub fn invalidate(&mut self) {
        self.guard.invalidate(OwnerIdentity {
            player: self.handle.player().slot() as u16,
            generation: u64::from(self.handle.generation()),
            side: match self.side {
                HandSide::Left => Side::Left,
                HandSide::Right => Side::Right,
            },
        });
    }

    /// Resolve definitions from immutable tables before beginning. Missing or
    /// mismatched definitions fail before any hand, reserve or protocol mutation.
    pub fn begin<const N: usize>(
        &mut self,
        store: &WeaponInventoryStore<N>,
        target: Record,
        previous: Option<WeaponAmmoRecord>,
        next: Option<WeaponAmmoRecord>,
        restrict_primary_to_one: bool,
    ) -> Result<Begin, OwnerError> {
        let inventory = store.get_handle(self.handle).ok_or(OwnerError::Stale)?;
        let hand = inventory.hands.hand(self.side);
        let before = HandSnapshot {
            record: hand
                .record
                .map_or(Record::Absent, |id| Record::Present(id.0)),
            child: Child(hand.child.unwrap_or(0)),
        };
        if before.record != target {
            if let Some(id) = hand.record {
                if previous.map(|r| r.id) != Some(id) {
                    return Err(OwnerError::Unsupported);
                }
            }
            if let Record::Present(id) = target {
                if target.creates_child() && next.map(|r| r.id) != Some(WeaponRecordId(id)) {
                    return Err(OwnerError::Unsupported);
                }
            }
        }
        let result = self.guard.begin(before, target)?;
        self.previous = previous;
        self.next = next;
        self.restrict_primary_to_one = restrict_primary_to_one;
        Ok(result)
    }

    /// The real generation check, stage check and owner mutation share one store
    /// borrow. No guest access, effect callback or cloned-owner writeback occurs.
    pub fn commit<const N: usize>(
        &mut self,
        store: &mut WeaponInventoryStore<N>,
        ticket: Ticket,
        expected: Stage,
        mutation: Mutation,
    ) -> Result<Publication, OwnerError> {
        let inventory = store.get_handle_mut(self.handle).ok_or(OwnerError::Stale)?;
        let mut publication = Publication::default();
        self.guard.commit(ticket, expected, mutation, || {
            match mutation {
                Mutation::ClearChild => inventory.hands.hand_mut(self.side).child = None,
                Mutation::UnloadPrevious => {
                    if let Some(record) = self.previous {
                        inventory.hands.unload_without_clamp(
                            &mut inventory.reserve,
                            self.side,
                            record,
                        );
                    }
                }
                Mutation::PublishRecord => {
                    inventory.hands.hand_mut(self.side).record = match ticket.target() {
                        Record::Absent => None,
                        Record::Present(id) => Some(WeaponRecordId(id)),
                    };
                    publication.clear_realization_flag = true;
                }
                Mutation::PublishChild(child) => {
                    inventory.hands.hand_mut(self.side).child = Some(child.0)
                }
                Mutation::Reload => {
                    // An ordinary target's definition was checked before begin.
                    if let Some(record) = self.next {
                        inventory.hands.transfer_reload(
                            &mut inventory.reserve,
                            self.side,
                            record,
                            false,
                            self.restrict_primary_to_one,
                        );
                    }
                }
                _ => {}
            }
        })?;
        Ok(publication)
    }
}

#[cfg(test)]
mod tests;
