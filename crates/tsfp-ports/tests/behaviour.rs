// SPDX-License-Identifier: GPL-3.0-or-later
// Standalone rustc --test: no Cargo dependency or crate download is needed.
use std::collections::BTreeMap;
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_ports::{behaviour, bridge, layouts, leaves};
#[derive(Default)]
struct Synthetic {
    bytes: BTreeMap<u32, u8>,
    writes: Vec<(u32, u8)>,
}
impl GuestMemory for Synthetic {
    fn read8(&self, address: u32) -> u8 {
        *self.bytes.get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.bytes.insert(address, value);
        self.writes.push((address, value));
    }
}
fn state(m: &mut Synthetic, args: &[u32]) -> bridge::Registers {
    for (i, &arg) in args.iter().enumerate() {
        m.write32(0x8004 + (i as u32) * 4, arg);
    }
    m.writes.clear();
    bridge::Registers {
        eax: 0xabcd1234,
        ecx: 0x87654321,
        edx: 0x13572468,
        esp: 0x8000,
    }
}
#[test]
fn layouts_and_wraparound_memory() {
    assert_eq!(core::mem::size_of::<bridge::Registers>(), 16);
    assert_eq!(
        core::mem::offset_of!(layouts::ObjectPrefix, model_id_94),
        0x94
    );
    assert_eq!(
        core::mem::offset_of!(layouts::CharacterTransitionPrefix, state_20),
        0x20
    );
    let mut m = Synthetic::default();
    m.write32(u32::MAX - 1, 0x12345678);
    assert_eq!(m.read32(u32::MAX - 1), 0x12345678);
    assert_eq!(m.read8(0), 0x34);
}
#[test]
fn table_stride_and_unsigned_wrap() {
    let mut m = Synthetic::default();
    m.write32(0x2004, 0xfffffff8);
    m.write8(0x3006, 1);
    assert_eq!(behaviour::table_entry(&m, 0x1000, 0x2000, 0x3000), 8);
    m.write8(0x100c, 0x10);
    assert_eq!(behaviour::table_entry(&m, 0x1000, 0x2000, 0x3000), 0);
}
#[test]
fn enabled_all_nonzero_inputs_and_register_residue() {
    let mut m = Synthetic::default();
    m.write32(0x1028, 0xfffffffe);
    let mut r = state(&mut m, &[0x1000, 0x80000000]);
    bridge::execute(0x71c70, &mut m, &mut r);
    assert_eq!(m.read32(0x1028), u32::MAX);
    assert_eq!(
        (r.eax, r.ecx, r.edx, r.esp),
        (0x1000, u32::MAX, 0x80000000, 0x8000)
    );
    assert_eq!(behaviour::set_enabled(&mut m, 0x1000, 0), 0xfffffffe);
}
#[test]
fn transition_alias_and_locked_no_write() {
    let mut m = Synthetic::default();
    m.write32(0x1114, 5);
    m.write32(0x100c, u32::MAX);
    assert_eq!(
        behaviour::request_state(&mut m, 0x1000, 0x1110),
        u32::MAX & !0x20400
    );
    assert_eq!(m.read32(0x1114), 5);
    assert_eq!(m.read32(0x1110), 5);
    assert_eq!(m.read32(0x1118), 0);
    m.write32(0x1114, 0x10);
    m.writes.clear();
    assert_eq!(behaviour::request_state(&mut m, 0x1000, 0x1110), 0x10);
    assert!(m.writes.is_empty());
}
#[test]
fn signed_record_predicate_and_old_value_store() {
    let mut m = Synthetic::default();
    m.write32(0x74c458, 0x1000);
    for (value, expected) in [
        (0, 0),
        (1, 1),
        (0x7fffffff, 1),
        (0x80000000, 0),
        (u32::MAX, 0),
    ] {
        m.write32(0x15d0, value);
        assert_eq!(behaviour::record_positive(&m, 0x10002), expected);
    }
    m.write32(0x15c0, 0x123);
    assert_eq!(behaviour::record_set_flags(&mut m, 2, 0x80), 0x123);
    assert_eq!(m.read32(0x15c0), 0x1a3);
}
#[test]
fn ai_state_domains_and_residue() {
    let mut m = Synthetic::default();
    m.write32(0x107c, 0x2000);
    for value in [0, 1, 2, 3, 4, 7, 9, u32::MAX] {
        m.write32(0x2020, value);
        m.write32(0x29d0, u32::MAX);
        let mut r = state(&mut m, &[0x1000]);
        bridge::execute(0x264740, &mut m, &mut r);
        assert_eq!((r.eax, r.ecx), (value, 0x2000));
        assert_eq!(
            m.read32(0x29d0),
            if [0, 1, 2, 4].contains(&value) {
                3
            } else {
                u32::MAX
            }
        );
        m.write32(0x200c, value);
        bridge::execute(0x243040, &mut m, &mut r);
        assert_eq!(r.eax, u32::from(value != 7 && value != 9));
    }
}
#[test]
fn selected_model_word_codes() {
    let mut m = Synthetic::default();
    for (code, wanted) in [
        (0x5c7u16, 100),
        (0x5d2, 100),
        (0x5e1, 100),
        (0xffff, 75),
        (0, 75),
    ] {
        for (i, b) in code.to_le_bytes().into_iter().enumerate() {
            m.write8(0x1094 + i as u32, b);
        }
        let mut r = state(&mut m, &[0x1000]);
        bridge::execute(0x228c20, &mut m, &mut r);
        assert_eq!(r.eax, wanted);
        assert_eq!(r.ecx, 0x87654321);
    }
}
#[test]
fn x86_shift_counts_are_masked() {
    let mut m = Synthetic::default();
    m.write32(0x10a0, 0x80000001);
    m.write32(0x78cd54, 0x80000001);
    for shift in [0, 1, 31, 32, 63, 255, u32::MAX] {
        let wanted = 1u32.wrapping_shl(shift) & 0x80000001;
        let mut r = state(&mut m, &[0x1000, shift]);
        bridge::execute(0x22df10, &mut m, &mut r);
        assert_eq!(r.eax, u32::from(wanted != 0));
        r = state(&mut m, &[shift]);
        bridge::execute(0x23ef30, &mut m, &mut r);
        assert_eq!((r.eax, r.ecx), (wanted, shift));
    }
}
#[test]
fn byte_swaps_preserve_upper_ax_and_stack_bytes() {
    let mut m = Synthetic::default();
    let mut r = state(&mut m, &[0x98761234]);
    bridge::execute(0x3bd2b0, &mut m, &mut r);
    assert_eq!((r.eax, r.ecx, r.edx), (0xabcd3412, 0x3412, 0x12));
    r = state(&mut m, &[0x98761234]);
    bridge::execute(0x3bd2d0, &mut m, &mut r);
    assert_eq!((r.eax, r.ecx, r.edx), (0x34127698, 0x34127698, 0x98));
    r = state(&mut m, &[0x98761234]);
    bridge::execute(0x3bd300, &mut m, &mut r);
    assert_eq!(m.read32(0x8004), 0x98763412);
    assert_eq!(r.eax, 0xabcd3412);
    assert_eq!(r.ecx, 0x87654321);
}
#[test]
fn header_sizes_use_signed_subtract_and_wrapping_add() {
    let mut m = Synthetic::default();
    m.write32(0x1000, 1);
    m.write32(0x1004, 1);
    for (value, wanted) in [
        (7, 7),
        (8, 0),
        (0x7fffffff, 0x7ffffff7),
        (0x80000000, 0x80000000),
        (u32::MAX, u32::MAX),
    ] {
        let mut r = state(&mut m, &[0x1000, value]);
        bridge::execute(0x3c3cd0, &mut m, &mut r);
        assert_eq!(r.eax, wanted);
    }
    let mut r = state(&mut m, &[0x1000, u32::MAX]);
    bridge::execute(0x3c3d00, &mut m, &mut r);
    assert_eq!(r.eax, 7);
    m.write32(0x1000, 0);
    r = state(&mut m, &[0x1000, 8]);
    bridge::execute(0x3c3cd0, &mut m, &mut r);
    assert_eq!(r.ecx, 0x87654321);
}

#[test]
fn leaf_popcount_halves_and_swap_stack() {
    let mut m = Synthetic::default();
    let mut r = state(&mut m, &[0x8000ffff]);
    bridge::execute(0x151090, &mut m, &mut r);
    assert_eq!((r.eax, r.ecx, r.edx), (17, 16, 0x13572468));
    let mut r = state(&mut m, &[0x12345678]);
    bridge::execute(0x3bd320, &mut m, &mut r);
    assert_eq!(r.eax, 0x78563412);
    assert_eq!(m.read32(0x8004), r.eax);
}
#[test]
fn leaf_decimal_signed_overflow_and_default_residue() {
    let mut m = Synthetic::default();
    for (text, result) in [
        (b"-2147483648x".as_slice(), 0x80000000),
        (b"+4294967296!".as_slice(), 0),
        (b" 12".as_slice(), 0),
        (b"9\xff".as_slice(), 9),
    ] {
        for (i, b) in text.iter().enumerate() {
            m.write8(0x1000 + i as u32, *b);
        }
        m.write8(0x1000 + text.len() as u32, 0);
        let mut r = state(&mut m, &[0x1000, 99]);
        bridge::execute(0x3b9f00, &mut m, &mut r);
        assert_eq!(r.eax, result);
        assert!(r.edx >= 0x1000);
    }
    let mut r = state(&mut m, &[0, 99]);
    bridge::execute(0x3b9f00, &mut m, &mut r);
    assert_eq!((r.eax, r.ecx, r.edx), (99, 0x87654321, 0));
}
#[test]
fn leaf_arena_alignment_wrap_and_argument_alias() {
    let mut m = Synthetic::default();
    let mut r = state(&mut m, &[5]);
    r.ecx = 0x8000;
    // The arena's offset is also its stack argument: read twice before writing.
    bridge::execute(0x3bb710, &mut m, &mut r);
    assert_eq!((r.eax, r.edx, m.read32(0x8004)), (5, 13, 13));
    assert_eq!(leaves::integer::align4(u32::MAX), 0);
}
#[test]
fn leaf_flag_pack_preserves_raw_last_field_and_pointer_high_bytes() {
    let mut m = Synthetic::default();
    let mut r = state(&mut m, &[0x1000]);
    m.write32(0x1004, 0x80000000);
    m.write32(0x1008, 0x1234);
    m.write32(0x100c, 0);
    m.write32(0x1010, u32::MAX);
    bridge::execute(0x31dcd0, &mut m, &mut r);
    assert_eq!((r.eax, r.ecx, r.edx), (13, 0x1001, 0x1234));
    let mut r = state(&mut m, &[0x1000, 0xfffffffbu32]);
    bridge::execute(0x31dd10, &mut m, &mut r);
    assert_eq!(
        (
            m.read32(0x1008),
            m.read32(0x100c),
            m.read32(0x1010),
            m.read32(0x1004)
        ),
        (1, 1, 0, 8)
    );
    assert_eq!((r.eax, r.ecx, r.edx), (8, 0x1000, 0));
}
#[test]
fn leaf_sentinel_list_and_conditional_clear() {
    let mut m = Synthetic::default();
    m.write32(0x1010, 0x2000);
    m.write32(0x2000, 0);
    m.write32(0x2010, 0x7fffffff);
    m.write32(0x2020, 0x80000000);
    assert_eq!(leaves::count_until_negative(&m, 0x1000), 2);
    m.write32(0x1000, 1);
    m.write32(0x1004, u32::MAX - 1);
    assert_eq!(leaves::first_node_data(&m, 0x1000), 2);
    m.write32(0x6b7aa8, 1);
    let mut r = state(&mut m, &[]);
    bridge::execute(0x3ea20, &mut m, &mut r);
    assert!(m.writes.is_empty());
    m.write32(0x6b7aa8, 0);
    m.writes.clear();
    bridge::execute(0x3ea20, &mut m, &mut r);
    assert_eq!(
        m.writes.iter().map(|(a, _)| *a).collect::<Vec<_>>(),
        (0x6b83c8..0x6b83d0).collect::<Vec<_>>()
    );
}

#[test]
fn wave2_animation_key_and_exact_registers() {
    let mut m = Synthetic::default();
    m.write32(0x1004, 0x2000);
    m.write32(0x20d4, 0x80000000);
    let mut r = state(&mut m, &[0x1000, 0x80000000]);
    bridge::execute(0x59d70, &mut m, &mut r);
    assert_eq!(
        (r.eax, r.ecx, r.edx, r.esp),
        (1, 0x80000000, 0x80000000, 0x8000)
    );
    m.write32(0x8008, 0x7fffffff);
    bridge::execute(0x59d70, &mut m, &mut r);
    assert_eq!(r.eax, 0);
    assert!(
        m.writes
            .iter()
            .all(|&(address, _)| address < 0x1000 || address >= 0x8000)
    );
}
#[test]
fn wave2_physics_sentinel_and_wrapping_index() {
    let mut m = Synthetic::default();
    let mut r = state(&mut m, &[u32::MAX]);
    bridge::execute(0xdc290, &mut m, &mut r);
    assert_eq!(r.eax, 0);
    m.write32(0x4deb40, 0x12345678);
    m.write32(0x8004, 0x04000000);
    bridge::execute(0xdc290, &mut m, &mut r);
    assert_eq!(r.eax, 0x12345678);
}
