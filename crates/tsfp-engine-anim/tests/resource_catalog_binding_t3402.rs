use tsfp_engine_anim::{
    binding::{BindingEntry, BindingWorld, LevelBindings},
    resource_catalog::AnimationResourceCatalog,
};

struct World {
    calls: Vec<(u16, String)>,
    slot: i16,
}

impl BindingWorld for World {
    fn resident(&mut self, _pak: &str) -> bool {
        false
    }

    fn load_clip(&mut self, path: &str, id: u16) -> i16 {
        self.calls.push((id, path.to_owned()));
        self.slot
    }
}

#[test]
fn level_binding_publication_updates_typed_catalog_without_replacing_owned_clip() {
    let mut catalog = AnimationResourceCatalog::default();
    let owner = catalog.owner_id(17);
    catalog
        .publish_for_owner(owner, 99, 7, [1, 2], Some(String::from("decoded clip")))
        .unwrap();

    let entries = [
        BindingEntry {
            id: 3,
            policy: [10, 11],
            name: "first",
        },
        BindingEntry {
            id: 4,
            policy: [20, 21],
            name: "second",
        },
    ];
    let mut world = World {
        calls: Vec::new(),
        slot: 7,
    };
    let mut bindings = LevelBindings::default();
    let handles = bindings
        .load_set_into_catalog("anim/data/", &entries, &mut world, &mut catalog)
        .unwrap();

    assert_eq!(
        world.calls,
        [
            (3, "anim/data/first.raw".into()),
            (4, "anim/data/second.raw".into())
        ]
    );
    assert_eq!(handles.len(), 2);
    assert_eq!(bindings.ids[&3].slot, 7);
    assert_eq!(bindings.ids[&4].slot, 7);
    assert_eq!(bindings.policy[&7], [20, 21]);

    let first = catalog.by_id(3).unwrap();
    let second = catalog.by_id(4).unwrap();
    assert_eq!(first.handle, second.handle);
    assert_eq!(first.policy, [20, 21]);
    assert_eq!(first.clip.map(String::as_str), Some("decoded clip"));
    assert_eq!(catalog.by_id(99).unwrap().policy, [20, 21]);

    let retired = catalog.release_owner(owner).unwrap();
    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].slot, 7);
    assert_eq!(retired[0].clip.as_deref(), Some("decoded clip"));
    assert!(catalog.by_id(3).is_none());
    assert!(catalog.by_id(4).is_none());
    assert!(catalog.by_id(99).is_none());
}

struct RemappedWorld;

impl BindingWorld for RemappedWorld {
    fn resident(&mut self, _pak: &str) -> bool {
        false
    }

    fn load_clip(&mut self, _path: &str, _id: u16) -> i16 {
        // The ordinary lookup is stale after this fixture's simulated callback.
        -7
    }

    fn binding_after_load(&mut self, index: usize, id: u16, _slot: i16) -> (u16, i16) {
        assert_eq!(index, 0);
        assert_eq!(id, 5);
        (9, -23)
    }
}

#[test]
fn owner_catalog_publishes_post_callback_id_and_signed_slot() {
    let entries = [BindingEntry {
        id: 5,
        policy: [0x11, 0x22],
        name: "mutated",
    }];
    let mut world = RemappedWorld;
    let mut bindings = LevelBindings::default();
    let mut catalog = AnimationResourceCatalog::<()>::default();
    let owner = catalog.owner_id(23);

    let handles = bindings
        .load_set_for_owner_into_catalog(owner, "anim/data/", &entries, &mut world, &mut catalog)
        .unwrap();

    assert_eq!(handles[0].legacy_slot(), -23);
    assert!(!bindings.ids.contains_key(&5));
    assert_eq!(bindings.ids[&9].slot, -23);
    assert_eq!(bindings.ids[&9].path, "anim/data/mutated.raw");
    let view = catalog.by_id(9).unwrap();
    assert_eq!(view.handle.legacy_slot(), -23);
    assert_eq!(view.policy, [0x11, 0x22]);
    assert!(catalog.by_id(5).is_none());
}
