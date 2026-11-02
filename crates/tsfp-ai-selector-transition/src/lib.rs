#![no_std]

use tsfp_port_runtime::memory::GuestMemory;

pub trait SelectorMemory {
    fn read32(&mut self, address: u32) -> u32;
    fn write32(&mut self, address: u32, value: u32);
}

impl<T: GuestMemory> SelectorMemory for T {
    fn read32(&mut self, address: u32) -> u32 {
        GuestMemory::read32(self, address)
    }

    fn write32(&mut self, address: u32, value: u32) {
        GuestMemory::write32(self, address, value);
    }
}

/// Rust-owned snapshot of the actor's measured local-state selector fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingSelector {
    pub current: u32,
    pub previous: u32,
    pub pending: u32,
    pub elapsed_bits: u32,
}

/// Result of consuming one nonnegative selector request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectorCommit {
    pub changed: bool,
    pub current: u32,
    pub previous: u32,
    pub pending: u32,
    pub elapsed_bits: u32,
}

/// Apply the bounded 0x26EFC0 pending-selector contract.
///
/// A negative pending value is left untouched. A nonnegative value is always
/// consumed; if it differs from current, old current is saved and elapsed
/// progress resets before current changes.
pub fn commit_pending_selector(input: PendingSelector) -> Option<SelectorCommit> {
    if (input.pending as i32) < 0 {
        return None;
    }
    let changed = input.pending != input.current;
    Some(SelectorCommit {
        changed,
        current: if changed {
            input.pending
        } else {
            input.current
        },
        previous: if changed {
            input.current
        } else {
            input.previous
        },
        pending: u32::MAX,
        elapsed_bits: if changed { 0 } else { input.elapsed_bits },
    })
}

/// Import, commit, and write back the measured extension selector fields.
pub fn commit_guest_selector(memory: &mut impl SelectorMemory, extension: u32) {
    let address = |offset| extension.wrapping_add(offset);
    if let Some(state) = commit_pending_selector(PendingSelector {
        current: memory.read32(address(0x20)),
        previous: memory.read32(address(0x9d4)),
        pending: memory.read32(address(0x9d0)),
        elapsed_bits: memory.read32(address(0x9d8)),
    }) {
        if state.changed {
            memory.write32(address(0x9d8), state.elapsed_bits);
            memory.write32(address(0x9d4), state.previous);
            memory.write32(address(0x20), state.current);
        }
        memory.write32(address(0x9d0), state.pending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Memory {
        words: [u32; 4],
        writes: [(u32, u32); 4],
        write_count: usize,
    }

    impl SelectorMemory for Memory {
        fn read32(&mut self, address: u32) -> u32 {
            self.words[match address - 0x100 {
                0x20 => 0,
                0x9d4 => 1,
                0x9d0 => 2,
                0x9d8 => 3,
                _ => panic!("unexpected selector read"),
            }]
        }

        fn write32(&mut self, address: u32, value: u32) {
            let index = match address - 0x100 {
                0x20 => 0,
                0x9d4 => 1,
                0x9d0 => 2,
                0x9d8 => 3,
                _ => panic!("unexpected selector write"),
            };
            self.words[index] = value;
            self.writes[self.write_count] = (address - 0x100, value);
            self.write_count += 1;
        }
    }

    #[test]
    fn consumes_changed_selector_and_resets_progress() {
        assert_eq!(
            commit_pending_selector(PendingSelector {
                current: 2,
                previous: 1,
                pending: 7,
                elapsed_bits: 0x3f80_0000,
            }),
            Some(SelectorCommit {
                changed: true,
                current: 7,
                previous: 2,
                pending: u32::MAX,
                elapsed_bits: 0,
            })
        );
    }

    #[test]
    fn consumes_same_selector_without_resetting_progress() {
        assert_eq!(
            commit_pending_selector(PendingSelector {
                current: 4,
                previous: 3,
                pending: 4,
                elapsed_bits: 0x4000_0000,
            }),
            Some(SelectorCommit {
                changed: false,
                current: 4,
                previous: 3,
                pending: u32::MAX,
                elapsed_bits: 0x4000_0000,
            })
        );
    }

    #[test]
    fn negative_selector_is_not_consumed() {
        assert_eq!(
            commit_pending_selector(PendingSelector {
                current: 4,
                previous: 3,
                pending: u32::MAX,
                elapsed_bits: 0x4000_0000,
            }),
            None
        );
    }

    #[test]
    fn guest_adapter_preserves_measured_write_set_and_order() {
        let mut changed = Memory {
            words: [2, 1, 7, 0x3f80_0000],
            ..Memory::default()
        };
        commit_guest_selector(&mut changed, 0x100);
        assert_eq!(
            &changed.writes[..changed.write_count],
            &[(0x9d8, 0), (0x9d4, 2), (0x20, 7), (0x9d0, u32::MAX)]
        );

        let mut unchanged = Memory {
            words: [4, 3, 4, 0x4000_0000],
            ..Memory::default()
        };
        commit_guest_selector(&mut unchanged, 0x100);
        assert_eq!(
            &unchanged.writes[..unchanged.write_count],
            &[(0x9d0, u32::MAX)]
        );
    }
}
