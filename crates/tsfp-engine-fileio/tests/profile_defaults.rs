//! Tier 0 constructor/spec checks, fabricated runtime table and retained fields.
use tsfp_engine_fileio::{
    profile_defaults::*, storage_bootstrap::RuntimeRead, storage_lifecycle::ProfileInitializer,
    vfs::status,
};
use tsfp_format_profile::Profile;
struct Table([u32; 9]);
impl RuntimeRead for Table {
    fn word(&self, address: u32) -> Option<u32> {
        let offset = address.checked_sub(0x2c7198)?;
        (offset % 4 == 0)
            .then(|| self.0.get(offset as usize / 4).copied())
            .flatten()
    }
}
fn retained() -> Profile {
    let mut p = Profile {
        payload: vec![0x35; 0x1f54],
        signature: [0x61; 20],
    };
    p.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    p
}
fn word(p: &Profile, offset: usize) -> u32 {
    u32::from_le_bytes(p.payload[offset..offset + 4].try_into().unwrap())
}
#[test]
fn runtime_mode_choices_are_checked_and_constructor_fields_match_reset_contract() {
    let modes = mode_defaults(&Table([
        0x2c715f, 0x2c716e, 0x2c7175, 0x2c717d, 0x2c7181, 0x2c718a, 0x2c715f, 0x2c718a, 0x2c717d,
    ]))
    .unwrap();
    let defaults = ProgressDefaults::new(modes);
    assert_eq!(defaults.mode_records[0], [0, 1, 0, 0, 4]);
    assert_eq!(defaults.mode_records[1], [0, 1, 3, 2, 4]);
    assert_eq!(defaults.mode_records[2], [0, 1, 3, 5, 4]);
    assert_eq!(defaults.mode_records[4], [0, 1, 1, 0, 4]);
    assert_eq!(defaults.mode_records[5], [0, 1, 2, 4, 4]);
    assert_eq!(defaults.mode_records[10], [0, 1, 0, 0, 4]);
    assert!(mode_defaults(&Table([0; 9])).is_err());
    let mut p = retained();
    let before = p.clone();
    defaults.reset_profile(&mut p).unwrap();
    assert_eq!(&p.payload[..0x80], &before.payload[..0x80]);
    assert_eq!(&p.payload[0x1edc..], &before.payload[0x1edc..]);
    assert_eq!(p.signature, before.signature);
    assert!(p.payload[0x1290..0x1edc].iter().all(|b| *b == 0));
    assert_eq!(word(&p, 0x80 + 0x120), 11);
    assert_eq!(word(&p, 0x80 + 0x124), 16);
    for category in 0..6 {
        for row in 0..20 {
            let at = 0x80 + 0x2d0 + category * 0xa4 + row * 8;
            assert_eq!(word(&p, at), 0xffff);
            assert_eq!(word(&p, at + 4), 0);
        }
    }
    let expected_options = [0, 0, 0, 1, 1, 1, 1, 0, 1, 0];
    for scenario in 0..18 {
        for (column, option) in expected_options.into_iter().enumerate() {
            let at = 0x80 + 0x6b0 + scenario * 0xa0 + column * 4;
            assert_eq!(word(&p, at), u32::from(column < 6));
            assert_eq!(word(&p, at + 0x28), 2);
            assert_eq!(word(&p, at + 0x50), option);
        }
    }
}
#[test]
fn native_head_and_progress_constructor_preserve_unmodeled_tail_and_reject_bad_profile() {
    let defaults = ProgressDefaults::new([ModeDefault::Unchanged; 9]);
    let mut init = ProfileInitializer::new([(2, 3); 4], 1, 0.2).unwrap();
    let p = init
        .initialize_with_progress(&retained(), &defaults)
        .unwrap();
    assert_eq!(init.next_ring_slot(), 2);
    assert_eq!(&p.payload[..4], &[0; 4]);
    assert_eq!(&p.payload[4..6], &1u16.to_le_bytes());
    assert_eq!(word(&p, 0x80 + 4), 1);
    assert_eq!(p.signature, [0; 20]);
    assert!(p.payload[0x1290..0x1edc].iter().all(|byte| *byte == 0));
    assert_eq!(&p.payload[0x1edc..], &retained().payload[0x1edc..]);
    assert_eq!(&p.payload[0x1ee4..], &retained().payload[0x1ee4..]);
    let mut invalid = retained();
    invalid.payload.truncate(16);
    assert_eq!(
        defaults.reset_profile(&mut invalid),
        Err(status::INVALID_PARAMETER)
    );
    assert_eq!(invalid.payload, vec![0x35; 16]);
    assert!(init.initialize_with_progress(&invalid, &defaults).is_err());
    assert_eq!(init.next_ring_slot(), 2);
}
