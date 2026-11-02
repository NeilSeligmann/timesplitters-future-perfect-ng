//! Rust-owned, per-profile Story progress contract (T2369). The progress updater is intentionally
//! separate from storage dirty requests and save/reload receipts.

use tsfp_engine_fileio::{
    story_progress::{StoryMode, StoryProgress, StoryProgressUpdate},
    vfs::status,
};
use tsfp_format_profile::Profile;

const SUMMARY: usize = 0x1290;

fn profile() -> Profile {
    let mut payload = vec![0x5a; 0x1f54];
    payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    Profile {
        payload,
        signature: [0x35; 20],
    }
}

fn word(profile: &Profile, offset: usize) -> u32 {
    u32::from_le_bytes(profile.payload[offset..offset + 4].try_into().unwrap())
}

#[test]
fn success_updates_only_the_selected_story_fields_and_preserves_opaque_bytes() {
    let mut profile = profile();
    let ordinal = 4usize;
    let time = SUMMARY + 0x288 + ordinal * 0x28 + 0x0c + 2 * 4;
    let single = SUMMARY + 0xbf4;
    let counter = SUMMARY + 0x918;
    let old = 40.0f32.to_bits();
    profile.payload[time..time + 4].copy_from_slice(&old.to_le_bytes());
    profile.payload[single..single + 12].copy_from_slice(
        &[0x10u32, 0x200, 0x400]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    profile.payload[counter..counter + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = profile.payload.clone();

    let receipt = StoryProgress::apply_success(
        &mut profile,
        StoryProgressUpdate {
            result_flags: 0x8000 | 0x10000,
            campaign_ordinal: ordinal as u8,
            selector: 2,
            mode: StoryMode::SinglePlayer,
            best_time: 25.0,
            minimum_time: 30.0,
        },
    )
    .unwrap();

    assert!(receipt.best_time_changed);
    assert_eq!(receipt.completion_words_changed, 2);
    assert_eq!(receipt.counter_before, u32::MAX);
    assert_eq!(receipt.counter_after, 0);
    assert_eq!(word(&profile, time), 25.0f32.to_bits());
    assert_eq!(word(&profile, single), 0x10 | (1 << ordinal));
    assert_eq!(word(&profile, single + 4), 0x200 | (1 << ordinal));
    assert_eq!(word(&profile, single + 8), 0x400 | (1 << ordinal));
    assert_eq!(word(&profile, counter), 0);

    let changed = [time, single, single + 4, single + 8, counter];
    for (offset, (old, new)) in before.iter().zip(&profile.payload).enumerate() {
        if !changed
            .iter()
            .any(|start| (*start..*start + 4).contains(&offset))
        {
            assert_eq!(old, new, "unexpected profile mutation at {offset:#x}");
        }
    }
    assert_eq!(profile.signature, [0x35; 20]);
    assert_eq!(StoryProgress::read(&profile).unwrap().story_counter, 0);
}

#[test]
fn multiplayer_uses_its_own_completion_words_and_retains_an_existing_best_time() {
    let mut profile = profile();
    let ordinal = 12usize;
    let time = SUMMARY + 0x288 + ordinal * 0x28 + 0x0c;
    let multi = SUMMARY + 0xc00;
    profile.payload[time..time + 4].copy_from_slice(&10.0f32.to_bits().to_le_bytes());
    profile.payload[multi..multi + 4].copy_from_slice(&0x2000u32.to_le_bytes());
    let receipt = StoryProgress::apply_success(
        &mut profile,
        StoryProgressUpdate {
            result_flags: 0x8000,
            campaign_ordinal: ordinal as u8,
            selector: 0,
            mode: StoryMode::Multiplayer,
            best_time: 20.0,
            minimum_time: 5.0,
        },
    )
    .unwrap();
    assert!(!receipt.best_time_changed);
    assert_eq!(receipt.completion_words_changed, 1);
    assert_eq!(word(&profile, time), 10.0f32.to_bits());
    assert_eq!(word(&profile, multi), 0x2000 | (1 << ordinal));
    assert_eq!(word(&profile, SUMMARY + 0xbf4), 0x5a5a_5a5a);
}

#[test]
fn repeated_accepted_success_reapplies_counter_on_the_opaque_fixture() {
    let mut profile = profile();
    let ordinal = 4usize;
    let time = SUMMARY + 0x288 + ordinal * 0x28 + 0x0c + 2 * 4;
    let single = SUMMARY + 0xbf4;
    let counter = SUMMARY + 0x918;
    for tier in 0..3 {
        profile.payload[single + tier * 4..single + tier * 4 + 4]
            .copy_from_slice(&0u32.to_le_bytes());
    }
    let update = StoryProgressUpdate {
        result_flags: 0x8000,
        campaign_ordinal: ordinal as u8,
        selector: 2,
        mode: StoryMode::SinglePlayer,
        best_time: 25.0,
        minimum_time: 10.0,
    };

    let first = StoryProgress::apply_success(&mut profile, update).unwrap();
    let repeated = StoryProgress::apply_success(&mut profile, update).unwrap();

    assert!(first.best_time_changed);
    assert_eq!(first.completion_words_changed, 3);
    assert_eq!(
        (first.counter_before, first.counter_after),
        (0x5a5a_5a5a, 0x5a5a_5a5b)
    );
    assert!(!repeated.best_time_changed);
    assert_eq!(repeated.completion_words_changed, 0);
    assert_eq!(
        (repeated.counter_before, repeated.counter_after),
        (0x5a5a_5a5b, 0x5a5a_5a5c)
    );
    assert_eq!(word(&profile, time), 25.0f32.to_bits());
    for tier in 0..3 {
        assert_ne!(word(&profile, single + tier * 4) & (1 << ordinal), 0);
    }
    assert_eq!(word(&profile, counter), 0x5a5a_5a5c);
}

#[test]
fn success_writer_rejects_unqualified_results_and_unsupported_selector_or_ordinal() {
    for update in [
        StoryProgressUpdate {
            result_flags: 0,
            campaign_ordinal: 0,
            selector: 0,
            mode: StoryMode::SinglePlayer,
            best_time: 1.0,
            minimum_time: 0.0,
        },
        StoryProgressUpdate {
            result_flags: 0x8000,
            campaign_ordinal: 13,
            selector: 0,
            mode: StoryMode::SinglePlayer,
            best_time: 1.0,
            minimum_time: 0.0,
        },
        StoryProgressUpdate {
            result_flags: 0x8000,
            campaign_ordinal: 0,
            selector: 3,
            mode: StoryMode::SinglePlayer,
            best_time: 1.0,
            minimum_time: 0.0,
        },
    ] {
        let mut profile = profile();
        let before = profile.payload.clone();
        assert_eq!(
            StoryProgress::apply_success(&mut profile, update),
            Err(status::INVALID_PARAMETER)
        );
        assert_eq!(profile.payload, before);
    }
}
