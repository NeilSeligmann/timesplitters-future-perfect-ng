use std::cell::RefCell;
use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, GuestCall};
use tsfp_ports::inventory_ammo::ammo_unload_clip_back_into_reserve_both_types;
use tsfp_ports::memory::GuestMemory;

#[derive(Default)]
struct Mem { bytes: BTreeMap<u32, u8>, accesses: RefCell<Vec<(char, u32)>> }
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 { *self.bytes.get(&a).unwrap_or(&0) }
    fn write8(&mut self, a: u32, v: u8) { self.bytes.insert(a, v); }
    fn read16(&self, a: u32) -> u16 { self.accesses.borrow_mut().push(('h', a)); u16::from_le_bytes([self.read8(a), self.read8(a+1)]) }
    fn read32(&self, a: u32) -> u32 { self.accesses.borrow_mut().push(('r', a)); u32::from_le_bytes(std::array::from_fn(|i| self.read8(a+i as u32))) }
    fn write32(&mut self, a: u32, v: u32) { self.accesses.borrow_mut().push(('W', a)); for (i,b) in v.to_le_bytes().into_iter().enumerate() { self.write8(a+i as u32,b); } }
}
impl Mem { fn seed16(&mut self,a:u32,v:u16) { for (i,b) in v.to_le_bytes().into_iter().enumerate() {self.write8(a+i as u32,b)} } }
struct NoCalls;
impl GuestCall for NoCalls { fn call(&mut self, _: u32, _: &mut ExtRegisters) { panic!("guest call") } }
#[test]
fn unload_both_types_preserves_accesses_and_registers() {
    let mut m=Mem::default();
    m.write32(0x1008,0x2000); m.write32(0x1004,0x3000); m.write32(0x2000+0x30,2);
    m.write32(0x4f9bac,0x5000); m.write32(0x5000+2*0x29c+0x14,3); m.write32(0x5000+2*0x29c+0x1c,4);
    for (t,clip,reserve) in [(3,7u16,10u16),(4,9,20)] {m.seed16(0x2000+0x38+t*2,clip);m.seed16(0x3000+0x61e+t*2,reserve);}
    m.accesses.borrow_mut().clear();
    let mut regs:ExtRegisters=unsafe {std::mem::zeroed()};regs.esp=0x1000;regs.esi=0x9876;regs.ebx=0xabcd;regs.eax=0x1234;
    ammo_unload_clip_back_into_reserve_both_types(&mut m,&mut NoCalls,&mut regs);
    assert_eq!(regs.esp,0x1000);assert_eq!(regs.esi,0x9876);assert_eq!(regs.ebx,0xabcd);
    assert_eq!(m.read16(0x3000+0x61e+6),17);assert_eq!(m.read16(0x3000+0x61e+8),29);
    assert_eq!(m.read16(0x2000+0x38+6),0);assert_eq!(m.read16(0x2000+0x38+8),0);
}
