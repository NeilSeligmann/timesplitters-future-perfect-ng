// SPDX-License-Identifier: GPL-3.0-or-later
//! The offline dispatcher must leave unknown addresses to its caller.
use tsfp_port_runtime::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};
use tsfp_t2_offline as _;

struct NoAccess;

impl GuestMemory for NoAccess {
    fn read8(&self, _: u32) -> u8 {
        panic!("unknown dispatch read guest memory")
    }
    fn write8(&mut self, _: u32, _: u8) {
        panic!("unknown dispatch wrote guest memory")
    }
}

impl GuestCall for NoAccess {
    fn call(&mut self, _: u32, _: &mut ExtRegisters) {
        panic!("unknown dispatch called guest code")
    }
}

#[test]
fn unknown_root_is_unhandled_without_touching_guest_state() {
    let mut regs = ExtRegisters {
        eax: 1,
        ecx: 2,
        edx: 3,
        esp: 4,
        ebx: 5,
        ebp: 6,
        esi: 7,
        edi: 8,
        eflags: 9,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 3,
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    let before = regs;
    assert_eq!(tsfp_t2_lanes::cleanup(u32::MAX), None);
    assert!(!tsfp_t2_lanes::run(
        u32::MAX,
        &mut NoAccess,
        &mut NoAccess,
        &mut regs
    ));
    assert_eq!(regs, before);
}

struct CutsceneMemory;
impl GuestMemory for CutsceneMemory {
    fn read8(&self, address: u32) -> u8 {
        let bytes = 0x8765_4321u32.to_le_bytes();
        bytes[(address.checked_sub(0x78cd58).expect("unexpected read")) as usize]
    }
    fn write8(&mut self, _: u32, _: u8) {
        panic!("getter wrote guest memory")
    }
}

#[test]
fn cutscene_getter_returns_global_and_preserves_other_registers() {
    let mut regs = ExtRegisters {
        eax: 1,
        ecx: 2,
        edx: 3,
        esp: 4,
        ebx: 5,
        ebp: 6,
        esi: 7,
        edi: 8,
        eflags: 9,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 3,
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    let mut expected = regs;
    expected.eax = 0x8765_4321;
    assert_eq!(tsfp_t2_lanes::cleanup(0x23ef00), Some(0));
    assert!(tsfp_t2_lanes::run(
        0x23ef00,
        &mut CutsceneMemory,
        &mut NoAccess,
        &mut regs
    ));
    assert_eq!(regs, expected);
}
