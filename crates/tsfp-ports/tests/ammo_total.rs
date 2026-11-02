use std::cell::RefCell;
use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, GuestCall};
use tsfp_ports::inventory_ammo::ammo_total_across_three_word_arrays_0x61e_0xdc;
use tsfp_ports::memory::GuestMemory;

#[derive(Default)]
struct Mem { words: BTreeMap<u32, u32>, halves: BTreeMap<u32, u16>, reads: RefCell<Vec<u32>> }
impl GuestMemory for Mem {
    fn read8(&self, _: u32) -> u8 { panic!("unexpected byte read") }
    fn write8(&mut self, _: u32, _: u8) { panic!("unexpected byte write") }
    fn read16(&self, a: u32) -> u16 { self.reads.borrow_mut().push(a); *self.halves.get(&a).unwrap_or(&0) }
    fn read32(&self, a: u32) -> u32 { self.reads.borrow_mut().push(a); *self.words.get(&a).unwrap_or(&0) }
    fn write32(&mut self, a: u32, v: u32) { self.words.insert(a, v); }
}
struct NoCalls;
impl GuestCall for NoCalls { fn call(&mut self, a: u32, _: &mut ExtRegisters) { panic!("unexpected call {a:x}") } }

#[test]
fn ammo_total_preserves_signed_sum_and_read_order() {
    let player = 0x8000u32;
    let base = 0x700000u32;
    for (index, record, kind, flags, expected, reads, ecx, edx) in [
        (0u32, 0u32, 3u32, (true, true), 2u32, vec![0x1008, 0x4fa138, 0x4f9bac, base + 0x14, 0x1004, player + 0x624, player + 0xd4, player + 0xe2, player + 0x204, 0x0ffc, player + 0x212], player, 2),
        (0x45, 0x41, 3, (false, false), 0xffff_fffe, vec![0x1008, 0x4fa138 + 0x45 * 0x3c, 0x4f9bac, base + 0x41 * 0x29c + 0x14, 0x1004, player + 0x624, player + 0xd4, player + 0x204, 0x0ffc], player, 0xffff_fffe),
        (0x46, 0, 3, (true, true), 0, vec![0x1008], 0x1234, 0xabcd),
        (u32::MAX, 0, 3, (true, true), 0, vec![0x1008], 0x1234, 0xabcd),
        (0, 0x42, 3, (true, true), 0, vec![0x1008, 0x4fa138], 0x1234, 0xabcd),
        (0, 0, 0, (true, true), 0, vec![0x1008, 0x4fa138, 0x4f9bac, base + 0x14], base, 0xabcd),
    ] {
        let mut m = Mem::default();
        m.words.extend([(0x1008, index), (0x1004, player), (0x4fa138u32.wrapping_add(index.wrapping_mul(0x3c)), record), (0x4f9bac, base), (base.wrapping_add(record.wrapping_mul(0x29c)).wrapping_add(0x14), kind), (player + 0xd4, if flags.0 { 0 } else { u32::MAX }), (player + 0x204, if flags.1 { 0 } else { u32::MAX })]);
        m.halves.extend([(player + 0x624, 0xfffe), (player + 0xe2, 5), (player + 0x212, 0xffff)]);
        let mut regs: ExtRegisters = unsafe { std::mem::zeroed() };
        regs.esp = 0x1000;
        regs.ecx = 0x1234;
        regs.edx = 0xabcd;
        regs.edi = 0x5678;
        ammo_total_across_three_word_arrays_0x61e_0xdc(&mut m, &mut NoCalls, &mut regs);
        assert_eq!(regs.eax, expected);
        assert_eq!(regs.ecx, ecx);
        assert_eq!(regs.edx, edx);
        assert_eq!(regs.edi, 0x5678);
        assert_eq!(regs.esp, 0x1000);
        assert_eq!(&*m.reads.borrow(), &reads);
    }
}
