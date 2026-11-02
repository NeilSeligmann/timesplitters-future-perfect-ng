//! Adapter consistency check over synthetic aliasing/wrapping memory.
//! The pilot now shares these bodies; independent original evidence is in
//! docs/t1835-rust-wave2.md, not in this round-trip check.
use tsfp_engine_object as new;
// tsfp-ports is no_std with a panic handler, so include the proven sources directly.
// The trait text (minus the unsafe MappedGuest) is copied, as the workspace forbids unsafe.
mod memory {
    pub trait GuestMemory {
        fn read8(&self, address: u32) -> u8;
        fn write8(&mut self, address: u32, value: u8);
        fn read16(&self, address: u32) -> u16 {
            u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
        }
        fn read32(&self, address: u32) -> u32 {
            u32::from_le_bytes(core::array::from_fn(|i| self.read8(address.wrapping_add(i as u32))))
        }
        fn write32(&mut self, address: u32, value: u32) {
            for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.write8(address.wrapping_add(i as u32), byte);
            }
        }
    }
}
#[path = "../../tsfp-ports/src/behaviour.rs"]
#[allow(dead_code)]
mod behaviour_src;
use behaviour_src as old;
use memory::GuestMemory;

const SIZE: usize = 0x400;
#[derive(Clone, PartialEq, Debug)]
struct Mem(Vec<u8>);
fn idx(a: u32) -> usize { a as usize % SIZE }
impl Mem {
    fn byte(&self, address: u32) -> u8 { self.0[idx(address)] }
    fn put_byte(&mut self, address: u32, value: u8) { self.0[idx(address)] = value; }
}
impl new::ObjectMemory for Mem {
    fn read8(&self, a: u32) -> u8 { self.byte(a) }
    fn write8(&mut self, a: u32, v: u8) { self.put_byte(a, v) }
}
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 { self.byte(a) }
    fn write8(&mut self, a: u32, v: u8) { self.put_byte(a, v) }
}
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}
fn pick(r: &mut Rng) -> u32 {
    match r.next() % 4 { 0 => r.next() % SIZE as u32, 1 => 0u32.wrapping_sub(r.next() % 8), 2 => 0x110u32.wrapping_add(r.next() % 3), _ => r.next() }
}

#[test]
fn adapter_roundtrip() {
    let mut r = Rng(0x2026_1010_1835);
    let mut cases = 0u32;
    for _ in 0..20000 {
        let mut a = Mem((0..SIZE).map(|_| r.next() as u8).collect());
        if r.next() % 3 == 0 { for o in [0x114usize, 0x28, 0x0c] { a.0[o] = if o == 0x114 { 0x10 } else { a.0[o] }; } if r.next() % 2 == 0 { a.0[0x115] = 0; a.0[0x116] = 0; a.0[0x117] = 0; } else { a.0[0x114] = r.next() as u8; } }
        let (o, t, k, arg, e) = (pick(&mut r), pick(&mut r), pick(&mut r), pick(&mut r), r.next() & 1);
        let (mut b, mut c) = (a.clone(), a.clone());
        assert_eq!(new::table_entry_address(&a, o, t, k), old::table_entry(&a, o, t, k));
        assert_eq!(new::state_allowed(&a, o), old::state_allowed(&a, o));
        assert_eq!(new::set_enabled(&mut b, o, e), old::set_enabled(&mut c, o, e));
        assert_eq!(b, c);
        assert_eq!(new::request_state(&mut b, o, arg), old::request_state(&mut c, o, arg));
        assert_eq!(b, c);
        cases += 1;
    }
    assert_eq!(cases, 20000);
}

#[test]
fn request_state_alias_reads_after_store() {
    let mut m = Mem(vec![0; SIZE]);
    m.0[0x114] = 5;
    // argument aliases O+0x110: must observe the previous-state store (5).
    new::request_state(&mut m, 0, 0x110);
    assert_eq!(new::ObjectMemory::read32(&m, 0x114), 5);
}

#[test]
fn mutant_is_detected() {
    // control: a body differing in the shift must disagree with the proven one.
    let mut r = Rng(7);
    let m = Mem((0..SIZE).map(|_| r.next() as u8).collect());
    let mutant = |obj: u32, table: u32, key: u32| {
        let shift = if new::ObjectMemory::read8(&m, obj + 0x0c) & 0x10 != 0 { 4 } else { 3 };
        new::ObjectMemory::read32(&m, table + 4).wrapping_add(u32::from(new::ObjectMemory::read16(&m, key + 6)) << shift)
    };
    let diffs = (0..200u32).filter(|i| mutant(*i, 8, 16 + i % 8) != old::table_entry(&m, *i, 8, 16 + i % 8)).count();
    assert!(diffs > 0);
}
