// SPDX-License-Identifier: GPL-3.0-or-later
//! Memory-unit slot bookkeeping of `src/input/mu_device.c` (T3926 slice 2): which slots are mounted,
//! which have an eject waiting for the unmount, and the stack of free `\Device\MU_<n>` ids.
//! Images, FATX volumes and file paths stay C-owned; this table answers the decisions the C file
//! used to take from its static arrays.

/// Ports times slots of the original device table (8 entries, slot >= 2 aliases the next port).
pub const MU_ENTRIES: usize = 8;

/// Original table index of a port/slot pair (0046D7E6..0046D7FA), `None` when out of range.
pub fn index_of(port: u32, slot: u32) -> Option<usize> {
    let index = port.checked_mul(2)?.checked_add(slot)?;
    (port < 0x1000 && slot < 0x1000 && (index as usize) < MU_ENTRIES).then_some(index as usize)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuTable {
    mounted_mask: u32,
    mounted: [bool; MU_ENTRIES],
    guest_backed: [bool; MU_ENTRIES],
    device_id: [u32; MU_ENTRIES],
    eject_pending: [bool; MU_ENTRIES],
    free_ids: [u32; MU_ENTRIES],
    free_count: usize,
}

impl Default for MuTable {
    fn default() -> Self {
        Self {
            mounted_mask: 0,
            mounted: [false; MU_ENTRIES],
            guest_backed: [false; MU_ENTRIES],
            device_id: [0; MU_ENTRIES],
            eject_pending: [false; MU_ENTRIES],
            free_ids: std::array::from_fn(|i| i as u32),
            free_count: MU_ENTRIES,
        }
    }
}

impl MuTable {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn mounted_mask(&self) -> u32 {
        self.mounted_mask
    }

    pub fn is_mounted(&self, index: usize) -> bool {
        self.mounted_mask & (1 << index) != 0
    }

    pub fn is_guest_backed(&self, index: usize) -> bool {
        self.guest_backed[index]
    }

    pub fn device_id(&self, index: usize) -> u32 {
        self.device_id[index]
    }

    /// Id the next guest-backed mount would take (top of the stack).
    pub fn peek_free_id(&self) -> Option<u32> {
        self.free_count.checked_sub(1).map(|top| self.free_ids[top])
    }

    /// Record a finished mount; `guest_id` is the id from [`peek_free_id`](Self::peek_free_id)
    /// when the volume was also published in the guest namespace.
    pub fn mark_mounted(&mut self, index: usize, guest_id: bool) {
        if guest_id && let Some(id) = self.peek_free_id() {
            self.free_count -= 1;
            self.device_id[index] = id;
            self.guest_backed[index] = true;
        }
        self.mounted[index] = true;
        self.mounted_mask |= 1 << index;
    }

    /// Record an unmount; returns whether a guest namespace entry was released.
    pub fn mark_unmounted(&mut self, index: usize) -> bool {
        let guest = self.guest_backed[index];
        if guest {
            self.free_ids[self.free_count] = self.device_id[index];
            self.free_count += 1;
            self.guest_backed[index] = false;
        }
        self.mounted[index] = false;
        self.mounted_mask &= !(1 << index);
        guest
    }

    pub fn set_eject_pending(&mut self, index: usize, pending: bool) {
        self.eject_pending[index] = pending;
    }

    pub fn eject_pending(&self, index: usize) -> bool {
        self.eject_pending[index]
    }

    pub fn mounted_flag(&self, index: usize) -> bool {
        self.mounted[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_matches_original_aliasing() {
        assert_eq!(index_of(0, 0), Some(0));
        assert_eq!(index_of(1, 0), Some(2));
        assert_eq!(index_of(0, 3), Some(3));
        assert_eq!(index_of(3, 1), Some(7));
        assert_eq!(index_of(4, 0), None);
        assert_eq!(index_of(0x1000, 0), None);
        assert_eq!(index_of(u32::MAX, 1), None);
    }

    #[test]
    fn guest_ids_are_a_stack() {
        let mut t = MuTable::default();
        assert_eq!(t.peek_free_id(), Some(7));
        t.mark_mounted(2, true);
        t.mark_mounted(5, true);
        assert_eq!((t.device_id(2), t.device_id(5)), (7, 6));
        assert_eq!(t.mounted_mask(), 0b100100);
        assert!(t.mark_unmounted(2));
        assert_eq!(t.peek_free_id(), Some(7));
        assert!(!t.mark_unmounted(2) && !t.is_mounted(2));
        t.mark_mounted(0, false);
        assert!(t.is_mounted(0) && !t.is_guest_backed(0) && t.peek_free_id() == Some(7));
    }

    #[test]
    fn exhaustion_and_reset() {
        let mut t = MuTable::default();
        for i in 0..MU_ENTRIES {
            t.mark_mounted(i, true);
        }
        assert_eq!(t.peek_free_id(), None);
        t.set_eject_pending(3, true);
        t.reset();
        assert_eq!(t, MuTable::default());
        assert!(!t.eject_pending(3));
    }
}
