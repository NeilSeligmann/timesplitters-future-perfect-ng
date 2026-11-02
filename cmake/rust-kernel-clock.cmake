# SPDX-License-Identifier: GPL-3.0-or-later
# T2207: both build and runtime opt-in; never enables timer/DPC dispatch.
if(TSFP_RUST_KCLOCK)
  find_program(TSFP_KCLOCK_CARGO cargo REQUIRED)
  set(kclock_target "${CMAKE_BINARY_DIR}/rust-kclock")
  set(kclock_archive "${kclock_target}/release/libtsfp_kernel_clock_capi.so")
  file(GLOB_RECURSE kclock_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-clock/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-frame-phase/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-vblank-quiescence/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-timers-host/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-clock-capi/src/*.rs")
  add_custom_command(OUTPUT "${kclock_archive}"
    COMMAND "${CMAKE_COMMAND}" -E env CARGO_BUILD_JOBS=4
      "${TSFP_KCLOCK_CARGO}" build --release --locked
      --manifest-path "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-clock-capi/Cargo.toml"
      --target-dir "${kclock_target}"
    DEPENDS ${kclock_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-clock/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-frame-phase/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-vblank-quiescence/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-timers-host/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-clock-capi/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-clock-capi/Cargo.lock"
    VERBATIM)
  add_custom_target(tsfp_kclock_build DEPENDS "${kclock_archive}")
  # A shared object, not a staticlib: libtsfp_ports.a already carries its own copy of core/std symbols.
  add_library(tsfp_kclock SHARED IMPORTED)
  set_target_properties(tsfp_kclock PROPERTIES IMPORTED_LOCATION "${kclock_archive}"
    IMPORTED_NO_SONAME ON INTERFACE_LINK_OPTIONS "-Wl,-rpath,${kclock_target}/release")
  add_dependencies(tsfp_kclock tsfp_kclock_build)
  target_compile_definitions(tsfp_xbox PRIVATE TSFP_RUST_KCLOCK=1)
  target_link_libraries(tsfp_xbox PUBLIC tsfp_kclock ${CMAKE_DL_LIBS} Threads::Threads m)
endif()
