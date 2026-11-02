//! Native Story owner dispatch at the gameplay tick boundary (T2494).
//!
//! The host provides a fully typed mission and implements the existing gameplay `World`
//! boundary. The native branch never reads guest state. Selection is default-off so a caller
//! can retain its lifted path until replay adoption is accepted.

use crate::story_events::engine::StepReport;
use crate::story_events::world::World;
use crate::story_mission::StoryMission;

/// Story simulation advances on the repository's fixed 60 Hz gameplay clock.
pub const STORY_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StoryExecution {
    /// Keep running the caller's existing lifted implementation.
    #[default]
    Lifted,
    /// Step the Rust-owned Story mission with the typed gameplay world.
    Native,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StorySchedule {
    execution: StoryExecution,
}

impl StorySchedule {
    pub const fn new(execution: StoryExecution) -> Self {
        Self { execution }
    }

    pub const fn execution(self) -> StoryExecution {
        self.execution
    }

    pub fn set_execution(&mut self, execution: StoryExecution) {
        self.execution = execution;
    }

    /// Dispatch one fixed gameplay step. The lifted closure owns its existing host call; the
    /// native branch advances only the supplied typed Story owner and world.
    pub fn step(
        &mut self,
        dt: f32,
        mission: &mut StoryMission,
        world: &mut dyn World,
        lifted: impl FnOnce() -> StepReport,
    ) -> StepReport {
        self.dispatch(lifted, || mission.step(dt, world))
    }

    /// Advance one simulation tick using the fixed gameplay interval. The caller invokes this
    /// from its 60 Hz simulation schedule; presentation may run at display rate independently.
    pub fn step_fixed(
        &mut self,
        mission: &mut StoryMission,
        world: &mut dyn World,
        lifted: impl FnOnce() -> StepReport,
    ) -> StepReport {
        self.step(STORY_FIXED_STEP_SECONDS, mission, world, lifted)
    }

    /// Testable form of the schedule selection for hosts that already own their
    /// branch-specific execution closures.
    pub fn dispatch(
        &mut self,
        lifted: impl FnOnce() -> StepReport,
        native: impl FnOnce() -> StepReport,
    ) -> StepReport {
        match self.execution {
            StoryExecution::Lifted => lifted(),
            StoryExecution::Native => native(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_defaults_to_lifted_and_can_opt_into_native_owner() {
        let mut lifted_called = false;
        let mut native_called = false;
        let mut schedule = StorySchedule::default();
        schedule.dispatch(
            || {
                lifted_called = true;
                StepReport::default()
            },
            || {
                native_called = true;
                StepReport::default()
            },
        );
        assert!(lifted_called);
        assert!(!native_called);

        lifted_called = false;
        native_called = false;
        schedule.set_execution(StoryExecution::Native);
        schedule.dispatch(
            || {
                lifted_called = true;
                StepReport::default()
            },
            || {
                native_called = true;
                StepReport::default()
            },
        );
        assert!(!lifted_called);
        assert!(native_called);
        assert_eq!(schedule.execution(), StoryExecution::Native);
    }
}
