// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential for the mask-removal target-alert branch in 0x267FF0.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::awareness::{
    AlertNodeIndex, AlertRecipientEffects, AlertRecipientState, GroupMask,
    apply_group_alert_to_recipient,
};
use tsfp_ai::navigation_scoring::ActorId;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::ai_behavior::game_char_alert_group_members_set_target_and_wake_by_group_mask,
    memory::GuestMemory,
    state_wire,
};

const ACTOR: u32 = 0x1000;
const EXTENSION: u32 = 0x2000;
const RECORD: u32 = 0x3000;
const TARGET: u32 = 0x3500;
const GROUP_ZERO: u32 = 0x7a2d80;
const STACK: u32 = 0x70000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Notification {
    actor: u32,
    removed: u32,
}

struct AlertCalls(Memory, Vec<Notification>);
impl GuestCall for AlertCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        match address {
            0x160cf0 => self.1.push(Notification {
                actor: self.0.get32(regs.esp),
                removed: self.0.get32(regs.esp + 4),
            }),
            _ => panic!("unexpected lifted group-alert delegate {address:#x}"),
        }
    }
}

#[derive(Default)]
struct NativeEffects(Vec<Notification>);
impl AlertRecipientEffects for NativeEffects {
    fn notify_removed_group_mask(&mut self, actor: ActorId, removed: GroupMask) {
        self.0.push(Notification {
            actor: actor.0,
            removed: removed.0,
        });
    }
}

fn compare_seed(seed: u32) {
    let group_mask = 1u32;
    let old_mask = seed.rotate_left(7) | group_mask;
    let actor_id = 0x40 + seed;
    let mut memory = Memory::default();
    memory.put32(ACTOR + 0x7c, EXTENSION);
    memory.put32(EXTENSION, actor_id);
    memory.put32(EXTENSION + 8, actor_id);
    memory.put32(EXTENSION + 4, TARGET);
    memory.put32(EXTENSION + 0x10, 0x0100_0000);
    memory.put32(EXTENSION + 0x14, 0);
    memory.put32(EXTENSION + 0x28, seed & !0x400);
    memory.put32(EXTENSION + 0x2c, 0);
    memory.put32(EXTENSION + 0xac8, 0);
    memory.put32(EXTENSION + 0xbec, old_mask);
    memory.put32(TARGET + 0x110, 777);
    memory.put32(RECORD + 4, group_mask);
    memory.put32(RECORD + 0x10, 0);
    memory.put32(GROUP_ZERO, 1);
    memory.put32(GROUP_ZERO + 0x10, ACTOR);
    memory.put32(STACK + 4, 1);
    memory.put32(STACK + 8, RECORD);
    memory.put32(STACK + 12, 0);
    state_wire::set_mode(&mut memory, 10);
    state_wire::set_flags(&mut memory, 0);

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
    let mut original_calls = AlertCalls(calls_memory, Vec::new());
    game_char_alert_group_members_set_target_and_wake_by_group_mask(
        &mut memory,
        &mut original_calls,
        &mut regs,
    );

    let mut native = AlertRecipientState {
        actor: ActorId(actor_id),
        has_target_alert_behavior: true,
        flags: seed & !0x400,
        alert_mask: GroupMask(old_mask),
        node: AlertNodeIndex(0),
    };
    let mut native_effects = NativeEffects::default();
    apply_group_alert_to_recipient(
        &mut native,
        GroupMask(group_mask),
        AlertNodeIndex(0),
        Some(AlertNodeIndex(777)),
        &mut native_effects,
    );
    assert_eq!(
        memory.get32(EXTENSION + 0x28),
        native.flags,
        "flags seed={seed}"
    );
    assert_eq!(
        memory.get32(EXTENSION + 0xac8) as i32,
        native.node.0,
        "node seed={seed}"
    );
    assert_eq!(
        memory.get32(EXTENSION + 0xbec),
        native.alert_mask.0,
        "mask seed={seed}"
    );
    assert_eq!(
        original_calls.1, native_effects.0,
        "notifications seed={seed}"
    );
}

#[test]
fn tier1_2048_seeded_mask_removal_alerts_match_lifted_original() {
    for seed in 0..2048 {
        compare_seed(seed);
    }
}
