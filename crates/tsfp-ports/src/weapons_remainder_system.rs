// SPDX-License-Identifier: GPL-3.0-or-later
//! T1940 weapon continuation boundary; guest memory remains authoritative.
pub use crate::weapons_system::{WeaponContext, WeaponDefinitions, WeaponSlot};
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
/// Per-root opt-in switch; GuestCall crosses into the neighbouring systems.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    enabled: bool,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    enabled
        && (crate::c2r::objchar_wired24::run(va, m, guest, r) // T2122 pass 24: CharacterSet fields through leaf setters
            || crate::c2r::player3_wired::run(va, m, guest, r) // T2137 pass 3: 0x1C2AF0 0x1B8950 store through PlayerSet
            || crate::c2r::player2_wired::run(va, m, guest, r) // T2137: 0x1C2090 0x1C1C50 store the player record through PlayerSet
            || crate::c2r::t1940_weapons::run(va, m, guest, r))
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Memory;
    impl GuestMemory for Memory {
        fn read8(&self, _: u32) -> u8 {
            panic!("unexpected read");
        }
        fn write8(&mut self, _: u32, _: u8) {
            panic!("unexpected write");
        }
    }
    struct Calls;
    impl GuestCall for Calls {
        fn call(&mut self, _: u32, _: &mut ExtRegisters) {
            panic!("unexpected call");
        }
    }
    #[test]
    fn disabled_and_unknown_are_inert() {
        let mut r: ExtRegisters = unsafe { core::mem::zeroed() };
        r.eax = 0xabcdef12;
        assert!(!run(0x1c0fa0, false, &mut Memory, &mut Calls, &mut r));
        assert!(!run(0xffffffff, true, &mut Memory, &mut Calls, &mut r));
        assert_eq!(r.eax, 0xabcdef12);
    }

    #[test]
    fn objchar_wired24_roots_are_registered_in_their_host_lanes() {
        let roots = crate::c2r::objchar_wired24::ROOTS;
        assert_eq!(roots, crate::leaf_lane::OBJCHAR_ROOTS24);
        assert!(crate::lane_inv::INV_ROOTS.contains(&0x1b7680));
        assert!(crate::weapons_lane::WEAPON_ROOTS.contains(&0x1c2090));
    }
}
