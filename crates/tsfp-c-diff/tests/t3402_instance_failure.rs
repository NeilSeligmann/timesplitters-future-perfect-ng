// SPDX-License-Identifier: GPL-3.0-or-later
//! T3402 Tier 1: compare the 0x60B30 empty-key early return with the lifted body.
//! Guest memory values are FABRICATED to reach the missing-bracket path.
#![cfg(w4_lifted)]

use std::ptr;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::anim_lane3;
use tsfp_ports::memory::GuestMemory;

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn munmap(addr: *mut u8, len: usize) -> i32;
    fn shim_set_base(base: *mut u8);
    fn shim_script(words: *const u32, entries: u32);
    fn w4_run(va: u32, regs: *mut u32) -> i32;
}

const ARENA_LEN: usize = (1usize << 32) + 0x1_0000;
const STACK: u32 = 0x0030_0000;
const BASE: u32 = 0x0001_0000;
const SIZE: usize = 0x80_0000;
const OWNER: u32 = 0x0002_0000;
const INSTANCE: u32 = 0x0003_0000;
const MODEL: u32 = 0x0004_0000;
const MATRICES: u32 = 0x0005_0000;
const CLIPS: u32 = 0x0006_0000;
const CLIP: u32 = 0x0007_0000;
const CLIP_TABLE: u32 = 0x007e_3954;

struct Fake(Vec<u8>);

impl Fake {
    fn index(address: u32) -> Option<usize> {
        let index = address.checked_sub(BASE)? as usize;
        (address < 0x8000_0000 && index < SIZE).then_some(index)
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

#[derive(Default)]
struct Guest(Vec<u32>);

impl GuestCall for Guest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.0.push(va);
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

fn port_layer_time(owner_time: f32, track_offset: f32, old_time: f32, duration: f32) -> u32 {
    let mut memory = Fake(vec![0; SIZE]);
    let mut regs = registers();
    let mut guest = Guest::default();

    memory.write32(STACK + 4, OWNER);
    memory.write32(STACK + 8, 0);
    memory.write32(STACK + 12, INSTANCE);
    memory.write32(STACK + 16, 0);
    memory.write32(OWNER + 8, owner_time.to_bits());
    memory.write32(OWNER + 0x34, track_offset.to_bits());
    memory.write32(INSTANCE, MODEL);
    memory.write32(INSTANCE + 4, MATRICES);
    memory.write32(INSTANCE + 0x18, old_time.to_bits());
    memory.write32(MODEL + 0x1c, 0);
    memory.write32(MODEL + 0x20, 0);
    memory.write32(CLIP_TABLE, CLIPS);
    memory.write32(CLIPS, CLIP);
    memory.write32(CLIP + 0x18, duration.to_bits());

    assert!(anim_lane3::run(0x60b30, &mut memory, &mut guest, &mut regs));
    assert_eq!(guest.0.first(), Some(&0x60920), "snap gate runs first");
    assert!(
        guest
            .0
            .iter()
            .all(|va| matches!(*va, 0x60920 | 0x60540 | 0x604d0 | 0x605d0)),
        "empty-key case must not sample keys or re-enter the evaluator"
    );
    memory.read32(INSTANCE + 0x18)
}

fn lifted_layer_time(owner_time: f32, track_offset: f32, old_time: f32, duration: f32) -> u32 {
    let base = unsafe { mmap(ptr::null_mut(), ARENA_LEN, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
    assert_ne!(base as isize, -1, "lifted guest arena mmap failed");
    unsafe { shim_set_base(base) };

    put32(base, STACK + 4, OWNER);
    put32(base, STACK + 8, 0);
    put32(base, STACK + 12, INSTANCE);
    put32(base, STACK + 16, 0);
    put32(base, OWNER + 8, owner_time.to_bits());
    put32(base, OWNER + 0x34, track_offset.to_bits());
    put32(base, INSTANCE, MODEL);
    put32(base, INSTANCE + 4, MATRICES);
    put32(base, INSTANCE + 0x18, old_time.to_bits());
    put32(base, MODEL + 0x1c, 0);
    put32(base, MODEL + 0x20, 0);
    put32(base, CLIP_TABLE, CLIPS);
    put32(base, CLIPS, CLIP);
    put32(base, CLIP + 0x18, duration.to_bits());

    let script = [0u32; 18];
    unsafe { shim_script(script.as_ptr(), 6) };
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(0x60b30, regs.as_mut_ptr()) }, 1);
    let result = get32(base, INSTANCE + 0x18);
    assert_eq!(unsafe { munmap(base, ARENA_LEN) }, 0, "lifted arena unmap");
    result
}

fn put32(base: *mut u8, address: u32, value: u32) {
    unsafe {
        ptr::copy_nonoverlapping(value.to_le_bytes().as_ptr(), base.add(address as usize), 4)
    };
}

fn get32(base: *mut u8, address: u32) -> u32 {
    let mut bytes = [0; 4];
    unsafe { ptr::copy_nonoverlapping(base.add(address as usize), bytes.as_mut_ptr(), 4) };
    u32::from_le_bytes(bytes)
}

#[test]
fn empty_key_early_return_matches_lifted_layer_time_writeback() {
    assert_eq!(std::env::var("TSFP_REF_LIFTED").as_deref(), Ok("1"));

    // Positive, zero and negative durations exercise advance, clamp, wrap and
    // old-time preservation while the empty key table takes the same early
    // return in both implementations.
    let mut cases = vec![
        (17.066_667, 0.0, 17.0, 119.86),
        (0.25, 0.5, 19.5, 1.0),
        (2.25, -0.25, 7.0, 0.5),
        (-1.0, 0.0, 3.0, 2.0),
        (4.0, 1.0, -2.0, 0.0),
        (4.0, 1.0, -2.0, -1.0),
        (9.0, 3.0, 2.5, f32::NAN),
    ];
    for seed in 0..64u32 {
        let duration = match seed % 5 {
            0 => 0.0,
            1 => -1.0,
            2 => 0.25,
            3 => 2.0,
            _ => 119.86,
        };
        let scale = if duration > 0.0 { duration } else { 1.0 };
        let target_steps = (seed % 9) as i32 - 4;
        let target_time = target_steps as f32 * scale * 0.5;
        let owner_time = seed as f32 * 0.125 - 4.0;
        let track_offset = owner_time - target_time;
        let old_time = seed as f32 * 0.25 - 3.0;
        cases.push((owner_time, track_offset, old_time, duration));
    }

    for (index, (owner_time, track_offset, old_time, duration)) in cases.into_iter().enumerate() {
        let rust = port_layer_time(owner_time, track_offset, old_time, duration);
        let lifted = lifted_layer_time(owner_time, track_offset, old_time, duration);
        assert_eq!(
            rust, lifted,
            "FABRICATED case {index}: owner={owner_time:?}, offset={track_offset:?}, old={old_time:?}, duration={duration:?}"
        );
    }
}
