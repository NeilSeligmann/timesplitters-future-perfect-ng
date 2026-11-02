// SPDX-License-Identifier: GPL-3.0-or-later
//! T2377 runtime adapter for the readable REA interaction/inventory owner.
//!
//! The owner sees typed records and stable IDs. This lane adapter snapshots the currently live
//! guest tables and translates its result back to the legacy ABI at the boundary. That snapshot
//! is a TEMPORARY-SHIM until T2044 moves these live records into Rust-owned state.

use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
    rea_interaction_inventory as owner,
};

const ENTRY_TABLE: u32 = 0x4fa128;
const RECORD_TABLE_POINTER: u32 = 0x4f9bac;
const ENTRY_STRIDE: u32 = 0x3c;
/// Weapon record index inside an entry row: lifted 0x1B99B0/0x1B99F0 read `entry * 0x3C + 0x4FA138`.
/// The row's `+0x0` word is the flags word (T3441: reading it faulted Story level 1 load).
const ENTRY_RECORD_INDEX_OFFSET: u32 = 0x10;

/// Address of the weapon record index of entry row `entry`.
const fn entry_record_index_address(entry: u32) -> u32 {
    ENTRY_TABLE
        .wrapping_add(entry.wrapping_mul(ENTRY_STRIDE))
        .wrapping_add(ENTRY_RECORD_INDEX_OFFSET)
}
const RECORD_STRIDE: u32 = 0x29c;
const AMMO_TABLE: u32 = 0x4ea3f8;
const AMMO_STRIDE: u32 = 0xe0;
const AMMO_DEFINITION_OFFSET: u32 = 0x8c;
static SWITCH_STATE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

#[cfg(test)]
std::thread_local! {
    static ENABLED_FOR_TEST: core::cell::Cell<Option<bool>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
fn set_enabled_for_test(enabled: Option<bool>) {
    ENABLED_FOR_TEST.with(|state| state.set(enabled));
}

/// Inventory stays opt-in while the r5 model-streaming fault is unresolved.
/// Set `TSFP_REA_INVENTORY_NATIVE=1` to select the Rust lane for investigation.
pub fn enabled() -> bool {
    #[cfg(test)]
    if let Some(enabled) = ENABLED_FOR_TEST.with(core::cell::Cell::get) {
        return enabled;
    }
    use core::sync::atomic::Ordering::Relaxed;
    if !crate::retired::lifted_selectable("inventory") {
        return true;
    }
    let cached = SWITCH_STATE.load(Relaxed);
    if cached != 0 {
        return cached == 2;
    }
    unsafe extern "C" {
        fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
    }
    let value = unsafe { getenv(c"TSFP_REA_INVENTORY_NATIVE".as_ptr()) };
    let enabled = if value.is_null() {
        enabled_value(None)
    } else {
        enabled_value(Some(unsafe { core::ffi::CStr::from_ptr(value) }.to_bytes()))
    };
    let state = if enabled { 2 } else { 1 };
    let _ = SWITCH_STATE.compare_exchange(0, state, Relaxed, Relaxed);
    SWITCH_STATE.load(Relaxed) == 2
}

fn enabled_value(value: Option<&[u8]>) -> bool {
    value == Some(b"1")
}

/// Runtime dispatch entry. Returns false for roots outside this bounded T2377 adoption slice.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !enabled() {
        return false;
    }
    run_enabled(va, memory, guest, regs)
}

pub fn run_enabled<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x00a0_d30 => {
            submit_interaction(memory, guest, regs);
            true
        }
        0x001b_99b0 => {
            ammo_type_lookup(memory, regs, owner::AmmoHand::Primary);
            true
        }
        0x001b_99f0 => {
            ammo_type_lookup(memory, regs, owner::AmmoHand::Secondary);
            true
        }
        0x000b_e860 if memory.read32(regs.esp.wrapping_add(4)) == 0x40 => {
            ammo_definition_lookup(memory, regs);
            true
        }
        _ => false,
    }
}

fn inventory_view<M: GuestMemory>(
    memory: &M,
) -> (
    [i32; owner::ENTRY_COUNT],
    [Option<owner::WeaponAmmoRecord>; 66],
) {
    let mut entry_records = [-1; owner::ENTRY_COUNT];
    for (entry, record_index) in entry_records.iter_mut().enumerate() {
        *record_index = memory.read32(entry_record_index_address(entry as u32)) as i32;
    }

    let records_base = memory.read32(RECORD_TABLE_POINTER);
    let mut records = [None; 66];
    for (index, record) in records.iter_mut().enumerate() {
        let base = records_base.wrapping_add((index as u32).wrapping_mul(RECORD_STRIDE));
        *record = Some(owner::WeaponAmmoRecord {
            primary: owner::AmmoPair {
                ammo_type: memory.read32(base.wrapping_add(0x14)),
                amount: memory.read32(base.wrapping_add(0x18)),
            },
            secondary: owner::AmmoPair {
                ammo_type: memory.read32(base.wrapping_add(0x1c)),
                amount: memory.read32(base.wrapping_add(0x20)),
            },
        });
    }
    (entry_records, records)
}

fn ammo_type_lookup<M: GuestMemory>(memory: &M, regs: &mut ExtRegisters, hand: owner::AmmoHand) {
    let entry_index = memory.read32(regs.esp.wrapping_add(4)) as i32;
    let record_index = if (0..owner::ENTRY_COUNT as i32).contains(&entry_index) {
        memory.read32(entry_record_index_address(entry_index as u32)) as i32
    } else {
        -1
    };
    if (0..66).contains(&record_index) {
        regs.ecx = memory.read32(RECORD_TABLE_POINTER);
    }

    let (entries, records) = inventory_view(memory);
    let view = owner::InventoryDefinitionView::new(&entries, &records, &[]);
    regs.eax = match hand {
        owner::AmmoHand::Primary => view.primary_ammo_type(entry_index),
        owner::AmmoHand::Secondary => view.secondary_ammo_type(entry_index),
    };
}

fn read_definition<M: GuestMemory>(memory: &M, address: u32) -> owner::GameObjectDefinition {
    let words = core::array::from_fn(|index| {
        memory.read32(address.wrapping_add((index as u32).wrapping_mul(4)))
    });
    owner::GameObjectDefinition::from_words(words)
}

fn ammo_definition_lookup<M: GuestMemory>(memory: &M, regs: &mut ExtRegisters) {
    let entry_index = memory.read32(regs.esp.wrapping_add(8)) as i32;
    let (entries, records) = inventory_view(memory);
    let view = owner::InventoryDefinitionView::new(&entries, &records, &[]);

    let mut ammo_rows = [None; owner::AMMO_TYPE_COUNT];
    for (ammo_type, row) in ammo_rows.iter_mut().enumerate().skip(1) {
        let row_address = AMMO_TABLE.wrapping_add((ammo_type as u32).wrapping_mul(AMMO_STRIDE));
        *row = Some(owner::AmmoDefinitionRow {
            reserve_limit: memory.read32(row_address) as i32,
            definition: read_definition(memory, row_address.wrapping_add(AMMO_DEFINITION_OFFSET)),
        });
    }
    let catalog = owner::DefinitionCatalog {
        ammo_rows: &ammo_rows,
        type_tables: &[],
        fixed_type_800: None,
    };

    // The owner selects the embedded definition. The adapter maps that successful selection to
    // the legacy definition address only because callers of 0xBE860 still consume an address.
    regs.eax = catalog
        .ammo_definition_for_entry(&view, entry_index, owner::AmmoHand::Primary)
        .map_or(0, |_| {
            let ammo_type = view.primary_ammo_type(entry_index);
            AMMO_TABLE
                .wrapping_add(ammo_type.wrapping_mul(AMMO_STRIDE))
                .wrapping_add(AMMO_DEFINITION_OFFSET)
        });
}

struct GuestInteraction<'a, M, C> {
    memory: &'a mut M,
    guest: &'a mut C,
    regs: &'a mut ExtRegisters,
}

impl<M: GuestMemory, C: GuestCall> owner::InteractionRuntime for GuestInteraction<'_, M, C> {
    fn capture_player(&self, player: owner::PlayerId) -> Option<owner::CapturedInteractionPlayer> {
        let record_base = self.memory.read32(0x7b0c48);
        let record = record_base.wrapping_add(player.0.wrapping_mul(0x1584));
        Some(owner::CapturedInteractionPlayer {
            record: owner::PlayerRecordId(record),
            actor: owner::ActorId(self.memory.read32(record.wrapping_add(0x14))),
        })
    }

    fn publish_target_reference(
        &mut self,
        player: owner::PlayerId,
        target: owner::InteractionTargetId,
    ) {
        let target_reference = self.memory.read32(target.0.wrapping_add(0x2c));
        call_cdecl(
            self.guest,
            self.memory,
            self.regs,
            0x35aa70,
            &[player.0, target_reference],
        );
    }

    fn callback_for_target(
        &mut self,
        target: owner::InteractionTargetId,
    ) -> Option<owner::InteractionCallbackId> {
        let callback = self.memory.read32(target.0.wrapping_add(0x154));
        self.regs.eax = callback;
        (callback != 0).then_some(owner::InteractionCallbackId(callback))
    }

    fn invoke_callback(
        &mut self,
        callback: owner::InteractionCallbackId,
        target: owner::InteractionTargetId,
        captured_actor: owner::ActorId,
    ) {
        call_cdecl(
            self.guest,
            self.memory,
            self.regs,
            callback.0,
            &[target.0, captured_actor.0],
        );
    }

    fn set_interacted_flag(&mut self, captured_record: owner::PlayerRecordId) {
        self.memory
            .write32(captured_record.0.wrapping_add(0x1570), 1);
    }

    fn mark_mission_entries(&mut self, target: owner::InteractionTargetId) {
        call_cdecl(self.guest, self.memory, self.regs, 0x2345d0, &[target.0]);
    }
}

fn submit_interaction<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let entry_esp = regs.esp;
    let player_index = memory.read32(entry_esp.wrapping_add(4));
    let target = memory.read32(entry_esp.wrapping_add(8));
    let saved = (regs.ebx, regs.esi, regs.edi);
    let mut runtime = GuestInteraction {
        memory,
        guest,
        regs,
    };
    let _ = owner::submit_interaction(
        &mut runtime,
        owner::PlayerId(player_index),
        owner::InteractionTargetId(target),
    );
    drop(runtime);
    (regs.ebx, regs.esi, regs.edi) = saved;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{collections::BTreeMap, rc::Rc};
    use core::cell::RefCell;

    extern crate alloc;

    #[derive(Clone, Default)]
    struct TestMemory(Rc<RefCell<BTreeMap<u32, u8>>>);

    impl TestMemory {
        fn put32(&self, address: u32, value: u32) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.0.borrow_mut().insert(address + offset as u32, byte);
            }
        }
        fn get32(&self, address: u32) -> u32 {
            u32::from_le_bytes(core::array::from_fn(|i| {
                *self.0.borrow().get(&(address + i as u32)).unwrap_or(&0)
            }))
        }
    }

    impl GuestMemory for TestMemory {
        fn read8(&self, address: u32) -> u8 {
            *self.0.borrow().get(&address).unwrap_or(&0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.borrow_mut().insert(address, value);
        }
        fn read32(&self, address: u32) -> u32 {
            self.get32(address)
        }
    }

    #[derive(Default)]
    struct TestGuest {
        memory: TestMemory,
        callback_target: Option<u32>,
        calls: alloc::vec::Vec<(u32, alloc::vec::Vec<u32>)>,
    }

    impl GuestCall for TestGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            let argc = if va == 0x35aa70 || va == 0x12345678 {
                2
            } else {
                1
            };
            let args = (0..argc)
                .map(|index| self.memory.get32(regs.esp.wrapping_add(index * 4)))
                .collect();
            self.calls.push((va, args));
            if va == 0x35aa70 {
                self.memory.put32(
                    self.callback_target.unwrap().wrapping_add(0x154),
                    0x12345678,
                );
            }
            regs.eax = match va {
                0x35aa70 => 0,
                0x12345678 => 0x55,
                0x2345d0 => 0x77,
                _ => 0,
            };
            regs.esp = regs.esp.wrapping_add(4);
        }
    }

    fn registers(esp: u32) -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp,
            ebx: 0xb,
            ebp: 0x55,
            esi: 0x66,
            edi: 0x77,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.; 8],
                top: 0,
                control: 0,
            },
            xmm: [crate::abi::Xmm::default(); 5],
            xmm_hi: [crate::abi::Xmm::default(); 3],
        }
    }

    struct InventorySwitchGuard;

    impl InventorySwitchGuard {
        fn enabled() -> Self {
            set_enabled_for_test(Some(true));
            Self
        }
    }

    impl Drop for InventorySwitchGuard {
        fn drop(&mut self) {
            set_enabled_for_test(None);
        }
    }

    #[test]
    fn primary_and_secondary_entry_lookups_use_typed_owner() {
        let _switch = InventorySwitchGuard::enabled();
        let mut memory = TestMemory::default();
        let record_base = 0x900000;
        memory.put32(RECORD_TABLE_POINTER, record_base);
        memory.put32(entry_record_index_address(5), 3);
        memory.put32(record_base + 3 * RECORD_STRIDE + 0x14, 7);
        memory.put32(record_base + 3 * RECORD_STRIDE + 0x1c, 9);
        let mut guest = TestGuest::default();
        let mut regs = registers(0x8000);
        memory.put32(0x8004, 5);

        assert!(crate::lane_inv::dispatch_inventory_owner(
            0x1b99b0,
            &mut memory,
            &mut guest,
            &mut regs
        ));
        assert_eq!(regs.eax, 7);
        assert_eq!(regs.ecx, record_base);
        assert!(crate::lane_inv::dispatch_inventory_owner(
            0x1b99f0,
            &mut memory,
            &mut guest,
            &mut regs
        ));
        assert_eq!(regs.eax, 9);
    }

    #[test]
    fn ammo_definition_lookup_uses_typed_catalog_and_keeps_legacy_row_abi() {
        let mut memory = TestMemory::default();
        let record_base = 0x900000;
        memory.put32(RECORD_TABLE_POINTER, record_base);
        memory.put32(entry_record_index_address(5), 3);
        memory.put32(record_base + 3 * RECORD_STRIDE + 0x14, 7);
        let row = AMMO_TABLE + 7 * AMMO_STRIDE;
        memory.put32(row, 180);
        memory.put32(row + AMMO_DEFINITION_OFFSET, 0x2468);
        let mut guest = TestGuest::default();
        let mut regs = registers(0x8000);
        memory.put32(0x8004, 0x40);
        memory.put32(0x8008, 5);

        assert!(run_enabled(0xbe860, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, row + AMMO_DEFINITION_OFFSET);
    }

    /// T3441 regression: the record index is the row's `+0x10` word (lifted `0x4FA138`), not the
    /// `+0x0` flags word. Reading the flags made `0xBE860(0x40, entry)` return 0 and the lifted
    /// `0xBE9E0` scale getter faulted at guest 0x20 while Story level 1 loaded.
    #[test]
    fn entry_record_index_is_row_word_0x10_not_flags() {
        let mut memory = TestMemory::default();
        let record_base = 0x900000;
        memory.put32(RECORD_TABLE_POINTER, record_base);
        assert_eq!(entry_record_index_address(5), 0x4fa138 + 5 * 0x3c);
        memory.put32(ENTRY_TABLE + 5 * ENTRY_STRIDE, 0x4004);
        memory.put32(entry_record_index_address(5), 3);
        memory.put32(record_base + 3 * RECORD_STRIDE + 0x14, 7);
        let row = AMMO_TABLE + 7 * AMMO_STRIDE;
        memory.put32(row, 180);
        let mut guest = TestGuest::default();
        let mut regs = registers(0x8000);
        memory.put32(0x8004, 5);
        assert!(run_enabled(0x1b99b0, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, 7);
        memory.put32(0x8004, 0x40);
        memory.put32(0x8008, 5);
        assert!(run_enabled(0xbe860, &mut memory, &mut guest, &mut regs));
        assert_eq!(regs.eax, row + AMMO_DEFINITION_OFFSET);
    }

    #[test]
    fn interaction_runtime_preserves_capture_send_callback_mark_order() {
        let _switch = InventorySwitchGuard::enabled();
        let mut memory = TestMemory::default();
        let mut guest = TestGuest::default();
        let record_table = 0x700000;
        let record = record_table + 2 * 0x1584;
        let target = 0x900000;
        guest.memory = memory.clone();
        guest.callback_target = Some(target);
        memory.put32(0x7b0c48, record_table);
        memory.put32(record + 0x14, 0xaabbccdd);
        memory.put32(target + 0x2c, 0x99887766);
        memory.put32(target + 0x154, 0x11111111);
        let mut regs = registers(0x8000);
        memory.put32(0x8004, 2);
        memory.put32(0x8008, target);

        assert!(crate::weapons_lane::PLAYER_ROOTS.contains(&0xa0d30));
        assert!(crate::lane_inv::dispatch_inventory_owner(
            0xa0d30,
            &mut memory,
            &mut guest,
            &mut regs
        ));

        assert_eq!(
            guest.calls,
            [
                (0x35aa70, alloc::vec![2, 0x99887766]),
                (0x12345678, alloc::vec![target, 0xaabbccdd]),
                (0x2345d0, alloc::vec![target]),
            ]
        );
        assert_eq!(memory.get32(record + 0x1570), 1);
        assert_eq!(regs.eax, 0x77);
        assert_eq!((regs.ebx, regs.esi, regs.edi), (0xb, 0x66, 0x77));
        assert_eq!(regs.esp, 0x8000);
    }

    #[test]
    fn switch_is_explicit_opt_in_and_allows_lifted_ab_fallback() {
        assert!(!enabled_value(None));
        assert!(!enabled_value(Some(b"0")));
        assert!(enabled_value(Some(b"1")));
        assert!(!enabled_value(Some(b"true")));
    }
}
