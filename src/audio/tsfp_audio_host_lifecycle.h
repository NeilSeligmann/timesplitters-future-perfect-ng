#ifndef TSFP_AUDIO_HOST_LIFECYCLE_H
#define TSFP_AUDIO_HOST_LIFECYCLE_H
/* T3913: the host-side reset, registration and teardown order of the DirectSound HLE modules, in ONE entry
 * point set so a host (src/host/main.c, src/xbox/xdk_report.c) calls these instead of every module. Same
 * calls, same order as before the move; the Rust audio host replaces this unit at retirement (T3879). */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Fresh run state: dsound_hle_init and every module reset. */
void tsfp_audio_host_reset_all(void);
/* The registration subset a static report needs (movie device rows included). */
void tsfp_audio_host_register_report_surface(void);
/* End of run: listener, buffer, stream, effects binding, device. Returns the checked-reset results. */
void tsfp_audio_host_teardown(bool *buffers_cleaned, bool *streams_cleaned);


/* T3917: the measured DirectSound surface as the XDK dispatcher and the registration report ask about it, by exact
 * address. Known: the address is a measured DSOUND entry. Implemented: a handler is registered for it. Consistent:
 * implemented exactly when a handler is held. Requires implementation: the measured stream, buffer and listener startup
 * surface, where a stub must stop the run instead of answering. */
bool tsfp_audio_host_hle_known(uint32_t address);
bool tsfp_audio_host_hle_implemented(uint32_t address);
bool tsfp_audio_host_hle_registration_consistent(uint32_t address);
bool tsfp_audio_host_hle_requires_implementation(uint32_t address);

/* T3918: the per-call dispatch of a measured DirectSound entry. Lookup, call counts, the once-per-function stub report
 * and its label are owned behind this contract (Rust with the Rust audio host, the C entry table otherwise). Returns the
 * guest result; `context` is the live kernel call frame. The call count of an entry feeds the backlog report. */
uint32_t tsfp_audio_host_hle_call(uint32_t address, void *context);
uint64_t tsfp_audio_host_hle_call_count(uint32_t address);

#endif
