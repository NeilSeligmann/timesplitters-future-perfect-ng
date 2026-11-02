// SPDX-License-Identifier: GPL-3.0-or-later
//! T2465: typed Story event link callback state at `0x235240`.
//!
//! The callback has two relevant operations: code 3 checks a kind-9 link against a finished
//! objective slot and requests completion of the trigger slot; code 4 marks the link word. The
//! generic event delay (`+0x38`) and the preceding `0x237E70` state pass remain separate.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackLink {
    pub kind: u32,
    pub referenced_slot: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackState {
    pub code: u32,
    pub trigger_slot: u32,
    pub link: CallbackLink,
    pub referenced_status: u8,
    pub link_marked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackPlan {
    pub mark_link: bool,
    pub finish_trigger_slot: Option<u32>,
}

/// Model the measured code-3 and code-4 transitions. Unsupported codes and link kinds are inert.
pub const fn callback_plan(state: CallbackState) -> CallbackPlan {
    CallbackPlan {
        mark_link: state.code == 4 && !state.link_marked,
        finish_trigger_slot: if state.code == 3
            && state.link.kind == 9
            && state.referenced_status == 5
        {
            Some(state.trigger_slot)
        } else {
            None
        },
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
pub fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_EVENT_LINK".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
pub fn enabled() -> bool {
    true
}

/// Dispatch the measured callback seam when called from the lifted event/slot path.
/// Unknown callback forms remain on the retained body. `false` means the caller must fall back.
pub fn run_nested<M: crate::memory::GuestMemory, C: crate::abi::GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut crate::abi::ExtRegisters,
) -> bool {
    use crate::c2r::rt::RegsExt;
    use crate::memory::GuestMemory;
    if va != 0x235240 || !enabled() {
        return false;
    }
    let slot = memory.read32(regs.esp.wrapping_add(4));
    let code = memory.read32(regs.esp.wrapping_add(8));
    if code == 4 {
        if memory.read16(slot.wrapping_add(4)) == 0 {
            let [lo, hi] = 1u16.to_le_bytes();
            memory.write8(slot.wrapping_add(4), lo);
            memory.write8(slot.wrapping_add(5), hi);
        }
        regs.esp = regs.esp.wrapping_add(4);
        return true;
    }
    if code != 3 {
        return false;
    }

    let event_index = memory.read16(slot.wrapping_add(0x2a)) as u32;
    let instance = memory
        .read32(0x78cd90)
        .wrapping_add(event_index.wrapping_mul(0x48));
    let definition = memory.read32(instance);
    if definition == 0 {
        return false;
    }
    // A link is twelve bytes at definition +0x40: mode then key/target and arguments.
    let link = CallbackLink {
        kind: memory.read32(definition.wrapping_add(0x40)),
        referenced_slot: memory.read32(definition.wrapping_add(0x48)),
    };
    if link.kind != 9 {
        return false;
    }
    let trigger_slot = memory.read32(slot);
    let slot_table = memory.read32(0x78cecc);
    let referenced_status = memory.read8(
        slot_table
            .wrapping_add(link.referenced_slot.wrapping_mul(0x4c))
            .wrapping_add(0x0b),
    );
    let plan = callback_plan(CallbackState {
        code,
        trigger_slot,
        link,
        referenced_status,
        link_marked: false,
    });
    if let Some(id) = plan.finish_trigger_slot {
        regs.push(memory, 0);
        regs.push(memory, id);
        regs.call(guest, 0x2274c0);
        regs.esp = regs.esp.wrapping_add(8);
    }
    regs.esp = regs.esp.wrapping_add(4);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_nine_finishes_only_after_referenced_slot_status_five() {
        for status in 0..=u8::MAX {
            let plan = callback_plan(CallbackState {
                code: 3,
                trigger_slot: 2,
                link: CallbackLink {
                    kind: 9,
                    referenced_slot: 5,
                },
                referenced_status: status,
                link_marked: false,
            });
            assert_eq!(plan.finish_trigger_slot, (status == 5).then_some(2));
            assert!(!plan.mark_link);
        }
    }

    #[test]
    fn code_four_marks_once_and_other_codes_do_not_mutate() {
        let base = CallbackState {
            code: 4,
            trigger_slot: 2,
            link: CallbackLink {
                kind: 9,
                referenced_slot: 5,
            },
            referenced_status: 5,
            link_marked: false,
        };
        assert!(callback_plan(base).mark_link);
        assert!(
            !callback_plan(CallbackState {
                link_marked: true,
                ..base
            })
            .mark_link
        );
        assert!(!callback_plan(CallbackState { code: 2, ..base }).mark_link);
        assert_eq!(
            callback_plan(CallbackState { code: 4, ..base }).finish_trigger_slot,
            None
        );
    }

    #[test]
    fn changing_required_slot_status_mutates_the_completion_result() {
        let ready = CallbackState {
            code: 3,
            trigger_slot: 2,
            link: CallbackLink {
                kind: 9,
                referenced_slot: 5,
            },
            referenced_status: 5,
            link_marked: false,
        };
        assert_eq!(callback_plan(ready).finish_trigger_slot, Some(2));
        assert_eq!(
            callback_plan(CallbackState {
                referenced_status: 4,
                ..ready
            })
            .finish_trigger_slot,
            None
        );
    }

    // Independent expression of the retained callback's branch conditions. Inputs here are
    // fabricated; this is a consistency check, not original-binary evidence.
    fn lifted_callback_plan(state: CallbackState) -> CallbackPlan {
        let mut mark_link = false;
        let mut finish_trigger_slot = None;
        match state.code {
            4 => mark_link = state.link_marked == false,
            3 => {
                if state.link.kind == 9 && state.referenced_status == 5 {
                    finish_trigger_slot = Some(state.trigger_slot);
                }
            }
            _ => {}
        }
        CallbackPlan {
            mark_link,
            finish_trigger_slot,
        }
    }

    #[test]
    fn tier1_weak_differential_matches_lifted_branch_model_for_4096_cases() {
        let mut seed = 0x2465_2352u32;
        for _ in 0..4096 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let state = CallbackState {
                code: seed & 7,
                trigger_slot: seed.rotate_left(9),
                link: CallbackLink {
                    kind: seed.rotate_right(5) % 12,
                    referenced_slot: seed.rotate_left(17),
                },
                referenced_status: (seed >> 16) as u8,
                link_marked: seed & 0x100 != 0,
            };
            assert_eq!(callback_plan(state), lifted_callback_plan(state));
        }
    }
}
