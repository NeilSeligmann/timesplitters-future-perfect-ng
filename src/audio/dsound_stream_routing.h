/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_AUDIO_DSOUND_STREAM_ROUTING_H
#define TSFP_AUDIO_DSOUND_STREAM_ROUTING_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

/* Original settings +20 headroom, +1C aggregate volume, +24 route count,
 * +28 ordered byte bins, +30 32 DWORD bin volumes. Stereo voice grouping is
 * floor(route_count/2). Checked adapter scope: <=8 routes, speaker bins0..5;
 * DSP/HRTF bins are not silently redirected. All arithmetic is modulo32. */
#define DSOUND_STREAM_ROUTE_SLOTS 8u
#define DSOUND_STREAM_ROUTE_BINS 32u

typedef struct dsound_stream_routing {
    uint32_t volume, headroom, aggregate, count;
    uint8_t bins[DSOUND_STREAM_ROUTE_SLOTS];
    uint32_t bin_volume[DSOUND_STREAM_ROUTE_BINS];
} dsound_stream_routing;

typedef struct dsound_stream_route_pair {
    uint32_t bin, volume;
} dsound_stream_route_pair;

typedef struct dsound_stream_guest_span {
    uint32_t address, bytes;
} dsound_stream_guest_span;

/* Rust frequency sidecar lifecycle is shared by both host variants. */
bool tsfp_audio_stream_routing_prepare_rust(uint32_t stream);
bool tsfp_audio_stream_routing_bind_rust(uint32_t stream, uint64_t serial);
bool tsfp_audio_stream_routing_cancel_rust(uint32_t stream);
bool tsfp_audio_stream_routing_retire_rust(uint32_t stream, uint64_t serial);

#if defined(TSFP_RUST_AUDIO_HOST)
/* Rust owns routing policy on the Rust-host path; the C host below remains the oracle. */
_Static_assert(sizeof(dsound_stream_routing) == 152u, "routing FFI size drift");
_Static_assert(offsetof(dsound_stream_routing, bins) == 16u, "routing FFI bins offset drift");
_Static_assert(offsetof(dsound_stream_routing, bin_volume) == 24u, "routing FFI volumes offset drift");
_Static_assert(sizeof(dsound_stream_route_pair) == 8u, "routing pair FFI size drift");
_Static_assert(sizeof(dsound_stream_guest_span) == 8u, "guest span FFI size drift");
/* Lifecycle membership and stream guest spans come from the Rust lease registry. */
bool tsfp_audio_stream_lease_guest_spans_rust(dsound_stream_guest_span *spans,
    uint32_t capacity, uint32_t *count_out);
void tsfp_audio_stream_routing_init_rust(dsound_stream_routing *state);
void tsfp_audio_stream_routing_volume_rust(dsound_stream_routing *state, uint32_t volume);
void tsfp_audio_stream_routing_headroom_rust(dsound_stream_routing *state, uint32_t headroom);
bool tsfp_audio_stream_routing_load_rust(uint32_t stream, uint64_t serial,
                                         dsound_stream_routing *output);
bool tsfp_audio_stream_routing_bins_rust(dsound_stream_routing *state,
                                         const dsound_stream_route_pair *pairs,
                                         uint32_t count, bool defaults);
bool tsfp_audio_stream_routing_volumes_rust(dsound_stream_routing *state,
                                            const dsound_stream_route_pair *pairs,
                                            uint32_t count);
uint32_t tsfp_audio_stream_routing_guest_descriptor_rust(
    int32_t host_pid, dsound_stream_routing *state, uint32_t descriptor_address,
    uint32_t maximum, bool volumes, uint32_t context,
    bool (*spans_snapshot)(uint32_t context, dsound_stream_guest_span *spans,
                           uint32_t capacity, uint32_t *count_out));
bool tsfp_audio_guest_span_aliases_rust(uint32_t address, uint32_t bytes,
    const dsound_stream_guest_span *protected_spans, uint32_t protected_count);
uint16_t tsfp_audio_stream_routing_attenuation_rust(const dsound_stream_routing *state,
                                                    uint32_t slot);
typedef bool (*dsound_stream_routing_apply_fn)(void *context, uint32_t stream,
    uint64_t serial, const dsound_stream_routing *candidate);
bool tsfp_audio_stream_routing_apply_note_rust(void *context, uint32_t stream,
    uint64_t serial, const dsound_stream_routing *candidate);
typedef bool (*dsound_stream_guest_spans_snapshot_fn)(uint32_t context,
    dsound_stream_guest_span *spans, uint32_t capacity, uint32_t *count_out);
enum {
    DSOUND_STREAM_ROUTING_UPDATE_HEADROOM = 0u,
    DSOUND_STREAM_ROUTING_UPDATE_BINS = 1u,
    DSOUND_STREAM_ROUTING_UPDATE_VOLUMES = 2u
};
/* Rust plans descriptor/headroom changes, applies the synchronous PCM callback, then publishes. */
uint32_t tsfp_audio_stream_routing_update_rust(uint32_t stream, uint64_t serial,
    uint32_t operation, uint32_t argument, int32_t host_pid, uint32_t descriptor_address,
    uint32_t internal, void *context, dsound_stream_routing_apply_fn apply,
    dsound_stream_guest_spans_snapshot_fn spans_snapshot);
/* Rust owns candidate/apply/sidecar publication order on the Rust-host route. The callback is
 * the synchronous host clock/completion boundary and must not re-enter the stream adapter. */
bool tsfp_audio_stream_routing_commit_rust(const dsound_stream_routing *candidate,
    uint32_t stream, uint64_t serial,
    void *context, dsound_stream_routing_apply_fn apply);
bool tsfp_audio_stream_routing_volume_commit_rust(uint32_t stream, uint64_t serial,
    uint32_t volume, void *context, dsound_stream_routing_apply_fn apply);

static inline void dsound_stream_routing_init(dsound_stream_routing *state)
{ tsfp_audio_stream_routing_init_rust(state); }
static inline void dsound_stream_routing_volume(dsound_stream_routing *state, uint32_t volume)
{ tsfp_audio_stream_routing_volume_rust(state, volume); }
static inline void dsound_stream_routing_headroom(dsound_stream_routing *state, uint32_t headroom)
{ tsfp_audio_stream_routing_headroom_rust(state, headroom); }
static inline bool dsound_stream_routing_bins(dsound_stream_routing *state,
                                               const dsound_stream_route_pair *pairs,
                                               uint32_t count, bool defaults)
{ return tsfp_audio_stream_routing_bins_rust(state, pairs, count, defaults); }
static inline bool dsound_stream_routing_volumes(dsound_stream_routing *state,
                                                  const dsound_stream_route_pair *pairs,
                                                  uint32_t count)
{ return tsfp_audio_stream_routing_volumes_rust(state, pairs, count); }
static inline uint16_t dsound_stream_routing_attenuation(const dsound_stream_routing *state,
                                                         uint32_t slot)
{ return tsfp_audio_stream_routing_attenuation_rust(state, slot); }
#else
static inline void dsound_stream_routing_init(dsound_stream_routing *state)
{
    memset(state, 0, sizeof(*state));
    state->headroom = 600u;
    state->aggregate = 0u - state->headroom;
    state->count = 2u;
    state->bins[1] = 1u;
}

static inline void dsound_stream_routing_volume(dsound_stream_routing *state,
                                                uint32_t volume)
{
    state->aggregate += volume - state->volume;
    state->volume = volume;
}

static inline void dsound_stream_routing_headroom(dsound_stream_routing *state,
                                                  uint32_t headroom)
{
    state->aggregate += state->headroom - headroom;
    state->headroom = headroom;
}

/* SetMixBins uses a BYTE bin identity in both route and indexed volume write.
 * Duplicate bins keep every route slot but share the last-written volume.
 * Empty input leaves earlier volume entries untouched. NULL restores stereo
 * defaults0/1, including their volumes0, and preserves other volume entries. */
static inline bool dsound_stream_routing_bins(dsound_stream_routing *state,
                                               const dsound_stream_route_pair *pairs,
                                               uint32_t count, bool defaults)
{
    const dsound_stream_route_pair normal[2] = {{0u, 0u}, {1u, 0u}};
    if (defaults) { pairs = normal; count = 2u; }
    if (count > DSOUND_STREAM_ROUTE_SLOTS || (count != 0u && pairs == NULL)) return false;
    for (uint32_t i = 0u; i < count; i++)
        if ((uint8_t)pairs[i].bin > 5u) return false;
    dsound_stream_routing candidate = *state;
    for (uint32_t i = 0u; i < count; i++) {
        const uint8_t bin = (uint8_t)pairs[i].bin;
        candidate.bins[i] = bin;
        candidate.bin_volume[bin] = pairs[i].volume;
    }
    candidate.count = count;
    *state = candidate;
    return true;
}

/* SetMixBinVolumes uses the full DWORD bin, unlike SetMixBins. */
static inline bool dsound_stream_routing_volumes(dsound_stream_routing *state,
                                                  const dsound_stream_route_pair *pairs,
                                                  uint32_t count)
{
    if (count > DSOUND_STREAM_ROUTE_BINS || (count != 0u && pairs == NULL)) return false;
    for (uint32_t i = 0u; i < count; i++)
        if (pairs[i].bin >= DSOUND_STREAM_ROUTE_BINS) return false;
    dsound_stream_routing candidate = *state;
    for (uint32_t i = 0u; i < count; i++) candidate.bin_volume[pairs[i].bin] = pairs[i].volume;
    *state = candidate;
    return true;
}

/* Original40CBC0: NEG, SHL6 (32-bit wrap), unsigned DIV100, cap0FFF.
 * xemu vp/vp.c volume0FFF means silence, not the finite63.984dB gain. */
static inline uint16_t dsound_stream_routing_attenuation(const dsound_stream_routing *state,
                                                         uint32_t slot)
{
    if (slot >= state->count || slot >= DSOUND_STREAM_ROUTE_SLOTS ||
        state->bins[slot] >= DSOUND_STREAM_ROUTE_BINS) return 4095u;
    uint32_t value = 0u - (state->aggregate + state->bin_volume[state->bins[slot]]);
    value = (value << 6u) / 100u;
    return (uint16_t)(value > 4095u ? 4095u : value);
}
#endif
#endif
