// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed model of the bounded controller target request commit at `0x26EFC0`.
//!
//! The target DWORDs stay opaque. This module only models the observed
//! current/pending/previous writes; actor ownership and the shared progress
//! reset are composed by `ReaTickState`.
//! Verification: Tier 0 contract tests against `docs/t-ai-route-goal-replacement.md`.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControllerTargetHandoff {
    current: u32,
    pending: i32,
    previous: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerTargetEffect {
    /// The signed pending value is negative, so the bounded commit skips it.
    NegativePendingSkipped { pending: i32 },
    /// The request equals current; pending is retired without other writes.
    EqualRequestRetired { target: u32 },
    /// A changed request installs the old current as previous and the request
    /// as current. The owner also resets its shared transition progress.
    TargetChanged { previous: u32, current: u32 },
}

/// Results specific to the selected-goal setter branches. Kept separate from
/// controller-entry effects so the runtime adapter's public match remains
/// limited to the controller-entry contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedGoalEffect {
    /// One of the measured selected-goal setter branches installed a new
    /// current target and left the same value pending for the next entry.
    SelectedGoalInstalled { previous: u32, current: u32 },
    /// The selected-goal setter's observed inequality guard did not fire.
    SelectedGoalAlreadyCurrent { target: u32 },
}

impl ControllerTargetHandoff {
    pub const fn new(current: u32, pending: i32, previous: u32) -> Self {
        Self {
            current,
            pending,
            previous,
        }
    }

    pub const fn current(self) -> u32 {
        self.current
    }

    pub const fn pending(self) -> i32 {
        self.pending
    }

    pub const fn previous(self) -> u32 {
        self.previous
    }

    /// Applies one controller request commit. Negative values skip all writes;
    /// nonnegative requests are retired to `-1`, even when equal to current.
    pub fn apply_pending_request(&mut self) -> ControllerTargetEffect {
        if self.pending < 0 {
            return ControllerTargetEffect::NegativePendingSkipped {
                pending: self.pending,
            };
        }

        let requested = self.pending as u32;
        self.pending = -1;
        if requested == self.current {
            return ControllerTargetEffect::EqualRequestRetired {
                target: self.current,
            };
        }

        let previous = self.current;
        self.previous = previous;
        self.current = requested;
        ControllerTargetEffect::TargetChanged {
            previous,
            current: requested,
        }
    }

    /// Applies the bounded write sequence seen in four selected-goal setter
    /// branches. This is intentionally not a general writer policy: other
    /// target writers remain unclassified. The measured branches install the
    /// target immediately and leave an equal one-shot marker for next entry.
    pub fn apply_measured_selected_goal(&mut self, target: u32) -> SelectedGoalEffect {
        if target == self.current {
            return SelectedGoalEffect::SelectedGoalAlreadyCurrent { target };
        }

        let previous = self.current;
        self.previous = previous;
        self.current = target;
        self.pending = target as i32;
        SelectedGoalEffect::SelectedGoalInstalled {
            previous,
            current: target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ControllerTargetEffect, ControllerTargetHandoff, SelectedGoalEffect};

    #[test]
    fn negative_pending_skips_without_mutation() {
        let mut target = ControllerTargetHandoff::new(0x120, -2, 0x80);

        assert_eq!(
            target.apply_pending_request(),
            ControllerTargetEffect::NegativePendingSkipped { pending: -2 }
        );
        assert_eq!(target, ControllerTargetHandoff::new(0x120, -2, 0x80));
    }

    #[test]
    fn equal_request_only_retires_pending() {
        let mut target = ControllerTargetHandoff::new(0x120, 0x120, 0x80);

        assert_eq!(
            target.apply_pending_request(),
            ControllerTargetEffect::EqualRequestRetired { target: 0x120 }
        );
        assert_eq!(target, ControllerTargetHandoff::new(0x120, -1, 0x80));
    }

    #[test]
    fn changed_request_moves_current_to_previous() {
        let mut target = ControllerTargetHandoff::new(0x120, 0x240, 0x80);

        assert_eq!(
            target.apply_pending_request(),
            ControllerTargetEffect::TargetChanged {
                previous: 0x120,
                current: 0x240,
            }
        );
        assert_eq!(target, ControllerTargetHandoff::new(0x240, -1, 0x120));
    }

    #[test]
    fn measured_selected_goal_sets_current_previous_and_equal_pending_marker() {
        let mut target = ControllerTargetHandoff::new(0x240, -1, 0x120);

        assert_eq!(
            target.apply_measured_selected_goal(0x360),
            SelectedGoalEffect::SelectedGoalInstalled {
                previous: 0x240,
                current: 0x360,
            }
        );
        assert_eq!(target, ControllerTargetHandoff::new(0x360, 0x360, 0x240));
        assert_eq!(
            target.apply_pending_request(),
            ControllerTargetEffect::EqualRequestRetired { target: 0x360 }
        );
        assert_eq!(target, ControllerTargetHandoff::new(0x360, -1, 0x240));
    }

    #[test]
    fn measured_selected_goal_guard_preserves_equal_target_state() {
        let mut target = ControllerTargetHandoff::new(0x240, -1, 0x120);

        assert_eq!(
            target.apply_measured_selected_goal(0x240),
            SelectedGoalEffect::SelectedGoalAlreadyCurrent { target: 0x240 }
        );
        assert_eq!(target, ControllerTargetHandoff::new(0x240, -1, 0x120));
    }
}
