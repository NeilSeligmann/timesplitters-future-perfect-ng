// SPDX-License-Identifier: GPL-3.0-or-later
//! T1953: hand ports of the two weapons-remainder roots that have no lifted C source
//! (0x20a8a0 and 0x2214e0, both flow-decoded additions, `tools/data/function_additions.csv`).
//! Ported from the static disassembly, Verification Tier 0 (compiles, smoke test in
//! `crates/tsfp-ports/tests/t1953/manual.rs`). No differential exists: there is no lifted original to
//! compare. Behaviour-faithful, not register-exact (scratch registers eax/ecx/edx are left unspecified).
//! Names, record interpretation and subsystem assignment are INFERRED.
#![allow(clippy::all)]
use super::rt::*;
use crate::abi::call_cdecl;

fn read16<M: GuestMemory>(m: &M, address: u32) -> u16 {
    m.read8(address) as u16 | (m.read8(address.wrapping_add(1)) as u16) << 8
}

fn f32_at<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// `game_hit_callback_forward_to_221410_when_fields_0x38_and_0x28_set`: original 0x2214E0.
/// `cdecl(rec)`: when `[rec+0x38]` and `[rec+0x28]` are set, tail-jumps to 0x221410 with
/// EAX = `[[rec+0x38]+4]` and EDX = `[rec]`, else returns.
pub fn game_hit_callback_forward_to_221410_when_fields_0x38_and_0x28_set<M: GuestMemory, C: GuestCall>(
    m: &mut M, guest: &mut C, r: &mut ExtRegisters,
) {
    r.edx = m.read32(r.esp.wrapping_add(4));
    r.eax = m.read32(r.edx.wrapping_add(0x38));
    if r.eax == 0 {
        return;
    }
    r.ecx = m.read32(r.edx.wrapping_add(0x28));
    if r.ecx == 0 {
        return;
    }
    r.eax = m.read32(r.eax.wrapping_add(4));
    r.edx = m.read32(r.edx);
    r.tail_call(guest, 0x221410);
}

/// `game_hit_guarded_health_update_allocate_random_lifetime_entry_then_break`: original 0x20A8A0.
/// `cdecl(hit)`: subtracts hit `+0x20` from the owner's health `+0x1cc`, and when health went negative
/// and `+0x1a4` equals the 0x475d5c constant, installs a random lifetime (0x153b40) with callbacks
/// 0x1ebed0/0x1ebdc0, then runs 0x1711d0 on the hit when the entry (`+0x80`) exists.
pub fn game_hit_guarded_health_update_allocate_random_lifetime_entry_then_break<M: GuestMemory, C: GuestCall>(
    m: &mut M, guest: &mut C, r: &mut ExtRegisters,
) {
    let (saved_esi, saved_edi) = (r.esi, r.edi);
    let hit = m.read32(r.esp.wrapping_add(4));
    r.edi = hit;
    let owner = m.read32(hit);
    r.esi = owner;
    let zero = f32_at(m, 0x475d5c);
    let extension = m.read32(owner.wrapping_add(0x2d8));
    let skip = (extension != 0 && m.read32(extension.wrapping_add(4)) == 0x4e4f_444d)
        || read16(m, owner.wrapping_add(0x94)) == 0x507;
    if !skip {
        let entry = m.read32(owner.wrapping_add(0x80));
        let mut elapsed = zero;
        if entry != 0 {
            call_cdecl(guest, m, r, 0x168040, &[entry]);
            elapsed = r.fpu.st(0) as f32; // FSTP m32 then MOVSS: rounded to single
            r.fpu.pop();
        }
        let kind_17_gate_open = m.read32(hit.wrapping_add(0x24)) != 0x17 || f32_at(m, 0x4785e4) > elapsed;
        if kind_17_gate_open {
            let health = f32_at(m, owner.wrapping_add(0x1cc)) - f32_at(m, hit.wrapping_add(0x20));
            m.write32(owner.wrapping_add(0x1cc), health.to_bits());
            // comiss 0, health + jbe: taken for health >= 0 and NaN, so only health < 0 continues
            if 0.0f32 > health && f32_at(m, owner.wrapping_add(0x1a4)) == zero {
                let record = m.read32(owner.wrapping_add(4));
                m.write32(record.wrapping_add(0x30), 4);
                call_cdecl(guest, m, r, 0x153b40, &[]);
                let mut value = r.eax as i32 as f64;
                if (r.eax as i32) < 0 {
                    value += f32_at(m, 0x475ccc) as f64;
                }
                value *= f32_at(m, 0x4785d0) as f64;
                value += f32_at(m, 0x47862c) as f64;
                m.write32(owner.wrapping_add(0x1a0), 0);
                m.write32(owner.wrapping_add(0x1a4), (value as f32).to_bits());
                call_cdecl(guest, m, r, 0xa5ce0, &[owner, 3, 0x1f3f20]);
                call_cdecl(guest, m, r, 0x167f90, &[owner]);
                if m.read32(owner.wrapping_add(0x80)) == 0 {
                    call_cdecl(guest, m, r, 0x16c170, &[owner, 1]);
                    let created = r.eax;
                    m.write32(owner.wrapping_add(0x80), created);
                    if created != 0 {
                        call_cdecl(guest, m, r, 0x172760, &[created, owner, 1, 0xffff_ffff, 0]);
                    }
                }
                let entry = m.read32(owner.wrapping_add(0x80));
                if entry != 0 {
                    call_cdecl(guest, m, r, 0x165d50, &[entry, 0x1ebed0, 0x1ebdc0]);
                }
            }
            if m.read32(owner.wrapping_add(0x80)) != 0 {
                call_cdecl(guest, m, r, 0x1711d0, &[hit, 0]);
            }
        }
    }
    r.esi = saved_esi;
    r.edi = saved_edi;
}

/// Hybrid dispatch for the hand-ported roots (per-function A/B switch lives in the caller).
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, r: &mut ExtRegisters) -> bool {
    match va {
        0x20a8a0 => game_hit_guarded_health_update_allocate_random_lifetime_entry_then_break(m, guest, r),
        0x2214e0 => game_hit_callback_forward_to_221410_when_fields_0x38_and_0x28_set(m, guest, r),
        _ => return false,
    }
    true
}
