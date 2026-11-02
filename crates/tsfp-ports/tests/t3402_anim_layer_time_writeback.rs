// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression for the 0x60B30 layer-time writeback when key lookup returns None.

use std::{cell::RefCell, rc::Rc};
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::anim_lane3;
use tsfp_ports::memory::GuestMemory;

const BASE: u32 = 0x1_0000;
const SIZE: usize = 0x80_0000;
const STACK: u32 = 0x7f_0000;
const OWNER: u32 = 0x2_0000;
const INSTANCE: u32 = 0x3_0000;
const MODEL: u32 = 0x4_0000;
const MATRICES: u32 = 0x5_0000;
const CLIPS: u32 = 0x6_0000;
const CLIP: u32 = 0x7_0000;

struct Fake(Vec<u8>);

impl Fake {
    fn index(address: u32) -> Option<usize> {
        assert!(
            (BASE..0x8000_0000).contains(&address),
            "bad guest address {address:#x}"
        );
        let index = (address - BASE) as usize;
        (index < SIZE).then_some(index)
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

struct Guest;

impl GuestCall for Guest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        assert_eq!(va, 0x60920, "empty key list must not invoke the sampler");
        regs.eax = 0;
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

#[test]
fn publishes_advanced_layer_time_when_key_bracket_is_empty() {
    let mut memory = Fake(vec![0; SIZE]);
    let mut regs = registers();
    let mut guest = Guest;
    let owner_time = 17.066_667_f32;

    // 0x60B30(owner, index, instance, save_base)
    memory.write32(STACK + 4, OWNER);
    memory.write32(STACK + 8, 0);
    memory.write32(STACK + 12, INSTANCE);
    memory.write32(STACK + 16, 0);

    memory.write32(OWNER + 8, owner_time.to_bits());
    memory.write32(INSTANCE, MODEL);
    memory.write32(INSTANCE + 4, MATRICES);
    memory.write32(INSTANCE + 8, 0);
    memory.write32(INSTANCE + 0x14, 0);
    memory.write32(INSTANCE + 0x18, 17.0_f32.to_bits());
    memory.write32(MODEL + 0x1c, 0);
    memory.write32(MODEL + 0x20, 0);
    memory.write32(MODEL + 0x2c, 0);

    // A valid clip with positive duration and no key rows makes the typed
    // evaluator advance state.layer_time, then return None at key_bracket.
    memory.write32(0x007e_3954, CLIPS);
    memory.write32(CLIPS, CLIP);
    memory.write32(CLIP + 0x10, 0);
    memory.write32(CLIP + 0x14, 0);
    memory.write32(CLIP + 0x18, 119.86_f32.to_bits());
    memory.write32(CLIP + 0x1c, 0);

    assert!(anim_lane3::run(0x60b30, &mut memory, &mut guest, &mut regs));
    assert_eq!(memory.read32(INSTANCE + 0x18), owner_time.to_bits());
}

struct RecordingGuest(Rc<RefCell<Vec<u32>>>);

impl GuestCall for RecordingGuest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.0.borrow_mut().push(va);
        regs.eax = 0;
    }
}

#[test]
fn instance_bracket_uses_rows_past_the_old_256_key_prefix() {
    const KEY_TABLE: u32 = 0x7_1000;
    const KEY_COUNT: u32 = 7_200;
    let mut memory = Fake(vec![0; SIZE]);
    let mut regs = registers();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut guest = RecordingGuest(calls.clone());
    let target_time = 256.0_f32 / 60.0;

    memory.write32(STACK + 4, OWNER);
    memory.write32(STACK + 8, 0);
    memory.write32(STACK + 12, INSTANCE);
    memory.write32(STACK + 16, 0);
    memory.write32(OWNER + 8, target_time.to_bits());
    memory.write32(INSTANCE, MODEL);
    memory.write32(INSTANCE + 4, MATRICES);
    memory.write32(INSTANCE + 0x18, (255.0_f32 / 60.0).to_bits());
    memory.write32(MODEL + 0x1c, 0);
    memory.write32(MODEL + 0x20, 0);

    memory.write32(0x007e_3954, CLIPS);
    memory.write32(CLIPS, CLIP);
    memory.write32(CLIP + 0x10, KEY_COUNT);
    memory.write32(CLIP + 0x14, 0);
    memory.write32(CLIP + 0x18, 120.0_f32.to_bits());
    memory.write32(CLIP + 0x1c, KEY_TABLE);
    for frame in 0..KEY_COUNT {
        memory.write32(KEY_TABLE + frame * 4, frame);
    }

    assert!(anim_lane3::run(0x60b30, &mut memory, &mut guest, &mut regs));
    assert_eq!(memory.read32(INSTANCE + 0x18), target_time.to_bits());
    assert_eq!(&*calls.borrow(), &[0x60920, 0x60540, 0x604d0, 0x605d0]);
}
