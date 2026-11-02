// SPDX-License-Identifier: GPL-3.0-or-later
//! T2240: render-only end-of-tick history. No guest memory access or simulation writes.
//! Owners supply typed values after all simulation updates. See docs/t2240-transform-history.md.
use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

/// Ownership namespace; a character's linked object may also have an Object entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransformOwner {
    Object,
    Player,
    Character,
    Camera,
}

/// Stable owner-local ID plus lifetime generation. Never use a recycled slot alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransformId {
    pub owner: TransformOwner,
    pub slot: u64,
    pub generation: u64,
}

/// Local-to-world rotation, columns are the world-space local X/Y/Z axes.
/// Owners convert their representation; history performs no normalization or interpolation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTransform {
    pub position: [f32; 3],
    pub orientation: [[f32; 3]; 3],
}

/// Immutable, owner-supplied palette in the owner's documented skinning space.
/// Arc keeps a previous palette alive without retaining a mutable animation buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct BonePalette {
    pub skeleton: u64,
    pub revision: u64,
    pub matrices: Arc<[[f32; 16]]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransformSample {
    pub world: WorldTransform,
    pub bones: Option<BonePalette>,
}

/// Explicit events; more than one can occur during a tick. No distance heuristic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Discontinuity {
    pub spawned: bool,
    pub teleported: bool,
    pub cut: bool,
    pub respawned: bool,
}
impl Discontinuity {
    pub fn any(self) -> bool {
        self.spawned || self.teleported || self.cut || self.respawned
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TickTransform {
    pub id: TransformId,
    pub sample: TransformSample,
    pub discontinuity: Discontinuity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransformPair {
    pub previous: TransformSample,
    pub current: TransformSample,
    pub discontinuity: Discontinuity,
}
impl TransformPair {
    /// Interpolation inputs snap to current on every discontinuity (including first observation).
    pub fn render_inputs(&self) -> (&TransformSample, &TransformSample) {
        if self.discontinuity.any() {
            (&self.current, &self.current)
        } else {
            (&self.previous, &self.current)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    NonIncreasingTick,
    DuplicateId(TransformId),
}

/// Complete scene snapshot, explicitly enabled by its caller; default is OFF.
/// Not part of ObjectPool/PlayerSet/CharacterSet write-through storage or PlayerRetire authority.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransformHistory {
    enabled: bool,
    tick: Option<u64>,
    entries: BTreeMap<TransformId, TransformPair>,
}
impl TransformHistory {
    /// Switching modes clears stale lifetimes and tick history. Repeating a mode preserves it.
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.tick = None;
            self.entries.clear();
        }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn tick(&self) -> Option<u64> {
        self.tick
    }
    pub fn get(&self, id: TransformId) -> Option<&TransformPair> {
        self.entries.get(&id)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&TransformId, &TransformPair)> {
        self.entries.iter()
    }

    /// Capture once after a sim tick, with ALL live owners in all namespaces.
    /// Omitted IDs retire immediately. Skipped ticks snap rather than interpolate across a gap.
    /// Validation is transactional: an error preserves the preceding snapshot.
    pub fn capture_tick(
        &mut self,
        tick: u64,
        samples: impl IntoIterator<Item = TickTransform>,
    ) -> Result<(), CaptureError> {
        if !self.enabled {
            return Ok(());
        }
        if self.tick.is_some_and(|last| tick <= last) {
            return Err(CaptureError::NonIncreasingTick);
        }
        let contiguous = self.tick.and_then(|last| last.checked_add(1)) == Some(tick);
        let mut next = BTreeMap::new();
        for input in samples {
            if next.contains_key(&input.id) {
                return Err(CaptureError::DuplicateId(input.id));
            }
            let old = self.entries.get(&input.id);
            let mut flags = input.discontinuity;
            if old.is_none() {
                flags.spawned = true;
            }
            let previous = if contiguous {
                old.map(|pair| pair.current.clone())
                    .unwrap_or_else(|| input.sample.clone())
            } else {
                input.sample.clone()
            };
            next.insert(
                input.id,
                TransformPair {
                    previous,
                    current: input.sample,
                    discontinuity: flags,
                },
            );
        }
        self.entries = next;
        self.tick = Some(tick);
        Ok(())
    }
}

/// Collectors receive shared access to authoritative owners, never mutable simulation access.
/// The implementation must return complete typed snapshots, not cached guest mirrors.
pub trait TransformSource {
    fn tick_transforms(&self) -> Vec<TickTransform>;
}
