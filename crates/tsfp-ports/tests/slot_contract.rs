use std::cell::RefCell;
use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::inventory_slots::{
    get_weapon_slot_4fa128_float_0x30_by_object_cu, get_weapon_slot_4fa128_float_0x34_by_object_cu,
    slot_order_4fc888_find_index_whose_slot_field,
};
use tsfp_ports::memory::GuestMemory;
#[derive(Default)]
struct Mem {
    words: BTreeMap<u32, u32>,
    reads: RefCell<Vec<u32>>,
}
impl GuestMemory for Mem {
    fn read8(&self, _: u32) -> u8 {
        panic!("byte read")
    }
    fn write8(&mut self, _: u32, _: u8) {
        panic!("byte write")
    }
    fn read32(&self, a: u32) -> u32 {
        self.reads.borrow_mut().push(a);
        *self.words.get(&a).unwrap_or(&0)
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x9000,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
#[test]
fn slot_float_reads_and_eax() {
    for (offset, default, run) in [
        (
            0x34,
            0x47e2b0,
            get_weapon_slot_4fa128_float_0x34_by_object_cu as fn(&mut Mem, &mut ExtRegisters),
        ),
        (
            0x30,
            0x475cac,
            get_weapon_slot_4fa128_float_0x30_by_object_cu as fn(&mut Mem, &mut ExtRegisters),
        ),
    ] {
        for slot in [0, 2, u32::MAX] {
            let mut m = Mem::default();
            let mut r = regs();
            let object = 0x4000;
            m.words.insert(0x9004, object);
            m.words.insert(object + 0x94, slot);
            let scaled = slot.wrapping_mul(0x3c);
            let addr = if slot == 0 {
                default
            } else {
                0x4fa128u32.wrapping_add(offset).wrapping_add(scaled)
            };
            m.words.insert(addr, 1.25f32.to_bits());
            run(&mut m, &mut r);
            assert_eq!(r.fpu.st(0), 1.25);
            assert_eq!(r.eax, if slot == 0 { 0 } else { scaled });
            assert_eq!(&*m.reads.borrow(), &[0x9004, object + 0x94, addr]);
        }
    }
}

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, addr: u32, _: &mut ExtRegisters) {
        panic!("unexpected call {addr:x}");
    }
}

#[test]
fn slot_order_reads_and_register_contract() {
    for match_at in [Some(0u32), Some(2), Some(0x45), None] {
        let mut m = Mem::default();
        let mut r = regs();
        r.ecx = 0x1234;
        r.eax = 0xdead;
        r.edx = 0xbeef;
        r.eflags = 0x202;
        let mut expected_reads = Vec::new();
        let last = match_at.unwrap_or(0x45);
        for index in 0..=last {
            let order = 0x4fc888 + index * 4;
            let weapon = index + 3;
            let group = 0x4fa128 + weapon * 0x3c + 0x38;
            m.words.insert(order, weapon);
            m.words
                .insert(group, if match_at == Some(index) { 0x1234 } else { 0 });
            expected_reads.extend_from_slice(&[order, group]);
        }
        if match_at.is_some() {
            expected_reads.push(0x4fc888 + last * 4);
        }
        slot_order_4fc888_find_index_whose_slot_field(&mut m, &mut NoCalls, &mut r);
        assert_eq!(r.eax, match_at.map_or(u32::MAX, |i| i + 3));
        assert_eq!(r.edx, (last + 3) * 0x3c);
        assert_eq!(r.ecx, 0x1234);
        assert_eq!(r.eflags, 0x202);
        assert_eq!(&*m.reads.borrow(), &expected_reads);
    }
}

#[derive(Default)]
struct OwnedMem {
    words: BTreeMap<u32, u32>,
    bytes: BTreeMap<u32, u8>,
    reads: RefCell<Vec<(u8, u32)>>,
}
impl GuestMemory for OwnedMem {
    fn read8(&self, a: u32) -> u8 {
        self.reads.borrow_mut().push((8, a));
        *self.bytes.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, _: u32, _: u8) {
        panic!("byte write")
    }
    fn read32(&self, a: u32) -> u32 {
        self.reads.borrow_mut().push((32, a));
        *self.words.get(&a).unwrap_or(&0)
    }
    fn write32(&mut self, a: u32, value: u32) {
        self.words.insert(a, value);
    }
}

#[test]
fn next_owned_slot_wraps_and_preserves_guest_registers() {
    use tsfp_ports::inventory_slots::slot_order_4fc888_next_owned_slot_with_flag_0x;
    for (current, selected, expected_index) in
        [(2u32, 3u32, 3u32), (0x45, 0, 0), (99, 1, 1), (2, 99, 2)]
    {
        let mut m = OwnedMem::default();
        let mut r = regs();
        r.eax = 0xaabb;
        r.ecx = 0xccdd;
        r.edx = 0xeeff;
        r.esi = 0x1111;
        r.edi = 0x2222;
        r.eflags = 0x202;
        m.words.insert(0x9008, current);
        m.words.insert(0x9004, 0x4000);
        for i in 0..0x46u32 {
            m.words.insert(0x4fc888 + i * 4, i);
        }
        m.bytes.insert(0x4fa128 + selected * 0x3c, 0x10);
        m.bytes.insert(0x4000 + 0x54c + selected, 1);
        slot_order_4fc888_next_owned_slot_with_flag_0x(&mut m, &mut NoCalls, &mut r);
        assert_eq!(r.eax, expected_index);
        assert_eq!(r.ecx, expected_index);
        assert_eq!(r.edx, if current < 0x46 { current } else { 0 });
        assert_eq!(
            (r.esp, r.esi, r.edi, r.ebx, r.eflags),
            (0x9000, 0x1111, 0x2222, 0, 0x202)
        );
        assert!(m.reads.borrow().iter().any(|&(width, _)| width == 8));
    }
}
