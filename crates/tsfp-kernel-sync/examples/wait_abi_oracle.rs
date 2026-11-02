// SPDX-License-Identifier: GPL-3.0-or-later
use std::io::{self, BufRead};
use tsfp_kernel_sync::{wait::*, *};
struct Memory([u8; 4096], bool);
impl GuestMemory for Memory {
    fn read<const N: usize>(&mut self, address: u32) -> Option<[u8; N]> {
        let offset = address.checked_sub(0x21000000)? as usize;
        self.0.get(offset..offset.checked_add(N)?)?.try_into().ok()
    }
    fn write<const N: usize>(&mut self, address: u32, bytes: [u8; N]) -> bool {
        if self.1 {
            return false;
        }
        let Some(offset) = address.checked_sub(0x21000000).map(|v| v as usize) else {
            return false;
        };
        let Some(target) = self.0.get_mut(offset..offset + N) else {
            return false;
        };
        target.copy_from_slice(&bytes);
        true
    }
}
fn main() {
    let mut memory = Memory([0; 4096], false);
    let mut objects = SyncObjects::default();
    // Repository generation-zero handles, with four-byte slot increments.
    let handles = [
        Handle(0xe10000),
        Handle(0xe10004),
        Handle(0xe10008),
        Handle(0xe1000c),
        Handle(0),
    ];
    objects.insert(handles[1], Object::Mutant(Mutant::default()));
    objects.insert(handles[3], Object::Semaphore); // unsupported nonthread kind with async off
    let mut diagnostics = KeDiagnostics::default();
    let mut irql = Irql::default();
    for line in io::stdin().lock().lines() {
        let values: Vec<u32> = line
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let [
            ordinal,
            kind,
            mode,
            alert,
            timeout_kind,
            frame_kind,
            signal,
            ty,
            caller,
            requested_irql,
            gates,
        ]: [u32; 11] = values.try_into().unwrap();
        let timeout: i64 = match timeout_kind {
            2 => -10,
            3 => 10,
            5 => i64::MIN,
            7 => -80000,
            8 => -160000,
            _ => 0,
        };
        let pointer = match timeout_kind {
            0 => 0,
            4 => 0x22000000,
            6 => 0x21000ffc,
            _ => 0x210000e0,
        };
        memory.1 = false;
        memory.write(0x21000000, signal.to_le_bytes());
        memory.write(0x210000e0, timeout.to_le_bytes());
        memory.write(0x21000200, ty.to_le_bytes());
        memory.write(0x21000204, signal.to_le_bytes());
        if gates & 8 != 0 {
            objects.insert(
                handles[1],
                Object::Mutant(Mutant {
                    owner: if signal == 0 {
                        None
                    } else {
                        Some(ThreadId(ty))
                    },
                    recursion: signal,
                }),
            );
        }
        objects.insert(
            handles[0],
            Object::Event(Event {
                kind: if ty & 1 == 0 {
                    EventKind::Notification
                } else {
                    EventKind::Synchronization
                },
                signaled: signal != 0,
            }),
        );
        let mut thread = Thread::new(ThreadId(handles[2].0), false);
        thread.started = ty & 1 != 0;
        thread.finished = ty & 2 != 0;
        thread.terminated = ty & 4 != 0;
        thread.suspend_count = u32::from(ty & 8 != 0);
        objects.insert(handles[2], Object::Thread(thread));
        let handle = if kind < 5 {
            handles[kind as usize].0
        } else {
            0xfffffffe
        };
        let object = match kind {
            0 => 0x21000200,
            1 => 0,
            2 => 0x22000000,
            4 => 0x20fffffc,
            _ => 0xfffffffc,
        };
        let args = if ordinal == 159 {
            [object, mode, alert, gates, pointer]
        } else if ordinal == 233 {
            [handle, alert, pointer, 0, 0]
        } else {
            [handle, mode, alert, pointer, 0]
        };
        memory.write(0x21000080, 0u32.to_le_bytes());
        for (index, value) in args.iter().enumerate() {
            memory.write(0x21000084 + index as u32 * 4, value.to_le_bytes());
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
                        + if ordinal == 159 {
                            24
                        } else if ordinal == 233 {
                            16
                        } else {
                            20
                        }
                }
                _ => 0x21000098,
            },
        };
        let frame = if frame_kind == 1 { None } else { Some(frame) };
        irql.lower(requested_irql);
        irql.raise(requested_irql);
        memory.1 = gates & 16 != 0;
        let before = diagnostics.logs;
        let result = if ordinal == 159 {
            diagnostics.wait_guest(frame, &mut memory)
        } else {
            let context = Context {
                caller: ThreadId(match caller {
                    0 => 0,
                    2 => handles[2].0,
                    _ => 123,
                }),
                irql: irql.level,
                resetting: gates & 1 != 0,
                active_waits_full: gates & 2 != 0,
                thread_binding_valid: gates & 4 == 0,
                current_started_unfinished: caller == 2 && ty & 1 != 0 && ty & 2 == 0,
            };
            match objects.wait_guest(ordinal, frame, &mut memory, context) {
                Outcome::Wait(WaitOutcome::Status(status)) => status,
                Outcome::Wait(WaitOutcome::Refused(reason)) => {
                    0xf0000000
                        + match reason {
                            Refusal::UnsupportedScope => 0,
                            Refusal::TerminalHostFailure => 1,
                            Refusal::WouldBlock => 3,
                        }
                }
                outcome => panic!("scheduling excluded from synthetic sequences: {outcome:?}"),
            }
        };
        let Some(Object::Event(event)) = objects.get(handles[0]) else {
            unreachable!()
        };
        let Some(Object::Mutant(mutant)) = objects.get(handles[1]) else {
            unreachable!()
        };
        print!(
            "{} {} {} {} {} {}",
            result,
            u32::from(event.signaled),
            mutant.recursion,
            mutant.owner.map_or(0, |v| v.0),
            diagnostics.unhonoured,
            if ordinal == 159 {
                diagnostics.logs.wrapping_sub(before)
            } else {
                0
            }
        );
        for (address, count) in [
            (0x21000080, 6),
            (0x210000e0, 2),
            (0x21000200, 4),
            (0x21000000, 1),
        ] {
            for index in 0..count {
                print!(
                    " {}",
                    u32::from_le_bytes(memory.read(address + index * 4).unwrap())
                );
            }
        }
        println!();
    }
}
