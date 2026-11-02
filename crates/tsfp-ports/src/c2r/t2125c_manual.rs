// SPDX-License-Identifier: GPL-3.0-or-later
//! Hand ports of the two LCG64 functions that `tools.c2rust` translated with a wrong carry (T2125, batch 3).
//!
//! The translator emitted the 64-bit carry-out of `add` as `((a as u64 + b as u64).wrapping_shr(0x20u32 & 31)) & 1`.
//! The shift count of a 64-bit shift is masked with 63, not 31, so `0x20 & 31 == 0` made the carry always the low bit
//! of the sum (gap recorded for the T1878 owner in docs/t2125-frontier-batch2.md, `tools/c2rust` not edited).
//! The differential caught it (`mem[4e7954] orig 00 rust 01`, 2000 of 2000 seeds), so these two are written by hand
//! from the lifted C (`generated/lifted/gen/recomp_0017.c`, pin 1029736c3dd9ddb2) and keep the register residue of the
//! original so the callee record of the differential matches.
//!
//! Verification tier 1 (weak, FABRICATED inputs): differential vs the lifted original, see docs/t2125-frontier-batch2.md.
#![allow(clippy::all)]
use super::rt::*;

const LCG_STATE_LO: u32 = 0x004e_7950; // 64-bit generator state, low word
const LCG_STATE_HI: u32 = 0x004e_7954; // ... high word
const LCG_ADVANCE_COUNT: u32 = 0x0074_97c8; // advances since the last seed
const LCG_COUNTER_A: u32 = 0x0074_97cc; // two further counters cleared by the seed call
const LCG_COUNTER_B: u32 = 0x0074_97d0;
const LCG_MULTIPLIER: u32 = 0x0003_43fd; // 214013
const LCG_INCREMENT: u32 = 0x0026_9ec3; // 2531011
const ALLMUL: u32 = 0x003c_9ed0; // 64-bit multiply helper (a_lo, a_hi, b_lo, b_hi), result in edx:eax

/// `game_rng_advance_lcg64` (0x153B40): `state = state * 214013 + 2531011` on the 64-bit state at 0x4E7950, counts the
/// advance in 0x7497C8. The multiply is the original's call to the 64-bit multiply helper (kept as a call so the
/// callee record matches), the increment carries into the high word.
// portreg: va=0x00153b40 tier=1
pub fn game_rng_advance_lcg64<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.ecx = m.read32(LCG_ADVANCE_COUNT);
    r.eax = m.read32(LCG_STATE_HI);
    r.push(m, 0);
    r.ecx = r.ecx.wrapping_add(1);
    r.push(m, LCG_MULTIPLIER);
    m.write32(LCG_ADVANCE_COUNT, r.ecx);
    r.ecx = m.read32(LCG_STATE_LO);
    r.push(m, r.eax);
    r.push(m, r.ecx);
    r.call(guest, ALLMUL);
    let (low, carry) = r.eax.overflowing_add(LCG_INCREMENT);
    r.eax = low;
    r.edx = r.edx.wrapping_add(carry as u32); // adc edx, 0
    m.write32(LCG_STATE_LO, r.eax);
    m.write32(LCG_STATE_HI, r.edx);
}

/// `game_rng_seed_lcg64_state_from_arg_plus_one` (0x153B80): cdecl(seed). State = `seed + 1` as a 64-bit value (the
/// carry of the 32-bit add goes to the high word), both counters and the advance count cleared.
// portreg: va=0x00153b80 tier=1
pub fn game_rng_seed_lcg64_state_from_arg_plus_one<M: GuestMemory>(
    m: &mut M,
    r: &mut ExtRegisters,
) {
    r.ecx = m.read32(r.esp.wrapping_add(4));
    r.eax = 0;
    let (sum, carry) = r.ecx.overflowing_add(1);
    r.ecx = sum;
    r.edx = carry as u32; // xor edx,edx ; adc edx, eax(0)
    m.write32(LCG_ADVANCE_COUNT, r.eax);
    m.write32(LCG_STATE_LO, r.ecx);
    m.write32(LCG_STATE_HI, r.edx);
    m.write32(LCG_COUNTER_A, r.eax);
    m.write32(LCG_COUNTER_B, r.eax);
}

/// Dispatch used by `c2r::run`.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x0015_3b40 => game_rng_advance_lcg64(m, guest, regs),
        0x0015_3b80 => game_rng_seed_lcg64_state_from_arg_plus_one(m, regs),
        _ => return false,
    }
    true
}
