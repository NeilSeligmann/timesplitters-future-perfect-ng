//! T2049/T2207 guest adapter: stdcall argument slots of the timer, DPC and calendar
//! ordinals (97, 107, 113, 119, 128, 137, 149, 150, 228) decoded into the typed
//! `Timers` and `Calendar` models, with the guest memory writes of
//! `src/xbox/kernel_event.c` behind the `GuestMemory` trait.
//! Default off: nothing in the host calls this yet (see docs/t-contract-kernel-clock.md).
use crate::Calendar;
use crate::timers::{ObjectId, Timers};

pub const STATUS_SUCCESS: u32 = 0;
pub const STATUS_UNSUCCESSFUL: u32 = 0xC000_0001;
pub const STATUS_NOT_IMPLEMENTED: u32 = 0xC000_0002;
pub const STATUS_ACCESS_VIOLATION: u32 = 0xC000_0005;
pub const STATUS_INVALID_PARAMETER: u32 = 0xC000_000D;
/// KDPC Type at +0; the guest's teardown skips KeRemoveQueueDpc unless it reads this.
pub const DPC_TYPE_INITIALISED: u16 = 0x0013;

/// Guest memory as the adapter sees it. `false`/`None` means the address is unmapped.
pub trait GuestMemory {
    fn read_u32(&self, address: u32) -> Option<u32>;
    fn write_u32(&mut self, address: u32, value: u32) -> bool;
    fn write_u16(&mut self, address: u32, value: u16) -> bool;
}

/// Host inputs sampled per call: elapsed guest units and the host wall clock
/// (100 ns since 1601, `None` when the host clock cannot be read).
#[derive(Clone, Copy, Debug)]
pub struct HostTime {
    pub elapsed: u64,
    pub wall: Option<u64>,
}

/// One kernel call: ordinal plus its stdcall stack slots (low dword first for
/// LARGE_INTEGER by value).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    CancelTimer {
        timer: u32,
    },
    InitializeDpc {
        dpc: u32,
        routine: u32,
        context: u32,
    },
    InitializeTimerEx {
        timer: u32,
        kind: u32,
    },
    InsertQueueDpc {
        dpc: u32,
        arguments: [u32; 2],
    },
    QuerySystemTime {
        out: u32,
    },
    RemoveQueueDpc {
        dpc: u32,
    },
    SetTimer {
        timer: u32,
        due_low: u32,
        due_high: u32,
        period_ms: Option<i32>,
        dpc: u32,
    },
    SetSystemTime {
        new_time: u32,
        old_time: u32,
    },
}

impl Call {
    /// Decodes the stack slots of `ordinal`; `None` for other ordinals or too few slots.
    pub fn decode(ordinal: u32, slots: &[u32]) -> Option<Call> {
        let s = |i: usize| slots.get(i).copied();
        Some(match ordinal {
            97 => Call::CancelTimer { timer: s(0)? },
            107 => Call::InitializeDpc {
                dpc: s(0)?,
                routine: s(1)?,
                context: s(2)?,
            },
            113 => Call::InitializeTimerEx {
                timer: s(0)?,
                kind: s(1)?,
            },
            119 => Call::InsertQueueDpc {
                dpc: s(0)?,
                arguments: [s(1)?, s(2)?],
            },
            128 => Call::QuerySystemTime { out: s(0)? },
            137 => Call::RemoveQueueDpc { dpc: s(0)? },
            149 => Call::SetTimer {
                timer: s(0)?,
                due_low: s(1)?,
                due_high: s(2)?,
                period_ms: None,
                dpc: s(3)?,
            },
            150 => Call::SetTimer {
                timer: s(0)?,
                due_low: s(1)?,
                due_high: s(2)?,
                period_ms: Some(s(3)? as i32),
                dpc: s(4)?,
            },
            228 => Call::SetSystemTime {
                new_time: s(0)?,
                old_time: s(1)?,
            },
            _ => return None,
        })
    }
}

/// Typed kernel timer state owned by Rust; guest pointers are the object identities.
#[derive(Debug, Default)]
pub struct Adapter {
    pub timers: Timers,
    pub calendar: Calendar,
}

impl Adapter {
    /// Runs one call and returns EAX, matching the C handler's return value.
    pub fn call(&mut self, call: Call, time: HostTime, memory: &mut impl GuestMemory) -> u32 {
        match call {
            Call::CancelTimer { timer } => self.timers.cancel(ObjectId(timer)).into(),
            Call::InitializeTimerEx { timer, kind } => {
                self.timers.initialize_timer(ObjectId(timer), kind);
                0
            }
            Call::SetTimer {
                timer,
                due_low,
                due_high,
                period_ms,
                dpc,
            } => {
                let due = (u64::from(due_high) << 32 | u64::from(due_low)) as i64;
                // Absolute deadlines are measured against the guest calendar.
                let calendar = if due < 0 {
                    0
                } else {
                    self.calendar.query(time.wall)
                };
                let dpc = (dpc != 0).then_some(ObjectId(dpc));
                let previous = self.timers.set(
                    ObjectId(timer),
                    due,
                    period_ms.unwrap_or(0),
                    dpc,
                    time.elapsed,
                    calendar,
                );
                previous.into()
            }
            Call::InitializeDpc {
                dpc,
                routine,
                context,
            } => {
                self.timers.initialize_dpc(ObjectId(dpc), routine, context);
                // A failed stamp is not fatal for the call; the C host only logs it.
                let _ = memory.write_u16(dpc, DPC_TYPE_INITIALISED);
                0
            }
            Call::InsertQueueDpc { dpc, arguments } => {
                self.timers.enqueue(ObjectId(dpc), arguments).into()
            }
            Call::RemoveQueueDpc { dpc } => self.timers.remove(ObjectId(dpc)).into(),
            Call::QuerySystemTime { out } => {
                if out != 0 {
                    let now = self.calendar.query(time.wall);
                    // Two dword stores, low first, exactly like the C handler.
                    let _ = memory.write_u32(out, now as u32);
                    let _ = memory.write_u32(out.wrapping_add(4), (now >> 32) as u32);
                }
                0
            }
            Call::SetSystemTime { new_time, old_time } => {
                if old_time != 0 {
                    return STATUS_NOT_IMPLEMENTED;
                }
                let read =
                    |address: u32| (address != 0).then(|| memory.read_u32(address)).flatten();
                let (Some(low), Some(high)) = (read(new_time), read(new_time.wrapping_add(4)))
                else {
                    return STATUS_ACCESS_VIOLATION;
                };
                let value = u64::from(high) << 32 | u64::from(low);
                if value == 0 || value >> 63 != 0 {
                    return STATUS_INVALID_PARAMETER;
                }
                if self.calendar.set(value, time.wall) {
                    STATUS_SUCCESS
                } else {
                    STATUS_UNSUCCESSFUL
                }
            }
        }
    }
}
