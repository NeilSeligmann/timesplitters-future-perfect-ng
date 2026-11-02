# SPDX-License-Identifier: GPL-3.0-or-later
# Drop cached Python interpreter paths that no longer exist (T3921).
#
# find_program and find_package keep a cached value without searching again. A build
# configured under a temporary PATH shim (a python3 symlink in a temp dir that is then
# deleted) cached that shim, so a reused build dir failed later with an empty error.
# Dropping the stale entry makes the next find detect the interpreter again, loudly.
function(tsfp_drop_dangling_python_cache)
  get_cmake_property(cache_vars CACHE_VARIABLES)
  foreach(var IN LISTS cache_vars)
    string(TOUPPER "${var}" upper_var)
    if(NOT upper_var MATCHES "PYTHON")
      continue()
    endif()
    set(value "${${var}}")
    if(value STREQUAL "")
      continue()
    endif()
    get_filename_component(value_name "${value}" NAME)
    if(value_name MATCHES "^python" AND NOT EXISTS "${value}")
      message(WARNING "cached ${var}=${value} no longer exists (temporary interpreter?): "
                      "dropped it, re-detecting Python")
      unset(${var} CACHE)
    endif()
  endforeach()
endfunction()
