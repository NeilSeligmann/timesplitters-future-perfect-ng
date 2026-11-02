// SPDX-License-Identifier: GPL-3.0-or-later
mod support;
use support::{regs, Calls, Mem};
use tsfp_engine_physics::{followup, memory::GuestMemory};
#[test]
fn disabled_and_unknown_are_pure() {
    for &va in followup::ROOTS.iter().chain([0u32].iter()) {
        let mut m = Mem::default();
        let mut c = Calls::default();
        let mut r = regs();
        let before = r;
        assert!(!followup::run(va, false, &mut m, &mut c, &mut r));
        assert_eq!(r, before);
        assert!(m.words.is_empty());
        assert!(c.0.is_empty());
        assert_eq!(m.reads.get(), 0);
    }
    assert!(!followup::run(
        0,
        true,
        &mut Mem::default(),
        &mut Calls::default(),
        &mut regs()
    ));
}
/// Exploratory bounded fixtures: report completion separately from behavior assertions.
#[test]
fn census_fixture_paths() {
    for &va in followup::ROOTS {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut m = Mem::default();
            let mut r = regs();
            let mut c = Calls::default();
            assert!(followup::run(va, true, &mut m, &mut c, &mut r));
        }));
        assert!(
            result.is_ok(),
            "root {va:08x} did not finish the bounded zero fixture"
        );
        println!("FIXTURE {va:08x} completed");
    }
}
#[test]
fn table_lookup_and_cross_product_use_guest_records() {
    let mut m = Mem::default();
    let mut c = Calls::default();
    let mut r = regs();
    m.write32(r.esp + 4, u32::MAX);
    assert!(followup::run(0xdc290, true, &mut m, &mut c, &mut r));
    assert_eq!(r.eax, 0);
    r = regs();
    m.write32(r.esp + 4, 3);
    m.write32(0x4deb40 + 3 * 64, 0xabcde);
    assert!(followup::run(0xdc290, true, &mut m, &mut c, &mut r));
    assert_eq!(r.eax, 0xabcde);
    r = regs();
    m.write32(0x74c458, 0x20000);
    m.write32(r.esp + 4, 0xabcd0001);
    m.write32(r.esp + 8, 0x30000);
    m.write32(r.esp + 12, 0x31000);
    let shape = 0x20200;
    for (off, value) in [(0x134, 1f32), (0x138, 2.), (0x13c, 3.), (0x140, 10.), (0x144, 20.), (0x148, 30.), (0x14c, 100.), (0x150, 200.), (0x154, 300.)] {
        m.write32(shape + off, value.to_bits());
    }
    for (i, value) in [14f32, 25., 36.].into_iter().enumerate() {
        m.write32(0x30000 + i as u32 * 4, value.to_bits());
    }
    assert!(followup::run(0x166060, true, &mut m, &mut c, &mut r));
    assert_eq!((0..3).map(|i| f32::from_bits(m.read32(0x31000+i*4))).collect::<Vec<_>>(), vec![97.,206.,297.]);
    assert!(c.0.is_empty());
}
