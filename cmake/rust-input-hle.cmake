# T3879 IN-3: link the Rust-owned XInput enumeration model behind a second, runtime opt-in.
# C remains the default and the synthetic pad path remains in the existing C adapter.
if(TSFP_RUST_INPUT_HLE)
  find_program(TSFP_INPUT_CARGO NAMES cargo REQUIRED)
  set(input_hle_target "${CMAKE_BINARY_DIR}/rust-input-hle")
  set(input_hle_library "${input_hle_target}/release/libtsfp_input_capi.so")
  file(GLOB_RECURSE input_hle_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-capi/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-devices/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-record/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-host-wiring/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-route-spec/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-hotkey-host/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-route-engine/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-mu/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-state/src/*.rs")
  add_custom_command(OUTPUT "${input_hle_library}"
    COMMAND "${CMAKE_COMMAND}" -E env CARGO_BUILD_JOBS=4
      "${TSFP_INPUT_CARGO}" build --release --locked --features rust-input-host
      --manifest-path "${CMAKE_SOURCE_DIR}/crates/tsfp-input-capi/Cargo.toml"
      --target-dir "${input_hle_target}"
    DEPENDS ${input_hle_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-devices/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-record/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-capi/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-host-wiring/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-route-spec/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-hotkey-host/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-route-engine/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-mu/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-capi/Cargo.lock"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-state/Cargo.toml"
    VERBATIM)
  add_custom_target(tsfp_input_hle_build DEPENDS "${input_hle_library}")
  add_library(tsfp_input_hle SHARED IMPORTED)
  set_target_properties(tsfp_input_hle PROPERTIES IMPORTED_LOCATION "${input_hle_library}"
    IMPORTED_NO_SONAME ON INTERFACE_LINK_OPTIONS "-Wl,-rpath,${input_hle_target}/release")
  add_dependencies(tsfp_input_hle tsfp_input_hle_build)
  add_executable(test_input_dispatch_abi tests/c/test_input_dispatch_abi.c)
  target_include_directories(test_input_dispatch_abi PRIVATE crates/tsfp-input-capi/include)
  target_link_libraries(test_input_dispatch_abi PRIVATE tsfp_input_hle)
  add_test(NAME input_dispatch_abi COMMAND test_input_dispatch_abi)
  target_sources(tsfp_xbox PRIVATE src/input/rust_input_devices_bridge.c)
  target_include_directories(tsfp_xbox PRIVATE crates/tsfp-input-capi/include)
  target_compile_definitions(tsfp_xbox PRIVATE TSFP_INPUT_HLE_LINKED=1)
  target_link_libraries(tsfp_xbox PUBLIC tsfp_input_hle)
  # T3975: tsfp-input-host-wiring (linked into the cdylib) calls the C XInput modules declared in its ffi.rs; the
  # shared library cannot extract archive members, so pull each in (list read from the declarations) and export them.
  file(STRINGS "${CMAKE_SOURCE_DIR}/crates/tsfp-input-host-wiring/src/ffi.rs" input_wiring_ffi
       REGEX "^    pub fn (xinput|xvoice|mu)_[a-z0-9_]+")
  set(input_wiring_undefined "")
  foreach(line IN LISTS input_wiring_ffi)
    string(REGEX REPLACE "^    pub fn ([a-z0-9_]+).*$" "\\1" symbol "${line}")
    list(APPEND input_wiring_undefined "LINKER:-u,${symbol}")
  endforeach()
  # Slice 5b: the route player's own C dependencies (the nav machine, the script, the source dispatcher).
  file(STRINGS "${CMAKE_SOURCE_DIR}/crates/tsfp-input-route-engine/src/sys.rs" input_route_ffi
       REGEX "^    pub fn (route_nav|xinput)_[a-z0-9_]+")
  foreach(line IN LISTS input_route_ffi)
    string(REGEX REPLACE "^    pub fn ([a-z0-9_]+).*$" "\\1" symbol "${line}")
    list(APPEND input_wiring_undefined "LINKER:-u,${symbol}")
  endforeach()
  # Slice 7: the MU device (mu_device.c, mu_startup.c) is tsfp-input-mu; its own C dependencies (FATX, guest accessors, rfio).
  file(STRINGS "${CMAKE_SOURCE_DIR}/crates/tsfp-input-mu/src/sys.rs" input_mu_ffi
       REGEX "^    pub fn (fatx|kernel|tsfp_rfio|xinput)_[a-z0-9_]+")
  foreach(line IN LISTS input_mu_ffi)
    string(REGEX REPLACE "^    pub fn ([a-z0-9_]+).*$" "\\1" symbol "${line}")
    list(APPEND input_wiring_undefined "LINKER:-u,${symbol}")
  endforeach()
  target_link_options(tsfp_xbox INTERFACE ${input_wiring_undefined} "LINKER:-u,tsfp_input_host_reset_all"
                      "LINKER:-u,xinput_host_key_effect" "LINKER:-u,xinput_record_suppress_begin"
                      "LINKER:-u,xinput_record_suppress_end"
                      "LINKER:--export-dynamic")
  target_compile_definitions(test_xinput_devices PRIVATE TSFP_INPUT_HLE_LINKED=1)

  # Run the existing dispatcher/guest-memory conformance cases a second time through Rust.
  set_tests_properties(xinput_devices PROPERTIES ENVIRONMENT "TSFP_RUST_INPUT_HLE=0")
  add_test(NAME xinput_devices_rust COMMAND test_xinput_devices)
  set_tests_properties(xinput_devices_rust PROPERTIES ENVIRONMENT "TSFP_RUST_INPUT_HLE=1")
  target_compile_definitions(test_xinput_hle PRIVATE TSFP_INPUT_HLE_LINKED=1)
  add_test(NAME xinput_hle_rust COMMAND test_xinput_hle)
  set_tests_properties(xinput_hle_rust PROPERTIES ENVIRONMENT "TSFP_RUST_INPUT_HLE=1")
endif()
