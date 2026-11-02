//! T2319 owned profile/Map Maker transactions. No device selection, guest memory,
//! pad synthesis or original callbacks. The live UI still needs to adopt this contract.
use crate::story_progress::{StoryProgress, StoryProgressReceipt, StoryProgressUpdate};
use crate::vfs::{Access, Disposition, NtStatus, Vfs, status};
use tsfp_format_profile::Profile;

pub const MAP_PAYLOAD_BYTES: usize = 0x7fe0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveKind {
    Profile,
    Map,
}

/// A local title-relative save path. A disc sample is deliberately not a SaveKey.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveKey(String);

impl SaveKey {
    /// Directory and filename come from the runtime save catalogue, never display text.
    pub fn new(directory: &str, filename: &str) -> Result<Self, NtStatus> {
        fn component(s: &str) -> bool {
            !s.is_empty()
                && s != "."
                && s != ".."
                && !s
                    .chars()
                    .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
        }
        if !component(directory) || !component(filename) {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(Self(format!(
            "U:\\{}\\{}",
            directory.to_ascii_lowercase(),
            filename.to_ascii_lowercase()
        )))
    }

    pub fn path(&self) -> &str {
        &self.0
    }
}

/// Required boundary: retain-only inspection codecs cannot sign edited saves.
/// Implementations authenticate against keys acquired at runtime; no fallback success.
pub trait SaveAuthentication {
    fn sign(&mut self, kind: SaveKind, payload: &[u8]) -> Result<[u8; 20], NtStatus>;
    fn verify(
        &mut self,
        kind: SaveKind,
        payload: &[u8],
        signature: &[u8; 20],
    ) -> Result<(), NtStatus>;
}

/// Ordinary local-save (XDK class 0) authentication. The caller must supply the
/// already-derived runtime title key, not certificate material or a console key.
/// No key bytes are built in; other XDK signature classes are outside this contract.
/// Intentionally no Debug/Clone implementation that exposes or duplicates the key.
pub struct LocalTitleAuthentication {
    key: [u8; 16],
}

impl LocalTitleAuthentication {
    pub fn from_runtime_title_key(key: [u8; 16]) -> Self {
        Self { key }
    }
}

impl SaveAuthentication for LocalTitleAuthentication {
    fn sign(&mut self, _kind: SaveKind, payload: &[u8]) -> Result<[u8; 20], NtStatus> {
        // Original class 0 uses the same title key for ordinary profile/map saves.
        Ok(tsfp_kernel_services::crypto::hmac(
            &self.key,
            payload,
            &[],
            false,
        ))
    }

    fn verify(
        &mut self,
        kind: SaveKind,
        payload: &[u8],
        signature: &[u8; 20],
    ) -> Result<(), NtStatus> {
        let expected = self.sign(kind, payload)?;
        let difference = expected
            .iter()
            .zip(signature)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        if difference == 0 {
            Ok(())
        } else {
            Err(status::UNRECOGNIZED_VOLUME)
        }
    }
}

/// Owned serialized map, supplied by the map serializer (0x3006C0 boundary).
/// Length validation is not a claim of semantic validation of the map chunks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapPayload(Vec<u8>);

impl MapPayload {
    pub fn new(bytes: Vec<u8>) -> Result<Self, NtStatus> {
        if bytes.len() != MAP_PAYLOAD_BYTES {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(Self(bytes))
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Samples are read-only disc inputs, not a second storage device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapSource {
    Local(SaveKey),
    DiscSample(u32),
}

/// Runtime-discovered read-only map source. The label is supplied by the
/// title's sample-data provider; it is never treated as a filesystem path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleMapEntry {
    pub id: u32,
    pub label: String,
}

impl SampleMapEntry {
    pub fn new(id: u32, label: String) -> Result<Self, NtStatus> {
        if label.is_empty()
            || label.encode_utf16().count() > 140
            || label.chars().any(char::is_control)
        {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(Self { id, label })
    }
}

pub trait SampleMaps {
    fn load_sample(&mut self, id: u32) -> Result<MapPayload, NtStatus>;

    /// The provider owns sample discovery and labels. Empty means no sample
    /// catalogue is available; the storage layer never invents sample IDs.
    fn catalogue(&mut self) -> Result<Vec<SampleMapEntry>, NtStatus> {
        Ok(Vec::new())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    ProfileSave,
    ProfileLoad,
    MapSave,
    MapLoad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure {
    pub operation: Operation,
    pub status: NtStatus,
}

/// Native list row. Labels are loaded from runtime SaveMeta, never embedded game text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveEntry {
    pub key: SaveKey,
    pub kind: SaveKind,
    pub label: Result<String, NtStatus>,
    pub bytes: u64,
}

/// SaveMeta uses a UTF-16LE BOM and name without a mandatory NUL terminator.
/// The original reads 280 name bytes at offset 2, so the envelope bound is 282 bytes.
pub fn decode_save_label(bytes: &[u8]) -> Result<String, NtStatus> {
    if bytes.len() < 2
        || bytes.len() > 282
        || bytes.len() % 2 != 0
        || !bytes.starts_with(&[0xff, 0xfe])
    {
        return Err(status::UNRECOGNIZED_VOLUME);
    }
    let mut words: Vec<u16> = bytes[2..]
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    if words.last() == Some(&0) {
        words.pop();
    }
    if words.is_empty() || words.contains(&0) {
        return Err(status::UNRECOGNIZED_VOLUME);
    }
    String::from_utf16(&words).map_err(|_| status::UNRECOGNIZED_VOLUME)
}

/// Encode a runtime display name in the original bounded SaveMeta envelope.
pub fn encode_save_label(label: &str) -> Result<Vec<u8>, NtStatus> {
    let words: Vec<_> = label.encode_utf16().collect();
    if words.is_empty() || words.len() > 140 || words.contains(&0) {
        return Err(status::INVALID_PARAMETER);
    }
    let mut bytes = vec![0xff, 0xfe];
    for word in words {
        bytes.extend(word.to_le_bytes());
    }
    Ok(bytes)
}

/// Reserve the caller's runtime-generated identity, then create its SaveMeta.
/// No display-name-to-directory algorithm is guessed. Existing saves are never
/// reused or overwritten. A failed metadata write retains the reserved directory
/// for recovery; it does not return a usable key or delete another operation's data.
pub fn allocate_save(
    vfs: &mut Vfs,
    directory: &str,
    kind: SaveKind,
    label: &str,
) -> Result<SaveKey, NtStatus> {
    let key = SaveKey::new(
        directory,
        match kind {
            SaveKind::Profile => "tsftrpft",
            SaveKind::Map => "anicemap.mkr",
        },
    )?;
    let metadata = SaveKey::new(directory, "SaveMeta.xbx")?;
    let bytes = encode_save_label(label)?;
    vfs.create_directory(&format!("U:\\{directory}"))?;
    // The directory is newly reserved, so Create must succeed without replacement.
    let handle = vfs.open(metadata.path(), Access::READ_WRITE, Disposition::Create)?;
    let written = vfs.write(handle, &bytes).and_then(|count| {
        if count == bytes.len() {
            Ok(())
        } else {
            Err(status::ACCESS_DENIED)
        }
    });
    let closed = vfs.close(handle);
    written?;
    closed?;
    Ok(key)
}

/// Enumerate the one title-local namespace. Metadata errors stay visible per row;
/// a directory containing both payload types produces two distinct typed rows.
pub fn catalogue(vfs: &mut Vfs) -> Result<Vec<SaveEntry>, NtStatus> {
    let mut entries = Vec::new();
    for directory in vfs.list_dir("U:\\")?.into_iter().filter(|e| e.is_directory) {
        let parent = SaveKey::new(&directory.name, "SaveMeta.xbx")?;
        let label = read_label(vfs, parent.path());
        for file in vfs
            .list_dir(&format!("U:\\{}", directory.name))?
            .into_iter()
            .filter(|e| !e.is_directory)
        {
            let kind = if file.name.eq_ignore_ascii_case("tsftrpft") {
                SaveKind::Profile
            } else if file.name.eq_ignore_ascii_case("anicemap.mkr") {
                SaveKind::Map
            } else {
                continue;
            };
            entries.push(SaveEntry {
                key: SaveKey::new(&directory.name, &file.name)?,
                kind,
                label: label.clone(),
                bytes: file.size,
            });
        }
    }
    Ok(entries)
}

fn read_label(vfs: &mut Vfs, path: &str) -> Result<String, NtStatus> {
    let handle = vfs.open(path, Access::READ, Disposition::Open)?;
    let result = (|| {
        let size = vfs.size(handle)?;
        if !(2..=282).contains(&size) {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        let mut bytes = vec![0; size as usize];
        if vfs.read_at(handle, 0, &mut bytes)? != bytes.len() {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        decode_save_label(&bytes)
    })();
    let closed = vfs.close(handle);
    let label = result?;
    closed?;
    Ok(label)
}

#[derive(Clone, Debug)]
pub struct ActiveProfile {
    pub key: SaveKey,
    pub profile: Profile,
    pub input: tsfp_input::settings::InputSettings,
    pub native_input: bool,
}

/// A successful authenticated write of Story progress to one explicitly selected profile.
/// This receipt means the save backend accepted the commit; a later `load_profile` is still
/// required to produce a separate reload/adoption receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryProgressCommitReceipt {
    pub key: SaveKey,
    pub progress: StoryProgressReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveMap {
    pub source: MapSource,
    pub payload: MapPayload,
    /// Editor clears its dirty indication only after successful publication.
    pub dirty: bool,
}

/// Replaces storage decisions of profile 0x192770/0x193120/0x193310 and
/// Map Maker 0x32B6A0/0x32AEE0 at the owned transaction boundary. This is not
/// registered as an entrypoint replacement until the frontend owns adoption.
pub struct StorageFlow<A> {
    vfs: Vfs,
    authentication: A,
    profiles: [Option<ActiveProfile>; 4],
    map: Option<ActiveMap>,
    failure: Option<Failure>,
}

impl<A: SaveAuthentication> StorageFlow<A> {
    pub fn new(vfs: Vfs, authentication: A) -> Self {
        Self {
            vfs,
            authentication,
            profiles: std::array::from_fn(|_| None),
            map: None,
            failure: None,
        }
    }

    pub fn profile(&self, player: usize) -> Option<&ActiveProfile> {
        self.profiles.get(player).and_then(Option::as_ref)
    }
    /// Persist an input edit with the active native profile; adopt only after publication.
    pub fn save_input(
        &mut self,
        player: usize,
        input: tsfp_input::settings::InputSettings,
    ) -> Result<(), NtStatus> {
        let result = (|| {
            let active = self
                .profile(player)
                .ok_or(status::INVALID_PARAMETER)?
                .clone();
            let mut serialized = active.profile.clone();
            serialized.payload[..4].fill(0);
            serialized.signature = self
                .authentication
                .sign(SaveKind::Profile, &serialized.payload)?;
            let envelope =
                crate::profile_input::encode(&serialized, &input, &mut self.authentication)?;
            write_envelope(&mut self.vfs, active.key.path(), &envelope)?;
            let adopted = self.profiles[player].as_mut().unwrap();
            adopted.input = input;
            adopted.native_input = true;
            Ok(())
        })();
        self.finish(Operation::ProfileSave, result)
    }
    pub fn map(&self) -> Option<&ActiveMap> {
        self.map.as_ref()
    }
    pub fn failure(&self) -> Option<Failure> {
        self.failure
    }

    pub fn catalogue(&mut self) -> Result<Vec<SaveEntry>, NtStatus> {
        catalogue(&mut self.vfs)
    }

    pub fn allocate_save(
        &mut self,
        directory: &str,
        kind: SaveKind,
        label: &str,
    ) -> Result<SaveKey, NtStatus> {
        allocate_save(&mut self.vfs, directory, kind, label)
    }

    fn finish<T>(
        &mut self,
        operation: Operation,
        result: Result<T, NtStatus>,
    ) -> Result<T, NtStatus> {
        self.failure = result
            .as_ref()
            .err()
            .map(|&status| Failure { operation, status });
        result
    }

    /// Candidate initialized by the native profile model using runtime defaults.
    /// Save catalogue owns directory/SaveMeta allocation. Publication here requires
    /// that directory to exist. Do not invent a compatible catalogue identifier.
    /// New profiles always own fresh input defaults, independent of session edits.
    /// Defaults and profile payload are authenticated and published in one write.
    pub fn create_profile(
        &mut self,
        player: usize,
        key: SaveKey,
        candidate: Profile,
    ) -> Result<(), NtStatus> {
        self.save_profile_record(player, key, candidate, true)
    }
    pub fn save_profile(
        &mut self,
        player: usize,
        key: SaveKey,
        candidate: Profile,
    ) -> Result<(), NtStatus> {
        self.save_profile_record(player, key, candidate, false)
    }

    /// Apply one already-guarded Story success to the active profile named by `selected_key`,
    /// then attempt to persist it. The key is explicit so the player slot is never treated as
    /// profile identity. The original commits Story completion to profile memory before its save
    /// attempt, so a storage failure returns an error but retains the in-memory update.
    pub fn save_story_progress(
        &mut self,
        player: usize,
        selected_key: &SaveKey,
        update: StoryProgressUpdate,
    ) -> Result<StoryProgressCommitReceipt, NtStatus> {
        let result = (|| {
            let active = self
                .profiles
                .get_mut(player)
                .and_then(Option::as_mut)
                .filter(|active| &active.key == selected_key)
                .ok_or(status::INVALID_PARAMETER)?;
            let progress = StoryProgress::apply_success(&mut active.profile, update)?;
            let candidate = active.profile.clone();
            self.save_profile_record(player, selected_key.clone(), candidate, false)?;
            Ok(StoryProgressCommitReceipt {
                key: selected_key.clone(),
                progress,
            })
        })();
        self.finish(Operation::ProfileSave, result)
    }

    fn save_profile_record(
        &mut self,
        player: usize,
        key: SaveKey,
        candidate: Profile,
        fresh: bool,
    ) -> Result<(), NtStatus> {
        let result = (|| {
            if player >= self.profiles.len() {
                return Err(status::INVALID_PARAMETER);
            }
            let mut serialized = candidate.clone();
            // Validate before the slice write; preserve the runtime pointer in owned state.
            serialized.encode().map_err(|_| status::INVALID_PARAMETER)?;
            serialized.payload[..4].fill(0);
            serialized.signature = self
                .authentication
                .sign(SaveKind::Profile, &serialized.payload)?;
            let input = if fresh {
                tsfp_input::settings::InputSettings::default()
            } else {
                self.profiles[player]
                    .as_ref()
                    .filter(|p| p.key == key)
                    .map(|p| p.input.clone())
                    .unwrap_or_default()
            };
            let native_input = fresh
                || self.profiles[player]
                    .as_ref()
                    .is_some_and(|p| p.key == key && p.native_input);
            let envelope = if native_input {
                crate::profile_input::encode(&serialized, &input, &mut self.authentication)?
            } else {
                serialized.encode().map_err(|_| status::INVALID_PARAMETER)?
            };
            write_envelope(&mut self.vfs, key.path(), &envelope)?;
            let mut adopted = candidate;
            adopted.signature = serialized.signature;
            self.profiles[player] = Some(ActiveProfile {
                key,
                profile: adopted,
                input,
                native_input,
            });
            Ok(())
        })();
        self.finish(Operation::ProfileSave, result)
    }

    pub fn load_profile(&mut self, player: usize, key: SaveKey) -> Result<(), NtStatus> {
        let result = (|| {
            if player >= self.profiles.len() {
                return Err(status::INVALID_PARAMETER);
            }
            let bytes = read_profile_envelope(&mut self.vfs, key.path())?;
            let (profile, input) = crate::profile_input::decode(&bytes, &mut self.authentication)?;
            self.profiles[player] = Some(ActiveProfile {
                key,
                profile,
                input,
                native_input: bytes.len() > 0x1f68,
            });
            Ok(())
        })();
        self.finish(Operation::ProfileLoad, result)
    }

    /// The editor hands over its serialized working copy; a failure retains dirty state.
    pub fn edit_map(&mut self, source: MapSource, payload: MapPayload) {
        self.map = Some(ActiveMap {
            source,
            payload,
            dirty: true,
        });
    }

    pub fn save_map(&mut self, key: SaveKey) -> Result<(), NtStatus> {
        let result = (|| {
            let map = self.map.as_ref().ok_or(status::INVALID_PARAMETER)?;
            let mut envelope = map.payload.0.clone();
            envelope.extend(self.authentication.sign(SaveKind::Map, &envelope)?);
            write_envelope(&mut self.vfs, key.path(), &envelope)?;
            let map = self.map.as_mut().expect("working map was checked above");
            map.source = MapSource::Local(key);
            map.dirty = false;
            Ok(())
        })();
        self.finish(Operation::MapSave, result)
    }

    pub fn load_map(
        &mut self,
        source: MapSource,
        samples: &mut impl SampleMaps,
    ) -> Result<(), NtStatus> {
        let result = (|| {
            let payload = match &source {
                MapSource::DiscSample(id) => samples.load_sample(*id)?,
                MapSource::Local(key) => {
                    let envelope = read_map_envelope(&mut self.vfs, key.path())?;
                    let (payload, signature) = envelope.split_at(MAP_PAYLOAD_BYTES);
                    self.authentication.verify(
                        SaveKind::Map,
                        payload,
                        signature.try_into().expect("fixed envelope size"),
                    )?;
                    MapPayload::new(payload.to_vec())?
                }
            };
            self.map = Some(ActiveMap {
                source,
                payload,
                dirty: false,
            });
            Ok(())
        })();
        self.finish(Operation::MapLoad, result)
    }
}

fn read_map_envelope(vfs: &mut Vfs, path: &str) -> Result<Vec<u8>, NtStatus> {
    let handle = vfs.open(path, Access::READ, Disposition::Open)?;
    let result = (|| {
        let size = MAP_PAYLOAD_BYTES + 20;
        if vfs.size(handle)? != size as u64 {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        let mut bytes = vec![0; size];
        if vfs.read_at(handle, 0, &mut bytes)? != size {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        Ok(bytes)
    })();
    let closed = vfs.close(handle);
    let bytes = result?;
    closed?;
    Ok(bytes)
}

fn write_envelope(vfs: &mut Vfs, path: &str, bytes: &[u8]) -> Result<(), NtStatus> {
    let temporary = format!("{path}.tmp");
    let handle = vfs.open(&temporary, Access::READ_WRITE, Disposition::OverwriteIf)?;
    let result = vfs.write(handle, bytes).and_then(|written| {
        if written != bytes.len() {
            return Err(status::ACCESS_DENIED);
        }
        vfs.set_end_of_file(handle)
    });
    let closed = vfs.close(handle);
    result?;
    closed?;
    vfs.rename(&temporary, path)
}

fn read_profile_envelope(vfs: &mut Vfs, path: &str) -> Result<Vec<u8>, NtStatus> {
    let handle = vfs.open(path, Access::READ, Disposition::Open)?;
    let result = (|| {
        let size = vfs.size(handle)?;
        if size < 0x1f68 || size > crate::profile_input::MAX_ENVELOPE as u64 {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        let mut bytes = vec![0; size as usize];
        if vfs.read_at(handle, 0, &mut bytes)? != bytes.len() {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        Ok(bytes)
    })();
    let closed = vfs.close(handle);
    let bytes = result?;
    closed?;
    Ok(bytes)
}
