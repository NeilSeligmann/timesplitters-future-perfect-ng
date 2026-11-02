//! Rust-owned Assault phase enable state for one checklist page opening.
//!
//! The measured handler gates a toggle on descriptor availability, then
//! writes `1 - old` with wrapping dword arithmetic. This model keeps row
//! identity and checklist state typed and page-scoped.

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultPhaseId(u32);

impl AssaultPhaseId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultPhaseToggleGeneration(u64);

impl AssaultPhaseToggleGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseToggleRow {
    id: AssaultPhaseId,
    available: bool,
    value: u32,
}

impl AssaultPhaseToggleRow {
    pub const fn new(id: AssaultPhaseId, available: bool, value: u32) -> Self {
        Self {
            id,
            available,
            value,
        }
    }
    pub const fn id(self) -> AssaultPhaseId {
        self.id
    }
    pub const fn available(self) -> bool {
        self.available
    }
    pub const fn value(self) -> u32 {
        self.value
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseToggleTarget {
    generation: AssaultPhaseToggleGeneration,
    phase: AssaultPhaseId,
}

impl AssaultPhaseToggleTarget {
    pub const fn phase(self) -> AssaultPhaseId {
        self.phase
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseToggleOutcome {
    Ignored,
    Toggled { phase: AssaultPhaseId, value: u32 },
}

/// A typed checklist snapshot with caller-supplied phase identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssaultPhaseTogglePage {
    generation: AssaultPhaseToggleGeneration,
    rows: Vec<AssaultPhaseToggleRow>,
}

impl AssaultPhaseTogglePage {
    pub fn new(
        generation: AssaultPhaseToggleGeneration,
        rows: Vec<AssaultPhaseToggleRow>,
    ) -> Option<Self> {
        let unique = rows
            .iter()
            .enumerate()
            .all(|(index, row)| rows[..index].iter().all(|prior| prior.id != row.id));
        unique.then_some(Self { generation, rows })
    }

    pub fn rows(&self) -> &[AssaultPhaseToggleRow] {
        &self.rows
    }

    pub const fn target(&self, phase: AssaultPhaseId) -> AssaultPhaseToggleTarget {
        AssaultPhaseToggleTarget {
            generation: self.generation,
            phase,
        }
    }

    /// Apply measured wrapping `1 - old` only for an available current row.
    pub fn toggle(&mut self, target: AssaultPhaseToggleTarget) -> AssaultPhaseToggleOutcome {
        if target.generation != self.generation {
            return AssaultPhaseToggleOutcome::Ignored;
        }
        let Some(row) = self.rows.iter_mut().find(|row| row.id == target.phase) else {
            return AssaultPhaseToggleOutcome::Ignored;
        };
        if !row.available {
            return AssaultPhaseToggleOutcome::Ignored;
        }
        row.value = 1u32.wrapping_sub(row.value);
        AssaultPhaseToggleOutcome::Toggled {
            phase: row.id,
            value: row.value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn page() -> AssaultPhaseTogglePage {
        AssaultPhaseTogglePage::new(
            AssaultPhaseToggleGeneration::new(7),
            vec![
                AssaultPhaseToggleRow::new(AssaultPhaseId::new(11), true, 0),
                AssaultPhaseToggleRow::new(AssaultPhaseId::new(12), false, 1),
                AssaultPhaseToggleRow::new(AssaultPhaseId::new(13), true, 5),
            ],
        )
        .unwrap()
    }

    #[test]
    fn available_rows_use_wrapping_one_minus_old_transition() {
        let mut page = page();
        let target = page.target(AssaultPhaseId::new(11));
        assert_eq!(
            page.toggle(target),
            AssaultPhaseToggleOutcome::Toggled {
                phase: AssaultPhaseId::new(11),
                value: 1,
            }
        );
        assert_eq!(
            page.toggle(target),
            AssaultPhaseToggleOutcome::Toggled {
                phase: AssaultPhaseId::new(11),
                value: 0,
            }
        );
        let non_boolean = page.target(AssaultPhaseId::new(13));
        assert_eq!(
            page.toggle(non_boolean),
            AssaultPhaseToggleOutcome::Toggled {
                phase: AssaultPhaseId::new(13),
                value: 1u32.wrapping_sub(5),
            }
        );
    }

    #[test]
    fn unavailable_unknown_and_stale_targets_do_not_change_state() {
        let mut page = page();
        let before = page.clone();
        assert_eq!(
            page.toggle(page.target(AssaultPhaseId::new(12))),
            AssaultPhaseToggleOutcome::Ignored
        );
        assert_eq!(
            page.toggle(page.target(AssaultPhaseId::new(99))),
            AssaultPhaseToggleOutcome::Ignored
        );
        let stale = AssaultPhaseToggleTarget {
            generation: AssaultPhaseToggleGeneration::new(6),
            phase: AssaultPhaseId::new(11),
        };
        assert_eq!(page.toggle(stale), AssaultPhaseToggleOutcome::Ignored);
        assert_eq!(page, before);
    }

    #[test]
    fn duplicate_phase_ids_are_rejected() {
        assert!(
            AssaultPhaseTogglePage::new(
                AssaultPhaseToggleGeneration::new(1),
                vec![
                    AssaultPhaseToggleRow::new(AssaultPhaseId::new(1), true, 0),
                    AssaultPhaseToggleRow::new(AssaultPhaseId::new(1), true, 1),
                ],
            )
            .is_none()
        );
    }
}
