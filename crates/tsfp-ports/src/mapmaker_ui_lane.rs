// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/mapmaker_ui
//! S4 Tier 2 (T2142): hybrid lane adapters for the 114 `mapmaker/ui` roots that have a Tier 1 machine translation in
//! `mapmaker_ui.rs` (T1916, differential against the lifted original in `t1933`..`T2127`): the Map Maker editor draw passes
//! (toolbar, hint labels, style panels, ramp overlay, scene overlays, edit menu). The translations already use the
//! `ExtRegisters` plus `GuestCall` contract of the lane, so this file only gates which roots the host may select, runs each
//! through [`crate::anim_lane::guarded`] (a null or wild pointer is refused, the write footprint is rolled back and the retained
//! original runs instead) and normalises ESP to the entry value like `physics_lane.rs`. Registered through `lane_inv.rs`
//! (the `export!` symbols are defined here). Excluded (not Tier 1): `0x802C0`, `0x30D4D0`, `0x30DE40`, `0x3133E0`.
use crate::{
    abi::{ExtRegisters, GuestCall},
    lane::LaneCpu,
    lane_inv::execute,
    memory::{GuestMemory, MappedGuest},
};

/// The mapmaker/ui lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 114] = [
    0x70e30, 0x81480, 0x81c10, 0x81c40, 0x2fa030, 0x2fe450, 0x308b10, 0x308c00,
    0x3091c0, 0x3092e0, 0x3096a0, 0x309a80, 0x30aea0, 0x30b150, 0x30b280, 0x30b3a0,
    0x30b480, 0x30b560, 0x30b7c0, 0x30b9b0, 0x30ba90, 0x30bc50, 0x30be60, 0x30c210,
    0x30c3c0, 0x30c440, 0x30cb30, 0x30cb90, 0x30ccd0, 0x30cf60, 0x30d120, 0x30d300,
    0x30d410, 0x30d7b0, 0x30dd40, 0x310100, 0x310310, 0x3108d0, 0x310a20, 0x312270,
    0x314420, 0x314f80, 0x314fa0, 0x315d20, 0x315d60, 0x315db0, 0x315e10, 0x316110,
    0x316b50, 0x3179d0, 0x317d70, 0x318350, 0x3188c0, 0x318be0, 0x318ea0, 0x3190c0,
    0x3191f0, 0x3192e0, 0x319300, 0x319bd0, 0x319da0, 0x319e60, 0x31ab80, 0x31ad90,
    0x31b6b0, 0x31b7d0, 0x31b8b0, 0x31c7c0, 0x31cb90, 0x31d140, 0x31d160, 0x31d180,
    0x31d3e0, 0x31d580, 0x31dc10, 0x31eab0, 0x31ec90, 0x31ee30, 0x31fdc0, 0x320250,
    0x320580, 0x321f20, 0x3223a0, 0x322bb0, 0x3236c0, 0x323c30, 0x3244e0, 0x3246a0,
    0x324c00, 0x3266b0, 0x3267c0, 0x3268c0, 0x3276f0, 0x327a70, 0x327f90, 0x328d20,
    0x329de0, 0x32ce00, 0x32d9b0, 0x32dba0, 0x331c70, 0x334730, 0x3368b0, 0x3368e0,
    0x336910, 0x336950, 0x336d30, 0x336d60, 0x336d90, 0x336dc0, 0x336e60, 0x336f60,
    0x336fb0, 0x337ed0,
];

/// Dispatches a root; `false` when `va` is not one of the mapmaker/ui adapters.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x70e30 | 0x81480 | 0x81c10 | 0x81c40 | 0x2fa030 | 0x2fe450 | 0x308b10 | 0x308c00 |
        0x3091c0 | 0x3092e0 | 0x3096a0 | 0x309a80 | 0x30aea0 | 0x30b150 | 0x30b280 | 0x30b3a0 |
        0x30b480 | 0x30b560 | 0x30b7c0 | 0x30b9b0 | 0x30ba90 | 0x30bc50 | 0x30be60 | 0x30c210 |
        0x30c3c0 | 0x30c440 | 0x30cb30 | 0x30cb90 | 0x30ccd0 | 0x30cf60 | 0x30d120 | 0x30d300 |
        0x30d410 | 0x30d7b0 | 0x30dd40 | 0x310100 | 0x310310 | 0x3108d0 | 0x310a20 | 0x312270 |
        0x314420 | 0x314f80 | 0x314fa0 | 0x315d20 | 0x315d60 | 0x315db0 | 0x315e10 | 0x316110 |
        0x316b50 | 0x3179d0 | 0x317d70 | 0x318350 | 0x3188c0 | 0x318be0 | 0x318ea0 | 0x3190c0 |
        0x3191f0 | 0x3192e0 | 0x319300 | 0x319bd0 | 0x319da0 | 0x319e60 | 0x31ab80 | 0x31ad90 |
        0x31b6b0 | 0x31b7d0 | 0x31b8b0 | 0x31c7c0 | 0x31cb90 | 0x31d140 | 0x31d160 | 0x31d180 |
        0x31d3e0 | 0x31d580 | 0x31dc10 | 0x31eab0 | 0x31ec90 | 0x31ee30 | 0x31fdc0 | 0x320250 |
        0x320580 | 0x321f20 | 0x3223a0 | 0x322bb0 | 0x3236c0 | 0x323c30 | 0x3244e0 | 0x3246a0 |
        0x324c00 | 0x3266b0 | 0x3267c0 | 0x3268c0 | 0x3276f0 | 0x327a70 | 0x327f90 | 0x328d20 |
        0x329de0 | 0x32ce00 | 0x32d9b0 | 0x32dba0 | 0x331c70 | 0x334730 | 0x3368b0 | 0x3368e0 |
        0x336910 | 0x336950 | 0x336d30 | 0x336d60 | 0x336d90 | 0x336dc0 | 0x336e60 | 0x336f60 |
        0x336fb0 | 0x337ed0 => {}
        _ => return false,
    }
    let entry_esp = regs.esp;
    let handled = crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        crate::mapmaker_ui::run(va, true, view, guest, regs)
    });
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
export!(tsfp_lane_00070e30, 0x70e30);
export!(tsfp_lane_00081480, 0x81480);
export!(tsfp_lane_00081c10, 0x81c10);
export!(tsfp_lane_00081c40, 0x81c40);
export!(tsfp_lane_002fa030, 0x2fa030);
export!(tsfp_lane_002fe450, 0x2fe450);
export!(tsfp_lane_00308b10, 0x308b10);
export!(tsfp_lane_00308c00, 0x308c00);
export!(tsfp_lane_003091c0, 0x3091c0);
export!(tsfp_lane_003092e0, 0x3092e0);
export!(tsfp_lane_003096a0, 0x3096a0);
export!(tsfp_lane_00309a80, 0x309a80);
export!(tsfp_lane_0030aea0, 0x30aea0);
export!(tsfp_lane_0030b150, 0x30b150);
export!(tsfp_lane_0030b280, 0x30b280);
export!(tsfp_lane_0030b3a0, 0x30b3a0);
export!(tsfp_lane_0030b480, 0x30b480);
export!(tsfp_lane_0030b560, 0x30b560);
export!(tsfp_lane_0030b7c0, 0x30b7c0);
export!(tsfp_lane_0030b9b0, 0x30b9b0);
export!(tsfp_lane_0030ba90, 0x30ba90);
export!(tsfp_lane_0030bc50, 0x30bc50);
export!(tsfp_lane_0030be60, 0x30be60);
export!(tsfp_lane_0030c210, 0x30c210);
export!(tsfp_lane_0030c3c0, 0x30c3c0);
export!(tsfp_lane_0030c440, 0x30c440);
export!(tsfp_lane_0030cb30, 0x30cb30);
export!(tsfp_lane_0030cb90, 0x30cb90);
export!(tsfp_lane_0030ccd0, 0x30ccd0);
export!(tsfp_lane_0030cf60, 0x30cf60);
export!(tsfp_lane_0030d120, 0x30d120);
export!(tsfp_lane_0030d300, 0x30d300);
export!(tsfp_lane_0030d410, 0x30d410);
export!(tsfp_lane_0030d7b0, 0x30d7b0);
export!(tsfp_lane_0030dd40, 0x30dd40);
export!(tsfp_lane_00310100, 0x310100);
export!(tsfp_lane_00310310, 0x310310);
export!(tsfp_lane_003108d0, 0x3108d0);
export!(tsfp_lane_00310a20, 0x310a20);
export!(tsfp_lane_00312270, 0x312270);
export!(tsfp_lane_00314420, 0x314420);
export!(tsfp_lane_00314f80, 0x314f80);
export!(tsfp_lane_00314fa0, 0x314fa0);
export!(tsfp_lane_00315d20, 0x315d20);
export!(tsfp_lane_00315d60, 0x315d60);
export!(tsfp_lane_00315db0, 0x315db0);
export!(tsfp_lane_00315e10, 0x315e10);
export!(tsfp_lane_00316110, 0x316110);
export!(tsfp_lane_00316b50, 0x316b50);
export!(tsfp_lane_003179d0, 0x3179d0);
export!(tsfp_lane_00317d70, 0x317d70);
export!(tsfp_lane_00318350, 0x318350);
export!(tsfp_lane_003188c0, 0x3188c0);
export!(tsfp_lane_00318be0, 0x318be0);
export!(tsfp_lane_00318ea0, 0x318ea0);
export!(tsfp_lane_003190c0, 0x3190c0);
export!(tsfp_lane_003191f0, 0x3191f0);
export!(tsfp_lane_003192e0, 0x3192e0);
export!(tsfp_lane_00319300, 0x319300);
export!(tsfp_lane_00319bd0, 0x319bd0);
export!(tsfp_lane_00319da0, 0x319da0);
export!(tsfp_lane_00319e60, 0x319e60);
export!(tsfp_lane_0031ab80, 0x31ab80);
export!(tsfp_lane_0031ad90, 0x31ad90);
export!(tsfp_lane_0031b6b0, 0x31b6b0);
export!(tsfp_lane_0031b7d0, 0x31b7d0);
export!(tsfp_lane_0031b8b0, 0x31b8b0);
export!(tsfp_lane_0031c7c0, 0x31c7c0);
export!(tsfp_lane_0031cb90, 0x31cb90);
export!(tsfp_lane_0031d140, 0x31d140);
export!(tsfp_lane_0031d160, 0x31d160);
export!(tsfp_lane_0031d180, 0x31d180);
export!(tsfp_lane_0031d3e0, 0x31d3e0);
export!(tsfp_lane_0031d580, 0x31d580);
export!(tsfp_lane_0031dc10, 0x31dc10);
export!(tsfp_lane_0031eab0, 0x31eab0);
export!(tsfp_lane_0031ec90, 0x31ec90);
export!(tsfp_lane_0031ee30, 0x31ee30);
export!(tsfp_lane_0031fdc0, 0x31fdc0);
export!(tsfp_lane_00320250, 0x320250);
export!(tsfp_lane_00320580, 0x320580);
export!(tsfp_lane_00321f20, 0x321f20);
export!(tsfp_lane_003223a0, 0x3223a0);
export!(tsfp_lane_00322bb0, 0x322bb0);
export!(tsfp_lane_003236c0, 0x3236c0);
export!(tsfp_lane_00323c30, 0x323c30);
export!(tsfp_lane_003244e0, 0x3244e0);
export!(tsfp_lane_003246a0, 0x3246a0);
export!(tsfp_lane_00324c00, 0x324c00);
export!(tsfp_lane_003266b0, 0x3266b0);
export!(tsfp_lane_003267c0, 0x3267c0);
export!(tsfp_lane_003268c0, 0x3268c0);
export!(tsfp_lane_003276f0, 0x3276f0);
export!(tsfp_lane_00327a70, 0x327a70);
export!(tsfp_lane_00327f90, 0x327f90);
export!(tsfp_lane_00328d20, 0x328d20);
export!(tsfp_lane_00329de0, 0x329de0);
export!(tsfp_lane_0032ce00, 0x32ce00);
export!(tsfp_lane_0032d9b0, 0x32d9b0);
export!(tsfp_lane_0032dba0, 0x32dba0);
export!(tsfp_lane_00331c70, 0x331c70);
export!(tsfp_lane_00334730, 0x334730);
export!(tsfp_lane_003368b0, 0x3368b0);
export!(tsfp_lane_003368e0, 0x3368e0);
export!(tsfp_lane_00336910, 0x336910);
export!(tsfp_lane_00336950, 0x336950);
export!(tsfp_lane_00336d30, 0x336d30);
export!(tsfp_lane_00336d60, 0x336d60);
export!(tsfp_lane_00336d90, 0x336d90);
export!(tsfp_lane_00336dc0, 0x336dc0);
export!(tsfp_lane_00336e60, 0x336e60);
export!(tsfp_lane_00336f60, 0x336f60);
export!(tsfp_lane_00336fb0, 0x336fb0);
export!(tsfp_lane_00337ed0, 0x337ed0);
