// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential for T1662's post-callback mode-10 claim cleanup.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::awareness::{
    NavigationClaimState, NodeKey, PrimaryTargetId, SecondaryTargetClaim, TargetHandle,
};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::t1926_characters::game_actor_apply_pending_flag_pair_by_message_dispatch,
    memory::GuestMemory,
    state_wire,
};

const ACTOR: u32 = 0x1000;
const EXTENSION: u32 = 0x2000;
const CURRENT_SECONDARY: u32 = 0x3000;
const SAVED_SECONDARY: u32 = 0x4000;
const CURRENT_PRIMARY: u32 = 0x5000;
const SAVED_PRIMARY: u32 = 0x6000;
const STACK: u32 = 0x70000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
    fn put16(&mut self, address: u32, value: u16) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address + offset as u32, byte);
        }
    }
    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address + offset as u32, byte);
        }
    }
    fn get32(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| {
            self.0
                .borrow()
                .get(&(address + i as u32))
                .copied()
                .unwrap_or(0)
        }))
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.put8(address, value);
    }
}

struct CleanupCalls(Memory, u16, u16);
impl GuestCall for CleanupCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        match address {
            0x15c310 => regs.eax = 0,
            0x9cca0 => {
                let key = self.0.get32(regs.esp) as u16;
                regs.eax = if key == self.1 {
                    CURRENT_PRIMARY
                } else if key == self.2 {
                    SAVED_PRIMARY
                } else {
                    panic!("unexpected primary key {key}");
                };
            }
            _ => panic!("unexpected lifted claim-cleanup delegate {address:#x}"),
        }
    }
}

fn target(object_pointer: u32, key: u16, primary: u32) -> SecondaryTargetClaim {
    SecondaryTargetClaim {
        handle: TargetHandle {
            object: tsfp_ai::navigation_scoring::ObjectId(object_pointer),
            generation: 1,
        },
        own_key: NodeKey(key as i32 + 9000),
        primary: PrimaryTargetId(primary),
    }
}

fn compare_seed(seed: u32) {
    const OLD_MASKS: [u32; 7] = [0, 0x10000, 0x20000, 0x40000, 0x60000, 0x70000, 0x80000];
    const NEW_MASKS: [u32; 7] = [0, 4, 0x20, 0x60, 0x10000, 0x40000, 0x4000000];
    let old_mask = OLD_MASKS[(seed as usize) % OLD_MASKS.len()];
    let new_mask = NEW_MASKS[((seed / OLD_MASKS.len() as u32) as usize) % NEW_MASKS.len()];
    let current_key = 100 + (seed % 1000) as u16;
    let saved_key = current_key + 1000;

    let mut memory = Memory::default();
    memory.put32(ACTOR + 0x7c, EXTENSION);
    memory.put32(EXTENSION + 0x10, new_mask);
    memory.put32(EXTENSION + 0x14, 0);
    memory.put32(EXTENSION + 0x18, new_mask);
    memory.put32(EXTENSION + 0x1c, 0);
    memory.put32(EXTENSION + 0xab0, old_mask);
    memory.put32(EXTENSION + 0xab4, 0);
    memory.put32(EXTENSION + 0x9c0, CURRENT_SECONDARY);
    memory.put32(EXTENSION + 0x9c4, SAVED_SECONDARY);
    memory.put16(CURRENT_SECONDARY + 8, current_key);
    memory.put16(SAVED_SECONDARY + 8, saved_key);
    memory.put32(CURRENT_PRIMARY + 4, 0xc000_0000);
    memory.put32(SAVED_PRIMARY + 4, 0xc000_0000);
    memory.put32(STACK + 4, ACTOR);
    state_wire::set_mode(&mut memory, 10);

    let mut regs = ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    let calls_memory = memory.clone();
    game_actor_apply_pending_flag_pair_by_message_dispatch(
        &mut memory,
        &mut CleanupCalls(calls_memory, current_key, saved_key),
        &mut regs,
    );

    let current = target(CURRENT_SECONDARY, current_key, 1);
    let saved = target(SAVED_SECONDARY, saved_key, 2);
    let mut native = NavigationClaimState::default();
    native.acquire(saved);
    native.save_current();
    native.acquire(current);
    native.mode10_cleanup(old_mask, new_mask);

    assert_eq!(
        memory.get32(EXTENSION + 0x9c0),
        native.current.map_or(0, |v| v.handle.object.0)
    );
    assert_eq!(
        memory.get32(EXTENSION + 0x9c4),
        native.saved.map_or(0, |v| v.handle.object.0)
    );
    assert_eq!(
        memory.get32(CURRENT_PRIMARY + 4) & 0x4000_0000 != 0,
        native.primary_is_claimed(current.primary)
    );
    assert_eq!(
        memory.get32(SAVED_PRIMARY + 4) & 0x4000_0000 != 0,
        native.primary_is_claimed(saved.primary)
    );
}

#[test]
fn tier1_seeded_mode10_claim_cleanup_matches_lifted_original() {
    for seed in 0..2048 {
        compare_seed(seed);
    }
}
