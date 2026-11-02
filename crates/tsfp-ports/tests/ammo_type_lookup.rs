use std::cell::RefCell;
use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, GuestCall};
use tsfp_ports::inventory_ammo::loadout_entry_primary_ammo_type_by_entry_index;
use tsfp_ports::memory::GuestMemory;

#[derive(Default)]
struct Mem { words: BTreeMap<u32, u32>, reads: RefCell<Vec<u32>> }
impl GuestMemory for Mem {
    fn read8(&self, _: u32) -> u8 { panic!("unexpected byte read") }
    fn write8(&mut self, _: u32, _: u8) { panic!("unexpected byte write") }
    fn read32(&self, a: u32) -> u32 { self.reads.borrow_mut().push(a); *self.words.get(&a).unwrap_or(&0) }
}
struct NoCalls;
impl GuestCall for NoCalls { fn call(&mut self, a: u32, _: &mut ExtRegisters) { panic!("unexpected call {a:x}") } }

#[test]
fn valid_and_invalid_indices_keep_guest_contract() {
    for (index, record, expected, reads, ecx) in [
        (0u32, 0u32, 0x55u32, vec![0x1004, 0x4fa138, 0x4f9bac, 0x700014], 0x700000),
        (0x45, 0x41, 0x55, vec![0x1004, 0x4fa138 + 0x45 * 0x3c, 0x4f9bac, 0x700000 + 0x41 * 0x29c + 0x14], 0x700000),
        (0x46, 0, 0, vec![0x1004], 0x1234),
        (u32::MAX, 0, 0, vec![0x1004], 0x1234),
        (1, 0x42, 0, vec![0x1004, 0x4fa138 + 0x3c], 0x1234),
        (1, u32::MAX, 0, vec![0x1004, 0x4fa138 + 0x3c], 0x1234),
    ] {
        let mut m = Mem::default();
        m.words.insert(0x1004, index);
        m.words.insert(0x4fa138u32.wrapping_add(index.wrapping_mul(0x3c)), record);
        m.words.insert(0x4f9bac, 0x700000);
        m.words.insert(0x700000u32.wrapping_add(record.wrapping_mul(0x29c)).wrapping_add(0x14), 0x55);
        let mut regs: ExtRegisters = unsafe { std::mem::zeroed() };
        regs.esp = 0x1000;
        regs.ecx = 0x1234;
        regs.edx = 0xabcd;
        let mut guest = NoCalls;
        loadout_entry_primary_ammo_type_by_entry_index(&mut m, &mut guest, &mut regs);
        assert_eq!(regs.eax, expected, "index {index:x}, record {record:x}");
        assert_eq!(regs.ecx, ecx);
        assert_eq!(regs.edx, 0xabcd);
        assert_eq!(&*m.reads.borrow(), &reads);
    }
}
