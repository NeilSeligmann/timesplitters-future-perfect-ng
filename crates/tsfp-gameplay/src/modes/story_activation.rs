// SPDX-License-Identifier: GPL-3.0-or-later
//! T2446: Story-selected activation boundary at `0x670C0`.
//!
//! The entry point belongs to the shared session/setup path. This adapter owns only the Story
//! selection whose runtime dispatch table chooses the local Story initializer; unrelated modes
//! and other dispatch targets remain on the hybrid path. Tables and setup bytes are read from the
//! current guest image. No game data is embedded.
use super::ModeHost;

const SETUP_SOURCE: u32 = 0x007d_e340;
const SETUP_WORK: u32 = 0x007d_e400;
const STORY_ACTIVE_TYPE: u8 = 10;
const STORY_ID_MIN: u8 = 0x65;
const STORY_ID_MAX: u8 = 0x6b;
const STORY_DISPATCH: u32 = 0x0006_71af;

const UPDATE_SESSION_STATE: u32 = 0x0015_3ac0;
const SETUP_ENTRY: u32 = 0x0006_4180;
const CLEAR_SESSION_TABLE: u32 = 0x0006_4090;
const CLEAR_SESSION_GLOBAL: u32 = 0x0006_40b0;
const SET_SETUP_OPTION: u32 = 0x0003_567f0;
const ACTIVATE_MODE: u32 = 0x0004_2510;
const SETUP_BYTES: usize = 0xa0;

/// Runtime mode record: named Story fields plus preserved bytes owned by neighboring mode paths.
/// The unclassified tail is copied through without assigning speculative meanings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryActivationRecord {
    bytes: [u8; SETUP_BYTES],
}

impl StoryActivationRecord {
    fn read<H: ModeHost>(h: &H, address: u32) -> Self {
        Self {
            bytes: core::array::from_fn(|index| h.read8(address + index as u32)),
        }
    }

    pub fn identifier(&self) -> u8 {
        self.bytes[0x54]
    }

    pub fn active_type(&self) -> u8 {
        self.bytes[0x55]
    }

    fn set_local_player_defaults<H: ModeHost>(&mut self, h: &mut H, address: u32) {
        self.bytes[0] = 1;
        self.bytes[1] = 1;
        self.bytes[2] = 0;
        h.write8(address, 1);
        h.write8(address + 1, 1);
        h.write8(address + 2, 0);
    }

    fn option(&self) -> u8 {
        self.bytes[1]
    }

    fn item_count(&self) -> u8 {
        self.bytes[3]
    }

    fn item(&self, slot: u8) -> u8 {
        self.bytes[4 + usize::from(slot)]
    }

    fn publish<H: ModeHost>(&self, h: &mut H, address: u32) {
        for (index, bytes) in self.bytes.chunks_exact(4).enumerate() {
            h.write32(
                address + (index as u32) * 4,
                u32::from_le_bytes(bytes.try_into().expect("four-byte chunk")),
            );
        }
    }
}

/// Pure gate for the Story branch. The dispatch table is runtime data; only its `0x671AF` arm is
/// ported here. Other arms are left to the original implementation until separately specified.
pub fn is_story_activation(active_type: u8, identifier: u8, dispatch_target: u32) -> bool {
    active_type == STORY_ACTIVE_TYPE
        && (STORY_ID_MIN..=STORY_ID_MAX).contains(&identifier)
        && dispatch_target == STORY_DISPATCH
}

/// Runs the typed Story initializer when the current setup resolves to its measured local arm.
/// `None` means the caller must retain the existing implementation. The shared session protocol
/// and all non-Story dispatch arms stay behind the hybrid call boundary.
pub fn activate_story<H: ModeHost>(h: &mut H) -> Option<u32> {
    let mut setup = StoryActivationRecord::read(h, SETUP_SOURCE);
    let identifier = setup.identifier();
    if !(STORY_ID_MIN..=STORY_ID_MAX).contains(&identifier) {
        return None;
    }
    let route_index = u32::from(h.read8(0x0006_7270 + u32::from(identifier - STORY_ID_MIN)));
    let dispatch_target = h.read32(0x0006_7268 + route_index * 4);
    if !is_story_activation(setup.active_type(), identifier, dispatch_target) {
        return None;
    }

    // Initial transition reset and setup snapshot, in the original store/call order.
    h.write32(0x004d_2878, 3);
    h.write32(0x004d_287c, u32::MAX);
    h.write32(0x006f_11a4, 0);
    h.write32(0x007d_e318, 0);
    h.write32(0x0074_97b0, 0);
    h.write32(0x007d_e31c, 0);
    if h.read32(0x007d_e30c) != 2 {
        h.write32(0x007d_e30c, 2);
        h.write32(0x007d_e300, 0);
    }
    h.write32(0x006f_11a8, 0);
    h.write32(0x006f_11ac, 0);
    h.write32(0x006f_11b0, 0);
    h.write32(0x007d_e308, 0);
    h.write32(0x007d_e324, 0);
    h.write32(0x007d_e320, 0);
    h.write32(0x007d_e314, u32::MAX);
    h.write32(0x007d_e310, u32::MAX);
    h.call(UPDATE_SESSION_STATE, &[]);
    h.write32(0x004d_2880, u32::MAX);
    setup.publish(h, SETUP_WORK);
    let session_value = h.read32(0x0047_5c78);
    h.write32(0x007d_e4a4, session_value);

    for entry in 1..=4 {
        h.call(SETUP_ENTRY, &[entry, 0]);
    }
    h.call(CLEAR_SESSION_TABLE, &[]);
    h.call(CLEAR_SESSION_GLOBAL, &[]);

    // The measured 0x671AF arm selects the default local-player setup before the common tail.
    setup.set_local_player_defaults(h, SETUP_WORK);
    h.write8(0x007d_e455, 0);
    if h.read8(0x007d_e455).wrapping_sub(2) == 0 {
        let flags = h.read32(0x007d_e458) | 1;
        h.write32(0x007d_e458, flags);
    }
    h.write32(0x0079_094c, u32::from(identifier));
    h.write32(0x0079_0950, 1);
    if !matches!(identifier, 0x64..=0x66) {
        h.call(SET_SETUP_OPTION, &[u32::from(setup.option())]);
    }
    h.write32(0x007d_e478, 0);
    h.write32(0x007d_e47c, 0);
    h.write32(0x007d_e480, 0);
    h.write32(0x007d_e45c, 0);
    h.write32(0x007d_e484, 0);
    let item_count = setup.item_count();
    for slot in 0..item_count {
        let item = setup.item(slot);
        h.write8(0x007d_e477 + u32::from(item), 1);
    }
    h.write32(0x007d_e488, 0);
    Some(h.call(ACTIVATE_MODE, &[]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_measured_story_dispatch_arm_is_selected() {
        assert!(is_story_activation(10, 0x65, STORY_DISPATCH));
        assert!(is_story_activation(10, 0x6b, STORY_DISPATCH));
        assert!(!is_story_activation(9, 0x65, STORY_DISPATCH));
        assert!(!is_story_activation(10, 0x64, STORY_DISPATCH));
        assert!(!is_story_activation(10, 0x65, 0x671c9));
    }
}
