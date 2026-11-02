use std::collections::{BTreeMap, BTreeSet, VecDeque};

use tsfp_engine_anim::{
    binding::{BindingWorld, LevelBindings, RESIDENT_SETS, STREAMED_SETS, StreamArchive},
    resource_catalog::AnimationResourceCatalog,
};

#[derive(Default)]
struct World {
    resident_paks: BTreeSet<&'static str>,
    resident_checks: Vec<String>,
    loads: Vec<(u16, String)>,
    slots: VecDeque<i16>,
    slot: i16,
}

impl BindingWorld for World {
    fn resident(&mut self, pak: &str) -> bool {
        self.resident_checks.push(pak.to_owned());
        self.resident_paks.contains(pak)
    }

    fn load_clip(&mut self, path: &str, id: u16) -> i16 {
        self.loads.push((id, path.to_owned()));
        self.slots.pop_front().unwrap_or(self.slot)
    }
}

#[test]
fn resident_sets_publish_in_order_under_caller_selected_resource_owners() {
    let set = RESIDENT_SETS
        .iter()
        .find(|candidate| {
            RESIDENT_SETS
                .iter()
                .filter(|other| other.pak == candidate.pak)
                .count()
                == 1
        })
        .expect("at least one resident set has a unique archive name");
    assert!(!set.entries.is_empty());

    let mut world = World {
        resident_paks: BTreeSet::from([set.pak]),
        slot: 9,
        ..World::default()
    };
    let mut bindings = LevelBindings::default();
    let mut catalog: AnimationResourceCatalog<()> = AnimationResourceCatalog::default();
    let owners_by_table: BTreeMap<_, _> = RESIDENT_SETS
        .iter()
        .map(|resident| (resident.table, catalog.owner_id(u64::from(resident.table))))
        .collect();
    let mut owner_queries = 0;
    let handles = bindings
        .load_resident_for_owners_into_catalog(&mut world, &mut catalog, |resident| {
            owner_queries += 1;
            owners_by_table[&resident.table]
        })
        .unwrap();

    assert_eq!(
        world.resident_checks,
        RESIDENT_SETS
            .iter()
            .map(|resident| resident.pak.to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(owner_queries, 1);
    assert_eq!(handles.len(), set.entries.len());
    assert_eq!(world.loads.len(), set.entries.len());
    assert_eq!(
        world.loads.first().unwrap().1,
        format!("{}{}.raw", set.base, set.entries[0].name)
    );
    assert_eq!(catalog.by_slot(9).unwrap().policy, bindings.policy[&9]);

    let owner = owners_by_table[&set.table];
    let retired = bindings.release_catalog_owner(owner, &mut catalog).unwrap();
    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].slot, 9);
    assert!(bindings.ids.is_empty());
    assert!(bindings.policy.is_empty());
    assert!(retired[0].clip.is_none());
    for entry in set.entries {
        assert!(catalog.by_id(entry.id).is_none());
    }
}

#[test]
fn installed_animation_stream_publishes_for_owner_and_skips_other_callbacks() {
    let row = STREAMED_SETS
        .iter()
        .find(|candidate| !candidate.set.entries.is_empty())
        .expect("at least one streamed animation set");
    let stream = StreamArchive {
        tag: row.tag,
        mask: row.mask,
        pak: Some(row.set.pak),
        callback: 0x1234,
    };
    let mut world = World {
        slot: -2,
        ..World::default()
    };
    let mut bindings = LevelBindings::default();
    let mut catalog: AnimationResourceCatalog<()> = AnimationResourceCatalog::default();
    let owner = catalog.owner_id(23);

    let ignored = StreamArchive {
        callback: 0,
        ..stream
    };
    assert!(
        !bindings
            .installed_stream_for_owner_into_catalog(ignored, owner, &mut world, &mut catalog)
            .unwrap()
    );
    assert!(world.loads.is_empty());

    assert!(
        bindings
            .installed_stream_for_owner_into_catalog(stream, owner, &mut world, &mut catalog)
            .unwrap()
    );
    assert_eq!(world.loads.len(), row.set.entries.len());
    assert_eq!(
        world.loads.first().unwrap().1,
        format!("{}{}.raw", row.set.base, row.set.entries[0].name)
    );
    assert_eq!(catalog.by_slot(-2).unwrap().policy, bindings.policy[&-2]);

    let retired = bindings.release_catalog_owner(owner, &mut catalog).unwrap();
    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].slot, -2);
    assert!(bindings.ids.is_empty());
    assert!(bindings.policy.is_empty());
    assert!(retired[0].clip.is_none());
    for entry in row.set.entries {
        assert!(catalog.by_id(entry.id).is_none());
    }
}

#[test]
fn releasing_owner_restores_older_level_binding_alias_and_path() {
    use tsfp_engine_anim::binding::BindingEntry;

    let older_entries = [BindingEntry {
        id: 77,
        policy: [1, 2],
        name: "older",
    }];
    let newer_entries = [BindingEntry {
        id: 77,
        policy: [3, 4],
        name: "newer",
    }];
    let mut world = World {
        slots: VecDeque::from([4, 7]),
        ..World::default()
    };
    let mut bindings = LevelBindings::default();
    let mut catalog: AnimationResourceCatalog<&str> = AnimationResourceCatalog::default();
    let older_owner = catalog.owner_id(41);
    let newer_owner = catalog.owner_id(42);

    bindings
        .load_set_for_owner_into_catalog(
            older_owner,
            "older/",
            &older_entries,
            &mut world,
            &mut catalog,
        )
        .unwrap();
    bindings
        .load_set_for_owner_into_catalog(
            newer_owner,
            "newer/",
            &newer_entries,
            &mut world,
            &mut catalog,
        )
        .unwrap();
    assert_eq!(bindings.ids[&77].path, "newer/newer.raw");
    assert_eq!(catalog.by_id(77).unwrap().handle.legacy_slot(), 7);

    let retired = bindings
        .release_catalog_owner(newer_owner, &mut catalog)
        .unwrap();

    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].slot, 7);
    assert_eq!(bindings.ids[&77].path, "older/older.raw");
    assert_eq!(bindings.ids[&77].slot, 4);
    assert_eq!(catalog.by_id(77).unwrap().handle.legacy_slot(), 4);
    assert_eq!(catalog.by_id(77).unwrap().policy, [1, 2]);

    let retired = bindings
        .release_catalog_owner(older_owner, &mut catalog)
        .unwrap();
    assert_eq!(retired.len(), 1);
    assert!(!bindings.ids.contains_key(&77));
    assert!(catalog.by_id(77).is_none());
}
