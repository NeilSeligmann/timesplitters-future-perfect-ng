//! Narrow host seam for T2409's Rust-owned frame counters and timing history.
#[cfg(not(test))]
use core::sync::atomic::{AtomicBool, Ordering};

use crate::frame_events::FrameEventCallback;
use crate::frame_state::{
    FrameClockState, FrameCounters, FrameEventState, FramePauseState, FrameStepOutputs,
    FrameTimingHistory, VblankCounter,
};

#[cfg(not(test))]
static NATIVE_FRAME_STATE_SELECTED: AtomicBool = AtomicBool::new(false);
// Unit tests: selection is per test thread. The process flag was never cleared, so once one test selected the
// native owner every later frame-loop test in the process saw it, an order/timing flake (2 of 4 parallel runs
// failed ext::frame_timing_tests on 2026-10-20). Same approach as state_wire::RETIRE (T3447).
#[cfg(test)]
thread_local! {
    static NATIVE_FRAME_STATE_SELECTED_TEST: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}
static NATIVE_VBLANK_COUNTER: VblankCounter = VblankCounter::new();
static NATIVE_FRAME_COUNTERS: FrameCounters = FrameCounters::new();
static NATIVE_FRAME_CLOCK: FrameClockState = FrameClockState::new();
static NATIVE_FRAME_EVENTS: FrameEventState = FrameEventState::new();
static NATIVE_FRAME_PAUSE: FramePauseState = FramePauseState::new();
static NATIVE_FRAME_TIMING_HISTORY: FrameTimingHistory = FrameTimingHistory::new();
static NATIVE_FRAME_STEP_OUTPUTS: FrameStepOutputs = FrameStepOutputs::new();

/// One-time seed copied from the guest image when the native frame-state owner
/// is selected. The host keeps this as a stack value; Rust retains only its
/// typed fields and never keeps a guest-memory pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct FrameStateSeed {
    pub vblank_count: u32,
    pub elapsed_frames: u32,
    pub frames: u32,
    pub multi_vblank_frames: u32,
    pub simulation_stamp: u32,
    pub sampled_vblank: u32,
    pub raw_vblank_step: u32,
    pub outer_frames: u32,
    pub frame_phase: u32,
    pub event_latch: u32,
    pub event_callback: u32,
    pub pause_word: u32,
    pub timing_history: [u32; FrameTimingHistory::WORD_COUNT],
    pub step_outputs: [u32; FrameStepOutputs::WORD_COUNT],
}

pub fn native_frame_state_selected() -> bool {
    #[cfg(not(test))]
    return NATIVE_FRAME_STATE_SELECTED.load(Ordering::Acquire);
    #[cfg(test)]
    return NATIVE_FRAME_STATE_SELECTED_TEST.with(|selected| selected.get());
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_native_selected() -> bool {
    native_frame_state_selected()
}

pub fn native_vblank_counter() -> &'static VblankCounter {
    &NATIVE_VBLANK_COUNTER
}

pub fn native_frame_counters() -> &'static FrameCounters {
    &NATIVE_FRAME_COUNTERS
}

pub fn native_frame_clock_state() -> &'static FrameClockState {
    &NATIVE_FRAME_CLOCK
}

pub fn native_frame_pause() -> &'static FramePauseState {
    &NATIVE_FRAME_PAUSE
}

pub fn native_frame_timing_history() -> &'static FrameTimingHistory {
    &NATIVE_FRAME_TIMING_HISTORY
}

pub fn native_frame_step_outputs() -> &'static FrameStepOutputs {
    &NATIVE_FRAME_STEP_OUTPUTS
}

/// Advance the selected Rust-owned outer-frame counter. `None` leaves the
/// unselected caller on its ordinary guest-memory path.
pub fn native_frame_clock_advance_outer_frame() -> Option<u32> {
    native_frame_state_selected().then(|| NATIVE_FRAME_CLOCK.advance_outer_frame())
}

/// Toggle the selected Rust-owned frame phase with the measured dword rule.
pub fn native_frame_clock_toggle_phase() -> Option<u32> {
    native_frame_state_selected().then(|| NATIVE_FRAME_CLOCK.toggle_frame_phase())
}

pub fn native_frame_owned_word_read(address: u32, width: u32) -> Option<u32> {
    native_frame_state_selected().then_some(())?;
    crate::frame_state::read_owned_frame_word(
        &NATIVE_FRAME_COUNTERS,
        &NATIVE_FRAME_CLOCK,
        &NATIVE_FRAME_STEP_OUTPUTS,
        &NATIVE_FRAME_PAUSE,
        &NATIVE_FRAME_TIMING_HISTORY,
        &NATIVE_FRAME_EVENTS,
        address,
        width,
    )
}

pub fn native_frame_owned_word_write(address: u32, width: u32, value: u32) -> bool {
    native_frame_state_selected()
        && crate::frame_state::write_owned_frame_word(
            &NATIVE_FRAME_COUNTERS,
            &NATIVE_FRAME_CLOCK,
            &NATIVE_FRAME_STEP_OUTPUTS,
            &NATIVE_FRAME_PAUSE,
            &NATIVE_FRAME_TIMING_HISTORY,
            &NATIVE_FRAME_EVENTS,
            address,
            width,
            value,
        )
}

/// Apply the selected frame event transition directly to its Rust owner.
/// `None` means the native frame state is not selected and the caller should
/// use its ordinary guest-memory path.
pub fn native_frame_event_clear_callback_when_latched() -> Option<bool> {
    native_frame_state_selected().then(|| NATIVE_FRAME_EVENTS.clear_callback_when_latched())
}

/// Read the selected event callback from Rust-owned state. The outer `Option`
/// indicates whether the native frame state is selected.
pub fn native_frame_event_callback_to_dispatch() -> Option<Option<FrameEventCallback>> {
    native_frame_state_selected().then(|| {
        NATIVE_FRAME_EVENTS
            .callback_to_dispatch()
            .map(FrameEventCallback)
    })
}

/// Apply one selected raw vblank step through readable Rust-owned state.
/// The sound callback runs at the original pause-branch point, so pause calls
/// see the old effective step while resume calls see the reset state.
pub fn native_apply_frame_step(
    vblank_difference: u32,
    one_step_float_bits: u32,
    sound_call: impl FnMut(crate::frame_state::FrameSoundTransition, u32, u32, u32, u32),
) -> Option<crate::frame_state::FrameStepTransition> {
    if !native_frame_state_selected() {
        return None;
    }
    Some(crate::frame_state::apply_native_frame_step(
        &NATIVE_FRAME_COUNTERS,
        &NATIVE_FRAME_PAUSE,
        &NATIVE_FRAME_STEP_OUTPUTS,
        vblank_difference,
        one_step_float_bits,
        sound_call,
    ))
}

pub fn native_frame_owned_span(address: u32, len: usize) -> bool {
    if !native_frame_state_selected() || len == 0 {
        return false;
    }
    let start = address as u64;
    let end = start.saturating_add(len as u64);
    let overlaps = |base: u32| start < base as u64 + 4 && (base as u64) < end;
    crate::frame_state::FrameCounters::ADDRESSES
        .iter()
        .chain(crate::frame_state::FrameClockState::ADDRESSES.iter())
        .chain(crate::frame_state::FrameStepOutputs::ADDRESSES.iter())
        .chain(crate::frame_state::FramePauseState::ADDRESSES.iter())
        .chain(crate::frame_state::FrameTimingHistory::ADDRESSES.iter())
        .chain(crate::frame_state::FrameEventState::ADDRESSES.iter())
        .copied()
        .any(overlaps)
}

/// Select the native frame-state owner and copy one complete typed seed.
///
/// # Safety
///
/// A non-null `seed` must point to one readable `FrameStateSeed` for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_frame_state_select_native(seed: *const FrameStateSeed) -> bool {
    if seed.is_null() {
        return false;
    }
    // SAFETY: the host passes a readable stack `FrameStateSeed` for this call;
    // the owner copies every value below and retains no pointer.
    let seed = unsafe { seed.read() };
    NATIVE_VBLANK_COUNTER.initialize(seed.vblank_count);
    NATIVE_FRAME_COUNTERS.initialize(
        seed.elapsed_frames,
        seed.frames,
        seed.multi_vblank_frames,
        seed.simulation_stamp,
    );
    NATIVE_FRAME_CLOCK.initialize(
        seed.sampled_vblank,
        seed.raw_vblank_step,
        seed.outer_frames,
        seed.frame_phase,
    );
    NATIVE_FRAME_EVENTS.initialize(seed.event_latch, seed.event_callback);
    NATIVE_FRAME_PAUSE.initialize(seed.pause_word);
    NATIVE_FRAME_TIMING_HISTORY.initialize(&seed.timing_history);
    NATIVE_FRAME_STEP_OUTPUTS.initialize(&seed.step_outputs);
    #[cfg(not(test))]
    NATIVE_FRAME_STATE_SELECTED.store(true, Ordering::Release);
    #[cfg(test)]
    NATIVE_FRAME_STATE_SELECTED_TEST.with(|selected| selected.set(true));
    true
}

/// Read-only host diagnostic for a raw/effective integer or float-bit output.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_step_output_snapshot(index: u32) -> u32 {
    NATIVE_FRAME_STEP_OUTPUTS
        .snapshot()
        .get(index as usize)
        .copied()
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_clock_snapshot(index: u32) -> u32 {
    NATIVE_FRAME_CLOCK
        .snapshot()
        .get(index as usize)
        .copied()
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_event_snapshot(index: u32) -> u32 {
    NATIVE_FRAME_EVENTS
        .snapshot()
        .get(index as usize)
        .copied()
        .unwrap_or(0)
}

/// Typed guest-memory seam for Rust-owned frame words and step outputs.
/// `write` is zero for read and nonzero for write; `value` is input/output.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_frame_state_owned_word_access(
    address: u32,
    width: u32,
    write: u32,
    value: *mut u32,
) -> u32 {
    if !native_frame_state_selected() || value.is_null() {
        return 0;
    }
    if write != 0 {
        let input = unsafe { *value };
        native_frame_owned_word_write(address, width, input).into()
    } else if let Some(output) = native_frame_owned_word_read(address, width) {
        unsafe { *value = output };
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_elapsed_frames_snapshot() -> u32 {
    NATIVE_FRAME_COUNTERS.elapsed_frames()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_simulation_stamp_snapshot() -> u32 {
    NATIVE_FRAME_COUNTERS.simulation_stamp()
}

/// Read-only host diagnostic; the count is never copied into guest memory.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_vblank_snapshot() -> u32 {
    NATIVE_VBLANK_COUNTER.delivered()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_record_vblank_delivery() -> u32 {
    NATIVE_VBLANK_COUNTER.record_delivery()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_vblank_counter_read() -> u32 {
    NATIVE_VBLANK_COUNTER.delivered()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_frames_snapshot() -> u32 {
    NATIVE_FRAME_COUNTERS.frames()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_multi_vblank_snapshot() -> u32 {
    NATIVE_FRAME_COUNTERS.multi_vblank_frames()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_pause_snapshot() -> u32 {
    NATIVE_FRAME_PAUSE.word()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_state_timing_history_snapshot(index: u32) -> u32 {
    NATIVE_FRAME_TIMING_HISTORY
        .word(index as usize)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        FrameStateSeed, native_frame_pause, native_frame_step_outputs, native_frame_timing_history,
        tsfp_frame_state_owned_word_access, tsfp_frame_state_select_native,
    };
    use crate::mem::GuestAccess;
    use crate::memory::GuestMemory;

    #[test]
    fn native_selection_copies_the_timing_history_seed() {
        let seed: [u32; 15] = core::array::from_fn(|index| (index as u32).wrapping_mul(0x101));
        let step_seed = [1, 0, 1.0f32.to_bits(), 0];
        let selection = FrameStateSeed {
            vblank_count: 5,
            elapsed_frames: 7,
            frames: 11,
            multi_vblank_frames: 13,
            simulation_stamp: 37,
            sampled_vblank: 17,
            raw_vblank_step: 19,
            outer_frames: 23,
            frame_phase: 41,
            event_latch: 29,
            event_callback: 31,
            pause_word: 0xFFFF_FFFE,
            timing_history: seed,
            step_outputs: step_seed,
        };
        assert_eq!(
            core::mem::size_of::<FrameStateSeed>(),
            31 * core::mem::size_of::<u32>()
        );
        assert!(unsafe { tsfp_frame_state_select_native(&selection) });

        assert_eq!(native_frame_timing_history().snapshot(), seed);
        assert_eq!(native_frame_pause().word(), 0xFFFF_FFFE);
        assert_eq!(native_frame_step_outputs().snapshot(), step_seed);
        assert_eq!(super::NATIVE_FRAME_CLOCK.snapshot(), [17, 19, 23, 41]);
        assert_eq!(super::native_frame_clock_advance_outer_frame(), Some(24));
        assert_eq!(
            super::native_frame_clock_toggle_phase(),
            Some(u32::MAX - 39)
        );
        assert_eq!(
            super::NATIVE_FRAME_CLOCK.snapshot(),
            [17, 19, 24, u32::MAX - 39]
        );
        assert_eq!(super::NATIVE_FRAME_COUNTERS.simulation_stamp(), 37);
        assert_eq!(super::NATIVE_FRAME_EVENTS.snapshot(), [29, 31]);
        assert_eq!(super::tsfp_frame_state_event_snapshot(0), 29);
        assert_eq!(super::tsfp_frame_state_event_snapshot(1), 31);
        assert_eq!(
            super::native_frame_event_callback_to_dispatch(),
            Some(Some(crate::frame_events::FrameEventCallback(31)))
        );
        assert_eq!(
            super::native_frame_event_clear_callback_when_latched(),
            Some(true)
        );
        assert_eq!(super::NATIVE_FRAME_EVENTS.snapshot(), [0, 0]);

        let mut backing = [0xa5; 16];
        let offset = (backing.as_mut_ptr() as usize).wrapping_sub(0x004e7940usize);
        let mut memory = unsafe { crate::memory::MappedGuest::new(offset) };
        assert_eq!(memory.read32(0x004e7940), 1);
        memory.write32(0x004e7944, 3);
        memory.write8(0x004e7948, 0x80);
        assert_eq!(memory.read32(0x004e7944), 3);
        assert_eq!(memory.read32(0x004e7948), 0x3f80_0080);
        let mut event_backing = [0xa5; 8];
        let event_offset = (event_backing.as_mut_ptr() as usize).wrapping_sub(0x007e_b2e0usize);
        let mut owner_only = unsafe { crate::memory::MappedGuest::new(event_offset) };
        assert_eq!(owner_only.read32(0x0074_97b0), 7);
        owner_only.write8(0x0074_97b0, 9);
        assert_eq!(owner_only.read32(0x0074_97b0), 9);
        assert_eq!(owner_only.read32(0x007497a8), 17);
        owner_only.write32(0x007eb2e0, 37);
        owner_only.write32(0x007eb2e4, 41);
        assert_eq!(owner_only.read32(0x007eb2e0), 37);
        assert_eq!(owner_only.read32(0x007eb2e4), 41);
        assert_eq!(super::tsfp_frame_state_event_snapshot(0), 37);
        assert_eq!(super::tsfp_frame_state_event_snapshot(1), 41);
        assert_eq!(event_backing, [0xa5; 8]);
        // Lifted callers can address either event word at byte or halfword
        // width. The typed seam must preserve neighboring bytes without
        // falling through to the guest image.
        owner_only.write8(0x007e_b2e1, 0xa5);
        assert_eq!(owner_only.read32(0x007e_b2e0), 0xa525);
        owner_only.write16(0x007e_b2e6, 0x1234);
        assert_eq!(owner_only.read32(0x007e_b2e4), 0x1234_0029);
        assert_eq!(event_backing, [0xa5; 8]);
        owner_only.write32(0x007e_b2e0, 37);
        owner_only.write32(0x007e_b2e4, 41);
        let mut stamp_backing = [0xa5; 4];
        let stamp_offset = (stamp_backing.as_mut_ptr() as usize).wrapping_sub(0x007d_e318usize);
        let mut owner_stamp = unsafe { crate::memory::MappedGuest::new(stamp_offset) };
        assert_eq!(owner_stamp.read32(0x007d_e318), 37);
        owner_stamp.write32(0x007d_e318, 43);
        assert_eq!(owner_stamp.read32(0x007d_e318), 43);
        assert_eq!(super::tsfp_frame_state_simulation_stamp_snapshot(), 43);
        assert_eq!(stamp_backing, [0xa5; 4]);
        owner_only.write32(0x007eb2e0, 0);
        owner_only.write32(0x007eb2e4, 0x0012_3456);
        assert_eq!(
            crate::frame_events::callback_to_dispatch(&owner_only),
            Some(crate::frame_events::FrameEventCallback(0x0012_3456))
        );
        assert!(!crate::frame_events::clear_callback_when_latched(
            &mut owner_only
        ));
        owner_only.write32(0x007eb2e0, 1);
        assert!(crate::frame_events::clear_callback_when_latched(
            &mut owner_only
        ));
        assert_eq!(super::tsfp_frame_state_event_snapshot(0), 0);
        assert_eq!(super::tsfp_frame_state_event_snapshot(1), 0);
        assert_eq!(event_backing, [0xa5; 8]);
        owner_only.write32(0x007eb2e4, 41);
        owner_only.write32(0x007497ac, 29);
        owner_only.write8(0x006b_7af0, 0x34);
        owner_only.write8(0x006b_7af1, 0x12);
        assert_eq!(owner_only.read32(0x007497ac), 29);
        assert_eq!(owner_only.read32(0x006b_7af0), 0x1234);
        owner_only.write32(0x007e_b2e0, 0);
        assert_eq!(owner_only.read32(0x007e_b2e0), 0);
        assert_eq!(owner_only.read32(0x007e_b2e4), 41);
        assert_eq!(owner_only.read32(0x0074_97b4), 0xffff_fffe);
        owner_only.write32(0x0074_97b4, 0x1234_5678);
        assert_eq!(native_frame_pause().word(), 0x1234_5678);
        assert_eq!(owner_only.read32(0x007a_58c0), seed[5]);
        owner_only.write32(0x007a_58a0, 0xdead_beef);
        assert_eq!(native_frame_timing_history().word(0), Some(0xdead_beef));
        assert!(backing.iter().all(|byte| *byte == 0xa5));

        let mut value = 0;
        assert_eq!(
            unsafe { tsfp_frame_state_owned_word_access(0x004e794c, 4, 0, &mut value) },
            1
        );
        assert_eq!(value, 0);
    }
}
