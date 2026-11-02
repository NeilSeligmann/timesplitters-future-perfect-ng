/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * The deterministic virtual time-stamp counter. See kernel_clock.h for the policy and the
 * measurements behind it.
 *
 * ONE MUTEX, NOT ATOMICS. A read is `max(now + step, floor)`, two dependent operations, and
 * a frame is an addition with a carry. A lock-free form would need a compare-exchange loop
 * for the read and a second one for the frame, to save a lock that is taken a handful of
 * times per frame. The counter this replaces was a bare `static uint64_t` incremented by
 * every guest thread with no exclusion at all, which loses updates on a multi-core host and
 * so was not even monotonic across threads.
 */

#include "kernel_clock.h"

#include <pthread.h>
#include <stdatomic.h>
#include <string.h>

#ifdef TSFP_RUST_KCLOCK
#include <stdlib.h>
#include <stdio.h>
#include "kernel_clock_rust.h"
#endif

#include "kernel_call.h"
#include "kernel_hle.h"
#include "kernel_thread.h"

#include <stdio.h>
#include <stdlib.h>
#include <execinfo.h>
#include <sys/syscall.h>
#include <unistd.h>

#define MAX_REFRESH_HZ 1000u

/* T3496 diagnostic (TSFP_KCLOCK_TRACE=1, read once): which host threads move the clock between two
 * frames. One line per thread per frame: reads, stalls and advances, and up to four lifted call
 * sites as offsets from kernel_clock_read (resolve with `nm` on the host binary). Under clock_lock. */
#define TRACE_SLOTS 16u
#define TRACE_SITES 4u
#define TRACE_DEPTH 5u
typedef struct trace_slot {
    long tid;
    uint32_t identity;
    uint64_t reads, stalls, advances, advanced_ticks, first, last;
    uintptr_t site[TRACE_SITES];
    uint64_t site_reads[TRACE_SITES];
    uintptr_t advance_site[TRACE_SITES][TRACE_DEPTH];
    uint64_t advance_site_count[TRACE_SITES];
} trace_slot;
static _Thread_local uintptr_t trace_advance_stack[TRACE_DEPTH];
static _Thread_local uint64_t trace_advance_before;
static trace_slot trace_slots[TRACE_SLOTS];
static uint64_t trace_frames;
static _Thread_local uintptr_t trace_caller;
static int trace_mode = -1;
static bool trace_enabled(void)
{
    if (trace_mode < 0) {
        const char *value = getenv("TSFP_KCLOCK_TRACE");
        trace_mode = value != NULL && strcmp(value, "0") != 0;
    }
    return trace_mode == 1;
}
void kernel_clock_note_caller(uintptr_t return_address)
{
    trace_caller = return_address;
}
static trace_slot *trace_slot_locked(void)
{
    const long tid = (long)syscall(SYS_gettid);
    for (unsigned index = 0; index < TRACE_SLOTS; ++index) {
        trace_slot *slot = &trace_slots[index];
        if (slot->tid == tid || slot->tid == 0) {
            slot->tid = tid;
            slot->identity = kernel_thread_current_identity();
            return slot;
        }
    }
    return NULL;
}
static void trace_event_locked(unsigned kind, uint64_t reading)
{
    if (!trace_enabled()) return;
    trace_slot *slot = trace_slot_locked();
    if (slot == NULL) return;
    if (kind == 0u) {
        if (slot->reads == 0u) slot->first = reading;
        slot->last = reading;
        slot->reads++;
        const uintptr_t site = trace_caller;
        trace_caller = 0u;
        for (unsigned index = 0; index < TRACE_SITES; ++index) {
            if (slot->site[index] == site || slot->site_reads[index] == 0u) {
                slot->site[index] = site;
                slot->site_reads[index]++;
                break;
            }
        }
    } else if (kind == 1u) {
        slot->stalls++;
    } else {
        slot->advances++;
        slot->advanced_ticks += reading - trace_advance_before;
        for (unsigned index = 0; index < TRACE_SITES; ++index) {
            if (slot->advance_site_count[index] == 0u ||
                memcmp(slot->advance_site[index], trace_advance_stack, sizeof trace_advance_stack) == 0) {
                memcpy(slot->advance_site[index], trace_advance_stack, sizeof trace_advance_stack);
                slot->advance_site_count[index]++;
                break;
            }
        }
    }
}
static void trace_frame_locked(uint64_t floor)
{
    if (!trace_enabled()) return;
    trace_frames++;
    const long frame_tid = (long)syscall(SYS_gettid);
    for (unsigned index = 0; index < TRACE_SLOTS; ++index) {
        trace_slot *slot = &trace_slots[index];
        if (slot->tid == 0 || (slot->reads | slot->stalls | slot->advances) == 0u) continue;
        char sites[160] = "";
        size_t used = 0u;
        for (unsigned site = 0; site < TRACE_SITES && slot->site_reads[site] != 0u; ++site) {
            const long offset = slot->site[site] == 0u
                ? 0L : (long)(slot->site[site] - (uintptr_t)&kernel_clock_read);
            const int wrote = snprintf(sites + used, sizeof sites - used, " %lx:%llu", offset,
                                       (unsigned long long)slot->site_reads[site]);
            if (wrote > 0 && (size_t)wrote < sizeof sites - used) used += (size_t)wrote;
        }
        char advance_sites[512] = "";
        used = 0u;
        for (unsigned site = 0; site < TRACE_SITES && slot->advance_site_count[site] != 0u; ++site) {
            int wrote = snprintf(advance_sites + used, sizeof advance_sites - used, " %llu@",
                                 (unsigned long long)slot->advance_site_count[site]);
            if (wrote > 0 && (size_t)wrote < sizeof advance_sites - used) used += (size_t)wrote;
            for (unsigned depth = 0; depth < TRACE_DEPTH; ++depth) {
                const uintptr_t frame = slot->advance_site[site][depth];
                wrote = snprintf(advance_sites + used, sizeof advance_sites - used, "%s%lx", depth ? "/" : "",
                                 frame == 0u ? 0L : (long)(frame - (uintptr_t)&kernel_clock_read));
                if (wrote > 0 && (size_t)wrote < sizeof advance_sites - used) used += (size_t)wrote;
            }
        }
        fprintf(stderr, "kclock-trace: frame %llu floor %llu by %ld tid %ld id %#x reads %llu first %llu last %llu "
                "stalls %llu advances %llu advanced %llu sites%s advance-sites%s\n",
                (unsigned long long)trace_frames, (unsigned long long)floor, frame_tid, slot->tid,
                (unsigned)slot->identity, (unsigned long long)slot->reads,
                (unsigned long long)slot->first, (unsigned long long)slot->last,
                (unsigned long long)slot->stalls, (unsigned long long)slot->advances,
                (unsigned long long)slot->advanced_ticks, sites, advance_sites);
        const long tid = slot->tid;
        memset(slot, 0, sizeof *slot);
        slot->tid = tid;
    }
}

static pthread_mutex_t clock_lock = PTHREAD_MUTEX_INITIALIZER;
/* T1289: atomic so kernel_clock_peek needs no lock (every XDK call and safepoint peeks). Writers still hold clock_lock and both values only
 * ever grow (reset aside), so a peek that sees one of them older than the other still returns a value no older than any earlier peek. */
static _Atomic uint64_t now_ticks;
static _Atomic uint64_t floor_ticks;
/* Remainder of FREQUENCY / refresh, accumulated in units of 1/refresh tick. Kept against the
 * rate that produced it, so a change of rate starts a fresh remainder instead of
 * mixing denominators. */
static uint64_t frame_carry;
static unsigned carry_rate;
/* Sub-tick remainder of the stalls, in millionths of a tick (the stall's own denominator is
 * 1,000,000 microseconds per second). Carried so that 1,000,000 stalls of 1 us add exactly
 * one second. */
static uint64_t stall_carry;
static uint32_t tick_count_address;
static kernel_clock_frame_hook frame_hook;
/* T1237: where guest time came from, for the timing report (host observation only, nothing here feeds back). */
static kernel_clock_stats stats;
/* T2282: render-only replays do not own simulation time. Native Rust owns
 * their clock classification; the C oracle/publisher and lock stay intact. */
extern void tsfp_frame_clock_replay(void *, uint64_t) __attribute__((weak));
extern bool tsfp_frame_clock_advances(const void *) __attribute__((weak));
static uint64_t frame_clock_state;
static bool render_clock_paused_locked(void)
{
    return tsfp_frame_clock_advances && !tsfp_frame_clock_advances(&frame_clock_state);
}
void kernel_clock_render_replay(bool replay)
{
    if (!tsfp_frame_clock_replay) return;
    pthread_mutex_lock(&clock_lock);
    tsfp_frame_clock_replay(&frame_clock_state, replay);
    pthread_mutex_unlock(&clock_lock);
}

/* T2207: linked by default (build.sh), Rust selected unless TSFP_RUST_KCLOCK=0 at startup
 * (Tier 2 EQUAL on Story and Map Maker routes, docs/t-contract-kernel-clock.md).
 * Selection is immutable; retain C publication, hook and lock boundaries.
 * Timer/event scheduling stays with K-1 until its adapter is qualified. */
#ifdef TSFP_RUST_KCLOCK
static pthread_once_t rust_clock_once = PTHREAD_ONCE_INIT;
static bool rust_clock_selected;
static void select_rust_clock(void)
{
    const char *value = getenv("TSFP_RUST_KCLOCK");
    rust_clock_selected = value == NULL || strcmp(value, "0") != 0;
    fprintf(stderr, "kernel_clock: backend %s (TSFP_RUST_KCLOCK)\n", rust_clock_selected ? "rust" : "c");
}
static bool rust_clock_enabled(void)
{
    (void)pthread_once(&rust_clock_once, select_rust_clock);
    return rust_clock_selected;
}
bool kernel_clock_rust_selected(void)
{
    return rust_clock_enabled();
}
/* Called under clock_lock. Mirrors support the non-advancing atomic peek and
 * the existing KeTickCount publisher; Rust owns every state transition. */
static uint64_t rust_clock_apply(uint32_t operation, uint64_t value)
{
    const tsfp_kernel_clock_snapshot s = tsfp_kernel_clock_apply(operation, value);
    now_ticks = s.total;
    floor_ticks = s.floor;
    stats = (kernel_clock_stats){s.total, s.floor, s.reads, s.reads_above_floor,
        s.frames, s.stall_ticks, s.advance_ticks, s.advance_calls};
    return s.result;
}
#endif


void kernel_clock_set_frame_hook(kernel_clock_frame_hook hook)
{
    pthread_mutex_lock(&clock_lock);
    frame_hook = hook;
    pthread_mutex_unlock(&clock_lock);
}

/* Called under the clock lock. Only the observed ULONG at the export address
 * is modelled: GetTickCount at 0x0037EB7C reads exactly this dword. */
static bool publish_tick_count(void)
{
    const uint64_t ticks = now_ticks > floor_ticks ? now_ticks : floor_ticks;
    const uint32_t ms = (uint32_t)((ticks / KERNEL_CLOCK_FREQUENCY_HZ) * 1000u +
        (ticks % KERNEL_CLOCK_FREQUENCY_HZ) * 1000u / KERNEL_CLOCK_FREQUENCY_HZ);
    return tick_count_address == 0u || kernel_guest_write_u32(tick_count_address, ms);
}

bool kernel_clock_bind_tick_count(uint32_t address)
{
    if (address != 0u && kernel_guest_at_ro(address, sizeof(uint32_t)) == NULL) {
        return false;
    }
    pthread_mutex_lock(&clock_lock);
    tick_count_address = address;
    const bool okay = publish_tick_count();
    pthread_mutex_unlock(&clock_lock);
    return okay;
}

uint64_t kernel_clock_read(void)
{
    pthread_mutex_lock(&clock_lock);
    if (render_clock_paused_locked()) {
        stats.reads++;
        const uint64_t reading = kernel_clock_peek();
        pthread_mutex_unlock(&clock_lock);
        return reading;
    }
#ifdef TSFP_RUST_KCLOCK
    if (rust_clock_enabled()) {
        const uint64_t reading = rust_clock_apply(0u, 0u);
        (void)publish_tick_count();
        trace_event_locked(0u, reading);
        pthread_mutex_unlock(&clock_lock);
        return reading;
    }
#endif
    stats.reads++;
    now_ticks += KERNEL_CLOCK_READ_STEP_TICKS;
    if (now_ticks < floor_ticks) {
        now_ticks = floor_ticks;
    } else {
        stats.reads_above_floor++;
    }
    (void)publish_tick_count();
    const uint64_t reading = now_ticks;
    trace_event_locked(0u, reading);
    pthread_mutex_unlock(&clock_lock);
    return reading;
}

bool kernel_clock_frame_floor(unsigned refresh_hz, uint64_t *floor_after)
{
    if (refresh_hz == 0u || refresh_hz > MAX_REFRESH_HZ) {
        return false;
    }
    pthread_mutex_lock(&clock_lock);
    if (render_clock_paused_locked()) {
        if (floor_after) *floor_after = floor_ticks;
        const kernel_clock_frame_hook hook = frame_hook;
        pthread_mutex_unlock(&clock_lock);
        if (hook) hook();
        return true;
    }
#ifdef TSFP_RUST_KCLOCK
    if (rust_clock_enabled()) {
        const uint64_t floor = rust_clock_apply(1u, refresh_hz);
        (void)publish_tick_count();
        trace_frame_locked(floor);
        if (floor_after != NULL) *floor_after = floor;
        const kernel_clock_frame_hook hook = frame_hook;
        pthread_mutex_unlock(&clock_lock);
        if (hook != NULL) hook();
        return true;
    }
#endif
    if (carry_rate != refresh_hz) {
        carry_rate = refresh_hz;
        frame_carry = 0u;
    }
    stats.frames++;
    floor_ticks += KERNEL_CLOCK_FREQUENCY_HZ / refresh_hz;
    frame_carry += KERNEL_CLOCK_FREQUENCY_HZ % refresh_hz;
    if (frame_carry >= refresh_hz) {
        frame_carry -= refresh_hz;
        floor_ticks++;
    }
    (void)publish_tick_count();
    trace_frame_locked(floor_ticks);
    if (floor_after != NULL) {
        *floor_after = floor_ticks;
    }
    const kernel_clock_frame_hook hook = frame_hook;
    pthread_mutex_unlock(&clock_lock);
    if (hook != NULL) {
        hook();
    }
    return true;
}

bool kernel_clock_frame(unsigned refresh_hz)
{
    return kernel_clock_frame_floor(refresh_hz, NULL);
}

uint64_t kernel_clock_stall_us(uint32_t microseconds)
{
    if (microseconds > KERNEL_CLOCK_STALL_MAX_US) {
        kernel_hle_log()("kernel: KeStallExecutionProcessor(%u) refused: above the %u us "
                         "bound (measured maximum 10000), clock not advanced\n",
                         microseconds, KERNEL_CLOCK_STALL_MAX_US);
        return 0u;
    }
    pthread_mutex_lock(&clock_lock);
    if (render_clock_paused_locked()) {
        const uint64_t reading = 0u;
        pthread_mutex_unlock(&clock_lock);
        return reading;
    }
#ifdef TSFP_RUST_KCLOCK
    if (rust_clock_enabled()) {
        const uint64_t ticks = rust_clock_apply(2u, microseconds);
        (void)publish_tick_count();
        trace_event_locked(1u, ticks);
        pthread_mutex_unlock(&clock_lock);
        return ticks;
    }
#endif
    /* From the later of the counter and the floor, or a stall below the floor is swallowed
     * by the next read's max(). */
    if (now_ticks < floor_ticks) {
        now_ticks = floor_ticks;
    }
    const uint64_t scaled = (uint64_t)microseconds * KERNEL_CLOCK_FREQUENCY_HZ + stall_carry;
    const uint64_t ticks = scaled / 1000000u;
    stall_carry = scaled % 1000000u;
    stats.stall_ticks += ticks;
    now_ticks += ticks;
    (void)publish_tick_count();
    trace_event_locked(1u, ticks);
    pthread_mutex_unlock(&clock_lock);
    return ticks;
}

uint64_t kernel_clock_advance_to(uint64_t ticks)
{
    if (trace_mode == 1) {
        void *frames[TRACE_DEPTH + 1u];
        const int depth = backtrace(frames, (int)(TRACE_DEPTH + 1u));
        for (unsigned index = 0; index < TRACE_DEPTH; ++index)
            trace_advance_stack[index] = (int)index + 1 < depth ? (uintptr_t)frames[index + 1u] : 0u;
        trace_advance_before = kernel_clock_peek();
    }
    pthread_mutex_lock(&clock_lock);
    if (render_clock_paused_locked()) {
        const uint64_t reading = kernel_clock_peek();
        pthread_mutex_unlock(&clock_lock);
        return reading;
    }
#ifdef TSFP_RUST_KCLOCK
    if (rust_clock_enabled()) {
        const uint64_t reading = rust_clock_apply(3u, ticks);
        (void)publish_tick_count();
        trace_event_locked(2u, reading);
        pthread_mutex_unlock(&clock_lock);
        return reading;
    }
#endif
    if (now_ticks < floor_ticks) {
        now_ticks = floor_ticks;
    }
    if (now_ticks < ticks) {
        stats.advance_to_ticks += ticks - now_ticks;
        stats.advance_to_calls++;
    }
    if (now_ticks < ticks) {
        now_ticks = ticks;
    }
    (void)publish_tick_count();
    trace_event_locked(2u, now_ticks);
    const uint64_t reading = now_ticks;
    pthread_mutex_unlock(&clock_lock);
    return reading;
}

kernel_clock_stats kernel_clock_get_stats(void)
{
    pthread_mutex_lock(&clock_lock);
    kernel_clock_stats copy = stats;
    copy.total_ticks = now_ticks > floor_ticks ? now_ticks : floor_ticks;
    copy.floor_ticks = floor_ticks;
    pthread_mutex_unlock(&clock_lock);
    return copy;
}

uint64_t kernel_clock_peek(void)
{
    const uint64_t now = atomic_load_explicit(&now_ticks, memory_order_relaxed);
    const uint64_t floor = atomic_load_explicit(&floor_ticks, memory_order_relaxed);
    return now > floor ? now : floor;
}

uint64_t kernel_clock_frequency(void)
{
    return KERNEL_CLOCK_FREQUENCY_HZ;
}

uint32_t kernel_clock_tick_count_ms(void)
{
    const uint64_t ticks = kernel_clock_peek();
    /* Divide first to avoid overflowing a full-width TSC before truncating to ULONG. */
    return (uint32_t)((ticks / KERNEL_CLOCK_FREQUENCY_HZ) * 1000u +
        (ticks % KERNEL_CLOCK_FREQUENCY_HZ) * 1000u / KERNEL_CLOCK_FREQUENCY_HZ);
}

void kernel_clock_reset(void)
{
    pthread_mutex_lock(&clock_lock);
    frame_clock_state = 0u;
    now_ticks = 0u;
    floor_ticks = 0u;
    frame_carry = 0u;
    carry_rate = 0u;
    memset(&stats, 0, sizeof stats);
    stall_carry = 0u;
#ifdef TSFP_RUST_KCLOCK
    if (rust_clock_enabled()) (void)rust_clock_apply(4u, 0u);
#endif
    (void)publish_tick_count();
    pthread_mutex_unlock(&clock_lock);
}

/* The title stores EDX:EAX after 126 at 0x004414EE and consumes both halves
 * after 127 at 0x00441502. No guest stack arguments (nxdk @0 and hand count). */
static uint32_t clock_result(void *context, uint64_t value)
{
    kernel_call_frame *frame = (kernel_call_frame *)context;
    if (frame != NULL) {
        frame->result_high = (uint32_t)(value >> 32);
        frame->has_result_high = true;
    }
    return (uint32_t)value;
}

static uint32_t hle_performance_counter(void *context)
{
    return clock_result(context, kernel_clock_read());
}

static uint32_t hle_performance_frequency(void *context)
{
    return clock_result(context, kernel_clock_frequency());
}

/* VOID export: EAX is not read by any measured caller, so 0 by convention. A frame that
 * cannot supply the argument is reported, never read as 0 (a zero stall is a real value). */
static uint32_t hle_stall_execution_processor(void *context)
{
    const kernel_call_frame *frame = (const kernel_call_frame *)context;
    uint32_t microseconds = 0u;
    if (frame == NULL || !kernel_frame_arg(frame, 0u, &microseconds)) {
        kernel_hle_log()("kernel: KeStallExecutionProcessor could not read its MicroSeconds "
                         "argument from the guest stack, clock not advanced\n");
        return 0u;
    }
    (void)kernel_clock_stall_us(microseconds);
    return 0u;
}

unsigned kernel_clock_register(void)
{
    unsigned bound = 0u;
    if (kernel_hle_register(151u, hle_stall_execution_processor)) {
        bound++;
    }
    if (kernel_hle_register(126u, hle_performance_counter)) {
        bound++;
    }
    if (kernel_hle_register(127u, hle_performance_frequency)) {
        bound++;
    }
    return bound;
}
