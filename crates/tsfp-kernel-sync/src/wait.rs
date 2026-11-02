// SPDX-License-Identifier: GPL-3.0-or-later
//! Raw stdcall wait boundary. Repository C semantics, no scheduler or host wiring.
//! Verification: Tier 1 FABRICATED guest-ABI sequences for nonscheduling paths;
//! HostPause and PendingThread requests are Tier 0 only. Tier 2 remains debt.
use crate::{Handle, Object, Refusal, SUCCESS, SyncObjects, ThreadId, Timeout, WaitOutcome};

pub const INVALID_PARAMETER: u32 = 0xc000000d;
pub const ACCESS_VIOLATION: u32 = 0xc0000005;

/// Exact byte ranges matter: Type is one byte, timeout is one atomic eight-byte read.
/// The adapter must stabilize mappings across accesses; failed writes are observable.
pub trait GuestMemory {
    fn read<const N: usize>(&mut self, address: u32) -> Option<[u8; N]>;
    fn write<const N: usize>(&mut self, address: u32, bytes: [u8; N]) -> bool;
}
#[derive(Clone, Copy, Debug)]
pub struct CallFrame {
    pub stack: u32,
    pub limit: u32,
}
impl CallFrame {
    pub(crate) fn args<const N: usize>(self, memory: &mut impl GuestMemory) -> Option<[u32; N]> {
        if self.stack == 0 {
            return None;
        }
        let mut args = [0; N];
        for (index, value) in args.iter_mut().enumerate() {
            let slot = u64::from(self.stack) + 4 + index as u64 * 4;
            let end = slot + 4;
            if end > 0x100000000 || (self.limit != 0 && end > u64::from(self.limit)) {
                return None;
            }
            *value = u32::from_le_bytes(memory.read(slot as u32)?);
        }
        Some(args)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A decoded wait request. Ordinal 233 supplies three words and forces mode 1;
/// ordinal 234 supplies four words including the mode.
///
/// ```
/// use tsfp_kernel_sync::{Handle, Timeout, wait::{CallFrame, GuestMemory, NtWait}};
/// struct Words;
/// impl GuestMemory for Words {
///     fn read<const N: usize>(&mut self, address: u32) -> Option<[u8; N]> {
///         let word = match address { 0x1004 => 7u32, 0x1008 | 0x100c => 0, _ => return None };
///         word.to_le_bytes().as_slice().try_into().ok()
///     }
///     fn write<const N: usize>(&mut self, _: u32, _: [u8; N]) -> bool { false }
/// }
/// let frame = CallFrame { stack: 0x1000, limit: 0x1010 };
/// let request = NtWait::decode(233, Some(frame), &mut Words).unwrap();
/// assert_eq!(request.handle, Handle(7));
/// assert_eq!(request.timeout, Timeout::Infinite);
/// ```
pub struct NtWait {
    pub handle: Handle,
    pub mode: u32,
    pub alertable: bool,
    pub timeout: Timeout,
}
impl NtWait {
    pub fn decode(
        ordinal: u32,
        frame: Option<CallFrame>,
        memory: &mut impl GuestMemory,
    ) -> Result<Self, u32> {
        let frame = frame.ok_or(INVALID_PARAMETER)?;
        let [handle, mode, alertable, pointer] = match ordinal {
            233 => {
                let [handle, alertable, pointer] = frame.args(memory).ok_or(INVALID_PARAMETER)?;
                [handle, 1, alertable, pointer]
            }
            234 => frame.args(memory).ok_or(INVALID_PARAMETER)?,
            _ => return Err(INVALID_PARAMETER),
        };
        let timeout = if pointer == 0 {
            Timeout::Infinite
        } else {
            let value = i64::from_le_bytes(memory.read(pointer).ok_or(ACCESS_VIOLATION)?);
            match value {
                0 => Timeout::Poll,
                value if value < 0 => Timeout::Relative(value.unsigned_abs()),
                value => Timeout::Absolute(value),
            }
        };
        Ok(Self {
            handle: Handle(handle),
            mode,
            alertable: alertable as u8 != 0,
            timeout,
        })
    }
}
/// Supplied by the object/thread adapter, never inferred from a guest address.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub caller: ThreadId,
    pub irql: u8,
    pub resetting: bool,
    pub active_waits_full: bool,
    pub thread_binding_valid: bool,
    pub current_started_unfinished: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Wait(WaitOutcome),
    /// Adapter must actually sleep on a monotonic deadline, then return TIMEOUT.
    HostPause {
        nanoseconds: u32,
    },
    Unregistered,
}
impl SyncObjects {
    pub fn wait_guest(
        &mut self,
        ordinal: u32,
        frame: Option<CallFrame>,
        memory: &mut impl GuestMemory,
        context: Context,
    ) -> Outcome {
        if !matches!(ordinal, 233 | 234) {
            return Outcome::Unregistered;
        }
        let request = match NtWait::decode(ordinal, frame, memory) {
            Ok(request) => request,
            Err(status) => return Outcome::Wait(WaitOutcome::Status(status)),
        };
        let refused = Outcome::Wait(WaitOutcome::Refused(Refusal::UnsupportedScope));
        if context.resetting || context.irql != 0 || request.mode != 1 || request.alertable {
            return refused;
        }
        if request.handle == Handle(0xfffffffe) {
            let nanoseconds = match request.timeout {
                Timeout::Relative(80000) => 8000000,
                Timeout::Relative(160000) => 16000000,
                _ => return refused,
            };
            return if context.current_started_unfinished && !context.active_waits_full {
                Outcome::HostPause { nanoseconds }
            } else {
                refused
            };
        }
        if matches!(self.get(request.handle), Some(Object::Thread(_))) {
            // The finite-timeout gate precedes thread table consistency checks.
            if !matches!(request.timeout, Timeout::Infinite | Timeout::Poll) {
                return refused;
            }
            if !context.thread_binding_valid {
                return Outcome::Wait(WaitOutcome::Status(crate::INVALID_HANDLE));
            }
            if context.active_waits_full {
                return refused;
            }
        }
        Outcome::Wait(self.wait(
            request.handle,
            context.caller,
            context.irql,
            true,
            false,
            request.timeout,
        ))
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeDiagnostics {
    pub unhonoured: u32,
    pub notification_logged: bool,
    /// C log calls, including frame/object failure and timeout-ignored diagnostics.
    pub logs: u32,
}
impl KeDiagnostics {
    pub fn wait_guest(&mut self, frame: Option<CallFrame>, memory: &mut impl GuestMemory) -> u32 {
        let Some([object, _, _, _, timeout]) = frame.and_then(|frame| frame.args(memory)) else {
            self.logs = self.logs.wrapping_add(1);
            return INVALID_PARAMETER;
        };
        let Some(state) = (if object == 0 {
            None
        } else {
            memory.read::<4>(object.wrapping_add(4))
        }) else {
            self.logs = self.logs.wrapping_add(1);
            return INVALID_PARAMETER;
        };
        let state = u32::from_le_bytes(state);
        if state == 0 {
            self.unhonoured = self.unhonoured.wrapping_add(1);
            self.logs = self.logs.wrapping_add(1);
        } else {
            let kind = memory.read::<1>(object).map(|bytes| bytes[0]);
            if kind == Some(1) {
                if !memory.write(object.wrapping_add(4), 0u32.to_le_bytes()) {
                    self.logs = self.logs.wrapping_add(1);
                }
            }
            if kind != Some(0) || !self.notification_logged {
                self.logs = self.logs.wrapping_add(1);
                if kind == Some(0) {
                    self.notification_logged = true;
                }
            }
        }
        if timeout != 0 {
            self.logs = self.logs.wrapping_add(1);
        }
        SUCCESS
    }
}
