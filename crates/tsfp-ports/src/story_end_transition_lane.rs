// SPDX-License-Identifier: GPL-3.0-or-later
//! T2450: typed Story admission/dispatch decision for the `0x67450` end transition.
//! The caller is only the opt-in T2448 Story reducer; every other call remains lifted.
use crate::state_wire::story_flow::{
    StoryEndInput, StoryEndRoute, StoryMatchPhase, StoryResultState,
};
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
};

const MODE: u32 = 0x7de455;
const FLAGS: u32 = 0x7de458;
const STATE: u32 = 0x7de30c;
const TIMER: u32 = 0x7de300;

/// Compatibility shim for the existing Tier 1 lifted-branch transcription test.
fn admission(
    mode: u8,
    flags: u32,
    match_state: u32,
    online_role_2: bool,
    session_allows: bool,
) -> crate::state_wire::story_flow::StoryEndPlan {
    crate::state_wire::story_flow::plan_story_end(StoryEndInput {
        mode_type: mode,
        route_flags: flags,
        state: StoryResultState {
            result_flags: 0,
            phase: StoryMatchPhase::from_raw(match_state),
            transition_timer: 9,
        },
        online_role_2,
        session_allows,
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
        let value = getenv(c"TSFP_STORY_END_TRANSITION".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

fn guest_call<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
    target: u32,
) {
    call_cdecl(guest, m, r, target, &[]);
}

/// Runs the Story-only transition adapter. `false` means the caller must use lifted `0x67450`.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if va != 0x67450 || !enabled() || m.read8(MODE) != 10 {
        return false;
    }

    // Online role-2 admission is a hard early return unless the session-wide slot predicate passes.
    guest_call(m, guest, r, 0x355f10);
    let online_role_2 = r.eax != 0;
    let session_allows = if online_role_2 {
        guest_call(m, guest, r, 0x357690);
        r.eax != 0
    } else {
        true
    };

    let plan = crate::state_wire::story_flow::plan_story_end(StoryEndInput {
        mode_type: m.read8(MODE),
        route_flags: m.read32(FLAGS),
        state: StoryResultState {
            result_flags: m.read32(0x7de45c),
            phase: StoryMatchPhase::from_raw(m.read32(STATE)),
            transition_timer: m.read32(TIMER),
        },
        online_role_2,
        session_allows,
    });
    if !plan.admitted {
        return true;
    }
    if plan.notify_players {
        m.write32(0x6f11a4, 1);
        guest_call(m, guest, r, 0x66e90);
    }
    match plan.route {
        StoryEndRoute::OrdinaryEnd => {
            if !crate::story_ordinary_end_lane::run(0x66fc0, m, guest, r) {
                guest_call(m, guest, r, 0x66fc0);
            }
        }
        StoryEndRoute::DescriptorCompletion => {
            if plan.enter_phase_8 {
                m.write32(STATE, plan.next_state.phase.raw());
                m.write32(TIMER, plan.next_state.transition_timer);
            }
            guest_call(m, guest, r, 0x231a20);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // Branch-for-branch transcription of the lifted `game_match_end_post_player_messages_then_enter_state_8`.
    fn lifted(
        mode: u8,
        flags: u32,
        state: u32,
        online_role_2: bool,
        session_allows: bool,
    ) -> crate::state_wire::story_flow::StoryEndPlan {
        let mut notify = true;
        if mode == 3 {
            notify = false;
        }
        if mode == 7 {
            notify = false;
        }
        let mut ordinary = false;
        if flags & 0x40 == 0 {
            if flags & 0x0800_0000 == 0 {
                ordinary = true;
            }
        }
        let mut enter = false;
        if !ordinary && state != 8 {
            enter = true;
        }
        let state = StoryResultState {
            result_flags: 0,
            phase: StoryMatchPhase::from_raw(state),
            transition_timer: 9,
        };
        crate::state_wire::story_flow::StoryEndPlan {
            admitted: !online_role_2 || session_allows,
            notify_players: notify,
            route: if ordinary {
                StoryEndRoute::OrdinaryEnd
            } else {
                StoryEndRoute::DescriptorCompletion
            },
            enter_phase_8: enter,
            next_state: if enter {
                StoryResultState {
                    phase: StoryMatchPhase::Phase8,
                    transition_timer: 0,
                    ..state
                }
            } else {
                state
            },
        }
    }

    #[test]
    fn switch_is_explicit_and_default_off() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }

    #[test]
    fn typed_admission_matches_lifted_branches_over_3000_fabricated_cases() {
        let mut x = 0x2450_17u32;
        for _ in 0..3000 {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            let mode = (x >> 8) as u8;
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            let flags = x;
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            let state = x;
            let online = x & 1 != 0;
            let allows = x & 2 != 0;
            assert_eq!(
                admission(mode, flags, state, online, allows),
                lifted(mode, flags, state, online, allows)
            );
        }
    }

    #[test]
    fn mutation_of_either_special_flag_changes_dispatch() {
        let normal = admission(10, 0, 2, false, true);
        assert_ne!(normal, admission(10, 0x40, 2, false, true));
        assert_ne!(normal, admission(10, 0x0800_0000, 2, false, true));
        assert_eq!(admission(10, 0x40, 8, false, true).enter_phase_8, false);
    }

    #[test]
    fn notification_exclusions_and_state_timer_contract() {
        assert!(!admission(3, 0, 0, false, true).notify_players);
        assert!(!admission(7, 0, 0, false, true).notify_players);
        assert!(admission(10, 0, 0, false, true).notify_players);
        assert!(admission(10, 0x40, 7, false, true).enter_phase_8);
        assert!(!admission(10, 0x40, 8, false, true).enter_phase_8);
        assert!(!admission(10, 0, 0, true, false).admitted);
    }
}
