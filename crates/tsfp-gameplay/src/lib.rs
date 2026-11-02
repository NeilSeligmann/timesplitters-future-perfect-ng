//! gameplay: architecture boundary reserved by T1812.
//! No game behavior is implemented; see docs/t-rust-architecture.md.

pub mod call_leaves;
pub mod char_anim;
pub mod char_anim_tables;
pub mod char_class;
pub mod char_despawn;
pub mod char_script_actor;
pub mod char_sequence;
pub mod char_tick_ambush;
pub mod char_tick_class_a;
pub mod char_tick_cover;
pub mod char_tick_entity;
pub mod char_tick_follow_pose;
pub mod char_tick_idle_variation;
pub mod char_tick_railbot;
pub mod char_tick_route_step;
pub mod char_tick_script_pose;
pub mod char_tick_static_pose;
pub mod char_tick_zombie;
#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;
pub mod mode_leaves;
pub mod modes;
/// Shared typed Story flow contract, also path-included by the no_std ports lane.
pub use tsfp_state::story_flow;
/// T2394 typed player motion candidate and actor-pose boundary.
pub mod motion;
/// T2148 readable, Rust-owned policy for nearby weapon-pickup touch eligibility (opt-in).
pub mod pickup_eligibility;
pub mod slot_leaves;
pub mod state_leaves;
pub mod story_events;
/// T2410 owner for Story flow, objectives, terminals, and progress handoff.
pub mod story_mission;
pub mod story_schedule;
pub mod synth_leaves;

pub mod char_tick_combat;

pub mod char_tick_hero_pose;

pub mod char_tick_hero;
pub mod char_tick_idle_variant;
pub mod char_tick_low_class;
/// T2490 typed Story objective checkpoint merge (default-off at the ports boundary).
pub mod port_scaffolds;
pub mod save_device;
pub mod save_files;
pub mod save_profile;
pub mod save_unlock;
pub mod t1702_party;
pub mod t1900_multiplayer;
pub use tsfp_port_gameplay_modes_multiplayer_1 as t1900_team_pickup;
pub mod t1900_vehicle_update;
pub mod w4_weapons;
pub mod w4b_gameplay;
/// Closure-model names shared with the `tsfp-ports` lane adapters (T1923: the same source file compiles in both crates).
pub mod closure_api {
    pub use tsfp_engine_object::closure::callee::{Callee, Cpu, guest_call, stack_arg};
    pub use tsfp_engine_object::closure::memory::GuestMemory;
}

pub mod cutscene_presentation;
pub mod cutscene_sequence;

/// T2377: typed REA inventory-definition and interaction continuation contracts.
pub mod rea_interaction_inventory;
/// T2433: typed engine/object frontier leaves (frame stamp, VICE/VICO tag, entry flag family).
pub mod rea_object_flags;
