// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/c2r/missions_follow
// portreg: replaces=tsfp-ports/assets
// portreg: replaces=tsfp-ports/c2r/data_missions
// portreg: replaces=tsfp-ports/c2r/data_missions_2
//! T2137: shared Missions writers. Library A/B remains off by default.
//! Retained translated ABI/logic; readable typed store boundary, not a Rule 19 feature gate.
//! Direct stores only; callee writes and Tier 2 routes remain debt. See docs/t-s3-missions.md.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
pub const ROOTS: [u32; 4] = [0x2fa150, 0x9deb0, 0x9dc70, 0x23f9d0];

struct Writes<'a, M>(&'a mut M);
impl<M: GuestMemory> GuestMemory for Writes<'_, M> {
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
        if !crate::missions_wire::store(self.0, address, u32::from(value), 1) {
            self.0.write8(address, value);
        }
    }
    fn write32(&mut self, address: u32, value: u32) {
        if !crate::missions_wire::store(self.0, address, value, 4) {
            self.0.write32(address, value);
        }
    }
}

/// Explicit library root selection; no host selector or shipped profile is added.
/// Reads remain guest-backed so retained callees see the same write-through mapping.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    crate::c2r::missions_follow::run(va, &mut Writes(m), guest, regs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        leaf_wire::missions::*,
        missions_system::PointTables,
        missions_wire::{MISSIONS, OwnedPoints, configure},
    };
    struct Image(Vec<u8>);
    impl GuestMemory for Image {
        fn read8(&self, a: u32) -> u8 {
            self.0[a as usize]
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0[a as usize] = v;
        }
    }
    #[test]
    fn shared_writer_setters_and_owned_point_boundary() {
        let mut m = Image(vec![0; 0x800000]);
        m.write32(MAIN_PTR, 0x10000);
        m.write32(LINK_PTR, 0x30000);
        m.write32(MAIN_COUNT, 1);
        m.write32(crate::missions_system::DRAW_CURSOR, 0x60000);
        assert_eq!(MISSIONS.mode(), 0);
        assert_eq!(configure(1, |va| va != 0x2fa150), Err(0x2fa150));
        configure(1, |_| true).unwrap();
        let mut rng = 0x2137u32;
        for seed in 0..2000u32 {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            {
                let mut writer = Writes(&mut m);
                writer.write32(0x10000, rng);
                writer.write32(0x10008, 0xfffe0003);
                writer.write32(0x30008, rng);
                writer.write32(VISITED + seed % 8 * 4, rng);
                writer.write8(VISITED + seed % 32, seed as u8);
                writer.write32(0x60010, rng);
            }
            assert_eq!(m.read32(0x10000), rng);
            m.write32(0x10000, rng ^ 1);
            m.write32(0x30008, rng ^ 1);
            let points = OwnedPoints(&m);
            assert_eq!(points.main_row(0).key, rng);
            assert_eq!(points.main_row(0).group, 3);
            assert_eq!(points.main_row(0).tag, -2);
            assert_eq!(points.link_word(0, 2), rng);
            assert_eq!(points.find_key(rng), Some(0));
            assert_eq!(points.draw_cursor(), 0x60000);
            assert_eq!(m.read32(0x60010), rng);
        }
        configure(0, |_| false).unwrap();
        Writes(&mut m).write32(VISITED, 0x12345678);
        assert_eq!(m.read32(VISITED), 0x12345678);
        assert_eq!(OwnedPoints(&m).main_row(0).key, m.read32(0x10000));
    }
}
