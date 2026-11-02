/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3442: deterministic guest thread scheduling (one baton). See kernel_detsched.h.
 */
#include "kernel_detsched.h"

#include <errno.h>
#include <execinfo.h>
#include <signal.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "kernel_sync.h"
#include "kernel_vtime.h"

#define NO_HOLDER (-1)

typedef enum { P_FREE = 0, P_PENDING, P_ACTIVE } participant_state;

typedef struct {
    participant_state state;
    uint64_t tested_progress; /* `progress` when it last blocked (re-tested and still waiting) */
    bool has_tested;
    pthread_cond_t cond;
    pthread_t thread;
} participant;

static pthread_mutex_t ds_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_once_t ds_once = PTHREAD_ONCE_INIT;
static participant parts[KERNEL_DETSCHED_MAX];
static int holder = NO_HOLDER;
static int enabled_state = -1;
static unsigned quantum = KERNEL_DETSCHED_DEFAULT_QUANTUM;
static unsigned watchdog_ms = KERNEL_DETSCHED_DEFAULT_WATCHDOG_MS;
/* Guest progress: own safepoints and HLE dispatches of the holder. Written only by the holder. */
static _Atomic uint64_t progress;
static uint64_t idle_since_ms; /* wall time the current all-blocked streak began, 0 = none */
static kernel_detsched_stats stats;
static void (*stop_sink)(const char *detail);

static _Thread_local int self_id = NO_HOLDER;
static _Thread_local unsigned own_safepoints;

/* Diagnostic for the stuck-holder stop: the holder prints its own host backtrace. */
static void backtrace_handler(int signo)
{
    (void)signo;
    void *frames[64];
    static const char head[] = "detsched       T3442 stuck holder host backtrace:\n";
    (void)!write(2, head, sizeof head - 1u);
    backtrace_symbols_fd(frames, backtrace(frames, 64), 2);
}

static void ds_init(void)
{
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = backtrace_handler;
    sigemptyset(&action.sa_mask);
    action.sa_flags = SA_RESTART;
    (void)sigaction(SIGRTMIN + 7, &action, NULL);
    pthread_condattr_t attr;
    pthread_condattr_init(&attr);
    pthread_condattr_setclock(&attr, CLOCK_MONOTONIC);
    for (unsigned i = 0u; i < KERNEL_DETSCHED_MAX; i++) pthread_cond_init(&parts[i].cond, &attr);
    pthread_condattr_destroy(&attr);
}

static uint64_t wall_ms(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (uint64_t)now.tv_sec * 1000u + (uint64_t)now.tv_nsec / 1000000u;
}

static unsigned env_unsigned(const char *name, unsigned fallback, unsigned low, unsigned high)
{
    const char *text = getenv(name);
    if (text == NULL || text[0] == '\0') return fallback;
    const unsigned long value = strtoul(text, NULL, 10);
    return value >= low && value <= high ? (unsigned)value : fallback;
}

bool kernel_detsched_enabled(void)
{
    if (enabled_state < 0) {
        const char *value = getenv("TSFP_DETSCHED");
        const bool asked = value != NULL && value[0] != '\0' && strcmp(value, "0") != 0;
        int on = 0;
        if (asked && !kernel_vtime_enabled()) {
            fprintf(stderr, "detsched       T3442 TSFP_DETSCHED ignored: it needs the virtual time base (TSFP_FAST_FORWARD=1)\n");
        } else if (asked) {
            quantum = env_unsigned("TSFP_DETSCHED_QUANTUM", KERNEL_DETSCHED_DEFAULT_QUANTUM, 16u, 1u << 24);
            watchdog_ms = env_unsigned("TSFP_DETSCHED_WATCHDOG_MS", KERNEL_DETSCHED_DEFAULT_WATCHDOG_MS, 1000u, 3600000u);
            fprintf(stderr, "detsched       T3442 deterministic guest scheduling ON (TSFP_DETSCHED): one runnable guest thread, "
                            "round robin hand-off at HLE waits, yields, thread start/exit and every %u own safepoints at PASSIVE "
                            "(INFERRED policy, FABRICATED quantum); all-blocked jumps virtual time; watchdog %u ms\n",
                    quantum, watchdog_ms);
            on = 1;
        }
        enabled_state = on;
    }
    return enabled_state == 1;
}

bool kernel_detsched_participant(void)
{
    return self_id != NO_HOLDER;
}

void kernel_detsched_set_stop_sink(void (*sink)(const char *detail))
{
    stop_sink = sink;
}

static void named_stop(const char *detail)
{
    fprintf(stderr, "STOP %s\n", detail);
    fflush(stderr);
    if (stop_sink != NULL) stop_sink(detail);
    abort();
}

/* Under ds_lock: the next participant after `from` in id order, wrapping; `from` itself only when it is
 * the sole one left; NO_HOLDER when there is none. */
static int next_locked(int from)
{
    for (unsigned step = 1u; step <= KERNEL_DETSCHED_MAX; step++) {
        const unsigned id = (unsigned)(from + (int)step) % KERNEL_DETSCHED_MAX;
        if (parts[id].state != P_FREE) return (int)id;
    }
    return NO_HOLDER;
}

/* Under ds_lock: give the baton to `to`. */
static void give_locked(int to)
{
    if (to != holder) stats.handoffs++;
    holder = to;
    if (to != NO_HOLDER) pthread_cond_signal(&parts[to].cond);
}

/* Under ds_lock: wait until the caller holds the baton again. A wait longer than the watchdog means
 * the holder is stuck outside every hand-off point (a spin with no safepoint, or a host wait for a
 * guest thread that does not hold the baton). */
static void await_locked(void)
{
    const uint64_t began = wall_ms();
    while (holder != self_id) {
        struct timespec until;
        clock_gettime(CLOCK_MONOTONIC, &until);
        until.tv_sec += 1;
        const int error = pthread_cond_timedwait(&parts[self_id].cond, &ds_lock, &until);
        if (error == ETIMEDOUT && holder != self_id && wall_ms() - began >= watchdog_ms) {
            static char detail[200];
            if (holder != NO_HOLDER && parts[holder].state == P_ACTIVE) {
                (void)pthread_kill(parts[holder].thread, SIGRTMIN + 7);
                pthread_mutex_unlock(&ds_lock);
                const struct timespec settle = {0, 300000000L};
                nanosleep(&settle, NULL);
                pthread_mutex_lock(&ds_lock);
            }
            snprintf(detail, sizeof detail,
                     "T3442 detsched stuck holder: participant %d waited %u ms for the baton held by %d "
                     "(no hand-off point reached)", self_id, watchdog_ms, holder);
            pthread_mutex_unlock(&ds_lock);
            named_stop(detail);
        }
    }
}

void kernel_detsched_register(unsigned id)
{
    if (!kernel_detsched_enabled() || id >= KERNEL_DETSCHED_MAX) return;
    pthread_once(&ds_once, ds_init);
    pthread_mutex_lock(&ds_lock);
    parts[id].state = P_PENDING;
    parts[id].has_tested = false;
    unsigned count = 0u;
    for (unsigned i = 0u; i < KERNEL_DETSCHED_MAX; i++) count += parts[i].state != P_FREE;
    if (count > stats.participants_max) stats.participants_max = count;
    if (holder == NO_HOLDER) holder = (int)id;
    pthread_mutex_unlock(&ds_lock);
}

void kernel_detsched_unregister(unsigned id)
{
    if (!kernel_detsched_enabled() || id >= KERNEL_DETSCHED_MAX) return;
    pthread_once(&ds_once, ds_init);
    pthread_mutex_lock(&ds_lock);
    parts[id].state = P_FREE;
    if (holder == (int)id) give_locked(next_locked((int)id));
    pthread_mutex_unlock(&ds_lock);
}

void kernel_detsched_attach(unsigned id)
{
    if (!kernel_detsched_enabled() || id >= KERNEL_DETSCHED_MAX) return;
    pthread_once(&ds_once, ds_init);
    pthread_mutex_lock(&ds_lock);
    if (parts[id].state == P_FREE) { /* not registered by a creator (a host path without one) */
        parts[id].state = P_PENDING;
        if (holder == NO_HOLDER) holder = (int)id;
    }
    parts[id].state = P_ACTIVE;
    parts[id].thread = pthread_self();
    self_id = (int)id;
    own_safepoints = 0u;
    await_locked();
    pthread_mutex_unlock(&ds_lock);
}

void kernel_detsched_detach(void)
{
    if (self_id == NO_HOLDER) return;
    pthread_mutex_lock(&ds_lock);
    const int me = self_id;
    parts[me].state = P_FREE;
    atomic_fetch_add_explicit(&progress, 1u, memory_order_relaxed);
    if (holder == me) give_locked(next_locked(me));
    self_id = NO_HOLDER;
    pthread_mutex_unlock(&ds_lock);
}

/* Under ds_lock, holding the baton: move it to the next participant and wait for it to come back. */
static bool pass_locked(void)
{
    const int next = next_locked(self_id);
    if (next == NO_HOLDER || next == self_id) return false;
    give_locked(next);
    await_locked();
    return true;
}

bool kernel_detsched_yield(void)
{
    if (self_id == NO_HOLDER) return false;
    pthread_mutex_lock(&ds_lock);
    atomic_fetch_add_explicit(&progress, 1u, memory_order_relaxed);
    own_safepoints = 0u;
    const bool moved = pass_locked();
    if (moved) stats.yields++;
    pthread_mutex_unlock(&ds_lock);
    return moved;
}

/* Under ds_lock: every live participant has re-tested its wait since the last guest progress. */
static bool all_blocked_locked(void)
{
    const uint64_t now = atomic_load_explicit(&progress, memory_order_relaxed);
    for (unsigned i = 0u; i < KERNEL_DETSCHED_MAX; i++) {
        if (parts[i].state == P_PENDING) return false;
        if (parts[i].state == P_ACTIVE && (!parts[i].has_tested || parts[i].tested_progress != now)) return false;
    }
    return true;
}

void kernel_detsched_block(void)
{
    if (self_id == NO_HOLDER) {
        const struct timespec nap = {0, 200000L};
        nanosleep(&nap, NULL);
        return;
    }
    pthread_mutex_lock(&ds_lock);
    stats.blocks++;
    parts[self_id].tested_progress = atomic_load_explicit(&progress, memory_order_relaxed);
    parts[self_id].has_tested = true;
    if (all_blocked_locked()) {
        if (kernel_vtime_jump_to_earliest()) {
            stats.idle_jumps++;
            idle_since_ms = 0u;
            atomic_fetch_add_explicit(&progress, 1u, memory_order_relaxed);
        } else {
            /* Nothing timed is pending: only the host (a hold, a host thread setting an event) can
             * change anything. Wait a little wall time, then let everyone re-test. */
            const uint64_t now = wall_ms();
            if (idle_since_ms == 0u) idle_since_ms = now;
            if (now - idle_since_ms >= watchdog_ms) {
                static char detail[200];
                snprintf(detail, sizeof detail,
                         "T3442 detsched deadlock: every guest thread blocked for %u ms with no timed wait pending "
                         "(participant %d)", watchdog_ms, self_id);
                pthread_mutex_unlock(&ds_lock);
                named_stop(detail);
            }
            stats.external_waits++;
            pthread_mutex_unlock(&ds_lock);
            const struct timespec nap = {0, 200000L};
            nanosleep(&nap, NULL);
            pthread_mutex_lock(&ds_lock);
            atomic_fetch_add_explicit(&progress, 1u, memory_order_relaxed);
        }
    } else {
        idle_since_ms = 0u;
    }
    (void)pass_locked();
    pthread_mutex_unlock(&ds_lock);
}

void kernel_detsched_note_progress(void)
{
    if (self_id != NO_HOLDER) atomic_fetch_add_explicit(&progress, 1u, memory_order_relaxed);
}

void kernel_detsched_safepoint(void)
{
    if (self_id == NO_HOLDER) return;
    atomic_fetch_add_explicit(&progress, 1u, memory_order_relaxed);
    if (++own_safepoints < quantum) return;
    if (kernel_sync_current_irql() != KERNEL_IRQL_PASSIVE) return; /* no preemption above PASSIVE */
    own_safepoints = 0u;
    pthread_mutex_lock(&ds_lock);
    if (pass_locked()) stats.preemptions++;
    pthread_mutex_unlock(&ds_lock);
}

kernel_detsched_stats kernel_detsched_get_stats(void)
{
    pthread_mutex_lock(&ds_lock);
    const kernel_detsched_stats copy = stats;
    pthread_mutex_unlock(&ds_lock);
    return copy;
}

void kernel_detsched_test_reset(bool on, unsigned test_quantum)
{
    pthread_once(&ds_once, ds_init);
    pthread_mutex_lock(&ds_lock);
    enabled_state = on ? 1 : 0;
    quantum = test_quantum != 0u ? test_quantum : KERNEL_DETSCHED_DEFAULT_QUANTUM;
    watchdog_ms = KERNEL_DETSCHED_DEFAULT_WATCHDOG_MS;
    for (unsigned i = 0u; i < KERNEL_DETSCHED_MAX; i++) {
        parts[i].state = P_FREE;
        parts[i].has_tested = false;
    }
    holder = NO_HOLDER;
    idle_since_ms = 0u;
    memset(&stats, 0, sizeof stats);
    atomic_store(&progress, 0u);
    pthread_mutex_unlock(&ds_lock);
    self_id = NO_HOLDER;
}
