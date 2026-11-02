// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4b Tier 0 smoke tests: fake guest memory plus a recorded callee trace.
use std::collections::HashMap;
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use tsfp_gameplay::w4b_gameplay as w;

#[derive(Default)]
struct Mem(HashMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
#[derive(Default)]
struct Rec {
    calls: Vec<(u32, [u32; 3])>,
    answers: HashMap<u32, u32>,
}
impl Callee<Mem> for Rec {
    fn call(&mut self, m: &mut Mem, t: u32, cpu: &mut Cpu) {
        let a = |i: u32| m.read32(cpu.esp + 4 + 4 * i);
        self.calls.push((t, [a(0), a(1), a(2)]));
        cpu.eax = *self.answers.get(&t).unwrap_or(&0);
    }
}
fn setup(args: &[u32]) -> (Mem, Cpu) {
    let mut m = Mem::default();
    let cpu = Cpu {
        esp: 0x9000,
        ..Cpu::default()
    };
    for (i, a) in args.iter().enumerate() {
        m.write32(0x9004 + 4 * i as u32, *a);
    }
    (m, cpu)
}

#[test]
fn pickup_tick_full_path_decays_counters_and_timer() {
    let (mut m, mut cpu) = setup(&[0x1000]);
    m.write32(0x1000 + 0x7C, 0x2000);
    m.write32(0x2000 + 0xC0, 2);
    m.write32(0x1000 + 0x28, 3); // flags 1|2
    m.write8(0x2000 + 0x50, 3);
    m.write8(0x2000 + 0x8E, 1); // last counter
    m.write32(0x2000 + 0xD0, 5.0f32.to_bits());
    m.write32(0x475CAC, 0.0f32.to_bits());
    m.write32(0x475C78, 1.0f32.to_bits());
    m.write32(0x4E794C, 1.0f32.to_bits());
    m.write32(0x4E8CEC, 4);
    let mut c = Rec::default();
    w::pickup_frame_tick(&mut c, &mut m, &mut cpu);
    assert_eq!(m.read8(0x2000 + 0x50), 2);
    assert_eq!(m.read8(0x2000 + 0x8E), 0);
    assert_eq!(f32::from_bits(m.read32(0x2000 + 0xD0)), 4.75);
    let targets: Vec<u32> = c.calls.iter().map(|x| x.0).collect();
    assert_eq!(targets, vec![0xC0C50, 0xBDD50, 0xBF080, 0xC15B0]);
    assert_eq!(c.calls[3].1[0], 0x1000);
}

#[test]
fn pickup_tick_gates() {
    let (mut m, mut cpu) = setup(&[0x1000]);
    m.write32(0x1000 + 0x7C, 0x2000);
    m.write32(0x2000 + 0xC0, 1); // phase < 2: nothing
    let mut c = Rec::default();
    w::pickup_frame_tick(&mut c, &mut m, &mut cpu);
    assert!(c.calls.is_empty());
    m.write32(0x2000 + 0xC0, 2); // flag 2 missing: only the refresh call
    w::pickup_frame_tick(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls.len(), 1);
    m.write32(0x1000 + 0x28, 2); // no flag 1, no model record
    w::pickup_frame_tick(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls.len(), 2);
}

#[test]
fn hands_callback_events() {
    let (mut m, mut cpu) = setup(&[3, 0x1000, 0x77]);
    m.write32(0x1000 + 0x54, 0x2000);
    m.write32(0x2000 + 0x28, 7);
    let mut c = Rec::default();
    w::weapon_record_37_default_hands_callback(&mut c, &mut m, &mut cpu);
    assert_eq!((cpu.eax, m.read32(0x2028)), (1, 6));
    m.write32(0x9004, 4);
    w::weapon_record_37_default_hands_callback(&mut c, &mut m, &mut cpu);
    assert_eq!(cpu.eax, 1);
    m.write32(0x9004, 0x11);
    m.write32(0x1000 + 0xE8, 0x16);
    w::weapon_record_37_default_hands_callback(&mut c, &mut m, &mut cpu);
    assert_eq!(
        (cpu.eax, c.calls.clone()),
        (0, vec![(0x1B8690, [0xCBB, 0x77, 1])])
    );
    m.write32(0x1000 + 0xE8, 0x14);
    w::weapon_record_37_default_hands_callback(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls.len(), 1);
}
