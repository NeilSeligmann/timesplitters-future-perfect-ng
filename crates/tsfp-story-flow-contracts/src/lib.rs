#![no_std]

/// Ordered optional objective-completion callback pair from the measured Story contract.
pub mod completion_callbacks;

/// Group-counter values returned by the caller's measured Story group query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StoryGroupProgress {
    pub completed: u32,
    pub eligible: u32,
}

impl StoryGroupProgress {
    /// Construct the predicate input from the original group's two returned counters.
    ///
    /// The caller remains responsible for identifying the Story group key and obtaining both
    /// values from the same query. This constructor only names the return-value mapping.
    pub const fn from_group_query(completed: u32, eligible: u32) -> Self {
        Self {
            completed,
            eligible,
        }
    }
}

/// Readiness of the Story group-counter predicate used by link mode 11.
///
/// The caller supplies both values; this contract does not infer group membership or eligibility.
/// The original predicate requires a nonzero eligible total and exact equality.
pub const fn story_group_completion_ready(progress: StoryGroupProgress) -> bool {
    progress.eligible != 0 && progress.completed == progress.eligible
}

/// Count threshold selected by the Story tag-count helper from active type.
///
/// The original `0x218e60` reads the active type byte (`0x7de455`); this does not
/// select by difficulty or derive the count from actor state.
pub const fn tag_count_completion_threshold_for_active_type(active_type: u8) -> u32 {
    match active_type {
        0 => 7,
        1 => 15,
        2 => 25,
        _ => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_11_requires_a_nonzero_exact_eligible_total() {
        assert!(!story_group_completion_ready(StoryGroupProgress::default()));
        assert!(!story_group_completion_ready(StoryGroupProgress {
            completed: 9,
            eligible: 10,
        }));
        assert!(story_group_completion_ready(StoryGroupProgress {
            completed: 10,
            eligible: 10,
        }));
        assert!(!story_group_completion_ready(StoryGroupProgress {
            completed: 11,
            eligible: 10,
        }));
    }

    #[test]
    fn group_query_return_value_maps_to_completed_and_out_parameter_to_eligible() {
        let progress = StoryGroupProgress::from_group_query(12, 12);
        assert_eq!(progress.completed, 12);
        assert_eq!(progress.eligible, 12);
        assert!(story_group_completion_ready(progress));

        let incomplete = StoryGroupProgress::from_group_query(11, 12);
        assert_eq!(incomplete.completed, 11);
        assert_eq!(incomplete.eligible, 12);
        assert!(!story_group_completion_ready(incomplete));
    }

    #[test]
    fn group_query_mapping_preserves_empty_and_over_count_boundaries() {
        let empty = StoryGroupProgress::from_group_query(0, 0);
        assert_eq!(empty.completed, 0);
        assert_eq!(empty.eligible, 0);
        assert!(!story_group_completion_ready(empty));

        let over_count = StoryGroupProgress::from_group_query(13, 12);
        assert_eq!(over_count.completed, 13);
        assert_eq!(over_count.eligible, 12);
        assert!(!story_group_completion_ready(over_count));
    }

    #[test]
    fn tag_count_threshold_uses_active_type_arms_and_default() {
        assert_eq!(tag_count_completion_threshold_for_active_type(0), 7);
        assert_eq!(tag_count_completion_threshold_for_active_type(1), 15);
        assert_eq!(tag_count_completion_threshold_for_active_type(2), 25);
        assert_eq!(tag_count_completion_threshold_for_active_type(3), 10);
        assert_eq!(tag_count_completion_threshold_for_active_type(10), 10);
        assert_eq!(tag_count_completion_threshold_for_active_type(u8::MAX), 10);
    }
}
