// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED semantic fixtures; bulk lifted differential is the Tier 1 evidence.
use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::math_slice::dispatch;
use tsfp_ports::memory::GuestMemory;
#[link(name = "m")]
unsafe extern "C" {}
#[derive(Default, Clone, PartialEq, Debug)]
struct Memory(BTreeMap<u32, u8>);
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 { *self.0.get(&a).unwrap_or(&0) }
    fn write8(&mut self, a: u32, v: u8) { self.0.insert(a, v); }
}
struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _: &mut ExtRegisters) { panic!("unexpected callee {va:x}"); }
}
fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 1, ecx: 2, edx: 3, ebx: 4, ebp: 5, esi: 6, edi: 7,
        esp: 0x8000, eflags: 0,
        fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x27f },
        xmm: [Xmm::default(); 5], xmm_hi: [Xmm::default(); 3],
    }
}
fn fixture() -> (Memory, ExtRegisters) {
    let mut m = Memory::default(); let r = registers();
    m.write32(r.esp + 4, 0x1000); m.write32(r.esp + 8, 0x2000);
    // Nonuniform scale and translation distinguish point and direction semantics.
    for (i, f) in [2.0f32,0.0,0.0,0.0, 0.0,3.0,0.0,0.0,
                   0.0,0.0,4.0,0.0, 10.0,20.0,30.0,1.0].iter().enumerate() {
        m.write32(0x1000 + i as u32 * 4, f.to_bits());
    }
    for (i, f) in [1.0f32,2.0,3.0].iter().enumerate() {
        m.write32(0x2000 + i as u32 * 4, f.to_bits());
    }
    (m, r)
}
#[test]
fn point_and_direction_share_matrix_but_translation_only_affects_point() {
    for (va, expected) in [(0x151e80, [12.0f32,26.0,42.0]), (0x152090,[2.0,6.0,12.0])] {
        let (mut m, mut r) = fixture();
        assert!(dispatch(true, va, &mut m, &mut NoCalls, &mut r));
        for (i, f) in expected.iter().enumerate() {
            assert_eq!(f32::from_bits(m.read32(0x2000+i as u32*4)), *f);
        }
    }
}
#[test]
fn disabled_and_unknown_addresses_preserve_original_entry_state() {
    for (enabled, va) in [(false,0x151e80), (true,0xffff_ffff)] {
        let (mut m, mut r) = fixture(); let before = m.clone(); let regs = r;
        assert!(!dispatch(enabled, va, &mut m, &mut NoCalls, &mut r));
        assert_eq!(m, before); assert_eq!(r, regs);
    }
}
