# SPDX-License-Identifier: GPL-3.0-or-later
include("${CMAKE_CURRENT_LIST_DIR}/rust-host-rpath.cmake")
# T3935: crates/tsfp-kmem-capi is the only guest-memory implementation (cmake/kmem-lib.cmake defines and links it for
# tsfp_xbox); tsfp_host also keeps the origin-relative rpaths of the other Rust host libraries and the game allocator slices
# (T2048) link the same small C ABI library.
add_dependencies(tsfp_host tsfp_rust_kmem_lib)
target_link_libraries(tsfp_host PRIVATE "${kmem_lib}")
tsfp_rust_host_rpath(tsfp_host "${TSFP_KMEM_TARGET_DIR}/release")


if(TSFP_RUST_GAME_BUMP)
  set_source_files_properties(src/game/bump_allocator.c PROPERTIES COMPILE_DEFINITIONS TSFP_RUST_GAME_BUMP=1)
endif()

if(TSFP_RUST_GAME_ARENA)
  set_source_files_properties(src/game/game_guarded_batch_a.c PROPERTIES COMPILE_DEFINITIONS TSFP_RUST_GAME_ARENA=1)
endif()

if(TSFP_RUST_GAME_POOL)
  # game_pool_rust_bridge.c is already compiled by tsfp_game from src/game/game.cmake,
  # and tsfp_game's objects are included in tsfp_host and its Rust/Bevy host variants.
  # Adding the source directly here duplicates its exported replacement symbols in those
  # cloned host targets. Keep only the per-source opt-in definitions below.
  set_source_files_properties(src/game/game_synth_T1781.c src/game/game_pool_rust_bridge.c
                              src/game/game_engine_campaign.c
                              PROPERTIES COMPILE_DEFINITIONS TSFP_RUST_GAME_POOL=1)
  set_property(SOURCE src/game/game_guarded_batch_a.c APPEND PROPERTY COMPILE_DEFINITIONS
               TSFP_RUST_GAME_POOL=1)
endif()

if(TSFP_RUST_GAME_LEDGER)
  set_source_files_properties(src/game/allocation_ledger.c src/game/allocation_release.c
                              src/game/secondary_ledger.c
                              src/game/game_ledger_rust_bridge.c
                              PROPERTIES COMPILE_DEFINITIONS TSFP_RUST_GAME_LEDGER=1)
endif()
