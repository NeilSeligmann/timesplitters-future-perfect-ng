// SPDX-License-Identifier: GPL-3.0-or-later
// Standalone rustc harness: avoids the pre-existing full lib's w4_object closure import defect.
#![allow(dead_code, unused_imports)]
#[path = "../abi.rs"]
mod abi;
#[path = "../audio_remainder.rs"]
mod audio;
#[path = "../inventory.rs"]
mod inventory;
#[path = "../memory.rs"]
mod memory;
#[path = "../pose.rs"]
mod pose;

#[path = "../c2r/rt.rs"]
pub mod c2r_rt;
#[path = "../combat.rs"]
mod combat;
#[path = "../ext.rs"]
mod ext;
#[path = "../inventory_ammo.rs"]
mod inventory_ammo;
#[path = "../inventory_grant.rs"]
mod inventory_grant;
#[path = "../inventory_pickup.rs"]
mod inventory_pickup;
#[path = "../inventory_select.rs"]
mod inventory_select;
#[path = "../inventory_slots.rs"]
mod inventory_slots;
#[path = "../pose_anim.rs"]
mod pose_anim;
#[path = "../pose_leaf.rs"]
mod pose_leaf;
#[path = "../system_slice/mod.rs"]
mod system_slice;
mod c2r {
    pub use crate::c2r_rt as rt;
}
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::{cell::Cell, collections::BTreeMap};
#[derive(Default)]
struct Mem {
    data: BTreeMap<u32, u8>,
    reads: Cell<usize>,
}
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        let n = self.reads.get() + 1;
        self.reads.set(n);
        assert!(n < 100000, "bounded smoke memory budget");
        *self.data.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.data.insert(a, v);
    }
}
#[derive(Default)]
struct Guest {
    seen: Vec<u32>,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert!(self.seen.len() < 1000, "bounded smoke call budget");
        self.seen.push(va);
        r.eax = 0;
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x10000,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
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
#[test]
fn disabled_boundary_is_inert() {
    let mut m = Mem::default();
    let mut g = Guest::default();
    let mut r = regs();
    for &va in audio::ROOTS {
        assert!(!audio::run(va, false, &mut m, &mut g, &mut r));
    }
    assert!(!audio::run(0, true, &mut m, &mut g, &mut r));
    assert!(m.data.is_empty());
    assert_eq!(m.reads.get(), 0);
    assert!(g.seen.is_empty());
    assert_eq!(r.esp, 0x10000);
}
#[test]
fn bounded_all_root_smoke() {
    for &va in audio::ROOTS {
        let result = std::panic::catch_unwind(|| {
            let mut m = Mem::default();
            m.write32(0x5659f4, 0x5659c4);
            m.write32(0x5836c0, 0x583690);
            if va == 0x27700 {
                m.write32(0x79094c, 0x67);
            }
            let mut g = Guest::default();
            let mut r = regs();
            assert!(audio::run(va, true, &mut m, &mut g, &mut r));
        });
        assert!(result.is_ok(), "root {va:08x} failed smoke");
        println!("SMOKE {va:08x} pass");
    }
}

#[test]
fn stream_setters_preserve_guest_ownership_and_pop_argument() {
    for (va, offset, value) in [
        (0x29e40, 0x64, 0x12345678),
        (0x29e50, 0x58, 2.5f32.to_bits()),
    ] {
        let mut m = Mem::default();
        let mut g = Guest::default();
        let mut r = regs();
        r.ecx = 0x20000;
        m.write32(r.esp, 0xabcdef);
        m.write32(r.esp + 4, value);
        assert!(audio::run(va, true, &mut m, &mut g, &mut r));
        assert_eq!(m.read32(0x20000 + offset), value);
        assert_eq!(r.esp, 0x10004);
        assert_eq!(m.read32(0x10000), 0xabcdef);
        assert!(g.seen.is_empty());
    }
    for (input, expected) in [
        (-3.0f32, 0.0f32),
        (0.25, 0.25),
        (4.0, 1.0),
        (f32::NAN, f32::NAN),
    ] {
        let mut m = Mem::default();
        let mut g = Guest::default();
        let mut r = regs();
        r.ecx = 0x20000;
        m.write32(r.esp + 4, input.to_bits());
        m.write32(0x475c78, 1.0f32.to_bits());
        audio::f29d90(&mut m, &mut g, &mut r);
        assert_eq!(m.read32(0x2005c), expected.to_bits());
        assert_eq!(r.esp, 0x10004);
    }
}
#[test]
fn frequency_bridge_obeys_backend_stdcall_and_preserves_callee_state() {
    struct Backend {
        esp: u32,
    }
    impl GuestCall for Backend {
        fn call(&mut self, va: u32, r: &mut ExtRegisters) {
            assert_eq!(va, 0x4085cf);
            self.esp = r.esp;
            r.esp += 8;
            r.eax = 0x55;
            r.fpu.stack[0] = 17.0;
            r.xmm_hi[2] = Xmm::scalar(0x12345678);
        }
    }
    let mut m = Mem::default();
    let mut g = Backend { esp: 0 };
    let mut r = regs();
    r.ecx = 0x20000;
    r.esi = 0x9876;
    m.write32(r.esp + 4, 44100);
    m.write32(0x20004, 0x7890);
    m.write32(0x20024, 99);
    audio::f29b20(&mut m, &mut g, &mut r);
    assert_eq!(m.read32(g.esp), 0x7890);
    assert_eq!(m.read32(g.esp + 4), 44100);
    assert_eq!(m.read32(0x20024), 0);
    assert_eq!(r.esi, 0x9876);
    assert_eq!(r.esp, 0x10004);
    assert_eq!(r.eax, 0x55);
    assert_eq!(r.fpu.stack[0], 17.0);
    assert_eq!(r.xmm_hi[2].lanes[0], 0x12345678);
}
#[test]
fn volume_tail_uses_native_ftol_and_preserves_return_slot() {
    let mut m = Mem::default();
    let mut g = Guest::default();
    let mut r = regs();
    r.eax = 1;
    m.write32(r.esp, 0x1234);
    m.write32(r.esp + 4, 0.1f32.to_bits());
    m.write32(0x475c78, 1.0f32.to_bits());
    m.write32(0x475d60, 0.00001f32.to_bits());
    m.write32(0x4761d4, 2000.0f32.to_bits());
    audio::f26f00(&mut m, &mut g, &mut r);
    assert!(g.seen.is_empty());
    assert_eq!(r.eax, (-2000i32) as u32);
    assert_eq!(r.esp, 0x10000);
    assert_eq!(m.read32(r.esp), 0x1234);
    assert_eq!(r.fpu.top, 0);
}
