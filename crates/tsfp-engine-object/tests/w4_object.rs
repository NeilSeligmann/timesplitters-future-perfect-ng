// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4 Tier 0 smoke tests: fake guest memory plus a recorded callee trace.
use tsfp_engine_object::closure::{callee::{Callee, Cpu}, memory::GuestMemory};
use tsfp_engine_object::w4_object::*;

mod support;
use support::{Mem, setup};

#[derive(Default)]
struct Rec { calls: Vec<(u32, Vec<u32>)>, ret: u32 }
impl Callee<Mem> for Rec {
    fn call(&mut self, m: &mut Mem, t: u32, cpu: &mut Cpu) {
        let n = match t { 0x47560 => 4, 0x153190 => 3, _ => 1 };
        self.calls.push((t, (0..n).map(|i| m.read32(cpu.esp + 4 + 4 * i)).collect()));
        cpu.eax = self.ret;
    }
}
fn slots(m: &mut Mem) {
    m.write32(0x74C36C, 0x1000); m.write32(0x1074, 0x2000); m.write32(0x7A2820, 0x3000);
}

#[test]
fn flag_from_field_3c() {
    for ret in [0, 5] {
        let (mut m, mut cpu) = setup(&[2]);
        slots(&mut m);
        m.write32(0x2000 + 2 * 0x98 + 0x3C, 0xAB);
        m.write32(0x3000 + 2 * 24 + 0x10, 9);
        let mut c = Rec { ret, ..Rec::default() };
        array_entry_flag_from_object_field_3c(&mut c, &mut m, &mut cpu);
        assert_eq!(c.calls, [(0x234270, vec![0xAB])]);
        assert_eq!(m.read32(0x3000 + 48 + 0x10), u32::from(ret != 0));
        assert_eq!(cpu.esp, 0x9000);
    }
}
#[test]
fn flag_when_matches_record_ids() {
    let (mut m, cpu) = setup(&[1]);
    slots(&mut m);
    m.write32(0x790950, 2); m.write32(0x7356D8, 0x5000); m.write32(0x715F2C, 0x6000);
    m.write32(0x5000 + 0x678, 1); m.write32(0x5000 + 0xEA0 + 0x678, 2);
    m.write32(0x6000 + 32, 0x77); m.write32(0x6000 + 64, 0xFFFF_FFFF);
    array_entry_flag_when_slot_matches_record_ids(&mut m, &cpu);
    assert_eq!(m.read32(0x3000 + 24 + 0x10), 0);
    m.write32(0x2000 + 0x98 + 0x40, 0x77);
    array_entry_flag_when_slot_matches_record_ids(&mut m, &cpu);
    assert_eq!(m.read32(0x3000 + 24 + 0x10), 1);
}
#[test]
fn room_collect() {
    let (mut m, mut cpu) = setup(&[7, 8, 0x4000, 3, 0x4100]);
    m.write32(0x708788, 0x8000);
    m.write8(0x4000, 1); // one entry already used
    for (i, id) in [1u16, 2, 1].iter().enumerate() {
        m.write8(0x4100 + 2 * i as u32, *id as u8);
    }
    m.write8(0x8000 + 2 * 0xB4 + 0xB2, 0x10); // id 2 already marked
    let mut c = Rec { ret: 0x100, ..Rec::default() };
    room_collect_unmarked_records_into_list(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls, [(0x47560, vec![7, 8, 0x8000 + 0xB4 + 0x10, 0x8000 + 0xB4 + 0x1C])]);
    assert_eq!(m.read16(0x4000), 2);
    assert_eq!(m.read16(0x4004), 0x102);
    assert_eq!(m.read8(0x8000 + 0xB4 + 0xB2) & 0x10, 0x10);
    assert_eq!(cpu.eax, 0);
}
#[test]
fn prop_advance_and_tail() {
    let (mut m, mut cpu) = setup(&[0x100]);
    m.write32(0x104, 0x55); m.write32(0x118, 1); m.write32(0x128, 0x10);
    let mut c = Rec::default();
    prop_anim_advance_track_then_set_flags(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls, [(0x5F040, vec![0x55])]);
    assert_eq!((m.read32(0x118), m.read32(0x128)), (3, 0x200010));
    tail_jump_to_33cb90(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls[1].0, 0x33CB90);
}
#[test]
fn copy_matrices() {
    let (mut m, mut cpu) = setup(&[0x100]);
    m.write32(0x104, 0x200); m.write32(0x210, 0x33); m.write32(0x204, 0x300); m.write32(0x304, 0x400);
    m.write32(0x180, 0x1_0002); m.write32(0x74C458, 0x10000);
    for f in [0x70, 0xC0, 0xC8] { m.write32(0x100 + f, 0x3F80_0000); }
    let mut c = Rec::default();
    object_copy_matrices_to_entry_clear_fields(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls, [(0x153190, vec![0x33, 0x10000 + (2 << 9), 0x400 ])].map(|(a, b)| (a, b)));
    assert_eq!(m.read32(0x170), 0);
    assert_eq!(m.read32(0x128), 0x200000);
    // null source: only the flag changes
    m.write32(0x210, 0); m.write32(0x170, 5);
    object_copy_matrices_to_entry_clear_fields(&mut c, &mut m, &mut cpu);
    assert_eq!((c.calls.len(), m.read32(0x170)), (1, 5));
}
