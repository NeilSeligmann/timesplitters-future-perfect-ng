// SPDX-License-Identifier: GPL-3.0-or-later
//! T2137: readable write boundary for existing weapon timer and pickup lane roots.
//! The retained translation supplies ABI/logic; this boundary routes each store through
//! the typed ObjectPool/CharacterSet decoder and write-through setter. Other bases stay raw.
//! This is Own-stage wiring, not a claim that the retained timer/pickup logic is reconstructed.
//! Verification Tier 1: 2000 seeds/root, O0 and O3 differential against lifted;
//! FABRICATED fixtures and stub callees. Tier 2 remains debt. See docs/t2137-remote-wiring.md.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

// portreg: wraps=tsfp-ports/weapons_lane2
// Existing weapon ports retain ownership; these roots select only the store wrapper.
pub const ROOTS: [u32; 3] = [0x1bde40, 0xc1950, 0xc2090];

/// A borrowed writer, with no persistent authority over guest data. Reads see prior writes
/// immediately, including writes performed by a callee through the retained guest mapping.
pub(crate) struct OwnerWrites<'a, M>(pub(crate) &'a mut M);
impl<M: GuestMemory> GuestMemory for OwnerWrites<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn read16(&self, address: u32) -> u16 {
        self.0.read16(address)
    }
    fn read32(&self, address: u32) -> u32 {
        self.0.read32(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        crate::leaf_wire::set_obj_byte(self.0, address, value);
    }
    fn write32(&mut self, address: u32, value: u32) {
        crate::leaf_wire::set_obj_word(self.0, address, value);
    }
}

/// Called only after the existing per-function A/B selection. Does not add lane roots,
/// change ESP handling or intercept stores made inside retained callees.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    crate::c2r::t1925_weapons::run(va, &mut OwnerWrites(memory), guest, regs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[derive(Clone, Default)]
    struct Ram(BTreeMap<u32, u8>);
    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            *self.0.get(&address).unwrap_or(&0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    #[test]
    fn owner_boundary_preserves_width_alias_and_raw_stores() {
        let mut plain = Ram::default();
        plain.write32(crate::leaf_wire::character::TABLE, 0x400000);
        plain.write32(crate::leaf_wire::character::TABLE + 4, 0x400000);
        plain.write32(0x72f5e8, 0x500000);
        plain.write32(0x7b4fac, 2);
        let mut owned = plain.clone();
        let mut random = 0x7137u32;
        for seed in 0..2000u32 {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            let address = match seed % 5 {
                0 => 0x400000 + seed % 0xea0,
                1 => 0x500000 + seed % 0x5d0,
                2 => 0x7b54c0 + seed % 32,
                3 => 0x600000 + seed,
                _ => 0x400e9f, // cross the owned extent: raw fallback
            };
            if seed & 1 == 0 {
                plain.write32(address, random);
                OwnerWrites(&mut owned).write32(address, random);
            } else {
                plain.write8(address, random as u8);
                OwnerWrites(&mut owned).write8(address, random as u8);
            }
            for address in plain.0.keys().chain(owned.0.keys()) {
                assert_eq!(
                    plain.read8(*address),
                    owned.read8(*address),
                    "seed {seed}, address {address:x}"
                );
            }
        }
    }
    #[test]
    fn roots_already_have_lane_adapters() {
        for root in ROOTS {
            assert!(crate::weapons_lane2::ROOTS.contains(&root));
            assert!(crate::lane_inv::INV_ROOTS.contains(&root));
        }
    }
}
