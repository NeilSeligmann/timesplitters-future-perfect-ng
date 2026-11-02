// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime-owned callback contract for C2R weapon hand transitions.
use spin::Mutex;

use crate::state_wire::GuestMemory;

/// Host callbacks installed by the owning runtime before weapon C2R dispatch.
#[derive(Clone, Copy)]
pub struct WeaponHandsHooks {
    pub enabled: fn() -> bool,
    pub holster_transition: fn(&mut dyn GuestMemory, u32, u32, u32) -> bool,
}

static HOOKS: Mutex<Option<WeaponHandsHooks>> = Mutex::new(None);

pub fn install(hooks: &'static WeaponHandsHooks) {
    *HOOKS.lock() = Some(*hooks);
}

pub fn enabled() -> bool {
    HOOKS.lock().map(|hooks| (hooks.enabled)()).unwrap_or(false)
}

pub fn holster_transition(
    memory: &mut dyn GuestMemory,
    extension: u32,
    hand_address: u32,
    active_record_id: u32,
) -> bool {
    HOOKS
        .lock()
        .map(|hooks| (hooks.holster_transition)(memory, extension, hand_address, active_record_id))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{enabled, holster_transition};

    struct Memory;

    impl crate::state_wire::GuestMemory for Memory {
        fn read8(&self, _address: u32) -> u8 {
            0
        }
        fn write8(&mut self, _address: u32, _value: u8) {}
    }

    #[test]
    fn missing_host_hooks_are_inactive() {
        assert!(!enabled());
        assert!(!holster_transition(&mut Memory, 0, 0, 0));
    }
}
