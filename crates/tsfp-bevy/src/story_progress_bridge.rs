//! Opt-in host attachment from typed T1703/T2369 Story state to native profile persistence.
//!
//! This module never reads guest memory. The mission outcome supplies completion and campaign
//! identity; StoryFlow supplies the active Story setup and ordinary selector; the storage
//! lifecycle supplies the authenticated selected profile.

use tsfp_engine_fileio::story_progress::{StoryMode, StoryProgressUpdate, StorySelector};
use tsfp_gameplay::story_events::slots::MissionOutcome;
use tsfp_state::story_flow::{MODE_STORY, RESULT_SUCCESS, StoryFlow};

pub(crate) fn extract_switch(arguments: &mut Vec<String>) -> bool {
    let mut enabled = false;
    arguments.retain(|argument| {
        if argument == "--story-progress-rust" {
            enabled = true;
            false
        } else {
            true
        }
    });
    enabled
}

/// Prepare the per-profile mutation only when the typed Story owner and gameplay result agree.
/// Phase 8 is a required route guard, never a substitute for the success bit. Objective
/// definition keys are intentionally not consulted: they describe data rows, not completion.
pub(crate) fn qualified_update(
    flow: &StoryFlow,
    outcome: &MissionOutcome,
    mode: StoryMode,
    best_time: f32,
    minimum_time: f32,
) -> Option<StoryProgressUpdate> {
    let campaign_ordinal = outcome.campaign_ordinal?;
    let selector = StorySelector::new(flow.active.byte(0x56))?.get();
    if flow.active.byte(0x55) != MODE_STORY
        || flow.phase != 8
        || outcome.phase != 8
        || flow.result_flags != outcome.result_flags
        || flow.result_flags & RESULT_SUCCESS == 0
        || outcome.result_flags & RESULT_SUCCESS == 0
        || flow.ordinal != campaign_ordinal
    {
        return None;
    }

    Some(StoryProgressUpdate {
        result_flags: outcome.result_flags,
        campaign_ordinal: campaign_ordinal.get(),
        selector,
        mode,
        best_time,
        minimum_time,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutcomeResult {
    Disabled,
    Ignored,
    Committed {
        campaign_ordinal: u8,
        receipt: tsfp_engine_fileio::storage_flow::StoryProgressCommitReceipt,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_state::story_flow::{CampaignOrdinal, SETUP_BYTES, SessionSetup};

    fn story_flow(selector: u8, ordinal: u8, flags: u32, phase: u32) -> StoryFlow {
        let mut flow = StoryFlow::default();
        let mut setup = SessionSetup::default();
        setup.set_byte(0x55, MODE_STORY);
        setup.set_byte(0x56, selector);
        assert_eq!(SETUP_BYTES, 0xa0);
        flow.active = setup;
        flow.ordinal = CampaignOrdinal::new(ordinal).unwrap();
        flow.result_flags = flags;
        flow.phase = phase;
        flow
    }

    fn outcome(ordinal: Option<u8>, flags: u32, phase: u32) -> MissionOutcome {
        MissionOutcome {
            mode_id: 0,
            result_flags: flags,
            phase,
            event: None,
            campaign_ordinal: ordinal.and_then(CampaignOrdinal::new),
            objectives: Vec::new(),
        }
    }

    #[test]
    fn switch_is_removed_from_default_host_arguments_and_defaults_off() {
        let mut arguments = vec!["default".to_owned(), "--story-progress-rust".to_owned()];
        assert!(extract_switch(&mut arguments));
        assert_eq!(arguments, ["default"]);
        let mut default_arguments = vec!["default".to_owned()];
        assert!(!extract_switch(&mut default_arguments));
        assert_eq!(default_arguments, ["default"]);
    }

    #[test]
    fn only_matching_typed_story_success_produces_a_profile_update() {
        let flow = story_flow(2, 4, RESULT_SUCCESS | 0x10000, 8);
        let result = outcome(Some(4), RESULT_SUCCESS | 0x10000, 8);
        let update = qualified_update(&flow, &result, StoryMode::SinglePlayer, 12.5, 15.0).unwrap();
        assert_eq!(update.campaign_ordinal, 4);
        assert_eq!(update.selector, 2);
        assert_eq!(update.result_flags, RESULT_SUCCESS | 0x10000);
        assert_eq!(update.best_time, 12.5);

        for (mismatched_flow, mismatched_result) in [
            (story_flow(2, 4, 0, 8), outcome(Some(4), 0, 8)),
            (
                story_flow(2, 4, RESULT_SUCCESS, 2),
                outcome(Some(4), RESULT_SUCCESS, 2),
            ),
            (
                story_flow(2, 3, RESULT_SUCCESS, 8),
                outcome(Some(4), RESULT_SUCCESS, 8),
            ),
            (
                story_flow(2, 4, RESULT_SUCCESS, 8),
                outcome(None, RESULT_SUCCESS, 8),
            ),
            (
                story_flow(2, 4, RESULT_SUCCESS, 8),
                outcome(Some(4), RESULT_SUCCESS | 0x10000, 8),
            ),
            (
                story_flow(3, 4, RESULT_SUCCESS, 8),
                outcome(Some(4), RESULT_SUCCESS, 8),
            ),
        ] {
            assert!(
                qualified_update(
                    &mismatched_flow,
                    &mismatched_result,
                    StoryMode::SinglePlayer,
                    1.0,
                    0.0,
                )
                .is_none()
            );
        }
        let mut non_story = story_flow(2, 4, RESULT_SUCCESS, 8);
        non_story.active.set_byte(0x55, 0);
        assert!(
            qualified_update(&non_story, &result, StoryMode::SinglePlayer, 1.0, 0.0,).is_none()
        );
    }
}
