# SPDX-License-Identifier: GPL-3.0-or-later
# Independent default-OFF K-3 bridge; no KMEM/RTL wrappers or owner data required.
if(TSFP_RUST_KSERVICES)
  find_program(TSFP_SERVICES_CARGO NAMES cargo REQUIRED)
  set(services_crate "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-services-capi")
  set(services_target "${CMAKE_BINARY_DIR}/rust-kernel-services")
  set(services_lib "${services_target}/release/libtsfp_kernel_services_capi.so")
  file(GLOB_RECURSE services_sources CONFIGURE_DEPENDS
    "${services_crate}/src/*.rs" "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-services/src/*.rs")
  add_custom_command(OUTPUT "${services_lib}"
    COMMAND ${CMAKE_COMMAND} -E env CARGO_BUILD_JOBS=4 ${TSFP_SERVICES_CARGO}
      build --release --offline --manifest-path "${services_crate}/Cargo.toml" --target-dir "${services_target}"
    DEPENDS ${services_sources} "${services_crate}/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-services/Cargo.toml"
    VERBATIM)
  add_custom_target(tsfp_rust_services_lib DEPENDS "${services_lib}")
  add_dependencies(tsfp_xbox tsfp_rust_services_lib)
  target_sources(tsfp_xbox PRIVATE src/xbox/kernel_services_rust.c)
  target_compile_definitions(tsfp_xbox PUBLIC TSFP_RUST_KSERVICES=1)
  target_link_libraries(tsfp_xbox PUBLIC "${services_lib}")
  add_executable(test_kernel_services_rust tests/c/test_kernel_services_rust.c src/host/kernel_thunk.c)
  target_link_libraries(test_kernel_services_rust PRIVATE tsfp_xbox tsfp_thunk)
  target_include_directories(test_kernel_services_rust PRIVATE src/host)
  target_compile_definitions(test_kernel_services_rust PRIVATE _DEFAULT_SOURCE)
  foreach(mode IN ITEMS off rust compare invalid subset)
    add_test(NAME kernel_services_${mode} COMMAND test_kernel_services_rust ${mode})
    set_tests_properties(kernel_services_${mode} PROPERTIES TIMEOUT 120)
  endforeach()
endif()
