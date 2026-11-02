// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_ai::characters::*;
use tsfp_engine_object::closure::memory;
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, stack_arg},
    memory::GuestMemory,
};
mod support;
use support::Memory;
#[derive(Default)]
struct Calls(Vec<(u32, u32, Vec<u32>)>);
impl Callee<Memory> for Calls {
    fn call(&mut self, m: &mut Memory, a: u32, cpu: &mut Cpu) {
        let count = if a == 0x15bd00 { 2 } else { 1 };
        self.0.push((
            a,
            m.read32(cpu.esp),
            (0..count).map(|i| stack_arg(m, cpu, i)).collect(),
        ));
        match a {
            0x15bd00 => pair_entry(m, cpu),
            0xd6dd0 => {
                cpu.eax = 0;
                cpu.ecx = 0x4d;
            }
            0x2b8f90 => {
                if self.0.len() == 1 {
                    m.write32(CHARACTER_COUNT, 2);
                }
            }
            _ => {}
        }
    }
}
fn fixture(args: &[u32]) -> (Memory, Cpu, Calls) {
    let mut m = Memory::default();
    let cpu = Cpu {
        esp: 0x1000,
        esi: 0xabcdef,
        edi: 0x123456,
        ..Cpu::default()
    };
    for (i, a) in args.iter().enumerate() {
        m.write32(cpu.esp + 4 + 4 * i as u32, *a);
    }
    (m, cpu, Calls::default())
}
#[test]
fn type_maps_boundaries_signed_ids_and_unknown_tail() {
    for (root, first, last, bytes, targets, yes, no) in [
        (0xd6c90, 0x5d, 0x9b, 0xd6cbc, 0xd6cb4, 0xd6caa, 0xd6cb0),
        (0xd6d00, 0x16, 0xa3, 0xd6d30, 0xd6d28, 0xd6d1c, 0xd6d22),
        (0xd6dd0, 0x12, 0xaa, 0xd6e00, 0xd6df8, 0xd6dec, 0xd6df2),
    ] {
        for id in [first - 1, first, last, last + 1, u32::MAX] {
            for set in [false, true] {
                let (mut m, mut cpu, mut c) = fixture(&[id]);
                m.write32(targets, yes);
                m.write32(targets + 4, no);
                if (first..=last).contains(&id) {
                    m.write8(bytes + id - first, u8::from(!set));
                }
                assert!(dispatch(true, root, &mut c, &mut m, &mut cpu));
                assert_eq!(cpu.eax, u32::from(set && (first..=last).contains(&id)));
                assert!(c.0.is_empty());
            }
        }
        let (mut m, mut cpu, mut c) = fixture(&[first]);
        m.write32(targets, 0x777777);
        dispatch(true, root, &mut c, &mut m, &mut cpu);
        assert_eq!(c.0[0].0, 0x777777);
    }
}
#[test]
fn simple_predicates_and_post_call_ecx() {
    for id in 0..256 {
        let (m, mut cpu, _) = fixture(&[id]);
        is_id_4a_or_50(&m, &mut cpu);
        assert_eq!(cpu.eax, u32::from(id == 0x4a || id == 0x50));
        is_id_4e(&m, &mut cpu);
        assert_eq!(cpu.eax, u32::from(id == 0x4e));
        assert_eq!(cpu.ecx, id);
    }
    let (mut m, mut cpu, mut c) = fixture(&[7]);
    is_other_type(&mut c, &mut m, &mut cpu);
    assert_eq!(cpu.eax, 0);
    assert_eq!(c.0[0], (0xd6dd0, 0xd6eda, vec![7]));
    assert_eq!(cpu.esp, 0x1000);
}
#[test]
fn triangular_pairs_and_field_wrappers() {
    for hi in 0u32..100 {
        for lo in 0..=hi {
            for args in [[hi, lo], [lo, hi]] {
                let (mut m, mut cpu, mut c) = fixture(&args);
                m.write32(PAIR_TABLE, 0x8000);
                pair_entry(&m, &mut cpu);
                let index = hi * (hi.saturating_sub(1)) / 2 + lo;
                assert_eq!(cpu.eax, 0x8000 + index * 12);
                let ptr = cpu.eax;
                m.write8(ptr, 0xfe);
                m.write32(ptr + 4, 0xaabbccdd);
                pair_signed_flag(&m, &mut cpu);
                assert_eq!(cpu.eax, 0xfffffffe);
                assert!(c.0.is_empty());
                pair_field_4(&m, &mut cpu);
                assert_eq!(cpu.eax, 0xaabbccdd);
                assert!(c.0.is_empty());
            }
        }
    }
    // Signed ordering and low-32-bit multiply followed by arithmetic shift.
    let (mut m, mut cpu, _) = fixture(&[u32::MAX, 0]);
    m.write32(PAIR_TABLE, 12);
    pair_entry(&m, &mut cpu);
    assert_eq!(cpu.eax, 0);
    assert_eq!(cpu.edx, u32::MAX);
    // Large signed operands overflow the low-word product before the arithmetic shift.
    let (mut m, mut cpu, _) = fixture(&[0x7fff_ffff, 0x8000_0000]);
    m.write32(PAIR_TABLE, 0xffff_fff0);
    pair_entry(&m, &mut cpu);
    assert_eq!(cpu.eax, 0xffff_fffc);
    assert_eq!(cpu.edx, 0x8000_0000);
}
#[test]
fn group_span_signed_comparisons_and_invalid_groups() {
    for group in [0, 1, 46, 47, u32::MAX] {
        for index in [u32::MAX, 0, 4, 5, 6] {
            let (mut m, mut cpu, _) = fixture(&[group, index]);
            m.write32(0x4fc9a0, 5);
            if group == 1 || group == 46 {
                m.write32(0x4fc99c + group * 4, 10);
                m.write32(0x4fc9a0 + group * 4, 15);
            }
            group_index_outside(&m, &mut cpu);
            assert_eq!(cpu.eax, u32::from(group < 47 && (index as i32) >= 5));
            group_count(&mut cpu);
            assert_eq!(cpu.eax, 47);
        }
    }
}
#[test]
fn pending_slot_modes_flags_and_existing_state() {
    for mode in [10, 9] {
        for set in [false, true] {
            for state in [1, 7] {
                let (mut m, mut cpu, _) = fixture(&[0x2000, 17]);
                m.write32(0x207c, 0x3000);
                m.write8(0x7de455, mode);
                let offset = if mode == 10 { 0x10 } else { 0x18 };
                m.write32(0x3000 + offset, if set { 0x80000 } else { 0 });
                m.write32(0x3000 + offset + 4, 0x1234);
                m.write32(0x3020, state);
                m.write32(0x39d0, 55);
                set_pending_slot(&mut m, &mut cpu);
                assert_eq!(m.read32(0x39ec), if set { 17 } else { 0 });
                assert_eq!(m.read32(0x39d0), if set && state != 1 { 1 } else { 55 });
                assert_eq!(m.read32(0xffc), 0x1234);
            }
        }
    }
}
#[test]
fn deactivate_only_expected_message_preserves_other_bits() {
    for message in [0, 1002, 1003, 1004] {
        let (mut m, mut cpu, _) = fixture(&[0x2000, message]);
        m.write32(0x207c, 0x3000);
        m.write32(0x302c, 0x42);
        m.write32(0x2028, 0xffff);
        deactivate_message(&mut m, &mut cpu);
        assert_eq!(cpu.eax, if message == 1003 { u32::MAX - 1 } else { 0 });
        assert_eq!(
            m.read32(0x302c),
            if message == 1003 { 0x100042 } else { 0x42 }
        );
        assert_eq!(
            m.read32(0x2028),
            if message == 1003 { 0xfffe } else { 0xffff }
        );
    }
}
#[test]
fn remote_search_and_mutating_count_call_boundary() {
    let (mut m, mut cpu, mut c) = fixture(&[]);
    m.write32(FIRST_REMOTE, 1);
    m.write32(CHARACTER_COUNT, 4);
    for i in 1..4 {
        m.write32(CHARACTER_POINTERS + i * 4, 0x2000 + i * 0x100);
        m.write32(0x2000 + i * 0x100, 90 + i);
    }
    m.write32(0x2210, 0x200000);
    first_flagged_remote(&mut m, &mut cpu);
    assert_eq!(cpu.eax, 0x2200);
    assert_eq!(cpu.esi, 0xabcdef);
    assert_eq!(cpu.edi, 0x123456);
    update_remote_characters(&mut c, &mut m, &mut cpu);
    assert_eq!(c.0, vec![(0x2b8f90, 0x1b8c5f, vec![91])]);
    assert_eq!(cpu.eax, 2);
    assert_eq!(cpu.esi, 0xabcdef);
    m.write32(FIRST_REMOTE, 4);
    first_flagged_remote(&mut m, &mut cpu);
    assert_eq!(cpu.eax, 0);
}
#[test]
fn every_root_has_tier_and_disabled_switch_is_inert() {
    for &(root, tier) in VERIFICATION_TIERS {
        assert_eq!(tier, 0);
        let (mut m, mut cpu, mut c) = fixture(&[]);
        let before = (m.clone(), cpu);
        assert!(!dispatch(false, root, &mut c, &mut m, &mut cpu));
        assert_eq!((m.clone(), cpu), before);
        assert!(c.0.is_empty());
    }
    assert_eq!(VERIFICATION_TIERS.len(), 15);
}
