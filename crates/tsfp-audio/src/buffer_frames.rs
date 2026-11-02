//! Typed guest-call boundary for the recovered DirectSound buffer entries.
//! The caller table is an adapter admission rule; the original API does not inspect return addresses.
use crate::adapter::{GuestMemory, Refusal};
use crate::device_frames::CallFrame;
use crate::lifecycle::{Device, GuestWorld, Objects};
use crate::scope::buffer_scope_snapshot;
use crate::setters::{BufferHooks, Setters};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferEntry {
    Create,
    I3dl2,
    MinDistance,
    Rolloff,
    SetData,
    SetVolume,
    Pause,
    SetFrequency,
    MaxDistance,
    SetPosition,
    SetLoopRegion,
}
impl BufferEntry {
    pub fn from_address(address: u32) -> Option<Self> {
        Some(match address {
            0x4093c8 => Self::Create,
            0x4085af => Self::I3dl2,
            0x408532 => Self::MinDistance,
            0x40858b => Self::Rolloff,
            0x408c0d => Self::SetData,
            0x407a64 => Self::SetVolume,
            0x407abc => Self::Pause,
            0x4084f2 => Self::SetFrequency,
            0x40850e => Self::MaxDistance,
            0x408556 => Self::SetPosition,
            0x407ad8 => Self::SetLoopRegion,
            _ => return None,
        })
    }
    fn argc(self) -> usize {
        match self {
            Self::Create | Self::Rolloff => 4,
            Self::SetPosition => 5,
            Self::I3dl2
            | Self::MinDistance
            | Self::SetData
            | Self::MaxDistance
            | Self::SetLoopRegion => 3,
            _ => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferCall {
    pub entry: BufferEntry,
    pub caller: u32,
    pub args: [u32; 5],
}

/// Decode the stdcall stack and enforce the recovered C adapter's caller and value scope.
/// `completion` widens admission for methods whose state is checked by the completion model.
pub fn decode(
    memory: &impl GuestMemory,
    entry: BufferEntry,
    frame: Option<CallFrame>,
    completion: bool,
) -> Result<BufferCall, Refusal> {
    let frame = frame.ok_or(Refusal("unreadable buffer frame"))?;
    let caller = memory
        .read_u32(frame.stack_pointer)
        .ok_or(Refusal("unreadable return address"))?;
    let free = completion
        && matches!(
            entry,
            BufferEntry::SetData
                | BufferEntry::SetVolume
                | BufferEntry::Pause
                | BufferEntry::SetFrequency
                | BufferEntry::SetLoopRegion
        );
    let admitted = match entry {
        BufferEntry::Create => matches!(caller, 0x27aa9 | 0x27b54),
        BufferEntry::I3dl2 => caller == 0x27ae8,
        BufferEntry::MinDistance => matches!(caller, 0x27af6 | 0x28390),
        BufferEntry::Rolloff => matches!(caller, 0x27b06 | 0x283ae),
        BufferEntry::SetData => caller == 0x282c0,
        BufferEntry::SetVolume => matches!(caller, 0x28348 | 0x28643),
        BufferEntry::Pause => caller == 0x2751e,
        BufferEntry::SetFrequency => caller == 0x27547,
        BufferEntry::MaxDistance => caller == 0x280b5,
        BufferEntry::SetPosition => matches!(caller, 0x26ef6 | 0x27c79 | 0x27d28),
        BufferEntry::SetLoopRegion => caller == 0,
    };
    if !admitted && !free {
        return Err(Refusal("only measured buffer caller is supported"));
    }
    let mut args = [0; 5];
    for (index, word) in args.iter_mut().enumerate().take(entry.argc()) {
        let slot = u64::from(frame.stack_pointer) + 4 + 4 * index as u64;
        if frame.stack_pointer == 0
            || slot + 4 > 1u64 << 32
            || (frame.stack_limit != 0 && slot + 4 > u64::from(frame.stack_limit))
        {
            return Err(Refusal("buffer argument outside stack boundary"));
        }
        *word = memory
            .read_u32(slot as u32)
            .ok_or(Refusal("unreadable buffer argument"))?;
    }
    if entry == BufferEntry::SetVolume && !completion {
        let expected = if caller == 0x28643 {
            0xfffff37c
        } else {
            0xffffd8f0
        };
        if args[1] != expected {
            return Err(Refusal("caller/volume pair mismatch"));
        }
    }
    if matches!(entry, BufferEntry::MinDistance | BufferEntry::Rolloff) {
        let start = matches!(caller, 0x28390 | 0x283ae);
        let apply = args[if entry == BufferEntry::MinDistance {
            2
        } else {
            3
        }];
        if start != (apply == 1) {
            return Err(Refusal("caller/apply pair mismatch"));
        }
    }
    Ok(BufferCall {
        entry,
        caller,
        args,
    })
}

impl BufferCall {
    /// Apply a decoded call to Rust-owned buffer state. Refusals stop dispatch.
    pub fn apply(
        self,
        setters: &mut Setters,
        objects: &mut Objects,
        device: &mut Device,
        world: &mut impl GuestWorld,
        hooks: &mut dyn BufferHooks,
    ) -> Result<u32, Refusal> {
        let a = self.args;
        match self.entry {
            BufferEntry::Create => {
                let scope = buffer_scope_snapshot(&*world, a[1])
                    .ok_or(Refusal("unreadable buffer descriptor"))?;
                if scope.flags != if self.caller == 0x27aa9 { 16 } else { 0 } {
                    return Err(Refusal("caller/descriptor class mismatch"));
                }
                setters.create_buffer(objects, device, world, a[0], a[1], a[2], a[3])
            }
            BufferEntry::I3dl2 => setters.buffer_i3dl2(objects, device, &*world, a[0], a[1], a[2]),
            BufferEntry::MinDistance => {
                setters.buffer_min_distance(objects, device, &*world, a[0], a[1], a[2])
            }
            BufferEntry::Rolloff => {
                setters.buffer_rolloff(objects, device, &*world, a[0], a[1], a[2], a[3])
            }
            BufferEntry::SetData => {
                setters.buffer_set_data(objects, device, &*world, a[0], a[1], a[2])
            }
            BufferEntry::SetVolume => {
                setters.buffer_volume(objects, device, &*world, a[0], a[1], hooks)
            }
            BufferEntry::Pause => setters.buffer_pause(objects, device, &*world, a[0], a[1], hooks),
            BufferEntry::SetFrequency => {
                setters.buffer_frequency(objects, device, &*world, a[0], a[1], hooks)
            }
            BufferEntry::MaxDistance => {
                setters.buffer_max_distance(objects, device, &*world, a[0], a[1], a[2])
            }
            BufferEntry::SetPosition => {
                setters.buffer_position(objects, device, &*world, a[0], [a[1], a[2], a[3]], a[4])
            }
            BufferEntry::SetLoopRegion => {
                setters.buffer_loop_region(objects, device, &*world, a[0], a[1], a[2], hooks)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct Words(BTreeMap<u32, u32>);
    impl GuestMemory for Words {
        fn read_bytes(&self, address: u32, length: usize) -> Option<Vec<u8>> {
            if length != 4 {
                return None;
            }
            Some(self.0.get(&address)?.to_le_bytes().to_vec())
        }
        fn read_u32(&self, address: u32) -> Option<u32> {
            self.0.get(&address).copied()
        }
        fn write_u32(&mut self, _: u32, _: u32) -> bool {
            false
        }
        fn range_readable(&self, _: u32, _: u32) -> bool {
            false
        }
    }
    fn frame(caller: u32, args: &[u32]) -> (Words, CallFrame) {
        let mut words = BTreeMap::from([(0x1000, caller)]);
        for (index, &arg) in args.iter().enumerate() {
            words.insert(0x1004 + 4 * index as u32, arg);
        }
        (
            Words(words),
            CallFrame {
                stack_pointer: 0x1000,
                stack_limit: 0x1020,
            },
        )
    }
    #[test]
    fn caller_and_value_scope() {
        let (words, stack) = frame(0x28348, &[7, 0xffffd8f0]);
        assert!(decode(&words, BufferEntry::SetVolume, Some(stack), false).is_ok());
        let (words, stack) = frame(0x28348, &[7, 0xfffff37c]);
        assert!(decode(&words, BufferEntry::SetVolume, Some(stack), false).is_err());
        assert!(decode(&words, BufferEntry::SetVolume, Some(stack), true).is_ok());
        let (words, stack) = frame(0x28390, &[7, 0x3f800000, 0]);
        assert!(decode(&words, BufferEntry::MinDistance, Some(stack), true).is_err());
    }
    #[test]
    fn argument_boundary_and_position_admission() {
        let (words, stack) = frame(0x27c79, &[1, 2, 3, 4, 1]);
        assert!(decode(&words, BufferEntry::SetPosition, Some(stack), false).is_ok());
        assert!(
            decode(
                &words,
                BufferEntry::SetPosition,
                Some(CallFrame {
                    stack_limit: 0x1010,
                    ..stack
                }),
                false
            )
            .is_err()
        );
        let (words, stack) = frame(0x12345, &[1, 2, 3, 4, 1]);
        assert!(decode(&words, BufferEntry::SetPosition, Some(stack), true).is_err());
    }
}
