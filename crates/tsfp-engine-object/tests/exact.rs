// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic access-order controls derived from the unchanged C exact bodies.
use std::{cell::RefCell, collections::BTreeMap};
use tsfp_engine_object::{ObjectMemory, exact::{self, Registers}};
#[derive(Default)]
struct Memory {
    bytes: BTreeMap<u32, u8>,
    reads: RefCell<Vec<(u32, u32)>>,
}
impl ObjectMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        self.reads.borrow_mut().push((a, 1));
        *self.bytes.get(&a).expect("unexpected read")
    }
    fn write8(&mut self, a: u32, v: u8) { self.bytes.insert(a, v); }
    fn read16(&self, a: u32) -> u16 {
        self.reads.borrow_mut().push((a, 2));
        u16::from_le_bytes(core::array::from_fn(|i| self.bytes[&a.wrapping_add(i as u32)]))
    }
    fn read32(&self, a: u32) -> u32 {
        self.reads.borrow_mut().push((a, 4));
        u32::from_le_bytes(core::array::from_fn(|i| self.bytes[&a.wrapping_add(i as u32)]))
    }
}
fn regs() -> Registers { Registers { eax: 0x11, ecx: 0x22, edx: 0x33 } }
#[test]
fn null_paths_skip_third_argument_and_keep_branch_residue() {
    for flag in [0, 0x10] {
        let mut m = Memory::default();
        m.write32(4, 0x100); m.write32(8, 0x200);
        m.write8(0x10c, flag); m.write32(0x204, 0);
        // Neither arg2 nor key exists: an eager read would panic.
        let mut r = regs();
        exact::indexed_field_pointer(&m, 0, &mut r);
        let expected = if flag == 0 { Registers { eax: 0, ecx: 0x22, edx: 0x200 } }
            else { Registers { eax: 0, ecx: 0x200, edx: 0x33 } };
        assert_eq!(r, expected);
        assert_eq!(*m.reads.borrow(), [(4,4), (0x10c,1), (8,4), (0x204,4)]);
    }
}
#[test]
fn full_unsigned_index_wraps_result_and_preserves_order() {
    for flag in [0, 0x10] {
        let mut m = Memory::default();
        m.write32(4, 0x100); m.write32(8, 0x200); m.write32(12, 0x300);
        m.write8(0x10c, flag); m.write32(0x204, 0xfffffff8);
        m.write8(0x306, 0xff); m.write8(0x307, 0xff);
        let mut r = regs(); exact::indexed_field_pointer(&m, 0, &mut r);
        let expected = if flag == 0 { Registers { eax: 0xffff4, ecx: 0x300, edx: 0xffff0 } }
            else { Registers { eax: 0x7fff2, ecx: 0xffff, edx: 0x300 } };
        assert_eq!(r, expected);
        assert_eq!(*m.reads.borrow(), [(4,4), (0x10c,1), (8,4), (0x204,4), (12,4), (0x306,2)]);
    }
}
#[test]
fn mode_words_precede_masks_and_addresses_wrap() {
    for mode in [10, 0, 255] {
        let mut m = Memory::default();
        let offset = if mode == 10 { 0x10 } else { 0x18 };
        // Object offset wraps into the stack, deliberately aliasing selected words/masks.
        let object = 8u32.wrapping_sub(offset);
        m.write8(0x7de455, mode); m.write32(4, object);
        m.write32(8, 0x80000000); m.write32(12, 0x40000000);
        let mut r = regs(); let g = tsfp_engine_object::object_leaves::mode_selected_flag_pair_intersects(
            &m, 0, tsfp_engine_object::gprs::Gprs { eax: r.eax, ecx: r.ecx, edx: r.edx });
        r = Registers { eax: g.eax, ecx: g.ecx, edx: g.edx };
        assert_eq!(r, Registers { eax: 1, ecx: 0x40000000, edx: 0x33 });
        assert_eq!(*m.reads.borrow(), [(0x7de455,1), (4,4), (8,4), (12,4), (8,4), (12,4)]);
    }
}
