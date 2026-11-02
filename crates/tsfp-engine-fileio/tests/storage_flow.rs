//! Tier 0, FABRICATED signing and payloads. Real host-directory VFS persistence.
use tsfp_engine_fileio::local_storage::LocalStorage;
use tsfp_engine_fileio::storage_flow::*;
use tsfp_engine_fileio::story_progress::{StoryMode, StoryProgress, StoryProgressUpdate};
use tsfp_engine_fileio::vfs::{NtStatus, status};
use tsfp_format_profile::Profile;

// Deliberately not a production signer, nor an Xbox authentication claim.
struct TestAuthentication;
impl SaveAuthentication for TestAuthentication {
    fn sign(&mut self, kind: SaveKind, bytes: &[u8]) -> Result<[u8; 20], NtStatus> {
        let mut result = [if kind == SaveKind::Profile { 1 } else { 2 }; 20];
        for (i, &byte) in bytes.iter().enumerate() {
            result[i % 20] ^= byte;
        }
        Ok(result)
    }
    fn verify(
        &mut self,
        kind: SaveKind,
        bytes: &[u8],
        signature: &[u8; 20],
    ) -> Result<(), NtStatus> {
        if &self.sign(kind, bytes)? == signature {
            Ok(())
        } else {
            Err(status::UNRECOGNIZED_VOLUME)
        }
    }
}

struct Samples;
impl SampleMaps for Samples {
    fn load_sample(&mut self, id: u32) -> Result<MapPayload, NtStatus> {
        if id != 999 {
            return Err(status::OBJECT_NAME_NOT_FOUND);
        }
        MapPayload::new(vec![7; MAP_PAYLOAD_BYTES])
    }

    fn catalogue(&mut self) -> Result<Vec<SampleMapEntry>, NtStatus> {
        Ok(vec![SampleMapEntry::new(999, "Synthetic sample".into())?])
    }
}

fn profile() -> Profile {
    let mut profile = Profile {
        payload: vec![0; 0x1f54],
        signature: [0; 20],
    };
    profile.payload[..4].copy_from_slice(&123u32.to_le_bytes());
    profile.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    profile
}

#[test]
fn native_profile_map_persistence_and_failed_adoption() {
    let root = std::env::temp_dir().join(format!("t2319-flow-{}", std::process::id()));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let vfs = storage.vfs().unwrap();
    let save_dir = root.join("UDATA/45410066/Synthetic");
    std::fs::create_dir_all(&save_dir).unwrap();
    let mut flow = StorageFlow::new(vfs, TestAuthentication);
    let key = SaveKey::new("Synthetic", "tsftrpft").unwrap();
    flow.save_profile(0, key.clone(), profile()).unwrap();
    assert_eq!(
        &flow.profile(0).unwrap().profile.payload[..4],
        &123u32.to_le_bytes()
    );
    assert_eq!(
        &std::fs::read(save_dir.join("tsftrpft")).unwrap()[..4],
        &[0; 4]
    );
    assert!(flow.profile(1).is_none());
    assert_eq!(
        flow.save_profile(4, key.clone(), profile()),
        Err(status::INVALID_PARAMETER)
    );
    assert_eq!(flow.failure().unwrap().operation, Operation::ProfileSave);
    assert!(flow.profile(4).is_none());

    drop(flow);
    let mut flow = StorageFlow::new(storage.vfs().unwrap(), TestAuthentication);
    flow.load_profile(0, key.clone()).unwrap();
    assert_eq!(flow.profile(0).unwrap().key, key);
    assert_eq!(&flow.profile(0).unwrap().profile.payload[..4], &[0; 4]);
    let mut broken = std::fs::read(save_dir.join("tsftrpft")).unwrap();
    broken[50] ^= 1;
    std::fs::write(save_dir.join("tsftrpft"), broken).unwrap();
    assert_eq!(flow.load_profile(1, key), Err(status::UNRECOGNIZED_VOLUME));
    assert!(flow.profile(1).is_none());
    assert!(flow.profile(0).is_some());

    flow.load_map(MapSource::DiscSample(999), &mut Samples)
        .unwrap();
    assert!(!flow.map().unwrap().dirty);
    let edited = MapPayload::new(vec![9; MAP_PAYLOAD_BYTES]).unwrap();
    flow.edit_map(MapSource::DiscSample(999), edited.clone());
    let missing = SaveKey::new("Absent", "anicemap.mkr").unwrap();
    assert!(flow.save_map(missing).is_err());
    assert_eq!(flow.map().unwrap().source, MapSource::DiscSample(999));
    assert!(flow.map().unwrap().dirty);
    let map_key = SaveKey::new("Synthetic", "anicemap.mkr").unwrap();
    flow.save_map(map_key.clone()).unwrap();
    assert_eq!(flow.failure(), None);
    assert!(!flow.map().unwrap().dirty);
    assert_eq!(
        flow.map().unwrap().source,
        MapSource::Local(map_key.clone())
    );
    assert_eq!(
        std::fs::read(save_dir.join("anicemap.mkr")).unwrap().len(),
        MAP_PAYLOAD_BYTES + 20
    );
    drop(flow);
    let mut reopened = StorageFlow::new(storage.vfs().unwrap(), TestAuthentication);
    reopened
        .load_map(MapSource::Local(map_key.clone()), &mut Samples)
        .unwrap();
    assert_eq!(reopened.map().unwrap().payload, edited);
    std::fs::write(save_dir.join("anicemap.mkr"), b"truncated").unwrap();
    assert!(
        reopened
            .load_map(MapSource::Local(map_key), &mut Samples)
            .is_err()
    );
    assert_eq!(reopened.map().unwrap().payload, edited);
    assert!(!reopened.map().unwrap().dirty);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn story_progress_commit_requires_active_key_and_survives_fresh_flow_reload() {
    let root = std::env::temp_dir().join(format!("t1703-story-progress-{}", std::process::id()));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let save_dir = root.join("UDATA/45410066/Synthetic");
    std::fs::create_dir_all(&save_dir).unwrap();
    let key = SaveKey::new("Synthetic", "tsftrpft").unwrap();
    let other_key = SaveKey::new("Synthetic", "other").unwrap();
    let mut initial = profile();
    let summary = 0x1290;
    let time = summary + 0x288 + 2 * 0x28 + 0x0c + 4;
    let counter = summary + 0x918;
    initial.payload[time..time + 4].copy_from_slice(&20.0f32.to_bits().to_le_bytes());
    initial.payload[counter..counter + 4].copy_from_slice(&u32::MAX.to_le_bytes());

    let mut flow = StorageFlow::new(storage.vfs().unwrap(), TestAuthentication);
    flow.save_profile(0, key.clone(), initial).unwrap();
    let unchanged_file = std::fs::read(save_dir.join("tsftrpft")).unwrap();
    let update = StoryProgressUpdate {
        result_flags: 0x8000,
        campaign_ordinal: 2,
        selector: 1,
        mode: StoryMode::SinglePlayer,
        best_time: 12.0,
        minimum_time: 15.0,
    };
    assert_eq!(
        flow.save_story_progress(0, &other_key, update),
        Err(status::INVALID_PARAMETER)
    );
    assert_eq!(
        std::fs::read(save_dir.join("tsftrpft")).unwrap(),
        unchanged_file
    );
    assert_eq!(
        StoryProgress::read(&flow.profile(0).unwrap().profile)
            .unwrap()
            .story_counter,
        u32::MAX
    );

    let receipt = flow.save_story_progress(0, &key, update).unwrap();
    assert_eq!(receipt.key, key);
    assert!(receipt.progress.best_time_changed);
    assert_eq!(receipt.progress.counter_before, u32::MAX);
    assert_eq!(receipt.progress.counter_after, 0);
    drop(flow);

    let mut reopened = StorageFlow::new(storage.vfs().unwrap(), TestAuthentication);
    reopened.load_profile(3, key.clone()).unwrap();
    assert_eq!(reopened.profile(3).unwrap().key, key);
    let progress = StoryProgress::read(&reopened.profile(3).unwrap().profile).unwrap();
    assert_eq!(progress.best_time_bits[2][1], 12.0f32.to_bits());
    assert_eq!(progress.single_player_completion[0], 1 << 2);
    assert_eq!(progress.single_player_completion[1], 1 << 2);
    assert_eq!(progress.story_counter, 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn story_progress_remains_in_active_profile_when_disk_write_fails() {
    let root = std::env::temp_dir().join(format!(
        "t1703-story-failed-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let save_dir = root.join("UDATA/45410066/Synthetic");
    std::fs::create_dir_all(&save_dir).unwrap();
    let key = SaveKey::new("Synthetic", "tsftrpft").unwrap();
    let mut flow = StorageFlow::new(storage.vfs().unwrap(), TestAuthentication);
    flow.save_profile(0, key.clone(), profile()).unwrap();

    let save_path = save_dir.join("tsftrpft");
    let backup_path = save_dir.join("tsftrpft.backup");
    let original_file = std::fs::read(&save_path).unwrap();
    std::fs::rename(&save_path, &backup_path).unwrap();
    std::fs::create_dir(&save_path).unwrap();
    let result = flow.save_story_progress(
        0,
        &key,
        StoryProgressUpdate {
            result_flags: 0x8000,
            campaign_ordinal: 2,
            selector: 1,
            mode: StoryMode::SinglePlayer,
            best_time: 12.0,
            minimum_time: 15.0,
        },
    );
    assert!(result.is_err());
    let progress = StoryProgress::read(&flow.profile(0).unwrap().profile).unwrap();
    assert_eq!(progress.best_time_bits[2][1], 12.0f32.to_bits());
    assert_eq!(progress.single_player_completion[0], 1 << 2);
    assert_eq!(progress.single_player_completion[1], 1 << 2);
    assert_eq!(progress.story_counter, 1);
    std::fs::remove_dir(&save_path).unwrap();
    std::fs::rename(&backup_path, &save_path).unwrap();
    assert_eq!(std::fs::read(&save_path).unwrap(), original_file);
    drop(flow);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn paths_payloads_and_authentication_fail_closed() {
    for directory in ["", ".", "..", "a/b", "a\\b", "C:", "a\0b"] {
        assert!(SaveKey::new(directory, "tsftrpft").is_err());
    }
    for filename in ["", "..", "a/b", "x:y", "a\nb"] {
        assert!(SaveKey::new("Synthetic", filename).is_err());
    }
    assert!(MapPayload::new(vec![0; MAP_PAYLOAD_BYTES - 1]).is_err());
    assert!(MapPayload::new(vec![0; MAP_PAYLOAD_BYTES + 1]).is_err());

    struct Unavailable;
    impl SaveAuthentication for Unavailable {
        fn sign(&mut self, _: SaveKind, _: &[u8]) -> Result<[u8; 20], NtStatus> {
            Err(status::ACCESS_DENIED)
        }
        fn verify(&mut self, _: SaveKind, _: &[u8], _: &[u8; 20]) -> Result<(), NtStatus> {
            Err(status::ACCESS_DENIED)
        }
    }
    let mut flow = StorageFlow::new(tsfp_engine_fileio::vfs::Vfs::new(), Unavailable);
    let key = SaveKey::new("Synthetic", "tsftrpft").unwrap();
    assert_eq!(
        flow.save_profile(0, key, profile()),
        Err(status::ACCESS_DENIED)
    );
    assert!(flow.profile(0).is_none());
    let mut malformed = profile();
    malformed.payload.clear();
    assert_eq!(
        flow.save_profile(0, SaveKey::new("Synthetic", "tsftrpft").unwrap(), malformed),
        Err(status::INVALID_PARAMETER)
    );
}

#[test]
fn native_catalogue_preserves_payload_identity_and_metadata_errors() {
    let root = std::env::temp_dir().join(format!("t2319-catalogue-{}", std::process::id()));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let mut vfs = storage.vfs().unwrap();
    let base = root.join("UDATA/45410066");
    for directory in ["First", "Second"] {
        std::fs::create_dir_all(base.join(directory)).unwrap();
    }
    let mut metadata = vec![0xff, 0xfe];
    for word in "Synthetic \u{1f4be}".encode_utf16() {
        metadata.extend(word.to_le_bytes());
    }
    std::fs::write(base.join("First/SaveMeta.xbx"), &metadata).unwrap();
    std::fs::write(base.join("First/tsftrpft"), vec![0; 8040]).unwrap();
    std::fs::write(
        base.join("First/anicemap.mkr"),
        vec![0; MAP_PAYLOAD_BYTES + 20],
    )
    .unwrap();
    std::fs::write(base.join("First/tsftrpft.tmp"), b"interrupted write").unwrap();
    std::fs::write(base.join("Second/tsftrpft"), b"malformed profile").unwrap();
    let rows = catalogue(&mut vfs).unwrap();
    assert_eq!(rows.len(), 3);
    let profiles: Vec<_> = rows
        .iter()
        .filter(|r| r.kind == SaveKind::Profile)
        .collect();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].key, SaveKey::new("First", "tsftrpft").unwrap());
    assert_eq!(profiles[0].label.as_deref(), Ok("Synthetic \u{1f4be}"));
    assert_eq!(profiles[0].bytes, 8040);
    assert!(profiles[1].label.is_err());
    assert_eq!(profiles[1].bytes, 17);
    assert_eq!(vfs.open_count(), 0);
    // Metadata corruption does not silently remove the save row.
    std::fs::write(base.join("First/SaveMeta.xbx"), b"bad metadata").unwrap();
    assert!(
        catalogue(&mut vfs)
            .unwrap()
            .iter()
            .all(|r| r.label.is_err())
    );
    assert_eq!(vfs.open_count(), 0);
    let mut maximum = vec![0xff, 0xfe];
    for _ in 0..140 {
        maximum.extend(65u16.to_le_bytes());
    }
    assert_eq!(decode_save_label(&maximum).unwrap().len(), 140);
    metadata.extend([0, 0]);
    assert_eq!(decode_save_label(&metadata).unwrap(), "Synthetic \u{1f4be}");
    for invalid in [
        vec![],
        vec![0xff, 0xfe],
        vec![0xff, 0xfe, 0, 0xd8],
        vec![0xff, 0xfe, 65],
        vec![0; 284],
    ] {
        assert!(decode_save_label(&invalid).is_err());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_allocation_metadata_and_collision_preserve_existing_saves() {
    let root = std::env::temp_dir().join(format!("t2319-allocation-{}", std::process::id()));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let mut flow = StorageFlow::new(storage.vfs().unwrap(), TestAuthentication);
    let key = flow
        .allocate_save("RuntimeIdentity", SaveKind::Profile, "Native \u{1f4be}")
        .unwrap();
    flow.save_profile(0, key.clone(), profile()).unwrap();
    let rows = flow.catalogue().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, key);
    assert_eq!(rows[0].label.as_deref(), Ok("Native \u{1f4be}"));
    let base = root.join("UDATA/45410066/runtimeidentity");
    let before = std::fs::read(base.join("tsftrpft")).unwrap();
    assert_eq!(
        flow.allocate_save("runtimeidentity", SaveKind::Map, "Different"),
        Err(status::OBJECT_NAME_COLLISION)
    );
    assert_eq!(std::fs::read(base.join("tsftrpft")).unwrap(), before);
    assert_eq!(
        flow.catalogue().unwrap()[0].label.as_deref(),
        Ok("Native \u{1f4be}")
    );
    for label in [
        String::new(),
        "a\0b".into(),
        "x".repeat(141),
        "\u{1f4be}".repeat(71),
    ] {
        assert_eq!(
            flow.allocate_save("Invalid", SaveKind::Profile, &label),
            Err(status::INVALID_PARAMETER)
        );
        assert!(!root.join("UDATA/45410066/Invalid").exists());
    }
    let maximum = "\u{1f4be}".repeat(70);
    assert_eq!(
        decode_save_label(&encode_save_label(&maximum).unwrap()).unwrap(),
        maximum
    );
    let map_key = flow
        .allocate_save("MapIdentity", SaveKind::Map, "Map")
        .unwrap();
    flow.edit_map(
        MapSource::DiscSample(999),
        MapPayload::new(vec![8; MAP_PAYLOAD_BYTES]).unwrap(),
    );
    flow.save_map(map_key).unwrap();
    assert_eq!(flow.catalogue().unwrap().len(), 2);
    drop(flow);
    let mut reopened = StorageFlow::new(storage.vfs().unwrap(), TestAuthentication);
    reopened.load_profile(1, key).unwrap();
    assert!(reopened.profile(1).is_some());
    let mut unmounted = tsfp_engine_fileio::vfs::Vfs::new();
    assert_eq!(
        allocate_save(&mut unmounted, "Absent", SaveKind::Map, "Map"),
        Err(status::OBJECT_PATH_NOT_FOUND)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_title_authentication_matches_independent_hmac_and_rejects_changes() {
    // FABRICATED key/payload; expected digest from Python stdlib hmac.digest.
    let key = std::array::from_fn(|i| i as u8);
    let mut auth = LocalTitleAuthentication::from_runtime_title_key(key);
    let bytes = b"Synthetic title save";
    let expected = [
        0xc8, 0x56, 0x21, 0xcb, 0xdf, 0x9d, 0x57, 0x31, 0x0c, 0x93, 0x26, 0x14, 0xe8, 0x15, 0x28,
        0x0f, 0x42, 0x40, 0x70, 0x8c,
    ];
    assert_eq!(auth.sign(SaveKind::Profile, bytes).unwrap(), expected);
    assert_eq!(auth.sign(SaveKind::Map, bytes).unwrap(), expected);
    auth.verify(SaveKind::Profile, bytes, &expected).unwrap();
    for i in 0..20 {
        let mut changed = expected;
        changed[i] ^= 1;
        assert_eq!(
            auth.verify(SaveKind::Profile, bytes, &changed),
            Err(status::UNRECOGNIZED_VOLUME)
        );
    }
    assert!(
        auth.verify(SaveKind::Map, b"Changed save", &expected)
            .is_err()
    );
    let mut other = LocalTitleAuthentication::from_runtime_title_key([9; 16]);
    assert!(other.verify(SaveKind::Profile, bytes, &expected).is_err());

    let root = std::env::temp_dir().join(format!("t2319-native-auth-{}", std::process::id()));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let mut flow = StorageFlow::new(storage.vfs().unwrap(), auth);
    let profile_key = flow
        .allocate_save("SignedProfile", SaveKind::Profile, "Profile")
        .unwrap();
    flow.save_profile(0, profile_key.clone(), profile())
        .unwrap();
    let map_key = flow
        .allocate_save("SignedMap", SaveKind::Map, "Map")
        .unwrap();
    flow.edit_map(
        MapSource::DiscSample(999),
        MapPayload::new(vec![3; MAP_PAYLOAD_BYTES]).unwrap(),
    );
    flow.save_map(map_key.clone()).unwrap();
    drop(flow);
    let mut reopened = StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key(key),
    );
    reopened.load_profile(2, profile_key.clone()).unwrap();
    reopened
        .load_map(MapSource::Local(map_key), &mut Samples)
        .unwrap();
    assert_eq!(reopened.profile(2).unwrap().key, profile_key);
    assert!(!reopened.map().unwrap().dirty);
    drop(reopened);
    let mut wrong = StorageFlow::new(storage.vfs().unwrap(), other);
    assert!(wrong.load_profile(0, profile_key).is_err());
    assert!(wrong.profile(0).is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn owned_frontend_create_update_reopen_and_map_without_device_screen() {
    use tsfp_engine_fileio::storage_frontend::{Command, Screen, StorageFrontend, Target};
    let root = std::env::temp_dir().join(format!("t2319-frontend-{}", std::process::id()));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let mut ui = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([0x52; 16]),
    ));
    let mut samples = Samples;
    ui.handle(Command::Open(Target::Profile { player: 0 }), &mut samples)
        .unwrap();
    assert!(matches!(ui.screen(), Screen::Catalogue { entries, .. } if entries.is_empty()));
    let receipt = ui
        .handle(
            Command::CreateProfile {
                player: 0,
                directory: "NativeProfile".into(),
                label: "Synthetic player".into(),
                candidate: profile(),
            },
            &mut samples,
        )
        .unwrap()
        .unwrap();
    assert_eq!(receipt.operation, Operation::ProfileSave);
    assert_eq!(ui.screen(), &Screen::Closed);
    let profile_key = receipt.key.unwrap();
    let mut updated = profile();
    updated.payload[128] = 73;
    ui.handle(
        Command::SaveProfile {
            player: 0,
            candidate: updated,
        },
        &mut samples,
    )
    .unwrap();
    let map = MapPayload::new(vec![25; MAP_PAYLOAD_BYTES]).unwrap();
    let map_key = ui
        .handle(
            Command::SaveMap {
                directory: "NativeMap".into(),
                label: "Synthetic map".into(),
                payload: map,
            },
            &mut samples,
        )
        .unwrap()
        .unwrap()
        .key
        .unwrap();
    assert!(!ui.flow().map().unwrap().dirty);
    ui.handle(
        Command::UpdateMap {
            payload: MapPayload::new(vec![26; MAP_PAYLOAD_BYTES]).unwrap(),
        },
        &mut samples,
    )
    .unwrap();
    drop(ui);
    let mut ui = StorageFrontend::new(StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key([0x52; 16]),
    ));
    ui.handle(Command::Open(Target::Profile { player: 1 }), &mut samples)
        .unwrap();
    assert!(
        matches!(ui.screen(), Screen::Catalogue { entries, .. } if entries.len() == 1 && entries[0].key == profile_key)
    );
    ui.handle(
        Command::LoadProfile {
            player: 1,
            key: profile_key.clone(),
        },
        &mut samples,
    )
    .unwrap();
    assert_eq!(ui.flow().profile(1).unwrap().profile.payload[128], 73);
    ui.handle(Command::Open(Target::Map), &mut samples).unwrap();
    assert!(
        matches!(ui.screen(), Screen::Catalogue { entries, sample_maps, .. } if entries.len() == 1 && entries[0].key == map_key && sample_maps == &[SampleMapEntry { id: 999, label: "Synthetic sample".into() }])
    );
    ui.handle(
        Command::LoadMap {
            source: MapSource::Local(map_key),
        },
        &mut samples,
    )
    .unwrap();
    assert_eq!(ui.flow().map().unwrap().payload.bytes()[100], 26);
    assert_eq!(ui.screen(), &Screen::Closed);
    // Invalid player must not even reserve an identity.
    assert_eq!(
        ui.handle(
            Command::CreateProfile {
                player: 4,
                directory: "Invalid".into(),
                label: "Invalid".into(),
                candidate: profile()
            },
            &mut samples
        ),
        Err(status::INVALID_PARAMETER)
    );
    assert!(!root.join("UDATA/45410066/invalid").exists());
    // Failed load emits no receipt, preserves active profile and displays the error.
    let bad_key = SaveKey::new("Missing", "tsftrpft").unwrap();
    assert!(
        ui.handle(
            Command::LoadProfile {
                player: 1,
                key: bad_key
            },
            &mut samples
        )
        .is_err()
    );
    assert!(matches!(
        ui.screen(),
        Screen::Error {
            target: Target::Profile { player: 1 },
            ..
        }
    ));
    assert_eq!(ui.flow().profile(1).unwrap().key, profile_key);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_input_is_authenticated_in_each_profile_and_applied_after_reload() {
    use tsfp_input::bindings::Action;
    use tsfp_input::settings::{Device, InputKind, InputSettings, SettingEdit};
    let root = std::env::temp_dir().join(format!("t2211-profile-input-{}", std::process::id()));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let title_key = [0x51; 16]; // FABRICATED key; production HMAC implementation.
    let mut flow = StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key(title_key),
    );
    let key = flow
        .allocate_save("InputProfile", SaveKind::Profile, "Native input")
        .unwrap();
    flow.save_profile(0, key.clone(), profile()).unwrap();
    let mut input = InputSettings::default();
    input
        .apply_player(2, SettingEdit::SensitivityYaw(0.12))
        .unwrap();
    input
        .apply_player(2, SettingEdit::InvertPitch(true))
        .unwrap();
    input
        .apply(SettingEdit::AssignDevice {
            player: 2,
            device: Device::KeyboardMouse,
        })
        .unwrap();
    input
        .apply_player(
            2,
            SettingEdit::Rebind {
                kind: InputKind::Key,
                input: "KeyW".into(),
                action: Action::A,
            },
        )
        .unwrap();
    flow.save_input(0, input.clone()).unwrap();
    flow.save_profile(0, key.clone(), profile()).unwrap(); // ordinary saves preserve extension
    let new_key = flow
        .allocate_save("NewProfile", SaveKind::Profile, "Defaults")
        .unwrap();
    flow.save_profile(1, new_key.clone(), profile()).unwrap();
    assert_eq!(flow.profile(1).unwrap().input, InputSettings::default());
    drop(flow);
    let mut reloaded = StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key(title_key),
    );
    reloaded.load_profile(3, key.clone()).unwrap();
    assert_eq!(reloaded.profile(3).unwrap().input, input);
    reloaded.load_profile(0, new_key).unwrap();
    assert_eq!(reloaded.profile(0).unwrap().input, InputSettings::default());
    // Exercise the consumer with settings authenticated from disk, rather than
    // merely comparing serialized values. Synthetic input, not a game trace.
    use tsfp_input::player::Look;
    use tsfp_input::router::{DeviceEvent, Router, RouterConfig};
    let sample = |settings: &InputSettings| {
        let mut router = Router::new(RouterConfig {
            raw_look: true,
            ..Default::default()
        });
        router.apply_settings(settings);
        let owner = router.mouse_owner().unwrap();
        router.feed(DeviceEvent::MouseMotion {
            dx: 100.0,
            dy: 100.0,
        });
        router.feed(DeviceEvent::Key {
            name: "KeyW".into(),
            down: true,
        });
        router.tick()[usize::from(owner)].unwrap()
    };
    let restored = sample(&reloaded.profile(3).unwrap().input);
    let fresh = sample(&reloaded.profile(0).unwrap().input);
    assert!(matches!(restored.look, Look::Delta { yaw_deg, .. } if (yaw_deg - 12.0).abs() < 0.001));
    assert_ne!(
        restored.look, fresh.look,
        "loaded sensitivity reaches router output"
    );
    assert_ne!(
        restored.held, fresh.held,
        "loaded binding reaches router output"
    );
    // Native reset publishes through the same authenticated profile boundary.
    input.reset_player(2).unwrap();
    reloaded.save_input(3, input.clone()).unwrap();
    drop(reloaded);
    let mut reloaded = StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key(title_key),
    );
    reloaded.load_profile(3, key.clone()).unwrap();
    assert_eq!(reloaded.profile(3).unwrap().input, input);
    assert_eq!(
        input.player_bindings(2),
        Some(&tsfp_input::bindings::Bindings::default())
    );
    // Switching back to automatic hot-swap survives a fresh authenticated owner.
    input.apply(SettingEdit::FixedDevices(false)).unwrap();
    reloaded.save_input(3, input.clone()).unwrap();
    drop(reloaded);
    let mut reloaded = StorageFlow::new(
        storage.vfs().unwrap(),
        LocalTitleAuthentication::from_runtime_title_key(title_key),
    );
    reloaded.load_profile(3, key.clone()).unwrap();
    assert_eq!(reloaded.profile(3).unwrap().input, input);
    assert!(!reloaded.profile(3).unwrap().input.fixed_devices);
    let path = root.join("UDATA/45410066/inputprofile/tsftrpft");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0x1f68 + 15] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert!(reloaded.load_profile(3, key).is_err());
    assert_eq!(
        reloaded.profile(3).unwrap().input,
        input,
        "failed authentication preserves active state"
    );
}
