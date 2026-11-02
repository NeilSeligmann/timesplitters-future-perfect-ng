// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential for the readable navigation scorer against lifted 0x261DE0.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::navigation_scoring::{
    ActorId, CandidateGroup, NavNode, NavPosition, NodeId, ObjectId, OtherActorGoal, ScoringActor,
    ScoringWorld, select_best_goal,
};
use tsfp_ports::abi::GuestCall;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, Xmm},
    c2r::ai_pathfinding::game_ai_select_best_nav_node_from_candidate_lists_by_distance_score,
    memory::GuestMemory,
};

const ACTOR_OBJECT: u32 = 0x1000;
const ACTOR_EXTENSION: u32 = 0x2000;
const TARGET_OBJECT: u32 = 0x3000;
const CONFIG: u32 = 0x4000;
const GROUPS: u32 = 0x5000;
const NODE_RECORDS: u32 = 0x10_000;
const OTHER_EXTENSIONS: u32 = 0x20_000;
const STACK: u32 = 0x70_000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }

    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address.wrapping_add(offset as u32), byte);
        }
    }

    fn put_f32(&mut self, address: u32, value: f32) {
        self.put32(address, value.to_bits());
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.put8(address, value);
    }

    fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|offset| {
            self.read8(address.wrapping_add(offset as u32))
        }))
    }

    fn write32(&mut self, address: u32, value: u32) {
        self.put32(address, value);
    }
}

struct WorldCalls(Memory, u32);

impl GuestCall for WorldCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        // RegsExt::call dispatches directly without pushing an x86 return slot.
        // The lifted scorer owns its explicit stack adjustments, so these
        // delegates model return values and writes while leaving ESP untouched.
        match address {
            0x9c550 => {
                // `RegsExt::call` exposes the caller's pre-CALL stack. The
                // first pushed argument is at ESP; its output argument points
                // at the caller's local outList slot.
                let group = self.0.read32(regs.esp);
                let output = self.0.read32(regs.esp + 4);
                let group_table_slot = 0x7b6770u32
                    .checked_add(group.checked_mul(4).unwrap_or_else(|| {
                        panic!("seed {} passed invalid group ID {group:#x}", self.1)
                    }))
                    .expect("valid group table slot");
                let group_ptr = self.0.read32(group_table_slot);
                let count = self.0.read32(group_ptr);
                self.0.write32(output, group_ptr + 24);
                regs.eax = count;
            }
            0x9a420 => {
                let node_id = self.0.read32(regs.esp);
                regs.eax = NODE_RECORDS + node_id * 0x20;
            }
            _ => panic!("unexpected lifted scorer delegate {address:#x}"),
        }
    }
}

struct Seed(u32);

impl Seed {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }

    fn index(&mut self, len: usize) -> usize {
        (self.next() as usize) % len
    }

    fn position(&mut self) -> NavPosition {
        const GRID: [f32; 11] = [
            -30.0, -20.0, -10.0, -9.0, -5.0, 0.0, 5.0, 9.0, 10.0, 20.0, 30.0,
        ];
        let jitter = (self.next() & 3) as f32 * 0.25;
        NavPosition {
            x: GRID[self.index(GRID.len())] + jitter,
            z: GRID[self.index(GRID.len())] - jitter,
        }
    }
}

fn compare_one(seed_value: u32) {
    let mut seed = Seed(seed_value);
    let nodes: Vec<_> = (1..=12)
        .map(|id| NavNode {
            id: NodeId(id),
            position: seed.position(),
        })
        .collect();
    let candidate_ids: Vec<_> = (1..=12).map(NodeId).collect();
    let groups: Vec<_> = candidate_ids
        .chunks(3)
        .map(|ids| CandidateGroup { node_ids: ids })
        .collect();
    let current_node = NodeId((seed.index(12) + 1) as u32);
    let actor_position = seed.position();
    let target_position = seed.position();
    let has_target = seed.next() % 7 != 0;
    let other_actors: Vec<_> = (0..seed.index(6))
        .map(|i| OtherActorGoal {
            extension: ActorId(OTHER_EXTENSIONS + i as u32 * 0x100),
            object: ObjectId(if i == 0 && seed.next() & 1 == 0 {
                TARGET_OBJECT
            } else {
                0x30_000 + i as u32 * 0x100
            }),
            flags: (seed.next() & 0x0f) as u8,
            goal: (seed.next() & 3 != 0).then(|| NodeId((seed.index(12) + 1) as u32)),
        })
        .collect();
    let actor = ScoringActor {
        extension: ActorId(ACTOR_EXTENSION),
        object: ObjectId(ACTOR_OBJECT),
        target_object: has_target.then_some(ObjectId(TARGET_OBJECT)),
        current_node,
        position: actor_position,
        target_position: has_target.then_some(target_position),
    };

    let mut memory = Memory::default();
    memory.put32(ACTOR_OBJECT + 0x7c, ACTOR_EXTENSION);
    memory.put32(
        ACTOR_EXTENSION + 4,
        if has_target { TARGET_OBJECT } else { 0 },
    );
    memory.put32(ACTOR_EXTENSION + 0x678, current_node.0);
    memory.put_f32(ACTOR_OBJECT + 0x64, actor_position.x);
    memory.put_f32(ACTOR_OBJECT + 0x6c, actor_position.z);
    memory.put_f32(TARGET_OBJECT + 0x64, target_position.x);
    memory.put_f32(TARGET_OBJECT + 0x6c, target_position.z);
    memory.put32(0x478618, (-f32::MAX).to_bits());
    memory.put32(0x475d5c, f32::MAX.to_bits());
    memory.put_f32(0x47d994, 400.0);
    memory.put_f32(0x4785c8, 100.0);
    memory.put_f32(0x475cd4, 0.5);

    for node in &nodes {
        let address = NODE_RECORDS + node.id.0 * 0x20;
        memory.put_f32(address + 0x0c, node.position.x);
        memory.put_f32(address + 0x14, node.position.z);
    }
    for (group_index, group) in groups.iter().enumerate() {
        let group_ptr = GROUPS + group_index as u32 * 0x100;
        memory.put32(0x7b6770 + group_index as u32 * 4, group_ptr);
        memory.put32(group_ptr, group.node_ids.len() as u32);
        for (slot, node_id) in group.node_ids.iter().enumerate() {
            memory.put32(group_ptr + 24 + slot as u32 * 4, node_id.0);
        }
        memory.put32(CONFIG + 0x14 + group_index as u32 * 4, group_index as u32);
    }
    memory.put32(0x74c37c, other_actors.len() as u32);
    for (index, other) in other_actors.iter().enumerate() {
        let extension = other.extension.0;
        let object = other.object.0;
        let pointer_slot = 0x7a3580 + index as u32 * 4;
        memory.put32(pointer_slot, extension);
        memory.put32(extension, object);
        memory.put8(extension + 0x24, other.flags);
        memory.put32(extension + 0x684, other.goal.map_or(u32::MAX, |id| id.0));
    }
    memory.put32(STACK + 4, ACTOR_OBJECT);

    let native = select_best_goal(
        actor,
        ScoringWorld {
            groups: &groups,
            nodes: &nodes,
            other_actors: &other_actors,
        },
    )
    .map_or(u32::MAX, |node| node.0);

    let mut registers = ExtRegisters {
        eax: 0,
        esp: STACK,
        ecx: CONFIG,
        edx: 0,
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
    let mut calls = WorldCalls(memory.clone(), seed_value);
    game_ai_select_best_nav_node_from_candidate_lists_by_distance_score(
        &mut memory,
        &mut calls,
        &mut registers,
    );
    assert_eq!(native, registers.eax, "seed {seed_value}");
}

#[test]
fn tier1_4096_seeded_valid_worlds_match_lifted_original() {
    for seed in 0..4096 {
        compare_one(seed);
    }
}
