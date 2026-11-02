//! Tier 0 fabricated runtime words, no owner key or executable bytes.
use std::cell::Cell;
use std::collections::BTreeMap;
use tsfp_engine_fileio::{
    storage_bootstrap::{RuntimeRead, title_key},
    vfs::status,
};

struct Runtime {
    words: BTreeMap<u32, u32>,
    key_reads: Cell<usize>,
    change_pointer: bool,
}
impl RuntimeRead for Runtime {
    fn word(&self, address: u32) -> Option<u32> {
        if address == 0x47583c {
            let count = self.key_reads.get();
            self.key_reads.set(count + 1);
            if self.change_pointer && count > 0 {
                return Some(0x5000);
            }
        }
        self.words.get(&address).copied()
    }
}
fn runtime() -> Runtime {
    Runtime {
        words: [
            (0x10118, 0x2000),
            (0x2008, 0x45410066),
            (0x47583c, 0x4000),
            (0x4000, 0x03020100),
            (0x4004, 0x07060504),
            (0x4008, 0x0b0a0908),
            (0x400c, 0x0f0e0d0c),
        ]
        .into(),
        key_reads: Cell::new(0),
        change_pointer: false,
    }
}
#[test]
fn imports_current_title_key_without_invented_fallback() {
    assert_eq!(
        title_key(&runtime()).unwrap(),
        Some(std::array::from_fn(|i| i as u8))
    );
    let mut r = runtime();
    for address in [0x4000, 0x4004, 0x4008, 0x400c] {
        r.words.insert(address, 0);
    }
    // All-zero input is a supplied runtime key, not an implicit fallback.
    assert_eq!(title_key(&r).unwrap(), Some([0; 16]));
}
#[test]
fn unavailable_boot_inputs_remain_pending() {
    let mut r = runtime();
    r.words.remove(&0x10118);
    assert_eq!(title_key(&r).unwrap(), None);
    r.words.insert(0x10118, 0);
    assert_eq!(title_key(&r).unwrap(), None);
    r.words.insert(0x10118, 0x2000);
    r.words.insert(0x47583c, 0);
    assert_eq!(title_key(&r).unwrap(), None);
}
#[test]
fn wrong_title_partial_key_overflow_and_changed_source_are_rejected() {
    let mut r = runtime();
    r.words.insert(0x2008, 0x12345678);
    assert_eq!(title_key(&r), Err(status::INVALID_PARAMETER));
    let mut r = runtime();
    r.words.remove(&0x4008);
    assert_eq!(title_key(&r), Err(status::INVALID_PARAMETER));
    let mut r = runtime();
    r.words.insert(0x47583c, u32::MAX - 1);
    r.words.insert(u32::MAX - 1, 3);
    assert_eq!(title_key(&r), Err(status::INVALID_PARAMETER));
    let mut r = runtime();
    r.change_pointer = true;
    assert_eq!(title_key(&r), Err(status::INVALID_PARAMETER));
}

#[test]
fn retained_constructor_input_is_checked_and_identity_is_native() {
    use tsfp_engine_fileio::storage_bootstrap::{
        new_profile_candidate, profile_identity, retained_profile,
    };
    let mut r = runtime();
    let base = 0x7bedc0 + 7 * 0x1f54;
    for offset in (0..0x1f54).step_by(4) {
        r.words.insert(base + offset, offset ^ 0x513);
    }
    r.words.insert(base + 0x1ee0, 2);
    let retained = retained_profile(&r, 7).unwrap();
    assert_eq!(retained.payload.len(), 0x1f54);
    assert_eq!(retained.signature, [0; 20]);
    assert!(retained_profile(&r, 16).is_err());
    r.words.remove(&(base + 0x1ef0));
    assert!(retained_profile(&r, 7).is_err());
    r.words.insert(base + 0x1ef0, 0);
    r.words.insert(base + 0x1ee0, 0);
    assert!(retained_profile(&r, 7).is_err());
    let candidate = new_profile_candidate(&r, 7).unwrap();
    assert_eq!(candidate.version().unwrap(), 2);
    assert_eq!(
        r.words[&(base + 0x1ee0)],
        0,
        "source slot remains untouched"
    );
    for offset in (0..0x1f54).step_by(4).filter(|offset| *offset != 0x1ee0) {
        assert_eq!(
            u32::from_le_bytes(
                candidate.payload[offset as usize..offset as usize + 4]
                    .try_into()
                    .unwrap()
            ),
            r.words[&(base + offset)]
        );
    }
    assert!(new_profile_candidate(&r, 16).is_err());
    r.words.remove(&(base + 0x1ef0));
    assert!(new_profile_candidate(&r, 7).is_err());
    let identities: std::collections::BTreeSet<_> =
        (0..1000).map(|_| profile_identity().unwrap()).collect();
    assert_eq!(identities.len(), 1000);
    for identity in identities {
        tsfp_engine_fileio::storage_flow::SaveKey::new(&identity, "tsftrpft").unwrap();
    }
}
