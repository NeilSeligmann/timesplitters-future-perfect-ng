// SPDX-License-Identifier: GPL-3.0-or-later
// T1831. FABRICATED synthetic fixtures with a scripted callee closure. Behavioural checks
// derived from the lifted originals; the original-vs-Rust equivalence is tests/c/test_rust_weapons.c.
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::collections::BTreeMap;
use tsfp_port_runtime::{abi, memory};
use tsfp_ports::weapons_slice;
#[derive(Default)]
struct Mem(BTreeMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
type Seen = (u32, [u32; 6], Vec<u32>);
#[derive(Default)]
struct Guest {
    seen: Vec<Seen>,
    results: Vec<u32>,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let m_args = vec![r.esp];
        self.seen
            .push((va, [r.eax, r.ecx, r.edx, r.esi, r.edi, r.esp], m_args));
        r.eax = if self.results.is_empty() {
            0
        } else {
            self.results.remove(0)
        };
        r.ecx = 0xc0c0;
        r.edx = 0xd0d0;
    }
}
const STACK: u32 = 0x9000;
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 1,
        ecx: 2,
        edx: 3,
        esp: STACK,
        ebx: 0xb,
        ebp: 0xe,
        esi: 0x51,
        edi: 0xd1,
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
fn with_args(m: &mut Mem, args: &[u32]) {
    m.write32(STACK, 0x1234);
    for (i, a) in args.iter().enumerate() {
        m.write32(STACK + 4 + 4 * i as u32, *a);
    }
}

#[test]
fn switch_and_leaf_contracts() {
    let mut m = Mem::default();
    let mut c = Guest::default();
    let mut r = regs();
    let before = r;
    for va in weapons_slice::ROOTS {
        assert!(!weapons_slice::run(*va, false, &mut m, &mut c, &mut r));
        assert_eq!(r, before);
    }
    assert!(!weapons_slice::run(0, false, &mut m, &mut c, &mut r));
    assert!(m.0.is_empty());
    assert!(c.seen.is_empty());
    with_args(&mut m, &[0x2000, 0xaabbccdd]);
    weapons_slice::run(0x1b5680, true, &mut m, &mut c, &mut r);
    assert_eq!(m.read32(0x2044), 0xaabbccdd);
    assert_eq!(m.read32(0x2048), 0);
    assert_eq!(r.xmm[0], Xmm::default());
    m.write32(0x2030, u32::MAX);
    m.write32(weapons_slice::DEFINITIONS, 0x6000);
    m.write32(0x6000u32.wrapping_sub(0x29c) + 0xc, 0x180);
    weapons_slice::run(0x1b58c0, true, &mut m, &mut c, &mut r);
    assert_eq!(r.eax, 0x80);
    assert_eq!(r.ecx, 0u32.wrapping_sub(0x29c));
    weapons_slice::run(0xc5c30, true, &mut m, &mut c, &mut r);
    assert_eq!(r.eax, 0x733094);
    assert_eq!(r.esp, STACK);
}
#[test]
fn mode_tails_cover_all_wrappers() {
    for va in [0xbc180, 0xbc2e0, 0xbc470, 0xbc4a0] {
        for mode in [0xa, 3] {
            for kind in [0x18, 0x1a, 0] {
                let mut m = Mem::default();
                let mut c = Guest::default();
                let mut r = regs();
                with_args(&mut m, &[0x2000]);
                m.write8(weapons_slice::MODE, mode);
                m.write32(0x2018, kind);
                m.write32(0x2024, kind);
                m.write32(0x2020, 0xdeadbeef);
                m.write32(0x47862c, 0x3f800000);
                m.write32(0x475d18, 0x40000000);
                weapons_slice::run(va, true, &mut m, &mut c, &mut r);
                let expect = match va {
                    0xbc470 => 0,
                    0xbc4a0 => 0x3f800000,
                    0xbc180 if kind == 0x1a => 0,
                    0xbc2e0 if mode == 0xa && kind == 0x18 => 0x40000000,
                    _ => 0xdeadbeef,
                };
                assert_eq!(m.read32(0x2020), expect);
                assert_eq!(r.esp, STACK);
                assert_eq!(m.read32(STACK), 0x1234);
                if va == 0xbc2e0 && mode == 0xa && kind != 0x18 {
                    assert!(c.seen.is_empty());
                } else {
                    assert_eq!(c.seen[0].0, if mode == 0xa { 0xba230 } else { 0xba310 });
                    assert_eq!(c.seen[0].1[5], STACK + 4);
                    assert_eq!(r.ecx, 0xc0c0);
                }
            }
        }
    }
}
#[test]
fn object_gate_all_paths() {
    for flags in [0, 0x40000] {
        for mode in [0, 9] {
            for ptr in [0, 0x3000] {
                for tag in [0, 0x42524757] {
                    let mut m = Mem::default();
                    let mut c = Guest::default();
                    let mut r = regs();
                    r.eax = 0x2000;
                    m.write32(0x2c6c, flags);
                    m.write32(0x79094c, mode);
                    m.write32(0x2bf0, ptr);
                    m.write32(0x3008, tag);
                    weapons_slice::run(0xb9300, true, &mut m, &mut c, &mut r);
                    assert_eq!(
                        r.eax,
                        if flags != 0 || (mode == 9 && ptr != 0 && tag == 0x42524757) {
                            0
                        } else {
                            1
                        }
                    );
                    assert!(c.seen.is_empty());
                }
            }
        }
    }
}
#[test]
fn packet_and_reset_stack_contracts() {
    let mut m = Mem::default();
    let mut c = Guest::default();
    let mut r = regs();
    r.esi = 0x2000;
    for (i, bits) in [0x3f800000, 0x40000000, 0x40400000].iter().enumerate() {
        m.write32(0x2000 + i as u32 * 4, *bits);
    }
    weapons_slice::run(0xbfda0, true, &mut m, &mut c, &mut r);
    assert_eq!(r.esp, STACK);
    assert_eq!(
        c.seen.iter().map(|s| s.0).collect::<Vec<_>>(),
        vec![0x45390, 0x45820]
    );
    assert_eq!(m.read32(STACK - 0x30), 0xd6d);
    assert_eq!(m.read32(STACK - 0x14), 0x3f800000);
    assert_eq!(m.read32(STACK - 0x10), 0x40000000);
    assert_eq!(m.read32(STACK - 0xc), 0x40400000);
    assert_eq!(m.read8(STACK - 8), 1);
    let mut c = Guest::default();
    let mut r = regs();
    with_args(&mut m, &[0x1111, 0x3000, 0x4000, 0x5555]);
    m.write32(0x7b0c7c, 0x5000);
    weapons_slice::run(0x1b7bc0, true, &mut m, &mut c, &mut r);
    assert_eq!(c.seen[0].0, 0x1b6ec0);
    assert_eq!(c.seen[0].1[0], 0x4000);
    assert_eq!(m.read32(STACK - 20), 0x1111);
    assert_eq!(m.read32(STACK - 16), 0x3000);
    // The wrapper normalizes the nonzero saved-round argument to its boolean value before pushing.
    assert_eq!(m.read32(STACK - 12), 1);
    assert_eq!((r.esi, r.edi, r.esp), (0x51, 0xd1, STACK));
    assert_eq!(m.read32(0x30a8), u32::MAX);
    assert_eq!(m.read32(0x40f0), 1000);
    assert_eq!(m.read32(0x40f4), 1000);
    assert_eq!(m.read32(0x5fb0), 0);
}
