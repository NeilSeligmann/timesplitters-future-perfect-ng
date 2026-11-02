// SPDX-License-Identifier: GPL-3.0-or-later
//! T1682 Rust-owned projectile slot and lease kernel.
//!
//! This models the documented static allocator contract (85 stable slots split
//! into class queues of 20/15/30/20, occupied-tail cleanup before reuse, and a
//! one-tick renewable continuous lease). It does not yet attach game callbacks,
//! resource delegates, collision, or guest dispatch; those remain explicit
//! integration boundaries. Evidence tier: 0 (typed implementation and unit
//! smoke tests); behavior is not yet validated by Tier 2 replay.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

pub const CLASS_CAPACITIES: [usize; 4] = [20, 15, 30, 20];
pub const SLOT_COUNT: usize = 85;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectileId {
    pub slot: u8,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileState {
    Free,
    Active { class: u8, continuous_renewed: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolError {
    InvalidClass,
    StaleHandle,
    RecycleInProgress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecycleTicket {
    slot: u8,
    generation: u64,
    class: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allocation {
    Ready(ProjectileId),
    CleanupRequired {
        retired: RetiredProjectile,
        ticket: RecycleTicket,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetiredProjectile {
    pub id: ProjectileId,
    pub class: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Slot {
    generation: u64,
    state: ProjectileState,
    pending_recycle: bool,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            generation: 0,
            state: ProjectileState::Free,
            pending_recycle: false,
        }
    }
}

/// Owns stable projectile slots and class-specific recycling order.
/// Allocation selects the class tail; if occupied, its identity is returned
/// for cleanup before the slot is reused. Callers must complete callback and
/// resource cleanup before publishing the replacement descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectilePool {
    slots: Vec<Slot>,
    queues: [VecDeque<u8>; 4],
}

impl Default for ProjectilePool {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectilePool {
    pub fn new() -> Self {
        let mut slots = Vec::with_capacity(SLOT_COUNT);
        let mut queues: [VecDeque<u8>; 4] = core::array::from_fn(|_| VecDeque::new());
        let mut slot = 0u8;
        for (class, capacity) in CLASS_CAPACITIES.into_iter().enumerate() {
            for _ in 0..capacity {
                slots.push(Slot::default());
                queues[class].push_back(slot);
                slot += 1;
            }
        }
        Self { slots, queues }
    }

    pub fn state(&self, id: ProjectileId) -> Result<ProjectileState, PoolError> {
        self.slots
            .get(id.slot as usize)
            .filter(|s| s.generation == id.generation)
            .map(|s| s.state)
            .ok_or(PoolError::StaleHandle)
    }

    pub fn class_order(&self, class: u8) -> Result<Vec<u8>, PoolError> {
        self.queues
            .get(class as usize)
            .map(|q| q.iter().copied().collect())
            .ok_or(PoolError::InvalidClass)
    }

    /// Selects the class tail. Occupied slots remain unchanged until the caller
    /// finishes all cleanup and supplies the returned ticket to `finish_recycle`.
    pub fn allocate(&mut self, class: u8) -> Result<Allocation, PoolError> {
        let queue = self
            .queues
            .get_mut(class as usize)
            .ok_or(PoolError::InvalidClass)?;
        let slot_id = *queue.back().ok_or(PoolError::InvalidClass)?;
        let slot = &mut self.slots[slot_id as usize];
        if slot.pending_recycle {
            return Err(PoolError::RecycleInProgress);
        }
        let result = match slot.state {
            ProjectileState::Free => {
                slot.generation = slot.generation.wrapping_add(1);
                slot.state = ProjectileState::Active {
                    class,
                    continuous_renewed: false,
                };
                Allocation::Ready(ProjectileId {
                    slot: slot_id,
                    generation: slot.generation,
                })
            }
            ProjectileState::Active {
                class: old_class, ..
            } => {
                slot.pending_recycle = true;
                let retired = RetiredProjectile {
                    id: ProjectileId {
                        slot: slot_id,
                        generation: slot.generation,
                    },
                    class: old_class,
                };
                Allocation::CleanupRequired {
                    retired,
                    ticket: RecycleTicket {
                        slot: slot_id,
                        generation: slot.generation,
                        class,
                    },
                }
            }
        };
        queue.pop_back();
        queue.push_front(slot_id);
        Ok(result)
    }

    /// Publishes the replacement only after the caller has completed cleanup.
    pub fn finish_recycle(&mut self, ticket: RecycleTicket) -> Result<ProjectileId, PoolError> {
        let slot = self
            .slots
            .get_mut(ticket.slot as usize)
            .filter(|s| s.generation == ticket.generation && s.pending_recycle)
            .ok_or(PoolError::StaleHandle)?;
        slot.generation = slot.generation.wrapping_add(1);
        slot.state = ProjectileState::Active {
            class: ticket.class,
            continuous_renewed: false,
        };
        slot.pending_recycle = false;
        Ok(ProjectileId {
            slot: ticket.slot,
            generation: slot.generation,
        })
    }

    pub fn renew_continuous(&mut self, id: ProjectileId) -> Result<(), PoolError> {
        let slot = self
            .slots
            .get_mut(id.slot as usize)
            .filter(|s| s.generation == id.generation)
            .ok_or(PoolError::StaleHandle)?;
        if slot.pending_recycle {
            return Err(PoolError::RecycleInProgress);
        }
        match slot.state {
            ProjectileState::Active { class, .. } => {
                slot.state = ProjectileState::Active {
                    class,
                    continuous_renewed: true,
                };
                Ok(())
            }
            ProjectileState::Free => Err(PoolError::StaleHandle),
        }
    }

    /// Consumes one simulation tick of the lease; false means caller should
    /// retire the projectile through its cleanup hooks.
    pub fn consume_continuous_tick(&mut self, id: ProjectileId) -> Result<bool, PoolError> {
        let slot = self
            .slots
            .get_mut(id.slot as usize)
            .filter(|s| s.generation == id.generation)
            .ok_or(PoolError::StaleHandle)?;
        if slot.pending_recycle {
            return Err(PoolError::RecycleInProgress);
        }
        match slot.state {
            ProjectileState::Active {
                class,
                continuous_renewed,
            } => {
                slot.state = ProjectileState::Active {
                    class,
                    continuous_renewed: false,
                };
                Ok(continuous_renewed)
            }
            ProjectileState::Free => Err(PoolError::StaleHandle),
        }
    }

    pub fn release(&mut self, id: ProjectileId) -> Result<(), PoolError> {
        let slot = self
            .slots
            .get_mut(id.slot as usize)
            .filter(|s| s.generation == id.generation)
            .ok_or(PoolError::StaleHandle)?;
        if slot.pending_recycle {
            return Err(PoolError::RecycleInProgress);
        }
        let class = match slot.state {
            ProjectileState::Active { class, .. } => class,
            ProjectileState::Free => return Err(PoolError::StaleHandle),
        };
        slot.state = ProjectileState::Free;
        self.queues[class as usize].retain(|&member| member != id.slot);
        self.queues[class as usize].push_back(id.slot);
        Ok(())
    }
}
