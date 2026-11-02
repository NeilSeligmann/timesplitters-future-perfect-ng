//! Host-facing timer operations shared by the native Rust host and presentation sinks.
//!
//! The clock backend remains selected by `kernel_clock.c`: Rust is the default and
//! `TSFP_RUST_GAME_TIMERS=0` retains the C compatibility backend. This crate owns the
//! host conversion policy and calls the selected clock through its stable host ABI.

const FREQUENCY_HZ: u64 = 733_333_333;

static MONOTONIC_EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Process-relative monotonic nanoseconds for host timeout accounting.
pub fn monotonic_ns() -> u64 {
    MONOTONIC_EPOCH
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos() as u64
}

unsafe extern "C" {
    fn kernel_clock_peek() -> u64;
    fn kernel_clock_frequency() -> u64;
    fn kernel_clock_tick_count_ms() -> u32;
    fn kernel_clock_bind_tick_count(address: u32) -> bool;
    fn kernel_clock_set_frame_hook(hook: Option<unsafe extern "C" fn()>);
    fn kernel_clock_get_stats() -> KernelClockStats;
}

/// Stable view of the selected guest clock's diagnostic counters.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct KernelClockStats {
    pub total_ticks: u64,
    pub floor_ticks: u64,
    pub reads: u64,
    pub reads_above_floor: u64,
    pub frames: u64,
    pub stall_ticks: u64,
    pub advance_to_ticks: u64,
    pub advance_to_calls: u64,
}

/// Convert virtual clock ticks to nanoseconds without overflowing an intermediate.
pub fn virtual_ns_from_ticks(ticks: u64) -> u64 {
    (ticks / FREQUENCY_HZ)
        .wrapping_mul(1_000_000_000)
        .wrapping_add((ticks % FREQUENCY_HZ) * 1_000_000_000 / FREQUENCY_HZ)
}

/// Read the selected guest clock without advancing it.
pub fn virtual_ticks() -> u64 {
    // SAFETY: kernel_clock_peek is a process-lifetime host function with no pointer arguments.
    unsafe { kernel_clock_peek() }
}

/// Current virtual time in nanoseconds for presentation and audio host consumers.
pub fn virtual_ns() -> u64 {
    virtual_ns_from_ticks(virtual_ticks())
}

pub fn virtual_ticks_abi() -> u64 {
    virtual_ticks()
}

pub fn audio_clock_abi(_context: *mut core::ffi::c_void) -> u64 {
    virtual_ticks()
}

pub fn virtual_ns_abi() -> u64 {
    virtual_ns()
}

pub fn ticks_to_ns_abi(ticks: u64) -> u64 {
    virtual_ns_from_ticks(ticks)
}

pub fn frequency_abi() -> u64 {
    // SAFETY: this accessor has no arguments or mutable side effects.
    unsafe { kernel_clock_frequency() }
}

pub fn tick_count_ms_abi() -> u32 {
    // SAFETY: this accessor has no arguments or mutable side effects.
    unsafe { kernel_clock_tick_count_ms() }
}

pub fn bind_tick_count_abi(address: u32) -> bool {
    // SAFETY: address is the guest mapping passed through the established clock ABI.
    unsafe { kernel_clock_bind_tick_count(address) }
}

pub fn set_frame_hook_abi(hook: Option<unsafe extern "C" fn()>) {
    // SAFETY: the selected clock stores the process-lifetime callback, or clears it with None.
    unsafe { kernel_clock_set_frame_hook(hook) }
}

pub fn stats_abi() -> KernelClockStats {
    // SAFETY: the C ABI returns the fixed-width stats structure declared in kernel_clock.h.
    unsafe { kernel_clock_get_stats() }
}

#[cfg(test)]
mod tests {
    use super::{monotonic_ns, virtual_ns_from_ticks};

    #[test]
    fn monotonic_host_clock_does_not_move_backwards() {
        let first = monotonic_ns();
        let second = monotonic_ns();
        assert!(second >= first);
    }

    #[test]
    fn conversion_preserves_fractional_seconds_and_large_values() {
        assert_eq!(virtual_ns_from_ticks(0), 0);
        assert_eq!(virtual_ns_from_ticks(733_333_333), 1_000_000_000);
        assert_eq!(
            virtual_ns_from_ticks(u64::MAX),
            (u64::MAX / 733_333_333)
                .wrapping_mul(1_000_000_000)
                .wrapping_add((u64::MAX % 733_333_333) * 1_000_000_000 / 733_333_333)
        );
    }
}
