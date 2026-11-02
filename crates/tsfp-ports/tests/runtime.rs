// SPDX-License-Identifier: GPL-3.0-or-later
extern crate self as tsfp_gameplay;
#[path = "../src/abi.rs"]
mod abi;
#[path = "../src/behaviour.rs"]
mod behaviour;
#[path = "../src/bridge.rs"]
mod bridge;
#[path = "../../tsfp-network/src/callee.rs"]
mod callee;
#[path = "../src/combat.rs"]
mod combat;
#[path = "../src/ext.rs"]
mod ext;
#[path = "../../tsfp-engine-memory/src/gprs.rs"]
mod gprs;
#[path = "../src/inventory.rs"]
mod inventory;
#[path = "../src/inventory_ammo.rs"]
mod inventory_ammo;
#[path = "../src/inventory_grant.rs"]
mod inventory_grant;
#[path = "../src/inventory_pickup.rs"]
mod inventory_pickup;
#[path = "../src/inventory_select.rs"]
mod inventory_select;
#[path = "../src/inventory_slots.rs"]
mod inventory_slots;
#[path = "../src/lane.rs"]
mod lane;
#[path = "../src/lane_inv.rs"]
mod lane_inv;
#[path = "../../tsfp-engine-anim/src/layers.rs"]
mod layers;
#[path = "../src/leaves.rs"]
mod leaves;
#[path = "../src/memory.rs"]
mod memory;
#[path = "../src/pose.rs"]
mod pose;
#[path = "../src/pose_anim.rs"]
mod pose_anim;
#[path = "../src/pose_leaf.rs"]
mod pose_leaf;
#[path = "../src/raw_look.rs"]
mod raw_look;
#[path = "../src/rea_inventory_runtime.rs"]
mod rea_inventory_runtime;
pub mod closure_api {
    pub use crate::callee::{Callee, Cpu, guest_call, stack_arg};
    pub use crate::memory::GuestMemory;
}
pub(crate) use modes_lane::modes;
// Moved to its own crate by the T3643 split; src/c2r/engine_anim.rs names it via crate::.
pub(crate) use tsfp_port_engine_anim_adapter::anim_resource_prefix;
pub(crate) use tsfp_port_wire::hud_metrics;
#[path = "../../tsfp-engine-anim/src/advance.rs"]
mod advance;
#[path = "../../tsfp-engine-anim/src/aim.rs"]
mod aim;
#[path = "../src/anim_lane.rs"]
mod anim_lane;
#[path = "../src/anim_lane2.rs"]
mod anim_lane2;
#[path = "../src/anim_lane3.rs"]
mod anim_lane3;
#[path = "../../tsfp-engine-anim/src/blend.rs"]
mod blend;
#[path = "../../tsfp-engine-anim/src/bounds.rs"]
mod bounds;
#[path = "../src/c2r/mod.rs"]
mod c2r;
#[path = "../src/fmath.rs"]
mod fmath;
#[path = "../src/gp_bridge.rs"]
mod gp_bridge;
#[path = "../../tsfp-engine-anim/src/heading.rs"]
mod heading;
#[path = "../src/hud_lane.rs"]
mod hud_lane;
#[path = "../../tsfp-engine-anim/src/instance.rs"]
mod instance;
#[path = "../../tsfp-engine-anim/src/keybracket.rs"]
mod keybracket;
#[path = "../../tsfp-engine-anim/src/keyrec.rs"]
mod keyrec;
#[path = "../src/modes_lane.rs"]
mod modes_lane;
#[path = "../src/modes_lane2.rs"]
mod modes_lane2;
#[path = "../src/mp_lane.rs"]
mod mp_lane;
#[path = "../../tsfp-engine-anim/src/posemath.rs"]
mod posemath;
#[path = "../../tsfp-engine-anim/src/posemix.rs"]
mod posemix;
#[path = "../../tsfp-engine-anim/src/prop.rs"]
mod prop;
#[path = "../../tsfp-engine-anim/src/queries.rs"]
mod queries;
#[path = "../../tsfp-engine-anim/src/start.rs"]
mod start;
#[path = "../src/state_lane.rs"]
mod state_lane;
#[path = "../../tsfp-engine-anim/src/tail.rs"]
mod tail;
mod weapon_hands_inventory {
    pub use crate::leaf_wire::inventory::{
        AMMO_TYPES, ENTRY_COUNT, FLAG_X2, Inventory, WeaponEntries,
    };
}
#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/audio_lane.rs"]
mod audio_lane;
#[path = "../src/audio_remainder.rs"]
mod audio_remainder;
#[path = "../src/char_lane.rs"]
mod char_lane;
#[path = "../src/cutscene_lane.rs"]
mod cutscene_lane;
#[path = "../src/input_lane.rs"]
mod input_lane;
#[path = "../src/input_lane2.rs"]
mod input_lane2;
#[path = "../src/leaf_lane.rs"]
mod leaf_lane;
#[path = "../src/leaf_wire.rs"]
mod leaf_wire;
#[path = "../src/mapmaker_ui.rs"]
mod mapmaker_ui;
#[path = "../src/mapmaker_ui_lane.rs"]
mod mapmaker_ui_lane;
#[path = "../src/object_lane.rs"]
mod object_lane;
#[path = "../src/physics_lane.rs"]
mod physics_lane;
#[path = "../src/physics_lane2.rs"]
mod physics_lane2;
#[path = "../src/player_wire.rs"]
mod player_wire;
#[path = "../src/rootmap.rs"]
mod rootmap;
#[path = "../src/save_lane.rs"]
mod save_lane;
#[path = "../../tsfp-port-runtime/src/state_wire.rs"]
mod state_wire;
#[path = "../src/system_slice/mod.rs"]
mod system_slice;
#[path = "../src/text_lane.rs"]
mod text_lane;
#[path = "../src/views.rs"]
mod views;
#[path = "../src/weapons_lane2.rs"]
mod weapons_lane2;
mod story_flow {
    pub use crate::state_wire::story_flow::*;
}
#[path = "../../tsfp-engine-render/src/device.rs"]
mod device;
#[path = "../src/mapmaker_lane.rs"]
mod mapmaker_lane;
#[path = "../src/npc_lane.rs"]
mod npc_lane;
#[path = "../../tsfp-engine-render/src/renderer.rs"]
mod renderer;
#[path = "../../tsfp-engine-render/src/trace_wire.rs"]
mod trace_wire;
extern crate alloc;
#[path = "../../tsfp-engine-render/src/draw.rs"]
mod draw;
use renderer::Renderer;
#[path = "../src/cutscene_presentation_lane.rs"]
mod cutscene_presentation_lane;
#[path = "../../tsfp-engine-render/src/effect.rs"]
mod effect;
#[path = "../../tsfp-engine-render/src/effect_setup.rs"]
mod effect_setup;
#[path = "../../tsfp-engine-render/src/emitter.rs"]
mod emitter;
#[path = "../../tsfp-engine-render/src/lighting.rs"]
mod lighting;
#[path = "../../tsfp-engine-render/src/map.rs"]
mod map;
#[path = "../src/mapmaker_hud_panels.rs"]
mod mapmaker_hud_panels;
#[path = "../src/mapmaker_overlay.rs"]
mod mapmaker_overlay;
#[path = "../../tsfp-engine-render/src/mem.rs"]
mod mem;
#[path = "../src/menu_dispatch.rs"]
mod menu_dispatch;
#[path = "../src/menu_ease.rs"]
mod menu_ease;
#[path = "../src/menu_state.rs"]
mod menu_state;
#[path = "../src/missions_lane.rs"]
mod missions_lane;
#[path = "../src/objchar_tail_writer.rs"]
mod objchar_tail_writer;
#[path = "../../tsfp-engine-render/src/overlay.rs"]
mod overlay;
#[path = "../../tsfp-engine-render/src/particle.rs"]
mod particle;
#[path = "../src/player_system.rs"]
mod player_system;
#[path = "../src/rea_object_runtime.rs"]
mod rea_object_runtime;
#[path = "../src/render_lane.rs"]
mod render_lane;
#[path = "../src/render_lane2.rs"]
mod render_lane2;
#[path = "../../tsfp-engine-render/src/render_math.rs"]
mod render_math;
#[path = "../src/render_trace.rs"]
mod render_trace;
#[path = "../../tsfp-engine-render/src/scene.rs"]
mod scene;
#[path = "../src/scene_lane.rs"]
mod scene_lane;
#[path = "../../tsfp-engine-render/src/state.rs"]
mod state;
#[path = "../src/story_sequencer_lane.rs"]
mod story_sequencer_lane;
#[path = "../src/text_field_lane.rs"]
mod text_field_lane;
#[path = "../../tsfp-engine-render/src/vertex.rs"]
mod vertex;
#[path = "../../tsfp-engine-render/src/vertex_format.rs"]
mod vertex_format;
#[path = "../src/weapons_lane.rs"]
mod weapons_lane;
#[path = "../src/weapons_remainder_system.rs"]
mod weapons_remainder_system;
#[path = "../src/weapons_slice.rs"]
mod weapons_slice;
#[path = "../src/weapons_system.rs"]
mod weapons_system;
mod transforms {
    pub use crate::object_transform::WorldTransform;
}
#[path = "../src/arcade_setup_runtime.rs"]
mod arcade_setup_runtime;
#[path = "../../tsfp-ai/src/awareness.rs"]
mod awareness;
#[path = "../src/camera.rs"]
mod camera;
#[path = "../src/character_yaw.rs"]
mod character_yaw;
#[path = "../src/contract_views/mod.rs"]
mod contract_views;
#[path = "../src/frame_events.rs"]
mod frame_events;
#[path = "../../tsfp-engine-core/src/frame_state.rs"]
mod frame_state;
#[path = "../src/frame_state_ffi.rs"]
mod frame_state_ffi;
#[path = "../../tsfp-engine-render/src/hook_policy.rs"]
mod hook_policy;
#[path = "../src/input_axis_quality.rs"]
mod input_axis_quality;
#[path = "../src/mapmaker_update.rs"]
mod mapmaker_update;
#[path = "../src/memory_alloc_wrapper_lane.rs"]
mod memory_alloc_wrapper_lane;
#[path = "../src/memory_atexit.rs"]
mod memory_atexit;
#[path = "../src/mission_action_leaves.rs"]
mod mission_action_leaves;
#[path = "../src/mission_links.rs"]
mod mission_links;
#[path = "../src/missions_reader.rs"]
mod missions_reader;
#[path = "../src/missions_system.rs"]
mod missions_system;
#[path = "../src/missions_wire.rs"]
mod missions_wire;
#[path = "../src/missions_writer.rs"]
mod missions_writer;
#[path = "../../tsfp-ai/src/movement_approach.rs"]
mod movement_approach;
#[path = "../src/objchar_player_writer.rs"]
mod objchar_player_writer;
#[path = "../src/objchar_writer.rs"]
mod objchar_writer;
#[path = "../src/object_transform.rs"]
mod object_transform;
#[path = "../src/pickup_tick.rs"]
mod pickup_tick;
#[path = "../src/player_interaction_runtime.rs"]
mod player_interaction_runtime;
#[path = "../src/player_look.rs"]
mod player_look;
#[path = "../src/player_look_gate.rs"]
mod player_look_gate;
#[path = "../src/rea_ai_runtime.rs"]
mod rea_ai_runtime;
#[path = "../../tsfp-gameplay/src/rea_interaction_inventory.rs"]
mod rea_interaction_inventory;
#[path = "../../tsfp-gameplay/src/rea_object_flags.rs"]
mod rea_object_flags;
#[path = "../src/retired.rs"]
mod retired;
#[path = "../src/runtime.rs"]
mod runtime;
#[path = "../src/save_slots.rs"]
mod save_slots;
#[path = "../src/story_activation_lane.rs"]
mod story_activation_lane;
#[path = "../../tsfp-gameplay/src/story_events/eligible_count.rs"]
mod story_eligible_count;
#[path = "../src/story_eligible_count_lane.rs"]
mod story_eligible_count_lane;
#[path = "../src/story_event_callback_lane.rs"]
mod story_event_callback_lane;
#[path = "../src/story_event_link_lane.rs"]
mod story_event_link_lane;
#[path = "../src/story_level_start.rs"]
mod story_level_start;
#[path = "../src/story_mission_tick.rs"]
mod story_mission_tick;
#[path = "../src/story_objective_admission_lane.rs"]
mod story_objective_admission_lane;
#[path = "../src/story_objective_lifecycle.rs"]
mod story_objective_lifecycle;
#[path = "../src/story_result_lane.rs"]
mod story_result_lane;
#[path = "../src/text_field.rs"]
mod text_field;
#[path = "../../tsfp-port-ui-menu-capture/src/lib.rs"]
mod ui_menu_capture;
#[path = "../src/video.rs"]
mod video;
#[path = "../../tsfp-engine-render/src/visual_effects.rs"]
mod visual_effects;
#[path = "../src/weapon_hands_runtime.rs"]
mod weapon_hands_runtime;
#[path = "../src/weapons.rs"]
mod weapons;
#[path = "../src/weather_presets.rs"]
mod weather_presets;
mod object_pool {
    pub use crate::leaf_wire::object_pool::*;
}
#[path = "../../tsfp-ai/src/navigation_scoring.rs"]
mod navigation_scoring;
#[path = "../../tsfp-ai/src/perception_geometry.rs"]
mod perception_geometry;
#[path = "../../tsfp-state/src/player_interaction.rs"]
mod player_interaction;
#[path = "../../tsfp-ai/src/point_tables.rs"]
mod point_tables;
#[path = "../../tsfp-ai/src/rea_ai_types.rs"]
mod rea_ai_types;
#[path = "../../tsfp-ai/src/rea_awareness_leaves.rs"]
mod rea_awareness_leaves;
#[path = "../../tsfp-ai/src/rea_awareness_sources.rs"]
mod rea_awareness_sources;
#[path = "../../tsfp-ai/src/rea_behavior_followup.rs"]
mod rea_behavior_followup;
#[path = "../../tsfp-ai/src/rea_behavior_guard.rs"]
mod rea_behavior_guard;
#[path = "../../tsfp-ai/src/rea_character_leaves.rs"]
mod rea_character_leaves;
#[path = "../../tsfp-ai/src/rea_controller_target_commit.rs"]
mod rea_controller_target_commit;
#[path = "../../tsfp-ai/src/rea_executor_gate.rs"]
mod rea_executor_gate;
#[path = "../../tsfp-ai/src/rea_group_alert.rs"]
mod rea_group_alert;
#[path = "../../tsfp-ai/src/rea_group_target.rs"]
mod rea_group_target;
#[path = "../../tsfp-ai/src/rea_link_decision.rs"]
mod rea_link_decision;
#[path = "../../tsfp-ai/src/rea_link_gate.rs"]
mod rea_link_gate;
#[path = "../../tsfp-ai/src/rea_mechanism.rs"]
mod rea_mechanism;
#[path = "../../tsfp-ai/src/rea_nav_link.rs"]
mod rea_nav_link;
#[path = "../../tsfp-ai/src/rea_object_pool_ledger.rs"]
mod rea_object_pool_ledger;
#[path = "../../tsfp-ai/src/rea_particle_pool.rs"]
mod rea_particle_pool;
#[path = "../../tsfp-ai/src/rea_route_peek.rs"]
mod rea_route_peek;
#[path = "../../tsfp-ai/src/rea_routes.rs"]
mod rea_routes;
#[path = "../../tsfp-ai/src/rea_tick_state.rs"]
mod rea_tick_state;
#[path = "../../tsfp-ai/src/weapon_ammo_cap.rs"]
mod rea_weapon_ammo_cap;
#[path = "../../tsfp-ai/src/rea_weapon_cycle.rs"]
mod rea_weapon_cycle;
#[path = "../../tsfp-ai/src/rea_weapon_group.rs"]
mod rea_weapon_group;
#[path = "../../tsfp-ai/src/rea_weapon_subrecord.rs"]
mod rea_weapon_subrecord;
#[path = "../../tsfp-ai/src/rea_weapon_turn.rs"]
mod rea_weapon_turn;
#[path = "../src/story_end_transition_lane.rs"]
mod story_end_transition_lane;
#[path = "../src/story_objective_progress_lane.rs"]
mod story_objective_progress_lane;
#[path = "../../tsfp-ai/src/weapon_ammo_fallback.rs"]
mod weapon_ammo_fallback;
#[path = "../../tsfp-ai/src/weapon_context.rs"]
mod weapon_context;
#[path = "../../tsfp-ai/src/weapon_readiness.rs"]
mod weapon_readiness;
#[path = "../../tsfp-ai/src/weapon_transition.rs"]
mod weapon_transition;
use tsfp_weapon_owner::scene_membership;
#[path = "../src/mapmaker_view.rs"]
mod mapmaker_view;
#[path = "../../tsfp-ai/src/rea_movement.rs"]
mod rea_movement;
#[path = "../../tsfp-ai/src/rea_partial_route_retry.rs"]
mod rea_partial_route_retry;
#[path = "../../tsfp-ai/src/rea_target_handoff.rs"]
mod rea_target_handoff;
#[path = "../../tsfp-ai/src/rea_target_selector.rs"]
mod rea_target_selector;
use tsfp_weapon_owner::{
    weapon_cycle, weapon_hands, weapon_interaction, weapon_inventory, weapon_payload,
    weapon_projectiles,
};
#[path = "../../tsfp-ai/src/navigation_node_domain.rs"]
mod navigation_node_domain;
#[path = "../src/story_ordinary_end_lane.rs"]
mod story_ordinary_end_lane;

#[unsafe(no_mangle)]
extern "C" fn tsfp_lane_guest_call(_: u32, _: *mut u32) {
    panic!("no guest in the selector test")
}
std::thread_local! {
    static TEST_FP_LOAD_ALLOWED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[unsafe(no_mangle)]
extern "C" fn tsfp_lane_x_fp_load(_: *mut abi::ExtRegisters) {
    assert!(TEST_FP_LOAD_ALLOWED.get(), "no guest in the selector test");
}
#[unsafe(no_mangle)]
extern "C" fn tsfp_lane_x_fp_store(_: *const abi::ExtRegisters) {
    panic!("no guest in the selector test")
}
#[unsafe(no_mangle)]
extern "C" fn tsfp_lane_x_call(_: u32, _: *mut abi::ExtRegisters) {
    panic!("no guest in the selector test")
}
// The production host provides this optional tracing hook. Keep the integration
// harness linkable when lane_inv's nested-call path is included.
// T3660: the host hook has no trace sink in this integration harness.
#[unsafe(no_mangle)]
extern "C" fn host_rust_ports_nested_call_args(_: u32, _: u32, _: u32, _: u32) {}
// T3398: the math lane roots reach the rdtsc translation (t3398_math4), which calls the host clock.
#[unsafe(no_mangle)]
extern "C" fn xbox_ReadTimeStampCounter() -> u64 {
    0
}
/// A selection mask whose first words are `head`.
fn mask_of(head: &[u64]) -> runtime::Mask {
    let mut mask = [0u64; runtime::WORDS];
    mask[..head.len()].copy_from_slice(head);
    mask
}
/// T3773: `all` is the adopted roots (HOST + LANE + INV), not the tsfp-lane-c2r Tier 2 checking rows behind them.
fn all_roots() -> runtime::Mask {
    let mut mask = [0u64; runtime::WORDS];
    for index in 0..24 + runtime::LANE_ROOTS.len() + lane_inv::INV_ROOTS.len() {
        mask[index / 64] |= 1u64 << (index % 64);
    }
    mask
}
#[test]
fn strict_selector() {
    assert_eq!(runtime::parse_selection(b"off"), Some([0; runtime::WORDS]));
    assert_eq!(runtime::parse_selection(b"all"), Some(all_roots()));
    assert_eq!(
        runtime::parse_selection(b"0x71C70,000d2ce0"),
        Some(mask_of(&[6, 0, 0, 0, 0, 0, 0]))
    );
    for (index, &va) in runtime::ROOTS.iter().enumerate() {
        let token = format!("{va:08X}");
        assert_eq!(
            runtime::parse_selection(token.as_bytes()),
            Some(runtime::bit(index))
        );
    }
    for bad in [
        "",
        "OFF",
        " all",
        "all,",
        "71c70,",
        ",71c70",
        "71c70,71C70",
        "BA230",
        "FFFFFFFF",
        "000071C70",
        "0x",
        "71_c70",
        "+71c70",
        "all,off",
    ] {
        assert_eq!(runtime::parse_selection(bad.as_bytes()), None, "{bad}");
    }
}

/// T3773 regress FAULT (story-level1 SIGSEGV via fpistol_event_callback): c2r checking rows must not go live with `all`, a group
/// or `all,-x`; only their explicit VA (the `--t2-offline` per-VA selection) enables one.
#[test]
fn all_excludes_unadopted_c2r_roots_and_an_explicit_va_enables_one() {
    let first_c2r = 24 + runtime::LANE_ROOTS.len() + lane_inv::INV_ROOTS.len();
    assert!(runtime::ROOTS.len() > first_c2r, "no c2r roots to check");
    let is_set = |mask: runtime::Mask, index: usize| mask[index / 64] & (1u64 << (index % 64)) != 0;
    let all = runtime::parse_selection(b"all").unwrap();
    let all_but_one = runtime::parse_selection(b"all,-0x5a2f0").unwrap();
    for index in first_c2r..runtime::ROOTS.len() {
        let va = runtime::ROOTS[index];
        assert!(!is_set(all, index), "all enables c2r root {va:#x}");
        assert!(
            !is_set(all_but_one, index),
            "all,-x enables c2r root {va:#x}"
        );
        let explicit = runtime::parse_selection(format!("{va:#x}").as_bytes()).unwrap();
        assert_eq!(explicit, runtime::bit(index), "explicit {va:#x}");
    }
    assert!(
        is_set(all, 0) && is_set(all, first_c2r - 1),
        "adopted roots stay in all"
    );
}

#[test]
fn lane_roots_are_selectable() {
    assert_eq!(
        runtime::ROOTS.len(),
        runtime::HOST_ROOTS.len()
            + runtime::LANE_ROOTS.len()
            + lane_inv::INV_ROOTS.len()
            + tsfp_lane_c2r::ROOTS.len()
    );
    assert_eq!(&runtime::ROOTS[24..69], &runtime::LANE_ROOTS[..]);
    assert_eq!(
        &runtime::ROOTS[69..69 + lane_inv::INV_ROOTS.len()],
        &lane_inv::INV_ROOTS[..]
    );
    assert_eq!(runtime::parse_selection(b"0x45d60"), Some(runtime::bit(24)));
    assert_eq!(
        runtime::parse_selection(b"0x2d74b0"),
        Some(runtime::bit(67))
    );
    assert_eq!(
        runtime::parse_selection(b"71c70,2d74b0"),
        Some(mask_of(&[2, 8, 0, 0, 0, 0, 0]))
    );
    let mut all = runtime::ROOTS.to_vec();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), runtime::ROOTS.len());
}

#[test]
fn inventory_and_pose_roots_cross_the_word_boundary() {
    // root 69 is the first inventory/pose root, root 128 starts the third word, root 196 is in the fourth, and root 403 starts the seventh word.
    let first = runtime::ROOTS[69];
    let word_two = runtime::ROOTS[128];
    let last = runtime::ROOTS[403];
    assert_eq!(
        runtime::parse_selection(format!("{first:x}").as_bytes()),
        Some(mask_of(&[0, 1 << 5, 0, 0, 0, 0, 0]))
    );
    assert_eq!(
        runtime::parse_selection(format!("{word_two:x}").as_bytes()),
        Some(mask_of(&[0, 0, 1, 0, 0, 0, 0]))
    );
    assert_eq!(
        runtime::parse_selection(format!("{last:x}").as_bytes()),
        Some(mask_of(&[0, 0, 0, 0, 0, 0, 1 << 19]))
    );
}

#[test]
fn t2031_exclusion_tokens_remove_roots_from_all() {
    let all = runtime::parse_selection(b"all").unwrap();
    let without_one = runtime::parse_selection(b"all,-0x5a2f0").unwrap();
    let index = runtime::ROOTS.iter().position(|&va| va == 0x5a2f0).unwrap();
    assert_eq!(
        all[index / 64] & !without_one[index / 64],
        1u64 << (index % 64)
    );
    let without_anim = runtime::parse_selection(b"all,-anim").unwrap();
    let mut checked = 0;
    for va in anim_lane::ROOTS
        .iter()
        .chain(&anim_lane2::ROOTS)
        .chain(&anim_lane3::ROOTS)
    {
        let Some(i) = runtime::ROOTS.iter().position(|root| root == va) else {
            continue;
        }; // a lane3 root not registered as a lane root
        checked += 1;
        assert_eq!(
            without_anim[i / 64] & (1u64 << (i % 64)),
            0,
            "{va:#x} must be excluded"
        );
    }
    assert!(checked >= 20, "checked {checked}");
    let kept: u32 = without_anim.iter().map(|w| w.count_ones()).sum();
    let total: u32 = all.iter().map(|w| w.count_ones()).sum();
    assert!(
        kept > total - 30 && kept < total,
        "only the anim roots are removed ({kept} of {total})"
    );
    for bad in ["-anim", "all,-", "all,-zz", "all,-all", "-0x5a2f0"] {
        assert_eq!(runtime::parse_selection(bad.as_bytes()), None, "{bad}");
    }
}

/// T2141 guard: every `crate::<module>` named by runtime.rs or by any `../src` file this test includes must be declared in this
/// file, which includes them by path and so does not see lib.rs. A lane added without its test declaration fails with the name.
#[test]
fn every_module_the_included_sources_use_is_declared_here() {
    let this_source = include_str!("runtime.rs");
    let weapon_owner_imports = this_source
        .split("use tsfp_weapon_owner::{")
        .nth(1)
        .and_then(|imports| imports.split("};").next())
        .unwrap_or("");
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec!["runtime.rs".to_string()];
    for part in this_source.split("#[path = \"../src/").skip(1) {
        if let Some(end) = part.find('"') {
            files.push(part[..end].to_string());
        }
    }
    assert!(files.len() > 40, "found {} included files", files.len());
    let mut missing = std::collections::BTreeSet::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(src.join(file)) else {
            continue;
        }; // c2r/mod.rs children are checked through their parent
        for (at, _) in text.match_indices("crate::") {
            let name: String = text[at + 7..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let declared = this_source.contains(&format!("mod {name};"))
                || this_source.contains(&format!("mod {name} {{"))
                || weapon_owner_imports
                    .split(',')
                    .any(|imported| imported.trim() == name);
            if !declared {
                missing.insert(format!("{name} (used by {file})"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "tests/runtime.rs must declare: {missing:?}"
    );
}

#[test]
fn cutscene_presentation_roots_are_selectable_and_grouped() {
    let group = runtime::parse_selection(b"cutscene").unwrap();
    for &va in &cutscene_presentation_lane::ROOTS {
        let index = runtime::ROOT_MAP
            .find(va)
            .expect("presentation root missing host selector");
        let mask = runtime::parse_selection(format!("{va:#x}").as_bytes()).unwrap();
        assert_eq!(
            mask[index / 64] & (1u64 << (index % 64)),
            1u64 << (index % 64)
        );
        assert_ne!(group[index / 64] & (1u64 << (index % 64)), 0);
    }
}

/// T2322: default `all` may select registered presentation roots whose native
/// preset is not admitted. Refuse before any FP/guest access and retain CPU state.
#[test]
fn default_all_declines_unadmitted_cutscene_roots_at_selector() {
    let spec = b"all";
    assert!(unsafe { runtime::tsfp_rust_configure(spec.as_ptr(), spec.len()) });
    let mut cpu = lane::LaneCpu {
        eax: 11,
        ecx: 22,
        edx: 33,
        ebx: 44,
        esi: 55,
        edi: 66,
        ebp: 77,
        esp: 88,
    };
    let before = cpu;
    for va in cutscene_presentation_lane::ROOTS {
        assert!(cutscene_presentation_lane::declined(va));
        assert!(!unsafe { runtime::tsfp_rust_try_lane(va, &mut cpu, 0) });
        assert_eq!(cpu, before);
    }
    let off = b"off";
    assert!(unsafe { runtime::tsfp_rust_configure(off.as_ptr(), off.len()) });
}

// T2322: selecting a declined root must never reach render-host services either.
macro_rules! deny_render_host {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => { $(
        #[unsafe(no_mangle)]
        extern "C" fn $name($($arg: $ty),*) -> $ret {
            $(let _ = $arg;)*
            panic!("no render host in the selector test")
        }
    )* };
}
deny_render_host! {
    tsfp_render_trace_binary_active() -> i32;
    tsfp_render_trace_native_push_method() -> i32;
    tsfp_render_trace_native_vertex_bindings() -> i32;
    tsfp_render_trace_native_state_handler(index: u32) -> i32;
    tsfp_render_trace_fast_hooks() -> i32;
    tsfp_render_trace_dropping() -> i32;
    tsfp_render_trace_texel_budget() -> u32;
    tsfp_render_trace_binary_send(bytes: *const u8, length: usize, presents: u32) -> ();
    tsfp_render_trace_enabled() -> i32;
    tsfp_render_trace_line(text: *const u8, length: usize) -> ();
    tsfp_render_trace_virtual_of_physical(physical: u32, length: u32) -> u32;
    tsfp_render_trace_readable(address: u32, length: u32) -> u32;
    tsfp_render_trace_now_ns() -> u64;
    tsfp_render_trace_stats_enabled() -> i32;
    tsfp_render_trace_stderr(text: *const u8, length: usize) -> ();
}

/// Unknown INV handlers refuse without storing FP, calling the guest or touching
/// an unmapped guest window. Repeated refusals issue only the first VA warning.
#[test]
fn unknown_inv_root_refuses_and_preserves_cpu() {
    let mut cpu = lane::LaneCpu {
        eax: 11,
        ecx: 22,
        edx: 33,
        ebx: 44,
        esi: 55,
        edi: 66,
        ebp: 77,
        esp: 88,
    };
    let before = cpu;
    let mut memory = unsafe { memory::MappedGuest::new(0) };
    TEST_FP_LOAD_ALLOWED.set(true);
    for _ in 0..2 {
        assert!(!lane_inv::execute(0xdeadbeef, &mut memory, &mut cpu));
        assert_eq!(cpu, before);
    }
    TEST_FP_LOAD_ALLOWED.set(false);
}
