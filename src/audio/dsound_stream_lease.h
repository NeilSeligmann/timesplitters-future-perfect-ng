/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_AUDIO_DSOUND_STREAM_LEASE_H
#define TSFP_AUDIO_DSOUND_STREAM_LEASE_H
#include <stdbool.h>
#include <stdint.h>
#include "dsound_device.h"
#include "dsound_stream_scope.h"

typedef enum dsound_stream_lease_probe_stage {
    DSOUND_STREAM_LEASE_DEVICE_HEAP = 0,
    DSOUND_STREAM_LEASE_GUEST_HEADER = 1,
    DSOUND_STREAM_LEASE_STREAM_HEAP = 2,
    DSOUND_STREAM_LEASE_BLOCK_SIZE = 3
} dsound_stream_lease_probe_stage;

/* C-heap fallback probes called by the Rust-owned predicate in measured order.
 * When Rust kmem is active, the Rust validator calls GuestRegions directly. */
typedef bool (*dsound_stream_lease_probe_fn)(void *context, uint32_t stage,
    uint32_t heap, uint32_t address, uint32_t *block_bytes);

/* Mutable SetFormat state is separate from the immutable creation format scope. */
typedef struct dsound_stream_format_state {
    uint8_t bytes[DSOUND_STREAM_FORMAT_BYTES];
    uint32_t sets;
} dsound_stream_format_state;

/* Last accepted SetVolume projection for one exact active stream generation. */
typedef struct dsound_stream_volume_state {
    uint32_t seen;
    int32_t value;
} dsound_stream_volume_state;

/* Caller facts for Rust's measured SetVolume policy; this plans state only. */
typedef struct dsound_stream_volume_request {
    uint32_t argument, count, completion_available;
} dsound_stream_volume_request;

/* Accepted Pause/FlushEx/Discontinuity observations for one exact generation.
 * These are handler observations; they do not drive playback or packet completion. */
typedef struct dsound_stream_control_state {
    uint32_t pause_seen,pause_mode,flush_seen,flush_time_low;
    uint32_t flush_time_high,flush_flags,discontinuity_seen,revision;
} dsound_stream_control_state;

enum {
    DSOUND_STREAM_CONTROL_DISCONTINUITY = 1,
    DSOUND_STREAM_CONTROL_FLUSH_EX = 2,
    DSOUND_STREAM_CONTROL_PAUSE = 3
};
typedef struct dsound_stream_control_request {
    uint32_t action,argument,count,apply,completion_available,pause_callback_available;
} dsound_stream_control_request;

/* Ordered startup spatial cache and accepted setter observations for one exact
 * active generation. This does not apply voice state or produce spatial playback. */
typedef struct dsound_stream_spatial_state {
    uint32_t startup_cache_mask,i3dl2_address,i3dl2_words[9],min_distance_bits;
    uint32_t position_bits[3],position_sets;
    uint32_t max_distance_seen,max_distance_bits;
    uint32_t curve_address,curve_count;
    uint32_t revision;
} dsound_stream_spatial_state;

/* Validates exact lease identity, required internal address, guest header equality,
 * and the exclusive 40-byte stream allocation. Rust reads the header via process_vm
 * and probes GuestRegions directly when linked/active; otherwise it calls the C-heap fallback. */
bool tsfp_audio_stream_lease_validate_rust(const dsound_device_lease *owned_lease,
    const dsound_device_lease *identity, uint32_t required_internal_address,
    uint32_t stream_address, uint32_t stream_heap, const uint32_t expected_header[10],
    void *context, dsound_stream_lease_probe_fn probe);
bool tsfp_audio_stream_lease_validate_record_rust(uint32_t stream_address,
    const dsound_device_lease *identity, uint32_t required_internal_address,
    void *context, dsound_stream_lease_probe_fn probe);
bool tsfp_audio_stream_lease_identity_matches_rust(const dsound_device_lease *left,
    const dsound_device_lease *right);
/* Rust owns lifecycle, creation scope, protected guest spans, and selected accepted
 * stream observations. The C adapter still owns the remaining handler payload. */
bool tsfp_audio_stream_lease_reserve_rust(uint32_t stream_address, uint32_t stream_heap,
    uint32_t publication_address, const uint32_t header[10], const dsound_stream_scope *scope);
/* Copies the immutable creation scope only for the exact active lease generation. */
bool tsfp_audio_stream_lease_scope_rust(uint32_t stream_address,
    const dsound_device_lease *lease, dsound_stream_scope *scope_out);
/* Read and compare/commit accepted SetFormat state for an exact active generation. */
bool tsfp_audio_stream_lease_format_state_rust(uint32_t stream_address,
    const dsound_device_lease *lease, dsound_stream_format_state *state_out);
bool tsfp_audio_stream_lease_format_commit_rust(uint32_t stream_address,
    const dsound_device_lease *lease, const dsound_stream_format_state *expected,
    const uint8_t bytes[DSOUND_STREAM_FORMAT_BYTES]);
bool tsfp_audio_stream_lease_volume_state_rust(uint32_t stream_address,
    const dsound_device_lease *lease, dsound_stream_volume_state *state_out);
bool tsfp_audio_stream_lease_volume_commit_rust(uint32_t stream_address,
    const dsound_device_lease *lease, const dsound_stream_volume_state *expected,
    const dsound_stream_volume_state *next);
uint32_t tsfp_audio_stream_lease_volume_apply_rust(uint32_t stream_address,
    const dsound_device_lease *lease, const dsound_stream_volume_request *request,
    dsound_stream_volume_state *next_out);
bool tsfp_audio_stream_lease_control_state_rust(uint32_t stream_address,
    const dsound_device_lease *lease, dsound_stream_control_state *state_out);
bool tsfp_audio_stream_lease_control_commit_rust(uint32_t stream_address,
    const dsound_device_lease *lease, const dsound_stream_control_state *expected,
    const dsound_stream_control_state *next);
/* The Rust-host audio path evaluates and commits the bounded startup handler policy here. */
uint32_t tsfp_audio_stream_lease_control_apply_rust(uint32_t stream_address,
    const dsound_device_lease *lease, const dsound_stream_control_request *request);
bool tsfp_audio_stream_lease_spatial_state_rust(uint32_t stream_address,
    const dsound_device_lease *lease, dsound_stream_spatial_state *state_out);
bool tsfp_audio_stream_lease_spatial_commit_rust(uint32_t stream_address,
    const dsound_device_lease *lease, const dsound_stream_spatial_state *expected,
    const dsound_stream_spatial_state *next);
bool tsfp_audio_stream_lease_bind_rust(uint32_t stream_address,
    const dsound_device_lease *lease);
bool tsfp_audio_stream_lease_cancel_pending_rust(uint32_t stream_address);
bool tsfp_audio_stream_lease_retain_rollback_rust(uint32_t stream_address);
bool tsfp_audio_stream_lease_detach_rust(uint32_t stream_address,
    const dsound_device_lease *lease);
bool tsfp_audio_stream_lease_begin_retire_detached_rust(uint32_t stream_address,
    const dsound_device_lease *lease);
bool tsfp_audio_stream_lease_restore_detached_rust(uint32_t stream_address,
    const dsound_device_lease *lease);
bool tsfp_audio_stream_lease_retire_detached_rust(uint32_t stream_address,
    const dsound_device_lease *lease);
bool tsfp_audio_stream_lease_begin_retire_rollback_rust(uint32_t stream_address);
bool tsfp_audio_stream_lease_restore_rollback_rust(uint32_t stream_address);
bool tsfp_audio_stream_lease_retire_rollback_rust(uint32_t stream_address);
/* Rust-host private heap + stream-object allocation. A created heap is returned
 * even when its object allocation fails, so the caller can run normal rollback. */
bool tsfp_audio_stream_heap_allocate_rust(uint32_t bytes, uint32_t *heap_out,
    uint32_t *address_out);
bool tsfp_audio_stream_heap_valid_rust(uint32_t heap);
bool tsfp_audio_stream_heap_destroy_rust(uint32_t heap);
/* Rust owns active/detached membership and list-head order on the Rust-host path. */
bool tsfp_audio_stream_lease_first_active_rust(uint32_t *stream_address_out);
bool tsfp_audio_stream_lease_first_detached_rust(uint32_t *stream_address_out);
bool tsfp_audio_stream_lease_is_active_rust(uint32_t stream_address);
bool tsfp_audio_stream_lease_header_rust(uint32_t stream_address,
    const dsound_device_lease *lease, uint32_t header_out[10]);
/* Rust owns tracked failed-create allocations. The C adapter holds its stream lock
 * across lookup, heap cleanup, and the begin/restore/retire transaction. */
bool tsfp_audio_stream_lease_first_rollback_rust(uint32_t *stream_address,
    uint32_t *stream_heap);
bool tsfp_audio_stream_lease_overlaps_guest_span_rust(uint32_t address, uint32_t bytes);
bool tsfp_audio_stream_lease_overlaps_i3dl2_span_rust(uint32_t address, uint32_t bytes);
bool tsfp_audio_stream_lease_registered_rust(uint32_t stream_address, uint32_t stream_heap,
    const uint32_t header[10], const dsound_device_lease *lease);

#endif
