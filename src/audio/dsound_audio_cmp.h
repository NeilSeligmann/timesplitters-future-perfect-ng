/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T2046 audio COMPARE MODE (the audio twin of T2048's kmem compare). With -DTSFP_RUST_AUDIO_CMP=ON and the host started with
 * TSFP_RUST_AUDIO=compare, every implemented DirectSound HLE handler call is observed AFTER the C handler ran: the Rust
 * lifecycle/setters/frames of crates/tsfp-audio (through crates/tsfp-audio-capi) are driven with the same arguments over a
 * shadow world, and return codes, device reference word, the object header and the whole setter snapshot are compared with the
 * C state. The C always answers. Hooks of the completion model are recorded on the C side and replayed to the Rust side.
 * Option OFF (default) or variable unset: every function here is an identity/no-op and nothing is observed.
 * Design and measurements: docs/t-audio-replace.md, "Live compare mode".
 */
#ifndef TSFP_AUDIO_DSOUND_AUDIO_CMP_H
#define TSFP_AUDIO_DSOUND_AUDIO_CMP_H
#include <stdbool.h>
#include <stdint.h>
#include "dsound_buffer.h"
#include "dsound_stream.h"

typedef uint32_t (*dsound_audio_cmp_stream_fn)(uint32_t stream);
typedef uint32_t (*dsound_audio_cmp_status_fn)(uint32_t stream, uint32_t output);
typedef struct {
    bool streams_enabled, buffers_enabled, completion;
    bool frequency_hook, routing_hook, format_hook, movie_calls;
    dsound_buffer_irql_provider irql;
} dsound_audio_cmp_config;

#ifdef TSFP_RUST_AUDIO_CMP
bool dsound_audio_cmp_requested(void);
bool dsound_audio_cmp_answering(void); /* TSFP_RUST_AUDIO=1: mode 1 */
void dsound_audio_cmp_start(const dsound_audio_cmp_config *config);
dsound_stream_routing_note_fn dsound_audio_cmp_wrap_routing(dsound_stream_routing_note_fn real);
dsound_stream_frequency_note_fn dsound_audio_cmp_wrap_frequency(dsound_stream_frequency_note_fn real);
dsound_stream_control_word_fn dsound_audio_cmp_wrap_control_word(dsound_stream_control_word_fn real);
dsound_stream_format_note_fn dsound_audio_cmp_wrap_format(dsound_stream_format_note_fn real);
dsound_stream_pause_note_fn dsound_audio_cmp_wrap_stream_pause(dsound_stream_pause_note_fn real);
dsound_buffer_started_fn dsound_audio_cmp_wrap_started(dsound_buffer_started_fn real);
dsound_buffer_pause_fn dsound_audio_cmp_wrap_buffer_pause(dsound_buffer_pause_fn real);
dsound_buffer_frequency_fn dsound_audio_cmp_wrap_buffer_frequency(dsound_buffer_frequency_fn real);
dsound_buffer_voice_running_fn dsound_audio_cmp_wrap_voice_running(dsound_buffer_voice_running_fn real);
dsound_buffer_loop_fn dsound_audio_cmp_wrap_loop(dsound_buffer_loop_fn real);
dsound_audio_cmp_stream_fn dsound_audio_cmp_wrap_discontinuity(dsound_audio_cmp_stream_fn real);
dsound_audio_cmp_status_fn dsound_audio_cmp_wrap_startup_status(dsound_audio_cmp_status_fn real);
#define AUDCMP_DISCONTINUITY(f) dsound_audio_cmp_wrap_discontinuity(f)
#define AUDCMP_STARTUP_STATUS(f) dsound_audio_cmp_wrap_startup_status(f)
#define AUDCMP_ROUTING(f) dsound_audio_cmp_wrap_routing(f)
#define AUDCMP_FREQUENCY(f) dsound_audio_cmp_wrap_frequency(f)
#define AUDCMP_CONTROL_WORD(f) dsound_audio_cmp_wrap_control_word(f)
#define AUDCMP_FORMAT(f) dsound_audio_cmp_wrap_format(f)
#define AUDCMP_STREAM_PAUSE(f) dsound_audio_cmp_wrap_stream_pause(f)
#define AUDCMP_STARTED(f) dsound_audio_cmp_wrap_started(f)
#define AUDCMP_BUFFER_PAUSE(f) dsound_audio_cmp_wrap_buffer_pause(f)
#define AUDCMP_BUFFER_FREQUENCY(f) dsound_audio_cmp_wrap_buffer_frequency(f)
#define AUDCMP_VOICE_RUNNING(f) dsound_audio_cmp_wrap_voice_running(f)
#define AUDCMP_LOOP(f) dsound_audio_cmp_wrap_loop(f)
#else
static inline void dsound_audio_cmp_start(const dsound_audio_cmp_config *config) { (void)config; }
#define AUDCMP_DISCONTINUITY(f) (f)
#define AUDCMP_STARTUP_STATUS(f) (f)
#define AUDCMP_ROUTING(f) (f)
#define AUDCMP_FREQUENCY(f) (f)
#define AUDCMP_CONTROL_WORD(f) (f)
#define AUDCMP_FORMAT(f) (f)
#define AUDCMP_STREAM_PAUSE(f) (f)
#define AUDCMP_STARTED(f) (f)
#define AUDCMP_BUFFER_PAUSE(f) (f)
#define AUDCMP_BUFFER_FREQUENCY(f) (f)
#define AUDCMP_VOICE_RUNNING(f) (f)
#define AUDCMP_LOOP(f) (f)
#endif
#endif
