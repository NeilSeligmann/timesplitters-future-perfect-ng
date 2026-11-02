// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-port-data-text/c2r/data_text
// portreg: mirrors=tsfp-ui/text
// portreg: mirrors=tsfp-ui/text_entry
//! S4 Tier 2 (T2157): hybrid lane adapters for the 36 `data/text` roots that have a Tier 1 machine translation in
//! `c2r/data_text.rs` (T1878, differential against the lifted original): text bank lookups (`0x3D7B0`, `0x461C0`, `0x66470`),
//! string helpers (copy, duplicate, length, tail replace, line extract), the script token scanners, the memory unit text
//! tables and the text entry screen. The translations use the `ExtRegisters` plus `GuestCall` contract, so this file only gates
//! which roots the host may select and runs all 36 roots through [`crate::anim_lane::guarded`] (a null or wild pointer is refused, the
//! write footprint is rolled back and the retained original runs; once a callee ran the access is a counted degraded no-op, T2177) with ESP restored to the entry value like `physics_lane.rs`.
//! Registered through `lane_inv.rs` (the `export!` symbols are defined here). Excluded (no Tier 1): `0x46030`, `0x2D1640`, `0x2D1700`.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::data_text,
    lane::LaneCpu,
    lane_inv::execute,
    memory::{GuestMemory, MappedGuest},
};

/// The data/text lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 36] = [
    0x3d7b0, 0x3fb90, 0x461c0, 0x150f60, 0x27250, 0x151160, 0x40650, 0x63c80,
    0x63e70, 0x3dd20, 0x3db80, 0x3de30, 0x46170, 0x150fe0, 0x31c50, 0x32860,
    0x37330, 0x38200, 0x3fc10, 0x406a0, 0x66470, 0x7c300, 0x7c390, 0x82860,
    0x828c0, 0x828f0, 0x82920, 0x195810, 0x1b2b80, 0x2be6f0, 0x2c7750, 0x2c7780,
    0x2d1370, 0x2d14c0, 0x2d1770, 0x311820,
];

/// Dispatches a root; `false` when `va` is not one of the data/text adapters.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x3d7b0 | 0x3fb90 | 0x461c0 | 0x150f60 | 0x27250 | 0x151160 | 0x40650 | 0x63c80 |
        0x63e70 | 0x3dd20 | 0x3db80 | 0x3de30 | 0x46170 | 0x150fe0 | 0x31c50 | 0x32860 |
        0x37330 | 0x38200 | 0x3fc10 | 0x406a0 | 0x66470 | 0x7c300 | 0x7c390 | 0x82860 |
        0x828c0 | 0x828f0 | 0x82920 | 0x195810 | 0x1b2b80 | 0x2be6f0 | 0x2c7750 | 0x2c7780 |
        0x2d1370 | 0x2d14c0 | 0x2d1770 | 0x311820 => {}
        _ => return false,
    }
    let entry_esp = regs.esp;
    // T2177: all 36 roots run guarded, `guarded` never re-runs the original after a callee ran (it degrades and counts instead)
    let handled = crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| data_text::run(va, view, guest, regs));
    regs.esp = entry_esp;
    handled
}

macro_rules! export {
    ($name:ident, $va:expr) => {
        /// # Safety
        /// `cpu` is a live C rust_cpu object and offset maps the guest window.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(cpu: *mut LaneCpu, offset: usize) {
            let mut memory = unsafe { MappedGuest::new(offset) };
            execute($va, &mut memory, unsafe { &mut *cpu });
        }
    };
}
export!(tsfp_lane_0003d7b0, 0x3d7b0);
export!(tsfp_lane_0003fb90, 0x3fb90);
export!(tsfp_lane_000461c0, 0x461c0);
export!(tsfp_lane_00150f60, 0x150f60);
export!(tsfp_lane_00027250, 0x27250);
export!(tsfp_lane_00151160, 0x151160);
export!(tsfp_lane_00040650, 0x40650);
export!(tsfp_lane_00063c80, 0x63c80);
export!(tsfp_lane_00063e70, 0x63e70);
export!(tsfp_lane_0003dd20, 0x3dd20);
export!(tsfp_lane_0003db80, 0x3db80);
export!(tsfp_lane_0003de30, 0x3de30);
export!(tsfp_lane_00046170, 0x46170);
export!(tsfp_lane_00150fe0, 0x150fe0);
export!(tsfp_lane_00031c50, 0x31c50);
export!(tsfp_lane_00032860, 0x32860);
export!(tsfp_lane_00037330, 0x37330);
export!(tsfp_lane_00038200, 0x38200);
export!(tsfp_lane_0003fc10, 0x3fc10);
export!(tsfp_lane_000406a0, 0x406a0);
export!(tsfp_lane_00066470, 0x66470);
export!(tsfp_lane_0007c300, 0x7c300);
export!(tsfp_lane_0007c390, 0x7c390);
export!(tsfp_lane_00082860, 0x82860);
export!(tsfp_lane_000828c0, 0x828c0);
export!(tsfp_lane_000828f0, 0x828f0);
export!(tsfp_lane_00082920, 0x82920);
export!(tsfp_lane_00195810, 0x195810);
export!(tsfp_lane_001b2b80, 0x1b2b80);
export!(tsfp_lane_002be6f0, 0x2be6f0);
export!(tsfp_lane_002c7750, 0x2c7750);
export!(tsfp_lane_002c7780, 0x2c7780);
export!(tsfp_lane_002d1370, 0x2d1370);
export!(tsfp_lane_002d14c0, 0x2d14c0);
export!(tsfp_lane_002d1770, 0x2d1770);
export!(tsfp_lane_00311820, 0x311820);
