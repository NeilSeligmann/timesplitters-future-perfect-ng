# SPDX-License-Identifier: GPL-3.0-or-later
# T3935: crates/tsfp-kmem-capi (the Rust KernelMemory of tsfp-engine-memory behind a C ABI, with a real low-4GB mmap
# placement) is the only guest-memory implementation, so every consumer of tsfp_xbox links it. TSFP_KMEM_TARGET_DIR
# overrides the cargo target dir (default: inside the build dir). tsfp_kmem_link carries the library, its rpath and the
# one remaining link wrap (kernel_hle_register, for the opt-in RTL thunks of src/xbox/kmem_rust.c).
find_program(TSFP_CARGO NAMES cargo REQUIRED)
set(kmem_crate "${CMAKE_SOURCE_DIR}/crates/tsfp-kmem-capi")
if(NOT TSFP_KMEM_TARGET_DIR)
  set(TSFP_KMEM_TARGET_DIR "${CMAKE_BINARY_DIR}/rust-kmem")
endif()
set(kmem_lib "${TSFP_KMEM_TARGET_DIR}/release/libtsfp_kmem_capi.so")
file(GLOB_RECURSE kmem_sources CONFIGURE_DEPENDS
     "${kmem_crate}/src/*.rs" "${CMAKE_SOURCE_DIR}/crates/tsfp-engine-memory/src/*.rs")
add_custom_command(OUTPUT "${kmem_lib}"
  COMMAND ${TSFP_CARGO} build --release --offline --manifest-path "${kmem_crate}/Cargo.toml"
          --target-dir "${TSFP_KMEM_TARGET_DIR}"
  DEPENDS ${kmem_sources} "${kmem_crate}/Cargo.toml"
  VERBATIM)
add_custom_target(tsfp_rust_kmem_lib DEPENDS "${kmem_lib}")
add_library(tsfp_kmem_link INTERFACE)
add_dependencies(tsfp_kmem_link tsfp_rust_kmem_lib)
target_link_libraries(tsfp_kmem_link INTERFACE "${kmem_lib}")
target_link_options(tsfp_kmem_link INTERFACE "-Wl,--wrap=kernel_hle_register"
  "-Wl,-rpath,${TSFP_KMEM_TARGET_DIR}/release")
