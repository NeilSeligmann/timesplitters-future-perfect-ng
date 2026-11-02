//! Tier 0 fabricated rows; durable-key navigation and root contract checks.
use tsfp_engine_fileio::{
    storage_flow::{MapSource, SampleMapEntry, SaveEntry, SaveKey, SaveKind},
    storage_frontend::{Command, MapLoadSource, Screen, Target},
    storage_route::{Root, StorageRoute},
    vfs::status,
};

fn row(directory: &str) -> SaveEntry {
    SaveEntry {
        key: SaveKey::new(directory, "profile").unwrap(),
        kind: SaveKind::Profile,
        label: Ok(directory.into()),
        bytes: 8040,
    }
}
#[test]
fn focus_survives_refresh_by_key_and_errors_never_load_a_different_row() {
    let target = Target::Profile { player: 3 };
    let mut route = StorageRoute::default();
    route.publish(
        &Screen::Catalogue {
            target,
            entries: vec![row("a"), row("b")],
            sample_maps: vec![],
        },
        None,
    );
    route.focus(1).unwrap();
    route.publish(
        &Screen::Catalogue {
            target,
            entries: vec![row("b"), row("a")],
            sample_maps: vec![],
        },
        None,
    );
    assert_eq!(route.cursor(), Some(0));
    assert!(
        matches!(route.load_selected().unwrap(), Command::LoadProfile { player: 3, key } if key == row("b").key)
    );
    let mut corrupt = row("b");
    corrupt.label = Err(status::UNRECOGNIZED_VOLUME);
    route.publish(
        &Screen::Catalogue {
            target,
            entries: vec![corrupt],
            sample_maps: vec![],
        },
        None,
    );
    assert!(matches!(
        route.load_selected(),
        Err(status::UNRECOGNIZED_VOLUME)
    ));
    route.publish(
        &Screen::Error {
            target,
            status: status::INVALID_PARAMETER,
        },
        None,
    );
    assert!(route.focus(0).is_err());
    assert!(route.load_selected().is_err());
    route.publish(&Screen::Closed, None);
    assert!(route.entries().is_empty());
    assert!(route.load_selected().is_err());
}

#[test]
fn mixed_map_catalogue_keeps_sample_identity_and_returns_a_typed_source() {
    let target = Target::Map;
    let mut local = row("local-map");
    local.kind = SaveKind::Map;
    let sample = |id, label: &str| SampleMapEntry::new(id, label.to_owned()).unwrap();
    let mut route = StorageRoute::default();
    route.publish(
        &Screen::Catalogue {
            target,
            entries: vec![local.clone()],
            sample_maps: vec![sample(77, "Sample A"), sample(88, "Sample B")],
        },
        None,
    );
    route.focus(2).unwrap();
    assert!(matches!(
        route.load_selected().unwrap(),
        Command::LoadMap {
            source: MapSource::DiscSample(88)
        }
    ));

    route.publish(
        &Screen::Catalogue {
            target,
            entries: vec![local],
            sample_maps: vec![sample(88, "Sample B"), sample(77, "Sample A")],
        },
        None,
    );
    assert_eq!(route.cursor(), Some(1));
    assert!(matches!(
        route.load_selected().unwrap(),
        Command::LoadMap {
            source: MapSource::DiscSample(88)
        }
    ));
}

#[test]
fn map_source_choice_is_owned_and_returns_a_typed_catalogue_command() {
    let mut route = StorageRoute::default();
    route.publish(&Screen::MapSourceChoice, None);
    assert!(route.map_source_choice());
    assert_eq!(route.cursor(), Some(0));
    assert!(matches!(
        route.load_selected().unwrap(),
        Command::OpenMapCatalogue(MapLoadSource::Samples)
    ));
    route.focus(1).unwrap();
    assert!(matches!(
        route.load_selected().unwrap(),
        Command::OpenMapCatalogue(MapLoadSource::LocalStorage)
    ));
    assert!(route.focus(2).is_err());
    route.publish(
        &Screen::Catalogue {
            target: Target::Map,
            entries: Vec::new(),
            sample_maps: Vec::new(),
        },
        None,
    );
    assert!(!route.map_source_choice());
}
#[test]
fn root_addresses_require_typed_matching_context() {
    for address in [0x192770, 0x193120, 0x193310] {
        let root = Root::from_address(address).unwrap();
        assert!(root.accepts(Target::Profile { player: 0 }));
        assert!(!root.accepts(Target::Profile { player: 4 }));
        assert!(!root.accepts(Target::Map));
    }
    for address in [0x32b6a0, 0x32aee0] {
        let root = Root::from_address(address).unwrap();
        assert!(root.accepts(Target::Map));
        assert!(!root.accepts(Target::Profile { player: 0 }));
    }
    assert_eq!(Root::from_address(0x194980), None);
}

#[test]
fn invalidation_discards_only_the_deactivated_consumers_receipt() {
    use tsfp_engine_fileio::{storage_flow::Operation, storage_frontend::Completion};
    let player = Target::Profile { player: 1 };
    let receipt = Completion {
        target: player,
        operation: Operation::ProfileLoad,
        key: Some(row("saved").key),
    };
    let mut route = StorageRoute::default();
    route.publish(&Screen::Closed, Some(receipt.clone()));
    route.invalidate_completion(Target::Map);
    route.invalidate_completion(Target::Profile { player: 0 });
    assert_eq!(route.take_completion(), Some(receipt.clone()));
    route.publish(&Screen::Closed, Some(receipt));
    route.invalidate_completion(player);
    assert!(route.take_completion().is_none());
}
