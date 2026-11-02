// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed movement values at the established REA boundaries.
//!
//! Tier 1 differential coverage for the XZ approach helper (0x266250) is in
//! `tests/t2385_rea.rs`. Animation-follow decisions use the endpoint and goal
//! fields documented by T1662; invalid resource and NaN behavior remains outside
//! the Rust-owned input contract.

use crate::awareness::TargetHandle;
pub use crate::movement_approach::{Position3, approach_xz};
pub use crate::rea_link_decision::{LinkFollowDecision, LinkFollowInput, link_follow_decision};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SecondaryMovement {
    pub local_state: u32,
    pub previous_state: u32,
    pub queued_state: Option<u32>,
    pub elapsed: f32,
    /// This is the secondary-state deadline at extension+aa0, not a route cursor.
    pub fire_deadline: f32,
    pub direction: i32,
    pub angle_code: i32,
    pub availability: u32,
    pub current_secondary: Option<TargetHandle>,
    pub saved_secondary: Option<TargetHandle>,
    /// The documented action-entry reset at extension+b34.
    pub movement_timer_b34: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryActionEffect {
    None,
    RequestDirectionPair { pair: u32, direction: i32 },
    ClaimAndSelectSecondary { action: u32, angle_code: i32 },
    RequestBehaviorMask4,
}

/// The measured action-to-local-state map at the 0x2A4120 state-2 boundary.
/// Controller flags, animation callbacks and target selection remain host-owned.
pub fn dispatch_secondary_action(
    movement: &mut SecondaryMovement,
    action: u32,
) -> SecondaryActionEffect {
    let (next_state, pair, clear_motion_timers) = match action {
        0 => (3, None, false),
        1 => (4, Some(0x520970), true),
        2 => (5, Some(0x520990), true),
        3 => (6, Some(0x520968), false),
        4 => (7, Some(0x5209a0), false),
        5 => (8, Some(0x5209b0), true),
        6 => (9, Some(0x5209d0), true),
        7 => (10, Some(0x5209e0), false),
        8 | 9 => (11, None, false),
        _ => return SecondaryActionEffect::RequestBehaviorMask4,
    };

    if next_state != movement.local_state {
        movement.elapsed = 0.0;
    }
    movement.previous_state = movement.local_state;
    movement.local_state = next_state;
    movement.queued_state = Some(next_state);
    if clear_motion_timers {
        movement.fire_deadline = 0.0;
        movement.movement_timer_b34 = 0.0;
    }

    match action {
        0 => SecondaryActionEffect::None,
        8 | 9 => SecondaryActionEffect::ClaimAndSelectSecondary {
            action,
            angle_code: movement.angle_code,
        },
        _ => SecondaryActionEffect::RequestDirectionPair {
            pair: pair.expect("action has a pair"),
            direction: movement.direction,
        },
    }
}
