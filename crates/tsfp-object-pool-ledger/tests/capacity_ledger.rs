// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures; no retail data.
use std::collections::BTreeMap;
use tsfp_object_pool_ledger::{
    ObjectPoolCapacityLedger, PoolCapacity, PoolCapacityLedgerState, PoolRetiredSlotCount,
    PoolSlotCount, RETIRED_SLOT_COUNT_ADDRESS, RETIRED_SLOT_COUNT_CELL,
};
use tsfp_state::{GuestMemory, object_pool::*};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Image(BTreeMap<u32, u8>);

impl GuestMemory for Image {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

#[test]
fn measured_capacity_ledger_aliases_retired_count_with_aux_214() {
    let mut memory = Image::default();
    let mut pool = ObjectPool::empty_limited(0);
    assert_eq!(
        cell_addr(AUX, RETIRED_SLOT_COUNT_CELL),
        RETIRED_SLOT_COUNT_ADDRESS
    );

    assert!(pool.set_slot_count(&mut memory, PoolSlotCount(19)));
    assert!(pool.set_capacity(&mut memory, PoolCapacity(48)));
    assert!(pool.set_cell_auxiliary(&mut memory, RETIRED_SLOT_COUNT_CELL - 1, 0xABCD));
    let before = pool.clone();

    assert!(pool.set_retired_slot_count(&mut memory, PoolRetiredSlotCount(7)));

    assert_eq!(
        pool.capacity_ledger_state(),
        PoolCapacityLedgerState {
            slot_count: PoolSlotCount(19),
            retired_slot_count: PoolRetiredSlotCount(7),
            capacity: PoolCapacity(48),
        }
    );
    assert_eq!(
        pool.cell_state(RETIRED_SLOT_COUNT_CELL).unwrap().auxiliary,
        7
    );
    assert_eq!(pool.tables[AUX][RETIRED_SLOT_COUNT_CELL - 1], 0xABCD);
    assert_eq!(memory.read32(RETIRED_SLOT_COUNT_ADDRESS), 7);
    assert_eq!(
        memory.read32(cell_addr(AUX, RETIRED_SLOT_COUNT_CELL - 1)),
        0xABCD
    );

    let mut expected = before;
    expected.tables[AUX][RETIRED_SLOT_COUNT_CELL] = 7;
    assert_eq!(pool, expected);
    assert_eq!(pool.diff(&memory), None);
}
