# SPDX-License-Identifier: GPL-3.0-or-later
# T2046: TSFP_RUST_AUDIO_CMP links the Rust DirectSound adapter twin and its C observer.
# TSFP_RUST_AUDIO_HOST replaces the C mixer/runtime symbols with host_runtime.rs exports from
# the same cdylib. The latter leaves the guest-side C DirectSound adapter in place while moving
# command application, decoding, mixer state, and PCM rendering behind SoundHost.
# A new small cdylib like cmake/rust-kmem.cmake, not an addition to tsfp-kmem-capi: the compare needs no memory model, the two compare
# modes stay independently buildable and a host built for one does not link the other.
# TSFP_AUDIO_CMP_TARGET_ROOT overrides the cargo target root (default: inside the build dir).
if(TSFP_RUST_AUDIO_CMP AND TSFP_RUST_AUDIO_HOST)
  message(FATAL_ERROR "TSFP_RUST_AUDIO_CMP needs the retired C audio host (T3959); the compare observer is a test oracle only")
endif()

if(TSFP_RUST_AUDIO_CMP OR TSFP_RUST_AUDIO_HOST OR TSFP_RUST_AUDIO_STREAM_FREQUENCY)
  find_program(TSFP_CARGO NAMES cargo REQUIRED)
  set(audio_cmp_crate "${CMAKE_SOURCE_DIR}/crates/tsfp-audio-capi")
  if(NOT TSFP_AUDIO_CMP_TARGET_ROOT)
    # Migrate a cached pre-feature-key path as the root, so existing build directories
    # naturally move to a configuration-specific output directory on reconfigure.
    if(TSFP_AUDIO_CMP_TARGET_DIR)
      set(TSFP_AUDIO_CMP_TARGET_ROOT "${TSFP_AUDIO_CMP_TARGET_DIR}" CACHE PATH "Cargo target root for the Rust audio C ABI")
    elseif(DEFINED ENV{CARGO_TARGET_DIR})
      set(TSFP_AUDIO_CMP_TARGET_ROOT "$ENV{CARGO_TARGET_DIR}/audio-capi" CACHE PATH "Cargo target root for the Rust audio C ABI")
    else()
      set(TSFP_AUDIO_CMP_TARGET_ROOT "${CMAKE_BINARY_DIR}/rust-audio-cmp" CACHE PATH "Cargo target root for the Rust audio C ABI")
    endif()
  endif()
  set(audio_cmp_features "")
  if(TSFP_RUST_KMEM)
    list(APPEND audio_cmp_features rust-kmem)
  endif()
  if(TSFP_RUST_KCLOCK)
    list(APPEND audio_cmp_features rust-kernel-clock)
  endif()
  if(TSFP_RUST_AUDIO_HOST)
    list(APPEND audio_cmp_features rust-audio-host)
  endif()
  if(TSFP_AUDIO_CMP_FAULT_INJECT)
    list(APPEND audio_cmp_features fault-inject)
  endif()
  if(audio_cmp_features)
    list(JOIN audio_cmp_features "-" audio_cmp_feature_key)
  else()
    set(audio_cmp_feature_key "default")
  endif()
  set(TSFP_AUDIO_CMP_TARGET_DIR "${TSFP_AUDIO_CMP_TARGET_ROOT}/${audio_cmp_feature_key}" CACHE PATH "Feature-specific Cargo target directory for the Rust audio C ABI" FORCE)
  set(audio_cmp_feature_args "")
  if(audio_cmp_features)
    list(JOIN audio_cmp_features "," audio_cmp_feature_set)
    set(audio_cmp_feature_args --features "${audio_cmp_feature_set}")
  endif()
  set(audio_cmp_lib "${TSFP_AUDIO_CMP_TARGET_DIR}/release/libtsfp_audio_capi.so")
  file(GLOB_RECURSE audio_cmp_sources CONFIGURE_DEPENDS
       "${audio_cmp_crate}/src/*.rs" "${CMAKE_SOURCE_DIR}/crates/tsfp-audio/src/*.rs"
       "${CMAKE_SOURCE_DIR}/crates/tsfp-audio-host-wiring/src/*.rs")
  add_custom_command(OUTPUT "${audio_cmp_lib}"
    COMMAND ${TSFP_CARGO} build --release --offline --manifest-path "${audio_cmp_crate}/Cargo.toml"
            --target-dir "${TSFP_AUDIO_CMP_TARGET_DIR}" ${audio_cmp_feature_args}
    DEPENDS ${audio_cmp_sources} "${audio_cmp_crate}/Cargo.toml"
    VERBATIM)
  add_custom_target(tsfp_rust_audio_cmp_lib DEPENDS "${audio_cmp_lib}")
  if(TSFP_RUST_AUDIO_HOST)
    add_dependencies(tsfp_xbox tsfp_rust_audio_cmp_lib)
    target_compile_definitions(tsfp_xbox PRIVATE TSFP_RUST_AUDIO_HOST=1)
    target_link_libraries(tsfp_xbox PUBLIC "${audio_cmp_lib}")
    # The cdylib calls back into these archive members. Shared-library
    # references do not extract static archive members during the first scan.
    # T3916: tsfp-audio-host-wiring calls the C dsound_* modules and xdk_thunk_* routes declared in its ffi.rs and pump.rs; pull each
    # in even where no C caller remains (the list is read from the declarations, so it cannot drift from them).
    file(STRINGS "${CMAKE_SOURCE_DIR}/crates/tsfp-audio-host-wiring/src/ffi.rs" audio_wiring_ffi
         REGEX "^    pub fn (dsound|xdk_thunk)_[a-z0-9_]+")
    file(STRINGS "${CMAKE_SOURCE_DIR}/crates/tsfp-audio-host-wiring/src/pump.rs" audio_pump_ffi
         REGEX "^    fn (dsound|xdk_thunk)_[a-z0-9_]+")
    # The cdylib's extern "C" imports of archive members (kernel, thunk and HLE glue): derived from its sources, so a C unit
    # removed here cannot silently drop the only reference that used to extract the member.
    file(GLOB audio_capi_sources "${CMAKE_SOURCE_DIR}/crates/tsfp-audio-capi/src/*.rs")
    set(audio_capi_imports "")
    foreach(source IN LISTS audio_capi_sources)
      file(STRINGS "${source}" import_lines
           REGEX "^    (pub )?fn (kernel_critsec|kernel_guest|kernel_frame|kernel_call|xdk_thunk|dsound_hle|dsound_device|tsfp_audio_dev|tsfp_audio_movie_release)_[a-z0-9_]+\\(")
      list(APPEND audio_capi_imports ${import_lines})
    endforeach()
    set(audio_wiring_undefined "")
    foreach(line IN LISTS audio_capi_imports)
      # A source line with a `;` is split by CMake: only the fragment that still starts with `fn` names a symbol.
      if(line MATCHES "^    (pub )?fn ([a-z0-9_]+)\\(")
        list(APPEND audio_wiring_undefined "LINKER:-u,${CMAKE_MATCH_2}")
      endif()
    endforeach()
    list(REMOVE_DUPLICATES audio_wiring_undefined)
    foreach(line IN LISTS audio_wiring_ffi audio_pump_ffi)
      string(REGEX REPLACE "^    (pub )?fn ([a-z0-9_]+).*$" "\\2" symbol "${line}")
      list(APPEND audio_wiring_undefined "LINKER:-u,${symbol}")
    endforeach()
    target_link_options(tsfp_xbox INTERFACE ${audio_wiring_undefined}
      "LINKER:-u,tsfp_audio_host_reset_all"
      "LINKER:-u,tsfp_audio_movie_release_drain_host_c"
      "LINKER:--export-dynamic"
      "-Wl,-rpath,${TSFP_AUDIO_CMP_TARGET_DIR}/release")
  endif()
endif()
