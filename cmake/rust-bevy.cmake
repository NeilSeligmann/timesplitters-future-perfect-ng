# SPDX-License-Identifier: GPL-3.0-or-later
# T2023: -DTSFP_RUST_BEVY=ON (needs TSFP_RUST_MAIN=ON and SDL3) adds tsfp_host_bevylink (EXCLUDE_FROM_ALL), the host as a
# library for crates/tsfp-bevy where Bevy owns the window, event loop, input and audio. Same sources as tsfp_host_rustlink
# (main.c renamed to tsfp_host_main, tsfp_host_entry), but present_sink.c is linked ahead of the tsfp_host_options archive as a
# second object compiled with TSFP_RUST_WINDOW=1 TSFP_RUST_BEVY=1: window/events/input are the tsfp_wi_* contract and every
# presented picture goes to tsfp_bevy_picture (src/host/bevy_sink.h). src/host/bevy_sink.c holds stand-ins plus a C main so ninja
# can link; crates/tsfp-bevy/build.rs drops it from the real link. src/host/audio_sink_bevy.c replaces the SDL audio device (audio_sink_sdl.c)
# with a pull seam the Bevy audio thread calls. tsfp_host, tsfp_host_rustlink and the tests are unchanged.
# Included after cmake/rust-main.cmake.
if(TSFP_RUST_BEVY)
  if(NOT TSFP_RUST_MAIN)
    message(FATAL_ERROR "TSFP_RUST_BEVY requires TSFP_RUST_MAIN=ON")
  endif()
  if(NOT TSFP_SDL3_FOUND)
    message(FATAL_ERROR "TSFP_RUST_BEVY requires SDL3 (TSFP_WITH_SDL3=ON and SDL3 found): the host sinks and audio ring are SDL3 code")
  endif()
  add_library(tsfp_present_sink_bevy OBJECT src/host/present_sink.c)
  target_link_libraries(tsfp_present_sink_bevy PRIVATE tsfp_host_options)
  target_compile_definitions(tsfp_present_sink_bevy PRIVATE TSFP_RUST_WINDOW=1 TSFP_RUST_BEVY=1)
  set_target_properties(tsfp_present_sink_bevy PROPERTIES POSITION_INDEPENDENT_CODE ON)
  # The Bevy variant needs one extra host callback from main.c into the Rust audio ring.
  # Keep the ordinary Rust host object free of Bevy-only symbols.
  add_library(tsfp_host_main_bevy_obj OBJECT src/host/main.c)
  target_compile_definitions(tsfp_host_main_bevy_obj PRIVATE ${rm_defs} main=tsfp_host_main TSFP_RUST_BEVY=1)
  target_include_directories(tsfp_host_main_bevy_obj PRIVATE ${rm_incs})
  target_link_libraries(tsfp_host_main_bevy_obj PRIVATE ${rm_libs})
  set_target_properties(tsfp_host_main_bevy_obj PROPERTIES POSITION_INDEPENDENT_CODE ON)
  add_executable(tsfp_host_bevylink EXCLUDE_FROM_ALL
    ${rm_sources} $<TARGET_OBJECTS:tsfp_host_main_bevy_obj> $<TARGET_OBJECTS:tsfp_present_sink_bevy>
    src/host/rust_main.c src/host/bevy_sink.c src/host/audio_sink_bevy.c)
  target_compile_definitions(tsfp_host_bevylink PRIVATE ${rm_defs})
  target_include_directories(tsfp_host_bevylink PRIVATE ${rm_incs})
  if(rm_link_opts)
    target_link_options(tsfp_host_bevylink PRIVATE ${rm_link_opts})
  endif()
  target_link_libraries(tsfp_host_bevylink PRIVATE ${rm_libs})
  # Offline Tier 2 comparisons use the Bevy executable too. Give that host the
  # capture runtime and capability define when building the capture profile.
  if(TSFP_BUILD_CAPTURE)
    target_sources(tsfp_host_bevylink PRIVATE src/host/t2_capture.c)
    target_compile_definitions(tsfp_host_bevylink PRIVATE TSFP_T2_CAPTURE_HOST=1)
    find_package(ZLIB REQUIRED)
    target_link_libraries(tsfp_host_bevylink PRIVATE ZLIB::ZLIB)
  endif()
  add_dependencies(tsfp_host_bevylink tsfp_rust_archive)
  set_target_properties(tsfp_host_bevylink PROPERTIES POSITION_INDEPENDENT_CODE ON)
endif()
