// SPDX-License-Identifier: GPL-3.0-or-later
//! Borrowed guest addresses rather than host-owned mirrors; record names INFERRED.
use crate::memory::GuestMemory;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VehicleObject(pub u32);
impl VehicleObject {
    pub fn state<M: GuestMemory>(self, memory: &M) -> u32 {
        memory.read32(self.0.wrapping_add(0x7c))
    }
    pub fn owner<M: GuestMemory>(self, memory: &M) -> u32 {
        memory.read32(self.0.wrapping_add(4))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VehicleGlobals {
    pub current_object: VehicleObject,
    pub current_player: u32,
    pub physics_entries: u32,
    pub frame_step: i32,
    pub elapsed: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicsEntryTable(pub u32);
impl PhysicsEntryTable {
    pub fn read<M: GuestMemory>(memory: &M) -> Self {
        Self(memory.read32(0x74c458))
    }
    /// Selects a 0x200-byte physics record using only the low 16 index bits.
    /// Address arithmetic wraps in the guest's 32-bit address space.
    ///
    /// ```
    /// use tsfp_ports::vehicles::PhysicsEntryTable;
    /// let table = PhysicsEntryTable(0xffff_ff00);
    /// assert_eq!(table.entry(0x1_0002), 0x300);
    /// ```
    pub fn entry(self, index: u32) -> u32 {
        self.0.wrapping_add((index & 0xffff).wrapping_mul(0x200))
    }
    /// Set the dword at +0x1c4 of the selected 0x200-byte entry.
    pub fn set_field<M: GuestMemory>(self, memory: &mut M, index: u32, value: u32) {
        memory.write32(self.entry(index).wrapping_add(0x1c4), value);
    }
}

#[cfg(test)]
mod quality_tests {
    use super::PhysicsEntryTable;

    #[test]
    fn physics_entry_low_word_alias_and_wrapping_grid() {
        for base in [0, 0x20000, 0xffff_ff00] {
            let table = PhysicsEntryTable(base);
            for index in [0, 1, 0xff, 0x7fff, 0xffff] {
                let expected = base.wrapping_add(index * 0x200);
                for high in [0, 1, 0x7fff, 0xffff] {
                    assert_eq!(table.entry((high << 16) | index), expected);
                }
            }
        }
    }
}
impl VehicleGlobals {
    pub fn read<M: GuestMemory>(memory: &M) -> Self {
        Self {
            current_object: VehicleObject(memory.read32(0x75bc04)),
            current_player: memory.read32(0x7b0c7c),
            physics_entries: memory.read32(0x74c458),
            frame_step: memory.read32(0x4e8cec) as i32,
            elapsed: memory.read32(0x4e7944) as i32,
        }
    }
    pub fn physics_entry(self, index: u32) -> u32 {
        PhysicsEntryTable(self.physics_entries).entry(index)
    }
}
