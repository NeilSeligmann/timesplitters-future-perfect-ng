// SPDX-License-Identifier: GPL-3.0-or-later
use std::io::{self, BufRead};
use tsfp_kernel_object::{
    GuestAddress, GuestMemory as Words, Handle, Kind, ObjectState, ObjectTable, Owner,
};
use tsfp_kernel_sync::{
    event::{Events, Outcome},
    wait::{CallFrame, GuestMemory},
};
struct Memory([u8; 4096], bool);
impl GuestMemory for Memory {
    fn read<const N: usize>(&mut self, a: u32) -> Option<[u8; N]> {
        let offset = a.checked_sub(0x21000000)? as usize;
        self.0.get(offset..offset.checked_add(N)?)?.try_into().ok()
    }
    fn write<const N: usize>(&mut self, a: u32, bytes: [u8; N]) -> bool {
        if self.1 {
            return false;
        }
        let Some(offset) = a.checked_sub(0x21000000).map(|v| v as usize) else {
            return false;
        };
        let Some(target) = self.0.get_mut(offset..offset + N) else {
            return false;
        };
        target.copy_from_slice(&bytes);
        true
    }
}
impl Words for Memory {
    fn read_word(&self, a: GuestAddress) -> Option<u32> {
        let offset = a.0.checked_sub(0x21000000)? as usize;
        Some(u32::from_le_bytes(
            self.0.get(offset..offset + 4)?.try_into().ok()?,
        ))
    }
    fn write_word(&mut self, a: GuestAddress, v: u32) -> bool {
        self.write(a.0, v.to_le_bytes())
    }
    fn readable(&self, a: GuestAddress, n: u32) -> bool {
        a.0.checked_sub(0x21000000)
            .is_some_and(|o| u64::from(o) + u64::from(n) <= 4096)
    }
}
fn main() {
    let mut memory = Memory([0; 4096], false);
    for line in io::stdin().lock().lines() {
        let v: Vec<u32> = line
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let [
            ordinal,
            frame_kind,
            output_kind,
            attributes,
            ty,
            initial,
            irql,
            signal,
            wait,
            write_fail,
        ]: [u32; 10] = v.try_into().unwrap();
        let mut table = ObjectTable::default();
        let handle = table.create(Kind::Event, Owner(189));
        // Seed through real publication API, preserving fresh slot order and type.
        table.reset();
        memory.1 = false;
        memory.write(0x21000240, 0xabcdef01u32.to_le_bytes());
        assert_eq!(
            table.create_event(ty & 1 == 0, GuestAddress(0x21000240), &mut memory),
            0
        );
        table.event_set(handle, signal != 0);
        let other = table.create(Kind::Mutant, Owner(192));
        let output = match output_kind {
            0 => 0x21000240,
            1 => 0,
            2 => 0x22000000,
            _ => 0x21000084,
        };
        let attr = match attributes {
            0 => 0,
            1 => 0x21000280,
            2 => 0x22000000,
            _ => 0xfffffffc,
        };
        memory.write(0x21000240, 0xabcdef01u32.to_le_bytes());
        for (a, v) in [
            (0x21000280, 0u32),
            (0x21000284, 0),
            (0x21000288, 128),
            (0x21000204, signal),
        ] {
            memory.write(a, v.to_le_bytes());
        }
        let args = match ordinal {
            225 => [
                match output_kind {
                    0 => handle.0,
                    1 => 0,
                    2 => other.0,
                    _ => handle.0 + 1024,
                },
                wait,
                ty,
                initial,
            ],
            145 => [
                match output_kind {
                    0 => 0x21000200,
                    1 => 0,
                    2 => 0x22000000,
                    _ => 0xfffffffc,
                },
                initial,
                wait,
                initial,
            ],
            _ => [output, attr, ty, initial],
        };
        memory.write(0x21000080, 0u32.to_le_bytes());
        for (i, v) in args.into_iter().enumerate() {
            memory.write(0x21000084 + 4 * i as u32, v.to_le_bytes());
        }
        let frame = CallFrame {
            stack: match frame_kind {
                2 => 0,
                4 => 0x22000000,
                5 => 0xfffffffc,
                _ => 0x21000080,
            },
            limit: match frame_kind {
                3 => 0x21000088,
                5 | 6 => 0,
                7 => {
                    0x21000080
                        + match ordinal {
                            189 => 20,
                            225 => 12,
                            _ => 16,
                        }
                }
                _ => 0x21000094,
            },
        };
        let mut events = Events::default();
        memory.1 = write_fail != 0;
        let result = if ordinal == 900 || ordinal == 901 {
            let target = match output_kind {
                0 => handle,
                1 => Handle(0),
                2 => other,
                _ => Handle(handle.0 + 1024),
            };
            if ordinal == 900 {
                match table.event_try_wait(target) {
                    Ok(value) => u32::from(value),
                    Err(status) => status,
                }
            } else {
                table.event_set(target, false)
            }
        } else if ordinal == 145 {
            events.set_guest((frame_kind != 1).then_some(frame), &mut memory)
        } else {
            let frame = tsfp_kernel_object::CallFrame {
                stack: GuestAddress(frame.stack),
                limit: GuestAddress(frame.limit),
                ecx: None,
            };
            match events.dispatch_handle(
                ordinal,
                (frame_kind != 1).then_some(frame),
                &mut memory,
                &mut table,
                irql as u8,
            ) {
                Outcome::Returned(v) => v,
                Outcome::Unregistered => panic!(),
            }
        };
        let signaled = match table.find(handle).unwrap().state {
            ObjectState::Event { signaled, .. } => signaled as u32,
            _ => panic!(),
        };
        print!(
            "{} {} {} {} {}",
            result,
            events.logs,
            table.live_count(),
            table.fresh_count(),
            signaled
        );
        for i in 0..5 {
            print!(
                " {}",
                memory.read_word(GuestAddress(0x21000080 + 4 * i)).unwrap()
            );
        }
        for a in [0x21000204, 0x21000240] {
            print!(" {}", memory.read_word(GuestAddress(a)).unwrap());
        }
        let (ty, sig) = match table.find(Handle::encode(2, 0)).map(|e| &e.state) {
            Some(ObjectState::Event { manual, signaled }) => {
                (u32::from(!*manual), u32::from(*signaled))
            }
            _ => (99, 99),
        };
        println!(" {ty} {sig}");
    }
}
