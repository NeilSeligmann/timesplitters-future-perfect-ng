// SPDX-License-Identifier: GPL-3.0-or-later
//! T1831: typed loadout, resource and weapon-slot lifecycle policies.
//! Tier 1 translated-reference differentials on fabricated inputs; no host admission.
//! Layouts, attachment points and the excluded animation root: docs/t1831-typed-weapons.md.
use crate::weapon_select::SelectionPreset;

/// Runtime object identity; the adapter resolves it, never the policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectId(pub u32);

/// The two preserved slot values have no established semantic names yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotContinuity {
    pub word_3c: u32,
    pub word_3cc: u32,
}

/// Owned projection used by both teardown operations. The full reset also discards
/// all opaque slot payload; the adapter must apply that explicit reset outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotState {
    pub record: i32,
    pub children: [Option<ObjectId>; 2],
    pub continuity: SlotContinuity,
}
impl Default for SlotState {
    fn default() -> Self {
        Self {
            record: -1,
            children: [None; 2],
            continuity: SlotContinuity::default(),
        }
    }
}

/// Effects refresh this owned projection if neighbours change the shared slot.
/// Destroy is followed by a fresh read of the next child; callbacks precede both reads.
pub trait SlotEffects {
    fn remove_membership(&mut self, slot: &mut SlotState, object: ObjectId);
    fn destroy(&mut self, slot: &mut SlotState, object: ObjectId);
    /// Publish the clear before another neighbour can refresh the projection.
    fn child_cleared(&mut self, slot: &SlotState, hand: usize);
    fn record_callback(&mut self, slot: &mut SlotState, record: i32, owner: ObjectId);
}

/// 0x1B7AF0. Caller resolves owner type >=0 and its table membership >=0.
/// The result orders a full 0x3D8-byte clear, record and field C8 = -1, then
/// continuity restoration. This is not the partial child-only cleanup below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotReset {
    pub continuity: SlotContinuity,
}
pub fn reset_slot<E: SlotEffects>(
    slot: &mut SlotState,
    remove_membership: bool,
    effects: &mut E,
) -> SlotReset {
    if remove_membership {
        if let Some(child) = slot.children[0] {
            effects.remove_membership(slot, child);
        }
    }
    if let Some(child) = slot.children[0] {
        effects.destroy(slot, child);
    }
    if let Some(child) = slot.children[1] {
        effects.destroy(slot, child);
    }
    let continuity = slot.continuity;
    *slot = SlotState {
        continuity,
        ..SlotState::default()
    };
    SlotReset { continuity }
}

/// 0x1B7C50. If the slot argument is absent, the owner supplies the callback
/// record; child cleanup still operates on the supplied slot projection. Adapters
/// must model the original null-address child reads explicitly, not silently skip them.
/// `callback_present` resolves the record's callback-6 entry before entering here.
pub fn release_children<E: SlotEffects>(
    slot: &mut SlotState,
    callback_record: i32,
    callback_present: bool,
    owner: ObjectId,
    effects: &mut E,
) {
    if callback_record >= 0 && callback_present {
        effects.record_callback(slot, callback_record, owner);
    }
    for hand in 0..2 {
        if let Some(child) = slot.children[hand] {
            effects.destroy(slot, child);
            slot.children[hand] = None;
            effects.child_cleared(slot, hand);
        }
    }
}

/// Signed counters and unsigned capacity are intentionally different encodings.
#[derive(Clone, Copy, Debug, Default)]
pub struct PoolAvailability {
    pub free: i16,
    pub active: i16,
    pub total: i16,
    pub capacity: u16,
}
impl PoolAvailability {
    /// 0x16C020 uses only the low byte of its mode argument.
    pub fn available(self, mode: u8) -> bool {
        match mode {
            0 => self.free > 0,
            1 => (self.active as i32) < self.capacity as i32 + 0x58,
            2 => self.total > self.active,
            _ => false,
        }
    }
}

pub trait LoadoutSource {
    /// A fresh lookup: the original calls 0x6BD30 a second time on the entry path.
    fn count(&mut self) -> i32;
    /// Resolve after the count lookup, which may rebuild the mode-owned list.
    fn mode_flags(&self) -> u32;
    fn level(&self) -> u32;
    /// Signed index intentionally retained. The original lacks a lower-bound check;
    /// the adapter must resolve negative indices or reject them before dispatch.
    fn entry(&mut self, index: i32) -> u32;
}
/// 0x6BF10: ordinary kinds return the caller's index unchanged, special kinds
/// resolve the loadout and convert a zero entry or denied index to -1.
pub fn loadout_entry<S: LoadoutSource>(kind: u32, index: i32, source: &mut S) -> u32 {
    if !matches!(kind, 2 | 0x40 | 0x800) {
        return index as u32;
    }
    let count = source.count();
    if index >= count
        || (index == 0 && source.mode_flags() & 0x400 != 0 && kind == 2 && source.level() != 0x64)
    {
        return u32::MAX;
    }
    let entry = source.entry(index);
    if entry == 0 { u32::MAX } else { entry }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponCallback {
    Primary,
    Secondary,
}
pub trait CallbackRegistry {
    fn register(&mut self, mask: u32, slot: u32, callback: WeaponCallback);
}
/// 0xAE8B0. Adapter resolves Primary=0xAC940 and Secondary=0xA92F0.
pub fn register_callbacks<R: CallbackRegistry>(registry: &mut R) {
    registry.register(0x80, 0, WeaponCallback::Primary);
    registry.register(0x80, 1, WeaponCallback::Secondary);
}

/// Resource enumeration stays live across calls, rather than snapshotting a table
/// that a neighbour could change. There are seven original composite templates.
pub trait CompositeResources {
    fn part_count(&mut self, template: usize) -> i32;
    fn resource(&mut self, template: usize, part: usize) -> u32;
    fn preload(&mut self, resource: u32);
}
/// 0x351210. Does not share the selection gate or invoke equip.
pub fn preload_resources<R: CompositeResources>(mode_flags: u32, resources: &mut R) {
    if mode_flags & 0x2000_0000 != 0 {
        return;
    }
    for template in 0..7 {
        let mut part = 0;
        let mut count = resources.part_count(template);
        while (part as i32) < count {
            let resource = resources.resource(template, part);
            resources.preload(resource);
            part += 1;
            count = resources.part_count(template);
        }
    }
}

/// Reuses T2193's default-reference selector at this neighbouring boundary.
/// Each root may be selected independently by its caller. No host registration is implied.
pub fn dispatch<T>(
    preset: SelectionPreset,
    reference: impl FnOnce() -> T,
    readable: impl FnOnce() -> T,
) -> T {
    match preset {
        SelectionPreset::Reference => reference(),
        SelectionPreset::ReadableOriginal => readable(),
    }
}
