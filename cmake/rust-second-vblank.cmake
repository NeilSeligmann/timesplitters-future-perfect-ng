# SPDX-License-Identifier: GPL-3.0-or-later
# The C implementation remains the fallback until the Rust policy is enabled.
if(TSFP_SECOND_VBLANK_RUST_POLICY)
  set(second_vblank_crate "${CMAKE_SOURCE_DIR}/crates/tsfp-second-vblank")
  if(NOT EXISTS "${second_vblank_crate}/Cargo.toml")
    message(FATAL_ERROR
      "TSFP_SECOND_VBLANK_RUST_POLICY requires crates/tsfp-second-vblank from the caller migration")
  endif()
  find_program(TSFP_SECOND_VBLANK_CARGO NAMES cargo REQUIRED)
  set(second_vblank_target "${CMAKE_BINARY_DIR}/rust-second-vblank")
  set(second_vblank_archive "${second_vblank_target}/release/libtsfp_second_vblank.a")
  file(GLOB_RECURSE second_vblank_sources CONFIGURE_DEPENDS
    "${second_vblank_crate}/src/*.rs")
  add_custom_command(OUTPUT "${second_vblank_archive}"
    COMMAND "${CMAKE_COMMAND}" -E env CARGO_BUILD_JOBS=4
      "${TSFP_SECOND_VBLANK_CARGO}" build --release
      --manifest-path "${second_vblank_crate}/Cargo.toml"
      --target-dir "${second_vblank_target}"
    DEPENDS ${second_vblank_sources} "${second_vblank_crate}/Cargo.toml"
    VERBATIM)
  add_custom_target(tsfp_second_vblank_build DEPENDS "${second_vblank_archive}")
  add_dependencies(tsfp_host_options tsfp_second_vblank_build)
  target_compile_definitions(tsfp_host_options PUBLIC TSFP_SECOND_VBLANK_RUST_POLICY=1)
  target_link_libraries(tsfp_host_options PUBLIC
    "${second_vblank_archive}" ${CMAKE_DL_LIBS} Threads::Threads m)
endif()
