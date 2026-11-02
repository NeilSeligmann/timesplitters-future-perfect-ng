//! Owned activation and context lifecycle above the persistence frontend.
//! No guest pointers, callbacks, synthetic input or device pages.
use crate::map_document::MapDocument;
use crate::map_history::MapHistory;
use crate::storage_flow::{
    ActiveProfile, MapPayload, MapSource, Operation, SampleMaps, SaveAuthentication,
    StoryProgressCommitReceipt,
};
use crate::storage_frontend::{Command, Completion, StorageFrontend, Target};
use crate::story_progress::StoryProgressUpdate;
use crate::vfs::{NtStatus, status};
use tsfp_format_profile::Profile;

/// Runtime initialized defaults, not an invented zero-filled retail profile.
/// The caller's defaults provider owns game-specific progress and unlock rules.
pub fn initialize_profile(defaults: &Profile) -> Result<Profile, NtStatus> {
    defaults.encode().map_err(|_| status::INVALID_PARAMETER)?;
    let mut profile = defaults.clone();
    // Retired guest pointer and obsolete signature are never native identity.
    profile.payload[..4].fill(0);
    profile.signature.fill(0);
    Ok(profile)
}

/// Native reconstruction of the settings-head constructor (0x695C0).
/// Ring pairs are loaded from the owner's runtime tables, never built-in data.
#[derive(Clone)]
pub struct ProfileInitializer {
    ring: [(u32, i32); 4],
    next: usize,
    turn_speed: f32,
}
impl ProfileInitializer {
    /// Read-only import at a quiescent hybrid frame boundary. The original
    /// 0x695C0 constructor sets speed to 0.5; only its pair tables and cursor
    /// come from runtime state (0x2BE850). No guest cursor is advanced here.
    pub fn from_runtime(
        runtime: &impl crate::storage_bootstrap::RuntimeRead,
    ) -> Result<Self, NtStatus> {
        let addresses = [
            0x523660, 0x523664, 0x523668, 0x52366c, 0x523670, 0x523674, 0x523678, 0x52367c,
            0x7613cc,
        ];
        let mut values = [0; 9];
        for (out, address) in values.iter_mut().zip(addresses) {
            *out = runtime.word(address).ok_or(status::INVALID_PARAMETER)?;
        }
        if addresses
            .into_iter()
            .zip(values)
            .any(|(address, value)| runtime.word(address) != Some(value))
        {
            return Err(status::INVALID_PARAMETER);
        }
        let ring = std::array::from_fn(|index| (values[index], values[4 + index] as i32));
        Self::new(ring, values[8] as usize, 0.5)
    }
    pub fn new(ring: [(u32, i32); 4], next: usize, turn_speed: f32) -> Result<Self, NtStatus> {
        if next >= 4 || !turn_speed.is_finite() {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(Self {
            ring,
            next,
            turn_speed,
        })
    }
    pub fn next_ring_slot(&self) -> usize {
        self.next
    }
    /// Caller supplies retained non-progress fields from an owned record. The
    /// complete touched progress constructor runs natively, before head settings.
    pub fn initialize_with_progress(
        &mut self,
        retained: &Profile,
        progress: &crate::profile_defaults::ProgressDefaults,
    ) -> Result<Profile, NtStatus> {
        let mut profile = initialize_profile(retained)?;
        progress.reset_profile(&mut profile)?;
        // 0x69D90 clears the independent 0x313-word progression summary.
        // Retaining another profile's summary would inherit its unlocks.
        profile.payload[0x1290..0x1edc].fill(0);
        self.initialize(&profile)
    }
    /// Caller supplies retained progress/default fields; use initialize_with_progress
    /// to reconstruct the native 0x2C7010 reset as well.
    /// This method overwrites exactly the settings head, pointer and signature;
    /// other progress bytes retain the supplied defaults. Invalid input consumes no ring slot.
    pub fn initialize(&mut self, progress_defaults: &Profile) -> Result<Profile, NtStatus> {
        use tsfp_format_profile::settings::{HEAD_SIZE, Settings};
        let mut profile = initialize_profile(progress_defaults)?;
        let mut head = [0; HEAD_SIZE];
        head[..2].copy_from_slice(&1u16.to_le_bytes());
        head[2..4].copy_from_slice(&0xffu16.to_le_bytes());
        // 0x2588B0 initializes mapping using layout index zero: only common
        // digital slots are filled, since zero is outside the 1..6 preset switch.
        for (index, value) in [(6, 4u32), (7, 8), (8, 1), (9, 2), (11, 4), (12, 8)] {
            head[4 + index * 4..8 + index * 4].copy_from_slice(&value.to_le_bytes());
        }
        let (first, second) = self.ring[self.next];
        for (offset, value) in [
            (0x5c, 0),
            (0x60, first),
            (0x64, second as u32),
            (0x68, 4),
            (0x6c, 3),
            (0x70, 0),
            (0x74, 2),
            (0x78, self.turn_speed.to_bits()),
        ] {
            head[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let settings = Settings::from_head(&head).map_err(|_| status::INVALID_PARAMETER)?;
        profile
            .accept_settings(&settings)
            .map_err(|_| status::INVALID_PARAMETER)?;
        self.next = (self.next + 1) % 4;
        Ok(profile)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Menu,
    Profile { player: usize },
    Editor,
    Storage(Target),
}

/// Extension travels with each active profile; T2211 supplies its versioned codec.
/// Generic ownership keeps this lifecycle independent of a particular extension version.
#[derive(Clone, Debug)]
pub struct PlayerProfile<E> {
    pub saved: ActiveProfile,
    pub extension: E,
}

pub struct StorageLifecycle<E> {
    players: [Option<PlayerProfile<E>>; 4],
    editor: Option<MapDocument>,
    editor_source: Option<MapSource>,
    editor_dirty: bool,
    editor_history: MapHistory,
    contexts: Vec<Context>,
}
impl<E> Default for StorageLifecycle<E> {
    fn default() -> Self {
        Self {
            players: std::array::from_fn(|_| None),
            editor: None,
            editor_source: None,
            editor_dirty: false,
            editor_history: MapHistory::default(),
            contexts: vec![Context::Menu],
        }
    }
}
impl<E> StorageLifecycle<E> {
    pub fn player(&self, player: usize) -> Option<&PlayerProfile<E>> {
        self.players.get(player).and_then(Option::as_ref)
    }
    pub fn editor(&self) -> Option<&MapDocument> {
        self.editor.as_ref()
    }
    /// Source identity for the currently adopted owned editor document.
    /// Save handlers use this to update a local save or open Save As for samples.
    pub fn editor_source(&self) -> Option<&MapSource> {
        self.editor_source.as_ref()
    }
    pub fn editor_dirty(&self) -> bool {
        self.editor_dirty
    }
    pub fn editor_can_undo(&self) -> bool {
        self.editor_history.can_undo()
    }
    pub fn editor_can_redo(&self) -> bool {
        self.editor_history.can_redo()
    }
    pub fn context(&self) -> Context {
        *self.contexts.last().unwrap()
    }
    pub fn open(&mut self, target: Target) -> Result<(), NtStatus> {
        if matches!(target, Target::Profile { player } if player >= 4)
            || matches!(self.context(), Context::Storage(_))
        {
            return Err(status::INVALID_PARAMETER);
        }
        self.contexts.push(Context::Storage(target));
        Ok(())
    }
    pub fn cancel(&mut self) -> Result<(), NtStatus> {
        if !matches!(self.context(), Context::Storage(_)) {
            return Err(status::INVALID_PARAMETER);
        }
        self.contexts.pop();
        Ok(())
    }
    pub fn deactivate(&mut self, player: usize) -> Result<(), NtStatus> {
        let slot = self
            .players
            .get_mut(player)
            .ok_or(status::INVALID_PARAMETER)?;
        *slot = None;
        self.contexts.retain(|c| !matches!(c, Context::Profile { player: p } | Context::Storage(Target::Profile { player: p }) if *p == player));
        Ok(())
    }
    pub fn edit(&mut self, document: MapDocument) {
        if let Ok(snapshot) = document.serialize() {
            if !self.editor_history.is_initialized() {
                self.editor_history.reset(snapshot);
            } else {
                // Documents are validated on construction; an invalid history frame must not
                // prevent the existing edit/save flow from accepting its owned document.
                if self.editor_history.commit(snapshot.clone()).is_err() {
                    self.editor_history.reset(snapshot);
                }
            }
        }
        self.editor = Some(document);
        self.editor_dirty = true;
    }
    /// Commit one accepted Rust-owned Map Maker description value into the
    /// current document. A no-op leaves persistence dirtiness and history
    /// unchanged; invalid text or a missing editor document leaves state intact.
    pub fn edit_map_description(&mut self, description: &str) -> Result<bool, NtStatus> {
        let current = self.editor.as_ref().ok_or(status::INVALID_PARAMETER)?;
        let updated = current.with_description(description)?;
        if updated == *current {
            return Ok(false);
        }
        self.edit(updated);
        Ok(true)
    }
    /// Adopt one serialized editor snapshot from the live Map Maker boundary. Unchanged frames
    /// are ignored so frame polling does not mark persistence dirty or add duplicate history.
    pub fn record_editor_snapshot(&mut self, snapshot: MapPayload) -> Result<bool, NtStatus> {
        let current = self.editor.as_ref().ok_or(status::INVALID_PARAMETER)?;
        if current.serialize()? == snapshot {
            return Ok(false);
        }
        let document = MapDocument::parse(&snapshot)?;
        let mut history = self.editor_history.clone();
        if !history.commit(snapshot)? {
            return Ok(false);
        }
        self.editor_history = history;
        self.editor = Some(document);
        self.editor_dirty = true;
        Ok(true)
    }
    /// Apply the last editor record to Rust-owned document state. Persistence dirty remains a
    /// separate transaction and becomes true until a successful native save adopts the result.
    pub fn undo_editor(&mut self) -> Result<bool, NtStatus> {
        self.apply_editor_history(true)
    }
    pub fn redo_editor(&mut self) -> Result<bool, NtStatus> {
        self.apply_editor_history(false)
    }
    fn apply_editor_history(&mut self, undo: bool) -> Result<bool, NtStatus> {
        let previous_history = self.editor_history.clone();
        let restored = if undo {
            self.editor_history.undo()?
        } else {
            self.editor_history.redo()?
        };
        let Some(payload) = restored else {
            return Ok(false);
        };
        let document = match MapDocument::parse(&payload) {
            Ok(document) => document,
            Err(error) => {
                self.editor_history = previous_history;
                return Err(error);
            }
        };
        self.editor = Some(document);
        self.editor_dirty = true;
        Ok(true)
    }
    pub fn save_editor<A: SaveAuthentication>(
        &mut self,
        frontend: &mut StorageFrontend<A>,
        samples: &mut impl SampleMaps,
        new_save: Option<(String, String)>,
    ) -> Result<Completion, NtStatus> {
        if self.context() != Context::Storage(Target::Map) {
            return Err(status::INVALID_PARAMETER);
        }
        let payload = self
            .editor
            .as_ref()
            .ok_or(status::INVALID_PARAMETER)?
            .serialize()?;
        let command = match new_save {
            Some((directory, label)) => Command::SaveMap {
                directory,
                label,
                payload,
            },
            None => Command::UpdateMap { payload },
        };
        let completion = frontend
            .handle(command, samples)?
            .ok_or(status::INVALID_PARAMETER)?;
        // Map save receipts have no extension to decode.
        self.adopt(frontend, &completion, |_| Err(status::INVALID_PARAMETER))?;
        Ok(completion)
    }
    /// Decode all consumer state before publishing activation or popping context.
    /// A decoder error retains the prior active profile/editor and storage context.
    pub fn adopt<A: SaveAuthentication>(
        &mut self,
        frontend: &StorageFrontend<A>,
        completion: &Completion,
        decode_extension: impl FnOnce(&ActiveProfile) -> Result<E, NtStatus>,
    ) -> Result<(), NtStatus> {
        if self.context() != Context::Storage(completion.target) {
            return Err(status::INVALID_PARAMETER);
        }
        match (completion.target, completion.operation) {
            (Target::Profile { player }, Operation::ProfileLoad | Operation::ProfileSave) => {
                let active = frontend
                    .flow()
                    .profile(player)
                    .ok_or(status::INVALID_PARAMETER)?;
                if completion.key.as_ref() != Some(&active.key) {
                    return Err(status::INVALID_PARAMETER);
                }
                let extension = decode_extension(active)?;
                let saved = active.clone();
                saved
                    .profile
                    .encode()
                    .map_err(|_| status::UNRECOGNIZED_VOLUME)?;
                self.players[player] = Some(PlayerProfile { saved, extension });
                self.contexts.pop();
                if self.context() != (Context::Profile { player }) {
                    self.contexts.push(Context::Profile { player });
                }
            }
            (Target::Map, Operation::MapLoad | Operation::MapSave) => {
                let active = frontend.flow().map().ok_or(status::INVALID_PARAMETER)?;
                let key = match &active.source {
                    MapSource::Local(key) => Some(key),
                    MapSource::DiscSample(_) => None,
                };
                if completion.key.as_ref() != key || active.dirty {
                    return Err(status::INVALID_PARAMETER);
                }
                let document = match &active.source {
                    MapSource::Local(_) => MapDocument::parse_local(&active.payload)?,
                    MapSource::DiscSample(_) => MapDocument::parse(&active.payload)?,
                };
                if completion.operation == Operation::MapLoad {
                    self.editor_history.reset(document.serialize()?);
                }
                self.editor = Some(document);
                self.editor_source = Some(active.source.clone());
                self.editor_dirty = false;
                self.contexts.pop();
                if self.context() != Context::Editor {
                    self.contexts.push(Context::Editor);
                }
            }
            _ => return Err(status::INVALID_PARAMETER),
        }
        Ok(())
    }
}

/// T2211's authenticated extension is part of the active profile, never a host sidecar.
pub type NativeStorageLifecycle = StorageLifecycle<tsfp_input::settings::InputSettings>;
impl NativeStorageLifecycle {
    pub fn adopt_native<A: SaveAuthentication>(
        &mut self,
        frontend: &StorageFrontend<A>,
        completion: &Completion,
    ) -> Result<(), NtStatus> {
        self.adopt(frontend, completion, |active| Ok(active.input.clone()))
    }
    /// The owning settings UI submits its typed edit directly. Failed persistence
    /// retains the active profile's previous extension and leaves error context open.
    pub fn save_input<A: SaveAuthentication>(
        &mut self,
        frontend: &mut StorageFrontend<A>,
        player: usize,
        input: tsfp_input::settings::InputSettings,
        samples: &mut impl SampleMaps,
    ) -> Result<Completion, NtStatus> {
        let target = Target::Profile { player };
        let active = self.player(player).ok_or(status::INVALID_PARAMETER)?;
        if frontend
            .flow()
            .profile(player)
            .is_none_or(|p| p.key != active.saved.key)
        {
            return Err(status::INVALID_PARAMETER);
        }
        if self.context() != Context::Storage(target) {
            self.open(target)?;
        }
        let completion = frontend
            .handle(Command::SaveInput { player, input }, samples)?
            .ok_or(status::INVALID_PARAMETER)?;
        self.adopt_native(frontend, &completion)?;
        Ok(completion)
    }

    /// Persist one already-guarded Story success for the profile selected in this lifecycle.
    /// The profile slot supplies the explicit key; player index is never identity. Story profile
    /// memory updates before the save attempt, so mirror that state even when persistence fails.
    pub fn save_story_progress<A: SaveAuthentication>(
        &mut self,
        frontend: &mut StorageFrontend<A>,
        player: usize,
        update: StoryProgressUpdate,
    ) -> Result<StoryProgressCommitReceipt, NtStatus> {
        let selected_key = self
            .player(player)
            .map(|profile| profile.saved.key.clone())
            .ok_or(status::INVALID_PARAMETER)?;
        if frontend
            .flow()
            .profile(player)
            .is_none_or(|profile| profile.key != selected_key)
        {
            return Err(status::INVALID_PARAMETER);
        }

        let result = frontend.save_story_progress(player, &selected_key, update);
        if let Some(active) = frontend
            .flow()
            .profile(player)
            .filter(|profile| profile.key == selected_key)
            .cloned()
        {
            if let Some(adopted) = self.players.get_mut(player).and_then(Option::as_mut) {
                adopted.saved = active;
            }
        }
        result
    }
}
