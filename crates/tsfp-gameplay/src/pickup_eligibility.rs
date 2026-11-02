// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable Rust policy for the nearby weapon-pickup touch gate (original 0xBE180).
//!
//! This is inferred from the Tier 1 lifted-C translation. The caller owns and resolves the
//! player, camera, mode, team, and pickup values before invoking the policy; this module does
//! not read guest memory. It is not selected by the runtime yet. Keep it opt-in until a valid
//! Tier 2 trace gate and the subsystem's 180-second fault-free adoption run pass.

/// Typed values consumed by the pickup touch eligibility predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickupTouchContext {
    /// Camera state at camera `+0x114`; absent when the player's camera pointer is null.
    pub camera_state: Option<u32>,
    /// Player context flags byte at `+0x24`.
    pub player_flags: u8,
    /// Current game mode byte.
    pub mode: u8,
    /// Player context team dword at `+0x44`.
    pub team: u32,
    /// Pickup context kind dword at `+0x0c`.
    pub pickup_kind: u32,
}

/// Returns whether the player may touch this pickup.
///
/// The original gate rejects camera state `0x400`, player flag bit `0x08`, and pickup kinds
/// other than `0x100` for teams 2 or 3 outside mode `0x0a`. The conditions and field meanings
/// remain INFERRED until validated by the original preset's trace gate.
pub const fn is_pickup_touch_eligible(context: PickupTouchContext) -> bool {
    if matches!(context.camera_state, Some(0x400)) || context.player_flags & 0x08 != 0 {
        return false;
    }

    !(context.mode != 0x0a
        && (context.team == 2 || context.team == 3)
        && context.pickup_kind != 0x100)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOWED: PickupTouchContext = PickupTouchContext {
        camera_state: None,
        player_flags: 0,
        mode: 0,
        team: 2,
        pickup_kind: 0,
    };

    #[test]
    fn allows_the_special_kind_and_other_kinds_in_exception_mode() {
        // ALLOWED is team 2, mode 0, kind 0: a non-special kind, rejected by the Tier 1 reference.
        assert!(!is_pickup_touch_eligible(ALLOWED));
        assert!(is_pickup_touch_eligible(PickupTouchContext {
            pickup_kind: 0x100,
            ..ALLOWED
        }));
        assert!(!is_pickup_touch_eligible(PickupTouchContext {
            pickup_kind: 0,
            ..ALLOWED
        }));
        assert!(is_pickup_touch_eligible(PickupTouchContext {
            mode: 0x0a,
            pickup_kind: 0,
            ..ALLOWED
        }));
    }

    #[test]
    fn rejects_camera_flag_and_team_restricted_kinds() {
        assert!(!is_pickup_touch_eligible(PickupTouchContext {
            camera_state: Some(0x400),
            ..ALLOWED
        }));
        assert!(!is_pickup_touch_eligible(PickupTouchContext {
            player_flags: 0x08,
            ..ALLOWED
        }));
        assert!(!is_pickup_touch_eligible(PickupTouchContext {
            pickup_kind: 0,
            ..ALLOWED
        }));
    }

    #[test]
    fn team_restriction_only_applies_to_teams_two_and_three() {
        for team in [0, 1, 4, u32::MAX] {
            assert!(is_pickup_touch_eligible(PickupTouchContext {
                team,
                pickup_kind: 0,
                ..ALLOWED
            }));
        }
        assert!(!is_pickup_touch_eligible(PickupTouchContext {
            team: 3,
            pickup_kind: 0,
            ..ALLOWED
        }));
    }
}
