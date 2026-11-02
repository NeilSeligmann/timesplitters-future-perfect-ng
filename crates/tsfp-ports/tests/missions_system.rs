// SPDX-License-Identifier: GPL-3.0-or-later
// Tier 1 against translated roots, FABRICATED inputs; no host/disc evidence.
use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::c2r::{t1937_math, t2049_exec_002};
use tsfp_ports::memory::GuestMemory;
use tsfp_ports::missions_system::*;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
struct Memory(BTreeMap<u32, u8>);
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _: &mut ExtRegisters) {
        panic!("unexpected callee {va:x}");
    }
}
fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 1,
        ecx: 2,
        edx: 3,
        ebx: 4,
        ebp: 5,
        esi: 6,
        edi: 7,
        esp: 0x8000,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
fn next(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed
}
#[test]
fn signed_search_matches_translated_9cc40_for_4096_images() {
    let mut seed = 2041;
    for case in 0..4096 {
        let mut m = Memory::default();
        let mut r = registers();
        let count = next(&mut seed) % 65;
        let base = if case % 7 == 0 { 0xffff_ff00 } else { 0x1000 };
        m.write32(MAIN_ROWS, base);
        m.write32(MAIN_COUNT, count);
        let mut keys = Vec::new();
        // Signed sorted keys, including duplicates, negative keys and readable row 0
        // for the original's zero-count probe. A zero-filled tail is not assumed.
        for _ in 0..count.max(1) {
            keys.push((next(&mut seed) % 200) as i32 - 100);
        }
        keys.sort();
        for (i, key) in keys.iter().enumerate() {
            m.write32(base.wrapping_add(i as u32 * ROW_SIZE), *key as u32);
        }
        let key = if case % 2 == 0 {
            keys[(next(&mut seed) as usize) % keys.len()] as u32
        } else {
            ((next(&mut seed) % 250) as i32 - 125) as u32
        };
        m.write32(r.esp + 4, key);
        let expected = GuestTables(&m).find_key(key).unwrap_or(u32::MAX);
        let mut wired = m.clone();
        let mut actual = r;
        assert!(tsfp_ports::missions_reader::run(
            0x9cc40,
            1,
            &mut wired,
            &mut NoCalls,
            &mut actual
        ));
        t2049_exec_002::game_table_715f2c_binary_search_key_to_id(&mut m, &mut NoCalls, &mut r);
        assert_eq!(wired, m, "search memory case {case}");
        assert_eq!(actual, r, "search ABI case {case}");
        assert_eq!(r.eax, expected, "case {case}, count {count}, key {key:x}");
        assert_eq!((r.ebx, r.esi, r.edi, r.esp), (4, 6, 7, 0x8000));
    }
}
#[test]
fn stream_matches_translated_f9080_for_4096_images() {
    let mut seed = 0xf9080;
    for case in 0..4096 {
        let mut m = Memory::default();
        let mut r = registers();
        let cursor = match case % 5 {
            0 => 0xffff_fffc,     // wrapping writes
            1 => DRAW_CURSOR,     // aliased first write
            2 => DRAW_CURSOR - 4, // aliased second write
            3 => 0x1001,          // unaligned writes
            _ => 0x1000,
        };
        let bits = core::array::from_fn(|_| next(&mut seed));
        m.write32(DRAW_CURSOR, cursor);
        for (i, word) in bits.iter().enumerate() {
            m.write32(r.esp + 4 + 4 * i as u32, *word);
        }
        let mut wired = m.clone();
        let mut actual = r;
        assert!(tsfp_ports::missions_reader::run(
            0xf9080,
            1,
            &mut wired,
            &mut NoCalls,
            &mut actual
        ));
        let mut expected = m.clone();
        GuestDrawStream(&mut expected).push_position_bits(bits);
        t1937_math::game_float_stream_55f974_push_3(&mut m, &mut r);
        assert_eq!(wired, m, "stream memory case {case}");
        assert_eq!(actual, r, "stream ABI case {case}");
        assert_eq!(m, expected, "case {case}");
    }
}
#[test]
fn typed_fields_decode_signed_words_and_preserve_float_bits() {
    let mut m = Memory::default();
    m.write32(MAIN_ROWS, 0x1000);
    m.write32(LINKS, 0x2000);
    m.write32(SECOND_ROWS, 0x3000);
    m.write32(THIRD_ROWS, 0x4000);
    m.write32(MAIN_COUNT, 1);
    m.write32(LINK_COUNT, 2);
    m.write32(SECOND_COUNT, 3);
    m.write32(DRAW_CURSOR, 0x5000);
    for (i, word) in [42, 3, 0xfffe_ffff, 0x7fc01234, 0x80000000, 0x7f800000]
        .iter()
        .enumerate()
    {
        m.write32(0x1000 + i as u32 * 4, *word);
    }
    for (i, word) in [7, 9, 11, 13, 15].iter().enumerate() {
        m.write32(0x2000 + i as u32 * 4, *word);
    }
    let tables = GuestTables(&m);
    assert_eq!(
        tables.main_row(0),
        Row {
            key: 42,
            flags: 3,
            group: -1,
            tag: -2,
            position_bits: [0x7fc01234, 0x80000000, 0x7f800000],
        }
    );
    assert_eq!(
        tables.link(0),
        Link {
            flags: 7,
            endpoints: [9, 11],
            unknown: [13, 15]
        }
    );
    assert_eq!(
        (
            tables.link_count(),
            tables.second_count(),
            tables.second_rows(),
            tables.third_rows(),
            tables.draw_cursor()
        ),
        (2, 3, 0x3000, 0x4000, 0x5000)
    );
}

#[derive(Default, Debug, PartialEq, Eq)]
struct Calls(Vec<u32>);
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.0.push(va);
        // Caller-derived fabricated edge: validity true; lookup index zero.
        r.eax = match va {
            0x9a330 => 1,
            0x9a3c0 => 0x1f40,
            _ => 0,
        };
    }
}
fn translated(va: u32, m: &mut Memory, c: &mut Calls, r: &mut ExtRegisters) {
    use tsfp_ports::c2r::{data_missions as d, t2049_exec_001 as e};
    match va {
        0x9cca0 => d::game_entry_id_to_ptr(m, c, r),
        0x9a510 => d::game_entry_ptr_to_id(m, c, r),
        0x9a420 => e::game_table_715f2c_family_select_by_id_range_and_lookup(m, c, r),
        0x9a3c0 => d::game_table_715f34_binary_search_key_to_id(m, r),
        0x9bef0 => d::game_table_715f2c_collect_entries_within_radius(m, r),
        0x9c0e0 => {
            d::game_point_table_715f2c_find_nearest_active_entry_in_group_beyond_min_distance(m, r)
        }
        _ => unreachable!(),
    }
}
#[test]
fn translated_family_matches_guest_boundary_4096_images_per_root() {
    let mut seed = 0x2041;
    let mut nearest_hits = 0;
    let mut radius_hits = 0;
    for case in 0..4096 {
        let mut m = Memory::default();
        let count = case % 9;
        let base = if case % 11 == 0 {
            0xffff_ff80u32
        } else {
            0x1000
        };
        m.write32(MAIN_ROWS, base);
        m.write32(MAIN_COUNT, count);
        m.write32(SECOND_ROWS, 0x3000);
        m.write32(SECOND_COUNT, count);
        m.write32(THIRD_ROWS, 0x4000);
        for i in 0..count.max(1) {
            let row = base.wrapping_add(i * ROW_SIZE);
            m.write32(row, i + 1001);
            m.write32(row.wrapping_add(4), next(&mut seed) & 0x400801);
            m.write32(
                row.wrapping_add(8),
                u32::from(((next(&mut seed) % 5) as i16 - 2) as u16)
                    | ((next(&mut seed) % 3) << 16),
            );
            for axis in 0..3 {
                m.write32(
                    row.wrapping_add(12 + axis * 4),
                    ((next(&mut seed) % 50) as f32 - 25.0).to_bits(),
                );
            }
            m.write32(0x3000 + i * ROW_SIZE, i + 9001);
        }
        m.write32(0x475d5c, 1e10f32.to_bits());
        m.write32(0x475d1c, 0.01f32.to_bits());
        for va in [0x9cca0, 0x9a510, 0x9a420, 0x9a3c0, 0x9bef0, 0x9c0e0] {
            let mut r = registers();
            let arg = match va {
                0x9cca0 => [1001, 9001, 0, u32::MAX][case as usize % 4],
                0x9a510 => base,
                0x9a420 => [0, 0x1f40, 0x209e, u32::MAX][case as usize % 4],
                0x9a3c0 => 9001 + next(&mut seed) % 12,
                _ => 0x5000,
            };
            m.write32(r.esp + 4, arg);
            m.write32(
                r.esp + 8,
                if va == 0x9c0e0 {
                    case % 3
                } else {
                    20f32.to_bits()
                },
            );
            m.write32(
                r.esp + 12,
                if va == 0x9c0e0 {
                    2f32.to_bits()
                } else {
                    0x6000
                },
            );
            m.write32(r.esp + 16, 8);
            let mut actual = r;
            let mut expected = m.clone();
            let mut wired = m.clone();
            let mut calls = Calls::default();
            let mut actual_calls = Calls::default();
            translated(va, &mut expected, &mut calls, &mut r);
            if va == 0x9c0e0 && r.eax != u32::MAX {
                nearest_hits += 1;
            }
            if va == 0x9bef0 && r.eax != 0 {
                radius_hits += 1;
            }
            assert!(tsfp_ports::missions_reader::run(
                va,
                1,
                &mut wired,
                &mut actual_calls,
                &mut actual
            ));
            assert_eq!(actual, r, "ABI {va:x} case {case}");
            assert_eq!(wired, expected, "memory {va:x} case {case}");
            assert_eq!(actual_calls, calls, "calls {va:x} case {case}");
        }
    }
    println!("family coverage: nearest hits={nearest_hits}, radius hits={radius_hits}");
    assert!(nearest_hits > 50, "nearest hit coverage {nearest_hits}");
    assert!(radius_hits > 50, "radius hit coverage {radius_hits}");
}

#[test]
fn owned_selector_and_boundary_mutants() {
    use tsfp_ports::{missions_reader as reader, missions_wire};
    assert_eq!(reader::mode(), 0);
    assert_eq!(reader::configure(3), Err(3));
    let mut m = Memory::default();
    m.write32(MAIN_ROWS, 0x10000);
    m.write32(MAIN_COUNT, 2);
    m.write32(LINKS, 0x30000);
    m.write32(LINK_COUNT, 3);
    m.write32(SECOND_ROWS, 0x50000);
    m.write32(SECOND_COUNT, 4);
    m.write32(THIRD_ROWS, 0x70000);
    let mut r = registers();
    let before = m.clone();
    let registers_before = r;
    assert!(!reader::run(0x9cc40, 0, &mut m, &mut NoCalls, &mut r));
    assert!(!reader::run(0xffffffff, 1, &mut m, &mut NoCalls, &mut r));
    assert_eq!(m, before);
    assert_eq!(r, registers_before);
    missions_wire::configure(1, |_| true).unwrap();
    for case in 0..4096u32 {
        // Changing only via typed setters keeps the persistent owner authoritative.
        let key = case % 200 + 1001;
        tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x10000, key);
        tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x10020, key + 1);
        m.write32(r.esp + 4, key);
        let mut original = m.clone();
        let mut oracle = registers();
        t2049_exec_002::game_table_715f2c_binary_search_key_to_id(
            &mut original,
            &mut NoCalls,
            &mut oracle,
        );
        // Row-key mutant: guest disagrees with the owner; guest search must differ.
        m.write32(0x10000, key - 1);
        let mut guest = registers();
        let mut owned = registers();
        let mut guest_image = m.clone();
        assert!(reader::run(
            0x9cc40,
            1,
            &mut guest_image,
            &mut NoCalls,
            &mut guest
        ));
        assert!(reader::run(0x9cc40, 2, &mut m, &mut NoCalls, &mut owned));
        assert_eq!(owned, oracle, "owned case {case}");
        assert_ne!(guest.eax, owned.eax, "key mutant {case}");
    }
    // Count mutant: count zero still probes row zero; key in row one disappears.
    tsfp_ports::leaf_wire::set_mission_cell(&mut m, MAIN_COUNT, 2);
    m.write32(MAIN_COUNT, 0);
    m.write32(r.esp + 4, m.read32(0x10020));
    let mut guest = registers();
    let mut owned = registers();
    let mut guest_image = m.clone();
    reader::run(0x9cc40, 1, &mut guest_image, &mut NoCalls, &mut guest);
    reader::run(0x9cc40, 2, &mut m, &mut NoCalls, &mut owned);
    assert_ne!(guest.eax, owned.eax);
    // Field mutants exercise byte/halfword/full-row conversion, not just lookup.
    tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x10004, 1);
    tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x10008, 0xfffe0003);
    tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x1000c, 0x7fc01234);
    m.write32(0x10004, 0);
    m.write32(0x10008, 0);
    m.write32(0x1000c, 0);
    let mut borrowed = m.clone();
    let view = reader::Reads(&mut borrowed, missions_wire::OwnedPoints(&m));
    assert_eq!(view.read8(0x10004), 1);
    assert_eq!(view.read16(0x1000a), 0xfffe);
    assert_eq!(view.read32(0x1000c), 0x7fc01234);
    assert_eq!(view.read32(MAIN_COUNT), 2);
    drop(view);
    // Actual radius-reader flag/position mutant with the count restored in both lanes.
    m.write32(MAIN_COUNT, 2);
    for axis in 0..3 {
        tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x1000c + 4 * axis, 0);
    }
    m.write32(0x10004, 0);
    m.write32(r.esp + 4, 0x5000);
    m.write32(r.esp + 8, 10f32.to_bits());
    m.write32(r.esp + 12, 0x6000);
    m.write32(r.esp + 16, 2);
    let mut guest_image = m.clone();
    let mut guest = registers();
    let mut owned = registers();
    reader::run(0x9bef0, 1, &mut guest_image, &mut NoCalls, &mut guest);
    reader::run(0x9bef0, 2, &mut m, &mut NoCalls, &mut owned);
    assert_ne!(guest.eax, owned.eax, "actual radius flags mutant");
    // Second-table keys and counts flow through the same typed owner boundary.
    tsfp_ports::leaf_wire::set_mission_cell(&mut m, SECOND_COUNT, 2);
    tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x50000, 9001);
    tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x50020, 9002);
    m.write32(0x50000, 9000);
    m.write32(r.esp + 4, 9001);
    let mut guest_image = m.clone();
    let mut guest = registers();
    let mut owned = registers();
    reader::run(0x9a3c0, 1, &mut guest_image, &mut NoCalls, &mut guest);
    reader::run(0x9a3c0, 2, &mut m, &mut NoCalls, &mut owned);
    assert_eq!(owned.eax, 0x1f40);
    assert_ne!(guest.eax, owned.eax, "second key mutant");
    tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x30004, 0x12345678);
    tsfp_ports::leaf_wire::set_mission_word(&mut m, 0x70008, 0xffff0007);
    m.write32(0x30004, 0);
    m.write32(0x70008, 0);
    let mut borrowed = m.clone();
    let view = reader::Reads(&mut borrowed, missions_wire::OwnedPoints(&m));
    assert_eq!(view.read32(0x30004), 0x12345678);
    assert_eq!(view.read16(0x7000a), 0xffff);
    assert_eq!(view.read32(LINK_COUNT), 3);
    assert_eq!(view.read32(SECOND_COUNT), 2);
    assert_eq!(view.read32(THIRD_ROWS), 0x70000);
    drop(view);
    // Dispatch gate on: module dispatch reaches owned search; off preserves fallback.
    reader::configure(2).unwrap();
    m.write32(r.esp + 4, 1001 + 4095 % 200);
    assert!(t2049_exec_002::run(0x9cc40, &mut m, &mut NoCalls, &mut r));
    assert_eq!(r.eax, 0);
    // Readable linked-record search consumes the same owned group extension.
    m.write32(0x9000, 0);
    m.write32(0x70e178, 0xffff);
    m.write32(0x70e17c, 0xfff40000);
    m.write32(0x70e184, 0x7fc01234);
    m.write32(r.esp + 4, 0x9000);
    m.write32(r.esp + 8, 3);
    m.write32(0x10008, 0);
    let mut hit = registers();
    tsfp_ports::mission_links::lookup_guest(&mut m, &mut hit);
    assert_eq!(hit.eax, (-12i32) as u32);
    assert_eq!(hit.xmm[0].lanes[0], 0x7fc01234);
    reader::configure(1).unwrap();
    let mut miss = registers();
    tsfp_ports::mission_links::lookup_guest(&mut m, &mut miss);
    assert_eq!(miss.eax, u32::MAX);
    reader::configure(0).unwrap();
    missions_wire::configure(0, |_| false).unwrap();
}

#[test]
fn readable_abi_preserves_stack_aliases_and_second_table_call_edge() {
    for case in 0..64u32 {
        let mut m = Memory::default();
        let mut r = registers();
        // First store changes a later stack argument; later loads must observe it.
        m.write32(DRAW_CURSOR, r.esp + 8 + case % 2 * 4);
        for i in 0..3 {
            m.write32(r.esp + 4 + i * 4, 0x7fc01234 + case + i);
        }
        let mut expected = m.clone();
        let mut oracle = r;
        t1937_math::game_float_stream_55f974_push_3(&mut expected, &mut oracle);
        assert!(tsfp_ports::missions_reader::run(
            0xf9080,
            1,
            &mut m,
            &mut NoCalls,
            &mut r
        ));
        assert_eq!(m, expected);
        assert_eq!(r, oracle);
        m.write32(r.esp + 4, 0x2329 + case);
        let mut expected = m.clone();
        let mut oracle = r;
        let mut actual_calls = Calls::default();
        let mut oracle_calls = Calls::default();
        t2049_exec_002::game_table_715f2c_binary_search_key_to_id(
            &mut expected,
            &mut oracle_calls,
            &mut oracle,
        );
        assert!(tsfp_ports::missions_reader::run(
            0x9cc40,
            1,
            &mut m,
            &mut actual_calls,
            &mut r
        ));
        assert_eq!(m, expected);
        assert_eq!(r, oracle);
        assert_eq!(actual_calls, oracle_calls);
        assert_eq!(actual_calls.0, [0x9a3c0]);
    }
}

#[test]
fn renderer_stream_does_not_consult_point_tables_in_owned_selection() {
    struct RenderOnly(Memory);
    impl GuestMemory for RenderOnly {
        fn read8(&self, a: u32) -> u8 {
            assert!(!(MAIN_ROWS..MAIN_ROWS + 16).contains(&a));
            assert!(!(MAIN_COUNT..MAIN_COUNT + 4).contains(&a));
            self.0.read8(a)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.write8(a, v);
        }
    }
    let mut m = RenderOnly(Memory::default());
    let mut r = registers();
    m.write32(DRAW_CURSOR, 0x1000);
    for i in 0..3 {
        m.write32(r.esp + 4 + 4 * i, 0x7fc01234 + i);
    }
    assert!(tsfp_ports::missions_reader::run(
        0xf9080,
        2,
        &mut m,
        &mut NoCalls,
        &mut r
    ));
    assert_eq!(m.read32(DRAW_CURSOR), 0x100c);
    assert_eq!(m.read32(0x1008), 0x7fc01236);
}
