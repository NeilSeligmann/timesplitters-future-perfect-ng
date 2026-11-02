//! Owned catalogue navigation for native storage entry adapters.
//! A root address identifies an attachment contract, never a guest state write.
use crate::storage_flow::{MapSource, SampleMapEntry, SaveEntry};
use crate::storage_frontend::{Command, Completion, MapLoadSource, Screen, Target};
use crate::vfs::{NtStatus, status};

/// These roots cannot be registered until their caller supplies an owned target
/// and consumes completion. In particular DeviceFeedback is NOT profile load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root {
    ProfileTick,
    ProfileSaveTick,
    DeviceFeedback,
    MapCatalogue,
    MapProgress,
}
impl Root {
    /// Complete native attachment catalogue; admission to the legacy ABI remains
    /// separate until its caller owns the context and consumes the receipt.
    pub const ALL: [(u32, Self); 5] = [
        (0x192770, Self::ProfileTick),
        (0x193120, Self::ProfileSaveTick),
        (0x193310, Self::DeviceFeedback),
        (0x32b6a0, Self::MapCatalogue),
        (0x32aee0, Self::MapProgress),
    ];
    pub fn from_address(address: u32) -> Option<Self> {
        Self::ALL
            .iter()
            .find_map(|(entry, root)| (*entry == address).then_some(*root))
    }

    pub fn accepts(self, target: Target) -> bool {
        matches!(
            (self, target),
            (
                Self::ProfileTick | Self::ProfileSaveTick | Self::DeviceFeedback,
                Target::Profile { player: 0..=3 }
            ) | (Self::MapCatalogue | Self::MapProgress, Target::Map)
        )
    }
}

#[derive(Default)]
pub struct StorageRoute {
    target: Option<Target>,
    entries: Vec<SaveEntry>,
    sample_maps: Vec<SampleMapEntry>,
    cursor: Option<usize>,
    map_source_choice: bool,
    error: Option<NtStatus>,
    completion: Option<Completion>,
}
impl StorageRoute {
    /// The frontend supplies the authoritative rows after I/O, including errors.
    /// Preserve focus by durable key across catalogue refresh/reordering.
    pub fn publish(&mut self, screen: &Screen, completion: Option<Completion>) {
        self.completion = completion;
        match screen {
            Screen::MapSourceChoice => {
                self.target = Some(Target::Map);
                self.entries.clear();
                self.sample_maps.clear();
                self.cursor = Some(0);
                self.map_source_choice = true;
                self.error = None;
            }
            Screen::Catalogue {
                target,
                entries,
                sample_maps,
            } => {
                let selected_local = (self.target == Some(*target))
                    .then(|| {
                        self.cursor
                            .and_then(|n| self.entries.get(n))
                            .map(|e| e.key.clone())
                    })
                    .flatten();
                let selected_sample = (self.target == Some(*target))
                    .then(|| {
                        self.cursor
                            .and_then(|n| n.checked_sub(self.entries.len()))
                            .and_then(|n| self.sample_maps.get(n))
                            .map(|sample| sample.id)
                    })
                    .flatten();
                self.cursor = selected_local
                    .and_then(|key| entries.iter().position(|e| e.key == key))
                    .or_else(|| {
                        selected_sample.and_then(|id| {
                            sample_maps
                                .iter()
                                .position(|sample| sample.id == id)
                                .map(|index| entries.len() + index)
                        })
                    })
                    .or_else(|| (!entries.is_empty() || !sample_maps.is_empty()).then_some(0));
                self.target = Some(*target);
                self.map_source_choice = false;
                self.entries = entries.clone();
                self.sample_maps = sample_maps.clone();
                self.error = None;
            }
            Screen::Error { target, status } => {
                self.map_source_choice = false;
                if self.target != Some(*target) {
                    self.entries.clear();
                    self.sample_maps.clear();
                    self.cursor = None;
                }
                self.target = Some(*target);
                self.error = Some(*status);
            }
            Screen::Closed => {
                self.target = None;
                self.entries.clear();
                self.sample_maps.clear();
                self.cursor = None;
                self.map_source_choice = false;
                self.error = None;
            }
        }
    }
    pub fn entries(&self) -> &[SaveEntry] {
        &self.entries
    }
    pub fn sample_maps(&self) -> &[SampleMapEntry] {
        &self.sample_maps
    }
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }
    pub fn map_source_choice(&self) -> bool {
        self.map_source_choice
    }
    pub fn error(&self) -> Option<NtStatus> {
        self.error
    }
    /// A deactivated consumer must not receive a previously queued adoption.
    /// Preserve receipts for other players and the editor.
    pub fn invalidate_completion(&mut self, target: Target) {
        if self
            .completion
            .as_ref()
            .is_some_and(|receipt| receipt.target == target)
        {
            self.completion = None;
        }
    }
    pub fn take_completion(&mut self) -> Option<Completion> {
        self.completion.take()
    }
    pub fn focus(&mut self, index: usize) -> Result<(), NtStatus> {
        let count = if self.map_source_choice {
            2
        } else {
            self.entries.len() + self.sample_maps.len()
        };
        if self.error.is_some() || index >= count {
            return Err(status::INVALID_PARAMETER);
        }
        self.cursor = Some(index);
        Ok(())
    }
    /// Controller, keyboard and pointer all use this same owned selection.
    /// Malformed metadata remains visible but cannot silently load another row.
    pub fn load_selected(&self) -> Result<Command, NtStatus> {
        if self.map_source_choice {
            return match self.cursor {
                Some(0) => Ok(Command::OpenMapCatalogue(MapLoadSource::Samples)),
                Some(1) => Ok(Command::OpenMapCatalogue(MapLoadSource::LocalStorage)),
                _ => Err(status::OBJECT_NAME_NOT_FOUND),
            };
        }
        if let Some(error) = self.error {
            return Err(error);
        }
        match self.target.ok_or(status::INVALID_PARAMETER)? {
            Target::Profile { player } => {
                let row = self
                    .cursor
                    .and_then(|n| self.entries.get(n))
                    .ok_or(status::OBJECT_NAME_NOT_FOUND)?;
                row.label.as_ref().map_err(|error| *error)?;
                Ok(Command::LoadProfile {
                    player,
                    key: row.key.clone(),
                })
            }
            Target::Map => {
                let index = self.cursor.ok_or(status::OBJECT_NAME_NOT_FOUND)?;
                if let Some(row) = self.entries.get(index) {
                    row.label.as_ref().map_err(|error| *error)?;
                    Ok(Command::LoadMap {
                        source: MapSource::Local(row.key.clone()),
                    })
                } else {
                    let sample = self
                        .sample_maps
                        .get(
                            index
                                .checked_sub(self.entries.len())
                                .ok_or(status::OBJECT_NAME_NOT_FOUND)?,
                        )
                        .ok_or(status::OBJECT_NAME_NOT_FOUND)?;
                    Ok(Command::LoadMap {
                        source: MapSource::DiscSample(sample.id),
                    })
                }
            }
        }
    }
}
