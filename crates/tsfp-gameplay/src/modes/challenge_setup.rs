// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned selected-Challenge setup fields and setup-record handoff (T2381).
//!
//! The original table and profile records are runtime inputs. This module owns the typed
//! selection and configuration after those inputs are supplied, while retaining every
//! uninterpreted byte from the prior record. It intentionally does not call guest functions or
//! change the hybrid default path. The REA contract and evidence limits are in
//! `docs/t2381-arcade-owned-handoff.md`.

pub const LAUNCH_RECORD_BYTES: usize = 0xa0;
pub const SETUP_RECORD_DWORDS: usize = LAUNCH_RECORD_BYTES / 4;
pub const SAVE_SETUP_FLAG: u32 = 0x0080_0000;

const fn byte(offset: usize) -> usize {
    offset
}

/// Original front-end category identity (`0x78ad38`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcadeCategoryId(pub u32);

/// Original position in a category list. Filtered-out rows leave gaps in this namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CategoryRowId(pub u32);

/// Mission-table identity (`0x78ad40`), independent of both category and row IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengeId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeUiMode {
    Mode6,
    Mode7,
}

impl ChallengeUiMode {
    const fn flags(self) -> u32 {
        match self {
            Self::Mode6 => 0x0800_0000,
            Self::Mode7 => 0x0000_0040,
        }
    }
}

/// Selection passed from the owned front end into the detail/setup boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengeLaunchRequest {
    pub category: ArcadeCategoryId,
    /// The UI row is not published by the current runtime handoff. `None` records that boundary
    /// honestly; native front-end callers may retain the original category position here.
    pub row: Option<CategoryRowId>,
    pub challenge: ChallengeId,
    pub ui_mode: ChallengeUiMode,
}

/// Raw selected mission descriptor fields consumed by the documented mode 6/7 branch.
///
/// The names retain the source offsets so they do not imply a semantic interpretation that REA
/// has not established. `first_roster_id` is the first signed dword at descriptor +0x6c.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChallengeDescriptor {
    pub word_04: u32,
    pub word_08: u32,
    pub byte_0c: u8,
    pub word_10: u32,
    pub first_roster_id: Option<i32>,
    pub word_90: u32,
    pub word_94: u32,
    pub word_98: u32,
    pub word_9c: u32,
}

/// Runtime inputs supplied by the original descriptor/table/profile providers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengeSetupContext {
    /// Menu participant count at `0x5236c4`, distinct from runtime `0x790950`.
    pub menu_participant_count: u8,
    /// Full current mode byte at `0x7de455`, used by the original flag helper.
    pub current_runtime_mode: u8,
    /// Whether `0x4d1f88 + variant * 0x20` is zero.
    pub variant_policy_word_is_zero: bool,
    /// The low-byte indexed `0x4d1f7c + 0x20 * variant` type-table values.
    pub type_by_variant: [u8; 256],
    /// Result of the existing fallback service for variant 2, when the descriptor's first ID
    /// is negative or absent. `None` means the caller has not supplied that service result.
    pub variant2_fallback_participant: Option<i32>,
    /// Multiplier read from `0x4e8cec`.
    pub timing_multiplier: u32,
}

/// A byte-preserving view of the original 0xa0-byte launch record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchRecord {
    bytes: [u8; LAUNCH_RECORD_BYTES],
}

impl LaunchRecord {
    pub const fn from_bytes(bytes: [u8; LAUNCH_RECORD_BYTES]) -> Self {
        Self { bytes }
    }

    pub const fn as_bytes(&self) -> &[u8; LAUNCH_RECORD_BYTES] {
        &self.bytes
    }

    pub fn into_bytes(self) -> [u8; LAUNCH_RECORD_BYTES] {
        self.bytes
    }

    pub fn read_u8(&self, offset: usize) -> u8 {
        self.bytes[offset]
    }

    pub fn write_u8(&mut self, offset: usize, value: u8) {
        self.bytes[offset] = value;
    }

    pub fn read_u32(&self, offset: usize) -> u32 {
        u32::from_le_bytes(
            self.bytes[offset..offset + 4]
                .try_into()
                .expect("dword offset"),
        )
    }

    pub fn write_u32(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}

/// The variant-2 early return's selected first participant, when the required source is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant2Participant(pub i32);

/// A selected descriptor result. `variant2_participant` remains separate because the REA body
/// establishes its selection rule but does not establish its destination as a launch-record
/// field in this slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedChallengeSetup {
    pub request: ChallengeLaunchRequest,
    pub record: LaunchRecord,
    pub variant2_participant: Option<Variant2Participant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupBuildError {
    Variant2FallbackUnavailable,
}

impl SelectedChallengeSetup {
    /// Apply the exact documented mode 6/7 fields over a previous record.
    ///
    /// Bytes not listed by the REA contract retain their prior value. This is required because
    /// the original selected-descriptor branch does not clear the full record first.
    pub fn build(
        request: ChallengeLaunchRequest,
        descriptor: ChallengeDescriptor,
        context: &ChallengeSetupContext,
        previous: &LaunchRecord,
    ) -> Result<Self, SetupBuildError> {
        let mut record = previous.clone();
        let variant = descriptor.word_04;
        let variant_byte = variant as u8;

        record.write_u8(byte(0x00), context.menu_participant_count);
        record.write_u8(byte(0x01), context.menu_participant_count);
        record.write_u8(byte(0x54), descriptor.word_08 as u8);
        record.write_u8(
            byte(0x55),
            context.type_by_variant[usize::from(variant_byte)],
        );
        record.write_u8(byte(0x56), variant_byte);
        record.write_u8(byte(0x57), descriptor.byte_0c);

        let mut flags = descriptor.word_10 | request.ui_mode.flags();
        if variant != 2 {
            flags |= 0x0008_0000;
            if context.current_runtime_mode != 0x0a {
                flags |= 0x0002_0000;
            }
            if context.variant_policy_word_is_zero {
                flags |= 0x0200_0000;
            }
        }
        record.write_u32(0x58, flags);
        record.write_u32(
            0x60,
            descriptor.word_90.wrapping_mul(context.timing_multiplier),
        );
        record.write_u32(0x64, descriptor.word_98);
        record.write_u32(0x68, descriptor.word_94);
        record.write_u32(0x70, request.challenge.0);
        record.write_u32(0x74, descriptor.word_9c);
        record.write_u32(0x8c, 1.0f32.to_bits());

        let variant2_participant = if variant == 2 {
            match descriptor.first_roster_id {
                Some(id) if id >= 0 => Some(Variant2Participant(id)),
                _ => Some(Variant2Participant(
                    context
                        .variant2_fallback_participant
                        .ok_or(SetupBuildError::Variant2FallbackUnavailable)?,
                )),
            }
        } else {
            None
        };
        Ok(Self {
            request,
            record,
            variant2_participant,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchSource {
    SelectedChallenge(ChallengeLaunchRequest),
    SavedSetup,
}

/// Owned configuration as it moves from menu staging to active runtime state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchConfiguration {
    pub source: LaunchSource,
    pub record: LaunchRecord,
}

impl LaunchConfiguration {
    pub fn mission_id(&self) -> ChallengeId {
        ChallengeId(self.record.read_u32(0x70))
    }

    pub fn level(&self) -> u8 {
        self.record.read_u8(0x54)
    }

    pub fn game_type(&self) -> u8 {
        self.record.read_u8(0x55)
    }

    pub fn variant(&self) -> u8 {
        self.record.read_u8(0x56)
    }

    pub fn flags(&self) -> u32 {
        self.record.read_u32(0x58)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandoffError {
    SavedSetupRequired,
}

/// A two-phase, Rust-owned setup publisher. `publish` constructs both new records before
/// replacing either field, so observers never see staging updated without its matching runtime
/// configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupHandoff {
    pub staging: LaunchConfiguration,
    pub active: Option<LaunchConfiguration>,
}

impl SetupHandoff {
    pub fn selected(setup: SelectedChallengeSetup) -> Self {
        Self {
            staging: LaunchConfiguration {
                source: LaunchSource::SelectedChallenge(setup.request),
                record: setup.record,
            },
            active: None,
        }
    }

    /// Mirror the known copy edge of `0x67280`. When the save flag is set, the saved record
    /// replaces staging first and its documented flag mask is applied before active publication.
    pub fn publish(
        &mut self,
        saved_setup: Option<&LaunchRecord>,
    ) -> Result<&LaunchConfiguration, HandoffError> {
        let mut next_staging = self.staging.clone();
        if next_staging.record.read_u32(0x58) & SAVE_SETUP_FLAG != 0 {
            let saved = saved_setup.ok_or(HandoffError::SavedSetupRequired)?;
            next_staging.record = saved.clone();
            let flags = (next_staging.record.read_u32(0x58) & 0x9fff_ffff) | SAVE_SETUP_FLAG;
            next_staging.record.write_u32(0x58, flags);
            next_staging.source = LaunchSource::SavedSetup;
        }
        let next_active = next_staging.clone();
        self.staging = next_staging;
        self.active = Some(next_active);
        Ok(self.active.as_ref().expect("active configuration assigned"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_record() -> LaunchRecord {
        LaunchRecord::from_bytes([0; LAUNCH_RECORD_BYTES])
    }

    fn context() -> ChallengeSetupContext {
        let mut type_by_variant = [0; 256];
        type_by_variant[3] = 0x5a;
        ChallengeSetupContext {
            menu_participant_count: 4,
            current_runtime_mode: 0x0a,
            variant_policy_word_is_zero: true,
            type_by_variant,
            variant2_fallback_participant: Some(17),
            timing_multiplier: 7,
        }
    }

    #[test]
    fn selected_setup_keeps_namespaces_and_only_replaces_documented_fields() {
        let request = ChallengeLaunchRequest {
            category: ArcadeCategoryId(6),
            row: Some(CategoryRowId(9)),
            challenge: ChallengeId(0x1234),
            ui_mode: ChallengeUiMode::Mode7,
        };
        let descriptor = ChallengeDescriptor {
            word_04: 3,
            word_08: 0x101,
            byte_0c: 0xa5,
            word_10: 0x10,
            word_90: 11,
            word_94: 0x94,
            word_98: 0x98,
            word_9c: 0x9c,
            ..ChallengeDescriptor::default()
        };
        let mut previous = zero_record();
        previous.write_u8(0x20, 0x77);
        let built = SelectedChallengeSetup::build(request, descriptor, &context(), &previous)
            .expect("non-variant-2 setup has no fallback dependency");

        assert_eq!(built.request.category, ArcadeCategoryId(6));
        assert_eq!(built.request.row, Some(CategoryRowId(9)));
        assert_eq!(built.record.read_u8(0x00), 4);
        assert_eq!(built.record.read_u8(0x01), 4);
        assert_eq!(built.record.read_u8(0x54), 1);
        assert_eq!(built.record.read_u8(0x55), 0x5a);
        assert_eq!(built.record.read_u8(0x56), 3);
        assert_eq!(built.record.read_u8(0x57), 0xa5);
        assert_eq!(built.record.read_u32(0x58), 0x0208_0050);
        assert_eq!(built.record.read_u32(0x60), 77);
        assert_eq!(built.record.read_u32(0x64), 0x98);
        assert_eq!(built.record.read_u32(0x68), 0x94);
        assert_eq!(built.record.read_u32(0x70), 0x1234);
        assert_eq!(built.record.read_u32(0x74), 0x9c);
        assert_eq!(built.record.read_u32(0x8c), 1.0f32.to_bits());
        assert_eq!(built.record.read_u8(0x20), 0x77);
        assert_eq!(built.variant2_participant, None);
    }

    #[test]
    fn variant_two_requires_an_explicit_fallback_and_keeps_full_width_selection() {
        let request = ChallengeLaunchRequest {
            category: ArcadeCategoryId(2),
            row: Some(CategoryRowId(3)),
            challenge: ChallengeId(5),
            ui_mode: ChallengeUiMode::Mode6,
        };
        let descriptor = ChallengeDescriptor {
            word_04: 2,
            first_roster_id: Some(-1),
            ..ChallengeDescriptor::default()
        };
        let mut ctx = context();
        ctx.variant2_fallback_participant = None;
        let mut previous = zero_record();
        previous.write_u8(0x6c, 0xa5);
        assert_eq!(
            SelectedChallengeSetup::build(request, descriptor, &ctx, &previous),
            Err(SetupBuildError::Variant2FallbackUnavailable)
        );
        ctx.variant2_fallback_participant = Some(21);
        let built = SelectedChallengeSetup::build(request, descriptor, &ctx, &previous)
            .expect("explicit fallback supplied");
        assert_eq!(built.record.read_u8(0x56), 2);
        assert_eq!(built.record.read_u8(0x6c), 0xa5);
        assert_eq!(built.variant2_participant, Some(Variant2Participant(21)));
        // A full-width variant of exactly 2 returns before the generic flag helper.
        assert_eq!(built.record.read_u32(0x58) & 0x0008_0000, 0);
        let wide_variant = ChallengeDescriptor {
            word_04: 0x102,
            ..descriptor
        };
        let wide = SelectedChallengeSetup::build(request, wide_variant, &ctx, &zero_record())
            .expect("the full-width 0x102 variant takes the generic path");
        assert_ne!(wide.record.read_u32(0x58) & 0x0008_0000, 0);
    }

    #[test]
    fn saved_setup_replaces_staging_and_active_together() {
        let request = ChallengeLaunchRequest {
            category: ArcadeCategoryId(1),
            row: Some(CategoryRowId(0)),
            challenge: ChallengeId(42),
            ui_mode: ChallengeUiMode::Mode7,
        };
        let mut selected = zero_record();
        selected.write_u32(0x58, SAVE_SETUP_FLAG | 0x6000_0000);
        let mut handoff = SetupHandoff::selected(SelectedChallengeSetup {
            request,
            record: selected,
            variant2_participant: None,
        });
        let mut saved = zero_record();
        saved.write_u32(0x58, 0x6000_0123);
        saved.write_u32(0x70, 0x9988);
        assert_eq!(handoff.publish(None), Err(HandoffError::SavedSetupRequired));
        assert!(handoff.active.is_none());
        let active = handoff
            .publish(Some(&saved))
            .expect("saved setup supplied")
            .clone();
        assert_eq!(active.source, LaunchSource::SavedSetup);
        assert_eq!(active.mission_id(), ChallengeId(0x9988));
        assert_eq!(active.flags(), SAVE_SETUP_FLAG | 0x123);
        assert_eq!(handoff.staging, active);
    }
}
