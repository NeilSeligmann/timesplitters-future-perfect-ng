//! Owned frame-delivery counters shared across the host callback and game thread.
//!
//! The title's 0x22020/0x22030 ABI increments and reads one wrapping dword.
//! This value type owns that counter without exposing guest-memory storage.
//! Event routing remains lifted until its producers and consumer cross the
//! same typed ownership boundary.
use core::sync::atomic::{AtomicU32, Ordering};

/// The title-owned frame counters.
#[derive(Debug, Default)]
pub struct FrameCounters {
    elapsed_frames: AtomicU32,
    frames: AtomicU32,
    multi_vblank_frames: AtomicU32,
    simulation_stamp: AtomicU32,
}

impl FrameCounters {
    pub const ADDRESSES: [u32; 4] = [0x0074_97b0, 0x0074_97b8, 0x0074_97bc, 0x007d_e318];

    pub const fn new() -> Self {
        Self {
            elapsed_frames: AtomicU32::new(0),
            frames: AtomicU32::new(0),
            multi_vblank_frames: AtomicU32::new(0),
            simulation_stamp: AtomicU32::new(0),
        }
    }

    /// Adopt the live values once at the opt-in ownership edge.
    pub fn initialize(
        &self,
        elapsed_frames: u32,
        frames: u32,
        multi_vblank_frames: u32,
        simulation_stamp: u32,
    ) {
        self.elapsed_frames.store(elapsed_frames, Ordering::Release);
        self.frames.store(frames, Ordering::Release);
        self.multi_vblank_frames
            .store(multi_vblank_frames, Ordering::Release);
        self.simulation_stamp
            .store(simulation_stamp, Ordering::Release);
    }

    /// Advance the wrapping game-frame count and, when the measured raw step
    /// exceeds one, the corresponding multi-vblank count.
    pub fn record_frame(&self, multi_vblank: bool) -> (u32, u32) {
        let frames = self.frames.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        let multi = if multi_vblank {
            self.multi_vblank_frames
                .fetch_add(1, Ordering::AcqRel)
                .wrapping_add(1)
        } else {
            self.multi_vblank_frames.load(Ordering::Acquire)
        };
        (frames, multi)
    }

    /// Add the raw vblank step to the wrapping elapsed-frame count. This
    /// advances even while the title's pause state suppresses simulation.
    pub fn record_elapsed_step(&self, raw_step: u32) -> u32 {
        self.elapsed_frames
            .fetch_add(raw_step, Ordering::AcqRel)
            .wrapping_add(raw_step)
    }

    pub fn elapsed_frames(&self) -> u32 {
        self.elapsed_frames.load(Ordering::Acquire)
    }

    pub fn frames(&self) -> u32 {
        self.frames.load(Ordering::Acquire)
    }

    pub fn multi_vblank_frames(&self) -> u32 {
        self.multi_vblank_frames.load(Ordering::Acquire)
    }

    pub fn simulation_stamp(&self) -> u32 {
        self.simulation_stamp.load(Ordering::Acquire)
    }

    /// Virtualize measured frame-counter globals for typed lifted callers.
    pub fn read_access(&self, address: u32, width: u32) -> Option<u32> {
        let (index, byte_offset) = access_location(address, width, &Self::ADDRESSES)?;
        let value = match index {
            0 => self.elapsed_frames(),
            1 => self.frames(),
            2 => self.multi_vblank_frames(),
            3 => self.simulation_stamp(),
            _ => return None,
        };
        Some((value >> (byte_offset * 8)) & width_mask(width)?)
    }

    pub fn write_access(&self, address: u32, width: u32, value: u32) -> bool {
        let Some((index, byte_offset)) = access_location(address, width, &Self::ADDRESSES) else {
            return false;
        };
        let Some(mask) = width_mask(width) else {
            return false;
        };
        let slot = match index {
            0 => &self.elapsed_frames,
            1 => &self.frames,
            2 => &self.multi_vblank_frames,
            3 => &self.simulation_stamp,
            _ => return false,
        };
        update_word(slot, byte_offset, mask, value);
        true
    }
}

/// Owns the sampled vblank words, outer frame count, and frame phase used by
/// the frame-loop callers. These globals retain their original dword ABI.
#[derive(Debug, Default)]
pub struct FrameClockState {
    sampled_vblank: AtomicU32,
    raw_vblank_step: AtomicU32,
    outer_frames: AtomicU32,
    frame_phase: AtomicU32,
}

/// Owns the measured one-slot frame-event latch/callback pair. This type does
/// not claim or invent a larger event-queue layout.
#[derive(Debug, Default)]
pub struct FrameEventState {
    latch: AtomicU32,
    callback: AtomicU32,
}

impl FrameEventState {
    pub const ADDRESSES: [u32; 2] = [0x007e_b2e0, 0x007e_b2e4];

    pub const fn new() -> Self {
        Self {
            latch: AtomicU32::new(0),
            callback: AtomicU32::new(0),
        }
    }

    pub fn initialize(&self, latch: u32, callback: u32) {
        self.latch.store(latch, Ordering::Release);
        self.callback.store(callback, Ordering::Release);
    }

    pub fn snapshot(&self) -> [u32; 2] {
        [
            self.latch.load(Ordering::Acquire),
            self.callback.load(Ordering::Acquire),
        ]
    }

    /// Apply the measured 0x3C440 event-slot clear without addressing guest memory.
    pub fn clear_callback_when_latched(&self) -> bool {
        if self.latch.load(Ordering::Acquire) == 0 {
            return false;
        }

        self.latch.store(0, Ordering::Release);
        self.callback.store(0, Ordering::Release);
        true
    }

    /// Read the registered target for the measured post-simulation callback branch.
    pub fn callback_to_dispatch(&self) -> Option<u32> {
        let callback = self.callback.load(Ordering::Acquire);
        (callback != 0).then_some(callback)
    }

    pub fn read_access(&self, address: u32, width: u32) -> Option<u32> {
        let (index, byte_offset) = access_location(address, width, &Self::ADDRESSES)?;
        let value = self.snapshot()[index];
        Some((value >> (byte_offset * 8)) & width_mask(width)?)
    }

    pub fn write_access(&self, address: u32, width: u32, value: u32) -> bool {
        let Some((index, byte_offset)) = access_location(address, width, &Self::ADDRESSES) else {
            return false;
        };
        let Some(mask) = width_mask(width) else {
            return false;
        };
        let slot = if index == 0 {
            &self.latch
        } else {
            &self.callback
        };
        update_word(slot, byte_offset, mask, value);
        true
    }
}

impl FrameClockState {
    pub const ADDRESSES: [u32; 4] = [0x0074_97a8, 0x0074_97ac, 0x006b_7af0, 0x006b_7ad4];

    pub const fn new() -> Self {
        Self {
            sampled_vblank: AtomicU32::new(0),
            raw_vblank_step: AtomicU32::new(0),
            outer_frames: AtomicU32::new(0),
            frame_phase: AtomicU32::new(0),
        }
    }

    pub fn initialize(
        &self,
        sampled_vblank: u32,
        raw_vblank_step: u32,
        outer_frames: u32,
        frame_phase: u32,
    ) {
        self.sampled_vblank.store(sampled_vblank, Ordering::Release);
        self.raw_vblank_step
            .store(raw_vblank_step, Ordering::Release);
        self.outer_frames.store(outer_frames, Ordering::Release);
        self.frame_phase.store(frame_phase, Ordering::Release);
    }

    pub fn snapshot(&self) -> [u32; 4] {
        [
            self.sampled_vblank.load(Ordering::Acquire),
            self.raw_vblank_step.load(Ordering::Acquire),
            self.outer_frames.load(Ordering::Acquire),
            self.frame_phase.load(Ordering::Acquire),
        ]
    }

    pub fn sampled_vblank(&self) -> u32 {
        self.sampled_vblank.load(Ordering::Acquire)
    }

    pub fn raw_vblank_step(&self) -> u32 {
        self.raw_vblank_step.load(Ordering::Acquire)
    }

    pub fn record_sampled_vblank(&self, sample: u32) {
        self.sampled_vblank.store(sample, Ordering::Release);
    }

    pub fn record_raw_vblank_step(&self, step: u32) {
        self.raw_vblank_step.store(step, Ordering::Release);
    }

    /// Advance the measured outer-loop frame count without going through a
    /// guest-memory address. The title counter is a wrapping dword.
    pub fn advance_outer_frame(&self) -> u32 {
        self.outer_frames
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1)
    }

    /// Apply the measured frame-loop toggle `1 - previous` with dword wrap.
    pub fn toggle_frame_phase(&self) -> u32 {
        let previous = self.frame_phase.load(Ordering::Acquire);
        let next = 1u32.wrapping_sub(previous);
        self.frame_phase.store(next, Ordering::Release);
        next
    }

    pub fn read_access(&self, address: u32, width: u32) -> Option<u32> {
        let (index, byte_offset) = access_location(address, width, &Self::ADDRESSES)?;
        let value = self.snapshot()[index];
        Some((value >> (byte_offset * 8)) & width_mask(width)?)
    }

    pub fn write_access(&self, address: u32, width: u32, value: u32) -> bool {
        let Some((index, byte_offset)) = access_location(address, width, &Self::ADDRESSES) else {
            return false;
        };
        let Some(mask) = width_mask(width) else {
            return false;
        };
        let slot = match index {
            0 => &self.sampled_vblank,
            1 => &self.raw_vblank_step,
            2 => &self.outer_frames,
            3 => &self.frame_phase,
            _ => return false,
        };
        update_word(slot, byte_offset, mask, value);
        true
    }
}

/// Dispatch a guest word access directly to the Rust-owned frame-state record
/// selected by its guest address. The address table is the ABI boundary; this
/// avoids probing every frame-state record for ordinary guest-memory accesses.
pub fn read_owned_frame_word(
    counters: &FrameCounters,
    clock: &FrameClockState,
    step_outputs: &FrameStepOutputs,
    pause: &FramePauseState,
    timing_history: &FrameTimingHistory,
    events: &FrameEventState,
    address: u32,
    width: u32,
) -> Option<u32> {
    match address & !3 {
        0x004e_7940..=0x004e_794c => step_outputs.read_access(address, width),
        0x006b_7ad4 | 0x006b_7af0 | 0x0074_97a8 | 0x0074_97ac => clock.read_access(address, width),
        0x0074_97b0 | 0x0074_97b8 | 0x0074_97bc | 0x007d_e318 => {
            counters.read_access(address, width)
        }
        0x0074_97b4 => pause.read_access(address, width),
        0x007a_58a0..=0x007a_58f0 => timing_history.read_access(address, width),
        0x007e_b2e0 | 0x007e_b2e4 => events.read_access(address, width),
        _ => None,
    }
}

/// Write a guest word directly to the Rust-owned frame-state record selected
/// by its guest address.
pub fn write_owned_frame_word(
    counters: &FrameCounters,
    clock: &FrameClockState,
    step_outputs: &FrameStepOutputs,
    pause: &FramePauseState,
    timing_history: &FrameTimingHistory,
    events: &FrameEventState,
    address: u32,
    width: u32,
    value: u32,
) -> bool {
    match address & !3 {
        0x004e_7940..=0x004e_794c => step_outputs.write_access(address, width, value),
        0x006b_7ad4 | 0x006b_7af0 | 0x0074_97a8 | 0x0074_97ac => {
            clock.write_access(address, width, value)
        }
        0x0074_97b0 | 0x0074_97b8 | 0x0074_97bc | 0x007d_e318 => {
            counters.write_access(address, width, value)
        }
        0x0074_97b4 => pause.write_access(address, width, value),
        0x007a_58a0..=0x007a_58f0 => timing_history.write_access(address, width, value),
        0x007e_b2e0 | 0x007e_b2e4 => events.write_access(address, width, value),
        _ => false,
    }
}

fn access_location(address: u32, width: u32, addresses: &[u32]) -> Option<(usize, u32)> {
    if !matches!(width, 1 | 2 | 4) {
        return None;
    }
    let base = address & !3;
    let index = addresses.iter().position(|candidate| *candidate == base)?;
    let byte_offset = address & 3;
    (byte_offset + width <= 4).then_some((index, byte_offset))
}

fn width_mask(width: u32) -> Option<u32> {
    match width {
        1 => Some(0xff),
        2 => Some(0xffff),
        4 => Some(u32::MAX),
        _ => None,
    }
}

fn update_word(slot: &AtomicU32, byte_offset: u32, value_mask: u32, value: u32) {
    let shift = byte_offset * 8;
    let mask = value_mask << shift;
    let mut old = slot.load(Ordering::Acquire);
    loop {
        let new = (old & !mask) | ((value & value_mask) << shift);
        match slot.compare_exchange_weak(old, new, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(current) => old = current,
        }
    }
}

/// Owns the title's pause request independently of its guest-memory address.
/// The frame timing root consumes the resume sentinel and resets it to zero.
#[derive(Debug, Default)]
pub struct FramePauseState {
    word: AtomicU32,
}

impl FramePauseState {
    pub const ADDRESSES: [u32; 1] = [0x0074_97b4];

    pub const fn new() -> Self {
        Self {
            word: AtomicU32::new(0),
        }
    }

    /// Adopt the title's current pause word when native frame state is selected.
    pub fn initialize(&self, word: u32) {
        self.word.store(word, Ordering::Release);
    }

    /// Match 0x153AC0: only a zero-to-pause or nonzero-to-resume edge writes.
    /// Repeated acquires/releases preserve an already pending transition.
    pub fn set_from_flag(&self, requested: bool) -> u32 {
        let mut old = self.word.load(Ordering::Acquire);
        loop {
            let next = if requested {
                if old == 0 { 1 } else { old }
            } else if old != 0 {
                u32::MAX
            } else {
                old
            };
            match self
                .word
                .compare_exchange_weak(old, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return next,
                Err(current) => old = current,
            }
        }
    }

    /// The pause state machine consumes -1 as a resume request.
    pub fn clear_resume_request(&self) {
        self.word.store(0, Ordering::Release);
    }

    pub fn word(&self) -> u32 {
        self.word.load(Ordering::Acquire)
    }

    pub fn is_paused(&self) -> bool {
        self.word() != 0
    }

    /// Read the pause request through the typed guest-call seam.
    pub fn read_access(&self, address: u32, width: u32) -> Option<u32> {
        let (_, byte_offset) = access_location(address, width, &Self::ADDRESSES)?;
        Some((self.word() >> (byte_offset * 8)) & width_mask(width)?)
    }

    /// Apply scalar writes to the Rust-owned pause request.
    pub fn write_access(&self, address: u32, width: u32, value: u32) -> bool {
        let Some((_, byte_offset)) = access_location(address, width, &Self::ADDRESSES) else {
            return false;
        };
        let Some(mask) = width_mask(width) else {
            return false;
        };
        update_word(&self.word, byte_offset, mask, value);
        true
    }
}

/// The three five-sample histories used by the measured frame timing roots.
///
/// Slots are kept as raw dwords so float samples retain their exact bit
/// patterns. Only the Rust timing roots read or update these samples once the
/// native frame owner is selected.
#[derive(Debug, Default)]
pub struct FrameTimingHistory {
    words: [AtomicU32; 15],
}

/// Owns the four shared raw/effective simulation-step words used by the frame
/// root and lifted callers. Float words stay as raw bits, matching the guest
/// dword ABI without storing state in the guest image.
#[derive(Debug, Default)]
pub struct FrameStepOutputs {
    words: [AtomicU32; 4],
}

impl FrameStepOutputs {
    pub const WORD_COUNT: usize = 4;
    pub const ADDRESSES: [u32; Self::WORD_COUNT] = [0x004e7940, 0x004e7944, 0x004e7948, 0x004e794c];

    pub const fn new() -> Self {
        Self {
            words: [const { AtomicU32::new(0) }; Self::WORD_COUNT],
        }
    }

    /// Adopt the original dwords once at the opt-in ownership edge.
    pub fn initialize(&self, words: &[u32; Self::WORD_COUNT]) {
        for (slot, value) in self.words.iter().zip(words) {
            slot.store(*value, Ordering::Release);
        }
    }

    pub fn snapshot(&self) -> [u32; Self::WORD_COUNT] {
        core::array::from_fn(|index| self.words[index].load(Ordering::Acquire))
    }

    pub fn raw_step(&self) -> u32 {
        self.words[0].load(Ordering::Acquire)
    }

    pub fn effective_step(&self) -> u32 {
        self.words[1].load(Ordering::Acquire)
    }

    pub fn set_raw_step(&self, step: u32, float_bits: u32) {
        self.words[0].store(step, Ordering::Release);
        self.words[2].store(float_bits, Ordering::Release);
    }

    pub fn set_effective_step(&self, step: u32, float_bits: u32) {
        self.words[1].store(step, Ordering::Release);
        self.words[3].store(float_bits, Ordering::Release);
    }

    /// Read a byte, halfword, or dword view through the typed guest-call seam.
    /// Spans that cross a dword boundary are rejected and remain ordinary
    /// guest-memory operations.
    pub fn read_access(&self, address: u32, width: u32) -> Option<u32> {
        let (index, byte_offset) = Self::access_location(address, width)?;
        let value = self.words[index].load(Ordering::Acquire);
        let mask = Self::width_mask(width)?;
        Some((value >> (byte_offset * 8)) & mask)
    }

    /// Apply a byte, halfword, or dword write atomically to its typed owner.
    pub fn write_access(&self, address: u32, width: u32, value: u32) -> bool {
        let Some((index, byte_offset)) = Self::access_location(address, width) else {
            return false;
        };
        let Some(value_mask) = Self::width_mask(width) else {
            return false;
        };
        let shift = byte_offset * 8;
        let mask = value_mask << shift;
        let slot = &self.words[index];
        let mut old = slot.load(Ordering::Acquire);
        loop {
            let new = (old & !mask) | ((value & value_mask) << shift);
            match slot.compare_exchange_weak(old, new, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return true,
                Err(current) => old = current,
            }
        }
    }

    fn access_location(address: u32, width: u32) -> Option<(usize, u32)> {
        if !matches!(width, 1 | 2 | 4) {
            return None;
        }
        let base = address & !3;
        let index = Self::ADDRESSES
            .iter()
            .position(|candidate| *candidate == base)?;
        let byte_offset = address & 3;
        (byte_offset + width <= 4).then_some((index, byte_offset))
    }

    fn width_mask(width: u32) -> Option<u32> {
        match width {
            1 => Some(0xff),
            2 => Some(0xffff),
            4 => Some(u32::MAX),
            _ => None,
        }
    }
}

/// Sound-side effect requested by the frame pause state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameSoundTransition {
    Pause,
    Resume,
}

/// Values the ABI adapter needs after applying one raw vblank step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameStepTransition {
    /// Raw step used to advance the wrapping elapsed-frame counter.
    pub elapsed_step: u32,
    /// Title frame count after this update.
    pub frames: u32,
    /// Value left in ECX by the original pause branch before its sound call.
    pub pause_branch_value: u32,
    /// Float step left in the scalar XMM lane.
    pub effective_float_bits: u32,
    pub sound: Option<FrameSoundTransition>,
}

/// Apply the measured 0x1538C0 clamp and pause transition to Rust-owned frame
/// state. The caller supplies the float bits used by the original resume path.
/// The input is the positive signed vblank difference that exits the original
/// polling loop; keeping the clamp's signed comparison preserves its contract.
pub fn apply_native_frame_step(
    counters: &FrameCounters,
    pause: &FramePauseState,
    outputs: &FrameStepOutputs,
    vblank_difference: u32,
    one_step_float_bits: u32,
    mut sound_call: impl FnMut(FrameSoundTransition, u32, u32, u32, u32),
) -> FrameStepTransition {
    let raw_step = if (vblank_difference as i32) > 3 {
        3
    } else if vblank_difference == 0 {
        1
    } else {
        vblank_difference
    };
    let (frames, _) = counters.record_frame((raw_step as i32) > 1);
    let raw_float_bits = (raw_step as i32 as f32).to_bits();
    outputs.set_raw_step(raw_step, raw_float_bits);

    let pause_word = pause.word();
    let prior_effective_step = outputs.effective_step();
    let (elapsed_step, effective_float_bits, pause_branch_value, sound) = if pause_word == u32::MAX
    {
        outputs.set_raw_step(1, one_step_float_bits);
        outputs.set_effective_step(1, one_step_float_bits);
        pause.clear_resume_request();
        sound_call(
            FrameSoundTransition::Resume,
            1,
            pause_word,
            one_step_float_bits,
            frames,
        );
        (
            outputs.raw_step(),
            one_step_float_bits,
            pause_word,
            Some(FrameSoundTransition::Resume),
        )
    } else if pause_word == 0 {
        outputs.set_effective_step(raw_step, raw_float_bits);
        (raw_step, raw_float_bits, pause_word, None)
    } else {
        let sound = if prior_effective_step != 0 {
            sound_call(
                FrameSoundTransition::Pause,
                raw_step,
                prior_effective_step,
                raw_float_bits,
                frames,
            );
            Some(FrameSoundTransition::Pause)
        } else {
            None
        };
        outputs.set_effective_step(0, 0);
        (outputs.raw_step(), 0, prior_effective_step, sound)
    };

    counters.record_elapsed_step(elapsed_step);

    FrameStepTransition {
        elapsed_step,
        frames,
        pause_branch_value,
        effective_float_bits,
        sound,
    }
}

impl FrameTimingHistory {
    pub const WORD_COUNT: usize = 15;
    pub const ADDRESSES: [u32; Self::WORD_COUNT] = [
        0x007a_58a0,
        0x007a_58a4,
        0x007a_58a8,
        0x007a_58ac,
        0x007a_58b0,
        0x007a_58c0,
        0x007a_58c4,
        0x007a_58c8,
        0x007a_58cc,
        0x007a_58d0,
        0x007a_58e0,
        0x007a_58e4,
        0x007a_58e8,
        0x007a_58ec,
        0x007a_58f0,
    ];

    pub const fn new() -> Self {
        Self {
            words: [const { AtomicU32::new(0) }; Self::WORD_COUNT],
        }
    }

    /// Adopt the original five dwords from each history exactly once.
    pub fn initialize(&self, words: &[u32; Self::WORD_COUNT]) {
        for (slot, value) in self.words.iter().zip(words) {
            slot.store(*value, Ordering::Release);
        }
    }

    /// Shift each history's first four samples left, retaining its latest
    /// sample for the frame root to consume before writing the new one.
    pub fn shift_for_frame(&self) {
        for base in [0, 5, 10] {
            for index in 0..4 {
                let next = self.words[base + index + 1].load(Ordering::Acquire);
                self.words[base + index].store(next, Ordering::Release);
            }
        }
    }

    /// Last vblank history sample after `shift_for_frame`.
    pub fn previous_vblank(&self) -> u32 {
        self.words[3].load(Ordering::Acquire)
    }

    /// Latest accepted sample in the five-word vblank history.
    pub fn latest_vblank_sample(&self) -> u32 {
        self.words[4].load(Ordering::Acquire)
    }

    /// Latest RDTSC-derived float sample, represented by its original bits.
    pub fn latest_scaled_sample(&self) -> u32 {
        self.words[9].load(Ordering::Acquire)
    }

    /// Record the latest accepted vblank sample.
    pub fn record_vblank_sample(&self, sample: u32) {
        self.words[4].store(sample, Ordering::Release);
    }

    pub fn record_scaled_sample(&self, sample_bits: u32) {
        self.words[9].store(sample_bits, Ordering::Release);
    }

    pub fn record_tsc_sample(&self, sample_bits: u32) {
        self.words[14].store(sample_bits, Ordering::Release);
    }

    pub fn snapshot(&self) -> [u32; Self::WORD_COUNT] {
        core::array::from_fn(|index| self.words[index].load(Ordering::Acquire))
    }

    pub fn word(&self, index: usize) -> Option<u32> {
        self.words
            .get(index)
            .map(|word| word.load(Ordering::Acquire))
    }

    /// Read a raw history sample through the typed guest-call seam.
    pub fn read_access(&self, address: u32, width: u32) -> Option<u32> {
        let (index, byte_offset) = access_location(address, width, &Self::ADDRESSES)?;
        let value = self.words[index].load(Ordering::Acquire);
        Some((value >> (byte_offset * 8)) & width_mask(width)?)
    }

    /// Apply scalar writes to Rust-owned raw history words.
    pub fn write_access(&self, address: u32, width: u32, value: u32) -> bool {
        let Some((index, byte_offset)) = access_location(address, width, &Self::ADDRESSES) else {
            return false;
        };
        let Some(mask) = width_mask(width) else {
            return false;
        };
        update_word(&self.words[index], byte_offset, mask, value);
        true
    }
}

#[derive(Debug, Default)]
pub struct VblankCounter {
    delivered: AtomicU32,
}

impl VblankCounter {
    pub const fn new() -> Self {
        Self {
            delivered: AtomicU32::new(0),
        }
    }

    /// Seed the owner once when the native frame subsystem is selected.
    pub fn initialize(&self, delivered: u32) {
        self.delivered.store(delivered, Ordering::Release);
    }

    /// Record one delivered vblank using the title's wrapping dword arithmetic.
    pub fn record_delivery(&self) -> u32 {
        self.delivered
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1)
    }

    pub fn delivered(&self) -> u32 {
        self.delivered.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrameClockState, FrameCounters, FrameEventState, FramePauseState, FrameStepOutputs,
        FrameTimingHistory, VblankCounter, read_owned_frame_word, write_owned_frame_word,
    };

    #[test]
    fn owned_frame_word_dispatch_routes_by_guest_address() {
        let counters = FrameCounters::new();
        let clock = FrameClockState::new();
        let step = FrameStepOutputs::new();
        let pause = FramePauseState::new();
        let history = FrameTimingHistory::new();
        let events = FrameEventState::new();

        for (address, value) in [
            (0x0074_97b0, 11),
            (0x0074_97a8, 13),
            (0x004e_7940, 17),
            (0x0074_97b4, 19),
            (0x007a_58c0, 23),
            (0x007e_b2e4, 29),
        ] {
            assert!(write_owned_frame_word(
                &counters, &clock, &step, &pause, &history, &events, address, 4, value,
            ));
            assert_eq!(
                read_owned_frame_word(
                    &counters, &clock, &step, &pause, &history, &events, address, 4,
                ),
                Some(value),
            );
        }

        assert_eq!(
            read_owned_frame_word(
                &counters,
                &clock,
                &step,
                &pause,
                &history,
                &events,
                0x007a_58b4,
                4
            ),
            None,
        );
        assert!(!write_owned_frame_word(
            &counters,
            &clock,
            &step,
            &pause,
            &history,
            &events,
            0x007a_58b4,
            4,
            31,
        ));
    }

    #[test]
    fn pause_state_tracks_pause_and_consumes_resume_request() {
        let pause = FramePauseState::new();
        pause.initialize(0);
        assert_eq!(pause.set_from_flag(true), 1);
        assert!(pause.is_paused());
        assert_eq!(pause.set_from_flag(true), 1);
        assert_eq!(pause.set_from_flag(false), u32::MAX);
        // A repeated acquire cannot cancel the pending resume sentinel.
        assert_eq!(pause.set_from_flag(true), u32::MAX);
        // Nor can a repeated release change it before the timing root consumes it.
        assert_eq!(pause.set_from_flag(false), u32::MAX);
        pause.clear_resume_request();
        assert_eq!(pause.word(), 0);
        assert_eq!(pause.set_from_flag(false), 0);
        assert_eq!(pause.set_from_flag(true), 1);
    }

    #[test]
    fn pause_and_timing_history_scalar_accesses_stay_in_typed_owners() {
        let pause = FramePauseState::new();
        pause.initialize(0x1122_3344);
        assert_eq!(pause.read_access(0x0074_97b5, 1), Some(0x33));
        assert!(pause.write_access(0x0074_97b6, 2, 0xabcd));
        assert_eq!(pause.word(), 0xabcd_3344);

        let history = FrameTimingHistory::new();
        history.initialize(&core::array::from_fn(|index| 0x1000 + index as u32));
        assert_eq!(history.read_access(0x007a_58c0, 4), Some(0x1005));
        assert!(history.write_access(0x007a_58a1, 1, 0xfe));
        assert_eq!(history.word(0), Some(0x0000_fe00));
        assert!(!history.write_access(0x007a_58b2, 4, 0));
    }

    #[test]
    fn timing_histories_shift_four_slots_and_keep_raw_float_bits() {
        let history = FrameTimingHistory::new();
        history.initialize(&core::array::from_fn(|index| index as u32));

        history.shift_for_frame();

        assert_eq!(
            history.snapshot(),
            [1, 2, 3, 4, 4, 6, 7, 8, 9, 9, 11, 12, 13, 14, 14]
        );
        assert_eq!(history.previous_vblank(), 4);
        history.record_vblank_sample(0xFFFF_FFFE);
        history.record_scaled_sample(0x7FC0_1234);
        history.record_tsc_sample(0x8000_0001);
        assert_eq!(history.word(4), Some(0xFFFF_FFFE));
        assert_eq!(history.latest_scaled_sample(), 0x7FC0_1234);
        assert_eq!(history.word(14), Some(0x8000_0001));
        assert_eq!(history.word(15), None);
    }

    #[test]
    fn frame_clock_exposes_sample_and_raw_step_as_typed_values() {
        let clock = FrameClockState::new();
        clock.initialize(40, 2, 3, 0);

        clock.record_sampled_vblank(47);
        clock.record_raw_vblank_step(7);

        assert_eq!(clock.sampled_vblank(), 47);
        assert_eq!(clock.raw_vblank_step(), 7);
        assert_eq!(clock.snapshot(), [47, 7, 3, 0]);
    }

    #[test]
    fn vblank_counter_reads_and_wraps_as_a_guest_dword() {
        let counter = VblankCounter::new();
        counter.initialize(u32::MAX);
        assert_eq!(counter.record_delivery(), 0);
        assert_eq!(counter.delivered(), 0);
        assert_eq!(counter.record_delivery(), 1);
        assert_eq!(counter.delivered(), 1);
    }

    #[test]
    fn initialization_adopts_the_current_delivery_count() {
        let counter = VblankCounter::new();
        counter.initialize(37);
        assert_eq!(counter.delivered(), 37);
        assert_eq!(counter.record_delivery(), 38);
    }

    #[test]
    fn frame_counters_wrap_and_track_only_multi_vblank_steps() {
        let counters = FrameCounters::new();
        counters.initialize(0, u32::MAX, 7, 19);
        assert_eq!(counters.record_frame(false), (0, 7));
        assert_eq!(counters.record_frame(true), (1, 8));
        assert_eq!(counters.frames(), 1);
        assert_eq!(counters.multi_vblank_frames(), 8);
        assert_eq!(counters.simulation_stamp(), 19);
        counters.initialize(u32::MAX, 0, 0, u32::MAX);
        assert_eq!(counters.record_elapsed_step(2), 1);
        assert_eq!(counters.elapsed_frames(), 1);
        assert_eq!(counters.simulation_stamp(), u32::MAX);
    }

    #[test]
    fn frame_counter_words_share_the_typed_owner_for_lifted_accesses() {
        let counters = FrameCounters::new();
        counters.initialize(7, 11, 13, 17);
        assert_eq!(counters.read_access(0x0074_97b0, 4), Some(7));
        assert!(counters.write_access(0x0074_97b8, 2, 0x1234));
        assert_eq!(counters.read_access(0x0074_97b8, 4), Some(0x1234));
        assert_eq!(counters.read_access(0x0074_97b9, 1), Some(0x12));
        assert!(!counters.write_access(0x0074_97bb, 2, 0xffff));
        assert_eq!(counters.read_access(0x007d_e318, 4), Some(17));
        assert!(counters.write_access(0x007d_e318, 4, u32::MAX));
        assert_eq!(counters.read_access(0x007d_e318, 1), Some(0xff));
    }

    #[test]
    fn frame_clock_words_keep_sample_and_outer_counter_state_separate() {
        let partial_write_clock = FrameClockState::new();
        partial_write_clock.initialize(17, 19, u32::MAX, 0);
        assert!(partial_write_clock.write_access(0x006b_7af0, 1, 0x42));
        assert_eq!(
            partial_write_clock.read_access(0x006b_7af0, 4),
            Some(0xffff_ff42)
        );

        let clock = FrameClockState::new();
        clock.initialize(17, 19, u32::MAX, 0);
        assert_eq!(clock.snapshot(), [17, 19, u32::MAX, 0]);
        assert_eq!(clock.advance_outer_frame(), 0);
        assert_eq!(clock.snapshot(), [17, 19, 0, 0]);
        assert_eq!(clock.read_access(0x0074_97ac, 4), Some(19));
        assert_eq!(clock.read_access(0x006b_7af1, 4), None);
    }

    #[test]
    fn frame_phase_matches_the_measured_wrapping_toggle() {
        let clock = FrameClockState::new();
        clock.initialize(0, 0, 0, 0);
        assert_eq!(clock.toggle_frame_phase(), 1);
        assert_eq!(clock.toggle_frame_phase(), 0);
        clock.initialize(0, 0, 0, 7);
        assert_eq!(clock.toggle_frame_phase(), u32::MAX - 5);
        assert_eq!(clock.read_access(0x006b_7ad4, 4), Some(u32::MAX - 5));
    }

    #[test]
    fn measured_event_latch_and_callback_are_owned_without_a_queue_layout_claim() {
        let events = FrameEventState::new();
        events.initialize(1, 0x1234);
        assert_eq!(events.read_access(0x007e_b2e0, 4), Some(1));
        assert_eq!(events.read_access(0x007e_b2e4, 4), Some(0x1234));
        assert_eq!(events.callback_to_dispatch(), Some(0x1234));
        assert!(events.clear_callback_when_latched());
        assert_eq!(events.snapshot(), [0, 0]);
        assert!(!events.clear_callback_when_latched());
        events.initialize(0, 0x5678);
        assert_eq!(events.callback_to_dispatch(), Some(0x5678));
        assert!(!events.clear_callback_when_latched());
        assert_eq!(events.snapshot(), [0, 0x5678]);

        events.initialize(1, 0x1234);
        assert!(events.write_access(0x007e_b2e0, 1, 0));
        assert_eq!(events.snapshot(), [0, 0x1234]);
        assert!(!events.write_access(0x007e_b2e3, 2, 1));
    }

    #[test]
    fn step_outputs_virtualize_lifted_byte_and_dword_accesses() {
        let outputs = FrameStepOutputs::new();
        outputs.initialize(&[0x1122_3344, 2, 0x3f80_0000, 0x4000_0000]);
        assert_eq!(outputs.read_access(0x4e7940, 4), Some(0x1122_3344));
        assert_eq!(outputs.read_access(0x4e7941, 1), Some(0x33));
        assert_eq!(outputs.read_access(0x4e7942, 2), Some(0x1122));
        assert_eq!(outputs.read_access(0x4e7943, 2), None);
        assert_eq!(outputs.read_access(0x4e7940, 3), None);

        assert!(outputs.write_access(0x4e7941, 1, 0xaa));
        assert!(outputs.write_access(0x4e794a, 2, 0x1234));
        assert!(!outputs.write_access(0x4e7943, 2, 0xffff));
        assert_eq!(
            outputs.snapshot(),
            [0x1122_aa44, 2, 0x1234_0000, 0x4000_0000]
        );
        assert!(!outputs.write_access(0x4e7950, 4, 9));
    }
}

#[cfg(test)]
mod native_frame_step_tests {
    use super::{
        FrameCounters, FramePauseState, FrameSoundTransition, FrameStepOutputs,
        apply_native_frame_step,
    };

    #[test]
    fn running_step_clamps_and_advances_rust_owned_counts() {
        let counters = FrameCounters::new();
        counters.initialize(10, 40, 8, 0);
        let pause = FramePauseState::new();
        pause.initialize(0);
        let outputs = FrameStepOutputs::new();
        outputs.initialize(&[0, 2, 0, 2f32.to_bits()]);

        let result = apply_native_frame_step(
            &counters,
            &pause,
            &outputs,
            7,
            1f32.to_bits(),
            |_, _, _, _, _| panic!("running frame must not change sound pause state"),
        );

        assert_eq!(result.elapsed_step, 3);
        assert_eq!(result.frames, 41);
        assert_eq!(result.sound, None);
        assert_eq!(counters.elapsed_frames(), 13);
        assert_eq!(counters.multi_vblank_frames(), 9);
        assert_eq!(outputs.snapshot(), [3, 3, 3f32.to_bits(), 3f32.to_bits()]);
    }

    #[test]
    fn pause_sound_observes_old_effective_step_before_it_is_cleared() {
        let counters = FrameCounters::new();
        counters.initialize(5, 0, 0, 0);
        let pause = FramePauseState::new();
        pause.initialize(1);
        let outputs = FrameStepOutputs::new();
        outputs.initialize(&[1, 1, 1f32.to_bits(), 1f32.to_bits()]);

        let result = apply_native_frame_step(
            &counters,
            &pause,
            &outputs,
            2,
            1f32.to_bits(),
            |sound, eax, ecx, xmm, frames| {
                assert_eq!(sound, FrameSoundTransition::Pause);
                assert_eq!((eax, ecx, xmm, frames), (2, 1, 2f32.to_bits(), 1));
                assert_eq!(outputs.snapshot(), [2, 1, 2f32.to_bits(), 1f32.to_bits()]);
                assert_eq!(counters.elapsed_frames(), 5);
            },
        );

        assert_eq!(result.elapsed_step, 2);
        assert_eq!(result.pause_branch_value, 1);
        assert_eq!(result.effective_float_bits, 0);
        assert_eq!(result.sound, Some(FrameSoundTransition::Pause));
        assert_eq!(outputs.snapshot(), [2, 0, 2f32.to_bits(), 0]);
        assert_eq!(counters.elapsed_frames(), 7);
    }

    #[test]
    fn resume_state_is_owned_before_sound_and_counts_one_elapsed_step() {
        let counters = FrameCounters::new();
        counters.initialize(10, 0, 0, 0);
        let pause = FramePauseState::new();
        pause.initialize(u32::MAX);
        let outputs = FrameStepOutputs::new();
        outputs.initialize(&[3, 0, 3f32.to_bits(), 0]);
        let unit = 1f32.to_bits();

        let result = apply_native_frame_step(
            &counters,
            &pause,
            &outputs,
            3,
            unit,
            |sound, eax, ecx, xmm, frames| {
                assert_eq!(sound, FrameSoundTransition::Resume);
                assert_eq!((eax, ecx, xmm, frames), (1, u32::MAX, unit, 1));
                assert_eq!(pause.word(), 0);
                assert_eq!(outputs.snapshot(), [1, 1, unit, unit]);
                assert_eq!(counters.elapsed_frames(), 10);
            },
        );

        assert_eq!(result.elapsed_step, 1);
        assert_eq!(result.effective_float_bits, unit);
        assert_eq!(result.sound, Some(FrameSoundTransition::Resume));
        assert_eq!(counters.elapsed_frames(), 11);
        assert_eq!(counters.frames(), 1);
        assert_eq!(outputs.snapshot(), [1, 1, unit, unit]);
    }
}
