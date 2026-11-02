// SPDX-License-Identifier: GPL-3.0-or-later
//! T3820: the replay side of the `D` rows, without Bevy: rows become due at their recorded host poll, the
//! lockstep learns which poll is fully delivered and processed, and the recorded routing decisions are
//! compared with the replay's own.

use crate::codec::{DeviceEvent, RouteKind, decode};
use std::collections::{HashMap, VecDeque};

/// Recorded device rows of one replay, in file order.
#[derive(Debug, Default)]
pub struct Schedule {
    rows: VecDeque<(u64, String)>,
    /// Host poll of the earliest row handed out but not yet processed by the consumers.
    unprocessed: Option<u64>,
    expected: HashMap<RouteKind, VecDeque<(u64, String)>>,
    pub delivered: u64,
    pub routes_checked: u64,
    pub route_mismatches: u64,
    pub first_mismatch: Option<String>,
    pub undecodable: u64,
    /// Recorded decisions in effect at the replay poll, by name.
    decisions: HashMap<String, String>,
    pub decisions_applied: u64,
    pub decisions_overridden: u64,
}

impl Schedule {
    pub fn new(rows: impl IntoIterator<Item = (u64, String)>) -> Self {
        Self {
            rows: rows.into_iter().collect(),
            ..Self::default()
        }
    }

    /// The device events due at `poll` (recorded host poll at or before it), in file order. Routing rows are
    /// kept for [`Schedule::observe_route`]; they are not events.
    pub fn due(&mut self, poll: u64) -> Vec<DeviceEvent> {
        let mut events = Vec::new();
        while let Some((row_poll, _)) = self.rows.front()
            && *row_poll <= poll
        {
            let (row_poll, payload) = self.rows.pop_front().expect("front row");
            self.unprocessed = Some(self.unprocessed.map_or(row_poll, |at| at.min(row_poll)));
            match decode(&payload) {
                Ok(DeviceEvent::Decision(name, value)) => {
                    self.decisions_applied += 1;
                    self.decisions.insert(name, value);
                }
                Ok(DeviceEvent::Route(kind, route)) => {
                    self.expected
                        .entry(kind)
                        .or_default()
                        .push_back((row_poll, route));
                }
                Ok(event) => {
                    self.delivered += 1;
                    events.push(event);
                }
                Err(_) => self.undecodable += 1,
            }
        }
        events
    }

    /// The consumers ran on everything handed out so far.
    pub fn processed(&mut self) -> bool {
        self.unprocessed.take().is_some()
    }

    /// Highest host poll whose rows are all delivered and processed (`u64::MAX` when none remain).
    pub fn ready_poll(&self) -> u64 {
        let next = self.rows.front().map(|(poll, _)| *poll);
        match (self.unprocessed, next) {
            (None, None) => u64::MAX,
            (at, next) => at
                .into_iter()
                .chain(next)
                .min()
                .unwrap_or(u64::MAX)
                .saturating_sub(1),
        }
    }

    pub fn remaining(&self) -> usize {
        self.rows.len()
    }

    /// The replay routed a replayed key or mouse button to `route`; compare with the recording's decision.
    pub fn observe_route(&mut self, kind: RouteKind, route: &str, poll: u64) {
        self.routes_checked += 1;
        let recorded = self.expected.entry(kind).or_default().pop_front();
        if recorded.as_ref().is_some_and(|(_, name)| name == route) {
            return;
        }
        self.route_mismatches += 1;
        if self.first_mismatch.is_none() {
            self.first_mismatch = Some(match recorded {
                Some((at, name)) => format!(
                    "{kind:?} recorded at poll {at} went to {name}, the replay sent it to {route} at poll {poll}"
                ),
                None => {
                    format!("{kind:?} routed to {route} at poll {poll} has no recorded routing row")
                }
            });
        }
    }

    /// The recorded value of decision `name` at this point of the replay (`None` before its first row): it wins over
    /// the replay's own `live` decision, which only counts as overridden when it differs.
    pub fn decision(&mut self, name: &str, live: &str) -> Option<String> {
        let recorded = self.decisions.get(name)?.clone();
        if recorded != live {
            self.decisions_overridden += 1;
        }
        Some(recorded)
    }

    /// The recorded value of decision `name` now, for a consumer whose own state is expected to differ from the
    /// recording (not counted as overridden, unlike [`Schedule::decision`]).
    pub fn recorded(&self, name: &str) -> Option<&str> {
        self.decisions.get(name).map(String::as_str)
    }

    /// Stop line for the host log.
    pub fn summary(&self) -> String {
        format!(
            "device rows delivered={} undelivered={} undecodable={} decisions applied={} overridden={} routes checked={} mismatched={}{}",
            self.delivered,
            self.rows.len(),
            self.undecodable,
            self.decisions_applied,
            self.decisions_overridden,
            self.routes_checked,
            self.route_mismatches,
            self.first_mismatch
                .as_ref()
                .map_or_else(String::new, |first| format!(" (first: {first})"))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::encode;

    fn row(poll: u64, event: DeviceEvent) -> (u64, String) {
        (poll, encode(&event))
    }

    #[test]
    fn rows_become_due_at_their_poll_and_hold_the_lockstep_until_processed() {
        let mut schedule = Schedule::new([
            row(5, DeviceEvent::FocusLost),
            row(5, DeviceEvent::Route(RouteKind::Key, "router".into())),
            row(9, DeviceEvent::Focus(true)),
        ]);
        assert_eq!(schedule.ready_poll(), 4);
        assert!(schedule.due(4).is_empty());
        assert_eq!(schedule.due(6), vec![DeviceEvent::FocusLost]);
        assert_eq!(schedule.ready_poll(), 4, "handed out, not yet processed");
        assert!(schedule.processed());
        assert_eq!(schedule.ready_poll(), 8);
        assert_eq!(schedule.due(100), vec![DeviceEvent::Focus(true)]);
        assert!(schedule.processed());
        assert_eq!(schedule.ready_poll(), u64::MAX);
        assert!(!schedule.processed());
        assert_eq!(schedule.delivered, 2);
    }

    #[test]
    fn routes_are_compared_per_device_kind_and_the_first_mismatch_is_named() {
        let mut schedule = Schedule::new([
            row(1, DeviceEvent::Route(RouteKind::Key, "text-entry".into())),
            row(
                1,
                DeviceEvent::Route(RouteKind::MouseButton, "ui-fallback".into()),
            ),
            row(2, DeviceEvent::Route(RouteKind::Key, "router".into())),
        ]);
        schedule.due(2);
        // Keys are routed before mouse buttons in one update; the kinds keep their own order.
        schedule.observe_route(RouteKind::Key, "text-entry", 2);
        schedule.observe_route(RouteKind::Key, "ui-live", 2);
        schedule.observe_route(RouteKind::MouseButton, "ui-fallback", 2);
        schedule.observe_route(RouteKind::Key, "router", 3);
        assert_eq!((schedule.routes_checked, schedule.route_mismatches), (4, 2));
        let first = schedule.first_mismatch.clone().unwrap();
        assert!(
            first.contains("went to router") && first.contains("ui-live"),
            "{first}"
        );
        assert!(schedule.summary().contains("mismatched=2"));
    }

    #[test]
    fn recorded_decisions_win_from_their_poll_on() {
        let decision = |value: &str| DeviceEvent::Decision("raw-look".into(), value.into());
        let mut schedule = Schedule::new([row(3, decision("1")), row(8, decision("0"))]);
        assert_eq!(
            schedule.decision("raw-look", "0"),
            None,
            "before its first row"
        );
        assert!(schedule.due(3).is_empty(), "decisions are not events");
        assert_eq!(schedule.decision("raw-look", "0").as_deref(), Some("1"));
        assert_eq!(schedule.decision("raw-look", "1").as_deref(), Some("1"));
        schedule.due(9);
        assert_eq!(schedule.decision("raw-look", "1").as_deref(), Some("0"));
        assert_eq!(
            (schedule.decisions_applied, schedule.decisions_overridden),
            (2, 2)
        );
        assert!(
            schedule
                .summary()
                .contains("decisions applied=2 overridden=2")
        );
    }

    #[test]
    fn recorded_decisions_can_be_read_without_counting_an_override() {
        let mut schedule =
            Schedule::new([row(3, DeviceEvent::Decision("control".into(), "1".into()))]);
        assert_eq!(schedule.recorded("control"), None);
        schedule.due(3);
        assert_eq!(schedule.recorded("control"), Some("1"));
        assert_eq!(schedule.decisions_overridden, 0);
    }

    #[test]
    fn undecodable_rows_are_counted_not_delivered() {
        let mut schedule = Schedule::new([(1, "k:bad".to_owned())]);
        assert!(schedule.due(1).is_empty());
        assert_eq!(schedule.undecodable, 1);
    }
}
