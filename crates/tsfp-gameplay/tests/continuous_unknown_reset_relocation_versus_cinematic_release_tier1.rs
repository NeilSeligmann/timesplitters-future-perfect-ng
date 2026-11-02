// SPDX-License-Identifier: GPL-3.0-or-later
//! T3415 Tier 1 differential against the retained lifted callback bodies.
//! WEAK: all memory, spawn rows and callback arguments are seeded fabricated state. The guest
//! callees 0x9CC40 and 0x25C440 are scripted to observe the callback contract; this does not
//! verify those callees or claim replay adoption.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tsfp_gameplay::story_events::reset_relocation::{
    CameraContext, PlacementFlags, PlayerPlacement, PlayerSlot, PlayerSlotState, ResetDisposition,
    SpawnKey, SpawnPlacement, SpawnTable, StoryResetEvent, StoryResetState, reset_first_player,
    reset_second_player,
};
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::c2r::data_missions_2;
use tsfp_ports::memory::GuestMemory;

#[derive(Clone, Default)]
struct SharedMemory(Arc<Mutex<BTreeMap<u32, u8>>>);

impl GuestMemory for SharedMemory {
    fn read8(&self, address: u32) -> u8 {
        self.0.lock().unwrap().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.lock().unwrap().insert(address, value);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Lookup {
        key: u32,
    },
    Relocate {
        player: u32,
        spawn_id: u32,
        flags: u32,
        trailing: [u32; 2],
    },
}

#[derive(Default)]
struct Guest {
    memory: SharedMemory,
    spawn_id: u32,
    calls: Vec<Call>,
}

impl GuestCall for Guest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        // The bridge omits a hardware CALL instruction; model its return slot and RET here.
        regs.esp = regs.esp.wrapping_sub(4);
        match va {
            0x9cc40 => {
                let key = self.memory.read32(regs.esp.wrapping_add(4));
                self.calls.push(Call::Lookup { key });
                regs.eax = self.spawn_id;
            }
            0x25c440 => {
                self.calls.push(Call::Relocate {
                    player: self.memory.read32(regs.esp.wrapping_add(4)),
                    spawn_id: self.memory.read32(regs.esp.wrapping_add(8)),
                    flags: self.memory.read32(regs.esp.wrapping_add(12)),
                    trailing: [
                        self.memory.read32(regs.esp.wrapping_add(16)),
                        self.memory.read32(regs.esp.wrapping_add(20)),
                    ],
                });
            }
            other => panic!("unexpected lifted callee {other:#x}"),
        }
        regs.esp = regs.esp.wrapping_add(4);
    }
}

fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
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

fn seed_lifted(event: StoryResetEvent, spawn_id: u32, player_count: u32) -> (Guest, ExtRegisters) {
    const STACK: u32 = 0x0080_0000;
    const EVENT: u32 = 0x0030_0000;
    const RECORD: u32 = 0x0031_0000;
    const PLAYERS: u32 = 0x0020_0000;
    const PLAYER_TWO: u32 = 0x0021_0000;
    const EXTENSION_TWO: u32 = 0x0022_0000;
    let mut memory = SharedMemory::default();
    memory.write32(STACK + 4, EVENT);
    memory.write32(EVENT, RECORD);
    memory.write32(RECORD + 0xc, event.spawn_key.0);
    memory.write32(
        RECORD + 0xb0,
        if event.placement_flags == PlacementFlags::PRESERVE_INPUT_DRIVEN {
            0x800000
        } else {
            0
        },
    );
    memory.write32(0x7b0c48, PLAYERS);
    memory.write32(0x790950, player_count);
    memory.write32(PLAYERS + 0x1598, PLAYER_TWO);
    memory.write32(PLAYER_TWO + 0x7c, EXTENSION_TWO);
    for offset in [0xfa0, 0xfa4, 0xfa8, 0x2524, 0x2528, 0x252c] {
        memory.write32(PLAYERS + offset, 0x3f800000);
    }
    let guest = Guest {
        memory: memory.clone(),
        spawn_id,
        calls: Vec::new(),
    };
    let mut regs = regs();
    regs.esp = STACK;
    (guest, regs)
}

fn lifted(event: StoryResetEvent, spawn_id: u32, player_count: u32) -> (Vec<Call>, [u32; 3]) {
    let (mut guest, mut regs) = seed_lifted(event, spawn_id, player_count);
    match event.player_slot {
        PlayerSlot::First => {
            data_missions_2::game_event_action_reset_input_driven_object_for_first_player(
                &mut guest.memory.clone(),
                &mut guest,
                &mut regs,
            )
        }
        PlayerSlot::Second => {
            data_missions_2::game_event_action_reset_input_driven_object_for_second_player(
                &mut guest.memory.clone(),
                &mut guest,
                &mut regs,
            )
        }
    }
    let base = guest.memory.read32(0x7b0c48);
    let first = if event.player_slot == PlayerSlot::First {
        0xfa0
    } else {
        0x2524
    };
    let vectors = core::array::from_fn(|i| guest.memory.read32(base + first + i as u32 * 4));
    (guest.calls, vectors)
}

fn fabricated_state(event: StoryResetEvent, spawn_id: u32) -> StoryResetState {
    let mut state = StoryResetState {
        local_player_count: 2,
        slots: [
            Some(PlayerSlotState {
                object_present: true,
                extension_present: true,
                placement: PlayerPlacement::default(),
                camera: CameraContext {
                    reset_vectors: [1.25, -2.0, 3.5],
                },
            }),
            Some(PlayerSlotState {
                object_present: true,
                extension_present: true,
                placement: PlayerPlacement::default(),
                camera: CameraContext {
                    reset_vectors: [-4.0, 5.5, 6.25],
                },
            }),
        ],
        spawns: SpawnTable::default(),
    };
    state.spawns.insert(
        event.spawn_key,
        SpawnPlacement {
            spawn_id,
            position: [12.0, -3.0, 40.0],
            cell: 0x142,
            orientation: 1.5,
        },
    );
    state
}

#[test]
fn tier1_lifted_callback_differential_3000_fabricated_seeds_weak() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..3000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let player_slot = if seed & 1 == 0 {
            PlayerSlot::First
        } else {
            PlayerSlot::Second
        };
        let placement_flags = if seed & 2 == 0 {
            PlacementFlags::NORMAL
        } else {
            PlacementFlags::PRESERVE_INPUT_DRIVEN
        };
        let spawn_key = SpawnKey(match player_slot {
            PlayerSlot::First => 1045,
            PlayerSlot::Second => 1502,
        });
        let spawn_id = (seed as u32).wrapping_add(0x3000);
        let event = StoryResetEvent {
            spawn_key,
            player_slot,
            placement_flags,
        };
        let mut state = fabricated_state(event, spawn_id);
        let disposition = match player_slot {
            PlayerSlot::First => reset_first_player(event, &mut state),
            PlayerSlot::Second => reset_second_player(event, &mut state),
        };
        let (lifted_calls, lifted_vectors) = lifted(event, spawn_id, 2);
        assert_eq!(disposition, Some(ResetDisposition::Applied));
        assert_eq!(
            lifted_calls,
            vec![
                Call::Lookup { key: spawn_key.0 },
                Call::Relocate {
                    player: player_slot.index() as u32,
                    spawn_id,
                    flags: placement_flags.0,
                    trailing: [0, 0],
                },
            ],
            "callback ABI seed={seed:#018x}"
        );
        assert_eq!(lifted_vectors, [0; 3], "callback vector seed={seed:#018x}");
        let rust_slot = state.slots[player_slot.index()].unwrap();
        assert_eq!(rust_slot.placement.spawn_key, spawn_key);
        assert_eq!(rust_slot.placement.spawn_id, spawn_id);
        assert_eq!(rust_slot.placement.flags, placement_flags);
        assert_eq!(rust_slot.camera.reset_vectors, [0.0; 3]);
    }
}
