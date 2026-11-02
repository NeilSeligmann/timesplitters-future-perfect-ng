# T3939: optional presenter-thread provider; the C provider remains the fallback.
if(TSFP_RUST_SDL_PAD)
  if(NOT TSFP_SDL3_FOUND)
    message(FATAL_ERROR "TSFP_RUST_SDL_PAD requires SDL3")
  endif()
  find_program(TSFP_SDL_PAD_CARGO NAMES cargo REQUIRED)
  set(sdl_pad_target "${CMAKE_BINARY_DIR}/rust-sdl-pad")
  set(sdl_pad_library "${sdl_pad_target}/release/libtsfp_input_sdl_provider.so")
  file(GLOB_RECURSE sdl_pad_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-input-sdl-provider/src/*.rs")
  add_custom_command(OUTPUT "${sdl_pad_library}"
    COMMAND "${CMAKE_COMMAND}" -E env CARGO_BUILD_JOBS=4
      "${TSFP_SDL_PAD_CARGO}" build --release --locked
      --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
      --package tsfp-input-sdl-provider --features sdl3 --target-dir "${sdl_pad_target}"
    DEPENDS ${sdl_pad_sources} "${CMAKE_SOURCE_DIR}/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/Cargo.lock"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-input-sdl-provider/Cargo.toml"
    VERBATIM)
  add_custom_target(tsfp_sdl_pad_build DEPENDS "${sdl_pad_library}")
  add_library(tsfp_sdl_pad SHARED IMPORTED)
  set_target_properties(tsfp_sdl_pad PROPERTIES IMPORTED_LOCATION "${sdl_pad_library}"
    IMPORTED_NO_SONAME ON INTERFACE_LINK_OPTIONS "-Wl,-rpath,${sdl_pad_target}/release")
  add_dependencies(tsfp_sdl_pad tsfp_sdl_pad_build)
  target_link_libraries(tsfp_controller_sdl PUBLIC tsfp_sdl_pad)
  target_include_directories(tsfp_controller_sdl PRIVATE crates/tsfp-input-sdl-provider/include)
  target_compile_definitions(tsfp_controller_sdl PRIVATE TSFP_RUST_SDL_PAD_LINKED=1)
endif()
