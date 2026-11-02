//! Rust-owned interaction state for T1714's mode-filtered kind list.
//!
//! The page builder is measured, but its runtime row values and callback
//! meanings are not. Rows therefore keep their live order and use
//! generation-scoped page-local ordinals.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KindListGeneration(u64);

impl KindListGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KindListRowId(u32);

impl KindListRowId {
    pub const fn ordinal(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KindListTarget {
    generation: KindListGeneration,
    row: KindListRowId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindListUpdate {
    Focused(KindListRowId),
    SelectionRequested(KindListRowId),
    BackRequested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindListPageState {
    generation: KindListGeneration,
    row_count: u32,
    focused: Option<KindListRowId>,
}

impl KindListPageState {
    pub const fn new(generation: KindListGeneration, row_count: u32) -> Self {
        Self {
            generation,
            row_count,
            focused: None,
        }
    }

    pub const fn row_count(self) -> u32 {
        self.row_count
    }

    pub const fn focused(self) -> Option<KindListRowId> {
        self.focused
    }

    pub fn target(&self, ordinal: usize) -> Option<KindListTarget> {
        let row = u32::try_from(ordinal).ok()?;
        (row < self.row_count).then_some(KindListTarget {
            generation: self.generation,
            row: KindListRowId(row),
        })
    }

    pub fn focus(&mut self, target: KindListTarget) -> Option<KindListUpdate> {
        if target.generation != self.generation || target.row.0 >= self.row_count {
            return None;
        }
        self.focused = Some(target.row);
        Some(KindListUpdate::Focused(target.row))
    }

    pub fn activate(&mut self, target: KindListTarget) -> Option<KindListUpdate> {
        self.focus(target)?;
        Some(KindListUpdate::SelectionRequested(target.row))
    }

    pub const fn back(&self) -> KindListUpdate {
        KindListUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_dynamic_rows_ordered_and_rejects_stale_targets() {
        let mut state = KindListPageState::new(KindListGeneration::new(4), 3);
        let last = state.target(2).unwrap();
        assert_eq!(last.row.ordinal(), 2);
        assert_eq!(
            state.activate(last),
            Some(KindListUpdate::SelectionRequested(KindListRowId(2)))
        );
        assert!(state.target(3).is_none());

        let stale = KindListPageState::new(KindListGeneration::new(3), 3)
            .target(2)
            .unwrap();
        assert_eq!(state.focus(stale), None);
        assert_eq!(state.focused(), Some(KindListRowId(2)));
        assert_eq!(state.back(), KindListUpdate::BackRequested);
    }
}
