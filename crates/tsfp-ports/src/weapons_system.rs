// SPDX-License-Identifier: GPL-3.0-or-later
//! T1925 weapons attachment boundary. Names INFERRED; guest memory owns all state.
pub use crate::weapons_slice::WeaponSlot;
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

/// A borrowed definition table shared by inventory, AI and weapon callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponDefinitions(pub u32);
impl WeaponDefinitions {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self(m.read32(0x4f9bac))
    }
    pub fn entry(self, index: u32) -> u32 {
        self.0.wrapping_add(index.wrapping_mul(0x29c))
    }
}
/// Frame and mode inputs shared with player, HUD and network/session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponContext {
    pub player: u32,
    pub mode: u8,
    pub tick_divisor: i32,
    pub tick_scale_bits: u32,
}
impl WeaponContext {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self {
            player: m.read32(0x7b0c7c),
            mode: m.read8(0x7de455),
            tick_divisor: m.read32(0x4e8cec) as i32,
            tick_scale_bits: m.read32(0x4e794c),
        }
    }
}
/// Per-function hybrid A/B choice; disabled and unknown roots have no side effects.
/// GuestCall remains the explicit edge to object, effects, audio, inventory and player.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    enabled: bool,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    enabled
        && (crate::objchar_writer::run(va, m, guest, r)
            || crate::c2r::t1925_weapons::run(va, m, guest, r))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, a: u32) -> u8 {
            *self.0.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }
    struct NoCalls;
    impl GuestCall for NoCalls {
        fn call(&mut self, _: u32, _: &mut ExtRegisters) {
            panic!("unexpected boundary call");
        }
    }
    fn registers() -> ExtRegisters {
        // All-zero integer and float bit patterns are valid register states.
        unsafe { core::mem::zeroed() }
    }
    #[test]
    fn disabled_and_unknown_leave_state_untouched() {
        let mut m = Memory::default();
        let mut r = registers();
        r.eax = 0x12345678;
        let mut c = NoCalls;
        assert!(!run(0xbe180, false, &mut m, &mut c, &mut r));
        assert!(!run(0xffffffff, true, &mut m, &mut c, &mut r));
        assert_eq!(r.eax, 0x12345678);
        assert!(m.0.is_empty());
    }
    #[test]
    fn context_and_definitions_borrow_guest_state() {
        let mut m = Memory::default();
        m.write32(0x4f9bac, 0xffffff00);
        m.write32(0x7b0c7c, 0x1000);
        m.write8(0x7de455, 10);
        m.write32(0x4e8cec, 3);
        m.write32(0x4e794c, 0x3f800000);
        assert_eq!(WeaponDefinitions::read(&m).entry(1), 0x19c);
        assert_eq!(
            WeaponContext::read(&m),
            WeaponContext {
                player: 0x1000,
                mode: 10,
                tick_divisor: 3,
                tick_scale_bits: 0x3f800000
            }
        );
        m.write32(0x7b0c7c, 0x2000);
        assert_eq!(WeaponContext::read(&m).player, 0x2000);
        assert_eq!(WeaponSlot(0xfffffff0).field(0x30), 0x20);
    }
    #[test]
    fn pickup_eligibility_obeys_team_and_mode() {
        let mut m = Memory::default();
        let mut r = registers();
        let mut c = NoCalls;
        r.eax = 0x1000;
        r.edx = 0x2000;
        m.write32(0x107c, 0x3000);
        m.write32(0x207c, 0x4000);
        m.write32(0x3044, 2);
        m.write32(0x400c, 0);
        assert!(run(0xbe180, true, &mut m, &mut c, &mut r));
        assert_eq!(r.eax, 0);
        r.eax = 0x1000;
        r.edx = 0x2000;
        m.write8(0x7de455, 10);
        assert!(run(0xbe180, true, &mut m, &mut c, &mut r));
        assert_eq!(r.eax, 1);
        r.eax = 0x1000;
        r.edx = 0x2000;
        m.write8(0x3024, 8);
        assert!(run(0xbe180, true, &mut m, &mut c, &mut r));
        assert_eq!(r.eax, 0);
    }
}
