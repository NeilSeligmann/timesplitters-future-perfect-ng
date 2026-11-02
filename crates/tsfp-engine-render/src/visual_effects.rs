// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned particle presentation state (T1699).
//!
//! This is a small attachment point for the native render-prep path. It owns only values; it
//! holds no guest pointers, asset pointers, D3D state, or audio handles. The original particle
//! step establishes linear position integration and age advancement. It does not establish a
//! general retirement condition, so reaching `lifetime` is observable data and does not remove
//! the particle here.

use alloc::vec::Vec;

/// A checked handle into [`VisualEffectsState`]. Reused slots advance their generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParticleHandle {
    slot: u32,
    generation: u32,
}

impl ParticleHandle {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// Values needed by the documented linear part of the particle update.
///
/// Callers supply every field at spawn time; this type invents no descriptor defaults or
/// resource ownership. `lifetime` and `age` follow the fields read by the lifted `0x10D070`
/// step. Other flag-dependent motion remains in the existing render slice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleState {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub age: f32,
    pub lifetime: f32,
}

/// Caller-assigned identity for a native effect class. This is deliberately
/// independent of the retail emission-shape, draw-kind, and pool-kind number
/// spaces; T1699 has not established a mapping between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectTypeId(pub u32);

/// Explicit presentation values owned by the native effect producer.
/// They are observational metadata here: the current renderer does not consume
/// them, and no original descriptor mapping is inferred.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectVisualParams {
    pub tint_rgba: [f32; 4],
    pub scale: [f32; 3],
    pub rotation_radians: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct EffectPresentation {
    effect_type: EffectTypeId,
    visual: EffectVisualParams,
}

/// Owned typed effect snapshot entry copied into native scene preparation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectEntry {
    pub handle: ParticleHandle,
    pub position: [f32; 3],
    pub effect_type: EffectTypeId,
    pub age: f32,
    pub lifetime: f32,
    pub visual: EffectVisualParams,
}

impl ParticleState {
    /// Apply the established position integration and age increment for one simulation step.
    /// The caller chooses the fixed-step duration; this function does not remove expired state.
    pub fn advance_linear(&mut self, dt: f32) {
        for axis in 0..3 {
            self.position[axis] = dt * self.velocity[axis] + self.position[axis];
        }
        self.age += dt;
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Slot {
    generation: u32,
    particle: Option<ParticleState>,
    presentation: Option<EffectPresentation>,
}

/// Owned visual particle records, suitable for typed render-prep readers.
///
/// This is not wired into the default guest-backed renderer. It is an opt-in state owner for
/// the later native scene/effect packet boundary. In particular, no borrowed model or geometry
/// lifetime is implied by a particle handle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisualEffectsState {
    slots: Vec<Slot>,
}

impl VisualEffectsState {
    pub const fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Insert a fully specified particle and return its generation-checked identity.
    pub fn spawn(&mut self, particle: ParticleState) -> ParticleHandle {
        self.spawn_inner(particle, None)
    }

    /// Insert a fully described native effect. Every presentation value is
    /// supplied by the caller; this does not derive defaults from guest data.
    pub fn spawn_effect(
        &mut self,
        particle: ParticleState,
        effect_type: EffectTypeId,
        visual: EffectVisualParams,
    ) -> ParticleHandle {
        self.spawn_inner(
            particle,
            Some(EffectPresentation {
                effect_type,
                visual,
            }),
        )
    }

    fn spawn_inner(
        &mut self,
        particle: ParticleState,
        presentation: Option<EffectPresentation>,
    ) -> ParticleHandle {
        if let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.particle.is_none())
        {
            slot.particle = Some(particle);
            slot.presentation = presentation;
            return ParticleHandle {
                slot: index as u32,
                generation: slot.generation,
            };
        }
        let slot = self.slots.len() as u32;
        self.slots.push(Slot {
            generation: 0,
            particle: Some(particle),
            presentation,
        });
        ParticleHandle {
            slot,
            generation: 0,
        }
    }

    /// Explicitly release a particle. This is caller-directed because the source contract does
    /// not establish that `age >= lifetime` retires every particle kind.
    pub fn release(&mut self, handle: ParticleHandle) -> Option<ParticleState> {
        let slot = self.slots.get_mut(handle.slot as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        let particle = slot.particle.take()?;
        slot.presentation = None;
        slot.generation = slot.generation.wrapping_add(1);
        Some(particle)
    }

    pub fn get(&self, handle: ParticleHandle) -> Option<&ParticleState> {
        let slot = self.slots.get(handle.slot as usize)?;
        (slot.generation == handle.generation)
            .then_some(slot.particle.as_ref())
            .flatten()
    }

    pub fn get_mut(&mut self, handle: ParticleHandle) -> Option<&mut ParticleState> {
        let slot = self.slots.get_mut(handle.slot as usize)?;
        (slot.generation == handle.generation)
            .then_some(slot.particle.as_mut())
            .flatten()
    }

    /// Advance each live record once and return stable handle/state pairs for render prep.
    pub fn update(&mut self, dt: f32) {
        for slot in &mut self.slots {
            if let Some(particle) = &mut slot.particle {
                particle.advance_linear(dt);
            }
        }
    }

    /// Borrow the active typed records without advancing simulation state.
    pub fn iter(&self) -> impl Iterator<Item = (ParticleHandle, &ParticleState)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.particle.as_ref().map(|particle| {
                (
                    ParticleHandle {
                        slot: index as u32,
                        generation: slot.generation,
                    },
                    particle,
                )
            })
        })
    }

    /// Copy only records with explicit native presentation metadata. Bare
    /// kinematic particles remain available through [`Self::iter`] but are not
    /// silently assigned a type or visual defaults for scene publication.
    pub fn iter_effects(&self) -> impl Iterator<Item = EffectEntry> + '_ {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            let particle = slot.particle.as_ref()?;
            let presentation = slot.presentation?;
            Some(EffectEntry {
                handle: ParticleHandle {
                    slot: index as u32,
                    generation: slot.generation,
                },
                position: particle.position,
                effect_type: presentation.effect_type,
                age: particle.age,
                lifetime: particle.lifetime,
                visual: presentation.visual,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{EffectTypeId, EffectVisualParams, ParticleState, VisualEffectsState};

    #[test]
    fn owned_particles_spawn_advance_keep_lifetime_and_reject_stale_handles() {
        let mut effects = VisualEffectsState::new();
        let first = effects.spawn(ParticleState {
            position: [1.0, 2.0, 3.0],
            velocity: [4.0, -2.0, 0.5],
            age: 0.25,
            lifetime: 1.0,
        });
        assert_eq!(effects.iter_effects().count(), 0);
        effects.update(0.125);
        let particle = effects.get(first).expect("spawned particle");
        assert_eq!(particle.position, [1.5, 1.75, 3.0625]);
        assert_eq!(particle.age, 0.375);
        assert_eq!(particle.lifetime, 1.0);

        let removed = effects.release(first).expect("explicit release");
        assert_eq!(removed.age, 0.375);
        let next = effects.spawn(ParticleState {
            position: [0.0; 3],
            velocity: [0.0; 3],
            age: 1.0,
            lifetime: 1.0,
        });
        assert_eq!(first.slot(), next.slot());
        assert_ne!(first.generation(), next.generation());
        assert!(effects.get(first).is_none());
        assert!(effects.get(next).is_some());
    }

    #[test]
    fn typed_effect_entries_preserve_explicit_presentation_values() {
        let mut effects = VisualEffectsState::new();
        let visual = EffectVisualParams {
            tint_rgba: [0.1, 0.2, 0.3, 0.4],
            scale: [1.0, 2.0, 3.0],
            rotation_radians: 0.75,
        };
        let handle = effects.spawn_effect(
            ParticleState {
                position: [4.0, 5.0, 6.0],
                velocity: [1.0, 0.0, -1.0],
                age: 0.25,
                lifetime: 2.0,
            },
            EffectTypeId(17),
            visual,
        );
        effects.update(0.5);

        let [entry] = effects.iter_effects().collect::<Vec<_>>()[..] else {
            panic!("one explicitly typed effect should be published");
        };
        assert_eq!(entry.handle, handle);
        assert_eq!(entry.position, [4.5, 5.0, 5.5]);
        assert_eq!(entry.effect_type, EffectTypeId(17));
        assert_eq!(entry.age, 0.75);
        assert_eq!(entry.lifetime, 2.0);
        assert_eq!(entry.visual, visual);
        effects.release(handle).unwrap();
        assert_eq!(effects.iter_effects().count(), 0);
    }
}
