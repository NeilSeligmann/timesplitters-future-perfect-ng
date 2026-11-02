// SPDX-License-Identifier: GPL-3.0-or-later
//! T2148 Tier 1 differential: readable pickup policy vs its Tier 1 c2r reference.
//! Inputs are fabricated guest-memory cases; this is not route, xemu, or hardware evidence.

use std::collections::BTreeMap;
use tsfp_gameplay::pickup_eligibility::{PickupTouchContext, is_pickup_touch_eligible};
use tsfp_ports::abi::{ExtRegisters, Fpu, Xmm};
use tsfp_ports::memory::GuestMemory;

const PLAYER: u32 = 0x1000;
const PLAYER_CONTEXT: u32 = 0x2000;
const CAMERA: u32 = 0x3000;
const PICKUP: u32 = 0x4000;
const PICKUP_CONTEXT: u32 = 0x5000;

#[derive(Default)]
struct Memory(BTreeMap<u32, u8>);

impl Memory {
    fn store32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.0.insert(address + offset as u32, byte);
        }
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: PLAYER,
        ecx: 0,
        edx: PICKUP,
        esp: 0,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x7f,
        },
        xmm: [Xmm { lanes: [0; 4] }; 5],
        xmm_hi: [Xmm { lanes: [0; 4] }; 3],
    }
}

fn compare_case(context: PickupTouchContext) {
    let mut memory = Memory::default();
    memory.store32(PLAYER + 0x7c, PLAYER_CONTEXT);
    memory.store32(
        PLAYER + 0x150,
        if context.camera_state.is_some() {
            CAMERA
        } else {
            0
        },
    );
    memory.store32(PLAYER_CONTEXT + 0x44, context.team);
    memory.write8(PLAYER_CONTEXT + 0x24, context.player_flags);
    if let Some(camera_state) = context.camera_state {
        memory.store32(CAMERA + 0x114, camera_state);
    }
    memory.store32(PICKUP + 0x7c, PICKUP_CONTEXT);
    memory.store32(PICKUP_CONTEXT + 0x0c, context.pickup_kind);
    tsfp_ports::state_wire::set_mode(&mut memory, context.mode);

    let expected = is_pickup_touch_eligible(context);
    let mut regs = registers();
    tsfp_ports::c2r::t1925_weapons::game_pickup_touch_eligibility_predicate_camera_state_0x400_flag_0x24_team_2_3(
        &mut memory,
        &mut regs,
    );
    assert_eq!(regs.eax != 0, expected, "context: {context:?}");
}

#[test]
fn readable_policy_matches_c2r_reference_for_fabricated_cases() {
    let camera_states = [None, Some(0), Some(0x400), Some(0x401), Some(u32::MAX)];
    let teams = [0, 1, 2, 3, 4, u32::MAX];
    let pickup_kinds = [0, 0xff, 0x100, 0x101, u32::MAX];
    for camera_state in camera_states {
        for player_flags in [0, 0x08, 0x10, 0xff] {
            for mode in [0, 0x09, 0x0a, 0x0b, 0xff] {
                for team in teams {
                    for pickup_kind in pickup_kinds {
                        compare_case(PickupTouchContext {
                            camera_state,
                            player_flags,
                            mode,
                            team,
                            pickup_kind,
                        });
                    }
                }
            }
        }
    }

    let mut seed = 0x2148_be18u32;
    for _ in 0..4_096 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let camera_state = match seed & 3 {
            0 => None,
            1 => Some(0x400),
            _ => Some(seed.rotate_left(11)),
        };
        let team = match (seed >> 3) % 8 {
            0 => 2,
            1 => 3,
            2 => u32::MAX,
            _ => seed.rotate_right(7),
        };
        let pickup_kind = if seed & 0x1000 != 0 {
            0x100
        } else {
            seed.rotate_left(17)
        };
        compare_case(PickupTouchContext {
            camera_state,
            player_flags: (seed >> 8) as u8,
            mode: (seed >> 16) as u8,
            team,
            pickup_kind,
        });
    }
}
