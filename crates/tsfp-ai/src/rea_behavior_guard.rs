// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable state-guard slice shared by AI behavior ports.

/// Actor-owned fields read or changed by original `0x26DBE0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionGuardState {
    pub local_flags: u32,
    pub local_state: u32,
    pub queued_state: u32,
    pub action_timer: f32,
    pub target_node: i32,
    pub pending_target_node: i32,
    pub route_progress_a: u32,
    pub route_progress_b: u32,
}

/// Side effects which remain at the runtime boundary while actor state is guest-owned.
pub trait ActionGuardHost {
    fn status(&mut self) -> i32;
    fn timer_threshold(&mut self) -> f32;
    fn masked_action_exists(&mut self) -> bool;
    fn alternate_action_available(&mut self) -> bool;
    fn movement_suppressed(&mut self) -> bool;
    fn set_mode_bit(&mut self, extension: u32, bit: u32);
    fn target_and_animation(&mut self) -> (i32, i32, i32, u32);
    fn play_animation(&mut self, animation: u32);
}

/// Applies the measured action guard. Returns whether the guard consumed the tick.
/// The caller resolves the selected owner node and animation pair from live tables.
pub fn apply_action_guard(
    state: &mut ActionGuardState,
    extension: u32,
    host: &mut impl ActionGuardHost,
) -> bool {
    let status = host.status();
    if state.local_flags & 0x8000_0000 != 0 && host.masked_action_exists() {
        state.local_flags &= 0x7fff_ffff;
    }

    if status == 0 {
        return false;
    }
    if status == 1 {
        if state.local_flags & 0x8000_0000 != 0 {
            return false;
        }
        state.action_timer = 0.0;
        if state.local_state != 3 {
            state.queued_state = 3;
        }
        return true;
    }
    if status == 3 && state.local_flags & 0x8000_0000 != 0 {
        return false;
    }

    if state.local_state == 5 {
        if host.alternate_action_available() {
            state.queued_state = 3;
        } else {
            host.set_mode_bit(extension, 8);
        }
        return true;
    }
    if !(state.action_timer > host.timer_threshold()) || host.movement_suppressed() {
        return false;
    }

    let (owner_node, player_node, fallback_node, selected_animation) = host.target_and_animation();
    state.action_timer = 0.0;
    state.pending_target_node = owner_node;
    state.target_node = if owner_node < 0 {
        player_node
    } else {
        owner_node
    };
    if state.target_node < 0 {
        state.target_node = fallback_node;
    }
    state.route_progress_a = 0;
    state.route_progress_b = 0;
    host.play_animation(selected_animation);
    if state.local_state != 5 {
        state.queued_state = 5;
    }
    true
}
