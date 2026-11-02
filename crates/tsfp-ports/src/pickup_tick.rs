// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-gameplay/w4b_gameplay
//! T3398 (frontier request): opt-in host selection of the pickup per-frame tick `0xC1850`.
//!
//! `0xC1850` is the tick callback registered by the pickup creator `0xC1950`
//! (`game_pickup_object_per_frame_tick_timers_motion_and_touch_scan`, cdecl(pickup)). The
//! readable body is the T1895/T1920 `tsfp-gameplay` port (`w4b_gameplay::pickup_frame_tick`,
//! Tier 1 against the private lifted original); this module is its `GuestCall` form for the
//! hybrid lane. Nested originals stay retained: `0xC0C50` refresh, `0xBDD50` guard, `0xBF080`
//! motion and `0xC15B0` touch scan (the last is also a c2r weapon root). Default off; the word
//! `pickup-tick` (or the VA) selects it through `weapons_lane::parse`.
//!
//! Register residue at exit is not modelled (EAX, ECX and EDX are caller-saved; the callee
//! saved registers are untouched). Tier 0 only until a differential and Tier 2 gate run.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::rt::RegsExt,
    memory::GuestMemory,
};

/// The pickup tick root.
pub const PICKUP_TICK: u32 = 0xc1850;
/// Pickup roots exposed to `weapons_lane` (the word `pickup-tick`).
pub const ROOTS: [u32; 1] = [0xc1850];

const PICKUP_STATE_BLOCK: u32 = 0x7c;
const PICKUP_FLAGS: u32 = 0x28;
const FLAG_ACTIVE: u32 = 1;
const FLAG_TOUCHABLE: u32 = 2;

const STATE_MARKS: u32 = 0x10;
const STATE_MODEL_RECORD: u32 = 0x18;
const STATE_COUNTER_GROUPS: u32 = 0x50;
const STATE_PHASE: u32 = 0xc0;
const STATE_TIMER: u32 = 0xd0;
const COUNTER_WORDS: u32 = 32;
const MARK_TIMER_FROZEN: u8 = 2;
const MODEL_MARK_TICKS: u8 = 8;
const MIN_PHASE: i32 = 2;

const REFRESH: u32 = 0x000c_0c50;
const GUARD: u32 = 0x000b_dd50;
const MOTION: u32 = 0x000b_f080;
const TOUCH_SCAN: u32 = 0x000c_15b0;

const ZERO_FLOAT: u32 = 0x0047_5cac;
const FRAME_UNIT: u32 = 0x0047_5c78;
const TICK_SCALE: u32 = 0x004e_794c;
const TICK_COUNT: u32 = 0x004e_8cec;

fn f32_at<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// Decrements every positive i16 counter of the eight 4-counter groups at state+0x50.
fn decay_counters<M: GuestMemory>(m: &mut M, state: u32) {
    for index in 0..COUNTER_WORDS {
        let address = state.wrapping_add(STATE_COUNTER_GROUPS + 2 * index);
        let value = m.read16(address) as i16;
        if value > 0 {
            let [low, high] = ((value - 1) as u16).to_le_bytes();
            m.write8(address, low);
            m.write8(address.wrapping_add(1), high);
        }
    }
}

/// One tick of the frame timer: `unit / tick_count * scale` is subtracted while it is positive.
fn step_timer<M: GuestMemory>(m: &mut M, state: u32) {
    let timer = f32_at(m, state.wrapping_add(STATE_TIMER));
    if timer > f32_at(m, ZERO_FLOAT) {
        let step =
            f32_at(m, FRAME_UNIT) / (m.read32(TICK_COUNT) as i32 as f32) * f32_at(m, TICK_SCALE);
        m.write32(state.wrapping_add(STATE_TIMER), (timer - step).to_bits());
    }
}

/// Runs the tick for the pickup at `[esp + 4]`; ESP is back at its entry value afterwards.
pub fn tick<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, r: &mut ExtRegisters) {
    let pickup = m.read32(r.esp.wrapping_add(4));
    let state = m.read32(pickup.wrapping_add(PICKUP_STATE_BLOCK));
    if (m.read32(state.wrapping_add(STATE_PHASE)) as i32) < MIN_PHASE {
        return;
    }
    r.eax = pickup;
    r.call(guest, REFRESH);
    let flags = m.read32(pickup.wrapping_add(PICKUP_FLAGS));
    if flags & FLAG_TOUCHABLE == 0 {
        return;
    }
    if flags & FLAG_ACTIVE == 0 {
        let record = m.read32(state.wrapping_add(STATE_MODEL_RECORD));
        if record == 0 || m.read8(record) & MODEL_MARK_TICKS == 0 {
            return;
        }
    }
    r.call(guest, GUARD);
    if r.eax != 0 {
        return;
    }
    decay_counters(m, state);
    r.eax = pickup;
    r.call(guest, MOTION);
    if m.read8(state.wrapping_add(STATE_MARKS)) & MARK_TIMER_FROZEN == 0 {
        step_timer(m, state);
    }
    r.push(m, pickup);
    r.call(guest, TOUCH_SCAN);
    r.esp = r.esp.wrapping_add(4);
}

/// Lane dispatch for `weapons_lane`: true when `va` is the pickup tick.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if va != PICKUP_TICK {
        return false;
    }
    tick(m, guest, r);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{Fpu, Xmm};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Mem(BTreeMap<u32, u8>);
    impl GuestMemory for Mem {
        fn read8(&self, a: u32) -> u8 {
            *self.0.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }

    #[derive(Default)]
    struct Guest {
        calls: Vec<(u32, u32)>,
        guard_result: u32,
    }
    impl GuestCall for Guest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.calls.push((va, regs.eax));
            if va == GUARD {
                regs.eax = self.guard_result;
            }
        }
    }

    fn regs(esp: u32) -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm { lanes: [0; 4] }; 5],
            xmm_hi: [Xmm { lanes: [0; 4] }; 3],
        }
    }

    const PICKUP: u32 = 0x0100_0000;
    const STATE: u32 = 0x0100_1000;
    const ESP: u32 = 0x0200_0000;

    /// FABRICATED fixture: phase 2, touchable and active, one positive counter, timer 3.0.
    fn fixture() -> Mem {
        let mut m = Mem::default();
        m.write32(ESP + 4, PICKUP);
        m.write32(PICKUP + PICKUP_STATE_BLOCK, STATE);
        m.write32(PICKUP + PICKUP_FLAGS, FLAG_ACTIVE | FLAG_TOUCHABLE);
        m.write32(STATE + STATE_PHASE, 2);
        m.write8(STATE + STATE_COUNTER_GROUPS, 5);
        m.write32(STATE + STATE_TIMER, 3.0f32.to_bits());
        m.write32(ZERO_FLOAT, 0.0f32.to_bits());
        m.write32(FRAME_UNIT, 1.0f32.to_bits());
        m.write32(TICK_SCALE, 1.0f32.to_bits());
        m.write32(TICK_COUNT, 2);
        m
    }

    #[test]
    fn full_tick_decays_counters_steps_timer_and_scans_touch() {
        let (mut m, mut g, mut r) = (fixture(), Guest::default(), regs(ESP));
        tick(&mut m, &mut g, &mut r);
        assert_eq!(
            g.calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            [REFRESH, GUARD, MOTION, TOUCH_SCAN]
        );
        assert!(g.calls.iter().take(3).all(|c| c.1 == PICKUP));
        assert_eq!(
            u16::from_le_bytes([
                m.read8(STATE + STATE_COUNTER_GROUPS),
                m.read8(STATE + STATE_COUNTER_GROUPS + 1)
            ]),
            4
        );
        assert_eq!(f32_at(&m, STATE + STATE_TIMER), 2.5);
        assert_eq!(r.esp, ESP);
    }

    #[test]
    fn low_phase_does_nothing() {
        let (mut m, mut g, mut r) = (fixture(), Guest::default(), regs(ESP));
        m.write32(STATE + STATE_PHASE, 1);
        tick(&mut m, &mut g, &mut r);
        assert!(g.calls.is_empty());
    }

    #[test]
    fn guard_result_stops_before_decay_and_frozen_timer_is_kept() {
        let (mut m, mut g, mut r) = (
            fixture(),
            Guest {
                guard_result: 1,
                ..Guest::default()
            },
            regs(ESP),
        );
        tick(&mut m, &mut g, &mut r);
        assert_eq!(
            g.calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            [REFRESH, GUARD]
        );
        assert_eq!(
            u16::from_le_bytes([
                m.read8(STATE + STATE_COUNTER_GROUPS),
                m.read8(STATE + STATE_COUNTER_GROUPS + 1)
            ]),
            5
        );

        let (mut m, mut g, mut r) = (fixture(), Guest::default(), regs(ESP));
        m.write8(STATE + STATE_MARKS, MARK_TIMER_FROZEN);
        tick(&mut m, &mut g, &mut r);
        assert_eq!(f32_at(&m, STATE + STATE_TIMER), 3.0);
    }

    #[test]
    fn inactive_pickup_needs_the_model_mark() {
        let (mut m, mut g, mut r) = (fixture(), Guest::default(), regs(ESP));
        m.write32(PICKUP + PICKUP_FLAGS, FLAG_TOUCHABLE);
        tick(&mut m, &mut g, &mut r);
        assert_eq!(g.calls.len(), 1);
        m.write32(STATE + STATE_MODEL_RECORD, 0x0100_2000);
        m.write8(0x0100_2000, MODEL_MARK_TICKS);
        let (mut g, mut r) = (Guest::default(), regs(ESP));
        tick(&mut m, &mut g, &mut r);
        assert_eq!(g.calls.len(), 4);
    }
}
