// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned actor health and the bounded regeneration work established by T1661.
//!
//! `ObjectId` is the lifecycle key: a reused slot with a new generation is a different actor.
//! This store is opt-in and does not change the hybrid game's current tick routing.
use crate::object_pool::ObjectId;
use alloc::collections::BTreeMap;

/// Health values owned for one object lifecycle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorHealth {
    pub health: f32,
    pub maximum: f32,
}

/// Inputs the bounded regeneration slice reads from adjacent systems/globals.
///
/// `mode_divisor` is the resolved table value for the leaf's selector mode (the established
/// cases are 1 or 3). `frame_divisor` preserves the signed guest integer before float conversion;
/// `frame_numerator` is the leaf's separately read numerator value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HealthTickInputs {
    /// Object identity read from the current-player record's `+0xFF4` member.
    /// This is not the identity of the player record itself.
    pub referenced_object: ObjectId,
    pub damage_timer: f32,
    pub selector: u32,
    pub frame_numerator: f32,
    pub frame_divisor: i32,
    pub mode_divisor: f32,
    pub tick_scale: f32,
}

/// Why the source-established regeneration write was skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthTickSkip {
    ActorMissing,
    DamageTimerNotAboveFive,
    SameAsReferencedObject,
    SelectorIsTwentySix,
}

/// Result of attempting the health portion of an actor tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HealthTickResult {
    Skipped(HealthTickSkip),
    Updated {
        previous: f32,
        term: f32,
        health: f32,
    },
}

/// Native owner for actor health. Other actor fields and extension lifetimes remain separate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActorHealthSet {
    actors: BTreeMap<ObjectId, ActorHealth>,
}

impl ActorHealthSet {
    pub fn insert(&mut self, id: ObjectId, health: ActorHealth) -> Option<ActorHealth> {
        self.actors.insert(id, health)
    }

    pub fn remove(&mut self, id: ObjectId) -> Option<ActorHealth> {
        self.actors.remove(&id)
    }

    pub fn get(&self, id: ObjectId) -> Option<&ActorHealth> {
        self.actors.get(&id)
    }

    /// Apply only the health update and guards documented for 0x2784D0/0x2777E0.
    ///
    /// The damage timer belongs to the separately typed extension; it is passed as a value so
    /// this owner does not conflate that extension with the actor object or keyed A buffer.
    pub fn tick(&mut self, id: ObjectId, input: HealthTickInputs) -> HealthTickResult {
        let Some(actor) = self.actors.get_mut(&id) else {
            return HealthTickResult::Skipped(HealthTickSkip::ActorMissing);
        };
        if !(input.damage_timer > 5.0) {
            return HealthTickResult::Skipped(HealthTickSkip::DamageTimerNotAboveFive);
        }
        if id == input.referenced_object {
            return HealthTickResult::Skipped(HealthTickSkip::SameAsReferencedObject);
        }
        if input.selector == 0x1a {
            return HealthTickResult::Skipped(HealthTickSkip::SelectorIsTwentySix);
        }

        // Keep the lifted scalar operation order: (numerator / signed-frame-divisor) *
        // (maximum / selector-divisor) * tick-scale.
        let frame_divisor = input.frame_divisor as f32;
        let term = (input.frame_numerator / frame_divisor)
            * (actor.maximum / input.mode_divisor)
            * input.tick_scale;
        let previous = actor.health;
        let candidate = previous + term;

        // Ordered comparisons match the source's SSE clamp for finite values and leave NaN
        // unordered, rather than Rust's `clamp` behavior.
        actor.health = if candidate > actor.maximum {
            actor.maximum
        } else if candidate < 0.0 {
            0.0
        } else {
            candidate
        };
        HealthTickResult::Updated {
            previous,
            term,
            health: actor.health,
        }
    }
}
