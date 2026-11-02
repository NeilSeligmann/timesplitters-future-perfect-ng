// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/c2r/player_wired
//! T2137: object/character stores of the existing weapon-release player root.
//! Borrowed typed setter boundary; retained translated logic and PlayerSet routing.
//! Tier 1 FABRICATED differential fixtures; Tier 2 remains debt. A/B default off.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
    objchar_writer::OwnerWrites,
};

pub const ROOTS: [u32; 1] = [0x1bc890];

/// No extra register changes, guest reads or retained-callee interception.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    crate::c2r::player_wired::run(va, &mut OwnerWrites(memory), guest, regs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_keeps_existing_translation_and_lane() {
        for root in ROOTS {
            assert!(crate::c2r::player_wired::ROOTS.contains(&root));
            assert_eq!(
                crate::lane_inv::INV_ROOTS
                    .iter()
                    .filter(|va| **va == root)
                    .count(),
                1
            );
        }
    }
}
