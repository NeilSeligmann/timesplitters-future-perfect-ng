// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/engine_physics
//! S4 Tier 2 (T2099): lane adapters for the 51 `engine/physics` roots that have a Tier 1 machine translation in
//! `c2r/engine_physics.rs` (T1878, differential against the lifted original): the ray casts over object boxes and room lists
//! (0x919D0, 0xA7A00, 0xC3B10, 0xC3C50, 0xC3E60, 0xA46A0, 0xA2CD0), the node and triangle tests (0x47D50, 0x475C0, 0x477B0,
//! 0x49C60), the box sweep family (0x4A800 .. 0x4B3B0) and the contact record helpers. The translations already use the
//! `ExtRegisters` plus `GuestCall` contract of the lane, so this file only gates which roots the host may select and
//! normalises ESP like `char_lane.rs`. Registered through `lane_inv.rs` (the `export!` symbols are defined here).
//! The leaves 0x47B30 (`tsfp-engine-physics::fp_leaves`), 0x47BD0 and 0x913F0 (`tsfp-gameplay::sweep_cells`) are readable
//! ports with a different calling form and no lane adapter, so they stay reached through the lifted original and the
//! translations above call them as guest functions.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r,
    lane::LaneCpu,
    lane_inv::execute,
    memory::{GuestMemory, MappedGuest},
};

/// Dispatches a root; `false` when `va` is not one of the physics adapters.
///
/// The c2r bodies end with ESP at the entry value, the host adapter pops the return slot and the callee-cleaned arguments
/// itself, so ESP is restored to the entry value here.
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    let entry_esp = regs.esp;
    let ran = match va {
        0x919d0 | 0xa2cd0 | 0x47d50 | 0x91960 | 0xc38f0 | 0xa7a00 |
        0xc3b10 | 0xa46a0 | 0x475c0 | 0x91390 | 0x48590 | 0x49370 |
        0xd6a70 | 0xc3c50 | 0x1665e0 | 0x1665f0 | 0xc3e60 | 0x167ca0 |
        0x16b8c0 | 0x477b0 | 0x48350 | 0x484d0 | 0x49c60 | 0x4a270 |
        0x4a800 | 0x4a9d0 | 0x4ae00 | 0x4b310 | 0x4b3b0 | 0x4fe00 |
        0x52550 | 0x913b0 | 0x913d0 | 0x92360 | 0xa3050 | 0xa3140 |
        0xa3500 | 0x128760 | 0x128c60 | 0x128ef0 | 0x129ca0 | 0x129f50 |
        0x166b60 | 0x16b130 | 0x16b550 | 0x16b590 | 0x16b800 | 0x16b850 |
        0x16b870 | 0x16b8a0 | 0x16b900 => c2r::engine_physics::run(va, m, guest, regs),
        _ => return false,
    };
    regs.esp = entry_esp;
    ran
}

/// The lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 51] = [
    0x919d0, 0xa2cd0, 0x47d50, 0x91960, 0xc38f0, 0xa7a00, 0xc3b10, 0xa46a0,
    0x475c0, 0x91390, 0x48590, 0x49370, 0xd6a70, 0xc3c50, 0x1665e0, 0x1665f0,
    0xc3e60, 0x167ca0, 0x16b8c0, 0x477b0, 0x48350, 0x484d0, 0x49c60, 0x4a270,
    0x4a800, 0x4a9d0, 0x4ae00, 0x4b310, 0x4b3b0, 0x4fe00, 0x52550, 0x913b0,
    0x913d0, 0x92360, 0xa3050, 0xa3140, 0xa3500, 0x128760, 0x128c60, 0x128ef0,
    0x129ca0, 0x129f50, 0x166b60, 0x16b130, 0x16b550, 0x16b590, 0x16b800, 0x16b850,
    0x16b870, 0x16b8a0, 0x16b900,
];

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
    export!(tsfp_lane_000919d0, 0x919d0); export!(tsfp_lane_000a2cd0, 0xa2cd0); export!(tsfp_lane_00047d50, 0x47d50);
    export!(tsfp_lane_00091960, 0x91960); export!(tsfp_lane_000c38f0, 0xc38f0); export!(tsfp_lane_000a7a00, 0xa7a00);
    export!(tsfp_lane_000c3b10, 0xc3b10); export!(tsfp_lane_000a46a0, 0xa46a0); export!(tsfp_lane_000475c0, 0x475c0);
    export!(tsfp_lane_00091390, 0x91390); export!(tsfp_lane_00048590, 0x48590); export!(tsfp_lane_00049370, 0x49370);
    export!(tsfp_lane_000d6a70, 0xd6a70); export!(tsfp_lane_000c3c50, 0xc3c50); export!(tsfp_lane_001665e0, 0x1665e0);
    export!(tsfp_lane_001665f0, 0x1665f0); export!(tsfp_lane_000c3e60, 0xc3e60); export!(tsfp_lane_00167ca0, 0x167ca0);
    export!(tsfp_lane_0016b8c0, 0x16b8c0); export!(tsfp_lane_000477b0, 0x477b0); export!(tsfp_lane_00048350, 0x48350);
    export!(tsfp_lane_000484d0, 0x484d0); export!(tsfp_lane_00049c60, 0x49c60); export!(tsfp_lane_0004a270, 0x4a270);
    export!(tsfp_lane_0004a800, 0x4a800); export!(tsfp_lane_0004a9d0, 0x4a9d0); export!(tsfp_lane_0004ae00, 0x4ae00);
    export!(tsfp_lane_0004b310, 0x4b310); export!(tsfp_lane_0004b3b0, 0x4b3b0); export!(tsfp_lane_0004fe00, 0x4fe00);
    export!(tsfp_lane_00052550, 0x52550); export!(tsfp_lane_000913b0, 0x913b0); export!(tsfp_lane_000913d0, 0x913d0);
    export!(tsfp_lane_00092360, 0x92360); export!(tsfp_lane_000a3050, 0xa3050); export!(tsfp_lane_000a3140, 0xa3140);
    export!(tsfp_lane_000a3500, 0xa3500); export!(tsfp_lane_00128760, 0x128760); export!(tsfp_lane_00128c60, 0x128c60);
    export!(tsfp_lane_00128ef0, 0x128ef0); export!(tsfp_lane_00129ca0, 0x129ca0); export!(tsfp_lane_00129f50, 0x129f50);
    export!(tsfp_lane_00166b60, 0x166b60); export!(tsfp_lane_0016b130, 0x16b130); export!(tsfp_lane_0016b550, 0x16b550);
    export!(tsfp_lane_0016b590, 0x16b590); export!(tsfp_lane_0016b800, 0x16b800); export!(tsfp_lane_0016b850, 0x16b850);
    export!(tsfp_lane_0016b870, 0x16b870); export!(tsfp_lane_0016b8a0, 0x16b8a0); export!(tsfp_lane_0016b900, 0x16b900);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_are_unique_and_match_the_dispatch_table() {
        let mut sorted = ROOTS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ROOTS.len());
        assert_eq!(ROOTS.to_vec(), c2r::engine_physics::ROOTS.to_vec());
        let text = include_str!("physics_lane.rs");
        let table = text.split("match va {").nth(1).expect("dispatch table").split("_ => return false").next().expect("end");
        assert_eq!(table.matches("0x").count(), ROOTS.len());
    }
}
