# SPDX-License-Identifier: GPL-3.0-or-later
# T3879: optional native owner of the existing file-I/O ABI. C remains the OFF oracle.
include_guard(GLOBAL)

# Follow local Cargo path dependencies so edits to a newly split leaf invalidate the
# shared library too. Cargo owns registry resolution; do not hard-code a leaf list.
function(tsfp_fileio_inputs crate output)
  set(pending "${crate}")
  set(visited)
  set(inputs)
  while(pending)
    list(POP_FRONT pending current)
    get_filename_component(current "${current}" REALPATH)
    if(current IN_LIST visited)
      continue()
    endif()
    list(APPEND visited "${current}")
    file(GLOB_RECURSE sources CONFIGURE_DEPENDS "${current}/src/*.rs")
    list(APPEND inputs ${sources} "${current}/Cargo.toml")
    foreach(extra Cargo.lock build.rs .cargo/config.toml)
      if(EXISTS "${current}/${extra}")
        list(APPEND inputs "${current}/${extra}")
      endif()
    endforeach()
    file(READ "${current}/Cargo.toml" manifest)
    string(REGEX MATCHALL "path[ \t]*=[ \t]*\"[^\"]+\"" local_paths "${manifest}")
    foreach(entry IN LISTS local_paths)
      string(REGEX REPLACE "^path[ \t]*=[ \t]*\"([^\"]+)\"$" "\\1" relative "${entry}")
      # Cargo's [lib] path is a source file, not a package dependency.
      if(EXISTS "${current}/${relative}/Cargo.toml")
        list(APPEND pending "${current}/${relative}")
      endif()
    endforeach()
  endwhile()
  set(${output} ${inputs} PARENT_SCOPE)
endfunction()

function(tsfp_native_fileio_runtime)
  if(NOT TARGET tsfp_native_fileio_runtime)
    set(crate "${CMAKE_SOURCE_DIR}/crates/tsfp-kernel-file-runtime")
    set(directory "${CMAKE_BINARY_DIR}/rust-kernel-file")
    set(archive "${directory}/release/libtsfp_kernel_file_runtime.so")
    tsfp_fileio_inputs("${crate}" sources)
    add_custom_command(OUTPUT "${archive}"
      COMMAND "${TSFP_CARGO}" build --release --manifest-path "${crate}/Cargo.toml"
        --target-dir "${directory}"
      DEPENDS ${sources} VERBATIM)
    add_custom_target(tsfp_native_fileio_archive DEPENDS "${archive}")
    add_library(tsfp_native_fileio_runtime SHARED IMPORTED GLOBAL)
    set_target_properties(tsfp_native_fileio_runtime PROPERTIES IMPORTED_LOCATION "${archive}"
      IMPORTED_NO_SONAME ON
      INTERFACE_LINK_OPTIONS "-Wl,--export-dynamic;-Wl,-rpath,$ORIGIN/rust-kernel-file/release;-Wl,-rpath,${directory}/release")
    add_dependencies(tsfp_native_fileio_runtime tsfp_native_fileio_archive)
  endif()
endfunction()

function(tsfp_fileio_capi)
  if(TARGET tsfp_rust_fileio_archive)
    return()
  endif()
  find_program(TSFP_CARGO NAMES cargo REQUIRED)
  set(crate "${CMAKE_SOURCE_DIR}/crates/tsfp-fileio-capi")
  if(TSFP_NATIVE_FILEIO)
    set(directory "${CMAKE_BINARY_DIR}/rust-fileio-native")
    set(cargo_features --features native-fileio)
  else()
    set(directory "${CMAKE_BINARY_DIR}/rust-fileio")
    set(cargo_features)
  endif()
  set(archive "${directory}/release/libtsfp_fileio_capi.so")
  tsfp_fileio_inputs("${crate}" sources)
  add_custom_command(OUTPUT "${archive}"
    COMMAND "${TSFP_CARGO}" build --release --manifest-path "${crate}/Cargo.toml"
      --target-dir "${directory}" ${cargo_features}
    DEPENDS ${sources} VERBATIM)
  add_custom_target(tsfp_rust_fileio_archive DEPENDS "${archive}")
  add_library(tsfp_fileio_capi SHARED IMPORTED GLOBAL)
  set_target_properties(tsfp_fileio_capi PROPERTIES IMPORTED_LOCATION "${archive}"
    IMPORTED_NO_SONAME ON
    INTERFACE_LINK_OPTIONS "-Wl,-rpath,$ORIGIN/rust-fileio/release;-Wl,-rpath,${directory}/release")
  add_dependencies(tsfp_fileio_capi tsfp_rust_fileio_archive)
  # The capi imports the kernel_file_* ABI, which only the Rust runtime defines (T3931 retired the C
  # units). Linking the runtime through the capi target makes every capi user link standalone.
  if(TSFP_NATIVE_FILEIO)
    tsfp_native_fileio_runtime()
    target_link_libraries(tsfp_fileio_capi INTERFACE tsfp_native_fileio_runtime)
  endif()
endfunction()

function(tsfp_native_fileio target)
  tsfp_fileio_capi()
  # The CAPI DSO imports kernel_file_* callbacks from the runtime DSO. Keep
  # the provider after the consumer so --as-needed linkers retain it.
  target_link_libraries(${target} PUBLIC tsfp_fileio_capi tsfp_native_fileio_runtime)
  target_compile_definitions(${target} PUBLIC TSFP_NATIVE_FILEIO=1)
  # Derive aliases from the public ABI, not a second list of exported functions.
  file(READ "${CMAKE_SOURCE_DIR}/src/xbox/xdvdfs.h" header)
  string(REGEX MATCHALL "xdvdfs_[a-z_]+[ \t\r\n]*\\(" declarations "${header}")
  set(aliases)
  foreach(declaration IN LISTS declarations)
    string(REGEX MATCH "xdvdfs_[a-z_]+" symbol "${declaration}")
    list(APPEND aliases "${symbol}=tsfp_native_${symbol}")
  endforeach()
  list(REMOVE_DUPLICATES aliases)
  if(NOT aliases)
    message(FATAL_ERROR "Native file-I/O requires the XDVDFS ABI declarations")
  endif()
  target_compile_definitions(${target} PUBLIC ${aliases})
endfunction()
