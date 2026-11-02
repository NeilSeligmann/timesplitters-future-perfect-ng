// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED contract tests for the extended ABI; not an original-vs-Rust receipt.
use std::collections::BTreeMap;
use tsfp_port_runtime::{abi::*, memory};
#[derive(Default)]
struct Mem(BTreeMap<u32, u8>);
impl memory::GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
struct Callee {
    seen: Vec<(u32, Vec<u32>)>,
}
impl GuestCall for Callee {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let m = r.esp;
        self.seen.push((va, vec![m]));
        r.fpu.return_st0(2.5);
    }
}
fn regs() -> ExtRegisters {
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
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
#[test]
fn x87_stack_wraps_like_the_lift() {
    let mut f = regs().fpu;
    f.push(1.0);
    assert_eq!(f.top, 7);
    f.push(2.0);
    assert_eq!(f.st(1), 1.0);
    f.pop();
    f.pop();
    assert_eq!(f.top, 0);
}
#[test]
fn precision_control_selects_single_rounding() {
    let mut f = regs().fpu;
    f.control = 0x0000;
    assert_eq!(f.round_single(0.1), 0.1f32 as f64);
    f.control = 0x27f;
    assert_eq!(f.round_single(0.1), 0.1);
}
#[test]
fn cdecl_call_restores_esp_and_returns_in_st0() {
    let mut m = Mem::default();
    let mut c = Callee { seen: vec![] };
    let mut r = regs();
    call_cdecl(&mut c, &mut m, &mut r, 0xb9960, &[7, 8, 9]);
    assert_eq!(r.esp, 0x9000);
    assert_eq!(c.seen[0].1[0], 0x9000 - 12);
    use memory::GuestMemory;
    assert_eq!(m.read32(0x9000 - 12), 7);
    assert_eq!(m.read32(0x9000 - 4), 9);
    assert_eq!(r.fpu.st(0), 2.5);
    assert_eq!(r.fpu.top, 7);
}
#[test]
fn scalar_xmm_clears_upper_lanes() {
    assert_eq!(Xmm::scalar(5).lanes, [5, 0, 0, 0]);
}

// T1815 w7: XMM5-7 slots and the lifted-code range check (FABRICATED contract tests).
#[test]
fn ext_registers_layout_with_xmm5_to_7() {
    assert_eq!(core::mem::size_of::<ExtRegisters>(), 240);
    assert_eq!(core::mem::offset_of!(ExtRegisters, xmm), 112);
    assert_eq!(core::mem::offset_of!(ExtRegisters, xmm_hi), 192);
}
struct Hi {
    called: Vec<u32>,
}
impl GuestCall for Hi {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.called.push(va);
        r.xmm_hi[0] = Xmm::scalar(0x11);
        r.xmm_hi[2] = Xmm::scalar(0x33);
        r.eax = 7;
    }
}
#[test]
fn xmm5_to_7_round_trip_through_a_call() {
    let mut m = Mem::default();
    let mut c = Hi { called: vec![] };
    let mut r = regs();
    r.xmm_hi[1] = Xmm::scalar(0x22);
    call_cdecl(&mut c, &mut m, &mut r, 0x1000, &[]);
    assert_eq!(
        (
            r.xmm_hi[0].lanes[0],
            r.xmm_hi[1].lanes[0],
            r.xmm_hi[2].lanes[0]
        ),
        (0x11, 0x22, 0x33)
    );
}
#[test]
fn code_range_matches_recomp_icall_is_code() {
    let r = CodeRange {
        lo: 0x1000,
        hi: 0x5000,
    };
    assert!(
        r.is_code(0x1000) && r.is_code(0x4fff) && r.is_code(0xFE00_0000) && r.is_code(u32::MAX)
    );
    assert!(!r.is_code(0xfff) && !r.is_code(0x5000) && !r.is_code(0) && !r.is_code(0xFDFF_FFFF));
    assert!(CodeRange { lo: 0, hi: 0 }.is_code(0x1234_5678)); // no span configured: everything is code
}
#[test]
fn indirect_hook_outside_code_range_is_skipped_with_eax_zero() {
    let mut m = Mem::default();
    let mut c = Hi { called: vec![] };
    let mut r = regs();
    r.eax = 5;
    let code = CodeRange {
        lo: 0x1000,
        hi: 0x5000,
    };
    assert!(!call_indirect_cdecl(
        &mut c,
        &mut m,
        &mut r,
        0x9000_0000,
        &[9, 1, 2, 0],
        code
    ));
    assert!(c.called.is_empty());
    assert_eq!((r.eax, r.esp), (0, 0x9000));
    assert!(call_indirect_cdecl(
        &mut c,
        &mut m,
        &mut r,
        0x2000,
        &[9, 1, 2, 0],
        code
    ));
    assert_eq!(c.called, vec![0x2000]);
    assert_eq!((r.eax, r.esp), (7, 0x9000));
}
