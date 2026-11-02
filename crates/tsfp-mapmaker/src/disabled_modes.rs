//! Rust-owned state for the Map Settings per-mode disable checklist.
//!
//! Row identities stay opaque until the original mode-to-document mapping is
//! established. Each row stores an independent disabled flag; this is not a
//! single selected game mode.

extern crate alloc;

use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisabledModesUpdateBatch {
    pub generation: DisabledModesGeneration,
    pub updates: Vec<DisabledModesUpdate>,
}

impl DisabledModesUpdateBatch {
    /// Consume this batch only while its originating checklist page is active.
    /// A stale batch is returned intact so the caller can diagnose or discard it.
    pub fn into_updates_for(
        self,
        active_generation: DisabledModesGeneration,
    ) -> Result<Vec<DisabledModesUpdate>, Self> {
        if self.generation == active_generation {
            Ok(self.updates)
        } else {
            Err(self)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisabledModesGeneration(u64);

impl DisabledModesGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisabledModeId(u64);

impl DisabledModeId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisabledModeTarget {
    generation: DisabledModesGeneration,
    id: DisabledModeId,
}

impl DisabledModeTarget {
    pub const fn new(generation: DisabledModesGeneration, id: DisabledModeId) -> Self {
        Self { generation, id }
    }

    pub const fn id(self) -> DisabledModeId {
        self.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisabledModeRow {
    pub id: DisabledModeId,
    /// `true` means this mode is disabled for the map.
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisabledModesError {
    DuplicateIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisabledModesUpdate {
    Ignored,
    Focused(DisabledModeId),
    Toggled { id: DisabledModeId, disabled: bool },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisabledModesPage {
    generation: Option<DisabledModesGeneration>,
    rows: Vec<DisabledModeRow>,
    focused: Option<DisabledModeId>,
}

impl DisabledModesPage {
    pub fn install(
        &mut self,
        generation: DisabledModesGeneration,
        rows: Vec<DisabledModeRow>,
    ) -> Result<(), DisabledModesError> {
        for (index, row) in rows.iter().enumerate() {
            if rows[..index].iter().any(|prior| prior.id == row.id) {
                return Err(DisabledModesError::DuplicateIdentity);
            }
        }
        let same_generation = self.generation == Some(generation);
        if !same_generation || !rows.iter().any(|row| Some(row.id) == self.focused) {
            self.focused = None;
        }
        self.generation = Some(generation);
        self.rows = rows;
        Ok(())
    }

    pub fn generation(&self) -> Option<DisabledModesGeneration> {
        self.generation
    }

    pub fn rows(&self) -> &[DisabledModeRow] {
        &self.rows
    }

    pub fn focused(&self) -> Option<DisabledModeId> {
        self.focused
    }

    pub fn target(&self, id: DisabledModeId) -> Option<DisabledModeTarget> {
        let generation = self.generation?;
        self.rows
            .iter()
            .any(|row| row.id == id)
            .then_some(DisabledModeTarget::new(generation, id))
    }

    pub fn focus(&mut self, target: DisabledModeTarget) -> DisabledModesUpdate {
        if !self.contains(target) {
            return DisabledModesUpdate::Ignored;
        }
        self.focused = Some(target.id);
        DisabledModesUpdate::Focused(target.id)
    }

    pub fn toggle(&mut self, target: DisabledModeTarget) -> DisabledModesUpdate {
        if !self.contains(target) {
            return DisabledModesUpdate::Ignored;
        }
        let Some(row) = self.rows.iter_mut().find(|row| row.id == target.id) else {
            return DisabledModesUpdate::Ignored;
        };
        row.disabled = !row.disabled;
        self.focused = Some(target.id);
        DisabledModesUpdate::Toggled {
            id: target.id,
            disabled: row.disabled,
        }
    }

    fn contains(&self, target: DisabledModeTarget) -> bool {
        self.generation == Some(target.generation)
            && self.rows.iter().any(|row| row.id == target.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(generation: u64) -> DisabledModesPage {
        let mut page = DisabledModesPage::default();
        page.install(
            DisabledModesGeneration::new(generation),
            vec![
                DisabledModeRow {
                    id: DisabledModeId::new(10),
                    disabled: false,
                },
                DisabledModeRow {
                    id: DisabledModeId::new(20),
                    disabled: true,
                },
            ],
        )
        .unwrap();
        page
    }

    #[test]
    fn each_mode_flag_toggles_independently() {
        let mut page = page(3);
        let first = page.target(DisabledModeId::new(10)).unwrap();
        let second = page.target(DisabledModeId::new(20)).unwrap();

        assert_eq!(
            page.toggle(first),
            DisabledModesUpdate::Toggled {
                id: first.id(),
                disabled: true,
            }
        );
        assert_eq!(page.rows()[0].disabled, true);
        assert_eq!(page.rows()[1].disabled, true);
        assert_eq!(
            page.toggle(second),
            DisabledModesUpdate::Toggled {
                id: second.id(),
                disabled: false,
            }
        );
        assert_eq!(page.rows()[1].disabled, false);
    }

    #[test]
    fn stale_generation_and_duplicate_rows_are_rejected() {
        let mut page = page(3);
        let stale =
            DisabledModeTarget::new(DisabledModesGeneration::new(2), DisabledModeId::new(10));
        assert_eq!(page.toggle(stale), DisabledModesUpdate::Ignored);
        assert_eq!(
            page.install(
                DisabledModesGeneration::new(4),
                vec![
                    DisabledModeRow {
                        id: DisabledModeId::new(1),
                        disabled: false
                    },
                    DisabledModeRow {
                        id: DisabledModeId::new(1),
                        disabled: true
                    },
                ],
            ),
            Err(DisabledModesError::DuplicateIdentity)
        );
    }

    #[test]
    fn replacing_the_page_generation_clears_focus() {
        let mut page = page(3);
        let id = DisabledModeId::new(10);
        page.focus(page.target(id).unwrap());
        page.install(
            DisabledModesGeneration::new(4),
            vec![DisabledModeRow {
                id,
                disabled: false,
            }],
        )
        .unwrap();
        assert_eq!(page.focused(), None);
    }

    #[test]
    fn batch_keeps_the_page_generation_for_the_caller() {
        let generation = DisabledModesGeneration::new(9);
        let update = DisabledModesUpdate::Toggled {
            id: DisabledModeId::new(4),
            disabled: true,
        };
        let batch = DisabledModesUpdateBatch {
            generation,
            updates: vec![update],
        };

        assert_eq!(batch.generation, generation);
        assert_eq!(batch.updates, vec![update]);
    }

    #[test]
    fn update_batch_only_consumes_for_matching_generation() {
        let generation = DisabledModesGeneration::new(9);
        let stale = DisabledModesGeneration::new(10);
        let update = DisabledModesUpdate::Focused(DisabledModeId::new(4));
        let batch = DisabledModesUpdateBatch {
            generation,
            updates: vec![update],
        };

        assert_eq!(batch.clone().into_updates_for(generation), Ok(vec![update]));
        assert_eq!(batch.clone().into_updates_for(stale), Err(batch));
    }
}
