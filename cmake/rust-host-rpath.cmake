# SPDX-License-Identifier: GPL-3.0-or-later
# T2235: emit relative paths once, ahead of every absolute fallback. Repeating
# combined -rpath options would interleave relative entries with fallbacks.
include_guard(GLOBAL)
function(tsfp_rust_host_rpath target fallback)
  get_property(has_origin TARGET "${target}" PROPERTY TSFP_HAS_ORIGIN_RPATH)
  if(NOT has_origin)
    target_link_options("${target}" PRIVATE
      "-Wl,-rpath,$ORIGIN:$ORIGIN/rust-fileio/release:$ORIGIN/rust-rt-host/release:$ORIGIN/rust-kmem/release")
    set_property(TARGET "${target}" PROPERTY TSFP_HAS_ORIGIN_RPATH TRUE)
  endif()
  target_link_options("${target}" PRIVATE "-Wl,-rpath,${fallback}")
endfunction()
