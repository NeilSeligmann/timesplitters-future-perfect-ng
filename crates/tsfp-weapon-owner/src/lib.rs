// SPDX-License-Identifier: GPL-3.0-or-later
//! T2411 shared weapon owner types using the production wire identities.
//! No global owner is created here. The runtime installs exactly one store.
#![no_std]
extern crate alloc;

pub use tsfp_port_wire::leaf_wire::object_pool;
pub mod weapon_hands_inventory {
    pub use tsfp_port_wire::leaf_wire::inventory::{
        AMMO_TYPES, ENTRY_COUNT, FLAG_X2, Inventory, WeaponEntries,
    };
}

/// Production scene placement, distinct from the standalone snapshot format.
pub mod transforms {
    #[derive(Clone, Copy, Debug, PartialEq)]
    #[repr(C)]
    pub struct WorldTransform {
        pub matrix: [f32; 16],
    }
    impl WorldTransform {
        pub const IDENTITY: Self = Self {
            matrix: [
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
            ],
        };
    }
}

#[path = "../../tsfp-state/src/scene_membership.rs"]
pub mod scene_membership;
#[path = "../../tsfp-state/src/weapon_cycle.rs"]
pub mod weapon_cycle;
#[path = "../../tsfp-state/src/weapon_hands.rs"]
pub mod weapon_hands;
#[path = "../../tsfp-state/src/weapon_interaction.rs"]
pub mod weapon_interaction;
#[path = "../../tsfp-state/src/weapon_inventory.rs"]
pub mod weapon_inventory;
#[path = "../../tsfp-state/src/weapon_payload.rs"]
pub mod weapon_payload;
#[path = "../../tsfp-state/src/weapon_projectiles.rs"]
pub mod weapon_projectiles;
