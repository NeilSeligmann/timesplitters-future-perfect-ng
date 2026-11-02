// SPDX-License-Identifier: GPL-3.0-or-later
//! T1831 (T1815 slice 2): weapon/ammo/inventory leaves over the extended `ExtRegisters` ABI.
//! Source-recovered from the lifted originals (docs/t-weapons-port.md). INFERRED field
//! names and semantics, byte/register behaviour checked against the retained lifted
//! originals by tests/c/test_rust_weapons.c on FABRICATED seeded memory. Not admitted:
//! this module is absent from every selector (`EXT_ROOTS`, `ROOTS`). Callees run as the
//! retained lifted originals through `GuestCall`.
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::{GuestMemory, MappedGuest},
};

/// Guest address of the pointer to the inventory/weapon-definition table (stride 0x29C).
pub const DEFINITION_TABLE: u32 = 0x4f9bac;
/// Global that suppresses ammo take when non-zero (read by 0x1B6790).
pub const AMMO_TAKE_SUPPRESS: u32 = 0x6d6118;

fn push<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters, value: u32) {
    regs.esp = regs.esp.wrapping_sub(4);
    m.write32(regs.esp, value);
}
fn arg<M: GuestMemory>(m: &M, regs: &ExtRegisters, index: u32) -> u32 {
    m.read32(regs.esp.wrapping_add(4 + 4 * index))
}
fn sext16<M: GuestMemory>(m: &M, address: u32) -> u32 {
    m.read16(address) as i16 as i32 as u32
}

/// 0x1B5660 `(base, index) -> &entry.field_68` (cdecl). Entries are 0x7C bytes at
/// `base + 0x68`. A negative first word selects the base entry (fire-mode select fallback).
/// EAX = result, ECX = base, EDX = first word of the indexed entry (residue matched).
pub fn b5660<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let base = arg(m, regs, 0);
    let index = arg(m, regs, 1);
    let entry = index
        .wrapping_mul(0x7c)
        .wrapping_add(base)
        .wrapping_add(0x68);
    let first = m.read32(entry);
    regs.ecx = base;
    regs.edx = first;
    regs.eax = if (first as i32) < 0 {
        base.wrapping_add(0x68)
    } else {
        entry
    };
}

/// 0x1B6DB0 `(owner, state, flag, _unused) -> bool` (cdecl). INFERRED: true when the
/// owner's i16 ammo count at `[type]` plus the state's count is at least the definition cap
/// (+0x90 when `flag != 0`, else +0x8C), else the answer of 0x1B6C90(owner, state, 1, flag).
/// A negative definition index at `state+0x30` skips the cap test.
pub fn b6db0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let owner = arg(m, regs, 0);
    let state = arg(m, regs, 1);
    let flag = arg(m, regs, 2);
    let index = m.read32(state.wrapping_add(0x30));
    let mut r = entry;
    push(m, &mut r, entry.esi);
    push(m, &mut r, entry.edi);
    r.edx = state;
    r.esi = flag;
    r.edi = owner;
    r.eax = index;
    let mut passes = false;
    if (index as i32) >= 0 {
        let definition = index
            .wrapping_mul(0x29c)
            .wrapping_add(m.read32(DEFINITION_TABLE));
        let kind = m.read32(definition.wrapping_add(0x14));
        let default_cap = m.read32(definition.wrapping_add(0x8c));
        let alternate_cap = m.read32(definition.wrapping_add(0x90));
        let cap = crate::rea_weapon_ammo_cap::selected_cap(
            default_cap as i32,
            alternate_cap as i32,
            flag,
        ) as u32;
        let doubled = kind.wrapping_mul(2);
        let have = sext16(m, owner.wrapping_add(doubled).wrapping_add(0x61e));
        let held = sext16(m, state.wrapping_add(doubled).wrapping_add(0x38));
        m.write32(r.esp.wrapping_sub(4), entry.ebx); // transient PUSH EBX, popped immediately
        r.eax = cap;
        r.ecx = held;
        let decision = crate::rea_weapon_ammo_cap::ammo_cap_decision(
            crate::rea_weapon_ammo_cap::AmmoCapState {
                definition_index: index as i32,
                ammo_kind: kind,
                cap: cap as i32,
                owner_rounds: have as i16,
                state_rounds: held as i16,
            },
        );
        passes = matches!(decision, crate::rea_weapon_ammo_cap::AmmoCapDecision::AtCap);
    }
    if !passes {
        call_cdecl(guest, m, &mut r, 0x1b6c90, &[owner, state, 1, flag]);
        passes = r.eax != 0;
    }
    if passes {
        r.eax = 1;
    }
    *regs = ExtRegisters {
        esi: entry.esi,
        edi: entry.edi,
        ebx: entry.ebx,
        esp: entry.esp,
        ..r
    };
}

/// 0x1B6790 `(a1, a2, a3, a4, a5)` (cdecl). INFERRED ammo take: when the gate 0x3533F0 is
/// zero or 0x358560(*a1) is non-zero, and global 0x6D6118 is zero, forward to 0x1B66E0.
/// EAX is the last value tested or the forwarded callee's result.
pub fn b6790<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut r = entry;
    push(m, &mut r, entry.esi);
    call_cdecl(guest, m, &mut r, 0x3533f0, &[]);
    let a1 = arg(m, &entry, 0);
    r.esi = a1;
    let mut proceed = r.eax == 0;
    if !proceed {
        r.eax = m.read32(a1);
        let value = r.eax;
        call_cdecl(guest, m, &mut r, 0x358560, &[value]);
        proceed = r.eax != 0;
    }
    if proceed {
        r.eax = m.read32(AMMO_TAKE_SUPPRESS);
        if r.eax == 0 {
            let (a2, a3, a4, a5) = (
                arg(m, &entry, 1),
                arg(m, &entry, 2),
                arg(m, &entry, 3),
                arg(m, &entry, 4),
            );
            r.eax = a3;
            r.ecx = a2;
            r.edx = a4;
            call_cdecl(guest, m, &mut r, 0x1b66e0, &[a1, a2, a3, a4, a5]);
        }
    }
    *regs = ExtRegisters {
        esi: entry.esi,
        esp: entry.esp,
        ..r
    };
}

// ---- batch 2 (T1831): 0x1B7ED0 and 0x1C11C0 --------------------------------------------
// x86 SSE scalar semantics (NaN payload choice, cvttss2si indefinite) must match the
// original, so these go through the real instructions on x86_64.
#[cfg(target_arch = "x86_64")]
mod sse {
    pub fn mulss(a: f32, b: f32) -> f32 {
        let mut r = a;
        unsafe {
            core::arch::asm!("mulss {r}, {b}", r = inout(xmm_reg) r, b = in(xmm_reg) b, options(pure, nomem, nostack))
        };
        r
    }
    pub fn divss(a: f32, b: f32) -> f32 {
        let mut r = a;
        unsafe {
            core::arch::asm!("divss {r}, {b}", r = inout(xmm_reg) r, b = in(xmm_reg) b, options(pure, nomem, nostack))
        };
        r
    }
    pub fn addss(a: f32, b: f32) -> f32 {
        let mut r = a;
        unsafe {
            core::arch::asm!("addss {r}, {b}", r = inout(xmm_reg) r, b = in(xmm_reg) b, options(pure, nomem, nostack))
        };
        r
    }
    pub fn subss(a: f32, b: f32) -> f32 {
        let mut r = a;
        unsafe {
            core::arch::asm!("subss {r}, {b}", r = inout(xmm_reg) r, b = in(xmm_reg) b, options(pure, nomem, nostack))
        };
        r
    }
    pub fn cvttss2si(a: f32) -> u32 {
        let r: u32;
        unsafe {
            core::arch::asm!("cvttss2si {r:e}, {a}", r = out(reg) r, a = in(xmm_reg) a, options(pure, nomem, nostack))
        };
        r
    }
}
#[cfg(not(target_arch = "x86_64"))]
mod sse {
    pub fn mulss(a: f32, b: f32) -> f32 {
        a * b
    }
    pub fn divss(a: f32, b: f32) -> f32 {
        a / b
    }
    pub fn addss(a: f32, b: f32) -> f32 {
        a + b
    }
    pub fn subss(a: f32, b: f32) -> f32 {
        a - b
    }
    pub fn cvttss2si(a: f32) -> u32 {
        if a.is_nan() || a >= 2147483648.0 || a < -2147483648.0 {
            0x8000_0000
        } else {
            a as i32 as u32
        }
    }
}
fn float<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}
fn lane0(x: &crate::abi::Xmm) -> f32 {
    f32::from_bits(x.lanes[0])
}
fn int_to_float(v: u32) -> f32 {
    v as i32 as f32
}
/// `(v - sign) >> 1` as the compiler's signed divide by two (rounds toward zero).
fn half_toward_zero(v: u32) -> u32 {
    let sign = if (v as i32) < 0 { 0xffff_ffff } else { 0 };
    ((v.wrapping_sub(sign) as i32) >> 1) as u32
}
/// MOVSS from memory / XORPS: lane 0 set, the other lanes cleared.
fn scalar(bits: u32) -> crate::abi::Xmm {
    crate::abi::Xmm::scalar(bits)
}

/// Entry select shared with 0x1B5660: entry at `definition + 0x68 + index*0x7C`, or the
/// definition's base entry when its first word is negative. Returns (entry, candidate, first).
fn select_entry<M: GuestMemory>(m: &M, definition: u32, index: u32) -> (u32, u32, u32) {
    let candidate = index
        .wrapping_mul(0x7c)
        .wrapping_add(definition)
        .wrapping_add(0x68);
    let first = m.read32(candidate);
    let entry = if (first as i32) < 0 {
        definition.wrapping_add(0x68)
    } else {
        candidate
    };
    (entry, candidate, first)
}

/// 0x1B7ED0 `(weapon, mode, alt, flag) -> bool` (cdecl, frame 0x28, no calls).
/// INFERRED purpose: "may this weapon fire now" gate (not ammo per tick). It is pure over
/// the player `[0x7B0C7C]`, the weapon object, the definition table and the fire-mode entry:
/// 1) a type-2 weapon object whose definition has flag 0x4000 always answers 0;
/// 2) else the mode entry (definition + 0x68 + alt*0x7C, `alt` is read before the EBX push so
///    ESP+0x40 is the third argument there and the second argument later; base entry when its first
///    word is negative; entry flags F, float rates at +0x10 and +0x14 chosen by `flag`)
///    gives `cost = trunc([0x4E8CEC] * rate)` and the cooldown `[owner+0xF0|0xF4]` by `alt`;
/// 3) F bit 7 with `flag == 1` answers 0; the "armed" state (owner row `[Y+0x98]` valid and
///    `[Y+0x80] == 0`) selects token pairs `[Y+0x194..0x2C8]` and an ammo-held refinement
///    (`i16 > 0` at `Y+kind*2+0xDC|0x20C|0x61E`) that may clear the mode token;
/// 4) answer 1 iff cooldown >= cost, (armed) the token for `alt` is at least half the cost
///    clamped to 1, and the mode matches the token; F bit 0x4000 with `flag == 0` takes a
///    shortcut that accepts when the clamped half test passes and the mode matches.
/// Residue: EAX = answer, ECX/EDX = last values written, XMM0 lane 0 = cost product once
/// computed, XMM0 other lanes preserved. ESI/EDI/EBX/EBP/ESP restored. Register flow mirrors
/// the original exactly; frame slots are written to the guest stack like the original.
#[allow(unused_assignments)]
pub fn b7ed0<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let e = entry.esp;
    let weapon = m.read32(e.wrapping_add(4));
    let mode = m.read32(e.wrapping_add(8));
    let alt = m.read32(e.wrapping_add(12));
    let flag = m.read32(e.wrapping_add(16));
    let f = e.wrapping_sub(0x38); // ESP after the four pushes, frame slot = f + offset
    let at = |off: u32| f.wrapping_add(off);
    let player = m.read32(0x7b0c7c);
    let mut ecx = m.read32(player.wrapping_add(0x14));
    let mut edx = m.read32(ecx.wrapping_add(0x7c));
    let mut eax = weapon;
    let mut xmm0 = entry.xmm[0];
    m.write32(e.wrapping_sub(0x2c), entry.ebp);
    m.write32(e.wrapping_sub(0x30), entry.esi);
    m.write32(e.wrapping_sub(0x34), entry.edi);
    m.write32(at(0x18), 1);
    let answer: u32 = 'body: {
        'main: {
            if weapon == 0 {
                break 'main;
            }
            if m.read32(weapon.wrapping_add(0x3c)) != 2 {
                break 'main;
            }
            ecx = m.read32(weapon.wrapping_add(0x30));
            if ecx == 0xffff_ffff {
                break 'main;
            }
            let table = m.read32(DEFINITION_TABLE);
            ecx = ecx.wrapping_mul(0x29c);
            if m.read32(ecx.wrapping_add(table).wrapping_add(0xc)) & 0x4000 == 0 {
                break 'main;
            }
            break 'body 0;
        }
        m.write32(f, entry.ebx);
        let owner = m.read32(weapon.wrapping_add(0x3cc));
        eax = m.read32(owner.wrapping_add(0x30)).wrapping_mul(0x29c);
        ecx = alt.wrapping_mul(0x7c); // read before the EBX push: ESP+0x40 is the third argument
        eax = eax.wrapping_add(m.read32(DEFINITION_TABLE)); // definition
        let definition = eax;
        let first = m.read32(ecx.wrapping_add(eax).wrapping_add(0x68));
        ecx = ecx.wrapping_add(eax).wrapping_add(0x68);
        let mut edi = if (first as i32) < 0 {
            eax.wrapping_add(0x68)
        } else {
            ecx
        };
        let mode_entry = edi;
        ecx = m.read32(mode_entry);
        let entry_flags = ecx;
        m.write32(at(0x1c), entry_flags);
        if (entry_flags as u8) & 0x80 != 0 && flag == 1 {
            break 'body 0;
        }
        ecx = m.read32(edx.wrapping_add(0x98));
        let kind = m.read32(definition.wrapping_add(0x14));
        ecx = ecx.wrapping_mul(0x3c);
        let mut armed = 0u32;
        if m.read32(ecx.wrapping_add(0x4fa134)) != 0xffff_ffff {
            ecx = m.read32(edx.wrapping_add(0x80));
            if ecx == 0 {
                armed = 1;
            }
        }
        m.write32(at(0x3c), armed);
        let cooldown = m.read32(owner.wrapping_add(if alt != 0 { 0xf4 } else { 0xf0 }));
        xmm0.lanes[0] = int_to_float(m.read32(0x4e8cec)).to_bits();
        m.write32(at(0x14), cooldown);
        let rate = float(
            m,
            mode_entry.wrapping_add(if flag != 0 { 0x14 } else { 0x10 }),
        );
        xmm0.lanes[0] = sse::mulss(lane0(&xmm0), rate).to_bits();
        ecx = sse::cvttss2si(lane0(&xmm0));
        let cost = ecx;
        m.write32(at(0x10), cost);
        let mut ebp = armed;
        let slot28;
        if ebp != 0 {
            ecx = m.read32(edx.wrapping_add(0x194));
            edi = m.read32(edx.wrapping_add(0x198));
            slot28 = ecx;
        } else {
            edi = 0xffff_ffff;
            slot28 = edi;
        }
        m.write32(at(0x28), slot28);
        ecx = m.read32(edx.wrapping_add(0x2c4));
        let slot30 = ecx;
        m.write32(at(0x30), slot30);
        ecx = m.read32(edx.wrapping_add(0x2c8));
        let token_in_primary = mode == 0 && ebp != 0;
        m.write32(at(0x20), if token_in_primary { slot30 } else { slot28 });
        let mut token_flag = 1u32; // frame slot 0x18
        'to_80e6: {
            let mut skip_secondary = false;
            if mode == 0 {
                m.write32(at(0x24), ecx);
                skip_secondary = ebp != 0;
            }
            if !skip_secondary {
                m.write32(at(0x24), edi);
                if ebp == 0 {
                    break 'to_80e6;
                }
            }
            ecx = m.read32(0x7b0c7c);
            if m.read32(ecx.wrapping_add(0x1c8)) != 1 {
                break 'to_80e6;
            }
            eax = m.read32(definition.wrapping_add(0x18));
            edi = 0;
            let held = if eax == 0 || eax == 1 { 0x61e } else { 0xdc };
            let count = |m: &M, base: u32| {
                sext16(m, edx.wrapping_add(kind.wrapping_mul(2)).wrapping_add(base)) as i32 > 0
            };
            ecx = count(m, held) as u32;
            ebp = ecx;
            ecx = m.read32(edx.wrapping_add(0x204));
            if ecx != 0xffff_ffff {
                edi = m.read32(DEFINITION_TABLE);
                ecx = ecx.wrapping_mul(0x29c);
                if m.read32(ecx.wrapping_add(edi).wrapping_add(0x18)) == 0 || eax == 1 {
                    ecx = count(m, 0x61e) as u32;
                    edi = ecx;
                } else {
                    eax = count(m, 0x20c) as u32;
                    edi = eax;
                }
            }
            if ebp != 0 {
                edx = slot30;
                let clear = (slot28 as i32) > (edx as i32) || {
                    eax = m.read32(0x7b0c7c);
                    m.read32(eax.wrapping_add(0x5a0)) != 1 || edi == 0
                };
                if clear {
                    token_flag = 0;
                    m.write32(at(0x18), 0);
                }
            }
            ebp = armed;
        }
        eax = entry_flags;
        ecx = cost;
        let esi = mode;
        let token_at = |m: &M, index: u32| m.read32(at(0x20).wrapping_add(index.wrapping_mul(4)));
        'general: {
            if entry_flags & 0x4000 == 0 {
                break 'general;
            }
            eax = flag;
            if flag != 0 {
                break 'general;
            }
            if (cooldown as i32) < (ecx as i32) {
                break 'body 0;
            }
            if ebp != 0 {
                eax = half_toward_zero(ecx);
                if (eax as i32) >= 1 {
                    eax = 1;
                }
                edx = alt;
                if (token_at(m, edx) as i32) < (eax as i32) {
                    break 'general;
                }
            }
            if esi == token_flag {
                break 'body 1;
            }
        }
        if (cooldown as i32) < (ecx as i32) {
            break 'body 0;
        }
        if ebp != 0 {
            eax = half_toward_zero(ecx);
            ecx = alt;
            edx = token_at(m, ecx);
            if (edx as i32) < (eax as i32) {
                break 'body 0;
            }
        }
        if esi != token_flag {
            break 'body 0;
        }
        1
    };
    regs.eax = answer;
    regs.ecx = ecx;
    regs.edx = edx;
    regs.ebx = entry.ebx;
    regs.esi = entry.esi;
    regs.edi = entry.edi;
    regs.ebp = entry.ebp;
    regs.esp = entry.esp;
    regs.xmm[0] = xmm0;
}

/// 0x1C11C0 shot-timer tick (register convention: EAX = weapon record, ESI = owner; no stack
/// args). INFERRED: advances the record's fire timers by one frame and starts the next shot.
/// 1) Mode entry select (owner definition + 0x68 + `[record+0x5C]`*0x7C, base entry when the
///    first word is negative). `record+0xBC = 1.0` ([0x475C78]).
/// 2) `dt = 1.0 / [0x4E8CEC] * [0x4E794C]`; 0x42760([player]) runs with dt stored in the
///    saved-ECX frame slot (it is the callee's fifth stack word, so the frame is written).
/// 3) When `[player+0x18] & result` is zero `record+0xB8 = 1`; else if `record+0xB8 == 0` the
///    owner timer `[owner+0x108] += dt` is clamped to 1.0 (an unordered compare takes the
///    original's `jb`, so a NaN sum is stored unclamped).
/// 4) With a definition, a player with `[player+4] >= 0`, `[player+0xE84] != 0` and a valid
///    definition word, state 0x2D with `[record+0xF8] == 0x2D`, `record+0xB8 != 0` and a
///    pending `record+0x44` runs 0x12F70(0x12) and 0x12F70(0x11) (state 0x2E, timers cleared,
///    flag word `(w & ~8) | 6`) then clears bit 0 of `[[record+0x54]+0x28]`. Otherwise the
///    counter `record+0xA8 += [0x4E7944]` (only when `record+0xB8 != 0` on the no-definition
///    path); when `record+0xAC == 0`, `(float)counter >= [0x4E8CEC] * entry.float(+0xC)` and
///    `[0x4E7944] >= 1`, `record+0x44 = 1`, `record+0x48 = 0.0` and 0x1C0200(owner) starts
///    the shot.
/// Callees: 0x42760, 0x12F70 x2, 0x1C0200 via GuestCall (retained lifted originals).
/// Residue: EAX/ECX/EDX follow the last instruction. The exit pops ECX from the frame slot
/// that now holds dt (stack reuse in the original, kept). XMM0/1/2 as written.
pub fn c11c0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let e = entry.esp;
    let record = entry.eax;
    let owner = entry.esi;
    let mut r = entry;
    m.write32(e.wrapping_sub(4), entry.ecx);
    m.write32(e.wrapping_sub(8), entry.ebx);
    m.write32(e.wrapping_sub(12), entry.ebp);
    m.write32(e.wrapping_sub(16), entry.edi);
    let base = e.wrapping_sub(16); // ESP after the four pushes
    r.esp = base;
    r.ebx = record;
    let definition = m
        .read32(owner.wrapping_add(0x30))
        .wrapping_mul(0x29c)
        .wrapping_add(m.read32(DEFINITION_TABLE));
    let (mode_entry, candidate, first) =
        select_entry(m, definition, m.read32(record.wrapping_add(0x5c)));
    r.eax = definition;
    r.edx = first;
    r.ecx = candidate;
    r.edi = mode_entry;
    r.xmm[1] = scalar(m.read32(0x475c78));
    r.xmm[2].lanes[0] = int_to_float(m.read32(0x4e8cec)).to_bits();
    r.xmm[0] = r.xmm[1];
    r.xmm[0].lanes[0] = sse::divss(lane0(&r.xmm[0]), lane0(&r.xmm[2])).to_bits();
    r.xmm[0].lanes[0] = sse::mulss(lane0(&r.xmm[0]), float(m, 0x4e794c)).to_bits();
    m.write32(record.wrapping_add(0xbc), r.xmm[1].lanes[0]);
    r.eax = m.read32(0x7b0c7c);
    r.ecx = m.read32(r.eax);
    let player_word = r.ecx;
    m.write32(e.wrapping_sub(4), r.xmm[0].lanes[0]); // dt overwrites the saved ECX slot
    call_cdecl(guest, m, &mut r, 0x42760, &[player_word]);
    r.edx = m.read32(0x7b0c7c);
    r.ecx = m.read32(r.edx.wrapping_add(0x18));
    let ebp = 1u32;
    'exit: {
        'timers_done: {
            if r.ecx & r.eax != 0 {
                r.eax = m.read32(record.wrapping_add(0xb8));
                if r.eax != 0 {
                    break 'timers_done;
                }
                r.xmm[0] = scalar(m.read32(owner.wrapping_add(0x108)));
                r.xmm[0].lanes[0] =
                    sse::addss(lane0(&r.xmm[0]), float(m, e.wrapping_sub(4))).to_bits();
                r.xmm[1] = scalar(m.read32(0x475c78));
                m.write32(owner.wrapping_add(0x108), r.xmm[0].lanes[0]);
                if !(lane0(&r.xmm[0]) >= lane0(&r.xmm[1])) {
                    break 'timers_done;
                }
                m.write32(owner.wrapping_add(0x108), r.xmm[1].lanes[0]);
            } else {
                m.write32(record.wrapping_add(0xb8), ebp);
            }
        }
        'accumulate: {
            'no_definition: {
                r.eax = m.read32(record.wrapping_add(0x30));
                if r.eax == 0xffff_ffff {
                    break 'no_definition;
                }
                r.ecx = m.read32(0x7b0c7c);
                r.edx = m.read32(r.ecx.wrapping_add(4));
                if (r.edx as i32) < 0 {
                    break 'no_definition;
                }
                r.edx = m.read32(r.ecx.wrapping_add(0xe84));
                if r.edx == 0 {
                    break 'no_definition;
                }
                r.ecx = m.read32(DEFINITION_TABLE);
                r.eax = r.eax.wrapping_mul(0x29c);
                if m.read32(r.eax.wrapping_add(r.ecx)) == 0xffff_ffff {
                    break 'no_definition;
                }
                r.ecx = m.read32(record.wrapping_add(0xe8));
                r.eax = 0x2d;
                if r.ecx != r.eax {
                    r.edx = m.read32(0x4e7944);
                    let total = m.read32(record.wrapping_add(0xa8)).wrapping_add(r.edx);
                    m.write32(record.wrapping_add(0xa8), total);
                    break 'accumulate;
                }
                if m.read32(record.wrapping_add(0xf8)) != r.eax {
                    break 'exit;
                }
                r.eax = m.read32(record.wrapping_add(0xb8));
                if r.eax == 0 {
                    break 'exit;
                }
                r.eax = m.read32(record.wrapping_add(0x44));
                if r.eax != 0 {
                    r.edx = m.read32(record.wrapping_add(0x3cc));
                    let first_args = [0x12, record, r.edx, 0];
                    call_cdecl(guest, m, &mut r, 0x12f70, &first_args);
                    r.eax = m.read32(record.wrapping_add(0x34));
                    r.ecx = m.read32(record.wrapping_add(0x3cc));
                    r.xmm[0] = crate::abi::Xmm::default();
                    r.eax = (r.eax & 0xffff_fff7) | 6;
                    m.write32(record.wrapping_add(0xe8), 0x2e);
                    m.write32(record.wrapping_add(0x34), r.eax);
                    m.write32(record.wrapping_add(0xf4), 0);
                    m.write32(record.wrapping_add(0xf0), 0);
                    r.esp = base.wrapping_sub(16); // the first call's four args stay pushed
                    let second_args = [0x11, record, r.ecx, ebp];
                    call_cdecl(guest, m, &mut r, 0x12f70, &second_args);
                    r.esp = base;
                }
                r.ebx = m.read32(record.wrapping_add(0x54));
                r.eax = m.read32(r.ebx.wrapping_add(0x28));
                r.eax &= 0xffff_fffe;
                m.write32(r.ebx.wrapping_add(0x28), r.eax);
                break 'exit;
            }
            r.eax = m.read32(record.wrapping_add(0xb8));
            if r.eax == 0 {
                break 'exit;
            }
            r.eax = m.read32(0x4e7944);
            let total = m.read32(record.wrapping_add(0xa8)).wrapping_add(r.eax);
            m.write32(record.wrapping_add(0xa8), total);
        }
        r.ecx = m.read32(record.wrapping_add(0xac));
        r.eax = m.read32(record.wrapping_add(0xa8));
        if r.ecx != 0 {
            break 'exit;
        }
        r.xmm[1].lanes[0] = int_to_float(m.read32(0x4e8cec)).to_bits();
        r.xmm[1].lanes[0] =
            sse::mulss(lane0(&r.xmm[1]), float(m, r.edi.wrapping_add(0xc))).to_bits();
        r.xmm[0].lanes[0] = int_to_float(r.eax).to_bits();
        if !(lane0(&r.xmm[0]) >= lane0(&r.xmm[1])) {
            break 'exit;
        }
        if (m.read32(0x4e7944) as i32) < (ebp as i32) {
            break 'exit;
        }
        r.xmm[0] = crate::abi::Xmm::default();
        m.write32(record.wrapping_add(0x48), 0);
        m.write32(record.wrapping_add(0x44), ebp);
        call_cdecl(guest, m, &mut r, 0x1c0200, &[owner]);
    }
    // 0x1C13AC / 0x1C1336 epilogue: edi, ebp, ebx, ecx are popped from the frame
    r.edi = m.read32(base);
    r.ebp = m.read32(base.wrapping_add(4));
    r.ebx = m.read32(base.wrapping_add(8));
    r.ecx = m.read32(base.wrapping_add(12));
    *regs = ExtRegisters {
        esi: entry.esi,
        esp: entry.esp,
        ..r
    };
}

// T2454: machine-translated block bodies (see the module docs), expected dead stores.
#[allow(
    dead_code,
    unused_assignments,
    unused_mut,
    unused_parens,
    unused_variables,
    unused_labels,
    unused_comparisons,
    unreachable_code,
    unreachable_patterns
)]
#[path = "weapons_bb5c0.rs"]
pub mod bb5c0;
#[allow(
    dead_code,
    unused_assignments,
    unused_mut,
    unused_parens,
    unused_variables,
    unused_labels,
    unused_comparisons,
    unreachable_code,
    unreachable_patterns
)]
#[path = "weapons_shot.rs"]
pub mod shot;

pub const WEAPON_ROOTS: [u32; 4] = [0x1b5660, 0x1b6db0, 0x1b6790, 0x1b9a30];
/// Batch 2 roots (T1831), also differential-only.
pub const BATCH2_ROOTS: [u32; 2] = [0x1b7ed0, 0x1c11c0];
/// Batch 3 root (T1831), differential-only.
pub const BATCH3_ROOTS: [u32; 2] = [0x1c0200, 0x1bb5c0];

/// Differential entry (not selector-registered). True when the Rust body ran.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x1b5660 => b5660(m, regs),
        // portreg: va=0x001b6db0 tier=1
        0x1b6db0 => b6db0(m, guest, regs),
        0x1b6790 => b6790(m, guest, regs),
        // portreg: va=0x001b7ed0 tier=1
        0x1b7ed0 => b7ed0(m, regs),
        // portreg: va=0x001c11c0 tier=1
        0x1c11c0 => c11c0(m, guest, regs),
        0x1c0200 => shot::shot_start(m, guest, regs),
        // portreg: va=0x001bb5c0 tier=1
        0x1bb5c0 => bb5c0::bb5c0(m, guest, regs),
        _ => return false,
    }
    true
}

/// # Safety
/// `regs` is a live `ExtRegisters`, `offset` maps the guest window, `call` is the trampoline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_weapons_try(
    va: u32,
    regs: *mut ExtRegisters,
    offset: usize,
    call: crate::ext::CallFn,
) -> bool {
    let mut memory = unsafe { MappedGuest::new(offset) };
    let mut guest = crate::ext::CGuest(call);
    run(va, &mut memory, &mut guest, unsafe { &mut *regs })
}
