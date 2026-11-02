// SPDX-License-Identifier: GPL-3.0-or-later
use std::io::{self, BufRead};
use tsfp_kernel_sync::*;
fn main() {
    let mut irql = Irql::default();
    let mut events = [
        Event {
            kind: EventKind::Notification,
            signaled: false,
        },
        Event {
            kind: EventKind::Synchronization,
            signaled: false,
        },
    ];
    let mut mutant = Mutant::default();
    let mut thread = Thread::new(ThreadId(7), true);
    let mut cs = CriticalSection::default();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let values: Vec<u32> = line
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let (op, a, b) = (values[0], values[1], values[2]);
        let (result, extra) = match op {
            0 => (irql.raise(a) as u32, irql.level as u32),
            1 => {
                irql.lower(a);
                (0, irql.level as u32)
            }
            2 | 3 if a < 2 => {
                let event = &mut events[a as usize];
                if op == 2 {
                    event.set();
                    (SUCCESS, u32::from(event.signaled))
                } else {
                    (SUCCESS, u32::from(event.try_wait()))
                }
            }
            2 | 3 => (
                if a == 2 {
                    TYPE_MISMATCH
                } else {
                    INVALID_HANDLE
                },
                0,
            ),
            4 | 5 if a == 2 => {
                if op == 4 {
                    let (s, v) = mutant.acquire(ThreadId(b));
                    (s, u32::from(v))
                } else {
                    let (s, v) = mutant.release(ThreadId(b));
                    (s, v.unwrap_or(0))
                }
            }
            4 | 5 => (if a < 2 { TYPE_MISMATCH } else { INVALID_HANDLE }, 0),
            6 => (stack_size(a), 0),
            7 | 8 => {
                if op == 7 {
                    assert!(cs.enter(ThreadId(1)));
                } else {
                    cs.leave(ThreadId(1));
                }
                (cs.recursion, cs.lock_count)
            }
            9 | 10 => {
                let mut event = DispatcherEvent {
                    object_type: a as u8,
                    signal_state: b,
                };
                let result = if op == 9 {
                    event.set()
                } else {
                    u32::from(event.wait() == KeWait::Unhonoured)
                };
                (result, event.signal_state)
            }
            11 => {
                let result = match thread.suspend() {
                    Ok(_) => SUCCESS,
                    Err(s) => s,
                };
                (result, thread.suspend_count)
            }
            12 => {
                thread.resume();
                (SUCCESS, thread.suspend_count)
            }
            13 => {
                thread.started = a != 0;
                (0, thread.suspend_count)
            }
            14 => {
                thread.finished = a != 0;
                (0, thread.suspend_count)
            }
            15 => match thread.suspend() {
                Ok(previous) => (SUCCESS, previous),
                Err(status) => (status, a),
            },
            16 => (SUCCESS, thread.resume().previous),
            17 | 18 => {
                let mut objects = SyncObjects::default();
                objects.insert(
                    Handle(1),
                    if op == 17 {
                        Object::Event(events[0].clone())
                    } else {
                        Object::Thread(thread.clone())
                    },
                );
                let timeout = match b {
                    0 => Timeout::Infinite,
                    1 => Timeout::Poll,
                    2 => Timeout::Relative(10),
                    _ => Timeout::Absolute(10),
                };
                let outcome = if b == 4 {
                    WaitOutcome::Status(0xc0000005)
                } else {
                    objects.wait(
                        Handle(1),
                        ThreadId(1),
                        irql.level,
                        a & 1 == 0,
                        a & 2 != 0,
                        timeout,
                    )
                };
                let result = match outcome {
                    WaitOutcome::Status(status) => status,
                    WaitOutcome::Refused(Refusal::UnsupportedScope) => 0xf0000000,
                    WaitOutcome::Refused(Refusal::TerminalHostFailure) => 0xf0000001,
                    WaitOutcome::Refused(Refusal::WouldBlock) => 0xf0000003,
                    WaitOutcome::PendingThread(_) => panic!("live infinite wait excluded"),
                };
                let extra = if let Some(Object::Event(event)) = objects.get(Handle(1)) {
                    events[0] = event.clone();
                    u32::from(event.signaled)
                } else {
                    thread.suspend_count
                };
                (result, extra)
            }
            _ => panic!("unknown operation"),
        };
        println!("{result} {extra}");
    }
}
