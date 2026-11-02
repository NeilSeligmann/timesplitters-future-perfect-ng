//! Typed, per-profile Story progress fields documented by T1665/T1762.
//!
//! This is an attachment to one authenticated native `Profile`, not a participant aggregate.
//! It edits only the three ordinary selector tiers covered by the Rust UI/profile contract and
//! preserves the rest of the payload. Dirty requests, accepted serialization, and reload are
//! separate operations owned by the existing storage transaction.

use crate::vfs::{NtStatus, status};
use tsfp_format_profile::Profile;

const STORY_RECORDS_OFFSET: usize = 0x288;
const STORY_RECORD_STRIDE: usize = 0x28;
const STORY_TIME_OFFSET: usize = 0x0c;
const SINGLE_COMPLETION_OFFSET: usize = 0xbf4;
const MULTI_COMPLETION_OFFSET: usize = 0xc00;
const COMPLETION_TIERS: usize = 3;
const STORY_COUNTER_OFFSET: usize = 0x918;
const PROFILE_SUMMARY_OFFSET: usize = 0x1290;

/// Story's three ordinary setup selectors. The original byte load is not locally clamped, so the
/// typed port keeps this API to the evidenced 0..=2 envelope instead of inventing wider fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorySelector(u8);

impl StorySelector {
    pub const fn new(value: u8) -> Option<Self> {
        if value < COMPLETION_TIERS as u8 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryMode {
    SinglePlayer,
    Multiplayer,
}

/// Data needed after the original guards have admitted a per-profile Story update.
///
/// `result_flags` must contain the original success bit. `best_time` and `minimum_time` are the
/// already-computed values supplied to the original best-time comparison; this type does not
/// guess the timing globals or turn phase 8/dirty into a success receipt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StoryProgressUpdate {
    pub result_flags: u32,
    pub campaign_ordinal: u8,
    pub selector: u8,
    pub mode: StoryMode,
    pub best_time: f32,
    pub minimum_time: f32,
}

/// Owned projection of only the Story fields whose addresses and update rules are documented.
/// Time values are stored as IEEE-754 bits to preserve the profile bytes exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryProgress {
    pub best_time_bits: [[u32; 3]; 13],
    /// Three lower-tier words for each mode; each word uses the campaign ordinal as its bit.
    pub single_player_completion: [u32; 3],
    pub multiplayer_completion: [u32; 3],
    pub story_counter: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryProgressReceipt {
    pub best_time_changed: bool,
    pub completion_words_changed: u8,
    pub counter_before: u32,
    pub counter_after: u32,
}

impl StoryProgress {
    /// Q-relative start of the original per-campaign Story record. The profile reader uses this
    /// same layout before selecting one profile summary or the shared aggregate.
    pub const fn record_offset(campaign_ordinal: u8) -> Option<usize> {
        if campaign_ordinal < 13 {
            Some(STORY_RECORDS_OFFSET + campaign_ordinal as usize * STORY_RECORD_STRIDE)
        } else {
            None
        }
    }

    /// Read one selected profile summary `Q = payload + 0x1290`.
    pub fn read(profile: &Profile) -> Result<Self, NtStatus> {
        profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
        let mut best_time_bits = [[0; 3]; 13];
        for (ordinal, row) in best_time_bits.iter_mut().enumerate() {
            for (selector, value) in row.iter_mut().enumerate() {
                *value = read_word(
                    &profile.payload,
                    PROFILE_SUMMARY_OFFSET
                        + STORY_RECORDS_OFFSET
                        + ordinal * STORY_RECORD_STRIDE
                        + STORY_TIME_OFFSET
                        + selector * 4,
                )?;
            }
        }
        let mut single_player_completion = [0; 3];
        let mut multiplayer_completion = [0; 3];
        for tier in 0..COMPLETION_TIERS {
            single_player_completion[tier] = read_word(
                &profile.payload,
                PROFILE_SUMMARY_OFFSET + SINGLE_COMPLETION_OFFSET + tier * 4,
            )?;
            multiplayer_completion[tier] = read_word(
                &profile.payload,
                PROFILE_SUMMARY_OFFSET + MULTI_COMPLETION_OFFSET + tier * 4,
            )?;
        }
        Ok(Self {
            best_time_bits,
            single_player_completion,
            multiplayer_completion,
            story_counter: read_word(
                &profile.payload,
                PROFILE_SUMMARY_OFFSET + STORY_COUNTER_OFFSET,
            )?,
        })
    }

    /// Write this typed projection back into the selected owned profile, retaining every byte
    /// outside the Story fields represented here.
    pub fn write_to(&self, profile: &mut Profile) -> Result<(), NtStatus> {
        profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
        for (ordinal, row) in self.best_time_bits.iter().enumerate() {
            for (selector, value) in row.iter().enumerate() {
                write_word(
                    &mut profile.payload,
                    PROFILE_SUMMARY_OFFSET
                        + STORY_RECORDS_OFFSET
                        + ordinal * STORY_RECORD_STRIDE
                        + STORY_TIME_OFFSET
                        + selector * 4,
                    *value,
                )?;
            }
        }
        for tier in 0..COMPLETION_TIERS {
            write_word(
                &mut profile.payload,
                PROFILE_SUMMARY_OFFSET + SINGLE_COMPLETION_OFFSET + tier * 4,
                self.single_player_completion[tier],
            )?;
            write_word(
                &mut profile.payload,
                PROFILE_SUMMARY_OFFSET + MULTI_COMPLETION_OFFSET + tier * 4,
                self.multiplayer_completion[tier],
            )?;
        }
        write_word(
            &mut profile.payload,
            PROFILE_SUMMARY_OFFSET + STORY_COUNTER_OFFSET,
            self.story_counter,
        )
    }

    /// Apply the documented successful per-profile Story mutation. The caller must first satisfy
    /// the writer's role, phase, participant, campaign lookup, and selected-profile guards.
    ///
    /// The update ORs the ordinal bit into tiers 0 through the selected tier, increments the
    /// documented counter with u32 wrapping, and conditionally replaces the selected best time.
    /// All other profile bytes, including unclassified Story-record fields, are retained.
    pub fn apply_success(
        profile: &mut Profile,
        update: StoryProgressUpdate,
    ) -> Result<StoryProgressReceipt, NtStatus> {
        profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
        let ordinal = usize::from(update.campaign_ordinal);
        let selector = StorySelector::new(update.selector).ok_or(status::INVALID_PARAMETER)?;
        if ordinal >= 13 || update.result_flags & 0x8000 == 0 {
            return Err(status::INVALID_PARAMETER);
        }

        let mut progress = Self::read(profile)?;
        let previous_time = f32::from_bits(
            progress.best_time_bits[ordinal][usize::from(selector.get())],
        );
        // This is the original ordered comparison: NaNs compare false and leave the old field.
        let best_time_changed =
            previous_time > update.best_time || update.minimum_time > previous_time;
        if best_time_changed {
            progress.best_time_bits[ordinal][usize::from(selector.get())] =
                update.best_time.to_bits();
        }

        let completion_words = match update.mode {
            StoryMode::SinglePlayer => &mut progress.single_player_completion,
            StoryMode::Multiplayer => &mut progress.multiplayer_completion,
        };
        let bit = 1u32 << ordinal;
        let mut completion_words_changed = 0;
        for tier in 0..=usize::from(selector.get()) {
            let before = completion_words[tier];
            completion_words[tier] |= bit;
            if completion_words[tier] != before {
                completion_words_changed += 1;
            }
        }

        let counter_before = progress.story_counter;
        let counter_after = counter_before.wrapping_add(1);
        progress.story_counter = counter_after;
        progress.write_to(profile)?;
        Ok(StoryProgressReceipt {
            best_time_changed,
            completion_words_changed,
            counter_before,
            counter_after,
        })
    }
}

fn read_word(bytes: &[u8], offset: usize) -> Result<u32, NtStatus> {
    let word = bytes
        .get(offset..offset + 4)
        .ok_or(status::INVALID_PARAMETER)?;
    Ok(u32::from_le_bytes(word.try_into().unwrap()))
}

fn write_word(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), NtStatus> {
    let word = bytes
        .get_mut(offset..offset + 4)
        .ok_or(status::INVALID_PARAMETER)?;
    word.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
