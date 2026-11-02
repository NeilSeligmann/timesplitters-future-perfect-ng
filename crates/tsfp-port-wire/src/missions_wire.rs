// SPDX-License-Identifier: GPL-3.0-or-later
//! T2137 library-only Missions A/B boundary, default off; docs/t-s3-missions.md.
use crate::leaf_wire::{
    missions::*,
    missions_owned::{MissionSlot, Missions},
};
use crate::memory::GuestMemory;
#[cfg(not(any(test, feature = "thread-retire")))]
pub static MISSIONS: Missions = Missions::new();
/// Unit tests (and `thread-retire` builds of the dependent crates, T3610) get one owner per thread; production keeps the static.
#[cfg(any(test, feature = "thread-retire"))]
pub static MISSIONS: TestMissions = TestMissions;
#[cfg(any(test, feature = "thread-retire"))]
pub struct TestMissions;
#[cfg(any(test, feature = "thread-retire"))]
impl core::ops::Deref for TestMissions {
    type Target = Missions;
    fn deref(&self) -> &Missions {
        std::thread_local! { static OWNER: &'static Missions = alloc::boxed::Box::leak(alloc::boxed::Box::new(Missions::new())); }
        OWNER.with(|owner| *owner)
    }
}
pub fn configure(mode: u8, selected: impl Fn(u32) -> bool) -> Result<(), u32> {
    MISSIONS.configure(mode, selected)
}
pub fn store<M: GuestMemory>(m: &mut M, address: u32, value: u32, width: u32) -> bool {
    if MISSIONS.mode() == 0 {
        return false;
    }
    MISSIONS.initialise(m);
    let key = match MISSIONS.classify(address) {
        Some(MissionSlot::Cell(i)) if i < 5 => MAIN_PTR,
        Some(MissionSlot::Cell(i)) if i < 8 => MAIN_COUNT,
        Some(MissionSlot::Cell(_)) => AUX_CELLS[0],
        Some(MissionSlot::Visited(_)) => VISITED,
        Some(MissionSlot::Table { table, .. }) => [T38_PTR, LINK_PTR, T34_PTR, T38_PTR][table],
        None => return false,
    };
    MISSIONS.store(
        m,
        address,
        crate::state_wire::mutate_state(key, value),
        width,
    )
}

/// Generic boundary for future readable mission ports. No raw slices bypass authority.
pub struct Wired<'a, M: GuestMemory>(pub &'a mut M);
impl<M: GuestMemory> GuestMemory for Wired<'_, M> {
    fn read8(&self, a: u32) -> u8 {
        MISSIONS.read8(&*self.0, a)
    }
    fn read16(&self, a: u32) -> u16 {
        if MISSIONS.mode() == 0 {
            return self.0.read16(a);
        }
        u16::from_le_bytes([self.read8(a), self.read8(a.wrapping_add(1))])
    }
    fn read_bytes(&self, a: u32, out: &mut [u8]) {
        if MISSIONS.mode() == 0 {
            self.0.read_bytes(a, out);
            return;
        }
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = self.read8(a.wrapping_add(i as u32));
        }
    }
    fn read32(&self, a: u32) -> u32 {
        MISSIONS.read32(&*self.0, a)
    }
    fn write8(&mut self, a: u32, v: u8) {
        if !store(self.0, a, u32::from(v), 1) {
            self.0.write8(a, v);
        }
    }
    fn write32(&mut self, a: u32, v: u32) {
        if !store(self.0, a, v, 4) {
            self.0.write32(a, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{vec, vec::Vec};
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
    fn persistent_leaf_setters_and_read_boundary_2000_seeds() {
        let mut m = Image(vec![0; 0x800000]);
        for (i, cell) in PTR_CELLS[..4].iter().enumerate() {
            m.write32(*cell, 0x10000 + i as u32 * 0x20000);
        }
        assert_eq!(MISSIONS.mode(), 0);
        configure(1, |_| true).unwrap();
        let mut rng = 0x2137u32;
        for seed in 0..2000u32 {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            crate::leaf_wire::set_mission_cell(&mut m, MAIN_COUNT, rng);
            crate::leaf_wire::set_mission_word(&mut m, 0x10004, rng);
            crate::leaf_wire::set_mission_half(&mut m, 0x30002, rng as u16);
            crate::leaf_wire::set_mission_byte(&mut m, 0x50003, rng as u8);
            crate::leaf_wire::mark_visited(&mut m, VISITED + seed % 32);
            crate::leaf_wire::fill_mission_dwords(&mut m, 0x70000, 8, rng);
            m.write32(MAIN_COUNT, rng ^ 1);
            m.write32(0x10004, rng ^ 1);
            let mut wired = Wired(&mut m);
            assert_eq!(wired.read32(MAIN_COUNT), rng);
            assert_eq!(wired.read32(0x10004), rng);
            assert_eq!(wired.read32(0x70000), rng);
            assert_eq!(wired.read8(VISITED + seed % 32), 1);
            assert!(wired.as_slice(0x10000, 32).is_none());
            wired.write32(MAIN_COUNT, rng);
            assert_eq!(m.read32(MAIN_COUNT), rng);
        }
        configure(0, |_| false).unwrap();
    }
}

/// T2041 read boundary backed by the persistent Missions owner.
/// Counts and table rows use the selected authority; the render cursor stays guest-owned.
/// Callers must request allocated indices within capacity (the legacy trait is infallible).
pub struct OwnedPoints<'a, M: GuestMemory>(pub &'a M);
impl<M: GuestMemory> crate::missions_system::PointTables for OwnedPoints<'_, M> {
    fn main_rows(&self) -> u32 {
        MISSIONS.read32(self.0, crate::missions_system::MAIN_ROWS)
    }
    fn links(&self) -> u32 {
        MISSIONS.read32(self.0, crate::missions_system::LINKS)
    }
    fn second_rows(&self) -> u32 {
        MISSIONS.read32(self.0, crate::missions_system::SECOND_ROWS)
    }
    fn main_count(&self) -> u32 {
        MISSIONS.read32(self.0, crate::missions_system::MAIN_COUNT)
    }
    fn link_count(&self) -> u32 {
        MISSIONS.read32(self.0, crate::missions_system::LINK_COUNT)
    }
    fn second_count(&self) -> u32 {
        MISSIONS.read32(self.0, crate::missions_system::SECOND_COUNT)
    }
    fn third_rows(&self) -> u32 {
        MISSIONS.read32(self.0, crate::missions_system::THIRD_ROWS)
    }
    fn draw_cursor(&self) -> u32 {
        self.0.read32(crate::missions_system::DRAW_CURSOR)
    }
    fn main_row(&self, index: u32) -> crate::missions_system::Row {
        self.row_at(0, index)
    }
    fn second_row(&self, index: u32) -> crate::missions_system::Row {
        self.row_at(2, index)
    }
    fn third_row(&self, index: u32) -> crate::missions_system::Row {
        self.row_at(3, index)
    }
    fn link_word(&self, index: u32, word: u32) -> u32 {
        MISSIONS
            .link_row(self.0, index)
            .expect("allocated Missions link within capacity")
            .words[word as usize]
    }
}

impl<M: GuestMemory> OwnedPoints<'_, M> {
    fn row_at(&self, table: usize, index: u32) -> crate::missions_system::Row {
        let row = MISSIONS
            .pickup_row(self.0, table, index)
            .expect("allocated Missions row within capacity");
        crate::missions_system::Row {
            key: row.words[0],
            flags: row.words[1],
            group: row.words[2] as i16,
            tag: (row.words[2] >> 16) as i16,
            position_bits: [row.words[3], row.words[4], row.words[5]],
        }
    }
}
