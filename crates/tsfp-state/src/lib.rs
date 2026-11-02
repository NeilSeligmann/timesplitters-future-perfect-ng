// SPDX-License-Identifier: GPL-3.0-or-later
//! T2058 (S3-1 of T2051): typed views over guest memory. Design and the mirror, own and retire
//! stages: docs/t-state-views.md and docs/t-s3-state-design.md. A view never owns state: every
//! access goes through `GuestMemory`, so guest addresses and byte layout stay exactly what the
//! lifted code sees.
#![no_std]

extern crate alloc;

// The same trait source as crates/tsfp-ports, so ports and views agree on one definition.
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
mod leaf;
pub use leaf::GuestMemory;

pub mod actor_health;
pub mod character;
pub mod game_state;
pub mod generated_tables;
pub mod generated_views;
pub mod inventory;
/// Shared typed inventory surface imported by standalone host lane adapters.
pub mod weapon_hands_inventory {
    pub use crate::inventory::{AMMO_TYPES, ENTRY_COUNT, FLAG_X2, Inventory, WeaponEntries};
}
pub mod mapedit;
pub mod menu;
pub mod mission_catalog;
pub mod missions;
pub mod missions_owned;
pub mod modes;
pub mod object_pool;
pub mod pad_audio;
pub mod player;
pub mod player_interaction;
/// Rust-owned profile catalogue and save-tracking lifecycle coordinator (T3420).
pub mod profile_lifecycle;
pub mod profile_save_state;
/// Rust-owned profile slot selection and profile/category/mission eligibility (T3420).
pub mod profile_state;
/// Rust-owned projectile descriptor pool and one-tick continuous leases (T1682).
pub mod projectile_pool;
pub mod save_text;
pub mod scene_membership;
pub mod sha256;
pub mod snapshots;
pub mod story_flow;
pub mod tables;
pub mod terminals;
pub mod unlock_requirements;
pub mod values;
pub mod weapon_animation;
pub mod weapon_cycle;
pub mod weapon_hands;
pub mod weapon_interaction;
pub mod weapon_inventory;
pub mod weapon_payload;
pub mod weapon_projectiles;
pub mod weapon_runtime;
pub mod weapon_select;
pub mod weapon_slot;
pub mod xbe;

pub mod local_players;

/// Opt-in end-of-simulation-tick render snapshots (T2240).
pub mod transforms;
