// SPDX-License-Identifier: GPL-3.0-or-later
//! T1850 (T1815 slice 3): weapon slot order, cycling and selection.
//! Same provenance, labels and conventions as inventory.rs / pose.rs (MEASURED against the lifted originals on
//! FABRICATED memory with stub callees, names INFERRED, not admitted).
#![allow(clippy::all, unused_parens, unused_assignments, unused_imports, unused_variables)]
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
    pose::write16,
};
#[allow(unused_imports)]
use crate::inventory::sse;
use crate::views::generated_views::{TsfpCharacterView, TsfpWeaponEntryView};

// ---- translated bodies begin ----
const WEAPON_SLOT_ARGUMENT_OFFSET: u32 = 4;
const WEAPON_ENTRY_TABLE: u32 = 0x4fa128;
const DEFAULT_SLOT_VALUE_34: u32 = 0x47e2b0;
const DEFAULT_SLOT_VALUE_30: u32 = 0x475cac;

/// Read one float from the selected weapon record, or its zero-slot default.
///
/// Keep the dword reads and wrapping address arithmetic of the guest ABI.
/// The selected slot index is returned in EAX as a byte offset for nonzero slots.
fn selected_slot_float<M: GuestMemory>(
    m: &M,
    regs: &mut ExtRegisters,
    field_offset: u32,
    default_address: u32,
) {
    let object = m.read32(regs.esp.wrapping_add(WEAPON_SLOT_ARGUMENT_OFFSET));
    let slot = m.read32(object.wrapping_add(TsfpCharacterView::OFFSET_LOADOUT_WEAPON));
    let float_address = if slot == 0 {
        regs.eax = 0;
        default_address
    } else {
        let byte_offset = slot.wrapping_mul(TsfpWeaponEntryView::SIZE);
        regs.eax = byte_offset;
        WEAPON_ENTRY_TABLE
            .wrapping_add(field_offset)
            .wrapping_add(byte_offset)
    };
    regs.fpu.top = (regs.fpu.top + 7) & 7;
    regs.fpu.stack[regs.fpu.top as usize] = f32::from_bits(m.read32(float_address)) as f64;
}

/// 0x1B9AB0. INFERRED: load selected weapon field +0x34 or its zero-slot default.
/// Verification: historical Tier 1 on FABRICATED lifted differential (T1880); this rewrite Tier 0.
pub fn get_weapon_slot_4fa128_float_0x34_by_object_cu<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    selected_slot_float(m, regs, TsfpWeaponEntryView::OFFSET_SLOT_VALUE_34, DEFAULT_SLOT_VALUE_34);
}

/// 0x1B9AD0. INFERRED: load selected weapon field +0x30 or its zero-slot default.
/// Verification: historical Tier 1 on FABRICATED lifted differential (T1880); this rewrite Tier 0.
pub fn get_weapon_slot_4fa128_float_0x30_by_object_cu<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters) {
    selected_slot_float(m, regs, TsfpWeaponEntryView::OFFSET_SLOT_VALUE_30, DEFAULT_SLOT_VALUE_30);
}


const SLOT_ORDER_TABLE: u32 = 0x4fc888;
const SLOT_ORDER_COUNT: u32 = 0x46;

/// Borrowed view of the ordered weapon indices in guest memory. Each entry is
/// a live dword; retaining the read width matters at the host boundary.
struct SlotOrderTable(u32);

impl SlotOrderTable {
    fn weapon_at<M: GuestMemory>(self, memory: &M, position: u32) -> u32 {
        memory.read32(self.0.wrapping_add(position.wrapping_mul(4)))
    }
}

/// 0x1B6BB0. Find the weapon entry whose group matches ECX in slot order.
/// Verification: Tier 0 for this readable rewrite on fabricated memory; source names INFERRED.
pub fn slot_order_4fc888_find_index_whose_slot_field<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) {
    for order_index in 0..SLOT_ORDER_COUNT {
        let order_address = SLOT_ORDER_TABLE.wrapping_add(order_index * 4);
        let weapon_index = SlotOrderTable(SLOT_ORDER_TABLE).weapon_at(m, order_index);
        let entry = TsfpWeaponEntryView(
            WEAPON_ENTRY_TABLE.wrapping_add(weapon_index.wrapping_mul(TsfpWeaponEntryView::SIZE)),
        );
        // Keep a dword read: the generated getter reads bytes and changes the host boundary.
        let group = m.read32(entry.0.wrapping_add(TsfpWeaponEntryView::OFFSET_GROUP));
        regs.edx = weapon_index.wrapping_mul(TsfpWeaponEntryView::SIZE);
        if group == regs.ecx {
            regs.eax = SlotOrderTable(SLOT_ORDER_TABLE).weapon_at(m, order_index);
            return;
        }
    }
    regs.eax = u32::MAX;
}


/// 0x1B6BF0. Scan the ordered weapon table for the next owned entry with bit 4 set.
/// Verification: Tier 0 for this readable rewrite on fabricated memory; names INFERRED.
pub fn slot_order_4fc888_next_owned_slot_with_flag_0x<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    _guest: &mut C,
    regs: &mut ExtRegisters,
) {
    const OWNED_FLAGS_OFFSET: u32 = 0x54c;
    const SELECTABLE_FLAG: u8 = 0x10;

    let entry = *regs;
    let wanted = m.read32(entry.esp.wrapping_add(8));
    let mut current = None;
    for position in 0..SLOT_ORDER_COUNT {
        let weapon = SlotOrderTable(SLOT_ORDER_TABLE).weapon_at(m, position);
        if weapon == wanted {
            current = Some(position);
            break;
        }
    }

    // The translated path uses zero when the requested weapon is absent.
    let start = current.unwrap_or(0);
    let mut esp = entry.esp.wrapping_sub(4);
    m.write32(esp, entry.esi);
    let owner = m.read32(esp.wrapping_add(8));
    esp = esp.wrapping_sub(4);
    m.write32(esp, entry.edi);

    let mut position = start;
    let weapon = loop {
        position += 1;
        if position == SLOT_ORDER_COUNT {
            position = 0;
        }
        let weapon = SlotOrderTable(SLOT_ORDER_TABLE).weapon_at(m, position);
        let weapon_entry = TsfpWeaponEntryView(
            WEAPON_ENTRY_TABLE.wrapping_add(weapon.wrapping_mul(TsfpWeaponEntryView::SIZE)),
        );
        let flags = m.read8(weapon_entry.0);
        if flags & SELECTABLE_FLAG != 0
            && m.read8(owner.wrapping_add(OWNED_FLAGS_OFFSET).wrapping_add(weapon)) != 0
        {
            break weapon;
        }
        if position == start {
            break weapon;
        }
    };

    let _saved_edi = m.read32(esp);
    esp = esp.wrapping_add(4);
    let _saved_esi = m.read32(esp);
    esp = esp.wrapping_add(4);
    regs.eax = weapon;
    regs.ecx = position;
    regs.edx = start;
    regs.edi = entry.edi;
    regs.esi = entry.esi;
    regs.esp = esp;
}


/// 0x1BA1B0. INFERRED: same chain walk as 1ba130 on [obj+0x94], calls 1ba130 only when the head differs
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn object_realign_current_weapon_slot_0x94_to_cha<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1ba1b0;
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
        { crate::pose::cover(0x1ba1b0, previous, pc); previous = pc; }
        match pc {
            0x1ba1b0 => {
                push!(ebx);
                push!(esi);
                esi = m.read32(esp.wrapping_add(0xc));
                edx = m.read32(esi.wrapping_add(0x94));
                eax = edx;
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = ((((m.read8(ecx.wrapping_add(0x4fa128)))) as u32) & 0xffu32);
                fb = ((((0x4i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1ba202; continue; }
                pc = 0x1ba1cc; continue;
            }
            0x1ba1cc => {
                ecx = m.read32(esi.wrapping_add(0x80));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba202; continue; }
                pc = 0x1ba1d6; continue;
            }
            0x1ba1d6 => {
                ecx = m.read32(0x7de458);
                fa = (((((((ecx) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x10i32)) as u32) & 0xffu32);
                if ((fa & fb) != 0) { pc = 0x1ba202; continue; }
                pc = 0x1ba1e1; continue;
            }
            0x1ba1e1 => {
                eax = eax.wrapping_sub(1);
                ecx = eax;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(0x4fa128))) as u32)) & 0xff);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                fas = (((((fa)) as i8)) as i32);
                fbs = (((((fb)) as i8)) as i32);
                ecx = ecx.wrapping_add(0x4fa128);
                if (((((((((((((fas)) as u8)) as i32).wrapping_sub((((((fbs)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) != 0i32)) { pc = 0x1ba202; continue; }
                pc = 0x1ba1f7; continue;
            }
            0x1ba1f7 => {
                ebx = (ebx & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(((0x3ci32.wrapping_neg()) as u32)))) as u32)) & 0xff);
                ecx = ecx.wrapping_sub(0x3c);
                eax = eax.wrapping_sub(1);
                fa = ((((((ebx) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (((((((((((((fa)) as u8)) as i32).wrapping_sub((((((fb)) as u8)) as i32)))) as u8)) as i32).wrapping_shr((0x7 & 31))) == 0i32)) { pc = 0x1ba1f7; continue; }
                pc = 0x1ba202; continue;
            }
            0x1ba202 => {
                fa = eax;
                fb = edx;
                if (fa == fb) { pc = 0x1ba210; continue; }
                pc = 0x1ba206; continue;
            }
            0x1ba206 => {
                push!(eax);
                push!(esi);
                call!(0x1ba130);
                pc = 0x1ba20d; continue;
            }
            0x1ba20d => {
                esp = esp.wrapping_add(0x8);
                pc = 0x1ba210; continue;
            }
            0x1ba210 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


const OBJECT_SELECT_WEAPON_SLOT_VA: u32 = 0x1bfa60;
const PLAYER_ARRAY_BASE: u32 = 0x7b0c48;
const PLAYER_ENTRY_STRIDE: i32 = 0x1584;
const WEAPON_SLOT_OFFSET: u32 = 0x8;
const PLAYER_FLAG_OFFSET: u32 = 0xc;
const PLAYER_FLAG_4000: u32 = 0x4000;
const SELECT_WEAPON_SLOT_CALL: u32 = 0x1ba130;

/// 0x1BFA60. INFERRED: selects a weapon slot and sets the player's 0x4000 flag.
/// Verification: Tier 1; lifted differential on fabricated fixtures (T1880).
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn object_select_weapon_slot_and_set_player_flag<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut xmm = entry.xmm;
    let mut xmm_hi = entry.xmm_hi;
    let fpu = entry.fpu;

    eax = m.read32(esp.wrapping_add(4));
    ecx = m.read32(PLAYER_ARRAY_BASE);
    esp = esp.wrapping_sub(4);
    m.write32(esp, esi);

    esi = m.read32(eax.wrapping_add(WEAPON_SLOT_OFFSET));
    esi = ((esi as i32).wrapping_mul(PLAYER_ENTRY_STRIDE)) as u32;
    esi = esi.wrapping_add(ecx);

    ecx = m.read32(esp.wrapping_add(0xc));
    esp = esp.wrapping_sub(4);
    m.write32(esp, ecx);
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
    guest.call(SELECT_WEAPON_SLOT_CALL, &mut call_regs);
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

    eax = m.read32(esi.wrapping_add(PLAYER_FLAG_OFFSET));
    eax |= PLAYER_FLAG_4000;
    esp = esp.wrapping_add(8);
    m.write32(esi.wrapping_add(PLAYER_FLAG_OFFSET), eax);

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
        xmm_hi,
        ..entry
    };
}


/// 0x1BA8F0. INFERRED: read in full: finds the entry in order table 4fc888 (0x46 entries), walks forward and wraps, accepts the first entry with the same group value (table 4fa160) as arg for which the selectable predicate 1ba410 holds
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn weapon_slot_order_next_selectable_entry_forwar<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1ba8f0;
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
        { crate::pose::cover(0x1ba8f0, previous, pc); previous = pc; }
        match pc {
            0x1ba8f0 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x10));
                push!(esi);
                eax = 0;
                pc = 0x1ba900; continue;
            }
            0x1ba900 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x4fc888));
                fb = ebp;
                if (fa == fb) { pc = 0x1ba939; continue; }
                pc = 0x1ba909; continue;
            }
            0x1ba909 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1ba900; continue; }
                pc = 0x1ba90f; continue;
            }
            0x1ba90f => {
                m.write32(esp.wrapping_add(0x14), 0);
                pc = 0x1ba917; continue;
            }
            0x1ba917 => {
                ebx = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(ebx.wrapping_add(0x8));
                edx = m.read32(0x790950);
                esi = m.read32(esp.wrapping_add(0x14));
                eax = 0;
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba93f; continue; }
                pc = 0x1ba933; continue;
            }
            0x1ba933 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1ba939 => {
                m.write32(esp.wrapping_add(0x14), eax);
                pc = 0x1ba917; continue;
            }
            0x1ba93f => {
                push!(edi);
                pc = 0x1ba940; continue;
            }
            0x1ba940 => {
                esi = esi.wrapping_add(1);
                fa = esi;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1ba95f; continue; }
                pc = 0x1ba946; continue;
            }
            0x1ba946 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1ba99b; continue; }
                pc = 0x1ba94e; continue;
            }
            0x1ba94e => {
                eax = m.read32(0x790950);
                ecx = m.read32(ebx.wrapping_add(0x8));
                edx = 0;
                fa = ecx;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                edx = (edx & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                esi = edx;
                pc = 0x1ba95f; continue;
            }
            0x1ba95f => {
                edi = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x4fc888));
                ecx = edi;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = m.read32(ecx.wrapping_add(0x4fa160));
                edx = ebp;
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = eax;
                fb = m.read32(edx.wrapping_add(0x4fa160));
                if (fa != fb) { pc = 0x1ba98e; continue; }
                pc = 0x1ba97e; continue;
            }
            0x1ba97e => {
                push!(0x1);
                push!(edi);
                push!(ebx);
                call!(0x1ba410);
                pc = 0x1ba987; continue;
            }
            0x1ba987 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1ba994; continue; }
                pc = 0x1ba98e; continue;
            }
            0x1ba98e => {
                fa = m.read32(esp.wrapping_add(0x18));
                fb = esi;
                if (fa != fb) { pc = 0x1ba940; continue; }
                pc = 0x1ba994; continue;
            }
            0x1ba994 => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1ba99b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BA9B0. INFERRED: read in full: mirror of 1ba8f0 stepping backward through order table 4fc888 and wrapping at 0x45
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn weapon_slot_order_previous_selectable_entry_ba<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1ba9b0;
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
        { crate::pose::cover(0x1ba9b0, previous, pc); previous = pc; }
        match pc {
            0x1ba9b0 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x10));
                push!(esi);
                eax = 0;
                pc = 0x1ba9c0; continue;
            }
            0x1ba9c0 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x4fc888));
                fb = ebp;
                if (fa == fb) { pc = 0x1ba9f9; continue; }
                pc = 0x1ba9c9; continue;
            }
            0x1ba9c9 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1ba9c0; continue; }
                pc = 0x1ba9cf; continue;
            }
            0x1ba9cf => {
                m.write32(esp.wrapping_add(0x14), 0);
                pc = 0x1ba9d7; continue;
            }
            0x1ba9d7 => {
                ebx = m.read32(esp.wrapping_add(0x10));
                ecx = m.read32(ebx.wrapping_add(0x8));
                edx = m.read32(0x790950);
                esi = m.read32(esp.wrapping_add(0x14));
                eax = 0;
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = ebp;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1ba9ff; continue; }
                pc = 0x1ba9f3; continue;
            }
            0x1ba9f3 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1ba9f9 => {
                m.write32(esp.wrapping_add(0x14), eax);
                pc = 0x1ba9d7; continue;
            }
            0x1ba9ff => {
                push!(edi);
                pc = 0x1baa00; continue;
            }
            0x1baa00 => {
                edx = m.read32(0x790950);
                edi = m.read32(ebx.wrapping_add(0x8));
                eax = 0;
                esi = esi.wrapping_sub(1);
                fa = edi;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1baa22; continue; }
                pc = 0x1baa15; continue;
            }
            0x1baa15 => {
                eax = m.read32(esp.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1baa5e; continue; }
                pc = 0x1baa1d; continue;
            }
            0x1baa1d => {
                esi = 0x45;
                pc = 0x1baa22; continue;
            }
            0x1baa22 => {
                edi = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x4fc888));
                eax = edi;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                edx = m.read32(eax.wrapping_add(0x4fa160));
                ecx = ebp;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = edx;
                fb = m.read32(ecx.wrapping_add(0x4fa160));
                if (fa != fb) { pc = 0x1baa51; continue; }
                pc = 0x1baa41; continue;
            }
            0x1baa41 => {
                push!(0x1);
                push!(edi);
                push!(ebx);
                call!(0x1ba410);
                pc = 0x1baa4a; continue;
            }
            0x1baa4a => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1baa57; continue; }
                pc = 0x1baa51; continue;
            }
            0x1baa51 => {
                fa = m.read32(esp.wrapping_add(0x18));
                fb = esi;
                if (fa != fb) { pc = 0x1baa00; continue; }
                pc = 0x1baa57; continue;
            }
            0x1baa57 => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1baa5e => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = ebp;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BAA70. INFERRED: 124 B read in full: player record +8 compared to count 790950, current entry byte at player +0xe3c via entry table 4fa160, slot_order_4fc888_find_index_whose_slot_field_0x38_equals_arg, selectable predicate 1ba410 on both, returns 0 when ei
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn weapon_slot_player_returns_1_unless_current_en<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1baa70;
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
        { crate::pose::cover(0x1baa70, previous, pc); previous = pc; }
        match pc {
            0x1baa70 => {
                push!(ebx);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0xc));
                fa = ebp;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                push!(esi);
                push!(edi);
                if (fa == fb) { pc = 0x1baae2; continue; }
                pc = 0x1baa7c; continue;
            }
            0x1baa7c => {
                eax = m.read32(ebp.wrapping_add(0x8));
                fa = eax;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1baae2; continue; }
                pc = 0x1baa87; continue;
            }
            0x1baa87 => {
                edi = m.read32(esp.wrapping_add(0x18));
                eax = ((eax as i32).wrapping_mul((0x1584) as i32)) as u32;
                edx = m.read32(0x7b0c48);
                ecx = edi;
                ecx = ((ecx as i32).wrapping_mul((0x3c) as i32)) as u32;
                ecx = m.read32(ecx.wrapping_add(0x4fa160));
                eax = eax.wrapping_add(edx);
                esi = ((m.read8(eax.wrapping_add(ecx).wrapping_add(0xe3c))) as u8 as u32);
                call!(0x1b6bb0);
                pc = 0x1baab1; continue;
            }
            0x1baab1 => {
                fa = esi;
                fb = edi;
                ebx = eax;
                if (fa == fb) { pc = 0x1baae2; continue; }
                pc = 0x1baab7; continue;
            }
            0x1baab7 => {
                push!(0x1);
                push!(esi);
                push!(ebp);
                call!(0x1ba410);
                pc = 0x1baac0; continue;
            }
            0x1baac0 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1baadb; continue; }
                pc = 0x1baac7; continue;
            }
            0x1baac7 => {
                fa = ebx;
                fb = edi;
                if (fa == fb) { pc = 0x1baae2; continue; }
                pc = 0x1baacb; continue;
            }
            0x1baacb => {
                push!(0x1);
                push!(ebx);
                push!(ebp);
                call!(0x1ba410);
                pc = 0x1baad4; continue;
            }
            0x1baad4 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1baae2; continue; }
                pc = 0x1baadb; continue;
            }
            0x1baadb => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1baae2 => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                eax = 0x1;
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BAAF0. INFERRED: read in full: starts from player +0x94 current entry in order table 4fc888, walks forward, skips same-group entries, requires 1baa70 and 1ba410 to accept
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_weapon_next_selectable_entry_from_curre<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1baaf0;
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
        { crate::pose::cover(0x1baaf0, previous, pc); previous = pc; }
        match pc {
            0x1baaf0 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(ebx.wrapping_add(0x94));
                push!(ebp);
                push!(esi);
                push!(edi);
                eax = 0;
                pc = 0x1bab00; continue;
            }
            0x1bab00 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x4fc888));
                fb = ecx;
                if (fa == fb) { pc = 0x1bab82; continue; }
                pc = 0x1bab09; continue;
            }
            0x1bab09 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1bab00; continue; }
                pc = 0x1bab0f; continue;
            }
            0x1bab0f => {
                ebp = 0;
                pc = 0x1bab11; continue;
            }
            0x1bab11 => {
                edi = ebp;
                pc = 0x1bab13; continue;
            }
            0x1bab13 => {
                edi = edi.wrapping_add(1);
                fa = edi;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1bab33; continue; }
                pc = 0x1bab19; continue;
            }
            0x1bab19 => {
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bab86; continue; }
                pc = 0x1bab21; continue;
            }
            0x1bab21 => {
                ecx = m.read32(ebx.wrapping_add(0x8));
                edx = m.read32(0x790950);
                eax = 0;
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                edi = eax;
                pc = 0x1bab33; continue;
            }
            0x1bab33 => {
                esi = m.read32(edi.wrapping_mul(0x4).wrapping_add(0x4fc888));
                edx = m.read32(ebx.wrapping_add(0x94));
                eax = esi;
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x4fa160));
                fa = ecx;
                fb = m.read32(edx.wrapping_add(0x4fa160));
                if (fa == fb) { pc = 0x1bab77; continue; }
                pc = 0x1bab56; continue;
            }
            0x1bab56 => {
                push!(esi);
                push!(ebx);
                call!(0x1baa70);
                pc = 0x1bab5d; continue;
            }
            0x1bab5d => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bab77; continue; }
                pc = 0x1bab64; continue;
            }
            0x1bab64 => {
                edx = m.read32(esp.wrapping_add(0x1c));
                push!(edx);
                push!(esi);
                push!(ebx);
                call!(0x1ba410);
                pc = 0x1bab70; continue;
            }
            0x1bab70 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bab7b; continue; }
                pc = 0x1bab77; continue;
            }
            0x1bab77 => {
                fa = ebp;
                fb = edi;
                if (fa != fb) { pc = 0x1bab13; continue; }
                pc = 0x1bab7b; continue;
            }
            0x1bab7b => {
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                eax = esi;
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1bab82 => {
                ebp = eax;
                pc = 0x1bab11; continue;
            }
            0x1bab86 => {
                eax = m.read32(ebx.wrapping_add(0x94));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BABA0. INFERRED: read in full: backward mirror of 1baaf0 over order table 4fc888
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_weapon_previous_selectable_entry_from_c<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1baba0;
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
        { crate::pose::cover(0x1baba0, previous, pc); previous = pc; }
        match pc {
            0x1baba0 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x8));
                ecx = m.read32(ebx.wrapping_add(0x94));
                push!(ebp);
                push!(esi);
                push!(edi);
                eax = 0;
                pc = 0x1babb0; continue;
            }
            0x1babb0 => {
                fa = m.read32(eax.wrapping_mul(0x4).wrapping_add(0x4fc888));
                fb = ecx;
                if (fa == fb) { pc = 0x1bac34; continue; }
                pc = 0x1babb9; continue;
            }
            0x1babb9 => {
                eax = eax.wrapping_add(1);
                fa = eax;
                fb = ((((0x46i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1babb0; continue; }
                pc = 0x1babbf; continue;
            }
            0x1babbf => {
                ebp = 0;
                pc = 0x1babc1; continue;
            }
            0x1babc1 => {
                esi = ebp;
                pc = 0x1babc3; continue;
            }
            0x1babc3 => {
                ecx = m.read32(ebx.wrapping_add(0x8));
                edx = m.read32(0x790950);
                eax = 0;
                esi = esi.wrapping_sub(1);
                fa = ecx;
                fb = edx;
                fas = (fa as i32);
                fbs = (fb as i32);
                eax = (eax & 0xffff_ff00) | ((((if ((fas < fbs)) { 0x1i32 } else { 0i32 }) as u32)) & 0xff);
                fa = esi;
                fb = eax;
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1babe5; continue; }
                pc = 0x1babd8; continue;
            }
            0x1babd8 => {
                eax = m.read32(esp.wrapping_add(0x18));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bac38; continue; }
                pc = 0x1babe0; continue;
            }
            0x1babe0 => {
                esi = 0x45;
                pc = 0x1babe5; continue;
            }
            0x1babe5 => {
                edi = m.read32(esi.wrapping_mul(0x4).wrapping_add(0x4fc888));
                edx = m.read32(ebx.wrapping_add(0x94));
                eax = edi;
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                eax = ((eax as i32).wrapping_mul((0x3c) as i32)) as u32;
                ecx = m.read32(eax.wrapping_add(0x4fa160));
                fa = ecx;
                fb = m.read32(edx.wrapping_add(0x4fa160));
                if (fa == fb) { pc = 0x1bac29; continue; }
                pc = 0x1bac08; continue;
            }
            0x1bac08 => {
                push!(edi);
                push!(ebx);
                call!(0x1baa70);
                pc = 0x1bac0f; continue;
            }
            0x1bac0f => {
                esp = esp.wrapping_add(0x8);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bac29; continue; }
                pc = 0x1bac16; continue;
            }
            0x1bac16 => {
                edx = m.read32(esp.wrapping_add(0x1c));
                push!(edx);
                push!(edi);
                push!(ebx);
                call!(0x1ba410);
                pc = 0x1bac22; continue;
            }
            0x1bac22 => {
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bac2d; continue; }
                pc = 0x1bac29; continue;
            }
            0x1bac29 => {
                fa = ebp;
                fb = esi;
                if (fa != fb) { pc = 0x1babc3; continue; }
                pc = 0x1bac2d; continue;
            }
            0x1bac2d => {
                eax = edi;
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1bac34 => {
                ebp = eax;
                pc = 0x1babc1; continue;
            }
            0x1bac38 => {
                eax = m.read32(ebx.wrapping_add(0x94));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BACA0. INFERRED: read in full: checks controller +0x150, current entry +0x94 records ammo types, special entries 0x2c and 0x2d swap, else picks 1baaf0 or 1baba0 result and calls object_set_current_weapon_slot
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_weapon_auto_switch_when_current_unusabl<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1baca0;
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
        { crate::pose::cover(0x1baca0, previous, pc); previous = pc; }
        match pc {
            0x1baca0 => {
                push!(esi);
                esi = m.read32(esp.wrapping_add(0x8));
                eax = m.read32(esi);
                eax = m.read32(eax.wrapping_add(0x150));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bacbc; continue; }
                pc = 0x1bacb1; continue;
            }
            0x1bacb1 => {
                ecx = m.read32(eax.wrapping_add(0x4));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1badc4; continue; }
                pc = 0x1bacbc; continue;
            }
            0x1bacbc => {
                ecx = m.read32(esi.wrapping_add(0x94));
                edx = ecx;
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = ecx;
                fb = ((((0x2di32)) as u32) & 0xffffffffu32);
                eax = m.read32(edx.wrapping_add(0x4fa138));
                if (fa != fb) { pc = 0x1bace5; continue; }
                pc = 0x1bacd2; continue;
            }
            0x1bacd2 => {
                push!(esi);
                call!(0xe02b0);
                pc = 0x1bacd8; continue;
            }
            0x1bacd8 => {
                esp = esp.wrapping_add(0x4);
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1badc4; continue; }
                pc = 0x1bace3; continue;
            }
            0x1bace3 => {
                pc = 0x1bad05; continue;
            }
            0x1bace5 => {
                ecx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = eax.wrapping_add(ecx);
                ecx = m.read32(eax.wrapping_add(0x14));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bad05; continue; }
                pc = 0x1bacfa; continue;
            }
            0x1bacfa => {
                ecx = m.read32(eax.wrapping_add(0x1c));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1badc4; continue; }
                pc = 0x1bad05; continue;
            }
            0x1bad05 => {
                eax = m.read32(esi.wrapping_add(0x94));
                fa = eax;
                fb = ((((0x2ci32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bad2c; continue; }
                pc = 0x1bad10; continue;
            }
            0x1bad10 => {
                edx = m.read32(esi.wrapping_add(0x8));
                fa = edx;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1bad2c; continue; }
                pc = 0x1bad1b; continue;
            }
            0x1bad1b => {
                push!(0x2d);
                push!(esi);
                call!(0x1ba130);
                pc = 0x1bad23; continue;
            }
            0x1bad23 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                tail_to!(0x142880); break 'run;  // direct tail jump
            }
            0x1bad2c => {
                fa = eax;
                fb = ((((0x2di32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bad3c; continue; }
                pc = 0x1bad31; continue;
            }
            0x1bad31 => {
                push!(0x2c);
                push!(esi);
                call!(0x1ba130);
                pc = 0x1bad39; continue;
            }
            0x1bad39 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x1bad3c; continue;
            }
            0x1bad3c => {
                push!(0);
                push!(0);
                push!(esi);
                call!(0x1baaf0);
                pc = 0x1bad46; continue;
            }
            0x1bad46 => {
                ecx = m.read32(esi.wrapping_add(0x94));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x1bad62; continue; }
                pc = 0x1bad53; continue;
            }
            0x1bad53 => {
                fa = eax;
                fb = ((((0x1i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bad62; continue; }
                pc = 0x1bad58; continue;
            }
            0x1bad58 => {
                fa = eax;
                fb = ((((0x38i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bad62; continue; }
                pc = 0x1bad5d; continue;
            }
            0x1bad5d => {
                fa = eax;
                fb = ((((0x39i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1bad6f; continue; }
                pc = 0x1bad62; continue;
            }
            0x1bad62 => {
                push!(0);
                push!(0);
                push!(esi);
                call!(0x1baba0);
                pc = 0x1bad6c; continue;
            }
            0x1bad6c => {
                esp = esp.wrapping_add(0xc);
                pc = 0x1bad6f; continue;
            }
            0x1bad6f => {
                fa = eax;
                fb = m.read32(esi.wrapping_add(0x94));
                if (fa != fb) { pc = 0x1bad8e; continue; }
                pc = 0x1bad77; continue;
            }
            0x1bad77 => {
                push!(0);
                push!(0);
                push!(esi);
                call!(0x1baaf0);
                pc = 0x1bad81; continue;
            }
            0x1bad81 => {
                ecx = m.read32(esi.wrapping_add(0x94));
                esp = esp.wrapping_add(0xc);
                fa = eax;
                fb = ecx;
                if (fa == fb) { pc = 0x1bad92; continue; }
                pc = 0x1bad8e; continue;
            }
            0x1bad8e => {
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1badba; continue; }
                pc = 0x1bad92; continue;
            }
            0x1bad92 => {
                eax = m.read32(esi.wrapping_add(0x8));
                fa = eax;
                fb = m.read32(0x790950);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas >= fbs) { pc = 0x1badb8; continue; }
                pc = 0x1bad9d; continue;
            }
            0x1bad9d => {
                eax = m.read32(0x7de458);
                fa = (((((((eax) >> 8) as u8))) as u32) & 0xffu32);
                fb = ((((0x20i32)) as u32) & 0xffu32);
                if ((fa & fb) == 0) { pc = 0x1badb8; continue; }
                pc = 0x1bada7; continue;
            }
            0x1bada7 => {
                eax = 0x1;
                push!(eax);
                push!(esi);
                call!(0x1ba130);
                pc = 0x1badb3; continue;
            }
            0x1badb3 => {
                esp = esp.wrapping_add(0x8);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            0x1badb8 => {
                eax = 0;
                pc = 0x1badba; continue;
            }
            0x1badba => {
                push!(eax);
                push!(esi);
                call!(0x1ba130);
                pc = 0x1badc1; continue;
            }
            0x1badc1 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x1badc4; continue;
            }
            0x1badc4 => {
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


/// 0x1BFEC0. INFERRED: read in full: for entries 1 and up not owned (byte +0x54c clear) whose flag has 0x210 and record ammo type equals arg collects via 1bfca0, then switches to a matching owned entry when both ammo pools are empty
#[allow(unused_assignments, unused_mut, unused_variables, unused_macros, clippy::all)]
pub fn player_weapon_collect_unowned_entries_matching<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let (mut eax, mut ecx, mut edx, mut esp) = (entry.eax, entry.ecx, entry.edx, entry.esp);
    let (mut ebx, mut ebp, mut esi, mut edi) = (entry.ebx, entry.ebp, entry.esi, entry.edi);
    let mut x = entry.xmm;
    let mut xh = entry.xmm_hi; // XMM5-7: only callees change them
    let fpu = entry.fpu;
    let (mut fa, mut fb, mut fas, mut fbs, mut cf) = (0u32, 0u32, 0i32, 0i32, 0i32);
    let mut pc: u32 = 0x1bfec0;
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
        { crate::pose::cover(0x1bfec0, previous, pc); previous = pc; }
        match pc {
            0x1bfec0 => {
                esp = esp.wrapping_sub(0x8);
                push!(ebp);
                ebp = m.read32(esp.wrapping_add(0x10));
                eax = m.read32(ebp);
                ecx = m.read32(eax.wrapping_add(0x150));
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                m.write32(esp.wrapping_add(0x4), ecx);
                m.write32(esp.wrapping_add(0x8), 0);
                if (fa != fb) { pc = 0x1bfee8; continue; }
                pc = 0x1bfee1; continue;
            }
            0x1bfee1 => {
                eax = 0;
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            0x1bfee8 => {
                push!(ebx);
                ebx = m.read32(esp.wrapping_add(0x18));
                push!(esi);
                push!(edi);
                edi = 0x1;
                esi = 0x4fa174;
                pc = 0x1bff00; continue;
            }
            0x1bff00 => {
                eax = (eax & 0xffff_ff00) | ((((m.read8(edi.wrapping_add(ebp).wrapping_add(0x54c))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa != fb) { pc = 0x1bff4f; continue; }
                pc = 0x1bff0b; continue;
            }
            0x1bff0b => {
                eax = m.read32(esi);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bff4f; continue; }
                pc = 0x1bff12; continue;
            }
            0x1bff12 => {
                edx = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = eax.wrapping_add(edx);
                fa = ((m.read32(esi.wrapping_add(((0x10i32.wrapping_neg()) as u32)))) & 0xffffffffu32);
                fb = ((((0x210i32)) as u32) & 0xffffffffu32);
                if ((fa & fb) == 0) { pc = 0x1bff4f; continue; }
                pc = 0x1bff29; continue;
            }
            0x1bff29 => {
                fa = m.read32(eax.wrapping_add(0x14));
                fb = ebx;
                if (fa == fb) { pc = 0x1bff39; continue; }
                pc = 0x1bff2e; continue;
            }
            0x1bff2e => {
                eax = m.read32(eax.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bff4f; continue; }
                pc = 0x1bff35; continue;
            }
            0x1bff35 => {
                fa = eax;
                fb = ebx;
                if (fa != fb) { pc = 0x1bff4f; continue; }
                pc = 0x1bff39; continue;
            }
            0x1bff39 => {
                push!(edi);
                push!(ebp);
                call!(0x1bfca0);
                pc = 0x1bff40; continue;
            }
            0x1bff40 => {
                ecx = m.read32(esp.wrapping_add(0x18));
                esp = esp.wrapping_add(0x8);
                m.write32(esp.wrapping_add(0x14), 0x1);
                pc = 0x1bff4f; continue;
            }
            0x1bff4f => {
                esi = esi.wrapping_add(0x3c);
                edi = edi.wrapping_add(1);
                fa = esi;
                fb = ((((0x4fb1a0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1bff00; continue; }
                pc = 0x1bff5b; continue;
            }
            0x1bff5b => {
                eax = m.read32(esp.wrapping_add(0x24));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1c0073; continue; }
                pc = 0x1bff67; continue;
            }
            0x1bff67 => {
                eax = m.read32(ecx.wrapping_add(0x1500));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1c0073; continue; }
                pc = 0x1bff75; continue;
            }
            0x1bff75 => {
                edx = m.read32(ecx.wrapping_add(0x4));
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas < fbs) { pc = 0x1c0073; continue; }
                pc = 0x1bff80; continue;
            }
            0x1bff80 => {
                eax = m.read32(eax.wrapping_add(0x68));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bff90; continue; }
                pc = 0x1bff87; continue;
            }
            0x1bff87 => {
                fa = eax;
                fb = ((((0x2i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1c0073; continue; }
                pc = 0x1bff90; continue;
            }
            0x1bff90 => {
                m.write32(esp.wrapping_add(0x10), 0x1);
                esi = 0x4fa174;
                pc = 0x1bffa0; continue;
            }
            0x1bffa0 => {
                ecx = m.read32(esp.wrapping_add(0x10));
                eax = (eax & 0xffff_ff00) | ((((m.read8(ecx.wrapping_add(ebp).wrapping_add(0x54c))) as u32)) & 0xff);
                fa = ((((((eax) as u8))) as u32) & 0xffu32);
                fb = ((((0i32)) as u32) & 0xffu32);
                if (fa == fb) { pc = 0x1c005b; continue; }
                pc = 0x1bffb3; continue;
            }
            0x1bffb3 => {
                eax = m.read32(esi.wrapping_add(((0x4i32.wrapping_neg()) as u32)));
                ecx = 0;
                edx = 0;
                edi = 0;
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfff6; continue; }
                pc = 0x1bffc1; continue;
            }
            0x1bffc1 => {
                ebp = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = eax.wrapping_add(ebp);
                fa = m.read32(eax.wrapping_add(0x14));
                fb = ebx;
                if (fa == fb) { pc = 0x1bffdf; continue; }
                pc = 0x1bffd4; continue;
            }
            0x1bffd4 => {
                eax = m.read32(eax.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1bfff2; continue; }
                pc = 0x1bffdb; continue;
            }
            0x1bffdb => {
                fa = eax;
                fb = ebx;
                if (fa != fb) { pc = 0x1bfff2; continue; }
                pc = 0x1bffdf; continue;
            }
            0x1bffdf => {
                ebp = m.read32(esp.wrapping_add(0x1c));
                edx = (((((m.read16(ebp.wrapping_add(ebx.wrapping_mul(0x2)).wrapping_add(0xdc)) as i16)) as i32)) as u32);
                ecx = 0x1;
                pc = 0x1bfff6; continue;
            }
            0x1bfff2 => {
                ebp = m.read32(esp.wrapping_add(0x1c));
                pc = 0x1bfff6; continue;
            }
            0x1bfff6 => {
                eax = m.read32(esi);
                fa = eax;
                fb = ((0xffffffffu32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1c002d; continue; }
                pc = 0x1bfffd; continue;
            }
            0x1bfffd => {
                ebp = m.read32(0x4f9bac);
                eax = ((eax as i32).wrapping_mul((0x29c) as i32)) as u32;
                eax = eax.wrapping_add(ebp);
                fa = m.read32(eax.wrapping_add(0x14));
                fb = ebx;
                if (fa == fb) { pc = 0x1c001b; continue; }
                pc = 0x1c0010; continue;
            }
            0x1c0010 => {
                eax = m.read32(eax.wrapping_add(0x1c));
                fa = eax;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1c0029; continue; }
                pc = 0x1c0017; continue;
            }
            0x1c0017 => {
                fa = eax;
                fb = ebx;
                if (fa != fb) { pc = 0x1c0029; continue; }
                pc = 0x1c001b; continue;
            }
            0x1c001b => {
                ebp = m.read32(esp.wrapping_add(0x1c));
                edi = (((((m.read16(ebp.wrapping_add(ebx.wrapping_mul(0x2)).wrapping_add(0x20c)) as i16)) as i32)) as u32);
                pc = 0x1c0031; continue;
            }
            0x1c0029 => {
                ebp = m.read32(esp.wrapping_add(0x1c));
                pc = 0x1c002d; continue;
            }
            0x1c002d => {
                fa = ecx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa == fb) { pc = 0x1c005b; continue; }
                pc = 0x1c0031; continue;
            }
            0x1c0031 => {
                fa = edx;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1c005b; continue; }
                pc = 0x1c0035; continue;
            }
            0x1c0035 => {
                fa = edi;
                fb = ((((0i32)) as u32) & 0xffffffffu32);
                if (fa != fb) { pc = 0x1c005b; continue; }
                pc = 0x1c0039; continue;
            }
            0x1c0039 => {
                edx = m.read32(ebp.wrapping_add(0x94));
                eax = m.read32(esi.wrapping_add(((0x8i32.wrapping_neg()) as u32)));
                edx = ((edx as i32).wrapping_mul((0x3c) as i32)) as u32;
                fa = eax;
                fb = m.read32(edx.wrapping_add(0x4fa130));
                fas = (fa as i32);
                fbs = (fb as i32);
                if (fas > fbs) { pc = 0x1c005b; continue; }
                pc = 0x1c004d; continue;
            }
            0x1c004d => {
                ecx = m.read32(esp.wrapping_add(0x10));
                push!(ecx);
                push!(ebp);
                call!(0x1ba130);
                pc = 0x1c0058; continue;
            }
            0x1c0058 => {
                esp = esp.wrapping_add(0x8);
                pc = 0x1c005b; continue;
            }
            0x1c005b => {
                edx = m.read32(esp.wrapping_add(0x10));
                edx = edx.wrapping_add(1);
                esi = esi.wrapping_add(0x3c);
                fa = esi;
                fb = ((((0x4fb1a0i32)) as u32) & 0xffffffffu32);
                fas = (fa as i32);
                fbs = (fb as i32);
                m.write32(esp.wrapping_add(0x10), edx);
                if (fas < fbs) { pc = 0x1bffa0; continue; }
                pc = 0x1c0073; continue;
            }
            0x1c0073 => {
                eax = m.read32(esp.wrapping_add(0x14));
                edi = m.read32(esp); esp = esp.wrapping_add(4);
                esi = m.read32(esp); esp = esp.wrapping_add(4);
                ebx = m.read32(esp); esp = esp.wrapping_add(4);
                ebp = m.read32(esp); esp = esp.wrapping_add(4);
                esp = esp.wrapping_add(0x8);
                break 'run;
            }
            _ => unreachable!(),
        }
    }
    *regs = ExtRegisters { eax, ecx, edx, esp, ebx, ebp, esi, edi, xmm: x, fpu, xmm_hi: xh, ..entry };
}


// ---- translated bodies end ----

// ---- dispatch begin ----
/// Roots ported in this module (differential-only, unadmitted).
pub const ROOTS: [u32; 13] = [0x1b9ab0, 0x1b9ad0, 0x1b6bb0, 0x1b6bf0, 0x1ba1b0, 0x1bfa60, 0x1ba8f0, 0x1ba9b0, 0x1baa70, 0x1baaf0, 0x1baba0, 0x1baca0, 0x1bfec0];

/// Differential entry (not selector-registered). True when the Rust body ran.
#[allow(unused_variables)]
pub fn run<M: GuestMemory, C: GuestCall>(va: u32, m: &mut M, guest: &mut C, regs: &mut ExtRegisters) -> bool {
    match va {
        0x1b9ab0 => get_weapon_slot_4fa128_float_0x34_by_object_cu(m, regs),
        0x1b9ad0 => get_weapon_slot_4fa128_float_0x30_by_object_cu(m, regs),
        0x1b6bb0 => slot_order_4fc888_find_index_whose_slot_field(m, guest, regs),
        0x1b6bf0 => slot_order_4fc888_next_owned_slot_with_flag_0x(m, guest, regs),
        0x1ba1b0 => object_realign_current_weapon_slot_0x94_to_cha(m, guest, regs),
        0x1bfa60 => object_select_weapon_slot_and_set_player_flag(m, guest, regs),
        0x1ba8f0 => weapon_slot_order_next_selectable_entry_forwar(m, guest, regs),
        0x1ba9b0 => weapon_slot_order_previous_selectable_entry_ba(m, guest, regs),
        0x1baa70 => weapon_slot_player_returns_1_unless_current_en(m, guest, regs),
        0x1baaf0 => player_weapon_next_selectable_entry_from_curre(m, guest, regs),
        0x1baba0 => player_weapon_previous_selectable_entry_from_c(m, guest, regs),
        0x1baca0 => player_weapon_auto_switch_when_current_unusabl(m, guest, regs),
        0x1bfec0 => player_weapon_collect_unowned_entries_matching(m, guest, regs),
        _ => return false,
    }
    true
}
// ---- dispatch end ----
