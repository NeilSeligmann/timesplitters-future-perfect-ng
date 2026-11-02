// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed access to the measured object-pool capacity ledger.
//!
//! The retired/reusable count aliases the existing ObjectPool `AUX[214]` cell;
//! this crate adds a semantic view without duplicating the owned word. It does
//! not model NPC population or define allocation, callback, rollback, or reset
//! ordering.

use tsfp_state::{
    GuestMemory,
    object_pool::{AUX, ObjectPool, S_CAPACITY, S_USED_RECORDS},
};

pub const RETIRED_SLOT_COUNT_CELL: usize = 0xd6;
pub const RETIRED_SLOT_COUNT_ADDRESS: u32 = 0x007b_65b8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolSlotCount(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolRetiredSlotCount(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolCapacity(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolCapacityLedgerState {
    pub slot_count: PoolSlotCount,
    pub retired_slot_count: PoolRetiredSlotCount,
    pub capacity: PoolCapacity,
}

/// Field-level access to the three raw words measured at the object-pool cap
/// guard. Setters do not compose allocator or lifecycle operations.
pub trait ObjectPoolCapacityLedger {
    fn capacity_ledger_state(&self) -> PoolCapacityLedgerState;

    fn set_slot_count<M: GuestMemory>(&mut self, memory: &mut M, value: PoolSlotCount) -> bool;

    fn set_retired_slot_count<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        value: PoolRetiredSlotCount,
    ) -> bool;

    fn set_capacity<M: GuestMemory>(&mut self, memory: &mut M, value: PoolCapacity) -> bool;
}

impl ObjectPoolCapacityLedger for ObjectPool {
    fn capacity_ledger_state(&self) -> PoolCapacityLedgerState {
        PoolCapacityLedgerState {
            slot_count: PoolSlotCount(self.scalar(S_USED_RECORDS)),
            retired_slot_count: PoolRetiredSlotCount(self.tables[AUX][RETIRED_SLOT_COUNT_CELL]),
            capacity: PoolCapacity(self.scalar(S_CAPACITY)),
        }
    }

    fn set_slot_count<M: GuestMemory>(&mut self, memory: &mut M, value: PoolSlotCount) -> bool {
        self.set_scalar(memory, S_USED_RECORDS, value.0)
    }

    fn set_retired_slot_count<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        value: PoolRetiredSlotCount,
    ) -> bool {
        self.set_cell_auxiliary(memory, RETIRED_SLOT_COUNT_CELL, value.0)
    }

    fn set_capacity<M: GuestMemory>(&mut self, memory: &mut M, value: PoolCapacity) -> bool {
        self.set_scalar(memory, S_CAPACITY, value.0)
    }
}
