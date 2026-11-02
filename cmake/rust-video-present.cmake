# T3938: optional Rust presentation boundary. The C sink remains the default.
if(TSFP_RUST_VIDEO_PRESENT)
  find_program(TSFP_VIDEO_CARGO cargo REQUIRED)
  set(video_target "${CMAKE_BINARY_DIR}/rust-video-present")
  set(video_lib "${video_target}/release/libtsfp_video_present_capi.so")
  file(GLOB_RECURSE video_sources CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/tsfp-video-present/src/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/tsfp-video-present-capi/src/*.rs")
  add_custom_command(OUTPUT "${video_lib}"
    COMMAND "${TSFP_VIDEO_CARGO}" build --release --locked
      --manifest-path "${CMAKE_SOURCE_DIR}/crates/tsfp-video-present-capi/Cargo.toml"
      --target-dir "${video_target}"
    DEPENDS ${video_sources}
      "${CMAKE_SOURCE_DIR}/crates/tsfp-video-present/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-video-present-capi/Cargo.toml"
      "${CMAKE_SOURCE_DIR}/crates/tsfp-video-present-capi/Cargo.lock"
    VERBATIM)
  add_custom_target(tsfp_video_present_build DEPENDS "${video_lib}")
  add_library(tsfp_video_present SHARED IMPORTED)
  set_target_properties(tsfp_video_present PROPERTIES IMPORTED_LOCATION "${video_lib}"
    IMPORTED_NO_SONAME ON INTERFACE_LINK_OPTIONS "-Wl,-rpath,${video_target}/release")
  add_dependencies(tsfp_video_present tsfp_video_present_build)
  target_link_libraries(tsfp_host_options PUBLIC tsfp_video_present)
  target_compile_definitions(tsfp_host_options PRIVATE TSFP_RUST_VIDEO_PRESENT=1)

  add_executable(test_video_present_capi tests/c/test_video_present_capi.c)
  target_include_directories(test_video_present_capi PRIVATE src/host)
  target_link_libraries(test_video_present_capi PRIVATE tsfp_video_present)
  add_test(NAME video_present_capi COMMAND test_video_present_capi)
endif()
