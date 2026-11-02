# SPDX-License-Identifier: GPL-3.0-or-later
# T2001: -DTSFP_RUST_MAIN=ON adds the tsfp_host_rustlink target (not part of `all`). It is tsfp_host with
# main.c compiled with main renamed to tsfp_host_main, plus src/host/rust_main.c (tsfp_host_entry) and a stub C
# main. crates/tsfp-game/build.rs runs `ninja -t commands tsfp_host_rustlink`, takes its link line without the
# stub and links it into the Rust binary, whose main calls tsfp_host_entry. Default builds are unchanged.
# Included after cmake/rust-ports.cmake so the copied definitions and sources include the Rust lane.
if(TSFP_RUST_MAIN)
  if(NOT TSFP_RUST_PORTS)
    message(FATAL_ERROR "TSFP_RUST_MAIN requires TSFP_RUST_PORTS=ON")
  endif()
  get_target_property(rm_sources tsfp_host SOURCES)
  list(REMOVE_ITEM rm_sources src/host/main.c)
  get_target_property(rm_libs tsfp_host LINK_LIBRARIES)
  get_target_property(rm_defs tsfp_host COMPILE_DEFINITIONS)
  get_target_property(rm_incs tsfp_host INCLUDE_DIRECTORIES)
  get_target_property(rm_link_opts tsfp_host LINK_OPTIONS)
  add_library(tsfp_host_main_obj OBJECT src/host/main.c)
  target_compile_definitions(tsfp_host_main_obj PRIVATE ${rm_defs} main=tsfp_host_main)
  target_include_directories(tsfp_host_main_obj PRIVATE ${rm_incs})
  target_link_libraries(tsfp_host_main_obj PRIVATE ${rm_libs})
  set_target_properties(tsfp_host_main_obj PROPERTIES POSITION_INDEPENDENT_CODE ON)
  add_executable(tsfp_host_rustlink EXCLUDE_FROM_ALL
    ${rm_sources} $<TARGET_OBJECTS:tsfp_host_main_obj>
    src/host/rust_main.c src/host/rust_main_link_stub.c)
  target_compile_definitions(tsfp_host_rustlink PRIVATE ${rm_defs})
  target_include_directories(tsfp_host_rustlink PRIVATE ${rm_incs})
  if(rm_link_opts)
    target_link_options(tsfp_host_rustlink PRIVATE ${rm_link_opts})
  endif()
  target_link_libraries(tsfp_host_rustlink PRIVATE ${rm_libs})
  add_dependencies(tsfp_host_rustlink tsfp_rust_archive)
  set_target_properties(tsfp_host_rustlink PROPERTIES POSITION_INDEPENDENT_CODE ON)
endif()
