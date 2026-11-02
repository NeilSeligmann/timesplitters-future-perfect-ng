# SPDX-License-Identifier: GPL-3.0-or-later
#
# Hand-written replacements for lifted functions. Included, not add_subdirectory()'d, from
# the top-level CMakeLists.txt AFTER tsfp_lifted and tsfp_host exist:
#
#     include(src/game/game.cmake)
#
# It runs in the including directory's scope on purpose: setting a COMPILE_OPTIONS property
# on a source file that another directory owns would need TARGET_DIRECTORY, and the lifted
# chunks are owned here.
#
# WHAT IT DOES, in the order it happens
#   1. Asks tools/replace which lifted chunk defines each address src/game registers, and
#      has it write one header per chunk containing `#pragma weak sub_XXXXXXXX` lines.
#   2. Force-includes that header into that chunk only, which makes the lifted definition
#      weak. Only the chunks that contain a replaced function recompile.
#   3. Compiles src/game with the project's own warning set into an OBJECT library and links
#      its objects into tsfp_host. They define the same sub_XXXXXXXX symbols strongly, so the
#      linker resolves every caller, in any chunk and through the dispatch table, to the
#      hand-written adapter. No re-lift and no runtime lookup is involved.
#   4. Builds `game_manifest`, which prints the registry linked into it. That registry is
#      what tools/coverage.py counts, never the file list.
#
# A fresh clone has no lifted tree, so none of this applies there (the caller guards on it).

set(TSFP_GAME_DIR "${CMAKE_SOURCE_DIR}/src/game")
file(GLOB TSFP_GAME_SOURCES CONFIGURE_DEPENDS "${TSFP_GAME_DIR}/*.c")
list(FILTER TSFP_GAME_SOURCES EXCLUDE REGEX "/game_manifest\\.c$")

# Editing a registration line must re-run the wiring, and CONFIGURE_DEPENDS on the glob only
# notices files appearing and disappearing.
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS ${TSFP_GAME_SOURCES})
# Re-lifting can move a replacement into another chunk without changing the file list.
# Recompute the weak-symbol mapping when the generated definitions change.
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS ${TSFP_LIFTED_SOURCES})

find_program(TSFP_PYTHON3 NAMES python3 python)
if(NOT TSFP_PYTHON3)
  message(FATAL_ERROR
    "src/game needs python3 at configure time to find which lifted chunk defines each "
    "replaced function (tools/replace wire, standard library only)")
endif()

set(TSFP_GAME_WEAK_DIR "${CMAKE_BINARY_DIR}/game-weak")
execute_process(
  COMMAND ${TSFP_PYTHON3} -m tools.replace wire
          --game-dir "${TSFP_GAME_DIR}" --gen-dir "${TSFP_LIFTED_DIR}"
          --out "${TSFP_GAME_WEAK_DIR}"
  WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
  RESULT_VARIABLE TSFP_GAME_WIRE_RESULT
  OUTPUT_VARIABLE TSFP_GAME_WIRE_OUTPUT
  ERROR_VARIABLE TSFP_GAME_WIRE_ERROR)
if(NOT TSFP_GAME_WIRE_RESULT EQUAL 0)
  # Fail, do not skip: linking the hand-written objects over unweakened lifted chunks is a
  # duplicate-symbol error, and a half-wired build is worse than a stopped one.
  message(FATAL_ERROR "tools/replace wire failed:\n${TSFP_GAME_WIRE_ERROR}")
endif()
string(STRIP "${TSFP_GAME_WIRE_OUTPUT}" TSFP_GAME_WIRE_OUTPUT)
message(STATUS "game replacements: ${TSFP_GAME_WIRE_OUTPUT}")

file(GLOB TSFP_GAME_WEAK_HEADERS "${TSFP_GAME_WEAK_DIR}/weak_recomp_*.h")
foreach(weak_header IN LISTS TSFP_GAME_WEAK_HEADERS)
  get_filename_component(weak_name "${weak_header}" NAME_WE)
  string(REGEX REPLACE "^weak_" "" chunk_name "${weak_name}")
  set(chunk_path "${TSFP_LIFTED_DIR}/${chunk_name}.c")
  # T2143: keep options a chunk already has (the hot chunk optimisation level of TSFP_LIFTED_HOT_CHUNKS).
  get_source_file_property(chunk_existing "${chunk_path}" COMPILE_OPTIONS)
  if(chunk_existing STREQUAL "NOTFOUND")
    set(chunk_existing "")
  endif()
  set_source_files_properties("${chunk_path}" PROPERTIES
    COMPILE_OPTIONS "${chunk_existing};-include;${weak_header}"
    OBJECT_DEPENDS "${weak_header}")
endforeach()

# Diagnostic source exclusions keep the weak headers for all registrations so
# omitted adapters resolve to the lifted bodies.
set(TSFP_GAME_EXCLUDE_SOURCES "" CACHE STRING
    "Semicolon-separated src/game basenames to omit for regression bisection")
foreach(excluded IN LISTS TSFP_GAME_EXCLUDE_SOURCES)
  if(NOT excluded MATCHES "^[A-Za-z0-9_]+\\.c$")
    message(FATAL_ERROR "Invalid game exclusion basename: ${excluded}")
  endif()
  set(excluded_path "${TSFP_GAME_DIR}/${excluded}")
  if(NOT excluded_path IN_LIST TSFP_GAME_SOURCES)
    message(FATAL_ERROR "Game exclusion source not found: ${excluded}")
  endif()
  list(REMOVE_ITEM TSFP_GAME_SOURCES "${excluded_path}")
endforeach()

# T1815-w4: the Rust ext root adapters need the retained originals only a Rust host links.
if(NOT TSFP_RUST_PORTS)
  list(REMOVE_ITEM TSFP_GAME_SOURCES "${TSFP_GAME_DIR}/rust_ext_roots.c")
endif()

option(TSFP_CODEWRITE_TX "Enable codewrite-tx adapters with retained lifted fallback" ON)
set(TSFP_CODEWRITE_TX_ROOTS "003C97AB;0040F720;0038AEB0;0038C010;003B9BC0" CACHE STRING
    "Fast-path roots: 003C97AB;0040F720;0038AEB0;0038C010;003B9BC0 (empty delegates all)")
set(codewrite_root_names 003C97AB 0040F720 0038AEB0 0038C010 003B9BC0)
set(codewrite_mask 0)
foreach(root IN LISTS TSFP_CODEWRITE_TX_ROOTS)
  list(FIND codewrite_root_names "${root}" bit)
  if(bit LESS 0)
    message(FATAL_ERROR "Invalid codewrite-tx root: ${root}")
  endif()
  math(EXPR codewrite_mask "${codewrite_mask} | (1 << ${bit})")
endforeach()
if(NOT TSFP_CODEWRITE_TX)
  list(REMOVE_ITEM TSFP_GAME_SOURCES "${TSFP_GAME_DIR}/codewrite_tx_roots.c")
  message(STATUS "codewrite-tx disabled explicitly")
endif()

add_library(tsfp_game OBJECT ${TSFP_GAME_SOURCES})
target_include_directories(tsfp_game PRIVATE "${TSFP_GAME_DIR}")
set_target_properties(tsfp_game PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_sources(tsfp_host PRIVATE $<TARGET_OBJECTS:tsfp_game>)

add_executable(game_manifest src/game/game_manifest.c $<TARGET_OBJECTS:tsfp_game>)
target_include_directories(game_manifest PRIVATE "${TSFP_GAME_DIR}")

add_executable(test_game_replacements tests/c/test_game_replacements.c
  $<TARGET_OBJECTS:tsfp_game>)
target_include_directories(test_game_replacements PRIVATE "${TSFP_GAME_DIR}")
set_target_properties(test_game_replacements PROPERTIES POSITION_INDEPENDENT_CODE ON)
# T2101: generated hex fixtures exceed the C99 4095-char literal minimum, clang/gcc accept them.
target_compile_options(test_game_replacements PRIVATE -Wno-overlength-strings)
add_test(NAME game_replacements COMMAND test_game_replacements)

# Retained lifted originals have distinct names; fallback bypasses the strong
# adapter and starts from the entry state after transaction rollback.
set(codewrite_originals "${CMAKE_BINARY_DIR}/codewrite-originals.c")
add_custom_command(OUTPUT "${codewrite_originals}"
  COMMAND ${TSFP_PYTHON3} -m tools.play.codewrite_originals
    --gen-dir "${TSFP_LIFTED_DIR}" --out "${codewrite_originals}"
  WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
  DEPENDS ${TSFP_LIFTED_SOURCES} tools/play/codewrite_originals.py tools/play/rust_originals.py
  VERBATIM)
target_sources(tsfp_host PRIVATE "${codewrite_originals}" src/host/codewrite_originals.c)
set_source_files_properties("${codewrite_originals}" PROPERTIES
  COMPILE_OPTIONS "${TSFP_LIFTED_OPT};-fno-strict-aliasing;-w;-Wno-error")
target_compile_definitions(tsfp_game PRIVATE TSFP_CODEWRITE_HOST=1 TSFP_CODEWRITE_TX_MASK=${codewrite_mask})
