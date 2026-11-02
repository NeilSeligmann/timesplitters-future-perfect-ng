//! T2490 typed Story objective checkpoint lane; opt-in only.
use alloc::vec::Vec;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectiveCheckpoint {
    pub saved_state: u32,
    pub saved_aux_state: u32,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectiveSlot {
    pub class: u32,
    pub state: u32,
    pub auxiliary_state: u32,
    pub present: bool,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub checkpoint: Option<ObjectiveCheckpoint>,
    pub slots: Vec<ObjectiveSlot>,
    pub class_counters: Vec<i32>,
}

pub fn enabled() -> bool {
    unsafe {
        unsafe extern "C" {
            fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
        }
        let value = getenv(c"TSFP_STORY_OBJECTIVE_CHECKPOINT".as_ptr());
        !value.is_null() && core::ffi::CStr::from_ptr(value).to_bytes() == b"rust"
    }
}

pub fn restore(state: &mut State) {
    if enabled() {
        let Some(checkpoint) = state.checkpoint.as_ref() else {
            return;
        };
        for slot in &mut state.slots {
            if !slot.present {
                continue;
            }
            let was_active = slot.state == 1;
            let becomes_active = checkpoint.saved_state == 1;
            if was_active != becomes_active {
                if let Some(count) = state.class_counters.get_mut(slot.class as usize) {
                    *count += if becomes_active { 1 } else { -1 };
                }
            }
            slot.auxiliary_state = checkpoint.saved_aux_state;
            slot.state = checkpoint.saved_state;
        }
    }
}
