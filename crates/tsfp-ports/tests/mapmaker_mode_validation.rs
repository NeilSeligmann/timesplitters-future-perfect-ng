use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::memory::GuestMemory;

#[unsafe(no_mangle)]
extern "C" fn xbox_ReadTimeStampCounter() -> u64 { 0 }

#[derive(Default)]
struct Memory(BTreeMap<u32, u32>);
impl GuestMemory for Memory {
    fn read8(&self, _: u32) -> u8 { panic!("unexpected byte read") }
    fn write8(&mut self, _: u32, _: u8) { panic!("unexpected byte write") }
    fn read32(&self, a: u32) -> u32 { *self.0.get(&a).unwrap_or(&0) }
    fn write32(&mut self, a: u32, value: u32) { self.0.insert(a, value); }
}

struct Validator { result: u32, calls: Vec<(u32, u32, u32)> }
impl GuestCall for Validator {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.calls.push((va, r.esp, r.ebp));
        r.eax = self.result;
        r.ebp = 0xdead_beef; // The guest-call adapter restores the caller frame.
    }
}

fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 7, ecx: 8, edx: 9, esp: 0x9000, ebx: 10, ebp: 11, esi: 12, edi: 13,
        eflags: 0x202,
        fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x27f },
        xmm: [Xmm::default(); 5], xmm_hi: [Xmm::default(); 3],
    }
}

#[test]
fn validation_callbacks_keep_guest_call_and_register_contract() {
    for (va, mode) in [(0x3127d0, 0), (0x3127f0, 1)] {
        for result in [0, 1, 0x8000_0000, u32::MAX] {
            let mut memory = Memory::default();
            let mut guest = Validator { result, calls: Vec::new() };
            let mut r = regs();
            assert!(tsfp_ports::c2r::run(va, &mut memory, &mut guest, &mut r));
            assert_eq!(guest.calls, vec![(0x3265a0, 0x8ffc, 11)]);
            assert_eq!(memory.0.get(&0x8ffc), Some(&mode));
            assert_eq!(r.eax, u32::from(result != 0));
            assert_eq!(r.esp, 0x9000);
            assert_eq!(r.ebp, 11);
            assert_eq!((r.ecx, r.edx, r.ebx, r.esi, r.edi, r.eflags), (8, 9, 10, 12, 13, 0x202));
        }
    }
}
