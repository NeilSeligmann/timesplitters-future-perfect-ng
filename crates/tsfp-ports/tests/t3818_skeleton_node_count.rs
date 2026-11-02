// SPDX-License-Identifier: GPL-3.0-or-later
//! T3818: the pose ports evaluate every node of a skeleton, not the first 40.
//!
//! Cutscene and character models have 44 to 267 nodes (MEASURED in the Tier 2 corpus). The posemix adapters used a fixed
//! 40-matrix buffer and clamped the model's node count to it, so nodes 40 and up kept stale matrices and skinned meshes
//! rendered broken (owner playtest 2026-10-26). The tests use a 267-node model (the largest in the corpus) and check a node past the old cap.

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::anim_lane3;
use tsfp_ports::memory::GuestMemory;

const BASE: u32 = 0x1_0000;
const SIZE: usize = 0x80_0000; // up to the clip table at 0x7E3954
const STACK: u32 = 0x3f_0000;
const CONTEXT: u32 = 0x2_0000;
const MODEL: u32 = 0x2_1000;
const INNER: u32 = 0x2_2000;
const ENTRIES: u32 = 0x2_3000;
const TREE: u32 = 0x2_4000;
const BASE_POSE: u32 = 0x3_0000;
const POSE: u32 = 0x4_0000;
const OUT: u32 = 0x5_0000;
/// The largest skeleton in the Tier 2 corpus (tests/test_anim_skeleton_buffers.py CORPUS_MAX_NODES).
const NODES: u32 = 267;
/// A node past the old 40-matrix cap.
const LATE: u32 = 50;

struct Fake(Vec<u8>);

impl Fake {
    fn new() -> Self {
        Self(vec![0; SIZE])
    }
    fn index(address: u32) -> Option<usize> {
        let index = address.checked_sub(BASE)? as usize;
        (index < SIZE).then_some(index)
    }
    fn write_matrix(&mut self, address: u32, matrix: &[f32; 16]) {
        for (i, value) in matrix.iter().enumerate() {
            self.write32(address + 4 * i as u32, value.to_bits());
        }
    }
    fn matrix(&self, address: u32) -> [f32; 16] {
        core::array::from_fn(|i| f32::from_bits(self.read32(address + 4 * i as u32)))
    }
}

impl GuestMemory for Fake {
    fn read8(&self, address: u32) -> u8 {
        Self::index(address).map_or(0, |index| self.0[index])
    }
    fn write8(&mut self, address: u32, value: u8) {
        if let Some(index) = Self::index(address) {
            self.0[index] = value;
        }
    }
}

struct NoCalls;

impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _regs: &mut ExtRegisters) {
        panic!("the pose ports call no guest code, got {va:#x}");
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
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
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

fn translation(x: f32, y: f32, z: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0,
    ]
}

/// A model of `NODES` nodes: a parent chain 0 <- 1 <- ... <- NODES-1 in the part tree table.
fn model(memory: &mut Fake) {
    memory.write32(MODEL + 0x10, INNER);
    memory.write32(MODEL + 0x14, TREE);
    memory.write32(MODEL + 0x20, NODES);
    memory.write32(MODEL + 0x2c, BASE_POSE);
    memory.write32(INNER + 0xc, ENTRIES);
    for node in 0..NODES {
        let row = TREE + 20 * node;
        memory.write32(row + 4, if node == 0 { u32::MAX } else { node - 1 });
        memory.write32(row + 8, if node + 1 < NODES { node + 1 } else { u32::MAX });
        memory.write32(row + 12, u32::MAX);
    }
}

#[test]
fn compose_or_copy_writes_every_node_of_the_largest_model() {
    let mut memory = Fake::new();
    model(&mut memory);
    memory.write32(CONTEXT, MODEL);
    memory.write32(CONTEXT + 4, POSE);
    memory.write32(CONTEXT + 8, OUT);
    for node in 0..NODES {
        // Weight-one entries copy the fully composed pose; blend writeback has
        // its own shared-memory helper regression in the animation adapter crate.
        memory.write32(ENTRIES + 8 * node, node);
        memory.write32(ENTRIES + 8 * node + 4, 1.0f32.to_bits());
        memory.write_matrix(BASE_POSE + 64 * node, &translation(0.0, 0.0, 0.0));
        memory.write_matrix(POSE + 64 * node, &translation(node as f32, 2.0, 3.0));
    }
    memory.write32(STACK + 4, 1);
    let mut regs = registers();
    regs.esi = CONTEXT;
    assert!(anim_lane3::run(
        0x605d0,
        &mut memory,
        &mut NoCalls,
        &mut regs
    ));
    for node in [0, 39, 40, LATE, NODES - 1] {
        let out = memory.matrix(OUT + 64 * node);
        assert_eq!(
            [out[12], out[13], out[14]],
            [node as f32, 2.0, 3.0],
            "node {node} not copied"
        );
    }
}

#[test]
fn part_tree_composes_every_node_of_the_largest_model() {
    let mut memory = Fake::new();
    model(&mut memory);
    memory.write32(CONTEXT, MODEL);
    for node in 0..NODES {
        memory.write_matrix(POSE + 64 * node, &translation(1.0, 0.0, 0.0));
    }
    // 0x604D0(context, matrices, node 0)
    memory.write32(STACK + 4, CONTEXT);
    memory.write32(STACK + 8, POSE);
    memory.write32(STACK + 12, 0);
    let mut regs = registers();
    assert!(anim_lane3::run(
        0x604d0,
        &mut memory,
        &mut NoCalls,
        &mut regs
    ));
    for node in [0, 39, 40, LATE, NODES - 1] {
        let world = memory.matrix(POSE + 64 * node);
        assert_eq!(world[12], (node + 1) as f32, "node {node} not composed");
    }
}

#[test]
fn a_corrupt_node_count_is_left_to_the_original() {
    let mut memory = Fake::new();
    model(&mut memory);
    memory.write32(MODEL + 0x20, 0xcdcd_cdcd);
    memory.write32(CONTEXT, MODEL);
    memory.write32(STACK + 4, CONTEXT);
    memory.write32(STACK + 8, POSE);
    let mut regs = registers();
    let mut guest = RecordingGuest(Vec::new());
    assert!(anim_lane3::run(0x604d0, &mut memory, &mut guest, &mut regs));
    assert_eq!(guest.0, [0x604d0], "the retained original runs");
}

struct RecordingGuest(Vec<u32>);

impl GuestCall for RecordingGuest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.0.push(va);
        regs.eax = 0;
    }
}

const INSTANCE: u32 = 0x6_0000;
const CLIPS: u32 = 0x6_1000;
const CLIP: u32 = 0x6_2000;
const KEY_TABLE: u32 = 0x6_3000;

/// 0x60B30(owner 0, 0, INSTANCE, 0) on a 3-node clip of `duration`, as the cutscene actor update (caller 0x98120) calls it.
fn owner_less_instance(duration: f32) -> (Fake, ExtRegisters) {
    let mut memory = Fake::new();
    memory.write32(STACK + 4, 0);
    memory.write32(STACK + 8, 0);
    memory.write32(STACK + 12, INSTANCE);
    memory.write32(STACK + 16, 0);
    memory.write32(INSTANCE, MODEL);
    memory.write32(INSTANCE + 4, POSE);
    memory.write32(INSTANCE + 0x14, 0);
    memory.write32(INSTANCE + 0x18, 0.25f32.to_bits());
    memory.write32(MODEL + 0x1c, 0);
    memory.write32(MODEL + 0x20, 0);
    memory.write32(0x007e_3954, CLIPS);
    memory.write32(CLIPS, CLIP);
    memory.write32(CLIP + 0x10, 2);
    memory.write32(CLIP + 0x14, 3);
    memory.write32(CLIP + 0x18, duration.to_bits());
    memory.write32(CLIP + 0x1c, KEY_TABLE);
    memory.write32(KEY_TABLE, 0);
    memory.write32(KEY_TABLE + 4, 30);
    (memory, registers())
}

#[test]
fn cutscene_actor_without_owner_is_posed() {
    // MEASURED (retail code under Unicorn, T3818): with a clip duration not above zero the original never reads the owner and
    // evaluates the instance at its own layer time. The port used to require an owner and silently skipped these actors.
    let (mut memory, mut regs) = owner_less_instance(0.0);
    let mut guest = RecordingGuest(Vec::new());
    assert!(anim_lane3::run(0x60b30, &mut memory, &mut guest, &mut regs));
    let samples = guest.0.iter().filter(|&&va| va == 0x59320).count();
    assert_eq!(samples, 6, "three nodes, two keys each: {:x?}", guest.0);
    assert_eq!(&guest.0[guest.0.len() - 3..], &[0x60540, 0x604d0, 0x605d0]);
    assert_eq!(
        memory.read32(INSTANCE + 0x18),
        0.25f32.to_bits(),
        "layer time kept"
    );
}

#[test]
fn an_owner_less_call_that_would_advance_time_is_left_to_the_original() {
    let (mut memory, mut regs) = owner_less_instance(2.0);
    let mut guest = RecordingGuest(Vec::new());
    assert!(anim_lane3::run(0x60b30, &mut memory, &mut guest, &mut regs));
    assert_eq!(guest.0, [0x60b30], "the retained original runs");
}
