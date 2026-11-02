// SPDX-License-Identifier: GPL-3.0-or-later
//! Event ABI over shared object identities; no wake queue or live scheduling.
//! Tier 1 FABRICATED C differential; Tier 2 remains debt.
use crate::wait::{CallFrame, GuestMemory, INVALID_PARAMETER};
use crate::{NOT_IMPLEMENTED, SUCCESS};
use tsfp_kernel_object::{GuestAddress, Handle, ObjectTable};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Returned(u32),
    Unregistered,
}
#[derive(Default, Debug)]
pub struct Events {
    /// Log call count only; adapter owns text and sink reentrancy.
    pub logs: u32,
}
impl Events {
    fn log(&mut self) {
        self.logs = self.logs.wrapping_add(1);
    }
    /// No table is cached: guest SignalState is read fresh, including address reuse.
    pub fn set_guest(&mut self, frame: Option<CallFrame>, memory: &mut impl GuestMemory) -> u32 {
        let Some([event, _increment, wait]) = frame.and_then(|f| f.args(memory)) else {
            self.log();
            return 0;
        };
        let Some(previous) = (if event == 0 {
            None
        } else {
            memory.read::<4>(event.wrapping_add(4))
        }) else {
            self.log();
            return 0;
        };
        if !memory.write(event.wrapping_add(4), 1u32.to_le_bytes()) {
            self.log();
            return 0;
        }
        if wait != 0 {
            self.log();
        }
        u32::from(u32::from_le_bytes(previous) != 0)
    }
    pub fn dispatch_handle(
        &mut self,
        ordinal: u32,
        frame: Option<tsfp_kernel_object::CallFrame>,
        memory: &mut impl tsfp_kernel_object::GuestMemory,
        objects: &mut ObjectTable,
        irql: u8,
    ) -> Outcome {
        if !matches!(ordinal, 189 | 225) {
            return Outcome::Unregistered;
        }
        let count = if ordinal == 189 { 4 } else { 2 };
        let mut args = [0; 4];
        for (index, arg) in args.iter_mut().enumerate().take(count) {
            let Some(value) = frame.and_then(|f| f.argument(memory, index)) else {
                if ordinal == 225 {
                    self.log();
                }
                return Outcome::Returned(INVALID_PARAMETER);
            };
            *arg = value;
        }
        let status = if ordinal == 225 {
            if args[1] != 0 {
                self.log();
                NOT_IMPLEMENTED
            } else {
                let status = objects.event_set(Handle(args[0]), true);
                if status != SUCCESS {
                    self.log();
                }
                status
            }
        } else if args[1] != 0 {
            // Event attributes read root/name/flags in that order, unlike mutant creation.
            let readable = [0u32, 4, 8].into_iter().all(|offset| {
                let address = args[1].checked_add(offset);
                address
                    .and_then(|a| memory.read_word(GuestAddress(a)))
                    .is_some()
            });
            self.log();
            if readable {
                NOT_IMPLEMENTED
            } else {
                INVALID_PARAMETER
            }
        } else if irql != 0 || args[2] > 1 || args[3] as u8 != 0 {
            self.log();
            NOT_IMPLEMENTED
        } else {
            let status = objects.create_event(args[2] == 0, GuestAddress(args[0]), memory);
            if status == tsfp_kernel_object::INSUFFICIENT_RESOURCES {
                self.log();
            }
            status
        };
        Outcome::Returned(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_kernel_object::{GuestMemory as Words, Kind, NoFileBodies, Owner};
    struct Memory {
        words: [u32; 32],
        writes: usize,
        fail: usize,
    }
    impl Words for Memory {
        fn read_word(&self, a: GuestAddress) -> Option<u32> {
            self.words.get(a.0 as usize / 4).copied()
        }
        fn write_word(&mut self, a: GuestAddress, v: u32) -> bool {
            self.writes += 1;
            if self.writes == self.fail {
                return false;
            }
            let Some(word) = self.words.get_mut(a.0 as usize / 4) else {
                return false;
            };
            *word = v;
            true
        }
        fn readable(&self, a: GuestAddress, n: u32) -> bool {
            u64::from(a.0) + u64::from(n) <= 128
        }
    }
    fn memory(fail: usize) -> Memory {
        Memory {
            words: [0; 32],
            writes: 0,
            fail,
        }
    }
    #[test]
    fn publication_rollback_and_saturation() {
        let mut table = ObjectTable::default();
        let mut mem = memory(2);
        mem.words[1] = 42;
        assert_eq!(
            table.create_event(false, GuestAddress(4), &mut mem),
            INVALID_PARAMETER
        );
        assert_eq!(
            (
                table.live_count(),
                table.fresh_count(),
                table.generations()[0]
            ),
            (0, 1, 1)
        );
        assert_eq!(mem.words[1], 42);
        mem.fail = usize::MAX;
        for _ in 0..256 {
            assert_ne!(table.create(Kind::Event, Owner(0)).0, 0);
        }
        assert_eq!(
            table.create_event(true, GuestAddress(4), &mut mem),
            tsfp_kernel_object::INSUFFICIENT_RESOURCES
        );
        assert_eq!(mem.words[1], 42);
    }
    #[test]
    fn shared_lifetime_and_auto_manual_waits() {
        let mut table = ObjectTable::default();
        let mut mem = memory(usize::MAX);
        for manual in [false, true] {
            assert_eq!(
                table.create_event(manual, GuestAddress(4), &mut mem),
                SUCCESS
            );
            let handle = Handle(mem.words[1]);
            assert_eq!(table.event_try_wait(handle), Ok(false));
            assert_eq!(table.event_set(handle, true), SUCCESS);
            assert_eq!(table.event_try_wait(handle), Ok(true));
            assert_eq!(table.event_try_wait(handle), Ok(manual));
            assert!(table.release(handle, &mut NoFileBodies));
            assert_eq!(
                table.event_set(handle, true),
                tsfp_kernel_object::INVALID_HANDLE
            );
        }
        let mutant = table.create(Kind::Mutant, Owner(0));
        assert_eq!(
            table.event_try_wait(mutant),
            Err(tsfp_kernel_object::TYPE_MISMATCH)
        );
    }
    #[test]
    fn absent_handlers_preserve_state() {
        let mut table = ObjectTable::default();
        let mut mem = memory(usize::MAX);
        let mut events = Events::default();
        for ordinal in [108, 123, 138, 186, 193, 205, 222] {
            assert_eq!(
                events.dispatch_handle(ordinal, None, &mut mem, &mut table, 0),
                Outcome::Unregistered
            );
        }
        assert_eq!((mem.writes, table.live_count(), events.logs), (0, 0, 0));
    }
}
