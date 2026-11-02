// SPDX-License-Identifier: GPL-3.0-or-later
//! Acquire compatibility memory before mutation and release it before callbacks.
use super::projection::{WriteProjection, memory::ExtensionSink};
use super::{Definitions, Projection, ScopedAccess};
use crate::{Begin, Mutation, OwnerAccess, OwnerError, Record, Stage, Ticket};

#[allow(unsafe_code)]
pub mod window;

/// Grants an exclusive live extension view only for this closure. Implementors
/// must not run callbacks, retain references, or substitute a copied snapshot.
/// Failure must not invoke the closure. Acquisition occurs under the owner lock
/// and must be nonblocking; release the view before returning.
pub trait ExtensionAccess {
    fn with<R>(&mut self, operation: impl FnOnce(&mut [u8]) -> R) -> Result<R, OwnerError>;
}

/// The memory lease and its bounds are checked before each owner mutation.
/// A missing mapping cannot leave the owner ahead of its compatibility view.
/// Lock order is owner then extension, with neither held across an effect.
pub struct MappedOwner<A, M, const N: usize> {
    pub access: A,
    pub memory: M,
    pub definitions: Definitions,
}

impl<A: ScopedAccess<N>, M: ExtensionAccess, const N: usize> OwnerAccess for MappedOwner<A, M, N> {
    fn begin(&mut self, target: Record) -> Result<Begin, OwnerError> {
        let definitions = self.definitions;
        self.access.with(|store, transition| {
            self.memory.with(|bytes| {
                let _view = ExtensionSink::new(bytes).ok_or(OwnerError::Unsupported)?;
                transition.begin(
                    store,
                    target,
                    definitions.previous,
                    definitions.next,
                    definitions.restrict_primary_to_one,
                )
            })?
        })?
    }

    fn commit(
        &mut self,
        ticket: Ticket,
        expected: Stage,
        mutation: Mutation,
    ) -> Result<(), OwnerError> {
        self.access.with(|store, transition| {
            self.memory.with(|bytes| {
                let view = ExtensionSink::new(bytes).ok_or(OwnerError::Unsupported)?;
                let publication = transition.commit(store, ticket, expected, mutation)?;
                let owner = store
                    .get_handle(transition.handle)
                    .ok_or(OwnerError::Stale)?;
                WriteProjection(view).publish(owner, ticket, mutation, publication);
                Ok(())
            })?
        })?
    }
}

#[cfg(test)]
mod tests;
