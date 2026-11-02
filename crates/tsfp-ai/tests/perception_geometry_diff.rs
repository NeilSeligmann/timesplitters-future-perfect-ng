// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 lifted differential for the measured angular body at `0x161730`.
//! Delegate internals are stubbed at the documented opaque direction boundary.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::{
    navigation_scoring::ActorId,
    perception_geometry::{
        ActorGeometry, AngularAdmissionQuery, DirectionProvider, MatrixDirectionRoute, Vec2, Vec3,
        angular_admitted,
    },
};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::ai_behavior::sub_00161730,
    memory::GuestMemory,
};

const ACTOR: u32 = 0x1000;
const EXTENSION: u32 = 0x2000;
const TARGET: u32 = 0x4000;
const STACK: u32 = 0x70000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put16(&mut self, address: u32, value: u16) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.0.borrow_mut().insert(address + offset as u32, byte);
        }
    }
    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.0.borrow_mut().insert(address + offset as u32, byte);
        }
    }
    fn put_f32(&mut self, address: u32, value: f32) {
        self.put32(address, value.to_bits());
    }
    fn read32(&self, address: u32) -> u32 {
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
        self.0.borrow_mut().insert(address, value);
    }
}

// The lifted callback needs to write guest memory, so its stub shares the sparse map.
struct MemoryDirection {
    memory: Memory,
    vector: Vec3,
}

impl GuestCall for MemoryDirection {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        match address {
            0x0d6ea0 => regs.eax = 1,
            0x3797b0 => {
                let output = self.memory.read32(regs.esp.wrapping_add(4));
                self.memory.put_f32(output, self.vector.x);
                self.memory.put_f32(output.wrapping_add(4), self.vector.y);
                self.memory.put_f32(output.wrapping_add(8), self.vector.z);
            }
            other => panic!("unexpected lifted direction call {other:#x}"),
        }
    }
}

struct RustDirection(Vec3);

impl DirectionProvider for RustDirection {
    fn has_delegated_direction(&mut self, selector: i16) -> bool {
        selector != 0
    }

    fn delegated_direction(&mut self, _: ActorId, _: i16) -> Vec3 {
        self.0
    }
    fn matrix_direction_xz(
        &mut self,
        _: ActorId,
        _: MatrixDirectionRoute,
        _: Vec3,
        _: Option<f32>,
    ) -> Vec2 {
        unreachable!("Tier 1 fixture selects the opaque vector delegate")
    }
    fn yaw_table_value(&mut self, _: u16) -> f32 {
        unreachable!("Tier 1 fixture selects the opaque vector delegate")
    }
}

fn edge_value(seed: u32) -> f32 {
    const EDGES: [f32; 12] = [
        0.0,
        -0.0,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        -f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        1.0,
        -1.0,
        1.0e-20,
    ];
    if seed % 3 == 0 {
        EDGES[((seed / 3) as usize) % EDGES.len()]
    } else {
        f32::from_bits(seed.wrapping_mul(0x9e37_79b9).rotate_left(11))
    }
}

fn compare_case(seed: u32, control: u16) {
    let actor_position = Vec3 {
        x: edge_value(seed.wrapping_add(3)),
        y: edge_value(seed.wrapping_add(7)),
        z: edge_value(seed.wrapping_add(11)),
    };
    let target = Vec3 {
        x: edge_value(seed.wrapping_add(13)),
        y: edge_value(seed.wrapping_add(17)),
        z: edge_value(seed.wrapping_add(19)),
    };
    let direction = Vec3 {
        x: edge_value(seed.wrapping_add(23)),
        y: edge_value(seed.wrapping_add(29)),
        z: edge_value(seed.wrapping_add(31)),
    };
    let extent = edge_value(seed.wrapping_add(37));
    let threshold = edge_value(seed.wrapping_add(41));
    let mut memory = Memory::default();
    memory.put32(STACK + 4, ACTOR);
    memory.put32(STACK + 8, TARGET);
    memory.put_f32(STACK + 12, threshold);
    memory.put32(ACTOR + 0x7c, EXTENSION);
    memory.put16(EXTENSION + 0x48, 0);
    memory.put16(EXTENSION + 0x4a, 1);
    memory.put_f32(0x53a3e4, extent);
    memory.put_f32(ACTOR + 0x64, actor_position.x);
    memory.put_f32(ACTOR + 0x68, actor_position.y);
    memory.put_f32(ACTOR + 0x6c, actor_position.z);
    memory.put16(ACTOR + 0x94, 0);
    memory.put_f32(TARGET, target.x);
    memory.put_f32(TARGET + 4, target.y);
    memory.put_f32(TARGET + 8, target.z);
    memory.put_f32(0x475cac, 0.0);
    memory.put_f32(0x475c78, 1.0);

    let mut regs = ExtRegisters {
        eax: 0,
        ecx: 0x1234_5678,
        edx: 0x8765_4321,
        esp: STACK,
        ebx: 0x1020_3040,
        ebp: 0x5060_7080,
        esi: 0x90a0_b0c0,
        edi: 0xd0e0_f000,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    let mut lifted_direction = MemoryDirection {
        memory: memory.clone(),
        vector: direction,
    };
    sub_00161730(&mut memory, &mut lifted_direction, &mut regs);

    let actor = ActorGeometry {
        id: ActorId(1),
        position: actor_position,
        yaw: 0.0,
        kind: 0,
        flags: 0,
        direction_selector: 1,
        vertical_extent: extent,
    };
    let rust = angular_admitted(
        AngularAdmissionQuery {
            actor,
            target,
            threshold,
            x87_control: control,
        },
        &mut RustDirection(direction),
    );
    assert_eq!(
        u32::from(rust),
        regs.eax,
        "seed={seed} x87_control={control:#x}"
    );
}

#[test]
fn tier1_4096_seeded_geometry_cases_match_lifted_original_with_float_edges() {
    for seed in 0..2048 {
        compare_case(seed, 0);
        compare_case(seed, 0x300);
    }
}
