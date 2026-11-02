//! Typed level entry for animation dependency mounts and resident bindings.
//!
//! The original prefix attempts dependency mounts before the level's resident
//! animation sets are bound. This composition keeps those two phases ordered
//! while leaving list storage, mount behavior, and actual archive residency at
//! the host boundary.

use crate::binding::{BindingWorld, LevelBindings};
use crate::dependency_prefix::{DependencyRun, DependencyWorld, run_dependency_prefix};

/// Host access shared by the dependency-prefix and resident-binding phases.
pub trait LevelResourceWorld: DependencyWorld + BindingWorld {}

impl<T: DependencyWorld + BindingWorld> LevelResourceWorld for T {}

/// Result of processing the prefix and then binding currently resident sets.
#[derive(Debug)]
pub struct LevelResourceLoad<R> {
    pub dependency_prefix: DependencyRun<R>,
    pub bindings: LevelBindings,
}

/// Process the dependency prefix before consulting actual archive residency.
///
/// Mount callback results do not decide which animation sets are bound. The
/// subsequent `BindingWorld::resident` queries remain authoritative, matching
/// the existing `LevelBindings` contract and allowing a failed mount attempt
/// to leave residency unchanged.
pub fn begin_level_resources<W: LevelResourceWorld>(
    level: i32,
    world: &mut W,
) -> LevelResourceLoad<W::MountResult> {
    let dependency_prefix = run_dependency_prefix(level, world);
    let bindings = LevelBindings::begin_level(world);
    LevelResourceLoad {
        dependency_prefix,
        bindings,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::binding::BindingWorld;
    use crate::dependency_prefix::DependencyWorld;

    use super::begin_level_resources;

    struct World {
        list: Option<Vec<u32>>,
        names: BTreeMap<u32, Vec<u8>>,
        events: Vec<&'static str>,
    }

    impl World {
        fn new(list: Option<Vec<u32>>) -> Self {
            Self {
                list,
                names: BTreeMap::from([(0x38, b"dependency".to_vec())]),
                events: Vec::new(),
            }
        }
    }

    impl DependencyWorld for World {
        type MountResult = bool;

        fn list_present(&mut self) -> bool {
            self.events.push("list");
            self.list.is_some()
        }

        fn list_count(&mut self) -> i32 {
            self.list.as_ref().map_or(0, |rows| rows.len() as i32)
        }

        fn list_index(&mut self, row: i32) -> u32 {
            self.list.as_ref().unwrap()[row as usize]
        }

        fn name_bytes(&mut self, index: u32) -> Option<Vec<u8>> {
            self.names.get(&index).cloned()
        }

        fn fallback_name(&mut self) -> Vec<u8> {
            b"fallback".to_vec()
        }

        fn mount(&mut self, _resource_index: u32, _name: &[u8]) -> Self::MountResult {
            self.events.push("mount");
            false
        }
    }

    impl BindingWorld for World {
        fn resident(&mut self, _pak: &str) -> bool {
            self.events.push("resident");
            false
        }

        fn load_clip(&mut self, _path: &str, _id: u16) -> i16 {
            self.events.push("load");
            -1
        }
    }

    #[test]
    fn mount_attempts_precede_residency_queries() {
        let mut world = World::new(Some(vec![0x38]));
        let result = begin_level_resources(0x64, &mut world);

        assert_eq!(world.events.first(), Some(&"list"));
        let mount = world
            .events
            .iter()
            .position(|event| *event == "mount")
            .unwrap();
        let resident = world
            .events
            .iter()
            .position(|event| *event == "resident")
            .unwrap();
        assert!(mount < resident);
        assert!(
            world.events[resident..]
                .iter()
                .all(|event| *event == "resident")
        );
        assert!(result.bindings.ids.is_empty());
    }

    #[test]
    fn null_dependency_list_still_runs_resident_binding_phase() {
        let mut world = World::new(None);
        let result = begin_level_resources(0x64, &mut world);

        assert_eq!(world.events.first(), Some(&"list"));
        assert!(!world.events.contains(&"mount"));
        assert!(world.events.contains(&"resident"));
        assert!(result.bindings.ids.is_empty());
    }
}
