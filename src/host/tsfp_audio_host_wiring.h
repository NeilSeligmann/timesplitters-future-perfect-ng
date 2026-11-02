#ifndef TSFP_AUDIO_HOST_WIRING_H
#define TSFP_AUDIO_HOST_WIRING_H
/* T3913: the DirectSound HLE host wiring (fatal/IRQL providers, passive completion, movie stream, thunk
 * routes, compare hook) behind two phase entry points plus report helpers, so src/host/main.c holds no
 * per-module dsound_* calls for it. Calls and their order are those main.c made inline; both phases return
 * true, or false after printing the refusal to stderr (exit code 2). Replaced by the Rust audio host at
 * retirement (T3879); TSFP_RUST_AUDIO_HOST=0 keeps the C paths selected inside. */
#include <stdbool.h>
#include <stdint.h>
#include "dsound_hle.h"
#include "dsound_listener.h"
#include "dsound_movie_stream.h"

typedef struct tsfp_audio_host_config {
    bool headless_effects, gp_effects, headless_streams, headless_buffers, headless_listener;
    bool headless_movie_audio, passive_audio_completion, passive_audio_completion_at_dowork;
    void (*fatal)(uint32_t address, const char *reason);
    bool (*irql)(uint8_t *level);
    bool (*control_word)(uint16_t *word);
    dsound_movie_stream_callback_fn movie_callback;
    dsound_listener_work_route_fn work_route;
} tsfp_audio_host_config;

/* Effects, stream, buffer, listener, mix-bin headroom, device/HRTF/movie registration. */
bool tsfp_audio_host_configure_early(const tsfp_audio_host_config *config);
/* Completion model, thunk method routes, movie device calls, compare hook, buffer/listener registration. */
bool tsfp_audio_host_configure_late(const tsfp_audio_host_config *config);
/* Startup policy lines for the movie, effects and handler count. */
void tsfp_audio_host_print_policy(const tsfp_audio_host_config *config);
/* End-of-run report blocks. */
void tsfp_audio_host_print_completion_report(void);
void tsfp_audio_host_print_movie_report(const tsfp_audio_host_config *config);


/* T3917: single option-handling calls the host used to make straight into the modules. */
/* True when every compiled buffer lifetime stop is present (--headless-buffers needs them). */
bool tsfp_audio_host_buffer_stops_ready(void);
void tsfp_audio_host_set_movie_trace(bool enabled);
/* Diagnostics sink of the HLE (the host points it at kernel_hle_log()). */
void tsfp_audio_host_set_hle_log(dsound_log_fn printer);
/* --ac97-ready: announce the modelled codec as ready (FABRICATED banner). */
void tsfp_audio_host_set_codec_ready(void);

#endif
