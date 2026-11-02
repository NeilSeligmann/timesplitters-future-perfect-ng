// SPDX-License-Identifier: GPL-3.0-or-later
// Standalone rustc harness: avoids the pre-existing full lib's w4_object closure import defect.
#![allow(dead_code, unused_imports)]
#[path = "../memory.rs"] mod memory;
#[path = "../views.rs"] mod views;
#[path = "../abi.rs"] mod abi;
#[path = "../inventory.rs"] mod inventory;
#[path = "../pose.rs"] mod pose;
#[path = "../video.rs"] mod video;
#[path = "../vehicles.rs"] mod vehicles;
#[path = "mod.rs"] mod system_slice;
#[path = "../combat.rs"] mod combat;
#[path = "../ext.rs"] mod ext;
#[path = "../inventory_ammo.rs"] mod inventory_ammo;
#[path = "../inventory_grant.rs"] mod inventory_grant;
#[path = "../inventory_pickup.rs"] mod inventory_pickup;
#[path = "../inventory_select.rs"] mod inventory_select;
#[path = "../inventory_slots.rs"] mod inventory_slots;
#[path = "../pose_anim.rs"] mod pose_anim;
#[path = "../pose_leaf.rs"] mod pose_leaf;
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::{collections::BTreeMap, rc::Rc, cell::RefCell};
#[derive(Default)] struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 { *self.0.borrow().get(&a).unwrap_or(&0) }
    fn write8(&mut self, a: u32, v: u8) { self.0.borrow_mut().insert(a, v); }
}
#[derive(Default)] struct Guest { seen: Vec<u32>, mem: Mem }
impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert!(self.seen.len() < 256, "unbounded callee loop");
        self.seen.push(va);
        r.eax = 0;
        // Script the decoder terminal status, and honor XDK stdcall argument cleanup.
        if va == 0x44525d {
            let status = self.mem.read32(r.esp + 8);
            self.mem.write32(status, 2);
        }
        let args = match va {
            0x444a2d => 3, 0x4450c2 => 5, 0x445241 => 2, 0x445055 => 2,
            0x3e6684 => 9, 0x3d4d70 => 2, 0x3d4e10 => 2, 0x3d5eb0 => 6,
            0x3d8e50 | 0x3d9990 | 0x444f71 | 0x3d4c90 | 0x445252 => 1,
            0x44525d => 4, 0x3d9810 => 5, 0x407b14 => 2,
            _ => 0,
        };
        r.esp += args * 4;
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters { eax: 0, ecx: 0, edx: 0, esp: 0x10000, ebx: 0, ebp: 0, esi: 0, edi: 0, eflags: 0,
        fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x27f }, xmm: [Xmm::default(); 5], xmm_hi: [Xmm::default(); 3] }
}
macro_rules! smoke {
    ($name:ident, $module:ident, $root:ident) => {
        #[test] fn $name() {
            let mut m = Mem::default(); let mut g = Guest { mem: Mem(m.0.clone()), ..Guest::default() }; let mut r = regs();
            $module::$root(&mut m, &mut g, &mut r);
            assert_eq!(r.esp, 0x10000, "root must balance caller stack");
        }
    }
}
smoke!(video_tail, video, f28d40);
smoke!(video_config, video, f30210);
smoke!(video_decoder_terminal_status, video, f30280);
smoke!(vehicle_transform, vehicles, f21860);
smoke!(vehicle_record_set, vehicles, f168160);
smoke!(vehicle_plne, vehicles, f1e1dc0);
smoke!(vehicle_plns, vehicles, f1e1de0);
smoke!(vehicle_driver_passenger, vehicles, f1e1e00);
smoke!(vehicle_initialize, vehicles, f1e2890);
smoke!(vehicle_mode, vehicles, f1e2ad0);
smoke!(vehicle_tick, vehicles, f1e2b80);
smoke!(vehicle_slot_release, vehicles, f1e4030);
smoke!(vehicle_effect_tick, vehicles, f1f9330);
smoke!(vehicle_effect_setup, vehicles, f1f9800);
smoke!(vehicle_tag_false, vehicles, f27e940);
smoke!(vehicle_tag_true, vehicles, f27e960);
#[test] fn x87_precision_preserves_exponent_range_and_unordered_compare() {
    assert!(system_slice::round_pc(1e100, 0).is_finite());
    assert_ne!(system_slice::round_pc(1e-100, 0), 0.0);
    assert_eq!(system_slice::round_pc(1.00000001, 0), 1.0);
    assert_eq!(system_slice::round_pc(1.00000001, 0x200), 1.00000001);
    assert_eq!(system_slice::compare_fp(f64::NAN, 1.0), 2);
}
#[test] fn viewport_sentinel_conversions_and_invalid_mode_preserve_state() {
    for (width, height, mode, expected_width, expected_height, expected_mode) in [
        (640u32, 480u32, 1u32, 640i32, 480i32, 1u32),
        (u32::MAX, 480, 0, 640, 480, 0),
        (640, u32::MAX, 9, 640, 480, 7),
        (u32::MAX, u32::MAX, 9, 11, 22, 7),
    ] {
        let mut m = Mem::default(); let mut g = Guest::default(); let mut r = regs();
        m.write32(0x476688, (4.0f32 / 3.0).to_bits());
        m.write32(0x475d40, 0.75f32.to_bits());
        for (a, v) in [(0x10004, width), (0x10008, height), (0x1000c, mode),
            (0x687e44, 11), (0x687e48, 22), (0x4bfc6c, 7)] { m.write32(a, v); }
        video::f30210(&mut m, &mut g, &mut r);
        assert_eq!(video::VideoViewport::read(&m), video::VideoViewport {
            width: expected_width, height: expected_height, mode: expected_mode });
    }
}
#[test] fn physics_entry_masks_index_and_sets_field_without_adjacent_writes() {
    let mut m = Mem::default(); let mut g = Guest::default(); let mut r = regs();
    m.write32(0x74c458, 0x20000);
    m.write32(0x10004, 0x10003); m.write32(0x10008, 0xdeadbeef);
    let a = 0x20000 + 3 * 0x200 + 0x1c4;
    m.write32(a - 4, 0xaabbccdd); m.write32(a + 4, 0x12345678);
    vehicles::f168160(&mut m, &mut g, &mut r);
    assert_eq!(m.read32(a), 0xdeadbeef);
    assert_eq!(m.read32(a - 4), 0xaabbccdd); assert_eq!(m.read32(a + 4), 0x12345678);
    assert_eq!(vehicles::VehicleGlobals::read(&m).physics_entry(0x10003), 0x20600);
    assert_eq!((r.eax, r.ecx, r.edx, r.esp), (0x600, 0xdeadbeef, 0x20000, 0x10000));
    assert!(g.seen.is_empty());
}
#[test] fn disabled_and_unknown_selectors_do_not_mutate() {
    let mut m = Mem::default(); let mut g = Guest::default(); let mut r = regs(); let before = r;
    assert!(!video::run(0x30210, false, &mut m, &mut g, &mut r));
    assert!(!vehicles::run(0x123456, true, &mut m, &mut g, &mut r));
    assert_eq!(r, before); assert!(m.0.borrow().is_empty()); assert!(g.seen.is_empty());
}
