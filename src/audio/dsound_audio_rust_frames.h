/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_AUDIO_DSOUND_AUDIO_RUST_FRAMES_H
#define TSFP_AUDIO_DSOUND_AUDIO_RUST_FRAMES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* T2046: Rust owns the typed DirectSound buffer stack decoder and fault-safe
 * guest-word reads on the Rust SoundHost build. */
typedef struct dsound_audio_buffer_call {
    uint32_t entry;
    uint32_t caller;
    uint32_t args[5];
} dsound_audio_buffer_call;

bool tsfp_audio_buffer_frame_decode(
    uint32_t entry, uint32_t stack_pointer, uint32_t stack_limit,
    bool completion_enabled,
    int32_t host_pid,
    dsound_audio_buffer_call *out);

/* T2046: Rust constructs and writes the passive buffer header, probes the
 * output word without changing it, and publishes the interface pointer last. */
bool tsfp_audio_buffer_output_probe_rust(int32_t host_pid, uint32_t output);
bool tsfp_audio_buffer_header_write_rust(
    int32_t host_pid, uint32_t header, uint32_t vtable,
    uint32_t references, uint32_t parent_internal);
bool tsfp_audio_buffer_pointer_publish_rust(
    int32_t host_pid, uint32_t output, uint32_t header);

/* T2046: Rust owns stream caller admission, bounded decoding, and fault-safe
 * guest-word reads on the Rust SoundHost build. State application remains C. */
typedef struct dsound_audio_stream_call {
    uint32_t entry;
    uint32_t caller;
    uint32_t args[5];
} dsound_audio_stream_call;

bool tsfp_audio_stream_frame_decode(
    uint32_t entry, uint32_t stack_pointer, uint32_t stack_limit,
    bool completion_enabled,
    int32_t host_pid,
    dsound_audio_stream_call *out);

/* Rust validates both optional completion words and publishes Process's initial
 * size=0 then pending-status writes on the Rust SoundHost path. */
bool tsfp_audio_stream_pending_publish_rust(
    int32_t host_pid, uint32_t stream, uint32_t completed, uint32_t status);
/* Rust publishes later completion size then status, preserving independent writes. */
bool tsfp_audio_stream_completion_publish_rust(
    int32_t host_pid, uint32_t completed, uint32_t size, uint32_t status);

/* Rust snapshots and validates the bounded stream ADPCM format after the C adapter's lease and
 * alias guards. Outputs are unchanged on a refused or unreadable guest span. */
bool tsfp_audio_stream_format_decode_rust(
    int32_t host_pid, uint32_t format_address, uint32_t channels,
    uint8_t *out_format, uint32_t *out_sample_rate);

/* Rust publishes XMV movie completion size then status on the Rust SoundHost path. */
bool tsfp_audio_movie_completion_word_probe_rust(int32_t host_pid, uint32_t address);
bool tsfp_audio_movie_completion_publish_rust(
    int32_t host_pid, uint32_t completed, uint32_t size,
    uint32_t status, uint32_t status_value);
/* Rust calls the host guest-callback runner after the object lock is released. */
bool tsfp_audio_movie_completion_call_guest_rust(
    uint32_t callback, uint32_t stream_context, uint32_t packet_context,
    uint32_t status_value);
/* Rust sequences completion publication, locked host bookkeeping/unlock, then guest callback. */
bool tsfp_audio_movie_completion_deliver_rust(
    int32_t host_pid, uint32_t completed, uint32_t size, uint32_t status_address,
    uint32_t status_value, void (*before_unlock)(uint32_t, uint32_t, uint32_t),
    void (*unlock)(void), uint32_t callback, uint32_t stream_context,
    uint32_t packet_context);
/* Rust publishes Process's initial XMV size=0 then pending status on the Rust SoundHost path. */
bool tsfp_audio_movie_pending_publish_rust(
    int32_t host_pid, uint32_t completed, uint32_t status);

/* Rust owns movie packet queue order and virtual deadlines on the Rust SoundHost path. */
typedef struct dsound_audio_movie_schedule_completion {
    uint32_t stream, callback, stream_context, packet_context;
    uint32_t size, status, completed_address, status_address;
} dsound_audio_movie_schedule_completion;
typedef struct dsound_audio_movie_schedule_snapshot {
    uint32_t refs, queued, pause_mode, flush_pending;
    uint64_t head_deadline;
    uint32_t packet_size[2];
} dsound_audio_movie_schedule_snapshot;
bool tsfp_audio_movie_schedule_configure_rust(uint64_t clock_frequency);
bool tsfp_audio_movie_schedule_attach_rust(
    uint32_t stream, uint32_t internal, uint32_t publication_address,
    uint32_t callback, uint32_t context);
bool tsfp_audio_movie_schedule_add_ref_rust(uint32_t stream, uint32_t *refs);
bool tsfp_audio_movie_schedule_release_ref_rust(uint32_t stream, uint32_t *refs);
bool tsfp_audio_movie_schedule_release_is_final_rust(uint32_t stream, bool *is_final);
bool tsfp_audio_movie_schedule_reset_volume_rust(uint32_t stream);
bool tsfp_audio_movie_release_teardown_rust(uint32_t stream);
bool tsfp_audio_movie_schedule_detach_rust(uint32_t stream);
bool tsfp_audio_movie_schedule_completion_word_protected_rust(
    uint32_t stream, uint32_t internal, uint32_t address);
bool tsfp_audio_movie_schedule_get_status_rust(
    int32_t host_pid, uint32_t stream, uint32_t internal, uint32_t output);
bool tsfp_audio_movie_schedule_submit_pcm_rust(
    int32_t host_pid, uint32_t stream, uint64_t ticks, uint32_t buffer, uint32_t bytes);
uint64_t tsfp_audio_movie_pcm_packets_rust(void);
uint64_t tsfp_audio_movie_pcm_frames_rust(void);
uint64_t tsfp_audio_movie_pcm_refused_rust(void);
void tsfp_audio_movie_pcm_stats_reset_rust(void);
uint32_t tsfp_audio_movie_schedule_count_rust(uint32_t stream);
bool tsfp_audio_movie_schedule_has_capacity_rust(uint32_t stream);
uint32_t tsfp_audio_movie_schedule_admission_rust(
    uint32_t stream, uint32_t size, uint32_t completed, uint32_t status);
bool tsfp_audio_movie_schedule_aliases_rust(
    uint32_t stream, uint32_t completed, uint32_t status);
bool tsfp_audio_movie_schedule_enqueue_rust(
    uint32_t stream, uint32_t size, uint32_t completed,
    uint32_t status, uint32_t context, uint64_t now);
bool tsfp_audio_movie_schedule_pause_rust(
    uint32_t stream, uint32_t mode, uint64_t now, uint32_t *result);
bool tsfp_audio_movie_schedule_flush_ex_rust(
    uint32_t stream, uint32_t time_low, uint32_t time_high,
    uint32_t flags, uint64_t now);
void tsfp_audio_movie_schedule_synch_rust(uint64_t now);
bool tsfp_audio_movie_schedule_take_rust(
    uint64_t now, bool everything, uint32_t only_stream,
    dsound_audio_movie_schedule_completion *output);
typedef uint64_t (*dsound_audio_movie_now_ticks_fn)(void);
typedef bool (*dsound_audio_movie_dispatch_fn)(
    uint32_t entry, bool everything,
    const dsound_audio_movie_schedule_completion *event);
size_t tsfp_audio_movie_schedule_drain_rust(
    uint32_t entry, bool everything, uint32_t only_stream,
    dsound_audio_movie_now_ticks_fn now_ticks,
    int32_t (*host_pid)(void),
    void (*before_unlock)(uint32_t size, uint32_t status, uint32_t context),
    void (*unlock)(void),
    void (*refuse)(uint32_t entry, const char *reason));
bool tsfp_audio_movie_schedule_snapshot_rust(
    uint32_t stream, dsound_audio_movie_schedule_snapshot *output);

/* Rust snapshots/decodes Process's six-word packet descriptor before C applies policy. */
bool tsfp_audio_stream_packet_decode_rust(
    int32_t host_pid, uint32_t packet_address, uint32_t words_out[6]);
/* Rust snapshots the six-word XMV packet descriptor; C retains admission policy. */
bool tsfp_audio_movie_packet_decode_rust(
    int32_t host_pid, uint32_t packet_address, uint32_t words_out[6]);

/* Rust preflights packet payload readability before initial completion publication. */
bool tsfp_audio_stream_packet_buffer_probe_rust(
    int32_t host_pid, uint32_t buffer_address, uint32_t buffer_bytes);

/* Rust owns the measured optional-mixer ADPCM packet shape/rate admission on
 * the Rust-host path; unsupported valid formats remain completion-only. */
bool tsfp_audio_stream_packet_classify_rust(
    const uint8_t *format, uint32_t format_bytes, uint32_t packet_bytes,
    uint32_t *kind_out, uint32_t *rate_out);

#endif
