// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4b Tier 0 smoke tests: fake guest memory plus a recorded callee trace.
use std::collections::HashMap;
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use tsfp_engine_object::w4b_object::*;

mod support;
use support::{Mem, setup};

#[derive(Default)]
struct Rec {
    calls: Vec<(u32, u32)>,
    answers: HashMap<u32, u32>,
}
impl Callee<Mem> for Rec {
    fn call(&mut self, m: &mut Mem, t: u32, cpu: &mut Cpu) {
        self.calls.push((t, m.read32(cpu.esp + 4)));
        cpu.eax = *self.answers.get(&t).unwrap_or(&0);
    }
}
fn trigger(m: &mut Mem, kind: u32) {
    m.write32(0x78CD90, 0x1000); // table, entry 0
    m.write32(0x1000, 0x2000); // head
    m.write32(0x100C, 7); // matches event id 7 -> node = head+0x40
    m.write16(0x3000, 7);
    m.write32(0x2040, kind);
    m.write32(0x78CED0, 0xBEEF);
}
trait W16 {
    fn write16(&mut self, a: u32, v: u16);
}
impl W16 for Mem {
    fn write16(&mut self, a: u32, v: u16) {
        self.write8(a, v as u8);
        self.write8(a + 1, (v >> 8) as u8);
    }
}

#[test]
fn trigger_mode4_latches_once() {
    let (mut m, mut cpu) = setup(&[0x3000, 4]);
    cpu.ebx = 0x11;
    cpu.esi = 0x16;
    cpu.edi = 0x17;
    trigger(&mut m, 18);
    let mut c = Rec::default();
    trigger_condition_check(&mut c, &mut m, &mut cpu);
    assert_eq!(m.read16(0x1004 + 4), 1);
    assert_eq!(cpu.eax, 4); // lifted entry leaves its loaded mode in EAX on this return path
    assert_eq!(cpu.edx, 0x1000); // lifted entry leaves the trigger table base in EDX
    assert_eq!((cpu.ebx, cpu.esi, cpu.edi), (0x11, 0x16, 0x17));
    assert!(c.calls.is_empty());
}

#[test]
fn trigger_restores_callee_saved_registers_after_nested_call() {
    struct Clobber;
    impl Callee<Mem> for Clobber {
        fn call(&mut self, _m: &mut Mem, _target: u32, cpu: &mut Cpu) {
            cpu.eax = 0;
            cpu.ebx = 0xdead;
            cpu.esi = 0xbeef;
            cpu.edi = 0xcafe;
        }
    }
    let (mut m, mut cpu) = setup(&[0x3000, 3]);
    cpu.ebx = 0x11;
    cpu.esi = 0x16;
    cpu.edi = 0x17;
    trigger(&mut m, 18);
    trigger_condition_check(&mut Clobber, &mut m, &mut cpu);
    assert_eq!((cpu.ebx, cpu.esi, cpu.edi), (0x11, 0x16, 0x17));
}

#[test]
fn trigger_mode3_kind18_completes_only_when_predicate_true() {
    for (ret, expect) in [(0u32, 1usize), (1, 2)] {
        let (mut m, mut cpu) = setup(&[0x3000, 3]);
        trigger(&mut m, 18);
        let mut c = Rec::default();
        c.answers.insert(0x61CA0, ret);
        trigger_condition_check(&mut c, &mut m, &mut cpu);
        assert_eq!(c.calls.len(), expect);
        if ret != 0 {
            assert_eq!(c.calls[1], (0x2274C0, 0xBEEF));
        }
    }
}

#[test]
fn trigger_unknown_kind_and_slot_variant() {
    let (mut m, mut cpu) = setup(&[0x3000, 3]);
    trigger(&mut m, 1);
    let mut c = Rec::default();
    trigger_condition_check(&mut c, &mut m, &mut cpu);
    assert!(c.calls.is_empty());
    assert_eq!(cpu.eax, 0); // lifted dispatch leaves kind - 1 in EAX on this no-call path

    let (mut m, mut cpu) = setup(&[0x3000, 3]);
    trigger(&mut m, 2); // jump table entry without an arm
    let mut c = Rec::default();
    trigger_condition_check(&mut c, &mut m, &mut cpu);
    assert!(c.calls.is_empty());
    let (mut m, mut cpu) = setup(&[0x3000, 3]);
    trigger(&mut m, 0);
    m.write32(0x100C, 9); // other id -> node = head+0x64, kind 4 via word at slot
    m.write32(0x2064, 4);
    m.write32(0x1010, 0x55);
    let mut c = Rec::default();
    trigger_condition_check(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls, vec![(0x234270, 0x55)]);
}

#[test]
fn slot_flag_update_sets_flag_through_174e0() {
    let (mut m, mut cpu) = setup(&[1]);
    m.write32(0x74C36C, 0x4000);
    m.write32(0x4074, 0x5000); // records
    m.write32(0x4078, 2); // count
    m.write32(0x7A2820, 0x6000);
    m.write32(0x5000 + 0x98 + 0x34, 0x77); // slot 1 key
    m.write32(0x5000 + 8, 0x77); // record 0 matches
    m.write32(0x5000 + 0xC, 0x11);
    m.write32(0x5000 + 0x2C, 0x22);
    m.write32(0x5000 + 0x98 + 8, 0x99);
    m.write32(0x6000 + 8, 0); // record 0 entry word != 1
    m.write32(0x8000 + 0x7C, 0x8100); // object -> player
    m.write32(0x8100 + 0xBF0, 0x8200);
    m.write32(0x8200 + 8, 0x77);
    let mut c = Rec::default();
    c.answers.insert(0x6C890, 1);
    c.answers.insert(0x23FF20, 0x8000);
    c.answers.insert(0x174E0, 1);
    slot_node_flag_entry_update(&mut c, &mut m, &mut cpu);
    assert_eq!(m.read32(0x6000 + 24 + 0x10), 1);
    assert_eq!(
        c.calls.iter().map(|x| x.0).collect::<Vec<_>>(),
        vec![0x1622C0, 0x6C890, 0x23FF20, 0x174E0]
    );
}

#[test]
fn slot_flag_update_stops_when_already_set() {
    let (mut m, mut cpu) = setup(&[0]);
    m.write32(0x74C36C, 0x4000);
    m.write32(0x4074, 0x5000);
    m.write32(0x7A2820, 0x6000);
    m.write32(0x6000 + 0x10, 1);
    let mut c = Rec::default();
    slot_node_flag_entry_update(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls.len(), 1);
}

#[test]
fn prop_head_inactive_returns_and_active_promotes_queue() {
    let (mut m, mut cpu) = setup(&[0x1000]);
    m.write32(0x1004, 0x2000); // state; +0xD4 = 0 -> returns
    let mut c = Rec::default();
    let mut round = |x: f64| x.floor();
    assert_eq!(
        prop_animation_update_head(&mut c, &mut m, &mut cpu, &mut round),
        PropHeadOutcome::Returned
    );
    m.write32(0x2000 + 0xD4, 5);
    m.write32(0x1020, 0x400); // kind other
    m.write32(0x475C78, 1.0f32.to_bits());
    m.write32(0x4E794C, 1.0f32.to_bits());
    m.write32(0x4E8CEC, 2);
    m.write32(0x2000 + 0x198, 1.0f32.to_bits()); // total
    m.write32(0x2000 + 0x1A0, 0.75f32.to_bits()); // elapsed -> 1.25 >= 1 promotes
    m.write32(0x2000 + 0x12C, 0x42);
    let out = prop_animation_update_head(&mut c, &mut m, &mut cpu, &mut round);
    assert!(matches!(out, PropHeadOutcome::Continue(h) if h.speed_scale == 1.0));
    assert_eq!(m.read32(0x2000 + 0xD4), 0x42);
    assert_eq!(m.read32(0x2000 + 0x12C), 0);
}
