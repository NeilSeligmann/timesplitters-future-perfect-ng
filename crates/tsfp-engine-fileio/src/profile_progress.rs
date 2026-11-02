//! Owned projection/reducer of the progression consumed by 0x69A60.
//! Tier 0, INFERRED static source contract; challenge meanings remain unnamed.
use crate::{
    storage_bootstrap::RuntimeRead,
    vfs::{NtStatus, status},
};
use tsfp_format_profile::Profile;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChallengeResult {
    pub completions: u32,
    pub score: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileProgress {
    pub counters: [[u32; 5]; 32],
    pub challenges: [ChallengeResult; 48],
    pub unlock_words: [u32; 23],
}
impl ProfileProgress {
    pub fn read(profile: &Profile) -> Result<Self, NtStatus> {
        profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
        let word = |index| {
            u32::from_le_bytes(
                profile.payload[0x1290 + index * 4..0x1294 + index * 4]
                    .try_into()
                    .unwrap(),
            )
        };
        Ok(Self {
            counters: std::array::from_fn(|row| {
                std::array::from_fn(|column| word(2 + row * 5 + column))
            }),
            challenges: std::array::from_fn(|row| ChallengeResult {
                completions: word(0x124 + row * 6),
                score: word(0x125 + row * 6) as i32,
            }),
            unlock_words: std::array::from_fn(|index| word(0x2fc + index)),
        })
    }
    /// Participant order matters for challenge class 2/unknown classes: the
    /// original retains the first completed value. All counters wrap as u32.
    pub fn combine(profiles: &[Self], runtime: &impl RuntimeRead) -> Result<Self, NtStatus> {
        let Some(first) = profiles.first() else {
            return Err(status::INVALID_PARAMETER);
        };
        let mut result = first.clone();
        for profile in &profiles[1..] {
            for (out, input) in result
                .counters
                .iter_mut()
                .flatten()
                .zip(profile.counters.iter().flatten())
            {
                *out = out.wrapping_add(*input);
            }
            for (out, input) in result.unlock_words.iter_mut().zip(profile.unlock_words) {
                *out |= input;
            }
            for (index, (out, input)) in result
                .challenges
                .iter_mut()
                .zip(profile.challenges)
                .enumerate()
            {
                if out.completions == 0 {
                    out.score = input.score;
                } else if input.completions != 0 {
                    match challenge_class(runtime, index)? {
                        0 | 1 => out.score = out.score.min(input.score),
                        3 | 4 => out.score = out.score.max(input.score),
                        _ => {}
                    }
                }
                out.completions = out.completions.wrapping_add(input.completions);
            }
        }
        Ok(result)
    }
}

/// 0x230530 returns the first signed-negative descriptor in the selected
/// runtime table. Only descriptors actually needed by a merge are consulted.
fn challenge_class(runtime: &impl RuntimeRead, index: usize) -> Result<u32, NtStatus> {
    let table = if index < 21 { 0x510788 } else { 0x513d04 };
    let definition = runtime
        .word(table + index as u32 * 4)
        .filter(|p| *p != 0)
        .ok_or(status::INVALID_PARAMETER)?;
    let count = runtime
        .word(
            definition
                .checked_add(0x20)
                .ok_or(status::INVALID_PARAMETER)?,
        )
        .ok_or(status::INVALID_PARAMETER)? as i32;
    let descriptors = runtime
        .word(
            definition
                .checked_add(0x1c)
                .ok_or(status::INVALID_PARAMETER)?,
        )
        .filter(|p| *p != 0)
        .ok_or(status::INVALID_PARAMETER)?;
    // Defensive bound for malformed runtime data, not an original size claim.
    if !(1..=4096).contains(&count) {
        return Err(status::INVALID_PARAMETER);
    }
    let mut scanned = Vec::new();
    for row in 0..count as u32 {
        let address = descriptors
            .checked_add(row * 0x14)
            .ok_or(status::INVALID_PARAMETER)?;
        let flags = runtime.word(address).ok_or(status::INVALID_PARAMETER)?;
        scanned.push((address, flags));
        if flags & 0x80 != 0 {
            let class = runtime
                .word(address.checked_add(0xc).ok_or(status::INVALID_PARAMETER)?)
                .ok_or(status::INVALID_PARAMETER)?;
            if scanned
                .iter()
                .any(|(address, flags)| runtime.word(*address) != Some(*flags))
                || runtime.word(address + 0xc) != Some(class)
                || runtime.word(table + index as u32 * 4) != Some(definition)
                || runtime.word(definition + 0x20) != Some(count as u32)
                || runtime.word(definition + 0x1c) != Some(descriptors)
            {
                return Err(status::INVALID_PARAMETER);
            }
            return Ok(class);
        }
    }
    Err(status::INVALID_PARAMETER)
}

/// Runtime policy consumed by the return-path availability refresh (0x1CD170).
/// Row flags stay runtime input; no original table values are embedded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailabilityPolicy {
    pub ui_mode: u32,
    pub session_flags: u32,
    pub game_mode: u8,
    pub restricted: bool,
    pub row_flags: [u32; 13],
}
impl AvailabilityPolicy {
    /// Capture each required word twice, rejecting a torn frame snapshot.
    pub fn read(runtime: &impl RuntimeRead) -> Result<Self, NtStatus> {
        let mut words = Vec::new();
        let mut read = |address| {
            let value = runtime.word(address).ok_or(status::INVALID_PARAMETER)?;
            words.push((address, value));
            Ok::<_, NtStatus>(value)
        };
        let ui_mode = read(0x79094c)?;
        let session_flags = if matches!(ui_mode, 0x65 | 0x67) { 0 } else { read(0x7de458)? };
        let suppressed = matches!(ui_mode, 0x65 | 0x67) || session_flags & 0x08800040 != 0;
        // Suppressed branches must not require unavailable table/mode data.
        let game_mode = if suppressed { 0 } else { (read(0x7de454)? >> 8) as u8 };
        let restricted = !suppressed && game_mode != 10 && read(0x76b108)? != 0;
        let mut row_flags = [0; 13];
        if !suppressed && !restricted {
            for (index, flags) in row_flags.iter_mut().enumerate() {
                *flags = read(0x4fcc84 + index as u32 * 28)?;
            }
        }
        if words.iter().any(|(address, value)| runtime.word(*address) != Some(*value)) {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(Self { ui_mode, session_flags, game_mode, restricted, row_flags })
    }
    /// Owned replacement of 0x1CD090's 13-row mask reduction.
    pub fn mask(&self) -> u32 {
        if matches!(self.ui_mode, 0x65 | 0x67) || self.session_flags & 0x08800040 != 0 || self.restricted {
            return 0;
        }
        let required = if self.game_mode == 10 { 4 } else { 2 };
        self.row_flags.iter().enumerate().fold(0, |mask, (index, flags)| {
            mask | if flags & required != 0 { 1 << index } else { 0 }
        })
    }
}
