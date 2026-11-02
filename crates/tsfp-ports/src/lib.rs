// SPDX-License-Identifier: GPL-3.0-or-later
// T2396: the no_std host archive stays separate from the std-feature Cargo rlib because the feature sets differ.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
extern crate self as tsfp_ports;

#[cfg(test)]
#[unsafe(no_mangle)]
extern "C" fn xbox_ReadTimeStampCounter() -> u64 {
    0
}

// T1923: lets path-included mapmaker sources keep `use tsfp_gameplay::memory::GuestMemory` (resolves to `crate::memory`).
extern crate self as tsfp_gameplay;
extern crate tsfp_player_interaction_fast_path;
pub mod abi;
/// T2407 shares typed AI awareness and claim state with the runtime archive.
#[allow(dead_code)]
#[path = "../../tsfp-ai/src/awareness.rs"]
pub mod awareness;
pub mod contract_views;
pub mod frame_events;
#[allow(dead_code)]
#[path = "../../tsfp-port-ui-menu-capture/src/lib.rs"]
mod ui_menu_capture;
pub mod wall_clock;
// T1670: share the readable engine/core timer owner with the no_std host archive.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-core/src/frame_state.rs"]
pub mod frame_state;
pub mod frame_state_ffi;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-core/src/game_timer.rs"]
pub mod game_timer;
pub mod game_timer_ffi;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-core/src/vblank_clock.rs"]
pub mod vblank_clock;
pub mod vblank_clock_ffi;
// T1976: the render slice (crates/tsfp-engine-render) compiled into the lane build. The sources name each other as
// `crate::<module>`, so they sit at this root (the crate cannot depend on tsfp-engine-render: that crate depends on this one).
extern crate alloc;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/renderer.rs"]
pub mod renderer;
pub use renderer::Renderer;
pub mod anim_lane;
pub use tsfp_port_engine_anim_adapter::anim_resource_prefix;
#[allow(dead_code)] // Shared source for the readable animation resource-prefix model.
#[path = "../../tsfp-engine-anim/src/dependency_prefix.rs"]
pub mod dependency_prefix;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/device.rs"]
pub mod device;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/draw.rs"]
pub mod draw;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/effect.rs"]
pub mod effect;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/effect_setup.rs"]
pub mod effect_setup;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/emitter.rs"]
pub mod emitter;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/lighting.rs"]
pub mod lighting;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/map.rs"]
pub mod map;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/mem.rs"]
pub mod mem;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/overlay.rs"]
pub mod overlay;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/particle.rs"]
pub mod particle;
pub mod render_lane;
pub mod render_lane2;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/render_math.rs"]
pub mod render_math;
pub mod render_trace;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/scene.rs"]
pub mod scene;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/state.rs"]
pub mod state;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/trace_wire.rs"]
pub mod trace_wire;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/vertex.rs"]
pub mod vertex;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/vertex_format.rs"]
pub mod vertex_format;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/visual_effects.rs"]
pub mod visual_effects;
// T1918: core only tsfp-engine-anim modules at the crate root, because they reference each other as `crate::<module>`.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/advance.rs"]
pub mod advance;
pub mod anim_lane2;
pub mod anim_lane3;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/keyrec.rs"]
pub mod keyrec;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/layers.rs"]
pub mod layers;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/queries.rs"]
pub mod queries;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/start.rs"]
pub mod start;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/tail.rs"]
pub mod tail;
// T1918 wave B: the pose math sources (core only, libm through `fmath`).
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/aim.rs"]
pub mod aim;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/blend.rs"]
pub mod blend;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/bounds.rs"]
pub mod bounds;
pub mod fmath;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/heading.rs"]
pub mod heading;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/instance.rs"]
pub mod instance;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/keybracket.rs"]
pub mod keybracket;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/posemath.rs"]
pub mod posemath;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/posemix.rs"]
pub mod posemix;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-anim/src/prop.rs"]
pub mod prop;
pub mod closure_api {
    pub use crate::callee::{Callee, Cpu, guest_call, stack_arg};
    pub use crate::memory::GuestMemory;
}
pub mod arcade_setup_runtime;
pub mod audio_lane;
pub mod char_lane;
pub mod cutscene_lane;
pub mod gp_bridge;
pub mod hud_lane;
pub use tsfp_port_wire::hud_metrics;
pub mod assets;
pub mod behaviour;
pub mod bridge;
pub mod c2r;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-network/src/callee.rs"]
pub mod callee;
pub mod combat;
pub mod ext;
pub mod fpclosure;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod input_axis_quality;
pub mod input_lane;
pub mod input_lane2;
pub mod mapmaker_ui_lane;
pub mod modes_lane;
pub mod modes_lane2;
pub mod npc_lane;
pub mod object_lane;
pub mod physics_lane;
pub mod physics_lane2;
pub mod state_lane;
pub mod story_buffer;
pub mod text_field;
pub mod text_lane;
pub mod weapons_lane2;
#[rustfmt::skip]
pub mod inventory;
/// T2376: share the typed weapon hand owner with tsfp-state without a crate dependency cycle.
pub mod weapon_hands_inventory {
    pub use crate::leaf_wire::inventory::{
        AMMO_TYPES, ENTRY_COUNT, FLAG_X2, Inventory, WeaponEntries,
    };
}
pub mod inventory_ammo;
pub mod inventory_grant;
pub mod inventory_pickup;
pub mod inventory_select;
pub mod inventory_slots;
pub mod lane;
pub mod lane_inv;
pub mod layouts;
pub mod leaf_lane;
pub mod leaf_wire;
// Shared state modules are also path-included at this crate root; mirror the tsfp-state
// ObjectPool path so `weapon_payload` resolves the same generation-bearing identity here.
pub use leaf_wire::object_pool;
pub mod leaves;
/// T2415 readable Map Maker HUD panel slides (lane roots, default declined).
pub mod mapmaker_hud_panels;
pub mod mapmaker_lane;
pub mod mapmaker_modes;
/// T2418 readable Map Maker overlay alpha ramp and anchor shift (lane roots, default declined).
pub mod mapmaker_overlay;
pub mod mapmaker_update;
pub mod mapmaker_view;
pub mod memory;
pub mod memory_alloc_wrapper_lane;
/// Readable default-off route for the CRT atexit allocator caller.
pub mod memory_atexit;
pub mod menu_dispatch;
pub mod menu_ease;
pub mod menu_state;
pub mod mp_lane;
pub mod objchar_writer;
pub mod player_wire;
pub mod pose;
pub mod story_activation_lane;
/// T2488 typed successful-ending control restoration (default-off selector).
#[path = "port_scaffolds/successful_ending_camera_completion_and_local_player_control_release_policy_from_the_retained_story_cutscene_path.rs"]
pub mod story_control_release_lane;
/// T2450 opt-in typed Story end-transition admission at `0x67450`.
pub mod story_end_transition_lane;
/// T2465 default-off typed Story event link callback at `0x235240`.
pub mod story_event_callback_lane;
pub mod story_event_combine_lane;
/// T2476/T2483/T2489 default-off typed Story event-condition predicates.
pub mod story_event_condition_lane;
/// T2490 default-off typed Story objective checkpoint restore lane.
pub mod story_objective_checkpoint_lane;
mod port_scaffolds {
    #[path = "story_kind_29_t2489.rs"]
    pub mod story_kind_29_t2489;
    pub mod story_kind_33_table_field_predicate_t2485;
}
/// T2493 default-off typed Story verb callback invocation context and raw result gate.
#[path = "port_scaffolds/verb_callback_dispatch_boundary_at_0x236099_as_a_typed_rust_borrowed_view_over_the_current_level_arena_event_instance.rs"]
pub mod story_callback_dispatch_lane;
/// T2468 default-off typed Story event-instance delay countdown and eligibility.
pub mod story_event_delay_lane;
/// T2477 default-off typed Story event run-gate at definition +0x5c.
pub mod story_event_gate_lane;
/// T2491 default-off typed Story event active-list iteration cursor.
pub mod story_event_iteration_lane;
/// T2452 default-off typed Story event-link status query at `0x2273C0`.
pub mod story_event_link_lane;
/// T2469 default-off typed Story active-instance control gates in the state pass.
pub mod story_event_state_lane;
/// T2458 default-off typed Story next-pass event wait/resume transition.
pub mod story_event_wait_lane;
/// T2492 default-off Rust-owned Story objective registry and bounded admission contract.
#[path = "port_scaffolds/story_objective_registry_admission_slice.rs"]
pub mod story_objective_registry_admission_slice;
/// T2451 default-off typed ordinary Story end bookkeeping at `0x66FC0`.
pub mod story_ordinary_end_lane;
/// T2448 opt-in Story objective result reducer at `0x228870`.
pub mod story_result_lane;
/// T2431 Story progression sequencer 0x65940 (lane root, default declined).
pub mod story_sequencer_lane;
// T2377: the same readable owner API is compiled into the hybrid lane so a runtime switch can
// adopt its typed interaction and inventory lookups without duplicating the implementation.
/// T2385 runtime uses the same typed movement helper source as `tsfp-ai`.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/movement_approach.rs"]
pub mod movement_approach;
#[allow(dead_code)] // T2407 shares the measured navigation ID domain model.
#[path = "../../tsfp-ai/src/navigation_node_domain.rs"]
pub mod navigation_node_domain;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/navigation_scoring.rs"]
pub mod navigation_scoring;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/perception_geometry.rs"]
pub mod perception_geometry;
#[allow(dead_code)] // T3398: shared player interaction owner, live in tsfp-state
#[path = "../../tsfp-state/src/player_interaction.rs"]
pub mod player_interaction;
/// T2385 compiles the same readable perception owner and tiny shared types
/// used by `tsfp-ai`, avoiding duplicate direction logic in the runtime.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/point_tables.rs"]
pub mod point_tables;
pub mod pose_anim;
pub mod pose_leaf;
pub mod raw_look;
pub mod rea_ai_runtime;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_ai_types.rs"]
pub mod rea_ai_types;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_awareness_leaves.rs"]
pub mod rea_awareness_leaves;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_awareness_sources.rs"]
pub mod rea_awareness_sources;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_behavior_followup.rs"]
pub mod rea_behavior_followup;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_behavior_guard.rs"]
pub mod rea_behavior_guard;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_character_leaves.rs"]
pub mod rea_character_leaves;
/// T2407 shares the typed controller-target commit model with the host archive.
#[allow(dead_code)]
#[path = "../../tsfp-ai/src/rea_controller_target_commit.rs"]
pub mod rea_controller_target_commit;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_executor_gate.rs"]
pub mod rea_executor_gate;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_group_alert.rs"]
pub mod rea_group_alert;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_group_target.rs"]
pub mod rea_group_target;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/rea_interaction_inventory.rs"]
pub mod rea_interaction_inventory;
pub mod rea_inventory_runtime;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_link_decision.rs"]
pub mod rea_link_decision;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_link_gate.rs"]
pub mod rea_link_gate;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_mechanism.rs"]
pub mod rea_mechanism;
/// T2494 shares the typed movement boundary required by adjacent tick-state tests.
#[allow(dead_code)]
#[path = "../../tsfp-ai/src/rea_movement.rs"]
pub mod rea_movement;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_nav_link.rs"]
pub mod rea_nav_link;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-gameplay/src/rea_object_flags.rs"]
pub mod rea_object_flags;
#[allow(dead_code)] // T2365: shared typed owner is also used by the AI crate.
#[path = "../../tsfp-ai/src/rea_object_pool_ledger.rs"]
pub mod rea_object_pool_ledger;
/// T2433 lane adapter for the engine/object frontier leaves (default declined).
pub mod rea_object_runtime;
#[path = "../../tsfp-ai/src/rea_partial_route_retry.rs"]
pub mod rea_partial_route_retry;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_particle_pool.rs"]
pub mod rea_particle_pool;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_route_peek.rs"]
pub mod rea_route_peek;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_routes.rs"]
pub mod rea_routes;
/// T2407 shares the typed target decision and arbitration models with the host archive.
#[allow(dead_code)]
#[path = "../../tsfp-ai/src/rea_target_handoff.rs"]
pub mod rea_target_handoff;
#[allow(dead_code)]
#[path = "../../tsfp-ai/src/rea_target_selector.rs"]
pub mod rea_target_selector;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_tick_state.rs"]
pub mod rea_tick_state;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/weapon_ammo_cap.rs"]
pub mod rea_weapon_ammo_cap;
/// T1678 runtime compiles the same readable weapon-cycle selector used by `tsfp-ai`.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_weapon_cycle.rs"]
pub mod rea_weapon_cycle;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_weapon_group.rs"]
pub mod rea_weapon_group;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_weapon_selection.rs"]
pub mod rea_weapon_selection;
/// T1678 Q3 shares the readable guarded weapon-row lookup with the AI crate.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_weapon_subrecord.rs"]
pub mod rea_weapon_subrecord;
/// T2434 shares the actor turn-rate table lookup owner with `tsfp-ai`.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/rea_weapon_turn.rs"]
pub mod rea_weapon_turn;
pub mod retired;
pub mod rootmap;
pub mod runtime;
pub mod save_lane;
pub mod save_slots;
pub use tsfp_port_runtime::state_wire;
/// T2411 native weapon payload retirement uses the same generation-checked scene owner.
pub use tsfp_weapon_owner::scene_membership;
// Shared gameplay sources use this crate-root alias in both tsfp-gameplay and the
// direct-rustc no_std ports build; tsfp-ports intentionally has no tsfp-state dependency.
pub use state_wire::story_flow;
/// T2453 shared typed reducer compiled into the no_std ports crate.
#[path = "../../tsfp-gameplay/src/story_events/eligible_count.rs"]
pub mod story_eligible_count;
pub mod story_level_start;
pub mod story_objective_admission_lane;
pub mod story_objective_lifecycle;
/// T2464 default-off typed Story objective member-progress counter update.
pub mod story_objective_progress_lane;
pub mod tick;
pub mod views;
/// T1682 Q8 shares the readable mode-aware ammo fallback with `tsfp-ai`.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/weapon_ammo_fallback.rs"]
pub mod weapon_ammo_fallback;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/weapon_context.rs"]
pub mod weapon_context;
/// T2411 shares the Rust-owned projectile lifetime state with `tsfp-state`.
pub use tsfp_weapon_owner::weapon_projectiles;
pub use tsfp_weapon_owner::{
    weapon_cycle, weapon_hands, weapon_interaction, weapon_inventory, weapon_payload,
};
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/weapon_readiness.rs"]
pub mod weapon_readiness;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-ai/src/weapon_transition.rs"]
pub mod weapon_transition;
pub mod weapons;

#[cfg(all(not(test), not(feature = "std")))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    // T1986: name the panic site on stderr (file:line) so a silent host abort can be located.
    if let Some(location) = info.location() {
        let mut line = [0u8; 160];
        let mut at = 0;
        for byte in b"RUST-PANIC "
            .iter()
            .chain(location.file().as_bytes())
            .take(140)
        {
            line[at] = *byte;
            at += 1;
        }
        line[at] = b':';
        at += 1;
        let mut digits = [0u8; 10];
        let (mut n, mut count) = (location.line(), 0);
        loop {
            digits[count] = b'0' + (n % 10) as u8;
            count += 1;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        while count > 0 && at < 158 {
            count -= 1;
            line[at] = digits[count];
            at += 1;
        }
        line[at] = b'\n';
        unsafe { write(2, line.as_ptr(), at + 1) };
    }
    // The existing host links libc. Panic must terminate, never unwind or hang.
    unsafe { abort() }
}

unsafe extern "C" {
    fn abort() -> !;
    fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
}

// Precompiled core retains this relocation even with panic=abort.
// An attempted unwind is an infrastructure failure, never a successful return.
#[cfg(all(not(test), not(feature = "std")))]
#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() -> ! {
    unsafe { abort() }
}

// T1902 opt-in system slices; no production dispatcher registrations.
/// T1907 engine/core bulk bootstrap and parser translations (Tier 0).
#[allow(
    dead_code,
    unused_assignments,
    unused_mut,
    unused_parens,
    unused_variables,
    unreachable_code,
    unreachable_patterns
)]
pub mod core_bulk;
pub(crate) use tsfp_port_native_candidate_support::system_slice;
pub mod vehicles;
pub mod video;

// Cold lifted candidates used only as Tier 1 differential references by tests. This feature is
// absent from the game host and does not admit these functions to any production selector.
#[cfg(feature = "editor-next-reference")]
#[path = "editor_next.rs"]
pub mod editor_next_reference;

pub mod mapmaker_ui;

// The editor translation is a differential reference only; the host does not dispatch through it.
#[cfg(feature = "editor-reference")]
#[path = "editor.rs"]
pub mod editor_reference;

// T1913 opt-in audio slice.
pub mod audio;
pub mod player_interaction_runtime;
pub mod weapon_hands_runtime;
/// T1831 weapons attachment leaves; Tier 1 lifted/stub differential, per-root opt-in only.
pub mod weapons_slice;

// T1924 opt-in audio continuation.
pub mod audio_remainder;
pub mod character_yaw;
pub mod mission_action_leaves;
pub mod mission_links;
pub mod missions_reader;
/// T2041 data/missions point-table boundary (0x715F2C family, 0x55F974).
pub mod missions_system;
pub mod player_look;
pub mod player_look_gate;
/// T2019 gameplay/player explicit opt-in attachment boundary.
pub mod player_system;
/// T1925 weapons bulk slice with an explicit opt-in attachment boundary.
pub mod weapons_system;
pub mod weather_presets;

/// T1937 address-selective math service boundary.
pub mod math_slice;
/// T2391 owned, readable math values; address wrappers remain opt-in in `math_slice`.
pub mod math_values;
/// T1967 opt-in host selection of the c2r weapon roots.
pub mod pickup_tick;
pub mod weapons_lane;
pub mod weapons_remainder_system;

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-engine-render/src/hook_policy.rs"]
mod hook_policy;
pub mod objchar_player_writer;
pub mod objchar_tail_writer;

pub mod cutscene_presentation_lane;
pub mod missions_lane;
pub mod missions_wire;
pub mod missions_writer;
pub mod scene_lane;
pub mod story_mission_tick;
pub mod text_field_lane;

pub mod camera;
pub mod object_transform;
/// Shared scene-membership source expects the state crate's transform module name.
pub mod transforms {
    pub use crate::object_transform::WorldTransform;
}
pub mod story_eligible_count_lane;
