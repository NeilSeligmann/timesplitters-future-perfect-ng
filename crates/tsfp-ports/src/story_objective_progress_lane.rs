// SPDX-License-Identifier: GPL-3.0-or-later
//! T2464: typed Story objective member-progress update at `0x2282B0`.
//!
//! This lane deliberately accepts only the fully measured, side-effect-free
//! counter-update case. Message/sound flags, alternate slot kinds/statuses,
//! arcade mode, and the distance-trigger tail remain on the lifted path.
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
    state_wire::story_flow,
    story_objective_lifecycle::{SlotKind, SlotStatus},
};
use alloc::vec::Vec;

const ROOT: u32 = 0x2282b0;
const MODE: u32 = 0x7de455;
const OBJECTIVES: u32 = 0x78cecc;
const MAX_STORY_SLOT: u32 = 0x2d;
const STRIDE: u32 = 0x4c;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoryObjectiveProgressState {
    pub kind: SlotKind,
    pub status: SlotStatus,
    pub progress_flags: u8,
    pub previous_members: u32,
    pub eligible_members: u32,
    pub required_members: u32,
    pub has_distance_trigger: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoryObjectiveProgressPlan {
    pub next_members: u32,
    pub write_progress: bool,
    pub finish: bool,
}

/// Text and audio requests produced by the Story progress feedback branch.
/// These are semantic requests; HUD and audio adapters own localization and
/// sound playback, respectively.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoryProgressEffect {
    FinishObjectiveSlot {
        slot_id: u32,
    },
    PostObjectiveMessage {
        player: u32,
        text_id: u16,
        remaining: u32,
    },
    PlayProgressSound {
        sound_id: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoryProgressFeedbackState {
    pub counter_changed: bool,
    pub slot_id: u32,
    pub remaining: u32,
    pub progress_messages_enabled: bool,
    pub one_remaining_text_id: u16,
    pub many_remaining_text_id: u16,
    pub local_players: Vec<u32>,
}

/// Build ordered typed requests for the changed-counter feedback case.
/// The lifted routine emits one HUD message per local player, then submits
/// sound 0xD6C. Zero remaining is handled by the separate finish contract.
pub fn plan_story_progress_feedback(
    state: &StoryProgressFeedbackState,
) -> Vec<StoryProgressEffect> {
    let mut effects = Vec::new();
    if !state.counter_changed {
        return effects;
    }
    if state.remaining == 0 {
        effects.push(StoryProgressEffect::FinishObjectiveSlot {
            slot_id: state.slot_id,
        });
    }
    if state.progress_messages_enabled && state.remaining != 0 {
        let text_id = if state.remaining == 1 {
            state.one_remaining_text_id
        } else {
            state.many_remaining_text_id
        };
        if text_id != 0 {
            effects.extend(state.local_players.iter().copied().map(|player| {
                StoryProgressEffect::PostObjectiveMessage {
                    player,
                    text_id,
                    remaining: state.remaining,
                }
            }));
        }
    }
    if state.progress_messages_enabled {
        effects.push(StoryProgressEffect::PlayProgressSound { sound_id: 0xd6c });
    }
    effects
}

/// Typed handler boundary implemented by the native HUD/audio systems.
pub trait StoryProgressEffectHandlers {
    fn finish_objective_slot(&mut self, slot_id: u32);
    fn post_objective_message(&mut self, player: u32, text_id: u16, remaining: u32);
    fn play_sound(&mut self, sound_id: u32);
}

pub fn dispatch_story_progress_effects(
    effects: &[StoryProgressEffect],
    handlers: &mut impl StoryProgressEffectHandlers,
) {
    for effect in effects {
        match *effect {
            StoryProgressEffect::FinishObjectiveSlot { slot_id } => {
                handlers.finish_objective_slot(slot_id)
            }
            StoryProgressEffect::PostObjectiveMessage {
                player,
                text_id,
                remaining,
            } => handlers.post_objective_message(player, text_id, remaining),
            StoryProgressEffect::PlayProgressSound { sound_id } => handlers.play_sound(sound_id),
        }
    }
}

/// Native host entry point for typed Story progress effects. The selector is
/// default off in production; the host supplies its Rust finisher, HUD, and
/// audio handlers when explicitly enabling the A/B lane.
pub fn run_story_progress_feedback(
    state: &StoryProgressFeedbackState,
    handlers: &mut impl StoryProgressEffectHandlers,
) -> bool {
    if !enabled() {
        return false;
    }
    let effects = plan_story_progress_feedback(state);
    if effects.is_empty() {
        return false;
    }
    dispatch_story_progress_effects(&effects, handlers);
    true
}

/// Returns a plan only for the bounded Story objective counter path.
/// The progress-message/sound branch and the later distance-trigger scan are
/// intentionally left to the retained body until their full effect contracts
/// are ported.
pub fn plan_story_objective_progress(
    state: StoryObjectiveProgressState,
) -> Option<StoryObjectiveProgressPlan> {
    if state.kind != SlotKind::Objective
        || !matches!(
            state.status,
            SlotStatus::Free | SlotStatus::Pending | SlotStatus::Active | SlotStatus::Marked
        )
        || state.progress_flags & 1 != 0
        || state.has_distance_trigger
    {
        return None;
    }
    let changed = state.previous_members != state.eligible_members;
    Some(StoryObjectiveProgressPlan {
        next_members: state.eligible_members,
        write_progress: changed,
        finish: changed && state.required_members.wrapping_sub(state.eligible_members) == 0,
    })
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
        let value = getenv(c"TSFP_STORY_OBJECTIVE_PROGRESS".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Handles only the typed counter sub-path; `false` guarantees no writes/calls.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != ROOT || !enabled() || memory.read8(MODE) != story_flow::MODE_STORY {
        return false;
    }
    let slot_id = memory.read32(regs.esp.wrapping_add(4));
    run_for_slot(memory, guest, regs, slot_id)
}

pub fn run_for_slot<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    slot_id: u32,
) -> bool {
    if !enabled() || memory.read8(MODE) != story_flow::MODE_STORY {
        return false;
    }
    if slot_id >= MAX_STORY_SLOT {
        return false;
    }
    let record = memory
        .read32(OBJECTIVES)
        .wrapping_add(slot_id.wrapping_mul(STRIDE));
    let kind = memory.read8(record.wrapping_add(0x48));
    let status = memory.read8(record.wrapping_add(0x0b));
    let progress_flags = memory.read8(record.wrapping_add(9));
    let required_members = memory.read32(record.wrapping_add(0x30));
    let previous_members = memory.read32(record.wrapping_add(0x2c));
    let has_distance_trigger = memory.read32(record.wrapping_add(0x10)) != u32::MAX;
    if has_distance_trigger {
        return false;
    }
    let member_slots = memory.read32(record.wrapping_add(0x0c)).wrapping_add(0x54);
    let mut eligible_members = 0u32;
    for index in 0..10u32 {
        let member = memory.read32(member_slots.wrapping_add(index.wrapping_mul(4)));
        if member == 0 {
            break;
        }
        let progress = f32::from_bits(memory.read32(member.wrapping_add(0x1cc)));
        if progress <= 0.0 {
            eligible_members = eligible_members.wrapping_add(1);
        }
    }
    let state = StoryObjectiveProgressState {
        kind: SlotKind::from_raw(kind),
        status: SlotStatus::from_raw(status),
        progress_flags,
        previous_members,
        eligible_members,
        required_members,
        has_distance_trigger,
    };
    let Some(plan) = plan_story_objective_progress(state) else {
        return false;
    };
    if plan.write_progress {
        memory.write32(record.wrapping_add(0x2c), plan.next_members);
    }
    if plan.finish {
        call_cdecl(guest, memory, regs, 0x2274c0, &[slot_id, 0]);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Clone, Default)]
    struct Ram(BTreeMap<u32, u8>);
    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    #[derive(Default)]
    struct Calls(Vec<u32>);
    impl GuestCall for Calls {
        fn call(&mut self, va: u32, _regs: &mut ExtRegisters) {
            self.0.push(va);
        }
    }
    fn put32(memory: &mut Ram, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            memory.write8(address.wrapping_add(i as u32), byte);
        }
    }
    fn fixture(previous: u32) -> (Ram, ExtRegisters) {
        let mut memory = Ram::default();
        let record = 0x30000;
        let member_table = 0x40000;
        let member = 0x50000;
        put32(&mut memory, OBJECTIVES, record);
        memory.write8(MODE, story_flow::MODE_STORY);
        memory.write8(record + 0x48, 0);
        memory.write8(record + 0x0b, 2);
        memory.write8(record + 9, 0);
        put32(&mut memory, record + 0x0c, member_table);
        put32(&mut memory, record + 0x2c, previous);
        put32(&mut memory, record + 0x10, u32::MAX);
        put32(&mut memory, record + 0x30, 1);
        put32(&mut memory, member_table + 0x54, member);
        put32(&mut memory, member + 0x1cc, 0.0f32.to_bits());
        let mut regs = ExtRegisters {
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
        put32(&mut memory, regs.esp + 4, 0);
        (memory, regs)
    }

    #[test]
    fn selector_is_default_off() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }

    #[test]
    fn tier1_weak_counter_differential_matches_retained_body() {
        let mut seed = 0x2464_2282u32;
        for _ in 0..3_000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let previous = seed % 5;
            let (mut rust_memory, mut rust_regs) = fixture(previous);
            let (mut lifted_memory, mut lifted_regs) = fixture(previous);
            let mut rust_calls = Calls::default();
            let mut lifted_calls = Calls::default();
            assert!(run(ROOT, &mut rust_memory, &mut rust_calls, &mut rust_regs));
            crate::c2r::data_missions::game_slot_4c_update_member_count_post_progress_messages_and_complete(
                &mut lifted_memory,
                &mut lifted_calls,
                &mut lifted_regs,
            );
            assert_eq!(
                rust_memory.read32(0x30000 + 0x2c),
                lifted_memory.read32(0x30000 + 0x2c)
            );
            assert_eq!(rust_calls.0, lifted_calls.0);
        }
    }

    #[test]
    fn mutation_checks_cover_counter_gate_and_unsupported_effect_path() {
        let base = StoryObjectiveProgressState {
            kind: SlotKind::Objective,
            status: SlotStatus::Active,
            progress_flags: 0,
            previous_members: 3,
            eligible_members: 1,
            required_members: u32::MAX,
            has_distance_trigger: false,
        };
        let plan = plan_story_objective_progress(base).unwrap();
        assert!(plan.write_progress);
        assert_eq!(plan.next_members, 1);
        assert!(!plan.finish);
        assert!(
            plan_story_objective_progress(StoryObjectiveProgressState {
                kind: SlotKind::Other,
                ..base
            })
            .is_none()
        );
        assert!(
            plan_story_objective_progress(StoryObjectiveProgressState {
                progress_flags: 1,
                ..base
            })
            .is_none()
        );
        assert!(
            plan_story_objective_progress(StoryObjectiveProgressState {
                has_distance_trigger: true,
                ..base
            })
            .is_none()
        );
        let complete = plan_story_objective_progress(StoryObjectiveProgressState {
            previous_members: 0,
            eligible_members: 1,
            required_members: 1,
            ..base
        })
        .unwrap();
        assert!(complete.finish);
    }

    #[test]
    fn weak_feedback_differential_matches_lifted_helper_order() {
        let mut seed = 0x2467_2282u32;
        for _ in 0..3_000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let players = seed as usize % 4 + 1;
            let remaining = seed.rotate_left(7) % 6;
            let mut memory = Ram::default();
            let slot = 0x30000;
            let members = 0x40000;
            let member = 0x50000;
            let player_table = 0x60000;
            put32(&mut memory, OBJECTIVES, slot);
            memory.write8(MODE, story_flow::MODE_STORY);
            memory.write8(slot + 0x48, 0);
            memory.write8(slot + 0x0b, 2);
            memory.write8(slot + 9, 1);
            put32(&mut memory, slot + 0x0c, members);
            put32(&mut memory, members + 0x54, member);
            put32(&mut memory, member + 0x1cc, 0.0f32.to_bits());
            put32(&mut memory, slot + 0x2c, 0);
            put32(&mut memory, slot + 0x30, remaining + 1);
            put32(&mut memory, slot + 0x10, u32::MAX);
            for (offset, value) in [(0x24, 0x121u16), (0x26, 0x122u16)] {
                for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
                    memory.write8(slot + offset + i as u32, byte);
                }
            }
            put32(&mut memory, 0x790950, players as u32);
            put32(&mut memory, 0x7b0c48, player_table);
            for i in 0..players {
                put32(
                    &mut memory,
                    player_table + (i as u32) * 0x1584 + 4,
                    0x70000 + i as u32 * 4,
                );
            }

            let state = StoryProgressFeedbackState {
                counter_changed: true,
                slot_id: 0,
                remaining,
                progress_messages_enabled: true,
                one_remaining_text_id: 0x121,
                many_remaining_text_id: 0x122,
                local_players: (0..players as u32).collect(),
            };
            let effects = plan_story_progress_feedback(&state);
            let mut calls = Calls::default();
            let (_, mut regs) = fixture(0);
            regs.esp = 0x90000;
            put32(&mut memory, regs.esp + 4, 0);
            crate::c2r::data_missions::game_slot_4c_update_member_count_post_progress_messages_and_complete(
                &mut memory, &mut calls, &mut regs,
            );

            let expected_messages = if remaining == 0 { 0 } else { players };
            let mut expected = Vec::new();
            if remaining == 0 {
                expected.push(0x2274c0);
            }
            for _ in 0..expected_messages {
                expected.extend([0x3d7b0, 0x151030, 0x12d350]);
            }
            expected.extend([0x45390, 0x45820]);
            assert_eq!(calls.0, expected, "seed={seed:#x}");
            assert_eq!(
                effects.len(),
                expected_messages + 1 + usize::from(remaining == 0),
                "seed={seed:#x}"
            );
            if remaining == 0 {
                assert_eq!(
                    effects[0],
                    StoryProgressEffect::FinishObjectiveSlot { slot_id: 0 }
                );
            }
            assert_eq!(
                effects.last(),
                Some(&StoryProgressEffect::PlayProgressSound { sound_id: 0xd6c })
            );
        }
    }

    #[test]
    fn feedback_mutations_cover_labels_gates_and_effect_order() {
        let state = StoryProgressFeedbackState {
            counter_changed: true,
            slot_id: 2,
            remaining: 1,
            progress_messages_enabled: true,
            one_remaining_text_id: 0x11,
            many_remaining_text_id: 0x22,
            local_players: vec![4, 7],
        };
        assert_eq!(
            plan_story_progress_feedback(&state),
            vec![
                StoryProgressEffect::PostObjectiveMessage {
                    player: 4,
                    text_id: 0x11,
                    remaining: 1
                },
                StoryProgressEffect::PostObjectiveMessage {
                    player: 7,
                    text_id: 0x11,
                    remaining: 1
                },
                StoryProgressEffect::PlayProgressSound { sound_id: 0xd6c },
            ]
        );
        assert_eq!(
            plan_story_progress_feedback(&StoryProgressFeedbackState {
                slot_id: 2,
                remaining: 2,
                ..state.clone()
            })[0],
            StoryProgressEffect::PostObjectiveMessage {
                player: 4,
                text_id: 0x22,
                remaining: 2
            }
        );
        let disabled = StoryProgressFeedbackState {
            progress_messages_enabled: false,
            ..state
        };
        assert!(plan_story_progress_feedback(&disabled).is_empty());
        assert!(
            plan_story_progress_feedback(&StoryProgressFeedbackState {
                counter_changed: false,
                ..disabled
            })
            .is_empty()
        );
    }

    #[derive(Default)]
    struct Handlers(Vec<StoryProgressEffect>);
    impl StoryProgressEffectHandlers for Handlers {
        fn finish_objective_slot(&mut self, slot_id: u32) {
            self.0
                .push(StoryProgressEffect::FinishObjectiveSlot { slot_id });
        }
        fn post_objective_message(&mut self, player: u32, text_id: u16, remaining: u32) {
            self.0.push(StoryProgressEffect::PostObjectiveMessage {
                player,
                text_id,
                remaining,
            });
        }
        fn play_sound(&mut self, sound_id: u32) {
            self.0
                .push(StoryProgressEffect::PlayProgressSound { sound_id });
        }
    }

    #[test]
    fn native_handler_dispatch_preserves_typed_effect_order() {
        let state = StoryProgressFeedbackState {
            counter_changed: true,
            slot_id: 9,
            remaining: 2,
            progress_messages_enabled: true,
            one_remaining_text_id: 0x31,
            many_remaining_text_id: 0x32,
            local_players: vec![1, 3],
        };
        let mut handlers = Handlers::default();
        assert!(run_story_progress_feedback(&state, &mut handlers));
        assert_eq!(handlers.0, plan_story_progress_feedback(&state));
    }
}
