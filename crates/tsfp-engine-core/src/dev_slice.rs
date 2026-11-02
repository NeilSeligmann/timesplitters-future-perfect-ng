// SPDX-License-Identifier: GPL-3.0-or-later
//! T1901 dev: Tier 0 behavioural candidates; original remains default.
#[path = "system_slice.rs"]
pub mod boundary;
use boundary::{FunctionTier, Services};

pub const FUNCTIONS: [FunctionTier; 7] = [
    FunctionTier {
        address: 0x31550,
        tier: 0,
        stack_cleanup: 12,
    },
    FunctionTier {
        address: 0x31780,
        tier: 0,
        stack_cleanup: 8,
    },
    FunctionTier {
        address: 0x3377e0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x337810,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x3378c0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x385ea9,
        tier: 0,
        stack_cleanup: 4,
    },
    FunctionTier {
        address: 0x3c4720,
        tier: 0,
        stack_cleanup: 4,
    },
];

/// Bare RET methods preserve incoming EAX; adapters apply FUNCTIONS stack cleanup.
pub fn iface_noop_pop12(incoming_eax: u32) -> u32 {
    incoming_eax
}
pub fn iface_noop_pop8(incoming_eax: u32) -> u32 {
    incoming_eax
}
pub fn com_noop_pop4(incoming_eax: u32) -> u32 {
    incoming_eax
}
pub fn iface_noop_pop4(incoming_eax: u32) -> u32 {
    incoming_eax
}

pub fn print_level_chunk<H: Services>(h: &mut H) {
    let level = h.read32(0x784014);
    h.call(0x32a5b0, &[level, 0xc77668]);
    for _ in 0..3 {
        h.call(0x389b0, &[]);
    }
}

pub fn print_node_records<H: Services>(h: &mut H) {
    let mut index = 0i32;
    while index < h.read32(h.read32(0x7844a8).wrapping_add(0x81a4)) as i32 {
        let node = h
            .read32(0x7844a8)
            .wrapping_add(0x81a8)
            .wrapping_add((index as u32).wrapping_mul(0xb4));
        h.call(0x389b0, &[]);
        let record_index = h.read16(node.wrapping_add(0xc)) as i16;
        if record_index != -1 {
            let record = h
                .read32(0x7844a8)
                .wrapping_add(0xdd14)
                .wrapping_add((record_index as i32 as u32).wrapping_mul(0x8c));
            h.call(0x389b0, &[]);
            if matches!(h.read32(record.wrapping_add(0x80)), 7 | 9 | 14) {
                let mut child = 0;
                while child < h.read8(record.wrapping_add(0x76)) as u32 {
                    h.call(0x321480, &[record, child]);
                    h.call(0x389b0, &[]);
                    child += 1;
                }
            }
        }
        index = index.wrapping_add(1);
    }
}

pub fn print_items<H: Services>(h: &mut H) {
    let mut index = 0i32;
    while index < h.read32(h.read32(0x7844a8).wrapping_add(0x280)) as i32 {
        h.call(0x389b0, &[]);
        index = index.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boundary::{Memory, test_support::Host};
    #[test]
    fn noops_preserve_arbitrary_returns_and_cleanup_metadata() {
        for x in [0, 1, u32::MAX, 0x81234567] {
            assert_eq!(
                [
                    iface_noop_pop12(x),
                    iface_noop_pop8(x),
                    com_noop_pop4(x),
                    iface_noop_pop4(x)
                ],
                [x; 4]
            );
        }
        assert_eq!(
            [
                FUNCTIONS[0].stack_cleanup,
                FUNCTIONS[1].stack_cleanup,
                FUNCTIONS[5].stack_cleanup,
                FUNCTIONS[6].stack_cleanup
            ],
            [12, 8, 4, 4]
        );
    }
    #[test]
    fn debug_calls_are_preserved_and_counts_reloaded() {
        let mut h = Host::default();
        h.write32(0x784014, 0x1234);
        print_level_chunk(&mut h);
        assert_eq!(h.calls[0].args, [0x1234, 0xc77668]);
        assert_eq!(h.calls.iter().filter(|c| c.target == 0x389b0).count(), 3);
        h.calls.clear();
        h.write32(0x7844a8, 0x10000);
        h.write32(0x10280, 3);
        h.mutations.insert(0x389b0, [vec![(0x10280, 1)]].into());
        print_items(&mut h);
        assert_eq!(h.calls.len(), 1);
        h.write32(0x10280, u32::MAX);
        h.calls.clear();
        print_items(&mut h);
        assert!(h.calls.is_empty());
    }
    #[test]
    fn node_children_use_real_service_and_signed_sentinel() {
        let mut h = Host::default();
        let e = 0x10000;
        h.write32(0x7844a8, e);
        h.write32(e + 0x81a4, 2);
        h.write32(e + 0x81a8 + 0xc, 0xffff); // first node: no record
        h.write32(e + 0x81a8 + 0xb4 + 0xc, 0);
        h.write32(e + 0xdd14 + 0x80, 9);
        h.write8(e + 0xdd14 + 0x76, 2);
        print_node_records(&mut h);
        let calls: Vec<_> = h.calls.iter().filter(|c| c.target == 0x321480).collect();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].args, [e + 0xdd14, 1]);
    }
}
