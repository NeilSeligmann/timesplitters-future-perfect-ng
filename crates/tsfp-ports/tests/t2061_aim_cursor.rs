// SPDX-License-Identifier: GPL-3.0-or-later
//! T2061: root 0x5AFB0 (blend space pose). When the global `[0x6D7804]` is 1 the original first calls 0x57550(ecx = context), which
//! refreshes the cursor globals (rows, columns, weights); the Rust port must do the same BEFORE it reads the cursor, otherwise it
//! samples the previous frame's cell (the T2043 shadow divergence, quaternion word 3). Synthetic structures only.
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::anim_lane3;
use tsfp_ports::memory::GuestMemory;

const BASE: u32 = 0x1_0000;
const SIZE: usize = 0x80_0000;
const STACK: u32 = 0x7f_0000;

struct Fake(Vec<u8>);
impl GuestMemory for Fake {
    fn read8(&self, a: u32) -> u8 {
        assert!((BASE..0x8000_0000).contains(&a), "raw access {a:#x}");
        self.0.get((a - BASE) as usize).copied().unwrap_or(0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        assert!((BASE..0x8000_0000).contains(&a), "raw access {a:#x}");
        if let Some(b) = self.0.get_mut((a - BASE) as usize) {
            *b = v;
        }
    }
}

#[derive(Default)]
struct Guest {
    calls: Vec<(u32, u32)>,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.calls.push((va, regs.ecx));
    }
}

fn run(flag: u32) -> Vec<(u32, u32)> {
    let mut m = Fake(vec![0; SIZE]);
    let (ctx, object, shape, out) = (0x2_0000u32, 0x2_1000u32, 0x2_2000u32, 0x2_3000u32);
    m.write32(ctx + 4, object);
    m.write32(ctx + 0x7c, shape);
    m.write32(0x6d7804, flag);
    let mut regs = ExtRegisters {
        eax: 0, ecx: 0, edx: 0, esp: STACK, ebx: 0, ebp: 0, esi: 0, edi: 0, eflags: 0,
        fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x37f },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    m.write32(STACK + 4, ctx);
    m.write32(STACK + 8, out);
    m.write32(STACK + 12, 3);
    let mut guest = Guest::default();
    assert!(anim_lane3::run(0x5afb0, &mut m, &mut guest, &mut regs));
    guest.calls
}

#[test]
fn cursor_refresh_runs_first_when_the_selector_is_one() {
    let calls = run(1);
    assert_eq!(calls.first(), Some(&(0x57550, 0x2_0000)), "calls: {calls:x?}");
}

#[test]
fn no_cursor_refresh_for_other_selector_values() {
    for flag in [0, 2, 0xffff_ffff] {
        assert!(!run(flag).iter().any(|c| c.0 == 0x57550), "selector {flag}");
    }
}
