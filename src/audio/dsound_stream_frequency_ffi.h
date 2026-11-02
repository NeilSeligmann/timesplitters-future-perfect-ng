/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_DSOUND_STREAM_FREQUENCY_FFI_H
#define TSFP_DSOUND_STREAM_FREQUENCY_FFI_H
#include <stdbool.h>
#include <stdint.h>

typedef bool (*dsound_stream_frequency_apply_fn)(void *context, uint32_t stream,
    uint64_t serial, uint32_t old_hz, uint32_t new_hz);
typedef bool (*dsound_stream_frequency_format_apply_fn)(void *context, uint32_t stream,
    uint64_t serial, uint32_t old_hz, uint32_t new_hz, uint32_t format_address);

typedef struct dsound_stream_frequency_state {
    uint32_t accepted_sets;
    uint32_t current_hz;
    int32_t pitch;
} dsound_stream_frequency_state;

/* Rust owns the measured pitch, accepted-event count and current-rate sidecar. The callback
 * remains the synchronous C completion adapter for clock/history and guest-word effects. */
bool tsfp_audio_stream_frequency_commit_rust(uint32_t stream, uint64_t serial,
    uint32_t accepted_sets, uint32_t old_hz, uint32_t new_hz, uint16_t control_word,
    void *context, dsound_stream_frequency_apply_fn apply, int32_t *pitch_out);
/* `apply` may be NULL only when no PCM runtime exists; Rust still commits the measured logical
 * rate/pitch after validating lease generation, prior rate, count, domain and x87 control word. */
bool tsfp_audio_stream_frequency_format_commit_rust(uint32_t stream, uint64_t serial,
    uint32_t accepted_sets, uint32_t old_hz, uint32_t new_hz, uint16_t control_word,
    uint32_t format_address, void *context, dsound_stream_frequency_format_apply_fn apply,
    int32_t *pitch_out);
/* Reads the Rust-owned lease-bound transaction; failure leaves output unchanged. */
bool tsfp_audio_stream_frequency_state_rust(uint32_t stream, uint64_t serial,
    dsound_stream_frequency_state *state_out);
bool tsfp_audio_stream_frequency_pitch_rust(uint32_t hertz, uint16_t control_word,
    int32_t *pitch_out);

#endif
