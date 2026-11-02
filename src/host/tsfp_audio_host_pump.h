#ifndef TSFP_AUDIO_HOST_PUMP_H
#define TSFP_AUDIO_HOST_PUMP_H
/* T3917: the audio runtime lifecycle and PCM pump the host drives (start, stop, the guest-thread DoWork route, the
 * 5 ms refill worker, its counters) behind one contract, so src/host/main.c holds no dsound_audio_runtime_* /
 * dsound_completion_* / dsound_movie_stream_* calls for it. The window and sink services stay in the host: it hands
 * the pump its clock and PCM writer in `tsfp_audio_host_pump_sink`.
 * With TSFP_RUST_AUDIO_HOST=ON the Rust crate tsfp-audio-host-wiring implements it and owns the state; with =OFF the C
 * unit src/host/tsfp_audio_host_pump.c does (the fallback, the pre-T3917 main.c code moved verbatim). */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "dsound_audio_runtime.h"

typedef struct tsfp_audio_host_pump_sink {
    dsound_audio_runtime_clock_fn clock;
    dsound_audio_runtime_sink_fn write;
    void *context;                  /* handed to clock and write */
    void (*register_direct)(void);  /* registers a direct Rust PCM target before pumping; NULL for none */
    bool direct_ring;               /* pump into the registered direct target, no C clock or writer */
} tsfp_audio_host_pump_sink;

bool tsfp_audio_host_runtime_start(uint32_t output_rate_hz, uint64_t ticks_per_second);
void tsfp_audio_host_runtime_stop(void);

/* DirectSoundDoWork route: completion model first, then the guest-thread render, then the movie stream route. */
bool tsfp_audio_host_work_route(uint32_t return_address, const tsfp_audio_host_pump_sink *sink);
/* Frames the guest-thread route rendered into the sink. */
uint64_t tsfp_audio_host_rendered_frames(void);
void tsfp_audio_host_work_lock(void);
void tsfp_audio_host_work_unlock(void);

/* The 5 ms refill worker. Start returns false when it could not start; stop is a no-op when it never started. */
bool tsfp_audio_host_pump_start(const tsfp_audio_host_pump_sink *sink);
void tsfp_audio_host_pump_stop(void);
/* Any pointer may be NULL. */
void tsfp_audio_host_pump_stats(uint64_t *frames, uint64_t *renders, uint64_t *failures);

/* Registers the direct Rust PCM target (Bevy's writer) the pump can render into. */
void tsfp_audio_host_set_direct_sink(dsound_audio_runtime_direct_sink_fn sink, void *context);

/* End of run: the completion capture finishes (`quiescent` is true when no guest thread is still running). */
void tsfp_audio_host_capture_finish(bool quiescent);

#endif
