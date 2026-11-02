// SPDX-License-Identifier: GPL-3.0-or-later
//! T2194 native targeting eligibility contract. Also included by the no_std host adapter.

/// Owned inputs to the original auto-aim eligibility decision (0xCFF20).
/// The object predicate is an explicit neighbouring-system contract and stays lazy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetingState {
    pub auto_aim: bool,
    pub alternate_input: bool,
    pub player_mode: u32,
    pub session_flags: u8,
    pub session_mode: u32,
}
impl TargetingState {
    pub fn profile_allows(self) -> bool {
        self.auto_aim
            && !(self.session_flags & 0x40 != 0 && matches!(self.session_mode, 10 | 11 | 16))
    }

    pub fn mode_allows(self) -> bool {
        self.alternate_input || matches!(self.player_mode, 0x40 | 0x8000 | 0x80)
    }

    pub fn requires_object_predicate(self) -> bool {
        self.profile_allows() && !self.mode_allows()
    }

    pub fn allows(self, object_allows_targeting: bool) -> bool {
        self.profile_allows() && (self.mode_allows() || object_allows_targeting)
    }
}
