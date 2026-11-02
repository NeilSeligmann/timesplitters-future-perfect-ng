// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/c2r/engine_object
// portreg: wraps=tsfp-ports/c2r/t2049_nx_002
//! T2137: six low-volume character/object writers behind the typed owner boundary.
//! Retained translated logic, Tier 1 FABRICATED differential evidence; Tier 2 debt.
//! See docs/t2137-remote-wiring.md. Selected per VA; default remains the C oracle.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
    objchar_writer::OwnerWrites,
};

pub const ROOTS: [u32; 6] = [0x1cb8a0, 0x1642c0, 0xdac30, 0xa6ec0, 0x58a40, 0x172180];

/// Only direct stores cross this borrowed boundary. Callees retain their own mapping.
/// The retained body owns register/stack residue; no extra ESP adjustment is applied.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    let mut writer = OwnerWrites(memory);
    if va == 0x1cb8a0 {
        crate::c2r::t2049_nx_002::run(va, &mut writer, guest, regs)
    } else {
        crate::c2r::engine_object::run(va, &mut writer, guest, regs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roots_have_one_owner_translation_and_lane_adapter() {
        for va in ROOTS {
            let owners = usize::from(crate::c2r::engine_object::ROOTS.contains(&va))
                + usize::from(crate::c2r::t2049_nx_002::ROOTS.contains(&va));
            assert_eq!(owners, 1);
            assert_eq!(crate::lane_inv::INV_ROOTS.iter().filter(|root| **root == va).count(), 1);
        }
    }
}
