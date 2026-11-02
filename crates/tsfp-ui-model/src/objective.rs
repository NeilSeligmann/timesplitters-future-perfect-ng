//! T2191 UI-8: the HUD objective text line as typed Rust. FABRICATED model, opt-in until validated.
//!
//! An objective notice is MEASURED to exist on screen (T1245 run: a new objective shown during
//! Story play), but its producer and drawer are not identified (live objective tracing is T1665;
//! it may travel through the message ring of `messages`). This model holds the current objective
//! text, restarts its display age when the objective changes, and fades with the message fade rule
//! so a measured producer only has to call [`ObjectiveLine::set`]. Contract:
//! docs/t2191-ingame-ui.md, section "Ammo, crosshair and objective models".

use crate::messages::fade_alpha;

/// Display timing, supplied at run time (FABRICATED until the original is found).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectiveTuning {
    pub fade_start: f32,
    pub end: f32,
    pub one: f32,
}

/// The current objective and how long it has been displayed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectiveLine {
    /// Objective label id and its resolved text; None before the first objective.
    pub current: Option<(u32, String)>,
    pub age: f32,
}

impl ObjectiveLine {
    /// Set the objective. Re-setting the same label keeps the age (no flash on repeats).
    pub fn set(&mut self, label: u32, text: String) {
        if self.current.as_ref().map(|(id, _)| *id) != Some(label) {
            self.age = 0.0;
        }
        self.current = Some((label, text));
    }

    pub fn clear(&mut self) {
        self.current = None;
    }

    pub fn tick(&mut self, seconds: f32) {
        self.age += seconds;
    }

    /// Text and opacity to draw this frame, None once faded out or unset.
    pub fn visible(&self, tuning: &ObjectiveTuning) -> Option<(&str, f32)> {
        let (_, text) = self.current.as_ref()?;
        if self.age >= tuning.end {
            return None;
        }
        Some((
            text,
            fade_alpha(self.age, tuning.fade_start, tuning.end, tuning.one),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TUNING: ObjectiveTuning = ObjectiveTuning {
        fade_start: 3.0,
        end: 5.0,
        one: 1.0,
    };

    #[test]
    fn new_objective_restarts_and_fades_out() {
        let mut line = ObjectiveLine::default();
        assert_eq!(line.visible(&TUNING), None);
        line.set(7, "A".into());
        line.tick(4.0);
        assert_eq!(line.visible(&TUNING), Some(("A", 0.5)));
        line.set(7, "A".into());
        assert_eq!(line.age, 4.0);
        line.tick(1.0);
        assert_eq!(line.visible(&TUNING), None);
        line.set(8, "B".into());
        assert_eq!(line.visible(&TUNING), Some(("B", 1.0)));
        line.clear();
        assert_eq!(line.visible(&TUNING), None);
    }
}
