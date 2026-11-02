// SPDX-License-Identifier: GPL-3.0-or-later
//! T2494: synchronous host callback for the Story owner at the original mode tick.
//!
//! The callback is installed by the Bevy host on the game thread. An absent callback or a
//! callback result of zero preserves the lifted C2R call. It may return one only after a
//! complete typed Story owner and its world/effect bridge are installed.

use core::sync::atomic::{AtomicUsize, Ordering};

pub type StoryMissionTick = unsafe extern "C" fn() -> u32;

static CALLBACK: AtomicUsize = AtomicUsize::new(0);

/// Install or clear the optional host Story tick callback.
///
/// # Safety
/// The function pointer must remain callable for every mode tick until it is replaced or
/// cleared. The Bevy host registers a process-lifetime callback before entering the C host.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_ports_register_story_mission_tick(
    callback: Option<StoryMissionTick>,
) {
    let address = callback.map_or(0, |callback| callback as usize);
    CALLBACK.store(address, Ordering::Release);
}

/// Run the registered owner synchronously and otherwise invoke the lifted fallback.
/// The return value tells the caller whether the native owner handled this fixed mode tick.
#[inline]
pub fn dispatch(callback: Option<StoryMissionTick>, lifted: impl FnOnce()) -> bool {
    if callback.is_some_and(|callback| {
        // SAFETY: registration documents the callback lifetime; the atomic publishes its
        // address before this game-thread load.
        unsafe { callback() != 0 }
    }) {
        true
    } else {
        lifted();
        false
    }
}

#[inline]
pub fn dispatch_registered(lifted: impl FnOnce()) -> bool {
    let address = CALLBACK.load(Ordering::Acquire);
    let callback = if address == 0 {
        None
    } else {
        // SAFETY: callback registration documents the function pointer lifetime and ABI.
        Some(unsafe { core::mem::transmute::<usize, StoryMissionTick>(address) })
    };
    dispatch(callback, lifted)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn decline() -> u32 {
        0
    }

    unsafe extern "C" fn handle() -> u32 {
        1
    }

    #[test]
    fn callback_result_preserves_lifted_or_claims_tick() {
        let mut lifted = 0;
        assert!(!dispatch(None, || lifted += 1));
        assert_eq!(lifted, 1);
        assert!(!dispatch(Some(decline), || lifted += 1));
        assert_eq!(lifted, 2);
        assert!(dispatch(Some(handle), || lifted += 1));
        assert_eq!(lifted, 2);
    }
}
