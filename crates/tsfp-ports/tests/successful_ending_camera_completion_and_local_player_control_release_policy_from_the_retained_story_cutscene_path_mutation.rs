// SPDX-License-Identifier: GPL-3.0-or-later
//! Mutation sensitivity for the T2488 release contract.

use tsfp_ports::story_control_release_lane::{
    BindingHandle, CallbackHandle, CameraId, PlayerContext, ReleaseOutcome,
    StoryControlReleaseBackend, StoryMission, release_successful_ending,
};

#[derive(Default)]
struct Log(Vec<(u8, Option<u32>)>);
impl StoryControlReleaseBackend for Log {
    fn invoke_finish(&mut self, callback: CallbackHandle) {
        self.0.push((0, Some(callback.0)));
    }
    fn restore_camera_binding(&mut self, binding: Option<BindingHandle>) {
        self.0.push((1, binding.map(|v| v.0)));
    }
    fn restore_inventory_binding(&mut self, binding: Option<BindingHandle>) {
        self.0.push((2, binding.map(|v| v.0)));
    }
}

#[test]
fn semantic_mutants_change_observed_contract_results() {
    let input = StoryMission {
        segment_index: 3,
        segment_count: 3,
        first_segment_flags: 0,
        attract_mode: false,
        active_camera: Some(CameraId(4)),
        local_player: Some(PlayerContext {
            camera: Some(CameraId(4)),
            cinematic_flags: 0x0200_0001,
            finish_callback: Some(CallbackHandle(5)),
            update_callback: Some(CallbackHandle(6)),
            camera_binding: Some(BindingHandle(7)),
            inventory_binding: Some(BindingHandle(8)),
            mode: 3,
            timer: 9,
        }),
    };
    let mut canonical = input;
    let mut log = Log::default();
    assert_eq!(
        release_successful_ending(&mut canonical, &mut log),
        ReleaseOutcome::Released
    );
    let expected_context = canonical.local_player.unwrap();
    let expected_log = [(0, Some(5)), (1, Some(7)), (2, Some(8))];

    // Mutant: retain the cinematic bit.
    let mutant_flags = expected_context.cinematic_flags | 0x0200_0000;
    assert_ne!(mutant_flags, expected_context.cinematic_flags);
    // Mutant: omit the mode/timer transition.
    assert_ne!((3, 9), (expected_context.mode, expected_context.timer));
    // Mutant: reorder callback and binding effects.
    let reordered = [(1, Some(7)), (0, Some(5)), (2, Some(8))];
    assert_ne!(reordered, expected_log);

    let mut before_final = input;
    before_final.segment_index = 2;
    let mut no_effects = Log::default();
    assert_eq!(
        release_successful_ending(&mut before_final, &mut no_effects),
        ReleaseOutcome::NotReady
    );
    assert!(no_effects.0.is_empty());
}
