// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 weak-input differential of object render-room assignment.
extern crate alloc;
pub use tsfp_ports::{abi, memory};
mod scene {
    pub use tsfp_engine_render::scene::scene_collect;
}
#[path = "../../tsfp-ports/src/scene_lane/object_room_adapter.rs"]
mod adapter;
use abi::{ExtRegisters, Fpu, Xmm};
use memory::GuestMemory;
use std::collections::BTreeMap;
#[derive(Clone, Default)]
struct Memory(BTreeMap<u32, u8>);
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
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
fn four_thousand_room_assignment_cases_match_translated_original() {
    let mut seed = 0x9ff10u32;
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };
    for trial in 0..4000 {
        let mut m = Memory::default();
        m.write32(0x7b4fac, 4);
        m.write32(0x72f5e8, 0x200000);
        m.write32(0x7b0cc0, 0x100000);
        m.write32(0x7b0ce0, 2);
        m.write32(0x7b0ce4, 0x100000);
        m.write32(0x100214, trial % 3);
        for room in 0..8 {
            m.write32(0x1003bc + room * 12, (random() % 12).wrapping_sub(2));
            for view in 0..2 {
                m.write8(0x100000 + view * 0xffc + 0xd2c + room, (random() % 2) as u8);
            }
        }
        for i in 0..4 {
            let object = 0x200000 + i * 0x2e8;
            m.write32(
                object + 0x28,
                if random() % 5 == 0 { 0xc0000000 } else { 0 },
            );
            m.write8(object + 0x30, (random() % 8) as u8);
            m.write32(object + 0x34, 1234);
            for j in 0..8 {
                m.write32(object + 0x38 + j * 4, random() % 8);
            }
        }
        let mut native = m.clone();
        let mut original = regs();
        let mut candidate = original;
        tsfp_ports::c2r::objchar_wired2::game_object_pool_72f5e8_select_field_0x34_from_candidate_list_0x38_by_min_per_player_value(&mut m,&mut original);
        assert!(adapter::run(0x9ff10, &mut native, &mut candidate));
        for i in 0..4 {
            assert_eq!(
                native.read32(0x200000 + i * 0x2e8 + 0x34),
                m.read32(0x200000 + i * 0x2e8 + 0x34),
                "trial {trial},object {i}"
            );
        }
    }
}
