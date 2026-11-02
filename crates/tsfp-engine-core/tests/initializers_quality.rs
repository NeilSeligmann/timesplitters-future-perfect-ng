// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic Tier 0 invariants for the typed core constructors.
mod support;

use support::Mem;
use tsfp_engine_core::initializers::{SlotArrays, ThresholdMasks};
use tsfp_engine_object::closure::memory::GuestMemory;

#[test]
fn threshold_masks_assign_bits_in_level_then_channel_order() {
    for a in 0..8 {
        for b in 0..8 {
            for c in 0..8 {
                let thresholds = [a, b, c];
                let result = ThresholdMasks::new(thresholds);
                let mut expected = [0u32; 3];
                let mut next_bit = 0;
                for level in [1, 2, 4] {
                    for channel in 0..3 {
                        if level < thresholds[channel] {
                            expected[channel] |= 1 << next_bit;
                            next_bit += 1;
                        }
                    }
                }
                assert_eq!(result.masks, expected, "thresholds={thresholds:?}");
                assert_eq!(
                    (result.masks[0] | result.masks[1] | result.masks[2]).count_ones(),
                    next_bit
                );
                assert_eq!(result.masks[0] & result.masks[1], 0);
                assert_eq!(result.masks[0] & result.masks[2], 0);
                assert_eq!(result.masks[1] & result.masks[2], 0);
            }
        }
    }
}

#[test]
fn slot_arrays_write_exactly_their_record_even_at_wrapping_address() {
    for seed in 0..64u32 {
        let base = (u32::MAX - 0x83).wrapping_add(seed * 4);
        let mut memory = Mem::default();
        memory.write32(base.wrapping_sub(4), 0xA5A5_A5A5);
        memory.write32(base.wrapping_add(0x84), 0x5A5A_5A5A);
        let value = seed.wrapping_mul(0x1020_3041);
        SlotArrays::new(value).write(&mut memory, base);
        for index in 0..33u32 {
            let expected = if index < 11 { value } else { 0 };
            assert_eq!(memory.read32(base.wrapping_add(index * 4)), expected);
        }
        assert_eq!(memory.read32(base.wrapping_sub(4)), 0xA5A5_A5A5);
        assert_eq!(memory.read32(base.wrapping_add(0x84)), 0x5A5A_5A5A);
    }
}
