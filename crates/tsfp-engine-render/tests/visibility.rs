// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 FABRICATED-input semantic differential of native portal projection.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_engine_render::{
    scene::visibility::{ProjectionViewport, Rect, clip_project},
    scene_batch, scene_t1938,
};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};
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
struct Calls(Memory);
impl GuestCall for Calls {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        assert_eq!(va, 0x87a70);
        regs.esp -= 4;
        self.0.write32(regs.esp, 0x1234);
        scene_t1938::game_portal_polygon_clip_one_plane_by_flag_array_0x140_interpolating_edge_points(&mut self.0,regs);
        regs.esp += 4;
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
fn four_thousand_portal_projections_match_reference_visible_output() {
    let mut seed = 0x87bb0u32;
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };
    for trial in 0..4000 {
        let mut memory = Memory::default();
        let mut r = regs();
        let left = (random() % 100) as i32;
        let right = left + 20 + (random() % 200) as i32;
        let top = (random() % 100) as i32;
        let bottom = top + 20 + (random() % 200) as i32;
        let rect = Rect {
            left,
            right,
            top,
            bottom,
        };
        for (i, value) in [0x2000, left as u32, right as u32, top as u32, bottom as u32]
            .iter()
            .enumerate()
        {
            memory.write32(r.esp + 4 + i as u32 * 4, *value);
        }
        memory.write32(0x7e71c8, 0x3000);
        for (offset, value) in [(8, 160), (0x10, 160), (0x1c, 120), (0x24, 120)] {
            memory.write32(0x3000 + offset, value);
        }
        let count = trial % 8;
        memory.write32(0x2194, count);
        let mut points = Vec::new();
        for i in 0..count {
            let point = core::array::from_fn(|_| (random() % 400) as f32 / 100. - 2.);
            for (j, value) in point.iter().enumerate() {
                memory.write32(0x2000 + i * 16 + j as u32 * 4, value.to_bits());
            }
            points.push(point);
        }
        let mut calls = Calls(memory.clone());
        scene_batch::game_portal_polygon_clip_to_viewport_rect_and_project_to_screen_coords(
            &mut memory,
            &mut calls,
            &mut r,
        );
        let actual = clip_project(
            &points,
            ProjectionViewport {
                x_offset: 160.,
                x_scale: 160.,
                y_offset: 120.,
                y_scale: 120.,
            },
            rect,
        );
        let rejected = memory.read32(0x2190) != 0;
        assert_eq!(actual.is_none(), rejected, "trial {trial}");
        if let Some(points) = actual {
            assert_eq!(
                points.len(),
                memory.read32(0x2194) as usize,
                "trial {trial}"
            );
            for (i, point) in points.iter().enumerate() {
                for (axis, value) in point.iter().enumerate() {
                    let original =
                        f32::from_bits(memory.read32(0x2000 + i as u32 * 16 + axis as u32 * 4));
                    assert!(
                        value.to_bits() == original.to_bits()
                            || (value.is_nan() && original.is_nan()),
                        "trial {trial}, point {i} axis {axis}: {value} != {original}"
                    );
                }
            }
        }
    }
}
