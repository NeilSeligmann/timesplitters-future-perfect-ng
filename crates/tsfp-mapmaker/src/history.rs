//! Rust-owned Map Maker edit history (T1689 / T2408).
//!
//! Callers provide typed document snapshots. The journal does not store guest
//! pointers, serialize maps, or conflate editor-history state with durable-save
//! dirtiness. The no-op redo policy is explicit because the original static
//! contract leaves that behavior unconfirmed at runtime.

extern crate alloc;

use alloc::{collections::VecDeque, vec::Vec};

/// Byte capacity of the editor history arena measured by T1689.
pub const MAP_EDIT_HISTORY_ARENA_BYTES: usize = 0x10000;

/// Original-candidate no-op behavior is INFERRED from T1689's bounded static
/// evidence; keep it opt-in until an owner-route replay confirms it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoopCommitPolicy {
    PreserveRedo,
    ClearRedoAndRefreshBaseline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HistoryRecordId(u64);

impl HistoryRecordId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryError {
    RecordIdExhausted,
    InvalidRecordSize,
    RecordExceedsArenaCapacity,
    NothingToUndo,
    NothingToRedo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureOutcome {
    Unchanged { redo_cleared: bool },
    Recorded(HistoryRecordId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryMove<S> {
    pub record: HistoryRecordId,
    pub snapshot: S,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryAvailability {
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HistoryRecord<S> {
    id: HistoryRecordId,
    before: S,
    after: S,
    arena_offset: usize,
    arena_bytes: usize,
}

/// A typed snapshot journal. `current` is the last snapshot accepted by this
/// service; live document handlers apply returned undo/redo snapshots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapEditHistory<S> {
    current: S,
    /// Oldest record first; undo consumes from the back and pressure evicts
    /// from the front, matching the measured oldest-undo walk.
    undo: VecDeque<HistoryRecord<S>>,
    redo: Vec<HistoryRecord<S>>,
    next_record_id: u64,
    noop_policy: NoopCommitPolicy,
    arena_capacity: usize,
    arena_cursor: usize,
}

impl<S: Clone + PartialEq> MapEditHistory<S> {
    pub fn new(initial: S, noop_policy: NoopCommitPolicy) -> Self {
        Self::with_arena_capacity(initial, noop_policy, MAP_EDIT_HISTORY_ARENA_BYTES)
    }

    /// Construct a journal with an explicit byte bound, useful for small
    /// deterministic tests. Production sessions use the measured 64 KiB size.
    pub fn with_arena_capacity(
        initial: S,
        noop_policy: NoopCommitPolicy,
        arena_capacity: usize,
    ) -> Self {
        Self {
            current: initial,
            undo: VecDeque::new(),
            redo: Vec::new(),
            next_record_id: 1,
            noop_policy,
            arena_capacity,
            arena_cursor: 0,
        }
    }

    pub fn current(&self) -> &S {
        &self.current
    }

    pub const fn noop_policy(&self) -> NoopCommitPolicy {
        self.noop_policy
    }

    pub fn availability(&self) -> HistoryAvailability {
        HistoryAvailability {
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }

    /// Check whether a handle still names a record reachable by undo or redo.
    /// IDs are never reused; a branch discard makes its redo handles stale.
    pub fn contains_record(&self, id: HistoryRecordId) -> bool {
        self.undo.iter().any(|record| record.id == id)
            || self.redo.iter().any(|record| record.id == id)
    }

    /// Admit a completed document snapshot and its serialized arena cost in
    /// bytes. Equal snapshots add no record and need no allocation.
    /// The candidate original policy can invalidate redo on that no-op, but is
    /// kept separate from the default safe policy until Tier 2 evidence lands.
    ///
    /// The caller must pass the exact allocation request it would write. On
    /// capacity errors the current snapshot and both history heads are intact.
    pub fn capture(
        &mut self,
        snapshot: S,
        record_bytes: usize,
    ) -> Result<CaptureOutcome, HistoryError> {
        if snapshot == self.current {
            let redo_cleared = match self.noop_policy {
                NoopCommitPolicy::PreserveRedo => false,
                NoopCommitPolicy::ClearRedoAndRefreshBaseline => {
                    let had_redo = !self.redo.is_empty();
                    self.redo.clear();
                    had_redo
                }
            };
            self.current = snapshot;
            return Ok(CaptureOutcome::Unchanged { redo_cleared });
        }

        if record_bytes == 0 {
            return Err(HistoryError::InvalidRecordSize);
        }
        if record_bytes > self.arena_capacity {
            return Err(HistoryError::RecordExceedsArenaCapacity);
        }

        let id = HistoryRecordId(self.next_record_id);
        let next_record_id = self
            .next_record_id
            .checked_add(1)
            .ok_or(HistoryError::RecordIdExhausted)?;
        // The measured carrier allocates from a 64 KiB arena, evicts the
        // oldest undo records to protect retained ranges, and wraps the
        // cursor when its request reaches the end. Redo is cleared by a new
        // branch only after an allocation succeeds. The Rust implementation
        // fails closed for a request larger than the whole arena.
        let mut arena_offset = self.arena_cursor;
        if arena_offset > self.arena_capacity || record_bytes > self.arena_capacity - arena_offset {
            arena_offset = 0;
        }
        let allocation_end = arena_offset + record_bytes;
        while let Some(overlap) = self.undo.iter().position(|record| {
            arena_offset < record.arena_offset + record.arena_bytes
                && record.arena_offset < allocation_end
        }) {
            for _ in 0..=overlap {
                self.undo.pop_front();
            }
        }

        let record = HistoryRecord {
            id,
            before: self.current.clone(),
            after: snapshot.clone(),
            arena_offset,
            arena_bytes: record_bytes,
        };
        self.undo.push_back(record);
        self.redo.clear();
        self.current = snapshot;
        self.next_record_id = next_record_id;
        self.arena_cursor = allocation_end;
        Ok(CaptureOutcome::Recorded(id))
    }

    /// Move the undo head before returning the snapshot to the document
    /// handler, matching the measured original call order. Handler failure
    /// semantics remain outside this model and require the A/B replay gate.
    pub fn undo(&mut self) -> Result<HistoryMove<S>, HistoryError> {
        let record = self.undo.pop_back().ok_or(HistoryError::NothingToUndo)?;
        let snapshot = record.before.clone();
        let result = HistoryMove {
            record: record.id,
            snapshot: snapshot.clone(),
        };
        self.redo.push(record);
        self.current = snapshot;
        Ok(result)
    }

    /// Move the redo head and return its snapshot for the typed document
    /// handler. The original serialize-before-move edge remains a caller seam.
    pub fn redo(&mut self) -> Result<HistoryMove<S>, HistoryError> {
        let record = self.redo.pop().ok_or(HistoryError::NothingToRedo)?;
        let snapshot = record.after.clone();
        let result = HistoryMove {
            record: record.id,
            snapshot: snapshot.clone(),
        };
        self.undo.push_back(record);
        self.current = snapshot;
        Ok(result)
    }
}

/// Editor snapshot dirtiness and durable file dirtiness have independent
/// owners and receipts. History movement never clears either automatically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapDocumentDirtiness {
    editor_snapshot: bool,
    durable_file: bool,
}

impl MapDocumentDirtiness {
    pub const fn editor_snapshot_dirty(self) -> bool {
        self.editor_snapshot
    }

    pub const fn durable_file_dirty(self) -> bool {
        self.durable_file
    }

    pub fn mark_editor_snapshot_dirty(&mut self) {
        self.editor_snapshot = true;
    }

    pub fn accept_editor_snapshot(&mut self) {
        self.editor_snapshot = false;
    }

    pub fn mark_durable_file_dirty(&mut self) {
        self.durable_file = true;
    }

    /// Only a successful durable-save receipt should call this method.
    pub fn accept_durable_save(&mut self) {
        self.durable_file = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_create_typed_records_and_undo_redo_move_heads() {
        let mut history = MapEditHistory::new(10_u32, NoopCommitPolicy::PreserveRedo);
        let CaptureOutcome::Recorded(id) = history.capture(20, 16).unwrap() else {
            panic!("changed snapshot must create a record");
        };
        assert_eq!(history.availability().can_undo, true);

        let undone = history.undo().unwrap();
        assert_eq!(undone.record, id);
        assert_eq!(undone.snapshot, 10);
        assert_eq!(*history.current(), 10);
        assert_eq!(
            history.availability(),
            HistoryAvailability {
                can_undo: false,
                can_redo: true,
            }
        );

        let redone = history.redo().unwrap();
        assert_eq!(redone.record, id);
        assert_eq!(redone.snapshot, 20);
        assert_eq!(*history.current(), 20);
    }

    #[test]
    fn no_op_preserves_redo_under_the_safe_policy() {
        let mut history = MapEditHistory::new(0_u32, NoopCommitPolicy::PreserveRedo);
        history.capture(1, 16).unwrap();
        history.undo().unwrap();

        assert_eq!(
            history.capture(0, 0),
            Ok(CaptureOutcome::Unchanged {
                redo_cleared: false
            })
        );
        assert!(history.availability().can_redo);
    }

    #[test]
    fn inferred_no_op_policy_clears_redo_without_adding_a_record() {
        let mut history = MapEditHistory::new(0_u32, NoopCommitPolicy::ClearRedoAndRefreshBaseline);
        history.capture(1, 16).unwrap();
        history.undo().unwrap();

        assert_eq!(
            history.capture(0, 0),
            Ok(CaptureOutcome::Unchanged { redo_cleared: true })
        );
        assert_eq!(history.availability(), HistoryAvailability::default());
        assert_eq!(*history.current(), 0);
    }

    #[test]
    fn changed_snapshot_invalidates_redo_and_record_ids_do_not_repeat() {
        let mut history = MapEditHistory::new(0_u32, NoopCommitPolicy::PreserveRedo);
        let CaptureOutcome::Recorded(first) = history.capture(1, 16).unwrap() else {
            panic!("expected first record");
        };
        history.undo().unwrap();
        let CaptureOutcome::Recorded(second) = history.capture(2, 16).unwrap() else {
            panic!("expected branched record");
        };

        assert!(second.get() > first.get());
        assert!(!history.contains_record(first));
        assert!(history.contains_record(second));
        assert_eq!(
            history.availability(),
            HistoryAvailability {
                can_undo: true,
                can_redo: false,
            }
        );
    }

    #[test]
    fn unavailable_history_moves_leave_state_unchanged() {
        let mut history = MapEditHistory::new(7_u32, NoopCommitPolicy::PreserveRedo);
        assert_eq!(history.undo(), Err(HistoryError::NothingToUndo));
        assert_eq!(history.redo(), Err(HistoryError::NothingToRedo));
        assert_eq!(*history.current(), 7);
        assert_eq!(history.availability(), HistoryAvailability::default());
    }

    #[test]
    fn oversized_record_is_rejected_without_mutating_owned_state() {
        let mut history =
            MapEditHistory::with_arena_capacity(0_u32, NoopCommitPolicy::PreserveRedo, 8);

        assert_eq!(
            history.capture(1, 9),
            Err(HistoryError::RecordExceedsArenaCapacity)
        );
        assert_eq!(*history.current(), 0);
        assert_eq!(history.availability(), HistoryAvailability::default());
    }

    #[test]
    fn wrapped_allocation_evicts_oldest_undo_and_invalidates_its_handle() {
        let mut history =
            MapEditHistory::with_arena_capacity(0_u32, NoopCommitPolicy::PreserveRedo, 8);
        let CaptureOutcome::Recorded(oldest) = history.capture(1, 4).unwrap() else {
            panic!("expected initial record");
        };
        let CaptureOutcome::Recorded(middle) = history.capture(2, 4).unwrap() else {
            panic!("expected second record");
        };

        let CaptureOutcome::Recorded(newest) = history.capture(3, 4).unwrap() else {
            panic!("expected wrapped record");
        };

        assert!(newest.get() > middle.get());
        assert!(!history.contains_record(oldest));
        assert!(history.contains_record(middle));
        assert!(history.contains_record(newest));
        assert_eq!(history.undo().unwrap().snapshot, 2);
        assert_eq!(history.undo().unwrap().snapshot, 1);
        assert_eq!(history.undo(), Err(HistoryError::NothingToUndo));
    }

    #[test]
    fn branched_capture_discards_redo_handle_without_reusing_its_id() {
        let mut history =
            MapEditHistory::with_arena_capacity(0_u32, NoopCommitPolicy::PreserveRedo, 16);
        let CaptureOutcome::Recorded(discarded) = history.capture(1, 4).unwrap() else {
            panic!("expected initial record");
        };
        history.undo().unwrap();

        let CaptureOutcome::Recorded(branch) = history.capture(2, 4).unwrap() else {
            panic!("expected branch record");
        };

        assert!(branch.get() > discarded.get());
        assert!(!history.contains_record(discarded));
        assert!(history.contains_record(branch));
        assert_eq!(*history.current(), 2);
        assert!(!history.availability().can_redo);
    }

    #[test]
    fn editor_and_durable_dirty_receipts_are_independent() {
        let mut dirty = MapDocumentDirtiness::default();
        dirty.mark_editor_snapshot_dirty();
        dirty.mark_durable_file_dirty();
        dirty.accept_editor_snapshot();

        assert!(!dirty.editor_snapshot_dirty());
        assert!(dirty.durable_file_dirty());

        dirty.accept_durable_save();
        assert!(!dirty.durable_file_dirty());
    }
}
