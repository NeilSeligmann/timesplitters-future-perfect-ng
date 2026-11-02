// SPDX-License-Identifier: GPL-3.0-or-later
//! Connect the effect runner to short accesses of the actual typed store.
use super::{OwnedTransition, Publication};
use crate::{Begin, Mutation, OwnerAccess, OwnerError, Record, Stage, Ticket};
use tsfp_weapon_owner::weapon_hands::WeaponAmmoRecord;
use tsfp_weapon_owner::weapon_inventory::{WeaponInventory, WeaponInventoryStore};

pub mod mapped;
pub mod projection;

/// Implement using the runtime's existing inventory lock. Metadata must share
/// that lock. The closure cannot retain a store borrow; unlock before returning.
/// A rejected access must not execute the closure.
pub trait ScopedAccess<const N: usize>:
    tsfp_weapon_realization_protocol::OwnerAccess<
        WeaponInventoryStore<N>,
        OwnedTransition,
        Error = OwnerError,
    >
{
}

impl<T, const N: usize> ScopedAccess<N> for T where
    T: tsfp_weapon_realization_protocol::OwnerAccess<
            WeaponInventoryStore<N>,
            OwnedTransition,
            Error = OwnerError,
        >
{
}

/// Temporary compatibility boundary (removal T2411). Called under the owner's
/// lock after a successful mutation, before any presentation effect. Implement
/// only infallible memory publications: NO guest calls, reentry or other locks.
/// Do not read back mutable guest state into the owner. Preflight addresses before
/// beginning a transition; a failed publication must never silently continue.
pub trait Projection {
    fn publish(
        &mut self,
        owner: &WeaponInventory,
        ticket: Ticket,
        mutation: Mutation,
        publication: Publication,
    );
}

/// Immutable inputs resolved before entering the replacement block.
#[derive(Clone, Copy, Debug, Default)]
pub struct Definitions {
    pub previous: Option<WeaponAmmoRecord>,
    pub next: Option<WeaponAmmoRecord>,
    pub restrict_primary_to_one: bool,
}

/// Does not hold a store reference/lock across runner calls. The access object
/// locates the authoritative store and metadata for each begin/commit.
pub struct ScopedOwner<A, P, const N: usize> {
    pub access: A,
    pub projection: P,
    pub definitions: Definitions,
}

impl<A: ScopedAccess<N>, P: Projection, const N: usize> OwnerAccess for ScopedOwner<A, P, N> {
    fn begin(&mut self, target: Record) -> Result<Begin, OwnerError> {
        let definitions = self.definitions;
        self.access.with(|store, transition| {
            transition.begin(
                store,
                target,
                definitions.previous,
                definitions.next,
                definitions.restrict_primary_to_one,
            )
        })?
    }

    fn commit(
        &mut self,
        ticket: Ticket,
        expected: Stage,
        mutation: Mutation,
    ) -> Result<(), OwnerError> {
        self.access.with(|store, transition| {
            let publication = transition.commit(store, ticket, expected, mutation)?;
            // commit checked this lifetime under this same exclusive access.
            let owner = store
                .get_handle(transition.handle)
                .ok_or(OwnerError::Stale)?;
            self.projection
                .publish(owner, ticket, mutation, publication);
            Ok(())
        })?
    }
}

#[cfg(test)]
mod tests;
