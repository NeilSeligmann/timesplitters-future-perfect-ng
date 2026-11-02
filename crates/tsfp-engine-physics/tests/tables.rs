// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::BTreeMap;
use tsfp_engine_physics::{memory::GuestMemory, tables::*};
#[derive(Default, Clone, PartialEq, Eq, Debug)]
struct Mem(BTreeMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}
fn fixture() -> Mem {
    let mut m = Mem::default();
    m.write32(SLOTS, 0x10000);
    m.write32(SHAPES, 0x20000);
    m
}
fn count(m: &mut Mem, value: u16) {
    m.write8(USED_COUNT, value as u8);
    m.write8(USED_COUNT + 1, (value >> 8) as u8);
}
#[test]
fn first_free_last_slot_full_and_wrapping_count() {
    let mut m = fixture();
    count(&mut m, u16::MAX);
    m.write32(0x1007c, 0xaabbccdd);
    m.write32(0x10078, 999);
    assert_eq!(run(&mut m, 0x16b130, true, [0; 3], false), Some(0));
    assert_eq!(m.read16(USED_COUNT), 0);
    assert_eq!(m.read32(0x10058), 1);
    assert_eq!(m.read32(0x10078), 0);
    assert_eq!(m.read32(0x1007c), 0xaabbccdd);
    for i in 0..SLOT_LIMIT - 1 {
        m.write32(0x10058 + (i << 7), 1);
    }
    assert_eq!(allocate(&mut m), 92);
    assert_eq!(allocate(&mut m), -1);
}
#[test]
fn every_free_slot_is_selected_before_later_slots() {
    for free in 0..SLOT_LIMIT {
        let mut m = fixture();
        count(&mut m, u16::MAX);
        for index in 0..SLOT_LIMIT {
            let slot = Slot::resolve(&m, index);
            m.write32(slot.0 + 0x58, if index == free { 0 } else { 1 });
            m.write32(slot.0 + 0x78, index + 100);
        }
        assert_eq!(allocate(&mut m), free as i32, "free slot {free}");
        assert_eq!(m.read16(USED_COUNT), 0, "count wraps for slot {free}");
        for index in 0..SLOT_LIMIT {
            let slot = Slot::resolve(&m, index);
            assert_eq!(m.read32(slot.0 + 0x58), 1, "used flag for slot {index}");
            assert_eq!(
                m.read32(slot.0 + 0x78),
                if index == free { 0 } else { index + 100 },
                "key for slot {index}"
            );
        }
    }
}
#[test]
fn find_scans_all_slots_and_requires_active_and_nonzero_count() {
    let mut m = fixture();
    count(&mut m, 1);
    Slot::resolve(&m, 0).set_key(&mut m, 77);
    Slot::resolve(&m, 92).set_key(&mut m, 77);
    m.write32(0x10058 + (92 << 7), 3);
    assert_eq!(run(&mut m, 0x16b8c0, true, [77, 0, 0], false), Some(92));
    m.write32(0x10058, 1);
    assert_eq!(find_key(&m, 77), 0);
    count(&mut m, 0);
    assert_eq!(find_key(&m, 77), -1);
    count(&mut m, 1);
    assert_eq!(find_key(&m, 78), -1);
}
#[test]
fn setters_preserve_float_payloads_and_unchecked_wrapping_indices() {
    let mut m = fixture();
    let values = [0x7fc01234, 0x80000000];
    for (va, offset) in [(0x16b800, 0x68), (0x16b870, 0x40)] {
        assert_eq!(
            run(&mut m, va, true, [3, values[0], values[1]], false),
            Some(0)
        );
        assert_eq!(m.read32(0x10180 + offset), values[0]);
        assert_eq!(m.read32(0x10180 + offset + 4), values[1]);
    }
    run(&mut m, 0x16b8a0, true, [3, values[0], 0], false);
    assert_eq!(m.read32(0x101e4), values[0]);
    run(&mut m, 0x16b850, true, [u32::MAX, 0xdeadbeef, 0], false);
    assert_eq!(m.read32(0xfff8), 0xdeadbeef);
}
#[test]
fn contact_snapshot_differs_from_sequential_vector_alias() {
    let mut m = fixture();
    let slot = Slot::resolve(&m, 0);
    for i in 0..4 {
        m.write32(0x10044 + i * 4, 10 + i);
    }
    run(&mut m, 0x16b550, true, [0, 0x10044, 0], false);
    assert_eq!(
        (0..4)
            .map(|i| m.read32(0x10048 + i * 4))
            .collect::<Vec<_>>(),
        vec![10, 11, 12, 13]
    );
    for i in 0..3 {
        m.write32(0x1001c + i * 4, 20 + i);
    }
    run(&mut m, 0x16b590, true, [0, 0, 0x1001c], false);
    assert_eq!(
        (0..3)
            .map(|i| m.read32(slot.0 + 0x20 + i * 4))
            .collect::<Vec<_>>(),
        vec![20; 3]
    );
}
#[test]
fn shape_handles_zero_gate_mask_and_sequential_position() {
    let mut m = fixture();
    let shape = Shape::resolve(&m, 0xabcd0003);
    assert_eq!(shape, Shape(0x20600));
    m.write8(shape.0 + 0x1c9, 0xa5);
    assert_eq!(
        run(&mut m, 0x166b60, true, [0xf0, 0xabcd0003, 0], false),
        Some(0xa0)
    );
    assert_eq!(shape_mask(&m, 0, 3), 1);
    assert_eq!(shape_mask(&m, 5, 0), 1);
    m.write8(0x201c9, 0xc3);
    assert_eq!(shape_mask(&m, 0xff, 0x10000), 0xc3);
    for i in 0..3 {
        m.write32(shape.0 + 0x13c + i * 4, 100 + i);
    }
    run(
        &mut m,
        0x167ca0,
        true,
        [0xabcd0003, shape.0 + 0x13c, 0],
        false,
    );
    assert_eq!(
        (0..3)
            .map(|i| m.read32(shape.0 + 0x140 + i * 4))
            .collect::<Vec<_>>(),
        vec![100; 3]
    );
}
#[test]
fn nullable_outputs_and_write_then_read_alias() {
    let mut m = fixture();
    m.write32(0x10070, 123);
    m.write32(0x10074, 456);
    run(&mut m, 0x16b900, true, [0, 0x10074, 0x30000], false);
    assert_eq!(m.read32(0x30000), 123);
    run(&mut m, 0x16b900, true, [0, 0, 0x30004], false);
    assert_eq!(m.read32(0x30004), 123);
    let before = m.clone();
    run(&mut m, 0x16b900, true, [0, 0, 0], false);
    assert_eq!(m, before);
}
#[test]
fn contact_roundtrip_direction_and_overlap() {
    for backwards in [false, true] {
        let mut m = fixture();
        let delta = |i: u32| {
            if backwards {
                0u32.wrapping_sub(i * 4)
            } else {
                i * 4
            }
        };
        for i in 0..8 {
            m.write32(0x70aa58u32.wrapping_add(delta(i)), i + 100);
        }
        run(&mut m, 0x91390, true, [0; 3], backwards);
        run(&mut m, 0x913d0, true, [0x30000, 0, 0], backwards);
        for i in 0..8 {
            assert_eq!(m.read32(0x30000u32.wrapping_add(delta(i))), i + 100);
        }
        run(&mut m, 0x913b0, true, [0x30000, 0, 0], backwards);
        for i in 0..8 {
            assert_eq!(m.read32(0x70aaacu32.wrapping_add(delta(i))), i + 100);
        }
    }
    let mut m = fixture();
    for i in 0..8 {
        m.write32(0x70aaa8 + i * 4, i + 10);
    }
    import_contact(&mut m, 0x70aaa8, false);
    for i in 0..8 {
        assert_eq!(m.read32(0x70aaac + i * 4), 10);
    }
}
#[test]
fn disabled_and_unknown_roots_are_pure() {
    let mut m = fixture();
    let before = m.clone();
    for va in [
        0x167ca0, 0x16b850, 0x16b800, 0x16b870, 0x16b8a0, 0x16b550, 0x16b590, 0x16b900, 0x16b130,
        0x16b8c0, 0x166b60, 0x91390, 0x913b0, 0x913d0,
    ] {
        assert_eq!(run(&mut m, va, false, [u32::MAX; 3], true), None);
        assert_eq!(m, before);
    }
    assert_eq!(run(&mut m, 0x16b190, true, [0; 3], false), None);
    assert_eq!(m, before);
}
