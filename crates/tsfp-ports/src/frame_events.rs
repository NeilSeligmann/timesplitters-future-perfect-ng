// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable transitions for the measured one-slot frame event contract.
//!
//! REA documents only the latch/callback pair consumed by 0x3C440. These
//! helpers intentionally do not model a larger queue. `GuestMemory` keeps the
//! transition usable by the typed host seam, where the two words resolve to
//! Rust-owned `FrameEventState` rather than the guest image.

use crate::memory::GuestMemory;

pub const FRAME_EVENT_LATCH: u32 = 0x007e_b2e0;
pub const FRAME_EVENT_CALLBACK: u32 = 0x007e_b2e4;

/// Guest target stored for the frame's alternate event callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameEventCallback(pub u32);

/// Apply the 0x3C440 transition after history rotation.
///
/// A nonzero latch clears both the latch and callback words. A zero latch
/// leaves the callback untouched for the later callback-dispatch branch.
pub fn clear_callback_when_latched<M: GuestMemory>(memory: &mut M) -> bool {
    if memory.read32(FRAME_EVENT_LATCH) == 0 {
        return false;
    }

    memory.write32(FRAME_EVENT_LATCH, 0);
    memory.write32(FRAME_EVENT_CALLBACK, 0);
    true
}

/// Return the callback target for the measured post-simulation dispatch branch.
///
/// The callback remains in the event slot; the next frame's latch transition
/// is responsible for clearing it when required.
pub fn callback_to_dispatch<M: GuestMemory>(memory: &M) -> Option<FrameEventCallback> {
    let callback = memory.read32(FRAME_EVENT_CALLBACK);
    (callback != 0).then_some(FrameEventCallback(callback))
}

#[cfg(test)]
mod tests {
    use super::{
        FRAME_EVENT_CALLBACK, FRAME_EVENT_LATCH, FrameEventCallback, callback_to_dispatch,
        clear_callback_when_latched,
    };
    use crate::frame_state::FrameEventState;
    use crate::memory::GuestMemory;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn frame_latch_clears_both_words_but_empty_latch_preserves_callback() {
        let mut memory = Memory::default();
        memory.write32(FRAME_EVENT_LATCH, 0);
        memory.write32(FRAME_EVENT_CALLBACK, 0x1234);
        assert!(!clear_callback_when_latched(&mut memory));
        assert_eq!(memory.read32(FRAME_EVENT_CALLBACK), 0x1234);

        memory.write32(FRAME_EVENT_LATCH, 1);
        assert!(clear_callback_when_latched(&mut memory));
        assert_eq!(memory.read32(FRAME_EVENT_LATCH), 0);
        assert_eq!(memory.read32(FRAME_EVENT_CALLBACK), 0);
    }

    #[test]
    fn frame_callback_dispatch_preserves_the_registered_target() {
        let mut memory = Memory::default();
        assert_eq!(callback_to_dispatch(&memory), None);

        memory.write32(FRAME_EVENT_CALLBACK, 0x0012_3456);
        assert_eq!(
            callback_to_dispatch(&memory),
            Some(FrameEventCallback(0x0012_3456))
        );
        assert_eq!(memory.read32(FRAME_EVENT_CALLBACK), 0x0012_3456);
    }

    #[test]
    fn owned_transition_clears_latched_pair_and_preserves_unlatched_callback() {
        let events = FrameEventState::new();
        events.initialize(0, 0x0012_3456);
        assert_eq!(
            events.callback_to_dispatch().map(FrameEventCallback),
            Some(FrameEventCallback(0x0012_3456))
        );
        assert!(!events.clear_callback_when_latched());
        assert_eq!(events.snapshot(), [0, 0x0012_3456]);

        events.write_access(FRAME_EVENT_LATCH, 4, 1);
        assert!(events.clear_callback_when_latched());
        assert_eq!(events.snapshot(), [0, 0]);
        assert_eq!(events.callback_to_dispatch(), None);
    }
}
