# SPDX-License-Identifier: GPL-3.0-or-later
include("${CMAKE_CURRENT_LIST_DIR}/rust-host-rpath.cmake")
# T2136: -DTSFP_RUST_RT_HOST=ON links crates/tsfp-rt-host (the in-process render runtime: a render thread owning the wgpu
# LiveRenderer behind a bounded line queue) into tsfp_host as libtsfp_rt_host.so. It replaces the popen'd subprocess of
# TSFP_WGPU_LIVE=1 (src/host/rust_lane_call.c); with the option OFF (default) or TSFP_WGPU_LIVE unset nothing changes.
# A shared object, not a staticlib: libtsfp_ports.a is no_std and carries its own core/compiler_builtins and a libc allocator,
# the std runtime of wgpu would collide with it in one static link (see docs/t-rust-hosted-dispatcher.md).
# TSFP_RT_HOST_TARGET_DIR overrides the cargo target dir (default: inside the build dir).
if(TSFP_RUST_RT_HOST)
  find_program(TSFP_CARGO NAMES cargo REQUIRED)
  set(rt_host_crate "${CMAKE_SOURCE_DIR}/crates/tsfp-rt-host")
  if(NOT TSFP_RT_HOST_TARGET_DIR)
    # Keep Cargo artifacts outside the CMake build tree when the host build
    # already supplies its shared target. A relift selects a new generated-C
    # directory and can force CMake to reconfigure; that must not discard this
    # unrelated Rust runtime artifact.
    if(DEFINED ENV{CARGO_TARGET_DIR} AND NOT "$ENV{CARGO_TARGET_DIR}" STREQUAL "")
      # A sibling, never the same directory: the host build script runs ninja
      # while the outer cargo holds the CARGO_TARGET_DIR lock, so sharing it
      # deadlocks a nested runtime rebuild (owner build hang 2026-10-16).
      set(TSFP_RT_HOST_TARGET_DIR "$ENV{CARGO_TARGET_DIR}-rt-host")
    else()
      set(TSFP_RT_HOST_TARGET_DIR "${CMAKE_BINARY_DIR}/rust-rt-host")
    endif()
  endif()
  # A relative CARGO_TARGET_DIR made this a bare name, which the linker read as -l<path>.
  get_filename_component(TSFP_RT_HOST_TARGET_DIR "${TSFP_RT_HOST_TARGET_DIR}" ABSOLUTE BASE_DIR "${CMAKE_SOURCE_DIR}")
  set(rt_host_lib "${TSFP_RT_HOST_TARGET_DIR}/release/libtsfp_rt_host.so")
  file(GLOB_RECURSE rt_host_sources CONFIGURE_DEPENDS
       "${rt_host_crate}/src/*.rs" "${CMAKE_SOURCE_DIR}/crates/tsfp-wgpu/src/*.rs"
       "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-render/src/*.rs"
       "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-anim/src/*.rs"
       "${CMAKE_SOURCE_DIR}/crates/tsfp-format-anim/src/*.rs")
  add_custom_command(OUTPUT "${rt_host_lib}"
    COMMAND ${TSFP_CARGO} build --release --manifest-path "${rt_host_crate}/Cargo.toml"
            --target-dir "${TSFP_RT_HOST_TARGET_DIR}"
    DEPENDS ${rt_host_sources} "${rt_host_crate}/Cargo.toml"
    VERBATIM)
  add_custom_target(tsfp_rust_rt_host_lib DEPENDS "${rt_host_lib}")
  add_dependencies(tsfp_host tsfp_rust_rt_host_lib)
  set_property(SOURCE src/host/rust_lane_call.c APPEND PROPERTY COMPILE_DEFINITIONS TSFP_RUST_RT_HOST=1)
  # T2241: scope copied/multiplied per-model animation matrices without
  # changing the original draw body. The wrapper is inactive unless opted in.
  if(TSFP_RUST_PORTS)
    target_link_options(tsfp_host PRIVATE "-Wl,--wrap=sub_0018EA90" "-Wl,--undefined=sub_0018EA90"
      "-Wl,--wrap=sub_001530E0" "-Wl,--undefined=sub_001530E0")
  endif()
  target_link_libraries(tsfp_host PRIVATE "${rt_host_lib}")
  tsfp_rust_host_rpath(tsfp_host "${TSFP_RT_HOST_TARGET_DIR}/release")
endif()
