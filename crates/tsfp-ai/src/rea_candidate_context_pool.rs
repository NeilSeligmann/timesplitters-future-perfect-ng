//! Rust-owned allocation plan for the six `0xFFC` records used by the
//! candidate-table initializer at `0xD2FD0`.
//!
//! This models only the free-slot scan, append limit, and marker write seen in
//! `0xCCA10`. The record initializer (`0xCC920`) and record contents stay at the
//! caller boundary; this type stores no guest pointers.

pub const CONTEXT_POOL_CAPACITY: usize = 6;
pub const CONTEXT_RECORD_STRIDE: u32 = 0xFFC;
pub const CONTEXT_MARKER_OFFSET: u32 = 0x21C;
const MARKER_DEFAULT: u32 = 0x8000_0011;
const MARKER_CONFIGURED: u32 = 0x8000_4013;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextSlotSource {
    Reused,
    Appended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextAllocationPlan {
    pub slot_index: usize,
    pub byte_offset: u32,
    pub source: ContextSlotSource,
    pub marker_after_init: u32,
    /// The original calls `0xCC920` on both reuse and append paths.
    pub call_record_initializer: bool,
}

/// Rust-owned marker state for the six-record candidate context pool.
///
/// Record payload initialization remains a caller boundary: obtain a plan,
/// run the original-equivalent initializer in the caller, then commit its
/// marker with `commit_initialized`. This prevents the pool from publishing a
/// marker before the record initializer has completed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateContextPool {
    markers: [i32; CONTEXT_POOL_CAPACITY],
    len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextPoolCommitError {
    SlotOutOfOrder,
    InvalidPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextPoolInitError<E> {
    NoAvailableSlot,
    Initializer(E),
    Commit(ContextPoolCommitError),
}

impl CandidateContextPool {
    /// Construct an empty typed pool.
    pub const fn new() -> Self {
        Self {
            markers: [-1; CONTEXT_POOL_CAPACITY],
            len: 0,
        }
    }

    /// Adopt marker snapshots read by the caller's explicit storage adapter.
    /// Oversized snapshots are rejected rather than silently truncated.
    pub fn from_markers(markers: &[i32]) -> Option<Self> {
        if markers.len() > CONTEXT_POOL_CAPACITY {
            return None;
        }
        let mut pool = Self::new();
        pool.markers[..markers.len()].copy_from_slice(markers);
        pool.len = markers.len();
        Some(pool)
    }

    /// Plan allocation from this Rust-owned state without mutating it.
    pub fn plan_allocation(&self, configured: bool) -> Option<ContextAllocationPlan> {
        plan_context_allocation(&self.markers[..self.len], configured)
    }

    /// Run a Rust-owned record initializer and publish its marker on success.
    ///
    /// The initializer receives the selected slot index and owns payload
    /// writes. If it reports an error, pool markers remain unchanged; any
    /// payload side effects from the callback remain the caller's responsibility.
    pub fn initialize_with<E>(
        &mut self,
        configured: bool,
        initialize: impl FnOnce(usize) -> Result<(), E>,
    ) -> Result<ContextAllocationPlan, ContextPoolInitError<E>> {
        let plan = self
            .plan_allocation(configured)
            .ok_or(ContextPoolInitError::NoAvailableSlot)?;
        initialize(plan.slot_index).map_err(ContextPoolInitError::Initializer)?;
        self.commit_initialized(plan)
            .map_err(ContextPoolInitError::Commit)?;
        Ok(plan)
    }

    /// Commit the marker after the caller has completed record initialization.
    ///
    /// Only plans consistent with current pool state, slot source, stride, and
    /// measured marker values are accepted. Failure leaves the pool unchanged.
    pub fn commit_initialized(
        &mut self,
        plan: ContextAllocationPlan,
    ) -> Result<(), ContextPoolCommitError> {
        if !plan.call_record_initializer
            || plan.byte_offset != (plan.slot_index as u32).wrapping_mul(CONTEXT_RECORD_STRIDE)
            || !matches!(plan.marker_after_init, MARKER_DEFAULT | MARKER_CONFIGURED)
        {
            return Err(ContextPoolCommitError::InvalidPlan);
        }
        let configured = plan.marker_after_init == MARKER_CONFIGURED;
        if self.plan_allocation(configured) != Some(plan) {
            return Err(ContextPoolCommitError::InvalidPlan);
        }
        match plan.source {
            ContextSlotSource::Reused if plan.slot_index < self.len => {}
            ContextSlotSource::Appended
                if plan.slot_index == self.len && self.len < CONTEXT_POOL_CAPACITY =>
            {
                self.len += 1;
            }
            _ => return Err(ContextPoolCommitError::SlotOutOfOrder),
        }
        self.markers[plan.slot_index] = plan.marker_after_init as i32;
        Ok(())
    }

    /// Current initialized marker prefix, in original pool order.
    pub fn markers(&self) -> &[i32] {
        &self.markers[..self.len]
    }
}

impl Default for CandidateContextPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Plan the observed slot choice made by `0xCCA10`.
///
/// `markers` are snapshots of `record+0x21C` in current pool order. The
/// original scan retains the last nonnegative marker encountered. If none is
/// reusable, it appends while fewer than six records exist. `configured` is
/// the raw first argument to `0xCCA10`; nonzero selects marker `0x80004013`,
/// otherwise the marker is `0x80000011`.
pub fn plan_context_allocation(markers: &[i32], configured: bool) -> Option<ContextAllocationPlan> {
    let count = markers.len().min(CONTEXT_POOL_CAPACITY);
    let reusable = markers[..count]
        .iter()
        .enumerate()
        .filter_map(|(index, &marker)| (marker >= 0).then_some(index))
        .last();

    let (slot_index, source) = match reusable {
        Some(index) => (index, ContextSlotSource::Reused),
        None if count < CONTEXT_POOL_CAPACITY => (count, ContextSlotSource::Appended),
        None => return None,
    };

    Some(ContextAllocationPlan {
        slot_index,
        byte_offset: (slot_index as u32).wrapping_mul(CONTEXT_RECORD_STRIDE),
        source,
        marker_after_init: if configured {
            MARKER_CONFIGURED
        } else {
            MARKER_DEFAULT
        },
        call_record_initializer: true,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CONTEXT_MARKER_OFFSET, CONTEXT_POOL_CAPACITY, CONTEXT_RECORD_STRIDE, ContextSlotSource,
        plan_context_allocation,
    };

    #[test]
    fn reuses_last_nonnegative_record_and_preserves_configured_marker() {
        let plan = plan_context_allocation(&[-1, 0, -2, 7, -3], true).unwrap();
        assert_eq!(plan.slot_index, 3);
        assert_eq!(plan.byte_offset, 3 * CONTEXT_RECORD_STRIDE);
        assert_eq!(plan.source, ContextSlotSource::Reused);
        assert_eq!(plan.marker_after_init, 0x8000_4013);
        assert!(plan.call_record_initializer);
        assert_eq!(CONTEXT_MARKER_OFFSET, 0x21C);
    }

    #[test]
    fn appends_below_capacity_and_uses_default_marker_when_unconfigured() {
        let plan = plan_context_allocation(&[-1, -2], false).unwrap();
        assert_eq!(plan.slot_index, 2);
        assert_eq!(plan.source, ContextSlotSource::Appended);
        assert_eq!(plan.marker_after_init, 0x8000_0011);
    }

    #[test]
    fn full_pool_without_reusable_record_has_no_allocation_plan() {
        assert_eq!(
            plan_context_allocation(&[-1; CONTEXT_POOL_CAPACITY], true),
            None
        );
    }
}
