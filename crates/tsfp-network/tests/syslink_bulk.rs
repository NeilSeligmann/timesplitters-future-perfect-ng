// SPDX-License-Identifier: GPL-3.0-or-later
use std::{cell::Cell, collections::BTreeMap};
use tsfp_network::{
    memory::GuestMemory,
    syslink_bulk::{
        self,
        state::{NetworkCall, NetworkRegisters},
        translated,
    },
};
#[derive(Default)]
struct Memory {
    bytes: BTreeMap<u32, u8>,
    reads: Cell<usize>,
}
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        let n = self.reads.get() + 1;
        self.reads.set(n);
        assert!(n < 200_000, "synthetic read budget");
        *self.bytes.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.bytes.insert(a, v);
    }
}
#[derive(Default)]
struct Calls {
    targets: Vec<u32>,
}
impl NetworkCall for Calls {
    fn call(&mut self, va: u32, r: &mut NetworkRegisters) {
        self.targets.push(va);
        assert!(self.targets.len() < 1024, "synthetic call budget");
        // Adapter supplies/consumes the nested RET slot; only ret N changes this ESP.
        r.esp = r.esp.wrapping_add(callee_pop(va));
        r.eax = 0;
    }
}
fn registers() -> NetworkRegisters {
    NetworkRegisters {
        esp: 0x100000,
        ecx: 0x200000,
        ..Default::default()
    }
}
#[test]
fn gate_preserves_state_for_disabled_and_unknown_roots() {
    let mut m = Memory::default();
    let mut c = Calls::default();
    let mut r = registers();
    let before = r;
    for (va, enabled) in [(translated::ROOTS[0], false), (0xdeadbeef, true)] {
        assert!(!syslink_bulk::run_if_enabled(
            va, enabled, &mut m, &mut c, &mut r
        ));
        assert_eq!(r, before);
    }
    assert!(m.bytes.is_empty());
    assert!(c.targets.is_empty());
    assert_eq!(m.reads.get(), 0);
}
#[test]
fn each_root_zero_state_smoke() {
    let mut complete = Vec::new();
    let mut bounded = Vec::new();
    for &va in &translated::ROOTS {
        let mut m = Memory::default();
        let mut c = Calls::default();
        let mut r = registers();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert!(syslink_bulk::run_if_enabled(
                va, true, &mut m, &mut c, &mut r
            ));
        }));
        if result.is_ok() {
            complete.push(va);
        } else {
            bounded.push(va);
        }
    }
    println!(
        "returned {} roots; bounded incomplete: {:x?}",
        complete.len(),
        bounded
    );
    assert_eq!(complete.len() + bounded.len(), translated::ROOTS.len());
    // Exact accepted synthetic returns pinned after first run; bounded roots remain debt.
    assert!(bounded.is_empty(), "incomplete roots: {bounded:x?}");
}

fn callee_pop(va: u32) -> u32 {
    match va {
        0xaaaa => 8,
        0x306d0 => 8,
        0x3a9d50 => 8,
        0x3a9de0 => 8,
        0x3aa200 => 4,
        0x3aaca0 => 4,
        0x3aace0 => 4,
        0x3aba60 => 4,
        0x3abe10 => 8,
        0x3abe80 => 8,
        0x3abeb0 => 8,
        0x3abf70 => 8,
        0x3abfc0 => 8,
        0x3ac030 => 12,
        0x3b64b0 => 4,
        0x3b7c40 => 4,
        0x3bd3b0 => 4,
        0x3bd460 => 4,
        0x3bd7c0 => 12,
        0x3bd870 => 4,
        0x3be590 => 8,
        0x3bfb90 => 4,
        0x3bfef0 => 4,
        0x3bffa0 => 4,
        0x3bffc0 => 4,
        0x3c0030 => 4,
        0x3c0070 => 4,
        0x3c0180 => 4,
        0x3c0210 => 4,
        0x3c02b0 => 4,
        0x3c0370 => 4,
        0x3c0400 => 4,
        0x3c04a0 => 8,
        0x3c0890 => 12,
        0x3c0900 => 16,
        0x3c4350 => 4,
        0x3c4570 => 4,
        0x3c4590 => 20,
        0x3c4730 => 4,
        0x3c4770 => 4,
        0x3c47f0 => 8,
        0x3c4830 => 4,
        0x3c4870 => 4,
        0x3c48d0 => 4,
        0x3c4970 => 8,
        0x3c4a40 => 12,
        0x3c4bb0 => 8,
        0x3c4c80 => 12,
        0x3c4d80 => 4,
        0x3c4ef0 => 4,
        0x3c4fb0 => 4,
        0x3c5540 => 4,
        0x3c55b0 => 16,
        0x3c5770 => 8,
        0x3c57b0 => 4,
        0x3c5a80 => 4,
        0x3c5bf0 => 4,
        0x3c5d30 => 4,
        0x3c6150 => 4,
        0x3c62d0 => 4,
        0x3c6340 => 4,
        0x3c6410 => 4,
        0x3c64e0 => 4,
        _ => 0,
    }
}

#[test]
fn stdcall_root_preserves_arguments_and_cleans_own_stack() {
    let mut m = Memory::default();
    let mut c = Calls::default();
    let mut r = registers();
    let entry = r.esp;
    m.write32(r.ecx, 0x300000);
    m.write32(0x300010, 0xaaaa);
    m.write32(entry + 4, 0x12345678);
    translated::root_003a9f10(&mut m, &mut c, &mut r);
    assert_eq!(c.targets, vec![0xaaaa]);
    assert_eq!(m.read32(entry - 8), 0x12345678);
    assert_eq!(m.read32(entry - 4), 0);
    assert_eq!(r.esp, entry + 4); // nested callee pops 8; root pops its own 4-byte argument
}

#[test]
fn word_compare_loop_keeps_count_direction_and_last_comparison() {
    use syslink_bulk::rt::{repe_cmpsw, write16};
    let mut m = Memory::default();
    for (a, v) in [
        (0x100, 7),
        (0x102, 8),
        (0x104, 9),
        (0x200, 7),
        (0x202, 0),
        (0x204, 9),
    ] {
        write16(&mut m, a, v);
    }
    let (mut a, mut b, mut n, mut z) = (0x100, 0x200, 3, 1);
    repe_cmpsw(&m, &mut a, &mut b, &mut n, &mut z, 0);
    assert_eq!((a, b, n, z), (0x104, 0x204, 1, 0));
    (a, b, n, z) = (0x104, 0x204, 3, 1);
    repe_cmpsw(&m, &mut a, &mut b, &mut n, &mut z, 0x400);
    assert_eq!((a, b, n, z), (0x100, 0x200, 1, 0));
    (a, b, n, z) = (0x100, 0x200, 0, 1);
    let reads = m.reads.get();
    repe_cmpsw(&m, &mut a, &mut b, &mut n, &mut z, 0);
    assert_eq!((a, b, n, z), (0x100, 0x200, 0, 1));
    assert_eq!(reads, m.reads.get());
}
