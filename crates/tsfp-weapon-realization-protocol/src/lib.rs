// SPDX-License-Identifier: GPL-3.0-or-later
//! State-free contract for short, callback-free owner transactions.
#![no_std]
#![deny(unsafe_code)]

/// Perform one bounded operation over the authoritative store and metadata.
/// Implementations must release their lock/borrows before returning and must
/// never invoke presentation effects from `operation`.
pub trait OwnerAccess<Store, Metadata> {
    type Error;

    fn with<R>(
        &mut self,
        operation: impl FnOnce(&mut Store, &mut Metadata) -> R,
    ) -> Result<R, Self::Error>;
}
