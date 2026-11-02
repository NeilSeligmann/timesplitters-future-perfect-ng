// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/data_missions
// portreg: mirrors=tsfp-ports/c2r/t2040_missions_3
// portreg: mirrors=tsfp-ports/c2r/t2049_nx_003
//! S4 Tier 2 (T2230): hybrid lane adapters for the 24 `data/missions` roots that have a Tier 1 machine translation in
//! `c2r/data_missions*.rs` (T1878/T1992/T2038, differential against the lifted original) and appear in the T1884 execution census
//! (level load and unload, the slot-4C match table, the event instance tables and the timed effect table). The translations use the
//! `ExtRegisters` plus `GuestCall` contract, so this file only gates which roots the host may select and runs every root through
//! [`crate::anim_lane::guarded`] (a null or wild pointer is refused, the write footprint is rolled back and the retained original runs;
//! once a callee ran the access is a counted degraded no-op, T2177) with ESP restored to the entry value like `physics_lane.rs`.
//! Registered through `lane_inv.rs` (the `export!` symbols are defined here). Not registered: `0x236070` (needs-manual, no Tier 1) and
//! the four census roots whose ports live in other crates (`0x58270`, `0x87260`, `0x234270`, `0x275620`).
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::{data_missions, t2040_missions_3, t2049_nx_003},
    lane::LaneCpu,
    lane_inv::execute,
    memory::{GuestMemory, MappedGuest},
};

/// The data/missions lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 24] = [
    0x17730, 0x83910, 0x85920, 0x86240, 0x87880, 0x89700, 0x162060, 0x162080, 0x215790, 0x216230,
    0x218910, 0x227430, 0x227650, 0x227710, 0x227750, 0x227920, 0x228180, 0x228870, 0x234110,
    0x234190, 0x235630, 0x237480, 0x237770, 0x237920,
];

/// Dispatches a root; `false` when `va` is not one of the data/missions adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x17730 | 0x83910 | 0x85920 | 0x86240 | 0x87880 | 0x89700 | 0x162060 | 0x162080
        | 0x215790 | 0x216230 | 0x218910 | 0x227430 | 0x227650 | 0x227710 | 0x227750 | 0x227920
        | 0x228180 | 0x228870 | 0x234110 | 0x234190 | 0x235630 | 0x237480 | 0x237770 | 0x237920 => {
        }
        _ => return false,
    }
    let entry_esp = regs.esp;
    let handled = crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        (va == 0x228180 && crate::story_level_start::run(va, view, guest, regs))
            || (va == 0x228870 && crate::story_result_lane::run(va, view, guest, regs))
            || data_missions::run(va, view, guest, regs)
            || t2040_missions_3::run(va, view, guest, regs)
            || t2049_nx_003::run(va, view, guest, regs)
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
export!(tsfp_lane_00017730, 0x17730);
export!(tsfp_lane_00083910, 0x83910);
export!(tsfp_lane_00085920, 0x85920);
export!(tsfp_lane_00086240, 0x86240);
export!(tsfp_lane_00087880, 0x87880);
export!(tsfp_lane_00089700, 0x89700);
export!(tsfp_lane_00162060, 0x162060);
export!(tsfp_lane_00162080, 0x162080);
export!(tsfp_lane_00215790, 0x215790);
export!(tsfp_lane_00216230, 0x216230);
export!(tsfp_lane_00218910, 0x218910);
export!(tsfp_lane_00227430, 0x227430);
export!(tsfp_lane_00227650, 0x227650);
export!(tsfp_lane_00227710, 0x227710);
export!(tsfp_lane_00227750, 0x227750);
export!(tsfp_lane_00227920, 0x227920);
export!(tsfp_lane_00228180, 0x228180);
export!(tsfp_lane_00228870, 0x228870);
export!(tsfp_lane_00234110, 0x234110);
export!(tsfp_lane_00234190, 0x234190);
export!(tsfp_lane_00235630, 0x235630);
export!(tsfp_lane_00237480, 0x237480);
export!(tsfp_lane_00237770, 0x237770);
export!(tsfp_lane_00237920, 0x237920);
