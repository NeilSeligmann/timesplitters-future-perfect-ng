//! Owned storage UI decisions. Keyboard, mouse and controller bindings submit
//! the same commands; presentation never contains a device selection screen.
//! This contract must be consumed by an owned frontend, not guest-state pokes.
use crate::storage_flow::{
    MapPayload, MapSource, Operation, SampleMapEntry, SampleMaps, SaveAuthentication, SaveEntry,
    SaveKey, SaveKind, StorageFlow, StoryProgressCommitReceipt,
};
use crate::story_progress::StoryProgressUpdate;
use crate::vfs::{NtStatus, status};
use tsfp_format_profile::Profile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Profile { player: usize },
    Map,
}

/// Content sources offered by Map Maker Load. These are not storage devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapLoadSource {
    Samples,
    LocalStorage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    Closed,
    MapSourceChoice,
    Catalogue {
        target: Target,
        entries: Vec<SaveEntry>,
        sample_maps: Vec<SampleMapEntry>,
    },
    Error {
        target: Target,
        status: NtStatus,
    },
}

/// Adoption receipts reference state owned by StorageFlow. The game frontend
/// consumes these after successful I/O; an error never emits a completion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub operation: Operation,
    pub target: Target,
    pub key: Option<SaveKey>,
}

pub enum Command {
    Open(Target),
    OpenMapSourceChoice,
    OpenMapCatalogue(MapLoadSource),
    CreateProfile {
        player: usize,
        directory: String,
        label: String,
        candidate: Profile,
    },
    SaveProfile {
        player: usize,
        candidate: Profile,
    },
    SaveInput {
        player: usize,
        input: tsfp_input::settings::InputSettings,
    },
    LoadProfile {
        player: usize,
        key: SaveKey,
    },
    SaveMap {
        directory: String,
        label: String,
        payload: MapPayload,
    },
    UpdateMap {
        payload: MapPayload,
    },
    LoadMap {
        source: MapSource,
    },
    Close,
}

pub struct StorageFrontend<A> {
    flow: StorageFlow<A>,
    screen: Screen,
}

impl<A: SaveAuthentication> StorageFrontend<A> {
    pub fn new(flow: StorageFlow<A>) -> Self {
        Self {
            flow,
            screen: Screen::Closed,
        }
    }
    pub fn screen(&self) -> &Screen {
        &self.screen
    }
    pub fn flow(&self) -> &StorageFlow<A> {
        &self.flow
    }

    /// Commit guarded Story progress through the same authenticated profile path as settings.
    /// This is a direct native call; it does not open or replay a storage UI command.
    pub fn save_story_progress(
        &mut self,
        player: usize,
        selected_key: &SaveKey,
        update: StoryProgressUpdate,
    ) -> Result<StoryProgressCommitReceipt, NtStatus> {
        self.flow.save_story_progress(player, selected_key, update)
    }

    /// Synchronous native handler. A returned receipt is delivered directly to
    /// the Rust game/editor consumer, never replayed as a pad event or callback.
    pub fn handle(
        &mut self,
        command: Command,
        samples: &mut impl SampleMaps,
    ) -> Result<Option<Completion>, NtStatus> {
        match command {
            Command::Close => {
                self.screen = Screen::Closed;
                Ok(None)
            }
            Command::OpenMapSourceChoice => {
                self.screen = Screen::MapSourceChoice;
                Ok(None)
            }
            Command::OpenMapCatalogue(source) => {
                let sample_maps = if source == MapLoadSource::Samples {
                    match samples.catalogue() {
                        Ok(entries) => {
                            let mut seen = std::collections::BTreeSet::new();
                            if entries.iter().any(|entry| {
                                !seen.insert(entry.id)
                                    || entry.label.is_empty()
                                    || entry.label.encode_utf16().count() > 140
                                    || entry.label.chars().any(char::is_control)
                            }) {
                                return self.fail(Target::Map, status::INVALID_PARAMETER);
                            }
                            entries
                        }
                        Err(status) => return self.fail(Target::Map, status),
                    }
                } else {
                    Vec::new()
                };
                match self.flow.catalogue() {
                    Ok(mut entries) => {
                        entries.retain(|entry| entry.kind == SaveKind::Map);
                        if source == MapLoadSource::Samples {
                            entries.clear();
                        }
                        self.screen = Screen::Catalogue {
                            target: Target::Map,
                            entries,
                            sample_maps,
                        };
                        Ok(None)
                    }
                    Err(status) => self.fail(Target::Map, status),
                }
            }
            Command::Open(target) => {
                if matches!(target, Target::Profile { player } if player >= 4) {
                    return self.fail(target, status::INVALID_PARAMETER);
                }
                let sample_maps = if target == Target::Map {
                    match samples.catalogue() {
                        Ok(entries) => {
                            let mut seen = std::collections::BTreeSet::new();
                            if entries.iter().any(|entry| {
                                !seen.insert(entry.id)
                                    || entry.label.is_empty()
                                    || entry.label.encode_utf16().count() > 140
                                    || entry.label.chars().any(char::is_control)
                            }) {
                                return self.fail(target, status::INVALID_PARAMETER);
                            }
                            entries
                        }
                        Err(status) => return self.fail(target, status),
                    }
                } else {
                    Vec::new()
                };
                match self.flow.catalogue() {
                    Ok(mut entries) => {
                        let kind = match target {
                            Target::Profile { .. } => SaveKind::Profile,
                            Target::Map => SaveKind::Map,
                        };
                        entries.retain(|entry| entry.kind == kind);
                        self.screen = Screen::Catalogue {
                            target,
                            entries,
                            sample_maps,
                        };
                        Ok(None)
                    }
                    Err(status) => self.fail(target, status),
                }
            }
            Command::CreateProfile {
                player,
                directory,
                label,
                candidate,
            } => {
                let target = Target::Profile { player };
                // Reject invalid input before reserving a directory.
                if player >= 4 || candidate.encode().is_err() {
                    return self.fail(target, status::INVALID_PARAMETER);
                }
                let result = self
                    .flow
                    .allocate_save(&directory, SaveKind::Profile, &label)
                    .and_then(|key| {
                        self.flow
                            .create_profile(player, key.clone(), candidate)
                            .map(|()| key)
                    });
                self.complete(target, Operation::ProfileSave, result)
            }
            Command::SaveProfile { player, candidate } => {
                let target = Target::Profile { player };
                let result = self
                    .flow
                    .profile(player)
                    .map(|p| p.key.clone())
                    .ok_or(status::INVALID_PARAMETER)
                    .and_then(|key| {
                        self.flow
                            .save_profile(player, key.clone(), candidate)
                            .map(|()| key)
                    });
                self.complete(target, Operation::ProfileSave, result)
            }
            Command::SaveInput { player, input } => {
                let target = Target::Profile { player };
                let key = self.flow.profile(player).map(|p| p.key.clone());
                let result = self
                    .flow
                    .save_input(player, input)
                    .and_then(|()| key.ok_or(status::INVALID_PARAMETER));
                self.complete(target, Operation::ProfileSave, result)
            }
            Command::LoadProfile { player, key } => {
                let result = self.flow.load_profile(player, key.clone()).map(|()| key);
                self.complete(Target::Profile { player }, Operation::ProfileLoad, result)
            }
            Command::SaveMap {
                directory,
                label,
                payload,
            } => {
                let result = self
                    .flow
                    .allocate_save(&directory, SaveKind::Map, &label)
                    .and_then(|key| {
                        self.flow.edit_map(MapSource::Local(key.clone()), payload);
                        self.flow.save_map(key.clone()).map(|()| key)
                    });
                self.complete(Target::Map, Operation::MapSave, result)
            }
            Command::UpdateMap { payload } => {
                let result = self
                    .flow
                    .map()
                    .and_then(|map| match &map.source {
                        MapSource::Local(key) => Some(key.clone()),
                        MapSource::DiscSample(_) => None,
                    })
                    .ok_or(status::INVALID_PARAMETER)
                    .and_then(|key| {
                        self.flow.edit_map(MapSource::Local(key.clone()), payload);
                        self.flow.save_map(key.clone()).map(|()| key)
                    });
                self.complete(Target::Map, Operation::MapSave, result)
            }
            Command::LoadMap { source } => {
                let key = match &source {
                    MapSource::Local(key) => Some(key.clone()),
                    MapSource::DiscSample(_) => None,
                };
                match self.flow.load_map(source, samples) {
                    Ok(()) => {
                        self.screen = Screen::Closed;
                        Ok(Some(Completion {
                            operation: Operation::MapLoad,
                            target: Target::Map,
                            key,
                        }))
                    }
                    Err(status) => self.fail(Target::Map, status),
                }
            }
        }
    }
    fn fail(&mut self, target: Target, status: NtStatus) -> Result<Option<Completion>, NtStatus> {
        self.screen = Screen::Error { target, status };
        Err(status)
    }
    fn complete(
        &mut self,
        target: Target,
        operation: Operation,
        result: Result<SaveKey, NtStatus>,
    ) -> Result<Option<Completion>, NtStatus> {
        match result {
            Ok(key) => {
                self.screen = Screen::Closed;
                Ok(Some(Completion {
                    operation,
                    target,
                    key: Some(key),
                }))
            }
            Err(status) => self.fail(target, status),
        }
    }
}
