// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordered replay of direct clock reads from a capture.
//!
//! The caller supplies the number of immediate callees already consumed and applies the
//! captured writes to its fault-tracked arena. A mismatch is sticky: a later call cannot
//! turn an incomplete or out-of-order replay into a successful verdict.

use crate::spool::{ClockEvent, Run};

pub struct ClockReplay<'a> {
    events: &'a [ClockEvent],
    next: usize,
    error: Option<String>,
}

impl<'a> ClockReplay<'a> {
    pub fn new(events: &'a [ClockEvent]) -> Self {
        Self { events, next: 0, error: None }
    }

    pub fn read(
        &mut self,
        callees: usize,
        mut write: impl FnMut(&Run) -> Result<(), String>,
    ) -> Result<u64, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let result = match self.events.get(self.next) {
            None => Err(format!("unexpected clock read after {callees} callees")),
            Some(event) if event.after_callees as usize != callees => Err(format!(
                "clock read {} after {callees} callees, expected {}",
                self.next, event.after_callees
            )),
            Some(event) => {
                let applied = event.writes.iter().try_for_each(&mut write);
                applied.map(|()| event.value)
            }
        };
        match &result {
            Ok(_) => self.next += 1,
            Err(error) => self.error = Some(error.clone()),
        }
        result
    }

    pub fn finish(&self) -> Result<(), String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self.next != self.events.len() {
            return Err(format!("missing clock read {} of {}", self.next, self.events.len()));
        }
        Ok(())
    }
}
