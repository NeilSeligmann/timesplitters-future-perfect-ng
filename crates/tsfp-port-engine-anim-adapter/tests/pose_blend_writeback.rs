// SPDX-License-Identifier: GPL-3.0-or-later
//! T3851: a nested matrix helper's destination is authoritative after it returns.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Mutex};
use tsfp_port_engine_anim_adapter::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    anim_lane3, blend,
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

impl Memory {
    fn matrix(&self, at: u32) -> [f32; 16] {
        std::array::from_fn(|i| f32::from_bits(self.read32(at + 4 * i as u32)))
    }
    fn put_matrix(&mut self, at: u32, matrix: [f32; 16]) {
        for (i, value) in matrix.into_iter().enumerate() {
            self.write32(at + 4 * i as u32, value.to_bits());
        }
    }
}

struct MatrixHelper {
    memory: Memory,
    calls: usize,
}

impl GuestCall for MatrixHelper {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        assert_eq!(va, 0x1526c0);
        // GuestCall sees the prepared argument block before the bridge pushes RET.
        let args: [u32; 4] = std::array::from_fn(|i| self.memory.read32(regs.esp + 4 * i as u32));
        let result = blend::matrix_blend(
            &self.memory.matrix(args[0]),
            &self.memory.matrix(args[1]),
            f32::from_bits(args[3]),
        );
        self.memory.put_matrix(args[2], result);
        self.calls += 1;
    }
}

fn translation(x: f32) -> [f32; 16] {
    let mut result = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
    result[12] = x;
    result
}

// The adapter journal models one guest thread. Isolate calls within this binary.
static GUEST_THREAD: Mutex<()> = Mutex::new(());

fn exercise(full: bool) {
    let _guest_thread = GUEST_THREAD
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut memory = Memory::default();
    let (ctx, model, inner, entries) = (0x20000, 0x21000, 0x22000, 0x23000);
    let (base, pose, out, stack) = (0x30000, 0x40000, 0x50000, 0x60000);
    let nodes = 267;
    memory.write32(ctx, model);
    memory.write32(ctx + 4, pose);
    memory.write32(ctx + 8, out);
    memory.write32(model + 0x10, inner);
    memory.write32(model + 0x20, nodes);
    memory.write32(model + 0x2c, base);
    memory.write32(inner + 0xc, entries);
    memory.write32(stack + 4, u32::from(full));
    for node in 0..nodes {
        // Include untouched weight-one entries and bones past both 40 and 256.
        let weight: f32 = if node % 3 == 0 { 1.0 } else { 0.25 };
        memory.write32(entries + 8 * node, node);
        memory.write32(entries + 8 * node + 4, weight.to_bits());
        memory.put_matrix(base + 64 * node, translation(node as f32));
        memory.put_matrix(pose + 64 * node, translation(node as f32 + 8.0));
        memory.put_matrix(out + 64 * node, translation(-123.0));
    }
    let mut helper = MatrixHelper {
        memory: memory.clone(),
        calls: 0,
    };
    let mut regs = ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: stack,
        ebx: 0,
        ebp: 0,
        esi: ctx,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    assert!(anim_lane3::run(
        0x60540,
        &mut memory,
        &mut helper,
        &mut regs
    ));
    assert_eq!(helper.calls, 178);
    for node in 0..nodes {
        let expected = if node % 3 == 0 {
            -123.0
        } else {
            node as f32 + if full { 8.0 } else { 2.0 }
        };
        assert_eq!(
            memory.matrix(out + 64 * node),
            translation(expected),
            "node {node}"
        );
    }
}

#[test]
fn weighted_bones_retain_nested_helper_output() {
    exercise(false);
}

#[test]
fn full_pose_retain_nested_helper_output() {
    exercise(true);
}
