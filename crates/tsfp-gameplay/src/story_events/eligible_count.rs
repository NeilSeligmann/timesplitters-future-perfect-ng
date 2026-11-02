// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed Story link-kind-11 eligible-count reducer (T2453).
//!
//! Setup eligibility is decided by the neighboring setup-query contract; this module
//! owns how accepted setup rows and live actor lifecycle facts form the target and
//! observed totals. Guest addresses and the `0x6C890` query are intentionally outside
//! the native reducer.

/// One already-filtered setup row for the requested Story key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EligibleSetupRow {
    /// Result of the neighboring setup eligibility query.
    pub eligible: bool,
    /// SETU `+0x60`; `0x7fff_ffff` selects this row's typed fallback count.
    pub configured_count: u32,
    /// Fallback count resolved by the caller for the sentinel case.
    pub fallback_count: u32,
    /// Paired setup-table contribution read by the lifted helper at cursor `+8`.
    pub paired_count: u32,
}

/// The lifecycle facts inspected for one live object with the requested key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiveActorFacts {
    /// Mode-10 lifecycle words (`+0x10`, `+0x14`).
    pub mode10_first: u32,
    pub mode10_second: u32,
    /// Other-mode lifecycle words (`+0x18`, `+0x1C`).
    pub other_first: u32,
    pub other_second: u32,
    /// Object flags byte (`+0x24`).
    pub flags: u8,
}

/// Typed inputs to the link-kind-11 count query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryEligibleCountInput<'a> {
    pub mode10: bool,
    pub setup_rows: &'a [EligibleSetupRow],
    pub live_actors: &'a [LiveActorFacts],
}

/// The original helper's two outputs: configured target and paired setup plus live count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoryEligibleCount {
    pub target: u32,
    pub total: u32,
}

/// Computes the Story kind-11 target and live total using only typed state.
///
/// All arithmetic wraps at 32 bits, matching the guest dword accumulators. The caller
/// filters rows/actors by key and resolves sentinel fallback counts before calling.
pub fn story_eligible_count(input: StoryEligibleCountInput<'_>) -> StoryEligibleCount {
    let mut target = 0u32;
    for row in input.setup_rows.iter().copied().filter(|row| row.eligible) {
        let count = if row.configured_count == 0x7fff_ffff {
            row.fallback_count
        } else {
            row.configured_count
        };
        target = target.wrapping_add(count);
    }

    let mut paired = 0u32;
    for row in input.setup_rows.iter().copied().filter(|row| row.eligible) {
        paired = paired.wrapping_add(row.paired_count);
    }

    let mut live = 0u32;
    for actor in input.live_actors {
        let first = if input.mode10 {
            actor.mode10_first
        } else {
            actor.other_first
        };
        if (first & 1) != 0 || (actor.flags & 8) != 0 {
            live = live.wrapping_add(1);
        }
    }
    StoryEligibleCount {
        target,
        total: paired.wrapping_add(live),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_target_from_mode_selected_live_count() {
        let setup = [
            EligibleSetupRow {
                eligible: true,
                configured_count: 2,
                fallback_count: 99,
                paired_count: 4,
            },
            EligibleSetupRow {
                eligible: false,
                configured_count: 4,
                fallback_count: 99,
                paired_count: 100,
            },
            EligibleSetupRow {
                eligible: true,
                configured_count: 0x7fff_ffff,
                fallback_count: 3,
                paired_count: 1,
            },
        ];
        let actors = [
            LiveActorFacts {
                mode10_first: 1,
                other_first: 0,
                ..Default::default()
            },
            LiveActorFacts {
                mode10_first: 0,
                other_first: 1,
                ..Default::default()
            },
            LiveActorFacts {
                flags: 8,
                ..Default::default()
            },
            LiveActorFacts::default(),
        ];
        let mode10 = story_eligible_count(StoryEligibleCountInput {
            mode10: true,
            setup_rows: &setup,
            live_actors: &actors,
        });
        let other = story_eligible_count(StoryEligibleCountInput {
            mode10: false,
            setup_rows: &setup,
            live_actors: &actors,
        });
        assert_eq!(
            mode10,
            StoryEligibleCount {
                target: 5,
                total: 7
            }
        );
        assert_eq!(
            other,
            StoryEligibleCount {
                target: 5,
                total: 7
            }
        );
    }

    #[test]
    fn preserves_guest_dword_wrapping() {
        let setup = [
            EligibleSetupRow {
                eligible: true,
                configured_count: u32::MAX - 1,
                fallback_count: 0,
                paired_count: 0,
            },
            EligibleSetupRow {
                eligible: true,
                configured_count: 4,
                fallback_count: 0,
                paired_count: 0,
            },
        ];
        assert_eq!(
            story_eligible_count(StoryEligibleCountInput {
                mode10: false,
                setup_rows: &setup,
                live_actors: &[],
            }),
            StoryEligibleCount {
                target: 2,
                total: 0
            }
        );
    }
}
