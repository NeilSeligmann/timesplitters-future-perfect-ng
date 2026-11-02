//! Rust-owned Map Maker edit history (T2383).
//!
//! This models the documented REA byte-range records on the fixed padded 0x7fe0-byte map
//! snapshot. The serialized document remains the owned editor state; history owns reversible
//! ranges and never borrows guest memory or a widget. Original grouping and payload swapping are
//! Tier 1 differential-tested in `tsfp-c-diff`.
use crate::{
    storage_flow::{MAP_PAYLOAD_BYTES, MapPayload},
    vfs::{NtStatus, status},
};

const COMMON_RANGE_MINIMUM: usize = 32;
/// The original arena is 64 KiB. Native storage caps retained range descriptors and payloads at
/// the same data budget; Vec metadata and allocator overhead are outside this explicit budget.
const MAX_RETAINED_RECORD_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DifferenceRange {
    pub offset: usize,
    pub length: usize,
}

/// The old bytes for each range are concatenated in descriptor order, matching the original
/// history node's range table and retained payload. Its payload is swapped on every apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoRecord {
    pub ranges: Vec<DifferenceRange>,
    pub payload: Vec<u8>,
}

impl UndoRecord {
    pub fn between(
        current: &[u8],
        baseline: &[u8],
        current_len: usize,
        baseline_len: usize,
    ) -> Option<Self> {
        if current.len() != MAP_PAYLOAD_BYTES
            || baseline.len() != MAP_PAYLOAD_BYTES
            || current_len > current.len()
            || baseline_len > baseline.len()
        {
            return None;
        }

        let mut ranges: Vec<DifferenceRange> =
            difference_ranges(current, baseline, current_len, baseline_len)?
                .into_iter()
                .map(|(offset, length)| DifferenceRange { offset, length })
                .collect();
        // The original helper deliberately emits minimum-width ranges for equal bytes. Native
        // history does not create a user-visible edit for an identical complete snapshot.
        if current == baseline {
            ranges.clear();
        }
        if ranges.is_empty() {
            return Some(Self {
                ranges,
                payload: Vec::new(),
            });
        }

        let payload = ranges
            .iter()
            .flat_map(|range| {
                baseline[range.offset..range.offset + range.length]
                    .iter()
                    .copied()
            })
            .collect();
        Some(Self { ranges, payload })
    }

    /// Restore retained bytes into the working snapshot, then retain the previous baseline bytes
    /// for the inverse operation. All ranges are checked before either buffer is mutated.
    pub fn apply(&mut self, working: &mut [u8], baseline: &[u8]) -> Result<(), NtStatus> {
        if working.len() != MAP_PAYLOAD_BYTES || baseline.len() != MAP_PAYLOAD_BYTES {
            return Err(status::INVALID_PARAMETER);
        }
        let payload_len = self
            .ranges
            .iter()
            .try_fold(0usize, |sum, range| {
                let end = range.offset.checked_add(range.length)?;
                (end <= working.len() && end <= baseline.len())
                    .then(|| sum.checked_add(range.length))
                    .flatten()
            })
            .ok_or(status::INVALID_PARAMETER)?;
        if payload_len != self.payload.len() {
            return Err(status::INVALID_PARAMETER);
        }

        let mut restored = working.to_vec();
        let mut replacement = Vec::with_capacity(payload_len);
        let mut payload_at = 0;
        for range in &self.ranges {
            let end = range.offset + range.length;
            let payload_end = payload_at + range.length;
            restored[range.offset..end].copy_from_slice(&self.payload[payload_at..payload_end]);
            replacement.extend_from_slice(&baseline[range.offset..end]);
            payload_at = payload_end;
        }
        working.copy_from_slice(&restored);
        self.payload = replacement;
        Ok(())
    }
}

/// Instruction-derived 3367A0 grouping policy. `current_len`/`baseline_len` are logical lengths;
/// slices remain physically padded because the original may read old bytes past its logical end.
pub fn difference_ranges(
    current: &[u8],
    baseline: &[u8],
    current_len: usize,
    baseline_len: usize,
) -> Option<Vec<(usize, usize)>> {
    if current.len() != MAP_PAYLOAD_BYTES
        || baseline.len() != MAP_PAYLOAD_BYTES
        || current_len > current.len()
        || baseline_len > baseline.len()
    {
        return None;
    }
    let common = current_len.min(baseline_len);
    let mut output = Vec::new();
    let mut at = 0;
    while at < common {
        let start = at;
        at = (at + COMMON_RANGE_MINIMUM).min(common);
        if at < common {
            while at < common && current[at] != baseline[at] {
                at += 1;
            }
        }
        output.push((start, at - start));
        if at < common {
            // The equal delimiter after the mandatory 32-byte group is deliberately skipped.
            at += 1;
        }
    }
    if current_len != baseline_len {
        output.push((common, current_len.max(baseline_len) - common));
    }
    Some(output)
}

/// Owned editor undo/redo state. Snapshot framing remains MapDocument's responsibility.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapHistory {
    current: Option<MapPayload>,
    undo: Vec<UndoRecord>,
    redo: Vec<UndoRecord>,
}

impl MapHistory {
    pub fn is_initialized(&self) -> bool {
        self.current.is_some()
    }

    /// Start a newly loaded editor history without changing its document or save-dirty state.
    pub fn reset(&mut self, snapshot: MapPayload) {
        self.current = Some(snapshot);
        self.undo.clear();
        self.redo.clear();
    }

    /// Record a new serialized editor state. Returns false for an identical snapshot.
    pub fn commit(&mut self, snapshot: MapPayload) -> Result<bool, NtStatus> {
        let Some(previous) = self.current.as_ref() else {
            self.reset(snapshot);
            return Ok(false);
        };
        let current_len = used_length(snapshot.bytes())?;
        let previous_len = used_length(previous.bytes())?;
        let record = UndoRecord::between(
            snapshot.bytes(),
            previous.bytes(),
            current_len,
            previous_len,
        )
        .ok_or(status::INVALID_PARAMETER)?;
        if record.ranges.is_empty() {
            return Ok(false);
        }
        let new_cost = record_cost(&record).ok_or(status::INVALID_PARAMETER)?;
        if new_cost > MAX_RETAINED_RECORD_BYTES {
            return Err(status::INVALID_PARAMETER);
        }
        self.redo.clear();
        while self
            .undo
            .iter()
            .try_fold(new_cost, |used, record| {
                used.checked_add(record_cost(record)?)
            })
            .is_none_or(|used| used > MAX_RETAINED_RECORD_BYTES)
        {
            if self.undo.is_empty() {
                return Err(status::INVALID_PARAMETER);
            }
            self.undo.remove(0);
        }
        self.undo.push(record);
        self.current = Some(snapshot);
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> Result<Option<MapPayload>, NtStatus> {
        self.apply_one(true)
    }

    pub fn redo(&mut self) -> Result<Option<MapPayload>, NtStatus> {
        self.apply_one(false)
    }

    fn apply_one(&mut self, undo: bool) -> Result<Option<MapPayload>, NtStatus> {
        let Some(mut record) = (if undo {
            self.undo.last()
        } else {
            self.redo.last()
        })
        .cloned() else {
            return Ok(None);
        };
        let current = self.current.as_ref().ok_or(status::INVALID_PARAMETER)?;
        let mut working = current.bytes().to_vec();
        record.apply(&mut working, current.bytes())?;
        let restored = MapPayload::new(working)?;
        if undo {
            self.undo.pop();
        } else {
            self.redo.pop();
        }
        self.current = Some(restored.clone());
        if undo {
            self.redo.push(record);
        } else {
            self.undo.push(record);
        }
        Ok(Some(restored))
    }
}

fn used_length(bytes: &[u8]) -> Result<usize, NtStatus> {
    let word = bytes.get(0x258..0x25c).ok_or(status::INVALID_PARAMETER)?;
    let length = u32::from_le_bytes(word.try_into().unwrap()) as usize;
    (length <= MAP_PAYLOAD_BYTES)
        .then_some(length)
        .ok_or(status::INVALID_PARAMETER)
}

fn record_cost(record: &UndoRecord) -> Option<usize> {
    record
        .ranges
        .len()
        .checked_mul(8)?
        .checked_add(record.payload.len())
}
