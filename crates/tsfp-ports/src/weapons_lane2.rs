// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/t1925_weapons
//! S4 Tier 2 (T2148): hybrid lane adapters for the 109 `gameplay/weapons` roots that have a Tier 1 machine translation in
//! `c2r/t1925_weapons.rs` (T1925, differential against the lifted original, `docs/data/t1925-weapons-evidence.csv`) but no
//! lane adapter until now (the weapon-slot, pickup, record-29c callback and ammo helpers, many called every frame). The
//! translations use the `ExtRegisters` plus `GuestCall` contract of the lane, so this file only gates which roots the host
//! may select, runs each through [`crate::anim_lane::guarded`] (a null or wild pointer is refused, the write footprint is
//! rolled back and the retained original runs instead) and normalises ESP to the entry value like `physics_lane.rs`.
//! Registered through `lane_inv.rs` (the `export!` symbols are defined here).
use crate::{
    abi::{ExtRegisters, GuestCall},
    lane::LaneCpu,
    lane_inv::execute,
    memory::{GuestMemory, MappedGuest},
};

/// The weapons lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 109] = [
    0x12f30, 0x12f50, 0x12f70, 0x6bec0, 0x9e750, 0xae790, 0xae830, 0xb3e40,
    0xb4470, 0xb4680, 0xb4b60, 0xb4df0, 0xb4f90, 0xb6090, 0xb6140, 0xb6430,
    0xb74c0, 0xb9270, 0xb9340, 0xb93e0, 0xb9430, 0xb9510, 0xb9720, 0xb9790,
    0xb9920, 0xb9960, 0xb99d0, 0xb9a30, 0xb9a90, 0xb9dc0, 0xba310, 0xba8a0,
    0xbb090, 0xbb6c0, 0xbc140, 0xbc1b0, 0xbc210, 0xbc310, 0xbc4d0, 0xbc520,
    0xbcb00, 0xbcd80, 0xbd190, 0xbd650, 0xbdbd0, 0xbe180, 0xbe990, 0xbee60,
    0xbef00, 0xbf080, 0xbf360, 0xbf520, 0xbf620, 0xbf740, 0xbfa30, 0xbfdf0,
    0xc0450, 0xc0540, 0xc1360, 0xc1950, 0xc2090, 0xc2d40, 0xdc270, 0xe0ce0,
    0xe28b0, 0xe9690, 0xea9c0, 0xeaaf0, 0xf7070, 0xf88c0, 0x10e9a0, 0x142880,
    0x142900, 0x142990, 0x15db20, 0x17bb10, 0x183040, 0x18b530, 0x1b56a0, 0x1b5c70,
    0x1b6350, 0x1b7e80, 0x1b8160, 0x1b81b0, 0x1b8c70, 0x1b90a0, 0x1b9f90, 0x1ba020,
    0x1ba300, 0x1bb030, 0x1bb2f0, 0x1bb480, 0x1bc990, 0x1bd020, 0x1bd390, 0x1bd440,
    0x1bd4f0, 0x1bd600, 0x1bdd20, 0x1bde40, 0x1bf000, 0x1bf140, 0x1bf320, 0x1c0110,
    0x1c1c00, 0x1c2c00, 0x1c5e10, 0x216190, 0x34ff30,
];

/// Dispatches a root; `false` when `va` is not one of the weapons adapters.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x12f30 | 0x12f50 | 0x12f70 | 0x6bec0 | 0x9e750 | 0xae790 | 0xae830 | 0xb3e40 |
        0xb4470 | 0xb4680 | 0xb4b60 | 0xb4df0 | 0xb4f90 | 0xb6090 | 0xb6140 | 0xb6430 |
        0xb74c0 | 0xb9270 | 0xb9340 | 0xb93e0 | 0xb9430 | 0xb9510 | 0xb9720 | 0xb9790 |
        0xb9920 | 0xb9960 | 0xb99d0 | 0xb9a30 | 0xb9a90 | 0xb9dc0 | 0xba310 | 0xba8a0 |
        0xbb090 | 0xbb6c0 | 0xbc140 | 0xbc1b0 | 0xbc210 | 0xbc310 | 0xbc4d0 | 0xbc520 |
        0xbcb00 | 0xbcd80 | 0xbd190 | 0xbd650 | 0xbdbd0 | 0xbe180 | 0xbe990 | 0xbee60 |
        0xbef00 | 0xbf080 | 0xbf360 | 0xbf520 | 0xbf620 | 0xbf740 | 0xbfa30 | 0xbfdf0 |
        0xc0450 | 0xc0540 | 0xc1360 | 0xc1950 | 0xc2090 | 0xc2d40 | 0xdc270 | 0xe0ce0 |
        0xe28b0 | 0xe9690 | 0xea9c0 | 0xeaaf0 | 0xf7070 | 0xf88c0 | 0x10e9a0 | 0x142880 |
        0x142900 | 0x142990 | 0x15db20 | 0x17bb10 | 0x183040 | 0x18b530 | 0x1b56a0 | 0x1b5c70 |
        0x1b6350 | 0x1b7e80 | 0x1b8160 | 0x1b81b0 | 0x1b8c70 | 0x1b90a0 | 0x1b9f90 | 0x1ba020 |
        0x1ba300 | 0x1bb030 | 0x1bb2f0 | 0x1bb480 | 0x1bc990 | 0x1bd020 | 0x1bd390 | 0x1bd440 |
        0x1bd4f0 | 0x1bd600 | 0x1bdd20 | 0x1bde40 | 0x1bf000 | 0x1bf140 | 0x1bf320 | 0x1c0110 |
        0x1c1c00 | 0x1c2c00 | 0x1c5e10 | 0x216190 | 0x34ff30
        => {}
        _ => return false,
    }
    let entry_esp = regs.esp;
    let handled = crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        // T2137 pass 3: 0x1BC990 and 0x1C2C00 store the player record through PlayerSet (player3_wired)
        crate::objchar_writer::run(va, view, guest, regs)
            || crate::c2r::player3_wired::run(va, view, guest, regs)
            || crate::c2r::t1925_weapons::run(va, view, guest, regs)
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
export!(tsfp_lane_00012f30, 0x12f30);
export!(tsfp_lane_00012f50, 0x12f50);
export!(tsfp_lane_00012f70, 0x12f70);
export!(tsfp_lane_0006bec0, 0x6bec0);
export!(tsfp_lane_0009e750, 0x9e750);
export!(tsfp_lane_000ae790, 0xae790);
export!(tsfp_lane_000ae830, 0xae830);
export!(tsfp_lane_000b3e40, 0xb3e40);
export!(tsfp_lane_000b4470, 0xb4470);
export!(tsfp_lane_000b4680, 0xb4680);
export!(tsfp_lane_000b4b60, 0xb4b60);
export!(tsfp_lane_000b4df0, 0xb4df0);
export!(tsfp_lane_000b4f90, 0xb4f90);
export!(tsfp_lane_000b6090, 0xb6090);
export!(tsfp_lane_000b6140, 0xb6140);
export!(tsfp_lane_000b6430, 0xb6430);
export!(tsfp_lane_000b74c0, 0xb74c0);
export!(tsfp_lane_000b9270, 0xb9270);
export!(tsfp_lane_000b9340, 0xb9340);
export!(tsfp_lane_000b93e0, 0xb93e0);
export!(tsfp_lane_000b9430, 0xb9430);
export!(tsfp_lane_000b9510, 0xb9510);
export!(tsfp_lane_000b9720, 0xb9720);
export!(tsfp_lane_000b9790, 0xb9790);
export!(tsfp_lane_000b9920, 0xb9920);
export!(tsfp_lane_000b9960, 0xb9960);
export!(tsfp_lane_000b99d0, 0xb99d0);
export!(tsfp_lane_000b9a30, 0xb9a30);
export!(tsfp_lane_000b9a90, 0xb9a90);
export!(tsfp_lane_000b9dc0, 0xb9dc0);
export!(tsfp_lane_000ba310, 0xba310);
export!(tsfp_lane_000ba8a0, 0xba8a0);
export!(tsfp_lane_000bb090, 0xbb090);
export!(tsfp_lane_000bb6c0, 0xbb6c0);
export!(tsfp_lane_000bc140, 0xbc140);
export!(tsfp_lane_000bc1b0, 0xbc1b0);
export!(tsfp_lane_000bc210, 0xbc210);
export!(tsfp_lane_000bc310, 0xbc310);
export!(tsfp_lane_000bc4d0, 0xbc4d0);
export!(tsfp_lane_000bc520, 0xbc520);
export!(tsfp_lane_000bcb00, 0xbcb00);
export!(tsfp_lane_000bcd80, 0xbcd80);
export!(tsfp_lane_000bd190, 0xbd190);
export!(tsfp_lane_000bd650, 0xbd650);
export!(tsfp_lane_000bdbd0, 0xbdbd0);
export!(tsfp_lane_000be180, 0xbe180);
export!(tsfp_lane_000be990, 0xbe990);
export!(tsfp_lane_000bee60, 0xbee60);
export!(tsfp_lane_000bef00, 0xbef00);
export!(tsfp_lane_000bf080, 0xbf080);
export!(tsfp_lane_000bf360, 0xbf360);
export!(tsfp_lane_000bf520, 0xbf520);
export!(tsfp_lane_000bf620, 0xbf620);
export!(tsfp_lane_000bf740, 0xbf740);
export!(tsfp_lane_000bfa30, 0xbfa30);
export!(tsfp_lane_000bfdf0, 0xbfdf0);
export!(tsfp_lane_000c0450, 0xc0450);
export!(tsfp_lane_000c0540, 0xc0540);
export!(tsfp_lane_000c1360, 0xc1360);
export!(tsfp_lane_000c1950, 0xc1950);
export!(tsfp_lane_000c2090, 0xc2090);
export!(tsfp_lane_000c2d40, 0xc2d40);
export!(tsfp_lane_000dc270, 0xdc270);
export!(tsfp_lane_000e0ce0, 0xe0ce0);
export!(tsfp_lane_000e28b0, 0xe28b0);
export!(tsfp_lane_000e9690, 0xe9690);
export!(tsfp_lane_000ea9c0, 0xea9c0);
export!(tsfp_lane_000eaaf0, 0xeaaf0);
export!(tsfp_lane_000f7070, 0xf7070);
export!(tsfp_lane_000f88c0, 0xf88c0);
export!(tsfp_lane_0010e9a0, 0x10e9a0);
export!(tsfp_lane_00142880, 0x142880);
export!(tsfp_lane_00142900, 0x142900);
export!(tsfp_lane_00142990, 0x142990);
export!(tsfp_lane_0015db20, 0x15db20);
export!(tsfp_lane_0017bb10, 0x17bb10);
export!(tsfp_lane_00183040, 0x183040);
export!(tsfp_lane_0018b530, 0x18b530);
export!(tsfp_lane_001b56a0, 0x1b56a0);
export!(tsfp_lane_001b5c70, 0x1b5c70);
export!(tsfp_lane_001b6350, 0x1b6350);
export!(tsfp_lane_001b7e80, 0x1b7e80);
export!(tsfp_lane_001b8160, 0x1b8160);
export!(tsfp_lane_001b81b0, 0x1b81b0);
export!(tsfp_lane_001b8c70, 0x1b8c70);
export!(tsfp_lane_001b90a0, 0x1b90a0);
export!(tsfp_lane_001b9f90, 0x1b9f90);
export!(tsfp_lane_001ba020, 0x1ba020);
export!(tsfp_lane_001ba300, 0x1ba300);
export!(tsfp_lane_001bb030, 0x1bb030);
export!(tsfp_lane_001bb2f0, 0x1bb2f0);
export!(tsfp_lane_001bb480, 0x1bb480);
export!(tsfp_lane_001bc990, 0x1bc990);
export!(tsfp_lane_001bd020, 0x1bd020);
export!(tsfp_lane_001bd390, 0x1bd390);
export!(tsfp_lane_001bd440, 0x1bd440);
export!(tsfp_lane_001bd4f0, 0x1bd4f0);
export!(tsfp_lane_001bd600, 0x1bd600);
export!(tsfp_lane_001bdd20, 0x1bdd20);
export!(tsfp_lane_001bde40, 0x1bde40);
export!(tsfp_lane_001bf000, 0x1bf000);
export!(tsfp_lane_001bf140, 0x1bf140);
export!(tsfp_lane_001bf320, 0x1bf320);
export!(tsfp_lane_001c0110, 0x1c0110);
export!(tsfp_lane_001c1c00, 0x1c1c00);
export!(tsfp_lane_001c2c00, 0x1c2c00);
export!(tsfp_lane_001c5e10, 0x1c5e10);
export!(tsfp_lane_00216190, 0x216190);
export!(tsfp_lane_0034ff30, 0x34ff30);
