// SPDX-License-Identifier: GPL-3.0-or-later
//! T2177: `anim_lane::guarded` for roots that call back into the retained original. A synthetic call-bearing root pushes an argument,
//! calls a callee and then reads a null pointer: the callee ran, so the original must NOT be re-run (it would repeat the callee's
//! side effects); the access is a counted degraded no-op. A null read BEFORE the first callee rolls back and runs the original once,
//! and a callee is never started on a view that already refused an access.
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::anim_lane::{guard_counters, guarded, journal_overflows};
use tsfp_ports::memory::GuestMemory;

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

const STACK: u32 = 0x7f_0000;
const ROOT: u32 = 0x3dd20;
const CALLEE: u32 = 0x3c85d4;

struct Mem(Vec<u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        self.0[a as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[a as usize] = v;
    }
}

#[derive(Default)]
struct Guest {
    calls: Vec<u32>,
    /// guest cell the callee increments, standing for any callee side effect (allocation, counter)
    side_effect: u32,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.calls.push(va);
        if va == CALLEE {
            self.side_effect += 1;
        }
        regs.eax = 0x77;
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0, ecx: 0, edx: 0, esp: STACK, ebx: 0, ebp: 0, esi: 0, edi: 0, eflags: 0,
        fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x37f },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

#[test]
fn null_access_after_a_callee_ran_does_not_rerun_the_original() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = Mem(vec![0; 0x80_0000]);
    let mut guest = Guest::default();
    let mut regs = registers();
    let (degraded0, _, rollbacks0) = guard_counters();
    let handled = guarded(ROOT, &mut m, &mut guest, &mut regs, |view, guest, regs| {
        view.write32(0x2_0000, 0xdead_beef);
        let arg_slot = regs.esp.wrapping_sub(4);
        view.write32(arg_slot, 5);
        guest.call(CALLEE, regs);
        let after = view.read32(0); // null read after the callee
        view.write32(0x2_0004, after.wrapping_add(1));
        true
    });
    let (degraded1, _, rollbacks1) = guard_counters();
    assert!(handled);
    assert_eq!(guest.side_effect, 1, "the callee must run exactly once");
    assert_eq!(guest.calls, vec![CALLEE], "the original root is not re-run after a callee ran");
    assert_eq!(degraded1 - degraded0, 1, "counted as degraded");
    assert_eq!(rollbacks1 - rollbacks0, 0);
    assert_eq!(m.read32(0x2_0000), 0xdead_beef, "the port's own writes stay (no rollback)");
}

#[test]
fn null_access_before_any_callee_rolls_back_and_runs_the_original_once() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = Mem(vec![0; 0x80_0000]);
    m.write32(STACK, 0x1234_5678); // return slot
    let mut guest = Guest::default();
    let mut regs = registers();
    let (_, skipped0, rollbacks0) = guard_counters();
    let handled = guarded(ROOT, &mut m, &mut guest, &mut regs, |view, guest, regs| {
        view.write32(0x2_0000, 0xdead_beef);
        let _ = view.read32(0); // null read first
        guest.call(CALLEE, regs); // must be skipped: the view is tainted
        true
    });
    let (_, skipped1, rollbacks1) = guard_counters();
    assert!(handled);
    assert_eq!(guest.side_effect, 0, "no callee starts on a tainted view");
    assert_eq!(guest.calls, vec![ROOT], "the original runs exactly once");
    assert_eq!(skipped1 - skipped0, 1);
    assert_eq!(rollbacks1 - rollbacks0, 1);
    assert_eq!(m.read32(0x2_0000), 0, "the port's write was rolled back");
    assert_eq!(m.read32(STACK), 0x1234_5678, "the return slot survives the re-run");
    assert_eq!(regs.esp, STACK);
}

#[test]
fn a_clean_call_bearing_root_is_untouched() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = Mem(vec![0; 0x80_0000]);
    let mut guest = Guest::default();
    let mut regs = registers();
    let before = guard_counters();
    assert!(guarded(ROOT, &mut m, &mut guest, &mut regs, |view, guest, regs| {
        view.write32(0x2_0000, 7);
        guest.call(CALLEE, regs);
        true
    }));
    assert_eq!(guard_counters(), before);
    assert_eq!(guest.calls, vec![CALLEE]);
    assert_eq!(m.read32(0x2_0000), 7);
}

/// Measured on the menu route (T2177): 0x3DD20 and 0x3DB80 write more bytes than the old 32 KB journal held (text bank relocation and copy).
/// The write must land, the call must not count as bad and the callee must not be re-run.
#[test]
fn a_write_larger_than_the_journal_still_lands_and_is_not_an_invalid_access() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = Mem(vec![0; 0x80_0000]);
    let mut guest = Guest::default();
    let mut regs = registers();
    let before = guard_counters();
    let overflows = journal_overflows();
    assert!(guarded(ROOT, &mut m, &mut guest, &mut regs, |view, guest, regs| {
        guest.call(CALLEE, regs);
        for i in 0..100_000u32 {
            view.write32(0x10_0000 + 4 * i, i | 0x8000_0000);
        }
        true
    }));
    assert!(journal_overflows() > overflows, "the test must exceed the journal");
    assert_eq!(guard_counters(), before, "an overflow is neither degraded nor a rollback");
    assert_eq!(guest.calls, vec![CALLEE]);
    assert_eq!(m.read32(0x10_0000 + 4 * 99_999), 99_999 | 0x8000_0000, "writes past the journal end land");
}
