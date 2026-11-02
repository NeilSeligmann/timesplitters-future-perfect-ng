// SPDX-License-Identifier: GPL-3.0-or-later
//! T3933: Rust-owned source-time frequency clock and pitch adapter for the Rust audio host. Replaces
//! `src/audio/dsound_stream_frequency.c` (kept as the `TSFP_RUST_AUDIO_HOST=0` fallback) under the same C symbols and
//! `repr(C)` layouts of `dsound_stream_frequency.h`. All transitions are transactional like the C: failure leaves the
//! output unchanged.

use tsfp_audio::routing::frequency_pitch;

pub const EVENTS: usize = 64;
const MAX_BASE_HZ: u32 = 200_000;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Phase {
    pub ticks: u64,
    pub fraction: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Event {
    pub active_ticks: u64,
    pub source_ticks: u64,
    pub frequency_q32: u64,
    pub fraction: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    pub base_hertz: u32,
    pub count: u32,
    pub events: [Event; EVENTS],
}

fn valid(clock: &Clock) -> bool {
    clock.base_hertz != 0 && clock.count != 0 && clock.count as usize <= EVENTS
}

fn init(clock: &mut Clock, base_hertz: u32) -> bool {
    if base_hertz == 0 || base_hertz > MAX_BASE_HZ {
        return false;
    }
    *clock = Clock {
        base_hertz,
        count: 1,
        events: [Event::default(); EVENTS],
    };
    clock.events[0].frequency_q32 = u64::from(base_hertz) << 32;
    true
}

fn phase(clock: &Clock, active: u64) -> Option<Phase> {
    if !valid(clock) || active < clock.events[0].active_ticks {
        return None;
    }
    let mut event = &clock.events[0];
    for next in &clock.events[1..clock.count as usize] {
        if next.active_ticks > active {
            break;
        }
        event = next;
    }
    let denominator = u128::from(clock.base_hertz) << 32;
    let product = u128::from(active - event.active_ticks) * u128::from(event.frequency_q32);
    let whole = product / denominator;
    let fraction =
        (((product % denominator) << 32) / denominator) as u64 + u64::from(event.fraction);
    let result = whole + u128::from(event.source_ticks) + u128::from(fraction >> 32);
    u64::try_from(result).ok().map(|ticks| Phase {
        ticks,
        fraction: fraction as u32,
    })
}

fn change(clock: &mut Clock, active: u64, frequency_q32: u64, retire_through: u64) -> bool {
    let count = clock.count as usize;
    if count == 0
        || count > EVENTS
        || frequency_q32 == 0
        || frequency_q32 > u64::from(MAX_BASE_HZ) << 32
        || retire_through > active
        || active < clock.events[count - 1].active_ticks
    {
        return false;
    }
    let Some(at) = phase(clock, active) else {
        return false;
    };
    let next = Event {
        active_ticks: active,
        source_ticks: at.ticks,
        frequency_q32,
        fraction: at.fraction,
    };
    if active == clock.events[count - 1].active_ticks {
        clock.events[count - 1] = next;
        return true;
    }
    let mut keep = 0;
    while keep + 1 < count && clock.events[keep + 1].active_ticks <= retire_through {
        keep += 1;
    }
    let retained = count - keep;
    if retained == EVENTS {
        return false;
    }
    clock.events.copy_within(keep..count, 0);
    clock.events[retained] = next;
    clock.count = retained as u32 + 1;
    true
}

/// `dsound_stream_frequency_pitch`.
///
/// # Safety
/// `pitch` is null or writable.
#[cfg_attr(feature = "rust-audio-host", unsafe(no_mangle))]
pub unsafe extern "C" fn dsound_stream_frequency_pitch(
    hertz: u32,
    control_word: u16,
    pitch: *mut i32,
) -> bool {
    let Some(value) = frequency_pitch(hertz, control_word) else {
        return false;
    };
    if pitch.is_null() {
        return false;
    }
    // SAFETY: non-null writable output by contract.
    unsafe { *pitch = value };
    true
}

/// `dsound_stream_frequency_clock_init`.
///
/// # Safety
/// `clock` is null or writable.
#[cfg_attr(feature = "rust-audio-host", unsafe(no_mangle))]
pub unsafe extern "C" fn dsound_stream_frequency_clock_init(
    clock: *mut Clock,
    base_hertz: u32,
) -> bool {
    // SAFETY: null checked; the caller owns the writable clock.
    unsafe { clock.as_mut() }.is_some_and(|clock| init(clock, base_hertz))
}

/// `dsound_stream_frequency_clock_phase`.
///
/// # Safety
/// Both pointers are null or valid.
#[cfg_attr(feature = "rust-audio-host", unsafe(no_mangle))]
pub unsafe extern "C" fn dsound_stream_frequency_clock_phase(
    clock: *const Clock,
    active: u64,
    out: *mut Phase,
) -> bool {
    // SAFETY: null checked; valid for the call by contract.
    let (Some(clock), Some(out)) = (unsafe { clock.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    phase(clock, active).is_some_and(|value| {
        *out = value;
        true
    })
}

/// `dsound_stream_frequency_clock_change`.
///
/// # Safety
/// `clock` is null or valid and writable.
#[cfg_attr(feature = "rust-audio-host", unsafe(no_mangle))]
pub unsafe extern "C" fn dsound_stream_frequency_clock_change(
    clock: *mut Clock,
    active: u64,
    frequency_q32: u64,
    retire_through: u64,
) -> bool {
    // SAFETY: null checked; the caller owns the writable clock.
    unsafe { clock.as_mut() }
        .is_some_and(|clock| change(clock, active, frequency_q32, retire_through))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(hz: u32) -> Clock {
        let mut clock = Clock {
            base_hertz: 0,
            count: 0,
            events: [Event::default(); EVENTS],
        };
        assert!(init(&mut clock, hz));
        clock
    }

    #[test]
    fn layout_matches_the_c_header() {
        assert_eq!(std::mem::size_of::<Event>(), 32);
        assert_eq!(std::mem::size_of::<Phase>(), 16);
        assert_eq!(std::mem::size_of::<Clock>(), 8 + 32 * EVENTS);
    }

    #[test]
    fn constant_rate_phase_is_proportional() {
        let clock = fresh(48000);
        assert_eq!(phase(&clock, 96000).unwrap().ticks, 96000);
        let mut half = fresh(48000);
        assert!(change(&mut half, 1000, 24000u64 << 32, 0));
        assert_eq!(phase(&half, 3000).unwrap().ticks, 1000 + 1000);
    }

    #[test]
    fn refusals_leave_the_clock_unchanged() {
        let mut clock = fresh(48000);
        let before = clock;
        assert!(!change(&mut clock, 10, 0, 0));
        assert!(!change(&mut clock, 10, u64::from(MAX_BASE_HZ + 1) << 32, 0));
        assert!(!change(&mut clock, 10, 1 << 32, 11));
        assert_eq!(clock, before);
        assert!(!init(&mut clock, 0));
        assert_eq!(clock, before);
    }

    #[test]
    fn event_table_full_refuses_without_retirement() {
        let mut clock = fresh(48000);
        for i in 1..EVENTS as u64 {
            assert!(change(&mut clock, i, 48000u64 << 32, 0));
        }
        let before = clock;
        assert!(!change(&mut clock, EVENTS as u64, 48000u64 << 32, 0));
        assert_eq!(clock, before);
        assert!(change(&mut clock, EVENTS as u64, 48000u64 << 32, 10));
    }
}
