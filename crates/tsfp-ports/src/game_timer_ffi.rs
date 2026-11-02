//! T1670 C bridge for the readable engine/core timer owner.
//!
//! The game thread is serialized by this table lock. It is released before
//! callback dispatch so callbacks can add timers or enter another update.

use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::game_timer::{TimerCallback, TimerSnapshot, TimerStep, TimerTable};

struct LockedTable {
    held: AtomicBool,
    table: UnsafeCell<TimerTable>,
}

// The atomic guard serializes all table accesses. It is dropped before callbacks.
unsafe impl Sync for LockedTable {}

impl LockedTable {
    const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
            table: UnsafeCell::new(TimerTable::new()),
        }
    }

    fn lock(&self) -> TableGuard<'_> {
        while self
            .held
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }
        TableGuard { owner: self }
    }
}

struct TableGuard<'a> {
    owner: &'a LockedTable,
}

impl Deref for TableGuard<'_> {
    type Target = TimerTable;
    fn deref(&self) -> &Self::Target {
        // SAFETY: this guard owns the lock until Drop.
        unsafe { &*self.owner.table.get() }
    }
}

impl DerefMut for TableGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: this guard owns the lock until Drop.
        unsafe { &mut *self.owner.table.get() }
    }
}

impl Drop for TableGuard<'_> {
    fn drop(&mut self) {
        self.owner.held.store(false, Ordering::Release);
    }
}

static NATIVE_TIMERS: LockedTable = LockedTable::new();

/// Tier 0 host adapter. `stamp` and `scale` arrive as typed values from the
/// caller boundary; timer records remain exclusively Rust-owned.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_game_timer_add(
    mode: u32,
    seconds: f32,
    argument: u32,
    callback: u32,
    stamp: u32,
    scale: u32,
) -> i32 {
    NATIVE_TIMERS
        .lock()
        .add(mode, seconds, argument, callback, stamp, scale)
}

/// Tier 0 reset adapter for the original timer-table clear call.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_game_timer_clear() {
    NATIVE_TIMERS.lock().clear();
}

/// Tier 1 update using the original simulation stamp and a typed callback seam.
/// A differential test compares its live state and callbacks to lifted C2R
/// `0x165540` across 2048 deterministic timer tables; replay coverage is open.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_game_timer_update(stamp: u32, dispatch: TimerCallback) {
    let native_stamp = crate::frame_state_ffi::native_frame_state_selected();
    update_with_stamp_source(
        || {
            if native_stamp {
                crate::frame_state_ffi::native_frame_counters().simulation_stamp()
            } else {
                stamp
            }
        },
        dispatch,
    );
}

/// Mirror 0x165540's per-row read of `[0x7DE318]`. A callback can advance the
/// simulation clock before the dispatcher visits the next row, so the stamp
/// must be sampled again after each callback. The table lock is still released
/// before guest callback dispatch.
fn update_with_stamp_source(mut stamp: impl FnMut() -> u32, dispatch: TimerCallback) {
    let mut index = 0;
    loop {
        let step = {
            let mut table = NATIVE_TIMERS.lock();
            if index >= table.len() {
                break;
            }
            table.advance_one(index, stamp())
        };
        match step {
            TimerStep::Complete => break,
            TimerStep::Continue => {}
            TimerStep::Fire(fire) => {
                dispatch(fire.callback, fire.argument);
                if fire.one_shot {
                    NATIVE_TIMERS.lock().finish_one_shot(fire);
                } else {
                    NATIVE_TIMERS.lock().finish_periodic(fire);
                }
            }
        }
        index += 1;
    }
    NATIVE_TIMERS.lock().compact_finished();
}

/// Copy a read-only host diagnostic snapshot without publishing into guest
/// memory. The caller supplies space for all 75 records.
///
/// # Safety
/// Both output pointers must be valid for one `u32` and 75 `TimerSnapshot`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_game_timer_snapshot(
    count_out: *mut u32,
    entries_out: *mut TimerSnapshot,
) -> bool {
    if count_out.is_null() || entries_out.is_null() {
        return false;
    }
    let table = NATIVE_TIMERS.lock();
    unsafe {
        count_out.write(table.len() as u32);
        for index in 0..crate::game_timer::TIMER_CAPACITY {
            entries_out.add(index).write(table.snapshot(index));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::GuestMemory;
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    static DISPATCHES: AtomicUsize = AtomicUsize::new(0);
    static RESCHEDULE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static RESCHEDULE_ORDER: [AtomicU32; 2] = [AtomicU32::new(0), AtomicU32::new(0)];
    static RESCHEDULE_ADD_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);
    static CLEAR_CALLBACK_CALLS: AtomicUsize = AtomicUsize::new(0);
    static CLEAR_ADD_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);
    static LIFTED_CLEAR_ADD_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static DIFFERENTIAL_CALLS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
    static OBSERVED_NATIVE_ROWS: Mutex<Vec<(u32, u32, TimerSnapshot)>> = Mutex::new(Vec::new());
    static OBSERVED_LIFTED_ROWS: Mutex<Vec<(u32, u32, TimerSnapshot)>> = Mutex::new(Vec::new());
    static LIVE_STAMP_CALLS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
    static STAMP_WRITE_ACCEPTED: AtomicBool = AtomicBool::new(false);

    #[derive(Clone, Default)]
    struct TimerMemory(Arc<Mutex<BTreeMap<u32, u32>>>);

    impl GuestMemory for TimerMemory {
        fn read8(&self, address: u32) -> u8 {
            let base = address & !3;
            let shift = (address & 3) * 8;
            (self.0.lock().unwrap().get(&base).copied().unwrap_or(0) >> shift) as u8
        }

        fn write8(&mut self, address: u32, value: u8) {
            let base = address & !3;
            let shift = (address & 3) * 8;
            let mut words = self.0.lock().unwrap();
            let word = words.entry(base).or_default();
            *word = (*word & !(0xff << shift)) | (u32::from(value) << shift);
        }

        fn read32(&self, address: u32) -> u32 {
            self.0.lock().unwrap().get(&address).copied().unwrap_or(0)
        }

        fn write32(&mut self, address: u32, value: u32) {
            self.0.lock().unwrap().insert(address, value);
        }
    }

    struct TimerGuest {
        memory: TimerMemory,
        calls: Vec<(u32, u32)>,
    }

    static LIFTED_STAMP_MUTATION: AtomicBool = AtomicBool::new(false);

    impl crate::abi::GuestCall for TimerGuest {
        fn call(&mut self, va: u32, regs: &mut crate::abi::ExtRegisters) {
            let argument = self.memory.read32(regs.esp);
            self.calls.push((va, argument));
            if matches!(va, 0xA00 | 0xB00) {
                OBSERVED_LIFTED_ROWS.lock().unwrap().push((
                    va,
                    argument,
                    timer_snapshot_from_guest(&self.memory, argument as usize),
                ));
            }
            if va == 0xC00 && LIFTED_STAMP_MUTATION.load(Ordering::Acquire) {
                self.memory.write32(0x007d_e318, 5);
            }
            if va == 0x300 {
                let mut clear_regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
                crate::c2r::t2049_remote13_all_001::game_global_74c410_clear(
                    &mut self.memory,
                    &mut clear_regs,
                );

                const STACK: u32 = 0x0010_2000;
                self.memory.write32(STACK, 0);
                self.memory.write32(STACK + 4, 0); // one-shot mode
                self.memory.write32(STACK + 8, 1.0f32.to_bits());
                self.memory
                    .write32(STACK + 12, self.memory.read32(regs.esp) + 1);
                self.memory.write32(STACK + 16, 0x500);
                let mut add_regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
                add_regs.esp = STACK;
                crate::c2r::t2049_w7_005::game_timer_callback_table_7a20e0_add_entry_mode_0_oneshot_or_1_repeating_seconds_arg_strid(
                    &mut self.memory,
                    &mut add_regs,
                );
                LIFTED_CLEAR_ADD_RESULT.store(add_regs.eax, Ordering::Relaxed);
            }
            if va == 0x600 {
                let mut clear_regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
                crate::c2r::t2049_remote13_all_001::game_global_74c410_clear(
                    &mut self.memory,
                    &mut clear_regs,
                );

                const STACK: u32 = 0x0010_3000;
                self.memory.write32(STACK, 0);
                self.memory.write32(STACK + 4, 1); // repeating mode
                self.memory.write32(STACK + 8, 0.25f32.to_bits());
                self.memory
                    .write32(STACK + 12, self.memory.read32(regs.esp) + 1);
                self.memory.write32(STACK + 16, 0x700);
                let mut add_regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
                add_regs.esp = STACK;
                crate::c2r::t2049_w7_005::game_timer_callback_table_7a20e0_add_entry_mode_0_oneshot_or_1_repeating_seconds_arg_strid(
                    &mut self.memory,
                    &mut add_regs,
                );
                LIFTED_CLEAR_ADD_RESULT.store(add_regs.eax, Ordering::Relaxed);
            }
            if va == 0x100 {
                // Synthetic Tier 1 callback: exercise the measured live-count
                // reload by calling the lifted registration body during update.
                let mut memory = self.memory.clone();
                const STACK: u32 = 0x0010_1000;
                memory.write32(STACK, 0);
                memory.write32(STACK + 4, 0); // one-shot mode
                memory.write32(
                    STACK + 8,
                    REGISTERED_TIMER_DELAY_BITS.load(Ordering::Relaxed),
                );
                memory.write32(STACK + 12, self.memory.read32(regs.esp) + 1);
                memory.write32(STACK + 16, 0x200);
                let mut add_regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
                add_regs.esp = STACK;
                crate::c2r::t2049_w7_005::game_timer_callback_table_7a20e0_add_entry_mode_0_oneshot_or_1_repeating_seconds_arg_strid(
                    &mut memory,
                    &mut add_regs,
                );
            }
        }
    }

    extern "C" fn capture_native_callback(callback: u32, argument: u32) {
        DIFFERENTIAL_CALLS
            .lock()
            .unwrap()
            .push((callback, argument));
    }

    extern "C" fn observe_native_timer_state(callback: u32, argument: u32) {
        DIFFERENTIAL_CALLS
            .lock()
            .unwrap()
            .push((callback, argument));
        if matches!(callback, 0xA00 | 0xB00) {
            let snapshot = NATIVE_TIMERS.lock().snapshot(argument as usize);
            OBSERVED_NATIVE_ROWS
                .lock()
                .unwrap()
                .push((callback, argument, snapshot));
        }
    }

    static REGISTERED_TIMER_INDEX: AtomicU32 = AtomicU32::new(u32::MAX);
    static REGISTERED_TIMER_DELAY_BITS: AtomicU32 = AtomicU32::new(0);

    extern "C" fn register_due_timer_during_dispatch(callback: u32, argument: u32) {
        DIFFERENTIAL_CALLS
            .lock()
            .unwrap()
            .push((callback, argument));
        if callback == 0x100 {
            let seconds = f32::from_bits(REGISTERED_TIMER_DELAY_BITS.load(Ordering::Relaxed));
            let index = tsfp_game_timer_add(0, seconds, argument + 1, 0x200, 10, 60);
            REGISTERED_TIMER_INDEX.store(index as u32, Ordering::Relaxed);
        }
    }

    fn timer_snapshot_from_guest(memory: &TimerMemory, index: usize) -> TimerSnapshot {
        let base = 0x007a_20e0 + index as u32 * 0x18;
        TimerSnapshot {
            mode: memory.read32(base),
            elapsed: memory.read32(base + 4) as i32,
            stamp: memory.read32(base + 8),
            period_bits: memory.read32(base + 12),
            argument: memory.read32(base + 16),
            callback: memory.read32(base + 20),
        }
    }

    fn seed_lifted_memory_from_cleared_native_rows(memory: &mut TimerMemory) {
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;

        let native = NATIVE_TIMERS.lock();
        assert_eq!(native.len(), 0);
        let rows = native.snapshots();
        drop(native);

        for (index, row) in rows.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            memory.write32(base, row.mode);
            memory.write32(base + 4, row.elapsed as u32);
            memory.write32(base + 8, row.stamp);
            memory.write32(base + 12, row.period_bits);
            memory.write32(base + 16, row.argument);
            memory.write32(base + 20, row.callback);
        }
        memory.write32(COUNT, 0);
    }

    fn compare_timer_registration(
        memory: &mut TimerMemory,
        mode: u32,
        seconds: f32,
        argument: u32,
        callback: u32,
        stamp: u32,
    ) -> i32 {
        compare_timer_registration_with_scale(
            memory, mode, seconds, argument, callback, stamp, 1000,
        )
    }

    fn compare_timer_registration_with_scale(
        memory: &mut TimerMemory,
        mode: u32,
        seconds: f32,
        argument: u32,
        callback: u32,
        stamp: u32,
        scale: u32,
    ) -> i32 {
        const STACK: u32 = 0x0010_4000;
        const COUNT: u32 = 0x0074_c410;
        const STAMP: u32 = 0x007d_e318;
        const SCALE: u32 = 0x004e_8cec;

        memory.write32(STACK, 0);
        memory.write32(STACK + 4, mode);
        memory.write32(STACK + 8, seconds.to_bits());
        memory.write32(STACK + 12, argument);
        memory.write32(STACK + 16, callback);
        memory.write32(STAMP, stamp);
        memory.write32(SCALE, scale);

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = STACK;
        crate::c2r::t2049_w7_005::game_timer_callback_table_7a20e0_add_entry_mode_0_oneshot_or_1_repeating_seconds_arg_strid(
            memory,
            &mut regs,
        );
        let lifted_index = regs.eax as i32;
        let native_index = tsfp_game_timer_add(mode, seconds, argument, callback, stamp, scale);
        assert_eq!(native_index, lifted_index);
        assert_eq!(NATIVE_TIMERS.lock().len() as u32, memory.read32(COUNT));
        for index in 0..crate::game_timer::TIMER_CAPACITY {
            assert_eq!(
                NATIVE_TIMERS.lock().snapshot(index),
                timer_snapshot_from_guest(memory, index),
                "row {index}, registration result {native_index}"
            );
        }
        native_index
    }

    fn next_seed(seed: &mut u32) -> u32 {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *seed
    }

    extern "C" fn count_callback(_: u32, _: u32) {
        DISPATCHES.fetch_add(1, Ordering::Relaxed);
    }

    extern "C" fn advance_live_stamp_during_dispatch(callback: u32, argument: u32) {
        LIVE_STAMP_CALLS.lock().unwrap().push((callback, argument));
        if argument == 1 {
            STAMP_WRITE_ACCEPTED.store(
                crate::frame_state_ffi::native_frame_owned_word_write(0x007d_e318, 4, 5),
                Ordering::Release,
            );
        }
    }

    extern "C" fn advance_owned_stamp_for_differential(callback: u32, argument: u32) {
        DIFFERENTIAL_CALLS
            .lock()
            .unwrap()
            .push((callback, argument));
        if callback == 0xC00 {
            STAMP_WRITE_ACCEPTED.store(
                crate::frame_state_ffi::native_frame_owned_word_write(0x007d_e318, 4, 5),
                Ordering::Release,
            );
        }
    }

    extern "C" fn reschedule_during_dispatch(callback: u32, argument: u32) {
        let index = RESCHEDULE_CALLS.fetch_add(1, Ordering::Relaxed);
        if index < RESCHEDULE_ORDER.len() {
            RESCHEDULE_ORDER[index].store(callback, Ordering::Relaxed);
        }
        if callback == 0x100 {
            let result = tsfp_game_timer_add(0, 0.0, argument + 1, 0x200, 10, 60);
            RESCHEDULE_ADD_RESULT.store(result as u32, Ordering::Relaxed);
        }
    }

    extern "C" fn clear_and_reuse_timer_slot(callback: u32, argument: u32) {
        DIFFERENTIAL_CALLS
            .lock()
            .unwrap()
            .push((callback, argument));
        if callback == 0x300 {
            CLEAR_CALLBACK_CALLS.fetch_add(1, Ordering::Relaxed);
            tsfp_game_timer_clear();
            let result = tsfp_game_timer_add(0, 1.0, argument + 1, 0x500, 10, 60);
            CLEAR_ADD_RESULT.store(result as u32, Ordering::Relaxed);
        }
    }

    extern "C" fn periodic_clear_and_reuse_timer_slot(callback: u32, argument: u32) {
        DIFFERENTIAL_CALLS
            .lock()
            .unwrap()
            .push((callback, argument));
        if callback == 0x600 {
            tsfp_game_timer_clear();
            let result = tsfp_game_timer_add(1, 0.25, argument + 1, 0x700, 12, 4);
            CLEAR_ADD_RESULT.store(result as u32, Ordering::Relaxed);
        }
    }

    #[test]
    fn empty_update_is_native_no_op() {
        let _guard = TEST_LOCK.lock().unwrap();
        tsfp_game_timer_clear();
        DISPATCHES.store(0, Ordering::Relaxed);

        tsfp_game_timer_update(0, count_callback);

        assert_eq!(NATIVE_TIMERS.lock().len(), 0);
        assert_eq!(DISPATCHES.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn callback_dispatch_crosses_the_typed_seam() {
        let _guard = TEST_LOCK.lock().unwrap();
        tsfp_game_timer_clear();
        DISPATCHES.store(0, Ordering::Relaxed);

        assert_eq!(tsfp_game_timer_add(0, 0.0, 7, 0x1234, 10, 60), 0);
        // Mode-0 deadlines are strict: a zero countdown needs a positive stamp delta.
        tsfp_game_timer_update(11, count_callback);

        assert_eq!(DISPATCHES.load(Ordering::Relaxed), 1);
        assert_eq!(NATIVE_TIMERS.lock().len(), 0);
    }

    #[test]
    fn later_timer_rows_read_the_stamp_after_callback_mutation() {
        let _guard = TEST_LOCK.lock().unwrap();
        let frame_seed = crate::frame_state_ffi::FrameStateSeed {
            vblank_count: 0,
            elapsed_frames: 0,
            frames: 0,
            multi_vblank_frames: 0,
            simulation_stamp: 1,
            sampled_vblank: 0,
            raw_vblank_step: 0,
            outer_frames: 0,
            frame_phase: 0,
            event_latch: 0,
            event_callback: 0,
            pause_word: 0,
            timing_history: [0; crate::frame_state::FrameTimingHistory::WORD_COUNT],
            step_outputs: [0; crate::frame_state::FrameStepOutputs::WORD_COUNT],
        };
        assert!(unsafe { crate::frame_state_ffi::tsfp_frame_state_select_native(&frame_seed) });
        NATIVE_TIMERS.lock().clear();
        assert_eq!(NATIVE_TIMERS.lock().add(0, 0.0, 1, 0x100, 0, 1), 0);
        assert_eq!(NATIVE_TIMERS.lock().add(0, 4.0, 2, 0x200, 0, 1), 1);
        STAMP_WRITE_ACCEPTED.store(false, Ordering::Release);
        LIVE_STAMP_CALLS.lock().unwrap().clear();

        tsfp_game_timer_update(0, advance_live_stamp_during_dispatch);

        assert!(STAMP_WRITE_ACCEPTED.load(Ordering::Acquire));
        assert_eq!(*LIVE_STAMP_CALLS.lock().unwrap(), [(0x100, 1), (0x200, 2)]);
        assert_eq!(NATIVE_TIMERS.lock().len(), 0);
    }

    /// Tier 1 synthetic differential for the measured per-row clock read:
    /// callback C00 advances the native frame owner or lifted guest stamp,
    /// and the later due row must observe that new value in the same update.
    /// This verifies the dispatcher contract, not natural game callback use.
    #[test]
    fn native_timer_resamples_live_stamp_like_lifted_dispatcher() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;

        let frame_seed = crate::frame_state_ffi::FrameStateSeed {
            vblank_count: 0,
            elapsed_frames: 0,
            frames: 0,
            multi_vblank_frames: 0,
            simulation_stamp: 1,
            sampled_vblank: 0,
            raw_vblank_step: 0,
            outer_frames: 0,
            frame_phase: 0,
            event_latch: 0,
            event_callback: 0,
            pause_word: 0,
            timing_history: [0; crate::frame_state::FrameTimingHistory::WORD_COUNT],
            step_outputs: [0; crate::frame_state::FrameStepOutputs::WORD_COUNT],
        };
        assert!(unsafe { crate::frame_state_ffi::tsfp_frame_state_select_native(&frame_seed) });

        tsfp_game_timer_clear();
        assert_eq!(tsfp_game_timer_add(0, 0.0, 1, 0xC00, 0, 1), 0);
        assert_eq!(tsfp_game_timer_add(0, 4.0, 2, 0xD00, 0, 1), 1);
        let before = NATIVE_TIMERS.lock().snapshots();
        let mut lifted_memory = TimerMemory::default();
        lifted_memory.write32(COUNT, 2);
        lifted_memory.write32(STAMP, 1);
        for (index, entry) in before.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            lifted_memory.write32(base, entry.mode);
            lifted_memory.write32(base + 4, entry.elapsed as u32);
            lifted_memory.write32(base + 8, entry.stamp);
            lifted_memory.write32(base + 12, entry.period_bits);
            lifted_memory.write32(base + 16, entry.argument);
            lifted_memory.write32(base + 20, entry.callback);
        }

        DIFFERENTIAL_CALLS.lock().unwrap().clear();
        STAMP_WRITE_ACCEPTED.store(false, Ordering::Release);
        tsfp_game_timer_update(0, advance_owned_stamp_for_differential);
        let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
        let native = NATIVE_TIMERS.lock();
        let native_count = native.len();
        let native_entries = native.snapshots();
        drop(native);

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x0010_0000;
        let mut lifted = TimerGuest {
            memory: lifted_memory.clone(),
            calls: Vec::new(),
        };
        LIFTED_STAMP_MUTATION.store(true, Ordering::Release);
        crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
            &mut lifted_memory,
            &mut lifted,
            &mut regs,
        );
        LIFTED_STAMP_MUTATION.store(false, Ordering::Release);

        assert!(STAMP_WRITE_ACCEPTED.load(Ordering::Acquire));
        assert_eq!(native_calls, [(0xC00, 1), (0xD00, 2)]);
        assert_eq!(lifted.calls, native_calls);
        assert_eq!(lifted_memory.read32(STAMP), 5);
        assert_eq!(lifted_memory.read32(COUNT) as usize, native_count);
        assert_eq!(native_count, 0);
        for (index, expected) in native_entries.iter().enumerate() {
            assert_eq!(
                timer_snapshot_from_guest(&lifted_memory, index),
                *expected,
                "live-stamp differential row {index}"
            );
        }
        tsfp_game_timer_clear();
    }

    #[test]
    fn callback_added_timer_is_visited_before_finished_rows_compact() {
        let _guard = TEST_LOCK.lock().unwrap();
        tsfp_game_timer_clear();
        RESCHEDULE_CALLS.store(0, Ordering::Relaxed);
        for callback in &RESCHEDULE_ORDER {
            callback.store(0, Ordering::Relaxed);
        }
        RESCHEDULE_ADD_RESULT.store(u32::MAX, Ordering::Relaxed);

        assert_eq!(tsfp_game_timer_add(0, 0.0, 7, 0x100, 9, 60), 0);
        tsfp_game_timer_update(10, reschedule_during_dispatch);

        assert_eq!(RESCHEDULE_ADD_RESULT.load(Ordering::Relaxed), 1);
        assert_eq!(RESCHEDULE_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(
            [
                RESCHEDULE_ORDER[0].load(Ordering::Relaxed),
                RESCHEDULE_ORDER[1].load(Ordering::Relaxed)
            ],
            [0x100, 0]
        );
        assert_eq!(NATIVE_TIMERS.lock().len(), 1);

        tsfp_game_timer_update(11, reschedule_during_dispatch);
        assert_eq!(RESCHEDULE_CALLS.load(Ordering::Relaxed), 2);
        assert_eq!(RESCHEDULE_ORDER[1].load(Ordering::Relaxed), 0x200);
        assert_eq!(NATIVE_TIMERS.lock().len(), 0);
    }

    #[test]
    fn callback_clear_and_reuse_finishes_the_reused_slot() {
        let _guard = TEST_LOCK.lock().unwrap();
        tsfp_game_timer_clear();
        CLEAR_CALLBACK_CALLS.store(0, Ordering::Relaxed);
        CLEAR_ADD_RESULT.store(u32::MAX, Ordering::Relaxed);

        assert_eq!(tsfp_game_timer_add(0, 0.0, 7, 0x300, 10, 60), 0);
        assert_eq!(tsfp_game_timer_add(0, 0.0, 8, 0x400, 10, 60), 1);
        // This callback-mutation case exercises an already-due timer; keep a
        // positive stamp delta under the strict mode-0 deadline rule.
        tsfp_game_timer_update(11, clear_and_reuse_timer_slot);

        assert_eq!(CLEAR_CALLBACK_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(CLEAR_ADD_RESULT.load(Ordering::Relaxed), 0);
        {
            let table = NATIVE_TIMERS.lock();
            assert_eq!(table.len(), 0);
            assert_eq!(table.snapshot(0).mode, 2);
            assert_eq!(table.snapshot(0).callback, 0x500);
            assert_eq!(table.snapshot(0).argument, 8);
        }
    }

    #[test]
    fn diagnostic_snapshot_is_host_owned_and_copies_all_slots() {
        let _guard = TEST_LOCK.lock().unwrap();
        tsfp_game_timer_clear();
        for index in 0..crate::game_timer::TIMER_CAPACITY {
            let argument = 0xAA00 + index as u32;
            let callback = 0x1234 + index as u32;
            assert_eq!(
                tsfp_game_timer_add(1, 0.25, argument, callback, 10, 100),
                index as i32
            );
        }
        let mut count = u32::MAX;
        let mut entries = [TimerSnapshot::default(); crate::game_timer::TIMER_CAPACITY];
        assert!(unsafe { tsfp_game_timer_snapshot(&mut count, entries.as_mut_ptr()) });
        assert_eq!(count as usize, crate::game_timer::TIMER_CAPACITY);
        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(
                *entry,
                TimerSnapshot {
                    mode: 1,
                    elapsed: 0,
                    stamp: 10,
                    period_bits: 25.0f32.to_bits(),
                    argument: 0xAA00 + index as u32,
                    callback: 0x1234 + index as u32,
                }
            );
        }
        tsfp_game_timer_clear();
    }

    /// Tier 1 registration differential against lifted C2R 0x1654A0. This
    /// covers the measured 75-row limit, fields retained by nonstandard
    /// modes when a cleared slot is reused, and the full-table failure path.
    #[test]
    fn native_timer_registration_matches_lifted_through_capacity_and_reuse() {
        let _guard = TEST_LOCK.lock().unwrap();
        tsfp_game_timer_clear();

        let mut memory = TimerMemory::default();
        // `clear` retains inactive slot bytes; match them before the first
        // differential registration so test order cannot affect row fields.
        seed_lifted_memory_from_cleared_native_rows(&mut memory);

        // Seed every slot so both implementations start reuse from the same
        // retained row values; clear only the measured live count.
        for index in 0..crate::game_timer::TIMER_CAPACITY {
            assert_eq!(
                compare_timer_registration(
                    &mut memory,
                    1,
                    0.125,
                    0xA000 + index as u32,
                    0xB000 + index as u32,
                    0x1000 + index as u32,
                ),
                index as i32
            );
        }
        tsfp_game_timer_clear();
        let mut clear_regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        crate::c2r::t2049_remote13_all_001::game_global_74c410_clear(&mut memory, &mut clear_regs);

        for index in 0..crate::game_timer::TIMER_CAPACITY {
            let mode = [0, 1, 2, 0, 1][index % 5];
            let seconds = ((index + 1) as f32) / 64.0;
            assert_eq!(
                compare_timer_registration(
                    &mut memory,
                    mode,
                    seconds,
                    0xC000 + index as u32,
                    0xD000 + index as u32,
                    0x2000 + index as u32,
                ),
                index as i32
            );
        }

        assert_eq!(
            compare_timer_registration(&mut memory, 1, 0.5, 0xEEEE, 0xFFFF, 0x3000),
            -1
        );
    }

    /// Tier 1 registration differential for the x86 float-to-i32 conversion
    /// boundary, including invalid-conversion results and truncation toward zero.
    #[test]
    fn native_timer_registration_matches_lifted_conversion_boundaries() {
        let _guard = TEST_LOCK.lock().unwrap();
        tsfp_game_timer_clear();

        // Match the process-global native table's stale, intentionally
        // retained slot fields in the lifted fixture before seeding. Tests
        // share this owner across cases, and registration writes period only
        // for mode 1, so even the seeding pass must begin from identical rows.
        let mut memory = TimerMemory::default();
        seed_lifted_memory_from_cleared_native_rows(&mut memory);
        for index in 0..crate::game_timer::TIMER_CAPACITY {
            assert_eq!(
                compare_timer_registration(
                    &mut memory,
                    1,
                    index as f32 + 0.25,
                    0xD000 + index as u32,
                    0xD800 + index as u32,
                    0x3000 + index as u32,
                ),
                index as i32
            );
        }
        tsfp_game_timer_clear();
        let mut clear_regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        crate::c2r::t2049_remote13_all_001::game_global_74c410_clear(&mut memory, &mut clear_regs);

        let cases = [
            (f32::NAN, i32::MIN),
            (f32::INFINITY, i32::MIN),
            (f32::NEG_INFINITY, i32::MIN),
            (2_147_483_648.0, i32::MIN),
            (f32::from_bits(0x4eff_ffff), 2_147_483_520),
            (-2_147_483_648.0, i32::MIN),
            (f32::from_bits(0xcf00_0001), i32::MIN),
            (f32::from_bits(0xceff_ffff), -2_147_483_520),
            (-1.75, -1),
            (1.75, 1),
            (-0.75, 0),
        ];
        for (index, (seconds, expected_elapsed)) in cases.into_iter().enumerate() {
            assert_eq!(
                compare_timer_registration_with_scale(
                    &mut memory,
                    0,
                    seconds,
                    0xE000 + index as u32,
                    0xF000 + index as u32,
                    0x4000 + index as u32,
                    1,
                ),
                index as i32
            );
            assert_eq!(
                NATIVE_TIMERS.lock().snapshot(index).elapsed,
                expected_elapsed,
                "scaled seconds {seconds:?}"
            );
        }
        tsfp_game_timer_clear();
    }

    /// Tier 1 differential against lifted update, excluding the measured strict zero deadline.
    #[test]
    fn native_timer_update_matches_lifted_body_except_strict_zero_deadlines() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;
        const SCALES: [u32; 4] = [30, 50, 60, 120];
        const SECONDS: [f32; 8] = [-1.0, -0.25, 0.0, 0.0625, 0.25, 0.5, 1.0, 3.0];

        let mut seed = 0x2409_1655;
        let mut compared_cases = 0;
        let mut strict_deadline_cases = 0;
        for case in 0..2048 {
            tsfp_game_timer_clear();
            let start_stamp = next_seed(&mut seed);
            let scale = SCALES[(next_seed(&mut seed) as usize) % SCALES.len()];
            let entries = (next_seed(&mut seed) as usize) % (crate::game_timer::TIMER_CAPACITY + 1);
            for index in 0..entries {
                let mode = next_seed(&mut seed) % 4;
                let seconds = SECONDS[(next_seed(&mut seed) as usize) % SECONDS.len()];
                let argument = next_seed(&mut seed);
                let callback = 0x0010_0000 + index as u32 * 4;
                assert_eq!(
                    tsfp_game_timer_add(mode, seconds, argument, callback, start_stamp, scale),
                    index as i32,
                    "seed case {case} add {index}"
                );
            }

            let step = next_seed(&mut seed) % 512;
            let update_stamp = start_stamp.wrapping_add(step);
            let before = NATIVE_TIMERS.lock().snapshots();
            let strict_deadline = before
                .iter()
                .take(entries)
                .any(|entry| entry.mode == 0 && entry.elapsed.wrapping_sub(step as i32) == 0);
            let mut lifted_memory = TimerMemory::default();
            lifted_memory.write32(COUNT, entries as u32);
            lifted_memory.write32(STAMP, update_stamp);
            for (index, entry) in before.iter().enumerate() {
                let base = TABLE + index as u32 * 0x18;
                lifted_memory.write32(base, entry.mode);
                lifted_memory.write32(base + 4, entry.elapsed as u32);
                lifted_memory.write32(base + 8, entry.stamp);
                lifted_memory.write32(base + 12, entry.period_bits);
                lifted_memory.write32(base + 16, entry.argument);
                lifted_memory.write32(base + 20, entry.callback);
            }

            DIFFERENTIAL_CALLS.lock().unwrap().clear();
            tsfp_game_timer_update(update_stamp, capture_native_callback);
            let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
            let native = NATIVE_TIMERS.lock();
            let native_count = native.len();
            let native_entries = native.snapshots();
            drop(native);

            let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
            regs.esp = 0x0010_0000;
            let mut lifted = TimerGuest {
                memory: lifted_memory.clone(),
                calls: Vec::new(),
            };
            crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
                &mut lifted_memory,
                &mut lifted,
                &mut regs,
            );

            if strict_deadline {
                // The carrier specifies `< 0`; the current lifted body uses
                // an inclusive deadline at zero. The dedicated boundary test
                // covers that known exception, so this differential skips it.
                strict_deadline_cases += 1;
                continue;
            }

            assert_eq!(
                lifted_memory.read32(COUNT) as usize,
                native_count,
                "seed case {case} live count"
            );
            assert_eq!(
                lifted.calls, native_calls,
                "seed case {case} callback order/arguments"
            );
            for (index, expected) in native_entries.iter().enumerate() {
                assert_eq!(
                    timer_snapshot_from_guest(&lifted_memory, index),
                    *expected,
                    "seed case {case} timer row {index}"
                );
            }
            compared_cases += 1;
        }
        assert_eq!(compared_cases + strict_deadline_cases, 2048);
        assert!(compared_cases > 0);
        assert!(strict_deadline_cases > 0);
        tsfp_game_timer_clear();
    }

    /// Tier 1 synthetic differential for the mapped timer table's due-row
    /// order: simultaneously due rows are dispatched in registration order
    /// while they remain live. Runtime callback order is still unconfirmed.
    #[test]
    fn native_timer_matches_lifted_zero_countdown_after_positive_delta_order() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;
        const DUE: [(u32, u32); 3] = [(0x110, 0xA1), (0x120, 0xB2), (0x130, 0xC3)];

        tsfp_game_timer_clear();
        for (index, (callback, argument)) in DUE.into_iter().enumerate() {
            assert_eq!(
                tsfp_game_timer_add(0, 0.0, argument, callback, 10, 60),
                index as i32
            );
        }

        let before = NATIVE_TIMERS.lock().snapshots();
        let mut lifted_memory = TimerMemory::default();
        lifted_memory.write32(COUNT, DUE.len() as u32);
        lifted_memory.write32(STAMP, 11);
        for (index, entry) in before.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            lifted_memory.write32(base, entry.mode);
            lifted_memory.write32(base + 4, entry.elapsed as u32);
            lifted_memory.write32(base + 8, entry.stamp);
            lifted_memory.write32(base + 12, entry.period_bits);
            lifted_memory.write32(base + 16, entry.argument);
            lifted_memory.write32(base + 20, entry.callback);
        }

        DIFFERENTIAL_CALLS.lock().unwrap().clear();
        tsfp_game_timer_update(11, capture_native_callback);
        let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x0010_0000;
        let mut lifted = TimerGuest {
            memory: lifted_memory.clone(),
            calls: Vec::new(),
        };
        crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
            &mut lifted_memory,
            &mut lifted,
            &mut regs,
        );

        let expected: Vec<_> = DUE.into_iter().collect();
        assert_eq!(native_calls, expected);
        assert_eq!(lifted.calls, native_calls);
        assert_eq!(lifted_memory.read32(COUNT), 0);
        tsfp_game_timer_clear();
    }

    /// Tier 1 differential for state visible at callback entry. The REA source
    /// says each row's stamp and elapsed value are written before its callback,
    /// while a repeating timer's period subtraction happens after callback return.
    #[test]
    fn native_timer_matches_lifted_callback_visible_row_state() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;

        tsfp_game_timer_clear();
        assert_eq!(tsfp_game_timer_add(1, 0.1, 0, 0xA00, 100, 100), 0);
        assert_eq!(tsfp_game_timer_add(0, 0.09, 1, 0xB00, 100, 100), 1);

        let before = NATIVE_TIMERS.lock().snapshots();
        let mut lifted_memory = TimerMemory::default();
        lifted_memory.write32(COUNT, 2);
        lifted_memory.write32(STAMP, 110);
        for (index, entry) in before.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            lifted_memory.write32(base, entry.mode);
            lifted_memory.write32(base + 4, entry.elapsed as u32);
            lifted_memory.write32(base + 8, entry.stamp);
            lifted_memory.write32(base + 12, entry.period_bits);
            lifted_memory.write32(base + 16, entry.argument);
            lifted_memory.write32(base + 20, entry.callback);
        }

        DIFFERENTIAL_CALLS.lock().unwrap().clear();
        OBSERVED_NATIVE_ROWS.lock().unwrap().clear();
        OBSERVED_LIFTED_ROWS.lock().unwrap().clear();
        tsfp_game_timer_update(110, observe_native_timer_state);
        let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
        let native_observed = OBSERVED_NATIVE_ROWS.lock().unwrap().clone();
        let native = NATIVE_TIMERS.lock();
        let native_count = native.len();
        let native_entries = native.snapshots();
        drop(native);

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x0010_0000;
        let mut lifted = TimerGuest {
            memory: lifted_memory.clone(),
            calls: Vec::new(),
        };
        crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
            &mut lifted_memory,
            &mut lifted,
            &mut regs,
        );

        assert_eq!(native_calls, [(0xA00, 0), (0xB00, 1)]);
        assert_eq!(lifted.calls, native_calls);
        assert_eq!(
            native_observed,
            [
                (
                    0xA00,
                    0,
                    TimerSnapshot {
                        mode: 1,
                        elapsed: 10,
                        stamp: 110,
                        period_bits: 10.0f32.to_bits(),
                        argument: 0,
                        callback: 0xA00,
                    },
                ),
                (
                    0xB00,
                    1,
                    TimerSnapshot {
                        mode: 0,
                        elapsed: -1,
                        stamp: 110,
                        period_bits: before[1].period_bits,
                        argument: 1,
                        callback: 0xB00,
                    },
                ),
            ]
        );
        assert_eq!(*OBSERVED_LIFTED_ROWS.lock().unwrap(), native_observed);
        assert_eq!(lifted_memory.read32(COUNT) as usize, native_count);
        for (index, expected) in native_entries.iter().enumerate() {
            assert_eq!(
                timer_snapshot_from_guest(&lifted_memory, index),
                *expected,
                "timer row {index} after callback dispatch"
            );
        }
        tsfp_game_timer_clear();
    }

    /// Source-contract check for strict mode-0 deadlines. The measured carrier
    /// says delta-zero rows are not due; current lifted C2R still treats them as
    /// due, so the comparison records that known disagreement explicitly.
    #[test]
    fn native_timer_zero_delay_rearm_waits_for_positive_delta() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;
        const SCALE: u32 = 0x004e_8cec;

        tsfp_game_timer_clear();
        assert_eq!(tsfp_game_timer_add(0, 0.0, 7, 0x100, 9, 60), 0);
        let before = NATIVE_TIMERS.lock().snapshots();
        let mut lifted_memory = TimerMemory::default();
        lifted_memory.write32(COUNT, 1);
        lifted_memory.write32(STAMP, 10);
        lifted_memory.write32(SCALE, 60);
        for (index, entry) in before.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            lifted_memory.write32(base, entry.mode);
            lifted_memory.write32(base + 4, entry.elapsed as u32);
            lifted_memory.write32(base + 8, entry.stamp);
            lifted_memory.write32(base + 12, entry.period_bits);
            lifted_memory.write32(base + 16, entry.argument);
            lifted_memory.write32(base + 20, entry.callback);
        }

        REGISTERED_TIMER_INDEX.store(u32::MAX, Ordering::Relaxed);
        REGISTERED_TIMER_DELAY_BITS.store(0.0f32.to_bits(), Ordering::Relaxed);
        DIFFERENTIAL_CALLS.lock().unwrap().clear();
        tsfp_game_timer_update(10, register_due_timer_during_dispatch);
        let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
        let native = NATIVE_TIMERS.lock();
        let native_count = native.len();
        let native_entries = native.snapshots();
        drop(native);

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x0010_0000;
        let mut lifted = TimerGuest {
            memory: lifted_memory.clone(),
            calls: Vec::new(),
        };
        crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
            &mut lifted_memory,
            &mut lifted,
            &mut regs,
        );

        assert_eq!(REGISTERED_TIMER_INDEX.load(Ordering::Relaxed), 1);
        assert_eq!(native_count, 1);
        assert_eq!(native_calls, [(0x100, 7)]);
        assert_eq!(native_entries[0].callback, 0x200);
        assert_eq!(native_entries[0].elapsed, 0);
        assert_eq!(lifted.calls, [(0x100, 7), (0x200, 8)]);
        assert_ne!(lifted.calls, native_calls);

        tsfp_game_timer_update(11, register_due_timer_during_dispatch);
        assert_eq!(
            DIFFERENTIAL_CALLS.lock().unwrap().as_slice(),
            [(0x100, 7), (0x200, 8)]
        );
        assert_eq!(NATIVE_TIMERS.lock().len(), 0);
        tsfp_game_timer_clear();
    }

    /// Tier 1 synthetic boundary differential: a newly appended peer is
    /// traversed in the same pass, but does not fire until its countdown is due.
    #[test]
    fn native_timer_matches_lifted_same_pass_registration_not_due() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;
        const SCALE: u32 = 0x004e_8cec;

        tsfp_game_timer_clear();
        REGISTERED_TIMER_DELAY_BITS.store(1.0f32.to_bits(), Ordering::Relaxed);
        assert_eq!(tsfp_game_timer_add(0, 0.0, 7, 0x100, 9, 60), 0);
        let before = NATIVE_TIMERS.lock().snapshots();
        let mut lifted_memory = TimerMemory::default();
        lifted_memory.write32(COUNT, 1);
        lifted_memory.write32(STAMP, 10);
        lifted_memory.write32(SCALE, 60);
        for (index, entry) in before.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            lifted_memory.write32(base, entry.mode);
            lifted_memory.write32(base + 4, entry.elapsed as u32);
            lifted_memory.write32(base + 8, entry.stamp);
            lifted_memory.write32(base + 12, entry.period_bits);
            lifted_memory.write32(base + 16, entry.argument);
            lifted_memory.write32(base + 20, entry.callback);
        }

        REGISTERED_TIMER_INDEX.store(u32::MAX, Ordering::Relaxed);
        DIFFERENTIAL_CALLS.lock().unwrap().clear();
        tsfp_game_timer_update(10, register_due_timer_during_dispatch);
        let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
        let native = NATIVE_TIMERS.lock();
        let native_count = native.len();
        let native_entries = native.snapshots();
        drop(native);

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x0010_0000;
        let mut lifted = TimerGuest {
            memory: lifted_memory.clone(),
            calls: Vec::new(),
        };
        crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
            &mut lifted_memory,
            &mut lifted,
            &mut regs,
        );

        assert_eq!(REGISTERED_TIMER_INDEX.load(Ordering::Relaxed), 1);
        assert_eq!(native_count, 1);
        assert_eq!(lifted_memory.read32(COUNT) as usize, native_count);
        assert_eq!(lifted.calls, native_calls);
        assert_eq!(native_calls, [(0x100, 7)]);
        assert_eq!(native_entries[0].callback, 0x200);
        assert_eq!(native_entries[0].elapsed, 60);
        for (index, expected) in native_entries.iter().enumerate() {
            assert_eq!(
                timer_snapshot_from_guest(&lifted_memory, index),
                *expected,
                "not-due registration row {index}"
            );
        }
        tsfp_game_timer_clear();
        REGISTERED_TIMER_DELAY_BITS.store(0.0f32.to_bits(), Ordering::Relaxed);
    }

    /// Tier 1 differential for the measured non-catch-up periodic timer rule:
    /// one dispatch fires once and subtracts one period even when several
    /// periods elapsed.
    #[test]
    fn native_timer_update_matches_lifted_periodic_overrun_once_per_dispatch() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;

        tsfp_game_timer_clear();
        assert_eq!(tsfp_game_timer_add(1, 0.1, 9, 0x800, 10, 100), 0);
        let before = NATIVE_TIMERS.lock().snapshots();
        let mut lifted_memory = TimerMemory::default();
        lifted_memory.write32(COUNT, 1);
        lifted_memory.write32(STAMP, 65);
        for (index, entry) in before.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            lifted_memory.write32(base, entry.mode);
            lifted_memory.write32(base + 4, entry.elapsed as u32);
            lifted_memory.write32(base + 8, entry.stamp);
            lifted_memory.write32(base + 12, entry.period_bits);
            lifted_memory.write32(base + 16, entry.argument);
            lifted_memory.write32(base + 20, entry.callback);
        }

        DIFFERENTIAL_CALLS.lock().unwrap().clear();
        tsfp_game_timer_update(65, capture_native_callback);
        let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
        let native = NATIVE_TIMERS.lock();
        let native_count = native.len();
        let native_entries = native.snapshots();
        drop(native);

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x0010_0000;
        let mut lifted = TimerGuest {
            memory: lifted_memory.clone(),
            calls: Vec::new(),
        };
        crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
            &mut lifted_memory,
            &mut lifted,
            &mut regs,
        );

        assert_eq!(native_calls, [(0x800, 9)]);
        assert_eq!(lifted.calls, native_calls);
        assert_eq!(lifted_memory.read32(COUNT) as usize, native_count);
        assert_eq!(native_count, 1);
        assert_eq!(native_entries[0].elapsed, 45);
        for (index, expected) in native_entries.iter().enumerate() {
            assert_eq!(
                timer_snapshot_from_guest(&lifted_memory, index),
                *expected,
                "periodic overrun row {index}"
            );
        }
        tsfp_game_timer_clear();
    }

    /// Tier 1 differential for the measured clock contract: the timer table
    /// consumes the raw simulation stamp supplied to each dispatch. A pause
    /// may leave one large stamp delta for the first later dispatch, or the
    /// caller may dispatch while paused and consume the deltas as they accrue.
    /// This checks timer arithmetic against lifted C2R; it does not exercise
    /// the frame clock or claim a natural pause replay.
    #[test]
    fn native_timer_matches_lifted_across_paused_raw_stamp_gap() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;
        const CALLBACK: u32 = 0x900;
        const ARGUMENT: u32 = 0x55;

        // Cross the one-second deadline by a tick: mode-0 dispatch is strict
        // and exact equality remains not due.
        for updates in [&[161_u32][..], &[130_u32, 161_u32][..]] {
            tsfp_game_timer_clear();
            assert_eq!(tsfp_game_timer_add(0, 1.0, ARGUMENT, CALLBACK, 100, 60), 0);
            let before = NATIVE_TIMERS.lock().snapshots();
            let mut lifted_memory = TimerMemory::default();
            lifted_memory.write32(COUNT, 1);
            for (index, entry) in before.iter().enumerate() {
                let base = TABLE + index as u32 * 0x18;
                lifted_memory.write32(base, entry.mode);
                lifted_memory.write32(base + 4, entry.elapsed as u32);
                lifted_memory.write32(base + 8, entry.stamp);
                lifted_memory.write32(base + 12, entry.period_bits);
                lifted_memory.write32(base + 16, entry.argument);
                lifted_memory.write32(base + 20, entry.callback);
            }

            let mut lifted = TimerGuest {
                memory: lifted_memory.clone(),
                calls: Vec::new(),
            };
            for &stamp in updates {
                lifted_memory.write32(STAMP, stamp);
                DIFFERENTIAL_CALLS.lock().unwrap().clear();
                tsfp_game_timer_update(stamp, capture_native_callback);
                let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
                let native = NATIVE_TIMERS.lock();
                let native_count = native.len();
                let native_entries = native.snapshots();
                drop(native);

                let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
                regs.esp = 0x0010_0000;
                crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
                    &mut lifted_memory,
                    &mut lifted,
                    &mut regs,
                );

                assert_eq!(lifted.calls, native_calls);
                assert_eq!(lifted_memory.read32(COUNT) as usize, native_count);
                for (index, expected) in native_entries.iter().enumerate() {
                    assert_eq!(
                        timer_snapshot_from_guest(&lifted_memory, index),
                        *expected,
                        "stamp {stamp} row {index}"
                    );
                }
            }

            assert_eq!(
                DIFFERENTIAL_CALLS.lock().unwrap().as_slice(),
                &[(CALLBACK, ARGUMENT)]
            );
            assert_eq!(lifted.calls, [(CALLBACK, ARGUMENT)]);
            assert_eq!(NATIVE_TIMERS.lock().len(), 0);
        }
        tsfp_game_timer_clear();
    }

    /// Tier 1 synthetic mutation differential for callback clear/reuse. The
    /// REA source leaves this mutation case open; the test compares the owner
    /// with the lifted update/add/clear bodies, not natural game callback use.
    #[test]
    fn native_timer_update_matches_lifted_clear_and_same_slot_reuse() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;
        const SCALE: u32 = 0x004e_8cec;

        tsfp_game_timer_clear();
        CLEAR_CALLBACK_CALLS.store(0, Ordering::Relaxed);
        CLEAR_ADD_RESULT.store(u32::MAX, Ordering::Relaxed);
        LIFTED_CLEAR_ADD_RESULT.store(u32::MAX, Ordering::Relaxed);
        assert_eq!(tsfp_game_timer_add(0, 0.0, 7, 0x300, 9, 60), 0);
        assert_eq!(tsfp_game_timer_add(0, 0.0, 8, 0x400, 9, 60), 1);

        let before = NATIVE_TIMERS.lock().snapshots();
        let mut lifted_memory = TimerMemory::default();
        lifted_memory.write32(COUNT, 2);
        lifted_memory.write32(STAMP, 10);
        lifted_memory.write32(SCALE, 60);
        for (index, entry) in before.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            lifted_memory.write32(base, entry.mode);
            lifted_memory.write32(base + 4, entry.elapsed as u32);
            lifted_memory.write32(base + 8, entry.stamp);
            lifted_memory.write32(base + 12, entry.period_bits);
            lifted_memory.write32(base + 16, entry.argument);
            lifted_memory.write32(base + 20, entry.callback);
        }

        DIFFERENTIAL_CALLS.lock().unwrap().clear();
        tsfp_game_timer_update(10, clear_and_reuse_timer_slot);
        let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
        let native = NATIVE_TIMERS.lock();
        let native_count = native.len();
        let native_entries = native.snapshots();
        drop(native);

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x0010_0000;
        let mut lifted = TimerGuest {
            memory: lifted_memory.clone(),
            calls: Vec::new(),
        };
        crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
            &mut lifted_memory,
            &mut lifted,
            &mut regs,
        );

        assert_eq!(CLEAR_ADD_RESULT.load(Ordering::Relaxed), 0);
        assert_eq!(LIFTED_CLEAR_ADD_RESULT.load(Ordering::Relaxed), 0);
        assert_eq!(lifted.calls, native_calls);
        assert_eq!(native_calls, [(0x300, 7)]);
        assert_eq!(lifted_memory.read32(COUNT) as usize, native_count);
        for (index, expected) in native_entries.iter().enumerate() {
            assert_eq!(
                timer_snapshot_from_guest(&lifted_memory, index),
                *expected,
                "clear/reuse row {index}"
            );
        }
        tsfp_game_timer_clear();
    }

    /// Tier 1 synthetic mutation differential. The lifted periodic path
    /// subtracts the row's period after callback return, so a replacement in
    /// the same slot ends with elapsed -1 in this fixture.
    #[test]
    fn native_periodic_timer_update_matches_lifted_clear_and_same_slot_reuse() {
        let _guard = TEST_LOCK.lock().unwrap();
        const COUNT: u32 = 0x0074_c410;
        const TABLE: u32 = 0x007a_20e0;
        const STAMP: u32 = 0x007d_e318;
        const SCALE: u32 = 0x004e_8cec;

        tsfp_game_timer_clear();
        CLEAR_ADD_RESULT.store(u32::MAX, Ordering::Relaxed);
        LIFTED_CLEAR_ADD_RESULT.store(u32::MAX, Ordering::Relaxed);
        assert_eq!(tsfp_game_timer_add(1, 0.5, 7, 0x600, 10, 4), 0);

        let before = NATIVE_TIMERS.lock().snapshots();
        let mut lifted_memory = TimerMemory::default();
        lifted_memory.write32(COUNT, 1);
        lifted_memory.write32(STAMP, 12);
        lifted_memory.write32(SCALE, 4);
        for (index, entry) in before.iter().enumerate() {
            let base = TABLE + index as u32 * 0x18;
            lifted_memory.write32(base, entry.mode);
            lifted_memory.write32(base + 4, entry.elapsed as u32);
            lifted_memory.write32(base + 8, entry.stamp);
            lifted_memory.write32(base + 12, entry.period_bits);
            lifted_memory.write32(base + 16, entry.argument);
            lifted_memory.write32(base + 20, entry.callback);
        }

        DIFFERENTIAL_CALLS.lock().unwrap().clear();
        tsfp_game_timer_update(12, periodic_clear_and_reuse_timer_slot);
        let native_calls = DIFFERENTIAL_CALLS.lock().unwrap().clone();
        let native = NATIVE_TIMERS.lock();
        let native_count = native.len();
        let native_entries = native.snapshots();
        drop(native);

        let mut regs: crate::abi::ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x0010_0000;
        let mut lifted = TimerGuest {
            memory: lifted_memory.clone(),
            calls: Vec::new(),
        };
        crate::c2r::t2049_exec_004::game_timer_callback_table_7a20e0_update_by_frame_stamp_call_callbacks_and_compact_finished(
            &mut lifted_memory,
            &mut lifted,
            &mut regs,
        );

        assert_eq!(CLEAR_ADD_RESULT.load(Ordering::Relaxed), 0);
        assert_eq!(LIFTED_CLEAR_ADD_RESULT.load(Ordering::Relaxed), 0);
        assert_eq!(lifted.calls, native_calls);
        assert_eq!(native_calls, [(0x600, 7)]);
        assert_eq!(lifted_memory.read32(COUNT) as usize, native_count);
        assert_eq!(native_count, 1);
        assert_eq!(
            native_entries[0],
            TimerSnapshot {
                mode: 1,
                elapsed: -1,
                stamp: 12,
                period_bits: 1.0f32.to_bits(),
                argument: 8,
                callback: 0x700,
            }
        );
        for (index, expected) in native_entries.iter().enumerate() {
            assert_eq!(
                timer_snapshot_from_guest(&lifted_memory, index),
                *expected,
                "periodic clear/reuse row {index}"
            );
        }
        tsfp_game_timer_clear();
    }
}
