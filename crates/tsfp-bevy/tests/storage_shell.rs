//! Tier 0: production shell owner with synthetic extension hooks, real VFS I/O.
//! This harness runs in the Bevy host crate context while avoiding the renderer.
#[path = "../src/editor_navigation.rs"]
mod editor_navigation;
#[path = "../src/storage_caller.rs"]
mod storage_caller;
#[path = "../src/storage_session.rs"]
mod storage_session;
#[path = "../src/story_progress_bridge.rs"]
mod story_progress_bridge;
mod ui_extensions {
    use std::sync::Mutex;
    use tsfp_input::settings::InputSettings;
    static STATE: Mutex<(
        Option<InputSettings>,
        Option<InputSettings>,
        Option<u32>,
        bool,
    )> = Mutex::new((None, None, None, false));
    #[derive(Clone)]
    pub struct ProfileInputPolicy {
        pub settings: InputSettings,
        pub fixed_devices: bool,
    }
    pub fn profile_policy() -> Option<ProfileInputPolicy> {
        let s = STATE.lock().unwrap();
        Some(ProfileInputPolicy {
            settings: s.0.clone().unwrap_or_default(),
            fixed_devices: s.3,
        })
    }
    pub fn adopt_policy(policy: ProfileInputPolicy) {
        *STATE.lock().unwrap() = (Some(policy.settings), None, None, policy.fixed_devices);
    }
    pub fn adopt_profile(settings: InputSettings) {
        adopt_policy(ProfileInputPolicy {
            settings,
            fixed_devices: true,
        });
    }
    pub fn take_profile_edit() -> Option<InputSettings> {
        STATE.lock().unwrap().1.take()
    }
    pub fn clear_profile() {
        adopt_policy(ProfileInputPolicy {
            settings: InputSettings::default(),
            fixed_devices: false,
        });
    }
    pub fn profile_save_failed(status: u32) {
        STATE.lock().unwrap().2 = Some(status);
    }
    pub fn stage_profile_edit(settings: InputSettings) {
        let mut s = STATE.lock().unwrap();
        s.0 = Some(settings.clone());
        s.1 = Some(settings);
    }
    pub fn snapshot() -> (Option<InputSettings>, Option<u32>) {
        let s = STATE.lock().unwrap();
        (s.0.clone(), s.2)
    }
}
#[cfg(test)]
mod tests {
    use crate::storage_session::*;
    use tsfp_engine_fileio::{
        local_storage::LocalStorage,
        storage_flow::SampleMaps,
        storage_frontend::{Command, Target},
        storage_lifecycle::Context,
        vfs::{NtStatus, status},
    };
    use tsfp_engine_fileio::{
        map_document::{Chunk, MapDocument, SETTINGS},
        storage_flow::MapPayload,
    };
    use tsfp_input::settings::{InputSettings, SettingEdit};
    struct Samples;
    impl SampleMaps for Samples {
        fn load_sample(&mut self, _: u32) -> Result<MapPayload, NtStatus> {
            Err(status::OBJECT_NAME_NOT_FOUND)
        }
    }

    #[test]
    fn editor_load_opens_native_source_choice_and_local_catalogue() {
        use crate::storage_caller::{CatalogueAction, NativePageId, RetainedPage};
        let root = std::env::temp_dir().join(format!(
            "t2319-source-choice-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let storage = LocalStorage::open(root.clone()).unwrap();
        let mut session = StorageSession::new(&storage, [29; 16]).unwrap();
        let page = NativePageId(0x313010);
        session
            .begin_editor_load_choice_from(
                vec![RetainedPage {
                    id: page,
                    cursor: 8,
                    scroll: 0.0,
                }],
                page,
                page,
                &mut Samples,
            )
            .unwrap();
        let view = session.catalogue_view().unwrap();
        assert!(view.map_source_choice);
        assert_eq!(view.cursor, Some(0));
        session
            .catalogue_action(CatalogueAction::Focus(1), &mut Samples)
            .unwrap();
        session
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap();
        let view = session.catalogue_view().unwrap();
        assert!(!view.map_source_choice);
        assert_eq!(view.target, Target::Map);
        assert!(view.rows.is_empty());
        assert!(view.sample_maps.is_empty());
        assert!(matches!(
            session.frontend.screen(),
            tsfp_engine_fileio::storage_frontend::Screen::Catalogue {
                target: Target::Map,
                ..
            }
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shell_adopts_roundtrip_and_frame_edits_and_preserves_failure_context() {
        let root = std::env::temp_dir().join(format!(
            "t2319-shell-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let storage = LocalStorage::open(root.clone()).unwrap();
        let mut session = StorageSession::new(&storage, [37; 16]).unwrap();
        session.progress_defaults =
            Some(tsfp_engine_fileio::profile_defaults::ProgressDefaults::new(
                [tsfp_engine_fileio::profile_defaults::ModeDefault::Unchanged; 9],
            ));
        let mut input = InputSettings::default();
        input.apply(SettingEdit::SensitivityYaw(0.12)).unwrap();
        crate::ui_extensions::stage_profile_edit(input.clone());
        session.settings_tick(&mut Samples);
        assert_eq!(
            crate::ui_extensions::take_profile_edit(),
            Some(input.clone()),
            "a frame with no active profile cannot discard a pending edit"
        );
        let mut launch = InputSettings::default();
        launch.apply(SettingEdit::SensitivityYaw(0.08)).unwrap();
        crate::ui_extensions::adopt_policy(crate::ui_extensions::ProfileInputPolicy {
            settings: launch.clone(),
            fixed_devices: false,
        });
        let mut defaults = tsfp_format_profile::Profile {
            payload: vec![0; 0x1f54],
            signature: [0; 20],
        };
        defaults.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
        let candidate =
            tsfp_engine_fileio::storage_lifecycle::initialize_profile(&defaults).unwrap();
        // Five owned handlers reject role/target mismatches without mutating
        // lifecycle or producing an adoption receipt.
        let target = Target::Profile { player: 0 };
        for address in [0, 0x193120, 0x193310, 0x32b6a0, 0x32aee0] {
            assert_eq!(
                session.entry(address, target, EntryAction::Open, &mut Samples),
                Err(status::INVALID_PARAMETER),
            );
        }
        assert_eq!(session.lifecycle.context(), Context::Menu);
        assert!(session.route.take_completion().is_none());
        let EntryResult::Device(device) = session
            .entry(0x193310, target, EntryAction::DeviceReady, &mut Samples)
            .unwrap()
        else {
            panic!("device feedback must not load a profile")
        };
        assert_eq!(device.id, 9);
        assert_eq!(device.root, storage.root());
        assert_eq!(session.lifecycle.context(), Context::Menu);
        assert!(session.route.take_completion().is_none());
        let receipt = session
            .entry(
                0x193120,
                target,
                EntryAction::Profile(Command::CreateProfile {
                    player: 0,
                    directory: "shell".into(),
                    label: "Synthetic shell".into(),
                    candidate,
                }),
                &mut Samples,
            )
            .unwrap();
        let EntryResult::Completed(receipt) = receipt else {
            panic!("save entry must complete")
        };
        assert_eq!(
            session.caller.players[0].as_ref().unwrap().saved.key,
            receipt.key.clone().unwrap()
        );
        assert_eq!(session.caller.last_completion, Some(receipt.clone()));
        assert!(session.catalogue_view().is_none());
        assert!(session.lifecycle.player(0).unwrap().saved.native_input);
        assert_eq!(
            u32::from_le_bytes(
                session.lifecycle.player(0).unwrap().saved.profile.payload[0x84..0x88]
                    .try_into()
                    .unwrap()
            ),
            1
        );
        assert_eq!(
            session.lifecycle.player(0).unwrap().extension,
            InputSettings::default()
        );
        assert_eq!(
            crate::ui_extensions::snapshot().0,
            Some(InputSettings::default())
        );
        let progress_at = 0x80 + 0x6b0;
        let mut updated = session.lifecycle.player(0).unwrap().saved.profile.clone();
        updated.payload[progress_at..progress_at + 4].fill(0);
        session
            .command(
                Command::SaveProfile {
                    player: 0,
                    candidate: updated,
                },
                &mut Samples,
            )
            .unwrap();
        assert!(
            crate::ui_extensions::profile_policy()
                .unwrap()
                .fixed_devices
        );
        crate::ui_extensions::stage_profile_edit(input.clone());
        session.settings_tick(&mut Samples);
        assert_eq!(session.lifecycle.player(0).unwrap().extension, input);
        assert_eq!(
            &session.lifecycle.player(0).unwrap().saved.profile.payload
                [progress_at..progress_at + 4],
            &[0; 4],
            "input edits must not reset saved progress"
        );
        assert_eq!(session.lifecycle.context(), Context::Profile { player: 0 });
        session.deactivate(0).unwrap();
        assert!(session.caller.players[0].is_none());
        assert!(session.caller.last_completion.is_none());
        assert!(session.route.take_completion().is_none());
        assert_eq!(crate::ui_extensions::snapshot().0, Some(launch.clone()));
        assert!(
            !crate::ui_extensions::profile_policy()
                .unwrap()
                .fixed_devices
        );
        let mut reopened = StorageSession::new(&storage, [37; 16]).unwrap();
        use tsfp_engine_fileio::storage_route::Root;
        assert!(
            reopened
                .open_root(
                    Root::MapCatalogue,
                    Target::Profile { player: 2 },
                    &mut Samples
                )
                .is_err()
        );
        reopened
            .entry(
                0x192770,
                Target::Profile { player: 2 },
                EntryAction::Open,
                &mut Samples,
            )
            .unwrap();
        assert_eq!(reopened.route.entries().len(), 1);
        assert_eq!(reopened.route.entries()[0].key, receipt.key.unwrap());
        use crate::storage_caller::CatalogueAction;
        let view = reopened.catalogue_view().unwrap();
        assert_eq!(view.target, Target::Profile { player: 2 });
        assert_eq!(view.rows.len(), 1);
        assert_eq!(view.cursor, Some(0));
        assert!(
            reopened
                .catalogue_action(CatalogueAction::Focus(2), &mut Samples)
                .is_err()
        );
        reopened
            .catalogue_action(CatalogueAction::Step(-1), &mut Samples)
            .unwrap();
        reopened
            .catalogue_action(CatalogueAction::Focus(0), &mut Samples)
            .unwrap();
        assert_eq!(
            reopened.entry(0x192770, target, EntryAction::LoadSelection, &mut Samples),
            Err(status::INVALID_PARAMETER)
        );
        let EntryResult::Completed(loaded) = reopened
            .entry(
                0x192770,
                Target::Profile { player: 2 },
                EntryAction::LoadSelection,
                &mut Samples,
            )
            .unwrap()
        else {
            panic!("catalogue entry must adopt")
        };
        assert_eq!(reopened.route.take_completion(), Some(loaded));
        assert!(reopened.route.take_completion().is_none());
        assert_eq!(reopened.lifecycle.player(2).unwrap().extension, input);
        assert_eq!(
            &reopened.lifecycle.player(2).unwrap().saved.profile.payload
                [progress_at..progress_at + 4],
            &[0; 4]
        );
        assert!(
            crate::ui_extensions::profile_policy()
                .unwrap()
                .fixed_devices
        );
        reopened.deactivate(2).unwrap();
        reopened.qualify_settings_roundtrip(&mut Samples).unwrap();
        assert!(!reopened.qualify_map_roundtrip(&mut Samples).unwrap());
        assert!(reopened.lifecycle.player(0).is_none());
        // Reactivate the original test player after qualification restored input.
        reopened
            .open_root(
                Root::ProfileTick,
                Target::Profile { player: 2 },
                &mut Samples,
            )
            .unwrap();
        reopened
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap();
        assert!(reopened.catalogue_view().is_none());
        assert_eq!(
            reopened.caller.players[2].as_ref().unwrap().extension,
            input
        );
        assert_eq!(reopened.lifecycle.player(2).unwrap().extension, input);
        let document = MapDocument::from_chunks(vec![Chunk {
            tag: SETTINGS,
            data: vec![0; 0x3dc],
        }])
        .unwrap();
        reopened.lifecycle.edit(document.clone());
        reopened
            .entry(
                0x32aee0,
                Target::Map,
                EntryAction::SaveEditor {
                    new_save: Some(("map".into(), "Synthetic map".into())),
                },
                &mut Samples,
            )
            .unwrap();
        assert_eq!(
            reopened
                .caller
                .editor
                .as_ref()
                .unwrap()
                .serialize()
                .unwrap(),
            document.serialize().unwrap()
        );
        assert!(!reopened.lifecycle.editor_dirty());
        assert_eq!(reopened.lifecycle.context(), Context::Editor);
        reopened
            .entry(0x32b6a0, Target::Map, EntryAction::Open, &mut Samples)
            .unwrap();
        assert_eq!(reopened.route.entries().len(), 1);
        let EntryResult::Completed(map_loaded) = reopened
            .entry(
                0x32b6a0,
                Target::Map,
                EntryAction::LoadSelection,
                &mut Samples,
            )
            .unwrap()
        else {
            panic!("map catalogue must adopt the document")
        };
        assert_eq!(map_loaded.target, Target::Map);
        assert!(reopened.qualify_map_roundtrip(&mut Samples).unwrap());
        assert_eq!(
            reopened.lifecycle.editor().unwrap().serialize().unwrap(),
            document.serialize().unwrap()
        );

        let directory = root.join("UDATA/45410066/shell");
        std::fs::rename(&directory, root.join("UDATA/45410066/shell-retained")).unwrap();
        std::fs::write(directory, b"synthetic obstacle").unwrap();
        let mut failed = input.clone();
        failed.apply(SettingEdit::SensitivityYaw(0.2)).unwrap();
        crate::ui_extensions::stage_profile_edit(failed);
        reopened.settings_tick(&mut Samples);
        assert_eq!(reopened.lifecycle.player(2).unwrap().extension, input);
        assert_eq!(
            reopened.lifecycle.context(),
            Context::Storage(Target::Profile { player: 2 })
        );
        assert_eq!(crate::ui_extensions::snapshot().0, Some(input.clone()));
        assert!(crate::ui_extensions::snapshot().1.is_some());
        assert!(reopened.route.error().is_some());
        assert!(reopened.route.load_selected().is_err());
        assert!(
            reopened
                .command(Command::Open(Target::Map), &mut Samples)
                .is_err()
        );
        assert!(reopened.catalogue_view().unwrap().error.is_some());
        assert_eq!(
            reopened.caller.players[2].as_ref().unwrap().extension,
            input
        );
        reopened
            .catalogue_action(CatalogueAction::Back, &mut Samples)
            .unwrap();
        assert!(reopened.catalogue_view().is_none());
        // Deactivation while a failed catalogue is open must invalidate its
        // view/error/rows as well as the active record. Reopening afterwards
        // must be admitted without retaining the dead player's context.
        reopened
            .open_root(
                Root::ProfileTick,
                Target::Profile { player: 2 },
                &mut Samples,
            )
            .unwrap();
        assert!(reopened.catalogue_view().is_some());
        reopened.deactivate(2).unwrap();
        assert!(reopened.caller.players[2].is_none());
        assert_eq!(
            reopened.caller.last_completion.as_ref().unwrap().target,
            Target::Map,
            "deactivation must preserve the editor's most recent receipt"
        );
        assert!(reopened.catalogue_view().is_none());
        assert!(reopened.route.entries().is_empty());
        assert!(reopened.route.error().is_none());
        assert!(reopened.route.take_completion().is_none());
        assert!(matches!(
            reopened.frontend.screen(),
            tsfp_engine_fileio::storage_frontend::Screen::Closed
        ));
        reopened
            .open_root(
                Root::ProfileTick,
                Target::Profile { player: 0 },
                &mut Samples,
            )
            .unwrap();
        assert_eq!(
            reopened.catalogue_view().unwrap().target,
            Target::Profile { player: 0 }
        );
        assert_eq!(reopened.deactivate(4), Err(status::INVALID_PARAMETER));
        assert_eq!(
            reopened.catalogue_view().unwrap().target,
            Target::Profile { player: 0 }
        );
        assert!(crate::ui_extensions::take_profile_edit().is_none());
        use crate::storage_caller::ProfileSelectionReturn;
        reopened
            .catalogue_action(CatalogueAction::Back, &mut Samples)
            .unwrap();
        for roster in [vec![], vec![0, 0], vec![4]] {
            assert_eq!(
                reopened.begin_profile_selection(&roster, &mut Samples),
                Err(status::INVALID_PARAMETER)
            );
            assert!(!reopened.caller.selection.active());
            assert!(reopened.catalogue_view().is_none());
        }
        // Use independent durable records for the two participating players.
        reopened.progress_defaults = session.progress_defaults.clone();
        reopened.profile_initializer = Some(
            tsfp_engine_fileio::storage_lifecycle::ProfileInitializer::new(
                [(41, -3), (22, 5), (80, 1), (9, -1)],
                2,
                0.5,
            )
            .unwrap(),
        );
        let mut selection_defaults = tsfp_format_profile::Profile {
            payload: vec![0; 0x1f54],
            signature: [0; 20],
        };
        selection_defaults.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
        reopened
            .open_root(
                Root::ProfileTick,
                Target::Profile { player: 1 },
                &mut Samples,
            )
            .unwrap();
        reopened
            .begin_profile_name("selection-other".into(), selection_defaults.clone())
            .unwrap();
        assert_eq!(
            reopened.catalogue_action(CatalogueAction::Accept, &mut Samples),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(
            reopened.submit_profile_name(&mut Samples),
            Err(status::INVALID_PARAMETER)
        );
        let name = reopened.caller.profile_name.as_mut().unwrap();
        name.append("Synthetic second😀").unwrap();
        name.backspace();
        assert_eq!(name.text(), "Synthetic second");
        assert_eq!(name.append("\0"), Err(status::INVALID_PARAMETER));
        assert_eq!(
            name.append(&"x".repeat(140)),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(name.text(), "Synthetic second");
        let defaults = reopened.progress_defaults.take();
        assert_eq!(
            reopened.submit_profile_name(&mut Samples),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(
            reopened.caller.profile_name.as_ref().unwrap().text(),
            "Synthetic second"
        );
        assert_eq!(
            reopened
                .profile_initializer
                .as_ref()
                .unwrap()
                .next_ring_slot(),
            2,
            "failed creation retains the owned constructor cursor"
        );
        assert_eq!(
            reopened.catalogue_view().unwrap().error,
            Some(status::INVALID_PARAMETER)
        );
        assert_eq!(
            reopened.caller.profile_name.as_ref().unwrap().error(),
            Some(status::INVALID_PARAMETER)
        );
        reopened
            .caller
            .profile_name
            .as_mut()
            .unwrap()
            .append(" retry")
            .unwrap();
        assert!(
            reopened
                .caller
                .profile_name
                .as_ref()
                .unwrap()
                .error()
                .is_none()
        );
        reopened.progress_defaults = defaults;
        assert!(
            reopened
                .catalogue_action(CatalogueAction::Accept, &mut Samples)
                .unwrap()
                .is_some()
        );
        assert!(reopened.caller.profile_name.is_none());
        assert_eq!(
            reopened
                .profile_initializer
                .as_ref()
                .unwrap()
                .next_ring_slot(),
            3
        );
        let settings = reopened.caller.players[1]
            .as_ref()
            .unwrap()
            .saved
            .profile
            .settings()
            .unwrap();
        assert_eq!(settings.turn_speed(), 0.5);
        assert_eq!(&settings.head()[0x60..0x64], &80u32.to_le_bytes());
        let second_key = reopened.caller.players[1]
            .as_ref()
            .unwrap()
            .saved
            .key
            .clone();
        reopened
            .begin_profile_selection(&[0, 1], &mut Samples)
            .unwrap();
        let first = reopened
            .route
            .entries()
            .iter()
            .position(|r| r.label.is_ok() && r.key != second_key)
            .unwrap();
        let first_key = reopened.route.entries()[first].key.clone();
        reopened
            .catalogue_action(CatalogueAction::Focus(first), &mut Samples)
            .unwrap();
        reopened
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap()
            .unwrap();
        assert!(reopened.caller.selection.take_return().is_none());
        assert_eq!(
            reopened.catalogue_view().unwrap().target,
            Target::Profile { player: 1 }
        );
        let duplicate = reopened
            .route
            .entries()
            .iter()
            .position(|r| r.key == first_key)
            .unwrap();
        reopened
            .catalogue_action(CatalogueAction::Focus(duplicate), &mut Samples)
            .unwrap();
        assert_eq!(
            reopened.catalogue_action(CatalogueAction::Accept, &mut Samples),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(reopened.caller.selection.next_player(), Some(1));
        assert!(reopened.caller.selection.take_return().is_none());
        // Cancelling just the child name editor returns to this participant's
        // catalogue. No save directory, adoption or group return is produced.
        let rows_before = reopened.route.entries().to_vec();
        let candidate = reopened.caller.players[0]
            .as_ref()
            .unwrap()
            .saved
            .profile
            .clone();
        let cursor_before = reopened
            .profile_initializer
            .as_ref()
            .unwrap()
            .next_ring_slot();
        reopened
            .begin_profile_name("selection-other".into(), candidate.clone())
            .unwrap();
        reopened
            .caller
            .profile_name
            .as_mut()
            .unwrap()
            .append("Existing directory")
            .unwrap();
        assert_eq!(
            reopened.submit_profile_name(&mut Samples),
            Err(status::OBJECT_NAME_COLLISION)
        );
        assert_eq!(
            reopened
                .profile_initializer
                .as_ref()
                .unwrap()
                .next_ring_slot(),
            cursor_before
        );
        assert_eq!(reopened.caller.selection.next_player(), Some(1));
        assert!(reopened.caller.selection.take_return().is_none());
        reopened
            .catalogue_action(CatalogueAction::Back, &mut Samples)
            .unwrap();
        reopened
            .begin_profile_name("cancelled-native-name".into(), candidate)
            .unwrap();
        reopened
            .caller
            .profile_name
            .as_mut()
            .unwrap()
            .append("Discarded text")
            .unwrap();
        assert!(
            reopened
                .catalogue_action(CatalogueAction::Back, &mut Samples)
                .unwrap()
                .is_none()
        );
        assert!(reopened.caller.profile_name.is_none());
        assert_eq!(reopened.route.entries(), rows_before.as_slice());
        assert_eq!(reopened.route.cursor(), Some(duplicate));
        assert_eq!(
            reopened.lifecycle.context(),
            Context::Storage(Target::Profile { player: 1 })
        );
        assert_eq!(reopened.caller.selection.next_player(), Some(1));
        assert!(reopened.caller.selection.take_return().is_none());
        assert_eq!(
            reopened.caller.players[0].as_ref().unwrap().saved.key,
            first_key
        );
        assert!(!root.join("UDATA/45410066/cancelled-native-name").exists());
        let second = reopened
            .route
            .entries()
            .iter()
            .position(|r| r.key == second_key)
            .unwrap();
        reopened
            .catalogue_action(CatalogueAction::Focus(second), &mut Samples)
            .unwrap();
        reopened
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap()
            .unwrap();
        assert_eq!(
            reopened.begin_profile_selection(&[2], &mut Samples),
            Err(status::INVALID_PARAMETER),
            "a caller cannot overwrite an unconsumed return"
        );
        let Some(ProfileSelectionReturn::Selected(keys)) = reopened.caller.selection.take_return()
        else {
            panic!("all adopted participants must return")
        };
        assert_eq!(
            keys[0].as_ref(),
            Some(&reopened.caller.players[0].as_ref().unwrap().saved.key)
        );
        assert_eq!(keys[1].as_ref(), Some(&second_key));
        assert!(keys[2..].iter().all(Option::is_none));
        assert!(reopened.caller.selection.take_return().is_none());
        assert!(reopened.catalogue_view().is_none());
        reopened
            .begin_profile_selection(&[0, 1], &mut Samples)
            .unwrap();
        let first = reopened
            .route
            .entries()
            .iter()
            .position(|r| r.key == first_key)
            .unwrap();
        reopened
            .catalogue_action(CatalogueAction::Focus(first), &mut Samples)
            .unwrap();
        reopened
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap();
        assert_eq!(
            reopened.catalogue_view().unwrap().target,
            Target::Profile { player: 1 }
        );
        reopened.deactivate(0).unwrap();
        assert_eq!(
            reopened.caller.selection.take_return(),
            Some(ProfileSelectionReturn::Cancelled)
        );
        assert!(reopened.catalogue_view().is_none());
        use crate::storage_caller::{NativePageId, RetainedPage};
        let retained = vec![
            RetainedPage {
                id: NativePageId(1),
                cursor: 4,
                scroll: 12.5,
            },
            RetainedPage {
                id: NativePageId(2),
                cursor: 1,
                scroll: -3.0,
            },
            RetainedPage {
                id: NativePageId(3),
                cursor: 0,
                scroll: 0.0,
            },
        ];
        assert_eq!(
            reopened.begin_profile_selection_from(
                &[2, 2],
                retained.clone(),
                NativePageId(2),
                NativePageId(1),
                &mut Samples
            ),
            Err(status::INVALID_PARAMETER)
        );
        assert!(!reopened.caller.navigation.awaiting_return());
        assert!(!reopened.caller.selection.active());
        use crate::storage_caller::{Participants, PlayerInputSource};
        let participants = Participants::new(vec![
            PlayerInputSource::KeyboardMouse,
            PlayerInputSource::Controller(17),
        ])
        .unwrap();
        reopened
            .begin_players_launch(
                participants.clone(),
                retained.clone(),
                NativePageId(2),
                NativePageId(1),
                &mut Samples,
            )
            .unwrap();
        assert_eq!(
            reopened.caller.players_launch.pending(),
            Some(&participants)
        );
        assert!(reopened.caller.players_launch.active().is_none());
        assert_eq!(
            reopened.caller.dispatch_players_input(
                NativePageId(2),
                PlayerInputSource::KeyboardMouse,
                |_, _, _| panic!("pending selection cannot dispatch destination input")
            ),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(
            reopened.begin_players_launch(
                participants.clone(),
                retained.clone(),
                NativePageId(2),
                NativePageId(1),
                &mut Samples
            ),
            Err(status::INVALID_PARAMETER)
        );
        reopened
            .catalogue_action(CatalogueAction::Back, &mut Samples)
            .unwrap();
        struct NoReturnRuntime;
        impl tsfp_engine_fileio::storage_bootstrap::RuntimeRead for NoReturnRuntime {
            fn word(&self, _: u32) -> Option<u32> {
                panic!("cancelled navigation must not read progression policy")
            }
        }
        assert_eq!(
            reopened.service_profile_navigation(&NoReturnRuntime),
            Some(ProfileSelectionReturn::Cancelled)
        );
        assert!(!reopened.caller.selection.active());
        assert!(reopened.catalogue_view().is_none());
        assert!(reopened.caller.players_launch.pending().is_none());
        assert!(reopened.caller.players_launch.active().is_none());
        assert_eq!(reopened.caller.navigation.active(), Some(&retained[0]));
        assert!(reopened.caller.navigation.history().is_empty());
        assert!(reopened.take_profile_return().is_none());
        // The real owned catalogue Create action is serviced at a frame boundary,
        // importing caller-selected retained input and generating its own identity.
        reopened
            .begin_profile_selection_from(
                &[2],
                retained.clone(),
                NativePageId(2),
                NativePageId(1),
                &mut Samples,
            )
            .unwrap();
        let create = reopened.route.entries().len();
        reopened
            .catalogue_action(CatalogueAction::Focus(create), &mut Samples)
            .unwrap();
        assert_eq!(reopened.catalogue_view().unwrap().cursor, Some(create));
        reopened
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap();
        struct RetainedRuntime(tsfp_format_profile::Profile);
        impl tsfp_engine_fileio::storage_bootstrap::RuntimeRead for RetainedRuntime {
            fn word(&self, address: u32) -> Option<u32> {
                if address == 0x79094c {
                    return Some(0x65);
                }
                if address == 0x7de458 {
                    return Some(0);
                }
                let at = address.checked_sub(0x7bedc0 + 2 * 0x1f54)? as usize;
                Some(u32::from_le_bytes(
                    self.0.payload.get(at..at + 4)?.try_into().ok()?,
                ))
            }
        }
        let mut runtime = RetainedRuntime(selection_defaults.clone());
        runtime.0.payload.truncate(0x1ef0);
        reopened.service_profile_creation(&runtime);
        assert!(reopened.caller.profile_name.is_none());
        assert_eq!(
            reopened.catalogue_view().unwrap().error,
            Some(status::INVALID_PARAMETER)
        );
        runtime.0.payload = selection_defaults.payload.clone();
        runtime.0.payload[0x1ee0..0x1ee4].fill(0);
        reopened
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap();
        reopened.service_profile_creation(&runtime);
        assert!(reopened.caller.profile_name.is_some());
        assert_eq!(reopened.catalogue_view().unwrap().error, None);
        reopened
            .caller
            .profile_name
            .as_mut()
            .unwrap()
            .replace("Native generated identity".into())
            .unwrap();
        let created = reopened
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap()
            .unwrap();
        assert!(created.key.as_ref().unwrap().path().contains("profile-"));
        assert!(
            !created
                .key
                .as_ref()
                .unwrap()
                .path()
                .contains("Native generated")
        );
        assert!(
            reopened.caller.players[2]
                .as_ref()
                .unwrap()
                .saved
                .profile
                .payload[0x1290..0x1edc]
                .iter()
                .all(|b| *b == 0)
        );
        assert!(
            reopened.take_profile_return().is_none(),
            "return waits for owned progression preparation"
        );
        assert!(reopened.caller.navigation.active().is_none());
        assert_eq!(reopened.caller.navigation.history(), retained);
        struct MissingPolicy<'a>(&'a RetainedRuntime);
        impl tsfp_engine_fileio::storage_bootstrap::RuntimeRead for MissingPolicy<'_> {
            fn word(&self, address: u32) -> Option<u32> {
                if address == 0x79094c {
                    None
                } else {
                    self.0.word(address)
                }
            }
        }
        assert!(
            reopened.catalogue_view().is_none(),
            "adoption has already released catalogue input"
        );
        assert!(
            reopened
                .service_profile_navigation(&MissingPolicy(&runtime))
                .is_none()
        );
        assert_eq!(
            reopened.caller.return_error,
            Some(status::INVALID_PARAMETER)
        );
        assert!(reopened.caller.returned_progress.is_none());
        assert!(reopened.caller.returned_availability.is_none());
        assert!(reopened.take_profile_return().is_none());
        let waiting = reopened.profile_return_view().unwrap();
        assert!(waiting.rows.is_empty());
        assert_eq!(waiting.error, Some(status::INVALID_PARAMETER));
        assert_eq!(
            reopened.catalogue_action(CatalogueAction::Focus(1), &mut Samples),
            Err(status::INVALID_PARAMETER)
        );
        reopened
            .catalogue_action(CatalogueAction::Accept, &mut Samples)
            .unwrap();
        assert_eq!(
            reopened.service_profile_navigation(&NoReturnRuntime),
            Some(ProfileSelectionReturn::Cancelled)
        );
        assert_eq!(reopened.caller.navigation.active(), Some(&retained[0]));
        assert!(reopened.profile_return_view().is_none());
        assert!(
            reopened.caller.players[2].is_some(),
            "cancellation must preserve authenticated adoption"
        );
        reopened
            .begin_profile_selection_from(
                &[2],
                retained.clone(),
                NativePageId(2),
                NativePageId(1),
                &mut Samples,
            )
            .unwrap();
        reopened
            .command(
                Command::LoadProfile {
                    player: 2,
                    key: created.key.clone().unwrap(),
                },
                &mut Samples,
            )
            .unwrap();
        assert!(matches!(
            reopened.service_profile_navigation(&runtime),
            Some(ProfileSelectionReturn::Selected(_))
        ));
        assert!(reopened.caller.returned_progress.is_some());
        assert_eq!(reopened.caller.returned_availability, Some(0));
        assert!(
            reopened.service_profile_navigation(&runtime).is_none(),
            "later frames must not activate the page twice"
        );
        assert_eq!(reopened.caller.navigation.active(), Some(&retained[1]));
        assert_eq!(reopened.caller.navigation.history(), &retained[..1]);
        assert!(!reopened.caller.navigation.awaiting_return());
        assert!(reopened.take_profile_return().is_none());
        // The production destination qualifier uses authenticated record identity,
        // not merely populated slots. Build a completed owned single-player return
        // from records already loaded through the real filesystem above.
        reopened.caller.players[0] = reopened.caller.players[2].clone();
        let bound_key = reopened.caller.players[0]
            .as_ref()
            .unwrap()
            .saved
            .key
            .clone();
        reopened
            .caller
            .players_launch
            .begin(Participants::new(vec![PlayerInputSource::KeyboardMouse]).unwrap())
            .unwrap();
        reopened
            .caller
            .players_launch
            .complete(&ProfileSelectionReturn::Selected([
                Some(bound_key),
                None,
                None,
                None,
            ]));
        assert!(reopened.caller.players_destination().is_some());
        let mut dispatched = 0;
        reopened
            .caller
            .dispatch_players_input(
                NativePageId(2),
                PlayerInputSource::KeyboardMouse,
                |page, player, profile| {
                    dispatched += 1;
                    assert_eq!(page, &retained[1]);
                    assert_eq!(player, 0);
                    assert_eq!(
                        profile.saved.key,
                        reopened.caller.players[0].as_ref().unwrap().saved.key
                    );
                    assert_eq!(
                        profile.extension,
                        reopened.caller.players[0].as_ref().unwrap().extension
                    );
                },
            )
            .unwrap();
        assert_eq!(dispatched, 1);
        for (page, source) in [
            (NativePageId(1), PlayerInputSource::KeyboardMouse),
            (NativePageId(2), PlayerInputSource::Controller(17)),
        ] {
            assert_eq!(
                reopened
                    .caller
                    .dispatch_players_input(page, source, |_, _, _| panic!(
                        "rejected destination input must not invoke handler"
                    )),
                Err(status::INVALID_PARAMETER)
            );
        }
        use crate::storage_caller::{DestinationAction, DestinationRequestAction, DestinationRow};
        let page = NativePageId(2);
        let source = PlayerInputSource::KeyboardMouse;
        let rows = vec![
            DestinationRow {
                id: 10,
                label: "runtime-a".into(),
                enabled: true,
            },
            DestinationRow {
                id: 20,
                label: "runtime-disabled".into(),
                enabled: false,
            },
            DestinationRow {
                id: 30,
                label: "runtime-b".into(),
                enabled: true,
            },
        ];
        reopened
            .caller
            .install_destination(page, rows.clone())
            .unwrap();
        assert_eq!(reopened.caller.destination_view().unwrap().cursor, Some(0));
        reopened
            .caller
            .destination_action(page, source, DestinationAction::Step(i32::MAX))
            .unwrap();
        assert_eq!(reopened.caller.navigation.active().unwrap().cursor, 2);
        let request = reopened
            .caller
            .destination_action(page, source, DestinationAction::Accept)
            .unwrap()
            .unwrap();
        assert_eq!(request.action, DestinationRequestAction::Activate(30));
        assert_eq!(request.player, 0);
        assert_eq!(
            request.profile,
            reopened.caller.players[0].as_ref().unwrap().saved.key
        );
        let before = reopened.caller.destination_view().unwrap();
        for action in [DestinationAction::Focus(20), DestinationAction::Focus(999)] {
            assert_eq!(
                reopened.caller.destination_action(page, source, action),
                Err(status::INVALID_PARAMETER)
            );
            assert_eq!(reopened.caller.destination_view().unwrap(), before);
        }
        assert_eq!(
            reopened.caller.destination_action(
                page,
                PlayerInputSource::Controller(99),
                DestinationAction::Step(-1)
            ),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(reopened.caller.destination_view().unwrap(), before);
        // Rendered pointer dispatch rejects stale geometry and unassigned sources
        // before focus mutation, and does not accept when the pointer misses.
        let rendered = before.clone();
        assert_eq!(
            reopened
                .caller
                .destination_pointer(&rendered, source, None, true, false)
                .unwrap(),
            None
        );
        assert_eq!(reopened.caller.destination_view().unwrap(), before);
        assert_eq!(
            reopened.caller.destination_pointer(
                &rendered,
                PlayerInputSource::Controller(99),
                Some(10),
                true,
                false
            ),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(reopened.caller.destination_view().unwrap(), before);
        let mut stale = rendered.clone();
        stale.rows.swap(0, 2);
        assert_eq!(
            reopened
                .caller
                .destination_pointer(&stale, source, Some(10), true, false),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(reopened.caller.destination_view().unwrap(), before);
        assert_eq!(
            reopened
                .caller
                .destination_pointer(&rendered, source, Some(20), true, false),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(reopened.caller.destination_view().unwrap(), before);
        assert_eq!(
            reopened
                .caller
                .destination_pointer(&rendered, source, Some(10), true, false)
                .unwrap()
                .unwrap()
                .action,
            DestinationRequestAction::Activate(10)
        );
        // Cursor-only changes preserve geometry; Back overrides even invalid focus.
        assert_eq!(
            reopened
                .caller
                .destination_pointer(&rendered, source, Some(999), true, true)
                .unwrap()
                .unwrap()
                .action,
            DestinationRequestAction::Back
        );
        reopened
            .caller
            .destination_action(page, source, DestinationAction::Focus(30))
            .unwrap();
        let mut duplicate = rows.clone();
        duplicate[1].id = 10;
        assert_eq!(
            reopened.caller.install_destination(page, duplicate),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(reopened.caller.destination_view().unwrap(), before);
        reopened
            .caller
            .destination_action(page, source, DestinationAction::Step(i32::MIN))
            .unwrap();
        assert_eq!(reopened.caller.destination_view().unwrap().cursor, Some(0));
        assert_eq!(
            reopened
                .caller
                .destination_action(page, source, DestinationAction::Back)
                .unwrap()
                .unwrap()
                .action,
            DestinationRequestAction::Back
        );
        let mut disabled = rows;
        for row in &mut disabled {
            row.enabled = false;
        }
        reopened.caller.install_destination(page, disabled).unwrap();
        assert_eq!(reopened.caller.destination_view().unwrap().cursor, None);
        assert_eq!(
            reopened
                .caller
                .destination_action(page, source, DestinationAction::Accept),
            Err(status::INVALID_PARAMETER)
        );
        reopened
            .caller
            .destination_action(page, source, DestinationAction::Step(1))
            .unwrap();
        let original = reopened.caller.players[0].clone();
        reopened.caller.players[0] = reopened.caller.players[1].clone();
        assert!(reopened.caller.players[0].is_some());
        assert!(
            reopened.caller.players_destination().is_none(),
            "replacement adoption must suppress stale routing"
        );
        assert!(reopened.caller.destination_view().is_none());
        assert_eq!(
            reopened
                .caller
                .destination_action(page, source, DestinationAction::Back),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(
            reopened.caller.dispatch_players_input(
                NativePageId(2),
                PlayerInputSource::KeyboardMouse,
                |_, _, _| panic!("replaced profile must suppress dispatch")
            ),
            Err(status::INVALID_PARAMETER)
        );
        reopened.caller.players[0] = original;
        assert!(reopened.caller.players_destination().is_some());
        // Window ownership requires a concrete executor, not just drawn rows.
        fn open_map(
            session: &mut StorageSession,
            request: crate::storage_caller::DestinationRequest,
        ) -> Result<(), NtStatus> {
            assert_eq!(
                request.action,
                crate::storage_caller::DestinationRequestAction::Activate(30)
            );
            session.open_root(
                tsfp_engine_fileio::storage_route::Root::MapCatalogue,
                Target::Map,
                &mut Samples,
            )
        }
        let active_rows = vec![DestinationRow {
            id: 30,
            label: "runtime-map".into(),
            enabled: true,
        }];
        reopened
            .caller
            .install_destination(page, active_rows.clone())
            .unwrap();
        assert!(!reopened.destination_input_active());
        assert_eq!(
            reopened.destination_input(page, source, DestinationAction::Accept),
            Err(status::INVALID_PARAMETER)
        );
        reopened
            .bind_destination(page, active_rows.clone(), open_map)
            .unwrap();
        assert!(reopened.destination_input_active());
        assert_eq!(
            reopened.destination_input(
                page,
                PlayerInputSource::Controller(99),
                DestinationAction::Accept
            ),
            Err(status::INVALID_PARAMETER)
        );
        // Replacing presentation contents never inherits old executable bindings.
        reopened
            .caller
            .install_destination(
                page,
                vec![DestinationRow {
                    id: 31,
                    label: "replacement".into(),
                    enabled: true,
                }],
            )
            .unwrap();
        assert!(!reopened.destination_input_active());
        assert_eq!(
            reopened.destination_input(page, source, DestinationAction::Accept),
            Err(status::INVALID_PARAMETER)
        );
        reopened
            .bind_destination(page, active_rows.clone(), open_map)
            .unwrap();
        let rendered = reopened.caller.destination_view().unwrap();
        reopened
            .destination_pointer_input(&rendered, source, None, true, false)
            .unwrap();
        assert!(
            reopened.destination_input_active(),
            "pointer miss cannot execute old focus"
        );
        reopened
            .destination_pointer_input(&rendered, source, Some(30), true, false)
            .unwrap();
        assert_eq!(reopened.catalogue_view().unwrap().target, Target::Map);
        assert!(
            !reopened.destination_input_active(),
            "map catalogue supersedes destination input"
        );
        assert_eq!(
            reopened.destination_input(page, source, DestinationAction::Accept),
            Err(status::INVALID_PARAMETER)
        );
        reopened
            .catalogue_action(CatalogueAction::Back, &mut Samples)
            .unwrap();
        reopened.deactivate(0).unwrap();
        assert!(reopened.caller.players_destination().is_none());
        assert_eq!(
            reopened.caller.dispatch_players_input(
                NativePageId(2),
                PlayerInputSource::KeyboardMouse,
                |_, _, _| panic!("deactivated profile cannot dispatch input")
            ),
            Err(status::INVALID_PARAMETER)
        );
        reopened.deactivate(2).unwrap();
        assert!(reopened.caller.returned_progress.is_none());
        assert!(reopened.caller.returned_availability.is_none());
    }
}

#[test]
fn retained_navigation_rejects_invalid_context_and_restores_owned_page() {
    use storage_caller::{NativePageId, ProfileNavigation, ProfileSelectionReturn, RetainedPage};
    let page = RetainedPage {
        id: NativePageId(7),
        cursor: 9,
        scroll: 6.25,
    };
    for pages in [
        vec![],
        vec![page.clone(); 2],
        vec![page.clone(); 61],
        vec![RetainedPage {
            id: NativePageId(0),
            ..page.clone()
        }],
        vec![RetainedPage {
            scroll: f32::NAN,
            ..page.clone()
        }],
        vec![RetainedPage {
            scroll: f32::INFINITY,
            ..page.clone()
        }],
    ] {
        assert!(ProfileNavigation::new(pages, page.id, page.id).is_err());
    }
    assert!(ProfileNavigation::new(vec![page.clone()], NativePageId(8), page.id).is_err());
    assert!(ProfileNavigation::new(vec![page.clone()], page.id, NativePageId(8)).is_err());
    let mut nav = ProfileNavigation::new(vec![page.clone()], page.id, page.id).unwrap();
    nav.complete(&ProfileSelectionReturn::Cancelled).unwrap();
    assert_eq!(nav.active(), Some(&page));
    assert!(nav.history().is_empty());
    nav.complete(&ProfileSelectionReturn::Selected(Default::default()))
        .unwrap();
    assert_eq!(
        nav.active(),
        Some(&page),
        "replayed result cannot replace active page"
    );
}

#[test]
fn owned_players_assignments_validate_and_publish_only_on_selected_return() {
    use storage_caller::{
        Participants, PlayerInputSource as Input, PlayersLaunch, ProfileSelectionReturn,
    };
    assert!(Participants::new(vec![]).is_err());
    assert!(Participants::new(vec![Input::KeyboardMouse; 2]).is_err());
    assert!(Participants::new(vec![Input::Controller(7); 2]).is_err());
    assert!(Participants::new((0..5).map(Input::Controller).collect()).is_err());
    let first = Participants::new(vec![Input::KeyboardMouse, Input::Controller(7)]).unwrap();
    assert_eq!(first.players(), vec![0, 1]);
    assert_eq!(first.inputs().len(), 2);
    let mut launch = PlayersLaunch::default();
    launch.begin(first.clone()).unwrap();
    assert!(launch.begin(first.clone()).is_err());
    launch.complete(&ProfileSelectionReturn::Selected(Default::default()));
    assert!(launch.active().is_none());
    assert_eq!(launch.pending(), Some(&first));
    let keys = [
        Some(tsfp_engine_fileio::storage_flow::SaveKey::new("p0", "tsftrpft").unwrap()),
        Some(tsfp_engine_fileio::storage_flow::SaveKey::new("p1", "tsftrpft").unwrap()),
        None,
        None,
    ];
    let mut duplicate = keys.clone();
    duplicate[1] = duplicate[0].clone();
    launch.complete(&ProfileSelectionReturn::Selected(duplicate));
    assert!(
        launch.active().is_none(),
        "duplicate authenticated identity cannot satisfy two players"
    );
    assert_eq!(launch.pending(), Some(&first));
    launch.complete(&ProfileSelectionReturn::Selected(keys.clone()));
    assert!(launch.matches_profiles(&keys));
    let mut replaced = keys.clone();
    replaced[0] = keys[1].clone();
    assert!(
        !launch.matches_profiles(&replaced),
        "present but replaced profile is not the selected participant"
    );
    replaced[0] = None;
    assert!(!launch.matches_profiles(&replaced));
    assert_eq!(launch.active(), Some(&first));
    let second = Participants::new(vec![Input::Controller(8)]).unwrap();
    launch.begin(second).unwrap();
    assert!(
        !launch.matches_profiles(&keys),
        "pending relaunch suppresses prior destination"
    );
    launch.complete(&ProfileSelectionReturn::Cancelled);
    assert_eq!(launch.active(), Some(&first));
    assert!(launch.pending().is_none());
    assert!(
        launch.matches_profiles(&keys),
        "cancel preserves the previous authenticated binding"
    );
}

#[test]
fn native_map_name_saves_adopts_reopens_and_preserves_failed_document() {
    use crate::storage_caller::CatalogueAction;
    use tsfp_engine_fileio::{
        local_storage::LocalStorage,
        map_document::{Chunk, MapDocument, SETTINGS},
        storage_flow::{MapPayload, SampleMaps},
        storage_frontend::Target,
        vfs::{NtStatus, status},
    };
    struct NoSamples;
    impl SampleMaps for NoSamples {
        fn load_sample(&mut self, _: u32) -> Result<MapPayload, NtStatus> {
            Err(status::OBJECT_NAME_NOT_FOUND)
        }
    }
    let root = std::env::temp_dir().join(format!(
        "t2319-map-name-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = LocalStorage::open(root.clone()).unwrap();
    let mut session = crate::storage_session::StorageSession::new(&storage, [29; 16]).unwrap();
    assert_eq!(
        session.begin_map_name("new-map".into()),
        Err(status::INVALID_PARAMETER)
    );
    let mut settings = vec![0x5a; 0x3dc];
    // FABRICATED unknown-byte sentinel with valid format counts, not 90 slots.
    settings[4..8].fill(0);
    settings[0x16] = 10;
    settings[0x42] = 6;
    settings[0x62..0x82].fill(0);
    let document = MapDocument::from_chunks(vec![
        Chunk {
            tag: SETTINGS,
            data: settings,
        },
        Chunk {
            tag: 0xC77672,
            data: (0..64u32).flat_map(u32::to_le_bytes).collect(),
        },
        Chunk {
            tag: 0xC77671,
            data: vec![0; 0x10],
        },
        Chunk {
            tag: 0x10001,
            data: vec![7, 8, 9],
        },
    ])
    .unwrap();
    let generated = document.serialize().unwrap();
    let generated_bytes = generated.bytes();
    let used = u32::from_le_bytes(generated_bytes[0x258..0x25c].try_into().unwrap()) as usize;
    assert_eq!(
        &generated_bytes[..8],
        &[0x67, 0x76, 0xc7, 0x00, 0x2d, 0x01, 0x00, 0x00]
    );
    assert!(used + 4 <= generated_bytes.len());
    assert_eq!(
        u32::from_le_bytes(generated_bytes[used - 8..used - 4].try_into().unwrap()),
        0xC7766A
    );
    assert_eq!(
        u32::from_le_bytes(generated_bytes[used - 4..used].try_into().unwrap()) as usize,
        used - 8
    );
    assert_eq!(
        MapDocument::parse_local(&generated)
            .unwrap()
            .serialize()
            .unwrap()
            .bytes(),
        generated.bytes()
    );
    session.lifecycle.edit(document.clone());
    assert_eq!(
        session.begin_map_name("../escape".into()),
        Err(status::INVALID_PARAMETER)
    );
    assert!(session.catalogue_view().is_none());
    use crate::editor_navigation::{EditorOperation, EditorReturn};
    use crate::storage_caller::{NativePageId, RetainedPage};
    let history = vec![
        RetainedPage {
            id: NativePageId(51),
            cursor: 3,
            scroll: 4.0,
        },
        RetainedPage {
            id: NativePageId(52),
            cursor: 1,
            scroll: -2.0,
        },
    ];
    session
        .begin_editor_save_as_from(
            "new-map".into(),
            history.clone(),
            NativePageId(52),
            NativePageId(51),
        )
        .unwrap();
    assert_eq!(
        session.caller.append_name(&"x".repeat(32)),
        Err(status::INVALID_PARAMETER)
    );
    assert_eq!(session.caller.name_text(), Some(""));
    session.caller.append_name("Éditor map").unwrap();
    let receipt = session
        .catalogue_action(CatalogueAction::Accept, &mut NoSamples)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.target, Target::Map);
    assert!(session.caller.map_name.is_none());
    assert_eq!(session.editor_returned_page(), Some(&history[1]));
    assert_eq!(
        session.begin_map_name("unconsumed-return".into()),
        Err(status::INVALID_PARAMETER)
    );
    let Some(EditorReturn::Completed {
        destination,
        operation,
        source,
        document: returned,
    }) = session.take_editor_return()
    else {
        panic!("save must return adopted document");
    };
    assert_eq!(destination, history[1]);
    assert_eq!(operation, EditorOperation::Save);
    assert_eq!(
        source,
        tsfp_engine_fileio::storage_flow::MapSource::Local(receipt.key.clone().unwrap())
    );
    assert_eq!(returned.title_bytes(), "Éditor map".as_bytes());
    assert!(session.take_editor_return().is_none());
    assert!(!session.lifecycle.editor_dirty());
    let saved = session.lifecycle.editor().unwrap().clone();
    assert_eq!(saved.title_bytes(), "Éditor map".as_bytes());
    assert_eq!(saved.chunks()[1], document.chunks()[1]);
    assert_eq!(
        &saved.chunks()[0].data[..0x62],
        &document.chunks()[0].data[..0x62]
    );
    assert_eq!(
        &saved.chunks()[0].data[0x82..0x248],
        &document.chunks()[0].data[0x82..0x248]
    );
    assert_eq!(
        &saved.chunks()[0].data[0x24c..],
        &document.chunks()[0].data[0x24c..]
    );
    let canonical = MapDocument::parse(&document.serialize().unwrap()).unwrap();
    assert_eq!(
        &saved.chunks()[0].data[0x248..0x24c],
        &canonical.chunks()[0].data[0x248..0x24c]
    );
    // Same-directory allocation fails rather than overwrite a durable map. The
    // candidate title and bytes publish only after successful authenticated I/O.
    session.lifecycle.edit(document.clone());
    session
        .begin_editor_save_as_from(
            "new-map".into(),
            history.clone(),
            NativePageId(52),
            NativePageId(51),
        )
        .unwrap();
    session.caller.append_name("Rejected title").unwrap();
    assert_eq!(
        session.command(
            tsfp_engine_fileio::storage_frontend::Command::LoadMap {
                source: tsfp_engine_fileio::storage_flow::MapSource::Local(
                    receipt.key.clone().unwrap()
                )
            },
            &mut NoSamples
        ),
        Err(status::INVALID_PARAMETER)
    );
    assert!(session.take_editor_return().is_none());
    assert!(
        session
            .catalogue_action(CatalogueAction::Accept, &mut NoSamples)
            .is_err()
    );
    assert_eq!(session.lifecycle.editor(), Some(&document));
    assert!(session.lifecycle.editor_dirty());
    assert_eq!(session.caller.name_text(), Some("Rejected title"));
    assert!(session.catalogue_view().unwrap().error.is_some());
    session
        .catalogue_action(CatalogueAction::Back, &mut NoSamples)
        .unwrap();
    assert!(session.catalogue_view().is_none());
    assert_eq!(
        session.take_editor_return(),
        Some(EditorReturn::Cancelled {
            destination: history[0].clone()
        })
    );
    assert!(session.take_editor_return().is_none());
    assert_eq!(session.lifecycle.editor(), Some(&document));
    // Reopen through the actual signed catalogue, not the candidate in memory.
    let mut reopened = crate::storage_session::StorageSession::new(&storage, [29; 16]).unwrap();
    reopened
        .begin_editor_load_from(
            history.clone(),
            NativePageId(52),
            NativePageId(51),
            &mut NoSamples,
        )
        .unwrap();
    assert_eq!(
        reopened.begin_map_name("wrong-operation".into()),
        Err(status::INVALID_PARAMETER)
    );
    assert_eq!(
        reopened.save_editor(None, &mut NoSamples),
        Err(status::INVALID_PARAMETER)
    );
    assert_eq!(
        reopened.catalogue_view().unwrap().rows[0].label.as_deref(),
        Ok("Éditor map")
    );
    reopened
        .catalogue_action(CatalogueAction::Accept, &mut NoSamples)
        .unwrap();
    let loaded = reopened.lifecycle.editor().unwrap().clone();
    assert_eq!(loaded.title_bytes(), saved.title_bytes());
    assert_eq!(
        loaded.serialize().unwrap().bytes(),
        saved.serialize().unwrap().bytes()
    );
    assert!(reopened.has_editor_return());
    assert!(!reopened.consume_editor_return_for(NativePageId(999)));
    assert!(reopened.consume_editor_return_for(history[1].id));
    assert!(!reopened.has_editor_return());
    assert!(reopened.take_editor_return().is_none());
    let update = reopened.save_editor(None, &mut NoSamples).unwrap();
    assert_eq!(
        update.operation,
        tsfp_engine_fileio::storage_flow::Operation::MapSave
    );
    assert!(reopened.catalogue_view().is_none());
    // Broken catalogue retains a native cancellation return and the prior editor.
    let title = root.join("UDATA/45410066");
    let backup = root.join("UDATA/test-title-backup");
    std::fs::rename(&title, &backup).unwrap();
    std::fs::write(&title, b"synthetic blocker").unwrap();
    assert!(
        reopened
            .begin_editor_load_from(
                history.clone(),
                NativePageId(52),
                NativePageId(51),
                &mut NoSamples
            )
            .is_err()
    );
    assert!(reopened.catalogue_view().unwrap().error.is_some());
    assert!(reopened.take_editor_return().is_none());
    reopened
        .catalogue_action(CatalogueAction::Back, &mut NoSamples)
        .unwrap();
    assert_eq!(
        reopened.take_editor_return(),
        Some(EditorReturn::Cancelled {
            destination: history[0].clone()
        })
    );
    assert_eq!(reopened.lifecycle.editor(), Some(&returned));
    std::fs::remove_file(&title).unwrap();
    std::fs::rename(&backup, &title).unwrap();
    // Disc samples remain a distinct read-only data source, not another device.
    struct Sample(tsfp_engine_fileio::storage_flow::MapPayload);
    impl SampleMaps for Sample {
        fn load_sample(&mut self, id: u32) -> Result<MapPayload, NtStatus> {
            assert_eq!(id, 77); // FABRICATED sample id, never a storage device id.
            Ok(self.0.clone())
        }

        fn catalogue(
            &mut self,
        ) -> Result<Vec<tsfp_engine_fileio::storage_flow::SampleMapEntry>, NtStatus> {
            Ok(vec![tsfp_engine_fileio::storage_flow::SampleMapEntry::new(
                77,
                "Synthetic disc sample".into(),
            )?])
        }
    }
    let local_file = root.join("UDATA/45410066/new-map/anicemap.mkr");
    let local_before = std::fs::read(&local_file).unwrap();
    reopened
        .begin_editor_load_from(
            history.clone(),
            NativePageId(52),
            NativePageId(51),
            &mut NoSamples,
        )
        .unwrap();
    let source = tsfp_engine_fileio::storage_flow::MapSource::DiscSample(77);
    assert_eq!(
        reopened.command(
            tsfp_engine_fileio::storage_frontend::Command::LoadMap {
                source: source.clone()
            },
            &mut NoSamples
        ),
        Err(status::OBJECT_NAME_NOT_FOUND)
    );
    assert!(reopened.take_editor_return().is_none());
    assert_eq!(reopened.lifecycle.editor(), Some(&returned));
    let sample_payload = document
        .with_title("Sample fixture")
        .unwrap()
        .serialize()
        .unwrap();
    let mut sample = Sample(sample_payload.clone());
    let mut choice_session =
        crate::storage_session::StorageSession::new(&storage, [29; 16]).unwrap();
    choice_session
        .begin_editor_load_choice_from(
            history.clone(),
            NativePageId(52),
            NativePageId(51),
            &mut sample,
        )
        .unwrap();
    assert!(choice_session.catalogue_view().unwrap().map_source_choice);
    choice_session
        .catalogue_action(CatalogueAction::Focus(0), &mut sample)
        .unwrap();
    choice_session
        .catalogue_action(CatalogueAction::Accept, &mut sample)
        .unwrap();
    assert_eq!(
        choice_session.catalogue_view().unwrap().sample_maps[0].id,
        77
    );
    choice_session
        .catalogue_action(CatalogueAction::Focus(0), &mut sample)
        .unwrap();
    choice_session
        .catalogue_action(CatalogueAction::Accept, &mut sample)
        .unwrap();
    let Some(EditorReturn::Completed {
        source: choice_source,
        ..
    }) = choice_session.take_editor_return()
    else {
        panic!("sample source choice must complete through the native handler");
    };
    assert_eq!(choice_source, source);
    reopened
        .open_root(
            tsfp_engine_fileio::storage_route::Root::MapCatalogue,
            Target::Map,
            &mut sample,
        )
        .unwrap();
    let view = reopened.catalogue_view().unwrap();
    assert_eq!(view.sample_maps[0].id, 77);
    assert_eq!(view.sample_maps[0].label, "Synthetic disc sample");
    let sample_row = view.rows.len();
    reopened
        .catalogue_action(CatalogueAction::Focus(sample_row), &mut sample)
        .unwrap();
    let receipt = reopened
        .catalogue_action(CatalogueAction::Accept, &mut sample)
        .unwrap()
        .unwrap();
    assert!(receipt.key.is_none());
    let Some(EditorReturn::Completed {
        operation,
        source: loaded_source,
        document: loaded,
        ..
    }) = reopened.take_editor_return()
    else {
        panic!("sample load must return");
    };
    assert_eq!(operation, EditorOperation::Load);
    assert_eq!(loaded_source, source);
    assert_eq!(loaded.title_bytes(), b"Sample fixture");
    assert_eq!(std::fs::read(&local_file).unwrap(), local_before);
    reopened
        .begin_editor_save_as_from(
            "sample-copy".into(),
            history,
            NativePageId(52),
            NativePageId(51),
        )
        .unwrap();
    reopened.caller.append_name("Local sample copy").unwrap();
    reopened
        .catalogue_action(CatalogueAction::Accept, &mut sample)
        .unwrap();
    let Some(EditorReturn::Completed {
        operation,
        source,
        document: copied,
        ..
    }) = reopened.take_editor_return()
    else {
        panic!("sample copy save must return");
    };
    assert_eq!(operation, EditorOperation::Save);
    assert!(matches!(
        source,
        tsfp_engine_fileio::storage_flow::MapSource::Local(_)
    ));
    assert_eq!(copied.title_bytes(), b"Local sample copy");
    assert_eq!(
        sample.0.bytes(),
        sample_payload.bytes(),
        "native Save As cannot modify disc sample input"
    );
    assert_eq!(std::fs::read(local_file).unwrap(), local_before);
    std::fs::remove_dir_all(root).unwrap();
}
