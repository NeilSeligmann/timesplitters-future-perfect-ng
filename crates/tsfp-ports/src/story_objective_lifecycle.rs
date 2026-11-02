// SPDX-License-Identifier: GPL-3.0-or-later
//! T2460/T2461: typed status lifecycle and completion effects for Story objective slots.
//!
//! This model owns the status byte and per-kind active counters. Guest callbacks, HUD
//! notifications and event propagation remain adapter effects at the lifted seams.

pub const FREE: u8 = 0;
pub const PENDING: u8 = 1;
pub const ACTIVE: u8 = 2;
pub const MARKED: u8 = 3;
pub const FAILED: u8 = 4;
pub const FINISHED: u8 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotKind {
    Objective,
    Other,
    Hidden,
    Unknown(u8),
}

impl SlotKind {
    pub const fn from_raw(value: u8) -> Self {
        match value {
            0 => Self::Objective,
            1 => Self::Other,
            2 => Self::Hidden,
            value => Self::Unknown(value),
        }
    }
    pub const fn raw(self) -> u8 {
        match self {
            Self::Objective => 0,
            Self::Other => 1,
            Self::Hidden => 2,
            Self::Unknown(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotStatus {
    Free,
    Pending,
    Active,
    Marked,
    Failed,
    Finished,
    Unknown(u8),
}

impl SlotStatus {
    pub const fn from_raw(value: u8) -> Self {
        match value {
            FREE => Self::Free,
            PENDING => Self::Pending,
            ACTIVE => Self::Active,
            MARKED => Self::Marked,
            FAILED => Self::Failed,
            FINISHED => Self::Finished,
            value => Self::Unknown(value),
        }
    }
    pub const fn raw(self) -> u8 {
        match self {
            Self::Free => FREE,
            Self::Pending => PENDING,
            Self::Active => ACTIVE,
            Self::Marked => MARKED,
            Self::Failed => FAILED,
            Self::Finished => FINISHED,
            Self::Unknown(value) => value,
        }
    }
}

/// Effects requested by the Story finisher after it publishes terminal status.
/// The adapter resolves hook/callback pointers lazily so notification side effects can
/// update those guest fields before the next effect is read, matching the lifted order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoryFinishPlan {
    pub next_status: SlotStatus,
    pub active_counter_delta: i32,
    pub call_notification_helper: bool,
    pub notify_players: bool,
    pub call_hook: bool,
    pub call_completion_callback: bool,
}

pub fn plan_story_finish(kind: SlotKind, status: SlotStatus) -> StoryFinishPlan {
    let mut next_status = status;
    let mut active_counter_delta = 0;
    if next_status == SlotStatus::Pending {
        next_status = SlotStatus::Active;
        active_counter_delta = 1;
    }
    if matches!(next_status, SlotStatus::Active | SlotStatus::Marked) {
        next_status = SlotStatus::Finished;
        StoryFinishPlan {
            next_status,
            active_counter_delta,
            call_notification_helper: true,
            notify_players: kind != SlotKind::Hidden,
            call_hook: true,
            call_completion_callback: true,
        }
    } else {
        StoryFinishPlan {
            next_status,
            active_counter_delta,
            call_notification_helper: false,
            notify_players: false,
            call_hook: false,
            call_completion_callback: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectiveSlot {
    pub kind: u8,
    pub status: u8,
    pub finish_stamp: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    None,
    Marked,
    Failed,
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectiveSlots {
    pub active: [i32; 3],
}

impl ObjectiveSlots {
    pub const fn new(active: [i32; 3]) -> Self {
        Self { active }
    }

    fn change(&mut self, slot: &mut ObjectiveSlot, target: u8) {
        if slot.status == target {
            return;
        }
        let kind = (slot.kind as usize).min(2);
        if slot.status == ACTIVE || slot.status == MARKED {
            self.active[kind] -= 1;
        }
        if target == ACTIVE {
            self.active[kind] += 1;
        }
        slot.status = target;
    }

    /// Lifted 0x227620: active or marked becomes pending.
    pub fn defer(&mut self, slot: &mut ObjectiveSlot) {
        if matches!(slot.status, ACTIVE | MARKED) {
            self.change(slot, PENDING);
        }
    }
    /// Lifted 0x227650: pending becomes active.
    pub fn activate(&mut self, slot: &mut ObjectiveSlot) {
        if slot.status == PENDING {
            self.change(slot, ACTIVE);
        }
    }
    /// Lifted 0x227690: pending activates; active or marked becomes marked.
    pub fn mark(&mut self, slot: &mut ObjectiveSlot) -> Effect {
        match slot.status {
            PENDING => self.change(slot, ACTIVE),
            ACTIVE | MARKED => self.change(slot, MARKED),
            _ => return Effect::None,
        }
        Effect::Marked
    }
    /// Lifted 0x2276E0: any started slot fails.
    pub fn fail(&mut self, slot: &mut ObjectiveSlot) -> Effect {
        if matches!(slot.status, PENDING | ACTIVE | MARKED) {
            self.change(slot, FAILED);
            Effect::Failed
        } else {
            Effect::None
        }
    }
    /// Lifted 0x227710: every non-free, non-active state becomes active.
    pub fn restore(&mut self, slot: &mut ObjectiveSlot) {
        if slot.status != FREE && slot.status != ACTIVE {
            self.change(slot, ACTIVE);
        }
    }
    /// Lifted 0x2275F0: active or marked becomes finished.
    pub fn finish(&mut self, slot: &mut ObjectiveSlot, now: u32) -> Effect {
        if matches!(slot.status, ACTIVE | MARKED) {
            self.change(slot, FINISHED);
            slot.finish_stamp = now.wrapping_sub(180);
            Effect::Finished
        } else {
            Effect::None
        }
    }
    /// Lifted 0x2274C0: starts pending first, then finishes only active/marked slots.
    pub fn finish_with_callbacks(&mut self, slot: &mut ObjectiveSlot, _now: u32) -> Effect {
        let plan = plan_story_finish(
            SlotKind::from_raw(slot.kind),
            SlotStatus::from_raw(slot.status),
        );
        let kind = usize::from(slot.kind.min(2));
        self.active[kind] = self.active[kind].wrapping_add(plan.active_counter_delta);
        slot.status = plan.next_status.raw();
        if plan.call_completion_callback {
            Effect::Finished
        } else {
            Effect::None
        }
    }
}

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}
fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}
#[cfg(not(test))]
fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_OBJECTIVE_LIFECYCLE".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Default-off adapter for Story slot status helpers and the typed `0x2274C0` finisher.
pub fn run<M: crate::memory::GuestMemory, C: crate::abi::GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut crate::abi::ExtRegisters,
) -> bool {
    // Match the VA first so unrelated roots never touch the guest window.
    if !matches!(
        va,
        0x227620 | 0x227650 | 0x227690 | 0x2276e0 | 0x227710 | 0x2275f0 | 0x2274c0
    ) || !enabled()
        || memory.read8(0x7de455) != 10
    {
        return false;
    }
    if va == 0x2274c0 {
        return finish_story_slot(memory, guest, regs);
    }
    let slot = memory.read32(regs.esp.wrapping_add(4));
    if slot == 0 {
        return true;
    }
    let status_at = slot.wrapping_add(0xb);
    let status = memory.read8(status_at);
    let kind = usize::from(memory.read8(slot.wrapping_add(8)).min(2));
    let counter = 0x75c134_u32.wrapping_add((kind as u32) * 4);
    let old_active = matches!(status, ACTIVE | MARKED);
    let next = match va {
        0x227620 if old_active => PENDING,
        0x227650 if status == PENDING => ACTIVE,
        0x227690 if status == PENDING => ACTIVE,
        0x227690 if old_active => MARKED,
        0x2276e0 if matches!(status, PENDING | ACTIVE | MARKED) => FAILED,
        0x227710 if status != FREE && status != ACTIVE => ACTIVE,
        0x2275f0 if old_active => FINISHED,
        _ => status,
    };
    if next != status {
        let new_active = next == ACTIVE;
        if old_active != new_active {
            let value = memory.read32(counter) as i32 + if new_active { 1 } else { -1 };
            memory.write32(counter, value as u32);
        }
        memory.write8(status_at, next);
    }
    true
}

fn finish_story_slot<M: crate::memory::GuestMemory, C: crate::abi::GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut crate::abi::ExtRegisters,
) -> bool {
    use crate::abi::call_cdecl;

    let slot_id = memory.read32(regs.esp.wrapping_add(4));
    let notify_arg = memory.read32(regs.esp.wrapping_add(8));
    let table = memory.read32(0x78cecc);
    let slot = table.wrapping_add(slot_id.wrapping_mul(0x4c));
    let raw_kind = memory.read8(slot.wrapping_add(8));
    let status_at = slot.wrapping_add(0xb);
    let status = SlotStatus::from_raw(memory.read8(status_at));
    let kind = SlotKind::from_raw(raw_kind);
    let plan = plan_story_finish(kind, status);

    // The lifted body publishes pending -> active before incrementing the class counter, then
    // publishes terminal status. Finishing active/marked leaves that counter unchanged here.
    if status == SlotStatus::Pending {
        memory.write8(status_at, ACTIVE);
    }
    if plan.active_counter_delta != 0 {
        let counter = 0x75c134_u32.wrapping_add(u32::from(raw_kind).wrapping_mul(4));
        let value = memory
            .read32(counter)
            .wrapping_add(plan.active_counter_delta as u32);
        memory.write32(counter, value);
    }
    if plan.next_status != status {
        memory.write8(status_at, plan.next_status.raw());
    }

    let saved_ebx = regs.ebx;
    let saved_esi = regs.esi;
    let saved_edi = regs.edi;
    regs.ecx = 2;
    if plan.call_hook || plan.call_completion_callback {
        regs.eax = notify_arg;
    } else {
        regs.eax = (regs.eax & !0xff) | u32::from(status.raw());
    }
    if plan.call_notification_helper {
        call_cdecl(guest, memory, regs, 0x227430, &[slot, notify_arg]);
    }
    if plan.call_hook {
        let hook = memory.read32(slot.wrapping_add(0x38));
        regs.eax = hook;
        if hook != 0 {
            guest.call(hook, regs);
        }
    }
    if plan.call_completion_callback {
        let callback = memory.read32(slot.wrapping_add(0x3c));
        regs.eax = callback;
        if callback != 0 {
            call_cdecl(guest, memory, regs, callback, &[slot, 4]);
        }
    }
    regs.ebx = saved_ebx;
    regs.esi = saved_esi;
    regs.edi = saved_edi;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::GuestMemory as _;

    // Branch transcription of the measured status helpers; this is weak Tier 1 evidence.
    fn lifted(state: u8, kind: u8, op: u8, now: u32) -> (u8, [i32; 3], u32) {
        let mut s = state;
        let mut c = [7, 7, 7];
        let k = usize::from(kind.min(2));
        let mut stamp = 0;
        let old = s;
        let mut completion_delta = 0;
        match op {
            0 if s == ACTIVE || s == MARKED => s = PENDING,
            1 if s == PENDING => s = ACTIVE,
            2 if s == PENDING => s = ACTIVE,
            2 if s == ACTIVE || s == MARKED => s = MARKED,
            3 if s == PENDING || s == ACTIVE || s == MARKED => s = FAILED,
            4 if s != FREE && s != ACTIVE => s = ACTIVE,
            5 if s == ACTIVE || s == MARKED => {
                s = FINISHED;
                stamp = now.wrapping_sub(180);
            }
            6 if s == PENDING => {
                s = ACTIVE;
                completion_delta = 1;
                s = FINISHED;
            }
            6 if s == ACTIVE || s == MARKED => {
                s = FINISHED;
            }
            _ => {}
        }
        // Counters change only when the helper actually changes the status byte.
        // In particular, an activation request against MARKED is a no-op.
        if op == 6 {
            c[k] += completion_delta;
        } else if s != old {
            if old == ACTIVE || old == MARKED {
                c[k] -= 1;
            }
            if s == ACTIVE {
                c[k] += 1;
            }
        }
        (s, c, stamp)
    }

    #[test]
    fn lifecycle_matches_lifted_transcription_and_mutations() {
        let mut seed = 0x2460_5eed_u32;
        for _ in 0..3_000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let state = ((seed >> 4) % 6) as u8;
            let kind = ((seed >> 12) % 3) as u8;
            let op = ((seed >> 19) % 7) as u8;
            let now = 0x1234_5678;
            let (expected, mut counters, stamp) = lifted(state, kind, op, now);
            let mut slot = ObjectiveSlot {
                kind,
                status: state,
                finish_stamp: 0,
            };
            let mut manager = ObjectiveSlots::new([7; 3]);
            match op {
                0 => manager.defer(&mut slot),
                1 => manager.activate(&mut slot),
                2 => {
                    manager.mark(&mut slot);
                }
                3 => {
                    manager.fail(&mut slot);
                }
                4 => manager.restore(&mut slot),
                5 => {
                    manager.finish(&mut slot, now);
                }
                _ => {
                    manager.finish_with_callbacks(&mut slot, now);
                }
            }
            assert_eq!(
                (slot.status, slot.finish_stamp),
                (expected, stamp),
                "state={state} kind={kind} op={op}"
            );
            counters[kind as usize] = manager.active[kind as usize];
            assert_eq!(
                counters,
                lifted(state, kind, op, now).1,
                "state={state} kind={kind} op={op}"
            );
        }
    }

    #[test]
    fn selector_is_default_off() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }

    #[derive(Clone, Default)]
    struct Ram(std::collections::BTreeMap<u32, u8>);
    impl crate::memory::GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[derive(Default)]
    struct Calls(Vec<u32>);
    impl crate::abi::GuestCall for Calls {
        fn call(&mut self, va: u32, _regs: &mut crate::abi::ExtRegisters) {
            self.0.push(va);
        }
    }

    fn put32(memory: &mut Ram, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            memory.write8(address.wrapping_add(offset as u32), byte);
        }
    }

    fn get32(memory: &Ram, address: u32) -> u32 {
        u32::from_le_bytes([
            memory.read8(address),
            memory.read8(address + 1),
            memory.read8(address + 2),
            memory.read8(address + 3),
        ])
    }

    fn finish_fixture(
        kind: u8,
        status: u8,
        hook: u32,
        callback: u32,
    ) -> (Ram, crate::abi::ExtRegisters) {
        let mut memory = Ram::default();
        const TABLE: u32 = 0x30000;
        const ID: u32 = 3;
        const RECORD: u32 = TABLE + ID * 0x4c;
        put32(&mut memory, 0x78cecc, TABLE);
        memory.write8(0x7de455, 10);
        memory.write8(RECORD + 8, kind);
        memory.write8(RECORD + 0xb, status);
        put32(&mut memory, RECORD + 0x38, hook);
        put32(&mut memory, RECORD + 0x3c, callback);
        put32(&mut memory, 0x75c134 + u32::from(kind) * 4, 11);
        let mut regs = crate::abi::ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x7000,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [crate::abi::Xmm::default(); 5],
            xmm_hi: [crate::abi::Xmm::default(); 3],
        };
        put32(&mut memory, regs.esp + 4, ID);
        put32(&mut memory, regs.esp + 8, 9);
        (memory, regs)
    }

    #[test]
    fn finish_lane_matches_lifted_body_for_seeded_story_slots() {
        let mut seed = 0x2461_5eed_u32;
        for _ in 0..3_000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let kind = ((seed >> 5) % 4) as u8;
            let status = ((seed >> 11) % 8) as u8;
            let hook = if seed & 0x10000 != 0 { 0x100100 } else { 0 };
            let callback = if seed & 0x20000 != 0 { 0x100200 } else { 0 };
            let (mut rust_memory, mut rust_regs) = finish_fixture(kind, status, hook, callback);
            let (mut lifted_memory, mut lifted_regs) = finish_fixture(kind, status, hook, callback);
            let mut rust_calls = Calls::default();
            let mut lifted_calls = Calls::default();

            assert!(run(
                0x2274c0,
                &mut rust_memory,
                &mut rust_calls,
                &mut rust_regs
            ));
            crate::c2r::data_missions::game_slot_4c_finish_post_player_messages_and_run_callbacks(
                &mut lifted_memory,
                &mut lifted_calls,
                &mut lifted_regs,
            );

            let record = 0x30000 + 3 * 0x4c;
            assert_eq!(
                rust_memory.read8(record + 0xb),
                lifted_memory.read8(record + 0xb)
            );
            assert_eq!(
                get32(&rust_memory, 0x75c134 + u32::from(kind) * 4),
                get32(&lifted_memory, 0x75c134 + u32::from(kind) * 4)
            );
            assert_eq!(
                rust_memory.read32(record + 0x34),
                lifted_memory.read32(record + 0x34)
            );
            assert_eq!(rust_calls.0, lifted_calls.0);
            assert_eq!(rust_regs.esp, lifted_regs.esp);
            assert_eq!(rust_regs.eax, lifted_regs.eax);
            assert_eq!(rust_regs.esi, lifted_regs.esi);
            assert_eq!(rust_regs.edi, lifted_regs.edi);
            assert_eq!(rust_regs.ebx, lifted_regs.ebx);
        }
    }

    #[test]
    fn finish_mutation_checks_cover_promotion_terminal_gate_and_hidden_notice() {
        let pending = plan_story_finish(SlotKind::Objective, SlotStatus::Pending);
        assert_eq!(pending.next_status, SlotStatus::Finished);
        assert_eq!(pending.active_counter_delta, 1);
        assert!(pending.call_notification_helper && pending.notify_players);
        assert!(pending.call_hook && pending.call_completion_callback);

        let hidden = plan_story_finish(SlotKind::Hidden, SlotStatus::Active);
        assert_eq!(hidden.next_status, SlotStatus::Finished);
        assert_eq!(hidden.active_counter_delta, 0);
        assert!(hidden.call_notification_helper);
        assert!(!hidden.notify_players);

        let failed = plan_story_finish(SlotKind::Objective, SlotStatus::Failed);
        assert_eq!(failed.next_status, SlotStatus::Failed);
        assert!(!failed.notify_players && !failed.call_hook && !failed.call_completion_callback);
    }
}
