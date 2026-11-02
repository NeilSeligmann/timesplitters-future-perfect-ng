// SPDX-License-Identifier: GPL-3.0-or-later
//! T2334: readable 0xA14B0 world-transform producer and owned tick endpoints.
//! Guest decoding/mirroring is a TEMPORARY-SHIM (T2044 S3); math and history
//! operate only on typed state. Native presentation never advances simulation.
use crate::c2r::rt::{cos, fp_pc, sin};
use crate::scene::render_prep::{Placement, PlacementSampler};
use crate::{abi::ExtRegisters, memory::GuestMemory};

pub use tsfp_weapon_owner::transforms::WorldTransform;
trait WorldTransformOps: Sized {
    fn placement(self) -> Placement;
    fn multiply(self, right: Self) -> Self;
    fn reflection(axis: usize) -> Self;
}
impl WorldTransformOps for WorldTransform {
    fn placement(self) -> Placement {
        Placement {
            position: [self.matrix[12], self.matrix[13], self.matrix[14]],
            orientation: core::array::from_fn(|i| core::array::from_fn(|j| self.matrix[i * 4 + j])),
        }
    }
    fn multiply(self, right: Self) -> Self {
        Self {
            matrix: core::array::from_fn(|i| {
                let row = i / 4;
                let col = i % 4;
                ((self.matrix[row * 4] * right.matrix[col]
                    + self.matrix[row * 4 + 1] * right.matrix[4 + col])
                    + self.matrix[row * 4 + 2] * right.matrix[8 + col])
                    + self.matrix[row * 4 + 3] * right.matrix[12 + col]
            }),
        }
    }
    fn reflection(axis: usize) -> Self {
        let mut m = Self::IDENTITY;
        m.matrix[axis * 5] = -1.;
        m
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationMode {
    Identity,
    Yaw,
    Euler,
    ParentDependent,
    Cached,
}
#[derive(Clone, Copy, Debug)]
pub struct TransformState {
    pub rotation_mode: RotationMode,
    pub rotation_dirty: bool,
    pub reflected_rotation: bool,
    /// Original degree order: pitch (+C0), yaw (+70), roll (+C8).
    pub angles_degrees: [f32; 3],
    pub parent_uses_euler: bool,
    pub position: [f32; 3],
    pub cached_rotation: WorldTransform,
}
#[derive(Clone, Copy, Debug)]
pub struct ViewTransform {
    pub mirror_axis: Option<usize>,
    pub mirror_plane: f32,
    pub translate: bool,
}
impl TransformState {
    /// Fixed-tick/current-state producer. The caller supplies retail conversion
    /// and x87 precision policy. No timers, allocations or geometry callbacks.
    pub fn produce(
        &mut self,
        view: ViewTransform,
        radians: f32,
        round: impl Fn(f64) -> f64,
    ) -> WorldTransform {
        if self.rotation_mode == RotationMode::Identity {
            return WorldTransform::IDENTITY;
        }
        if self.rotation_dirty {
            self.rotation_dirty = false;
            match self.rotation_mode {
                RotationMode::Yaw | RotationMode::ParentDependent
                    if self.rotation_mode == RotationMode::Yaw || !self.parent_uses_euler =>
                {
                    let yaw = self.angles_degrees[1] * radians;
                    let s = sin(yaw as f64) as f32;
                    let c = cos(yaw as f64) as f32;
                    let mut m = WorldTransform::IDENTITY;
                    m.matrix[0] = c;
                    m.matrix[2] = -s;
                    m.matrix[8] = s;
                    m.matrix[10] = c;
                    self.cached_rotation = m;
                }
                RotationMode::Euler | RotationMode::ParentDependent | RotationMode::Yaw => {
                    let a = self.angles_degrees.map(|v| v * radians);
                    let [sx, sy, sz] = a.map(|v| sin(v as f64) as f32);
                    let [cx, cy, cz] = a.map(|v| cos(v as f64) as f32);
                    let sy_sx = sy * sx;
                    let cy_sx = cy * sx;
                    self.cached_rotation = WorldTransform {
                        matrix: [
                            (sy_sx * sz) + (cz * cy),
                            round(sz as f64 * cx as f64) as f32,
                            (0. - cz * sy) + (cy_sx * sz),
                            0.,
                            (0. - sz * cy) + (sy_sx * cz),
                            round(cz as f64 * cx as f64) as f32,
                            (cy_sx * cz) + (sz * sy),
                            0.,
                            sy * cx,
                            0. - sx,
                            cy * cx,
                            0.,
                            0.,
                            0.,
                            0.,
                            1.,
                        ],
                    };
                }
                _ => {}
            }
            if self.reflected_rotation {
                self.cached_rotation = WorldTransform::reflection(0).multiply(self.cached_rotation);
            }
        }
        let mut world = self.cached_rotation;
        if let Some(axis) = view.mirror_axis {
            world = world.multiply(WorldTransform::reflection(axis));
        }
        if view.translate {
            let mut position = self.position;
            if let Some(axis) = view.mirror_axis {
                let distance = position[axis] - view.mirror_plane;
                position[axis] = view.mirror_plane - distance;
            }
            world.matrix[12..15].copy_from_slice(&position);
        }
        world
    }
}

/// A lifetime key is supplied by the owning object pool, never inferred from
/// position or a distance threshold. Views can have different mirror matrices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct TransformId {
    pub slot: u32,
    pub generation: u64,
    pub view: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformHistory {
    pub id: TransformId,
    pub tick: u64,
    pub previous: WorldTransform,
    pub current: WorldTransform,
    pub discontinuity: bool,
}
/// Coherent, copied transform endpoints for a render owner. The host archive
/// exports this record so callers never read a separately linked Rust bank.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct TransformHistorySnapshot {
    pub id: TransformId,
    pub tick: u64,
    pub previous: WorldTransform,
    pub current: WorldTransform,
    /// C ABI boolean: 0 is continuous, nonzero snaps presentation to current.
    pub discontinuity: u32,
}
impl TransformHistorySnapshot {
    fn from_history(history: TransformHistory) -> Self {
        Self {
            id: history.id,
            tick: history.tick,
            previous: history.previous,
            current: history.current,
            discontinuity: u32::from(history.discontinuity),
        }
    }
}

/// Query key for an endpoint copy from the host-owned history bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct TransformHistoryRequest {
    pub reference: u32,
    pub view: u32,
    /// C ABI boolean: zero disables the mirror-plane translation.
    pub translate: u32,
}

/// One result from a batched endpoint copy. `found == 0` means the request was
/// valid but the bank has no entry; callers must ignore `history` in that case.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct TransformHistoryRead {
    pub request: TransformHistoryRequest,
    pub found: u32,
    pub history: TransformHistorySnapshot,
}

impl TransformHistoryRead {
    fn missing(request: TransformHistoryRequest) -> Self {
        Self {
            request,
            found: 0,
            history: TransformHistorySnapshot {
                id: TransformId {
                    slot: 0,
                    generation: 0,
                    view: 0,
                },
                tick: 0,
                previous: WorldTransform::IDENTITY,
                current: WorldTransform::IDENTITY,
                discontinuity: 1,
            },
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishError {
    OutOfOrder,
    ConflictingTick,
}
impl TransformHistory {
    pub fn new(id: TransformId, tick: u64, current: WorldTransform) -> Self {
        Self {
            id,
            tick,
            previous: current,
            current,
            discontinuity: true,
        }
    }
    pub fn publish(
        &mut self,
        id: TransformId,
        tick: u64,
        world: WorldTransform,
        cut: bool,
    ) -> Result<(), PublishError> {
        if tick < self.tick {
            return Err(PublishError::OutOfOrder);
        }
        if id == self.id && tick == self.tick {
            // Reentrant render requests cannot rotate tick history.
            if world != self.current {
                return Err(PublishError::ConflictingTick);
            }
            if cut {
                self.previous = world;
                self.discontinuity = true;
            }
            return Ok(());
        }
        let discontinuity = cut || id != self.id || self.tick.checked_add(1) != Some(tick);
        self.previous = if discontinuity { world } else { self.current };
        self.current = world;
        self.id = id;
        self.tick = tick;
        self.discontinuity = discontinuity;
        Ok(())
    }
    pub fn sample(&self, alpha: f32) -> Option<WorldTransform> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return None;
        }
        if self.discontinuity || alpha == 1. {
            return Some(self.current);
        }
        if alpha == 0. {
            return Some(self.previous);
        }
        let placement =
            TransformSampler.sample(self.previous.placement(), self.current.placement(), alpha);
        let mut world = self.current;
        for i in 0..3 {
            for j in 0..3 {
                world.matrix[i * 4 + j] = placement.orientation[i][j];
            }
        }
        world.matrix[12..15].copy_from_slice(&placement.position);
        Some(world)
    }
}

/// Polar-free rigid rotation interpolation uses the camera's shortest-arc
/// quaternion sampler. Mirror parity must be equal; parity changes are cuts.
pub struct TransformSampler;
impl PlacementSampler for TransformSampler {
    fn sample(&self, previous: Placement, current: Placement, alpha: f32) -> Placement {
        crate::camera::sample_object_placement(previous, current, alpha)
    }
}

pub fn selected() -> bool {
    unsafe extern "C" {
        fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
    }
    unsafe {
        let p = getenv(c"TSFP_NATIVE_OBJECT_TRANSFORM".as_ptr());
        !p.is_null() && p.read() == b'1' as core::ffi::c_char
    }
}
/// Original cdecl args: output, object, translate, view index. Preserves stack
/// and callee-saved registers. Volatile math-register residue is not a contract.
pub fn run<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    let out = m.read32(r.esp + 4);
    let object = m.read32(r.esp + 8);
    let translate = m.read32(r.esp + 12) != 0;
    let view = m.read32(r.esp + 16);
    let read = |offset| f32::from_bits(m.read32(object + offset));
    let mode = m.read8(object + 0x32);
    let parent = m.read32(object + 0x78);
    let parent_uses_euler = parent != 0 && m.read8(m.read32(parent) + 0x32) == 2;
    let flags = m.read32(object + 0x28);
    let mut state = TransformState {
        rotation_mode: match mode {
            0 => RotationMode::Identity,
            1 => RotationMode::Yaw,
            2 => RotationMode::Euler,
            3 => RotationMode::ParentDependent,
            _ => RotationMode::Cached,
        },
        rotation_dirty: flags & 0x1000 != 0,
        reflected_rotation: flags & 0x20000000 != 0,
        angles_degrees: [read(0xc0), read(0x70), read(0xc8)],
        parent_uses_euler,
        position: [read(0x64), read(0x68), read(0x6c)],
        cached_rotation: WorldTransform {
            matrix: core::array::from_fn(|i| read(0x274 + i as u32 * 4)),
        },
    };
    let variant = (m.read32(object + 0x2b4) >> ((view * 2) & 31)) & 3;
    let axis = match variant {
        1 => Some(0),
        2 => Some(1),
        3 => Some(2),
        _ => None,
    };
    let plane = if axis.is_some() && translate {
        read(0x2b8u32.wrapping_add(view.wrapping_sub(1).wrapping_mul(4)))
    } else {
        0.
    };
    let world = state.produce(
        ViewTransform {
            mirror_axis: axis,
            mirror_plane: plane,
            translate,
        },
        f32::from_bits(m.read32(0x475d0c)),
        |v| fp_pc(&r.fpu, v),
    );
    if mode != 0 && flags & 0x1000 != 0 {
        crate::leaf_wire::set_obj_word(m, object + 0x28, flags & !0x1000);
        for (i, v) in state.cached_rotation.matrix.iter().enumerate() {
            crate::leaf_wire::set_obj_word(m, object + 0x274 + i as u32 * 4, v.to_bits());
        }
    }
    for (i, v) in world.matrix.iter().enumerate() {
        m.write32(out + i as u32 * 4, v.to_bits());
    }
    r.eax = out;
    TRANSFORMS.with(|bank| {
        bank.publish(
            ReferenceKey {
                reference: object,
                view,
                translate,
            },
            m.read32(object + 8),
            m.read32(0x7de318) as u64,
            world,
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{Fpu, GuestCall, Xmm};
    use std::collections::BTreeMap;
    #[derive(Clone, Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, a: u32) -> u8 {
            self.0.get(&a).copied().unwrap_or(0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }
    struct Math(*mut Memory);
    impl GuestCall for Math {
        fn call(&mut self, va: u32, r: &mut ExtRegisters) {
            // Test bridge models the real CALL return slot; no stub math.
            unsafe {
                let m = &mut *self.0;
                r.esp -= 4;
                m.write32(r.esp, 0);
                if va == 0x1518f0 {
                    crate::c2r::engine_object::game_object_set_vec3_0x30(m, r);
                } else {
                    assert!(
                        crate::c2r::t1937_math::run(va, m, self, r),
                        "unknown math {va:x}"
                    );
                }
                r.esp += 4;
            }
        }
    }
    fn regs() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x9000,
            ebx: 17,
            ebp: 23,
            esi: 31,
            edi: 47,
            eflags: 0,
            fpu: Fpu {
                stack: [0.; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }
    #[test]
    fn object_transform_matches_real_translated_math() {
        let mut seed = 2334u32;
        for case in 0..4096u32 {
            let mut m = Memory::default();
            for i in 0..16 {
                m.write32(
                    0x4e7900 + i * 4,
                    WorldTransform::IDENTITY.matrix[i as usize].to_bits(),
                );
            }
            m.write32(0x475c78, 1f32.to_bits());
            m.write32(0x475d0c, (core::f32::consts::PI / 180.).to_bits());
            for (i, va) in [0xa165d, 0xa1672, 0xa16db, 0xa1744].into_iter().enumerate() {
                m.write32(0xa17ac + i as u32 * 4, va);
            }
            let object = 0x2000;
            let output = 0x4000;
            let view = case % 5;
            m.write32(0x9004, output);
            m.write32(0x9008, object);
            m.write32(0x900c, case % 2);
            m.write32(0x9010, view);
            m.write8(object + 0x32, (case % 5) as u8);
            m.write32(
                object + 0x28,
                (if case % 7 != 0 { 0x1000 } else { 0 })
                    | if case % 3 == 0 { 0x20000000 } else { 0 },
            );
            m.write32(object + 0x2b4, (case % 4) << (view * 2));
            m.write32(object + 0x78, if case % 3 == 0 { 0x5000 } else { 0 });
            m.write32(0x5000, 0x6000);
            m.write8(0x6032, 2);
            for offset in [0x64, 0x68, 0x6c, 0x70, 0xc0, 0xc8, 0x2b4 + view * 4] {
                if offset == 0x2b4 {
                    continue;
                }
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                m.write32(
                    object + offset,
                    (seed as i32 as f32 / i32::MAX as f32 * 360.).to_bits(),
                );
            }
            for i in 0..16 {
                m.write32(
                    object + 0x274 + i * 4,
                    WorldTransform::IDENTITY.matrix[i as usize].to_bits(),
                );
            }
            let mut n = m.clone();
            let mut a = regs();
            if case % 2 == 0 {
                a.fpu.control = 0x7f;
            }
            let mut b = a;
            let mut calls = Math(&mut m);
            crate::c2r::engine_object::game_object_build_matrix_for_player_by_state_field_0x2b4_and_mode_0x32(&mut m,&mut calls,&mut a);
            run(&mut n, &mut b);
            for start in [output, object + 0x274] {
                for i in 0..16 {
                    assert_eq!(
                        m.read32(start + i * 4),
                        n.read32(start + i * 4),
                        "case {case}, word {start:x}+{i}"
                    );
                }
            }
            assert_eq!(
                m.read32(object + 0x28),
                n.read32(object + 0x28),
                "flags {case}"
            );
            assert_eq!(
                (a.esp, a.ebx, a.ebp, a.esi, a.edi),
                (b.esp, b.ebx, b.ebp, b.esi, b.edi)
            );
        }
    }
    #[test]
    fn history_preserves_endpoints_reentrancy_cuts_and_lifetimes() {
        let id = TransformId {
            slot: 1,
            generation: 7,
            view: 0,
        };
        let mut world = WorldTransform::IDENTITY;
        let mut h = TransformHistory::new(id, 1, world);
        world.matrix[12] = 10.;
        h.publish(id, 2, world, false).unwrap();
        let frozen = h;
        assert_eq!(h.sample(0.).unwrap().matrix[12], 0.);
        assert_eq!(h.sample(0.5).unwrap().matrix[12], 5.);
        for _ in 0..10 {
            h.publish(id, 2, world, false).unwrap();
            h.sample(0.3);
            assert_eq!(h, frozen);
        }
        assert_eq!(
            h.publish(id, 1, world, false),
            Err(PublishError::OutOfOrder)
        );
        let mut conflicting = world;
        conflicting.matrix[12] = 11.;
        assert_eq!(
            h.publish(id, 2, conflicting, false),
            Err(PublishError::ConflictingTick)
        );
        assert_eq!(h, frozen);
        h.publish(
            TransformId {
                generation: 8,
                ..id
            },
            3,
            conflicting,
            false,
        )
        .unwrap();
        assert_eq!(h.sample(0.).unwrap(), conflicting);
        assert!(h.sample(f32::NAN).is_none());
        world.matrix[12] = 100.;
        h.publish(h.id, 4, world, true).unwrap();
        assert_eq!(h.sample(0.5).unwrap(), world);
    }
    #[test]
    fn sampler_retains_rigidity_short_arc_and_mirrors() {
        let mut a = WorldTransform::IDENTITY;
        let mut b = a;
        b.matrix[0] = -1.;
        b.matrix[10] = -1.;
        let mid = TransformSampler.sample(a.placement(), b.placement(), 0.5);
        for row in mid.orientation {
            assert!((row.iter().map(|v| v * v).sum::<f32>() - 1.).abs() < 0.000001);
        }
        a = WorldTransform::reflection(0);
        b = a.multiply(b);
        let mid = TransformSampler.sample(a.placement(), b.placement(), 0.5);
        for row in mid.orientation {
            assert!((row.iter().map(|v| v * v).sum::<f32>() - 1.).abs() < 0.000001);
        }
        assert_eq!(
            TransformSampler.sample(a.placement(), WorldTransform::IDENTITY.placement(), 0.5),
            WorldTransform::IDENTITY.placement()
        );
    }
}

/// Native object owner: typed simulation inputs persist here; tick endpoints
/// are committed once and may be sampled arbitrarily often by T2238/T2244.
/// One owner per object/view/lifetime keeps mirror variants separate.
pub struct TransformProducer {
    pub state: TransformState,
    pub view: ViewTransform,
    pub history: Option<TransformHistory>,
}
impl TransformProducer {
    pub fn tick(
        &mut self,
        id: TransformId,
        tick: u64,
        cut: bool,
        radians: f32,
        round: impl Fn(f64) -> f64,
    ) -> Result<WorldTransform, PublishError> {
        if let Some(history) = self.history {
            if tick < history.tick {
                return Err(PublishError::OutOfOrder);
            }
            if tick == history.tick && id == history.id {
                if cut {
                    let h = self.history.as_mut().unwrap();
                    h.previous = h.current;
                    h.discontinuity = true;
                }
                return Ok(history.current);
            }
        }
        let world = self.state.produce(self.view, radians, round);
        match &mut self.history {
            Some(history) => history.publish(id, tick, world, cut)?,
            None => self.history = Some(TransformHistory::new(id, tick, world)),
        }
        Ok(world)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ReferenceKey {
    reference: u32,
    view: u32,
    translate: bool,
}
struct PublishedTransform {
    object_index: u32,
    history: TransformHistory,
}
struct TransformBank {
    entries: alloc::collections::BTreeMap<ReferenceKey, PublishedTransform>,
    generation: u64,
}
impl TransformBank {
    const fn new() -> Self {
        Self {
            entries: alloc::collections::BTreeMap::new(),
            generation: 0,
        }
    }
    fn publish(&mut self, key: ReferenceKey, index: u32, tick: u64, world: WorldTransform) {
        let replacement = self
            .entries
            .get(&key)
            .is_none_or(|entry| entry.object_index != index || tick < entry.history.tick);
        if replacement {
            self.generation = self
                .generation
                .checked_add(1)
                .expect("transform generation exhausted");
            let id = TransformId {
                slot: index,
                generation: self.generation,
                view: key.view,
            };
            self.entries.insert(
                key,
                PublishedTransform {
                    object_index: index,
                    history: TransformHistory::new(id, tick, world),
                },
            );
        } else {
            let entry = self.entries.get_mut(&key).unwrap();
            if tick == entry.history.tick {
                // Multiple producer calls in one update can revise the current
                // endpoint. The previous completed tick is never rotated here.
                entry.history.current = world;
                if entry.history.discontinuity {
                    entry.history.previous = world;
                }
            } else {
                entry
                    .history
                    .publish(entry.history.id, tick, world, false)
                    .unwrap();
            }
        }
    }

    fn copy_histories(
        &self,
        requests: &[TransformHistoryRequest],
        output: &mut [TransformHistoryRead],
    ) {
        debug_assert_eq!(requests.len(), output.len());
        for (request, result) in requests.iter().zip(output) {
            let key = ReferenceKey {
                reference: request.reference,
                view: request.view,
                translate: request.translate != 0,
            };
            *result = self.entries.get(&key).map_or_else(
                || TransformHistoryRead::missing(*request),
                |entry| TransformHistoryRead {
                    request: *request,
                    found: 1,
                    history: TransformHistorySnapshot::from_history(entry.history),
                },
            );
        }
    }

    fn retire(&mut self, reference: u32) {
        self.entries.retain(|key, _| key.reference != reference);
    }
}
struct TransformStore {
    held: core::sync::atomic::AtomicBool,
    bank: core::cell::UnsafeCell<TransformBank>,
}
// SAFETY: the guard copies endpoint packets; no borrowed data or callback escapes.
unsafe impl Sync for TransformStore {}
impl TransformStore {
    const fn new() -> Self {
        Self {
            held: core::sync::atomic::AtomicBool::new(false),
            bank: core::cell::UnsafeCell::new(TransformBank::new()),
        }
    }
    fn with<R>(&self, f: impl FnOnce(&mut TransformBank) -> R) -> R {
        use core::sync::atomic::Ordering;
        while self
            .held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        struct Unlock<'a>(&'a core::sync::atomic::AtomicBool);
        impl Drop for Unlock<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _unlock = Unlock(&self.held);
        unsafe { f(&mut *self.bank.get()) }
    }
}
static TRANSFORMS: TransformStore = TransformStore::new();
/// Opaque reference mapping is a compatibility inlet. Scene/object lifecycle
/// MUST retire a reference on despawn/reuse, and cut it on teleport/respawn.
/// Sampling never reads guest memory. Host archive is the single store owner.
pub fn transform_for_reference(
    reference: u32,
    view: u32,
    translate: bool,
) -> Option<TransformHistory> {
    TRANSFORMS.with(|bank| {
        bank.entries
            .get(&ReferenceKey {
                reference,
                view,
                translate,
            })
            .map(|entry| entry.history)
    })
}
pub fn retire_transform(reference: u32) {
    TRANSFORMS.with(|bank| bank.retire(reference));
}
pub fn cut_transform(reference: u32) {
    TRANSFORMS.with(|bank| {
        for (key, entry) in bank.entries.iter_mut() {
            if key.reference == reference {
                entry.history.previous = entry.history.current;
                entry.history.discontinuity = true;
            }
        }
    });
}
pub fn reset_transforms() {
    TRANSFORMS.with(|bank| bank.entries.clear());
}
/// Native callers hold a lifetime id. Retired generations cannot sample new
/// occupants, even if a guest reference or scene slot is recycled.
pub fn sample_transform(id: TransformId, alpha: f32) -> Option<WorldTransform> {
    let history = TRANSFORMS.with(|bank| {
        bank.entries
            .values()
            .find(|entry| entry.history.id == id)
            .map(|entry| entry.history)
    });
    history?.sample(alpha)
}
/// Unique host-archive ABI (matching camera bank ownership). The renderer must
/// bridge to this archive, not sample a separately linked Cargo crate's store.
#[cfg(not(feature = "std"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_object_transform_sample(
    reference: u32,
    view: u32,
    translate: u32,
    alpha: f32,
    out: *mut WorldTransform,
) -> u32 {
    if out.is_null() {
        return 0;
    }
    let Some(world) =
        transform_for_reference(reference, view, translate != 0).and_then(|h| h.sample(alpha))
    else {
        return 0;
    };
    // SAFETY: caller supplies writable repr(C) output storage.
    unsafe {
        out.write(world);
    }
    1
}

/// Copy unsampled endpoint history from the single host-owned bank. A renderer
/// can build a reusable ScenePair from this packet and sample it repeatedly.
#[cfg(not(feature = "std"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_object_transform_history(
    reference: u32,
    view: u32,
    translate: u32,
    out: *mut TransformHistorySnapshot,
) -> u32 {
    if out.is_null() {
        return 0;
    }
    let Some(history) = transform_for_reference(reference, view, translate != 0) else {
        return 0;
    };
    // SAFETY: caller supplies writable repr(C) output storage.
    unsafe {
        out.write(TransformHistorySnapshot::from_history(history));
    }
    1
}

/// Copy many transform histories under one bank guard so a whole render scene
/// observes one coherent publication instant. Results preserve request order;
/// missing keys have `found == 0`. A renderer maps each echoed request to its
/// own stable scene identity, then reuses the copied endpoints across display
/// samples without holding the bank lock.
///
/// # Safety
/// For nonzero `count`, `requests` must point to `count` initialized requests
/// and `out` to `capacity` writable results. The ranges must not overlap.
#[cfg(not(feature = "std"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_object_transform_histories(
    requests: *const TransformHistoryRequest,
    count: u32,
    out: *mut TransformHistoryRead,
    capacity: u32,
) -> u32 {
    if count > capacity || (count != 0 && (requests.is_null() || out.is_null())) {
        return 0;
    }
    if count == 0 {
        return 1;
    }
    // SAFETY: caller guarantees initialized, disjoint request and writable output ranges.
    let requests = unsafe { core::slice::from_raw_parts(requests, count as usize) };
    // SAFETY: count <= capacity and caller supplies `capacity` writable elements.
    let output = unsafe { core::slice::from_raw_parts_mut(out, count as usize) };
    TRANSFORMS.with(|bank| bank.copy_histories(requests, output));
    1
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    #[test]
    fn bank_revisions_preserve_completed_tick_and_retire_invalidates_handles() {
        let mut bank = TransformBank::new();
        let key = ReferenceKey {
            reference: 0x2000,
            view: 1,
            translate: true,
        };
        let mut world = WorldTransform::IDENTITY;
        bank.publish(key, 7, 10, world);
        world.matrix[12] = 10.;
        bank.publish(key, 7, 11, world);
        let id = bank.entries[&key].history.id;
        world.matrix[12] = 20.;
        bank.publish(key, 7, 11, world);
        assert_eq!(bank.entries[&key].history.previous.matrix[12], 0.);
        assert_eq!(
            bank.entries[&key].history.sample(0.5).unwrap().matrix[12],
            10.
        );
        let other = ReferenceKey { view: 2, ..key };
        bank.publish(other, 7, 11, world);
        assert_ne!(bank.entries[&other].history.id, id);
        bank.entries.remove(&key);
        bank.publish(key, 7, 12, world);
        assert_ne!(bank.entries[&key].history.id, id);
        assert_eq!(bank.entries[&key].history.sample(0.).unwrap(), world);
        // Rewind/load and index change create new lifetime mappings and cuts.
        let id = bank.entries[&key].history.id;
        bank.publish(key, 7, 1, world);
        assert_ne!(bank.entries[&key].history.id, id);
        let id = bank.entries[&key].history.id;
        bank.publish(key, 8, 2, world);
        assert_ne!(bank.entries[&key].history.id, id);
    }

    #[test]
    fn history_snapshot_copies_both_endpoints_and_lifetime_metadata() {
        let mut previous = WorldTransform::IDENTITY;
        previous.matrix[12] = 3.0;
        let mut current = WorldTransform::IDENTITY;
        current.matrix[12] = 9.0;
        let history = TransformHistory {
            id: TransformId {
                slot: 7,
                generation: 42,
                view: 3,
            },
            tick: 101,
            previous,
            current,
            discontinuity: false,
        };
        let snapshot = TransformHistorySnapshot::from_history(history);
        assert_eq!(snapshot.id, history.id);
        assert_eq!(snapshot.tick, 101);
        assert_eq!(snapshot.previous, previous);
        assert_eq!(snapshot.current, current);
        assert_eq!(snapshot.discontinuity, 0);
    }

    #[test]
    fn retirement_removes_every_view_and_translation_for_one_reference() {
        let mut bank = TransformBank::new();
        let reference = 0x2000;
        for (view, translate) in [(0, false), (1, true), (4, false)] {
            bank.publish(
                ReferenceKey {
                    reference,
                    view,
                    translate,
                },
                7,
                10,
                WorldTransform::IDENTITY,
            );
        }
        let other = ReferenceKey {
            reference: 0x3000,
            view: 0,
            translate: false,
        };
        bank.publish(other, 8, 10, WorldTransform::IDENTITY);

        bank.retire(reference);

        assert!(bank.entries.keys().all(|key| key.reference != reference));
        assert!(bank.entries.contains_key(&other));
    }

    #[test]
    fn batch_copy_keeps_scene_order_and_one_tick_of_endpoint_history() {
        let mut bank = TransformBank::new();
        let request_a = TransformHistoryRequest {
            reference: 0x2000,
            view: 1,
            translate: 1,
        };
        let request_b = TransformHistoryRequest {
            reference: 0x3000,
            view: 4,
            translate: 0,
        };
        let mut first = WorldTransform::IDENTITY;
        first.matrix[12] = 3.0;
        let mut second = WorldTransform::IDENTITY;
        second.matrix[12] = 9.0;
        bank.publish(
            ReferenceKey {
                reference: request_a.reference,
                view: request_a.view,
                translate: true,
            },
            7,
            10,
            first,
        );
        bank.publish(
            ReferenceKey {
                reference: request_a.reference,
                view: request_a.view,
                translate: true,
            },
            7,
            11,
            second,
        );
        bank.publish(
            ReferenceKey {
                reference: request_b.reference,
                view: request_b.view,
                translate: false,
            },
            8,
            11,
            second,
        );

        let requests = [
            request_b,
            request_a,
            TransformHistoryRequest {
                reference: 4,
                ..request_a
            },
        ];
        let mut output = requests.map(TransformHistoryRead::missing);
        bank.copy_histories(&requests, &mut output);
        assert_eq!(output.map(|read| read.request), requests);
        assert_eq!(output[0].found, 1);
        assert_eq!(output[0].history.id.view, 4);
        assert_eq!(output[1].found, 1);
        assert_eq!(output[1].history.tick, 11);
        assert_eq!(output[1].history.previous.matrix[12], 3.0);
        assert_eq!(output[1].history.current.matrix[12], 9.0);
        assert_eq!(output[2].found, 0);
    }
    #[test]
    fn native_producer_rejects_old_ticks_before_mutating_simulation() {
        let mut producer = TransformProducer {
            state: TransformState {
                rotation_mode: RotationMode::Yaw,
                rotation_dirty: true,
                reflected_rotation: false,
                angles_degrees: [0., 90., 0.],
                parent_uses_euler: false,
                position: [1., 2., 3.],
                cached_rotation: WorldTransform::IDENTITY,
            },
            view: ViewTransform {
                mirror_axis: None,
                mirror_plane: 0.,
                translate: true,
            },
            history: None,
        };
        let id = TransformId {
            slot: 1,
            generation: 1,
            view: 1,
        };
        let world = producer
            .tick(id, 4, false, core::f32::consts::PI / 180., |v| v)
            .unwrap();
        producer.state.rotation_dirty = true;
        producer.state.position = [4.; 3];
        assert_eq!(
            producer.tick(id, 3, false, 1., |v| v),
            Err(PublishError::OutOfOrder)
        );
        assert!(producer.state.rotation_dirty);
        assert_eq!(producer.tick(id, 4, false, 1., |v| v).unwrap(), world);
        producer
            .tick(id, 5, false, core::f32::consts::PI / 180., |v| v)
            .unwrap();
        let h = producer.history.unwrap();
        assert_eq!(h.previous, world);
        producer.tick(id, 5, true, 1., |v| v).unwrap();
        assert!(producer.history.unwrap().discontinuity);
    }
}
