// SPDX-License-Identifier: GPL-3.0-or-later
//! T1901 Story leaf system: six Tier 0 behavioural candidates, unadmitted.
#[path = "../../tsfp-engine-core/src/system_slice.rs"]
pub mod boundary;
use boundary::{FunctionTier, Memory, Register, Services};

pub const FUNCTIONS: [FunctionTier; 6] = [
    FunctionTier {
        address: 0x1e4a10,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x1e4ad0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x1e8bd0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x1e9cc0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x227be0,
        tier: 0,
        stack_cleanup: 0,
    },
    FunctionTier {
        address: 0x227c00,
        tier: 0,
        stack_cleanup: 0,
    },
];

/// Store the checkpoint buffer's guest address and byte count.
///
/// Clearing is a separate operation. A zero address or count disables it.
pub fn set_checkpoint_buffer<H: Memory>(h: &mut H, address: u32, size: u32) {
    h.write32(0x75c148, address);
    h.write32(0x75c150, size);
}

/// Zero exactly the selected byte count, including a one to three byte tail.
///
/// Guest addresses wrap at 32 bits. The stored address and count remain available
/// after clearing, so calling this twice has the same effect as calling it once.
pub fn clear_checkpoint_buffer<H: Memory>(h: &mut H) {
    let address = h.read32(0x75c148);
    if address == 0 {
        return;
    }
    let size = h.read32(0x75c150);
    if size == 0 {
        return;
    }
    for word in 0..(size / 4) {
        h.write32(address.wrapping_add(word.wrapping_mul(4)), 0);
    }
    for byte in (size & !3)..size {
        h.write8(address.wrapping_add(byte), 0);
    }
}

fn scene<H: Memory>(h: &H) -> u32 {
    h.read32(0x75bc04)
}
fn extension<H: Memory>(h: &H, object: u32) -> u32 {
    h.read32(object.wrapping_add(0x7c))
}

pub fn checkpoint_object<H: Services>(h: &mut H, entry: u32, code: u32) {
    match code {
        1 => {
            let saved = h.read8(entry.wrapping_add(0xa)) as u32 & 0x1ff;
            h.register_call(0x1e48f0, Register::Edi, saved);
            let object = h.read32(scene(h).wrapping_add(0x658));
            let ext = extension(h, object);
            // Original XOR ECX,ECX makes the apparent mask test always clear.
            let _saved = h.read8(entry.wrapping_add(0xa));
            h.write32(ext.wrapping_add(0xac), 0);
            h.write_float(ext.wrapping_add(0x10), 0.0);
        }
        2 => {
            let saved = h.call(0x1e4850, &[]) as u8;
            h.write8(entry.wrapping_add(0xa), saved);
            let object = h.read32(scene(h).wrapping_add(0x658));
            let ext = extension(h, object);
            if h.read_float(ext.wrapping_add(0x10)) > h.read_float(0x475d24) {
                h.write8(entry.wrapping_add(0xa), saved);
            }
        }
        3 => {
            if h.call(0x228010, &[0x4fea58]) != 0 {
                let slot = h.read32(0x78ced0);
                h.call(0x2274c0, &[slot, 0]);
            }
        }
        _ => {}
    }
}

pub fn checkpoint_slot_presence<H: Services>(h: &mut H, entry: u32, code: u32) {
    match code {
        2 => {
            let slot = h.read32(0x78ced0);
            let present = h.call(0x227980, &[slot, 0x29b, 0]) != 0;
            h.write8(entry.wrapping_add(0xa), u8::from(present));
        }
        1 if h.read8(entry.wrapping_add(0xa)) != 0 => {
            let mut objects = [0; 3];
            for (object, key) in objects.iter_mut().zip([0x29b, 0x2a1, 0x2a2]) {
                let slot = h.read32(0x78ced0);
                *object = h.call(0x227980, &[slot, key, 0]);
            }
            // Look up all three before any destruction, including null results.
            for object in objects {
                h.call(0xa9070, &[object]);
            }
        }
        _ => {}
    }
}

pub fn checkpoint_tagged_state<H: Services>(h: &mut H, _entry: u32, code: u32) {
    match code {
        2 => {
            let object = h.call(0x23ff20, &[0x574e4c46]);
            let enabled = h.read32(scene(h).wrapping_add(0x668)) != 0;
            let word = h.read32(extension(h, object).wrapping_add(0xac));
            h.write32(0x75b62c, word | if enabled { 0x80 } else { 0 });
            let object = h.call(0x23ff20, &[0x43594556]);
            let first_ext = extension(h, object);
            let object = h.call(0x23ff20, &[0x544c4452]);
            let high = h
                .read32(extension(h, object).wrapping_add(0xac))
                .wrapping_shl(8);
            let low = h.read32(first_ext.wrapping_add(0xac));
            h.write32(0x75b640, high | low);
            h.call(0x1e4740, &[]);
            h.call(0x1e4660, &[]);
        }
        1 => {
            let word = h.read32(0x75b62c);
            h.call(0x1e8920, &[word]);
            let word = h.read32(0x75b640);
            h.call(0x1e8a50, &[word]);
            h.call(0x1e4790, &[0]);
            h.call(0x1e46d0, &[]);
        }
        _ => {}
    }
}

fn setup_slot<H: Services>(h: &mut H, index: u32, label: u32, mode: u32) {
    h.call(0x227850, &[index, label, mode, mode, mode]);
}
fn finish_slot<H: Services>(h: &mut H) {
    let slot = h.read32(0x78ced0);
    h.call(0x227620, &[slot]);
}
fn story_coop<H: Memory>(h: &H) -> bool {
    h.read8(0x7de455) == 10 && h.read32(0x790950) as i32 > 1
}

/// Complete 2421-byte controller expressed as setup, flag-table and slot phases.
/// Every neighbouring service remains a real boundary; no fake resource/slot success.
pub fn initialize_lv4<H: Services>(h: &mut H) {
    h.call(0x128440, &[1, 0x24d, 0, 0x4fea40, 0x75bc14]);
    h.call(0x87600, &[0x5b, 8]);
    h.call(0x87600, &[0x5a, 8]);
    let state = scene(h);
    let one = h.read32(0x475c78);
    for offset in (0..0x7c4).step_by(4) {
        h.write32(state.wrapping_add(offset), 0);
    }
    h.write32(state.wrapping_add(0x680), one);
    for offset in [0x7b4, 0x7bc, 0x7b8] {
        h.write32(state.wrapping_add(offset), u32::MAX);
    }
    if h.read8(0x7de458) & 0x40 != 0 {
        let value = h.read32(0x47cff4);
        h.write32(state.wrapping_add(0x768), value);
        h.call(0x1e3710, &[]);
        let state = scene(h);
        let value = h.read32(0x47d01c);
        h.write32(state.wrapping_add(0x678), 0x1628);
        h.write32(state.wrapping_add(0x67c), value);
        return;
    }
    let value = h.read32(0x4785c8);
    h.write32(state.wrapping_add(0x678), 0x19b);
    h.write32(state.wrapping_add(0x67c), value);
    let resource = h.call(0x341850, &[1, 0x4fe460, 0, 2, (-1f32).to_bits()]);
    let state = scene(h);
    h.write32(state.wrapping_add(0x4fc), resource);
    h.call(0x616d0, &[0, 0x4fe568, 8]);
    h.call(0x616d0, &[1, 0x4fe628, 8]);
    h.call(0x61720, &[0x4fe568, 0x4fe628]);
    for &(key, mask) in OBJECT_FLAGS {
        let index = h.call(0x9cc40, &[key]);
        let flags = h
            .read32(0x715f2c)
            .wrapping_add(index.wrapping_mul(0x20))
            .wrapping_add(4);
        h.write32(flags, h.read32(flags) | mask);
    }
    setup_slot(h, 1, 0xf43, 0);
    h.call(0x2278e0, &[0x480]);
    finish_slot(h);
    setup_slot(h, 10, 0xf45, 0);
    finish_slot(h);
    setup_slot(h, 2, 0xf44, 0);
    finish_slot(h);
    setup_slot(h, 3, 0xf46, 0);
    h.call(0x2278e0, &[0x95e]);
    finish_slot(h);
    setup_slot(h, 4, 0xf47, 0);
    h.call(0x2278e0, &[0x991]);
    finish_slot(h);
    setup_slot(h, 5, 0xf48, 0);
    h.call(0x227880, &[0x281]);
    h.call(0x227a50, &[0x2532d0]);
    finish_slot(h);
    if !story_coop(h) {
        setup_slot(h, 6, 0xf49, 0);
        h.call(0x2278e0, &[0x7b5]);
        // Predicate is deliberately reloaded after helper mutation.
        if !story_coop(h) {
            setup_slot(h, 7, 0xf4a, 1);
            for key in [0x29b, 0x2a1, 0x2a2] {
                h.call(0x227880, &[key]);
            }
            h.call(0x227a50, &[0x1e4ad0]);
            finish_slot(h);
        }
    }
    setup_slot(h, 8, 0, 2);
    setup_slot(h, 9, 0, 2);
    h.call(0x2278e0, &[0xb07]);
    for index in [18, 19, 11, 12, 13, 14, 15] {
        setup_slot(h, index, 0, 2);
    }
    h.call(0x2278e0, &[0x59c]);
    setup_slot(h, 0, 0, 2);
    for key in [0x254, 0x253, 0x27c, 0x256] {
        h.call(0x227880, &[key]);
    }
    h.call(0x227a50, &[0x1e4a10]);
    finish_slot(h);
    setup_slot(h, 20, 0, 2);
    setup_slot(h, 17, 0, 2);
    for key in [0x280, 0x257, 0x276, 0x293] {
        h.call(0x227880, &[key]);
    }
    h.call(0x227a50, &[0x1e8bd0]);
    finish_slot(h);
    // Preserve per-function A/B routing through the neighbour service boundary.
    h.call(0x227be0, &[0x75b628, 0x78]);
    h.call(0x227c00, &[]);
    let state = scene(h);
    h.write32(0x75b62c, 3);
    h.write32(state.wrapping_add(0x668), 0);
    for index in [21, 22, 23] {
        setup_slot(h, index, 0, 2);
    }
    h.call(0x1e0490, &[]);
    let state = scene(h);
    let value = h.read32(0x47cff4);
    h.write32(state.wrapping_add(0x768), value);
    h.call(0x1e3710, &[]);
}

// Key/mask pairs follow the original 0x1e9dc9..0x1ea2fb lookup/write order.
const OBJECT_FLAGS: &[(u32, u32)] = &[
    (0x9d8, 0x20000),
    (0x9d9, 0x10000),
    (0x9d5, 0x20000),
    (0x9d4, 0x10000),
    (0x9e6, 0x10000),
    (0x9e5, 0x20000),
    (0x9ef, 0x20000),
    (0x9f0, 0x20000),
    (0x9f3, 0x10000),
    (0xa08, 0x10000),
    (0xa09, 0x20000),
    (0xa0a, 0x20000),
    (0xa0f, 0x10000),
    (0xa10, 0x10000),
    (0xa12, 0x20000),
    (0xa3e, 0x20000),
    (0xa3f, 0x10000),
    (0x9ff, 0x10000),
    (0xa40, 0x20000),
    (0x9cc, 0x10000),
    (0x9d0, 0x20000),
    (0xa39, 0x10000),
    (0xa4a, 0x20000),
    (0xa6b, 0x10000),
    (0xa81, 0x20000),
    (0xa82, 0x20000),
    (0xa83, 0x10000),
    (0xa85, 0x20000),
    (0xa86, 0x20000),
    (0xa87, 0x20000),
    (0xa9a, 0x20000),
    (0xa9b, 0x20000),
    (0x9c0, 0x10000),
    (0xaa3, 0x20000),
    (0xaa4, 0x20000),
    (0xaa6, 0x20000),
    (0xaaa, 0x20000),
    (0xaac, 0x20000),
    (0xabe, 0x20000),
    (0xb25, 0x20000),
];

#[cfg(test)]
mod tests {
    use super::*;
    use boundary::test_support::{Host, call_digest};
    fn host() -> Host {
        let mut h = Host::default();
        h.write32(0x75bc04, 0x10000);
        h.write32(0x78ced0, 0x7000);
        h.write32(0x715f2c, 0x20000);
        h.write32(0x475c78, 1f32.to_bits());
        h
    }
    #[test]
    fn buffer_exact_length_remainders_guards_and_wrapping() {
        for len in 0..20 {
            let mut h = host();
            for a in 0x3000..0x3020 {
                h.write8(a, 0xa5);
            }
            set_checkpoint_buffer(&mut h, 0x3000, len);
            clear_checkpoint_buffer(&mut h);
            for n in 0..32 {
                assert_eq!(h.read8(0x3000 + n), if n < len { 0 } else { 0xa5 });
            }
        }
        let mut h = host();
        set_checkpoint_buffer(&mut h, 0, 8);
        h.write8(0, 0x42);
        clear_checkpoint_buffer(&mut h);
        assert_eq!(h.read8(0), 0x42);
        set_checkpoint_buffer(&mut h, u32::MAX - 1, 4);
        for a in [u32::MAX - 1, u32::MAX, 0, 1] {
            h.write8(a, 0xaa);
        }
        clear_checkpoint_buffer(&mut h);
        for a in [u32::MAX - 1, u32::MAX, 0, 1] {
            assert_eq!(h.read8(a), 0);
        }
    }
    #[test]
    fn buffer_property_grid_preserves_neighbors_and_selection() {
        for start in 0..8u32 {
            for len in 0..65u32 {
                let base = 0x3000 + start;
                let mut h = host();
                for offset in 0..80u32 {
                    h.write8(
                        0x3000 + offset,
                        (offset as u8).wrapping_mul(17).wrapping_add(3),
                    );
                }
                set_checkpoint_buffer(&mut h, base, len);
                clear_checkpoint_buffer(&mut h);
                clear_checkpoint_buffer(&mut h);
                assert_eq!(h.read32(0x75c148), base);
                assert_eq!(h.read32(0x75c150), len);
                for offset in 0..80u32 {
                    let address = 0x3000 + offset;
                    let original = (offset as u8).wrapping_mul(17).wrapping_add(3);
                    let expected = if address >= base && address - base < len {
                        0
                    } else {
                        original
                    };
                    assert_eq!(
                        h.read8(address),
                        expected,
                        "start={start} len={len} offset={offset}"
                    );
                }
            }
        }
    }
    #[test]
    fn checkpoint_object_save_restore_clear_and_finish() {
        let mut h = host();
        h.write32(0x10658, 0x3000);
        h.write32(0x307c, 0x4000);
        h.write_float(0x4010, 3.0);
        h.returns.insert(0x1e4850, [0x1234].into());
        checkpoint_object(&mut h, 0x5000, 2);
        assert_eq!(h.read8(0x500a), 0x34);
        h.write32(0x40ac, 0xffffffff);
        checkpoint_object(&mut h, 0x5000, 1);
        assert_eq!(h.calls.last().unwrap().registers, [(Register::Edi, 0x34)]);
        assert_eq!(h.read32(0x40ac), 0);
        assert_eq!(h.read_float(0x4010), 0.0);
        h.returns.insert(0x228010, [0, 1].into());
        checkpoint_object(&mut h, 0x5000, 3);
        assert_eq!(h.calls.last().unwrap().target, 0x228010);
        checkpoint_object(&mut h, 0x5000, 3);
        assert_eq!(h.calls.last().unwrap().args, [0x7000, 0]);
        let count = h.calls.len();
        checkpoint_object(&mut h, 0x5000, 0);
        assert_eq!(h.calls.len(), count);
    }
    #[test]
    fn slot_presence_looks_up_all_before_destroy_and_reloads_slot() {
        let mut h = host();
        h.returns.insert(0x227980, [0x80000000, 11, 0, 33].into());
        checkpoint_slot_presence(&mut h, 0x5000, 2);
        assert_eq!(h.read8(0x500a), 1);
        h.calls.clear();
        h.mutations
            .insert(0x227980, [vec![(0x78ced0, 0x8000)]].into());
        checkpoint_slot_presence(&mut h, 0x5000, 1);
        assert_eq!(
            h.calls.iter().map(|c| c.target).collect::<Vec<_>>(),
            [0x227980, 0x227980, 0x227980, 0xa9070, 0xa9070, 0xa9070]
        );
        assert_eq!(h.calls[1].args[0], 0x8000);
        assert_eq!(h.calls[4].args, [0]);
        h.calls.clear();
        h.write8(0x500a, 0);
        checkpoint_slot_presence(&mut h, 0x5000, 1);
        assert!(h.calls.is_empty());
    }
    #[test]
    fn tagged_snapshot_packs_full_dwords_and_reads_after_helpers() {
        let mut h = host();
        h.returns.insert(0x23ff20, [0x3000, 0x3100, 0x3200].into());
        for (object, ext, word) in [
            (0x3000, 0x4000, 0x103),
            (0x3100, 0x4100, 0x234),
            (0x3200, 0x4200, 0x12345678),
        ] {
            h.write32(object + 0x7c, ext);
            h.write32(ext + 0xac, word);
        }
        h.write32(0x10668, 1);
        checkpoint_tagged_state(&mut h, 0, 2);
        assert_eq!(h.read32(0x75b62c), 0x183);
        assert_eq!(h.read32(0x75b640), (0x12345678u32 << 8) | 0x234);
        h.calls.clear();
        h.mutations
            .insert(0x1e8920, [vec![(0x75b640, 0xabc)]].into());
        checkpoint_tagged_state(&mut h, 0, 1);
        assert_eq!(h.calls[1].args, [0xabc]);
        assert_eq!(h.calls.last().unwrap().target, 0x1e46d0);
    }
    #[test]
    fn initializer_early_arm_resets_only_scene_prefix_and_reloads_scene() {
        let mut h = host();
        h.write32(0x107c4, 0xdeadbeef);
        h.write8(0x7de458, 0x40);
        h.write32(0x47cff4, 0x123);
        h.write32(0x47d01c, 0x456);
        h.mutations
            .insert(0x1e3710, [vec![(0x75bc04, 0x30000)]].into());
        initialize_lv4(&mut h);
        assert_eq!(h.read32(0x107c4), 0xdeadbeef);
        assert_eq!(h.read32(0x107b4), u32::MAX);
        assert_eq!(h.read_float(0x10680), 1.0);
        assert_eq!(h.read32(0x10768), 0x123);
        assert_eq!(h.read32(0x30678), 0x1628);
        assert_eq!(h.read32(0x3067c), 0x456);
        assert!(!h.calls.iter().any(|c| c.target == 0x9cc40));
        assert_eq!(call_digest(&h.calls), 0xd48530d59707e716);
    }
    #[test]
    fn initializer_ordinary_arm_all_flags_callbacks_and_slots() {
        let mut h = host();
        h.write32(0x47cff4, 0x123);
        h.returns.insert(0x9cc40, (0..40).collect());
        h.returns.insert(0x341850, [0x999].into());
        initialize_lv4(&mut h);
        assert_eq!(h.read32(0x104fc), 0x999);
        assert_eq!(call_digest(&h.calls), 0xdf7c145f3fdc7a15);
        let lookups: Vec<_> = h.calls.iter().filter(|c| c.target == 0x9cc40).collect();
        assert_eq!(lookups.len(), 40);
        for (index, ((key, mask), call)) in OBJECT_FLAGS.iter().zip(lookups).enumerate() {
            assert_eq!(call.args, [*key]);
            assert_eq!(h.read32(0x20004 + index as u32 * 0x20), *mask);
        }
        let callbacks: Vec<_> = h
            .calls
            .iter()
            .filter(|c| c.target == 0x227a50)
            .map(|c| c.args[0])
            .collect();
        assert_eq!(callbacks, [0x2532d0, 0x1e4ad0, 0x1e4a10, 0x1e8bd0]);
        let slots: Vec<_> = h
            .calls
            .iter()
            .filter(|c| c.target == 0x227850)
            .map(|c| c.args[0])
            .collect();
        assert_eq!(
            slots,
            [
                1, 10, 2, 3, 4, 5, 6, 7, 8, 9, 18, 19, 11, 12, 13, 14, 15, 0, 20, 17, 21, 22, 23
            ]
        );
        assert_eq!(h.read32(0x75b62c), 3);
        assert_eq!(h.calls.last().unwrap().target, 0x1e3710);
    }
    #[test]
    fn initializer_coop_original_service_trace() {
        let mut h = host();
        h.write8(0x7de455, 10);
        h.write32(0x790950, 2);
        h.returns.insert(0x9cc40, (0..40).collect());
        h.returns.insert(0x341850, [0x999].into());
        initialize_lv4(&mut h);
        assert_eq!(call_digest(&h.calls), 0xec230e48dc3f6220);
    }
    #[test]
    fn initializer_coop_gate_rechecked_after_service() {
        let mut h = host();
        h.write8(0x7de455, 10);
        h.write32(0x790950, 2);
        initialize_lv4(&mut h);
        assert!(
            !h.calls
                .iter()
                .any(|c| c.target == 0x227850 && matches!(c.args[0], 6 | 7))
        );
        let mut h = host();
        h.write8(0x7de455, 10);
        h.write32(0x790950, 1);
        let mut changes = std::collections::VecDeque::new();
        // Slot 6 invokes this service for the fourth time before the slot 7 gate.
        for _ in 0..3 {
            changes.push_back(vec![]);
        }
        changes.push_back(vec![(0x790950, 2)]);
        h.mutations.insert(0x2278e0, changes);
        initialize_lv4(&mut h);
        assert!(
            h.calls
                .iter()
                .any(|c| c.target == 0x227850 && c.args[0] == 6)
        );
        assert!(
            !h.calls
                .iter()
                .any(|c| c.target == 0x227850 && c.args[0] == 7)
        );
    }
}
