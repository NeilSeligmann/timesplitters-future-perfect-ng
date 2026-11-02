//! Resident-PAK gated id-to-clip binding, original 0x1CA2F0.
//! The level and streamed zone determine residency; this API preserves the original
//! ordered gate checks instead of treating every global animation table as loaded.
use std::collections::{BTreeMap, BTreeSet};

use crate::resource_catalog::{
    AnimationResourceCatalog, CatalogError, ClipSlotHandle, ResourceGeneration, ResourceOwnerId,
    RetiredSlot,
};

#[derive(Clone, Copy, Debug)]
pub struct BindingEntry {
    pub id: u16,
    pub policy: [u32; 2],
    pub name: &'static str,
}
#[derive(Clone, Copy, Debug)]
pub struct ResidentSet {
    pub pak: &'static str,
    pub base: &'static str,
    /// Original entry-table address, useful for trace comparison.
    pub table: u32,
    pub entries: &'static [BindingEntry],
}
/// Stream-table row (+0 tag, +4 PAK, +8 callback, +0xC zone mask).
#[derive(Clone, Copy, Debug)]
pub struct StreamedSet {
    pub tag: u32,
    pub mask: u32,
    pub set: ResidentSet,
}

#[path = "binding_tables.rs"]
mod tables;
pub use tables::{RESIDENT_SETS, STREAMED_SETS};

pub trait BindingWorld {
    /// 0x62260(path) != -1; consult actual open archives, not ISO file existence.
    fn resident(&mut self, pak: &str) -> bool;
    /// Run 0x57F30(path, id, 0), then read the signed slot of id from
    /// 0x7E03C0. The original ignores the function return register.
    fn load_clip(&mut self, path: &str, id: u16) -> i16;

    /// Observe the binding after the load callback. Original 0x5B980 rereads
    /// the row ID after 0x57F30 returns, then sign-extends the slot-table word
    /// for that reread ID before publishing its two policy words. Static
    /// binding tables use this identity default; a mutable native table adapter
    /// overrides it to return the post-callback ID and slot.
    fn binding_after_load(
        &mut self,
        _entry_index: usize,
        requested_id: u16,
        observed_slot: i16,
    ) -> (u16, i16) {
        (requested_id, observed_slot)
    }
}

/// Requested path and resulting id slot. A failed load can retain an older slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundClip {
    pub path: String,
    pub slot: i16,
}

#[derive(Clone, Debug)]
struct IdPublication {
    owner: Option<ResourceOwnerId>,
    id: u16,
    binding: BoundClip,
}

#[derive(Default, Debug)]
pub struct LevelBindings {
    pub ids: BTreeMap<u16, BoundClip>,
    pub policy: BTreeMap<i16, [u32; 2]>,
    id_publications: Vec<IdPublication>,
}
impl LevelBindings {
    fn publish_id(&mut self, owner: Option<ResourceOwnerId>, id: u16, binding: BoundClip) {
        self.id_publications
            .retain(|publication| publication.owner != owner || publication.id != id);
        self.id_publications
            .push(IdPublication { owner, id, binding });
        self.refresh_id(id);
    }

    fn refresh_id(&mut self, id: u16) {
        if let Some(publication) = self
            .id_publications
            .iter()
            .rev()
            .find(|publication| publication.id == id)
        {
            self.ids.insert(id, publication.binding.clone());
        } else {
            self.ids.remove(&id);
        }
    }

    /// Bind the callback of an archive the streaming state machine just installed.
    /// Returns false for unknown paths; residency and stream scheduling belong to
    /// the caller (original 0x61910 invokes callback at 0x61A7F).
    pub fn load_streamed(&mut self, pak: &str, world: &mut impl BindingWorld) -> bool {
        let Some(row) = STREAMED_SETS.iter().find(|row| row.set.pak == pak) else {
            return false;
        };
        self.load_set(row.set.base, row.set.entries, world);
        true
    }

    /// Original 0x5B980 loop. Later requests overwrite the id even if a loader
    /// returns a negative slot; success semantics belong to the clip loader.
    pub fn load_set(
        &mut self,
        base: &str,
        entries: &[BindingEntry],
        world: &mut impl BindingWorld,
    ) {
        for (entry_index, entry) in entries.iter().enumerate() {
            let path = format!("{base}{}.raw", entry.name);
            let observed_slot = world.load_clip(&path, entry.id);
            let (id, slot) = world.binding_after_load(entry_index, entry.id, observed_slot);
            self.publish_id(None, id, BoundClip { path, slot });
            self.policy.insert(slot, entry.policy);
        }
    }

    /// Load one ordered set and publish its observed signed slot and final
    /// per-slot policy into the Rust-owned resource catalog. The catalog keeps
    /// any already-owned clip payload (`None` is not a replacement), so this
    /// joins the legacy ID/policy view to native resource storage without
    /// inventing decode, archive-residency, or release behavior.
    pub fn load_set_into_catalog<T>(
        &mut self,
        base: &str,
        entries: &[BindingEntry],
        world: &mut impl BindingWorld,
        catalog: &mut AnimationResourceCatalog<T>,
    ) -> Result<Vec<ClipSlotHandle>, CatalogError> {
        self.load_set_into_catalog_with_owner(None, base, entries, world, catalog)
    }

    /// Load one set and attach every observed slot to the caller's Rust owner.
    /// The owner represents the caller's chosen archive/level lifetime; REA has
    /// not established the original shared-release policy.
    pub fn load_set_for_owner_into_catalog<T>(
        &mut self,
        owner: ResourceOwnerId,
        base: &str,
        entries: &[BindingEntry],
        world: &mut impl BindingWorld,
        catalog: &mut AnimationResourceCatalog<T>,
    ) -> Result<Vec<ClipSlotHandle>, CatalogError> {
        self.load_set_into_catalog_with_owner(Some(owner), base, entries, world, catalog)
    }

    fn load_set_into_catalog_with_owner<T>(
        &mut self,
        owner: Option<ResourceOwnerId>,
        base: &str,
        entries: &[BindingEntry],
        world: &mut impl BindingWorld,
        catalog: &mut AnimationResourceCatalog<T>,
    ) -> Result<Vec<ClipSlotHandle>, CatalogError> {
        if let Some(owner) = owner {
            catalog.validate_owner(owner)?;
        }
        let mut handles = Vec::with_capacity(entries.len());
        for (entry_index, entry) in entries.iter().enumerate() {
            let path = format!("{base}{}.raw", entry.name);
            let observed_slot = world.load_clip(&path, entry.id);
            let (id, slot) = world.binding_after_load(entry_index, entry.id, observed_slot);
            let handle = match owner {
                Some(owner) => catalog.publish_for_owner(owner, id, slot, entry.policy, None)?,
                None => catalog.publish(id, slot, entry.policy, None)?,
            };
            self.publish_id(owner, id, BoundClip { path, slot });
            self.policy.insert(slot, entry.policy);
            handles.push(handle);
        }
        Ok(handles)
    }

    /// Apply all ordered resident gates and attach each accepted set to the
    /// owner chosen by the caller. The callback is asked only after a set is
    /// confirmed resident, and the archive checks keep their original order.
    pub fn load_resident_for_owners_into_catalog<T>(
        &mut self,
        world: &mut impl BindingWorld,
        catalog: &mut AnimationResourceCatalog<T>,
        mut owner_for: impl FnMut(&ResidentSet) -> ResourceOwnerId,
    ) -> Result<Vec<ClipSlotHandle>, CatalogError> {
        let mut handles = Vec::new();
        for set in RESIDENT_SETS {
            if world.resident(set.pak) {
                handles.extend(self.load_set_for_owner_into_catalog(
                    owner_for(set),
                    set.base,
                    set.entries,
                    world,
                    catalog,
                )?);
            }
        }
        Ok(handles)
    }

    /// Publish the animation set for an already-installed streamed archive.
    /// Unknown PAKs stay with the host's other stream callback handlers.
    pub fn load_streamed_for_owner_into_catalog<T>(
        &mut self,
        pak: &str,
        owner: ResourceOwnerId,
        world: &mut impl BindingWorld,
        catalog: &mut AnimationResourceCatalog<T>,
    ) -> Result<bool, CatalogError> {
        let Some(row) = STREAMED_SETS.iter().find(|row| row.set.pak == pak) else {
            return Ok(false);
        };
        self.load_set_for_owner_into_catalog(owner, row.set.base, row.set.entries, world, catalog)?;
        Ok(true)
    }

    /// Native counterpart to `installed_stream`: callback-zero rows and
    /// non-animation callbacks remain outside this binding/catalog seam.
    pub fn installed_stream_for_owner_into_catalog<T>(
        &mut self,
        row: StreamArchive,
        owner: ResourceOwnerId,
        world: &mut impl BindingWorld,
        catalog: &mut AnimationResourceCatalog<T>,
    ) -> Result<bool, CatalogError> {
        if row.callback == 0 {
            return Ok(false);
        }
        match row.pak {
            Some(pak) => self.load_streamed_for_owner_into_catalog(pak, owner, world, catalog),
            None => Ok(false),
        }
    }

    /// Release the caller-selected owner and clear legacy binding aliases only
    /// for slots the catalog actually retires. Shared slots remain addressable
    /// while another Rust owner still holds them.
    pub fn release_catalog_owner<T>(
        &mut self,
        owner: ResourceOwnerId,
        catalog: &mut AnimationResourceCatalog<T>,
    ) -> Result<Vec<RetiredSlot<T>>, CatalogError> {
        let retired = catalog.release_owner(owner)?;
        let retired_slots: BTreeSet<_> = retired.iter().map(|slot| slot.slot).collect();
        let affected_ids: BTreeSet<_> = self
            .id_publications
            .iter()
            .filter_map(|publication| {
                (publication.owner == Some(owner)
                    || retired_slots.contains(&publication.binding.slot))
                .then_some(publication.id)
            })
            .collect();
        self.id_publications.retain(|publication| {
            publication.owner != Some(owner) && !retired_slots.contains(&publication.binding.slot)
        });
        for slot in &retired {
            self.policy.remove(&slot.slot);
        }
        for id in affected_ids {
            self.refresh_id(id);
        }
        Ok(retired)
    }

    /// Start a new Rust resource generation and clear the matching legacy
    /// binding view as one operation. The catalog rejects this while any Rust
    /// owner is active; on rejection neither the catalog nor these aliases
    /// change. REA has not established that the original engine uses this
    /// generation policy.
    pub fn begin_resource_generation<T>(
        &mut self,
        catalog: &mut AnimationResourceCatalog<T>,
    ) -> Result<ResourceGeneration, CatalogError> {
        let generation = catalog.begin_generation()?;
        self.ids.clear();
        self.policy.clear();
        self.id_publications.clear();
        Ok(generation)
    }

    /// Replay all 103 ordered PAK residency gates of 0x1CA2F0.
    /// Stream callbacks call load_set separately after the streaming loader installs
    /// the selected archive, preserving their actual later execution order.
    pub fn load_resident(&mut self, world: &mut impl BindingWorld) {
        for set in RESIDENT_SETS {
            if world.resident(set.pak) {
                self.load_set(set.base, set.entries, world);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ZoneRow {
    pub tag: u32,
    pub mask: u32,
}
/// Original 0x618C0, CL=zone: channel 0 before channel 1, first matching row.
/// x86 masks a 32-bit shift count to five bits, including CL values above 31.
/// A tag is a stream request identifier; it is not an animation id or a class mask.
pub fn zone_tag(channels: [&[ZoneRow]; 2], zone: u8) -> Option<u32> {
    let bit = 1u32.wrapping_shl(u32::from(zone));
    channels
        .into_iter()
        .flatten()
        .find(|row| row.mask & bit != 0)
        .map(|row| row.tag)
}

/// A row of the level's registered 24-byte stream table. Property and cutscene
/// rows belong here too: choosing only animation rows can change tag selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamArchive {
    pub tag: u32,
    pub mask: u32,
    pub pak: Option<&'static str>,
    pub callback: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamPlan {
    pub tag: u32,
    /// First row with that tag in each channel, in installation order.
    pub channels: [Option<StreamArchive>; 2],
}
/// Compose zone selection0x618C0 and tag lookup0x61C10. This is a request plan;
/// it does not open an archive or invoke callbacks before installation completes.
pub fn plan_stream(channels: [&[StreamArchive]; 2], zone: u8) -> Option<StreamPlan> {
    let bit = 1u32.wrapping_shl(u32::from(zone));
    let tag = channels
        .iter()
        .flat_map(|c| c.iter())
        .find(|r| r.mask & bit != 0)?
        .tag;
    Some(StreamPlan {
        tag,
        channels: channels.map(|c| {
            c.iter()
                .find(|r| r.tag == tag)
                .copied()
                .filter(|r| r.tag != 0x454D5059)
        }),
    })
}
impl LevelBindings {
    /// Caller-facing level entry: use actual archives installed by the level
    /// loader as BindingWorld residency, then replay the original resident gates.
    pub fn begin_level(world: &mut impl BindingWorld) -> Self {
        let mut bindings = Self::default();
        bindings.load_resident(world);
        bindings
    }
    /// Invoke after the stream controller installs this exact row via0x62040.
    /// Returns true for an animation callback handled here. Other callbacks
    /// (properties/cutscenes) remain the host's responsibility.
    pub fn installed_stream(&mut self, row: StreamArchive, world: &mut impl BindingWorld) -> bool {
        if row.callback == 0 {
            return false;
        }
        row.pak.is_some_and(|pak| self.load_streamed(pak, world))
    }
}
