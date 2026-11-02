//! Readable route-program accessors reconstructed from the retained T1662 evidence.
//!
//! `RouteProgram::peek_after` models the observed selection performed by original
//! `0x26D690`. It does not own actor state, resolve node IDs, or execute route steps.

/// The route-step fields visible to the executor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteStep {
    /// Step kind at entry offset `+0`.
    pub kind: u32,
    /// First DWORD payload at entry offset `+4`.
    pub payload: u32,
    /// Second DWORD payload at entry offset `+8`.
    pub target: u32,
}

/// A route program as typed twelve-byte entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteProgram<'a> {
    steps: &'a [RouteStep],
}

/// Failure to resolve a selection from a malformed or incomplete route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteSelectionError {
    MissingNextStep,
    ControlPayloadUnderflow,
    SelectedStepOutOfRange,
}

/// The selected route entry and its index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteSelection<'a> {
    pub index: usize,
    pub step: &'a RouteStep,
}

impl<'a> RouteProgram<'a> {
    pub const fn new(steps: &'a [RouteStep]) -> Self {
        Self { steps }
    }

    /// Select the entry that original `0x26D690` peeks after `current_index`.
    ///
    /// Ordinary kinds select `current_index + 1`; kind 12 selects entry 1;
    /// kind 13 selects `current_index - payload + 1`, where payload belongs to
    /// the next entry. The original uses unchecked pointer arithmetic. This
    /// typed boundary rejects missing entries and underflow instead of forming
    /// an invalid pointer; malformed-route behavior is therefore not claimed
    /// to match the original fault/undefined-memory behavior.
    pub fn peek_after(
        &self,
        current_index: usize,
    ) -> Result<RouteSelection<'a>, RouteSelectionError> {
        let next_index = current_index
            .checked_add(1)
            .ok_or(RouteSelectionError::SelectedStepOutOfRange)?;
        let next = self
            .steps
            .get(next_index)
            .ok_or(RouteSelectionError::MissingNextStep)?;
        let selected_index = match next.kind {
            12 => 1,
            13 => current_index
                .checked_sub(next.payload as usize)
                .and_then(|index| index.checked_add(1))
                .ok_or(RouteSelectionError::ControlPayloadUnderflow)?,
            _ => next_index,
        };
        let step = self
            .steps
            .get(selected_index)
            .ok_or(RouteSelectionError::SelectedStepOutOfRange)?;
        Ok(RouteSelection {
            index: selected_index,
            step,
        })
    }
}
