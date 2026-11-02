/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_AUDIO_DSOUND_AUDIO_RUNTIME_H
#define TSFP_AUDIO_DSOUND_AUDIO_RUNTIME_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "dsound_stream_routing.h"

bool dsound_audio_runtime_start(uint32_t output_rate_hz, uint64_t ticks_per_second);
void dsound_audio_runtime_stop(void);
void dsound_audio_runtime_reset_buffers(void);
bool dsound_audio_runtime_active(void);
uint64_t dsound_audio_runtime_clock_frequency(void);
bool dsound_audio_runtime_submit(uint32_t stream_key, uint64_t now_ticks, uint32_t source_rate_hz,
                                 const uint8_t *encoded, size_t encoded_bytes);
bool dsound_audio_runtime_submit_mono(uint32_t stream_key, uint64_t now_ticks, uint32_t source_rate_hz,
                                      const uint8_t *encoded, size_t encoded_bytes);
#ifdef TSFP_RUST_AUDIO_HOST
bool dsound_audio_runtime_submit_guest(uint32_t stream_key, uint64_t now_ticks,
                                       uint32_t source_rate_hz, int32_t host_pid,
                                       uint32_t guest_data, size_t encoded_bytes, bool mono);
bool dsound_audio_runtime_submit_guest_pcm16_stereo(uint32_t stream_key, uint64_t now_ticks,
                                                     uint32_t source_rate_hz, int32_t host_pid,
                                                     uint32_t guest_data, size_t pcm_bytes);
#endif
bool dsound_audio_runtime_submit_pcm16_stereo(uint32_t stream_key, uint64_t now_ticks,
                                              uint32_t source_rate_hz, const int16_t *pcm,
                                              size_t frames);
bool dsound_audio_runtime_cut_stream(uint32_t stream_key, uint64_t now_ticks);
bool dsound_audio_runtime_set_stream_running(uint32_t stream_key, uint64_t now_ticks,
                                             bool running);
bool dsound_audio_runtime_set_stream_frequency(uint32_t stream_key, uint64_t ticks,
                                               uint32_t source_rate_hz);
bool dsound_audio_runtime_frequency_stream(uint32_t key,uint64_t serial,uint64_t ticks,
    uint32_t base_hz,uint32_t old_hz,uint32_t new_hz);
bool dsound_audio_runtime_set_stream_volume(uint32_t stream_key, int32_t millibels);
bool dsound_audio_runtime_reset_stream_volume(uint32_t stream_key);
bool dsound_audio_runtime_route_stream(uint32_t key, uint64_t serial, uint64_t ticks,
                                       const dsound_stream_routing *routing);
/* Rust-host path samples the selected kernel virtual clock inside the Rust runtime. */
bool dsound_audio_runtime_route_stream_now(uint32_t key, uint64_t serial,
                                          const dsound_stream_routing *routing);
bool dsound_audio_runtime_route_stream_admitted_now(uint32_t key, uint64_t serial,
                                                    bool completion_enabled,
                                                    uint64_t completion_clock_frequency,
                                                    const dsound_stream_routing *routing);
#ifdef TSFP_RUST_AUDIO_HOST
/* Rust-host completion policy and callback; C configures the gate but is not called per route. */
void dsound_audio_runtime_set_completion_route_policy_rust(bool enabled,
                                                            uint64_t clock_frequency);
bool dsound_audio_runtime_route_stream_completion_rust(uint32_t key, uint64_t serial,
                                                        const dsound_stream_routing *routing);
#endif
bool dsound_audio_runtime_render(uint64_t now_ticks, int16_t *output, size_t frame_capacity,
                                 size_t *frames_written);

/* T2046: the host-side PCM pump loop is owned by the selected runtime backend. Clock is sampled
 * before each render chunk and sink_write runs synchronously after each non-empty chunk. The
 * callbacks must not retain their context or PCM pointer. `frames_written` may be NULL. */
typedef uint64_t (*dsound_audio_runtime_clock_fn)(void *context);
typedef bool (*dsound_audio_runtime_sink_fn)(void *context, const int16_t *stereo,
                                             size_t frames);
/* Optional direct Rust sink. Return 1 when handled, 0 to use the legacy sink callback,
 * or a negative value on failure. PCM is valid only for the synchronous call. */
typedef int (*dsound_audio_runtime_direct_sink_fn)(void *context, const int16_t *stereo,
                                                   size_t frames, uint64_t end_vt_ticks,
                                                   uint64_t ticks_per_second);
void dsound_audio_runtime_set_direct_sink(dsound_audio_runtime_direct_sink_fn sink,
                                          void *context);
bool dsound_audio_runtime_pump(dsound_audio_runtime_clock_fn clock,
                               dsound_audio_runtime_sink_fn sink, void *context,
                               uint64_t *frames_written);
/* Rust Bevy direct-ring mode supplies no C clock or PCM sink callback. Rust samples the shared
 * kernel virtual clock and each rendered block must be accepted by its registered Rust sink. */
bool dsound_audio_runtime_pump_direct(uint64_t *frames_written);
/* Rust-owned guest DoWork pump/target selection; callbacks are legacy-only. */
bool dsound_audio_runtime_guest_work(dsound_audio_runtime_clock_fn clock,
                                     dsound_audio_runtime_sink_fn sink, void *context,
                                     uint64_t *frames_written);

/* T2046: Rust owns the opt-in 5 ms refill worker, its counters, and the shared audio-work lock.
 * Clock and sink callbacks/context remain live until stop joins. */
bool dsound_audio_runtime_start_pump(dsound_audio_runtime_clock_fn clock,
                                     dsound_audio_runtime_sink_fn sink, void *context);
/* Start the same Rust worker with the direct-only pump contract above. */
bool dsound_audio_runtime_start_pump_direct(void);
/* Rust-owned work lock shared by the refill worker and synchronous guest-thread rendering. */
void dsound_audio_runtime_work_lock(void);
void dsound_audio_runtime_work_unlock(void);
void dsound_audio_runtime_stop_pump(void);
void dsound_audio_runtime_pump_stats(uint64_t *frames_written, uint64_t *renders,
                                     uint64_t *failures);

#define DSOUND_AUDIO_BUFFER_MAX_BYTES (36u * 32768u)
/* Original-backed ordinary buffer transport with real mono ADPCM samples;
 * verified buffer lease serial distinguishes recycled guest addresses.
 * Timing/resampling remains INFERRED under the existing opt-in audio runtime. */
bool dsound_audio_runtime_play_buffer(uint32_t key, uint64_t serial, uint64_t ticks,
                                      uint32_t frequency, const uint8_t *encoded, size_t bytes,
                                      bool paused, bool looping, size_t loop_start, size_t loop_end,
                                      int32_t volume);
#ifdef TSFP_RUST_AUDIO_HOST
bool dsound_audio_runtime_play_buffer_guest(uint32_t key, uint64_t serial, uint64_t ticks,
                                            uint32_t frequency, int32_t host_pid,
                                            uint32_t guest_data, size_t bytes, bool paused,
                                            bool looping, size_t loop_start, size_t loop_end,
                                            int32_t volume);
#endif
bool dsound_audio_runtime_pause_buffer(uint32_t key, uint64_t serial, uint64_t ticks, bool paused);
bool dsound_audio_runtime_stop_buffer(uint32_t key, uint64_t serial, uint64_t ticks);
/* T1216: the mixer's reason for its last refused buffer command (a static string, for the stop text). */
const char *dsound_audio_runtime_last_buffer_refusal(void);
bool dsound_audio_runtime_frequency_buffer(uint32_t key, uint64_t serial, uint64_t ticks,
                                           uint32_t frequency);
bool dsound_audio_runtime_volume_buffer(uint32_t key, uint64_t serial, uint64_t ticks, int32_t volume);
/* T1524: live loop region of a running buffer voice, source sample frames. */
bool dsound_audio_runtime_loop_buffer(uint32_t key, uint64_t serial, uint64_t ticks,
                                      size_t loop_start, size_t loop_end);

bool dsound_audio_runtime_enable_mixbin_headroom(bool enabled);
bool dsound_audio_runtime_mixbin_headroom(uint64_t identity, uint64_t ticks,
                                          uint32_t bin, uint32_t headroom);
bool dsound_audio_runtime_bind_mixbin_headroom(uint64_t identity);
/* T1245: verified lease; abort pending packets at the format event. */
bool dsound_audio_runtime_format_stream(uint32_t key,uint64_t serial,uint64_t ticks,
    uint32_t old_hz,uint32_t new_hz);
/* T2046: observation hook of the completion model (TSFP_AUDIO_CMD_LOG); no-ops without the variable. */
void dsound_audio_cmdlog_line(const char *line);
bool dsound_audio_cmdlog_enabled(void);
#endif
