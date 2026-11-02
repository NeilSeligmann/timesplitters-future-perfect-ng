//! Tier 0 synthetic profiles and runtime policy, no original data fixtures.
use std::{cell::Cell, collections::BTreeMap};
use tsfp_engine_fileio::{
    profile_progress::ProfileProgress, storage_bootstrap::RuntimeRead, vfs::status,
};
use tsfp_format_profile::Profile;
fn profile() -> Profile {
    let mut profile = Profile {
        payload: vec![0; 0x1f54],
        signature: [0; 20],
    };
    profile.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    profile
}
fn progress() -> ProfileProgress {
    ProfileProgress::read(&profile()).unwrap()
}
struct Runtime(BTreeMap<u32, u32>);
impl RuntimeRead for Runtime {
    fn word(&self, address: u32) -> Option<u32> {
        self.0.get(&address).copied()
    }
}
fn runtime() -> Runtime {
    let mut words = BTreeMap::new();
    for index in 0..48u32 {
        let table = if index < 21 { 0x510788 } else { 0x513d04 };
        let definition = 0x900000 + index * 0x100;
        let descriptor = definition + 0x40;
        for (address, value) in [
            (table + index * 4, definition),
            (definition + 0x20, 2),
            (definition + 0x1c, descriptor),
            (descriptor, 0),
            (descriptor + 0x14, 0x80),
            (descriptor + 0x20, index % 6),
        ] {
            words.insert(address, value);
        }
    }
    Runtime(words)
}
#[test]
fn projection_uses_summary_not_reset_block_and_single_needs_no_runtime() {
    let mut p = profile();
    p.payload[0x80..0x1290].fill(0xff);
    p.payload[0x1298..0x129c].copy_from_slice(&7u32.to_le_bytes());
    let result = ProfileProgress::read(&p).unwrap();
    assert_eq!(result.counters[0][0], 7);
    assert_eq!(result.challenges[0].completions, 0);
    assert_eq!(
        ProfileProgress::combine(&[result.clone()], &Runtime(BTreeMap::new())).unwrap(),
        result
    );
    p.payload.clear();
    assert!(ProfileProgress::read(&p).is_err());
    assert_eq!(
        ProfileProgress::combine(&[], &runtime()),
        Err(status::INVALID_PARAMETER)
    );
}
#[test]
fn participant_merge_wraps_unions_and_uses_signed_challenge_policy() {
    let mut a = progress();
    let mut b = progress();
    a.counters[0][0] = u32::MAX;
    b.counters[0][0] = 3;
    a.unlock_words[22] = 0x80;
    b.unlock_words[22] = 0x40;
    for i in 0..48 {
        a.challenges[i].completions = 1;
        a.challenges[i].score = -9;
        b.challenges[i].completions = 2;
        b.challenges[i].score = 11;
    }
    a.challenges[8].completions = 0;
    b.challenges[9].completions = 0;
    let merged = ProfileProgress::combine(&[a, b], &runtime()).unwrap();
    assert_eq!(merged.counters[0][0], 2);
    assert_eq!(merged.unlock_words[22], 0xc0);
    for i in 0..48 {
        let expected = if i == 8 {
            11
        } else if i == 9 {
            -9
        } else if matches!(i % 6, 3 | 4) {
            11
        } else {
            -9
        };
        assert_eq!(merged.challenges[i].score, expected, "challenge {i}");
    }
    assert_eq!(merged.challenges[0].completions, 3);
}
#[test]
fn missing_or_changing_needed_policy_never_publishes_partial_merge() {
    let mut a = progress();
    a.challenges[0].completions = 1;
    let mut r = runtime();
    r.0.remove(&0x900060);
    assert!(ProfileProgress::combine(&[a.clone(), a.clone()], &r).is_err());
    struct Changed {
        source: Runtime,
        reads: Cell<u32>,
    }
    impl RuntimeRead for Changed {
        fn word(&self, address: u32) -> Option<u32> {
            if address == 0x900060 {
                let old = self.reads.get();
                self.reads.set(old + 1);
                if old > 0 {
                    return Some(4);
                }
            }
            self.source.word(address)
        }
    }
    assert!(
        ProfileProgress::combine(
            &[a.clone(), a],
            &Changed {
                source: runtime(),
                reads: Cell::new(0)
            }
        )
        .is_err()
    );
}

#[test]
fn availability_refresh_obeys_mode_flags_restriction_and_runtime_rows() {
    use tsfp_engine_fileio::profile_progress::AvailabilityPolicy;
    let mut r = Runtime(BTreeMap::from([(0x79094c, 0x65)]));
    assert_eq!(AvailabilityPolicy::read(&r).unwrap().mask(), 0);
    r.0.insert(0x79094c, 0x67);
    assert_eq!(AvailabilityPolicy::read(&r).unwrap().mask(), 0);
    r.0.insert(0x79094c, 0);
    for flag in [0x40, 0x800000, 0x8000000] {
        r.0.insert(0x7de458, flag);
        assert_eq!(AvailabilityPolicy::read(&r).unwrap().mask(), 0);
    }
    r.0.insert(0x7de458, 0);
    r.0.insert(0x7de454, 0);
    r.0.insert(0x76b108, 1);
    assert_eq!(AvailabilityPolicy::read(&r).unwrap().mask(), 0);
    r.0.insert(0x76b108, 0);
    assert!(AvailabilityPolicy::read(&r).is_err());
    for index in 0..13u32 { r.0.insert(0x4fcc84 + index * 28, if index == 12 { 6 } else { 4 }); }
    assert_eq!(AvailabilityPolicy::read(&r).unwrap().mask(), 1 << 12);
    r.0.insert(0x7de454, 10 << 8);
    r.0.remove(&0x76b108);
    assert_eq!(AvailabilityPolicy::read(&r).unwrap().mask(), 0x1fff);
    struct Changed { source: Runtime, reads: Cell<u32> }
    impl RuntimeRead for Changed {
        fn word(&self, address: u32) -> Option<u32> {
            if address == 0x4fcc84 {
                let n = self.reads.get(); self.reads.set(n + 1);
                if n > 0 { return Some(0); }
            }
            self.source.word(address)
        }
    }
    assert!(AvailabilityPolicy::read(&Changed { source: r, reads: Cell::new(0) }).is_err());
}
