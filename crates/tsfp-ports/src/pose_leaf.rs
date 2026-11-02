// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-physics/fp_scalar
// portreg: mirrors=tsfp-engine-input/leaves
// portreg: mirrors=tsfp-engine-object/closure/calls_w8
// portreg: mirrors=tsfp-gameplay/synth_leaves
//! T1850 (T1815 slice 4): pose and control leaves (input pads, anim object accessors, player slot helpers).
//! Same provenance, labels and conventions as pose.rs (MEASURED against the lifted originals on
//! FABRICATED memory with stub callees, names INFERRED, not admitted).
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables)]
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
    pose::write16,
};
#[allow(unused_imports)]
use crate::inventory::sse;

// ---- translated bodies begin ----
const SAVED_PROFILE_ENTRY_ADDRESS: u32 = 0x78ad34;
const CURRENT_PLAYER_BLOCK_BASE: u32 = 0x7ba364;
const CURRENT_PLAYER_BLOCK_INDEX_ADDRESS: u32 = 0x7ba9e0;
const PLAYER_BLOCK_ENTRY_SIZE: u32 = 0x1e0;
const PLAYER_BLOCK_COMPARE_DWORDS: u32 = 0x1f;
const DWORD_SIZE: u32 = 4;
const PREPARE_PLAYER_BLOCK_CALL: u32 = 0x69620;

/// 0x2cbce0. INFERRED: Compares the current player block with the saved profile entry.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_block_differs_from_saved_profile_entry<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let mut xmm_hi = entry.xmm_hi;
    let fpu = entry.fpu;

    eax = m.read32(SAVED_PROFILE_ENTRY_ADDRESS);

    esp = esp.wrapping_sub(DWORD_SIZE);
    m.write32(esp, esi);
    esp = esp.wrapping_sub(DWORD_SIZE);
    m.write32(esp, edi);
    esp = esp.wrapping_sub(DWORD_SIZE);
    m.write32(esp, eax);

    let mut call_regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        eflags: entry.eflags,
        fpu,
        xmm,
        xmm_hi,
    };
    guest.call(PREPARE_PLAYER_BLOCK_CALL, &mut call_regs);
    (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
        call_regs.eax,
        call_regs.ecx,
        call_regs.edx,
        call_regs.esp,
        call_regs.ebx,
        call_regs.ebp,
        call_regs.esi,
        call_regs.edi,
    );
    xmm = call_regs.xmm;
    xmm_hi = call_regs.xmm_hi;

    edi = m.read32(CURRENT_PLAYER_BLOCK_INDEX_ADDRESS);
    edi = ((edi as i32).wrapping_mul(PLAYER_BLOCK_ENTRY_SIZE as i32)) as u32;
    esp = esp.wrapping_add(DWORD_SIZE);
    edi = edi.wrapping_add(CURRENT_PLAYER_BLOCK_BASE);

    esi = eax;
    edx = 0;
    ecx = PLAYER_BLOCK_COMPARE_DWORDS;
    let step = if entry.eflags & 0x400 != 0 {
        DWORD_SIZE.wrapping_neg()
    } else {
        DWORD_SIZE
    };

    let mut equal = false;
    while ecx != 0 {
        let saved_value = m.read32(esi);
        let current_value = m.read32(edi);
        equal = saved_value == current_value;
        esi = esi.wrapping_add(step);
        edi = edi.wrapping_add(step);
        ecx = ecx.wrapping_sub(1);
        if !equal {
            break;
        }
    }

    eax = edx;
    eax = (eax & 0xffff_ff00) | ((!equal as u32) & 0xff);

    edi = m.read32(esp);
    esp = esp.wrapping_add(DWORD_SIZE);
    esi = m.read32(esp);
    esp = esp.wrapping_add(DWORD_SIZE);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        xmm_hi,
        ..entry
    };
}


/// 0x259DF0. INFERRED: Rotates float +0x388 into +0x38c and +0x384 of object 7b0cc0, refreshes +0x388 from cb3b0, then clears bits 0x20 and 0x40 of the player 7b0c7c flags at +0xc
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_7b0c7c_clear_flags_0x20_0x40_and_rotate<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let mut fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x259df0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        fpu = cr.fpu;
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x259df0, previous, pc); previous = pc; }
        match pc {
            0x259df0 => {
                eax = m.read32(0x7b0cc0);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x259e30; continue; }
                pc = 0x259df9; continue;
            }
            0x259df9 => {
                ecx = m.read32(eax.wrapping_add(0x388));
                m.write32(eax.wrapping_add(0x38c), ecx);
                eax = m.read32(0x7b0cc0);
                edx = m.read32(eax.wrapping_add(0x388));
                m.write32(eax.wrapping_add(0x384), edx);
                eax = m.read32(0x7b0cc0);
                push!(eax);
                call!(0xcb3b0);
                pc = 0x259e21; continue;
            }
            0x259e21 => {
                ecx = m.read32(0x7b0cc0);
                m.write32(ecx.wrapping_add(0x388), (fpu.stack[fpu.top as usize] as f32).to_bits()); fpu.top = (fpu.top + 1) & 7;
                esp = esp.wrapping_add(0x4);
                pc = 0x259e30; continue;
            }
            0x259e30 => {
                eax = m.read32(0x7b0c7c);
                m.write32(eax.wrapping_add(0xc), (m.read32(eax.wrapping_add(0xc)) & 0xffffffdfu32));
                eax = m.read32(0x7b0c7c);
                m.write32(eax.wrapping_add(0xc), (m.read32(eax.wrapping_add(0xc)) & 0xffffffbfu32));
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0xD1E20. INFERRED: stores x into +0xb4 and y into +0xb8 clamped to within constants 478620 and 475d10 of +0xac unless mode 79094c above 0x64, z into +0xbc
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_set_world_position_0xb4_with_vertical_d<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xd1e20;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0xd1e20, previous, pc); previous = pc; }
        match pc {
            0xd1e20 => {
                eax = m.read32(esp.wrapping_add(0x4));
                xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                xmm3 = Xmm::scalar(m.read32(esp.wrapping_add(0xc)));
                m.write32(eax.wrapping_add(0xb4), xmm1.lanes[0]);
                fa = m.read32(0x79094c);
                fb = ((((0x64i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                xmm0 = xmm3;
                xmm0.lanes[0] = sse::subss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(eax.wrapping_add(0xac)))).to_bits();
                if (fas > fbs) { pc = 0xd1ea7; continue; }
                pc = 0xd1e4c; continue;
            }
            0xd1e4c => {
                xmm1 = Xmm::scalar(m.read32(0x478620));
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                xmm2 = Xmm::scalar(m.read32(0x475d10));
                if ((fca > fcb)) { pc = 0xd1e66; continue; }
                pc = 0xd1e61; continue;
            }
            0xd1e61 => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0xd1ea7; continue; }
                pc = 0xd1e66; continue;
            }
            0xd1e66 => {
                fca = f32::from_bits(xmm1.lanes[0]) as f64; fcb = f32::from_bits(xmm0.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0xd1e7f; continue; }
                pc = 0xd1e6b; continue;
            }
            0xd1e6b => {
                xmm3 = Xmm::scalar(m.read32(eax.wrapping_add(0xac)));
                xmm3.lanes[0] = sse::addss(f32::from_bits(xmm3.lanes[0]), f32::from_bits(xmm1.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0xb8), xmm3.lanes[0]);
                pc = 0xd1e7f; continue;
            }
            0xd1e7f => {
                fca = f32::from_bits(xmm0.lanes[0]) as f64; fcb = f32::from_bits(xmm2.lanes[0]) as f64;
                if (((fca <= fcb) || (((fca != fca) || (fcb != fcb))))) { pc = 0xd1eaf; continue; }
                pc = 0xd1e84; continue;
            }
            0xd1e84 => {
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(0xac)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(xmm2.lanes[0])).to_bits();
                m.write32(eax.wrapping_add(0xb8), xmm0.lanes[0]);
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                m.write32(eax.wrapping_add(0xbc), xmm0.lanes[0]);
                break 'run;
            }
            0xd1ea7 => {
                m.write32(eax.wrapping_add(0xb8), xmm3.lanes[0]);
                pc = 0xd1eaf; continue;
            }
            0xd1eaf => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                m.write32(eax.wrapping_add(0xbc), xmm0.lanes[0]);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, ..entry };
}


const PLAYER_SLOT_TABLE_BASE_D2D20: u32 = 0x7b0ce4;
const PLAYER_SLOT_INDEX_MULTIPLIER: u32 = 0x80200803;
const PLAYER_SLOT_INDEX_PRODUCT_SHIFT: u32 = 11;
const PLAYER_SLOT_INDEX_SIGN_SHIFT: u32 = 31;

/// 0xD2D20. INFERRED: returns the slot index for the object field at offset 8, or -1 when that field is null.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn player_slot_index_from_object_field8<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let xmm = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(4));
    eax = m.read32(eax.wrapping_add(8));

    if eax == 0 {
        eax = u32::MAX;
    } else {
        ecx = eax.wrapping_sub(m.read32(PLAYER_SLOT_TABLE_BASE_D2D20));

        let product = ((PLAYER_SLOT_INDEX_MULTIPLIER as i32) as i64)
            .wrapping_mul((ecx as i32) as i64) as u64;
        eax = product as u32;
        edx = (product >> 32) as u32;

        edx = edx.wrapping_add(ecx);
        edx = ((edx as i32) >> PLAYER_SLOT_INDEX_PRODUCT_SHIFT) as u32;
        eax = edx;
        eax = eax.wrapping_shr(PLAYER_SLOT_INDEX_SIGN_SHIFT);
        eax = eax.wrapping_add(edx);
    }

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        ..entry
    };
}


const INPUT_PAD_MAP_ARGUMENT_OFFSET: u32 = 0x4;
const INPUT_PAD_MAP_ENTRY_SIZE: u32 = 0x4;
const INPUT_PAD_MAP_TABLE_ADDRESS: u32 = 0x6b93c0;

/// 0x426d0. INFERRED: returns the input pad map entry indexed by the stack argument.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_pad_map_get<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(INPUT_PAD_MAP_ARGUMENT_OFFSET));
    eax = m.read32(
        eax.wrapping_mul(INPUT_PAD_MAP_ENTRY_SIZE)
            .wrapping_add(INPUT_PAD_MAP_TABLE_ADDRESS),
    );

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        ..entry
    };
}


const INPUT_PAD_ARRAY_BASE: u32 = 0x6b93d4;
const INPUT_PAD_ENTRY_STRIDE: u32 = 0x3c;

/// 0x42710. INFERRED: returns the dword at offset 0x04 of the indexed input pad entry.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn input_pad_6b93d4_get_field_0x04<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = entry.eax;
    let ecx = entry.ecx;
    let edx = entry.edx;
    let esp = entry.esp;
    let ebx = entry.ebx;
    let ebp = entry.ebp;
    let esi = entry.esi;
    let edi = entry.edi;
    let x = entry.xmm;
    let fpu = entry.fpu;

    let pad_index = m.read32(esp.wrapping_add(0x4));
    let pad_offset = pad_index.wrapping_mul(INPUT_PAD_ENTRY_STRIDE);
    eax = m.read32(pad_offset.wrapping_add(INPUT_PAD_ARRAY_BASE));

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        ..entry
    };
}


const INPUT_PAD_GATE_FLAG_ADDRESS: u32 = 0x6b9888;
const INPUT_PAD_GATE_MODE_ADDRESS: u32 = 0x76b0fc;
const INPUT_PAD_GATE_ARGUMENT_OFFSET: u32 = 0x4;
const INPUT_PAD_GATE_ENTRY_SIZE: i32 = 0x3c;
const INPUT_PAD_GATE_TABLE_ADDRESS: u32 = 0x6b9500;
const INPUT_PAD_GATE_MASK_ADDRESS: u32 = 0x6b94d4;
const INPUT_PAD_GATE_INDEX_TABLE_ADDRESS: u32 = 0x6b93c0;
const INPUT_PAD_GATE_INDEX_SCALE: u32 = 0x4;
const INPUT_PAD_GATE_VALUE_ADDRESS: u32 = 0x6b9408;
const INPUT_PAD_GATE_MASKED_VALUE_ADDRESS: u32 = 0x6b93dc;

/// 0x42760. INFERRED: returns zero when 6b9888 is set; otherwise computes the input pad gate value.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn input_pad_gate_zero_when_6b9888_set<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(INPUT_PAD_GATE_FLAG_ADDRESS);
    if eax != 0 {
        eax = 0;
    } else if m.read32(INPUT_PAD_GATE_MODE_ADDRESS) != 0 {
        ecx = m.read32(esp.wrapping_add(INPUT_PAD_GATE_ARGUMENT_OFFSET));
        ecx = ((ecx as i32).wrapping_mul(INPUT_PAD_GATE_ENTRY_SIZE)) as u32;
        eax = m.read32(ecx.wrapping_add(INPUT_PAD_GATE_TABLE_ADDRESS));
        edx = m.read32(ecx.wrapping_add(INPUT_PAD_GATE_MASK_ADDRESS));
        eax = !eax & edx;
    } else {
        eax = m.read32(esp.wrapping_add(INPUT_PAD_GATE_ARGUMENT_OFFSET));
        ecx = m.read32(
            eax.wrapping_mul(INPUT_PAD_GATE_INDEX_SCALE)
                .wrapping_add(INPUT_PAD_GATE_INDEX_TABLE_ADDRESS),
        );
        ecx = ((ecx as i32).wrapping_mul(INPUT_PAD_GATE_ENTRY_SIZE)) as u32;
        eax = m.read32(ecx.wrapping_add(INPUT_PAD_GATE_VALUE_ADDRESS));
        edx = m.read32(ecx.wrapping_add(INPUT_PAD_GATE_MASKED_VALUE_ADDRESS));
        eax = !eax & edx;
    }

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        ..entry
    };
}


const INPUT_PAD_SELECTOR_ADDRESS: u32 = 0x76b0fc;
const INPUT_PAD_MAPPED_TABLE_ADDRESS: u32 = 0x6b93c0;
const INPUT_PAD_MAPPED_BASE: u32 = 0x6b93d0;
const INPUT_PAD_DIRECT_BASE: u32 = 0x6b94c8;
const INPUT_PAD_ENTRY_SIZE: u32 = 0x3c;
const INPUT_PAD_INDEX_SCALE: u32 = 4;
const INPUT_PAD_ARGUMENT_OFFSET: u32 = 4;
const INPUT_PAD_MASK_OFFSET: u32 = 8;
const INPUT_PAD_MASK_FIELD_OFFSET: u32 = 0x38;
const INPUT_PAD_CLEAR_FIELD_OFFSET: u32 = 0x10;

/// 0x42830. INFERRED: Selects a pad entry, ORs a mask into its +0x38 field, and clears that mask from its +0x10 field.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn input_pad_or_field_0x38_clear_field_0x10_mask<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = entry.eax;
    let mut ecx = entry.ecx;
    let mut edx = entry.edx;
    let esp = entry.esp;

    let selector = m.read32(INPUT_PAD_SELECTOR_ADDRESS);
    eax = m.read32(esp.wrapping_add(INPUT_PAD_ARGUMENT_OFFSET));

    if selector == 0 {
        eax = m.read32(
            eax.wrapping_mul(INPUT_PAD_INDEX_SCALE)
                .wrapping_add(INPUT_PAD_MAPPED_TABLE_ADDRESS),
        );
        eax = eax
            .wrapping_mul(INPUT_PAD_ENTRY_SIZE)
            .wrapping_add(INPUT_PAD_MAPPED_BASE);
    } else {
        eax = eax
            .wrapping_mul(INPUT_PAD_ENTRY_SIZE)
            .wrapping_add(INPUT_PAD_DIRECT_BASE);
    }

    ecx = m.read32(esp.wrapping_add(INPUT_PAD_MASK_OFFSET));
    let mask_field_address = eax.wrapping_add(INPUT_PAD_MASK_FIELD_OFFSET);
    let mask_field = m.read32(mask_field_address);
    m.write32(mask_field_address, mask_field | ecx);

    edx = m.read32(eax.wrapping_add(INPUT_PAD_CLEAR_FIELD_OFFSET));
    ecx = !ecx;
    edx &= ecx;
    m.write32(eax.wrapping_add(INPUT_PAD_CLEAR_FIELD_OFFSET), edx);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx: entry.ebx,
        ebp: entry.ebp,
        esi: entry.esi,
        edi: entry.edi,
        xmm: entry.xmm,
        fpu: entry.fpu,
        ..entry
    };
}


const INPUT_PAD_GLOBAL_ADDRESS: u32 = 0x76b0fc;
const INPUT_PAD_POINTER_TABLE_ADDRESS: u32 = 0x6b93c0;
const INPUT_PAD_POINTER_BASE_ADDRESS: u32 = 0x6b93d0;
const INPUT_PAD_MAPPED_BASE_ADDRESS: u32 = 0x6b94c8;
const INPUT_PAD_ENTRY_STRIDE_42870: i32 = 0x3c;
const INPUT_PAD_ENTRY_WORD_OFFSET: u32 = 0x10;
const INPUT_PAD_ENTRY_NEUTRAL_OFFSET: u32 = 0x18;
const INPUT_PAD_ENTRY_MAPPED_OFFSET: u32 = 0x38;
const INPUT_PAD_NEUTRAL_VALUE: u32 = 0x80;

/// 0x42870. INFERRED: resets one pad entry to neutral values.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, clippy::all)]
pub fn input_pad_reset_neutral_mapped<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let xmm = entry.xmm;
    let fpu = entry.fpu;

    let global_value = m.read32(INPUT_PAD_GLOBAL_ADDRESS);
    eax = m.read32(esp.wrapping_add(4));

    if global_value == 0 {
        eax = m.read32(
            eax.wrapping_mul(4)
                .wrapping_add(INPUT_PAD_POINTER_TABLE_ADDRESS),
        );
        eax = (eax as i32).wrapping_mul(INPUT_PAD_ENTRY_STRIDE_42870) as u32;
        eax = eax.wrapping_add(INPUT_PAD_POINTER_BASE_ADDRESS);
    } else {
        eax = (eax as i32).wrapping_mul(INPUT_PAD_ENTRY_STRIDE_42870) as u32;
        eax = eax.wrapping_add(INPUT_PAD_MAPPED_BASE_ADDRESS);
    }

    ecx = m.read32(eax.wrapping_add(INPUT_PAD_ENTRY_MAPPED_OFFSET));
    write16(m, eax.wrapping_add(INPUT_PAD_ENTRY_WORD_OFFSET), 0);
    ecx = ecx | 0xffffu32;
    m.write32(eax.wrapping_add(INPUT_PAD_ENTRY_MAPPED_OFFSET), ecx);

    ecx = INPUT_PAD_NEUTRAL_VALUE;
    m.write32(eax.wrapping_add(INPUT_PAD_ENTRY_NEUTRAL_OFFSET), ecx);
    m.write32(eax.wrapping_add(INPUT_PAD_ENTRY_NEUTRAL_OFFSET + 4), ecx);
    m.write32(eax.wrapping_add(INPUT_PAD_ENTRY_NEUTRAL_OFFSET + 8), ecx);
    m.write32(eax.wrapping_add(INPUT_PAD_ENTRY_NEUTRAL_OFFSET + 12), ecx);
    m.write32(eax.wrapping_add(INPUT_PAD_ENTRY_NEUTRAL_OFFSET + 16), ecx);
    m.write32(eax.wrapping_add(INPUT_PAD_ENTRY_NEUTRAL_OFFSET + 20), ecx);
    m.write32(eax.wrapping_add(INPUT_PAD_ENTRY_NEUTRAL_OFFSET + 24), ecx);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        ..entry
    };
}


const PAD_ARRAY_START: u32 = 0x6b9408;
const PAD_ARRAY_END: u32 = 0x6b94f8;
const PAD_ENTRY_STRIDE: u32 = 0x3c;
const PAD_NEUTRAL_VALUE: u32 = 0x80;

/// 0x428d0. INFERRED: Resets every pad entry in the array to neutral values.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn input_pads_reset_neutral_all<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let xmm = entry.xmm;
    let fpu = entry.fpu;

    eax = PAD_ARRAY_START;
    ecx = 0;
    esp = esp.wrapping_sub(4);
    m.write32(esp, esi);

    loop {
        m.write32(eax, m.read32(eax) | 0xffff);
        write16(m, eax.wrapping_sub(0x28), ecx as u16);

        esi = eax.wrapping_sub(0x20);
        edx = PAD_NEUTRAL_VALUE;
        m.write32(esi, edx);
        m.write32(esi.wrapping_add(0x4), edx);
        m.write32(esi.wrapping_add(0x8), edx);
        m.write32(esi.wrapping_add(0xc), edx);
        m.write32(esi.wrapping_add(0x10), edx);

        eax = eax.wrapping_add(PAD_ENTRY_STRIDE);
        m.write32(esi.wrapping_add(0x14), edx);
        m.write32(esi.wrapping_add(0x18), edx);

        if (eax as i32) >= (PAD_ARRAY_END as i32) {
            break;
        }
    }

    esi = m.read32(esp);
    esp = esp.wrapping_add(4);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        ..entry
    };
}


/// 0x42920. INFERRED: stores float arg into +0 of the pad entry unless 0x6b9888 is nonzero
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_pad_set_float_field_0<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x42920;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x42920, previous, pc); previous = pc; }
        match pc {
            0x42920 => {
                eax = m.read32(0x6b9888);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x42962; continue; }
                pc = 0x42929; continue;
            }
            0x42929 => {
                eax = m.read32(0x76b0fc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = m.read32(esp.wrapping_add(0x4));
                if (fa == fb) { pc = 0x42949; continue; }
                pc = 0x42936; continue;
            }
            0x42936 => {
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = eax.wrapping_add(0x6b94c8);
                m.write32(eax, xmm0.lanes[0]);
                break 'run;
            }
            0x42949 => {
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x6b93c0));
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x8)));
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = eax.wrapping_add(0x6b93d0);
                m.write32(eax, xmm0.lanes[0]);
                pc = 0x42962; continue;
            }
            0x42962 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, ..entry };
}


const INPUT_PAD_SELECTOR_TABLE: u32 = 0x6b93c0;
const INPUT_PAD_SELECTOR_BASE: u32 = 0x6b93d0;
const INPUT_PAD_ENTRY_BASE: u32 = 0x6b94c8;
const INPUT_PAD_DISABLED_ADDRESS: u32 = 0x6b9888;
const INPUT_PAD_AXIS_STRIDE: u32 = 4;
const INPUT_PAD_AXIS_OFFSET: u32 = 0x18;
const INPUT_PAD_DISABLED_VALUE: u32 = 0x80;
const INPUT_PAD_LOW_BYTE_MASK: u32 = 0xff;
const INPUT_PAD_HIGH_BYTE_MASK: u32 = 0xffff_ff00;

/// 0x42A30. INFERRED: returns the selected pad axis byte, or 0x80 when input is disabled
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_pad_get_axis_byte<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    let selector = m.read32(INPUT_PAD_SELECTOR_ADDRESS);
    let mut axis_address = m.read32(esp.wrapping_add(4));

    if selector == 0 {
        axis_address = m.read32(
            axis_address
                .wrapping_mul(INPUT_PAD_AXIS_STRIDE)
                .wrapping_add(INPUT_PAD_SELECTOR_TABLE),
        );
        axis_address = (axis_address as i32)
            .wrapping_mul(INPUT_PAD_ENTRY_STRIDE as i32) as u32;
        axis_address = axis_address.wrapping_add(INPUT_PAD_SELECTOR_BASE);
    } else {
        axis_address = (axis_address as i32)
            .wrapping_mul(INPUT_PAD_ENTRY_STRIDE as i32) as u32;
        axis_address = axis_address.wrapping_add(INPUT_PAD_ENTRY_BASE);
    }

    ecx = m.read32(INPUT_PAD_DISABLED_ADDRESS);
    if ecx != 0 {
        eax = (axis_address & INPUT_PAD_HIGH_BYTE_MASK) | INPUT_PAD_DISABLED_VALUE;
    } else {
        ecx = m.read32(esp.wrapping_add(8));
        let axis_byte = m.read8(
            axis_address
                .wrapping_add(ecx.wrapping_mul(INPUT_PAD_AXIS_STRIDE))
                .wrapping_add(INPUT_PAD_AXIS_OFFSET),
        );
        eax = (axis_address & INPUT_PAD_HIGH_BYTE_MASK)
            | ((axis_byte as u32) & INPUT_PAD_LOW_BYTE_MASK);
    }

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        ..entry
    };
}


/// 0x42CA0. INFERRED: stores byte arg into dword +0x18+idx*4 of the pad entry unless 0x6b9888 is nonzero
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_pad_set_axis_byte<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x42ca0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x42ca0, previous, pc); previous = pc; }
        match pc {
            0x42ca0 => {
                eax = m.read32(0x76b0fc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                eax = m.read32(esp.wrapping_add(0x4));
                if (fa == fb) { pc = 0x42cb7; continue; }
                pc = 0x42cad; continue;
            }
            0x42cad => {
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = eax.wrapping_add(0x6b94c8);
                pc = 0x42cc6; continue;
            }
            0x42cb7 => {
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x6b93c0));
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = eax.wrapping_add(0x6b93d0);
                pc = 0x42cc6; continue;
            }
            0x42cc6 => {
                ecx = m.read32(0x6b9888);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x42cdd; continue; }
                pc = 0x42cd0; continue;
            }
            0x42cd0 => {
                ecx = ((m.read8(esp.wrapping_add(0xc))) as u8 as u32);
                edx = m.read32(esp.wrapping_add(0x8));
                m.write32(eax.wrapping_add(edx.wrapping_mul(0x4)).wrapping_add(0x18), ecx);
                pc = 0x42cdd; continue;
            }
            0x42cdd => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const INPUT_PAD_MAP_BASE: u32 = 0x6b93c0;
const INPUT_PAD_MAP_ENTRY_SIZE_42D00: u32 = 4;
const INPUT_PAD_MAP_VALUE_OFFSET: u32 = 0x8;
const INPUT_PAD_MAP_INDEX_OFFSET: u32 = 0x4;

/// 0x42d00. INFERRED: stores arg2 into 0x6b93c0[arg1]
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_pad_map_set<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(INPUT_PAD_MAP_VALUE_OFFSET));
    ecx = m.read32(esp.wrapping_add(INPUT_PAD_MAP_INDEX_OFFSET));
    m.write32(
        ecx.wrapping_mul(INPUT_PAD_MAP_ENTRY_SIZE_42D00)
            .wrapping_add(INPUT_PAD_MAP_BASE),
        eax,
    );

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        ..entry
    };
}


const ANIM_OBJ_ARGUMENT_STACK_OFFSET: u32 = 0x4;
const ANIM_OBJ_POINTER_OFFSET: u32 = 0x4;
const ANIM_OBJ_BLEND_OFFSET: u32 = 0x198;
const ANIM_OBJ_BLEND_THRESHOLD_ADDRESS: u32 = 0x475cac;
const ANIM_OBJ_ACTIVE_SLOT_OFFSET: u32 = 0x12c;
const ANIM_OBJ_INACTIVE_SLOT_OFFSET: u32 = 0xd4;

/// 0x59cf0. INFERRED: returns the active slot pointer based on the object's blend value.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn anim_obj_active_slot_ptr_0xd4_or_0x12c_by_blen<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = entry.eax;
    let ecx = entry.ecx;
    let edx = entry.edx;
    let esp = entry.esp;
    let ebx = entry.ebx;
    let ebp = entry.ebp;
    let esi = entry.esi;
    let edi = entry.edi;
    let mut xmm = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(ANIM_OBJ_ARGUMENT_STACK_OFFSET));
    eax = m.read32(eax.wrapping_add(ANIM_OBJ_POINTER_OFFSET));

    let blend_value = m.read32(eax.wrapping_add(ANIM_OBJ_BLEND_OFFSET));
    xmm[0] = Xmm::scalar(blend_value);

    let blend = f32::from_bits(xmm[0].lanes[0]);
    let threshold = f32::from_bits(m.read32(ANIM_OBJ_BLEND_THRESHOLD_ADDRESS));

    if blend <= threshold || blend.is_nan() || threshold.is_nan() {
        eax = eax.wrapping_add(ANIM_OBJ_INACTIVE_SLOT_OFFSET);
    } else {
        eax = eax.wrapping_add(ANIM_OBJ_ACTIVE_SLOT_OFFSET);
    }

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        ..entry
    };
}


const STACK_ARG_OBJECT: u32 = 0x4;
const OBJECT_STATE: u32 = 0x4;
const STATE_ANIM_ID: u32 = 0x12c;
const STACK_ARG_ANIM_ID: u32 = 0x8;

/// 0x59d90. INFERRED: returns whether the object's animation ID equals the argument.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn anim_obj_slot_0x12c_anim_id_equals_arg<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(STACK_ARG_OBJECT));
    ecx = m.read32(eax.wrapping_add(OBJECT_STATE));
    edx = m.read32(ecx.wrapping_add(STATE_ANIM_ID));
    ecx = m.read32(esp.wrapping_add(STACK_ARG_ANIM_ID));
    eax = u32::from(edx == ecx);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        ..entry
    };
}


const ANIM_OBJ_FIELD_0X12C: u32 = 0x12c;
const ANIM_OBJ_FIELD_0X124: u32 = 0x124;
const ANIM_OBJ_FIELD_0X17C: u32 = 0x17c;

/// 0x59DD0. INFERRED: Returns zero for a null object, otherwise selects +0x124 or +0x17c based on +0x12c.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_obj_get_field_0x17c_or_0x124_by_field_0x1<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = entry.eax;
    let mut ecx = entry.ecx;
    let edx = entry.edx;
    let esp = entry.esp;
    let x = entry.xmm;
    let fpu = entry.fpu;

    let object = m.read32(esp.wrapping_add(STACK_ARGUMENT_OFFSET));
    eax = if object == 0 {
        0
    } else {
        ecx = m.read32(object.wrapping_add(ANIM_OBJ_FIELD_0X12C));
        if ecx == 0 {
            m.read32(object.wrapping_add(ANIM_OBJ_FIELD_0X124))
        } else {
            m.read32(object.wrapping_add(ANIM_OBJ_FIELD_0X17C))
        }
    };

    *regs = ExtRegisters { eax, ecx, edx, esp, xmm: x, fpu, ..entry };
}


/// 0x59E00. INFERRED: object arg1 -> state at +4: for the animation slot indices at +0xd8 and +0x130 (both looked up in the animation table 0x7e3954 and the 8 byte flag words at 0x7de660) returns 1 when either slot is active and its 64 bit flag word in
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_obj_either_slot_0xd8_or_0x130_flag_mask_t<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x59e00;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x59e00, previous, pc); previous = pc; }
        match pc {
            0x59e00 => {
                eax = m.read32(esp.wrapping_add(0x4));
                edx = m.read32(eax.wrapping_add(0x4));
                ecx = m.read32(edx.wrapping_add(0xd8));
                push!(ebx);
                ebx = m.read32(0x7e3954);
                fa = m.read32(ebx.wrapping_add(ecx.wrapping_mul(0x4)));
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x14));
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x14));
                if (fa == fb) { pc = 0x59e3a; continue; }
                pc = 0x59e24; continue;
            }
            0x59e24 => {
                eax = m.read32(ecx.wrapping_mul(0x8).wrapping_add(0x7de660));
                ecx = m.read32(ecx.wrapping_mul(0x8).wrapping_add(0x7de664));
                eax = (eax & edi);
                ecx = (ecx & esi);
                eax = (eax | ecx);
                fa = eax;
                if ((fa != 0)) { pc = 0x59e5c; continue; }
                pc = 0x59e3a; continue;
            }
            0x59e3a => {
                ecx = m.read32(edx.wrapping_add(0x130));
                fa = m.read32(ebx.wrapping_add(ecx.wrapping_mul(0x4)));
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x59e65; continue; }
                pc = 0x59e46; continue;
            }
            0x59e46 => {
                eax = m.read32(ecx.wrapping_mul(0x8).wrapping_add(0x7de660));
                ecx = m.read32(ecx.wrapping_mul(0x8).wrapping_add(0x7de664));
                eax = (eax & edi);
                ecx = (ecx & esi);
                eax = (eax | ecx);
                fa = eax;
                if ((fa == 0)) { pc = 0x59e65; continue; }
                pc = 0x59e5c; continue;
            }
            0x59e5c => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x59e65 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const ANIM_OBJ_RESET_SPEED_ARG_OFFSET: u32 = 0x4;
const ANIM_OBJ_RESET_SPEED_OBJECT_DURATION_PTR_OFFSET: u32 = 0xd8;
const ANIM_OBJ_RESET_SPEED_VALUE_OFFSET: u32 = 0x108;
const ANIM_OBJ_RESET_SPEED_DURATION_OFFSET: u32 = 0x18;
const ANIM_OBJ_RESET_SPEED_TIME_END_OFFSET: u32 = 0xe0;
const ANIM_OBJ_RESET_SPEED_TIME_START_OFFSET: u32 = 0xdc;
const ANIM_OBJ_RESET_SPEED_TABLE_ADDRESS: u32 = 0x7e3954;
const ANIM_OBJ_RESET_SPEED_POINTER_SIZE: u32 = 0x4;

/// 0x5BA00. INFERRED: Resets speed and sets both time fields to the duration when it is negative.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn anim_obj_reset_speed_0x108_and_set_time_0xdc_0<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let [mut xmm0, xmm1, xmm2, xmm3, xmm4] = entry.xmm;
    let fpu = entry.fpu;

    ecx = m.read32(esp.wrapping_add(ANIM_OBJ_RESET_SPEED_ARG_OFFSET));
    xmm0 = Xmm::default();

    eax = m.read32(ecx.wrapping_add(ANIM_OBJ_RESET_SPEED_OBJECT_DURATION_PTR_OFFSET));
    m.write32(ecx.wrapping_add(ANIM_OBJ_RESET_SPEED_VALUE_OFFSET), xmm0.lanes[0]);

    edx = m.read32(ANIM_OBJ_RESET_SPEED_TABLE_ADDRESS);
    eax = m.read32(edx.wrapping_add(eax.wrapping_mul(ANIM_OBJ_RESET_SPEED_POINTER_SIZE)));

    if eax != 0 {
        let duration = m.read32(eax.wrapping_add(ANIM_OBJ_RESET_SPEED_DURATION_OFFSET));
        let zero = f32::from_bits(xmm0.lanes[0]) as f64;
        let duration_value = f32::from_bits(duration) as f64;

        if !(zero <= duration_value || zero.is_nan() || duration_value.is_nan()) {
            xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(ANIM_OBJ_RESET_SPEED_DURATION_OFFSET)));
        }
    }

    m.write32(ecx.wrapping_add(ANIM_OBJ_RESET_SPEED_TIME_END_OFFSET), xmm0.lanes[0]);
    m.write32(ecx.wrapping_add(ANIM_OBJ_RESET_SPEED_TIME_START_OFFSET), xmm0.lanes[0]);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: [xmm0, xmm1, xmm2, xmm3, xmm4],
        fpu,
        ..entry
    };
}


/// 0x64650. INFERRED: returns word +0xa52 when flag 0x40 else sums words 0xa10 over players with matching +8.
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_score_total_by_team_field_0x8_or_word_0<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x64650;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x64650, previous, pc); previous = pc; }
        match pc {
            0x64650 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(0x7de458)) as u32)) & 0xff);
                eax = 0;
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0x40i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x64669; continue; }
                pc = 0x6465d; continue;
            }
            0x6465d => {
                eax = m.read32(esp.wrapping_add(0x4));
                eax = (((((m.read16(eax.wrapping_add(0xa52)) as i16)) as i32)) as u32);
                break 'run;
            }
            0x64669 => {
                ecx = m.read32(0x790950);
                edx = m.read32(0x7a2958);
                edx = edx.wrapping_add(ecx);
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas <= fbs) { pc = 0x6469f; continue; }
                pc = 0x6467b; continue;
            }
            0x6467b => {
                ecx = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(ecx.wrapping_add(0x8));
                push!(esi);
                esi = m.read32(0x7356d8);
                ecx = esi.wrapping_add(ecx.wrapping_mul(0x2)).wrapping_add(0xa10);
                pc = 0x64690; continue;
            }
            0x64690 => {
                esi = (((((m.read16(ecx) as i16)) as i32)) as u32);
                eax = eax.wrapping_add(esi);
                ecx = ecx.wrapping_add(0xea0);
                edx = edx.wrapping_sub(1);
                fa = edx;
                if ((fa != 0)) { pc = 0x64690; continue; }
                pc = 0x6469e; continue;
            }
            0x6469e => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                pc = 0x6469f; continue;
            }
            0x6469f => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const PLAYER_GET_SIGNED_WORD_ARG_OFFSET: u32 = 0x4;
const PLAYER_SIGNED_WORD_FIELD_OFFSET: u32 = 0xa60;

/// 0x646a0. INFERRED: returns the sign-extended word at +0xa60 of the argument.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn player_get_signed_word_0xa60<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = entry.eax;
    let ecx = entry.ecx;
    let edx = entry.edx;
    let esp = entry.esp;
    let ebx = entry.ebx;
    let ebp = entry.ebp;
    let esi = entry.esi;
    let edi = entry.edi;
    let xmm = entry.xmm;
    let fpu = entry.fpu;

    let argument = m.read32(esp.wrapping_add(PLAYER_GET_SIGNED_WORD_ARG_OFFSET));
    let value = m.read16(argument.wrapping_add(PLAYER_SIGNED_WORD_FIELD_OFFSET)) as i16;
    eax = value as i32 as u32;

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        ..entry
    };
}


const ANIM_STATE_TYPE_TABLE_BASE: u32 = 0x7e03c0;
const ANIM_STATE_TYPE_ENTRY_SIZE: u32 = 2;
const ANIM_STATE_FIRST_ARG_OFFSET: u32 = 0x4;
const ANIM_STATE_SECOND_ARG_OFFSET: u32 = 0x8;
const ANIM_STATE_TYPE_ARG_OFFSET: u32 = 0xc;
const ANIM_STATE_SPEED_ARG_OFFSET: u32 = 0x10;
const ANIM_STATE_FIELD_4_OFFSET: u32 = 0x4;
const ANIM_STATE_FIELD_8_OFFSET: u32 = 0x8;
const ANIM_STATE_FIELD_C_OFFSET: u32 = 0xc;
const ANIM_STATE_FIELD_10_OFFSET: u32 = 0x10;
const ANIM_STATE_FIELD_14_OFFSET: u32 = 0x14;
const ANIM_STATE_FIELD_18_OFFSET: u32 = 0x18;
const ANIM_STATE_FIELD_1C_OFFSET: u32 = 0x1c;
const ANIM_STATE_FIELD_20_OFFSET: u32 = 0x20;
const ANIM_STATE_FIELD_24_OFFSET: u32 = 0x24;
const ANIM_STATE_FIELD_28_OFFSET: u32 = 0x28;
const ANIM_STATE_FIELD_2C_OFFSET: u32 = 0x2c;
const ANIM_STATE_FIELD_30_OFFSET: u32 = 0x30;
const ANIM_STATE_SPEED_OFFSET: u32 = 0x34;
const ANIM_STATE_FIELD_50_OFFSET: u32 = 0x50;
const ANIM_STATE_FIELD_54_OFFSET: u32 = 0x54;

/// 0x8EA20. INFERRED: initializes animation state fields using a type lookup and speed argument.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_state_struct_init_type_and_speed_from_tab<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, xmm2, xmm3, xmm4) =
        (entry.xmm[0], entry.xmm[1], entry.xmm[2], entry.xmm[3], entry.xmm[4]);

    eax = m.read32(esp.wrapping_add(ANIM_STATE_FIRST_ARG_OFFSET));
    xmm0 = Xmm::default();
    ecx = m.read32(esp.wrapping_add(ANIM_STATE_TYPE_ARG_OFFSET));
    xmm1 = Xmm::scalar(m.read32(esp.wrapping_add(ANIM_STATE_SPEED_ARG_OFFSET)));

    m.write32(eax, ecx);
    ecx = (m.read16(
        ecx.wrapping_mul(ANIM_STATE_TYPE_ENTRY_SIZE)
            .wrapping_add(ANIM_STATE_TYPE_TABLE_BASE),
    ) as i16 as i32) as u32;
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_4_OFFSET), ecx);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_8_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_C_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_SPEED_OFFSET), xmm1.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_28_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_2C_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_30_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_10_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_14_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_18_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_1C_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_20_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_24_OFFSET), xmm0.lanes[0]);
    ecx = 0;
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_54_OFFSET), ecx);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_50_OFFSET), ecx);

    eax = m.read32(esp.wrapping_add(ANIM_STATE_SECOND_ARG_OFFSET));
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_54_OFFSET), ecx);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_28_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_2C_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_30_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_10_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_14_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_18_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_1C_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_20_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_24_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(ANIM_STATE_FIELD_50_OFFSET), ecx);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: [xmm0, xmm1, xmm2, xmm3, xmm4],
        fpu,
        ..entry
    };
}


/// 0x949C0. INFERRED: walks 10 byte entries summing byte at +2 rounded up to multiple of 4 until sign byte negative
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_track_list_sum_entry_sizes_rounded_up_to<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x949c0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x949c0, previous, pc); previous = pc; }
        match pc {
            0x949c0 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                eax = 0;
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x949f0; continue; }
                pc = 0x949ca; continue;
            }
            0x949ca => {
                edx = (edx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x3))) as u32)) & 0xff);
                fa = ((((((edx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                fas = (((((fa)) as i8)) as i32);
                fbs = (((((fb)) as i8)) as i32);
                if (fas < fbs) { pc = 0x949f0; continue; }
                pc = 0x949d1; continue;
            }
            0x949d1 => {
                edx = ecx.wrapping_add(0x2);
                pc = 0x949d4; continue;
            }
            0x949d4 => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(0xb))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                fas = (((((fa)) as i8)) as i32);
                fbs = (((((fb)) as i8)) as i32);
                ecx = ((m.read8(edx)) as u8 as u32);
                if (fas < fbs) { pc = 0x949e4; continue; }
                pc = 0x949de; continue;
            }
            0x949de => {
                ecx = ecx.wrapping_add(0x3);
                ecx = (ecx & 0xfffffffcu32);
                pc = 0x949e4; continue;
            }
            0x949e4 => {
                edx = edx.wrapping_add(0xa);
                eax = eax.wrapping_add(ecx);
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(edx.wrapping_add(0x1))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                fas = (((((fa)) as i8)) as i32);
                fbs = (((((fb)) as i8)) as i32);
                if (fas >= fbs) { pc = 0x949d4; continue; }
                pc = 0x949f0; continue;
            }
            0x949f0 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const PLAYER_SLOTS_BASE_ADDRESS: u32 = 0x7b0ce4;

/// 0xc6140. INFERRED: returns the base address of the player slots array.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn player_slots_base_get<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let mut result = *regs;
    result.eax = m.read32(PLAYER_SLOTS_BASE_ADDRESS);
    *regs = result;
}


const PLAYER_FIELD_0X218_OFFSET: u32 = 0x218;
const PLAYER_FLAGS_0X21C_OFFSET: u32 = 0x21c;
const SPLIT_SCREEN_GLOBAL_ADDRESS: u32 = 0x7b0ce8;
const FIELD_ARGUMENT_STACK_OFFSET: u32 = 0x4;
const VALUE_ARGUMENT_STACK_OFFSET: u32 = 0x8;
const PLAYER_FLAG_1: u32 = 0x1;
const PLAYER_FLAG_4: u32 = 0x4;

/// 0xC63F0. INFERRED: sets the player's field at +0x218 and updates flag bits at +0x21c based on the split-screen global and argument.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_set_field_0x218_and_update_flags_0x21c<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, mut esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(FIELD_ARGUMENT_STACK_OFFSET));
    ecx = m.read32(eax.wrapping_add(PLAYER_FLAGS_0X21C_OFFSET));
    edx = m.read32(esp.wrapping_add(VALUE_ARGUMENT_STACK_OFFSET));
    ecx |= PLAYER_FLAG_1;

    esp = esp.wrapping_sub(4);
    m.write32(esp, esi);
    m.write32(eax.wrapping_add(PLAYER_FIELD_0X218_OFFSET), edx);
    m.write32(eax.wrapping_add(PLAYER_FLAGS_0X21C_OFFSET), ecx);

    esi = m.read32(SPLIT_SCREEN_GLOBAL_ADDRESS);
    esi = m.read32(esp);
    esp = esp.wrapping_add(4);

    if m.read32(SPLIT_SCREEN_GLOBAL_ADDRESS) == 0 && edx == 0 {
        ecx |= PLAYER_FLAG_4;
    } else {
        ecx &= !PLAYER_FLAG_4;
    }
    m.write32(eax.wrapping_add(PLAYER_FLAGS_0X21C_OFFSET), ecx);

    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


/// 0xC7950. INFERRED: zeroes the six floats 0x7330a0..0x7330a8 and 0x733094..0x73309c and sets float +0x3b8 of each object of the array 0x7b0ce4 (count 0x7b0ce0) to 1.0
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_slots_reset_field_0x3b8_to_one_and_clea<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xc7950;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0xc7950, previous, pc); previous = pc; }
        match pc {
            0xc7950 => {
                xmm0 = Xmm::default();
                eax = m.read32(0x7b0ce0);
                ecx = 0;
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(0x7330a0, xmm0.lanes[0]);
                m.write32(0x7330a4, xmm0.lanes[0]);
                m.write32(0x7330a8, xmm0.lanes[0]);
                m.write32(0x733094, xmm0.lanes[0]);
                m.write32(0x733098, xmm0.lanes[0]);
                m.write32(0x73309c, xmm0.lanes[0]);
                if (fas <= fbs) { pc = 0xc79bf; continue; }
                pc = 0xc798e; continue;
            }
            0xc798e => {
                xmm0 = Xmm::scalar(m.read32(0x475c78));
                eax = 0;
                pc = 0xc79a0; continue;
            }
            0xc79a0 => {
                edx = m.read32(0x7b0ce4);
                m.write32(eax.wrapping_add(edx).wrapping_add(0x3b8), xmm0.lanes[0]);
                edx = m.read32(0x7b0ce0);
                ecx = ecx.wrapping_add(1);
                eax = eax.wrapping_add(0xffc);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0xc79a0; continue; }
                pc = 0xc79bf; continue;
            }
            0xc79bf => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, ..entry };
}


const PLAYER_SLOT_POINTER_ADDRESS: u32 = 0x733108;
const PLAYER_SLOT_FLAGS_OFFSET: u32 = 0x21c;
const FIRST_ARGUMENT_STACK_OFFSET: u32 = 0x4;
const SECOND_ARGUMENT_STACK_OFFSET: u32 = 0x8;

/// 0xC9BE0. INFERRED: Updates the player slot flags using a clear mask and a set mask.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn player_slot_733108_modify_slot_flags_0x21c<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(PLAYER_SLOT_POINTER_ADDRESS);
    let clear_mask = m.read32(esp.wrapping_add(FIRST_ARGUMENT_STACK_OFFSET));
    edx = m.read32(eax.wrapping_add(PLAYER_SLOT_FLAGS_OFFSET));
    ecx = !clear_mask;
    ecx = ecx & edx;
    ecx |= m.read32(esp.wrapping_add(SECOND_ARGUMENT_STACK_OFFSET));
    m.write32(eax.wrapping_add(PLAYER_SLOT_FLAGS_OFFSET), ecx);

    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const PLAYER_SLOT_FIELD_0FF8_OFFSET: u32 = 0xff8;
const STACK_ARGUMENT_OFFSET: u32 = 4;

/// 0xc9c00. INFERRED: stores the argument at [[0x733108] + 0xff8].
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn player_slot_733108_set_field_0xff8<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(STACK_ARGUMENT_OFFSET));
    ecx = m.read32(PLAYER_SLOT_733108_ADDRESS);
    m.write32(ecx.wrapping_add(PLAYER_SLOT_FIELD_0FF8_OFFSET), eax);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        ..entry
    };
}


const PLAYER_SLOT_FLOAT_0X384_OFFSET: u32 = 0x384;
const PLAYER_SLOT_FLOAT_ARGUMENT_OFFSET: u32 = 0x4;

/// 0xC9CC0. INFERRED: stores float arg at [[0x733108]+0x384]
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_slot_733108_set_float_0x384<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(PLAYER_SLOT_733108_ADDRESS);
    xmm[0] = Xmm::scalar(m.read32(esp.wrapping_add(PLAYER_SLOT_FLOAT_ARGUMENT_OFFSET)));
    m.write32(
        eax.wrapping_add(PLAYER_SLOT_FLOAT_0X384_OFFSET),
        xmm[0].lanes[0],
    );

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        ..entry
    };
}


const PLAYER_SLOT_733108_ADDRESS: u32 = 0x733108;

/// 0xC9F50. INFERRED: returns the dword at 0x733108
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_slot_733108_get<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let eax = m.read32(PLAYER_SLOT_733108_ADDRESS);
    *regs = ExtRegisters { eax, ..entry };
}


/// 0xCFEC0. INFERRED: counts active slots in +0x1030 stride 0xcc (6 slots) down to arg index and returns object or player +0xf78 default
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_get_nth_active_tracked_slot_object_or_d<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xcfec0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0xcfec0, previous, pc); previous = pc; }
        match pc {
            0xcfec0 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                push!(ebp);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x14));
                push!(edi);
                edi = ebx.wrapping_add(0x103c);
                pc = 0xcfed2; continue;
            }
            0xcfed2 => {
                edx = esi;
                ecx = 0;
                eax = edi;
                pc = 0xcfed8; continue;
            }
            0xcfed8 => {
                ebp = m.read32(eax.wrapping_add(((0xci32.wrapping_neg()) as u32)));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xcfee9; continue; }
                pc = 0xcfedf; continue;
            }
            0xcfedf => {
                fa = m.read32(eax);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xcfee9; continue; }
                pc = 0xcfee4; continue;
            }
            0xcfee4 => {
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xcfefc; continue; }
                pc = 0xcfee8; continue;
            }
            0xcfee8 => {
                edx = edx.wrapping_sub(1);
                pc = 0xcfee9; continue;
            }
            0xcfee9 => {
                ecx = ecx.wrapping_add(1);
                eax = eax.wrapping_add(0xcc);
                fa = ecx;
                fb = ((((0x6i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0xcfed8; continue; }
                pc = 0xcfef4; continue;
            }
            0xcfef4 => {
                fa = esi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0xcff0e; continue; }
                pc = 0xcfef8; continue;
            }
            0xcfef8 => {
                esi = 0;
                pc = 0xcfed2; continue;
            }
            0xcfefc => {
                ecx = ((ecx as i32).wrapping_mul((0xcc) as i32)) as u32;
                eax = m.read32(ecx.wrapping_add(ebx).wrapping_add(0x1030));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0xcff0e => {
                eax = m.read32(ebx.wrapping_add(0xf78));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const PLAYER_RECORD_MODE_ADDRESS: u32 = 0x79094c;
const PLAYER_RECORDS_ADDRESS: u32 = 0x7356d8;
const PLAYER_RECORD_COUNT_ADDRESS: u32 = 0x7b0c44;
const PLAYER_RECORD_MODE_CLEAR_FOUR: u32 = 0x65;
const PLAYER_RECORD_STRIDE: u32 = 0xea0;
const PLAYER_RECORDS_NORMAL_COUNT: u32 = 32;
const PLAYER_RECORDS_SPECIAL_COUNT: u32 = 4;
const DIRECTION_FLAG: u32 = 0x400;

/// 0xD6920. INFERRED: zeroes all player records and resets the record count.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_records_clear_all_and_reset_count<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let xmm = entry.xmm;
    let fpu = entry.fpu;

    let mode = m.read32(PLAYER_RECORD_MODE_ADDRESS);
    esp = esp.wrapping_sub(4);
    m.write32(esp, edi);

    edi = m.read32(PLAYER_RECORDS_ADDRESS);
    eax = 0;
    let record_count = if mode == PLAYER_RECORD_MODE_CLEAR_FOUR {
        PLAYER_RECORDS_SPECIAL_COUNT
    } else {
        PLAYER_RECORDS_NORMAL_COUNT
    };
    ecx = record_count
        .wrapping_mul(PLAYER_RECORD_STRIDE)
        .wrapping_div(4);

    let step = if entry.eflags & DIRECTION_FLAG != 0 {
        4u32.wrapping_neg()
    } else {
        4
    };
    for index in 0..ecx {
        m.write32(edi.wrapping_add(index.wrapping_mul(step)), eax);
    }
    edi = edi.wrapping_add(ecx.wrapping_mul(step));

    ecx = 0;
    m.write32(PLAYER_RECORD_COUNT_ADDRESS, eax);
    edi = m.read32(esp);
    esp = esp.wrapping_add(4);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        ..entry
    };
}


const PLAYER_SLOT_ENTRY_SIZE: u32 = 0x1d8;
const PLAYER_SLOT_TABLE_BASE: u32 = 0x73ae08;

/// 0xF8AD0. INFERRED: returns the address of a player slot entry.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn player_slot_table_73ae08_get_entry<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = m.read32(entry.esp.wrapping_add(0x4));
    let ecx = entry.ecx;
    let edx = entry.edx;
    let esp = entry.esp;
    let ebx = entry.ebx;
    let ebp = entry.ebp;
    let esi = entry.esi;
    let edi = entry.edi;
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = ((eax as i32).wrapping_mul(PLAYER_SLOT_ENTRY_SIZE as i32)) as u32;
    eax = eax.wrapping_add(PLAYER_SLOT_TABLE_BASE);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm: x,
        fpu,
        ..entry
    };
}


const PLAYER_SLOT_TABLE_POINTER_ADDRESS: u32 = 0x7b0cc0;
const PLAYER_SLOT_TABLE_CURRENT_INDEX_OFFSET: u32 = 0x214;
const PLAYER_SLOT_TABLE_ENTRIES_BASE: u32 = 0x73ae08;
const PLAYER_SLOT_TABLE_ENTRY_SIZE: u32 = 0x1d8;

/// 0xF8AE0. INFERRED: returns the address of the current player slot table entry.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn player_slot_table_73ae08_get_current_slot_entr<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = m.read32(PLAYER_SLOT_TABLE_POINTER_ADDRESS);
    let current_index = m.read32(eax.wrapping_add(PLAYER_SLOT_TABLE_CURRENT_INDEX_OFFSET));
    eax = PLAYER_SLOT_TABLE_ENTRIES_BASE
        .wrapping_add(current_index.wrapping_mul(PLAYER_SLOT_TABLE_ENTRY_SIZE));

    *regs = ExtRegisters {
        eax,
        ecx: entry.ecx,
        edx: entry.edx,
        esp: entry.esp,
        ebx: entry.ebx,
        ebp: entry.ebp,
        esi: entry.esi,
        edi: entry.edi,
        xmm: entry.xmm,
        fpu: entry.fpu,
        ..entry
    };
}


const INPUT_BINDING_LIMIT: u32 = 0x0e;
const PLAYER_BLOCK_BASE_ADDRESS: u32 = 0x7b0c48;
const PLAYER_BLOCK_STRIDE: i32 = 0x561;
const PLAYER_BLOCK_FIELD_OFFSET: u32 = 0x18;
const LOCAL_FRAME_SIZE: u32 = 0x38;
const PLAYER_ARGUMENT_OFFSET: u32 = 0x40;
const TABLE_INDEX_SCALE: u32 = 0x4;
const BUTTON_TABLE_BASE_OFFSET: u32 = 0x3c;

const BUTTON_MASK_0: u32 = 0x1;
const BUTTON_MASK_1: u32 = 0x2;
const BUTTON_MASK_2: u32 = 0x4;
const BUTTON_MASK_3: u32 = 0x8;
const BUTTON_MASK_4: u32 = 0x10;
const BUTTON_MASK_5: u32 = 0x20;
const BUTTON_MASK_6: u32 = 0x40;
const BUTTON_MASK_7: u32 = 0x80;
const BUTTON_MASK_8: u32 = 0x8000;
const BUTTON_MASK_9: u32 = 0x400;
const BUTTON_MASK_10: u32 = 0x4000;
const BUTTON_MASK_11: u32 = 0x800;
const BUTTON_MASK_12: u32 = 0xc;
const BUTTON_MASK_13: u32 = 0;

/// 0x147080. INFERRED: Returns a player's input binding below index 0xe, otherwise the corresponding fixed button mask.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_input_binding_or_button_mask_get<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(0x4));
    esp = esp.wrapping_sub(LOCAL_FRAME_SIZE);

    if (eax as i32) < INPUT_BINDING_LIMIT as i32 {
        ecx = m.read32(esp.wrapping_add(PLAYER_ARGUMENT_OFFSET));
        edx = m.read32(PLAYER_BLOCK_BASE_ADDRESS);
        ecx = ((ecx as i32).wrapping_mul(PLAYER_BLOCK_STRIDE)) as u32;
        ecx = ecx.wrapping_add(eax);
        eax = m.read32(
            edx.wrapping_add(ecx.wrapping_mul(TABLE_INDEX_SCALE))
                .wrapping_add(PLAYER_BLOCK_FIELD_OFFSET),
        );
        esp = esp.wrapping_add(LOCAL_FRAME_SIZE);
    } else {
        m.write32(esp, BUTTON_MASK_0);
        m.write32(esp.wrapping_add(0x4), BUTTON_MASK_1);
        m.write32(esp.wrapping_add(0x8), BUTTON_MASK_2);
        m.write32(esp.wrapping_add(0xc), BUTTON_MASK_3);
        m.write32(esp.wrapping_add(0x10), BUTTON_MASK_4);
        m.write32(esp.wrapping_add(0x14), BUTTON_MASK_5);
        m.write32(esp.wrapping_add(0x18), BUTTON_MASK_6);
        m.write32(esp.wrapping_add(0x1c), BUTTON_MASK_7);
        m.write32(esp.wrapping_add(0x20), BUTTON_MASK_8);
        m.write32(esp.wrapping_add(0x24), BUTTON_MASK_9);
        m.write32(esp.wrapping_add(0x28), BUTTON_MASK_10);
        m.write32(esp.wrapping_add(0x2c), BUTTON_MASK_11);
        m.write32(esp.wrapping_add(0x30), BUTTON_MASK_12);
        m.write32(esp.wrapping_add(0x34), BUTTON_MASK_13);
        eax = m.read32(
            esp.wrapping_add(eax.wrapping_mul(TABLE_INDEX_SCALE))
                .wrapping_add(BUTTON_TABLE_BASE_OFFSET.wrapping_neg()),
        );
        esp = esp.wrapping_add(LOCAL_FRAME_SIZE);
    }

    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const PLAYER_SLOT_NAME_BASE: u32 = 0x74ed60;
const PLAYER_SLOT_NAME_STRIDE: u32 = 0x1e;

/// 0x190e80. INFERRED: copies a C string into a 30-byte player slot name, or clears the slot when the source pointer is null.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_slot_name_74ed60_set_from_cstring_30_by<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    ecx = ecx.wrapping_mul(PLAYER_SLOT_NAME_STRIDE);

    if eax == 0 {
        m.write8(ecx.wrapping_add(PLAYER_SLOT_NAME_BASE), 0);
    } else {
        edx = ecx.wrapping_add(PLAYER_SLOT_NAME_BASE).wrapping_sub(eax);

        loop {
            ecx = (ecx & 0xffff_ff00) | u32::from(m.read8(eax));
            m.write8(edx.wrapping_add(eax), ecx as u8);
            eax = eax.wrapping_add(1);

            if ecx as u8 == 0 {
                break;
            }
        }
    }

    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


/// 0x22DD90. INFERRED: 76 bytes: arg field +8 below 0x790950 and [arg+0xbf0]+0x54 sign check, state 0x7de470 0..0xe through byte map 0x22dde8 jump table, else -1
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_arg_valid_state_7de470_0_to_0xe_byte_ma<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x22dd90;
    let mut jt = 0u32;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x22dd90, previous, pc); previous = pc; }
        match pc {
            0x22dd90 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x22dda3; continue; }
                pc = 0x22dd98; continue;
            }
            0x22dd98 => {
                ecx = m.read32(eax.wrapping_add(0x8));
                fa = ecx;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x22ddd8; continue; }
                pc = 0x22dda3; continue;
            }
            0x22dda3 => {
                eax = m.read32(eax.wrapping_add(0xbf0));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x22ddb4; continue; }
                pc = 0x22ddad; continue;
            }
            0x22ddad => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(0x54))) as u32)) & 0xff);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                fas = (((((fa)) as i8)) as i32);
                fbs = (((((fb)) as i8)) as i32);
                if (((((((((((((fas)) as u8)) as i32).wrapping_sub((((((fbs)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) != 0i32)) { pc = 0x22ddd8; continue; }
                pc = 0x22ddb4; continue;
            }
            0x22ddb4 => {
                eax = m.read32(0x7de470);
                fa = eax;
                fb = ((((0xei32)) as u32) & 0xffffffffu32);
                if (fa > fb) { pc = 0x22ddd8; continue; }
                pc = 0x22ddbe; continue;
            }
            0x22ddbe => {
                edx = ((m.read8(eax.wrapping_add(0x22dde8))) as u8 as u32);
                jt = m.read32(edx.wrapping_mul(0x4).wrapping_add(0x22dddc));
                if (jt == 0x22ddccu32) { pc = 0x22ddcc; continue; }
                if (jt == 0x22ddd2u32) { pc = 0x22ddd2; continue; }
                if (jt == 0x22ddd8u32) { pc = 0x22ddd8; continue; }
                tail_to!(jt); break 'run;  // indirect tail jump, target outside the switch
            }
            0x22ddcc => {
                eax = 0x5;
                break 'run;
            }
            0x22ddd2 => {
                eax = 0x7;
                break 'run;
            }
            0x22ddd8 => {
                eax = (eax | 0xffffffffu32);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


const INPUT_DRIVEN_OBJECT_75EAF8_ADDRESS: u32 = 0x75eaf8;

/// 0x246D10. INFERRED: Tests whether the input-driven object pointer is null.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn input_driven_object_75eaf8_is_null<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let input_driven_object = m.read32(INPUT_DRIVEN_OBJECT_75EAF8_ADDRESS);
    let eax = u32::from(input_driven_object == 0);

    *regs = ExtRegisters {
        eax,
        ecx: input_driven_object,
        edx: entry.edx,
        esp: entry.esp,
        ebx: entry.ebx,
        ebp: entry.ebp,
        esi: entry.esi,
        edi: entry.edi,
        xmm: entry.xmm,
        fpu: entry.fpu,
        ..entry
    };
}


/// 0x246DC0. INFERRED: revised from game_input_driven_advance_frame_counters_75eae0_75eaec (T1473 rename review T1513): address tail
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_driven_advance_frame_counter_and_step_ta<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x246dc0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x246dc0, previous, pc); previous = pc; }
        match pc {
            0x246dc0 => {
                eax = m.read32(0x75eae0);
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x1ei32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(0x75eae0, eax);
                if (fas < fbs) { pc = 0x246e38; continue; }
                pc = 0x246dd0; continue;
            }
            0x246dd0 => {
                eax = m.read32(0x75eaec);
                ecx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x51acb4));
                eax = eax.wrapping_add(1);
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(0x75eae0, 0);
                m.write32(0x75eaec, eax);
                if (fa != fb) { pc = 0x246df7; continue; }
                pc = 0x246df0; continue;
            }
            0x246df0 => {
                eax = 0;
                m.write32(0x75eaec, eax);
                pc = 0x246df7; continue;
            }
            0x246df7 => {
                ecx = m.read32(0x75eaf0);
                edx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x51ad64));
                ecx = ecx.wrapping_add(1);
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                m.write32(0x75eaf0, ecx);
                if (fa != fb) { pc = 0x246e18; continue; }
                pc = 0x246e10; continue;
            }
            0x246e10 => {
                ecx = 0;
                m.write32(0x75eaf0, ecx);
                pc = 0x246e18; continue;
            }
            0x246e18 => {
                edx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x51acb0));
                ecx = m.read32(ecx.wrapping_mul(0x4).wrapping_add(0x51ad60));
                ecx = (ecx | edx);
                edx = m.read32(esp.wrapping_add(0x4));
                eax = m.read32(edx.wrapping_add(0x4));
                ecx = (ecx | 0x40000);
                m.write32(eax.wrapping_add(0x30), ecx);
                pc = 0x246e38; continue;
            }
            0x246e38 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const INPUT_ENTRIES_BASE: u32 = 0x75eabc;
const INPUT_ENTRIES_END: u32 = 0x75eaec;
const INPUT_FLAG_OFFSET: u32 = 0xffff_fff8;
const INPUT_PAIR_OFFSET: u32 = 0xffff_fffc;
const INPUT_ENTRY_STRIDE: u32 = 0x10;

/// 0x2474e0. INFERRED: Checks that every input entry is enabled and has an equal pair.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_driven_all_entries_flag_and_pair_equal<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    ecx = INPUT_ENTRIES_BASE;
    eax = 1;

    loop {
        let flag = m.read8(ecx.wrapping_add(INPUT_FLAG_OFFSET));
        if flag & (eax as u8) != 0 {
            eax = 0;
            break;
        }

        edx = m.read32(ecx.wrapping_add(INPUT_PAIR_OFFSET));
        let pair_value = m.read32(ecx);
        if edx != pair_value {
            eax = 0;
            break;
        }

        ecx = ecx.wrapping_add(INPUT_ENTRY_STRIDE);
        if (ecx as i32) >= (INPUT_ENTRIES_END as i32) {
            break;
        }
    }

    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


/// 0x258870. INFERRED: INFERRED: copies +0x3c/+0x38/+0x40/+0x44 into +0x4c/+0x48/+0x50/+0x54 and swaps +0x48/+0x50 for layouts (3,2) and (1,4), twin of tail of 2588b0, caller current_player_block_7ba364 dispatch
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_mapping_derive_alt_fields_0x48_to_0x54<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x258870;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x258870, previous, pc); previous = pc; }
        match pc {
            0x258870 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_add(0x3c));
                edx = m.read32(eax.wrapping_add(0x40));
                m.write32(eax.wrapping_add(0x4c), ecx);
                ecx = m.read32(eax.wrapping_add(0x44));
                fa = ecx;
                fb = ((((0x3i32)) as u32) & 0xffffffffu32);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x38));
                m.write32(eax.wrapping_add(0x48), esi);
                m.write32(eax.wrapping_add(0x50), edx);
                m.write32(eax.wrapping_add(0x54), ecx);
                if (fa != fb) { pc = 0x258899; continue; }
                pc = 0x258892; continue;
            }
            0x258892 => {
                fa = edx;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x2588a3; continue; }
                pc = 0x258897; continue;
            }
            0x258897 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x258899 => {
                fa = ecx;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2588a9; continue; }
                pc = 0x25889e; continue;
            }
            0x25889e => {
                fa = edx;
                fb = ((((0x4i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x2588a9; continue; }
                pc = 0x2588a3; continue;
            }
            0x2588a3 => {
                m.write32(eax.wrapping_add(0x50), esi);
                m.write32(eax.wrapping_add(0x48), edx);
                pc = 0x2588a9; continue;
            }
            0x2588a9 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const PLAYER_7B0C7C_ADDRESS: u32 = 0x7b0c7c;
const PLAYER_FLAGS_OFFSET: u32 = 0xc;
const PLAYER_STATE_OFFSET: u32 = 0x114;
const PLAYER_FLAG_F5C_OFFSET: u32 = 0xf5c;
const PLAYER_FLAG_MASK: u32 = 0x20;
const PLAYER_STATE_MASK: u32 = 0x650;

/// 0x25a080. INFERRED: Clears the player's flag at +0xf5c, then sets it when flag +0xc is 0x20 and state +0x114 has none of the bits in 0x650.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn player_7b0c7c_set_flag_f5c_when_flag_0x20_and<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(PLAYER_7B0C7C_ADDRESS);
    m.write32(eax.wrapping_add(PLAYER_FLAG_F5C_OFFSET), 0);

    eax = m.read32(PLAYER_7B0C7C_ADDRESS);
    let player_flags = m.read8(eax.wrapping_add(PLAYER_FLAGS_OFFSET)) as u32;
    if player_flags & PLAYER_FLAG_MASK != 0 {
        let player_state = m.read32(eax.wrapping_add(PLAYER_STATE_OFFSET));
        if player_state & PLAYER_STATE_MASK == 0 {
            m.write32(eax.wrapping_add(PLAYER_FLAG_F5C_OFFSET), 1);
        }
    }

    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


const PLAYER_POINTER_ADDRESS: u32 = 0x7b0c7c;
const PLAYER_OBJECT_OFFSET: u32 = 0x14;
const PLAYER_EXTENSION_OFFSET: u32 = 0x7c;
const EXTENSION_FIELD_OFFSET: u32 = 0x678;
const PLAYER_HISTORY_CURRENT_OFFSET: u32 = 0x155c;
const PLAYER_HISTORY_PREVIOUS_OFFSET: u32 = 0x1560;
const PLAYER_HISTORY_OLDER_OFFSET: u32 = 0x1564;

/// 0x25b720. INFERRED: Shifts the player's history fields when the extension field changes.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn player_7b0c7c_shift_history_1564_1560_155c_whe<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let xmm = entry.xmm;
    let fpu = entry.fpu;

    let mut player = m.read32(PLAYER_POINTER_ADDRESS);
    let mut extension = m.read32(player.wrapping_add(PLAYER_OBJECT_OFFSET));
    extension = m.read32(extension.wrapping_add(PLAYER_EXTENSION_OFFSET));
    edx = m.read32(extension.wrapping_add(EXTENSION_FIELD_OFFSET));
    let current_history = m.read32(player.wrapping_add(PLAYER_HISTORY_CURRENT_OFFSET));

    if edx != current_history {
        edx = m.read32(player.wrapping_add(PLAYER_HISTORY_PREVIOUS_OFFSET));
        m.write32(player.wrapping_add(PLAYER_HISTORY_OLDER_OFFSET), edx);

        player = m.read32(PLAYER_POINTER_ADDRESS);
        edx = m.read32(player.wrapping_add(PLAYER_HISTORY_CURRENT_OFFSET));
        m.write32(player.wrapping_add(PLAYER_HISTORY_PREVIOUS_OFFSET), edx);

        eax = m.read32(extension.wrapping_add(EXTENSION_FIELD_OFFSET));
        ecx = m.read32(PLAYER_POINTER_ADDRESS);
        m.write32(ecx.wrapping_add(PLAYER_HISTORY_CURRENT_OFFSET), eax);
    } else {
        eax = player;
        ecx = extension;
    }

    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm, fpu, ..entry };
}


/// 0x2E86A0. INFERRED: zeros mapping words 7ba3e0.. for current player (stride 0x78 dwords) and 7ba414-7ba434 where table 5249a0 entry is not -1, caller 2e8840
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_mapping_clear_current_player_bindings<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2e86a0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2e86a0, previous, pc); previous = pc; }
        match pc {
            0x2e86a0 => {
                eax = m.read32(0x7ba9e0);
                eax = ((eax as i32).wrapping_mul((0x78) as i32)) as u32;
                ecx = 0;
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba3e0), ecx);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba3e4), ecx);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba3e8), ecx);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba3ec), ecx);
                push!(esi);
                esi = m.read32(0x5249a0);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba3f0), ecx);
                edx = (edx | 0xffffffffu32);
                fa = esi;
                fb = edx;
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba3f4), ecx);
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba414), ecx);
                if (fa == fb) { pc = 0x2e86f0; continue; }
                pc = 0x2e86e9; continue;
            }
            0x2e86e9 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba418), ecx);
                pc = 0x2e86f0; continue;
            }
            0x2e86f0 => {
                fa = m.read32(0x5249ac);
                fb = edx;
                if (fa == fb) { pc = 0x2e86ff; continue; }
                pc = 0x2e86f8; continue;
            }
            0x2e86f8 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba41c), ecx);
                pc = 0x2e86ff; continue;
            }
            0x2e86ff => {
                fa = m.read32(0x5249b8);
                fb = edx;
                if (fa == fb) { pc = 0x2e870e; continue; }
                pc = 0x2e8707; continue;
            }
            0x2e8707 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba420), ecx);
                pc = 0x2e870e; continue;
            }
            0x2e870e => {
                fa = m.read32(0x5249c4);
                fb = edx;
                if (fa == fb) { pc = 0x2e871d; continue; }
                pc = 0x2e8716; continue;
            }
            0x2e8716 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba424), ecx);
                pc = 0x2e871d; continue;
            }
            0x2e871d => {
                fa = m.read32(0x5249d0);
                fb = edx;
                if (fa == fb) { pc = 0x2e872c; continue; }
                pc = 0x2e8725; continue;
            }
            0x2e8725 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba428), ecx);
                pc = 0x2e872c; continue;
            }
            0x2e872c => {
                fa = m.read32(0x5249dc);
                fb = edx;
                if (fa == fb) { pc = 0x2e873b; continue; }
                pc = 0x2e8734; continue;
            }
            0x2e8734 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba42c), ecx);
                pc = 0x2e873b; continue;
            }
            0x2e873b => {
                fa = m.read32(0x5249e8);
                fb = edx;
                if (fa == fb) { pc = 0x2e874a; continue; }
                pc = 0x2e8743; continue;
            }
            0x2e8743 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba430), ecx);
                pc = 0x2e874a; continue;
            }
            0x2e874a => {
                fa = m.read32(0x5249f4);
                fb = edx;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                if (fa == fb) { pc = 0x2e875a; continue; }
                pc = 0x2e8753; continue;
            }
            0x2e8753 => {
                m.write32(eax.wrapping_mul(0x4).wrapping_add(0x7ba434), ecx);
                pc = 0x2e875a; continue;
            }
            0x2e875a => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}


/// 0x2E8C00. INFERRED: fills 0x16 entries of 761bf0 with action index from table 5248d8 (7 candidates) or -1
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_mapping_build_row_to_action_index_table<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2e8c00;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2e8c00, previous, pc); previous = pc; }
        match pc {
            0x2e8c00 => {
                ecx = 0;
                edx = (edx | 0xffffffffu32);
                push!(esi);
                pc = 0x2e8c10; continue;
            }
            0x2e8c10 => {
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x761bf0), edx);
                eax = 0;
                pc = 0x2e8c20; continue;
            }
            0x2e8c20 => {
                esi = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x5248d8));
                eax = eax.wrapping_add(1);
                fa = ecx;
                fb = esi;
                if (fa == fb) { pc = 0x2e8c33; continue; }
                pc = 0x2e8c2c; continue;
            }
            0x2e8c2c => {
                fa = eax;
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa < fb) { pc = 0x2e8c20; continue; }
                pc = 0x2e8c31; continue;
            }
            0x2e8c31 => {
                pc = 0x2e8c3a; continue;
            }
            0x2e8c33 => {
                m.write32(ecx.wrapping_mul(0x4).wrapping_add(0x761bf0), eax);
                pc = 0x2e8c3a; continue;
            }
            0x2e8c3a => {
                ecx = ecx.wrapping_add(1);
                fa = ecx;
                fb = ((((0x16i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2e8c10; continue; }
                pc = 0x2e8c40; continue;
            }
            0x2e8c40 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, ..entry };
}



/// 0x42C50. INFERRED: calls named game_input_devices_init (0x18feb0) then stores one float into +0 of every pad entry
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn input_devices_init_then_pads_set_float_field_0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x42c50;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x42c50, previous, pc); previous = pc; }
        match pc {
            0x42c50 => {
                call!(0x18feb0);
                pc = 0x42c55; continue;
            }
            0x42c55 => {
                xmm0 = Xmm::scalar(m.read32(0x475d4c));
                m.write32(0x6b93d0, xmm0.lanes[0]);
                m.write32(0x6b940c, xmm0.lanes[0]);
                m.write32(0x6b9448, xmm0.lanes[0]);
                m.write32(0x6b9484, xmm0.lanes[0]);
                eax = 0x6b94c8;
                pc = 0x42c82; continue;
            }
            0x42c82 => {
                m.write32(eax, xmm0.lanes[0]);
                eax = eax.wrapping_add(0x3c);
                fa = eax;
                fb = ((((0x6b9888i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x42c82; continue; }
                pc = 0x42c90; continue;
            }
            0x42c90 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}


const NODE_TABLE_ADDRESS: u32 = 0x7e3954;
const NODE_TABLE_ENTRY_SIZE: u32 = 4;
const ARGUMENT_NODE_OFFSET: u32 = 0x4;
const ARGUMENT_TIME_OFFSET: u32 = 0x8;
const ARGUMENT_INDEX_OFFSET: u32 = 0xc;
const NODE_INDEX_FIELD_OFFSET: u32 = 0x4;
const NODE_TIME_FIELD_OFFSET: u32 = 0x8;
const STACK_WORD_SIZE: u32 = 4;
const STACK_CLEANUP_SIZE: u32 = 0x8;
const FIND_KEYFRAME_TIME_BRACKET_ADDRESS: u32 = 0x584a0;

/// 0x585b0. INFERRED: loads a node entry from the table by index and finds its keyframe time bracket.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_track_keyframe_time_bracket_by_node_index<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let fpu = entry.fpu;
    let mut xmm = entry.xmm;
    let mut xmm_hi = entry.xmm_hi;

    eax = m.read32(esp.wrapping_add(ARGUMENT_INDEX_OFFSET));
    ecx = m.read32(esp.wrapping_add(ARGUMENT_TIME_OFFSET));

    esp = esp.wrapping_sub(STACK_WORD_SIZE);
    m.write32(esp, ebx);
    esp = esp.wrapping_sub(STACK_WORD_SIZE);
    m.write32(esp, eax);

    eax = m.read32(esp.wrapping_add(ARGUMENT_INDEX_OFFSET));
    edx = m.read32(eax.wrapping_add(NODE_INDEX_FIELD_OFFSET));
    xmm[2] = Xmm::scalar(m.read32(eax.wrapping_add(NODE_TIME_FIELD_OFFSET)));

    eax = m.read32(NODE_TABLE_ADDRESS);
    ebx = m.read32(
        eax.wrapping_add(edx.wrapping_mul(NODE_TABLE_ENTRY_SIZE)),
    );

    esp = esp.wrapping_sub(STACK_WORD_SIZE);
    m.write32(esp, ecx);

    let mut call_regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        eflags: entry.eflags,
        fpu,
        xmm,
        xmm_hi,
    };
    guest.call(FIND_KEYFRAME_TIME_BRACKET_ADDRESS, &mut call_regs);
    eax = call_regs.eax;
    ecx = call_regs.ecx;
    edx = call_regs.edx;
    esp = call_regs.esp;
    ebx = call_regs.ebx;
    ebp = call_regs.ebp;
    esi = call_regs.esi;
    edi = call_regs.edi;
    xmm = call_regs.xmm;
    xmm_hi = call_regs.xmm_hi;

    esp = esp.wrapping_add(STACK_CLEANUP_SIZE);
    ebx = m.read32(esp);
    esp = esp.wrapping_add(STACK_WORD_SIZE);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        xmm_hi,
        ..entry
    };
}


const ACTIVE_SLOT_ID_HELPER: u32 = 0x59cf0;
const ANIM_TABLE_LOOKUP_HELPER: u32 = 0x56fc0;

/// 0x61430. INFERRED: Passes the active slot id to the animation table lookup and returns whether it has an entry.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn anim_obj_active_slot_anim_has_table_7e1be0_ent<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let mut xmm_hi = entry.xmm_hi;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(4));
    esp = esp.wrapping_sub(4);
    m.write32(esp, eax);

    let mut call_regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        eflags: entry.eflags,
        fpu,
        xmm,
        xmm_hi,
    };
    guest.call(ACTIVE_SLOT_ID_HELPER, &mut call_regs);
    (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
        call_regs.eax,
        call_regs.ecx,
        call_regs.edx,
        call_regs.esp,
        call_regs.ebx,
        call_regs.ebp,
        call_regs.esi,
        call_regs.edi,
    );
    xmm = call_regs.xmm;
    xmm_hi = call_regs.xmm_hi;

    ecx = m.read32(eax);
    esp = esp.wrapping_sub(4);
    m.write32(esp, ecx);

    call_regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        eflags: entry.eflags,
        fpu,
        xmm,
        xmm_hi,
    };
    guest.call(ANIM_TABLE_LOOKUP_HELPER, &mut call_regs);
    (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
        call_regs.eax,
        call_regs.ecx,
        call_regs.edx,
        call_regs.esp,
        call_regs.ebx,
        call_regs.ebp,
        call_regs.esi,
        call_regs.edi,
    );
    xmm = call_regs.xmm;
    xmm_hi = call_regs.xmm_hi;

    esp = esp.wrapping_add(8);
    eax = u32::from(eax != 0);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        xmm_hi,
        ..entry
    };
}


const ACTIVE_SLOT_ROUTINE: u32 = 0x59cf0;
const ACTIVE_SLOT_TIME_ROUTINE: u32 = 0x585e0;
const FIRST_ARGUMENT_OFFSET: u32 = 0x8;
const RESULT_OFFSET: u32 = 0x10;
const CLEANUP_SIZE: u32 = 0x14;
const STACK_SLOT_SIZE_61610: u32 = 4;


/// 0x61610. INFERRED: calls game_object_08_0c_mul_4d1994_trunc_optional_i32_outputs on 59cf0 active slot and returns first int.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_obj_active_slot_time_scaled_by_4d1994_tru<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let mut xmm_hi = entry.xmm_hi;
    let fpu = entry.fpu;

    esp = esp.wrapping_sub(STACK_SLOT_SIZE_61610);
    m.write32(esp, ecx);
    eax = m.read32(esp.wrapping_add(FIRST_ARGUMENT_OFFSET));
    esp = esp.wrapping_sub(STACK_SLOT_SIZE_61610);
    m.write32(esp, eax);

    let mut call_regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        eflags: entry.eflags,
        fpu,
        xmm: xmm,
        xmm_hi,
    };
    guest.call(ACTIVE_SLOT_ROUTINE, &mut call_regs);
    xmm_hi = call_regs.xmm_hi;
    (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
        call_regs.eax,
        call_regs.ecx,
        call_regs.edx,
        call_regs.esp,
        call_regs.ebx,
        call_regs.ebp,
        call_regs.esi,
        call_regs.edi,
    );
    xmm = call_regs.xmm;

    esp = esp.wrapping_sub(STACK_SLOT_SIZE_61610);
    m.write32(esp, 0);
    ecx = esp.wrapping_add(FIRST_ARGUMENT_OFFSET);
    esp = esp.wrapping_sub(STACK_SLOT_SIZE_61610);
    m.write32(esp, ecx);
    esp = esp.wrapping_sub(STACK_SLOT_SIZE_61610);
    m.write32(esp, eax);

    let mut call_regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        eflags: entry.eflags,
        fpu,
        xmm,
        xmm_hi,
    };
    guest.call(ACTIVE_SLOT_TIME_ROUTINE, &mut call_regs);
    xmm_hi = call_regs.xmm_hi;
    (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
        call_regs.eax,
        call_regs.ecx,
        call_regs.edx,
        call_regs.esp,
        call_regs.ebx,
        call_regs.ebp,
        call_regs.esi,
        call_regs.edi,
    );
    xmm = call_regs.xmm;

    eax = m.read32(esp.wrapping_add(RESULT_OFFSET));
    esp = esp.wrapping_add(CLEANUP_SIZE);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        xmm_hi,
        ..entry
    };
}


/// 0x61630. INFERRED: game_anim_obj_active_slot_ptr_0xd4_or_0x12c_by_blend_0x198, key lookup 0x613d0, game_object_08_0c_mul_4d1994_trunc_optional_i32_outputs then returns 1 when the current index is at least the target
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn anim_obj_current_key_index_reached_target_key<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x61630;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x61630, previous, pc); previous = pc; }
        match pc {
            0x61630 => {
                push!(ecx);
                eax = m.read32(esp.wrapping_add(0x8));
                push!(esi);
                push!(edi);
                push!(eax);
                call!(0x59cf0);
                pc = 0x6163d; continue;
            }
            0x6163d => {
                ecx = m.read32(esp.wrapping_add(0x1c));
                edx = m.read32(esp.wrapping_add(0x18));
                push!(ecx);
                esi = eax;
                eax = m.read32(esi);
                push!(edx);
                push!(eax);
                call!(0x613d0);
                pc = 0x61651; continue;
            }
            0x61651 => {
                edi = eax;
                esp = esp.wrapping_add(0x10);
                fa = edi;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x61661; continue; }
                pc = 0x6165b; continue;
            }
            0x6165b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x61661 => {
                push!(0);
                ecx = esp.wrapping_add(0xc);
                push!(ecx);
                push!(esi);
                call!(0x585e0);
                pc = 0x6166e; continue;
            }
            0x6166e => {
                ecx = m.read32(esp.wrapping_add(0x14));
                esp = esp.wrapping_add(0xc);
                eax = 0;
                fa = ecx;
                fb = edi;
                fas = (fa as i32);
                fbs = (fb as i32);
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = (eax & 0xffff_ff00) | ((((if ((fas >= fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x660C0. INFERRED: 9 groups stride 0x48 of three dwords nonzero counter from record+0x694
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_record_count_nonzero_entries_in_9_group<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x660c0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x660c0, previous, pc); previous = pc; }
        match pc {
            0x660c0 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                push!(eax);
                esi = 0;
                call!(0x69900);
                pc = 0x660cd; continue;
            }
            0x660cd => {
                esp = esp.wrapping_add(0x4);
                eax = eax.wrapping_add(0x6ac);
                ecx = 0x9;
                pc = 0x660e0; continue;
            }
            0x660e0 => {
                edx = m.read32(eax.wrapping_add(((0x18i32.wrapping_neg()) as u32)));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x660e8; continue; }
                pc = 0x660e7; continue;
            }
            0x660e7 => {
                esi = esi.wrapping_add(1);
                pc = 0x660e8; continue;
            }
            0x660e8 => {
                fa = m.read32(eax);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x660ee; continue; }
                pc = 0x660ed; continue;
            }
            0x660ed => {
                esi = esi.wrapping_add(1);
                pc = 0x660ee; continue;
            }
            0x660ee => {
                edx = m.read32(eax.wrapping_add(0x18));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x660f6; continue; }
                pc = 0x660f5; continue;
            }
            0x660f5 => {
                esi = esi.wrapping_add(1);
                pc = 0x660f6; continue;
            }
            0x660f6 => {
                eax = eax.wrapping_add(0x48);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x660e0; continue; }
                pc = 0x660fc; continue;
            }
            0x660fc => {
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x66100. INFERRED: 7 groups stride 0x48 of three dwords nonzero counter from record+0x49c
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_record_count_nonzero_entries_in_7_group<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x66100;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x66100, previous, pc); previous = pc; }
        match pc {
            0x66100 => {
                eax = m.read32(esp.wrapping_add(0x4));
                push!(esi);
                push!(eax);
                esi = 0;
                call!(0x69900);
                pc = 0x6610d; continue;
            }
            0x6610d => {
                esp = esp.wrapping_add(0x4);
                eax = eax.wrapping_add(0x4b4);
                ecx = 0x7;
                pc = 0x66120; continue;
            }
            0x66120 => {
                edx = m.read32(eax.wrapping_add(((0x18i32.wrapping_neg()) as u32)));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x66128; continue; }
                pc = 0x66127; continue;
            }
            0x66127 => {
                esi = esi.wrapping_add(1);
                pc = 0x66128; continue;
            }
            0x66128 => {
                fa = m.read32(eax);
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x6612e; continue; }
                pc = 0x6612d; continue;
            }
            0x6612d => {
                esi = esi.wrapping_add(1);
                pc = 0x6612e; continue;
            }
            0x6612e => {
                edx = m.read32(eax.wrapping_add(0x18));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x66136; continue; }
                pc = 0x66135; continue;
            }
            0x66135 => {
                esi = esi.wrapping_add(1);
                pc = 0x66136; continue;
            }
            0x66136 => {
                eax = eax.wrapping_add(0x48);
                ecx = ecx.wrapping_sub(1);
                fa = ecx;
                if ((fa != 0)) { pc = 0x66120; continue; }
                pc = 0x6613c; continue;
            }
            0x6613c => {
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x66220. INFERRED: 13 bit loop testing masks at 0xbf4+arg*4 and 0xc00+arg*4
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_record_count_bits_set_in_either_mask_0x<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x66220;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x66220, previous, pc); previous = pc; }
        match pc {
            0x66220 => {
                push!(ecx);
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0xc));
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x14));
                push!(esi);
                push!(edi);
                edi = 0;
                m.write32(esp.wrapping_add(0x10), edi);
                pc = 0x66233; continue;
            }
            0x66233 => {
                ecx = edi;
                esi = 0x1;
                push!(ebx);
                esi = esi.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                call!(0x69900);
                pc = 0x66242; continue;
            }
            0x66242 => {
                ecx = m.read32(eax.wrapping_add(ebp.wrapping_mul(0x4)).wrapping_add(0xbf4));
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ecx;
                if ((fa & fb) != 0) { pc = 0x66264; continue; }
                pc = 0x66250; continue;
            }
            0x66250 => {
                push!(ebx);
                call!(0x69900);
                pc = 0x66256; continue;
            }
            0x66256 => {
                ecx = m.read32(eax.wrapping_add(ebp.wrapping_mul(0x4)).wrapping_add(0xc00));
                esp = esp.wrapping_add(0x4);
                fa = esi;
                fb = ecx;
                if ((fa & fb) == 0) { pc = 0x66268; continue; }
                pc = 0x66264; continue;
            }
            0x66264 => {
                m.write32(esp.wrapping_add(0x10), m.read32(esp.wrapping_add(0x10)).wrapping_add(0x1));
                pc = 0x66268; continue;
            }
            0x66268 => {
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ((((0xdi32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x66233; continue; }
                pc = 0x6626e; continue;
            }
            0x6626e => {
                eax = m.read32(esp.wrapping_add(0x10));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ecx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x115F10. INFERRED: id range 0xca..0xcd maps through 356800 to player 7b0c48 stride 0x1584, returns pointer 0x14 field 0x34
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_by_id_0xca_to_0xcd_get_field_0x14_0x34<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x115f10;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x115f10, previous, pc); previous = pc; }
        match pc {
            0x115f10 => {
                eax = m.read32(esp.wrapping_add(0x4));
                fa = eax;
                fb = ((((0xcai32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x115f44; continue; }
                pc = 0x115f1b; continue;
            }
            0x115f1b => {
                fa = eax;
                fb = ((((0xcei32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x115f44; continue; }
                pc = 0x115f22; continue;
            }
            0x115f22 => {
                eax = eax.wrapping_add(0xffffff36u32);
                push!(eax);
                call!(0x356800);
                pc = 0x115f2d; continue;
            }
            0x115f2d => {
                eax = ((eax as i32).wrapping_mul((0x1584) as i32)) as u32;
                ecx = m.read32(0x7b0c48);
                esp = esp.wrapping_add(0x4);
                eax = eax.wrapping_add(ecx);
                eax = m.read32(eax.wrapping_add(0x14));
                eax = m.read32(eax.wrapping_add(0x34));
                pc = 0x115f44; continue;
            }
            0x115f44 => {
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x259800. INFERRED: INFERRED: ext +0x7c nonzero, callee game_check_byte_7de455_is_0xa_and_mode_not_0x64_0x38 false, +0x14f8 == 0x11, caller game_player_lock_on_target_update
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_lock_on_allowed_when_action_0x11_and_ex<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x259800;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x259800, previous, pc); previous = pc; }
        match pc {
            0x259800 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(esi.wrapping_add(0x14));
                ecx = m.read32(eax.wrapping_add(0x7c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x259828; continue; }
                pc = 0x25980f; continue;
            }
            0x25980f => {
                call!(0x64d10);
                pc = 0x259814; continue;
            }
            0x259814 => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x259828; continue; }
                pc = 0x259818; continue;
            }
            0x259818 => {
                fa = m.read32(esi.wrapping_add(0x14f8));
                fb = ((((0x11i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x259828; continue; }
                pc = 0x259821; continue;
            }
            0x259821 => {
                eax = 0x1;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x259828 => {
                eax = 0;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x25EF30. INFERRED: Indexes record 75f5d0[player 7b0c7c field0] and returns 2 when bit (arg) of record field4 is set, else 0 or 1 from game_return_constant_1 unless bit 0x10000 of 7de45c is set
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_7b0c7c_record_238_field4_bit_test_retur<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x25ef30;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x25ef30, previous, pc); previous = pc; }
        match pc {
            0x25ef30 => {
                eax = m.read32(0x7b0c7c);
                edx = m.read32(eax);
                ecx = m.read32(esp.wrapping_add(0x4));
                edx = ((edx as i32).wrapping_mul((0x238) as i32)) as u32;
                eax = 0x1;
                eax = eax.wrapping_shl((((((((((ecx) as u8))) as u32) & 0x1fu32))) as u32 & 31));
                ecx = m.read32(0x75f5d0);
                fa = m.read32(edx.wrapping_add(ecx).wrapping_add(0x4));
                fb = eax;
                if ((fa & fb) == 0) { pc = 0x25ef5a; continue; }
                pc = 0x25ef54; continue;
            }
            0x25ef54 => {
                eax = 0x2;
                break 'run;
            }
            0x25ef5a => {
                fa = m.read32(0x7de45c);
                fb = ((((0x10000i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) != 0) { pc = 0x25ef72; continue; }
                pc = 0x25ef66; continue;
            }
            0x25ef66 => {
                call!(0x35de60);
                pc = 0x25ef6b; continue;
            }
            0x25ef6b => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x25ef72; continue; }
                pc = 0x25ef6f; continue;
            }
            0x25ef6f => {
                eax = 0;
                break 'run;
            }
            0x25ef72 => {
                eax = 0x1;
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


const CURRENT_EFFECT_ARGUMENT_OFFSET: u32 = 0x4;
const CURRENT_EFFECT_TABLE_OFFSET: u32 = 0xf54;
const CURRENT_EFFECT_FLAG_SHIFT: u32 = 0x19;
const CURRENT_EFFECT_TABLE_LOOKUP_CALL: u32 = 0x83970;
const STACK_SLOT_SIZE: u32 = 0x4;

/// 0x3381D0. INFERRED: Looks up the current effect entry and tests whether flag bit 25 is clear.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_current_effect_entry_has_flag_bit_25_cl<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let mut xmm_hi = entry.xmm_hi;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(CURRENT_EFFECT_ARGUMENT_OFFSET));
    eax = m.read32(eax.wrapping_add(CURRENT_EFFECT_TABLE_OFFSET));
    eax = eax.wrapping_sub(1);

    esp = esp.wrapping_sub(STACK_SLOT_SIZE);
    m.write32(esp, eax);

    let mut call_regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        eflags: entry.eflags,
        fpu,
        xmm,
        xmm_hi,
    };
    guest.call(CURRENT_EFFECT_TABLE_LOOKUP_CALL, &mut call_regs);
    (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
        call_regs.eax,
        call_regs.ecx,
        call_regs.edx,
        call_regs.esp,
        call_regs.ebx,
        call_regs.ebp,
        call_regs.esi,
        call_regs.edi,
    );
    xmm = call_regs.xmm;
    xmm_hi = call_regs.xmm_hi;

    eax = eax.wrapping_shr(CURRENT_EFFECT_FLAG_SHIFT) & 1;
    ecx = u32::from(eax == 0);
    esp = esp.wrapping_add(STACK_SLOT_SIZE);
    eax = ecx;

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        xmm_hi,
        ..entry
    };
}


/// 0x2CE120. INFERRED: loops 0x3b bytes at stats record +0xbb4 (record via game_profile_stats_record_ptr_by_slot_default with 78ad34) counting nonzero
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_stats_count_set_flags_in_59_byte_array<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2ce120;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: x, xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        x = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    let _ = &mut x;
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2ce120, previous, pc); previous = pc; }
        match pc {
            0x2ce120 => {
                push!(esi);
                push!(edi);
                edi = 0;
                esi = 0;
                pc = 0x2ce130; continue;
            }
            0x2ce130 => {
                eax = m.read32(0x78ad34);
                push!(eax);
                call!(0x69900);
                pc = 0x2ce13b; continue;
            }
            0x2ce13b => {
                ecx = (ecx & 0xffff_ff00) | ((((m.read8(eax.wrapping_add(esi).wrapping_add(0xbb4))) as u32)) & 0xff);
                esp = esp.wrapping_add(0x4);
                fa = ((((((ecx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x2ce14a; continue; }
                pc = 0x2ce149; continue;
            }
            0x2ce149 => {
                edi = edi.wrapping_add(1);
                pc = 0x2ce14a; continue;
            }
            0x2ce14a => {
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0x3bi32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x2ce130; continue; }
                pc = 0x2ce150; continue;
            }
            0x2ce150 => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


const EFFECT_FLOAT_ONE_BITS: u32 = 0x3f800000;
const EFFECT_FLOAT_CALL_TARGET: u32 = 0x1252c0;
const EFFECT_FLOAT_ARG1_OFFSET: u32 = 0x8;
const EFFECT_FLOAT_ARG2_OFFSET: u32 = 0x4;
const EFFECT_FLOAT_STACK_CLEANUP: u32 = 0xc;

/// 0x126260. INFERRED: Calls the player effect routine with a float value of 1.0 and an object reference.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn player_apply_effect_float_and_object_ref_with<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let mut xmm_hi = entry.xmm_hi;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(EFFECT_FLOAT_ARG1_OFFSET));
    ecx = m.read32(esp.wrapping_add(EFFECT_FLOAT_ARG2_OFFSET));

    esp = esp.wrapping_sub(4);
    m.write32(esp, eax);
    esp = esp.wrapping_sub(4);
    m.write32(esp, EFFECT_FLOAT_ONE_BITS);
    esp = esp.wrapping_sub(4);
    m.write32(esp, ecx);

    let mut call_regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        eflags: entry.eflags,
        fpu,
        xmm,
        xmm_hi,
    };
    guest.call(EFFECT_FLOAT_CALL_TARGET, &mut call_regs);
    xmm_hi = call_regs.xmm_hi;
    (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (
        call_regs.eax,
        call_regs.ecx,
        call_regs.edx,
        call_regs.esp,
        call_regs.ebx,
        call_regs.ebp,
        call_regs.esi,
        call_regs.edi,
    );
    xmm = call_regs.xmm;

    esp = esp.wrapping_add(EFFECT_FLOAT_STACK_CLEANUP);

    *regs = ExtRegisters {
        eax,
        ecx,
        edx,
        esp,
        ebx,
        ebp,
        esi,
        edi,
        xmm,
        fpu,
        xmm_hi,
        ..entry
    };
}


/// 0x238460. INFERRED: revised from game_player_stat_set_float_75ccc0_stride_0x273_if_player_valid_and_state_7de30c_is_7, per player stat block set by index
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_round_stat_set_float<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x238460;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x238460, previous, pc); previous = pc; }
        match pc {
            0x238460 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                call!(0x2382b0);
                pc = 0x23846a; continue;
            }
            0x23846a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x238493; continue; }
                pc = 0x23846e; continue;
            }
            0x23846e => {
                fa = m.read32(0x7de30c);
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x238493; continue; }
                pc = 0x238477; continue;
            }
            0x238477 => {
                eax = m.read32(esp.wrapping_add(0x8));
                esi = ((esi as i32).wrapping_mul((0x273) as i32)) as u32;
                xmm0 = Xmm::scalar(m.read32(esp.wrapping_add(0x10)));
                esi = esi.wrapping_add(eax);
                eax = m.read32(0x75ccc0);
                m.write32(eax.wrapping_add(esi.wrapping_mul(0x4)), xmm0.lanes[0]);
                pc = 0x238493; continue;
            }
            0x238493 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}


/// 0x238510. INFERRED: revised from game_player_stat_add_float_75ccc0_stride_0x273_if_player_valid_and_state_7de30c_is_7, used by kill event recorder
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_round_stat_add_float<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x238510;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x238510, previous, pc); previous = pc; }
        match pc {
            0x238510 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                call!(0x2382b0);
                pc = 0x23851a; continue;
            }
            0x23851a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x23854a; continue; }
                pc = 0x23851e; continue;
            }
            0x23851e => {
                fa = m.read32(0x7de30c);
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x23854a; continue; }
                pc = 0x238527; continue;
            }
            0x238527 => {
                eax = m.read32(esp.wrapping_add(0x8));
                esi = ((esi as i32).wrapping_mul((0x273) as i32)) as u32;
                esi = esi.wrapping_add(eax);
                eax = m.read32(0x75ccc0);
                xmm0 = Xmm::scalar(m.read32(eax.wrapping_add(esi.wrapping_mul(0x4))));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x10)))).to_bits();
                eax = eax.wrapping_add(esi.wrapping_mul(0x4));
                m.write32(eax, xmm0.lanes[0]);
                pc = 0x23854a; continue;
            }
            0x23854a => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}


/// 0x2385B0. INFERRED: player validity check, state 7de30c == 7, entry index below 0x46, adds float at 75ccc0 offset 0x184 indexed by player stride 0x273 and entry stride 5
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_stat_series_add_float_75ccc0_offset_0x1<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x2385b0;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x2385b0, previous, pc); previous = pc; }
        match pc {
            0x2385b0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                call!(0x2382b0);
                pc = 0x2385ba; continue;
            }
            0x2385ba => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x238601; continue; }
                pc = 0x2385be; continue;
            }
            0x2385be => {
                eax = m.read32(esp.wrapping_add(0x10));
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x238601; continue; }
                pc = 0x2385c7; continue;
            }
            0x2385c7 => {
                fa = m.read32(0x7de30c);
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x238601; continue; }
                pc = 0x2385d0; continue;
            }
            0x2385d0 => {
                edx = m.read32(esp.wrapping_add(0x8));
                esi = ((esi as i32).wrapping_mul((0x273) as i32)) as u32;
                ecx = m.read32(0x75ccc0);
                eax = eax.wrapping_add(eax.wrapping_mul(0x4));
                esi = esi.wrapping_add(edx);
                eax = eax.wrapping_add(esi);
                xmm0 = Xmm::scalar(m.read32(ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x184)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x14)))).to_bits();
                eax = ecx.wrapping_add(eax.wrapping_mul(0x4)).wrapping_add(0x184);
                m.write32(eax, xmm0.lanes[0]);
                pc = 0x238601; continue;
            }
            0x238601 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}


/// 0x238610. INFERRED: validity check on both player indexes, state 7de30c == 7, adds float at 75ccc0 offset 0x6fc indexed by first player stride 0x273 and second player times 6
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_stat_pair_matrix_add_float_75ccc0_offse<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let (mut fca, mut fcb) = (0f64, 0f64);
    let fpu = entry.fpu;
    let (mut xmm0, mut xmm1, mut xmm2, mut xmm3, mut xmm4) = (x[0], x[1], x[2], x[3], x[4]);
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x238610;
    macro_rules! push { ($v:expr) => {{ let value: u32 = $v; esp = esp.wrapping_sub(4); m.write32(esp, value); }}; }
    macro_rules! call_to { ($va:expr) => {{
        let target: u32 = $va;
        let mut cr = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags: entry.eflags, fpu, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], xmm_hi: xh };
        guest.call(target, &mut cr);
        xh = cr.xmm_hi;
        (eax, ecx, edx, esp, ebx, ebp, esi, edi) = (cr.eax, cr.ecx, cr.edx, cr.esp, cr.ebx, cr.ebp, cr.esi, cr.edi);
        [xmm0, xmm1, xmm2, xmm3, xmm4] = cr.xmm;
    }}; }
    macro_rules! call { ($va:expr) => {{ call_to!($va) }}; }
    macro_rules! tail_to { ($va:expr) => {{ let slot = esp; let saved = m.read32(slot); esp = esp.wrapping_add(4); call_to!($va); esp = esp.wrapping_sub(4); m.write32(slot, saved); }}; }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        { crate::pose::cover(0x238610, previous, pc); previous = pc; }
        match pc {
            0x238610 => {
                push!(esi);
                push!(edi);
                edi = m.read32(esp.wrapping_add(0x10));
                esi = edi;
                call!(0x2382b0);
                pc = 0x23861d; continue;
            }
            0x23861d => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x238669; continue; }
                pc = 0x238621; continue;
            }
            0x238621 => {
                esi = m.read32(esp.wrapping_add(0x14));
                call!(0x2382b0);
                pc = 0x23862a; continue;
            }
            0x23862a => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x238669; continue; }
                pc = 0x23862e; continue;
            }
            0x23862e => {
                fa = m.read32(0x7de30c);
                fb = ((((0x7i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x238669; continue; }
                pc = 0x238637; continue;
            }
            0x238637 => {
                ecx = m.read32(esp.wrapping_add(0xc));
                edi = ((edi as i32).wrapping_mul((0x273) as i32)) as u32;
                edx = m.read32(0x75ccc0);
                eax = esi.wrapping_add(esi.wrapping_mul(0x2));
                edi = edi.wrapping_add(ecx);
                ecx = edi.wrapping_add(eax.wrapping_mul(0x2));
                xmm0 = Xmm::scalar(m.read32(edx.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x6fc)));
                xmm0.lanes[0] = sse::addss(f32::from_bits(xmm0.lanes[0]), f32::from_bits(m.read32(esp.wrapping_add(0x18)))).to_bits();
                eax = edx.wrapping_add(ecx.wrapping_mul(0x4)).wrapping_add(0x6fc);
                m.write32(eax, xmm0.lanes[0]);
                pc = 0x238669; continue;
            }
            0x238669 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: [xmm0, xmm1, xmm2, xmm3, xmm4], fpu, xmm_hi: xh, ..entry };
}


// ---- translated bodies end ----

// ---- dispatch begin ----
/// Roots ported in this module (differential-only, unadmitted).
pub const ROOTS: [u32; 63] = [0x2cbce0, 0x259df0, 0xd1e20, 0xd2d20, 0x426d0, 0x42710, 0x42760, 0x42830, 0x42870, 0x428d0, 0x42920, 0x42a30, 0x42ca0, 0x42d00, 0x59cf0, 0x59d90, 0x59dd0, 0x59e00, 0x5ba00, 0x64650, 0x646a0, 0x8ea20, 0x949c0, 0xc6140, 0xc63f0, 0xc7950, 0xc9be0, 0xc9c00, 0xc9cc0, 0xc9f50, 0xcfec0, 0xd6920, 0xf8ad0, 0xf8ae0, 0x147080, 0x190e80, 0x22dd90, 0x246d10, 0x246dc0, 0x2474e0, 0x258870, 0x25a080, 0x25b720, 0x2e86a0, 0x2e8c00, 0x42c50, 0x585b0, 0x61430, 0x61610, 0x61630, 0x660c0, 0x66100, 0x66220, 0x115f10, 0x259800, 0x25ef30, 0x3381d0, 0x2ce120, 0x126260, 0x238460, 0x238510, 0x2385b0, 0x238610];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x2cbce0 => player_block_differs_from_saved_profile_entry(m, guest, regs),
        0x259df0 => player_7b0c7c_clear_flags_0x20_0x40_and_rotate(m, guest, regs),
        0xd1e20 => player_set_world_position_0xb4_with_vertical_d(m, regs),
        0xd2d20 => player_slot_index_from_object_field8(m, regs),
        0x426d0 => input_pad_map_get(m, regs),
        0x42710 => input_pad_6b93d4_get_field_0x04(m, regs),
        0x42760 => input_pad_gate_zero_when_6b9888_set(m, regs),
        0x42830 => input_pad_or_field_0x38_clear_field_0x10_mask(m, regs),
        0x42870 => input_pad_reset_neutral_mapped(m, regs),
        0x428d0 => input_pads_reset_neutral_all(m, regs),
        0x42920 => input_pad_set_float_field_0(m, regs),
        0x42a30 => input_pad_get_axis_byte(m, regs),
        0x42ca0 => input_pad_set_axis_byte(m, regs),
        0x42d00 => input_pad_map_set(m, regs),
        0x59cf0 => anim_obj_active_slot_ptr_0xd4_or_0x12c_by_blen(m, regs),
        0x59d90 => anim_obj_slot_0x12c_anim_id_equals_arg(m, regs),
        0x59dd0 => anim_obj_get_field_0x17c_or_0x124_by_field_0x1(m, regs),
        0x59e00 => anim_obj_either_slot_0xd8_or_0x130_flag_mask_t(m, regs),
        0x5ba00 => anim_obj_reset_speed_0x108_and_set_time_0xdc_0(m, regs),
        0x64650 => player_score_total_by_team_field_0x8_or_word_0(m, regs),
        0x646a0 => player_get_signed_word_0xa60(m, regs),
        0x8ea20 => anim_state_struct_init_type_and_speed_from_tab(m, regs),
        0x949c0 => anim_track_list_sum_entry_sizes_rounded_up_to(m, regs),
        0xc6140 => player_slots_base_get(m, regs),
        0xc63f0 => player_set_field_0x218_and_update_flags_0x21c(m, regs),
        0xc7950 => player_slots_reset_field_0x3b8_to_one_and_clea(m, regs),
        0xc9be0 => player_slot_733108_modify_slot_flags_0x21c(m, regs),
        0xc9c00 => player_slot_733108_set_field_0xff8(m, regs),
        0xc9cc0 => player_slot_733108_set_float_0x384(m, regs),
        0xc9f50 => player_slot_733108_get(m, regs),
        0xcfec0 => player_get_nth_active_tracked_slot_object_or_d(m, regs),
        0xd6920 => player_records_clear_all_and_reset_count(m, regs),
        0xf8ad0 => player_slot_table_73ae08_get_entry(m, regs),
        0xf8ae0 => player_slot_table_73ae08_get_current_slot_entr(m, regs),
        0x147080 => player_input_binding_or_button_mask_get(m, regs),
        0x190e80 => player_slot_name_74ed60_set_from_cstring_30_by(m, regs),
        0x22dd90 => player_arg_valid_state_7de470_0_to_0xe_byte_ma(m, guest, regs),
        0x246d10 => input_driven_object_75eaf8_is_null(m, regs),
        0x246dc0 => input_driven_advance_frame_counter_and_step_ta(m, regs),
        0x2474e0 => input_driven_all_entries_flag_and_pair_equal(m, regs),
        0x258870 => input_mapping_derive_alt_fields_0x48_to_0x54(m, regs),
        0x25a080 => player_7b0c7c_set_flag_f5c_when_flag_0x20_and(m, regs),
        0x25b720 => player_7b0c7c_shift_history_1564_1560_155c_whe(m, regs),
        0x2e86a0 => input_mapping_clear_current_player_bindings(m, regs),
        0x2e8c00 => input_mapping_build_row_to_action_index_table(m, regs),
        0x42c50 => input_devices_init_then_pads_set_float_field_0(m, guest, regs),
        0x585b0 => anim_track_keyframe_time_bracket_by_node_index(m, guest, regs),
        0x61430 => anim_obj_active_slot_anim_has_table_7e1be0_ent(m, guest, regs),
        0x61610 => anim_obj_active_slot_time_scaled_by_4d1994_tru(m, guest, regs),
        0x61630 => anim_obj_current_key_index_reached_target_key(m, guest, regs),
        0x660c0 => player_record_count_nonzero_entries_in_9_group(m, guest, regs),
        0x66100 => player_record_count_nonzero_entries_in_7_group(m, guest, regs),
        0x66220 => player_record_count_bits_set_in_either_mask_0x(m, guest, regs),
        0x115f10 => player_by_id_0xca_to_0xcd_get_field_0x14_0x34(m, guest, regs),
        0x259800 => player_lock_on_allowed_when_action_0x11_and_ex(m, guest, regs),
        0x25ef30 => player_7b0c7c_record_238_field4_bit_test_retur(m, guest, regs),
        0x3381d0 => player_current_effect_entry_has_flag_bit_25_cl(m, guest, regs),
        0x2ce120 => player_stats_count_set_flags_in_59_byte_array(m, guest, regs),
        0x126260 => player_apply_effect_float_and_object_ref_with(m, guest, regs),
        0x238460 => player_round_stat_set_float(m, guest, regs),
        0x238510 => player_round_stat_add_float(m, guest, regs),
        0x2385b0 => player_stat_series_add_float_75ccc0_offset_0x1(m, guest, regs),
        0x238610 => player_stat_pair_matrix_add_float_75ccc0_offse(m, guest, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----
