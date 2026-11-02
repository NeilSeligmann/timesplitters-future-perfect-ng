/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T2487: virtual guest time base for fast-forward replays. See kernel_vtime.h.
 */
#include "kernel_vtime.h"

#include "kernel_detsched.h"

#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static pthread_mutex_t vtime_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t vtime_cond;
static pthread_once_t vtime_once = PTHREAD_ONCE_INIT;
static int enabled = -1;
static bool started;
static uint64_t start_ns;
static uint64_t now_ns;
static uint64_t generation; /* bumped by every forward move */
static uint64_t pending[KERNEL_VTIME_MAX_WAITERS]; /* 0 = free slot */
static kernel_vtime_stats stats;
/* T3393: while the host holds the guest (the replay input lockstep waits for the Bevy shell), the
 * guest is not idle, it is waiting on the HOST. Idle jumps then would move virtual time by an amount
 * that depends on how fast the host runs (MEASURED story-level1-20261018: the level load ended at
 * pad poll 7920 instead of 7898 once the guest also waited for the native UI each tick). */
static void vtime_init(void);
/* T3393 diagnostic: TSFP_VTIME_IDLE_US overrides the idle period (default KERNEL_VTIME_IDLE_NS). */
static uint64_t idle_ns(void)
{
    static uint64_t value;
    if (value == 0u) {
        const char *text = getenv("TSFP_VTIME_IDLE_US");
        const unsigned long long us = text != NULL ? strtoull(text, NULL, 10) : 0ull;
        value = us >= 1ull && us <= 900000ull ? (uint64_t)us * 1000ull : KERNEL_VTIME_IDLE_NS;
    }
    return value;
}
static int host_holds;

void kernel_vtime_host_hold(int on)
{
    pthread_once(&vtime_once, vtime_init);
    pthread_mutex_lock(&vtime_lock);
    host_holds += on ? 1 : -1;
    if (host_holds < 0) host_holds = 0;
    pthread_mutex_unlock(&vtime_lock);
}

static uint64_t real_monotonic_ns(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (uint64_t)now.tv_sec * 1000000000ull + (uint64_t)now.tv_nsec;
}

static void vtime_init(void)
{
    /* The waits use CLOCK_MONOTONIC wall timeouts only to detect idleness. */
    pthread_condattr_t attr;
    pthread_condattr_init(&attr);
    pthread_condattr_setclock(&attr, CLOCK_MONOTONIC);
    pthread_cond_init(&vtime_cond, &attr);
    pthread_condattr_destroy(&attr);
}

bool kernel_vtime_enabled(void)
{
    if (enabled < 0) {
        const char *value = getenv("TSFP_FAST_FORWARD");
        const int on = value != NULL && value[0] != '\0' && strcmp(value, "0") != 0;
        if (on) fprintf(stderr, "kernel_vtime   T2487 fast-forward: virtual guest time base, timed waits on virtual time (TSFP_FAST_FORWARD)\n");
        enabled = on;
    }
    return enabled == 1;
}

/* Under vtime_lock. */
static void start_locked(void)
{
    if (!started) {
        started = true;
        start_ns = real_monotonic_ns();
        now_ns = start_ns;
    }
}

uint64_t kernel_vtime_now_ns(void)
{
    pthread_once(&vtime_once, vtime_init);
    pthread_mutex_lock(&vtime_lock);
    start_locked();
    const uint64_t value = now_ns;
    pthread_mutex_unlock(&vtime_lock);
    return value;
}

uint64_t kernel_vtime_elapsed_ns(void)
{
    pthread_once(&vtime_once, vtime_init);
    pthread_mutex_lock(&vtime_lock);
    start_locked();
    const uint64_t value = now_ns - start_ns;
    pthread_mutex_unlock(&vtime_lock);
    return value;
}

/* Under vtime_lock. */
static void move_locked(uint64_t ns)
{
    if (ns > now_ns) {
        now_ns = ns;
        generation++;
        pthread_cond_broadcast(&vtime_cond);
    }
}

void kernel_vtime_advance_to_ns(uint64_t ns)
{
    pthread_once(&vtime_once, vtime_init);
    pthread_mutex_lock(&vtime_lock);
    start_locked();
    if (ns > now_ns) stats.advances++;
    move_locked(ns);
    pthread_mutex_unlock(&vtime_lock);
}

/* Under vtime_lock: the earliest pending deadline, or 0 when none. */
static uint64_t earliest_locked(void)
{
    uint64_t best = 0u;
    for (unsigned i = 0u; i < KERNEL_VTIME_MAX_WAITERS; i++) {
        if (pending[i] != 0u && (best == 0u || pending[i] < best)) best = pending[i];
    }
    return best;
}

int kernel_vtime_sleep_until_ns(uint64_t deadline_ns)
{
    pthread_once(&vtime_once, vtime_init);
    pthread_mutex_lock(&vtime_lock);
    start_locked();
    stats.sleeps++;
    if (deadline_ns <= now_ns) {
        pthread_mutex_unlock(&vtime_lock);
        return 0;
    }
    unsigned slot = KERNEL_VTIME_MAX_WAITERS;
    for (unsigned i = 0u; i < KERNEL_VTIME_MAX_WAITERS; i++) {
        if (pending[i] == 0u) {
            slot = i;
            pending[i] = deadline_ns;
            break;
        }
    }
    if (slot == KERNEL_VTIME_MAX_WAITERS) {
        pthread_mutex_unlock(&vtime_lock);
        return ENOMEM;
    }
    /* T3442: a scheduled guest thread re-tests on each baton visit; the scheduler jumps virtual time
     * when every thread is blocked, so no wall-clock idle detection decides when time moves. */
    while (now_ns < deadline_ns && kernel_detsched_participant()) {
        pthread_mutex_unlock(&vtime_lock);
        kernel_detsched_block();
        pthread_mutex_lock(&vtime_lock);
    }
    while (now_ns < deadline_ns) {
        const uint64_t seen = generation;
        struct timespec until;
        clock_gettime(CLOCK_MONOTONIC, &until);
        until.tv_nsec += (long)idle_ns();
        if (until.tv_nsec >= 1000000000L) {
            until.tv_nsec -= 1000000000L;
            until.tv_sec++;
        }
        const int error = pthread_cond_timedwait(&vtime_cond, &vtime_lock, &until);
        if (error == ETIMEDOUT && generation == seen && now_ns < deadline_ns && host_holds > 0) {
            stats.held_jumps_refused++;
        } else if (error == ETIMEDOUT && generation == seen && now_ns < deadline_ns) {
            /* Nothing advanced virtual time for a whole idle period: every advancer waits too.
             * Jump to the earliest pending deadline (possibly another waiter's). */
            const uint64_t earliest = earliest_locked();
            if (earliest > now_ns) {
                stats.idle_jumps++;
                move_locked(earliest);
            }
        } else if (error != 0 && error != ETIMEDOUT && error != EINTR) {
            pending[slot] = 0u;
            pthread_mutex_unlock(&vtime_lock);
            return error;
        }
    }
    pending[slot] = 0u;
    pthread_mutex_unlock(&vtime_lock);
    return 0;
}

bool kernel_vtime_jump_to_earliest(void)
{
    pthread_once(&vtime_once, vtime_init);
    pthread_mutex_lock(&vtime_lock);
    start_locked();
    const uint64_t earliest = earliest_locked();
    const bool jump = host_holds == 0 && earliest > now_ns;
    if (jump) {
        stats.idle_jumps++;
        move_locked(earliest);
    }
    pthread_mutex_unlock(&vtime_lock);
    return jump;
}

int kernel_vtime_sleep_ns(uint64_t duration_ns)
{
    const uint64_t now = kernel_vtime_now_ns();
    return kernel_vtime_sleep_until_ns(duration_ns > UINT64_MAX - now ? UINT64_MAX : now + duration_ns);
}

kernel_vtime_stats kernel_vtime_get_stats(void)
{
    pthread_mutex_lock(&vtime_lock);
    const kernel_vtime_stats copy = stats;
    pthread_mutex_unlock(&vtime_lock);
    return copy;
}

void kernel_vtime_test_reset(bool on, uint64_t start)
{
    pthread_once(&vtime_once, vtime_init);
    pthread_mutex_lock(&vtime_lock);
    enabled = on ? 1 : 0;
    started = true;
    start_ns = start;
    now_ns = start;
    generation = 0u;
    memset(pending, 0, sizeof pending);
    memset(&stats, 0, sizeof stats);
    pthread_mutex_unlock(&vtime_lock);
}
