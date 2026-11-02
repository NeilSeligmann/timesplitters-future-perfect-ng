// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4b object ports (Tier 0). Readable translations of lifted originals; nested guest
//! calls go through the closure model (`Callee`/`guest_call`). Register residue is not modelled.
use super::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

const SLOT_FLAGS_ARRAY: u32 = 0x007A_2820;
const NAV_RECORD_TABLE: u32 = 0x0074_C36C;
const TRIGGER_TABLE: u32 = 0x0078_CD90;
const COMPLETE_TARGET: u32 = 0x0078_CED0;
const PLAYER_COUNT: u32 = 0x0079_0950;
const PLAYER_BASE: u32 = 0x007B_0C48;
const PLAYER_STRIDE: u32 = 0x1584;
const ZERO_FLOAT: u32 = 0x0047_5cac;
const FRAME_UNIT: u32 = 0x0047_5c78;
const TICK_SCALE: u32 = 0x004e_794c;
const TICK_COUNT: u32 = 0x004e_8cec;

fn f32_at<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

fn put_f32<M: GuestMemory>(m: &mut M, address: u32, value: f32) {
    m.write32(address, value.to_bits());
}

fn tick_step<M: GuestMemory>(m: &M) -> f32 {
    f32_at(m, TICK_SCALE) / (m.read32(TICK_COUNT) as i32 as f32)
}

/// Branch taken by `comiss`/`jb`: below or unordered.
fn below_or_nan(a: f32, b: f32) -> bool {
    !(a >= b)
}

// ---------------------------------------------------------------------------------------------
// 0x235240 trigger condition state machine
// ---------------------------------------------------------------------------------------------

/// Calls the completion callee 2274C0([78CED0], 0), the shared tail of every satisfied condition.
fn complete_trigger<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    // Tail jump: the arguments overwrite the caller's own two argument slots (event, mode) and
    // 2274C0 runs on the unchanged frame.
    let target = m.read32(COMPLETE_TARGET);
    m.write32(cpu.esp.wrapping_add(4), target);
    m.write32(cpu.esp.wrapping_add(8), 0);
    c.call(m, 0x0022_74c0, cpu);
}

/// Do any of the node's four id fields (+8, +0xC, +0x10, +0x14) equal `id`.
fn node_lists_id<M: GuestMemory>(m: &M, node: u32, id: u32) -> bool {
    [8u32, 0xC, 0x10, 0x14]
        .iter()
        .any(|field| m.read32(node.wrapping_add(*field)) == id)
}

/// Condition kind for each node type (node+0). Kinds without an arm do nothing (jump table
/// entries that point at the plain return).
fn condition_holds<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    kind: u32,
    node: u32,
    slot: u32,
) -> bool {
    let field = |m: &M, offset: u32| m.read32(node.wrapping_add(offset));
    match kind {
        // 18: plain predicate.
        18 => {
            guest_call(c, m, cpu, 0x0006_1ca0, 0x0023_52ac, 12, &[]);
            cpu.eax != 0
        }
        // 4: predicate on the slot's first word.
        4 => {
            let word = m.read32(slot);
            guest_call(c, m, cpu, 0x0023_4270, 0x0023_52d6, 12, &[word]);
            cpu.eax != 0
        }
        // 16: two objects of type 8 (looked up by id, or by negative player index) must match
        // via 12FA0(left record +8, right record +8).
        16 => both_type_8_objects_match(c, m, cpu, node),
        // 1: some live player (strength > 0) has an id listed in the node.
        1 => {
            let count = m.read32(PLAYER_COUNT) as i32;
            for index in 0..count.max(0) as u32 {
                let player = m.read32(
                    m.read32(PLAYER_BASE)
                        .wrapping_add(index * PLAYER_STRIDE + 0x14),
                );
                if !(f32_at(m, player.wrapping_add(0x1CC)) > f32_at(m, ZERO_FLOAT)) {
                    continue;
                }
                let record = m.read32(player.wrapping_add(0x7C));
                let key = m.read32(record.wrapping_add(0x678));
                guest_call(c, m, cpu, 0x0009_a4c0, 0x0023_53fa, 12, &[key]);
                if node_lists_id(m, node, cpu.eax) {
                    return true;
                }
            }
            false
        }
        // 6: object found through 15BA10(node+4), its player id must be listed.
        6 => {
            let key = field(m, 4);
            guest_call(c, m, cpu, 0x0015_ba10, 0x0023_5440, 12, &[key]);
            if cpu.eax == 0 {
                return false;
            }
            let record = m.read32(cpu.eax.wrapping_add(0x7C));
            let id_key = m.read32(record.wrapping_add(0x678));
            guest_call(c, m, cpu, 0x0009_a4c0, 0x0023_545a, 12, &[id_key]);
            node_lists_id(m, node, cpu.eax)
        }
        // 15: object found through 23FF20(node+4) with type 0x400000, resolved via 177910.
        15 => {
            let key = field(m, 4);
            guest_call(c, m, cpu, 0x0023_ff20, 0x0023_5468, 12, &[key]);
            let object = cpu.eax;
            if object == 0 || m.read32(object.wrapping_add(0x20)) != 0x0040_0000 {
                return false;
            }
            guest_call(c, m, cpu, 0x0017_7910, 0x0023_5486, 12, &[object]);
            let first = m.read32(cpu.eax);
            guest_call(c, m, cpu, 0x0009_a4c0, 0x0023_548e, 16, &[first]);
            node_lists_id(m, node, cpu.eax)
        }
        // 10: B0EA0(node+4) yields an object whose B0770 result must be nonzero.
        10 => {
            let key = field(m, 4);
            guest_call(c, m, cpu, 0x000b_0ea0, 0x0023_54cc, 12, &[key]);
            if cpu.eax == 0 {
                return false;
            }
            let object = cpu.eax;
            guest_call(c, m, cpu, 0x000b_0770, 0x0023_54dd, 12, &[object]);
            cpu.eax != 0
        }
        // 8: word at 7A2D24 + node[8]*96 nonzero.
        8 => {
            let index = field(m, 8);
            m.read32(index.wrapping_mul(96).wrapping_add(0x007A_2D24)) != 0
        }
        // 17: 25FA10(node+8) selects an entry at 78B2F4 + n*32 that must equal 1.
        17 => {
            let key = field(m, 8);
            guest_call(c, m, cpu, 0x0025_fa10, 0x0023_5519, 12, &[key]);
            m.read32(cpu.eax.wrapping_shl(5).wrapping_add(0x0078_B2F4)) == 1
        }
        // 9: 2273C0(node+8) nonzero.
        9 => {
            let key = field(m, 8);
            guest_call(c, m, cpu, 0x0022_73c0, 0x0023_5551, 12, &[key]);
            cpu.eax != 0
        }
        // 11: 25F370(node+4, &out) result must be nonzero-out and equal to out.
        11 => {
            let key = field(m, 4);
            // The out slot is the event argument slot of this call frame (reused as scratch).
            let out = cpu.esp.wrapping_add(4);
            m.write32(out, 0);
            guest_call(c, m, cpu, 0x0025_f370, 0x0023_558b, 12, &[key, out]);
            let value = m.read32(out);
            value != 0 && cpu.eax == value
        }
        _ => false,
    }
}

/// Kind 16 body. A non-negative node+0xC names one object by id (15BA10), a negative one picks
/// the per-player object table instead and scans every player with the same test.
fn both_type_8_objects_match<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    node: u32,
) -> bool {
    let key = m.read32(node.wrapping_add(8));
    guest_call(c, m, cpu, 0x0015_ba10, 0x0023_5304, 12, &[key]);
    let left = cpu.eax;
    if left == 0 || m.read32(left.wrapping_add(0x20)) != 8 {
        return false;
    }
    let left_record = m.read32(left.wrapping_add(0x7C));
    let second = m.read32(node.wrapping_add(0xC)) as i32;
    let matches = |c: &mut C, m: &mut M, cpu: &mut Cpu, object: u32, pc: u32| -> bool {
        let right_record = m.read32(object.wrapping_add(0x7C));
        let right_id = m.read32(right_record.wrapping_add(8));
        let left_id = m.read32(left_record.wrapping_add(8));
        guest_call(c, m, cpu, 0x0001_2fa0, pc, 16, &[right_id, left_id]);
        cpu.eax != 0
    };
    if second >= 0 {
        guest_call(c, m, cpu, 0x0015_ba10, 0x0023_537c, 16, &[second as u32]);
        let right = cpu.eax;
        if right == 0 || m.read32(right.wrapping_add(0x20)) != 8 {
            return false;
        }
        return matches(c, m, cpu, right, 0x0023_5398);
    }
    for _ in 0..(m.read32(PLAYER_COUNT) as i32).max(0) {
        let base = m
            .read32(PLAYER_BASE)
            .wrapping_sub((second as u32).wrapping_mul(PLAYER_STRIDE));
        let object = m.read32(base.wrapping_sub(5488));
        if object != 0
            && m.read32(object.wrapping_add(0x20)) == 8
            && matches(c, m, cpu, object, 0x0023_5360)
        {
            return true;
        }
    }
    false
}

// portreg: va=0x00235240
/// 00235240 cdecl(event, mode): trigger condition check for the entry named by the u16 pair at
/// event+0 / event+0x2A in the 72-byte table at [78CD90]. Mode 4 latches the entry's flag word
/// (+4 of the active slot) once. Mode 3 evaluates the node's condition (kinds 1..18) and on
/// success calls the completion 2274C0([78CED0], 0). Other modes do nothing.
pub fn trigger_condition_check<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let saved = (cpu.ebx, cpu.esi, cpu.edi);
    trigger_condition_check_inner(c, m, cpu);
    cpu.ebx = saved.0;
    cpu.esi = saved.1;
    cpu.edi = saved.2;
}

fn trigger_condition_check_inner<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let event = stack_arg(m, cpu, 0);
    let mode = stack_arg(m, cpu, 1);
    // The lifted entry loads the mode into EAX and the trigger table base into EDX before
    // dispatch. Both remain caller-visible on paths that do not make another call.
    cpu.eax = mode;
    cpu.edx = m.read32(TRIGGER_TABLE);
    let entry_index = u32::from(m.read16(event.wrapping_add(0x2A)));
    let wanted = u32::from(m.read16(event));
    let entry = m
        .read32(TRIGGER_TABLE)
        .wrapping_add(entry_index.wrapping_mul(72));
    let head = m.read32(entry);
    let (slot, node) = if m.read32(entry.wrapping_add(0xC)) == wanted {
        (entry.wrapping_add(4), head.wrapping_add(0x40))
    } else {
        (entry.wrapping_add(0x10), head.wrapping_add(0x64))
    };
    match mode {
        4 => {
            if m.read16(slot.wrapping_add(4)) == 0 {
                m.write8(slot.wrapping_add(4), 1);
                m.write8(slot.wrapping_add(5), 0);
            }
        }
        3 => {
            let kind = m.read32(node);
            // The lifted jump-table dispatch decrements the node kind into EAX before its
            // unsigned range check; unhandled kinds return with that value still visible.
            cpu.eax = kind.wrapping_sub(1);
            if kind.wrapping_sub(1) > 0x11 {
                return;
            }
            if condition_holds(c, m, cpu, kind, node, slot) {
                complete_trigger(c, m, cpu);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// 0x25F520 slot node flag entry update
// ---------------------------------------------------------------------------------------------

/// 0025F520 cdecl(index): refreshes flag entry `index` (array 7A2820, 24-byte entries, slot
/// records at [[74C36C]+0x74] stride 0x98). Runs 1622C0(index) first and stops when the entry's
/// +0x10 is already 1. Otherwise the first record whose +8 equals this slot's +0x34 and for
/// which 6C890(rec+0xC, rec+0x2C) is nonzero decides: when that record's own entry word +8 is 1
/// the flag is set directly, else the flag is set when 23FF20(slot+0x34) names an object whose
/// player table entry ([obj+0x7C]+0xBF0) has +8 equal to slot+0x34 and 174E0(obj, 1, 0) is
/// nonzero.
pub fn slot_node_flag_entry_update<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let index = stack_arg(m, cpu, 0);
    let table = m.read32(NAV_RECORD_TABLE);
    let slot = m
        .read32(table.wrapping_add(0x74))
        .wrapping_add(index.wrapping_mul(0x98));
    let entry = m
        .read32(SLOT_FLAGS_ARRAY)
        .wrapping_add(index.wrapping_mul(24));
    guest_call(c, m, cpu, 0x0016_22c0, 0x0025_f550, 0x10, &[index]);
    if m.read32(entry.wrapping_add(0x10)) == 1 {
        return;
    }
    let slot_key = m.read32(slot.wrapping_add(0x34));
    let count = m.read32(m.read32(NAV_RECORD_TABLE).wrapping_add(0x78)) as i32;
    for other in 0..count.max(0) as u32 {
        let table = m.read32(NAV_RECORD_TABLE);
        let record = m
            .read32(table.wrapping_add(0x74))
            .wrapping_add(other * 0x98);
        if m.read32(record.wrapping_add(8)) != slot_key {
            continue;
        }
        let other_entry = m.read32(SLOT_FLAGS_ARRAY).wrapping_add(other * 24);
        let (first, second) = (
            m.read32(record.wrapping_add(0xC)),
            m.read32(record.wrapping_add(0x2C)),
        );
        guest_call(c, m, cpu, 0x0006_c890, 0x0025_f5a8, 0x14, &[first, second]);
        if cpu.eax == 0 {
            continue;
        }
        if m.read32(other_entry.wrapping_add(8)) == 1 {
            m.write32(entry.wrapping_add(0x10), 1);
            return;
        }
        guest_call(c, m, cpu, 0x0023_ff20, 0x0025_f5f2, 0x14, &[slot_key]);
        let object = cpu.eax;
        if object == 0 {
            return;
        }
        let player = m.read32(object.wrapping_add(0x7C));
        let record_ptr = m.read32(player.wrapping_add(0xBF0));
        if m.read32(record_ptr.wrapping_add(8)) != slot_key {
            return;
        }
        guest_call(c, m, cpu, 0x0001_74e0, 0x0025_f613, 0x14, &[player, 1, 0]);
        if cpu.eax != 0 {
            m.write32(entry.wrapping_add(0x10), 1);
        }
        return;
    }
}

// ---------------------------------------------------------------------------------------------
// 0x5CEF0 prop animation update (PARTIAL: head stages only, see debt in docs)
// ---------------------------------------------------------------------------------------------

/// How the ported head of 5CEF0 ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropHeadOutcome {
    /// Function returned early (idle prop with nothing driving it, or no active animation).
    Returned,
    /// Head stages done; the unported body (sampling, matrices, root motion) would continue.
    Continue(PropHeadState),
}

/// Locals the head computes for the unported body.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PropHeadState {
    pub resource_word_14: u32,
    pub driver_flags: u32,
    pub attached_object: u32,
    pub speed_scale: f32,
}

/// Fixed-point "CRT trunc/floor" helper at 003C8501: its x87 result is the only FP return in this
/// function, so the caller supplies it (the exact rounding mode is unverified, see debt).
pub type CrtRound<'a> = &'a mut dyn FnMut(f64) -> f64;

fn apply_pending_offset<M: GuestMemory>(
    m: &mut M,
    state: u32,
    flags_at: u32,
    obj: u32,
    scalar: bool,
) {
    // Pending root offsets accumulate into the object (+0xC4 for scalar bit 0x800, +0x64..+0x6C
    // for vector bit 0x400) and the bit is cleared.
    let flags = m.read32(state.wrapping_add(flags_at));
    if scalar && flags & 0x800 != 0 {
        let source = if flags_at == 0x128 { 0x11C } else { 0x174 };
        let sum = f32_at(m, state.wrapping_add(source)) + f32_at(m, obj.wrapping_add(0xC4));
        put_f32(m, obj.wrapping_add(0xC4), sum);
        let cleared = m.read32(state.wrapping_add(flags_at)) & !0x800;
        m.write32(state.wrapping_add(flags_at), cleared);
    }
    let flags = m.read32(state.wrapping_add(flags_at));
    if !scalar && flags & 0x400 != 0 {
        let source = if flags_at == 0x128 { 0x110 } else { 0x168 };
        for axis in 0..3u32 {
            let sum = f32_at(m, state.wrapping_add(source + 4 * axis))
                + f32_at(m, obj.wrapping_add(0x64 + 4 * axis));
            put_f32(m, obj.wrapping_add(0x64 + 4 * axis), sum);
        }
        let cleared = m.read32(state.wrapping_add(flags_at)) & !0x400;
        m.write32(state.wrapping_add(flags_at), cleared);
    }
}

/// Crossfade (+0xA8 mode: 1 fading in, other nonzero fading out). Progress +0xAC = +0xB4/+0xB0.
fn advance_blend<M: GuestMemory>(m: &mut M, state: u32) {
    let mode = m.read32(state.wrapping_add(0xA8));
    if mode == 0 {
        return;
    }
    let step = tick_step(m);
    let elapsed = f32_at(m, state.wrapping_add(0xB4));
    let elapsed = if mode == 1 {
        step + elapsed
    } else {
        elapsed - step
    };
    put_f32(m, state.wrapping_add(0xB4), elapsed);
    let progress = elapsed / f32_at(m, state.wrapping_add(0xB0));
    put_f32(m, state.wrapping_add(0xAC), progress);
    let one = f32_at(m, FRAME_UNIT);
    if !below_or_nan(progress, one) {
        m.write32(state.wrapping_add(0xA8), 2);
        put_f32(m, state.wrapping_add(0xAC), one);
    }
    let progress = f32_at(m, state.wrapping_add(0xAC));
    if !(0.0 < progress || progress.is_nan()) {
        m.write32(state.wrapping_add(0xA8), 0);
        for offset in [0xB0u32, 0xB4, 0xAC] {
            put_f32(m, state.wrapping_add(offset), 0.0);
        }
    }
}

// portreg: va=0x0005cef0
/// 0005CEF0 prop_animation_update(prop, extra). PARTIAL port (Tier 0 head): early outs, pending
/// offset accumulation, phase wrap of +0xC4 through the CRT helper, queued-animation promotion
/// (copy of 0x16 words from +0x12C to +0xD4), crossfade and one-shot timers, previous-pose
/// snapshots, and the speed scale selection. Returns the head locals for the unported body.
pub fn prop_animation_update_head<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    crt_round: CrtRound<'_>,
) -> PropHeadOutcome {
    let prop = stack_arg(m, cpu, 0);
    let state = m.read32(prop.wrapping_add(4));
    let mut head = PropHeadState::default();
    let hooked = m.read32(state.wrapping_add(0x88));
    if hooked != 0 {
        let slot = m.read32(hooked.wrapping_add(8));
        let index = i32::from(m.read16(0x007E_03C0 + slot.wrapping_mul(2)) as i16);
        let record = m.read32(
            m.read32(0x007E_3954)
                .wrapping_add((index as u32).wrapping_mul(4)),
        );
        head.resource_word_14 = m.read32(record.wrapping_add(0x14));
    }
    let active = m.read32(state.wrapping_add(0xD4));
    if active == 0 || active == 1 {
        return PropHeadOutcome::Returned;
    }
    let kind = m.read32(prop.wrapping_add(0x20));
    if kind == 8 {
        let owner = m.read32(prop.wrapping_add(0x7C));
        head.attached_object = owner;
        let (low, high) = if m.read8(0x007D_E455) == 0x0A {
            (0x10, 0x14)
        } else {
            (0x18, 0x1C)
        };
        let driven = m.read32(owner.wrapping_add(low)) | m.read32(owner.wrapping_add(high));
        if driven == 0 && m.read8(prop.wrapping_add(0x28)) & 1 == 0 {
            guest_call(c, m, cpu, 0x0005_6ff0, 0x0005_cfbd, 0x1F0, &[state]);
            return PropHeadOutcome::Returned;
        }
    } else if kind == 0x1000 {
        head.driver_flags = m.read32(prop.wrapping_add(0x7C));
    }
    apply_pending_offset(m, state, 0x128, prop, true);
    apply_pending_offset(m, state, 0x128, prop, false);
    let zero = 0.0f32;
    if f32_at(m, state.wrapping_add(0x198)) > zero {
        apply_pending_offset(m, state, 0x180, prop, true);
        apply_pending_offset(m, state, 0x180, prop, false);
    }
    // Phase wrap: phase - round(phase * K1) * K2, stored at +0xC4 and +0x70.
    let phase = f32_at(m, prop.wrapping_add(0xC4));
    let scaled = f64::from(phase) * f64::from(f32_at(m, 0x0047_5cb4));
    let rounded = crt_round(scaled) * f64::from(f32_at(m, 0x0047_5c88));
    let wrapped = (f64::from(phase) - rounded) as f32;
    put_f32(m, prop.wrapping_add(0xC4), wrapped);
    put_f32(m, prop.wrapping_add(0x70), wrapped);
    // Queued animation promotion.
    let total = f32_at(m, state.wrapping_add(0x198));
    if total > zero {
        let elapsed = tick_step(m) + f32_at(m, state.wrapping_add(0x1A0));
        put_f32(m, state.wrapping_add(0x1A0), elapsed);
        let ratio = elapsed / total;
        put_f32(m, state.wrapping_add(0x19C), ratio);
        if !below_or_nan(ratio, f32_at(m, FRAME_UNIT)) {
            for word in 0..0x16u32 {
                let value = m.read32(state.wrapping_add(0x12C + 4 * word));
                m.write32(state.wrapping_add(0xD4 + 4 * word), value);
            }
            for offset in [0x198u32, 0x19C, 0x1A0] {
                put_f32(m, state.wrapping_add(offset), zero);
            }
            m.write32(state.wrapping_add(0x12C), 0);
            let first_resource = i32::from(m.read16(0x007E_03C0) as i16);
            m.write32(state.wrapping_add(0x130), first_resource as u32);
            m.write32(state.wrapping_add(0x180), 0);
        }
    }
    advance_blend(m, state);
    advance_one_shot_timer(m, state);
    for (from, to) in [
        (0xE4u32, 0xF0u32),
        (0xE8, 0xF4),
        (0xEC, 0xF8),
        (0x13C, 0x148),
        (0x140, 0x14C),
        (0x144, 0x150),
    ] {
        let value = m.read32(state.wrapping_add(from));
        m.write32(state.wrapping_add(to), value);
    }
    head.speed_scale = match kind {
        8 => f32_at(m, m.read32(prop.wrapping_add(0x7C)).wrapping_add(0xB08)),
        0x1000 => f32_at(m, m.read32(prop.wrapping_add(0x7C)).wrapping_add(0x40)),
        _ => f32_at(m, FRAME_UNIT),
    };
    PropHeadOutcome::Continue(head)
}

/// State mode word +0xD0: while bits 0xC are set a timer (+0xCC) runs to +0xC8, then the mode
/// becomes 1 (or 2 when bit 4 was set) and the timer resets.
fn advance_one_shot_timer<M: GuestMemory>(m: &mut M, state: u32) {
    let mode = m.read32(state.wrapping_add(0xD0));
    if mode & 0xC == 0 {
        return;
    }
    let limit = f32_at(m, state.wrapping_add(0xC8));
    let timer = tick_step(m) + f32_at(m, state.wrapping_add(0xCC));
    put_f32(m, state.wrapping_add(0xCC), timer);
    if !below_or_nan(timer, limit) {
        put_f32(m, state.wrapping_add(0xCC), 0.0);
        m.write32(state.wrapping_add(0xD0), u32::from(mode & 4 != 0) + 1);
    }
}
