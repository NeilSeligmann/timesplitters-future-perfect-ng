/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T2487: the virtual guest time base for fast-forward replays (TSFP_FAST_FORWARD=1).
 *
 * The guest TSC (kernel_clock.c), kernel timers, DPCs and event timeouts already run on the
 * deterministic virtual counter. What still paced a replay on the wall clock were the host
 * deadlines behind KeDelayExecutionThread and the current-thread pseudo pauses, the fixed-tick
 * sleep of the simulation loop and the system time. Under fast-forward all of them read and wait
 * on ONE virtual monotonic nanosecond clock:
 *
 * - the simulation tick advances it to the tick's deadline (kernel_vtime_advance_to_ns), so a
 *   tick takes exactly one virtual period and no wall time is slept;
 * - a timed wait blocks until virtual time reaches its deadline, never on a wall deadline;
 * - when no advance happens for KERNEL_VTIME_IDLE_NS of wall time (every guest thread that could
 *   advance it is itself waiting), virtual time jumps to the earliest pending deadline, so no
 *   thread spins or hangs forever.
 *
 * Unset (the default) nothing here is consulted and the old wall-clock behaviour is unchanged.
 */
#ifndef TSFP_KERNEL_VTIME_H
#define TSFP_KERNEL_VTIME_H

#include <stdbool.h>
#include <stdint.h>

/* Wall nanoseconds without any advance before the earliest pending deadline is taken. */
#define KERNEL_VTIME_IDLE_NS 1000000ull
/* Pending timed waits tracked for the earliest-deadline jump (above the guest thread limit). */
#define KERNEL_VTIME_MAX_WAITERS 256u

/* TSFP_FAST_FORWARD set and not "0", read once. */
bool kernel_vtime_enabled(void);
/* Virtual CLOCK_MONOTONIC in nanoseconds. Starts at the real monotonic time of the first use. */
uint64_t kernel_vtime_now_ns(void);
/* Move virtual time forward to `ns` (never back) and wake the waiters that are due. */
void kernel_vtime_advance_to_ns(uint64_t ns);
/* Block until virtual time reaches `deadline_ns`. Returns 0, or an errno-style code. */
int kernel_vtime_sleep_until_ns(uint64_t deadline_ns);
/* Block for `duration_ns` of virtual time from now. */
int kernel_vtime_sleep_ns(uint64_t duration_ns);
/* T3393: the host holds the guest (1) or releases it (0); no idle jump while held. Nests. */
void kernel_vtime_host_hold(int on);
/* T3442: jump to the earliest pending timed wait now (every guest thread blocked, decided by the
 * deterministic scheduler). False when nothing is pending or the host holds the guest. */
bool kernel_vtime_jump_to_earliest(void);
/* Virtual nanoseconds since the first use, for the virtual system time. */
uint64_t kernel_vtime_elapsed_ns(void);

typedef struct kernel_vtime_stats {
    uint64_t advances;     /* forward moves by the simulation tick */
    uint64_t idle_jumps;   /* earliest-deadline jumps while every advancer waited */
    uint64_t sleeps;       /* timed waits served on virtual time */
    uint64_t held_jumps_refused; /* idle jumps refused while the host held virtual time */
} kernel_vtime_stats;
kernel_vtime_stats kernel_vtime_get_stats(void);

/* Tests only: force the mode and the start time. */
void kernel_vtime_test_reset(bool enabled, uint64_t start_ns);

#endif
