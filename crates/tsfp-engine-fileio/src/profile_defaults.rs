//! Owned reconstruction of 0x2C7010 after 0x69D90 clears its progress block.
//! Names describe observed fields; gameplay meanings of some option words remain unknown.
//! Verification Tier 0: INFERRED lift-source contract, synthetic constructor/spec tests.
use crate::{
    storage_bootstrap::RuntimeRead,
    vfs::{NtStatus, status},
};
use tsfp_format_profile::Profile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeDefault {
    Unchanged,
    ThreeTwo,
    ThreeFive,
    Three,
    One,
    TwoFour,
}
impl ModeDefault {
    fn pair(self) -> (u32, u32) {
        match self {
            Self::Unchanged => (0, 0),
            Self::ThreeTwo => (3, 2),
            Self::ThreeFive => (3, 5),
            Self::Three => (3, 0),
            Self::One => (1, 0),
            Self::TwoFour => (2, 4),
        }
    }
}

/// The nine original mode choices come from the owner's executable at runtime.
/// Unknown branch targets are rejected rather than silently given a guessed preset.
pub fn mode_defaults(runtime: &impl RuntimeRead) -> Result<[ModeDefault; 9], NtStatus> {
    let mut modes = [ModeDefault::Unchanged; 9];
    for (index, mode) in modes.iter_mut().enumerate() {
        *mode = match runtime.word(0x2c7198 + index as u32 * 4) {
            Some(0x2c715f) => ModeDefault::ThreeTwo,
            Some(0x2c716e) => ModeDefault::ThreeFive,
            Some(0x2c7175) => ModeDefault::Three,
            Some(0x2c717d) => ModeDefault::One,
            Some(0x2c7181) => ModeDefault::TwoFour,
            Some(0x2c718a) => ModeDefault::Unchanged,
            _ => return Err(status::INVALID_PARAMETER),
        };
    }
    Ok(modes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ranking {
    pub index: i16,
    pub reserved: u16,
    pub value: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScenarioColumn {
    pub enabled: bool,
    pub difficulty: u32,
    pub option: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressDefaults {
    pub startup: [u32; 9],
    pub mode_records: [[u32; 5]; 11],
    pub option_words: [u32; 8],
    pub category_counts: [u16; 6],
    pub rankings: [[Ranking; 20]; 6],
    pub secondary: [u32; 3],
    pub scenarios: [[ScenarioColumn; 10]; 18],
}
impl ProgressDefaults {
    pub fn new(modes: [ModeDefault; 9]) -> Self {
        // Record zero is mode -1; record ten is mode 9. Both bypass the switch.
        let mode_records = std::array::from_fn(|index| {
            let mode = if (1..=9).contains(&index) {
                modes[index - 1]
            } else {
                ModeDefault::Unchanged
            };
            let (first, second) = mode.pair();
            [0, 1, first, second, 4]
        });
        let columns = std::array::from_fn(|index| ScenarioColumn {
            enabled: index < 6,
            difficulty: 2,
            option: if index < 3 {
                0
            } else if index < 7 {
                1
            } else {
                ((index - 7) & 1) as u32
            },
        });
        Self {
            startup: [0, 1, 1, 2, 1, 1, 2, 0, 1],
            mode_records,
            option_words: [0, 1, 1, 11, 16, 0, 0, 0],
            category_counts: [0; 6],
            rankings: [[Ranking {
                index: -1,
                reserved: 0,
                value: 0,
            }; 20]; 6],
            secondary: [0, 1, 1],
            scenarios: [columns; 18],
        }
    }

    /// Reset is distinct from normal profile persistence: loaded progress must
    /// never be rewritten with defaults merely because input settings were edited.
    pub fn reset_profile(&self, profile: &mut Profile) -> Result<(), NtStatus> {
        profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
        let bytes = &mut profile.payload[0x80..0x1290];
        bytes.fill(0);
        let word = |bytes: &mut [u8], at: usize, value: u32| {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        for (index, value) in self.startup.iter().enumerate() {
            word(bytes, index * 4, *value);
        }
        for (index, record) in self.mode_records.iter().enumerate() {
            for (field, value) in record.iter().enumerate() {
                word(bytes, 0x24 + index * 0x14 + field * 4, *value);
            }
        }
        for (index, value) in self.option_words.iter().enumerate() {
            word(bytes, 0x114 + index * 4, *value);
        }
        for (category, count) in self.category_counts.iter().enumerate() {
            let at = 0x134 + category * 0x44;
            bytes[at..at + 2].copy_from_slice(&count.to_le_bytes());
        }
        for (category, rankings) in self.rankings.iter().enumerate() {
            for (index, rank) in rankings.iter().enumerate() {
                let at = 0x2d0 + category * 0xa4 + index * 8;
                bytes[at..at + 2].copy_from_slice(&rank.index.to_le_bytes());
                bytes[at + 2..at + 4].copy_from_slice(&rank.reserved.to_le_bytes());
                word(bytes, at + 4, rank.value);
            }
        }
        for (index, value) in self.secondary.iter().enumerate() {
            word(bytes, 0x6a4 + index * 4, *value);
        }
        for (row, columns) in self.scenarios.iter().enumerate() {
            for (column, value) in columns.iter().enumerate() {
                let at = 0x6b0 + row * 0xa0 + column * 4;
                word(bytes, at, u32::from(value.enabled));
                word(bytes, at + 0x28, value.difficulty);
                word(bytes, at + 0x50, value.option);
            }
        }
        // 0x69D90 clears this second record immediately after 0x2C7010.
        profile.payload[0x1290..0x1edc].fill(0);
        Ok(())
    }
}
