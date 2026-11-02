// SPDX-License-Identifier: GPL-3.0-or-later
//! T2448: Story objective-result reducer at `0x228870`, default-on after the EQUAL 180 s gate.
//!
//! The adapter owns the typed aggregate decision. Slot updates, mode callbacks, result messages
//! and the guarded end/phase-8 helper remain explicit guest contracts at their original seams.
//! Special flag-0x40 and non-Story cases retain the lifted implementation.
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
    state_wire::story_flow,
};
use alloc::vec::Vec;

const MODE: u32 = 0x7de455;
const FLAGS: u32 = 0x7de45c;
const FRAME: u32 = 0x7de318;
const PLAYER_COUNT: u32 = 0x790950;
const PLAYERS: u32 = 0x7b0c48;
const OBJECTIVE_COUNT: u32 = 0x78cec8;
const OBJECTIVE_INDICES: u32 = 0x75c14c;
const OBJECTIVES: u32 = 0x78cecc;
const OBJECTIVE_COUNTER: u32 = 0x75c134;
const MATCH_PHASE: u32 = 0x7de30c;
const TRANSITION_TIMER: u32 = 0x7de300;

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value.is_none() || value == Some(b"rust")
}

fn guest_call<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
    target: u32,
    args: &[u32],
) {
    call_cdecl(guest, m, r, target, args);
}

#[cfg(not(test))]
fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_RESULT".as_ptr());
        if value.is_null() {
            enabled_from(None)
        } else {
            enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
        }
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Returns false without side effects when the Story aggregate contract does not apply.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if va != 0x228870
        || !enabled()
        || m.read8(MODE) != story_flow::MODE_STORY
        || m.read32(0x4e7944) != 0
        || m.read8(0x7de458) & 0x40 != 0
    {
        return false;
    }

    let level = m.read32(0x79094c);
    guest_call(m, guest, r, 0x6a850, &[level]);
    let descriptor = r.eax;
    let count = m.read32(OBJECTIVE_COUNT) as i32;
    if count > 0 {
        for i in 0..count as u32 {
            let index = m.read32(OBJECTIVE_INDICES.wrapping_add(i.wrapping_mul(4)));
            if !crate::story_objective_progress_lane::run_for_slot(m, guest, r, index) {
                guest_call(m, guest, r, 0x2282b0, &[index]);
            }
        }
    }
    match m.read8(MODE) {
        3 => guest_call(m, guest, r, 0x232270, &[]),
        5 => guest_call(m, guest, r, 0x2284d0, &[]),
        _ => {}
    }
    let callback = if descriptor == 0 {
        0
    } else {
        m.read32(descriptor.wrapping_add(0x110))
    };
    if callback != 0 {
        guest_call(m, guest, r, callback, &[]);
    }

    let result_flags = m.read32(FLAGS);
    let player_count = m.read32(PLAYER_COUNT) as i32;
    let all_players_down = player_count <= 0
        || (0..player_count as u32).all(|i| {
            m.read32(
                PLAYERS
                    .wrapping_add(i.wrapping_mul(0x1584))
                    .wrapping_add(0x114),
            ) == 0x10
        });
    let n = m.read32(OBJECTIVE_COUNT) as i32;
    let objectives = if n > 0 {
        (0..n as u32)
            .map(|i| {
                let index = m.read32(OBJECTIVE_INDICES.wrapping_add(i.wrapping_mul(4)));
                let base = OBJECTIVES.wrapping_add(index.wrapping_mul(0x4c));
                story_flow::ObjectiveState {
                    included: m.read8(base.wrapping_add(8)) == 0,
                    status: m.read8(base.wrapping_add(0xb)),
                    flags: m.read8(base.wrapping_add(9)),
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let evaluation = story_flow::reduce_story_result(
        story_flow::StoryResultState {
            result_flags,
            phase: story_flow::StoryMatchPhase::from_raw(m.read32(MATCH_PHASE)),
            transition_timer: m.read32(TRANSITION_TIMER),
        },
        story_flow::StoryResultInput {
            mode_type: m.read8(MODE),
            player_count,
            all_players_down,
            objective_counter: m.read32(OBJECTIVE_COUNTER) as i32,
            frame: m.read32(FRAME),
            objectives: &objectives,
        },
    );
    if evaluation.auto_finish_flagged {
        guest_call(m, guest, r, 0x227e40, &[]);
    }

    match evaluation.event {
        Some(story_flow::MissionResultEvent::ObjectiveFailedPending) => {
            guest_call(m, guest, r, 0x3d7b0, &[0x15f8, 0]);
            let text = r.eax;
            guest_call(m, guest, r, 0x1cdae0, &[]);
            let local_count = r.eax as i32;
            for player in 0..local_count.max(0) as u32 {
                guest_call(m, guest, r, 0x12d350, &[player, 2, text, 0]);
            }
        }
        Some(story_flow::MissionResultEvent::ObjectivesFailed) => {
            guest_call(m, guest, r, 0x3d7b0, &[0x15f7, 0]);
            let text = r.eax;
            guest_call(m, guest, r, 0x1cdae0, &[]);
            let local_count = r.eax as i32;
            for player in 0..local_count.max(0) as u32 {
                guest_call(m, guest, r, 0x12d350, &[player, 4, text, 0]);
            }
        }
        Some(story_flow::MissionResultEvent::Complete) => {
            let level = m.read32(0x79094c);
            if level == 0x64 {
                guest_call(m, guest, r, 0x33ed70, &[0x15f9]);
            } else {
                guest_call(m, guest, r, 0x3d7b0, &[0x15f9, 0]);
            }
            let text = r.eax;
            guest_call(m, guest, r, 0x1cdae0, &[]);
            let local_count = r.eax as i32;
            for player in 0..local_count.max(0) as u32 {
                guest_call(m, guest, r, 0x12d350, &[player, 4, text, 0]);
            }
        }
        _ => {}
    }
    m.write32(FLAGS, evaluation.state.result_flags);
    if evaluation.request_end {
        if !crate::story_end_transition_lane::run(0x67450, m, guest, r) {
            guest_call(m, guest, r, 0x67450, &[0]);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::enabled_from;
    #[test]
    fn switch_defaults_on_and_accepts_explicit_off() {
        assert!(enabled_from(None));
        assert!(!enabled_from(Some(b"off")));
        assert!(enabled_from(Some(b"rust")));
    }
}
