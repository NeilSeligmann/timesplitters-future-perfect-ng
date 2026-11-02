# SPDX-License-Identifier: GPL-3.0-or-later
# T2003: -DTSFP_RUST_WINDOW=ON (needs TSFP_RUST_MAIN=ON) builds the window, event loop and host input module of
# src/host/present_sink.c against the tsfp_wi_* contract (src/host/window_input.h) that crates/tsfp-game/src/sdl_window.rs
# defines, instead of its own SDL code. Only tsfp_host_rustlink changes: a second object of present_sink.c compiled with
# TSFP_RUST_WINDOW is linked ahead of the tsfp_host_options archive, so the archive's present_sink.o member (default,
# SDL code inside) is never pulled in. tsfp_host, the tests and the default build keep the C implementation.
# crates/tsfp-game/build.rs reads TSFP_RUST_WINDOW from CMakeCache.txt to enable the Rust side (cfg tsfp_rust_window).
# Included after cmake/rust-main.cmake.
if(TSFP_RUST_WINDOW)
  if(NOT TSFP_RUST_MAIN)
    message(FATAL_ERROR "TSFP_RUST_WINDOW requires TSFP_RUST_MAIN=ON")
  endif()
  if(NOT TSFP_SDL3_FOUND)
    message(FATAL_ERROR "TSFP_RUST_WINDOW requires SDL3 (TSFP_WITH_SDL3=ON and SDL3 found)")
  endif()
  add_library(tsfp_present_sink_rustwin OBJECT src/host/present_sink.c)
  target_link_libraries(tsfp_present_sink_rustwin PRIVATE tsfp_host_options)
  target_compile_definitions(tsfp_present_sink_rustwin PRIVATE TSFP_RUST_WINDOW=1)
  set_target_properties(tsfp_present_sink_rustwin PROPERTIES POSITION_INDEPENDENT_CODE ON)
  # The stub defines the contract symbols only so the stub link succeeds; build.rs drops it from the real link.
  target_sources(tsfp_host_rustlink PRIVATE $<TARGET_OBJECTS:tsfp_present_sink_rustwin> src/host/rust_window_link_stub.c)
endif()
