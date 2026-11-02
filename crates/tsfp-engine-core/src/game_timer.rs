//! T1670 native owner for the original 75-slot callback timer table.
//!
//! This is game simulation time (`0x7DE318`), not host wall time or the render
//! clock. The callback ABI stays at the host boundary so timer state itself is
//! typed, fixed-capacity, and independent of guest-memory offsets.
//!
//! Verification: **Tier 0** table specs plus **Tier 1** differentials of update
//! behavior through the host adapter against lifted C2R `0x165540`: 2048 seeded
//! tables, callback state, periodic overrun, and synthetic clear/reuse. The
//! measured strict mode-0 zero-deadline edge disagrees with the current C2R body
//! and is tested as a source-contract exception. There is no Tier 2 replay yet.

pub const TIMER_CAPACITY: usize = 75;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TimerEntry {
    mode: u32,
    elapsed: i32,
    stamp: u32,
    period: f32,
    argument: u32,
    callback: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerFire {
    pub index: usize,
    pub callback: u32,
    pub argument: u32,
    pub one_shot: bool,
}

/// Host diagnostic record in the original field order. This is copied only to
/// host memory; it is never a shared guest-memory view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct TimerSnapshot {
    pub mode: u32,
    pub elapsed: i32,
    pub stamp: u32,
    pub period_bits: u32,
    pub argument: u32,
    pub callback: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerStep {
    Complete,
    Continue,
    Fire(TimerFire),
}

/// Rust-owned timer state. One update step is separated from callback dispatch
/// so the host can release the table lock before entering lifted guest code.
#[derive(Debug)]
pub struct TimerTable {
    entries: [TimerEntry; TIMER_CAPACITY],
    len: usize,
}

pub type TimerCallback = extern "C" fn(u32, u32);

impl Default for TimerTable {
    fn default() -> Self {
        Self {
            entries: [TimerEntry::default(); TIMER_CAPACITY],
            len: 0,
        }
    }
}

impl TimerTable {
    /// Tier 0. Construct an empty table with its fixed original capacity.
    pub const fn new() -> Self {
        Self {
            entries: [TimerEntry {
                mode: 0,
                elapsed: 0,
                stamp: 0,
                period: 0.0,
                argument: 0,
                callback: 0,
            }; TIMER_CAPACITY],
            len: 0,
        }
    }

    /// Restore a complete captured timer table for an offline call.
    ///
    /// Captures must include all 75 rows, including inactive rows: a later add
    /// reuses the row at `len` and fields not written by that add retain their
    /// previous bits. `None` indicates a malformed capture shape or live count.
    pub fn from_snapshots(len: usize, snapshots: &[TimerSnapshot]) -> Option<Self> {
        if len > TIMER_CAPACITY || snapshots.len() != TIMER_CAPACITY {
            return None;
        }
        let entries = core::array::from_fn(|index| {
            let row = snapshots[index];
            TimerEntry {
                mode: row.mode,
                elapsed: row.elapsed,
                stamp: row.stamp,
                period: f32::from_bits(row.period_bits),
                argument: row.argument,
                callback: row.callback,
            }
        });
        Some(Self { entries, len })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Tier 0. Clear the live count and retain inactive slot bytes, as reset does.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Tier 0. Copy the complete host-only diagnostic view.
    pub fn snapshots(&self) -> [TimerSnapshot; TIMER_CAPACITY] {
        core::array::from_fn(|index| self.snapshot(index))
    }

    /// Tier 0. Snapshot one slot for diagnostics.
    pub fn snapshot(&self, index: usize) -> TimerSnapshot {
        let entry = self.entries[index];
        TimerSnapshot {
            mode: entry.mode,
            elapsed: entry.elapsed,
            stamp: entry.stamp,
            period_bits: entry.period.to_bits(),
            argument: entry.argument,
            callback: entry.callback,
        }
    }

    /// Tier 0. Add ABI order: `(mode, seconds, argument, callback)`.
    /// Mode 0 stores a scaled integer countdown; mode 1 stores an integer
    /// elapsed counter and a scaled float period. Other modes occupy a slot
    /// without mode-specific initialization, as the measured original does.
    /// Fields the original does not write retain the reused row's prior value.
    pub fn add(
        &mut self,
        mode: u32,
        seconds: f32,
        argument: u32,
        callback: u32,
        stamp: u32,
        scale: u32,
    ) -> i32 {
        if self.len == TIMER_CAPACITY {
            return -1;
        }
        let scaled = seconds * (scale as i32) as f32;
        let previous = self.entries[self.len];
        let elapsed = match mode {
            0 => x86_truncate_to_i32(scaled),
            1 => 0,
            _ => previous.elapsed,
        };
        let period = if mode == 1 { scaled } else { previous.period };
        let index = self.len;
        self.entries[index] = TimerEntry {
            mode,
            elapsed,
            stamp,
            period,
            argument,
            callback,
        };
        self.len += 1;
        index as i32
    }

    /// Tier 0. Advance one entry using the current original simulation stamp.
    /// Mode-0 callbacks fire only when the updated integer countdown is below 0.
    /// Periodic timers fire once per update and subtract one period after the
    /// callback; a large elapsed interval can leave multiple periods due for
    /// later updates, matching the original's non-catch-up behavior.
    pub fn advance_one(&mut self, index: usize, stamp: u32) -> TimerStep {
        if index >= self.len {
            return TimerStep::Complete;
        }
        let entry = &mut self.entries[index];
        let delta = stamp.wrapping_sub(entry.stamp) as i32;
        entry.stamp = stamp;
        match entry.mode {
            0 => {
                entry.elapsed = entry.elapsed.wrapping_sub(delta);
                (entry.elapsed < 0)
                    .then_some(TimerStep::Fire(TimerFire {
                        index,
                        callback: entry.callback,
                        argument: entry.argument,
                        one_shot: true,
                    }))
                    .unwrap_or(TimerStep::Continue)
            }
            1 => {
                entry.elapsed = entry.elapsed.wrapping_add(delta);
                if (entry.elapsed as f32) >= entry.period {
                    TimerStep::Fire(TimerFire {
                        index,
                        callback: entry.callback,
                        argument: entry.argument,
                        one_shot: false,
                    })
                } else {
                    TimerStep::Continue
                }
            }
            _ => TimerStep::Continue,
        }
    }

    /// Tier 0. Mark the callback's table slot finished after it returns. The
    /// lifted body writes through its saved row pointer, so a callback that clears
    /// and reuses the same slot still leaves that replacement finished.
    pub fn finish_one_shot(&mut self, fire: TimerFire) {
        if fire.one_shot {
            if let Some(entry) = self.entries.get_mut(fire.index) {
                entry.mode = 2;
            }
        }
    }

    /// Tier 0. Apply the repeating timer's period subtraction after its callback,
    /// matching the original callback-visible elapsed value and call order.
    pub fn finish_periodic(&mut self, fire: TimerFire) {
        if !fire.one_shot {
            if let Some(entry) = self.entries.get_mut(fire.index) {
                entry.elapsed = x86_truncate_to_i32(entry.elapsed as f32 - entry.period);
            }
        }
    }

    /// Tier 0. Compact in reverse order by replacing each finished entry with the last
    /// live row. Indices and relative order are intentionally unstable.
    pub fn compact_finished(&mut self) {
        let mut index = self.len;
        while index > 0 {
            index -= 1;
            if self.entries[index].mode == 2 {
                self.len -= 1;
                self.entries[index] = self.entries[self.len];
            }
        }
    }

    /// Run the timer update against captured simulation-clock reads and a
    /// deterministic callback seam. This uses caller-owned table state, so
    /// capture tools can invoke it offline without the live global table or
    /// host callback ABI. The callback may mutate this table before the row is
    /// finished, matching the live dispatch ordering.
    pub fn update_offline(
        &mut self,
        mut simulation_stamp: impl FnMut() -> u32,
        mut dispatch: impl FnMut(&mut Self, TimerFire),
    ) {
        let mut index = 0;
        loop {
            if index >= self.len {
                break;
            }
            let step = self.advance_one(index, simulation_stamp());
            match step {
                TimerStep::Complete => break,
                TimerStep::Continue => {}
                TimerStep::Fire(fire) => {
                    dispatch(self, fire);
                    if fire.one_shot {
                        self.finish_one_shot(fire);
                    } else {
                        self.finish_periodic(fire);
                    }
                }
            }
            index += 1;
        }
        self.compact_finished();
    }
}

fn x86_truncate_to_i32(value: f32) -> i32 {
    if !value.is_finite() || value < i32::MIN as f32 || value >= 2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(table: &mut TimerTable, index: usize, stamp: u32) -> Option<TimerFire> {
        match table.advance_one(index, stamp) {
            TimerStep::Fire(fire) => Some(fire),
            TimerStep::Continue => None,
            TimerStep::Complete => panic!("unexpected end at entry {index}"),
        }
    }

    #[test]
    fn offline_restore_preserves_all_rows_and_exact_period_bits() {
        let rows = core::array::from_fn(|index| TimerSnapshot {
            mode: index as u32,
            elapsed: index as i32 - 10,
            stamp: 100 + index as u32,
            period_bits: if index == 74 {
                0x7fc0_1234
            } else {
                index as u32
            },
            argument: index as u32 + 1,
            callback: index as u32 + 2,
        });
        let restored = TimerTable::from_snapshots(3, &rows).unwrap();
        assert_eq!(restored.len(), 3);
        assert_eq!(restored.snapshots(), rows);
    }

    #[test]
    fn offline_restore_rejects_incomplete_or_impossible_capture() {
        let rows = [TimerSnapshot::default(); TIMER_CAPACITY];
        assert!(TimerTable::from_snapshots(0, &rows[..TIMER_CAPACITY - 1]).is_none());
        assert!(TimerTable::from_snapshots(TIMER_CAPACITY + 1, &rows).is_none());
    }

    #[test]
    fn offline_update_uses_each_captured_clock_read_and_callback_order() {
        let mut table = TimerTable::new();
        table.add(1, 0.01, 0xA, 0x1234, 100, 100);
        table.add(1, 0.01, 0xB, 0x5678, 100, 100);
        let mut stamps = [101, 102].into_iter();
        let mut callbacks = Vec::new();
        table.update_offline(
            || stamps.next().unwrap(),
            |_, fire| callbacks.push((fire.index, fire.callback, fire.argument)),
        );
        assert_eq!(callbacks, [(0, 0x1234, 0xA), (1, 0x5678, 0xB)]);
        assert_eq!(table.snapshot(0).elapsed, 0);
        assert_eq!(table.snapshot(1).elapsed, 1);
    }

    #[test]
    fn offline_callback_can_clear_and_reuse_a_firing_slot_before_finish() {
        let mut table = TimerTable::new();
        table.add(0, 0.0, 0xA, 0x1234, 100, 1);
        table.update_offline(
            || 101,
            |table, _| {
                table.clear();
                table.add(0, 1.0, 0xB, 0x5678, 101, 1);
            },
        );
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn fixed_capacity_returns_index_and_then_minus_one() {
        let mut table = TimerTable::default();
        for index in 0..TIMER_CAPACITY {
            assert_eq!(
                table.add(7, 0.0, index as u32, 0x1000 + index as u32, 20, 100),
                index as i32
            );
        }
        assert_eq!(table.len(), 75);
        assert_eq!(table.add(0, 1.0, 0, 1, 20, 100), -1);
    }

    #[test]
    fn one_shot_uses_simulation_stamp_and_compacts_finished_tail() {
        let mut table = TimerTable::default();
        assert_eq!(table.add(0, 0.05, 0xA, 0x1234, 100, 1000), 0);
        assert_eq!(table.add(7, 0.0, 0xB, 0x5678, 100, 1000), 1);
        assert_eq!(event(&mut table, 0, 149), None);
        assert_eq!(event(&mut table, 0, 150), None);
        let fire = event(&mut table, 0, 151).unwrap();
        assert_eq!(
            (fire.callback, fire.argument, fire.one_shot),
            (0x1234, 0xA, true)
        );
        table.finish_one_shot(fire);
        table.compact_finished();
        assert_eq!(table.len(), 1);
        // Reverse compaction replaces index 0 with the unsupported last row.
        assert_eq!(table.entries[0].argument, 0xB);
        assert_eq!(table.advance_one(0, 151), TimerStep::Continue);
    }

    #[test]
    fn equally_due_timers_are_visited_in_current_table_index_order() {
        let mut table = TimerTable::default();
        assert_eq!(table.add(0, 0.0, 0xA, 0x1234, 100, 1000), 0);
        assert_eq!(table.add(0, 0.0, 0xB, 0x5678, 100, 1000), 1);

        let mut callbacks = Vec::new();
        for index in 0..table.len() {
            let fire = event(&mut table, index, 101).unwrap();
            callbacks.push((fire.callback, fire.argument));
            table.finish_one_shot(fire);
        }

        assert_eq!(callbacks, [(0x1234, 0xA), (0x5678, 0xB)]);
        table.compact_finished();
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn zero_countdown_waits_for_a_positive_stamp_delta() {
        let mut table = TimerTable::default();
        assert_eq!(table.add(0, 0.0, 0xA, 0x1234, 100, 1000), 0);

        assert_eq!(table.advance_one(0, 100), TimerStep::Continue);
        assert_eq!(table.entries[0].elapsed, 0);
        let fire = event(&mut table, 0, 101).unwrap();
        assert_eq!((fire.callback, fire.argument), (0x1234, 0xA));
    }

    #[test]
    fn zero_delay_mode_zero_appended_during_dispatch_is_not_due_at_same_stamp() {
        let mut table = TimerTable::default();
        // This first timer models a callback that appends an immediately armed
        // one-shot while the dispatcher is already visiting the live table.
        table.add(0, 1.0, 0xA, 0x1234, 100, 1000);
        assert_eq!(table.add(0, 0.0, 0xB, 0x5678, 100, 1000), 1);

        assert_eq!(table.advance_one(1, 100), TimerStep::Continue);
        assert_eq!(table.entries[1].elapsed, 0);

        let later = event(&mut table, 1, 101).unwrap();
        assert_eq!((later.callback, later.argument), (0x5678, 0xB));
    }

    #[test]
    fn timer_progress_uses_supplied_frame_stamp_not_effective_step_delta() {
        let mut table = TimerTable::default();
        assert_eq!(table.add(0, 0.02, 7, 0x1234, 100, 1000), 0);

        assert_eq!(table.advance_one(0, 119), TimerStep::Continue);
        assert_eq!(event(&mut table, 0, 120), None);
        let fire = event(&mut table, 0, 121).unwrap();

        assert_eq!((fire.callback, fire.argument), (0x1234, 7));
    }

    #[test]
    fn repeating_timer_fires_once_and_subtracts_one_period_after_overrun() {
        let mut table = TimerTable::default();
        table.add(1, 0.1, 9, 0xCAFE, 10, 100);
        let fire = event(&mut table, 0, 35).unwrap();
        assert_eq!(fire.argument, 9);
        assert_eq!(table.entries[0].elapsed, 25); // callback sees the elapsed count before period subtraction.
        table.finish_periodic(fire);
        assert_eq!(table.entries[0].elapsed, 15);
    }

    #[test]
    fn unsupported_mode_consumes_a_slot_and_only_advances_its_stamp() {
        let mut table = TimerTable::default();
        table.add(9, f32::NAN, 1, 2, u32::MAX - 2, 100);
        assert_eq!(table.advance_one(0, 3), TimerStep::Continue);
        assert_eq!(table.entries[0].stamp, 3);
        assert_eq!(table.entries[0].elapsed, 0);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn add_preserves_fields_the_original_does_not_write_on_reused_rows() {
        let mut table = TimerTable::default();
        table.add(1, 0.25, 1, 2, 10, 100);
        table.clear();

        table.add(0, 0.5, 3, 4, 20, 100);
        assert_eq!(table.entries[0].elapsed, 50);
        assert_eq!(table.entries[0].period, 25.0);
        table.clear();

        table.add(9, f32::NAN, 5, 6, 30, 100);
        assert_eq!(table.entries[0].elapsed, 50);
        assert_eq!(table.entries[0].period, 25.0);
        assert_eq!(table.entries[0].stamp, 30);
        assert_eq!(table.entries[0].argument, 5);
        assert_eq!(table.entries[0].callback, 6);
    }

    #[test]
    fn diagnostic_snapshot_preserves_field_order_and_clear_keeps_stale_rows() {
        let mut table = TimerTable::default();
        table.add(1, 0.25, 0xAA, 0x12345678, 0x1020, 100);
        assert_eq!(
            table.snapshot(0),
            TimerSnapshot {
                mode: 1,
                elapsed: 0,
                stamp: 0x1020,
                period_bits: 25.0_f32.to_bits(),
                argument: 0xAA,
                callback: 0x12345678,
            }
        );
        table.clear();
        assert_eq!(table.len(), 0);
        assert_eq!(table.snapshot(0).callback, 0x12345678);
    }

    #[test]
    fn callback_finish_writes_to_a_cleared_and_reused_slot() {
        let mut table = TimerTable::default();
        table.add(0, 0.0, 1, 10, 5, 100);
        let fire = event(&mut table, 0, 6).unwrap();
        table.clear();
        table.add(0, 3.0, 2, 20, 5, 100);
        table.finish_one_shot(fire);
        assert_eq!(table.entries[0].mode, 2);
        table.compact_finished();
        assert_eq!(table.len(), 0);
    }
}
