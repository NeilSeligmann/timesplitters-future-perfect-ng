//! Small C ABI boundary; readable state/logic lives in tsfp-kernel-clock.
use std::sync::Mutex;
pub mod frame_phase;
pub mod vblank_quiescence;
use tsfp_kernel_clock::guest::{Adapter, Call, GuestMemory, HostTime};
use tsfp_kernel_clock::{Clock, ClockBackend, Operation};
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_virtual_ticks() -> u64 {
    tsfp_timers_host::virtual_ticks_abi()
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_audio_clock(context: *mut core::ffi::c_void) -> u64 {
    tsfp_timers_host::audio_clock_abi(context)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_virtual_ns() -> u64 {
    tsfp_timers_host::virtual_ns_abi()
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_ticks_to_ns(ticks: u64) -> u64 {
    tsfp_timers_host::ticks_to_ns_abi(ticks)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_frequency() -> u64 {
    tsfp_timers_host::frequency_abi()
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_tick_count_ms() -> u32 {
    tsfp_timers_host::tick_count_ms_abi()
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_bind_tick_count(address: u32) -> bool {
    tsfp_timers_host::bind_tick_count_abi(address)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_set_frame_hook(hook: Option<unsafe extern "C" fn()>) {
    tsfp_timers_host::set_frame_hook_abi(hook)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_get_stats() -> tsfp_timers_host::KernelClockStats {
    tsfp_timers_host::stats_abi()
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_timers_host_monotonic_ns() -> u64 {
    tsfp_timers_host::monotonic_ns()
}
static CLOCK: Mutex<Option<Clock>> = Mutex::new(None);
static ADAPTER: Mutex<Option<Adapter>> = Mutex::new(None);
#[repr(C)]
pub struct Snapshot {
    result: u64,
    total: u64,
    floor: u64,
    reads: u64,
    reads_above_floor: u64,
    frames: u64,
    stall_ticks: u64,
    advance_ticks: u64,
    advance_calls: u64,
}
/// Apply an elapsed-clock operation through the C ABI boundary.
///
/// Codes 0..=4 select read, frame, stall, advance and reset respectively.
/// Other codes peek without changing the clock. Frame and stall operands are
/// truncated to 32 bits, matching the guest call ABI.
///
/// ```
/// use tsfp_kernel_clock_capi::tsfp_kernel_clock_apply;
/// assert_eq!(tsfp_kernel_clock_apply(4, 0).result(), 0);
/// assert_eq!(tsfp_kernel_clock_apply(3, 42).result(), 42);
/// assert_eq!(tsfp_kernel_clock_apply(99, 0).result(), 42);
/// ```
impl Snapshot {
    /// Value returned to the C caller for this operation.
    pub const fn result(&self) -> u64 {
        self.result
    }
}
/// C callers serialize through clock_lock; the mutex also protects direct ABI callers.
/// No pointers, borrowed memory, guest writes or callbacks cross this boundary.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kernel_clock_apply(operation: u32, value: u64) -> Snapshot {
    let mut state = CLOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let clock = state.get_or_insert_with(Clock::default);
    let op = match operation {
        0 => Operation::Read,
        1 => Operation::Frame(value as u32),
        2 => Operation::Stall(value as u32),
        3 => Operation::Advance(value),
        4 => Operation::Reset,
        _ => Operation::Peek,
    };
    let snapshot = clock.apply(op);
    let s = snapshot.stats;
    Snapshot {
        result: snapshot.result,
        total: s.total,
        floor: s.floor,
        reads: s.reads,
        reads_above_floor: s.reads_above_floor,
        frames: s.frames,
        stall_ticks: s.stall_ticks,
        advance_ticks: s.advance_ticks,
        advance_calls: s.advance_calls,
    }
}

/// Read the selected Rust clock state without advancing it.
///
/// The audio host uses this entry point when `TSFP_RUST_KCLOCK` selects the Rust
/// backend, avoiding a trip through the C mirror used by other host callers.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kernel_clock_peek() -> u64 {
    CLOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get_or_insert_with(Clock::default)
        .peek()
}

/// Guest memory accessors supplied by the C host (K-0); each returns false when unmapped.
#[repr(C)]
pub struct GuestMemoryOps {
    pub read_u32: unsafe extern "C" fn(address: u32, out: *mut u32) -> bool,
    pub write_u32: unsafe extern "C" fn(address: u32, value: u32) -> bool,
    pub write_u16: unsafe extern "C" fn(address: u32, value: u16) -> bool,
}
impl GuestMemory for &GuestMemoryOps {
    fn read_u32(&self, address: u32) -> Option<u32> {
        let mut value = 0;
        unsafe { (self.read_u32)(address, &mut value) }.then_some(value)
    }
    fn write_u32(&mut self, address: u32, value: u32) -> bool {
        unsafe { (self.write_u32)(address, value) }
    }
    fn write_u16(&mut self, address: u32, value: u16) -> bool {
        unsafe { (self.write_u16)(address, value) }
    }
}

/// Host wall clock in 100 ns units since 1601, `None` when unreadable.
fn wall_units() -> Option<u64> {
    let since_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let units = since_unix
        .as_secs()
        .checked_add(11_644_473_600)?
        .checked_mul(10_000_000)?;
    units.checked_add(u64::from(since_unix.subsec_nanos() / 100))
}

/// T2207: timer, DPC and calendar ordinals (97, 107, 113, 119, 128, 137, 149, 150, 228) on the
/// Rust `guest::Adapter`. Elapsed time comes from this crate's clock, wall time from the host.
/// Returns false (EAX untouched) for an unknown ordinal or too few slots.
///
/// # Safety
/// `slots` points to `count` dwords; `memory` and `eax` are valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kernel_clock_guest_call(
    ordinal: u32,
    slots: *const u32,
    count: u32,
    memory: *const GuestMemoryOps,
    eax: *mut u32,
) -> bool {
    let slots = unsafe { std::slice::from_raw_parts(slots, count as usize) };
    let Some(call) = Call::decode(ordinal, slots) else {
        return false;
    };
    let elapsed = CLOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get_or_insert_with(Clock::default)
        .elapsed_units();
    let time = HostTime {
        elapsed,
        wall: wall_units(),
    };
    let mut adapter = ADAPTER.lock().unwrap_or_else(|poison| poison.into_inner());
    let result = adapter
        .get_or_insert_with(Adapter::default)
        .call(call, time, &mut unsafe { &*memory });
    unsafe { *eax = result };
    true
}

/// Drops all Rust timer, DPC and calendar state; mirrors `kernel_event_reset`.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kernel_clock_guest_reset() {
    *ADAPTER.lock().unwrap_or_else(|poison| poison.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_operation_mapping_and_operand_width() {
        tsfp_kernel_clock_apply(4, 0);
        tsfp_kernel_clock_apply(3, 123);
        assert_eq!(tsfp_kernel_clock_peek(), 123);

        let values = [
            0,
            1,
            60,
            1000,
            1_000_001,
            u32::MAX as u64,
            1_u64 << 32,
            u64::MAX,
        ];
        for value in values {
            for code in [0, 1, 2, 3, 4, 5, u32::MAX] {
                tsfp_kernel_clock_apply(4, 0);
                let mut expected = Clock::default();
                let operation = match code {
                    0 => Operation::Read,
                    1 => Operation::Frame(value as u32),
                    2 => Operation::Stall(value as u32),
                    3 => Operation::Advance(value),
                    4 => Operation::Reset,
                    _ => Operation::Peek,
                };
                let reference = expected.apply(operation);
                let actual = tsfp_kernel_clock_apply(code, value);
                assert_eq!(actual.result, reference.result, "code={code} value={value}");
                assert_eq!(actual.total, reference.stats.total);
                assert_eq!(actual.floor, reference.stats.floor);
                assert_eq!(actual.reads, reference.stats.reads);
                assert_eq!(actual.reads_above_floor, reference.stats.reads_above_floor);
                assert_eq!(actual.frames, reference.stats.frames);
                assert_eq!(actual.stall_ticks, reference.stats.stall_ticks);
                assert_eq!(actual.advance_ticks, reference.stats.advance_ticks);
                assert_eq!(actual.advance_calls, reference.stats.advance_calls);
            }
        }
    }
}
