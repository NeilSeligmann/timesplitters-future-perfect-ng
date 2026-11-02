// SPDX-License-Identifier: GPL-3.0-or-later
//! The legacy single synthetic pad (port 0): open/removed lifecycle, packet counter, pending
//! feedback and the last rumble report. Contracts measured by tests/test_xinput_pad_oracle.py.

pub const GAMEPAD_TYPE: u32 = 0x0046_C75C;
/// Fabricated opaque token for the one open pad; only its non-zero identity matters.
pub const PAD_HANDLE: u32 = 0x5850_4430;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SinglePad {
    open: bool,
    removed: bool,
    packet: u32,
    report_base: u32,
    remove_after_polls: u32,
    polls: u32,
    pending_feedback: u32,
    output: [u8; 6],
    output_count: u64,
}

impl SinglePad {
    /// Fresh session. `remove_after_polls` is host policy and survives.
    pub fn reset(&mut self) {
        *self = Self {
            remove_after_polls: self.remove_after_polls,
            ..Self::default()
        };
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_removed(&self) -> bool {
        self.removed
    }

    pub fn packet(&self) -> u32 {
        self.packet
    }

    pub fn pending_feedback(&self) -> u32 {
        self.pending_feedback
    }

    pub fn set_remove_after_polls(&mut self, polls: u32) {
        self.remove_after_polls = polls;
    }

    /// Open succeeds only for the gamepad table, port 0, a slot other than 1, once, while present.
    pub fn open_allowed(&self, device_type: u32, port: u32, slot: u32) -> bool {
        device_type == GAMEPAD_TYPE && port == 0 && slot != 1 && !self.open && !self.removed
    }

    pub fn open(&mut self, report_changes: u32) {
        self.open = true;
        self.report_base = report_changes;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.pending_feedback = 0;
    }

    pub fn remove_refusal(&self) -> Option<&'static str> {
        if self.removed {
            Some("removing an already removed pad")
        } else if self.pending_feedback != 0 {
            Some("removal with a feedback transfer pending is not recovered")
        } else {
            None
        }
    }

    pub fn insert_refusal(&self) -> Option<&'static str> {
        if !self.removed {
            Some("inserting a pad that is not removed")
        } else if self.open {
            Some("reinsertion with the old handle still open is not recovered")
        } else {
            None
        }
    }

    pub fn mark_removed(&mut self) {
        self.removed = true;
    }

    pub fn mark_inserted(&mut self) {
        self.removed = false;
    }

    /// Packet number follows the raw report: 1 plus the changes since the Open.
    pub fn sample(&mut self, report_changes: u32) -> u32 {
        self.packet = 1u32
            .wrapping_add(report_changes)
            .wrapping_sub(self.report_base);
        self.packet
    }

    /// Address of a pending feedback block to complete (0 when none); clears it.
    pub fn take_pending(&mut self) -> u32 {
        std::mem::take(&mut self.pending_feedback)
    }

    pub fn set_pending(&mut self, address: u32) {
        self.pending_feedback = address;
    }

    /// Record the rumble report (id 0, length 6, left then right little endian).
    pub fn record_output(&mut self, motors: [u8; 4]) {
        self.output = [0, 6, motors[0], motors[1], motors[2], motors[3]];
        self.output_count += 1;
    }

    pub fn last_output(&self) -> Option<[u8; 6]> {
        (self.output_count != 0).then_some(self.output)
    }

    pub fn output_count(&self) -> u64 {
        self.output_count
    }

    /// Count a completed GetState of a present pad; true when the fabricated removal source fires.
    pub fn poll_fires_removal(&mut self) -> bool {
        if self.removed || self.remove_after_polls == 0 {
            return false;
        }
        self.polls += 1;
        self.polls == self.remove_after_polls
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_matches_measured_contract() {
        let mut pad = SinglePad::default();
        assert!(!pad.open_allowed(GAMEPAD_TYPE, 1, 0));
        assert!(!pad.open_allowed(GAMEPAD_TYPE, 0, 1));
        assert!(pad.open_allowed(GAMEPAD_TYPE, 0, 2));
        pad.open(5);
        assert!(!pad.open_allowed(GAMEPAD_TYPE, 0, 0));
        assert_eq!(pad.sample(5), 1);
        assert_eq!(pad.sample(8), 4);
        pad.set_pending(0x100);
        assert!(pad.remove_refusal().is_some());
        assert_eq!(pad.take_pending(), 0x100);
        assert_eq!(pad.take_pending(), 0);
        pad.mark_removed();
        assert!(pad.remove_refusal().is_some() && pad.insert_refusal().is_some());
        pad.close();
        assert!(pad.insert_refusal().is_none());
        assert!(!pad.open_allowed(GAMEPAD_TYPE, 0, 0));
        pad.mark_inserted();
        assert!(pad.open_allowed(GAMEPAD_TYPE, 0, 0));
    }

    #[test]
    fn removal_source_and_output() {
        let mut pad = SinglePad::default();
        pad.set_remove_after_polls(2);
        assert!(!pad.poll_fires_removal());
        assert!(pad.poll_fires_removal());
        assert_eq!(pad.last_output(), None);
        pad.record_output([1, 2, 3, 4]);
        assert_eq!(pad.last_output(), Some([0, 6, 1, 2, 3, 4]));
        pad.reset();
        assert_eq!(pad.output_count(), 0);
        // the removal source is host policy and survives a reset
        assert!(!pad.poll_fires_removal());
        assert!(pad.poll_fires_removal());
    }
}
