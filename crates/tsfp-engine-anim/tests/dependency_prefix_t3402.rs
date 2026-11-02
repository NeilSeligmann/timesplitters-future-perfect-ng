use std::collections::BTreeMap;

use tsfp_engine_anim::dependency_prefix::{
    DependencyRun, DependencyWorld, FALLBACK_SUPPRESSING_INDEX, PrefixDisposition,
    run_dependency_prefix,
};

struct World {
    list: Option<Vec<u32>>,
    names: BTreeMap<u32, Vec<u8>>,
    fallback: Vec<u8>,
    mounts: Vec<Vec<u8>>,
    append_after_mount: Option<u32>,
    rewrite_row_after_mount: Option<(i32, u32)>,
    list_presence_reads: usize,
}

impl World {
    fn new(list: Option<Vec<u32>>) -> Self {
        Self {
            list,
            names: BTreeMap::from([(1, b"fallback".to_vec())]),
            fallback: b"fallback".to_vec(),
            mounts: Vec::new(),
            append_after_mount: None,
            rewrite_row_after_mount: None,
            list_presence_reads: 0,
        }
    }

    fn name(mut self, index: u32, name: &[u8]) -> Self {
        self.names.insert(index, name.to_vec());
        self
    }
}

impl DependencyWorld for World {
    type MountResult = bool;

    fn list_present(&mut self) -> bool {
        self.list_presence_reads += 1;
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
        self.fallback.clone()
    }

    fn mount(&mut self, _resource_index: u32, name: &[u8]) -> Self::MountResult {
        self.mounts.push(name.to_vec());
        if let Some(index) = self.append_after_mount.take() {
            self.list.as_mut().unwrap().push(index);
        }
        if let Some((row, index)) = self.rewrite_row_after_mount.take() {
            self.list.as_mut().unwrap()[row as usize] = index;
        }
        false // FABRICATED mount failure: marker/fallback follows attempt order.
    }
}

#[test]
fn null_empty_and_level_gate_keep_distinct_fallback_behavior() {
    let mut null_list = World::new(None);
    let null_run = run_dependency_prefix(0x64, &mut null_list);
    assert_eq!(null_run.disposition, PrefixDisposition::NullList);
    assert!(null_run.attempts.is_empty());
    assert!(null_list.mounts.is_empty());

    let mut empty_list = World::new(Some(Vec::new()));
    let empty_run = run_dependency_prefix(0x64, &mut empty_list);
    assert_eq!(
        empty_run.disposition,
        PrefixDisposition::ListProcessed {
            suppressing_row_attempted: false,
            fallback_attempted: true,
        }
    );
    assert_eq!(empty_list.mounts, [b"fallback".to_vec()]);

    let mut excluded = World::new(Some(vec![7])).name(7, b"unused");
    let excluded_run = run_dependency_prefix(0x65, &mut excluded);
    assert_eq!(excluded_run.disposition, PrefixDisposition::LevelExcluded);
    assert_eq!(excluded.list_presence_reads, 0);
    assert!(excluded.mounts.is_empty());
}

#[test]
fn deduplicates_exact_names_in_order_and_only_an_attempted_unique_marker_suppresses_fallback() {
    // Row zero is skipped as a current row, but its non-null name still
    // participates in duplicate checks. The later marker row duplicates that
    // name, so it is not attempted and cannot suppress fallback.
    let mut duplicate_marker = World::new(Some(vec![0, 5, 0x38]))
        .name(0, b"shared")
        .name(5, b"shared")
        .name(0x38, b"shared");
    let duplicate_run = run_dependency_prefix(4, &mut duplicate_marker);
    assert_eq!(duplicate_marker.mounts, [b"fallback".to_vec()]);
    assert_eq!(
        duplicate_run.disposition,
        PrefixDisposition::ListProcessed {
            suppressing_row_attempted: false,
            fallback_attempted: true,
        }
    );

    // Comparison is byte-exact; case variants are separate requests.
    let mut case_sensitive = World::new(Some(vec![3, 4]))
        .name(3, b"Asset")
        .name(4, b"asset");
    let _ = run_dependency_prefix(4, &mut case_sensitive);
    assert_eq!(
        case_sensitive.mounts,
        [b"Asset".to_vec(), b"asset".to_vec(), b"fallback".to_vec()]
    );

    // The unique marker row is attempted. A failed mount still suppresses the
    // fallback, matching the observed original marker placement.
    let mut unique_marker = World::new(Some(vec![0x38])).name(0x38, b"marked");
    let unique_run = run_dependency_prefix(4, &mut unique_marker);
    assert_eq!(unique_marker.mounts, [b"marked".to_vec()]);
    assert_eq!(unique_run.attempts[0].result, false);
    assert_eq!(
        unique_run.disposition,
        PrefixDisposition::ListProcessed {
            suppressing_row_attempted: true,
            fallback_attempted: false,
        }
    );
}

#[test]
fn list_count_and_post_mount_index_are_reread_after_callback_mutation() {
    let mut world = World::new(Some(vec![2]))
        .name(2, b"first")
        .name(3, b"second")
        .name(FALLBACK_SUPPRESSING_INDEX, b"marked");
    world.append_after_mount = Some(3);
    world.rewrite_row_after_mount = Some((0, FALLBACK_SUPPRESSING_INDEX));

    let run = run_dependency_prefix(4, &mut world);
    assert_eq!(world.mounts, [b"first".to_vec(), b"second".to_vec()]);
    assert_eq!(run.attempts.len(), 2);
    assert!(run.attempts.iter().all(|attempt| attempt.row.is_some()));
    assert_eq!(
        run.disposition,
        PrefixDisposition::ListProcessed {
            suppressing_row_attempted: true,
            fallback_attempted: false,
        }
    );
}

#[test]
fn null_name_is_skipped_without_turning_the_list_into_a_null_list() {
    let mut world = World::new(Some(vec![8]));
    let run: DependencyRun<bool> = run_dependency_prefix(4, &mut world);
    assert_eq!(world.mounts, [b"fallback".to_vec()]);
    assert_eq!(
        run.disposition,
        PrefixDisposition::ListProcessed {
            suppressing_row_attempted: false,
            fallback_attempted: true,
        }
    );
}
