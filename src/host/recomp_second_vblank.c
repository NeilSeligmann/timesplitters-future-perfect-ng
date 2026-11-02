/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "recomp_second_vblank.h"
#include "d3d8_guest.h"
#include "d3d8_flip.h"
#include "d3d8_gpu.h"
#include "d3d8_vblank_effects.h"
#include "host_runtime.h"
#include "kernel_detsched.h"
#include "kernel_av.h"
#include "kernel_call.h"
#include "kernel_sync.h"
#include "kernel_vtime.h"
#include "recomp_vblank_quiescence.h"
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>
extern bool tsfp_frame_state_native_selected(void) __attribute__((weak));
extern uint32_t tsfp_frame_state_vblank_counter_read(void) __attribute__((weak));
extern uint32_t tsfp_frame_state_timing_history_snapshot(uint32_t index) __attribute__((weak));
#define FRAME 0x1538C0u
#define DEVICE 0x3E3F60u
/* The title counter the frame wait's getter 0x22030 returns and the counter its last exit stored. */
#define COUNTER 0x563918u
#define FRAME_LAST_EXIT 0x7A58B0u
/* The title counter after the startup event and the credited second event. */
#define OWNER_WAIT_FIRST_COUNTER 2u
extern void tsfp_startup_vblank_clock_reset(void);
extern uint32_t tsfp_startup_vblank_clock_poll(uint64_t now_ns, uint32_t refresh_hz, uint64_t *wait_ns);
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static recomp_second_vblank_snapshot state;
static pthread_t owner, producer;
static d3d8_first_vblank_invoker invoke;
static recomp_second_vblank_producer_confirmed confirm;
static uint32_t stack_low, stack_high;
/* T460: configuration, not epoch state. 0 is off, cleared by every configure. */
static uint32_t owner_budget;
static bool interactive;
static bool startup_clock_enabled;
static atomic_bool owner_clock_announced;
static bool (*shutdown_query)(void); /* configured before guest startup */
void recomp_second_vblank_set_shutdown_query(bool (*query)(void)) { shutdown_query = query; }
/* T696: the frame wait poll may deliver one blank when its exit test fails. Configuration, cleared by configure. */
static bool poll_blank_enabled;
/* T592: the loading bar worker's blanks, see docs/vblank-delivery.md "T592 results". The loading bar
 * gate is the title's `while (1.0f > [PROGRESS])` in sub_00156840, entered from the frame loop. */
#define GATE 0x156840u
#define GATE_FLAG 0x4E7A94u
#define GATE_RUNNING 0x74A9ACu
#define GATE_WAIT 0x3800BFu
#define PROGRESS 0x749848u
#define PROGRESS_ONE 0x475C78u
#define GETTER 0x22030u
#define WORKER_START 0x156CB0u
#define WORKER_SYSTEM 0x37FE1Du
#define WORKER_HOLD_MS_DEFAULT 600000u

/* The selected frame-state port owns this counter in Rust. Keep the lifted
 * guest-memory read for the oracle path, but do not mistake its stale mirror
 * for the selected value or write a Rust-owned counter back into guest memory. */
static bool title_counter_is_native(void)
{
    return tsfp_frame_state_native_selected != NULL && tsfp_frame_state_native_selected();
}

static bool title_counter_read(uint32_t *counter)
{
    if (counter == NULL) return false;
    if (title_counter_is_native()) {
        if (tsfp_frame_state_vblank_counter_read == NULL) return false;
        *counter = tsfp_frame_state_vblank_counter_read();
        return true;
    }
    return kernel_guest_read_u32(COUNTER,counter);
}

/* [0x7A58B0] is history word 4; with the native frame state its guest mirror is stale, so the
 * frame wait's exit test would see counter - stale >= 1 forever and never ask for a blank. */
static bool frame_last_exit_read(uint32_t *last)
{
    if (last == NULL) return false;
    if (title_counter_is_native()) {
        if (tsfp_frame_state_timing_history_snapshot == NULL) return false;
        *last = tsfp_frame_state_timing_history_snapshot(4u);
        return true;
    }
    return kernel_guest_read_u32(FRAME_LAST_EXIT,last);
}

static bool title_counter_probe_write(uint32_t counter)
{
    return title_counter_is_native() || kernel_guest_write_u32(COUNTER,counter);
}
#define STARTUP_OWNER_WAIT_MS 2000u
/* T604: the entries that reach a store to the running flag [0x74A9AC]. The stop function 0x155550 (store at
 * 0x15556B) and the start 0x156D80 with its body 0x156D10 (store at 0x156D31, entered by 0x156D80's tail jump)
 * are the writers, the gate 0x156840 tail jumps into the stop function. A tail jump is a plain C call in the
 * lifted code with no safepoint, so only a CALL (direct or indirect) of these entries is seen. */
#define RUNNING_STOP 0x155550u
#define RUNNING_START 0x156D80u
#define RUNNING_START_BODY 0x156D10u
enum { OWNER_ELSEWHERE = 0, OWNER_GATE, OWNER_GATE_WAIT };
static atomic_uint worker_budget;       /* 0 is off, cleared by every configure */
static atomic_uint worker_hold_ms;      /* the hold limit, configuration like the budget */
static atomic_uint owner_id;            /* the bound owner's handle, for the lock free call hook */
static atomic_uint worker_id;           /* the worker that delivered, for the epoch test */
static atomic_uint held_id;             /* the worker held idle by this module, else 0 */
static atomic_uint owner_location;
static _Noreturn void refuse(const char *reason)
{
    host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, FRAME, 0u, reason);
    abort();
}
static void finish(void)
{
    pthread_mutex_lock(&lock);
    state.inflight = false;
    state.refused = true;
    pthread_mutex_unlock(&lock);
}
bool recomp_second_vblank_configure(bool enabled, d3d8_first_vblank_invoker fn,
                                    uint32_t low, uint32_t high,
                                    recomp_second_vblank_producer_confirmed checked)
{
    pthread_mutex_lock(&lock);
    if (state.inflight || state.epoch == UINT64_MAX ||
        (enabled && (!fn || !checked || !low || high <= low || high-low < 128u || (high & 15u)))) {
        pthread_mutex_unlock(&lock);
        return false;
    }
    uint64_t epoch = state.epoch + 1u;
    memset(&state, 0, sizeof(state));
    state.epoch = epoch;
    state.enabled = enabled;
    invoke = fn; confirm = checked; stack_low = low; stack_high = high; owner_budget = 0u; poll_blank_enabled = false; interactive = false; startup_clock_enabled = false; shutdown_query = NULL;
    atomic_store(&worker_budget, 0u); atomic_store(&worker_hold_ms, WORKER_HOLD_MS_DEFAULT);
    atomic_store(&owner_clock_announced, false);
    atomic_store(&owner_id, 0u); atomic_store(&worker_id, 0u); atomic_store(&held_id, 0u);
    atomic_store(&owner_location, OWNER_ELSEWHERE);
    pthread_mutex_unlock(&lock);
    d3d8_first_vblank_configure(enabled, fn, low, high);
    tsfp_startup_vblank_clock_reset();
    return true;
}
bool recomp_second_vblank_reset(void)
{
    pthread_mutex_lock(&lock);
    if (state.inflight || state.epoch == UINT64_MAX) {
        pthread_mutex_unlock(&lock); return false;
    }
    uint64_t epoch = state.epoch + 1u;
    bool enabled = state.enabled;
    memset(&state, 0, sizeof(state)); state.epoch = epoch; state.enabled = enabled;
    atomic_store(&worker_id, 0u); atomic_store(&held_id, 0u); atomic_store(&owner_location, OWNER_ELSEWHERE);
    pthread_mutex_unlock(&lock);
    d3d8_first_vblank_reset();
    tsfp_startup_vblank_clock_reset();
    return true;
}
bool recomp_second_vblank_set_owner_waits(uint32_t budget)
{
    pthread_mutex_lock(&lock);
    const bool accepted = state.inflight == false && state.epoch != UINT64_MAX &&
                          budget <= RECOMP_SECOND_VBLANK_OWNER_WAITS_MAX &&
                          (budget == 0u || state.enabled);
    if (accepted) owner_budget = budget;
    pthread_mutex_unlock(&lock);
    return accepted;
}
bool recomp_second_vblank_set_interactive(bool enabled)
{
    pthread_mutex_lock(&lock);
    const bool accepted = !state.inflight && (!enabled || state.enabled);
    if (accepted) interactive = enabled;
    pthread_mutex_unlock(&lock);
    return accepted;
}
bool recomp_second_vblank_set_startup_clock(bool enabled)
{
    pthread_mutex_lock(&lock);
    const bool accepted = !state.inflight && (!enabled || (interactive && state.enabled));
    if (accepted) startup_clock_enabled = enabled;
    pthread_mutex_unlock(&lock);
    return accepted;
}
bool recomp_second_vblank_set_startup_clock_from_environment(void)
{
    const char *value = getenv("TSFP_RUST_STARTUP_VBLANK_CLOCK");
    const bool enabled = value && strcmp(value, "1") == 0;
    if (!recomp_second_vblank_set_startup_clock(enabled)) return false;
    if (enabled)
        fprintf(stderr,"T3409 Rust startup vblank clock ON: one default 50/60 Hz display period for the exact terminal loading-worker wait\n");
    return true;
}
bool recomp_second_vblank_set_poll_blank(bool enabled)
{
    pthread_mutex_lock(&lock);
    const bool accepted = state.inflight == false && state.epoch != UINT64_MAX &&
                          (!enabled || owner_budget != 0u); /* a budget implies an enabled policy */
    if (accepted) poll_blank_enabled = enabled;
    pthread_mutex_unlock(&lock);
    return accepted;
}
bool recomp_second_vblank_set_worker_blanks(uint32_t budget)
{
    pthread_mutex_lock(&lock);
    const bool accepted = state.inflight == false && state.epoch != UINT64_MAX &&
                          budget <= RECOMP_SECOND_VBLANK_WORKER_BLANKS_MAX &&
                          (budget == 0u || (state.enabled && owner_budget != 0u));
    if (accepted) atomic_store(&worker_budget, budget);
    pthread_mutex_unlock(&lock);
    return accepted;
}
void recomp_second_vblank_set_worker_hold_ms(uint32_t milliseconds)
{
    atomic_store(&worker_hold_ms, milliseconds);
}
void recomp_second_vblank_get_snapshot(recomp_second_vblank_snapshot *out)
{
    if (!out) return;
    pthread_mutex_lock(&lock);
    *out = state; out->owner_budget = owner_budget; out->worker_budget = atomic_load(&worker_budget);
    pthread_mutex_unlock(&lock);
}
void recomp_second_vblank_bind_owner(uint32_t handle, uint32_t fs, uint32_t start, uint32_t system)
{
    const char *error = NULL;
    pthread_mutex_lock(&lock);
    bool enabled = state.enabled;
    if (enabled && start == 0x3801D9u && system == 0x37FE1Du) {
        if (state.bound || !handle || !fs || (fs & 4095u)) error = "invalid or duplicate two-event owner";
        else { state.bound = true; state.owner_handle = handle; state.owner_fs = fs; owner = pthread_self();
               atomic_store(&owner_id, handle); }
    }
    pthread_mutex_unlock(&lock);
    if (error) refuse(error);
    if (enabled) d3d8_first_vblank_bind_owner(handle, fs, start, system);
}
void recomp_second_vblank_note_registration(uint32_t callback, uint32_t handle, uint32_t fs)
{
    const char *error = NULL;
    pthread_mutex_lock(&lock);
    if (state.enabled) {
        if (state.inflight || state.registered || !state.bound || callback != 0x22020u ||
            handle != state.owner_handle || fs != state.owner_fs || !pthread_equal(owner,pthread_self()))
            error = "unsupported callback registration/rebind";
        else {
            uint32_t device, old_callback;
            if (!kernel_guest_read_u32(0x3E3F58u,&device) || device != DEVICE ||
                !kernel_guest_read_u32(DEVICE+0x1DB8u,&old_callback) || old_callback != 0u ||
                !kernel_guest_write_u32(DEVICE+0x1DB8u,old_callback))
                error = "callback registration requires fixed device and writable NULL slot";
            else state.registered = true;
        }
    }
    if (error) state.refused = true;
    pthread_mutex_unlock(&lock);
    if (error) refuse(error);
}
static void deliver_blank(uint32_t handle, uint32_t fs, bool worker, bool startup);
static bool worker_identity(uint32_t handle, uint32_t fs);
/* T587: a completed wait by ANOTHER guest thread after the owner-wait model began. It stays the T183
 * refusal, with the facts that decide it: which thread, where it started, and what the quiescence
 * predicate says with that thread as the deliverer. The text keeps the T183 prefix. */
static const char *later_thread_refusal(uint32_t handle)
{
    /* host_stop keeps the pointer and stores 255 bytes, so the worst case below is 245. */
    static _Thread_local char text[320];
    kernel_thread_record records[KERNEL_THREAD_MAX], self;
    const unsigned count = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
    const uint32_t start = kernel_thread_get(handle, &self) ? self.start_routine : 0u;
    const rvq_verdict verdict = recomp_vblank_quiescence_evaluate(records, count, handle);
    const int used = snprintf(text, sizeof text, "unsupported second-event refill/producer/backlog: wait by thread 0x%x "
                              "(start 0x%x), no second-thread delivery (T587), quiescence ", (unsigned)handle, (unsigned)start);
    if (verdict.holds) snprintf(text + used, sizeof text - (size_t)used, "holds");
    else snprintf(text + used, sizeof text - (size_t)used, "refused: %s (thread 0x%x, start 0x%x)",
                  recomp_vblank_quiescence_reason_name(verdict.reason), (unsigned)verdict.handle,
                  (unsigned)verdict.start_routine);
    return text;
}
void recomp_second_vblank_note_wait_completed(bool success, uint32_t handle, uint32_t fs)
{
    const char *error = NULL;
    bool deliver = false, later_thread = false, worker_blank = false;
    /* T592 and T3409: the thread table has its own lock. The startup clock
     * admits this exact worker's completed, normally-clocked waits without
     * turning on T592's independent loading-bar budget. */
    const bool budgeted_worker = atomic_load(&worker_budget) != 0u;
    bool startup_clock = false;
    if (!budgeted_worker) {
        pthread_mutex_lock(&lock);
        startup_clock = startup_clock_enabled;
        pthread_mutex_unlock(&lock);
    }
    const bool worker_thread = (budgeted_worker || startup_clock) && worker_identity(handle, fs);
    pthread_mutex_lock(&lock);
    if (state.enabled && !success) error = "unsuccessful GPU wait cannot produce callback credit";
    else if (state.enabled && success && state.inflight) error = "wait completion during callback reservation";
    else if (state.enabled && success && state.first_delivered && owner_budget != 0u && state.second_delivered &&
             !state.refused && handle && fs && handle == state.owner_handle && fs == state.owner_fs &&
             pthread_equal(owner,pthread_self())) {
        /* T460: the owner waits for its own blank. One callback per completed wait, bounded. */
        if (!interactive && state.owner_delivered >= owner_budget) error = "owner-wait callback budget exhausted";
        else { state.inflight = true; deliver = true; }
    }
    else if (worker_thread && state.enabled && success && state.first_delivered &&
             (owner_budget != 0u || startup_clock) &&
             state.second_delivered && !state.refused && !pthread_equal(owner,pthread_self()) &&
             handle != state.owner_handle && (handle != state.producer_handle || state.second_via_worker)) {
        /* T592: the loading bar worker's own blank. The inflight flag is taken after the hold. */
        worker_blank = true;
    }
    else if (state.enabled && success && startup_clock && interactive && owner_budget == 0u &&
             state.bound && state.registered && state.first_delivered && !state.refused &&
             !state.inflight && state.credits == 0u && handle && fs &&
             handle == state.owner_handle && fs == state.owner_fs && pthread_equal(owner,pthread_self())) {
        /* T3409: the owner's completed GPU wait is not a second-event producer. Leave the next
         * period to the exact frame-counter getter, which advances the Rust display clock. */
    }
    else if (state.enabled && success && state.first_delivered) {
        if (state.inflight || state.second_attempted || state.credits != 0u || !handle || !fs ||
            (fs & 4095u) || handle == state.owner_handle || fs == state.owner_fs || pthread_equal(owner,pthread_self())) {
            error = "unsupported second-event refill/producer/backlog";
            fprintf(stderr,"second-vblank refusal detail: inflight=%d attempted=%d credits=%u handle=%u fs_ok=%d is_owner_handle=%d "
                    "is_owner_fs=%d on_owner_thread=%d owner_budget=%u\n",(int)state.inflight,(int)state.second_attempted,
                    (unsigned)state.credits,(unsigned)(handle!=0u),(int)(fs!=0u&&(fs&4095u)==0u),(int)(handle==state.owner_handle),
                    (int)(fs==state.owner_fs),(int)pthread_equal(owner,pthread_self()),(unsigned)owner_budget);
            later_thread = state.second_delivered && owner_budget != 0u && !pthread_equal(owner,pthread_self());
        }
        else { state.credits = 1u; state.producer_handle = handle; state.producer_fs = fs; producer = pthread_self(); }
    }
    if (error) state.refused = true;
    pthread_mutex_unlock(&lock);
    if (error && later_thread) error = later_thread_refusal(handle);
    if (error) refuse(error);
    if (deliver) deliver_blank(handle, fs, false, false);
    else if (worker_blank) deliver_blank(handle, fs, true, false);
}
static bool overlap(uint32_t a, uint64_t n, uint32_t b, uint64_t m)
{ return (uint64_t)a < (uint64_t)b+m && (uint64_t)b < (uint64_t)a+n; }
/* T513: the flip words of the two preflights, as d3d8_first_vblank.c proves them (T407). The flip
 * model changes state the measured empty state pins to zero, so only with BOTH --model-flips and
 * --couple-vblank-effects the proof is the drained queue instead of constants. With either off the
 * zeros stay demanded, nothing here loosens. The queue is drained when the consumer index and the
 * producer index both equal the flips processed (every flip queued was processed) and both slots are
 * empty (the loop over the nine words). The threshold is whatever the queue left, and the helper run
 * of this blank (d3d8_vblank_effects_apply) already updated it when the count met it. */
static bool flip_modelled(void)
{ return d3d8_flip_enabled() && d3d8_vblank_effects_enabled(); }
/* The consumer index the nine words must hold: the flips processed under the model, else 0. */
static uint32_t flip_expected_consumer(bool modelled)
{ return modelled ? (uint32_t)d3d8_flip_hardware_get().flips : 0u; }
enum { FLIP_REASON_UNREADABLE, FLIP_REASON_NOT_DRAINED, FLIP_REASON_PHANTOM_THRESHOLD, FLIP_REASON_RECORD, FLIP_REASONS };
/* After the nine words matched (the threshold word is free under the model): why the flip state is
 * not the proven drained one (an index into the caller's reasons), or -1, and the record this blank's
 * helper run built, which is what the callback is given. Reads only. */
static int flip_queue_refusal(uint32_t count, d3d8_vblank_record *record)
{
    const d3d8_flip_hardware hardware = d3d8_flip_hardware_get();
    uint32_t producer_index, threshold;
    if (!kernel_guest_read_u32(DEVICE+D3D8_FLIP_DEV_PRODUCER,&producer_index) ||
        !kernel_guest_read_u32(DEVICE+D3D8_FLIP_DEV_THRESHOLD,&threshold))
        return FLIP_REASON_UNREADABLE;
    if (producer_index != (uint32_t)hardware.flips || hardware.queued != hardware.flips) return FLIP_REASON_NOT_DRAINED;
    /* No flip was ever queued: nothing set the threshold, it is the measured zero. */
    if (hardware.queued == 0u && threshold != 0u) return FLIP_REASON_PHANTOM_THRESHOLD;
    *record = d3d8_vblank_effects_last_record();
    if (record->count != count || record->flip_index != producer_index ||
        (record->flags == 2u && threshold != count + 1u))
        return FLIP_REASON_RECORD;
    return -1;
}
static const char *const owner_flip_reasons[FLIP_REASONS] = {
    "owner-wait flip words unreadable", "owner-wait flip queue not drained",
    "owner-wait flip threshold set without a queued flip", "owner-wait helper record disagrees with the flip words"};
static const char *const second_flip_reasons[FLIP_REASONS] = {
    "second-event flip words unreadable", "second-event flip queue not drained",
    "second-event flip threshold set without a queued flip", "second-event helper record disagrees with the flip words"};
/* T592: the loading bar worker's identity, from the thread table: the start routine and system
 * routine of the credited producer's kind, this control block, alive, and not the producer itself. */
static bool worker_identity(uint32_t handle, uint32_t fs)
{
    kernel_thread_record record;
    if (!handle || !fs || !kernel_thread_get(handle,&record)) return false;
    return record.in_use && record.started && !record.finished && record.start_routine == WORKER_START &&
           record.system_routine == WORKER_SYSTEM && record.start_context == 0u && record.control_base == fs;
}
static uint64_t now_milliseconds(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC,&now);
    return (uint64_t)now.tv_sec*1000u + (uint64_t)now.tv_nsec/1000000u;
}
static bool wait_for_startup_display_period(void)
{
    for (;;) {
        if (shutdown_query != NULL && shutdown_query()) return false;
        const bool virtual_time = kernel_vtime_enabled();
        uint64_t now_ns;
        if (virtual_time) {
            now_ns = kernel_vtime_now_ns();
        } else {
            struct timespec now;
            if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) return false;
            now_ns = (uint64_t)now.tv_sec * 1000000000u + (uint64_t)now.tv_nsec;
        }
        const uint32_t mode = d3d8_device_load32(0x1DDCu);
        const uint32_t refresh_hz = (mode & 0x00400000u) != 0u ? 60u : 50u;
        uint64_t wait_ns = 0u;
        const uint32_t result = tsfp_startup_vblank_clock_poll(now_ns, refresh_hz, &wait_ns);
        if (result == 1u) return true;
        if (result != 0u || wait_ns == 0u) return false;
        if (virtual_time) {
            if (wait_ns > UINT64_MAX - now_ns) return false;
            /* Replay vblank delivery advances from a guest wait boundary by one typed period.
             * It never waits for host scheduling or wall-clock time. */
            kernel_vtime_advance_to_ns(now_ns + wait_ns);
            continue;
        }
        struct timespec pause = {
            .tv_sec = (time_t)(wait_ns / 1000000000u),
            .tv_nsec = (long)(wait_ns % 1000000000u),
        };
        if (nanosleep(&pause, NULL) != 0 && errno != EINTR) return false;
    }
}
/* The startup deadline is one-shot. Once the loading worker has handed off to
 * the owner frame loop, each exact frame-wait getter arms the next period from
 * the current monotonic time. The caller still owns sleeping and callback
 * dispatch; Rust owns the typed deadline transition. */
static bool wait_for_owner_display_period(void)
{
    tsfp_startup_vblank_clock_reset();
    return wait_for_startup_display_period();
}
static void idle_briefly(void)
{
    /* T3442: these loops wait for ANOTHER guest thread; under the deterministic scheduler the baton
     * must move for it to run. */
    if (kernel_detsched_participant()) { kernel_detsched_block(); return; }
    const struct timespec pause = {0,200000L};
    nanosleep(&pause,NULL);
}
static bool worker_record_identity(uint32_t handle)
{
    kernel_thread_record record;
    return handle != 0u && kernel_thread_get(handle,&record) && record.in_use && record.started && !record.finished &&
           record.start_routine == WORKER_START && record.system_routine == WORKER_SYSTEM && record.start_context == 0u;
}
/* The owner called its one wait on this worker's termination and is blocked in it. */
static bool owner_blocked_on(uint32_t handle)
{
    kernel_thread_record record;
    return kernel_thread_get(atomic_load(&owner_id),&record) && record.block_state == KERNEL_THREAD_BLOCK_THREAD &&
           record.block_target == handle;
}
/* The worker can reach its terminal getter just before the owner enters NtWaitForSingleObjectEx.
 * Hold this exact getter call until the thread table proves the owner is blocked on this worker. */
static void await_startup_owner_block(uint32_t handle)
{
    const uint64_t began = now_milliseconds();
    while (!owner_blocked_on(handle)) {
        if (shutdown_query != NULL && shutdown_query())
            host_run_stop(HOST_STOP_HOST_SHUTDOWN, FRAME, 0u, "interactive shutdown while awaiting startup worker owner wait");
        if (now_milliseconds() - began >= STARTUP_OWNER_WAIT_MS)
            refuse("interactive startup worker owner wait was not observed");
        idle_briefly();
    }
}
/* The gate's prologue stores are visible: stage 0xA at 0x4E7968 (0x156856) and the target [0x74A990] equal to
 * the title's 1.0f (0x156860). The worker's steps read both, so it is released only after them, which makes
 * the number of steps it takes to fill the bar independent of when the owner's stores land. If they were
 * already these values the store timing cannot matter either. */
static bool gate_prologue_visible(void)
{
    uint32_t stage, target, one;
    return kernel_guest_read_u32(0x4E7968u,&stage) && kernel_guest_read_u32(0x74A990u,&target) &&
           kernel_guest_read_u32(PROGRESS_ONE,&one) && stage == 0xAu && target == one;
}
/* The owner is parked by location: it entered the gate (or called its one wait) and the proof says no
 * path from there reads the counter before it blocks on the worker's termination. */
static bool owner_parked_by_location(void)
{
    return atomic_load(&owner_location) != OWNER_ELSEWHERE;
}
/* The title's own gate test at 0x15686C: the owner loops while 1.0f > progress (an ordered compare, a
 * NaN leaves the loop). True while the owner still waits for the worker's steps. */
static bool gate_closed(void)
{
    uint32_t progress_bits, one_bits;
    float progress, one;
    if (!kernel_guest_read_u32(PROGRESS,&progress_bits) || !kernel_guest_read_u32(PROGRESS_ONE,&one_bits))
        refuse("worker-blank gate progress unreadable");
    memcpy(&progress,&progress_bits,sizeof progress);
    memcpy(&one,&one_bits,sizeof one);
    return one > progress;
}
/* The worker idles, at its getter poll before each step and in its completed wait before each delivery,
 * until quiescence holds with the worker as deliverer. An owner blocked on this exact worker is a
 * directly measured parked thread; otherwise the loading-gate path uses the tracked owner location.
 * The owner's gate prologue stores must also be visible (or the owner already blocked on the worker).
 * The loop at the wait IS the mandatory quiescence check: nothing is evaluated again after it, and a
 * verdict that never holds is a named stop. The poll is the worker's deterministic safepoint (it
 * spins only in the getter loop 0x156CB1). */
static void worker_hold(uint32_t handle)
{
    const uint64_t limit = (uint64_t)atomic_load(&worker_hold_ms);
    const uint64_t began = now_milliseconds();
    bool held = false;
    for (;;) {
        if (shutdown_query != NULL && shutdown_query())
            host_run_stop(HOST_STOP_HOST_SHUTDOWN, FRAME, 0u, "interactive host shutdown while holding worker");
        kernel_thread_record records[KERNEL_THREAD_MAX];
        const unsigned count = kernel_thread_snapshot(records,KERNEL_THREAD_MAX);
        unsigned table_count = 0u, parked_count = 0u;
        const uint32_t *table = recomp_vblank_quiescence_default_non_readers(&table_count);
        uint32_t parked[1];
        if (owner_blocked_on(handle) || owner_parked_by_location()) parked[parked_count++] = atomic_load(&owner_id);
        const rvq_verdict verdict = recomp_vblank_quiescence_evaluate_parked(records,count,handle,table,table_count,
                                                                             parked,parked_count);
        if (verdict.holds && (owner_blocked_on(handle) || gate_prologue_visible())) break;
        if (!held) {
            held = true;
            atomic_store(&held_id,handle);
            pthread_mutex_lock(&lock);
            state.worker_held++;
            pthread_mutex_unlock(&lock);
        }
        if (now_milliseconds() - began >= limit) {
            static _Thread_local char text[320];
            if (verdict.holds)
                snprintf(text,sizeof text,"worker blank held %u ms (T592): quiescence holds but the owner's gate prologue "
                         "stores are not visible",(unsigned)limit);
            else
                snprintf(text,sizeof text,"worker blank held %u ms without quiescence (T592): %s (thread 0x%x, start 0x%x)",
                         (unsigned)limit,recomp_vblank_quiescence_reason_name(verdict.reason),
                         (unsigned)verdict.handle,(unsigned)verdict.start_routine);
            refuse(text);
        }
        idle_briefly();
    }
    atomic_store(&held_id,0u);
}
/* After the blank that let the owner out of the gate the worker idles until the owner has blocked on the
 * worker's termination, which it does only after it stored the stop flag the worker tests next. Without
 * it the worker's next getter poll and the owner's exit race, and the number of blanks would depend on
 * the host's timing. */
static void worker_hold_final(uint32_t handle)
{
    const uint64_t limit = (uint64_t)atomic_load(&worker_hold_ms);
    const uint64_t began = now_milliseconds();
    bool counted = false;
    for (;;) {
        if (shutdown_query != NULL && shutdown_query())
            host_run_stop(HOST_STOP_HOST_SHUTDOWN, FRAME, 0u, "interactive host shutdown while holding worker");
        kernel_thread_record record;
        if (kernel_thread_get(atomic_load(&owner_id),&record) && record.block_state == KERNEL_THREAD_BLOCK_THREAD &&
            record.block_target == handle) break;
        if (!counted) {
            counted = true;
            atomic_store(&held_id,handle);
            pthread_mutex_lock(&lock);
            state.worker_final_held++;
            pthread_mutex_unlock(&lock);
        }
        if (now_milliseconds() - began >= limit) refuse("worker blank: the owner left the gate but never blocked on the worker (T592)");
        idle_briefly();
    }
    atomic_store(&held_id,0u);
}
/* The refusal texts that differ by deliverer. The owner's are the T460 ones byte for byte. */
typedef struct {
    const char *scope, *quiescence_off, *coupling_off, *caller, *bookkeeping, *probes, *invoke, *advance,
               *during, *stop_scope;
    const char *const *flips;
} blank_texts;
static const blank_texts owner_texts = {
    "owner-wait caller/PCR/device/counter scope refused", "owner-wait callback requires the quiescence check",
    "owner-wait callback requires the coupled vblank effects", NULL, "owner-wait nonempty or changed device bookkeeping",
    "owner-wait write probes refused", "owner-wait callback invocation failed",
    "owner-wait callback did not advance the counter by one", "owner-wait refill during callback refused",
    "owner-wait stop scope unavailable", owner_flip_reasons};
static const char *const worker_flip_reasons[FLIP_REASONS] = {
    "worker-wait flip words unreadable", "worker-wait flip queue not drained",
    "worker-wait flip threshold set without a queued flip", "worker-wait helper record disagrees with the flip words"};
static const blank_texts worker_texts = {
    "worker-wait caller/PCR/device/counter scope refused", "worker-wait callback requires the quiescence check",
    "worker-wait callback requires the coupled vblank effects", NULL, "worker-wait nonempty or changed device bookkeeping",
    "worker-wait write probes refused", "worker-wait callback invocation failed",
    "worker-wait callback did not advance the counter by one", "worker-wait refill during callback refused",
    "worker-wait stop scope unavailable", worker_flip_reasons};
/* T460: the owner's own blank. The original runs the helper from the DPC while the owner is blocked in
 * its wait, and a single processor resumes it only after the DPC (and so the callback) finished, so the
 * owner sees the counter advanced when its wait returns and nothing else samples it in between. The
 * delivery here is that, on the waiting thread inside the completed wait, after the coupled device
 * effects (the count is already published, this only reads it). Every precondition refuses before
 * any write, the quiescence check is mandatory, nothing is retried and no credit is invented.
 *
 * T592: the same delivery for the loading bar worker's own completed wait (worker == true). It differs
 * in who is parked: the owner is runnable, proven by location (owner_parked_by_location), so the worker
 * first idles until the predicate holds (worker_hold), and a blank that opens the owner's gate is
 * followed by the final hold. The counter the delivery expects counts both kinds of blank. */
static void deliver_blank(uint32_t handle, uint32_t fs, bool worker, bool startup)
{
    const blank_texts *text = worker ? &worker_texts : &owner_texts;
    if (!host_run_armed()) { finish(); abort(); }
    volatile host_run_scope scope = HOST_RUN_SCOPE_INITIALIZER;
    if (!host_run_scope_init(&scope)) { finish(); abort(); }
    if (sigsetjmp(*host_run_scope_jmp(&scope),0) != 0) {
        const host_stop stopped = *host_run_result();
        finish(); if (!host_run_scope_pop(&scope)) abort(); host_run_rethrow(&stopped);
    }
    if (!host_run_scope_push(&scope)) { finish(); refuse(text->stop_scope); }
    if (!recomp_vblank_quiescence_enabled()) refuse(text->quiescence_off);
    if (!d3d8_vblank_effects_enabled()) refuse(text->coupling_off);
    if (worker) {
        pthread_mutex_lock(&lock);
        const bool exhausted = !interactive && state.worker_delivered >= atomic_load(&worker_budget);
        pthread_mutex_unlock(&lock);
        if (exhausted) refuse("worker-wait callback budget exhausted");
        worker_hold(handle);
        pthread_mutex_lock(&lock);
        const bool busy = state.refused || (startup ? !state.inflight : state.inflight);
        if (!busy) { state.inflight = true; atomic_store(&worker_id,handle); }
        pthread_mutex_unlock(&lock);
        if (busy) refuse("worker-wait delivery raced another callback or a refusal");
    }
    pthread_mutex_lock(&lock);
    const uint32_t done = state.owner_delivered + state.worker_delivered_total;
    pthread_mutex_unlock(&lock);
    uint32_t device = UINT32_MAX, title_counter = UINT32_MAX;
    uint8_t pcr[4096];
    const uint32_t current_irql = kernel_sync_current_irql();
    const bool pcr_readable = kernel_guest_read_bytes(fs,pcr,sizeof(pcr));
    const uint32_t pcr_irql = pcr_readable ? pcr[0x24u] : UINT32_MAX;
    const bool device_readable = kernel_guest_read_u32(0x3E3F58u,&device);
    const bool counter_readable = title_counter_read(&title_counter);
    const uint32_t expected_title_counter = startup ? 1u : OWNER_WAIT_FIRST_COUNTER + done;
    if (current_irql != 0u || !pcr_readable || pcr_irql != 0u || !device_readable || device != DEVICE ||
        !counter_readable || title_counter != expected_title_counter) {
        if (startup && !worker)
            fprintf(stderr,"T3409 owner frame startup scope rejected: irql=%u pcr_irql=%u device_readable=%u device=0x%08x counter_readable=%u counter=%u expected_counter=%u\n",
                    (unsigned)current_irql,(unsigned)pcr_irql,(unsigned)device_readable,(unsigned)device,
                    (unsigned)counter_readable,(unsigned)title_counter,(unsigned)expected_title_counter);
        refuse(text->scope);
    }
    const uint32_t offsets[] = {0x1DB8u,0x1DE8u,0x1DE4u,0x1DECu,0x1DDCu,0x1D9Cu,0x1DA8u,0x2448u,0x244Cu};
    /* The waits applied include this one, the unbacked startup event adds its one count. */
    const uint32_t count = (uint32_t)d3d8_vblank_effects_applied() + 1u;
    const bool modelled = flip_modelled();
    const uint32_t expected[] = {0x22020u,count,flip_expected_consumer(modelled),0u,kernel_av_widescreen_claimed() ? 0x02490104u : 0x02480104u,0u,0u,0u,0u};
    for (unsigned i=0u;i<9u;i++) {
        uint32_t value;
        if (modelled && i == 3u) continue; /* T513: the threshold is what the flip queue left */
        if (!kernel_guest_read_u32(DEVICE+offsets[i],&value) || value != expected[i])
            refuse(text->bookkeeping);
    }
    d3d8_vblank_record record = {count,0u,0u};
    if (modelled) {
        const int flip_error = flip_queue_refusal(count,&record);
        if (flip_error >= 0) refuse(text->flips[flip_error]);
    }
    if (!worker) {
        /* T592: a worker held idle by this module cannot sample the counter either. */
        const uint32_t held = atomic_load(&held_id);
        (void)recomp_vblank_quiescence_check_parked(handle,0x3D3550u,&held,held != 0u ? 1u : 0u);
    }
    uint8_t frame[20];
    uint32_t fs_head;
    if (!kernel_guest_read_u32(fs,&fs_head) || !kernel_guest_write_u32(fs,fs_head) ||
        !kernel_guest_read_bytes(stack_high-20u,frame,sizeof(frame)) ||
        !kernel_guest_write_bytes(stack_high-20u,frame,sizeof(frame)) ||
        !kernel_guest_write_u32(DEVICE+0x1DE8u,count) || !title_counter_probe_write(title_counter))
        refuse(text->probes);
    const uint32_t payload[3] = {record.count,record.flip_index,record.flags};
    if (!invoke(0x22020u,stack_low,stack_high,payload)) refuse(text->invoke);
    uint32_t after;
    if (!title_counter_read(&after) || after != title_counter + 1u)
        refuse(text->advance);
    pthread_mutex_lock(&lock);
    const bool refused_during = state.refused;
    if (!refused_during) {
        if (startup) { state.second_delivered = true; state.second_via_worker = worker; }
        else if (worker) {
            state.worker_delivered++;
            state.worker_delivered_total++;
        }
        else state.owner_delivered++;
        state.inflight = false;
    }
    pthread_mutex_unlock(&lock);
    if (refused_during) refuse(text->during);
    if (worker && !gate_closed()) worker_hold_final(handle);
    if (!host_run_scope_pop(&scope)) abort();
}
/* T592: the owner's position tracked at every call boundary, see the header. Runs on every thread,
 * touches only atomics for any thread but the owner. */
static bool worker_epoch_active(void)
{
    const uint32_t handle = atomic_load(&worker_id);
    kernel_thread_record record;
    return handle != 0u && kernel_thread_get(handle,&record) && !record.terminated;
}
static bool running_flag_entry(uint32_t callee)
{
    return callee == RUNNING_STOP || callee == RUNNING_START || callee == RUNNING_START_BODY || callee == GATE;
}
/* T604: the proof of the gate region leaves the running flag writers owner only as a NAMED ASSUMPTION (the
 * entry barrier is open, T599). The claim is relied on while the owner sits in the gate or the worker's blanks
 * run, so a call of a writer entry by another thread in either state is a named stop. Outside both nothing
 * is parked on the flag and the call changes no verdict. */
static void check_running_flag_caller(uint32_t callee, uint32_t handle)
{
    const bool in_gate = atomic_load(&owner_location) != OWNER_ELSEWHERE;
    if (!in_gate && !worker_epoch_active()) return;
    static _Thread_local char text[320];
    kernel_thread_record self;
    const uint32_t start = kernel_thread_get(handle,&self) ? self.start_routine : 0u;
    snprintf(text,sizeof text,"thread 0x%x (start 0x%x) called 0x%x, a running flag writer entry, while %s: "
             "the owner is not the only writer of [0x74A9AC] (T604)",(unsigned)handle,(unsigned)start,
             (unsigned)callee,in_gate ? "the owner is in the loading bar gate" : "the worker's blanks are delivered");
    refuse(text);
}
void recomp_second_vblank_note_call(uint32_t callee, uint32_t handle)
{
    if (handle == 0u) return;
    const bool budgeted_worker = atomic_load(&worker_budget) != 0u;
    if (!budgeted_worker) {
        pthread_mutex_lock(&lock);
        const bool startup_clock = startup_clock_enabled;
        pthread_mutex_unlock(&lock);
        if (!startup_clock) return;
    }
    if (handle != atomic_load(&owner_id)) {
        /* Startup-clock worker waits are admitted from the completed wait
         * seam, where worker_hold proves the owner is parked. T592's getter
         * epoch guards apply only to its separately budgeted model. */
        if (!budgeted_worker) return;
        if (running_flag_entry(callee)) check_running_flag_caller(callee,handle);
        /* The worker's own getter poll, after the credited second event and not for the producer. */
        if (callee != GETTER) return;
        pthread_mutex_lock(&lock);
        const bool armed = state.second_delivered && !state.refused && handle != state.producer_handle;
        pthread_mutex_unlock(&lock);
        if (armed && worker_record_identity(handle)) {
            worker_hold(handle);
            /* The worker tests the stop flag after a step, never in its getter loop, and the owner stores it
             * before it blocks on the worker. A poll with the owner already blocked is that test lost to a
             * frame the title skipped, and no blank would ever come: a named stop instead of a hang. */
            if (owner_blocked_on(handle))
                refuse("worker polled the vblank counter after the owner blocked on its termination, no blank is owed (T592)");
        }
        return;
    }
    if (callee == GATE) {
        uint32_t flag = 1u, running = 0u;
        /* The two entry conditions of the proof: the gate is not passed yet and the bar is running (the
         * stop function returns at once otherwise). Both have owner-only writers. */
        const bool entered = kernel_guest_read_u32(GATE_FLAG,&flag) && flag == 0u &&
                             kernel_guest_read_u32(GATE_RUNNING,&running) && running != 0u;
        if (entered) {
            pthread_mutex_lock(&lock);
            state.owner_gate_entries++;
            /* The worker callback limit bounds one loading-bar episode. The title can enter
             * this gate repeatedly during a single run; carrying the spent episode budget into
             * the next gate would stop a later, otherwise finite progress loop. Keep the
             * run-wide delivery total separately for callback/device accounting. */
            state.worker_delivered = 0u;
            pthread_mutex_unlock(&lock);
        }
        atomic_store(&owner_location,entered ? OWNER_GATE : OWNER_ELSEWHERE);
        return;
    }
    const unsigned location = atomic_load(&owner_location);
    if (location == OWNER_GATE) {
        if (callee == GATE_WAIT) { atomic_store(&owner_location,OWNER_GATE_WAIT); return; }
        atomic_store(&owner_location,OWNER_ELSEWHERE);
        if (budgeted_worker && worker_epoch_active())
            refuse("owner left the loading bar gate through a call outside the proof (T592)");
        return;
    }
    if (location == OWNER_GATE_WAIT) atomic_store(&owner_location,OWNER_ELSEWHERE);
    if (budgeted_worker && callee == GETTER && worker_epoch_active())
        refuse("owner sampled the vblank counter while the worker's blanks were delivered (T592)");
}
/* Interactive startup can reach the owner's terminal worker wait before its second FRAME. The
 * worker then polls counter1 forever. At the exact getter boundary, either deliver its completed
 * wait credit or (when the T3409 switch is selected and there is no credit) wait one Rust-owned
 * display period, apply that blank's effects, and deliver the callback. The owner must be blocked
 * on this worker, so the worker is the only counter reader during the callback. */
static bool startup_worker_poll(uint32_t handle, uint32_t fs, uint32_t esp, uint32_t irql)
{
    pthread_mutex_lock(&lock);
    const bool clock_enabled = startup_clock_enabled;
    const bool no_worker_budget = atomic_load(&worker_budget) == 0u;
    const bool startup_clock_path = clock_enabled && no_worker_budget;
    const bool has_credit = state.credits == 1u;
    const bool producer_matches = !has_credit ||
        (handle == state.producer_handle && fs == state.producer_fs && pthread_equal(producer,pthread_self()));
    const bool candidate = interactive && (startup_clock_path || owner_budget != 0u) && (clock_enabled || !no_worker_budget) &&
        state.enabled && state.bound && state.registered &&
        state.first_delivered && !state.second_attempted && !state.second_delivered &&
        !state.inflight && !state.refused && state.credits <= 1u && (has_credit || clock_enabled) &&
        producer_matches && handle != state.owner_handle;
    pthread_mutex_unlock(&lock);
    if (!candidate || !worker_identity(handle,fs)) return false;
    /* Without the startup clock, check the terminal scope only once the owner is blocked on this
     * worker. Early worker polls can precede the loading-bar state (owner playtest 2026-10-21,
     * stop=0 gate=0). */
    if (!clock_enabled && !owner_blocked_on(handle)) return false;
    /* Startup captures have observed the loading-bar running word as both 0 and 1
     * while the stop and gate words remain 0. Accept only its boolean states here;
     * the budgeted T592 path keeps its measured terminal state. */
    uint32_t caller = 0u, counter = 0u, previous = 0u, stop = 0u, running = 0u, gate = 0u;
    const char *scope_failure = NULL;
    if (irql != 0u) scope_failure = "irql";
    else if (!kernel_guest_read_u32(esp,&caller) || caller != 0x156CB6u) scope_failure = "caller";
    else if (!title_counter_read(&counter) || counter != 1u) scope_failure = "counter";
    else if (!kernel_guest_read_u32(0x7497D8u,&previous) ||
             (startup_clock_path ? (previous != 0u && previous != counter) : previous != counter)) scope_failure = "previous";
    else if (!kernel_guest_read_u32(0x74A9B0u,&stop) || stop != (startup_clock_path ? 0u : 1u)) scope_failure = "stop";
    else if (!kernel_guest_read_u32(GATE_RUNNING,&running) ||
             (startup_clock_path ? running > 1u : running != 0u)) scope_failure = "gate-running";
    else if (!kernel_guest_read_u32(GATE_FLAG,&gate) || gate != (startup_clock_path ? 0u : 1u)) scope_failure = "gate-flag";
    if (scope_failure != NULL) {
        fprintf(stderr,"T3409 startup worker scope rejected at %s: irql=%u caller=0x%08x counter=%u previous=%u stop=%u running=%u gate=%u\n",
                scope_failure,(unsigned)irql,(unsigned)caller,(unsigned)counter,(unsigned)previous,
                (unsigned)stop,(unsigned)running,(unsigned)gate);
        refuse("interactive startup worker caller/counter/termination scope refused");
    }
    const uint64_t bytes = (uint64_t)stack_high-stack_low;
    if (overlap(stack_low,bytes,DEVICE,0x2450u) || overlap(stack_low,bytes,fs,4096u) ||
        overlap(stack_low,bytes,esp,4u) || overlap(stack_low,bytes,COUNTER,4u) ||
        overlap(fs,4096u,DEVICE,0x2450u) || overlap(fs,4096u,COUNTER,4u) ||
        overlap(esp,4u,DEVICE,0x2450u) || overlap(esp,4u,fs,4096u))
        refuse("interactive startup worker aliased callback context");
    if (!owner_blocked_on(handle)) {
        if (!clock_enabled) return false;
        await_startup_owner_block(handle);
    }
    const bool owner_still_blocked = owner_blocked_on(handle);
    pthread_mutex_lock(&lock);
    const bool clock_path = startup_clock_enabled && atomic_load(&worker_budget) == 0u;
    const bool budgeted_path = owner_budget != 0u && atomic_load(&worker_budget) != 0u;
    const bool raced = !interactive || !(clock_path || budgeted_path) ||
        !state.enabled || !state.bound || !state.registered ||
        !state.first_delivered || state.second_delivered || state.inflight || state.refused ||
        state.second_attempted || state.credits != (has_credit ? 1u : 0u) || handle == state.owner_handle ||
        !owner_still_blocked ||
        (has_credit && (handle != state.producer_handle || fs != state.producer_fs || !pthread_equal(producer,pthread_self())));
    if (!raced) { state.inflight = true; state.second_attempted = true; state.credits = 0u; }
    pthread_mutex_unlock(&lock);
    if (raced) refuse("interactive startup worker credit reservation raced");
    if (!has_credit) {
        if (!wait_for_startup_display_period()) {
            finish();
            refuse("Rust startup display clock refused its one-shot deadline");
        }
        if (!d3d8_gpu_model_blank()) {
            finish();
            refuse("Rust startup display clock blank effects refused");
        }
        fprintf(stderr,"T3409 startup worker: one default display-clock period completed at the terminal getter\n");
    } else {
        fprintf(stderr,"T3409 startup worker: delivered the producer's completed wait credit at the terminal getter\n");
    }
    deliver_blank(handle,fs,true,true);
    return true;
}
/* After the loading-worker handoff, the owner's frame wait can spin at the
 * same counter getter. Give that exact, measured caller a periodic display
 * deadline when T3409 is selected, instead of making poll count the clock.
 * Other uses of 0x22030 remain passive. */
static void startup_owner_frame_poll(uint32_t handle, uint32_t fs, uint32_t esp, uint32_t caller_override,
                                     uint32_t irql)
{
    if (atomic_load(&worker_budget) != 0u || handle == 0u || handle != atomic_load(&owner_id)) return;
    uint32_t caller = caller_override;
    if ((caller == 0u && !kernel_guest_read_u32(esp,&caller)) || caller != 0x153975u) return;
    pthread_mutex_lock(&lock);
    const bool eligible = startup_clock_enabled && atomic_load(&worker_budget) == 0u && interactive && state.enabled && state.bound && state.registered &&
        !state.refused && !state.inflight && handle == state.owner_handle && fs == state.owner_fs &&
        (state.second_delivered || (state.first_delivered && !state.second_attempted && state.credits == 0u));
    const bool startup_delivery = eligible && !state.second_delivered;
    pthread_mutex_unlock(&lock);
    if (!eligible) return;
    if (irql != 0u) refuse("T3409 owner frame clock requires IRQL 0");
    uint32_t counter = 0u, last = 0u;
    if (!title_counter_read(&counter) || !frame_last_exit_read(&last))
        refuse("T3409 owner frame clock counter state unreadable");
    if (counter != last) return;
    pthread_mutex_lock(&lock);
    const bool raced = !interactive || !startup_clock_enabled || atomic_load(&worker_budget) != 0u ||
        !state.enabled || !state.bound || !state.registered || state.refused ||
        (startup_delivery ? (!state.first_delivered || state.second_delivered || state.second_attempted || state.credits != 0u) : !state.second_delivered) ||
        state.inflight || handle != state.owner_handle || fs != state.owner_fs;
    if (!raced) {
        state.inflight = true;
        if (startup_delivery) state.second_attempted = true;
    }
    pthread_mutex_unlock(&lock);
    if (raced) return;
    if (!wait_for_owner_display_period() || !d3d8_gpu_model_blank()) {
        finish();
        refuse("Rust owner display clock refused its frame-wait period");
    }
    if (!atomic_exchange(&owner_clock_announced,true))
        fprintf(stderr,"T3409 Rust display clock: periodic blank delivered at the owner frame-wait getter\n");
    deliver_blank(handle,fs,false,startup_delivery);
}
/* T1351: after the first blank the producer thread registers its one credit at its own frame wait (the else-if branch of the
 * getter hook above). The owner can poll the frame wait before the producer thread ran that far: under heavy host load the
 * producer is merely late, and the old code refused with "second-event credit/attempt refused" at startup (1 in 15 runs at
 * 160 busy loops on 32 cores, census 883). The owner now waits, bounded in real time like worker_hold, for the credit. A
 * credit that never arrives still ends in the same named stop below. */
static void await_second_credit(void)
{
    const uint64_t began = now_milliseconds();
    for (;;) {
        pthread_mutex_lock(&lock);
        const bool clocked_startup_frame = startup_clock_enabled && interactive && atomic_load(&worker_budget) == 0u &&
            state.enabled && state.bound && state.registered && state.first_delivered && !state.second_delivered &&
            !state.second_attempted && !state.refused && !state.inflight && state.credits == 0u && pthread_equal(owner,pthread_self());
        const bool late = state.enabled && state.bound && state.registered && state.first_delivered && !state.second_delivered &&
            !state.second_attempted && !state.refused && !state.inflight && state.credits == 0u && pthread_equal(owner,pthread_self());
        pthread_mutex_unlock(&lock);
        if (clocked_startup_frame || !late || now_milliseconds() - began >= 10000u) return;
        if (shutdown_query != NULL && shutdown_query())
            host_run_stop(HOST_STOP_HOST_SHUTDOWN, FRAME, 0u, "interactive host shutdown while waiting for the second-event credit");
        idle_briefly();
    }
}
void recomp_second_vblank_poll_with_caller(uint32_t callee, uint32_t handle, uint32_t fs, uint32_t esp,
                                           uint32_t irql, uint32_t caller_override)
{
    if (callee == GETTER) {
        if (!startup_worker_poll(handle,fs,esp,irql))
            startup_owner_frame_poll(handle,fs,esp,caller_override,irql);
        return;
    }
    const char *error = NULL;
    bool first = false, admitted = false, poll_blank = false;
    uint32_t ph = 0u, pf = 0u;
    if (callee != FRAME) return; /* T1289: answered without the lock, the locked test below returns for it as well */
    await_second_credit();
    pthread_mutex_lock(&lock);
    if (!state.enabled || callee != FRAME) { pthread_mutex_unlock(&lock); return; }
    if (state.refused || !state.bound || !state.registered || state.inflight || handle != state.owner_handle ||
        fs != state.owner_fs || !pthread_equal(owner,pthread_self())) error = "two-event owner/registration/reentry refused";
    else if (!state.first_delivered) first = true;
    else if (startup_clock_enabled && interactive && atomic_load(&worker_budget) == 0u &&
             !state.second_delivered && !state.second_attempted && state.credits == 0u) {
        /* Let the owner reach its measured frame-counter getter. The Rust
         * display deadline supplies this startup event there, before the
         * loading worker exists to produce a completed-wait credit. */
        state.frames_admitted++;
        admitted = true;
    }
    else if (state.second_delivered &&
             (owner_budget != 0u || (startup_clock_enabled && atomic_load(&worker_budget) == 0u))) {
        /* T549: a later frame wait. Its own exit test (0x153985..0x15398F) is counter - [0x7A58B0] >= 1,
         * where [0x7A58B0] is the counter its previous exit stored. When that already holds the title
         * runs through without waiting. If it does not, only the explicit poll model or T3409's clocked
         * exact-getter continuation may provide the next blank. */
        uint32_t counter = 0u, last = 0u;
        if (!title_counter_read(&counter) || !frame_last_exit_read(&last))
            error = "frame-wait counter or last exit unreadable";
        else if (counter == last && poll_blank_enabled && (interactive || state.owner_delivered < owner_budget)) {
            /* T696: the exit test fails by exactly one blank. Deliver it here, from the poll, as the
             * owner-wait blank (same budget). FABRICATED timing: the title's own poll stands in for
             * the 60 Hz interrupt. The counter and [0x7A58B0] are never written by this module. */
            state.inflight = true; poll_blank = true;
        }
        else if (counter == last && startup_clock_enabled && atomic_load(&worker_budget) == 0u) {
            /* T3409: the exact getter below waits for a Rust-owned display
             * period. This is a clock event, not a callback per guest poll. */
            state.frames_admitted++; admitted = true;
        }
        else if ((int32_t)(counter - last) < 1) {
            /* T695: say which numbers made it spin (the stop's detail is all a later reader has). */
            static _Thread_local char text[160];
            snprintf(text,sizeof text,"frame wait would spin for a blank nobody delivers (counter %u, last exit %u, "
                     "owner blanks delivered %u of %u)",(unsigned)counter,(unsigned)last,(unsigned)state.owner_delivered,
                     (unsigned)owner_budget);
            error = text;
        }
        else { state.frames_admitted++; admitted = true; }
    }
    else if (state.second_attempted || state.credits != 1u || pthread_equal(owner,producer)) error = "second-event credit/attempt refused";
    if (!error && admitted) { pthread_mutex_unlock(&lock); return; }
    if (!error && poll_blank) {
        pthread_mutex_unlock(&lock);
        /* The refusals that must precede the blank's own device effects. */
        if (!recomp_vblank_quiescence_enabled()) { finish(); refuse(owner_texts.quiescence_off); }
        if (!d3d8_vblank_effects_enabled()) { finish(); refuse(owner_texts.coupling_off); }
        if (!d3d8_gpu_model_blank()) { finish(); refuse("poll blank: the clock or the helper effects refused the blank"); }
        deliver_blank(handle,fs,false,false);
        pthread_mutex_lock(&lock);
        state.poll_delivered++;
        pthread_mutex_unlock(&lock);
        return;
    }
    if (!error) { state.inflight = true; ph = state.producer_handle; pf = state.producer_fs;
        if (!first) { state.second_attempted = true; state.credits = 0u; } }
    pthread_mutex_unlock(&lock);
    if (error) refuse(error);
    if (!host_run_armed()) { finish(); abort(); }
    volatile host_run_scope scope = HOST_RUN_SCOPE_INITIALIZER;
    if (!host_run_scope_init(&scope)) { finish(); abort(); }
    if (sigsetjmp(*host_run_scope_jmp(&scope),0) != 0) {
        const host_stop stopped = *host_run_result();
        finish(); if (!host_run_scope_pop(&scope)) abort(); host_run_rethrow(&stopped);
    }
    if (!host_run_scope_push(&scope)) { finish(); refuse("two-event stop scope unavailable"); }
    if (first) {
        d3d8_first_vblank_poll(callee, fs, esp, irql);
        d3d8_first_vblank_snapshot previous;
        d3d8_first_vblank_get_snapshot(&previous);
        if (!previous.delivered) refuse("first callback did not complete");
        pthread_mutex_lock(&lock);
        bool poisoned = state.refused;
        if (!poisoned) { state.first_delivered = true; state.credits = 0u; state.inflight = false; }
        pthread_mutex_unlock(&lock);
        if (poisoned) refuse("wait/refill during first callback refused");
    } else {
        if (!confirm(ph,pf)) refuse("second-event producer lacks confirmed termination");
        uint32_t caller, device, title_counter;
        uint8_t pcr[4096];
        if (!kernel_guest_read_u32(esp,&caller) || caller != 0x3D66Du || irql != 0u ||
            kernel_sync_current_irql() != irql || !kernel_guest_read_bytes(fs,pcr,sizeof(pcr)) || pcr[0x24u] != 0u ||
            !kernel_guest_read_u32(0x3E3F58u,&device) || device != DEVICE ||
            !title_counter_read(&title_counter) || title_counter != 1u)
            refuse("second-event caller/PCR/device scope refused");
        const uint32_t offsets[] = {0x1DB8u,0x1DE8u,0x1DE4u,0x1DECu,0x1DDCu,0x1D9Cu,0x1DA8u,0x2448u,0x244Cu};
        /* T372 coupling: the waits applied (the credit's own backing wait included) plus the
         * unbacked startup event's one count. Off, the measured 2. */
        const bool coupled = d3d8_vblank_effects_enabled();
        const uint32_t second_count = coupled ? (uint32_t)d3d8_vblank_effects_applied() + 1u : 2u;
        const bool modelled = flip_modelled();
        const uint32_t expected[] = {0x22020u,second_count,flip_expected_consumer(modelled),0u,kernel_av_widescreen_claimed() ? 0x02490104u : 0x02480104u,0u,0u,0u,0u};
        for (unsigned i=0u;i<9u;i++) {
            uint32_t value;
            if (modelled && i == 3u) continue; /* T513: the threshold is what the flip queue left */
            if (!kernel_guest_read_u32(DEVICE+offsets[i],&value) || value != expected[i])
                refuse("second-event nonempty or changed device bookkeeping");
        }
        d3d8_vblank_record record = {second_count,0u,0u};
        if (modelled) {
            const int flip_error = flip_queue_refusal(second_count,&record);
            if (flip_error >= 0) refuse(second_flip_reasons[flip_error]);
        }
        uint64_t bytes = (uint64_t)stack_high-stack_low;
        if (overlap(stack_low,bytes,DEVICE,0x2450u) || overlap(stack_low,bytes,0x3E3F58u,4u) ||
            overlap(stack_low,bytes,fs,4096u) || overlap(stack_low,bytes,esp,4u) ||
            overlap(fs,4096u,DEVICE,0x2450u) || overlap(esp,4u,DEVICE,0x2450u) || overlap(esp,4u,fs,4096u) ||
            overlap(stack_low,bytes,0x563918u,4u) || overlap(fs,4096u,0x563918u,4u) ||
            overlap(esp,4u,0x563918u,4u))
            refuse("second-event aliased callback context");
        uint8_t frame[20];
        uint32_t fs_head;
        if (!kernel_guest_read_u32(fs,&fs_head) || !kernel_guest_write_u32(fs,fs_head) ||
            !kernel_guest_read_bytes(stack_high-20u,frame,sizeof(frame)) ||
            !kernel_guest_write_bytes(stack_high-20u,frame,sizeof(frame)) ||
            !kernel_guest_write_u32(DEVICE+0x1DE8u,second_count) ||
            !title_counter_probe_write(title_counter)) refuse("second-event write probes refused");
        pthread_mutex_lock(&lock);
        bool poisoned = state.refused;
        /* Coupled, the backing wait already published the count: this event only reads it. */
        const uint32_t published_count = coupled ? second_count : 3u;
        bool published = !poisoned && kernel_guest_write_u32(DEVICE+0x1DE8u,published_count);
        pthread_mutex_unlock(&lock);
        if (poisoned) refuse("second-event concurrent refill refused");
        if (!published) refuse("second-event count publication refused");
        const uint32_t payload[3] = {published_count,record.flip_index,record.flags};
        if (!invoke(0x22020u,stack_low,stack_high,payload)) refuse("second callback invocation failed after publication");
        pthread_mutex_lock(&lock);
        poisoned = state.refused;
        if (!poisoned) { state.second_delivered = true; state.inflight = false; }
        pthread_mutex_unlock(&lock);
        if (poisoned) refuse("second-event refill during callback refused");
    }
    if (!host_run_scope_pop(&scope)) abort();
}
void recomp_second_vblank_poll(uint32_t callee, uint32_t handle, uint32_t fs, uint32_t esp, uint32_t irql)
{
    recomp_second_vblank_poll_with_caller(callee,handle,fs,esp,irql,0u);
}
