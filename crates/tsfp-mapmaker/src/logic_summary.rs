//! Typed state for the Map Maker Logic Summary page (T2408).
//!
//! Row identity is semantic and generation-scoped. Runtime labels, localized
//! values, widget addresses, and effect handlers stay at the UI adapter seam.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogicSummaryAction {
    AssaultPhase,
    AssaultObjective,
    FirstTriggerList,
    FirstActionList,
    ObjectiveList,
    EditName,
    SecondTriggerList,
    SecondActionList,
    Done,
}

pub const LOGIC_SUMMARY_ACTIONS: [LogicSummaryAction; 9] = [
    LogicSummaryAction::AssaultPhase,
    LogicSummaryAction::AssaultObjective,
    LogicSummaryAction::FirstTriggerList,
    LogicSummaryAction::FirstActionList,
    LogicSummaryAction::ObjectiveList,
    LogicSummaryAction::EditName,
    LogicSummaryAction::SecondTriggerList,
    LogicSummaryAction::SecondActionList,
    LogicSummaryAction::Done,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogicSummaryGeneration(u64);

impl LogicSummaryGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogicSummaryTarget {
    generation: LogicSummaryGeneration,
    action: LogicSummaryAction,
}

impl LogicSummaryTarget {
    pub const fn action(self) -> LogicSummaryAction {
        self.action
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicSummaryUpdate {
    Focused(LogicSummaryAction),
    ActionRequested(LogicSummaryAction),
    BackRequested,
}

/// Rust-owned interaction state for one opening of the Logic Summary page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicSummaryPageState {
    generation: LogicSummaryGeneration,
    focused: Option<LogicSummaryAction>,
}

impl LogicSummaryPageState {
    pub const fn new(generation: LogicSummaryGeneration) -> Self {
        Self {
            generation,
            focused: None,
        }
    }

    pub const fn generation(&self) -> LogicSummaryGeneration {
        self.generation
    }

    pub const fn focused(&self) -> Option<LogicSummaryAction> {
        self.focused
    }

    pub fn target(&self, action: LogicSummaryAction) -> Option<LogicSummaryTarget> {
        LOGIC_SUMMARY_ACTIONS
            .contains(&action)
            .then_some(LogicSummaryTarget {
                generation: self.generation,
                action,
            })
    }

    pub fn focus(&mut self, target: LogicSummaryTarget) -> Option<LogicSummaryUpdate> {
        if target.generation != self.generation || !LOGIC_SUMMARY_ACTIONS.contains(&target.action) {
            return None;
        }
        self.focused = Some(target.action);
        Some(LogicSummaryUpdate::Focused(target.action))
    }

    pub fn activate(&mut self, target: LogicSummaryTarget) -> Option<LogicSummaryUpdate> {
        self.focus(target)?;
        Some(LogicSummaryUpdate::ActionRequested(target.action))
    }

    pub const fn back(&self) -> LogicSummaryUpdate {
        LogicSummaryUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_typed_and_generation_scoped() {
        let generation = LogicSummaryGeneration::new(7);
        let mut state = LogicSummaryPageState::new(generation);
        let target = state.target(LogicSummaryAction::SecondTriggerList).unwrap();
        assert_eq!(target.action(), LogicSummaryAction::SecondTriggerList);
        assert_eq!(
            state.focus(target),
            Some(LogicSummaryUpdate::Focused(
                LogicSummaryAction::SecondTriggerList
            ))
        );
        assert_eq!(
            state.activate(target),
            Some(LogicSummaryUpdate::ActionRequested(
                LogicSummaryAction::SecondTriggerList
            ))
        );

        let stale = LogicSummaryTarget {
            generation: LogicSummaryGeneration::new(6),
            action: LogicSummaryAction::Done,
        };
        assert_eq!(state.activate(stale), None);
        assert_eq!(state.focused(), Some(LogicSummaryAction::SecondTriggerList));
    }
}
