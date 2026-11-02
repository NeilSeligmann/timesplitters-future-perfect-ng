// SPDX-License-Identifier: GPL-3.0-or-later
// Tier 0 smoke test of the hand ports 0x20a8a0 and 0x2214e0 (scoped harness, FABRICATED fixtures).
#![allow(dead_code, unused_imports)]
#[path = "../../src/abi.rs"]
mod abi;
#[path = "../../src/memory.rs"]
mod memory;
use tsfp_ports::c2r::t1953_manual;

use abi::{ExtRegisters, GuestCall};
use memory::GuestMemory;

struct Mem(Vec<u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        self.0[a as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[a as usize] = v
    }
}
#[derive(Default)]
struct Calls(Vec<(u32, u32)>);
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        // one slot above esp is the first argument (the return slot sits at esp)
        self.0.push((va, r.eax));
        r.eax = 7;
    }
}

fn setup() -> (Mem, ExtRegisters) {
    let mut m = Mem(vec![0; 0x80_0000]);
    let mut r: ExtRegisters = unsafe { core::mem::zeroed() }; // plain integer, float and array fields
    r.esp = 0x7f_0000;
    m.write32(r.esp.wrapping_add(4), 0x1000); // hit
    (m, r)
}

#[test]
fn forward_requires_both_fields() {
    let (mut m, mut r) = setup();
    let mut c = Calls::default();
    t1953_manual::run(0x2214e0, &mut m, &mut c, &mut r);
    assert!(c.0.is_empty(), "no fields set: plain return");
    m.write32(0x1038, 0x2000);
    t1953_manual::run(0x2214e0, &mut m, &mut c, &mut r);
    assert!(c.0.is_empty(), "only +0x38 set");
    m.write32(0x1028, 1);
    m.write32(0x2004, 0xabcd);
    m.write32(0x1000, 0x55);
    let esp = r.esp;
    t1953_manual::run(0x2214e0, &mut m, &mut c, &mut r);
    assert_eq!(c.0, vec![(0x221410, 0xabcd)]);
    assert_eq!(r.edx, 0x55);
    assert_eq!(r.esp, esp, "tail call keeps the entry ESP");
}

#[test]
fn health_update_without_death_only_subtracts() {
    let (mut m, mut r) = setup();
    m.write32(0x1000, 0x3000); // owner
    m.write32(0x3000 + 0x1cc, 10.0f32.to_bits());
    m.write32(0x1020, 4.0f32.to_bits());
    let mut c = Calls::default();
    let (esi, edi) = (r.esi, r.edi);
    t1953_manual::run(0x20a8a0, &mut m, &mut c, &mut r);
    assert_eq!(f32::from_bits(m.read32(0x31cc)), 6.0);
    assert!(c.0.is_empty());
    assert_eq!((r.esi, r.edi), (esi, edi));
}

#[test]
fn health_update_death_allocates_entry_and_forwards() {
    let (mut m, mut r) = setup();
    m.write32(0x1000, 0x3000);
    m.write32(0x3000 + 4, 0x4000); // record
    m.write32(0x3000 + 0x1cc, 1.0f32.to_bits());
    m.write32(0x1020, 4.0f32.to_bits());
    let mut c = Calls::default();
    t1953_manual::run(0x20a8a0, &mut m, &mut c, &mut r);
    assert!(f32::from_bits(m.read32(0x31cc)) < 0.0);
    assert_eq!(m.read32(0x4030), 4);
    let order: Vec<u32> = c.0.iter().map(|x| x.0).collect();
    assert_eq!(
        order,
        vec![
            0x153b40, 0xa5ce0, 0x167f90, 0x16c170, 0x172760, 0x165d50, 0x1711d0
        ]
    );
    assert_eq!(m.read32(0x3000 + 0x80), 7);
}

#[test]
fn unknown_va_is_inert() {
    let (mut m, mut r) = setup();
    assert!(!t1953_manual::run(
        0x1234,
        &mut m,
        &mut Calls::default(),
        &mut r
    ));
}
