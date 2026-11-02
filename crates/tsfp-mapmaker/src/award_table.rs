//! Rust-owned interaction state for the T1714 dynamic Awards table.
//!
//! T1714 measures the page builder and that it contains a trailing extra
//! item, but it does not identify table-row callback effects. Rows therefore
//! keep their live order and use generation-scoped page-local ordinals.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardTableGeneration(u64);

impl AwardTableGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardTableRowId(u32);

impl AwardTableRowId {
    pub const fn ordinal(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardTableTarget {
    generation: AwardTableGeneration,
    row: AwardTableRowId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwardTableUpdate {
    Focused(AwardTableRowId),
    SelectionRequested(AwardTableRowId),
    BackRequested,
}

/// Interaction identity for one live Awards table opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwardTablePageState {
    generation: AwardTableGeneration,
    row_count: u32,
    focused: Option<AwardTableRowId>,
}

impl AwardTablePageState {
    pub const fn new(generation: AwardTableGeneration, row_count: u32) -> Self {
        Self {
            generation,
            row_count,
            focused: None,
        }
    }

    pub const fn row_count(self) -> u32 {
        self.row_count
    }

    pub const fn focused(self) -> Option<AwardTableRowId> {
        self.focused
    }

    pub fn target(&self, ordinal: usize) -> Option<AwardTableTarget> {
        let row = u32::try_from(ordinal).ok()?;
        (row < self.row_count).then_some(AwardTableTarget {
            generation: self.generation,
            row: AwardTableRowId(row),
        })
    }

    pub fn focus(&mut self, target: AwardTableTarget) -> Option<AwardTableUpdate> {
        if target.generation != self.generation || target.row.0 >= self.row_count {
            return None;
        }
        self.focused = Some(target.row);
        Some(AwardTableUpdate::Focused(target.row))
    }

    pub fn activate(&mut self, target: AwardTableTarget) -> Option<AwardTableUpdate> {
        self.focus(target)?;
        Some(AwardTableUpdate::SelectionRequested(target.row))
    }

    pub const fn back(&self) -> AwardTableUpdate {
        AwardTableUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owns_dynamic_rows_and_rejects_stale_page_targets() {
        let mut state = AwardTablePageState::new(AwardTableGeneration::new(8), 4);
        let row = state.target(3).unwrap();
        assert_eq!(row.row.ordinal(), 3);
        assert_eq!(
            state.activate(row),
            Some(AwardTableUpdate::SelectionRequested(AwardTableRowId(3)))
        );
        assert!(state.target(4).is_none());

        let stale = AwardTablePageState::new(AwardTableGeneration::new(7), 4)
            .target(3)
            .unwrap();
        assert_eq!(state.focus(stale), None);
        assert_eq!(state.focused(), Some(AwardTableRowId(3)));
        assert_eq!(state.back(), AwardTableUpdate::BackRequested);
    }
}
