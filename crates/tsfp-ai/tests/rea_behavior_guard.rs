// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_ai::rea_behavior_guard::{ActionGuardHost, ActionGuardState, apply_action_guard};

#[derive(Default)]
struct Host {
    status: i32,
    masked: bool,
    alternate: bool,
    suppressed: bool,
    mode_bits: Vec<(u32, u32)>,
    animations: Vec<u32>,
    target_info: (i32, i32, i32, u32),
}

impl ActionGuardHost for Host {
    fn status(&mut self) -> i32 {
        self.status
    }
    fn timer_threshold(&mut self) -> f32 {
        1.0
    }
    fn masked_action_exists(&mut self) -> bool {
        self.masked
    }
    fn alternate_action_available(&mut self) -> bool {
        self.alternate
    }
    fn movement_suppressed(&mut self) -> bool {
        self.suppressed
    }
    fn set_mode_bit(&mut self, extension: u32, bit: u32) {
        self.mode_bits.push((extension, bit));
    }
    fn target_and_animation(&mut self) -> (i32, i32, i32, u32) {
        self.target_info
    }
    fn play_animation(&mut self, animation: u32) {
        self.animations.push(animation);
    }
}

fn state() -> ActionGuardState {
    ActionGuardState {
        local_flags: 0,
        local_state: 2,
        queued_state: u32::MAX,
        action_timer: 1.5,
        target_node: 40,
        pending_target_node: 41,
        route_progress_a: 7,
        route_progress_b: 8,
    }
}

#[test]
fn action_guard_status_one_queues_idle_and_resets_timer() {
    let mut actor = state();
    let mut host = Host {
        status: 1,
        ..Host::default()
    };
    assert!(apply_action_guard(&mut actor, 0x1000, &mut host));
    assert_eq!(actor.queued_state, 3);
    assert_eq!(actor.action_timer, 0.0);
}

#[test]
fn action_guard_timer_path_resets_target_and_requests_animation() {
    let mut actor = state();
    let mut host = Host {
        status: 2,
        target_info: (-1, 12, 13, 0x5203a8),
        ..Host::default()
    };
    assert!(apply_action_guard(&mut actor, 0x1000, &mut host));
    assert_eq!(actor.pending_target_node, -1);
    assert_eq!(actor.target_node, 12);
    assert_eq!((actor.route_progress_a, actor.route_progress_b), (0, 0));
    assert_eq!(actor.queued_state, 5);
    assert_eq!(host.animations, [0x5203a8]);
}

#[test]
fn action_guard_mutations_cover_strict_timer_and_mode_flag_boundaries() {
    let mut at_threshold = state();
    at_threshold.action_timer = 1.0;
    let mut host = Host {
        status: 2,
        ..Host::default()
    };
    assert!(!apply_action_guard(&mut at_threshold, 0x1000, &mut host));
    assert_eq!(at_threshold.action_timer, 1.0);
    assert!(host.animations.is_empty());

    let mut mode_blocked = state();
    let mut host = Host {
        status: 2,
        suppressed: true,
        ..Host::default()
    };
    assert!(!apply_action_guard(&mut mode_blocked, 0x1000, &mut host));
    assert_eq!(mode_blocked.queued_state, u32::MAX);
    assert!(host.animations.is_empty());
}
