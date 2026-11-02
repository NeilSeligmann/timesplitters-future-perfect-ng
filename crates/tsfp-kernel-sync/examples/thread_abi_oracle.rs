// SPDX-License-Identifier: GPL-3.0-or-later
//! FABRICATED seeded ABI runner, no scheduling and no host adapter.
use std::collections::BTreeMap;
use std::io::{self, BufRead};
use tsfp_kernel_object::{GuestAddress as Address, GuestMemory, Handle, Kind, ObjectTable, Owner};
use tsfp_kernel_sync::thread::*;
use tsfp_kernel_sync::{Thread, ThreadId};

struct Memory {
    words: BTreeMap<u32, u8>,
    ranges: Vec<(u32, u32, u8)>,
    readonly: bool,
    failed_write: u32,
}
impl Memory {
    fn byte(&self, address: u32) -> Option<u8> {
        if address == 0 {
            return None;
        }
        let fill = self
            .ranges
            .iter()
            .find(|(base, size, _)| {
                address >= *base && u64::from(address) < u64::from(*base) + u64::from(*size)
            })?
            .2;
        Some(*self.words.get(&address).unwrap_or(&fill))
    }
    fn word(&self, address: u32) -> u32 {
        self.read_word(Address(address)).unwrap_or(0)
    }
}
impl GuestMemory for Memory {
    fn read_word(&self, address: Address) -> Option<u32> {
        let mut bytes = [0; 4];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.byte(address.0.checked_add(i as u32)?)?;
        }
        Some(u32::from_le_bytes(bytes))
    }
    fn write_word(&mut self, address: Address, value: u32) -> bool {
        if address.0 == self.failed_write {
            return false;
        }
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            let Some(at) = address.0.checked_add(i as u32) else {
                return false;
            };
            if !self.write_byte(Address(at), byte) {
                return false;
            }
        }
        true
    }
    fn readable(&self, address: Address, length: u32) -> bool {
        (0..length).all(|i| {
            address
                .0
                .checked_add(i)
                .is_some_and(|at| self.byte(at).is_some())
        })
    }
}
impl ThreadMemory for Memory {
    fn write_byte(&mut self, address: Address, value: u8) -> bool {
        if self.byte(address.0).is_none()
            || (self.readonly && (0x21000000..0x21001000).contains(&address.0))
        {
            return false;
        }
        self.words.insert(address.0, value);
        true
    }
}
#[derive(Default)]
struct Provider {
    attempts: u32,
    live: u32,
    fail_at: u32,
}
impl Provider {
    fn alloc(&mut self, memory: &mut Memory, bytes: u32) -> Result<Address, u32> {
        self.attempts += 1;
        if self.attempts == self.fail_at || bytes > 0x900000 {
            return Err(NO_MEMORY);
        }
        let base = 0x30000000 + (self.attempts - 1) * 0x1000000;
        memory.ranges.push((base, bytes, 0));
        self.live += 1;
        Ok(Address(base))
    }
}
// The generic storage contract is exercised using this sparse-memory fixture.
// Production mapped allocation and page guards are explicitly absent.
struct Provisioned<'a> {
    result: Result<Storage, u32>,
    provider: &'a mut Provider,
}
impl ThreadStorage for Provisioned<'_> {
    fn provision(&mut self, _: CreateRequest, _: &mut impl GuestMemory) -> Result<Storage, u32> {
        self.result
    }
    fn release(&mut self, _: Storage) {
        self.provider.live = 0;
    }
}
fn provision(
    provider: &mut Provider,
    memory: &mut Memory,
    request: CreateRequest,
) -> Result<Storage, u32> {
    let result = (|| {
        let stack = provider.alloc(memory, stack_region_size(request.stack_bytes))?;
        let control = provider.alloc(memory, 0x1000)?;
        let bytes = tls_allocation_size(request.tls_bytes).ok_or(INVALID_PARAMETER)?;
        let tls = provider.alloc(memory, bytes)?;
        let monitor = provider.alloc(memory, 0x1000)?;
        let storage = Storage {
            stack_low: Address(stack.0 + 0x10000),
            stack_high: Address(
                stack.0 + 0x10000 + tsfp_kernel_sync::stack_size(request.stack_bytes),
            ),
            control,
            tls,
            monitor,
        };
        if !initialize_control(memory, storage, 0x1000, 0)
            || !publish_tls_end(memory, control, tls, request.tls_bytes)
        {
            return Err(INVALID_PARAMETER);
        }
        // Sparse equivalent of successful mprotect: remove both guard ranges.
        memory.ranges[2] = (
            storage.stack_low.0,
            storage.stack_high.0 - storage.stack_low.0,
            0,
        );
        Ok(storage)
    })();
    if result.is_err() {
        memory.ranges.truncate(2);
        memory.words.retain(|at, _| *at < 0x30000000);
        provider.live = 0;
    }
    result
}
fn main() {
    for line in io::stdin().lock().lines() {
        let input: Vec<u32> = line
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let [
            ordinal,
            frame_kind,
            target_kind,
            output_kind,
            value,
            state,
            count,
            gates,
            stack,
            tls,
            suspended,
            system,
        ]: [u32; 12] = input.try_into().unwrap();
        let mut memory = Memory {
            words: BTreeMap::new(),
            ranges: vec![(0x21000000, 4096, 0xa5), (0x24000000, 8192, 0xa5)],
            readonly: false,
            failed_write: if gates & 4096 != 0 {
                0x33000004
            } else if gates & 8192 != 0 {
                0x31000004
            } else {
                0
            },
        };
        let mut objects = ObjectTable::default();
        let mut threads = Threads {
            resetting: gates & 1 != 0,
            ..Threads::default()
        };
        let body = 0x24000800;
        let handle = if ordinal != 255 {
            objects.create(Kind::Thread, Owner(if target_kind == 5 { 2 } else { 0 }))
        } else {
            Handle(0)
        };
        let storage = Storage {
            stack_low: Address(0),
            stack_high: Address(0),
            control: Address(if target_kind == 7 {
                0x24000004
            } else {
                0x24000000
            }),
            tls: Address(0),
            monitor: Address(0),
        };
        let mut original = Record {
            handle,
            state: Thread::new(ThreadId(handle.0), false),
            request: CreateRequest::default(),
            storage,
            entry: 0,
            esp: Address(0),
            frame_ready: ordinal != 255 && state & 8 != 0,
            priority_set: false,
            termination_requested: false,
            exit_eax: 0,
        };
        original.state.suspend_count = count;
        original.state.started = state & 1 != 0;
        original.state.finished = state & 2 != 0;
        original.state.base_priority = 0x87654321u32 as i32;
        original.state.boost_disabled = state & 4 != 0;
        if ordinal != 255 {
            assert!(objects.bind_thread_body(
                handle,
                Owner(if target_kind == 5 { 2 } else { 0 }),
                Address(body),
                &memory
            ));
            objects.seed_reference_counts(handle, 1, if target_kind == 6 { 0 } else { 1 });
            threads.records.push(if gates & 2 == 0 {
                Some(original.clone())
            } else {
                None
            });
        }
        if state & 16 != 0 {
            threads.current = Some(CurrentThread {
                slot: 0,
                handle: if state & 32 != 0 { Handle(123) } else { handle },
            });
        }
        if ordinal != 255 && gates & 1024 != 0 {
            objects.release(handle, &mut tsfp_kernel_object::NoFileBodies);
        }
        let output = match output_kind {
            0 => 0,
            2 => 0x22000000,
            3 => 0x21000084,
            4 => {
                if ordinal == 255 {
                    0x31000800
                } else {
                    body
                }
            }
            _ => 0x21000100,
        };
        let target = match target_kind {
            1 | 5..=7 => body,
            2 => body + 4,
            3 => 123,
            4 if ordinal != 255 => objects.create(Kind::Event, Owner(0)).0,
            _ => handle.0,
        };
        let mut args = [0u32; 10];
        args[0] = target;
        args[1] = output;
        if ordinal == 255 {
            args = [
                output,
                0x55,
                stack,
                tls,
                match target_kind {
                    2 => 0x22000000,
                    3 => 0x31000800,
                    _ => 0x21000104,
                },
                0x12345678,
                value,
                suspended,
                0x77,
                system,
            ];
        } else if ordinal == 143 || ordinal == 144 {
            args[1] = value;
        } else if ordinal == 258 {
            args[0] = value;
        }
        memory.write_word(Address(0x21000080), 0);
        for (i, arg) in args.into_iter().enumerate() {
            memory.write_word(Address(0x21000084 + i as u32 * 4), arg);
        }
        let frame = if frame_kind == 1 {
            None
        } else {
            Some(Frame {
                stack: Address(match frame_kind {
                    2 => 0,
                    4 => 0x22000000,
                    5 => 0xfffffffc,
                    _ => 0x21000080,
                }),
                limit: Address(match frame_kind {
                    3 => 0x21000088,
                    5 | 6 => 0,
                    _ => 0x210000ac,
                }),
            })
        };
        memory.readonly = output_kind == 5;
        let mut provider = Provider {
            fail_at: (gates >> 5) & 7,
            ..Provider::default()
        };
        let outcome;
        if ordinal == 255 {
            // Honor handler's early gates before invoking the concrete storage fixture.
            let arg = |i| frame.and_then(|f| f.argument(&memory, i)).unwrap_or(0);
            let request = CreateRequest {
                stack_bytes: arg(2),
                tls_bytes: arg(3),
                start: arg(5),
                context: arg(6),
                system: arg(9),
                suspended: arg(7) != 0,
            };
            let result = if arg(0) != 0 && !threads.resetting {
                provision(&mut provider, &mut memory, request)
            } else {
                Err(NO_MEMORY)
            };
            outcome = threads.create(
                frame,
                &mut memory,
                &mut objects,
                &mut Provisioned {
                    result,
                    provider: &mut provider,
                },
            );
            if provider.live == 0 {
                memory.ranges.truncate(2);
                memory.words.retain(|at, _| *at < 0x30000000);
            }
        } else {
            outcome = threads.call(
                ordinal,
                frame,
                &mut memory,
                &objects,
                if gates & 16 != 0 { 2 } else { 0 },
                ordinal == 258 && gates & 4 == 0,
            );
        }
        let outcome =
            if matches!(outcome, Outcome::Pending(Action::Terminate { .. })) && gates & 8 != 0 {
                threads.termination_hook_returned(threads.current.unwrap())
            } else {
                outcome
            };
        let result = match outcome {
            Outcome::Returned(v) => v,
            Outcome::Pending(_) => 0xf0000001,
            Outcome::Unregistered => 0,
        };
        let record = threads.records.first().and_then(Option::as_ref);
        let r = record.unwrap_or(&original);
        let mut fields = vec![
            result,
            u32::from(record.is_some()),
            if ordinal == 255 && record.is_none() {
                0
            } else {
                r.handle.0
            },
            if ordinal == 255 && record.is_none() {
                0
            } else {
                r.state.suspend_count
            },
            if ordinal == 255 && record.is_none() {
                0
            } else {
                u32::from(r.state.started)
            },
            if ordinal == 255 && record.is_none() {
                0
            } else {
                u32::from(r.state.finished)
            },
            if ordinal == 255 && record.is_none() {
                0
            } else {
                r.state.base_priority as u32
            },
            u32::from(r.priority_set),
            if ordinal == 255 && record.is_none() {
                0
            } else {
                u32::from(r.state.boost_disabled)
            },
            u32::from(r.termination_requested),
            r.exit_eax,
            r.entry,
            r.esp.0,
            threads.unhonoured_suspends,
            threads.ignored_priorities,
            if ordinal == 255 {
                threads.records.len() as u32
            } else {
                0
            },
            threads.current_identity().0,
            provider.live,
            provider.attempts,
            threads.diagnostics.len() as u32,
        ];
        fields.extend([
            objects.live_count() as u32,
            objects.fresh_count() as u32,
            objects.generations()[0] as u32,
            objects.generations()[1] as u32,
            0,
        ]);
        fields.extend([
            r.request.start,
            r.request.context,
            r.request.system,
            r.request.stack_bytes,
            r.request.tls_bytes,
            u32::from(r.request.suspended),
            u32::from(r.frame_ready),
        ]);
        for i in 0..11 {
            fields.push(memory.word(0x21000080 + 4 * i));
        }
        fields.extend([memory.word(0x21000100), memory.word(0x21000104)]);
        let control = if ordinal == 255 {
            0x31000000
        } else {
            0x24000000
        };
        for offset in [0, 4, 0x20, 0x24, 0x28, 0x278, 0x804, 0x828, 0x920] {
            fields.push(memory.word(control + offset));
        }
        if ordinal == 255 {
            for i in 0..10 {
                fields.push(memory.word(0x33000000 + 4 * i));
            }
            for i in 0..3 {
                fields.push(memory.word(r.esp.0 + 4 * i));
            }
        }
        println!(
            "{}",
            fields
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}
