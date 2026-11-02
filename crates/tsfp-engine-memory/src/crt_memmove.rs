// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest payload movement used by the measured CRT/XDK realloc move path.

use crate::{GuestAddr, GuestMemory, MemoryFault};

/// Copy `count` payload bytes with memmove overlap behavior. Heap headers are caller-owned and
/// excluded. The caller supplies the measured extent (`min(old usable payload, new request)`).
pub fn move_payload<M: GuestMemory>(
    memory: &mut M,
    destination: u32,
    source: u32,
    count: u32,
) -> Result<(), MemoryFault> {
    move_payload_with(destination, source, count, |write, address, byte| {
        if write {
            memory.write(GuestAddr(address), core::slice::from_ref(byte))
        } else {
            memory.read(GuestAddr(address), core::slice::from_mut(byte))
        }
    })
}

/// Same byte ordering as [`move_payload`], with host-provided guest-byte access.
pub fn move_payload_with(
    destination: u32,
    source: u32,
    count: u32,
    mut access: impl FnMut(bool, u32, &mut u8) -> Result<(), MemoryFault>,
) -> Result<(), MemoryFault> {
    if destination > source && destination.wrapping_sub(source) < count {
        for offset in (0..count).rev() {
            let mut byte = 0;
            access(false, source.wrapping_add(offset), &mut byte)?;
            access(true, destination.wrapping_add(offset), &mut byte)?;
        }
    } else {
        for offset in 0..count {
            let mut byte = 0;
            access(false, source.wrapping_add(offset), &mut byte)?;
            access(true, destination.wrapping_add(offset), &mut byte)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::move_payload_with;

    #[test]
    fn payload_move_matches_snapshot_copy_for_alignment_overlap_and_tails() {
        const BASE: u32 = 0x1000;
        for source_offset in 0..4u32 {
            for destination_offset in 0..4u32 {
                for count in [0, 1, 2, 3, 4, 7, 8, 15, 16, 17, 24, 31] {
                    for displacement in [-9i32, -1, 0, 1, 9] {
                        let source = BASE + 64 + source_offset;
                        let destination =
                            (source as i32 + displacement) as u32 + destination_offset;
                        let mut actual: Vec<u8> = (0..256).map(|n| (n * 73 + 19) as u8).collect();
                        let mut expected = actual.clone();
                        let src = (source - BASE) as usize;
                        let dst = (destination - BASE) as usize;
                        let snapshot = expected[src..src + count as usize].to_vec();
                        expected[dst..dst + count as usize].copy_from_slice(&snapshot);

                        move_payload_with(destination, source, count, |write, address, byte| {
                            let index = (address - BASE) as usize;
                            if write {
                                actual[index] = *byte;
                            } else {
                                *byte = actual[index];
                            }
                            Ok(())
                        })
                        .unwrap();

                        assert_eq!(
                            actual, expected,
                            "src={source:#x} dst={destination:#x} n={count}"
                        );
                    }
                }
            }
        }
    }
}
