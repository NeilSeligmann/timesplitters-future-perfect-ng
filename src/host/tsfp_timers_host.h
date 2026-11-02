/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_TIMERS_HOST_H
#define TSFP_TIMERS_HOST_H

#include <stdbool.h>
#include <stdint.h>

/* Rust host contract for reading and converting the selected virtual timer. */
uint64_t tsfp_timers_host_virtual_ticks(void);
uint64_t tsfp_timers_host_audio_clock(void *context);
uint64_t tsfp_timers_host_virtual_ns(void);
uint64_t tsfp_timers_host_monotonic_ns(void);
uint64_t tsfp_timers_host_ticks_to_ns(uint64_t ticks);
uint64_t tsfp_timers_host_frequency(void);
uint32_t tsfp_timers_host_tick_count_ms(void);
bool tsfp_timers_host_bind_tick_count(uint32_t address);
typedef void (*tsfp_timers_host_frame_hook)(void);
void tsfp_timers_host_set_frame_hook(tsfp_timers_host_frame_hook hook);
typedef struct tsfp_timers_host_clock_stats {
    uint64_t total_ticks, floor_ticks;
    uint64_t reads, reads_above_floor, frames;
    uint64_t stall_ticks, advance_to_ticks, advance_to_calls;
} tsfp_timers_host_clock_stats;
tsfp_timers_host_clock_stats tsfp_timers_host_get_stats(void);

#endif
