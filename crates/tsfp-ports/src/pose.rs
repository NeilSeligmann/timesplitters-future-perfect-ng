// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-anim/leaf
// portreg: mirrors=tsfp-engine-anim/flag_leaves
// portreg: mirrors=tsfp-engine-physics/fp_scalar
// portreg: mirrors=tsfp-engine-physics/fp_vector
// portreg: mirrors=tsfp-gameplay/state_leaves
// portreg: mirrors=tsfp-engine-input/leaves
//! T1850 (T1815 slice 4): pose/control leaves (`game_input_pad_*` helpers 0x426B0..0x43AF0,
//! `game_anim_obj_*` flag tests 0x59D70..0x5A1E0 and the player/anim record helpers).
//! See docs/t-inventory-pose-port.md for the evidence, the labels and the limits.
//!
//! Provenance: every function was transcribed from its lifted original (private, recomp_0004,
//! 0005, 0011, 0012 chunks) by tools/t1850_translate.py and hand cleaned. Behaviour is MEASURED
//! equal to the lifted original on FABRICATED structured memory (tests/c/test_rust_invpose.c).
//! Field names in comments are INFERRED. Not admitted: absent from every selector.
//!
//! Shape: one match arm per lifted basic block keyed by its original address. Registers are
//! plain locals so the EAX/ECX/EDX residue of the original is reproduced. The lifted flag model
//! (`fa/fb/fas/fbs/cf`) is kept only where a later conditional jump reads it. `esp` is left at
//! the entry value (the original's RET pops the return slot, the bridge does that).
#![allow(
    clippy::all,
    unused_parens,
    unused_assignments,
    unused_imports,
    unused_variables
)]
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
};

pub(crate) use tsfp_port_native_candidate_support::pose::write16;

/// Edge coverage for the differential (build with `--cfg tsfp_cover`; absent from normal builds).
/// Edges are (function, from-block, to-block) triples, deduplicated through an open-addressing index.
#[cfg(tsfp_cover)]
#[unsafe(no_mangle)]
pub static mut tsfp_invpose_edges: [[u32; 3]; 16384] = [[0; 3]; 16384];
#[cfg(tsfp_cover)]
#[unsafe(no_mangle)]
pub static mut tsfp_invpose_edge_count: u32 = 0;
#[cfg(tsfp_cover)]
static mut EDGE_INDEX: [u16; 65536] = [0; 65536];
#[cfg(tsfp_cover)]
pub fn cover(function: u32, from: u32, to: u32) {
    unsafe {
        let count = core::ptr::addr_of_mut!(tsfp_invpose_edge_count);
        let list = core::ptr::addr_of_mut!(tsfp_invpose_edges) as *mut [u32; 3];
        let index = core::ptr::addr_of_mut!(EDGE_INDEX) as *mut u16;
        let mut slot = (function.wrapping_mul(0x9e37_79b1)
            ^ from.wrapping_mul(0x85eb_ca6b)
            ^ to.wrapping_mul(0xc2b2_ae35)) as usize
            & 0xffff;
        loop {
            let entry = *index.add(slot);
            if entry == 0 {
                if (*count as usize) < 16384 {
                    *list.add(*count as usize) = [function, from, to];
                    *count += 1;
                    *index.add(slot) = *count as u16;
                }
                return;
            }
            if *list.add(entry as usize - 1) == [function, from, to] {
                return;
            }
            slot = (slot + 1) & 0xffff;
        }
    }
}

/// Stable bridge for per-subsystem C2R crates that share the host's edge recorder.
#[cfg(tsfp_cover)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_port_cover(function: u32, from: u32, to: u32) {
    cover(function, from, to);
}

// ---- translated bodies begin ----
const PAD_MAP_ENTRIES_ADDRESS: u32 = 0x6b93c0;
const PAD_MAP_ENTRY_COUNT: i32 = 4;

/// 0x426B0. INFERRED: searches the four-entry pad map for a value and returns its index, or -1 if absent.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn pad_map_find_index<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    ecx = m.read32(esp.wrapping_add(PAD_MAP_ARGUMENT_OFFSET));
    eax = 0;

    loop {
        let mapped_value = m.read32(eax.wrapping_mul(4).wrapping_add(PAD_MAP_ENTRIES_ADDRESS));
        if mapped_value == ecx {
            break;
        }

        eax = eax.wrapping_add(1);
        if (eax as i32) >= PAD_MAP_ENTRY_COUNT {
            eax = u32::MAX;
            break;
        }
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

const INPUT_STATE_ARGUMENT_OFFSET: u32 = 0x4;
const INPUT_STATE_ZERO_START_OFFSET: u32 = 0x8;
const INPUT_STATE_ZERO_END_OFFSET: u32 = 0x14;
const INPUT_STATE_NEUTRAL_START_OFFSET: u32 = 0x18;
const INPUT_STATE_NEUTRAL_END_OFFSET: u32 = 0x30;
const INPUT_STATE_NEUTRAL_AXIS_VALUE: u32 = 0x80;

/// 0x426E0. INFERRED: cdecl `(state)`, zeroes dwords +8..+0x14 and sets +0x18..+0x30 to 0x80 (neutral axes).
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn input_state_reset_neutral<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = entry.eax;
    let mut ecx;
    let edx = entry.edx;
    let esp = entry.esp;
    let ebx = entry.ebx;
    let ebp = entry.ebp;
    let esi = entry.esi;
    let edi = entry.edi;
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(INPUT_STATE_ARGUMENT_OFFSET));
    ecx = 0;
    m.write32(eax.wrapping_add(INPUT_STATE_ZERO_START_OFFSET), ecx);
    m.write32(eax.wrapping_add(0xc), ecx);
    m.write32(eax.wrapping_add(0x10), ecx);
    m.write32(eax.wrapping_add(INPUT_STATE_ZERO_END_OFFSET), ecx);

    ecx = INPUT_STATE_NEUTRAL_AXIS_VALUE;
    m.write32(eax.wrapping_add(INPUT_STATE_NEUTRAL_START_OFFSET), ecx);
    m.write32(eax.wrapping_add(0x1c), ecx);
    m.write32(eax.wrapping_add(0x20), ecx);
    m.write32(eax.wrapping_add(0x24), ecx);
    m.write32(eax.wrapping_add(0x28), ecx);
    m.write32(eax.wrapping_add(0x2c), ecx);
    m.write32(eax.wrapping_add(INPUT_STATE_NEUTRAL_END_OFFSET), ecx);

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

const PAD_BUTTON_STATE_FLAG_ADDRESS: u32 = 0x6b9888;
const PAD_BUTTON_STATE_MAPPING_FLAG_ADDRESS: u32 = 0x76b0fc;
const PAD_BUTTON_STATE_RECORDS_ADDRESS: u32 = 0x6b94d8;
const PAD_BUTTON_STATE_INDEX_MAP_ADDRESS: u32 = 0x6b93c0;
const PAD_BUTTON_STATE_MAPPED_RECORDS_ADDRESS: u32 = 0x6b93e0;
const PAD_BUTTON_STATE_RECORD_STRIDE: i32 = 0x3c;

/// 0x427b0. INFERRED: Returns zero when the global flag is set, otherwise reads the current button state for the given pad.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn pad_button_state<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let xmm = entry.xmm;
    let fpu = entry.fpu;

    if m.read32(PAD_BUTTON_STATE_FLAG_ADDRESS) != 0 {
        eax = 0;
    } else if m.read32(PAD_BUTTON_STATE_MAPPING_FLAG_ADDRESS) != 0 {
        eax = m.read32(esp.wrapping_add(4));
        eax = (eax as i32).wrapping_mul(PAD_BUTTON_STATE_RECORD_STRIDE) as u32;
        eax = m.read32(eax.wrapping_add(PAD_BUTTON_STATE_RECORDS_ADDRESS));
    } else {
        ecx = m.read32(esp.wrapping_add(4));
        edx = m.read32(
            ecx.wrapping_mul(4)
                .wrapping_add(PAD_BUTTON_STATE_INDEX_MAP_ADDRESS),
        );
        edx = (edx as i32).wrapping_mul(PAD_BUTTON_STATE_RECORD_STRIDE) as u32;
        eax = m.read32(edx.wrapping_add(PAD_BUTTON_STATE_MAPPED_RECORDS_ADDRESS));
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

const PAD_FIELD_10_DISABLED_ADDRESS: u32 = 0x6b9888;
const PAD_FIELD_10_INDEX_ADDRESS: u32 = 0x6b93e0;
const PAD_FIELD_10_ENTRY_OFFSET: u32 = 4;
const PAD_FIELD_10_ENTRY_STRIDE: u32 = 0x3c;

/// 0x42c30. INFERRED: Return zero when the pad field is set; otherwise return the indexed dword.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn pad_field_10<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    if m.read32(PAD_FIELD_10_DISABLED_ADDRESS) != 0 {
        eax = 0;
    } else {
        let pad_index = m.read32(esp.wrapping_add(PAD_FIELD_10_ENTRY_OFFSET));
        let field_offset = (pad_index as i32).wrapping_mul(PAD_FIELD_10_ENTRY_STRIDE as i32) as u32;
        eax = m.read32(field_offset.wrapping_add(PAD_FIELD_10_INDEX_ADDRESS));
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

const PADS_FIRST_FIELD_ADDRESS: u32 = 0x6b94dc;
const PADS_LOOP_END_ADDRESS: u32 = 0x6b989c;
const PAD_RECORD_STRIDE: u32 = 0x3c;
const PAD_FIELD_BEFORE_ADDRESS: u32 = 4;
const PAD_FIELD_AFTER_ADDRESS: u32 = 0x24;

/// 0x42ce0. INFERRED: zeroes three fields in each pad record.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn pads_clear_fields<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, ecx, edx, esp) = (PADS_FIRST_FIELD_ADDRESS, 0, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    while (eax as i32) < (PADS_LOOP_END_ADDRESS as i32) {
        m.write32(eax.wrapping_sub(PAD_FIELD_BEFORE_ADDRESS), ecx);
        m.write32(eax, ecx);
        m.write32(eax.wrapping_add(PAD_FIELD_AFTER_ADDRESS), ecx);
        eax = eax.wrapping_add(PAD_RECORD_STRIDE);
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

const PAD_MAP_ARGUMENT_OFFSET: u32 = 0x4;
const PAD_MAP_ENTRY_SIZE: u32 = 0x4;
const PAD_MAP_BIT_INDEX_MASK: u32 = 0x1f;
const PAD_MAP_TABLE_ADDRESS: u32 = 0x6b93c0;
const PAD_MAP_BITS_ADDRESS: u32 = 0x7e7240;

/// 0x431C0. INFERRED: Tests whether the pad's mapped bit is set.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn pad_map_is_bit_set<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    ecx = m.read32(esp.wrapping_add(PAD_MAP_ARGUMENT_OFFSET));
    ecx = m.read32(
        ecx.wrapping_mul(PAD_MAP_ENTRY_SIZE)
            .wrapping_add(PAD_MAP_TABLE_ADDRESS),
    );
    edx = m.read16(PAD_MAP_BITS_ADDRESS) as u32;

    let bit_index = (ecx as u8 as u32) & PAD_MAP_BIT_INDEX_MASK;
    eax = 1u32.wrapping_shl(bit_index);
    eax &= edx;
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
        xmm: x,
        fpu,
        ..entry
    };
}

/// 0x43A90. INFERRED: cdecl `(pad, out, count)`, copies the pad record word +0xC and (count-2) masked bytes (+0x1C stride 4, low bit cleared) to `out`.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn pad_copy_axes_and_buttons<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x43a90;
    macro_rules! push {
        ($v:expr) => {{
            let value: u32 = $v;
            esp = esp.wrapping_sub(4);
            m.write32(esp, value);
        }};
    }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        {
            crate::pose::cover(0x43a90, previous, pc);
            previous = pc;
        }
        match pc {
            0x43a90 => {
                eax = m.read32(0x76b0fc);
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                if (fa == fb) {
                    pc = 0x43aa8;
                    continue;
                }
                pc = 0x43a99;
                continue;
            }
            0x43a99 => {
                ecx = m.read32(esp.wrapping_add(0x4));
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                ecx = ecx.wrapping_add(0x6b94c8);
                pc = 0x43abc;
                continue;
            }
            0x43aa8 => {
                eax = m.read32(esp.wrapping_add(0x4));
                ecx = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x6b93c0));
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                ecx = ecx.wrapping_add(0x6b93d0);
                pc = 0x43abc;
                continue;
            }
            0x43abc => {
                edx = (edx & 0xffff_0000) | (((m.read16(ecx.wrapping_add(0xc))) as u32) & 0xffff);
                eax = m.read32(esp.wrapping_add(0x8));
                write16(m, eax, (((edx) as u16) as u32) as u16);
                edx = m.read32(esp.wrapping_add(0xc));
                eax = eax.wrapping_add(0x2);
                edx = edx.wrapping_add(0xfffffffeu32);
                fa = edx;
                if (fa == 0) {
                    pc = 0x43aef;
                    continue;
                }
                pc = 0x43ad3;
                continue;
            }
            0x43ad3 => {
                push!(esi);
                ecx = ecx.wrapping_add(0x1c);
                esi = edx;
                pc = 0x43ae0;
                continue;
            }
            0x43ae0 => {
                edx = (edx & 0xffff_ff00) | (((m.read8(ecx)) as u32) & 0xff);
                edx = (edx & 0xffff_ff00) | ((((((edx) as u8) as i32) & 0xfei32) as u32) & 0xff);
                m.write8(eax, (((edx) as u8) as u32) as u8);
                eax = eax.wrapping_add(1);
                ecx = ecx.wrapping_add(0x4);
                esi = esi.wrapping_sub(1);
                fa = esi;
                if (fa != 0) {
                    pc = 0x43ae0;
                    continue;
                }
                pc = 0x43aee;
                continue;
            }
            0x43aee => {
                esi = m.read32(esp);
                esp = esp.wrapping_add(4);
                pc = 0x43aef;
                continue;
            }
            0x43aef => {
                break 'run;
            }
            _ => unreachable!(),
        }
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

/// 0x43AF0. INFERRED: cdecl `(pad, report, count)`, stores newly pressed bits (cur & ~prev) at +0x10, the 16-bit button word at +0xC and +8, released bits at +0x14, and copies the following report bytes into the axis dwords from +0x1C.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn pad_update_from_report<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x43af0;
    macro_rules! push {
        ($v:expr) => {{
            let value: u32 = $v;
            esp = esp.wrapping_sub(4);
            m.write32(esp, value);
        }};
    }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        {
            crate::pose::cover(0x43af0, previous, pc);
            previous = pc;
        }
        match pc {
            0x43af0 => {
                eax = m.read32(0x76b0fc);
                fa = eax;
                fb = (((0i32) as u32) & 0xffffffffu32);
                eax = m.read32(esp.wrapping_add(0x4));
                if (fa == fb) {
                    pc = 0x43b07;
                    continue;
                }
                pc = 0x43afd;
                continue;
            }
            0x43afd => {
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = eax.wrapping_add(0x6b94c8);
                pc = 0x43b16;
                continue;
            }
            0x43b07 => {
                eax = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x6b93c0));
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = eax.wrapping_add(0x6b93d0);
                pc = 0x43b16;
                continue;
            }
            0x43b16 => {
                edx = m.read32(esp.wrapping_add(0x8));
                ecx = (ecx & 0xffff_0000) | (((m.read16(edx)) as u32) & 0xffff);
                ecx = (((ecx) as u16) as u16 as u32);
                push!(esi);
                esi = m.read32(eax.wrapping_add(0x8));
                push!(edi);
                edi = esi;
                edi = !edi;
                edi = (edi & ecx);
                m.write32(eax.wrapping_add(0x10), edi);
                edi = ecx;
                m.write32(eax.wrapping_add(0xc), ecx);
                m.write32(eax.wrapping_add(0x8), ecx);
                ecx = m.read32(esp.wrapping_add(0x14));
                edi = !edi;
                edi = (edi & esi);
                edx = edx.wrapping_add(0x2);
                ecx = ecx.wrapping_add(0xfffffffeu32);
                fa = ecx;
                m.write32(eax.wrapping_add(0x14), edi);
                if (fa == 0) {
                    pc = 0x43b5e;
                    continue;
                }
                pc = 0x43b49;
                continue;
            }
            0x43b49 => {
                eax = eax.wrapping_add(0x1c);
                esi = ecx;
                edi = edi;
                pc = 0x43b50;
                continue;
            }
            0x43b50 => {
                ecx = (ecx & 0xffff_ff00) | (((m.read8(edx)) as u32) & 0xff);
                ecx = (((ecx) as u8) as u8 as u32);
                m.write32(eax, ecx);
                edx = edx.wrapping_add(1);
                eax = eax.wrapping_add(0x4);
                esi = esi.wrapping_sub(1);
                fa = esi;
                if (fa != 0) {
                    pc = 0x43b50;
                    continue;
                }
                pc = 0x43b5e;
                continue;
            }
            0x43b5e => {
                edi = m.read32(esp);
                esp = esp.wrapping_add(4);
                esi = m.read32(esp);
                esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
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

const ANIM_SLOT_D4_OBJECT_ARG_OFFSET: u32 = 0x4;
const ANIM_SLOT_D4_FIELD_OFFSET: u32 = 0xd4;
const ANIM_SLOT_D4_ID_ARG_OFFSET: u32 = 0x8;

/// 0x59d70. INFERRED: Checks whether an object's animation slot ID matches the given ID.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn anim_slot_d4_equals<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    let object = m.read32(esp.wrapping_add(ANIM_SLOT_D4_OBJECT_ARG_OFFSET));
    let object_prefix = m.read32(object.wrapping_add(ANIM_SLOT_D4_OBJECT_ARG_OFFSET));
    edx = m.read32(object_prefix.wrapping_add(ANIM_SLOT_D4_FIELD_OFFSET));
    ecx = m.read32(esp.wrapping_add(ANIM_SLOT_D4_ID_ARG_OFFSET));

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

const ANIM_D8_FLAG_TABLE_PTR: u32 = 0x7e3954;
const ANIM_D8_MASK_TABLE: u32 = 0x7de660;
const ANIM_D8_MASK_HIGH_OFFSET: u32 = 4;
const ANIM_D8_MASK_STRIDE: u32 = 8;
const ANIM_D8_INDEX_OFFSET: u32 = 0xd8;
const ANIM_D8_OBJECT_OFFSET: u32 = 4;

/// 0x59f10. INFERRED: Tests whether an object's indexed animation flag mask intersects a supplied 64-bit mask.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn anim_d8_flag_mask_test<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(4));
    ecx = m.read32(eax.wrapping_add(ANIM_D8_OBJECT_OFFSET));
    ecx = m.read32(ecx.wrapping_add(ANIM_D8_INDEX_OFFSET));
    edx = m.read32(ANIM_D8_FLAG_TABLE_PTR);
    let flag_entry = m.read32(edx.wrapping_add(ecx.wrapping_mul(4)));

    if flag_entry == 0 {
        eax = 0;
    } else {
        eax = m.read32(
            ecx.wrapping_mul(ANIM_D8_MASK_STRIDE)
                .wrapping_add(ANIM_D8_MASK_TABLE),
        );
        edx = m.read32(esp.wrapping_add(8));
        ecx = m.read32(
            ecx.wrapping_mul(ANIM_D8_MASK_STRIDE)
                .wrapping_add(ANIM_D8_MASK_TABLE)
                .wrapping_add(ANIM_D8_MASK_HIGH_OFFSET),
        );
        eax &= edx;
        ecx &= m.read32(esp.wrapping_add(12));
        eax |= ecx;

        if eax == 0 {
            eax = 0;
        } else {
            eax = 1;
        }
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

const FLAG_TABLE_POINTER_ADDRESS: u32 = 0x7e3954;
const FLAG_VALUE_TABLE_ADDRESS: u32 = 0x7de660;
const FLAG_MASK_TABLE_ADDRESS: u32 = 0x7de664;
const OBJECT_INDEX_OFFSET: u32 = 0x130;
const STACK_OBJECT_OFFSET: u32 = 0x4;
const STACK_FIRST_MASK_OFFSET: u32 = 0x8;
const STACK_SECOND_MASK_OFFSET: u32 = 0xc;

/// 0x59f50. INFERRED: Tests the object's indexed flag value against two masks.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn anim_130_flag_mask_test<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(STACK_OBJECT_OFFSET));
    ecx = m.read32(eax.wrapping_add(STACK_OBJECT_OFFSET));
    ecx = m.read32(ecx.wrapping_add(OBJECT_INDEX_OFFSET));
    edx = m.read32(FLAG_TABLE_POINTER_ADDRESS);
    let indexed_flag = m.read32(edx.wrapping_add(ecx.wrapping_mul(4)));

    if indexed_flag == 0 {
        eax = 0;
    } else {
        let flag_value = m.read32(ecx.wrapping_mul(8).wrapping_add(FLAG_VALUE_TABLE_ADDRESS));
        edx = m.read32(esp.wrapping_add(STACK_FIRST_MASK_OFFSET));
        let flag_mask = m.read32(ecx.wrapping_mul(8).wrapping_add(FLAG_MASK_TABLE_ADDRESS));
        ecx = flag_mask & m.read32(esp.wrapping_add(STACK_SECOND_MASK_OFFSET));

        eax = u32::from((flag_value & edx) | ecx != 0);
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

const ANIM_BLEND_OFFSET: u32 = 0x198;
const ANIM_FIRST_SLOT_OFFSET: u32 = 0x12c;
const ANIM_SECOND_SLOT_OFFSET: u32 = 0xd4;
const ANIM_RESULT_TRUE: u32 = 1;
const ANIM_RESULT_FALSE: u32 = 0;

/// 0x5A1E0. INFERRED: cdecl `(obj, id) -> bool`, comparing the selected animation slot ID with `id`.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn anim_either_slot_equals<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(0x4));
    eax = m.read32(eax.wrapping_add(0x4));
    xmm[1] = Xmm::scalar(m.read32(eax.wrapping_add(ANIM_BLEND_OFFSET)));
    xmm[0] = Xmm::default();

    let blend = f32::from_bits(xmm[1].lanes[0]) as f64;
    let zero = f32::from_bits(xmm[0].lanes[0]) as f64;
    ecx = m.read32(esp.wrapping_add(0x8));

    let result = if blend > zero {
        if m.read32(eax.wrapping_add(ANIM_FIRST_SLOT_OFFSET)) == ecx {
            ANIM_RESULT_TRUE
        } else {
            let reread_blend = f32::from_bits(m.read32(eax.wrapping_add(ANIM_BLEND_OFFSET))) as f64;
            if zero < reread_blend || zero.is_nan() || reread_blend.is_nan() {
                ANIM_RESULT_FALSE
            } else if m.read32(eax.wrapping_add(ANIM_SECOND_SLOT_OFFSET)) == ecx {
                ANIM_RESULT_TRUE
            } else {
                ANIM_RESULT_FALSE
            }
        }
    } else {
        let reread_blend = f32::from_bits(m.read32(eax.wrapping_add(ANIM_BLEND_OFFSET))) as f64;
        if zero < reread_blend || zero.is_nan() || reread_blend.is_nan() {
            ANIM_RESULT_FALSE
        } else if m.read32(eax.wrapping_add(ANIM_SECOND_SLOT_OFFSET)) == ecx {
            ANIM_RESULT_TRUE
        } else {
            ANIM_RESULT_FALSE
        }
    };

    eax = result;
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

const PLAYER_VELOCITY_X_OFFSET: u32 = 0xC0;
const PLAYER_VELOCITY_Y_OFFSET: u32 = 0xC8;
const PLAYER_STATE_BYTE_OFFSET: u32 = 0x32;
const PLAYER_ACTIVE_STATE: u8 = 1;

/// 0xb2d20. INFERRED: Zeroes the player's velocity fields and sets its state byte to 1.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
pub fn player_zero_velocity<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut eax = entry.eax;
    let (ecx, edx, esp) = (entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let (xmm1, xmm2, xmm3, xmm4) = (entry.xmm[1], entry.xmm[2], entry.xmm[3], entry.xmm[4]);
    let xmm0 = Xmm::default();
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(4));
    m.write32(eax.wrapping_add(PLAYER_VELOCITY_X_OFFSET), xmm0.lanes[0]);
    m.write32(eax.wrapping_add(PLAYER_VELOCITY_Y_OFFSET), xmm0.lanes[0]);
    m.write8(
        eax.wrapping_add(PLAYER_STATE_BYTE_OFFSET),
        PLAYER_ACTIVE_STATE,
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
        xmm: [xmm0, xmm1, xmm2, xmm3, xmm4],
        fpu,
        ..entry
    };
}

/// 0xC4560. INFERRED: cdecl `(index, out)`, copies an 8 dword summary of player slot `index` (stride 0xEA0 from [0x7356D8]) to `out`; index -1 zeroes it and stores -1 in the first dword.
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn slot_summary_fill<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0xc4560;
    macro_rules! push {
        ($v:expr) => {{
            let value: u32 = $v;
            esp = esp.wrapping_sub(4);
            m.write32(esp, value);
        }};
    }
    #[cfg(tsfp_cover)]
    let mut previous = 0u32;
    'run: loop {
        #[cfg(tsfp_cover)]
        {
            crate::pose::cover(0xc4560, previous, pc);
            previous = pc;
        }
        match pc {
            0xc4560 => {
                edx = m.read32(esp.wrapping_add(0x4));
                fa = edx;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) {
                    pc = 0xc45be;
                    continue;
                }
                pc = 0xc4569;
                continue;
            }
            0xc4569 => {
                ecx = m.read32(0x7356d8);
                eax = edx;
                eax = ((eax as i32).wrapping_mul((0xea0) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(esp.wrapping_add(0x8));
                m.write32(ecx, edx);
                edx = m.read32(eax);
                edx = m.read32(edx.wrapping_add(0x34));
                m.write32(ecx.wrapping_add(0x4), edx);
                edx = m.read32(eax.wrapping_add(0xbfc));
                m.write32(ecx.wrapping_add(0x8), edx);
                edx = m.read32(eax.wrapping_add(0xc00));
                m.write32(ecx.wrapping_add(0xc), edx);
                edx = m.read32(eax.wrapping_add(0xc1c));
                m.write32(ecx.wrapping_add(0x10), edx);
                edx = m.read32(eax.wrapping_add(0xc20));
                m.write32(ecx.wrapping_add(0x14), edx);
                edx = m.read32(eax.wrapping_add(0xbf8));
                m.write32(ecx.wrapping_add(0x18), edx);
                eax = m.read32(eax.wrapping_add(0xc04));
                m.write32(ecx.wrapping_add(0x1c), eax);
                break 'run;
            }
            0xc45be => {
                eax = m.read32(esp.wrapping_add(0x8));
                ecx = 0;
                edx = eax;
                m.write32(edx, ecx);
                m.write32(edx.wrapping_add(0x4), ecx);
                m.write32(edx.wrapping_add(0x8), ecx);
                m.write32(edx.wrapping_add(0xc), ecx);
                m.write32(edx.wrapping_add(0x10), ecx);
                m.write32(edx.wrapping_add(0x14), ecx);
                m.write32(edx.wrapping_add(0x18), ecx);
                m.write32(edx.wrapping_add(0x1c), ecx);
                m.write32(eax, 0xffffffffu32);
                break 'run;
            }
            _ => unreachable!(),
        }
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

const SLOT_733108: u32 = 0x733108;

/// 0xc9bb0. INFERRED: Returns whether the player slot at 0x733108 is set.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(clippy::all)]
pub fn slot_733108_is_set<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let ecx = m.read32(SLOT_733108);
    let eax = u32::from(ecx != 0);

    *regs = ExtRegisters {
        eax,
        ecx,
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

const PLAYER_POINTER_ADDRESS: u32 = 0x7b0c7c;
const PLAYER_STATE_OFFSET: u32 = 0x114;
const PLAYER_OBJECT_OFFSET: u32 = 0x14;
const PLAYER_POSITION_X_OFFSET: u32 = 0xa8;
const PLAYER_POSITION_Y_OFFSET: u32 = 0xac;
const PLAYER_POSITION_Z_OFFSET: u32 = 0xb0;
const OWNER_POSITION_X_OFFSET: u32 = 0x64;
const OWNER_POSITION_Y_OFFSET: u32 = 0x68;
const OWNER_POSITION_Z_OFFSET: u32 = 0x6c;
const EXCLUDED_PLAYER_STATE: u32 = 0x400;

/// 0xD0CA0. INFERRED: Copies player position into its owner object unless the player is in state 0x400.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(
    unused_assignments,
    unused_mut,
    unused_variables,
    unused_macros,
    clippy::all
)]
pub fn sync_owner_position<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (ebx, ebp, esi, edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let x = entry.xmm;
    let fpu = entry.fpu;

    let player = m.read32(PLAYER_POINTER_ADDRESS);
    ecx = player;
    let state = m.read32(player.wrapping_add(PLAYER_STATE_OFFSET));
    eax = m.read32(player.wrapping_add(PLAYER_OBJECT_OFFSET));

    if state != EXCLUDED_PLAYER_STATE {
        ecx = m.read32(player.wrapping_add(PLAYER_POSITION_X_OFFSET));
        m.write32(eax.wrapping_add(OWNER_POSITION_X_OFFSET), ecx);

        edx = m.read32(PLAYER_POINTER_ADDRESS);
        ecx = m.read32(edx.wrapping_add(PLAYER_POSITION_Y_OFFSET));
        m.write32(eax.wrapping_add(OWNER_POSITION_Y_OFFSET), ecx);

        edx = m.read32(PLAYER_POINTER_ADDRESS);
        ecx = m.read32(edx.wrapping_add(PLAYER_POSITION_Z_OFFSET));
        m.write32(eax.wrapping_add(OWNER_POSITION_Z_OFFSET), ecx);
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

// ---- translated bodies end ----

// ---- dispatch begin ----
/// Roots ported in this module (differential-only, unadmitted).
pub const ROOTS: [u32; 16] = [
    0x426b0, 0x426e0, 0x427b0, 0x42c30, 0x42ce0, 0x431c0, 0x43a90, 0x43af0, 0x59d70, 0x59f10,
    0x59f50, 0x5a1e0, 0xb2d20, 0xc4560, 0xc9bb0, 0xd0ca0,
];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x426b0 => pad_map_find_index(m, regs),
        0x426e0 => input_state_reset_neutral(m, regs),
        0x427b0 => pad_button_state(m, regs),
        0x42c30 => pad_field_10(m, regs),
        0x42ce0 => pads_clear_fields(m, regs),
        0x431c0 => pad_map_is_bit_set(m, regs),
        0x43a90 => pad_copy_axes_and_buttons(m, regs),
        0x43af0 => pad_update_from_report(m, regs),
        0x59d70 => anim_slot_d4_equals(m, regs),
        0x59f10 => anim_d8_flag_mask_test(m, regs),
        0x59f50 => anim_130_flag_mask_test(m, regs),
        0x5a1e0 => anim_either_slot_equals(m, regs),
        0xb2d20 => player_zero_velocity(m, regs),
        0xc4560 => slot_summary_fill(m, regs),
        0xc9bb0 => slot_733108_is_set(m, regs),
        0xd0ca0 => sync_owner_position(m, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----

/// # Safety
/// `regs` is a live `ExtRegisters`, `offset` maps the guest window, `call` is the trampoline.
/// Differential-only entry for tests/c/test_rust_invpose.c: dispatches over every T1850 module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_invpose_try(
    va: u32,
    regs: *mut ExtRegisters,
    offset: usize,
    call: crate::ext::CallFn,
) -> bool {
    let mut memory = unsafe { crate::memory::MappedGuest::new(offset) };
    let mut guest = crate::ext::CGuest(call);
    let regs = unsafe { &mut *regs };
    run(va, &mut memory, &mut guest, regs)
        || crate::pose_anim::run(va, &mut memory, &mut guest, regs)
        || crate::pose_leaf::run(va, &mut memory, &mut guest, regs)
        || crate::inventory_ammo::run(va, &mut memory, &mut guest, regs)
        || crate::inventory_slots::run(va, &mut memory, &mut guest, regs)
        || crate::inventory_pickup::run(va, &mut memory, &mut guest, regs)
        || crate::inventory::run(va, &mut memory, &mut guest, regs)
        || crate::inventory_select::run(va, &mut memory, &mut guest, regs)
        || crate::inventory_grant::run(va, &mut memory, &mut guest, regs)
}
