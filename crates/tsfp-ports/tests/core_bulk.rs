// SPDX-License-Identifier: GPL-3.0-or-later
//! T1907 Tier 0 smoke with explicit synthetic guest memory/callee boundary.
use std::{cell::Cell, collections::HashMap};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    core_bulk::translated,
    memory::GuestMemory,
};
#[derive(Default)]
struct Mem {
    bytes: HashMap<u32, u8>,
    reads: Cell<usize>,
}
impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        let n = self.reads.get() + 1;
        self.reads.set(n);
        assert!(n < 100_000, "smoke exceeded read budget");
        *self.bytes.get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.bytes.insert(address, value);
    }
}
#[derive(Default)]
struct Calls {
    targets: Vec<u32>,
    stop_boot: bool,
}
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.targets.push(va);
        if self.stop_boot && va == 0x65F40 {
            panic!("observed complete boot-loop iteration");
        }
        assert!(self.targets.len() < 1000, "smoke exceeded call budget");
        r.eax = 0;
        // GuestCall owns its synthetic nested return slot; stdcall XDK helpers pop arguments.
        r.esp = r.esp.wrapping_add(match va {
            0x37CB19 => 8,
            _ => 0,
        });
    }
}
fn registers() -> ExtRegisters {
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
            control: 0x37F,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
#[test]
fn returning_bulk_roots_return_on_synthetic_zero_state() {
    for va in translated::ROOTS {
        if va == 0x3D440 {
            continue;
        }
        let mut m = Mem::default();
        let mut r = registers();
        let mut calls = Calls::default();
        m.write32(0x564558, 1); // launch info already queried: isolate tokenizer from XDK stdcall.
        assert!(translated::run(va, &mut m, &mut calls, &mut r));
        assert_eq!(r.esp, 0x9000, "stack contract for {va:08X}");
        assert_eq!(
            (r.ebx, r.ebp, r.esi, r.edi),
            (0, 0, 0, 0),
            "callee-saved contract for {va:08X}"
        );
    }
}
#[test]
fn unsupported_bulk_root_leaves_registers_unchanged() {
    let mut m = Mem::default();
    let mut r = registers();
    let before = r;
    let mut calls = Calls::default();
    assert!(!translated::run(0xBAD, &mut m, &mut calls, &mut r));
    assert!(!tsfp_ports::core_bulk::run_if_enabled(
        0x22040, false, &mut m, &mut calls, &mut r
    ));
    assert_eq!(r, before);
    assert!(calls.targets.is_empty());
}

#[test]
fn boot_loop_reaches_observation_boundary_without_faking_return() {
    let mut m = Mem::default();
    let mut r = registers();
    let mut calls = Calls {
        stop_boot: true,
        ..Calls::default()
    };
    let stop = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        translated::run(0x3D440, &mut m, &mut calls, &mut r)
    }));
    assert!(stop.is_err());
    assert_eq!(calls.targets.last(), Some(&0x65F40));
    assert!(calls.targets.contains(&0x68610));
    assert!(calls.targets.contains(&0x2FF60));
    assert_eq!(m.read32(0x4C0A28), 1);
    assert_eq!(m.read32(0x4C0A2C), 0);
}
