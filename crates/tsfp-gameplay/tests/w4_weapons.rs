// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4 Tier 0 smoke tests: fake guest memory plus a recorded callee trace.
use std::collections::HashMap;
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use tsfp_gameplay::w4_weapons as w;

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
/// Records (target, return address, first four stack args); answers with scripted EAX values.
#[derive(Default)]
struct Trace {
    calls: Vec<(u32, u32, [u32; 4])>,
    answers: HashMap<u32, u32>,
}
impl Callee<Mem> for Trace {
    fn call(&mut self, m: &mut Mem, target: u32, cpu: &mut Cpu) {
        let a = |i: u32| m.read32(cpu.esp + 4 + 4 * i);
        self.calls
            .push((target, m.read32(cpu.esp), [a(0), a(1), a(2), a(3)]));
        cpu.eax = *self.answers.get(&target).unwrap_or(&0);
    }
}
fn cpu() -> Cpu {
    Cpu {
        esp: 0x1000,
        ..Cpu::default()
    }
}

#[test]
fn record_decay_list_tail_call_passes_record_plus_0x304() {
    let (mut m, mut t, mut c) = (Mem::default(), Trace::default(), cpu());
    m.write32(0x1004, 0x2000);
    m.write32(0x2000 + 0x7c, 0x3000);
    w::record_decay_list_timers(&mut t, &mut m, &mut c);
    assert_eq!(m.read32(0x1004), 0x3304);
    assert_eq!(t.calls[0].0, 0xdb4c0);
}

#[test]
fn player_frame_update_stops_on_mode_9() {
    let (mut m, mut t, mut c) = (Mem::default(), Trace::default(), cpu());
    m.write32(0x1004, 0x2000);
    m.write32(0x2000 + 0x7c, 0x3000);
    t.answers.insert(0xc63d0, 9);
    w::player_frame_update(&mut t, &mut m, &mut c);
    assert_eq!(t.calls.len(), 1);
    assert_eq!(c.esp, 0x1000);
}

#[test]
fn player_frame_update_decays_timers_and_starts_effect() {
    let (mut m, mut t, mut c) = (Mem::default(), Trace::default(), cpu());
    m.write32(0x1004, 0x2000);
    m.write32(0x2000 + 0x7c, 0x3000);
    m.write32(0x3000 + 8, 5);
    m.write32(0x3000 + 0xad4, 1.0f32.to_bits());
    m.write32(0x3000 + 0x94, 0x7777);
    m.write32(0x4e8cec, 2);
    m.write32(0x4e794c, 1.0f32.to_bits());
    m.write32(0x475c78, 4.0f32.to_bits());
    t.answers.insert(0x3568c0, 1);
    m.write32(0x790950, 9);
    w::player_frame_update(&mut t, &mut m, &mut c);
    assert_eq!(f32::from_bits(m.read32(0x3000 + 0xad4)), 0.5);
    let effect = t.calls.iter().find(|call| call.0 == 0x2385b0).unwrap();
    assert_eq!(effect.2, [3, 5, 0x7777, 2.0f32.to_bits()]);
    assert_eq!(t.calls.last().unwrap().0, 0x1bde40);
}

#[test]
fn fpistol_callback_clear_code_dispatches_by_table() {
    let (mut m, mut t, mut c) = (Mem::default(), Trace::default(), cpu());
    m.write32(0x1004, 0x12); // above 0x11: no-op
    w::fpistol_event_callback(&mut t, &mut m, &mut c);
    assert_eq!((c.eax, t.calls.len()), (0, 0));
    // code 0 -> table entry 0 -> jump target 0x1C6AB7 (sound 0xC87)
    m.write32(0x1004, 0);
    m.write8(0x1c6be4, 0);
    m.write32(0x1c6bc8, 0x1c6ab7);
    w::fpistol_event_callback(&mut t, &mut m, &mut c);
    assert_eq!(t.calls[0].0, 0x45590);
    assert_eq!(t.calls[0].2[0], 0xc87);
}

#[test]
fn fpistol_callback_state_9_plays_sound() {
    let (mut m, mut t, mut c) = (Mem::default(), Trace::default(), cpu());
    m.write32(0x1004, 1);
    m.write8(0x1c6be4 + 1, 6);
    m.write32(0x1c6bc8 + 24, 0x1c6b40);
    m.write32(0x1008, 0x2000);
    m.write32(0x100c, 0x3000);
    m.write32(0x2000 + 0xe8, 9);
    w::fpistol_event_callback(&mut t, &mut m, &mut c);
    assert_eq!(c.eax, 1);
    assert_eq!(t.calls[0].2, [0xc87, 0x3000, 0x3f19999a, 1]);
}

#[test]
fn fpistol_callback_counter_bump_leaves_the_converted_value_in_edx() {
    // Jump target 0x1C6AF4: two counters of the 0xF8AE0 record get their float bump; the
    // original converts the second into EDX, which a caller may read (T3702 capture Tier 2).
    let (mut m, mut t, mut c) = (Mem::default(), Trace::default(), cpu());
    m.write32(0x1004, 2);
    m.write8(0x1c6be4 + 2, 3);
    m.write32(0x1c6bc8 + 12, 0x1c6af4);
    t.answers.insert(0xf8ae0, 0x5000);
    m.write32(0x5000 + 0x34, 10);
    m.write32(0x5000 + 0x2c, 52);
    m.write32(0x47cff4, 1.0f32.to_bits());
    m.write32(0x4782c4, 1.5f32.to_bits());
    w::fpistol_event_callback(&mut t, &mut m, &mut c);
    assert_eq!((m.read32(0x5034), m.read32(0x502c)), (11, 53));
    assert_eq!((c.eax, c.ecx, c.edx), (1, 11, 53));
}
