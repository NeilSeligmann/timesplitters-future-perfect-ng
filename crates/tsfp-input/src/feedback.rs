// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline model of the synthetic pad's pending feedback completion boundary.

const IO_PENDING: u32 = 997;
const DEVICE_NOT_CONNECTED: u32 = 0x48f;

/// Guest status access is supplied by the eventual device adapter.
pub trait FeedbackMemory {
    fn read_status(&mut self, address: u32) -> Option<u32>;
    fn write_status(&mut self, address: u32, status: u32) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompletionError {
    Unreadable,
    Unwritable,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SubmissionError {
    NullAddress,
    NotificationEvent,
    Completion(CompletionError),
    Unwritable,
}

/// Outcome of submitting a validated guest feedback block.
#[derive(Debug, PartialEq, Eq)]
pub struct Submission {
    pub status: u32,
    pub report: Option<[u8; 6]>,
}

#[derive(Default)]
pub struct PendingFeedback(Option<u32>);

impl PendingFeedback {
    /// Register a validated, nonzero guest feedback address after setting its status pending.
    pub fn set(&mut self, address: u32) {
        self.0 = (address != 0).then_some(address);
    }

    /// Closing or resetting drops the address without touching the old guest memory.
    pub fn clear(&mut self) {
        self.0 = None;
    }

    pub fn is_pending(&self) -> bool {
        self.0.is_some()
    }

    /// Model C's single synthetic-pad submission after handle and block validation.
    /// The adapter must verify that the complete 0x46-byte block is mapped first.
    pub fn submit(
        &mut self,
        memory: &mut impl FeedbackMemory,
        address: u32,
        event: u32,
        removed: bool,
        left: u16,
        right: u16,
    ) -> Result<Submission, SubmissionError> {
        if address == 0 {
            return Err(SubmissionError::NullAddress);
        }
        if event != 0 {
            return Err(SubmissionError::NotificationEvent);
        }
        if removed {
            if !memory.write_status(address, DEVICE_NOT_CONNECTED) {
                return Err(SubmissionError::Unwritable);
            }
            return Ok(Submission {
                status: DEVICE_NOT_CONNECTED,
                report: None,
            });
        }
        self.complete(memory).map_err(SubmissionError::Completion)?;
        let report = crate::pad_report::encode_feedback(left, right);
        if !memory.write_status(address, IO_PENDING) {
            return Err(SubmissionError::Unwritable);
        }
        self.set(address);
        Ok(Submission {
            status: IO_PENDING,
            report: Some(report),
        })
    }

    /// Match C: only replace IO_PENDING; retain the address if guest access fails.
    pub fn complete(&mut self, memory: &mut impl FeedbackMemory) -> Result<(), CompletionError> {
        if let Some(address) = self.0 {
            let status = memory
                .read_status(address)
                .ok_or(CompletionError::Unreadable)?;
            if status == IO_PENDING && !memory.write_status(address, 0) {
                return Err(CompletionError::Unwritable);
            }
            self.0 = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Blocks(std::collections::BTreeMap<u32, u32>);
    impl FeedbackMemory for Blocks {
        fn read_status(&mut self, address: u32) -> Option<u32> {
            self.0.get(&address).copied()
        }
        fn write_status(&mut self, address: u32, status: u32) -> bool {
            if let Some(value) = self.0.get_mut(&address) {
                *value = status;
                true
            } else {
                false
            }
        }
    }

    #[test]
    fn submission_completes_previous_and_removed_preserves_pending() {
        let mut memory = Blocks::default();
        memory.0.insert(1, 0);
        memory.0.insert(2, 0);
        let mut pending = PendingFeedback::default();
        let first = pending
            .submit(&mut memory, 1, 0, false, 0x1234, 0xabcd)
            .unwrap();
        assert_eq!(first.report, Some([0, 6, 0x34, 0x12, 0xcd, 0xab]));
        assert_eq!(memory.0[&1], IO_PENDING);
        assert_eq!(
            pending.submit(&mut memory, 2, 1, false, 0, 0),
            Err(SubmissionError::NotificationEvent)
        );
        assert_eq!(memory.0[&1], IO_PENDING);
        let removed = pending.submit(&mut memory, 2, 0, true, 0, 0).unwrap();
        assert_eq!(removed.status, DEVICE_NOT_CONNECTED);
        assert_eq!(removed.report, None);
        assert!(pending.is_pending());
        pending.submit(&mut memory, 2, 0, false, 0, 0).unwrap();
        assert_eq!(memory.0[&1], 0);
        assert_eq!(memory.0[&2], IO_PENDING);
        pending.submit(&mut memory, 2, 0, false, 0, 0).unwrap();
        assert_eq!(memory.0[&2], IO_PENDING);
    }

    struct Memory {
        status: Option<u32>,
        writable: bool,
        writes: usize,
    }
    impl FeedbackMemory for Memory {
        fn read_status(&mut self, _: u32) -> Option<u32> {
            self.status
        }
        fn write_status(&mut self, _: u32, status: u32) -> bool {
            if !self.writable {
                return false;
            }
            self.status = Some(status);
            self.writes += 1;
            true
        }
    }

    #[test]
    fn completion_preserves_changed_status_and_retries_access_failures() {
        let mut pending = PendingFeedback::default();
        let mut memory = Memory {
            status: None,
            writable: false,
            writes: 0,
        };
        pending.set(0x1000);
        assert_eq!(
            pending.complete(&mut memory),
            Err(CompletionError::Unreadable)
        );
        assert!(pending.is_pending());
        memory.status = Some(IO_PENDING);
        assert_eq!(
            pending.complete(&mut memory),
            Err(CompletionError::Unwritable)
        );
        assert!(pending.is_pending());
        memory.writable = true;
        pending.complete(&mut memory).unwrap();
        assert_eq!(memory.status, Some(0));
        assert!(!pending.is_pending());
        pending.set(0x1000);
        memory.status = Some(42);
        pending.complete(&mut memory).unwrap();
        assert_eq!(memory.status, Some(42));
        assert_eq!(memory.writes, 1);
        pending.set(0x1000);
        pending.clear();
        memory.status = None;
        pending.complete(&mut memory).unwrap();
    }
}
