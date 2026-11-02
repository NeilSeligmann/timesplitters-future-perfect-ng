// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/engine_input
//! S4 Tier 2 (T1995): hybrid lane adapters for the 15 remaining `engine/input` roots that have a Tier 1 machine translation in
//! `c2r/engine_input.rs` (T1878/T1956, differential against the lifted original) and no lane adapter after T1988 (`input_lane.rs`
//! registered the eight heaviest): pad record merge (`0x43220`), record collection and lookup, mapping fills, the left-stick
//! edge detector, the button-sequence capture, the CRC check of the pad level buffer and the local-player words. The
//! translations use the `ExtRegisters` plus `GuestCall` contract of the lane, so this file only gates which roots the host may
//! select, runs each through [`crate::anim_lane::guarded`] (a null or wild pointer is refused, the write footprint is rolled
//! back and the retained original runs instead) and normalises ESP to the entry value like `physics_lane.rs`. Registered
//! through `lane_inv.rs` (the `export!` symbols are defined here). `0x190d60` is already a T1854 lane root; the Tier 0
//! `t1892_input` roots (`0x18feb0`, `0x190340`) are not Tier 1 and stay out. The pad input of the routes is FABRICATED.
use crate::{
    abi::{ExtRegisters, GuestCall},
    lane::LaneCpu,
    lane_inv::execute,
    memory::{GuestMemory, MappedGuest},
};

/// The input lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 15] = [
    0x190d50, 0x43220, 0x42720, 0x42bf0, 0x2586c0, 0x2588b0, 0x2fb00, 0x31fc0,
    0x373f0, 0x42a70, 0x42ae0, 0x196460, 0x196570, 0x2e7df0, 0x32b400,
];

/// Dispatches a root; `false` when `va` is not one of the input adapters.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    let entry_esp = regs.esp;
    let handled = crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        crate::c2r::engine_input::run(va, view, guest, regs)
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
export!(tsfp_lane_00190d50, 0x190d50);
export!(tsfp_lane_00043220, 0x43220);
export!(tsfp_lane_00042720, 0x42720);
export!(tsfp_lane_00042bf0, 0x42bf0);
export!(tsfp_lane_002586c0, 0x2586c0);
export!(tsfp_lane_002588b0, 0x2588b0);
export!(tsfp_lane_0002fb00, 0x2fb00);
export!(tsfp_lane_00031fc0, 0x31fc0);
export!(tsfp_lane_000373f0, 0x373f0);
export!(tsfp_lane_00042a70, 0x42a70);
export!(tsfp_lane_00042ae0, 0x42ae0);
export!(tsfp_lane_00196460, 0x196460);
export!(tsfp_lane_00196570, 0x196570);
export!(tsfp_lane_002e7df0, 0x2e7df0);
export!(tsfp_lane_0032b400, 0x32b400);
