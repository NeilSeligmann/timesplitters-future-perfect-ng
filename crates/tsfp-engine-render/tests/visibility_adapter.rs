// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 FABRICATED room graph differential. Shared render outputs only.
extern crate alloc;
pub use tsfp_ports::{abi, memory};
mod scene {
    pub use tsfp_engine_render::scene::{visibility, spatial};
}
#[path = "../../tsfp-ports/src/scene_lane/visibility_adapter.rs"]
mod adapter;
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_engine_render::{scene_batch, scene_t1938};
#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        *self.0.borrow().get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}
impl Memory {
    fn set16(&mut self, address: u32, value: u16) {
        for (i, b) in value.to_le_bytes().iter().enumerate() {
            self.write8(address + i as u32, *b);
        }
    }
    fn vector(&mut self, address: u32, value: [f32; 3]) {
        for (i, v) in value.iter().enumerate() {
            self.write32(address + i as u32 * 4, v.to_bits());
        }
    }
    fn independent(&self) -> Self {
        Self(Rc::new(RefCell::new(self.0.borrow().clone())))
    }
}
struct Calls(Memory);
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        r.esp -= 4;
        self.0.write32(r.esp, 0x1234);
        match va {
            0x87f80=>scene_batch::game_level_room_visibility_update_screen_rect_and_recurse_through_portals(&mut self.0.clone(),self,r),
            0x87bb0=>scene_batch::game_portal_polygon_clip_to_viewport_rect_and_project_to_screen_coords(&mut self.0.clone(),self,r),
            0x87a70=>scene_t1938::game_portal_polygon_clip_one_plane_by_flag_array_0x140_interpolating_edge_points(&mut self.0,r),
            0x151fc0=>tsfp_ports::c2r::t1937_math::game_matrix4x4_transform_point_vec3_to_vec4(&mut self.0,r),
            0x8c620=>tsfp_ports::c2r::data_missions::game_level_portal_clip_list_object_points_inside_any_polygon_returns_minus_one_else_1(&mut self.0.clone(),self,r),
            0x8bdd0=>scene_batch::game_level_room_points_all_inside_polygon_planes_test(&mut self.0,r),
            _=>panic!("unexpected {va:x}"),
        }
        r.esp += 4;
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x10000,
        ebx: 1,
        ebp: 2,
        esi: 3,
        edi: 4,
        eflags: 0x202,
        fpu: Fpu {
            stack: [0.; 8],
            top: 0,
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
#[test]
fn five_hundred_graphs_match_original_shared_visibility_outputs() {
    let mut seed = 0x2238u32;
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };
    for trial in 0..500 {
        let mut m = Memory::default();
        let mut original = regs();
        for (a, v) in [
            (0x7b975c, 3),
            (0x7b9754, 3),
            (0x7b0cc0, 0x100000),
            (0x708788, 0x200000),
            (0x70878c, 0x300000),
            (0x708778, 0x400000),
            (0x70383c, 0x500000),
            (0x7e71c8, 0x700000),
        ] {
            m.write32(a, v);
        }
        m.write32(0x10020c, 0x600000);
        m.write32(0x100214, 0);
        m.write32(0x79094c, if trial % 3 == 0 { 0x64 } else { 1 });
        m.write32(0x475dbc, 0.1f32.to_bits());
        m.write32(0x475cac, 0.0f32.to_bits());
        m.vector(0x10027c, [0., 0., (random() % 10) as f32 - 5.]);
        for (i, v) in [
            1f32, 0., 0., 0., 0., 1., 0., 0., 0.1, 0.1, 1., 0.1, 0., 0., 0., 1.,
        ]
        .iter()
        .enumerate()
        {
            m.write32(0x600000 + i as u32 * 4, v.to_bits());
        }
        for (o, v) in [
            (0, 0),
            (4, 640),
            (8, 320),
            (0x10, 320),
            (0x14, 0),
            (0x18, 480),
            (0x1c, 240),
            (0x24, 240),
        ] {
            m.write32(0x700000 + o, v);
        }
        for room in 0..=3 {
            let slot = 0x1003bc + room * 12;
            m.write32(slot, u32::MAX);
            for (o, v) in [(4, 640), (6, 480), (8, 65535), (10, 65535)] {
                m.set16(slot + o, v);
            }
            m.write32(
                0x500000 + room * 4,
                if room != 1 && random() % 11 == 0 {
                    1
                } else {
                    0
                },
            );
            let list = 0xa00000 + room * 0x100;
            if room != 0 {
                m.write32(0x200000 + room * 0xb4 + 4, list);
                m.write32(list, 2);
                for (j, portal) in match room {
                    1 => [1, 3],
                    2 => [1, 2],
                    _ => [2, 3],
                }
                .iter()
                .enumerate()
                {
                    m.write32(list + 4 + j as u32 * 4, *portal);
                }
                m.write8(
                    0x200000 + room * 0xb4 + 0xb2,
                    if random() % 9 == 0 { 0x40 } else { 0 },
                );
            }
        }
        m.write32(0x100d28, u32::MAX);
        for portal in 0..=3 {
            let slot = 0x400000 + portal * 40;
            for (o, v) in [(0, 640), (2, 480), (4, 65535), (6, 65535)] {
                m.set16(slot + o, v);
            }
            m.write32(slot + 0x24, random() % 4);
            if portal != 0 {
                let p = 0x800000 + portal * 0x100;
                m.write32(0x300000 + portal * 4, p);
                let [a, b] = match portal {
                    1 => [1, 2],
                    2 => [2, 3],
                    _ => [3, 1],
                };
                m.write32(p, a);
                m.write32(p + 4, b);
                m.vector(p + 8, [0., 0., if random() % 2 == 0 { 1. } else { -1. }]);
                m.set16(p + 0x14, 4);
                m.set16(p + 0x16, if random() % 7 == 0 { 1 } else { 0 });
                let z = (random() % 10) as f32 - 5.;
                let size = (random() % 20 + 1) as f32 / 10.;
                for (i, [x, y]) in [[-size, -size], [size, -size], [size, size], [-size, size]]
                    .iter()
                    .enumerate()
                {
                    m.vector(p + 0x18 + i as u32 * 12, [*x, *y, z]);
                }
            }
        }
        if trial % 13 == 0 {
            m.write32(0x70a908, 1);
            m.write32(0x708928, 0x900000);
            m.set16(0x900010, 0);
            m.vector(0x708930, [1., 0., 0.]);
            m.write32(0x70893c, (-0.2f32).to_bits());
        }
        for (i, v) in [1, trial % 3, 0, 640, 0, 480].iter().enumerate() {
            m.write32(original.esp + 4 + i as u32 * 4, *v);
        }
        let mut native = original;
        let mut native_memory = m.independent();
        let mut calls = Calls(m.clone());
        scene_batch::game_level_room_visibility_update_screen_rect_and_recurse_through_portals(
            &mut m,
            &mut calls,
            &mut original,
        );
        assert!(
            adapter::run(0x87f80, &mut native_memory, &mut native),
            "trial {trial} refused"
        );
        for (base, len) in [
            (0x1003bc, 48),
            (0x100d28, 4),
            (0x400000, 160),
            (0x500000, 16),
        ] {
            for offset in 0..len {
                assert_eq!(
                    native_memory.read8(base + offset),
                    m.read8(base + offset),
                    "trial {trial}, byte {:x}",
                    base + offset
                );
            }
        }
        assert_eq!(native.esp, original.esp);
    }
}
