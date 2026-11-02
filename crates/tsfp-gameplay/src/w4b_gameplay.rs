// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4b gameplay ports (Tier 0): translated from the lifted originals, nested guest
//! calls over the T1815 closure model. Register residue at exit is not modelled.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

const ZERO_FLOAT: u32 = 0x0047_5cac;
const FRAME_UNIT: u32 = 0x0047_5c78;
const TICK_SCALE: u32 = 0x004e_794c;
const TICK_COUNT: u32 = 0x004e_8cec;

fn f32_at<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// Pickup state block fields (reached through the pickup object at +0x7C).
mod state {
    pub const MARKS: u32 = 0x10;
    pub const MODEL_RECORD: u32 = 0x18;
    pub const COUNTER_GROUPS: u32 = 0x50;
    pub const PHASE: u32 = 0xC0;
    pub const TIMER: u32 = 0xD0;
}

/// Decrements every positive i16 counter of the eight 4-counter groups at state+0x50.
fn decay_counter_groups<M: GuestMemory>(m: &mut M, state_block: u32) {
    for index in 0..32u32 {
        let address = state_block.wrapping_add(state::COUNTER_GROUPS + 2 * index);
        let value = m.read16(address) as i16;
        if value > 0 {
            let [low, high] = ((value - 1) as u16).to_le_bytes();
            m.write8(address, low);
            m.write8(address.wrapping_add(1), high);
        }
    }
}

// portreg: va=0x000c1850
/// 000C1850 pickup per-frame tick, cdecl(pickup). Gate: state phase >= 2, flag 2 set after the
/// C0C50 refresh (EAX = pickup), and either flag 1 or the model record's mark 8. Then BDD50()
/// must return 0, the counters decay, BF080 (EAX = pickup) runs, and when state mark 2 is clear
/// the timer at +0xD0 loses one tick while positive. C15B0(pickup) always closes the tick.
pub fn pickup_frame_tick<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let pickup = stack_arg(m, cpu, 0);
    let block = m.read32(pickup.wrapping_add(0x7C));
    if (m.read32(block.wrapping_add(state::PHASE)) as i32) < 2 {
        return;
    }
    cpu.eax = pickup;
    guest_call(c, m, cpu, 0x000c_0c50, 0x000c_186d, 8, &[]);
    let flags = m.read32(pickup.wrapping_add(0x28));
    if flags & 2 == 0 {
        return;
    }
    if flags & 1 == 0 {
        let record = m.read32(block.wrapping_add(state::MODEL_RECORD));
        if record == 0 || m.read8(record) & 8 == 0 {
            return;
        }
    }
    guest_call(c, m, cpu, 0x000b_dd50, 0x000c_1895, 8, &[]);
    if cpu.eax != 0 {
        return;
    }
    decay_counter_groups(m, block);
    cpu.eax = pickup;
    guest_call(c, m, cpu, 0x000b_f080, 0x000c_18f0, 8, &[]);
    if m.read8(block.wrapping_add(state::MARKS)) & 2 != 0 {
        return;
    }
    let timer = f32_at(m, block.wrapping_add(state::TIMER));
    if timer > f32_at(m, ZERO_FLOAT) {
        let step =
            f32_at(m, FRAME_UNIT) / (m.read32(TICK_COUNT) as i32 as f32) * f32_at(m, TICK_SCALE);
        m.write32(block.wrapping_add(state::TIMER), (timer - step).to_bits());
    }
    guest_call(c, m, cpu, 0x000c_15b0, 0x000c_193d, 8, &[pickup]);
}

/// 001C5DB0 weapon record 37 default hands callback, cdecl(event, owner, context).
/// Event 3 clears bit 0 of the owner's hand object flags (+0x28 of [owner+0x54]) and returns 1.
/// Event 4 returns 1. Event 0x11 with owner state [owner+0xE8] in 0x15..=0x16 calls
/// 1B8690(0xCBB, context, 1) and returns 0. Everything else returns 0 (EAX).
// portreg: va=0x001c5db0
pub fn weapon_record_37_default_hands_callback<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let event = stack_arg(m, cpu, 0);
    let owner = stack_arg(m, cpu, 1);
    let context = stack_arg(m, cpu, 2);
    match event {
        3 => {
            let hands = m.read32(owner.wrapping_add(0x54));
            if hands != 0 {
                let flags = m.read32(hands.wrapping_add(0x28));
                m.write32(hands.wrapping_add(0x28), flags & !1);
            }
            cpu.eax = 1;
        }
        4 => cpu.eax = 1,
        0x11 => {
            let state = m.read32(owner.wrapping_add(0xE8)) as i32;
            if (0x15..=0x16).contains(&state) {
                guest_call(c, m, cpu, 0x001b_8690, 0x001c_5de6, 0, &[0xCBB, context, 1]);
            }
            cpu.eax = 0;
        }
        _ => cpu.eax = 0,
    }
}
