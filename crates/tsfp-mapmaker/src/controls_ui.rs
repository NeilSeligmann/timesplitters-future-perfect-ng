// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned interaction state for the Map Maker Controls page.
//!
//! The live page supplies its measured row order and rendering data. This
//! model owns only generation-scoped focus and Back intent; it does not assign
//! gameplay meaning to the opaque control rows.

pub const MEASURED_CONTROLS_ROW_COUNT: usize = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ControlsPageGeneration(u64);

impl ControlsPageGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ControlsRowId(u8);

impl ControlsRowId {
    pub const fn new(value: u8) -> Option<Self> {
        if (value as usize) < MEASURED_CONTROLS_ROW_COUNT {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ControlsRowTarget {
    generation: ControlsPageGeneration,
    row: ControlsRowId,
}

impl ControlsRowTarget {
    pub const fn new(generation: ControlsPageGeneration, row: ControlsRowId) -> Self {
        Self { generation, row }
    }

    pub const fn generation(self) -> ControlsPageGeneration {
        self.generation
    }

    pub const fn row(self) -> ControlsRowId {
        self.row
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlsFocusStep {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlsPageCommand {
    Focus(ControlsRowTarget),
    Step(ControlsFocusStep),
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlsPageUpdate {
    Focused(ControlsRowId),
    BackRequested,
    Ignored,
}

/// Focus state for one observed Controls page. Row ordinals are local to this
/// page generation and retain no guest node address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlsPageState {
    generation: ControlsPageGeneration,
    focused: Option<ControlsRowId>,
}

impl ControlsPageState {
    pub const fn new(
        generation: ControlsPageGeneration,
        measured_row_count: usize,
        focused: Option<ControlsRowId>,
    ) -> Option<Self> {
        if generation.get() == 0
            || measured_row_count != MEASURED_CONTROLS_ROW_COUNT
            || match focused {
                Some(row) => (row.get() as usize) >= measured_row_count,
                None => false,
            }
        {
            return None;
        }
        Some(Self {
            generation,
            focused,
        })
    }

    pub const fn generation(self) -> ControlsPageGeneration {
        self.generation
    }

    pub const fn focused(self) -> Option<ControlsRowId> {
        self.focused
    }

    pub const fn target(&self, row: ControlsRowId) -> ControlsRowTarget {
        ControlsRowTarget::new(self.generation, row)
    }

    pub fn dispatch(&mut self, command: ControlsPageCommand) -> ControlsPageUpdate {
        match command {
            ControlsPageCommand::Focus(target) if target.generation == self.generation => {
                self.focused = Some(target.row);
                ControlsPageUpdate::Focused(target.row)
            }
            ControlsPageCommand::Step(direction) => {
                let next = match (self.focused, direction) {
                    (Some(row), ControlsFocusStep::Up) => row.get().checked_sub(1),
                    (Some(row), ControlsFocusStep::Down) => row
                        .get()
                        .checked_add(1)
                        .filter(|next| (*next as usize) < MEASURED_CONTROLS_ROW_COUNT),
                    (None, ControlsFocusStep::Up) => Some((MEASURED_CONTROLS_ROW_COUNT - 1) as u8),
                    (None, ControlsFocusStep::Down) => Some(0),
                };
                next.and_then(ControlsRowId::new)
                    .map_or(ControlsPageUpdate::Ignored, |row| {
                        self.focused = Some(row);
                        ControlsPageUpdate::Focused(row)
                    })
            }
            ControlsPageCommand::Back => ControlsPageUpdate::BackRequested,
            ControlsPageCommand::Focus(_) => ControlsPageUpdate::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_focus_is_generation_scoped_and_preserves_runtime_row_order() {
        let generation = ControlsPageGeneration::new(7);
        let mut state = ControlsPageState::new(
            generation,
            MEASURED_CONTROLS_ROW_COUNT,
            Some(ControlsRowId::new(8).unwrap()),
        )
        .unwrap();
        assert_eq!(
            state.dispatch(ControlsPageCommand::Step(ControlsFocusStep::Down)),
            ControlsPageUpdate::Focused(ControlsRowId::new(9).unwrap())
        );
        assert_eq!(
            state.dispatch(ControlsPageCommand::Focus(ControlsRowTarget::new(
                ControlsPageGeneration::new(6),
                ControlsRowId::new(2).unwrap(),
            ))),
            ControlsPageUpdate::Ignored
        );
        assert_eq!(state.focused(), Some(ControlsRowId::new(9).unwrap()));
    }

    #[test]
    fn controls_state_fails_closed_on_unmeasured_page_shapes() {
        assert!(ControlsPageState::new(ControlsPageGeneration::new(1), 17, None).is_none());
        assert!(ControlsPageState::new(ControlsPageGeneration::new(0), 18, None).is_none());
    }
}
