// SPDX-License-Identifier: GPL-3.0-or-later
//! Fixed-stride guest freelist initialization at 0x156DB0.
//!
//! The descriptor stores the node buffer, free-list head, capacity, and free count at offsets
//! 0, 4, 8, and 12. Nodes are linked in ascending storage order, so the final node becomes the
//! head. This pool has no generation field; raw guest pointers retain the original stale-after-
//! reuse behavior. `GuestMemory` preserves ordered writes and fault locations.
use crate::{GuestMemory, MemoryFault};
use tsfp_types::GuestAddr;

/// Initialize a fixed-stride free list and return `(head, end)` as left by the original leaf.
pub fn initialize_freelist<M: GuestMemory>(
    memory: &mut M,
    pool: u32,
    stride: u32,
    storage: u32,
    count: u32,
) -> Result<(u32, u32), MemoryFault> {
    memory.write_u32(GuestAddr(pool), storage)?;
    memory.write_u32(GuestAddr(pool.wrapping_add(8)), count)?;
    memory.write_u32(GuestAddr(pool.wrapping_add(12)), count)?;

    let mut node = storage;
    let mut head = 0;
    for _ in 0..count {
        memory.write_u32(GuestAddr(node), head)?;
        head = node;
        node = node.wrapping_add(stride);
    }
    memory.write_u32(GuestAddr(pool.wrapping_add(4)), head)?;
    Ok((head, node))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MappedMemory;

    #[test]
    fn fixed_pool_initializer_writes_descriptor_and_descending_links() {
        let base = 0x0073_2000;
        let mut bytes = vec![0xA5; 0x2000];
        let mut memory = MappedMemory {
            base: GuestAddr(base),
            bytes: &mut bytes,
        };
        let (head, end) =
            initialize_freelist(&mut memory, 0x0073_29F0, 0x24, 0x0073_27B0, 0x10).unwrap();
        assert_eq!((head, end), (0x0073_29CC, 0x0073_29F0));
        assert_eq!(
            memory.read_u32(GuestAddr(0x0073_29F0)).unwrap(),
            0x0073_27B0
        );
        assert_eq!(memory.read_u32(GuestAddr(0x0073_29F4)).unwrap(), head);
        assert_eq!(memory.read_u32(GuestAddr(0x0073_29F8)).unwrap(), 0x10);
        assert_eq!(memory.read_u32(GuestAddr(0x0073_29FC)).unwrap(), 0x10);
        assert_eq!(memory.read_u32(GuestAddr(0x0073_27B0)).unwrap(), 0);
        for index in 1..16 {
            let node = 0x0073_27B0 + index * 0x24;
            assert_eq!(memory.read_u32(GuestAddr(node)).unwrap(), node - 0x24);
        }
        assert_eq!(
            memory.read_u32(GuestAddr(0x0073_2A00)).unwrap(),
            0xA5A5_A5A5
        );
    }
}
