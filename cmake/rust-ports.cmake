# SPDX-License-Identifier: GPL-3.0-or-later
if(TSFP_RUST_PORTS)
  if(NOT TSFP_PYTHON3)
    find_package(Python3 COMPONENTS Interpreter REQUIRED)
    set(TSFP_PYTHON3 "${Python3_EXECUTABLE}")
  endif()
  find_program(TSFP_RUSTC NAMES rustc REQUIRED)
  find_program(TSFP_CARGO NAMES cargo REQUIRED)
  execute_process(COMMAND ${TSFP_RUSTC} --version OUTPUT_VARIABLE rust_version
                  OUTPUT_STRIP_TRAILING_WHITESPACE)
  if(NOT rust_version STREQUAL "rustc 1.99.1 (58cf8f9c3 2026-10-13)")
    message(FATAL_ERROR "Rust ports require the pinned rustc 1.99.1 (rust-toolchain.toml); see docs/t-rust-lane.md")
  endif()
  if(NOT CMAKE_SIZEOF_VOID_P EQUAL 8 OR NOT CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|AMD64)$")
    message(FATAL_ERROR "The measured hybrid Rust lane requires an x86_64 host")
  endif()
  set(rust_crate "${CMAKE_SOURCE_DIR}/crates/tsfp-ports")
  file(GLOB_RECURSE rust_sources CONFIGURE_DEPENDS "${rust_crate}/src/*.rs")
  # Render modules are included by #[path] in the no_std lane archive. Direct
  # rustc does not emit dep-info here, so edits must be explicit Ninja inputs.
  file(GLOB_RECURSE render_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-render/src/*.rs")
  list(APPEND rust_sources ${render_rust_sources})
  file(GLOB_RECURSE runtime_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-port-runtime/src/*.rs")
  # T3610: the S3 wiring layer (leaf_wire, player_wire, ...) is its own crate between the runtime and the family crates.
  file(GLOB_RECURSE wire_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-port-wire/src/*.rs")
  # T3398: player interaction fast path is a dedicated archive dependency of
  # tsfp-ports. Keep it in the same no_std rustc build graph as runtime/wire.
  file(GLOB_RECURSE player_interaction_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-player-interaction-fast-path/src/*.rs")
  # T3691: actor-spawn binding is a standalone no_std leaf linked by the host archive.
  file(GLOB_RECURSE ai_spawn_binding_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-ai-spawn-binding/src/*.rs")
  file(GLOB_RECURSE ai_selector_transition_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-ai-selector-transition/src/*.rs")
  file(GLOB_RECURSE render_prep_wire_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-render-prep-wire/src/*.rs")
  file(GLOB_RECURSE lighting_t2_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-port-lighting-t2-div-lighting-1/src/*.rs")
  file(GLOB_RECURSE data_cutscene_key_node_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-port-data-cutscene-key-node/src/*.rs")
  file(GLOB_RECURSE scene_t2_div_rust_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-port-scene-t2-div-scene-1/src/*.rs")
  set(family_config "${CMAKE_SOURCE_DIR}/tools/data/c2r_family_crates.json")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${family_config}")
  file(READ "${family_config}" family_config_json)
  string(JSON family_count LENGTH "${family_config_json}" families)
  set(family_rlibs "")
  set(family_externs "")
  list(APPEND rust_sources "${CMAKE_SOURCE_DIR}/crates/tsfp-gameplay/src/cutscene_movie_setup.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input/src/look_gate.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input/src/look_native.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input/src/look_vector.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input/src/view.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-gameplay/src/cutscene_overlays.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-ai/src/movement_approach.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-core/src/leaf.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-memory/src/leaf.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-object/src/lib.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-anim/src/leaf.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-physics/src/leaf.rs"
    # T2197: text_field.rs includes these by path
    "${CMAKE_SOURCE_DIR}/crates/tsfp-text-field/src/keyboard.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-text-field/src/widget.rs")
  # T3605: the ports archive is one large crate: its type check, borrow check and monomorphisation are serial in
  # rustc (about 48 s of an 85 s cold build on 16 idle cores, docs/t3605-build-speed.md). The parallel front end
  # (-Zthreads) is nightly-only; the pinned stable compiler accepts it with RUSTC_BOOTSTRAP=1 (set for this one
  # command only, never for build scripts or registry crates). MEASURED: the archive is byte-identical to the
  # single-threaded one (sha256 equal for 4 and 8 threads). TSFP_RUST_PORTS_THREADS=0 or 1 turns it off, and a
  # compiler that rejects the flag falls back to the plain command.
  set(rust_frontend_env "")
  set(rust_frontend_flags "")
  set(rust_oracle_flags "")
  if(TSFP_LIFTED_ORACLE)
    list(APPEND rust_oracle_flags "--cfg=feature=\"lifted-oracle\"")
  endif()
  set(rust_frontend_threads "$ENV{TSFP_RUST_PORTS_THREADS}")
  if(rust_frontend_threads STREQUAL "")
    if("$ENV{TSFP_BUILD_LOCAL}" STREQUAL "1")
      set(rust_frontend_threads 2)
    else()
      set(rust_frontend_threads 8)
    endif()
  endif()
  if(rust_frontend_threads GREATER 1)
    execute_process(COMMAND ${CMAKE_COMMAND} -E env RUSTC_BOOTSTRAP=1 ${TSFP_RUSTC} -Zthreads=2 --print=sysroot
                    RESULT_VARIABLE rust_frontend_probe OUTPUT_QUIET ERROR_QUIET)
    if(rust_frontend_probe EQUAL 0)
      set(rust_frontend_env ${CMAKE_COMMAND} -E env RUSTC_BOOTSTRAP=1)
      set(rust_frontend_flags -Zthreads=${rust_frontend_threads})
      message(STATUS "Rust ports archive: parallel rustc front end, ${rust_frontend_threads} threads (T3605)")
    endif()
  endif()
  set(rust_archive "${CMAKE_BINARY_DIR}/libtsfp_ports.a")
  set(rust_incremental_dir "${CMAKE_BINARY_DIR}/rustc-ports-incremental")
  set(runtime_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_runtime.rlib")
  # T1990/T3484: lane mutants. state_wire lives in the shared runtime crate, so the cfg must reach it too.
  set(rust_mutation_flags "")
  if(TSFP_RUST_LANE_MUTATION)
    set(rust_mutation_flags --cfg=rust_lane_mutation)
  endif()
  # T3740 (I-T3740-2): tsfp-port-runtime depends on the registry crate `spin` (weapon_hands_hooks.rs). Bare rustc has no
  # registry, so build that one crate here from the offline cargo home, with the features of the runtime's Cargo.toml.
  # tests/test_rust_ports_cmake_externs.py checks that every registry dependency of a crate built here gets an --extern.
  file(STRINGS "${CMAKE_SOURCE_DIR}/Cargo.lock" spin_lock_lines REGEX "^name = \"spin\"$|^version = ")
  set(spin_version "")
  set(spin_next FALSE)
  foreach(spin_line IN LISTS spin_lock_lines)
    if(spin_next AND spin_line MATCHES "^version = \"([^\"]+)\"$")
      set(spin_version "${CMAKE_MATCH_1}")
      break()
    endif()
    set(spin_next FALSE)
    if(spin_line STREQUAL "name = \"spin\"")
      set(spin_next TRUE)
    endif()
  endforeach()
  set(spin_source "")
  foreach(spin_home "$ENV{CARGO_HOME}" "$ENV{TSFP_BEVY_CARGO_HOME}" "${CMAKE_SOURCE_DIR}/tmp/rust/cargo" "$ENV{HOME}/.cargo")
    if(spin_source STREQUAL "" AND NOT spin_home STREQUAL "" AND NOT spin_version STREQUAL "")
      file(GLOB spin_candidates "${spin_home}/registry/src/*/spin-${spin_version}/src/lib.rs")
      if(spin_candidates)
        list(GET spin_candidates 0 spin_source)
      endif()
    endif()
  endforeach()
  if(spin_source STREQUAL "")
    message(FATAL_ERROR "Rust ports: no source of spin ${spin_version} (Cargo.lock) in the cargo home; run `cargo fetch` once or set CARGO_HOME")
  endif()
  set(spin_rlib "${CMAKE_BINARY_DIR}/libspin.rlib")
  add_custom_command(OUTPUT "${spin_rlib}"
    COMMAND ${TSFP_RUSTC} "${spin_source}"
      --crate-name spin --crate-type rlib --edition=2015 --cap-lints allow
      --cfg "feature=\"mutex\"" --cfg "feature=\"spin_mutex\""
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3
      -o "${spin_rlib}"
    DEPENDS "${spin_source}" "${CMAKE_SOURCE_DIR}/Cargo.lock" "${TSFP_RUSTC}"
    VERBATIM)
  add_custom_command(OUTPUT "${runtime_rlib}"
    COMMAND "${TSFP_PYTHON3}" "${CMAKE_SOURCE_DIR}/tools/dev/build_port_runtime.py"
      --root "${CMAKE_SOURCE_DIR}" --output-dir "${CMAKE_BINARY_DIR}"
      --target x86_64-unknown-linux-gnu --cargo "${TSFP_CARGO}"
      ${rust_mutation_flags}
    DEPENDS ${runtime_rust_sources} "${CMAKE_SOURCE_DIR}/crates/tsfp-port-runtime/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/Cargo.toml" "${CMAKE_SOURCE_DIR}/Cargo.lock"
      "${CMAKE_SOURCE_DIR}/tools/dev/build_port_runtime.py"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-memory/src/leaf.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/game_state.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/story_flow.rs"
      "${TSFP_RUSTC}"
    VERBATIM)
  # T3610: tsfp-port-wire (state half of the wiring modules, hooks to the facade). Same flags as the runtime crate; the
  # lane-mutation cfg must reach it because player_wire's mutation hooks live here.
  set(wire_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_wire.rlib")
  add_custom_command(OUTPUT "${wire_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-port-wire/src/lib.rs"
      --crate-name tsfp_port_wire --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-port-wire-incremental ${rust_mutation_flags}
      --extern "tsfp_port_runtime=${runtime_rlib}" -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${wire_rlib}"
    DEPENDS ${wire_rust_sources} "${CMAKE_SOURCE_DIR}/crates/tsfp-port-wire/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/character.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/inventory.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/menu.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/missions.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/missions_owned.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/modes.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/object_pool.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/pad_audio.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/player.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/save_text.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/generated_views.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/values.rs"
      "${runtime_rlib}" "${TSFP_RUSTC}"
    VERBATIM)
  set(player_interaction_rlib "${CMAKE_BINARY_DIR}/libtsfp_player_interaction_fast_path.rlib")
  add_custom_command(OUTPUT "${player_interaction_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-player-interaction-fast-path/src/lib.rs"
      --crate-name tsfp_player_interaction_fast_path --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-player-interaction-fast-path-incremental
      --extern "tsfp_port_runtime=${runtime_rlib}" -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${player_interaction_rlib}"
    DEPENDS ${player_interaction_rust_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-player-interaction-fast-path/Cargo.toml"
      "${runtime_rlib}" "${TSFP_RUSTC}"
    VERBATIM)
  # T2411: one nominal weapon owner shared by facade and realization.
  set(weapon_protocol_rlib "${CMAKE_BINARY_DIR}/libtsfp_weapon_realization_protocol.rlib")
  add_custom_command(OUTPUT "${weapon_protocol_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-weapon-realization-protocol/src/lib.rs"
      --crate-name tsfp_weapon_realization_protocol --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-tsfp-weapon-realization-protocol-incremental ${rust_mutation_flags}
      -L "dependency=${CMAKE_BINARY_DIR}" -o "${weapon_protocol_rlib}"
    DEPENDS "${CMAKE_SOURCE_DIR}/crates/tsfp-weapon-realization-protocol/src/lib.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-weapon-realization-protocol/Cargo.toml" "${TSFP_RUSTC}"
    VERBATIM COMMAND_EXPAND_LISTS)
  set(weapon_rlibs "")
  set(weapon_externs --extern "tsfp_weapon_realization_protocol=${weapon_protocol_rlib}")
  set(weapon_dependencies "${wire_rlib}" "${weapon_protocol_rlib}")
  foreach(weapon_package tsfp-weapon-owner tsfp-weapon-realization)
    string(REPLACE "-" "_" weapon_crate "${weapon_package}")
    set(weapon_rlib "${CMAKE_BINARY_DIR}/lib${weapon_crate}.rlib")
    file(GLOB_RECURSE weapon_sources CONFIGURE_DEPENDS
      "${CMAKE_SOURCE_DIR}/crates/${weapon_package}/src/*.rs")
    if(weapon_package STREQUAL "tsfp-weapon-owner")
      list(APPEND weapon_sources
        "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/scene_membership.rs"
        "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/weapon_cycle.rs"
        "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/weapon_hands.rs"
        "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/weapon_interaction.rs"
        "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/weapon_inventory.rs"
        "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/weapon_payload.rs"
        "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/weapon_projectiles.rs")
    endif()
    add_custom_command(OUTPUT "${weapon_rlib}"
      COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/${weapon_package}/src/lib.rs"
        --crate-name ${weapon_crate} --crate-type rlib --edition=2024
        --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
        -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
        -Cincremental=${CMAKE_BINARY_DIR}/rustc-${weapon_package}-incremental ${rust_mutation_flags}
        --extern "tsfp_port_wire=${wire_rlib}" ${weapon_externs}
        -L "dependency=${CMAKE_BINARY_DIR}" -o "${weapon_rlib}"
      DEPENDS ${weapon_sources} "${CMAKE_SOURCE_DIR}/crates/${weapon_package}/Cargo.toml"
        ${weapon_dependencies} "${TSFP_RUSTC}"
      VERBATIM COMMAND_EXPAND_LISTS)
    list(APPEND weapon_rlibs "${weapon_rlib}")
    list(APPEND weapon_dependencies "${weapon_rlib}")
    list(APPEND weapon_externs --extern "${weapon_crate}=${weapon_rlib}")
  endforeach()
  set(render_prep_wire_rlib "${CMAKE_BINARY_DIR}/libtsfp_render_prep_wire.rlib")
  add_custom_command(OUTPUT "${render_prep_wire_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-render-prep-wire/src/lib.rs"
      --crate-name tsfp_render_prep_wire --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-render-prep-wire-incremental
      -o "${render_prep_wire_rlib}"
    DEPENDS ${render_prep_wire_rust_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-render-prep-wire/Cargo.toml"
      "${TSFP_RUSTC}"
    VERBATIM)
  set(ai_spawn_binding_rlib "${CMAKE_BINARY_DIR}/libtsfp_ai_spawn_binding.rlib")
  add_custom_command(OUTPUT "${ai_spawn_binding_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-ai-spawn-binding/src/lib.rs"
      --crate-name tsfp_ai_spawn_binding --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-ai-spawn-binding-incremental
      --extern "tsfp_port_runtime=${runtime_rlib}" -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${ai_spawn_binding_rlib}"
    DEPENDS ${ai_spawn_binding_rust_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-ai-spawn-binding/Cargo.toml"
      "${runtime_rlib}" "${TSFP_RUSTC}"
    VERBATIM)
  set(ai_selector_transition_rlib "${CMAKE_BINARY_DIR}/libtsfp_ai_selector_transition.rlib")
  add_custom_command(OUTPUT "${ai_selector_transition_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-ai-selector-transition/src/lib.rs"
      --crate-name tsfp_ai_selector_transition --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-ai-selector-transition-incremental
      --extern "tsfp_port_runtime=${runtime_rlib}" -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${ai_selector_transition_rlib}"
    DEPENDS ${ai_selector_transition_rust_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-ai-selector-transition/Cargo.toml"
      "${runtime_rlib}" "${TSFP_RUSTC}"
    VERBATIM)
  set(lighting_t2_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_lighting_t2_div_lighting_1.rlib")
  add_custom_command(OUTPUT "${lighting_t2_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-port-lighting-t2-div-lighting-1/src/lib.rs"
      --crate-name tsfp_port_lighting_t2_div_lighting_1 --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-lighting-t2-incremental
      -o "${lighting_t2_rlib}"
    DEPENDS ${lighting_t2_rust_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-port-lighting-t2-div-lighting-1/Cargo.toml"
      "${TSFP_RUSTC}"
    VERBATIM)
  set(data_cutscene_key_node_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_data_cutscene_key_node.rlib")
  add_custom_command(OUTPUT "${data_cutscene_key_node_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-port-data-cutscene-key-node/src/lib.rs"
      --crate-name tsfp_port_data_cutscene_key_node --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-data-cutscene-key-node-incremental
      --extern "tsfp_port_runtime=${runtime_rlib}" -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${data_cutscene_key_node_rlib}"
    DEPENDS ${data_cutscene_key_node_rust_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-port-data-cutscene-key-node/Cargo.toml"
      "${runtime_rlib}" "${TSFP_RUSTC}"
    VERBATIM)
  set(engine_physics_ray_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_engine_physics_ray.rlib")
  add_custom_command(OUTPUT "${engine_physics_ray_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-port-engine-physics-ray/src/lib.rs"
      --crate-name tsfp_port_engine_physics_ray --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-engine-physics-ray-incremental
      --extern "tsfp_port_runtime=${runtime_rlib}" -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${engine_physics_ray_rlib}"
    DEPENDS "${CMAKE_SOURCE_DIR}/crates/tsfp-port-engine-physics-ray/src/lib.rs"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-port-engine-physics-ray/Cargo.toml"
      "${runtime_rlib}" "${TSFP_RUSTC}"
    VERBATIM)
  set(scene_t2_div_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_scene_t2_div_scene_1.rlib")
  add_custom_command(OUTPUT "${scene_t2_div_rlib}"
    COMMAND ${TSFP_RUSTC} "${CMAKE_SOURCE_DIR}/crates/tsfp-port-scene-t2-div-scene-1/src/lib.rs"
      --crate-name tsfp_port_scene_t2_div_scene_1 --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-scene-t2-div-incremental
      -o "${scene_t2_div_rlib}"
    DEPENDS ${scene_t2_div_rust_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-port-scene-t2-div-scene-1/Cargo.toml"
      "${TSFP_RUSTC}"
    VERBATIM)
  if(family_count GREATER 0)
    math(EXPR family_last "${family_count} - 1")
    # I-T3747-8: family crates may depend on each other (split leaf crates re-export a sibling). Collect the package
    # names first; each family then gets --extern for every sibling its Cargo.toml lists as a path dependency.
    set(family_packages "")
    foreach(family_index RANGE 0 ${family_last})
      string(JSON family_package GET "${family_config_json}" families ${family_index} package)
      list(APPEND family_packages "${family_package}")
    endforeach()
    set(family_seen "")
    foreach(family_index RANGE 0 ${family_last})
      string(JSON family_package GET "${family_config_json}" families ${family_index} package)
      if("${family_package}" IN_LIST family_seen)
        continue()  # a package listed twice is built once
      endif()
      list(APPEND family_seen "${family_package}")
      string(REPLACE "-" "_" family_crate "${family_package}")
      file(STRINGS "${CMAKE_SOURCE_DIR}/crates/${family_package}/Cargo.toml" family_path_deps
        REGEX "path = \"\\.\\./[a-z0-9-]+\"")
      set(family_sibling_args "")
      set(family_sibling_deps "")
      foreach(family_dep_line IN LISTS family_path_deps)
        string(REGEX REPLACE ".*path = \"\\.\\./([a-z0-9-]+)\".*" "\\1" family_dep "${family_dep_line}")
        if("${family_dep}" IN_LIST family_packages AND NOT "${family_dep}" STREQUAL "${family_package}")
          string(REPLACE "-" "_" family_dep_crate "${family_dep}")
          list(APPEND family_sibling_args --extern "${family_dep_crate}=${CMAKE_BINARY_DIR}/lib${family_dep_crate}.rlib")
          list(APPEND family_sibling_deps "${CMAKE_BINARY_DIR}/lib${family_dep_crate}.rlib")
        endif()
      endforeach()
      # T3610: families that hold a wiring-layer user (config key "wire") link and wait for tsfp-port-wire.
      string(JSON family_wire ERROR_VARIABLE family_wire_error GET "${family_config_json}" families ${family_index} wire)
      set(family_wire_args "")
      set(family_wire_deps "")
      if(NOT family_wire_error AND family_wire)
        set(family_wire_args --extern "tsfp_port_wire=${wire_rlib}")
        set(family_wire_deps "${wire_rlib}")
      endif()
      set(family_dir "${CMAKE_SOURCE_DIR}/crates/${family_package}")
      file(GLOB_RECURSE family_sources CONFIGURE_DEPENDS "${family_dir}/src/*.rs")
      set(family_rlib "${CMAKE_BINARY_DIR}/lib${family_crate}.rlib")
      add_custom_command(OUTPUT "${family_rlib}"
        COMMAND ${TSFP_RUSTC} "${family_dir}/src/lib.rs"
          --crate-name ${family_crate} --crate-type rlib --edition=2024
          --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
          -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
          -Cincremental=${CMAKE_BINARY_DIR}/rustc-${family_package}-incremental
          --extern "tsfp_port_runtime=${runtime_rlib}" ${family_wire_args} ${family_sibling_args}
          -L "dependency=${CMAKE_BINARY_DIR}"
          -o "${family_rlib}"
        DEPENDS ${family_sources} "${family_dir}/Cargo.toml"
          "${CMAKE_SOURCE_DIR}/crates/tsfp-ports/src/c2r/rt.rs"
          "${CMAKE_SOURCE_DIR}/crates/tsfp-ports/src/c2r/runtime/mmx.rs"
          "${runtime_rlib}" ${family_wire_deps} ${family_sibling_deps} "${TSFP_RUSTC}"
        VERBATIM COMMAND_EXPAND_LISTS)
      list(APPEND family_rlibs "${family_rlib}")
      list(APPEND family_externs --extern "${family_crate}=${family_rlib}")
    endforeach()
  endif()
  # T3765: generated c2r lane crate (tools/lanes.py), depends on the family crates.
  set(lane_c2r_dir "${CMAKE_SOURCE_DIR}/crates/tsfp-lane-c2r")
  set(lane_c2r_rlib "${CMAKE_BINARY_DIR}/libtsfp_lane_c2r.rlib")
  add_custom_command(OUTPUT "${lane_c2r_rlib}"
    COMMAND ${TSFP_RUSTC} "${lane_c2r_dir}/src/lib.rs"
      --crate-name tsfp_lane_c2r --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-tsfp-lane-c2r-incremental
      --extern "tsfp_port_runtime=${runtime_rlib}" ${family_externs} -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${lane_c2r_rlib}"
    DEPENDS "${lane_c2r_dir}/src/lib.rs" "${lane_c2r_dir}/Cargo.toml" "${runtime_rlib}" ${family_rlibs} "${TSFP_RUSTC}"
    VERBATIM COMMAND_EXPAND_LISTS)
  list(APPEND family_rlibs "${lane_c2r_rlib}")
  list(APPEND family_externs --extern "tsfp_lane_c2r=${lane_c2r_rlib}")
  # I-T3738-7: render effects live-return leaf (tools/agents/no_std_archive.py LATE_ARCHIVE_CRATES).
  set(effects_live_return_dir "${CMAKE_SOURCE_DIR}/crates/tsfp-port-engine-render-effects-live-return")
  set(effects_live_return_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_engine_render_effects_live_return.rlib")
  add_custom_command(OUTPUT "${effects_live_return_rlib}"
    COMMAND ${TSFP_RUSTC} "${effects_live_return_dir}/src/lib.rs"
      --crate-name tsfp_port_engine_render_effects_live_return --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-effects-live-return-incremental
      --extern "tsfp_port_runtime=${runtime_rlib}" -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${effects_live_return_rlib}"
    DEPENDS "${effects_live_return_dir}/src/lib.rs" "${effects_live_return_dir}/Cargo.toml"
      "${runtime_rlib}" "${TSFP_RUSTC}"
    VERBATIM)
  list(APPEND family_rlibs "${effects_live_return_rlib}")
  list(APPEND family_externs
    --extern "tsfp_port_engine_render_effects_live_return=${effects_live_return_rlib}")
  # Audio lane nested-call router (T3702): this non-family leaf is a facade path dependency.
  set(audio_dispatch_dir "${CMAKE_SOURCE_DIR}/crates/tsfp-port-audio-dispatch")
  set(audio_dispatch_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_audio_dispatch.rlib")
  add_custom_command(OUTPUT "${audio_dispatch_rlib}"
    COMMAND ${TSFP_RUSTC} "${audio_dispatch_dir}/src/lib.rs"
      --crate-name tsfp_port_audio_dispatch --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${CMAKE_BINARY_DIR}/rustc-audio-dispatch-incremental ${rust_mutation_flags}
      --extern "tsfp_port_runtime=${runtime_rlib}" -L "dependency=${CMAKE_BINARY_DIR}"
      -o "${audio_dispatch_rlib}"
    DEPENDS "${audio_dispatch_dir}/src/lib.rs" "${audio_dispatch_dir}/Cargo.toml"
      "${runtime_rlib}" "${TSFP_RUSTC}"
    VERBATIM COMMAND_EXPAND_LISTS)
  list(APPEND family_rlibs "${audio_dispatch_rlib}")
  list(APPEND family_externs --extern "tsfp_port_audio_dispatch=${audio_dispatch_rlib}")
  # T2 multiplayer mode port closure (T3702): its nested calls use the typed object model.
  set(types_dir "${CMAKE_SOURCE_DIR}/crates/tsfp-types")
  set(types_rlib "${CMAKE_BINARY_DIR}/libtsfp_types.rlib")
  add_custom_command(OUTPUT "${types_rlib}"
    COMMAND ${TSFP_RUSTC} "${types_dir}/src/lib.rs" --crate-name tsfp_types --crate-type rlib
      --edition=2024 --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 -o "${types_rlib}"
    DEPENDS "${types_dir}/src/lib.rs" "${types_dir}/Cargo.toml" "${TSFP_RUSTC}"
    VERBATIM COMMAND_EXPAND_LISTS)
  set(object_dir "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-object")
  set(object_rlib "${CMAKE_BINARY_DIR}/libtsfp_engine_object.rlib")
  add_custom_command(OUTPUT "${object_rlib}"
    COMMAND ${TSFP_RUSTC} "${object_dir}/src/lib.rs" --crate-name tsfp_engine_object --crate-type rlib
      --edition=2024 --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 --extern "tsfp_types=${types_rlib}"
      -L "dependency=${CMAKE_BINARY_DIR}" -o "${object_rlib}"
    DEPENDS "${object_dir}/src/lib.rs" "${object_dir}/Cargo.toml" "${types_rlib}" "${TSFP_RUSTC}"
    VERBATIM COMMAND_EXPAND_LISTS)
  set(multiplayer_dir "${CMAKE_SOURCE_DIR}/crates/tsfp-port-gameplay-modes-multiplayer-1")
  set(multiplayer_rlib "${CMAKE_BINARY_DIR}/libtsfp_port_gameplay_modes_multiplayer_1.rlib")
  add_custom_command(OUTPUT "${multiplayer_rlib}"
    COMMAND ${TSFP_RUSTC} "${multiplayer_dir}/src/lib.rs"
      --crate-name tsfp_port_gameplay_modes_multiplayer_1 --crate-type rlib --edition=2024
      --target x86_64-unknown-linux-gnu -Cpanic=abort -Coverflow-checks=off
      -Crelocation-model=pic -Copt-level=3 --extern "tsfp_engine_object=${object_rlib}"
      -L "dependency=${CMAKE_BINARY_DIR}" -o "${multiplayer_rlib}"
    DEPENDS "${multiplayer_dir}/src/lib.rs" "${multiplayer_dir}/Cargo.toml"
      "${object_rlib}" "${TSFP_RUSTC}"
    VERBATIM COMMAND_EXPAND_LISTS)
  list(APPEND family_rlibs "${multiplayer_rlib}")
  list(APPEND family_externs --extern "tsfp_port_gameplay_modes_multiplayer_1=${multiplayer_rlib}")
  # T1990: lane mutants (EAX or guest-memory bit flip per root, env TSFP_LANE_MUTATE_*), for detection-power checks only.
  add_custom_command(OUTPUT "${rust_archive}"
    COMMAND ${rust_frontend_env} ${TSFP_RUSTC} "${rust_crate}/src/lib.rs" --crate-name tsfp_ports
      --crate-type staticlib --edition=2024 --target x86_64-unknown-linux-gnu
      -Cpanic=abort -Coverflow-checks=off -Crelocation-model=pic -Copt-level=3 -Ccodegen-units=256
      -Cincremental=${rust_incremental_dir} ${rust_mutation_flags} ${rust_frontend_flags} ${rust_oracle_flags}
      --extern "tsfp_port_runtime=${runtime_rlib}" --extern "tsfp_port_wire=${wire_rlib}"
      --extern "tsfp_player_interaction_fast_path=${player_interaction_rlib}"
      --extern "tsfp_ai_spawn_binding=${ai_spawn_binding_rlib}"
      --extern "tsfp_ai_selector_transition=${ai_selector_transition_rlib}"
      --extern "tsfp_render_prep_wire=${render_prep_wire_rlib}"
      --extern "tsfp_engine_object=${object_rlib}"
      --extern "lighting_t2=${lighting_t2_rlib}"
      --extern "tsfp_port_lighting_t2_div_lighting_1=${lighting_t2_rlib}"
      --extern "tsfp_port_data_cutscene_key_node=${data_cutscene_key_node_rlib}"
      --extern "tsfp_port_engine_physics_ray=${engine_physics_ray_rlib}"
      --extern "tsfp_port_scene_t2_div_scene_1=${scene_t2_div_rlib}"
      ${family_externs} ${weapon_externs}
      -L "dependency=${CMAKE_BINARY_DIR}" -o "${rust_archive}"
    DEPENDS ${rust_sources} "${rust_crate}/Cargo.toml"
      "${runtime_rlib}" "${wire_rlib}" "${player_interaction_rlib}" "${ai_spawn_binding_rlib}"
      "${ai_selector_transition_rlib}"
      "${render_prep_wire_rlib}"
      "${lighting_t2_rlib}"
      "${data_cutscene_key_node_rlib}"
      "${engine_physics_ray_rlib}"
      "${scene_t2_div_rlib}"
      ${family_rlibs} ${weapon_rlibs} "${TSFP_RUSTC}"
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    VERBATIM COMMAND_EXPAND_LISTS)
  add_custom_target(tsfp_rust_archive DEPENDS "${rust_archive}")
  set(rust_originals_dir "${CMAKE_BINARY_DIR}/rust-originals")
  set(rust_originals)
  foreach(rust_original_index RANGE 0 3)
    list(APPEND rust_originals "${rust_originals_dir}/rust-originals-${rust_original_index}.c")
  endforeach()
  add_custom_command(OUTPUT ${rust_originals} "${rust_originals_dir}/rust-originals.json"
    COMMAND ${TSFP_PYTHON3} -m tools.play.rust_originals
      --gen-dir "${TSFP_LIFTED_DIR}" --chunk-dir "${rust_originals_dir}" --chunk-stem rust-originals
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    DEPENDS ${TSFP_LIFTED_SOURCES} tools/play/rust_originals.py tools/replace/rust_lane.py
      crates/tsfp-ports/lanes.json tools/replace/invpose_roots.json
    VERBATIM)
  target_sources(tsfp_host PRIVATE ${rust_originals} src/host/rust_lane_call.c)
  # T2136d: the build stamp of the "wgpu-live:" banner (src/host/rust_lane_call.c)
  if(DEFINED ENV{TSFP_SOURCE_COMMIT} AND NOT "$ENV{TSFP_SOURCE_COMMIT}" STREQUAL "")
    # T3760 (rule 47): a remote exec tree has no repository; its commit arrives as data.
    string(SUBSTRING "$ENV{TSFP_SOURCE_COMMIT}" 0 12 TSFP_BUILD_SHA)
  else()
    find_program(TSFP_GIT_PROGRAM NAMES git)
    execute_process(COMMAND "${TSFP_GIT_PROGRAM}" -C "${CMAKE_SOURCE_DIR}" rev-parse --short HEAD OUTPUT_VARIABLE TSFP_BUILD_SHA
                    OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
  endif()
  if(TSFP_BUILD_SHA)
    set_property(SOURCE src/host/rust_lane_call.c APPEND PROPERTY COMPILE_DEFINITIONS "TSFP_GIT_SHA=\"${TSFP_BUILD_SHA}\"")
  endif()
  if(TSFP_RUST_LANE_MUTATION)
    target_sources(tsfp_host PRIVATE src/host/rust_lane_mutation.c)
  endif()
  set_source_files_properties(${rust_originals} PROPERTIES
    COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error;-Wno-unused-variable;-Wno-unused-but-set-variable;-Wno-conversion;-Wno-sign-conversion;-Wno-unused-label")
  # T1874: adapters of the inventory and pose/control lane roots that have no C replacement in
  # src/game. Generated into the build directory (not registered: the registry, census tests and
  # proof snapshot must not see them), plus one weak header per defining lifted chunk.
  set(rust_adapter_dir "${CMAKE_BINARY_DIR}/rust-adapters")
  set(rust_weapon_dir "${CMAKE_BINARY_DIR}/rust-weapons")
  set(ui_draw_dir "${CMAKE_BINARY_DIR}/ui-draw-hooks")
  set(hud_gate_dir "${CMAKE_BINARY_DIR}/hud-gate")
  file(GLOB_RECURSE rust_generator_inputs CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/tools/play/*.py" "${CMAKE_SOURCE_DIR}/tools/replace/*.py"
    "${CMAKE_SOURCE_DIR}/tools/replace/*.json" "${CMAKE_SOURCE_DIR}/tools/data/*.json"
    "${CMAKE_SOURCE_DIR}/src/game/*.c" "${CMAKE_SOURCE_DIR}/src/game/*.h"
    "${TSFP_LIFTED_DIR}/recomp_*.c" "${TSFP_LIFTED_DIR}/recomp_*.h")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
    ${rust_generator_inputs}
    "${CMAKE_SOURCE_DIR}/crates/tsfp-ports/src/weapons_lane.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-ports/lanes.json")
  execute_process(
    COMMAND ${TSFP_PYTHON3} -m tools.play.configure_generators
            --root "${CMAKE_SOURCE_DIR}" --gen-dir "${TSFP_LIFTED_DIR}"
            --game-dir "${CMAKE_SOURCE_DIR}/src/game"
            --cache-dir "${CMAKE_BINARY_DIR}"
            --adapters-dir "${rust_adapter_dir}" --weapons-dir "${rust_weapon_dir}"
            --ui-draw-dir "${ui_draw_dir}" --hud-gate-dir "${hud_gate_dir}"
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    RESULT_VARIABLE rust_generators_result
    OUTPUT_VARIABLE rust_generators_output
    ERROR_VARIABLE rust_generators_error)
  if(NOT rust_generators_result EQUAL 0)
    message(FATAL_ERROR "Rust configure generators failed:\n${rust_generators_error}")
  endif()
  string(STRIP "${rust_generators_output}" rust_generators_output)
  message(STATUS "${rust_generators_output}")
  file(GLOB rust_adapter_headers "${rust_adapter_dir}/weak_recomp_*.h")
  foreach(weak_header IN LISTS rust_adapter_headers)
    get_filename_component(weak_name "${weak_header}" NAME_WE)
    string(REGEX REPLACE "^weak_" "" chunk_name "${weak_name}")
    set(chunk_path "${TSFP_LIFTED_DIR}/${chunk_name}.c")
    get_source_file_property(chunk_options "${chunk_path}" COMPILE_OPTIONS)
    if(chunk_options STREQUAL "NOTFOUND")
      set(chunk_options "")
    endif()
    get_source_file_property(chunk_deps "${chunk_path}" OBJECT_DEPENDS)
    if(chunk_deps STREQUAL "NOTFOUND")
      set(chunk_deps "")
    endif()
    set_source_files_properties("${chunk_path}" PROPERTIES
      COMPILE_OPTIONS "${chunk_options};-include;${weak_header}"
      OBJECT_DEPENDS "${chunk_deps};${weak_header}")
  endforeach()
  target_sources(tsfp_host PRIVATE "${rust_adapter_dir}/rust_invpose_adapters.c")
  set_source_files_properties("${rust_adapter_dir}/rust_invpose_adapters.c" PROPERTIES
    COMPILE_OPTIONS "-I${CMAKE_SOURCE_DIR}/src/game;-DTSFP_RUST_PORTS=1;-w")
  # T1967: opt-in c2r weapon roots (tools/play/rust_weapons.py), adapters, originals and table in the build dir.
  file(GLOB rust_weapon_headers "${rust_weapon_dir}/weakw_recomp_*.h")
  foreach(weak_header IN LISTS rust_weapon_headers)
    get_filename_component(weak_name "${weak_header}" NAME_WE)
    string(REGEX REPLACE "^weakw_" "" chunk_name "${weak_name}")
    set(chunk_path "${TSFP_LIFTED_DIR}/${chunk_name}.c")
    get_source_file_property(chunk_options "${chunk_path}" COMPILE_OPTIONS)
    if(chunk_options STREQUAL "NOTFOUND")
      set(chunk_options "")
    endif()
    get_source_file_property(chunk_deps "${chunk_path}" OBJECT_DEPENDS)
    if(chunk_deps STREQUAL "NOTFOUND")
      set(chunk_deps "")
    endif()
    set_source_files_properties("${chunk_path}" PROPERTIES
      COMPILE_OPTIONS "${chunk_options};-include;${weak_header}"
      OBJECT_DEPENDS "${chunk_deps};${weak_header}")
  endforeach()
  target_sources(tsfp_host PRIVATE "${rust_weapon_dir}/rust_weapon_adapters.c" "${rust_weapon_dir}/rust_weapon_originals.c")
  target_include_directories(tsfp_host PRIVATE "${rust_weapon_dir}")
  set_source_files_properties("${rust_weapon_dir}/rust_weapon_adapters.c" PROPERTIES
    COMPILE_OPTIONS "-I${CMAKE_SOURCE_DIR}/src/game;-DTSFP_RUST_PORTS=1;-w")
  set_source_files_properties("${rust_weapon_dir}/rust_weapon_originals.c" PROPERTIES
    COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error;-Wno-unused-variable;-Wno-unused-but-set-variable;-Wno-conversion;-Wno-sign-conversion;-Wno-unused-label")
  # T2309: per-page switch of the original page draw 0x76810 (src/host/ui_draw_hook.c), retained original and weak header.
  file(GLOB ui_draw_headers "${ui_draw_dir}/weaku_recomp_*.h")
  foreach(weak_header IN LISTS ui_draw_headers)
    get_filename_component(weak_name "${weak_header}" NAME_WE)
    string(REGEX REPLACE "^weaku_" "" chunk_name "${weak_name}")
    set(chunk_path "${TSFP_LIFTED_DIR}/${chunk_name}.c")
    get_source_file_property(chunk_options "${chunk_path}" COMPILE_OPTIONS)
    if(chunk_options STREQUAL "NOTFOUND")
      set(chunk_options "")
    endif()
    get_source_file_property(chunk_deps "${chunk_path}" OBJECT_DEPENDS)
    if(chunk_deps STREQUAL "NOTFOUND")
      set(chunk_deps "")
    endif()
    set_source_files_properties("${chunk_path}" PROPERTIES
      COMPILE_OPTIONS "${chunk_options};-include;${weak_header}"
      OBJECT_DEPENDS "${chunk_deps};${weak_header}")
  endforeach()
  target_sources(tsfp_host PRIVATE src/host/ui_draw_hook.c "${ui_draw_dir}/ui_draw_originals.c")
  set_source_files_properties(src/host/ui_draw_hook.c PROPERTIES COMPILE_OPTIONS "-I${CMAKE_SOURCE_DIR}/src/game")
  set_source_files_properties("${ui_draw_dir}/ui_draw_originals.c" PROPERTIES
    COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error;-Wno-unused-variable;-Wno-unused-but-set-variable;-Wno-conversion;-Wno-sign-conversion;-Wno-unused-label")
  # T2312: per-element legacy HUD gate (tools/play/hud_gate.py); the native Rust HUD decides at run time.
  file(GLOB hud_gate_headers "${hud_gate_dir}/weakh_recomp_*.h")
  foreach(weak_header IN LISTS hud_gate_headers)
    get_filename_component(weak_name "${weak_header}" NAME_WE)
    string(REGEX REPLACE "^weakh_" "" chunk_name "${weak_name}")
    set(chunk_path "${TSFP_LIFTED_DIR}/${chunk_name}.c")
    get_source_file_property(chunk_options "${chunk_path}" COMPILE_OPTIONS)
    if(chunk_options STREQUAL "NOTFOUND")
      set(chunk_options "")
    endif()
    get_source_file_property(chunk_deps "${chunk_path}" OBJECT_DEPENDS)
    if(chunk_deps STREQUAL "NOTFOUND")
      set(chunk_deps "")
    endif()
    set_source_files_properties("${chunk_path}" PROPERTIES
      COMPILE_OPTIONS "${chunk_options};-include;${weak_header}"
      OBJECT_DEPENDS "${chunk_deps};${weak_header}")
  endforeach()
  target_sources(tsfp_host PRIVATE "${hud_gate_dir}/hud_gate_adapters.c" "${hud_gate_dir}/hud_gate_originals.c")
  set_source_files_properties("${hud_gate_dir}/hud_gate_adapters.c" PROPERTIES COMPILE_OPTIONS "-w")
  set_source_files_properties("${hud_gate_dir}/hud_gate_originals.c" PROPERTIES
    COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error;-Wno-unused-variable;-Wno-unused-but-set-variable;-Wno-conversion;-Wno-sign-conversion;-Wno-unused-label")
  add_dependencies(tsfp_host tsfp_rust_archive)
  target_compile_definitions(tsfp_game PRIVATE TSFP_RUST_PORTS=1)
  target_compile_definitions(tsfp_host PRIVATE TSFP_RUST_PORTS=1)
  target_link_libraries(tsfp_host PRIVATE "${rust_archive}")
  # Pull the optional native primitive-expansion entry point out of the Rust archive. C-only
  # targets keep the weak symbol absent and continue to use the reference implementation.
  target_link_options(tsfp_host PRIVATE "-Wl,-u,tsfp_render_native_expand_primitive")
  target_link_options(tsfp_host PRIVATE "-Wl,-u,tsfp_render_native_resolve_primitive")
  if(TSFP_RUST_GAME_ARENA)
    # T2048: the C arena adapter consults the generated Rule-24 retirement policy.
    target_link_options(tsfp_host PRIVATE "-Wl,-u,tsfp_game_arena_lifted_selectable")
  endif()
endif()
if(TSFP_RUST_PORTS)
  target_sources(tsfp_host PRIVATE src/host/shadow_port_leaf.c src/host/shadow_port_stack.c
    src/host/shadow_port_guard.c src/host/shadow_port_denylist.c)
  add_executable(test_shadow_port_policy tests/c/test_shadow_port_policy.c src/host/shadow_port_denylist.c)
  target_include_directories(test_shadow_port_policy PRIVATE src/host)
  target_compile_definitions(test_shadow_port_policy PRIVATE _POSIX_C_SOURCE=200809L)
  target_compile_options(test_shadow_port_policy PRIVATE -UNDEBUG)
  add_test(NAME shadow_port_policy COMMAND test_shadow_port_policy)
  add_executable(test_shadow_port_guard tests/c/test_shadow_port_guard.c
    src/host/shadow_port_guard.c src/host/shadow_port_stack.c src/host/shadow_port_leaf.c src/host/host_runtime.c)
  target_include_directories(test_shadow_port_guard PRIVATE src/host src/xbox)
  target_compile_definitions(test_shadow_port_guard PRIVATE _POSIX_C_SOURCE=200809L)
  target_compile_options(test_shadow_port_guard PRIVATE -UNDEBUG)
  target_link_libraries(test_shadow_port_guard PRIVATE pthread)
  add_test(NAME shadow_port_guard COMMAND test_shadow_port_guard)
  add_executable(test_rust_ports tests/c/test_rust_ports.c src/host/rust_ports.c
    src/game/object_state.c src/game/game_byte_swap_leaves.c src/host/rust_lane_call.c
    tests/c/rust_ports_test_stubs.c)
  target_include_directories(test_rust_ports PRIVATE src/game src/host src/xbox)
  target_compile_definitions(test_rust_ports PRIVATE TSFP_RUST_PORTS=1 _POSIX_C_SOURCE=200809L)
  add_dependencies(test_rust_ports tsfp_rust_archive)
  target_link_libraries(test_rust_ports PRIVATE "${rust_archive}")
  add_test(NAME rust_ports COMMAND test_rust_ports)
endif()
if(TSFP_RUST_PORTS)
  add_executable(test_rust_replay_identity tests/c/test_rust_replay_identity.c)
  target_link_libraries(test_rust_replay_identity PRIVATE tsfp_xbox)
  add_test(NAME rust_replay_identity COMMAND test_rust_replay_identity)
endif()

if(TSFP_RUST_PORTS)
  # T1858 stub-callee native A/B: only the 70 harness roots are retained here (the 129 inventory and
  # pose/control roots have their own A/B on the T1850 fixtures, tools/t1850_invpose_diff.py --lane).
  set(rust_harness_originals "${CMAKE_BINARY_DIR}/rust-harness-originals.c")
  add_custom_command(OUTPUT "${rust_harness_originals}"
    COMMAND ${TSFP_PYTHON3} -m tools.play.rust_originals
      --gen-dir "${TSFP_LIFTED_DIR}" --out "${rust_harness_originals}" --harness-roots
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    DEPENDS ${TSFP_LIFTED_SOURCES} tools/play/rust_originals.py tools/replace/rust_lane.py
      crates/tsfp-ports/lanes.json tools/replace/invpose_roots.json
    VERBATIM)
  set_source_files_properties("${rust_harness_originals}" PROPERTIES
    COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error;-Wno-unused-variable;-Wno-unused-but-set-variable;-Wno-conversion;-Wno-sign-conversion;-Wno-unused-label")
  add_executable(test_rust_ports_originals tests/c/test_rust_ports.c src/host/rust_ports.c
    src/game/object_state.c src/game/game_byte_swap_leaves.c src/host/rust_lane_call.c
    tests/c/rust_ports_test_stubs.c "${rust_harness_originals}")
  target_include_directories(test_rust_ports_originals PRIVATE src/game src/host src/xbox "${TSFP_LIFTED_DIR}")
  target_compile_definitions(test_rust_ports_originals PRIVATE TSFP_RUST_PORTS=1 TSFP_TEST_REAL_ORIGINALS=1 _POSIX_C_SOURCE=200809L)
  add_dependencies(test_rust_ports_originals tsfp_rust_archive)
  target_link_libraries(test_rust_ports_originals PRIVATE "${rust_archive}")
  add_test(NAME rust_ports_originals COMMAND test_rust_ports_originals)
  add_executable(test_shadow_real_leaves tests/c/test_rust_ports.c src/host/rust_ports.c
    src/game/object_state.c src/game/game_byte_swap_leaves.c src/host/rust_lane_call.c
    tests/c/rust_ports_test_stubs.c "${rust_harness_originals}"
    src/host/shadow_port_leaf.c src/host/shadow_port_guard.c src/host/shadow_port_stack.c
    src/host/shadow_port_denylist.c src/host/host_runtime.c)
  target_include_directories(test_shadow_real_leaves PRIVATE src/game src/host src/xbox src/input src/gpu "${TSFP_LIFTED_DIR}")
  target_compile_definitions(test_shadow_real_leaves PRIVATE TSFP_RUST_PORTS=1 TSFP_TEST_REAL_ORIGINALS=1
    TSFP_TEST_SHADOW_LEAF=1 _POSIX_C_SOURCE=200809L)
  add_dependencies(test_shadow_real_leaves tsfp_rust_archive)
  target_link_libraries(test_shadow_real_leaves PRIVATE "${rust_archive}" m pthread)
  add_test(NAME shadow_real_leaves COMMAND test_shadow_real_leaves)
  add_test(NAME rust_ports_shadow_seeded_mismatch COMMAND test_rust_ports_originals)
  set_tests_properties(rust_ports_shadow_seeded_mismatch PROPERTIES
    ENVIRONMENT "TSFP_RUST_SHADOW_TEST_MODE=seeded")
endif()

if(TSFP_RUST_PORTS)
  # T1815: extended register bridge (ExtRegisters + GuestCall trampoline) and its
  # in-process Rust-vs-lifted-original differential over a stubbed callee closure.
  foreach(ext_root bb490 ba230 bd0f0 baba0)
    set(rust_ext_originals "${CMAKE_BINARY_DIR}/rust-ext-originals-${ext_root}.c")
    add_custom_command(OUTPUT "${rust_ext_originals}"
      COMMAND ${TSFP_PYTHON3} -m tools.play.rust_originals
        --gen-dir "${TSFP_LIFTED_DIR}" --out "${rust_ext_originals}" --roots ${ext_root}
      WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
      DEPENDS ${TSFP_LIFTED_SOURCES} tools/play/rust_originals.py tools/replace/rust_lane.py
      crates/tsfp-ports/lanes.json tools/replace/invpose_roots.json
      VERBATIM)
    set_source_files_properties("${rust_ext_originals}" PROPERTIES
      COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error")
    add_executable(test_rust_ext_${ext_root} tests/c/test_rust_ext.c src/host/rust_ext_bridge.c src/host/rust_lane_call.c "${rust_ext_originals}")
    target_include_directories(test_rust_ext_${ext_root} PRIVATE src/game src/host "${TSFP_LIFTED_DIR}")
    target_compile_definitions(test_rust_ext_${ext_root} PRIVATE TSFP_RUST_PORTS=1 _POSIX_C_SOURCE=200809L)
    string(TOUPPER "${ext_root}" ext_upper)
    if(NOT ext_root STREQUAL "bb490")
      target_compile_definitions(test_rust_ext_${ext_root} PRIVATE ROOT_${ext_upper}=1)
    endif()
    add_dependencies(test_rust_ext_${ext_root} tsfp_rust_archive)
    target_link_libraries(test_rust_ext_${ext_root} PRIVATE "${rust_archive}")
    add_test(NAME rust_ext_${ext_root} COMMAND test_rust_ext_${ext_root} 20000)
  endforeach()
  target_sources(tsfp_host PRIVATE src/host/rust_ext_bridge.c)
  # T2263: presenter-rate experiment; T1670: default-off fixed-tick/timer replacement. The outer 0x3C440, post-vblank
  # 0x1538C0 boundary, and timer calls go through src/host/render_rate.c; their callers and definitions are separate TUs.
  target_sources(tsfp_host PRIVATE src/host/render_rate.c src/host/frame_state_sync.c)
  target_link_options(tsfp_host PRIVATE "-Wl,--wrap=sub_0003C440" "-Wl,--wrap=sub_0003D370" "-Wl,--wrap=sub_001538C0" "-Wl,--wrap=sub_00165490"
    "-Wl,--wrap=sub_001654A0" "-Wl,--wrap=sub_00165540")
  # T2273: 0xA1B80 (bone palette build) is logged so replays rebuild palettes (render_rate.c); its caller 0x8F6B0 is in another TU.
  target_link_options(tsfp_host PRIVATE "-Wl,--wrap=sub_000A1B80")
  # T1815-w4: retained lifted originals of the four ext roots for the src/game/rust_ext_roots.c
  # GAME_REPLACE adapters (fallback body when a root is not selected).
  set(rust_ext_all "${CMAKE_BINARY_DIR}/rust-ext-originals-all.c")
  add_custom_command(OUTPUT "${rust_ext_all}"
    COMMAND ${TSFP_PYTHON3} -m tools.play.rust_originals
      --gen-dir "${TSFP_LIFTED_DIR}" --out "${rust_ext_all}" --roots bb490,ba230,bd0f0,baba0
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    DEPENDS ${TSFP_LIFTED_SOURCES} tools/play/rust_originals.py tools/replace/rust_lane.py
      crates/tsfp-ports/lanes.json tools/replace/invpose_roots.json
    VERBATIM)
  set_source_files_properties("${rust_ext_all}" PROPERTIES
    COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error")
  target_sources(tsfp_host PRIVATE "${rust_ext_all}")
endif()

if(TSFP_RUST_PORTS AND EXISTS "${CMAKE_SOURCE_DIR}/build/default.xbe")
  # T1835 w12: entity tick 0x2784D0 whole-root differential (Rust region 0 + resumable lifted tail
  # versus the unchanged lifted original). Test only: nothing registers the port.
  set(rust_tick_originals "${CMAKE_BINARY_DIR}/rust-tick-originals.c")
  add_custom_command(OUTPUT "${rust_tick_originals}" "${CMAKE_BINARY_DIR}/rust-tick-originals.tables.inc"
    COMMAND ${TSFP_PYTHON3} -m tools.play.rust_tick_resume
      --gen-dir "${TSFP_LIFTED_DIR}" --out "${rust_tick_originals}"
      --xbe "${CMAKE_SOURCE_DIR}/build/default.xbe"
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    DEPENDS ${TSFP_LIFTED_SOURCES} tools/play/rust_tick_resume.py
    VERBATIM)
  set_source_files_properties("${rust_tick_originals}" PROPERTIES
    COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error")
  add_executable(test_rust_tick tests/c/test_rust_tick.c src/host/rust_ext_bridge.c src/host/rust_lane_call.c "${rust_tick_originals}")
  target_include_directories(test_rust_tick PRIVATE src/game src/host "${TSFP_LIFTED_DIR}" "${CMAKE_BINARY_DIR}")
  set_source_files_properties(tests/c/test_rust_tick.c PROPERTIES OBJECT_DEPENDS "${CMAKE_BINARY_DIR}/rust-tick-originals.tables.inc")
  target_compile_definitions(test_rust_tick PRIVATE TSFP_RUST_PORTS=1)
  add_dependencies(test_rust_tick tsfp_rust_archive)
  target_link_libraries(test_rust_tick PRIVATE "${rust_archive}")
  add_test(NAME rust_tick COMMAND test_rust_tick 20000)
endif()

if(TSFP_RUST_PORTS)
  # T1967: libtsfp_ports.a now references the c2r weapon modules (weapons_lane.rs), which call sin/cos/atan2.
  foreach(rust_test test_rust_ports test_rust_ports_originals test_rust_ports_off test_rust_ext_bb490
          test_rust_ext_ba230 test_rust_ext_bd0f0 test_rust_ext_baba0 test_rust_tick)
    if(TARGET ${rust_test})
      target_link_libraries(${rust_test} PRIVATE m)
    endif()
  endforeach()
endif()
