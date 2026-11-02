// SPDX-License-Identifier: GPL-3.0-or-later
//! Public API for the bounded per-actor admission and timer model at `0xD8E30`.
//!
//! Shared types and ordered timer effects live in `rea_tick_state` because that
//! source is path-included by `tsfp-ports`, which has a distinct crate root.

pub use crate::rea_tick_state::{
    ActorTickAdmission, ActorTickAdmissionResult, ActorTickTimers, actor_tick_time_step,
    decay_actor_tick_timers, run_actor_tick_prelude,
};

/// Applies the observed `0xD8E30` guards before its `0x15EAE0(actor)` call.
///
/// This is a contract model for the Rust-owned seam. It does not execute guest
/// callbacks or claim that the model is connected to the live lifted entry.
pub const fn admit_actor_tick(input: ActorTickAdmission) -> ActorTickAdmissionResult {
    if input.mode == 9 {
        return ActorTickAdmissionResult::RejectedMode9;
    }
    if input.global_tick_blocked {
        return ActorTickAdmissionResult::RejectedGlobalGate;
    }
    if !input.slot_connected {
        return ActorTickAdmissionResult::RejectedDisconnectedSlot;
    }
    if input.slot < input.minimum_slot {
        return ActorTickAdmissionResult::RejectedBelowMinimumSlot;
    }
    if input.actor_permission_check_required && !input.actor_permission_granted {
        return ActorTickAdmissionResult::RejectedActorPermission;
    }
    if input.actor_flag_4_set {
        return ActorTickAdmissionResult::RejectedActorFlag4;
    }
    ActorTickAdmissionResult::Admitted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eligible() -> ActorTickAdmission {
        ActorTickAdmission {
            mode: 7,
            global_tick_blocked: false,
            slot_connected: true,
            slot: 4,
            minimum_slot: 4,
            actor_permission_check_required: false,
            actor_permission_granted: false,
            actor_flag_4_set: false,
        }
    }

    #[test]
    fn follows_measured_actor_tick_guard_order() {
        let mut input = eligible();
        input.mode = 9;
        input.global_tick_blocked = true;
        assert_eq!(
            admit_actor_tick(input),
            ActorTickAdmissionResult::RejectedMode9
        );

        input = eligible();
        input.global_tick_blocked = true;
        assert_eq!(
            admit_actor_tick(input),
            ActorTickAdmissionResult::RejectedGlobalGate
        );

        input = eligible();
        input.slot_connected = false;
        assert_eq!(
            admit_actor_tick(input),
            ActorTickAdmissionResult::RejectedDisconnectedSlot
        );

        input = eligible();
        input.slot = 3;
        assert_eq!(
            admit_actor_tick(input),
            ActorTickAdmissionResult::RejectedBelowMinimumSlot
        );

        input = eligible();
        input.actor_permission_check_required = true;
        assert_eq!(
            admit_actor_tick(input),
            ActorTickAdmissionResult::RejectedActorPermission
        );

        input = eligible();
        input.actor_permission_check_required = true;
        input.actor_permission_granted = true;
        input.actor_flag_4_set = true;
        assert_eq!(
            admit_actor_tick(input),
            ActorTickAdmissionResult::RejectedActorFlag4
        );
    }

    #[test]
    fn permission_check_is_conditional_and_minimum_slot_is_inclusive() {
        let mut input = eligible();
        input.actor_permission_granted = false;
        assert_eq!(admit_actor_tick(input), ActorTickAdmissionResult::Admitted);

        input.actor_permission_check_required = true;
        input.actor_permission_granted = true;
        assert_eq!(admit_actor_tick(input), ActorTickAdmissionResult::Admitted);
    }

    #[test]
    fn actor_prelude_decays_timers_between_early_and_late_guards() {
        let mut timers = ActorTickTimers {
            primary: 2.0,
            secondary: 1.0,
        };
        let mut input = eligible();
        input.slot_connected = false;
        assert_eq!(
            run_actor_tick_prelude(input, &mut timers, 0.25),
            ActorTickAdmissionResult::RejectedDisconnectedSlot
        );
        assert_eq!(timers.primary, 2.0);

        input = eligible();
        input.slot = 3;
        assert_eq!(
            run_actor_tick_prelude(input, &mut timers, 0.25),
            ActorTickAdmissionResult::RejectedBelowMinimumSlot
        );
        assert_eq!(timers.primary, 1.75);
        assert_eq!(timers.secondary, 0.75);
    }

    #[test]
    fn positive_actor_timers_decay_by_the_shared_measured_ratio() {
        let mut timers = ActorTickTimers {
            primary: 2.5,
            secondary: 0.5,
        };
        decay_actor_tick_timers(&mut timers, 0.25);
        assert_eq!(timers.primary, 2.25);
        assert_eq!(timers.secondary, 0.25);

        timers = ActorTickTimers {
            primary: 0.0,
            secondary: -1.0,
        };
        decay_actor_tick_timers(&mut timers, 0.25);
        assert_eq!(timers.primary, 0.0);
        assert_eq!(timers.secondary, -1.0);

        timers = ActorTickTimers {
            primary: f32::NAN,
            secondary: 0.25,
        };
        decay_actor_tick_timers(&mut timers, 0.5);
        assert!(timers.primary.is_nan());
        assert_eq!(timers.secondary, -0.25);
    }

    #[test]
    fn shared_actor_time_step_uses_signed_denominator_conversion() {
        assert_eq!(actor_tick_time_step(3.0, 2), 1.5);
        assert_eq!(actor_tick_time_step(3.0, -2), -1.5);
        assert!(actor_tick_time_step(3.0, 0).is_infinite());
    }
}
