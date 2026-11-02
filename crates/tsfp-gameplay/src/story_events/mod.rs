//! Data-driven Story event system (T1840, T1841), specified by docs/t-story-event-verbs.md (T1838).
//!
//! `record` parses and serializes the 0xC8-byte SETU event definitions, `engine` runs the
//! instance pool (links, gate, conditions, delay, repeat, checkpoint mask, snapshot), `slots` is
//! the slot manager, `mode` the build predicate, `condition` the condition kinds, `actions` the
//! typed verb effects, `verbs` the executors and tail-pass completion and `world` the trait for
//! game-world side effects. Coverage and confidence labels: docs/t-story-event-engine.md.

pub mod actions;
pub mod arcade;
pub mod callbacks;
pub mod callees_t1846;
pub mod condition;
pub mod eff_dispatch;
pub mod eff_flare;
pub mod eff_guest;
pub mod eff_helpers;
pub mod eff_init;
pub mod eff_late;
pub mod eff_update;
pub mod eff_update2;
pub mod eff_update3;
pub mod effect_routing;
pub mod eligible_count;
pub mod engine;
pub mod face_geometry;
pub mod frame_clock;
pub mod group_members;
pub mod group_paths;
pub mod hook_wiring;
pub mod hooks_audio;
pub mod hooks_big;
pub mod hooks_fx;
pub mod hooks_l06_0b;
pub mod hooks_l11;
pub mod hooks_scene;
pub mod hooks_story;
pub mod level_roles;
pub mod levels;
pub mod mode;
pub mod native_world;
pub mod piece_follow;
pub mod pose;
pub mod prelude_1e2b80;
pub mod progress;
pub mod record;
/// T3415 readable, opt-in Story reset relocation over Rust-owned player/spawn state.
#[path = "../port_scaffolds/continuous_unknown_reset_relocation_versus_cinematic_release.rs"]
pub mod reset_relocation;
pub mod setu;
pub mod shape_obb;
pub mod shape_parts;
pub mod shape_scan;
pub mod slots;
pub mod snapshot;
pub mod strip_update;
pub mod svc_destroy;
pub mod svc_destroy_callees;
pub mod svc_effect_submit;
pub mod svc_effect_tick;
pub mod svc_ember_services;
pub mod svc_emitter;
pub mod svc_kind_table;
pub mod svc_light;
pub mod svc_objects;
pub mod svc_particle_efbd0;
pub mod svc_particle_kinds;
pub mod svc_particles;
pub mod svc_pool;
pub mod svc_pool_classes;
pub mod svc_pose;
pub mod svc_reloc;
pub mod svc_resource;
pub mod svc_shape;
pub mod svc_shape_services;
pub mod svc_slot_teardown;
pub mod svc_spawn_c1950;
pub mod svc_spray;
pub mod svc_stream;
pub mod svc_template_defs;
pub mod svc_wiring;
pub mod sweep_cells;
pub mod sweep_dynamic_shape;
pub mod sweep_records;
pub mod sweep_shapes;
pub mod sweep_triangles;
pub mod ticks;
pub mod verbs;
pub mod world;
pub mod world_queries;

pub use engine::{BuildOptions, Engine, EngineError, Fired, Instance, LinkSide, StepReport, Trace};
pub use mode::ModeContext;
pub use record::{Condition, EventRecord, Link, RECORD_SIZE};
pub use slots::{MissionEvent, SlotManager};
pub use snapshot::EventSnapshot;
pub use world::World;
