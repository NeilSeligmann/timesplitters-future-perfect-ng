# T3879: the Rust dsp56300-emu core is the only effects DSP of the Rust audio host (PCM EQUAL to the retired C core,
# docs/t3949-gpdsp.md). The pinned-xemu C core (dsound_effects_dsp.c) lives on only as a test oracle in
# crates/tsfp-audio-conformance/oracle.
if(NOT TSFP_RUST_AUDIO_HOST)
  message(FATAL_ERROR "the GP effects DSP is served by the Rust audio host (tsfp_gpdsp_* ABI in tsfp-audio-capi)")
endif()
message(STATUS "T3949: GP effects DSP served by Rust (no xemu DSP source needed)")
target_compile_definitions(tsfp_xbox PRIVATE TSFP_DSOUND_DSP_ENABLED=1 TSFP_GPDSP_RUST=1)
