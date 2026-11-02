use super::*;

fn config() -> Config<'static> {
    Config {
        mounts: &[b"\\Device\\empty-a", b"\\Device\\empty-b"],
        disc: Some(Mount {
            device: b"\\Device\\CdRom0",
            path: b"/disc/image.iso",
        }),
        hdd: Some(Mount {
            device: b"\\Device\\Harddisk0\\partition1",
            path: b"/hdd",
        }),
        partition0: Partition0 {
            device: b"\\Device\\Harddisk0\\partition0",
            file_name: b".tsfp-partition0.bin",
            capacity: 12345,
        },
        pre_mounted: false,
        window_mapped: true,
        missing_empty: true,
        cache_count: 2,
        cache_first: 3,
        cache_capacity: 67890,
    }
}

fn kinds(config: &Config<'_>) -> Vec<ActionKind> {
    let mut result = Vec::new();
    run(config, |action| {
        result.push(action.kind());
        true
    })
    .unwrap();
    result
}

#[test]
fn ordinary_setup_preserves_mount_and_diagnostic_order() {
    assert_eq!(
        kinds(&config()),
        [
            ActionKind::Empty,
            ActionKind::Empty,
            ActionKind::Disc,
            ActionKind::HostDir,
            ActionKind::Partition0,
            ActionKind::PublishCacheCount,
            ActionKind::CachePartition,
            ActionKind::CachePartition,
            ActionKind::MissingEmpty,
        ]
    );
}

#[test]
fn failure_at_every_stage_stops_without_rollback_or_later_events() {
    let config = config();
    let expected = kinds(&config);
    for failed in 0..expected.len() {
        let mut seen = Vec::new();
        let result = run(&config, |action| {
            seen.push(action.kind());
            seen.len() - 1 != failed
        });
        assert_eq!(
            result,
            Err(RunError {
                index: failed,
                kind: expected[failed],
            })
        );
        assert_eq!(seen, expected[..=failed]);
    }
}

#[test]
fn pre_mounted_partition0_still_emits_one_event_with_supplied_policy() {
    for pre_mounted in [false, true] {
        let config = Config {
            pre_mounted,
            ..config()
        };
        let mut events = 0;
        run(&config, |action| {
            if let Action::Partition0 {
                device,
                host_dir,
                file_name,
                capacity,
                skip_mount,
            } = action
            {
                events += 1;
                assert_eq!(*device, config.partition0.device);
                assert_eq!(*host_dir, config.hdd.unwrap().path);
                assert_eq!(*file_name, config.partition0.file_name);
                assert_eq!(*capacity, config.partition0.capacity);
                assert_eq!(*skip_mount, pre_mounted);
            }
            true
        })
        .unwrap();
        assert_eq!(events, 1);
    }
}

#[test]
fn cache_names_use_supplied_first_number_and_capacity() {
    let config = Config {
        cache_first: 7,
        ..config()
    };
    let mut seen = 0;
    run(&config, |action| {
        if let Action::CachePartition {
            device,
            host_dir,
            number,
            image_name,
            content_dir,
            capacity,
        } = action
        {
            let expected = 7 + seen;
            assert_eq!(*number, expected);
            assert_eq!(
                device,
                format!("\\Device\\Harddisk0\\Partition{expected}").as_bytes()
            );
            assert_eq!(image_name, format!(".tsfp-cache{expected}.bin").as_bytes());
            assert_eq!(content_dir, format!(".tsfp-cache{expected}").as_bytes());
            assert_eq!(*host_dir, b"/hdd");
            assert_eq!(*capacity, 67890);
            seen += 1;
        }
        true
    })
    .unwrap();
    assert_eq!(seen, 2);
}

#[test]
fn absent_hdd_suppresses_all_hdd_and_cache_actions() {
    let config = Config {
        hdd: None,
        pre_mounted: true,
        ..config()
    };
    assert_eq!(
        kinds(&config),
        [
            ActionKind::Empty,
            ActionKind::Empty,
            ActionKind::Disc,
            ActionKind::MissingEmpty
        ]
    );
}

#[test]
fn unmapped_window_suppresses_publication_but_keeps_cache_mounts() {
    let config = Config {
        window_mapped: false,
        ..config()
    };
    let actions = kinds(&config);
    assert!(!actions.contains(&ActionKind::PublishCacheCount));
    assert_eq!(
        actions
            .iter()
            .filter(|kind| **kind == ActionKind::CachePartition)
            .count(),
        2
    );
}

#[test]
fn zero_cache_count_has_neither_publication_nor_cache_mounts() {
    let config = Config {
        cache_count: 0,
        ..config()
    };
    assert_eq!(
        kinds(&config),
        [
            ActionKind::Empty,
            ActionKind::Empty,
            ActionKind::Disc,
            ActionKind::HostDir,
            ActionKind::Partition0,
            ActionKind::MissingEmpty
        ]
    );
}

#[test]
fn no_requested_operations_does_nothing() {
    let config = Config {
        mounts: &[],
        disc: None,
        hdd: None,
        missing_empty: false,
        ..config()
    };
    run(&config, |_| panic!("no operation requested")).unwrap();
}

#[test]
fn raw_path_bytes_are_preserved_without_utf8_conversion() {
    let empty: &[u8] = b"\\Device\\empty\xff";
    let device: &[u8] = b"\\Device\\drive\xfe";
    let path: &[u8] = b"/selected/\xfd";
    let names = [empty];
    let config = Config {
        mounts: &names,
        disc: Some(Mount { device, path }),
        hdd: Some(Mount { device, path }),
        partition0: Partition0 {
            device,
            file_name: b"config\xfc.bin",
            capacity: 99,
        },
        ..config()
    };
    let mut path_actions = 0;
    run(&config, |action| {
        match action {
            Action::Empty { path } => {
                assert_eq!(*path, empty);
                path_actions += 1;
            }
            Action::Disc {
                device: got_device,
                path: got_path,
            }
            | Action::HostDir {
                device: got_device,
                path: got_path,
            } => {
                assert_eq!(*got_device, device);
                assert_eq!(*got_path, path);
                path_actions += 1;
            }
            Action::Partition0 {
                device: got_device,
                host_dir,
                file_name,
                ..
            } => {
                assert_eq!(*got_device, device);
                assert_eq!(*host_dir, path);
                assert_eq!(*file_name, b"config\xfc.bin");
                path_actions += 1;
            }
            Action::CachePartition { host_dir, .. } => {
                assert_eq!(*host_dir, path);
                path_actions += 1;
            }
            _ => {}
        }
        true
    })
    .unwrap();
    assert_eq!(path_actions, 6);
}
