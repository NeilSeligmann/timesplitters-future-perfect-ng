// SPDX-License-Identifier: GPL-3.0-or-later
//! T3402 Tier 1: compare readable quaternion matrix generation with lifted `0x1506B0`.
//! Quaternions and the two scalar constants are FABRICATED test state.
#![cfg(w4_lifted)]

use std::ptr;

use tsfp_engine_anim::posemath::quat_matrix;

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn munmap(addr: *mut u8, len: usize) -> i32;
    fn shim_set_base(base: *mut u8);
    fn w4_run(va: u32, regs: *mut u32) -> i32;
}

const ARENA_LEN: usize = (1usize << 32) + 0x1_0000;
const STACK: u32 = 0x0030_0000;
const MATRIX: u32 = 0x0004_0000;
const QUAT: u32 = 0x0005_0000;
const ONE: u32 = 0x0047_5c78;
const TWO: u32 = 0x0047_5d10;

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

fn lifted_matrix(base: *mut u8, quaternion: [f32; 4]) -> [u32; 16] {
    put32(base, ONE, 1.0f32.to_bits());
    put32(base, TWO, 2.0f32.to_bits());
    put32(base, STACK, 0x00f0_0d00);
    put32(base, STACK + 4, MATRIX);
    put32(base, STACK + 8, QUAT);
    for index in 0..16 {
        put32(base, MATRIX + index * 4, 0x7fc0_0000);
    }
    for (index, value) in quaternion.into_iter().enumerate() {
        put32(base, QUAT + index as u32 * 4, value.to_bits());
    }

    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(0x1506b0, regs.as_mut_ptr()) }, 1);

    let mut result = [0; 16];
    for (index, word) in result.iter_mut().enumerate() {
        *word = get32(base, MATRIX + index as u32 * 4);
    }
    result
}

fn next(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    ((*seed >> 8) as f32 / 16_777_215.0) * 2.0 - 1.0
}

#[test]
fn lifted_quaternion_matrix_matches_recorded_and_fabricated_inputs() {
    let base = unsafe { mmap(ptr::null_mut(), ARENA_LEN, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
    assert_ne!(base as isize, -1, "lifted guest arena mmap failed");
    unsafe { shim_set_base(base) };

    let mut seed = 0x3402_1506;
    let mut compared = 0;
    while compared < 2_000 {
        let quaternion = if compared == 0 {
            // Recorded node-9 sample from the owner-route mismatch (Job 7).
            [
                f32::from_bits(0xbefb_f6fc),
                f32::from_bits(0x3ef7_66f8),
                f32::from_bits(0x3e84_9a84),
                f32::from_bits(0xbf01_3b81),
            ]
        } else {
            [
                next(&mut seed),
                next(&mut seed),
                next(&mut seed),
                next(&mut seed),
            ]
        };
        let norm = quaternion.iter().map(|value| value * value).sum::<f32>();
        if norm < 1.0e-3 {
            continue;
        }

        let expected = quat_matrix(quaternion);
        let actual = lifted_matrix(base, quaternion).map(f32::from_bits);
        for index in 0..16 {
            if compared == 0 {
                assert_eq!(
                    actual[index].to_bits(),
                    expected[index].to_bits(),
                    "recorded node-9 matrix word {index}: lifted={:#010x} Rust={:#010x}",
                    actual[index].to_bits(),
                    expected[index].to_bits()
                );
            }
            let tolerance = 2.0e-5 * expected[index].abs().max(1.0);
            assert!(
                (actual[index] - expected[index]).abs() <= tolerance,
                "case {compared}, matrix word {index}: lifted={:#010x} ({}) Rust={:#010x} ({})",
                actual[index].to_bits(),
                actual[index],
                expected[index].to_bits(),
                expected[index]
            );
        }
        compared += 1;
    }
    assert_eq!(unsafe { munmap(base, ARENA_LEN) }, 0, "lifted arena unmap");
}
