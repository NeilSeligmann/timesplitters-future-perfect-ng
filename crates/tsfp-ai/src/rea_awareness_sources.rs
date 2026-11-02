// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordered, typed source-alert processing shared with the runtime lane.

use alloc::vec::Vec;

use crate::{
    navigation_scoring::{ActorId, NodeId, ObjectId},
    rea_ai_types::{GroupMask, NodeKey},
};

/// Alert payload variants stay distinct because they read different fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertSource {
    Actor {
        actor: ActorId,
        mask: GroupMask,
        excluded_actor: Option<ActorId>,
        node: Option<NodeId>,
    },
    RecordOne {
        mask: GroupMask,
        target: Option<ObjectId>,
    },
    RecordTwo {
        mask: GroupMask,
        node_key: NodeKey,
        node: Option<NodeId>,
    },
    OriginalDefault,
}

/// The fields read or updated by lifted `0x25FAF0` (record stride `0x20`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AwarenessSourceRecord {
    pub alert_mask: GroupMask,
    pub filter_mask: GroupMask,
    pub target: Option<ObjectId>,
    pub state: i32,
    pub elapsed: f32,
}

/// Rust-owned source records for the measured `0x25FAF0` table.
///
/// The store deliberately has no fixed capacity: the observed loop reloads
/// its count on every iteration, and REA has not established a maximum.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReaAwarenessSourceStore {
    records: Vec<AwarenessSourceRecord>,
}

impl ReaAwarenessSourceStore {
    pub fn new(records: Vec<AwarenessSourceRecord>) -> Self {
        Self { records }
    }

    pub fn records(&self) -> &[AwarenessSourceRecord] {
        &self.records
    }

    pub fn push(&mut self, record: AwarenessSourceRecord) {
        self.records.push(record);
    }

    pub fn replace(&mut self, index: usize, record: AwarenessSourceRecord) -> bool {
        let Some(slot) = self.records.get_mut(index) else {
            return false;
        };
        *slot = record;
        true
    }
}

/// Non-record effects for an owned source pass. `dispatch_alert` receives the
/// store only after the admitted record has been committed, so a nested world
/// handler can observe or update native-owned source values before the outer
/// loop reloads its count.
pub trait OwnedAwarenessAlertEffects {
    fn set_group_alert_value(&mut self, value: u32);
    fn dispatch_alert(&mut self, sources: &mut ReaAwarenessSourceStore, source: AlertSource);
    fn register_spawn_entry(&mut self, index: usize);
    fn increment_alert_count(&mut self);
}

/// Apply the measured `0x25FAF0` update order to Rust-owned source records.
pub fn process_owned_awareness_sources(
    incoming: GroupMask,
    sources: &mut ReaAwarenessSourceStore,
    effects: &mut impl OwnedAwarenessAlertEffects,
) -> usize {
    let mut index = 0;
    let mut admitted = 0;
    while index < sources.records.len() {
        let mut record = sources.records[index];
        if record.state != 1 && record.filter_mask.0 & incoming.0 != 0 && record.state != 2 {
            effects.set_group_alert_value(2);
            record.target = None;
            record.elapsed = 0.0;
            record.state = 1;
            sources.records[index] = record;
            effects.dispatch_alert(
                sources,
                AlertSource::RecordOne {
                    mask: record.alert_mask,
                    target: record.target,
                },
            );
            effects.register_spawn_entry(index);
            effects.increment_alert_count();
            admitted += 1;
        }
        index += 1;
    }
    admitted
}

/// Effects remain separate so reentrant handlers observe each preceding write.
pub trait AwarenessAlertWorld {
    fn source_count(&self) -> usize;
    fn source_record(&self, index: usize) -> Option<AwarenessSourceRecord>;
    fn write_source_record(&mut self, index: usize, record: AwarenessSourceRecord);
    fn set_group_alert_value(&mut self, value: u32);
    fn dispatch_alert(&mut self, source: AlertSource);
    fn register_spawn_entry(&mut self, index: usize);
    fn increment_alert_count(&mut self);
}

/// Apply the measured source-update loop from `0x25FAF0` in original order.
pub fn process_awareness_sources(
    incoming: GroupMask,
    world: &mut impl AwarenessAlertWorld,
) -> usize {
    let mut index = 0;
    let mut admitted = 0;
    loop {
        if index >= world.source_count() {
            break;
        }
        let mut record = world
            .source_record(index)
            .expect("source count must name an owned source record");
        if record.state != 1 && record.filter_mask.0 & incoming.0 != 0 && record.state != 2 {
            world.set_group_alert_value(2);
            record.target = None;
            record.elapsed = 0.0;
            record.state = 1;
            world.write_source_record(index, record);
            world.dispatch_alert(AlertSource::RecordOne {
                mask: record.alert_mask,
                target: record.target,
            });
            world.register_spawn_entry(index);
            world.increment_alert_count();
            admitted += 1;
        }
        index += 1;
    }
    admitted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum Event {
        Group(u32),
        Dispatch(AlertSource),
        Register(usize),
        Increment,
    }

    #[derive(Default)]
    struct Effects {
        events: Vec<Event>,
        observed_first_record: Vec<Option<AwarenessSourceRecord>>,
        append_during_dispatch: Option<AwarenessSourceRecord>,
    }

    impl OwnedAwarenessAlertEffects for Effects {
        fn set_group_alert_value(&mut self, value: u32) {
            self.events.push(Event::Group(value));
        }

        fn dispatch_alert(&mut self, sources: &mut ReaAwarenessSourceStore, source: AlertSource) {
            self.events.push(Event::Dispatch(source));
            self.observed_first_record
                .push(sources.records().first().copied());
            if let Some(record) = self.append_during_dispatch.take() {
                sources.push(record);
            }
        }

        fn register_spawn_entry(&mut self, index: usize) {
            self.events.push(Event::Register(index));
        }

        fn increment_alert_count(&mut self) {
            self.events.push(Event::Increment);
        }
    }

    fn source_record(state: i32, alert_mask: u32, filter_mask: u32) -> AwarenessSourceRecord {
        AwarenessSourceRecord {
            alert_mask: GroupMask(alert_mask),
            filter_mask: GroupMask(filter_mask),
            target: Some(ObjectId(0x1234)),
            state,
            elapsed: 3.5,
        }
    }

    #[test]
    fn owned_source_store_commits_before_ordered_callbacks_and_reloads_count() {
        let mut sources = ReaAwarenessSourceStore::new(vec![source_record(0, 2, 1)]);
        let mut effects = Effects {
            append_during_dispatch: Some(source_record(0, 4, 1)),
            ..Effects::default()
        };

        assert_eq!(
            process_owned_awareness_sources(GroupMask(1), &mut sources, &mut effects),
            2
        );
        let mut expected = vec![source_record(1, 2, 1), source_record(1, 4, 1)];
        for record in &mut expected {
            record.target = None;
            record.elapsed = 0.0;
        }
        assert_eq!(sources.records(), expected.as_slice());
        assert_eq!(
            effects.observed_first_record[0],
            Some({
                let mut record = source_record(1, 2, 1);
                record.target = None;
                record.elapsed = 0.0;
                record
            })
        );
        assert_eq!(
            effects.events,
            [
                Event::Group(2),
                Event::Dispatch(AlertSource::RecordOne {
                    mask: GroupMask(2),
                    target: None,
                }),
                Event::Register(0),
                Event::Increment,
                Event::Group(2),
                Event::Dispatch(AlertSource::RecordOne {
                    mask: GroupMask(4),
                    target: None,
                }),
                Event::Register(1),
                Event::Increment,
            ]
        );
    }

    #[test]
    fn owned_source_store_skips_nonmatching_and_terminal_records() {
        let mut sources = ReaAwarenessSourceStore::new(vec![
            source_record(0, 1, 2),
            source_record(1, 1, 1),
            source_record(2, 1, 1),
        ]);
        let mut effects = Effects::default();

        assert_eq!(
            process_owned_awareness_sources(GroupMask(1), &mut sources, &mut effects),
            0
        );
        assert!(effects.events.is_empty());
        assert_eq!(sources.records()[0], source_record(0, 1, 2));
    }
}
