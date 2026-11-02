// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned weapon projectile descriptor lifetimes (T2411 / T1682).
//!
//! This stores lifecycle identity after a neighbouring allocator chooses a slot. It does not
//! implement the original class-list allocator or descriptor memory layout.

use alloc::vec;
use alloc::vec::Vec;

use crate::object_pool::ObjectId;

/// Principal original descriptor class pool size (T1746/T1682); configurable for tests/hosts.
pub const PROJECTILE_POOL_CAPACITY: usize = 85;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileHandle {
    pub slot: u16,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileAttribution {
    OwnerExtension(ObjectId),
    Actor(ObjectId),
}

/// Typed owner inputs corresponding to measured descriptor writes +0x104/+0x108/+0x10c.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileOwners {
    pub owner_extension: Option<ObjectId>,
    pub actor: ObjectId,
    pub attribution: ProjectileAttribution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileDescriptor {
    pub class: u32,
    pub dispatch_type: u32,
    pub owners: ProjectileOwners,
    /// Native generation protects Rust references; the reviewed original has no such counter.
    pub handle: ProjectileHandle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectilePoolError {
    SlotOutOfRange,
    SlotAlreadyLive,
    StaleHandle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Slot {
    generation: u64,
    descriptor: Option<ProjectileDescriptor>,
    renewals: u32,
}

/// Generation-checked projectile lifetime owner. Allocation policy remains with the class-pool
/// contract; `publish_recycled_slot` admits the selected free slot after the allocator returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponProjectileStore {
    slots: Vec<Slot>,
}

impl Default for WeaponProjectileStore {
    fn default() -> Self {
        Self::with_capacity(PROJECTILE_POOL_CAPACITY)
    }
}

impl WeaponProjectileStore {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: vec![
                Slot {
                    generation: 0,
                    descriptor: None,
                    renewals: 0,
                };
                capacity.min(u16::MAX as usize + 1)
            ],
        }
    }

    pub fn publish_recycled_slot(
        &mut self,
        slot: usize,
        class: u32,
        dispatch_type: u32,
        owners: ProjectileOwners,
    ) -> Result<ProjectileHandle, ProjectilePoolError> {
        let entry = self
            .slots
            .get_mut(slot)
            .ok_or(ProjectilePoolError::SlotOutOfRange)?;
        if entry.descriptor.is_some() {
            return Err(ProjectilePoolError::SlotAlreadyLive);
        }
        entry.generation = entry.generation.wrapping_add(1).max(1);
        entry.renewals = 0;
        let handle = ProjectileHandle {
            slot: slot as u16,
            generation: entry.generation,
        };
        entry.descriptor = Some(ProjectileDescriptor {
            class,
            dispatch_type,
            owners,
            handle,
        });
        Ok(handle)
    }

    /// Continuous reuse refreshes presentation externally but retains the recorded owner identity.
    pub fn renew(&mut self, handle: ProjectileHandle) -> Result<u32, ProjectilePoolError> {
        let entry = self.slot_mut(handle)?;
        entry.renewals = entry.renewals.saturating_add(1);
        Ok(entry.renewals)
    }

    /// Release is idempotent at the call boundary: only a live matching generation retires.
    pub fn retire(
        &mut self,
        handle: ProjectileHandle,
    ) -> Result<ProjectileDescriptor, ProjectilePoolError> {
        let entry = self.slot_mut(handle)?;
        entry
            .descriptor
            .take()
            .ok_or(ProjectilePoolError::StaleHandle)
    }

    pub fn get(
        &self,
        handle: ProjectileHandle,
    ) -> Result<&ProjectileDescriptor, ProjectilePoolError> {
        let entry = self
            .slots
            .get(handle.slot as usize)
            .ok_or(ProjectilePoolError::StaleHandle)?;
        if entry.generation != handle.generation {
            return Err(ProjectilePoolError::StaleHandle);
        }
        entry
            .descriptor
            .as_ref()
            .ok_or(ProjectilePoolError::StaleHandle)
    }

    fn slot_mut(&mut self, handle: ProjectileHandle) -> Result<&mut Slot, ProjectilePoolError> {
        let entry = self
            .slots
            .get_mut(handle.slot as usize)
            .ok_or(ProjectilePoolError::StaleHandle)?;
        if entry.generation != handle.generation || entry.descriptor.is_none() {
            return Err(ProjectilePoolError::StaleHandle);
        }
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(slot: u32, generation: u64) -> ObjectId {
        ObjectId { slot, generation }
    }

    #[test]
    fn recycle_changes_generation_and_stale_owner_cannot_retire_reused_slot() {
        let owners = ProjectileOwners {
            owner_extension: Some(owner(4, 9)),
            actor: owner(7, 3),
            attribution: ProjectileAttribution::OwnerExtension(owner(4, 9)),
        };
        let mut pool = WeaponProjectileStore::with_capacity(1);
        let first = pool.publish_recycled_slot(0, 12, 0x21, owners).unwrap();
        pool.retire(first).unwrap();
        let second = pool.publish_recycled_slot(0, 12, 0x21, owners).unwrap();
        assert_ne!(first.generation, second.generation);
        assert_eq!(pool.retire(first), Err(ProjectilePoolError::StaleHandle));
        assert_eq!(pool.get(second).unwrap().owners, owners);
    }

    #[test]
    fn continuous_renewal_preserves_owner_and_rejects_retired_handle() {
        let owners = ProjectileOwners {
            owner_extension: None,
            actor: owner(2, 5),
            attribution: ProjectileAttribution::Actor(owner(2, 5)),
        };
        let mut pool = WeaponProjectileStore::with_capacity(1);
        let handle = pool.publish_recycled_slot(0, 3, 4, owners).unwrap();
        assert_eq!(pool.renew(handle), Ok(1));
        assert_eq!(pool.renew(handle), Ok(2));
        assert_eq!(pool.get(handle).unwrap().owners, owners);
        pool.retire(handle).unwrap();
        assert_eq!(pool.renew(handle), Err(ProjectilePoolError::StaleHandle));
    }

    #[test]
    fn invalid_slot_and_live_slot_are_rejected_without_mutation() {
        let owners = ProjectileOwners {
            owner_extension: None,
            actor: owner(1, 1),
            attribution: ProjectileAttribution::Actor(owner(1, 1)),
        };
        let mut pool = WeaponProjectileStore::with_capacity(1);
        assert_eq!(
            pool.publish_recycled_slot(1, 3, 4, owners),
            Err(ProjectilePoolError::SlotOutOfRange)
        );

        let first = pool.publish_recycled_slot(0, 3, 4, owners).unwrap();
        assert_eq!(
            pool.publish_recycled_slot(0, 9, 8, owners),
            Err(ProjectilePoolError::SlotAlreadyLive)
        );
        assert_eq!(pool.get(first).unwrap().class, 3);
        assert_eq!(pool.retire(first).unwrap().class, 3);
    }

    #[test]
    fn zero_capacity_has_no_allocatable_slots() {
        let mut pool = WeaponProjectileStore::with_capacity(0);
        let owners = ProjectileOwners {
            owner_extension: None,
            actor: owner(0, 1),
            attribution: ProjectileAttribution::Actor(owner(0, 1)),
        };
        assert_eq!(
            pool.publish_recycled_slot(0, 1, 1, owners),
            Err(ProjectilePoolError::SlotOutOfRange)
        );
    }
}
