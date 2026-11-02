// SPDX-License-Identifier: GPL-3.0-or-later
//! WEAK Tier 1 coverage is in the port module's 3,000-seed lifted-expression differential.

use tsfp_ports::story_control_release_lane::{enabled, lane_root_selected};

#[test]
fn runtime_switch_remains_default_off_and_roots_are_explicit() {
    assert!(!enabled());
    assert!(lane_root_selected(0x000c8640));
    assert!(lane_root_selected(0x000c8920));
    assert!(lane_root_selected(0x000d4be0));
    assert!(!lane_root_selected(0x000d4be4));
}
