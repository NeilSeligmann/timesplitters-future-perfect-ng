// SPDX-License-Identifier: GPL-3.0-or-later
mod support;
use support::{regs, Calls, Mem};
use tsfp_engine_physics::{
    boundary::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
    remainder,
};
#[test]
fn disabled_and_unknown_are_pure() {
    for &va in remainder::ROOTS.iter().chain([0u32].iter()) {
        let mut m = Mem::default();
        let mut c = Calls::default();
        let mut r = regs();
        let before = r;
        assert!(!remainder::run(va, false, &mut m, &mut c, &mut r));
        assert_eq!(r, before);
        assert!(m.words.is_empty());
        assert!(c.0.is_empty());
        assert_eq!(m.reads.get(), 0);
    }
    assert!(!remainder::run(
        0,
        true,
        &mut Mem::default(),
        &mut Calls::default(),
        &mut regs()
    ));
}
#[test]
fn slot_vector_copy_preserves_sequential_alias() {
    let mut m = Mem::default();
    let mut r = regs();
    let mut c = Calls::default();
    m.write32(0x74c464, 0x20000);
    m.write32(r.esp + 4, 0);
    m.write32(r.esp + 8, 0);
    m.write32(r.esp + 12, 0x1fffc);
    m.write32(0x1fffc, 7);
    m.write32(0x20000, 8);
    m.write32(0x20004, 9);
    assert!(remainder::run(0x16b300, true, &mut m, &mut c, &mut r));
    assert_eq!(
        [m.read32(0x20000), m.read32(0x20004), m.read32(0x20008)],
        [7, 7, 7]
    );
    assert!(c.0.is_empty());
}
#[test]
fn queue_append_gates_and_copies_four_words() {
    let mut m = Mem::default();
    let mut r = regs();
    let mut c = Calls::default();
    r.ecx = 0x30000;
    for (a, v) in [
        (0x74c484, 0x20000),
        (0x74c474, 0x21000),
        (0x74c47c, 2),
        (r.esp + 4, 99),
    ] {
        m.write32(a, v);
    }
    for i in 0..4 {
        m.write32(r.ecx + i * 4, i + 20);
    }
    assert!(remainder::run(0x16c070, true, &mut m, &mut c, &mut r));
    assert_eq!(m.read32(0x74c490), 1);
    assert_eq!(m.read32(0x20000), 99);
    assert_eq!(
        (0..4)
            .map(|i| m.read32(0x21000 + i * 4))
            .collect::<Vec<_>>(),
        vec![20, 21, 22, 23]
    );
}
/// Exploratory bounded fixtures: report completion separately from behavior assertions.
#[test]
fn census_fixture_paths() {
    for &va in remainder::ROOTS {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut m = Mem::default();
            let mut r = regs();
            let mut c = Calls::default();
            assert!(remainder::run(va, true, &mut m, &mut c, &mut r));
        }));
        assert!(
            result.is_ok(),
            "root {va:08x} did not finish the bounded zero fixture"
        );
        println!("FIXTURE {va:08x} completed");
    }
}

#[test]
fn shape_setter_calls_neighbor_and_keeps_callee_fp_state() {
    struct Neighbor;
    impl GuestCall for Neighbor {
        fn call(&mut self, va: u32, r: &mut ExtRegisters) {
            assert_eq!(va, 0x167880);
            r.xmm_hi[0] = Xmm::scalar(0x12345678);
            r.fpu.push(3.25);
        }
    }
    let mut m = Mem::default();
    let mut r = regs();
    m.write32(0x74c458, 0x20000);
    m.write32(r.esp + 4, 0xabcd0001);
    m.write32(r.esp + 8, 0x30000);
    for i in 0..3 {
        m.write32(0x30000 + i * 4, 10 + i);
    }
    assert!(remainder::run(
        0x167bc0,
        true,
        &mut m,
        &mut Neighbor,
        &mut r
    ));
    assert_eq!(
        (0..3)
            .map(|i| m.read32(0x20328 + i * 4))
            .collect::<Vec<_>>(),
        vec![10, 11, 12]
    );
    assert_eq!(r.xmm_hi[0].lanes[0], 0x12345678);
    assert_eq!(r.fpu.st(0), 3.25);
}
