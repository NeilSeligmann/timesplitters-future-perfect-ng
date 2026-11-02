// SPDX-License-Identifier: GPL-3.0-or-later
//! T2451: typed ordinary Story end bookkeeping at `0x66FC0`, selected only from T2450.
//! Neighboring profile, local-player, sound, event and state-dispatch routines stay call contracts.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::rt::RegsExt,
    memory::GuestMemory,
};

const MODE: u32 = 0x7de455;
const LEVEL: u32 = 0x79094c;
const FLAGS: u32 = 0x7de458;
const RESULT: u32 = 0x7de45c;
const MATCH_STATE: u32 = 0x7de30c;
const MATCH_TIMER: u32 = 0x7de300;
const FRAME: u32 = 0x7de318;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FinalAction {
    Dispatch(u32),
    Return,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EndState {
    flags: u32,
    result: u32,
    mode: u8,
    level: u32,
    match_state: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EndPlan {
    enter_state_8: bool,
    final_action: FinalAction,
}

/// Typed transcription of the state/timer and final `0x45A00` branch decisions.
fn plan(state: EndState) -> EndPlan {
    let special_route = state.flags & (0x40 | 0x0800_0000) != 0;
    let final_action = if !special_route {
        if state.result & 0x1_0000 != 0 {
            FinalAction::Dispatch(1)
        } else if state.mode != 10 || state.level == 0x64 || state.level == 0x38 {
            FinalAction::Dispatch(2)
        } else {
            FinalAction::Return
        }
    } else if state.result & 0x8000 != 0 {
        FinalAction::Dispatch(2)
    } else {
        FinalAction::Dispatch(1)
    };
    EndPlan {
        enter_state_8: state.match_state != 8,
        final_action,
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
        let value = getenv(c"TSFP_STORY_END_BOOKKEEPING".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Runs the default-off Story attachment. `false` leaves the original lifted `0x66FC0` active.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if va != 0x66fc0 || !enabled() || m.read8(MODE) != 10 {
        return false;
    }

    // Preserve the lifted callback sequence. These neighboring helpers remain explicit ABI seams.
    r.esp = r.esp.wrapping_sub(0x30);
    r.call(guest, 0x63eb0);
    let match_state_at_entry = m.read32(MATCH_STATE);
    if match_state_at_entry != 8 {
        m.write32(MATCH_STATE, 8);
        m.write32(MATCH_TIMER, 0);
    }
    r.push(m, r.esi);
    r.call(guest, 0x668f0);
    r.esi = 0;
    r.call(guest, 0x1cdae0);
    if (r.eax as i32) > 0 {
        loop {
            r.push(m, r.esi);
            r.call(guest, 0x2e4b80);
            r.esp = r.esp.wrapping_add(4);
            if r.eax != 0 {
                r.push(m, r.esi);
                r.call(guest, 0x2e6d40);
                r.esp = r.esp.wrapping_add(4);
                r.call(guest, 0x3533f0);
                if r.eax != 0 {
                    r.push(m, r.esi);
                    r.call(guest, 0x2e7010);
                    r.esp = r.esp.wrapping_add(4);
                }
            }
            r.esi = r.esi.wrapping_add(1);
            r.call(guest, 0x1cdae0);
            if (r.esi as i32) >= (r.eax as i32) {
                break;
            }
        }
    }

    let frame = m.read32(FRAME);
    m.write32(0x6f11b8, frame);
    r.call(guest, 0x455d0);
    r.call(guest, 0x15bdc0);
    r.ecx = r.esp.wrapping_add(4);
    r.push(m, r.ecx);
    r.call(guest, 0x45390);
    r.edx = r.esp.wrapping_add(8);
    r.push(m, r.edx);
    m.write32(r.esp.wrapping_add(0xc), 0xd6a);
    r.call(guest, 0x45820);

    r.esp = r.esp.wrapping_add(8);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    let decision = plan(EndState {
        flags: m.read32(FLAGS),
        result: m.read32(RESULT),
        mode: m.read8(MODE),
        level: m.read32(LEVEL),
        match_state: match_state_at_entry,
    });
    if let FinalAction::Dispatch(state) = decision.final_action {
        r.push(m, 0);
        r.push(m, state);
        r.call(guest, 0x45a00);
        r.esp = r.esp.wrapping_add(8);
    }
    r.esp = r.esp.wrapping_add(0x30);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifted(s: EndState) -> EndPlan {
        let mut dispatch = None;
        if s.flags & 0x40 == 0 {
            if s.flags & 0x0800_0000 == 0 {
                if s.result & 0x1_0000 != 0 {
                    dispatch = Some(1);
                } else if s.mode != 10 {
                    dispatch = Some(2);
                } else if s.level == 0x64 {
                    dispatch = Some(2);
                } else if s.level == 0x38 {
                    dispatch = Some(2);
                }
            } else if s.result & 0x8000 != 0 {
                dispatch = Some(2);
            } else {
                dispatch = Some(1);
            }
        } else if s.result & 0x8000 != 0 {
            dispatch = Some(2);
        } else {
            dispatch = Some(1);
        }
        EndPlan {
            enter_state_8: s.match_state != 8,
            final_action: dispatch.map_or(FinalAction::Return, FinalAction::Dispatch),
        }
    }

    #[test]
    fn switch_is_explicit_and_default_off() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }

    #[test]
    fn typed_plan_matches_lifted_branches_over_3000_fabricated_cases() {
        let mut x = 0x2451_17u32;
        for _ in 0..3000 {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            let s = EndState {
                flags: x,
                result: x.rotate_left(11),
                mode: (x >> 8) as u8,
                level: x.rotate_right(5),
                match_state: x.rotate_left(19),
            };
            assert_eq!(plan(s), lifted(s));
        }
    }

    #[test]
    fn mutation_of_special_flags_changes_dispatch_contract() {
        let ordinary = EndState {
            flags: 0,
            result: 0,
            mode: 10,
            level: 2,
            match_state: 8,
        };
        assert_eq!(plan(ordinary).final_action, FinalAction::Return);
        assert_ne!(
            plan(ordinary).final_action,
            plan(EndState {
                flags: 0x40,
                ..ordinary
            })
            .final_action
        );
        assert_ne!(
            plan(ordinary).final_action,
            plan(EndState {
                flags: 0x0800_0000,
                ..ordinary
            })
            .final_action
        );
        assert!(!plan(ordinary).enter_state_8);
        assert!(
            plan(EndState {
                match_state: 7,
                ..ordinary
            })
            .enter_state_8
        );
    }
}
