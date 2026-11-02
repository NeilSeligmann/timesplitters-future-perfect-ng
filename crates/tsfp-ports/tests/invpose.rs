// SPDX-License-Identifier: GPL-3.0-or-later
// T1850. FABRICATED synthetic fixtures with a scripted callee closure. Shape and regression checks for
// the pose and inventory ports; the original-vs-Rust equivalence is tests/c/test_rust_invpose.c.
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::collections::BTreeMap;
use tsfp_port_runtime::{abi, memory};
use tsfp_ports::{
    combat, ext, inventory, inventory_ammo, inventory_grant, inventory_pickup, inventory_select,
    inventory_slots, pose, pose_anim, pose_leaf,
};
#[derive(Default)]
struct Mem(BTreeMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
#[derive(Default)]
struct Guest {
    seen: Vec<(u32, u32)>,
    results: Vec<u32>,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.seen.push((va, r.esp));
        r.eax = if self.results.is_empty() {
            0
        } else {
            self.results.remove(0)
        };
        r.ecx = 0xc0c0;
        r.edx = 0xd0d0;
    }
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
const STACK: u32 = 0x9000;
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 1,
        ecx: 2,
        edx: 3,
        esp: STACK,
        ebx: 0xb,
        ebp: 0xe,
        esi: 0x51,
        edi: 0xd1,
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
fn with_args(m: &mut Mem, args: &[u32]) {
    m.write32(STACK, 0x1234);
    for (i, a) in args.iter().enumerate() {
        m.write32(STACK + 4 + 4 * i as u32, *a);
    }
}
#[test]
fn pad_map_find_index_returns_index_or_minus_one() {
    let mut m = Mem::default();
    for (i, v) in [7u32, 9, 11, 13].iter().enumerate() {
        m.write32(0x6b93c0 + 4 * i as u32, *v);
    }
    for (value, expect) in [(7u32, 0u32), (11, 2), (13, 3), (8, 0xffff_ffff)] {
        with_args(&mut m, &[value]);
        let mut r = regs();
        pose::pad_map_find_index(&mut m, &mut r);
        assert_eq!((r.eax, r.esp), (expect, STACK));
    }
}
#[test]
fn input_state_reset_neutral_zeroes_buttons_and_centres_axes() {
    let mut m = Mem::default();
    for o in (8..0x34).step_by(4) {
        m.write32(0x4000 + o, 0xdead_beef);
    }
    with_args(&mut m, &[0x4000]);
    let mut r = regs();
    pose::input_state_reset_neutral(&mut m, &mut r);
    for o in [8u32, 0xc, 0x10, 0x14] {
        assert_eq!(m.read32(0x4000 + o), 0);
    }
    for o in (0x18..0x34).step_by(4) {
        assert_eq!(m.read32(0x4000 + o), 0x80);
    }
    assert_eq!(m.read32(0x4000 + 4), 0, "bytes outside +8..+0x30 untouched");
}
#[test]
fn pad_map_is_bit_set_tests_the_mapped_bit() {
    let mut m = Mem::default();
    m.write32(0x6b93c0 + 4 * 2, 5);
    m.write16(0x7e7240, 1 << 5);
    with_args(&mut m, &[2]);
    let mut r = regs();
    pose::pad_map_is_bit_set(&mut m, &mut r);
    assert_eq!(r.eax, 1);
    m.write16(0x7e7240, 0);
    with_args(&mut m, &[2]);
    let mut r = regs();
    pose::pad_map_is_bit_set(&mut m, &mut r);
    assert_eq!(r.eax, 0);
}
#[test]
fn anim_slot_d4_equals_compares_state_slot() {
    let mut m = Mem::default();
    m.write32(0x3004, 0x3800);
    m.write32(0x3800 + 0xd4, 77);
    for (id, expect) in [(77u32, 1u32), (78, 0)] {
        with_args(&mut m, &[0x3000, id]);
        let mut r = regs();
        pose::anim_slot_d4_equals(&mut m, &mut r);
        assert_eq!(r.eax, expect);
    }
}
#[test]
fn anim_either_slot_equals_follows_the_blend_weight() {
    let mut m = Mem::default();
    m.write32(0x3004, 0x3800);
    m.write32(0x3800 + 0xd4, 5);
    m.write32(0x3800 + 0x12c, 6);
    m.write32(0x3800 + 0x198, 0x3f80_0000); // blend > 0 selects slot 0x12C
    with_args(&mut m, &[0x3000, 6]);
    let mut r = regs();
    pose::anim_either_slot_equals(&mut m, &mut r);
    assert_eq!(r.eax, 1);
    m.write32(0x3800 + 0x198, 0); // blend == 0 selects slot 0xD4
    with_args(&mut m, &[0x3000, 5]);
    let mut r = regs();
    pose::anim_either_slot_equals(&mut m, &mut r);
    assert_eq!(r.eax, 1);
    with_args(&mut m, &[0x3000, 6]);
    let mut r = regs();
    pose::anim_either_slot_equals(&mut m, &mut r);
    assert_eq!(r.eax, 0);
}
#[test]
fn renumber_entry_groups_counts_only_unflagged_entries() {
    let mut m = Mem::default();
    // entries 0..70 at 0x4FA128, stride 0x3C: flags with bit 0xC set do not start a new group
    for e in 0..70u32 {
        m.write32(0x4fa128 + e * 0x3c, if e % 3 == 1 { 4 } else { 0 });
    }
    let mut r = regs();
    inventory::renumber_entry_groups(&mut m, &mut r);
    let mut group = 0u32;
    for e in 0..70u32 {
        if e % 3 != 1 {
            group += 1;
        }
        assert_eq!(m.read32(0x4fa128 + e * 0x3c + 0x38), group - 1, "entry {e}");
    }
    assert_eq!(
        m.read32(0x75a024),
        group - 1,
        "final count is the last counter value"
    );
}
#[test]
fn group_has_owned_x2_entry_needs_same_group_flag_four_and_owned() {
    let mut m = Mem::default();
    let ext = 0x6000;
    m.write32(0x4fa128 + 5 * 0x3c + 0x38, 9); // entry 5 is in group 9
    m.write32(0x4fa128 + 12 * 0x3c + 0x38, 9); // entry 12 too
    m.write8(0x4fa128 + 12 * 0x3c, 4); // entry 12 is an x2 entry
    m.write8(ext + 0x54c + 12, 1); // and owned
    let mut r = regs();
    r.eax = 5;
    r.edx = ext;
    inventory::group_has_owned_x2_entry(&mut m, &mut r);
    assert_eq!(r.eax, 1);
    m.write8(ext + 0x54c + 12, 0);
    let mut r = regs();
    r.eax = 5;
    r.edx = ext;
    inventory::group_has_owned_x2_entry(&mut m, &mut r);
    assert_eq!(r.eax, 0);
}
#[test]
fn weapon_entry_release_clears_owned_and_reselects_only_the_current_entry() {
    let mut m = Mem::default();
    let ext = 0x6000;
    m.write8(ext + 0x54c + 4, 1);
    m.write32(ext + 0x94, 9); // current is entry 9, not 4
    m.write32(ext + 0x9c, 8);
    m.write32(ext + 8, 0xffff_ffff);
    with_args(&mut m, &[ext, 4]);
    let mut guest = Guest::default();
    let mut r = regs();
    inventory::weapon_entry_release(&mut m, &mut guest, &mut r);
    assert_eq!(m.read8(ext + 0x54c + 4), 0);
    assert!(
        guest.seen.is_empty(),
        "neither current nor secondary: no reselect call"
    );
    // releasing the current entry reselects through 0x1BAC50 then 0x1BA130
    m.write8(ext + 0x54c + 9, 1);
    with_args(&mut m, &[ext, 9]);
    guest.results = vec![3, 0];
    let mut r = regs();
    inventory::weapon_entry_release(&mut m, &mut guest, &mut r);
    let calls: Vec<u32> = guest.seen.iter().map(|c| c.0).collect();
    assert_eq!(&calls[..2], &[0x1bac50, 0x1ba130]);
    assert_eq!(
        m.read32(ext + 0x94),
        0,
        "the current slot is cleared before the reselect"
    );
}
#[test]
fn ammo_tail_jump_hands_the_callee_the_callers_argument_block() {
    // 0x1BA2B0 ends in a tail jump to 0x1BA220: no new return slot, so the bridge call must start one
    // word above the caller's return slot (a plain call would shift every argument by one word).
    let mut m = Mem::default();
    m.write32(0x4fa128 + 7 * 0x3c + 0x10, 0xffff_ffff); // no record: returns 0 without the tail jump
    with_args(&mut m, &[0x6000, 7, 3]);
    let mut guest = Guest::default();
    let mut r = regs();
    inventory_ammo::ammo_add_clips_of_slot_weapon_to_reserve(&mut m, &mut guest, &mut r);
    assert!(guest.seen.is_empty());
    m.write32(0x4fa128 + 7 * 0x3c + 0x10, 2);
    m.write32(0x4f9bac, 0x7000);
    m.write32(0x7000 + 2 * 0x29c + 0x14, 4);
    with_args(&mut m, &[0x6000, 7, 3]);
    let mut r = regs();
    inventory_ammo::ammo_add_clips_of_slot_weapon_to_reserve(&mut m, &mut guest, &mut r);
    assert_eq!(guest.seen, vec![(0x1ba220, STACK + 4)]);
    assert_eq!(
        r.esp, STACK,
        "esp is back at the entry value after the tail jump"
    );
}
