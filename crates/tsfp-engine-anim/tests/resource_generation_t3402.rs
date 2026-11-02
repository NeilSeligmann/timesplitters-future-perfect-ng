//! FABRICATED Tier 0 coverage for coordinated Rust resource generation reset.

use tsfp_engine_anim::{
    binding::{BindingEntry, BindingWorld, LevelBindings},
    resource_catalog::{AnimationResourceCatalog, CatalogError},
};

const ENTRIES: [BindingEntry; 1] = [BindingEntry {
    id: 7,
    policy: [0x11, 0x22],
    name: "clip",
}];

#[derive(Default)]
struct World {
    loads: usize,
}

impl BindingWorld for World {
    fn resident(&mut self, _pak: &str) -> bool {
        false
    }

    fn load_clip(&mut self, _path: &str, _id: u16) -> i16 {
        self.loads += 1;
        3
    }
}

#[test]
fn generation_reset_clears_catalog_and_legacy_binding_views() {
    let mut bindings = LevelBindings::default();
    let mut catalog: AnimationResourceCatalog<()> = AnimationResourceCatalog::default();
    let mut world = World::default();
    bindings
        .load_set_into_catalog("anim/", &ENTRIES, &mut world, &mut catalog)
        .unwrap();
    let previous = catalog.by_id(7).unwrap().handle;

    let next_generation = bindings.begin_resource_generation(&mut catalog).unwrap();

    assert_ne!(next_generation, previous.generation());
    assert!(catalog.by_id(7).is_none());
    assert!(catalog.view(previous).is_none());
    assert!(bindings.ids.is_empty());
    assert!(bindings.policy.is_empty());
}

#[test]
fn active_owner_rejects_generation_reset_without_mutating_either_view() {
    let mut bindings = LevelBindings::default();
    let mut catalog: AnimationResourceCatalog<()> = AnimationResourceCatalog::default();
    let mut world = World::default();
    let owner = catalog.owner_id(41);
    bindings
        .load_set_for_owner_into_catalog(owner, "anim/", &ENTRIES, &mut world, &mut catalog)
        .unwrap();
    let previous = catalog.by_id(7).unwrap().handle;
    let previous_ids = bindings.ids.clone();
    let previous_policy = bindings.policy.clone();

    assert_eq!(
        bindings.begin_resource_generation(&mut catalog),
        Err(CatalogError::OwnersActive)
    );

    assert_eq!(catalog.by_id(7).unwrap().handle, previous);
    assert_eq!(bindings.ids, previous_ids);
    assert_eq!(bindings.policy, previous_policy);
}

#[test]
fn stale_owner_release_after_generation_reset_preserves_new_binding_view() {
    let mut bindings = LevelBindings::default();
    let mut catalog: AnimationResourceCatalog<()> = AnimationResourceCatalog::default();
    let mut world = World::default();
    let old_owner = catalog.owner_id(41);
    bindings
        .load_set_for_owner_into_catalog(old_owner, "old/", &ENTRIES, &mut world, &mut catalog)
        .unwrap();
    bindings
        .release_catalog_owner(old_owner, &mut catalog)
        .unwrap();

    let generation = bindings.begin_resource_generation(&mut catalog).unwrap();
    assert_eq!(catalog.generation(), generation);
    let current_owner = catalog.owner_id(41);
    bindings
        .load_set_for_owner_into_catalog(
            current_owner,
            "current/",
            &ENTRIES,
            &mut world,
            &mut catalog,
        )
        .unwrap();
    let current_handle = catalog.by_id(7).unwrap().handle;
    let previous_ids = bindings.ids.clone();
    let previous_policy = bindings.policy.clone();
    let loads_before_stale_attempt = world.loads;

    assert_eq!(
        bindings.load_set_for_owner_into_catalog(
            old_owner,
            "stale/",
            &ENTRIES,
            &mut world,
            &mut catalog,
        ),
        Err(CatalogError::StaleOwner)
    );
    assert_eq!(world.loads, loads_before_stale_attempt);

    assert!(matches!(
        bindings.release_catalog_owner(old_owner, &mut catalog),
        Err(CatalogError::StaleOwner)
    ));

    assert_eq!(bindings.ids, previous_ids);
    assert_eq!(bindings.policy, previous_policy);
    assert_eq!(bindings.ids[&7].path, "current/clip.raw");
    assert_eq!(catalog.view(current_handle).unwrap().policy, [0x11, 0x22]);
}
