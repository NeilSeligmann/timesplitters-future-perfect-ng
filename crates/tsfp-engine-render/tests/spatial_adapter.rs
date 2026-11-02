// SPDX-License-Identifier: GPL-3.0-or-later
//! Fabricated-input differential, not live original evidence. T2238.
extern crate alloc;
pub use tsfp_ports::{abi, c2r, memory};
#[path = "../../tsfp-ports/src/scene_lane/spatial_adapter.rs"]
mod adapter;
#[path = "../src/spatial.rs"]
mod spatial;
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_engine_render::scene_batch;

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
struct Calls {
    memory: Memory,
    leaves: Vec<Vec<u16>>,
    stop: u16,
}
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        // GuestCall models CALL by supplying the return word to the callee.
        r.esp -= 4;
        self.memory.write32(r.esp, 0x1234);
        if va == 0x1571c0 {
            scene_batch::game_spatial_tree_collect_leaf_items_overlapping_box_in_compact_u16_tree(
                &mut self.memory.clone(),
                self,
                r,
            );
        } else if va == 0x47560 {
            tsfp_ports::c2r::t2049_exec_001::game_box_overlap_vec3_min_max(&mut self.memory, r);
        } else {
            assert_eq!(va, 0x100);
            assert_eq!(self.memory.read32(r.esp + 4), 0x3000);
            assert_eq!(self.memory.read32(r.esp + 8), 0x300c);
            assert_eq!(self.memory.read32(r.esp + 12), 0x9090);
            let count = self.memory.read32(r.esp + 16);
            let list = self.memory.read32(r.esp + 20);
            let items: Vec<_> = (0..count)
                .map(|i| self.memory.read16(list + i * 2))
                .collect();
            r.eax = if items.contains(&self.stop) { 1 } else { 2 };
            self.leaves.push(items);
        }
        r.esp += 4;
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 99,
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
fn four_thousand_fabricated_trees_match_translation_callback_order_and_result() {
    let mut seed = 0x2238_u32;
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };
    for trial in 0..4000 {
        let mut memory = Memory::default();
        memory.write32(0x2000, 0x4000);
        memory.write32(0x2004, 0x5000);
        memory.write32(0x47dd70, (1_f32 / 16383.).to_bits());
        for axis in 0..3 {
            memory.write32(0x2008 + axis * 4, 0_f32.to_bits());
            memory.write32(0x2014 + axis * 4, 100_f32.to_bits());
            let lo = (random() % 160) as f32 - 30.;
            let hi = lo + (random() % 60) as f32;
            memory.write32(0x3000 + axis * 4, lo.to_bits());
            memory.write32(0x300c + axis * 4, hi.to_bits());
        }
        for node in 1..8 {
            let value = (((random() % 3) << 14) | (random() & 0x3fff)) as u16;
            memory.write8(0x4000 + node * 2, value as u8);
            memory.write8(0x4001 + node * 2, (value >> 8) as u8);
        }
        for node in 8..16 {
            let offset = (node - 8) * 4;
            let value = (0xc000 | offset) as u16;
            memory.write8(0x4000 + node * 2, value as u8);
            memory.write8(0x4001 + node * 2, (value >> 8) as u8);
            // Each leaf owns two synthetic IDs; no original bytes in fixtures.
            memory.write8(0x5000 + offset * 2, 2);
            memory.write8(0x5002 + offset * 2, node as u8);
            memory.write8(0x5004 + offset * 2, (node + 20) as u8);
        }
        let mut original = regs();
        for (i, value) in [0x2000, 0x3000, 0x300c, 0x100, 0x9090].iter().enumerate() {
            memory.write32(original.esp + 4 + i as u32 * 4, *value);
        }
        let mut native = original;
        let mut reference_calls = Calls {
            memory: memory.clone(),
            leaves: vec![],
            stop: (trial % 16) as u16,
        };
        scene_batch::game_spatial_tree_query_when_box_overlaps_node_bounds(
            &mut memory.clone(),
            &mut reference_calls,
            &mut original,
        );
        let mut native_calls = Calls {
            memory: memory.clone(),
            leaves: vec![],
            stop: reference_calls.stop,
        };
        assert!(adapter::run(
            0x157440,
            &mut memory,
            &mut native_calls,
            &mut native
        ));
        assert_eq!(native_calls.leaves, reference_calls.leaves, "trial {trial}");
        assert_eq!(native.eax, original.eax, "trial {trial}");
        assert_eq!(
            (native.esp, native.ebx, native.ebp, native.esi, native.edi),
            (
                original.esp,
                original.ebx,
                original.ebp,
                original.esi,
                original.edi
            )
        );
    }
}

#[test]
fn four_thousand_plane_predicates_match_translation_including_nan() {
    let mut seed = 0x8bdd0_u32;
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };
    for trial in 0..4000 {
        let mut memory = Memory::default();
        let mut original = regs();
        original.eax = 0x3000;
        original.ebx = trial % 9;
        let planes = trial % 6;
        memory.write32(original.esp + 4, 0x2000);
        memory.write32(0x2000, 0x4000);
        let count = (planes as i16 - 1).to_le_bytes();
        memory.write8(0x4010, count[0]);
        memory.write8(0x4011, count[1]);
        for i in 0..original.ebx * 3 {
            memory.write32(
                0x3000 + i * 4,
                if trial % 41 == 0 {
                    f32::NAN.to_bits()
                } else {
                    ((random() % 400) as f32 - 200.).to_bits()
                },
            );
        }
        for i in 0..planes * 4 {
            memory.write32(
                0x2008 + i * 4,
                if trial % 101 == 0 {
                    f32::NAN.to_bits()
                } else {
                    ((random() % 40) as f32 / 10. - 2.).to_bits()
                },
            );
        }
        let mut native = original;
        scene_batch::game_level_room_points_all_inside_polygon_planes_test(
            &mut memory,
            &mut original,
        );
        let mut calls = Calls {
            memory: memory.clone(),
            leaves: vec![],
            stop: 0,
        };
        assert!(adapter::run(0x8bdd0, &mut memory, &mut calls, &mut native));
        assert_eq!(native.eax, original.eax, "trial {trial}");
        assert!(calls.leaves.is_empty());
    }
}
