// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4c Tier 0 smoke tests for the engine/core second half.
mod support;
use support::Mem;
use tsfp_engine_core::w4c_b::*;
use tsfp_engine_object::closure::{callee::{Callee, Cpu}, memory::GuestMemory};

#[derive(Default)]
struct Rec { calls: Vec<(u32, u32, Vec<u32>)>, ret: u32 }
impl Callee<Mem> for Rec {
    fn call(&mut self, m: &mut Mem, t: u32, cpu: &mut Cpu) {
        let n = match t { 0x3_ED90 | 0x8_0F20 | 0xC_B3B0 | 0x3_C8FD0 => 1, 0x12_8440 => 5, 0x8_7600 => 2, 0x2_2040 => 2, _ => 0 };
        self.calls.push((t, m.read32(cpu.esp), (0..n).map(|i| m.read32(cpu.esp + 4 + 4 * i)).collect()));
        cpu.eax = self.ret;
    }
}
fn setup(args: &[u32]) -> (Mem, Cpu) {
    let mut m = Mem::default();
    for (i, a) in args.iter().enumerate() { m.write32(0x9004 + 4 * i as u32, *a); }
    (m, Cpu { esp: 0x9000, ..Cpu::default() })
}

#[test]
fn simple_flag_handlers() {
    let (mut m, mut cpu) = setup(&[]);
    launch_arg_set_flag_6b7a94(&mut m, &mut cpu);
    launch_arg_set_flag_6b7a98(&mut m, &mut cpu);
    launch_arg_set_flag_6b7a9c(&mut m, &mut cpu);
    launch_arg_set_flag_6b7aa0(&mut m, &mut cpu);
    launch_arg_set_flag_6b7aa4(&mut m, &mut cpu);
    for a in [0x6B7A94, 0x6B7A98, 0x6B7A9C, 0x6B7AA0, 0x6B7AA4] { assert_eq!(m.read32(a), 1); }
    m.write32(0x4C0468, 7);
    launch_arg_clear_4c0468(&mut m, &mut cpu);
    assert_eq!((m.read32(0x4C0468), cpu.eax), (0, 1));
    m.write32(0x6B7B10, 3);
    global_6b7b10_clear(&mut m);
    assert_eq!(m.read32(0x6B7B10), 0);
}
#[test]
fn atol_handler_and_array_get() {
    let (mut m, mut cpu) = setup(&[0x1234]);
    let mut c = Rec { ret: 42, ..Rec::default() };
    launch_arg_set_6b7a90_from_atol(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls, [(0x3C8FD0, 0x3D2EA, vec![0x1234])]);
    assert_eq!((m.read32(0x6B7A90), cpu.eax, cpu.esp), (42, 1, 0x9000));
    let (mut m, mut cpu) = setup(&[2]);
    m.write32(0x6B7A28 + 8, 99);
    launch_array_6b7a28_get(&m, &mut cpu);
    assert_eq!(cpu.eax, 99);
}
#[test]
fn pool_reset_calls_used_entries() {
    let (mut m, mut cpu) = setup(&[1]);
    m.write32(0x6B83D8 + 16, 2);
    m.write32(0x6B83E4 + 16, 0x5000);
    m.write32(0x5000 + 8, 1); m.write32(0x5000, 0xA); m.write32(0x5000 + 4, 0xB); m.write32(0x5000 + 0xC, 0x3ED90);
    m.write32(0x5030 + 8, 0);
    let mut c = Rec::default();
    stream_block_pool_group_reset(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls.len(), 1);
    assert_eq!(c.calls[0].1, 0x3EDCD);
    assert_eq!(m.read32(0x5000 + 8), 0);
    assert_eq!(m.read32(0x5000 + 0x10), u32::MAX);
    assert_eq!(m.read32(0x5030 + 0x18), u32::MAX);
    assert_eq!(cpu.esp, 0x9000);
}
#[test]
fn groups_0_and_1_and_clear() {
    let (mut m, mut cpu) = setup(&[]);
    let mut c = Rec::default();
    release_stream_block_pool_groups_0_and_1(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls, [(0x3ED90, 0x3EE09, vec![0]), (0x3ED90, 0x3EE09, vec![1])]);
    m.write32(0x6B83F4, 5);
    globals_6b83d8_block_clear(&mut m, &mut cpu);
    assert_eq!(m.read32(0x6B83F4), 0);
}
#[test]
fn table_find() {
    let (mut m, mut cpu) = setup(&[0x77]);
    m.write16_for_test();
    table_515ff8_find_entry(&m, &mut cpu);
    assert_eq!(cpu.eax, 0x515FF8 + 0x80);
}
trait W { fn write16_for_test(&mut self); }
impl W for Mem {
    fn write16_for_test(&mut self) {
        self.write32(0x515FFC, 1); // record 0 valid
        self.write32(0x515FFC + 0x80, 1); // record 1 valid
        self.write32(0x515FFC + 0x80 + 4, 0x77);
    }
}
#[test]
fn record_init_and_launch_copy() {
    let (mut m, cpu) = setup(&[0x4000, 9]);
    m.write32(0x4000 + 0x100, 5);
    init_record_owner_ids_minus1(&mut m, &cpu);
    assert_eq!((m.read32(0x4000), m.read32(0x4054), m.read32(0x4074), m.read32(0x4100)), (9, u32::MAX, u32::MAX, 0));
    let (mut m, mut cpu) = setup(&[]);
    m.write32(0x6B7A94, 1); m.write32(0x7B0CC0, 0x55);
    let mut c = Rec { ret: 8, ..Rec::default() };
    launch_flags_copy_and_init(&mut c, &mut m, &mut cpu);
    assert_eq!((m.read32(0x6D6128), m.read32(0x6D6140)), (1, 8));
    assert_eq!(c.calls[0], (0xCB3B0, 0x56F19, vec![0x55]));
}
#[test]
fn pair_and_mode_dispatch() {
    let (mut m, mut cpu) = setup(&[1, 2]);
    let mut c = Rec::default();
    launch_info_query_store_pair(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls[0], (0x22040, 0x18E8DF, vec![0x9004, 0x9008]));
    assert_eq!((m.read32(0x74CF68), m.read32(0x74CF6C)), (1, 2));
    assert_eq!(c.calls.len(), 3);
    let (mut m, mut cpu) = setup(&[]);
    m.write32(0x79094C, 0x1E);
    let mut c = Rec::default();
    init_globals_75f5a8_then_dispatch_by_mode(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls[0].2, [1, 0x24D, 0, 0x5201C0, 0x75F5B4]);
    assert_eq!(c.calls[1], (0x87600, 0x257B9A, vec![2, 8]));
    assert_eq!(m.read32(0x75F5AC), 0xFFFF_FFF6);
    m.write32(0x79094C, 0x21);
    let mut c = Rec::default();
    init_globals_75f5a8_then_dispatch_by_mode(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls[0].0, 0x1F2560);
}
#[test]
fn buffer_and_effect_params() {
    let (mut m, mut cpu) = setup(&[0x6000]);
    m.write32(0x78B1EC, 0x7000); m.write32(0x475D5C, 0x3F80_0000);
    init_buffer_78b1ec_floats_and_terminator(&mut m);
    assert_eq!(m.read32(0x7000 + 0x18), 0x3F80_0000);
    assert_eq!(m.read32(0x7000 + 0x3E60 + 0x18), 0x3F80_0000);
    assert_eq!(m.read32(0x7000 + 0x3E80), 0x1F3);
    m.write32(0x4C0454, 4);
    let mut c = Rec { ret: 0x33, ..Rec::default() };
    init_effect_params_with_two_texture_entries(&mut c, &mut m, &mut cpu);
    assert_eq!(c.calls, [(0x80F20, 0x2BD9CA, vec![4]), (0x80F20, 0x2BD9D4, vec![8])]);
    assert_eq!((m.read32(0x6014), m.read32(0x6018)), (0x33, 0x33));
}
