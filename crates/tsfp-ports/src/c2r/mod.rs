// SPDX-License-Identifier: GPL-3.0-or-later
//! Mechanically translated functions (`python3 -m tools.c2rust`, T1878, docs/t-c2rust.md).
//! Generated index: one `pub mod` per batch file in this directory. Not admitted to any selector.
#![allow(
    clippy::all,
    dead_code,
    unused_assignments,
    unused_mut,
    unused_parens,
    unused_variables,
    unused_labels,
    unused_comparisons,
    unreachable_code,
    unreachable_patterns
)]
pub mod rt;
pub use tsfp_port_ai_behavior::c2r::ai_behavior;
pub use tsfp_port_ai_characters::c2r::t1954_characters;
pub use tsfp_port_ai_pathfinding::c2r::ai_pathfinding;
pub use tsfp_port_engine_object::c2r::engine_object_6;
pub use tsfp_port_gameplay_modes::c2r::modes_deferred;
pub use tsfp_port_gameplay_player::c2r::gameplay_player_2;
pub mod data_missions;
pub use tsfp_port_data_missions_2::c2r::data_missions_2;
pub use tsfp_port_data_missions_3::c2r::data_missions_3;
pub use tsfp_port_data_missions::c2r::data_missions_4;
pub use tsfp_port_data_missions_4::c2r::missions_follow;
pub use tsfp_port_data_text::c2r::data_text;
pub mod engine_anim;
pub use tsfp_port_engine_input::c2r::engine_input;
pub use tsfp_port_engine_object_2::c2r::engine_object;
pub use tsfp_port_engine_object_3::c2r::engine_object_2;
pub use tsfp_port_engine_object_4::c2r::engine_object_3;
pub use tsfp_port_engine_object_5::c2r::engine_object_4;
pub use tsfp_port_engine_object_6::c2r::engine_object_5;
pub use tsfp_port_engine_physics::c2r::engine_physics;
pub use tsfp_port_library_xdk::c2r::frontier_001;
pub use tsfp_port_gameplay_inventory::c2r::gameplay_inventory;
pub use tsfp_port_gameplay_player_2::c2r::gameplay_player;
pub use tsfp_port_gameplay_player::c2r::gameplay_player_3;
pub use tsfp_port_gameplay_player::c2r::gameplay_player_stats;
pub use tsfp_port_data_missions::c2r::gamestate_wired;
pub use tsfp_port_data_missions::c2r::leaf2_wired;
pub use tsfp_port_engine_input::c2r::leaf3_wired;
pub use tsfp_port_data_missions::c2r::leaf4_wired;
pub use tsfp_port_gameplay_inventory::c2r::leaf5_wired;
pub use tsfp_port_gameplay_player::c2r::leaf_wired;
pub use tsfp_port_mapmaker_editor::c2r::mapmaker_editor;
pub use tsfp_port_mapmaker_editor::c2r::mapmaker_editor_2;
pub use tsfp_port_mapmaker_editor::c2r::mapmaker_editor_3;
pub use tsfp_port_mapmaker_ui::c2r::mapmaker_ui;
pub mod modes_deferred_readable;
pub use tsfp_port_network_live_2::c2r::network_live;
pub use tsfp_port_network_live_2::c2r::network_live_2;
pub use tsfp_port_network_live::c2r::network_live_3;
pub use tsfp_port_network_live::c2r::network_live_4;
pub use tsfp_port_network_live::c2r::network_live_5;
pub use tsfp_port_network_live::c2r::network_live_6;
pub use tsfp_port_network_session::c2r::network_session;
pub use tsfp_port_network_session::c2r::network_session_2;
pub use tsfp_port_network_session::c2r::network_session_3;
pub use tsfp_port_engine_object_3::c2r::objchar_wired;
pub use tsfp_port_engine_object::c2r::objchar_wired10;
pub use tsfp_port_engine_object::c2r::objchar_wired11;
pub use tsfp_port_data_missions::c2r::objchar_wired12;
pub use tsfp_port_engine_object::c2r::objchar_wired13;
pub use tsfp_port_engine_object::c2r::objchar_wired14;
pub use tsfp_port_ai_characters::c2r::objchar_wired15;
pub use tsfp_port_engine_object::c2r::objchar_wired17;
pub use tsfp_port_gameplay_player::c2r::objchar_wired18;
pub use tsfp_port_engine_object_6::c2r::objchar_wired2;
pub use tsfp_port_gameplay_player::c2r::objchar_wired20;
pub use tsfp_port_gameplay_player::c2r::objchar_wired21;
pub use tsfp_port_gameplay_player::c2r::objchar_wired22;
pub use tsfp_port_gameplay_player::c2r::objchar_wired23;
pub use tsfp_port_gameplay_weapons::c2r::objchar_wired24;
pub use tsfp_port_engine_object::c2r::objchar_wired27;
pub use tsfp_port_gameplay_player::c2r::objchar_wired28;
pub mod objchar_wired3;
pub use tsfp_port_ai_pathfinding::c2r::objchar_wired30;
pub use tsfp_port_engine_object::c2r::objchar_wired31;
pub use tsfp_port_engine_object_2::c2r::objchar_wired32;
pub use tsfp_port_data_missions::c2r::objchar_wired33;
pub use tsfp_port_engine_object::c2r::objchar_wired34;
pub use tsfp_port_engine_render_effects_2::c2r::objchar_wired35;
pub use tsfp_port_engine_object_2::c2r::objchar_wired36;
pub use tsfp_port_engine_math::c2r::objchar_wired4;
pub use tsfp_port_engine_math::c2r::t3398_math4;
pub use tsfp_port_engine_object_2::c2r::objchar_wired5;
pub use tsfp_port_engine_object_2::c2r::objchar_wired6;
pub use tsfp_port_engine_object_2::c2r::objchar_wired7;
pub use tsfp_port_engine_object_2::c2r::objchar_wired8;
pub use tsfp_port_engine_object_3::c2r::objchar_wired9;
pub use tsfp_port_gameplay_weapons::c2r::player2_wired;
pub use tsfp_port_gameplay_weapons::c2r::player3_wired;
pub use tsfp_port_gameplay_player::c2r::player_wired;
pub use tsfp_port_data_save::c2r::save_menu_001;
pub use tsfp_port_mapmaker_storage::c2r::save_mapmaker_002;
pub use tsfp_port_engine_input::c2r::t1892_adapters;
pub use tsfp_port_engine_input::c2r::t1892_input;
pub use tsfp_port_ai_pathfinding::c2r::t1892_pathfinding;
pub use tsfp_port_ui_settings::c2r::t1892_settings;
// Keep the translator's layout: rustfmt would reflow the generated module for local ABI-edge edits.
#[rustfmt::skip]
pub mod t1925_weapons;
pub use tsfp_port_ai_characters::c2r::t1926_characters;
pub mod t1937_math;
pub use tsfp_port_gameplay_weapons_2::c2r::t1940_weapons;
pub use tsfp_port_ai_characters::c2r::t1941_characters;
pub mod t1953_manual;
pub use tsfp_port_gameplay_weapons::c2r::t1953_weapons;
pub use tsfp_port_engine_object_6::c2r::t1966_sweep;
pub use tsfp_port_ui_framework::c2r::t1973_sweep;
pub use tsfp_port_engine_render_d3d::c2r::t1976_render;
pub use tsfp_port_data_cutscenes::c2r::t2006_001;
pub use tsfp_port_engine_render_effects_2::c2r::t2006_002;
pub use tsfp_port_library_xdk::c2r::t2008_integer;
pub use tsfp_port_library_xdk::c2r::t2008_mmx;
pub use tsfp_port_library_xdk::c2r::t2008_mmx_fixture;
pub use tsfp_port_library_xdk::c2r::t2008_original;
pub use tsfp_port_library_crt::c2r::t2008_seh;
pub use tsfp_port_library_crt::c2r::t2008_switch;
pub use tsfp_port_library_xdk::c2r::t2008_tail;
pub use tsfp_port_data_missions::c2r::t2040_missions;
pub use tsfp_port_data_missions::c2r::t2040_missions_2;
pub use tsfp_port_data_missions::c2r::t2040_missions_3;
pub use tsfp_port_misc::c2r::t2049_cls_001;
pub mod t2049_exec_001;
pub mod t2049_exec_002;
pub mod t2049_exec_003;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_exec_004;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_exec_005;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_exec_006;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_exec_007;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_exec_008;
pub use tsfp_port_data_cutscenes::c2r::t2049_exec_009;
pub mod t2049_manual_actions;
pub mod t2049_manual_links;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_nx_001;
pub use tsfp_port_engine_object_2::c2r::t2049_nx_002;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_nx_003;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_nx_004;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_nx_005;
pub use tsfp_port_engine_render_effects::c2r::t2049_nx_006;
pub use tsfp_port_engine_render_effects_2::c2r::t2049_nx_007;
pub use tsfp_port_data_cutscenes::c2r::t2049_nx_008;
pub use tsfp_port_data_save::c2r::t2049_r3_save_001;
pub use tsfp_port_mapmaker_storage::c2r::t2049_r3_storage_001;
pub use tsfp_port_engine_render_effects_3::c2r::t2049_remote10_effects_001;
pub use tsfp_port_engine_render_effects::c2r::t2049_remote10_effects_002;
pub use tsfp_port_engine_render_effects_3::c2r::t2049_remote11_effects_001;
pub use tsfp_port_engine_render_effects::c2r::t2049_remote12_effects_001;
pub mod t2049_remote13_all_001;
pub use tsfp_port_data_cutscenes::c2r::t2049_remote2_cutscenes_001;
pub use tsfp_port_mapmaker_modes::c2r::t2049_remote2_mapmaker_001;
pub use tsfp_port_gameplay_modes::c2r::t2049_remote2_modes_001;
pub use tsfp_port_engine_render_effects::c2r::t2049_remote3_effects_001;
pub use tsfp_port_engine_render_effects_3::c2r::t2049_remote4_effects_001;
pub use tsfp_port_engine_render_effects::c2r::t2049_remote5_effects_001;
pub use tsfp_port_engine_render_effects::c2r::t2049_remote7_effects_001;
pub use tsfp_port_engine_render_effects::c2r::t2049_remote8_effects_001;
pub use tsfp_port_engine_render_effects_3::c2r::t2049_remote9_effects_001;
pub use tsfp_port_engine_render_effects::c2r::t2049_remote9_effects_002;
pub use tsfp_port_engine_object::c2r::t2049_w23_001;
pub use tsfp_port_engine_object::c2r::t2049_w23_002;
pub use tsfp_port_mapmaker_editor::c2r::t2049_w23_003;
pub use tsfp_port_data_missions::c2r::t2049_w23_004;
pub use tsfp_port_engine_render_effects::c2r::t2049_w23_005;
pub use tsfp_port_engine_render_effects::c2r::t2049_w23_006;
pub use tsfp_port_engine_object::c2r::t2049_w23_007;
pub use tsfp_port_engine_render_effects::c2r::t2049_w23_008;
pub use tsfp_port_engine_object::c2r::t2049_w34_001;
pub use tsfp_port_ui_hud::c2r::t2049_w34_002;
pub use tsfp_port_engine_object::c2r::t2049_w34_003;
pub use tsfp_port_engine_render_effects::c2r::t2049_w34_004;
pub use tsfp_port_engine_render_effects::c2r::t2049_w34_005;
pub use tsfp_port_engine_render_effects::c2r::t2049_w34_006;
pub use tsfp_port_engine_render_effects::c2r::t2049_w34_007;
pub use tsfp_port_engine_render_effects::c2r::t2049_w34_008;
pub use tsfp_port_ui_hud::c2r::t2049_w34_imul16_001;
pub use tsfp_port_engine_object::c2r::t2049_w7_001;
pub use tsfp_port_network_syslink::c2r::t2049_w7_002;
pub use tsfp_port_engine_object::c2r::t2049_w7_003;
pub use tsfp_port_engine_render_effects::c2r::t2049_w7_004;
pub use tsfp_port_engine_object::c2r::t2049_w7_005;
pub use tsfp_port_engine_object::c2r::t2049_w7_006;
pub use tsfp_port_ai_behavior_2::c2r::t2064_flags;
pub use tsfp_port_engine_object::c2r::t2064_flags_2;
pub use tsfp_port_data_missions::c2r::t2064_missions;
pub use tsfp_port_engine_object::c2r::t2069_live_frame;
pub use tsfp_port_mapmaker_editor::c2r::t2086_alloc;
pub mod t2123_manual;
pub use tsfp_port_engine_render_effects::c2r::t2125_001;
pub use tsfp_port_ai_behavior::c2r::t2125_002;
pub use tsfp_port_ui_menu::c2r::t2125_003;
pub use tsfp_port_library_xdk::c2r::t2125_remote10_001;
pub use tsfp_port_library_xdk::c2r::t2125_remote11_001;
pub use tsfp_port_library_xdk::c2r::t2125_remote12_001;
pub use tsfp_port_library_xdk::c2r::t2125_remote13_001;
pub use tsfp_port_library_crt::c2r::t2125_remote14_001;
pub use tsfp_port_library_crt::c2r::t2125_remote15;
pub use tsfp_port_library_xdk::c2r::t2125_remote7_001;
pub use tsfp_port_library_xdk_2::c2r::t2125_remote8_001;
pub use tsfp_port_library_xdk_2::c2r::t2125_remote9_001;
pub use tsfp_port_library_xdk_2::c2r::t2125_remote_001;
pub use tsfp_port_misc::c2r::t2125c_001;
pub use tsfp_port_engine_render_effects_2::c2r::t2125c_002;
pub use tsfp_port_data_cutscenes::c2r::t2125c_003;
pub mod t2125c_manual;
pub use tsfp_port_library_crt::c2r::t2125d_001;
pub use tsfp_port_library_xdk_2::c2r::t2125d_003;
pub use tsfp_port_library_crt::c2r::t2125d_004;
pub use tsfp_port_library_crt::c2r::t2125d_005;
pub use tsfp_port_library_xdk_2::c2r::t2125e_001;
pub use tsfp_port_misc::c2r::t2128_001;
pub use tsfp_port_data_missions::c2r::t2378_terminal_reference;
pub use tsfp_port_ai_characters::c2r::t2435_18af40;
pub use tsfp_port_ui_framework_2::c2r::ui_framework;
pub use tsfp_port_ui_framework::c2r::ui_framework_2;
pub mod ui_hud;
pub mod ui_hud_2;
pub use tsfp_port_ui_menu::c2r::ui_menu;
pub use tsfp_port_ui_menu_2::c2r::ui_menu_2;
pub use tsfp_port_ui_menu_3::c2r::ui_menu_3;
pub use tsfp_port_ui_menu::c2r::ui_menu_4;
pub use tsfp_port_ui_menu::c2r::ui_menu_t2042;
pub use tsfp_port_ui_menu_2::c2r::ui_menu_t2042_dispatch;
pub mod ui_menu_t2042_readable;
pub use tsfp_port_ui_settings::c2r::ui_settings;
pub use tsfp_port_engine_physics::c2r::widemul_t1980;

use crate::abi::{ExtRegisters, GuestCall};
use crate::memory::GuestMemory;

/// Runs the translation of `va` if any generated module holds it.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    false
        || crate::missions_reader::selected(va, m, guest, regs)
        || crate::missions_writer::run(va, m, guest, regs)
        || crate::objchar_writer::run(va, m, guest, regs)
        || crate::objchar_tail_writer::run(va, m, guest, regs)
        || crate::objchar_player_writer::run(va, m, guest, regs)
        || gamestate_wired::run(va, m, guest, regs)
        || leaf2_wired::run(va, m, guest, regs)
        || leaf3_wired::run(va, m, guest, regs)
        || leaf4_wired::run(va, m, guest, regs)
        || leaf5_wired::run(va, m, guest, regs)
        || leaf_wired::run(va, m, guest, regs)
        || modes_deferred_readable::run(va, m, guest, regs)
        || objchar_wired::run(va, m, guest, regs)
        || player2_wired::run(va, m, guest, regs)
        || player3_wired::run(va, m, guest, regs)
        || player_wired::run(va, m, guest, regs)
        || ui_menu_t2042_readable::run(va, m, guest, regs)
        || data_missions::run(va, m, guest, regs)
        || data_missions_2::run(va, m, guest, regs)
        || data_missions_3::run(va, m, guest, regs)
        || data_missions_4::run(va, m, guest, regs)
        || data_text::run(va, m, guest, regs)
        || engine_anim::run(va, m, guest, regs)
        || engine_input::run(va, m, guest, regs)
        || engine_object::run(va, m, guest, regs)
        || engine_object_2::run(va, m, guest, regs)
        || engine_object_3::run(va, m, guest, regs)
        || engine_object_4::run(va, m, guest, regs)
        || engine_object_5::run(va, m, guest, regs)
        || engine_physics::run(va, m, guest, regs)
        || frontier_001::run(va, m, guest, regs)
        || gameplay_inventory::run(va, m, guest, regs)
        || gameplay_player::run(va, m, guest, regs)
        || gameplay_player_3::run(va, m, guest, regs)
        || gameplay_player_stats::run(va, m, guest, regs)
        || mapmaker_editor::run(va, m, guest, regs)
        || mapmaker_editor_2::run(va, m, guest, regs)
        || mapmaker_editor_3::run(va, m, guest, regs)
        || mapmaker_ui::run(va, m, guest, regs)
        || network_live::run(va, m, guest, regs)
        || network_live_2::run(va, m, guest, regs)
        || network_live_3::run(va, m, guest, regs)
        || network_live_4::run(va, m, guest, regs)
        || network_live_5::run(va, m, guest, regs)
        || network_live_6::run(va, m, guest, regs)
        || network_session::run(va, m, guest, regs)
        || network_session_2::run(va, m, guest, regs)
        || network_session_3::run(va, m, guest, regs)
        || objchar_wired10::run(va, m, guest, regs)
        || objchar_wired11::run(va, m, guest, regs)
        || objchar_wired12::run(va, m, guest, regs)
        || objchar_wired13::run(va, m, guest, regs)
        || objchar_wired14::run(va, m, guest, regs)
        || objchar_wired15::run(va, m, guest, regs)
        || objchar_wired17::run(va, m, guest, regs)
        || objchar_wired18::run(va, m, guest, regs)
        || objchar_wired2::run(va, m, guest, regs)
        || objchar_wired20::run(va, m, guest, regs)
        || objchar_wired21::run(va, m, guest, regs)
        || objchar_wired22::run(va, m, guest, regs)
        || objchar_wired23::run(va, m, guest, regs)
        || objchar_wired24::run(va, m, guest, regs)
        || objchar_wired27::run(va, m, guest, regs)
        || objchar_wired28::run(va, m, guest, regs)
        || objchar_wired3::run(va, m, guest, regs)
        || objchar_wired30::run(va, m, guest, regs)
        || objchar_wired31::run(va, m, guest, regs)
        || objchar_wired32::run(va, m, guest, regs)
        || objchar_wired33::run(va, m, guest, regs)
        || objchar_wired34::run(va, m, guest, regs)
        || objchar_wired35::run(va, m, guest, regs)
        || objchar_wired36::run(va, m, guest, regs)
        || objchar_wired4::run(va, m, guest, regs)
        || t3398_math4::run(va, m, guest, regs)
        || objchar_wired5::run(va, m, guest, regs)
        || objchar_wired6::run(va, m, guest, regs)
        || objchar_wired7::run(va, m, guest, regs)
        || objchar_wired8::run(va, m, guest, regs)
        || objchar_wired9::run(va, m, guest, regs)
        || save_mapmaker_002::run(va, m, guest, regs)
        || save_menu_001::run(va, m, guest, regs)
        || t1892_adapters::run(va, m, guest, regs)
        || t1892_input::run(va, m, guest, regs)
        || t1892_pathfinding::run(va, m, guest, regs)
        || t1892_settings::run(va, m, guest, regs)
        || t1925_weapons::run(va, m, guest, regs)
        || t1926_characters::run(va, m, guest, regs)
        || t1937_math::run(va, m, guest, regs)
        || t1940_weapons::run(va, m, guest, regs)
        || t1941_characters::run(va, m, guest, regs)
        || t1953_manual::run(va, m, guest, regs)
        || t1953_weapons::run(va, m, guest, regs)
        || t1966_sweep::run(va, m, guest, regs)
        || t1973_sweep::run(va, m, guest, regs)
        || t1976_render::run(va, m, guest, regs)
        || t2006_001::run(va, m, guest, regs)
        || t2006_002::run(va, m, guest, regs)
        || t2008_integer::run(va, m, guest, regs)
        || t2008_mmx::run(va, m, guest, regs)
        || t2008_mmx_fixture::run(va, m, guest, regs)
        || t2008_original::run(va, m, guest, regs)
        || t2008_seh::run(va, m, guest, regs)
        || t2008_switch::run(va, m, guest, regs)
        || t2008_tail::run(va, m, guest, regs)
        || t2040_missions::run(va, m, guest, regs)
        || t2040_missions_2::run(va, m, guest, regs)
        || t2040_missions_3::run(va, m, guest, regs)
        || t2049_cls_001::run(va, m, guest, regs)
        || t2049_exec_001::run(va, m, guest, regs)
        || t2049_exec_002::run(va, m, guest, regs)
        || t2049_exec_003::run(va, m, guest, regs)
        || t2049_exec_004::run(va, m, guest, regs)
        || t2049_exec_005::run(va, m, guest, regs)
        || t2049_exec_006::run(va, m, guest, regs)
        || t2049_exec_007::run(va, m, guest, regs)
        || t2049_exec_008::run(va, m, guest, regs)
        || t2049_exec_009::run(va, m, guest, regs)
        || t2049_manual_actions::run(va, m, guest, regs)
        || t2049_manual_links::run(va, m, guest, regs)
        || t2049_nx_001::run(va, m, guest, regs)
        || t2049_nx_002::run(va, m, guest, regs)
        || t2049_nx_003::run(va, m, guest, regs)
        || t2049_nx_004::run(va, m, guest, regs)
        || t2049_nx_005::run(va, m, guest, regs)
        || t2049_nx_006::run(va, m, guest, regs)
        || t2049_nx_007::run(va, m, guest, regs)
        || t2049_nx_008::run(va, m, guest, regs)
        || t2049_r3_save_001::run(va, m, guest, regs)
        || t2049_r3_storage_001::run(va, m, guest, regs)
        || t2049_remote10_effects_001::run(va, m, guest, regs)
        || t2049_remote10_effects_002::run(va, m, guest, regs)
        || t2049_remote11_effects_001::run(va, m, guest, regs)
        || t2049_remote12_effects_001::run(va, m, guest, regs)
        || t2049_remote13_all_001::run(va, m, guest, regs)
        || t2049_remote2_cutscenes_001::run(va, m, guest, regs)
        || t2049_remote2_mapmaker_001::run(va, m, guest, regs)
        || t2049_remote2_modes_001::run(va, m, guest, regs)
        || t2049_remote3_effects_001::run(va, m, guest, regs)
        || t2049_remote4_effects_001::run(va, m, guest, regs)
        || t2049_remote5_effects_001::run(va, m, guest, regs)
        || t2049_remote7_effects_001::run(va, m, guest, regs)
        || t2049_remote8_effects_001::run(va, m, guest, regs)
        || t2049_remote9_effects_001::run(va, m, guest, regs)
        || t2049_remote9_effects_002::run(va, m, guest, regs)
        || t2049_w23_001::run(va, m, guest, regs)
        || t2049_w23_002::run(va, m, guest, regs)
        || t2049_w23_003::run(va, m, guest, regs)
        || t2049_w23_004::run(va, m, guest, regs)
        || t2049_w23_005::run(va, m, guest, regs)
        || t2049_w23_006::run(va, m, guest, regs)
        || t2049_w23_007::run(va, m, guest, regs)
        || t2049_w23_008::run(va, m, guest, regs)
        || t2049_w34_001::run(va, m, guest, regs)
        || t2049_w34_002::run(va, m, guest, regs)
        || t2049_w34_003::run(va, m, guest, regs)
        || t2049_w34_004::run(va, m, guest, regs)
        || t2049_w34_005::run(va, m, guest, regs)
        || t2049_w34_006::run(va, m, guest, regs)
        || t2049_w34_007::run(va, m, guest, regs)
        || t2049_w34_008::run(va, m, guest, regs)
        || t2049_w34_imul16_001::run(va, m, guest, regs)
        || t2049_w7_001::run(va, m, guest, regs)
        || t2049_w7_002::run(va, m, guest, regs)
        || t2049_w7_003::run(va, m, guest, regs)
        || t2049_w7_004::run(va, m, guest, regs)
        || t2049_w7_005::run(va, m, guest, regs)
        || t2049_w7_006::run(va, m, guest, regs)
        || t2064_flags::run(va, m, guest, regs)
        || t2064_flags_2::run(va, m, guest, regs)
        || t2064_missions::run(va, m, guest, regs)
        || t2069_live_frame::run(va, m, guest, regs)
        || t2086_alloc::run(va, m, guest, regs)
        || t2123_manual::run(va, m, guest, regs)
        || t2125_001::run(va, m, guest, regs)
        || t2125_002::run(va, m, guest, regs)
        || t2125_003::run(va, m, guest, regs)
        || t2125_remote10_001::run(va, m, guest, regs)
        || t2125_remote11_001::run(va, m, guest, regs)
        || t2125_remote12_001::run(va, m, guest, regs)
        || t2125_remote13_001::run(va, m, guest, regs)
        || t2125_remote14_001::run(va, m, guest, regs)
        || t2125_remote15::run(va, m, guest, regs)
        || t2125_remote7_001::run(va, m, guest, regs)
        || t2125_remote8_001::run(va, m, guest, regs)
        || t2125_remote9_001::run(va, m, guest, regs)
        || t2125_remote_001::run(va, m, guest, regs)
        || t2125c_001::run(va, m, guest, regs)
        || t2125c_002::run(va, m, guest, regs)
        || t2125c_003::run(va, m, guest, regs)
        || t2125c_manual::run(va, m, guest, regs)
        || t2125d_001::run(va, m, guest, regs)
        || t2125d_003::run(va, m, guest, regs)
        || t2125d_004::run(va, m, guest, regs)
        || t2125d_005::run(va, m, guest, regs)
        || t2125e_001::run(va, m, guest, regs)
        || t2128_001::run(va, m, guest, regs)
        || t2378_terminal_reference::run(va, m, guest, regs)
        || t2435_18af40::run(va, m, guest, regs)
        || ui_framework::run(va, m, guest, regs)
        || ui_framework_2::run(va, m, guest, regs)
        || ui_hud::run(va, m, guest, regs)
        || ui_hud_2::run(va, m, guest, regs)
        || ui_menu::run(va, m, guest, regs)
        || ui_menu_2::run(va, m, guest, regs)
        || ui_menu_3::run(va, m, guest, regs)
        || ui_menu_4::run(va, m, guest, regs)
        || ui_menu_t2042::run(va, m, guest, regs)
        || ui_menu_t2042_dispatch::run(va, m, guest, regs)
        || ui_settings::run(va, m, guest, regs)
        || widemul_t1980::run(va, m, guest, regs)
}

/// # Safety
/// `regs` is a live `ExtRegisters`, `offset` maps the guest window, `call` is the trampoline.
/// Differential-only entry for tests/c/test_rust_c2r.c.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_c2r_try(
    va: u32,
    regs: *mut ExtRegisters,
    offset: usize,
    call: crate::ext::CallFn,
) -> bool {
    #[cfg(s3_missions_owned_differential)]
    crate::missions_wire::configure(1, |_| true).expect("Missions differential root set");
    let mut memory = unsafe { crate::memory::MappedGuest::new(offset) };
    let mut guest = crate::ext::CGuest(call);
    let regs = unsafe { &mut *regs };
    run(va, &mut memory, &mut guest, regs)
}
#[rustfmt::skip]
pub use tsfp_port_ui_hud_4::c2r::ui_hud_split_001;
#[rustfmt::skip]
pub use tsfp_port_ui_hud_4::c2r::ui_hud_split_002;
#[rustfmt::skip]
pub use tsfp_port_ui_hud_5::c2r::ui_hud_split_003;
#[rustfmt::skip]
pub use tsfp_port_ui_hud_5::c2r::ui_hud_split_004;
#[rustfmt::skip]
pub use tsfp_port_ui_hud_6::c2r::ui_hud_split_005;
#[rustfmt::skip]
pub use tsfp_port_ui_hud::c2r::ui_hud_split_006;
#[rustfmt::skip]
pub use tsfp_port_ui_hud::c2r::ui_hud_2_split_001;
#[rustfmt::skip]
pub use tsfp_port_ui_hud_2::c2r::ui_hud_2_split_002;
#[rustfmt::skip]
pub use tsfp_port_ui_hud_2::c2r::ui_hud_2_split_003;
#[rustfmt::skip]
pub use tsfp_port_ui_hud_3::c2r::ui_hud_2_split_004;
#[rustfmt::skip]
pub use tsfp_port_ui_hud_3::c2r::ui_hud_2_split_005;
#[rustfmt::skip]
pub use tsfp_port_data_missions_6::c2r::data_missions_split_001;
#[rustfmt::skip]
pub use tsfp_port_data_missions_4::c2r::data_missions_split_002;
#[rustfmt::skip]
pub use tsfp_port_data_missions_5::c2r::data_missions_split_003;
#[rustfmt::skip]
pub use tsfp_port_gameplay_weapons_4::c2r::t1925_weapons_split_001;
#[rustfmt::skip]
pub use tsfp_port_gameplay_weapons::c2r::t1925_weapons_split_002;
#[rustfmt::skip]
pub use tsfp_port_gameplay_weapons_3::c2r::t1925_weapons_split_003;
