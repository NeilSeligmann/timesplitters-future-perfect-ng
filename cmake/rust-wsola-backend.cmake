# SPDX-License-Identifier: GPL-3.0-or-later
# T2046: independently built, runtime opt-in WSOLA backend.
find_program(TSFP_CARGO NAMES cargo REQUIRED)
set(wsola_crate "${CMAKE_SOURCE_DIR}/crates/tsfp-wsola-backend")
if(DEFINED ENV{CARGO_TARGET_DIR})
  set(wsola_target "$ENV{CARGO_TARGET_DIR}/wsola-backend")
else()
  set(wsola_target "${CMAKE_BINARY_DIR}/rust-wsola-backend")
endif()
set(wsola_lib "${wsola_target}/release/libtsfp_wsola_backend.so")
file(GLOB_RECURSE wsola_sources CONFIGURE_DEPENDS
  "${wsola_crate}/src/*.rs" "${CMAKE_SOURCE_DIR}/crates/tsfp-audio/src/*.rs")
add_custom_command(OUTPUT "${wsola_lib}"
  COMMAND ${TSFP_CARGO} build --release --offline --manifest-path "${wsola_crate}/Cargo.toml"
          --target-dir "${wsola_target}"
  DEPENDS ${wsola_sources} "${wsola_crate}/Cargo.toml" "${CMAKE_SOURCE_DIR}/crates/tsfp-audio/Cargo.toml"
  VERBATIM)
add_custom_target(tsfp_rust_wsola_lib DEPENDS "${wsola_lib}")
foreach(target tsfp_host_options tsfp_host_options_t926)
  add_dependencies(${target} tsfp_rust_wsola_lib)
  # Rust/Bevy window builds compile present_sink.c again in object targets
  # which link these libraries for their usage requirements.
  target_compile_definitions(${target} PUBLIC TSFP_RUST_WSOLA=1)
  target_link_libraries(${target} PUBLIC "${wsola_lib}")
  target_link_options(${target} INTERFACE "-Wl,-rpath,${wsola_target}/release")
endforeach()
