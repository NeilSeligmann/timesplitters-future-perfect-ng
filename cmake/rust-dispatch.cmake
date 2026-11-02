# SPDX-License-Identifier: GPL-3.0-or-later
# T3869 (lane B phase 1, docs/t-rust-dispatcher-plan.md): -DTSFP_RUST_DISPATCH=ON links crates/tsfp-dispatch-capi (the Rust
# dispatch table and guest-call trampoline) into tsfp_host as libtsfp_dispatch_capi.so, and with it tsfp_host_rustlink and
# tsfp_host_bevylink (cmake/rust-main.cmake copies tsfp_host's sources, definitions and libraries; include this first).
# Selection is a second, run-time opt-in: TSFP_RUST_DISPATCH=1 (src/host/rust_dispatch.c). Default OFF at both levels; the C
# dispatcher stays the default and the oracle. Included after the tsfp_host target exists.
if(TSFP_RUST_DISPATCH)
  find_program(TSFP_DISPATCH_CARGO cargo REQUIRED)
  set(dispatch_target "${CMAKE_BINARY_DIR}/rust-dispatch")
  set(dispatch_lib "${dispatch_target}/release/libtsfp_dispatch_capi.so")
  file(GLOB_RECURSE dispatch_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-dispatch/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-dispatch-capi/src/*.rs")
  add_custom_command(OUTPUT "${dispatch_lib}"
    COMMAND "${CMAKE_COMMAND}" -E env CARGO_BUILD_JOBS=4
      "${TSFP_DISPATCH_CARGO}" build --release --locked
      --manifest-path "${CMAKE_SOURCE_DIR}/crates/tsfp-dispatch-capi/Cargo.toml"
      --target-dir "${dispatch_target}"
    DEPENDS ${dispatch_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-dispatch/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-dispatch-capi/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-dispatch-capi/Cargo.lock"
    VERBATIM)
  add_custom_target(tsfp_dispatch_build DEPENDS "${dispatch_lib}")
  # A shared object, not a staticlib: libtsfp_ports.a already carries its own copy of core/std symbols.
  add_library(tsfp_dispatch SHARED IMPORTED)
  set_target_properties(tsfp_dispatch PROPERTIES IMPORTED_LOCATION "${dispatch_lib}"
    IMPORTED_NO_SONAME ON INTERFACE_LINK_OPTIONS "-Wl,-rpath,${dispatch_target}/release")
  add_dependencies(tsfp_dispatch tsfp_dispatch_build)
  target_sources(tsfp_host PRIVATE src/host/rust_dispatch.c)
  target_compile_definitions(tsfp_host PRIVATE TSFP_RUST_DISPATCH=1)
  target_link_libraries(tsfp_host PRIVATE tsfp_dispatch)
  # The C <-> Rust ABI with real C callees on a stand-in table (no lifted code needed).
  add_executable(test_rust_dispatch_abi tests/c/test_rust_dispatch_abi.c src/host/rust_dispatch.c)
  target_include_directories(test_rust_dispatch_abi PRIVATE src/host src/xbox)
  target_compile_definitions(test_rust_dispatch_abi PRIVATE TSFP_RUST_DISPATCH=1 _GNU_SOURCE)
  target_link_libraries(test_rust_dispatch_abi PRIVATE tsfp_dispatch m)
  add_test(NAME rust_dispatch_abi COMMAND test_rust_dispatch_abi)
endif()
