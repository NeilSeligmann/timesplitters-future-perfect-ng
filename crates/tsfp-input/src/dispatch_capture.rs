// SPDX-License-Identifier: GPL-3.0-or-later
//! Input dispatch lifecycle metadata. This model owns no call snapshots or journal:
//! its returned tokens must be attached to the existing bounded capture frame.

use std::collections::BTreeMap;

/// Identities name registration sites, never process addresses. Unknown handlers
/// cannot be used as evidence of an equivalent captured call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum HandlerId {
    Unknown = 0,
    MuMount = 1,
    MuUnmount = 2,
    InputOpen = 3,
    InputClose = 4,
    InputCapabilities = 5,
    InputState = 6,
    InputFeedback = 7,
    DevicesInit = 8,
    DevicesGet = 9,
    DevicesPeek = 10,
    DevicesChanges = 11,
    DevicesStatus = 12,
    VoiceCreateEx = 13,
}

impl HandlerId {
    /// Unknown and unrecognized wire values never acquire a trusted identity.
    pub fn from_wire(value: u32) -> Self {
        match value {
            1 => Self::MuMount,
            2 => Self::MuUnmount,
            3 => Self::InputOpen,
            4 => Self::InputClose,
            5 => Self::InputCapabilities,
            6 => Self::InputState,
            7 => Self::InputFeedback,
            8 => Self::DevicesInit,
            9 => Self::DevicesGet,
            10 => Self::DevicesPeek,
            11 => Self::DevicesChanges,
            12 => Self::DevicesStatus,
            13 => Self::VoiceCreateEx,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    RustSelected,
    CHandler,
    CStub,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    pub generation: u64,
    pub lifecycle_sequence: u64,
    pub selection_sequence: u64,
    pub address: u32,
    pub supported: bool,
    pub overflow: bool,
    pub handler: HandlerId,
    pub registered: bool,
    pub route: Route,
    pub default_return: u32,
}

/// Bounded registry pre-state, supplied at selection time; it does not prove
/// the sequence of registrations that constructed the state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub generation: u64,
    pub lifecycle_sequence: u64,
    pub complete: bool,
    pub entries: Vec<SnapshotEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotEntry {
    pub address: u32,
    pub handler: Option<HandlerId>,
    pub default_return: u32,
}

impl Selection {
    /// A handler token is incomplete if identity was unavailable. Stub calls
    /// intentionally have no handler and remain distinguishable by their route.
    pub fn identity_complete(self) -> bool {
        if !self.supported || self.overflow || self.generation == 0 {
            return false;
        }
        match self.route {
            Route::CStub => !self.registered,
            Route::RustSelected | Route::CHandler => {
                self.registered && self.handler != HandlerId::Unknown
            }
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Entry {
    handler: Option<HandlerId>,
    default_return: u32,
}

/// Embed this state in the dispatch registry and mutate it while holding the
/// same mutex as handler registration and selection.
pub struct Lifecycle {
    generation: u64,
    lifecycle_sequence: u64,
    selection_sequence: u64,
    entries: BTreeMap<u32, Entry>,
    overflow: bool,
    capacity: usize,
}

fn next(value: &mut u64) -> u64 {
    *value = value
        .checked_add(1)
        .expect("input capture sequence exhausted");
    *value
}

impl Lifecycle {
    /// Capacity is supplied by the dispatch surface owner, not copied here.
    pub fn new(capacity: usize) -> Self {
        Self {
            generation: 0,
            lifecycle_sequence: 0,
            selection_sequence: 0,
            entries: BTreeMap::new(),
            overflow: false,
            capacity,
        }
    }

    pub fn reset(&mut self) -> u64 {
        let state = self;
        let generation = next(&mut state.generation);
        next(&mut state.lifecycle_sequence);
        state.entries.clear();
        state.overflow = false;
        generation
    }

    pub fn register(&mut self, address: u32, handler: HandlerId) -> u64 {
        let state = self;
        if let Some(entry) = state.entry(address) {
            entry.handler = Some(handler);
        }
        next(&mut state.lifecycle_sequence)
    }

    pub fn set_default(&mut self, address: u32, value: u32) -> u64 {
        let state = self;
        if let Some(entry) = state.entry(address) {
            entry.default_return = value;
        }
        next(&mut state.lifecycle_sequence)
    }

    pub fn select(&mut self, address: u32, route: Route, supported: bool) -> Selection {
        let state = self;
        let entry = state.entries.get(&address).copied().unwrap_or_default();
        let selection_sequence = next(&mut state.selection_sequence);
        let lifecycle_sequence = next(&mut state.lifecycle_sequence);
        Selection {
            generation: state.generation,
            lifecycle_sequence,
            selection_sequence,
            address,
            supported,
            overflow: state.overflow,
            handler: entry.handler.unwrap_or(HandlerId::Unknown),
            registered: entry.handler.is_some(),
            route,
            default_return: entry.default_return,
        }
    }

    /// Call while holding the registry lock, before invoking the C handler.
    pub fn select_with_snapshot(
        &mut self,
        address: u32,
        route: Route,
        surface: &[u32],
    ) -> (Selection, Snapshot) {
        let snapshot = self.snapshot(surface);
        let selection = self.select(address, route, surface.contains(&address));
        (selection, snapshot)
    }

    /// The dispatch owner provides its complete surface in stable order.
    /// Duplicate or missing addresses, unknown identities and overflow are
    /// reported as incomplete rather than silently omitting state.
    pub fn snapshot(&self, surface: &[u32]) -> Snapshot {
        let valid_surface = surface.len() == self.capacity
            && surface
                .iter()
                .enumerate()
                .all(|(i, a)| !surface[..i].contains(a))
            && self.entries.keys().all(|a| surface.contains(a));
        let complete = self.generation != 0
            && !self.overflow
            && valid_surface
            && self
                .entries
                .values()
                .all(|e| e.handler != Some(HandlerId::Unknown));
        let entries = if surface.len() <= self.capacity {
            surface
                .iter()
                .map(|&address| {
                    let entry = self.entries.get(&address).copied().unwrap_or_default();
                    SnapshotEntry {
                        address,
                        handler: entry.handler,
                        default_return: entry.default_return,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        Snapshot {
            generation: self.generation,
            lifecycle_sequence: self.lifecycle_sequence,
            complete,
            entries,
        }
    }

    fn entry(&mut self, address: u32) -> Option<&mut Entry> {
        if !self.entries.contains_key(&address) && self.entries.len() >= self.capacity {
            self.overflow = true;
            return None;
        }
        Some(self.entries.entry(address).or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn reset_replace_and_default_are_reflected_in_selection() {
        let mut lifecycle = Lifecycle::new(15);
        assert!(!lifecycle.select(7, Route::CStub, true).identity_complete());
        assert_eq!(lifecycle.reset(), 1);
        lifecycle.set_default(7, 42);
        lifecycle.register(7, HandlerId::InputOpen);
        let first = lifecycle.select(7, Route::RustSelected, true);
        assert!(!lifecycle.select(7, Route::CStub, true).identity_complete());
        lifecycle.register(7, HandlerId::InputClose);
        let replacement = lifecycle.select(7, Route::CHandler, true);
        assert_eq!(
            (first.handler, first.default_return),
            (HandlerId::InputOpen, 42)
        );
        assert_eq!(replacement.handler, HandlerId::InputClose);
        assert!(first.lifecycle_sequence < replacement.lifecycle_sequence);
        assert_eq!(lifecycle.reset(), 2);
        let stub = lifecycle.select(7, Route::CStub, true);
        assert_eq!((stub.generation, stub.default_return), (2, 0));
        assert!(stub.selection_sequence > replacement.selection_sequence);
        assert!(stub.identity_complete());
        assert!(
            !lifecycle
                .select(7, Route::CHandler, true)
                .identity_complete()
        );
        assert!(!lifecycle.select(7, Route::CStub, false).identity_complete());
        lifecycle.register(7, HandlerId::Unknown);
        assert!(
            !lifecycle
                .select(7, Route::RustSelected, true)
                .identity_complete()
        );
    }

    #[test]
    fn concurrent_selections_allocate_contiguous_unique_sequences() {
        let lifecycle = Arc::new(Mutex::new(Lifecycle::new(15)));
        lifecycle.lock().unwrap().register(7, HandlerId::InputState);
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let lifecycle = Arc::clone(&lifecycle);
                std::thread::spawn(move || {
                    (0..64)
                        .map(|_| {
                            lifecycle
                                .lock()
                                .unwrap()
                                .select(7, Route::RustSelected, true)
                                .selection_sequence
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut sequences: Vec<_> = workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect();
        sequences.sort_unstable();
        assert_eq!(sequences, (1..=512).collect::<Vec<_>>());
    }

    #[test]
    fn too_many_entries_remain_incomplete_until_reset() {
        let mut lifecycle = Lifecycle::new(15);
        lifecycle.reset();
        for address in 0..=15 {
            lifecycle.register(address, HandlerId::InputOpen);
        }
        assert!(
            !lifecycle
                .select(0, Route::RustSelected, true)
                .identity_complete()
        );
        lifecycle.reset();
        assert!(lifecycle.select(0, Route::CStub, true).identity_complete());
    }

    #[test]
    fn snapshot_requires_complete_unique_surface_and_known_handlers() {
        let mut lifecycle = Lifecycle::new(2);
        lifecycle.reset();
        lifecycle.register(7, HandlerId::InputOpen);
        lifecycle.set_default(8, 9);
        let snapshot = lifecycle.snapshot(&[7, 8]);
        assert!(snapshot.complete);
        assert_eq!(snapshot.entries[0].handler, Some(HandlerId::InputOpen));
        assert_eq!(snapshot.entries[1].default_return, 9);
        let (selection, pre_state) =
            lifecycle.select_with_snapshot(7, Route::RustSelected, &[7, 8]);
        assert!(pre_state.complete && selection.identity_complete());
        assert_eq!(
            selection.lifecycle_sequence,
            pre_state.lifecycle_sequence + 1
        );
        assert!(!lifecycle.snapshot(&[7, 7]).complete);
        assert!(!lifecycle.snapshot(&[7]).complete);
        lifecycle.register(7, HandlerId::Unknown);
        assert!(!lifecycle.snapshot(&[7, 8]).complete);
        assert_eq!(HandlerId::from_wire(u32::MAX), HandlerId::Unknown);
        assert_eq!(HandlerId::from_wire(3), HandlerId::InputOpen);
    }

    #[test]
    fn concurrent_replacements_do_not_tear_selection_from_snapshot() {
        let lifecycle = Arc::new(Mutex::new(Lifecycle::new(1)));
        {
            let mut state = lifecycle.lock().unwrap();
            state.reset();
            state.register(7, HandlerId::InputOpen);
        }
        let writer = Arc::clone(&lifecycle);
        let replacement = std::thread::spawn(move || {
            for i in 0..128 {
                let id = if i % 2 == 0 {
                    HandlerId::InputClose
                } else {
                    HandlerId::InputOpen
                };
                writer.lock().unwrap().register(7, id);
            }
        });
        for _ in 0..128 {
            let (selection, snapshot) =
                lifecycle
                    .lock()
                    .unwrap()
                    .select_with_snapshot(7, Route::RustSelected, &[7]);
            assert!(snapshot.complete && selection.identity_complete());
            assert_eq!(snapshot.entries[0].handler, Some(selection.handler));
            assert_eq!(
                selection.lifecycle_sequence,
                snapshot.lifecycle_sequence + 1
            );
        }
        replacement.join().unwrap();
    }
}
