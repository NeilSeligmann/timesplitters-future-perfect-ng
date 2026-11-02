//! Tier 0 synthetic documents/defaults/key, real filesystem persistence.
use tsfp_engine_fileio::story_progress::{StoryMode, StoryProgress, StoryProgressUpdate};
use tsfp_engine_fileio::{
    local_storage::LocalStorage,
    map_document::{Chunk, MapDocument, SETTINGS},
    storage_flow::*,
    storage_frontend::*,
    storage_lifecycle::*,
    vfs::{NtStatus, status},
};
use tsfp_format_profile::Profile;
struct Samples;
impl SampleMaps for Samples {
    fn load_sample(&mut self, _: u32) -> Result<MapPayload, NtStatus> {
        Err(status::OBJECT_NAME_NOT_FOUND)
    }
}
fn document() -> MapDocument {
    MapDocument::from_chunks(vec![
        Chunk {
            tag: SETTINGS,
            data: vec![0; 0x3dc],
        },
        Chunk {
            tag: 0xc77672,
            data: vec![42; 256],
        },
    ])
    .unwrap()
}
fn defaults() -> Profile {
    let mut p = Profile {
        payload: vec![0; 0x1f54],
        signature: [3; 20],
    };
    p.payload[..4].copy_from_slice(&123u32.to_le_bytes());
    p.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    p
}
#[test]
fn bounded_document_roundtrip_and_malformed_inputs() {
    let doc = document();
    let payload = doc.serialize().unwrap();
    assert_eq!(
        MapDocument::parse(&payload).unwrap().serialize().unwrap(),
        payload
    );
    let used = u32::from_le_bytes(payload.bytes()[0x258..0x25c].try_into().unwrap()) as usize;
    // Independent bitwise IEEE CRC, unlike the production table implementation.
    let mut crc = u32::MAX;
    for byte in &payload.bytes()[..used] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32.wrapping_mul(crc & 1));
        }
    }
    assert_eq!(&payload.bytes()[used..used + 4], &(!crc).to_le_bytes());
    let mut corrupt = payload.bytes().to_vec();
    corrupt[0x80] ^= 1;
    let corrupt = MapPayload::new(corrupt).unwrap();
    assert!(MapDocument::parse_local(&corrupt).is_err());
    assert!(MapDocument::parse(&corrupt).is_ok()); // original disc-sample CRC bypass
    // Earlier native saves and sample paths can omit the inner CRC. Re-saving
    // upgrades the envelope; the outer title signature is checked by StorageFlow.
    let mut omitted = payload.bytes().to_vec();
    omitted[used..used + 4].fill(0);
    let upgraded = MapDocument::parse(&MapPayload::new(omitted).unwrap())
        .unwrap()
        .serialize()
        .unwrap();
    assert_eq!(upgraded, payload);
    for offset in [0, 4, 12, 0x258] {
        let mut bytes = payload.bytes().to_vec();
        bytes[offset..offset + 4].fill(0xff);
        assert!(MapDocument::parse(&MapPayload::new(bytes).unwrap()).is_err());
    }
    assert!(
        MapDocument::from_chunks(vec![
            Chunk {
                tag: SETTINGS,
                data: vec![0; 0x3dc]
            },
            Chunk {
                tag: 9,
                data: vec![0; MAP_PAYLOAD_BYTES]
            }
        ])
        .is_err()
    );
    let maximum_data = MAP_PAYLOAD_BYTES - 4 - 16 - (8 + 0x3dc) - 8;
    let maximum = MapDocument::from_chunks(vec![
        Chunk {
            tag: SETTINGS,
            data: vec![0; 0x3dc],
        },
        Chunk {
            tag: 9,
            data: vec![0; maximum_data],
        },
    ])
    .unwrap();
    assert!(MapDocument::parse(&maximum.serialize().unwrap()).is_ok());
    assert!(
        MapDocument::from_chunks(vec![
            Chunk {
                tag: SETTINGS,
                data: vec![0; 0x3dc]
            },
            Chunk {
                tag: 9,
                data: vec![0; maximum_data + 1]
            },
        ])
        .is_err()
    );
    assert!(MapDocument::from_chunks(vec![]).is_err());
    let mut chunks = doc.chunks().to_vec();
    chunks.push(chunks[0].clone());
    assert!(MapDocument::from_chunks(chunks).is_err());
}

#[test]
fn t2408_description_edit_updates_only_typed_settings_and_roundtrips() {
    let initial = document();
    let original = initial.serialize().unwrap();
    let edited = initial.with_description("Rust-owned description").unwrap();
    assert_eq!(edited.description_bytes(), b"Rust-owned description");
    assert_eq!(
        edited.settings().title_bytes(),
        initial.settings().title_bytes()
    );
    assert_eq!(edited.nodes(), initial.nodes());
    let original_settings = initial.settings().encode().unwrap();
    let edited_settings = edited.settings().encode().unwrap();
    assert_eq!(&edited_settings[..0x82], &original_settings[..0x82]);
    assert_eq!(&edited_settings[0x162..], &original_settings[0x162..]);

    let serialized = edited.serialize().unwrap();
    let reparsed = MapDocument::parse(&serialized).unwrap();
    assert_eq!(reparsed.description_bytes(), b"Rust-owned description");
    assert_eq!(
        reparsed.settings().title_bytes(),
        initial.settings().title_bytes()
    );
    assert_ne!(serialized, original);
    assert!(initial.with_description(&"x".repeat(223)).is_ok());
    assert!(initial.with_description(&"x".repeat(224)).is_err());
    assert!(initial.with_description("invalid\ntext").is_err());
}

#[test]
fn t2408_description_commit_updates_owned_editor_history_and_dirty_state() {
    let initial = document();
    let mut state: StorageLifecycle<Vec<u8>> = StorageLifecycle::default();
    assert_eq!(
        state.edit_map_description("new description"),
        Err(status::INVALID_PARAMETER)
    );

    state.edit(initial.clone());
    assert!(state.edit_map_description("new description").unwrap());
    assert!(state.editor_dirty());
    assert_eq!(
        state.editor().unwrap().description_bytes(),
        b"new description"
    );
    assert!(state.editor_can_undo());
    assert!(!state.edit_map_description("new description").unwrap());
    assert!(state.editor_can_undo());

    let edited = state.editor().unwrap().serialize().unwrap();
    assert!(state.undo_editor().unwrap());
    assert_eq!(
        state.editor().unwrap().serialize().unwrap(),
        initial.serialize().unwrap()
    );
    assert!(state.redo_editor().unwrap());
    assert_eq!(state.editor().unwrap().serialize().unwrap(), edited);

    let before_invalid = state.editor().unwrap().serialize().unwrap();
    assert!(state.edit_map_description("invalid\ntext").is_err());
    assert_eq!(state.editor().unwrap().serialize().unwrap(), before_invalid);
}

#[test]
fn t2383_editor_undo_redo_is_owned_and_branching_drops_redo() {
    let initial = document();
    let first_edit = initial.with_title("First edit").unwrap();
    let branch = initial.with_title("Branched edit").unwrap();
    let mut state: StorageLifecycle<Vec<u8>> = StorageLifecycle::default();

    state.edit(initial.clone());
    state.edit(first_edit.clone());
    assert!(state.editor_can_undo());
    assert!(!state.editor_can_redo());
    assert!(state.undo_editor().unwrap());
    assert_eq!(
        state.editor().unwrap().serialize().unwrap(),
        initial.serialize().unwrap()
    );
    assert!(state.editor_dirty());
    assert!(state.editor_can_redo());

    assert!(state.redo_editor().unwrap());
    assert_eq!(
        state.editor().unwrap().serialize().unwrap(),
        first_edit.serialize().unwrap()
    );
    assert!(state.undo_editor().unwrap());
    state.edit(branch);
    assert!(!state.editor_can_redo());
    assert!(state.undo_editor().unwrap());
    assert_eq!(
        state.editor().unwrap().serialize().unwrap(),
        initial.serialize().unwrap()
    );
    assert_eq!(state.editor_source(), None);
}

#[test]
fn t2383_live_editor_snapshots_record_only_changed_owned_documents() {
    let initial = document();
    let edited = initial.with_title("Live frame edit").unwrap();
    let mut state: StorageLifecycle<Vec<u8>> = StorageLifecycle::default();
    state.edit(initial.clone());

    assert!(
        state
            .record_editor_snapshot(edited.serialize().unwrap())
            .unwrap()
    );
    assert!(!state.editor_can_redo());
    assert!(state.editor_can_undo());
    assert!(
        !state
            .record_editor_snapshot(edited.serialize().unwrap())
            .unwrap()
    );
    assert!(state.undo_editor().unwrap());
    assert_eq!(
        state.editor().unwrap().serialize().unwrap(),
        initial.serialize().unwrap()
    );
}

#[test]
fn activation_extension_failures_context_and_editor_persistence() {
    let root = std::env::temp_dir().join(format!(
        "t2319-lifecycle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = LocalStorage::open(root).unwrap();
    let mut frontend = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([19; 16]),
    ));
    let mut state: StorageLifecycle<Vec<u8>> = StorageLifecycle::default();
    let p = initialize_profile(&defaults()).unwrap();
    assert_eq!(&p.payload[..4], &[0; 4]);
    assert_eq!(p.signature, [0; 20]);
    state.open(Target::Profile { player: 0 }).unwrap();
    let receipt = frontend
        .handle(
            Command::CreateProfile {
                player: 0,
                directory: "player".into(),
                label: "Synthetic".into(),
                candidate: p,
            },
            &mut Samples,
        )
        .unwrap()
        .unwrap();
    assert!(
        state
            .adopt(&frontend, &receipt, |_| Err(status::UNRECOGNIZED_VOLUME))
            .is_err()
    );
    assert!(state.player(0).is_none());
    assert_eq!(
        state.context(),
        Context::Storage(Target::Profile { player: 0 })
    );
    state
        .adopt(&frontend, &receipt, |_| Ok(vec![4, 5, 6]))
        .unwrap();
    assert_eq!(state.player(0).unwrap().extension, vec![4, 5, 6]);
    assert!(state.adopt(&frontend, &receipt, |_| Ok(vec![])).is_err());
    state.open(Target::Map).unwrap();
    let original_document = document();
    state.edit(original_document.clone());
    state.edit(original_document.with_title("Undo before save").unwrap());
    assert!(state.undo_editor().unwrap());
    assert_eq!(
        state.editor().unwrap().serialize().unwrap(),
        original_document.serialize().unwrap()
    );
    let receipt = state
        .save_editor(
            &mut frontend,
            &mut Samples,
            Some(("map".into(), "Synthetic map".into())),
        )
        .unwrap();
    assert!(!state.editor_dirty());
    assert_eq!(state.context(), Context::Editor);
    state.edit(document());
    state.open(Target::Map).unwrap();
    state
        .save_editor(&mut frontend, &mut Samples, None)
        .unwrap();
    assert!(!state.editor_dirty());
    state.deactivate(0).unwrap();
    assert!(state.player(0).is_none());
    let mut reopened = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([19; 16]),
    ));
    state.open(Target::Map).unwrap();
    let loaded = reopened
        .handle(
            Command::LoadMap {
                source: MapSource::Local(receipt.key.unwrap()),
            },
            &mut Samples,
        )
        .unwrap()
        .unwrap();
    state.adopt(&reopened, &loaded, |_| Ok(vec![])).unwrap();
    assert_eq!(
        state.editor().unwrap().serialize().unwrap(),
        document().serialize().unwrap()
    );
    state.open(Target::Profile { player: 1 }).unwrap();
    state.cancel().unwrap();
    assert_eq!(state.context(), Context::Editor);
}

#[test]
fn selected_story_profile_commit_updates_lifecycle_and_survives_reopen() {
    let root = std::env::temp_dir().join(format!(
        "t1703-story-lifecycle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let mut frontend = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([19; 16]),
    ));
    let mut state: NativeStorageLifecycle = StorageLifecycle::default();
    state.open(Target::Profile { player: 0 }).unwrap();
    let created = frontend
        .handle(
            Command::CreateProfile {
                player: 0,
                directory: "story".into(),
                label: "Synthetic Story Profile".into(),
                candidate: initialize_profile(&defaults()).unwrap(),
            },
            &mut Samples,
        )
        .unwrap()
        .unwrap();
    state.adopt_native(&frontend, &created).unwrap();
    let key = created.key.unwrap();

    let receipt = state
        .save_story_progress(
            &mut frontend,
            0,
            StoryProgressUpdate {
                result_flags: 0x8000,
                campaign_ordinal: 2,
                selector: 1,
                mode: StoryMode::SinglePlayer,
                best_time: 12.0,
                minimum_time: 20.0,
            },
        )
        .unwrap();
    assert_eq!(receipt.key, key);
    assert_eq!(state.player(0).unwrap().saved.key, key);
    assert_eq!(
        StoryProgress::read(&state.player(0).unwrap().saved.profile)
            .unwrap()
            .single_player_completion[1],
        1 << 2
    );

    drop(frontend);
    let mut reopened = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([19; 16]),
    ));
    let loaded = reopened
        .handle(
            Command::LoadProfile {
                player: 0,
                key: key.clone(),
            },
            &mut Samples,
        )
        .unwrap()
        .unwrap();
    assert_eq!(loaded.key.as_ref(), Some(&key));
    let progress = StoryProgress::read(&reopened.flow().profile(0).unwrap().profile).unwrap();
    assert_eq!(progress.best_time_bits[2][1], 12.0f32.to_bits());
    assert_eq!(progress.single_player_completion[1], 1 << 2);
    assert_eq!(progress.story_counter, 1);
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn selected_story_profile_memory_updates_even_if_save_fails() {
    let root = std::env::temp_dir().join(format!(
        "t1703-story-lifecycle-failed-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let mut frontend = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([19; 16]),
    ));
    let mut state: NativeStorageLifecycle = StorageLifecycle::default();
    state.open(Target::Profile { player: 0 }).unwrap();
    let created = frontend
        .handle(
            Command::CreateProfile {
                player: 0,
                directory: "story".into(),
                label: "Synthetic Story Profile".into(),
                candidate: initialize_profile(&defaults()).unwrap(),
            },
            &mut Samples,
        )
        .unwrap()
        .unwrap();
    state.adopt_native(&frontend, &created).unwrap();
    let key = created.key.unwrap();
    let file_name = key.path().rsplit('\\').next().unwrap();
    let save_path = root.join("UDATA/45410066/story").join(file_name);
    let backup_path = save_path.with_extension("backup");
    let original_file = std::fs::read(&save_path).unwrap();
    std::fs::rename(&save_path, &backup_path).unwrap();
    std::fs::create_dir(&save_path).unwrap();

    let result = state.save_story_progress(
        &mut frontend,
        0,
        StoryProgressUpdate {
            result_flags: 0x8000,
            campaign_ordinal: 2,
            selector: 1,
            mode: StoryMode::SinglePlayer,
            best_time: 12.0,
            minimum_time: 20.0,
        },
    );
    assert!(result.is_err());
    let flow_progress = StoryProgress::read(&frontend.flow().profile(0).unwrap().profile).unwrap();
    let adopted_progress = StoryProgress::read(&state.player(0).unwrap().saved.profile).unwrap();
    assert_eq!(flow_progress, adopted_progress);
    assert_eq!(adopted_progress.best_time_bits[2][1], 12.0f32.to_bits());
    assert_eq!(adopted_progress.single_player_completion[1], 1 << 2);
    std::fs::remove_dir(&save_path).unwrap();
    std::fs::rename(&backup_path, &save_path).unwrap();
    assert_eq!(std::fs::read(&save_path).unwrap(), original_file);

    drop(frontend);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_settings_constructor_uses_owned_runtime_ring_and_retains_progress() {
    let ring = [(41, -3), (22, 5), (80, 1), (9, -1)]; // synthetic, no retail table
    let mut initializer = ProfileInitializer::new(ring, 2, 1.25).unwrap();
    let mut progress = defaults();
    progress.payload[0x80..0x1290].fill(0x5a);
    progress.payload[0x1290..0x1edc].fill(0xa5);
    for index in [2, 3, 0, 1, 2] {
        let result = initializer.initialize(&progress).unwrap();
        let settings = result.settings().unwrap();
        assert_eq!(settings.layout(), 1);
        assert_eq!(settings.head()[2..4], [0xff, 0]);
        assert_eq!(settings.turn_speed(), 1.25);
        assert_eq!(
            settings.mapping(),
            [
                0, 0, 0, 0, 0, 0, 4, 8, 1, 2, 0, 4, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        let word = |at| u32::from_le_bytes(settings.head()[at..at + 4].try_into().unwrap());
        assert_eq!(word(0x60), ring[index].0);
        assert_eq!(word(0x64), ring[index].1 as u32);
        assert_eq!(word(0x68), 4);
        assert_eq!(word(0x6c), 3);
        assert_eq!(word(0x74), 2);
        assert_eq!(&result.payload[0x80..], &progress.payload[0x80..]);
    }
    let cursor = initializer.next_ring_slot();
    progress.payload.clear();
    assert!(initializer.initialize(&progress).is_err());
    assert_eq!(initializer.next_ring_slot(), cursor);
    assert!(ProfileInitializer::new(ring, 4, 1.0).is_err());
    assert!(ProfileInitializer::new(ring, 0, f32::NAN).is_err());
}

#[test]
fn native_profile_extension_reaches_activation_and_survives_reopen() {
    use tsfp_input::settings::{InputSettings, SettingEdit};
    let root = std::env::temp_dir().join(format!(
        "t2319-native-extension-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let mut frontend = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([24; 16]),
    ));
    let mut state = NativeStorageLifecycle::default();
    state.open(Target::Profile { player: 0 }).unwrap();
    let completion = frontend
        .handle(
            Command::CreateProfile {
                player: 0,
                directory: "native".into(),
                label: "Synthetic".into(),
                candidate: defaults(),
            },
            &mut Samples,
        )
        .unwrap()
        .unwrap();
    state.adopt_native(&frontend, &completion).unwrap();
    let mut input = InputSettings::default();
    input.apply(SettingEdit::SensitivityYaw(0.12)).unwrap();
    state
        .save_input(&mut frontend, 0, input.clone(), &mut Samples)
        .unwrap();
    assert_eq!(state.player(0).unwrap().extension, input);
    assert!(state.player(0).unwrap().saved.native_input);
    state
        .save_input(&mut frontend, 0, input.clone(), &mut Samples)
        .unwrap();
    state.deactivate(0).unwrap();
    assert_eq!(state.context(), Context::Menu);
    let key = completion.key.unwrap();
    let mut reopened = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([24; 16]),
    ));
    state.open(Target::Profile { player: 2 }).unwrap();
    let receipt = reopened
        .handle(
            Command::LoadProfile {
                player: 2,
                key: key.clone(),
            },
            &mut Samples,
        )
        .unwrap()
        .unwrap();
    state.adopt_native(&reopened, &receipt).unwrap();
    assert_eq!(state.player(2).unwrap().extension, input);
    let mut broken = input.clone();
    broken.apply(SettingEdit::SensitivityYaw(0.2)).unwrap();
    // Real filesystem failure: replace the directory with a non-directory after loading.
    let directory = root.join("UDATA/45410066/native");
    let retained = root.join("UDATA/45410066/native-retained");
    std::fs::rename(&directory, retained).unwrap();
    std::fs::write(directory, b"synthetic obstacle").unwrap();
    assert!(
        state
            .save_input(&mut reopened, 2, broken, &mut Samples)
            .is_err()
    );
    assert_eq!(state.player(2).unwrap().extension, input);
    assert_eq!(
        state.context(),
        Context::Storage(Target::Profile { player: 2 })
    );
}

#[test]
fn runtime_constructor_checks_complete_stable_tables_and_uses_original_speed() {
    use std::{cell::Cell, collections::BTreeMap};
    use tsfp_engine_fileio::storage_bootstrap::RuntimeRead;
    struct Runtime {
        values: BTreeMap<u32, u32>,
        reads: Cell<usize>,
        changed: Option<u32>,
    }
    impl RuntimeRead for Runtime {
        fn word(&self, address: u32) -> Option<u32> {
            let reads = self.reads.get();
            self.reads.set(reads + 1);
            self.values.get(&address).copied().map(|value| {
                if reads >= 9 && self.changed == Some(address) {
                    value ^ 1
                } else {
                    value
                }
            })
        }
    }
    let mut runtime = Runtime {
        values: [
            (0x523660, 41),
            (0x523664, 22),
            (0x523668, 80),
            (0x52366c, 9),
            (0x523670, (-3i32) as u32),
            (0x523674, 5),
            (0x523678, 1),
            (0x52367c, u32::MAX),
            (0x7613cc, 2),
        ]
        .into_iter()
        .collect(),
        reads: Cell::new(0),
        changed: None,
    };
    let mut owned = ProfileInitializer::from_runtime(&runtime).unwrap();
    assert_eq!(owned.next_ring_slot(), 2);
    let profile = owned.initialize(&defaults()).unwrap();
    let settings = profile.settings().unwrap();
    assert_eq!(settings.turn_speed(), 0.5);
    assert_eq!(&settings.head()[0x60..0x64], &80u32.to_le_bytes());
    assert_eq!(&settings.head()[0x64..0x68], &1u32.to_le_bytes());
    assert_eq!(owned.next_ring_slot(), 3);
    assert_eq!(
        runtime.values[&0x7613cc], 2,
        "owned advancement never writes the runtime cursor"
    );
    for changed in [0x523670, 0x7613cc] {
        runtime.reads.set(0);
        runtime.changed = Some(changed);
        assert!(ProfileInitializer::from_runtime(&runtime).is_err());
    }
    runtime.changed = None;
    runtime.values.insert(0x7613cc, 4);
    assert!(ProfileInitializer::from_runtime(&runtime).is_err());
    runtime.values.insert(0x7613cc, 2);
    runtime.values.remove(&0x523660);
    assert!(ProfileInitializer::from_runtime(&runtime).is_err());
}
