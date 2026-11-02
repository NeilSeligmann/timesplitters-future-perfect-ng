/* SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdio.h>
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <setjmp.h>
#include "host_timers.h"
#include "guest_mem.h"
#include "kernel_call.h"
#include "kernel_thread.h"
#include "kernel_sync.h"
#include "kernel_clock.h"
#include "kernel_vtime.h"
#include "kernel_detsched.h"
#include "kernel_av.h"
#include "kmem_abi.h"
#include "nt_status.h"
#include "recomp_callback.h"
#include "recomp_second_vblank.h"
#include "recomp_vblank_quiescence.h"
#include "d3d8_first_vblank.h"
#include "d3d8_callbacks.h"
#include "d3d8_gpu.h"
#include "d3d8_device.h"
#include "d3d8_guest.h"
#include "d3d8_flip.h"
#include "d3d8_vblank_effects.h"
#include "tsfp_timers_host.h"
#include "guest_frame_trace.h"
#include "host_runtime.h"

extern const char *tsfp_vblank_quiescence_state_name(uint32_t state);
extern bool tsfp_vblank_quiescence_is_registered_reader(uint32_t return_address, uint32_t start_routine);
typedef struct {
    uint32_t handle, start_routine, block_target, block_state, suspend_count;
    bool in_use, terminated, finished, started;
} main_vblank_thread_view;
extern uint32_t tsfp_vblank_quiescence_classify(const main_vblank_thread_view *thread);
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
typedef struct {
    bool holds;
    uint32_t reason, handle, start_routine, examined;
} main_vblank_verdict;
extern void tsfp_vblank_quiescence_evaluate(const main_vblank_thread_view *threads, uint32_t count,
    uint32_t deliverer, const uint32_t *non_readers, uint32_t non_reader_count,
    const uint32_t *parked, uint32_t parked_count, main_vblank_verdict *out);
extern const uint32_t *tsfp_vblank_quiescence_default_non_readers(uint32_t *count);
extern const char *tsfp_vblank_quiescence_reason_name(uint32_t reason);
extern void tsfp_second_vblank_quiescence_configure(bool enabled);
extern bool tsfp_second_vblank_quiescence_enabled(void);
extern uint32_t tsfp_second_vblank_quiescence_getter_checks(void);
extern bool tsfp_second_vblank_quiescence_check_getter(uint32_t, uint32_t,
                                                       bool (*)(uint32_t, uint32_t));
extern void tsfp_second_vblank_set_shutdown_query(bool (*query)(void));
extern bool tsfp_second_vblank_startup_set_interactive(bool enabled);
extern bool tsfp_second_vblank_startup_set_clock(bool enabled);
extern bool tsfp_second_vblank_startup_set_owner_waits(uint32_t budget);
extern bool tsfp_second_vblank_startup_set_worker_blanks(uint32_t budget);
extern bool tsfp_second_vblank_startup_set_poll_blank(bool enabled);
extern void tsfp_second_vblank_policy_set_worker_hold_ms(uint32_t milliseconds);
extern recomp_second_vblank_snapshot tsfp_second_vblank_snapshot(void);
extern void tsfp_second_vblank_host_lock(void);
extern void tsfp_second_vblank_host_unlock(void);
extern bool tsfp_second_vblank_is_owner_thread(void);
extern bool tsfp_vblank_enabled_get(void);
extern bool tsfp_vblank_bound_get(void);
extern uint32_t tsfp_second_vblank_host_registration_begin(uint32_t, uint32_t, uint32_t, bool);
extern void tsfp_second_vblank_host_registration_finish(bool);
extern uint32_t tsfp_second_vblank_bind_host_owner(uint32_t, uint32_t, uint32_t, uint32_t);
extern bool tsfp_vblank_inflight_get(void);
extern bool tsfp_second_vblank_host_reset(void);
extern void tsfp_startup_vblank_clock_reset(void);
extern bool tsfp_second_vblank_host_configure(bool, d3d8_first_vblank_invoker,
    recomp_second_vblank_producer_confirmed, uint32_t, uint32_t);
extern uint32_t tsfp_second_vblank_worker_budget_get(void);
extern bool tsfp_vblank_startup_clock_get(void);
extern uint32_t tsfp_vblank_owner_id_get(void);
extern uint32_t tsfp_second_vblank_call_role(uint32_t, uint32_t, bool, bool);
extern void tsfp_second_vblank_host_observe_gate(bool (*read)(uint32_t, uint32_t *));
extern uint32_t tsfp_vblank_worker_id_get(void);
extern uint32_t tsfp_second_vblank_host_owner_call(uint32_t, bool, bool (*)(void));
extern bool tsfp_second_vblank_worker_record_matches(bool, bool, bool, uint32_t, uint32_t, uint32_t);
extern bool tsfp_second_vblank_producer_record_matches(bool, bool, bool, bool, bool,
    uint32_t, uint32_t, uint32_t, uint32_t, uint32_t);
extern bool tsfp_second_vblank_lookup_worker_identity(uint32_t, uint32_t, bool (*)(uint32_t, uint32_t *));
extern uint32_t tsfp_second_vblank_host_wait_completed(bool, uint32_t, uint32_t, bool, bool);
static void second_vblank_deliver_blank(uint32_t, uint32_t, bool, bool);
extern bool tsfp_second_vblank_host_complete_original(bool);
extern bool tsfp_second_vblank_host_worker_exhausted(void);
extern bool tsfp_second_vblank_host_reserve_worker(uint32_t, bool);
extern uint32_t tsfp_second_vblank_host_expected_counter(bool);
extern bool tsfp_second_vblank_delivery_scope_valid(uint32_t, bool, uint32_t, bool, uint32_t, uint32_t, bool, uint32_t, uint32_t);
extern bool tsfp_second_vblank_delivery_advanced(bool, uint32_t, uint32_t);
extern bool tsfp_second_vblank_host_complete_delivery(bool, bool);
extern int32_t tsfp_second_vblank_delivery_preflight(uint32_t, bool, void *, bool (*)(uint32_t, bool), int32_t (*)(uint32_t, void *));
extern uint32_t tsfp_vblank_held_id_get(void);
extern uint32_t tsfp_vblank_worker_final_held_get(void);
extern void tsfp_vblank_worker_final_held_set(uint32_t);
extern bool tsfp_second_vblank_gate_closed(uint32_t, uint32_t);
extern bool tsfp_vblank_confirm(uint32_t, uint32_t);
extern bool tsfp_second_vblank_second_scope_observation(uint32_t, uint32_t);
extern bool tsfp_second_vblank_read_second_scope(uint32_t, uint32_t, uint32_t, uint32_t *,
    bool (*)(uint32_t, uint32_t *), uint32_t (*)(void),
    bool (*)(uint32_t, uint32_t *), bool (*)(uint32_t *));
extern bool tsfp_second_vblank_delivery_probes(uint32_t, uint32_t, uint32_t, uint32_t,
    bool (*)(uint32_t, uint32_t *), bool (*)(uint32_t, uint32_t), bool (*)(uint32_t), bool (*)(uint32_t));
extern uint32_t tsfp_second_vblank_publish_second(bool, bool, uint32_t, bool (*)(uint32_t, uint32_t));
extern bool tsfp_second_vblank_running_flag_entry(uint32_t);
extern uint32_t tsfp_vblank_owner_location_get(void);
extern bool tsfp_second_vblank_running_flag_refusal(uint32_t, bool, bool);
extern bool tsfp_second_vblank_host_worker_getter_armed(uint32_t);
extern bool tsfp_second_vblank_host_startup_candidate(uint32_t, bool);
extern bool tsfp_second_vblank_host_startup_reserve(uint32_t, bool, bool, bool);
extern uint32_t tsfp_second_vblank_startup_scope_read(uint32_t, uint32_t, bool, uint32_t *,
    bool (*)(uint32_t, uint32_t *), bool (*)(uint32_t *));
extern bool tsfp_second_vblank_is_producer_thread(void);
extern uint32_t tsfp_vblank_credits_get(void);
extern uint32_t tsfp_vblank_producer_handle_get(void);
extern uint32_t tsfp_vblank_producer_fs_get(void);
typedef struct {
    uint64_t (*now)(void);
    bool (*owner_blocked)(uint32_t);
    bool (*shutdown)(void);
    void (*idle)(void);
} main_startup_owner_wait_host;
extern uint32_t tsfp_second_vblank_wait_startup_owner(uint32_t, uint64_t, main_startup_owner_wait_host);
typedef struct {
    bool (*shutdown)(void);
    bool (*virtual_time)(void);
    bool (*now)(bool, uint64_t *);
    uint32_t (*mode)(void);
    uint32_t (*poll)(uint64_t, uint32_t, uint64_t *);
    bool (*wait)(bool, uint64_t, uint64_t);
} main_display_wait_host;
extern bool tsfp_second_vblank_wait_display_period(main_display_wait_host);
extern bool tsfp_second_vblank_wait_owner_display_period(void (*)(void), bool (*)(void));
extern uint32_t tsfp_startup_vblank_clock_poll(uint64_t, uint32_t, uint64_t *);
extern uint32_t tsfp_second_vblank_host_owner_clock_candidate(uint32_t, uint32_t);
extern bool tsfp_second_vblank_host_owner_clock_reserve(uint32_t, uint32_t, bool);
extern bool tsfp_second_vblank_mark_clock_announced(void);
typedef struct { uint32_t action, producer_handle, producer_fs; } main_frame_result;
extern bool tsfp_second_vblank_host_await_credit(bool);
extern bool tsfp_second_vblank_shutdown_requested(void);
extern bool tsfp_frame_state_native_selected(void) __attribute__((weak));
extern uint32_t tsfp_frame_state_vblank_counter_read(void) __attribute__((weak));
extern uint32_t tsfp_frame_state_timing_history_snapshot(uint32_t) __attribute__((weak));
static bool second_vblank_counter_native(void)
{
    return tsfp_frame_state_native_selected && tsfp_frame_state_native_selected();
}
static bool tsfp_second_vblank_host_title_counter_read(uint32_t *counter)
{
    if (!counter) return false;
    if (!second_vblank_counter_native()) return kernel_guest_read_u32(0x563918u, counter);
    if (!tsfp_frame_state_vblank_counter_read) return false;
    *counter = tsfp_frame_state_vblank_counter_read();
    return true;
}
static bool tsfp_second_vblank_host_frame_last_exit_read(uint32_t *last)
{
    if (!last) return false;
    if (!second_vblank_counter_native()) return kernel_guest_read_u32(0x7A58B0u, last);
    if (!tsfp_frame_state_timing_history_snapshot) return false;
    *last = tsfp_frame_state_timing_history_snapshot(4u);
    return true;
}
extern bool tsfp_second_vblank_owner_is_producer(void);
extern bool tsfp_second_vblank_host_frame_needs_counters(uint32_t, uint32_t, bool);
extern main_frame_result tsfp_second_vblank_host_frame(uint32_t, uint32_t, bool, bool, bool, uint32_t, uint32_t);
extern uint32_t tsfp_vblank_owner_delivered_get(void);
extern uint32_t tsfp_vblank_owner_budget_get(void);
extern void tsfp_vblank_inflight_set(bool);
extern void tsfp_vblank_refused_set(bool);
extern uint32_t tsfp_vblank_poll_delivered_get(void);
extern void tsfp_vblank_poll_delivered_set(uint32_t);
extern uint32_t tsfp_vblank_stack_low_get(void);
extern uint32_t tsfp_vblank_stack_high_get(void);
extern bool tsfp_vblank_refused_get(void);
extern bool tsfp_vblank_invoke(uint32_t, uint32_t, uint32_t, const uint32_t *);
extern bool tsfp_second_vblank_context_aliased(uint32_t, uint32_t, uint32_t, uint32_t, bool);
static bool tsfp_second_vblank_host_title_counter_probe_write(uint32_t counter)
{
    return second_vblank_counter_native() || kernel_guest_write_u32(0x563918u, counter);
}
extern bool tsfp_second_vblank_bookkeeping_matches(uint32_t, uint32_t, uint32_t, uint32_t, bool);
extern bool tsfp_second_vblank_bookkeeping_valid(uint32_t, uint32_t, bool, bool, bool (*)(uint32_t, uint32_t *));
extern int32_t tsfp_second_vblank_check_flip_queue(uint64_t, uint64_t, uint32_t, uint32_t,
                                                    uint32_t, uint32_t, uint32_t, uint32_t);
extern int32_t tsfp_second_vblank_check_flip_queue_state(uint64_t, uint64_t, uint32_t, uint32_t);
extern int32_t tsfp_second_vblank_check_flip_queue_host(uint64_t, uint64_t, uint32_t, void *,
    bool (*)(uint32_t, uint32_t *), int32_t (*)(void *, uint32_t, uint32_t, uint32_t));
extern int32_t tsfp_second_vblank_check_flip_record(uint32_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t);
static int32_t second_vblank_check_flip_record(void *out, uint32_t count, uint32_t producer, uint32_t threshold)
{
    d3d8_vblank_record *record = out;
    *record = d3d8_vblank_effects_last_record();
    return tsfp_second_vblank_check_flip_record(count, producer, threshold,
                                                record->count, record->flip_index, record->flags);
}
static int flip_queue_refusal(uint32_t count, d3d8_vblank_record *record)
{
    const d3d8_flip_hardware hardware = d3d8_flip_hardware_get();
    return tsfp_second_vblank_check_flip_queue_host(hardware.queued, hardware.flips, count, record,
        kernel_guest_read_u32, second_vblank_check_flip_record);
}
#else
#define tsfp_second_vblank_quiescence_configure recomp_vblank_quiescence_configure
#define tsfp_second_vblank_quiescence_enabled recomp_vblank_quiescence_enabled
#define tsfp_second_vblank_quiescence_getter_checks recomp_vblank_quiescence_getter_checks
#define tsfp_second_vblank_set_shutdown_query recomp_second_vblank_set_shutdown_query
#define tsfp_second_vblank_startup_set_interactive recomp_second_vblank_set_interactive
#define tsfp_second_vblank_startup_set_clock recomp_second_vblank_set_startup_clock
#define tsfp_second_vblank_startup_set_owner_waits recomp_second_vblank_set_owner_waits
#define tsfp_second_vblank_startup_set_worker_blanks recomp_second_vblank_set_worker_blanks
#define tsfp_second_vblank_startup_set_poll_blank recomp_second_vblank_set_poll_blank
#define tsfp_second_vblank_policy_set_worker_hold_ms recomp_second_vblank_set_worker_hold_ms
#endif

/* Only the opt-in startup callback uses this exclusive private guest region. */
#define FIRST_VBLANK_STACK_BYTES 0x10000u
static kernel_guest_ptr first_vblank_stack;
bool second_vblank_policy;
TSFP_RECOMP_TLS uint32_t callback_thread_handle;
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
static bool second_vblank_bookkeeping_valid(uint32_t count, bool modelled)
{
    const uint32_t consumer = modelled ? (uint32_t)d3d8_flip_hardware_get().flips : 0u;
    const bool widescreen = kernel_av_widescreen_claimed();
    return tsfp_second_vblank_bookkeeping_valid(count, consumer, widescreen, modelled, kernel_guest_read_u32);
}
static uint32_t second_scope_current_irql(void) { return kernel_sync_current_irql(); }
static bool second_scope_pcr_irql(uint32_t fs, uint32_t *value)
{
    uint8_t pcr[4096];
    if (!kernel_guest_read_bytes(fs, pcr, sizeof(pcr))) return false;
    *value = pcr[0x24u];
    return true;
}
static bool second_vblank_probe_frame(uint32_t address)
{
    uint8_t frame[20];
    return kernel_guest_read_bytes(address, frame, sizeof(frame)) &&
           kernel_guest_write_bytes(address, frame, sizeof(frame));
}
#endif
static bool second_vblank_producer_confirmed(uint32_t handle,uint32_t fs)
{
    kernel_thread_record record;
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
    return kernel_thread_get(handle, &record) && tsfp_second_vblank_producer_record_matches(
        record.in_use, record.started, record.finished, record.terminated, record.termination_requested,
        record.control_base, fs, record.system_routine, record.start_routine, record.start_context);
#else
    return kernel_thread_get(handle,&record) && record.in_use && record.started &&
           record.finished && record.terminated && record.termination_requested &&
           record.control_base==fs && record.system_routine==0x37FE1Du &&
           record.start_routine==0x156CB0u && record.start_context==0u;
#endif
}
/* T183: opt-in schedule record. Every field is guest progress or the virtual clock,
 * never wall time or an ASLR address, so two runs compare line by line. */
bool vblank_schedule_trace;
static TSFP_RECOMP_TLS uint32_t schedule_safepoints;
/* T372: with --couple-vblank-effects every schedule line also carries the device's own vblank
 * count (device+0x1DE8), so the coupling is visible per event. Absent otherwise, which keeps the
 * T183 lines byte-identical with the flag off. */
static void schedule_device_count(char out[32])
{
    uint32_t count = 0u;
    out[0] = '\0';
    if (d3d8_vblank_effects_enabled() && kernel_guest_read_u32(0x3E3F60u + 0x1DE8u, &count))
        snprintf(out, 32, " devcount=%u", count);
}
bool vblank_readers_trace;
static void trace_reader_census(const char *label, uint32_t caller);
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
static void second_vblank_finish_refused(void)
{
    tsfp_second_vblank_host_lock();
    tsfp_vblank_inflight_set(false);
    tsfp_vblank_refused_set(true);
    tsfp_second_vblank_host_unlock();
}

static bool second_vblank_display_now(bool virtual_time, uint64_t *now_ns)
{
    if (virtual_time) { *now_ns = kernel_vtime_now_ns(); return true; }
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) return false;
    *now_ns = (uint64_t)now.tv_sec * 1000000000u + (uint64_t)now.tv_nsec;
    return true;
}

static uint32_t second_vblank_display_mode(void) { return d3d8_device_load32(0x1DDCu); }

static bool second_vblank_display_sleep(bool virtual_time, uint64_t now_ns, uint64_t wait_ns)
{
    if (virtual_time) { kernel_vtime_advance_to_ns(now_ns + wait_ns); return true; }
    const struct timespec pause = {(time_t)(wait_ns / 1000000000u), (long)(wait_ns % 1000000000u)};
    return nanosleep(&pause, NULL) == 0 || errno == EINTR;
}

static bool second_vblank_wait_display(void)
{
    const main_display_wait_host host = {tsfp_second_vblank_shutdown_requested, kernel_vtime_enabled,
        second_vblank_display_now, second_vblank_display_mode, tsfp_startup_vblank_clock_poll,
        second_vblank_display_sleep};
    return tsfp_second_vblank_wait_display_period(host);
}

static void second_vblank_startup_owner_frame_poll(uint32_t handle, uint32_t fs, uint32_t esp,
                                                   uint32_t caller, uint32_t irql)
{
    if (tsfp_second_vblank_worker_budget_get() != 0u || handle == 0u || handle != tsfp_vblank_owner_id_get()) return;
    if ((caller == 0u && !kernel_guest_read_u32(esp, &caller)) || caller != 0x153975u) return;
    tsfp_second_vblank_host_lock();
    const uint32_t candidate = tsfp_second_vblank_host_owner_clock_candidate(handle, fs);
    tsfp_second_vblank_host_unlock();
    if (!candidate) return;
    const bool startup = candidate == 1u;
    const char *error = NULL;
    uint32_t counter = 0u, last = 0u;
    if (irql != 0u) error = "T3409 owner frame clock requires IRQL 0";
    else if (!tsfp_second_vblank_host_title_counter_read(&counter) ||
             !tsfp_second_vblank_host_frame_last_exit_read(&last))
        error = "T3409 owner frame clock counter state unreadable";
    if (error) { host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, error); abort(); }
    if (counter != last) return;
    tsfp_second_vblank_host_lock();
    const bool reserved = tsfp_second_vblank_host_owner_clock_reserve(handle, fs, startup);
    tsfp_second_vblank_host_unlock();
    if (!reserved) return;
    if (!tsfp_second_vblank_wait_owner_display_period(tsfp_startup_vblank_clock_reset,
                                                      second_vblank_wait_display) || !d3d8_gpu_model_blank()) {
        second_vblank_finish_refused();
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u,
                      "Rust owner display clock refused its frame-wait period");
        abort();
    }
    if (!tsfp_second_vblank_mark_clock_announced())
        fprintf(stderr, "T3409 Rust display clock: periodic blank delivered at the owner frame-wait getter\n");
    second_vblank_deliver_blank(handle, fs, false, startup);
}

static void second_vblank_deliver_first(uint32_t fs, uint32_t esp, uint32_t irql)
{
    if (!host_run_armed()) { second_vblank_finish_refused(); abort(); }
    volatile host_run_scope scope = HOST_RUN_SCOPE_INITIALIZER;
    if (!host_run_scope_init(&scope)) { second_vblank_finish_refused(); abort(); }
    if (sigsetjmp(*host_run_scope_jmp(&scope), 0) != 0) {
        const host_stop stopped = *host_run_result();
        second_vblank_finish_refused();
        if (!host_run_scope_pop(&scope)) abort();
        host_run_rethrow(&stopped);
    }
    if (!host_run_scope_push(&scope)) {
        second_vblank_finish_refused();
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "two-event stop scope unavailable");
        abort();
    }
    d3d8_first_vblank_poll(0x1538C0u, fs, esp, irql);
    d3d8_first_vblank_snapshot previous;
    d3d8_first_vblank_get_snapshot(&previous);
    if (!previous.delivered)
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "first callback did not complete");
    tsfp_second_vblank_host_lock();
    const bool poisoned = !tsfp_second_vblank_host_complete_original(true);
    tsfp_second_vblank_host_unlock();
    if (poisoned)
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "wait/refill during first callback refused");
    if (!host_run_scope_pop(&scope)) abort();
}

static void second_vblank_deliver_second(uint32_t fs, uint32_t esp,
                                         uint32_t irql, main_frame_result result)
{
    tsfp_second_vblank_host_unlock();
    if (!host_run_armed()) { second_vblank_finish_refused(); abort(); }
    volatile host_run_scope scope = HOST_RUN_SCOPE_INITIALIZER;
    if (!host_run_scope_init(&scope)) { second_vblank_finish_refused(); abort(); }
    if (sigsetjmp(*host_run_scope_jmp(&scope), 0) != 0) {
        const host_stop stopped = *host_run_result();
        second_vblank_finish_refused();
        if (!host_run_scope_pop(&scope)) abort();
        host_run_rethrow(&stopped);
    }
    if (!host_run_scope_push(&scope)) {
        second_vblank_finish_refused();
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "two-event stop scope unavailable");
        abort();
    }
    if (!tsfp_vblank_confirm(result.producer_handle, result.producer_fs))
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u,
                      "second-event producer lacks confirmed termination");
    uint32_t title_counter;
    if (!tsfp_second_vblank_read_second_scope(esp, fs, irql, &title_counter, kernel_guest_read_u32,
            second_scope_current_irql, second_scope_pcr_irql, tsfp_second_vblank_host_title_counter_read))
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u,
                      "second-event caller/PCR/device scope refused");
    uint32_t publication[6];
    const bool coupled = d3d8_vblank_effects_enabled();
    const uint32_t second_count = coupled ? (uint32_t)d3d8_vblank_effects_applied() + 1u : 2u;
    publication[0] = coupled ? second_count : 3u;
    publication[3] = tsfp_vblank_stack_low_get();
    publication[4] = tsfp_vblank_stack_high_get();
    publication[5] = second_count;
    const bool modelled = d3d8_flip_enabled() && d3d8_vblank_effects_enabled();
    if (!second_vblank_bookkeeping_valid(publication[5], modelled))
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u,
                      "second-event nonempty or changed device bookkeeping");
    d3d8_vblank_record record = {publication[5], 0u, 0u};
    if (modelled) {
        static const char *const reasons[] = {
            "second-event flip words unreadable", "second-event flip queue not drained",
            "second-event flip threshold set without a queued flip",
            "second-event helper record disagrees with the flip words"};
        const int error = flip_queue_refusal(publication[5], &record);
        if (error >= 0) host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, reasons[error]);
    }
    publication[1] = record.flip_index;
    publication[2] = record.flags;
    if (tsfp_second_vblank_context_aliased(publication[3], publication[4], fs, esp, false))
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "second-event aliased callback context");
    if (!tsfp_second_vblank_delivery_probes(fs, publication[4], publication[5], title_counter,
            kernel_guest_read_u32, kernel_guest_write_u32, second_vblank_probe_frame,
            tsfp_second_vblank_host_title_counter_probe_write))
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "second-event write probes refused");
    tsfp_second_vblank_host_lock();
    bool poisoned = tsfp_vblank_refused_get();
    const uint32_t publish_result = tsfp_second_vblank_publish_second(poisoned, coupled, second_count,
                                                                       kernel_guest_write_u32);
    tsfp_second_vblank_host_unlock();
    if (poisoned)
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "second-event concurrent refill refused");
    if (publish_result == 2u)
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "second-event count publication refused");
    if (!tsfp_vblank_invoke(0x22020u, publication[3], publication[4], publication))
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "second callback invocation failed after publication");
    tsfp_second_vblank_host_lock();
    poisoned = !tsfp_second_vblank_host_complete_original(false);
    tsfp_second_vblank_host_unlock();
    if (poisoned)
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, "second-event refill during callback refused");
    if (!host_run_scope_pop(&scope)) abort();
}

static bool second_vblank_lookup_worker(uint32_t handle, uint32_t *control_base)
{
    kernel_thread_record record;
    if (!kernel_thread_get(handle, &record) ||
        !tsfp_second_vblank_worker_record_matches(record.in_use, record.started, record.finished,
            record.start_routine, record.system_routine, record.start_context)) return false;
    *control_base = record.control_base;
    return true;
}
static bool second_vblank_worker_identity(uint32_t handle, uint32_t fs)
{
    return tsfp_second_vblank_lookup_worker_identity(handle, fs, second_vblank_lookup_worker);
}
static const char *second_vblank_later_thread_refusal(uint32_t handle)
{
    static _Thread_local char text[320];
    kernel_thread_record records[KERNEL_THREAD_MAX], self;
    main_vblank_thread_view views[KERNEL_THREAD_MAX];
    const unsigned count = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
    const uint32_t start = kernel_thread_get(handle, &self) ? self.start_routine : 0u;
    for (unsigned i = 0; i < count; ++i) {
        const kernel_thread_record *r = &records[i];
        views[i] = (main_vblank_thread_view){r->handle, r->start_routine, r->block_target,
            (uint32_t)r->block_state, r->suspend_count, r->in_use, r->terminated, r->finished, r->started};
    }
    uint32_t table_count = 0u;
    const uint32_t *table = tsfp_vblank_quiescence_default_non_readers(&table_count);
    main_vblank_verdict verdict;
    tsfp_vblank_quiescence_evaluate(views, count, handle, table, table_count, NULL, 0u, &verdict);
    const int used = snprintf(text, sizeof text, "unsupported second-event refill/producer/backlog: wait by thread 0x%x "
                              "(start 0x%x), no second-thread delivery (T587), quiescence ", (unsigned)handle, (unsigned)start);
    if (verdict.holds) snprintf(text + used, sizeof text - (size_t)used, "holds");
    else snprintf(text + used, sizeof text - (size_t)used, "refused: %s (thread 0x%x, start 0x%x)",
                  tsfp_vblank_quiescence_reason_name(verdict.reason), (unsigned)verdict.handle,
                  (unsigned)verdict.start_routine);
    return text;
}
#endif
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
static uint64_t second_vblank_milliseconds(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (uint64_t)now.tv_sec * 1000u + (uint64_t)now.tv_nsec / 1000000u;
}

static bool second_vblank_worker_epoch_active(void);
extern bool tsfp_second_vblank_check_running_flag(uint32_t, bool, bool (*)(void));
static void second_vblank_check_running_flag_caller(uint32_t callee, uint32_t handle)
{
    const bool in_gate = tsfp_vblank_owner_location_get() != 0u;
    if (!tsfp_second_vblank_check_running_flag(callee, in_gate, second_vblank_worker_epoch_active)) return;
    static _Thread_local char text[320];
    kernel_thread_record self;
    const uint32_t start = kernel_thread_get(handle, &self) ? self.start_routine : 0u;
    snprintf(text, sizeof text, "thread 0x%x (start 0x%x) called 0x%x, a running flag writer entry, while %s: "
             "the owner is not the only writer of [0x74A9AC] (T604)", (unsigned)handle, (unsigned)start,
             (unsigned)callee, in_gate ? "the owner is in the loading bar gate" : "the worker's blanks are delivered");
    host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, text);
    abort();
}

extern bool tsfp_second_vblank_await_credit(uint64_t (*)(void), bool (*)(void), bool (*)(void), void (*)(void));
static bool second_vblank_credit_pending(void)
{
    tsfp_second_vblank_host_lock();
    const bool wait = tsfp_second_vblank_host_await_credit(tsfp_second_vblank_is_owner_thread());
    tsfp_second_vblank_host_unlock();
    return wait;
}
static void second_vblank_credit_idle(void)
{
    if (kernel_detsched_participant()) kernel_detsched_block();
    else {
        const struct timespec pause = {0, 200000L};
        nanosleep(&pause, NULL);
    }
}
typedef struct {
    uint64_t (*now)(void);
    bool (*shutdown)(void);
    bool (*ready)(uint32_t);
    void (*mark_held)(uint32_t);
    void (*idle)(void);
} main_worker_hold_host;
extern uint32_t tsfp_second_vblank_worker_hold(uint32_t, uint64_t, main_worker_hold_host);
extern void tsfp_vblank_held_id_set(uint32_t);
extern uint32_t tsfp_vblank_worker_held_get(void);
extern void tsfp_vblank_worker_held_set(uint32_t);
extern bool tsfp_second_vblank_gate_prologue_visible(uint32_t, uint32_t, uint32_t);
static _Thread_local main_vblank_verdict main_worker_hold_verdict;
static bool main_owner_blocked_on(uint32_t handle)
{
    kernel_thread_record record;
    return kernel_thread_get(tsfp_vblank_owner_id_get(), &record) &&
           record.block_state == KERNEL_THREAD_BLOCK_THREAD && record.block_target == handle;
}
static bool main_worker_hold_ready(uint32_t handle)
{
    kernel_thread_record records[KERNEL_THREAD_MAX];
    main_vblank_thread_view views[KERNEL_THREAD_MAX];
    const unsigned count = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
    for (unsigned i = 0; i < count; ++i) {
        const kernel_thread_record *r = &records[i];
        views[i] = (main_vblank_thread_view){r->handle, r->start_routine, r->block_target,
            (uint32_t)r->block_state, r->suspend_count, r->in_use, r->terminated, r->finished, r->started};
    }
    uint32_t parked[1], parked_count = 0, table_count = 0;
    if (main_owner_blocked_on(handle) || tsfp_vblank_owner_location_get() != 0u)
        parked[parked_count++] = tsfp_vblank_owner_id_get();
    const uint32_t *table = tsfp_vblank_quiescence_default_non_readers(&table_count);
    tsfp_vblank_quiescence_evaluate(views, count, handle, table, table_count, parked, parked_count,
                                     &main_worker_hold_verdict);
    if (!main_worker_hold_verdict.holds) return false;
    if (main_owner_blocked_on(handle)) return true;
    uint32_t stage, target, one;
    return kernel_guest_read_u32(0x4E7968u, &stage) && kernel_guest_read_u32(0x74A990u, &target) &&
           kernel_guest_read_u32(0x475C78u, &one) && tsfp_second_vblank_gate_prologue_visible(stage, target, one);
}
static void main_mark_worker_held(uint32_t handle)
{
    tsfp_vblank_held_id_set(handle);
    tsfp_second_vblank_host_lock();
    tsfp_vblank_worker_held_set(tsfp_vblank_worker_held_get() + 1u);
    tsfp_second_vblank_host_unlock();
}
static void main_worker_hold(uint32_t handle)
{
    const uint64_t limit = tsfp_second_vblank_policy_configuration_snapshot().worker_hold_ms;
    const main_worker_hold_host host = {second_vblank_milliseconds, tsfp_second_vblank_shutdown_requested,
                                         main_worker_hold_ready, main_mark_worker_held, second_vblank_credit_idle};
    const uint32_t result = tsfp_second_vblank_worker_hold(handle, limit, host);
    if (result == 1u)
        host_run_stop(HOST_STOP_HOST_SHUTDOWN, 0x1538C0u, 0u, "interactive host shutdown while holding worker");
    if (result == 2u) {
        static _Thread_local char text[320];
        if (main_worker_hold_verdict.holds)
            snprintf(text, sizeof text, "worker blank held %u ms (T592): quiescence holds but the owner's gate prologue stores are not visible", (unsigned)limit);
        else
            snprintf(text, sizeof text, "worker blank held %u ms without quiescence (T592): %s (thread 0x%x, start 0x%x)",
                     (unsigned)limit, tsfp_vblank_quiescence_reason_name(main_worker_hold_verdict.reason),
                     (unsigned)main_worker_hold_verdict.handle, (unsigned)main_worker_hold_verdict.start_routine);
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, text);
        abort();
    }
    tsfp_vblank_held_id_set(0u);
}
static void second_vblank_await_credit(void)
{
    if (!tsfp_second_vblank_await_credit(second_vblank_milliseconds, second_vblank_credit_pending,
            tsfp_second_vblank_shutdown_requested, second_vblank_credit_idle))
        host_run_stop(HOST_STOP_HOST_SHUTDOWN, 0x1538C0u, 0u,
                      "interactive host shutdown while waiting for the second-event credit");
}
static _Noreturn void second_vblank_startup_refuse(const char *reason)
{
    host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, reason);
    abort();
}

typedef struct {
    uint64_t (*now)(void);
    bool (*shutdown)(void);
    bool (*owner_blocked)(uint32_t);
    void (*mark_held)(uint32_t);
    void (*idle)(void);
} main_final_hold_host;
extern uint32_t tsfp_second_vblank_final_hold(uint32_t, uint64_t, main_final_hold_host);
static void main_mark_final_held(uint32_t handle)
{
    tsfp_vblank_held_id_set(handle);
    tsfp_second_vblank_host_lock();
    tsfp_vblank_worker_final_held_set(tsfp_vblank_worker_final_held_get() + 1u);
    tsfp_second_vblank_host_unlock();
}
static int32_t main_flip_preflight(uint32_t count, void *record)
{
    return flip_queue_refusal(count, record);
}
static void second_vblank_deliver_blank(uint32_t handle, uint32_t fs, bool worker, bool startup)
{
    static const char *const reasons[2][13] = {
        {"owner-wait caller/PCR/device/counter scope refused", "owner-wait callback requires the quiescence check",
         "owner-wait callback requires the coupled vblank effects", "owner-wait nonempty or changed device bookkeeping",
         "owner-wait write probes refused", "owner-wait callback invocation failed", "owner-wait callback did not advance the counter by one",
         "owner-wait refill during callback refused", "owner-wait stop scope unavailable", "owner-wait flip words unreadable",
         "owner-wait flip queue not drained", "owner-wait flip threshold set without a queued flip", "owner-wait helper record disagrees with the flip words"},
        {"worker-wait caller/PCR/device/counter scope refused", "worker-wait callback requires the quiescence check",
         "worker-wait callback requires the coupled vblank effects", "worker-wait nonempty or changed device bookkeeping",
         "worker-wait write probes refused", "worker-wait callback invocation failed", "worker-wait callback did not advance the counter by one",
         "worker-wait refill during callback refused", "worker-wait stop scope unavailable", "worker-wait flip words unreadable",
         "worker-wait flip queue not drained", "worker-wait flip threshold set without a queued flip", "worker-wait helper record disagrees with the flip words"}
    };
    const char *const *text = reasons[worker ? 1 : 0];
    if (!host_run_armed()) { second_vblank_finish_refused(); abort(); }
    volatile host_run_scope scope = HOST_RUN_SCOPE_INITIALIZER;
    if (!host_run_scope_init(&scope)) { second_vblank_finish_refused(); abort(); }
    if (sigsetjmp(*host_run_scope_jmp(&scope), 0) != 0) {
        const host_stop stopped = *host_run_result();
        second_vblank_finish_refused();
        if (!host_run_scope_pop(&scope)) abort();
        host_run_rethrow(&stopped);
    }
    if (!host_run_scope_push(&scope)) { second_vblank_finish_refused(); second_vblank_startup_refuse(text[8]); }
    if (!tsfp_second_vblank_quiescence_enabled()) second_vblank_startup_refuse(text[1]);
    if (!d3d8_vblank_effects_enabled()) second_vblank_startup_refuse(text[2]);
    if (worker) {
        tsfp_second_vblank_host_lock();
        const bool exhausted = tsfp_second_vblank_host_worker_exhausted();
        tsfp_second_vblank_host_unlock();
        if (exhausted) second_vblank_startup_refuse("worker-wait callback budget exhausted");
        main_worker_hold(handle);
        tsfp_second_vblank_host_lock();
        const bool reserved = tsfp_second_vblank_host_reserve_worker(handle, startup);
        tsfp_second_vblank_host_unlock();
        if (!reserved) second_vblank_startup_refuse("worker-wait delivery raced another callback or a refusal");
    }
    tsfp_second_vblank_host_lock();
    const uint32_t expected = tsfp_second_vblank_host_expected_counter(startup);
    tsfp_second_vblank_host_unlock();
    uint32_t device = UINT32_MAX, counter = UINT32_MAX;
    uint8_t pcr[4096];
    const uint32_t irql = kernel_sync_current_irql();
    const bool pcr_readable = kernel_guest_read_bytes(fs, pcr, sizeof pcr);
    const uint32_t pcr_irql = pcr_readable ? pcr[0x24u] : UINT32_MAX;
    const bool device_readable = kernel_guest_read_u32(0x3E3F58u, &device);
    const bool counter_readable = tsfp_second_vblank_host_title_counter_read(&counter);
    if (!tsfp_second_vblank_delivery_scope_valid(irql, pcr_readable, pcr_irql, device_readable,
            device, 0x3E3F60u, counter_readable, counter, expected)) {
        if (startup && !worker)
            fprintf(stderr, "T3409 owner frame startup scope rejected: irql=%u pcr_irql=%u device_readable=%u device=0x%08x counter_readable=%u counter=%u expected_counter=%u\n",
                (unsigned)irql, (unsigned)pcr_irql, (unsigned)device_readable, (unsigned)device,
                (unsigned)counter_readable, (unsigned)counter, (unsigned)expected);
        second_vblank_startup_refuse(text[0]);
    }
    const uint32_t count = (uint32_t)d3d8_vblank_effects_applied() + 1u;
    const bool modelled = d3d8_flip_enabled() && d3d8_vblank_effects_enabled();
    d3d8_vblank_record record = {count, 0u, 0u};
    const int preflight = tsfp_second_vblank_delivery_preflight(count, modelled, &record,
        second_vblank_bookkeeping_valid, main_flip_preflight);
    if (preflight == -2) second_vblank_startup_refuse(text[3]);
    if (preflight >= 0) second_vblank_startup_refuse(text[9 + preflight]);
    if (!worker) {
        kernel_thread_record records[KERNEL_THREAD_MAX];
        main_vblank_thread_view views[KERNEL_THREAD_MAX];
        const unsigned n = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
        for (unsigned i = 0; i < n; ++i) {
            const kernel_thread_record *r = &records[i];
            views[i] = (main_vblank_thread_view){r->handle, r->start_routine, r->block_target,
                (uint32_t)r->block_state, r->suspend_count, r->in_use, r->terminated, r->finished, r->started};
        }
        const uint32_t held = tsfp_vblank_held_id_get();
        uint32_t table_count = 0u;
        const uint32_t *table = tsfp_vblank_quiescence_default_non_readers(&table_count);
        main_vblank_verdict verdict;
        tsfp_vblank_quiescence_evaluate(views, n, handle, table, table_count, &held, held != 0u ? 1u : 0u, &verdict);
        if (!verdict.holds) {
            static _Thread_local char detail[192];
            snprintf(detail, sizeof detail, "vblank delivery quiescence refused: %s (thread %#x started at %#x)",
                tsfp_vblank_quiescence_reason_name(verdict.reason), (unsigned)verdict.handle, (unsigned)verdict.start_routine);
            host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x3D3550u, 0u, detail);
            abort();
        }
    }
    if (!tsfp_second_vblank_delivery_probes(fs, tsfp_vblank_stack_high_get(), count, counter,
            kernel_guest_read_u32, kernel_guest_write_u32, second_vblank_probe_frame, tsfp_second_vblank_host_title_counter_probe_write))
        second_vblank_startup_refuse(text[4]);
    const uint32_t payload[3] = {record.count, record.flip_index, record.flags};
    if (!tsfp_vblank_invoke(0x22020u, tsfp_vblank_stack_low_get(), tsfp_vblank_stack_high_get(), payload))
        second_vblank_startup_refuse(text[5]);
    uint32_t after = 0u;
    const bool after_readable = tsfp_second_vblank_host_title_counter_read(&after);
    if (!tsfp_second_vblank_delivery_advanced(after_readable, counter, after)) second_vblank_startup_refuse(text[6]);
    tsfp_second_vblank_host_lock();
    const bool completed = tsfp_second_vblank_host_complete_delivery(worker, startup);
    tsfp_second_vblank_host_unlock();
    if (!completed) second_vblank_startup_refuse(text[7]);
    if (worker) {
        uint32_t progress, one;
        if (!kernel_guest_read_u32(0x749848u, &progress) || !kernel_guest_read_u32(0x475C78u, &one))
            second_vblank_startup_refuse("worker-blank gate progress unreadable");
        if (!tsfp_second_vblank_gate_closed(progress, one)) {
            const main_final_hold_host host = {second_vblank_milliseconds, tsfp_second_vblank_shutdown_requested,
                main_owner_blocked_on, main_mark_final_held, second_vblank_credit_idle};
            const uint32_t result = tsfp_second_vblank_final_hold(handle,
                tsfp_second_vblank_policy_configuration_snapshot().worker_hold_ms, host);
            if (result == 1u) host_run_stop(HOST_STOP_HOST_SHUTDOWN, 0x1538C0u, 0u, "interactive host shutdown while holding worker");
            if (result == 2u) second_vblank_startup_refuse("worker blank: the owner left the gate but never blocked on the worker (T592)");
            tsfp_vblank_held_id_set(0u);
        }
    }
    if (!host_run_scope_pop(&scope)) abort();
}

static bool second_vblank_startup_worker_poll(uint32_t handle, uint32_t fs, uint32_t esp, uint32_t irql)
{
    tsfp_second_vblank_host_lock();
    const bool clock_enabled = tsfp_vblank_startup_clock_get();
    const bool clock_path = clock_enabled && tsfp_second_vblank_worker_budget_get() == 0u;
    const bool has_credit = tsfp_vblank_credits_get() == 1u;
    const bool producer_matches = !has_credit ||
        (handle == tsfp_vblank_producer_handle_get() && fs == tsfp_vblank_producer_fs_get() &&
         tsfp_second_vblank_is_producer_thread());
    const bool candidate = tsfp_second_vblank_host_startup_candidate(handle, producer_matches);
    tsfp_second_vblank_host_unlock();
    if (!candidate || !second_vblank_worker_identity(handle, fs)) return false;
    if (!clock_enabled && !main_owner_blocked_on(handle)) return false;

    uint32_t values[6] = {0};
    static const char *const fields[] = {"irql", "caller", "counter", "previous", "stop", "gate-running", "gate-flag"};
    const uint32_t failure = tsfp_second_vblank_startup_scope_read(irql, esp, clock_path, values,
        kernel_guest_read_u32, tsfp_second_vblank_host_title_counter_read);
    if (failure < 7u) {
        fprintf(stderr, "T3409 startup worker scope rejected at %s: irql=%u caller=0x%08x counter=%u previous=%u stop=%u running=%u gate=%u\n",
            fields[failure], (unsigned)irql, (unsigned)values[0], (unsigned)values[1],
            (unsigned)values[2], (unsigned)values[3], (unsigned)values[4], (unsigned)values[5]);
        second_vblank_startup_refuse("interactive startup worker caller/counter/termination scope refused");
    }
    if (tsfp_second_vblank_context_aliased(tsfp_vblank_stack_low_get(), tsfp_vblank_stack_high_get(), fs, esp, true))
        second_vblank_startup_refuse("interactive startup worker aliased callback context");
    if (!main_owner_blocked_on(handle)) {
        if (!clock_enabled) return false;
        const main_startup_owner_wait_host host = {second_vblank_milliseconds, main_owner_blocked_on,
            tsfp_second_vblank_shutdown_requested, second_vblank_credit_idle};
        const uint32_t result = tsfp_second_vblank_wait_startup_owner(handle, 2000u, host);
        if (result == 1u)
            host_run_stop(HOST_STOP_HOST_SHUTDOWN, 0x1538C0u, 0u, "interactive shutdown while awaiting startup worker owner wait");
        if (result == 2u) second_vblank_startup_refuse("interactive startup worker owner wait was not observed");
    }
    const bool owner_blocked = main_owner_blocked_on(handle);
    tsfp_second_vblank_host_lock();
    const bool still_matches = !has_credit ||
        (handle == tsfp_vblank_producer_handle_get() && fs == tsfp_vblank_producer_fs_get() &&
         tsfp_second_vblank_is_producer_thread());
    const bool reserved = tsfp_second_vblank_host_startup_reserve(handle, still_matches, has_credit, owner_blocked);
    tsfp_second_vblank_host_unlock();
    if (!reserved) second_vblank_startup_refuse("interactive startup worker credit reservation raced");
    if (!has_credit) {
        if (!second_vblank_wait_display()) {
            second_vblank_finish_refused();
            second_vblank_startup_refuse("Rust startup display clock refused its one-shot deadline");
        }
        if (!d3d8_gpu_model_blank()) {
            second_vblank_finish_refused();
            second_vblank_startup_refuse("Rust startup display clock blank effects refused");
        }
        fprintf(stderr, "T3409 startup worker: one default display-clock period completed at the terminal getter\n");
    } else {
        fprintf(stderr, "T3409 startup worker: delivered the producer's completed wait credit at the terminal getter\n");
    }
    second_vblank_deliver_blank(handle, fs, true, true);
    return true;
}
#endif
static void second_vblank_wait_observer(bool success)
{
    if (success && vblank_readers_trace) {
        /* T460: the thread census at each owner wait that is about to deliver, observation only and
         * only with the owner-wait budget on, so the T371 records stay byte identical otherwise. */
        recomp_second_vblank_snapshot before;
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
        before = tsfp_second_vblank_snapshot();
#else
        recomp_second_vblank_get_snapshot(&before);
#endif
        if (before.owner_budget != 0u && before.second_delivered) trace_reader_census("owner-wait", 0u);
    }
    if (vblank_schedule_trace) {
        char count[32], flip[512];
        schedule_device_count(count);
        d3d8_vblank_effects_trace(flip,sizeof flip);
        fprintf(stderr,"vblank-schedule: wait success=%d thread=0x%X clock=%llu%s%s\n",(int)success,
                callback_thread_handle,(unsigned long long)tsfp_timers_host_virtual_ticks(),count,flip);
    }
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
    const bool budgeted_worker = tsfp_second_vblank_worker_budget_get() != 0u;
    bool startup_clock = false;
    if (!budgeted_worker) {
        tsfp_second_vblank_host_lock();
        startup_clock = tsfp_vblank_startup_clock_get();
        tsfp_second_vblank_host_unlock();
    }
    const bool worker = (budgeted_worker || startup_clock) &&
        second_vblank_worker_identity(callback_thread_handle, g_fs_base);
    tsfp_second_vblank_host_lock();
    const uint32_t action = tsfp_second_vblank_host_wait_completed(
        success, callback_thread_handle, g_fs_base, tsfp_second_vblank_is_owner_thread(), worker);
    tsfp_second_vblank_host_unlock();
    const char *error = NULL;
    if (action == 4u) error = "unsuccessful GPU wait cannot produce callback credit";
    else if (action == 5u) error = "wait completion during callback reservation";
    else if (action == 6u) error = "owner-wait callback budget exhausted";
    else if (action == 7u) error = "unsupported second-event refill/producer/backlog";
    else if (action == 8u) error = second_vblank_later_thread_refusal(callback_thread_handle);
    if (error) {
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, error);
        abort();
    }
    if (action == 2u || action == 3u)
        second_vblank_deliver_blank(callback_thread_handle, g_fs_base, action == 3u, false);
#else
    recomp_second_vblank_note_wait_completed(success,callback_thread_handle,g_fs_base);
#endif
    if (vblank_schedule_trace) {
        /* T460: only with the owner-wait budget on, so the T183 lines stay byte identical otherwise. */
        recomp_second_vblank_snapshot snapshot;
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
        snapshot = tsfp_second_vblank_snapshot();
#else
        recomp_second_vblank_get_snapshot(&snapshot);
#endif
        /* T592: with the worker blanks on the line is the owner's, a worker wait has its own line below. */
        if (snapshot.owner_budget != 0u && snapshot.owner_delivered != 0u &&
            (snapshot.worker_budget == 0u || callback_thread_handle == snapshot.owner_handle)) {
            uint32_t counter = 0u, count = 0u, flip = 0u, delta = 0u, last = 0u;
            (void)kernel_guest_read_u32(0x563918u, &counter);
            (void)kernel_guest_read_u32(0x3E3F60u + 0x1DE8u, &count);
            (void)kernel_guest_read_u32(0x3E3F60u + 0x1DE4u, &flip);
            (void)kernel_guest_read_u32(0x3E3F60u + 0x1DFCu, &delta);
            (void)kernel_guest_read_u32(0x3E3F60u + 0x1E00u, &last);
            fprintf(stderr,"vblank-schedule: owner-wait delivered=%u of %u thread=0x%X counter=%u "
                    "record=%u,%u delta=0x%X last=0x%X\n",
                    (unsigned)snapshot.owner_delivered,(unsigned)snapshot.owner_budget,
                    callback_thread_handle,counter,count,flip,delta,last);
        }
        /* T592: one line per worker blank, only with the worker budget on. */
        static uint32_t worker_printed;
        if (snapshot.worker_budget != 0u && snapshot.worker_delivered != worker_printed) {
            uint32_t counter = 0u, count = 0u, flip = 0u, delta = 0u, last = 0u;
            worker_printed = snapshot.worker_delivered;
            (void)kernel_guest_read_u32(0x563918u, &counter);
            (void)kernel_guest_read_u32(0x3E3F60u + 0x1DE8u, &count);
            (void)kernel_guest_read_u32(0x3E3F60u + 0x1DE4u, &flip);
            (void)kernel_guest_read_u32(0x3E3F60u + 0x1DFCu, &delta);
            (void)kernel_guest_read_u32(0x3E3F60u + 0x1E00u, &last);
            fprintf(stderr,"vblank-schedule: worker-wait delivered=%u of %u thread=0x%X counter=%u "
                    "record=%u,%u delta=0x%X last=0x%X\n",
                    (unsigned)snapshot.worker_delivered,(unsigned)snapshot.worker_budget,
                    callback_thread_handle,counter,count,flip,delta,last);
        }
    }
}
static void second_vblank_registration_observer(uint32_t callback)
{
    if (vblank_schedule_trace) {
        char count[32];
        schedule_device_count(count);
        fprintf(stderr,"vblank-schedule: register callback=0x%08X thread=0x%X clock=%llu%s\n",callback,
                callback_thread_handle,(unsigned long long)tsfp_timers_host_virtual_ticks(),count);
    }
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
    const char *error = NULL;
    tsfp_second_vblank_host_lock();
    const uint32_t action = tsfp_second_vblank_host_registration_begin(
        callback, callback_thread_handle, g_fs_base,
        tsfp_vblank_enabled_get() && tsfp_vblank_bound_get() && tsfp_second_vblank_is_owner_thread());
    if (action == 2u) error = "unsupported callback registration/rebind";
    else if (action == 1u) {
        uint32_t device, old_callback;
        const bool valid = kernel_guest_read_u32(0x3E3F58u, &device) && device == 0x3E3F60u &&
            kernel_guest_read_u32(0x3E3F60u + 0x1DB8u, &old_callback) && old_callback == 0u &&
            kernel_guest_write_u32(0x3E3F60u + 0x1DB8u, old_callback);
        tsfp_second_vblank_host_registration_finish(valid);
        if (!valid) error = "callback registration requires fixed device and writable NULL slot";
    }
    tsfp_second_vblank_host_unlock();
    if (error) {
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, error);
        abort();
    }
#else
    recomp_second_vblank_note_registration(callback,callback_thread_handle,g_fs_base);
#endif
}
static void trace_schedule_point(const char *label, uint32_t callee)
{
    uint32_t counter = 0u, caller = 0u;
    (void)kernel_guest_read_u32(0x563918u, &counter);
    (void)kernel_guest_read_u32(g_esp, &caller);
    char devcount[32];
    schedule_device_count(devcount);
    fprintf(stderr,"vblank-schedule: %s callee=0x%08X thread=0x%X caller=0x%08X counter=%u safepoints=%u clock=%llu%s\n",
            label,callee,callback_thread_handle,caller,counter,schedule_safepoints,
            (unsigned long long)tsfp_timers_host_virtual_ticks(),devcount);
}
/* T371: counter-reader census and the quiescence check. Both are observation only: they read the
 * thread table and the guest counter and write nothing in the guest. */
bool vblank_readers_trace;
static TSFP_RECOMP_TLS uint32_t reader_polls, reader_repeat, reader_last_counter;
static void trace_reader_census(const char *label, uint32_t caller)
{
    kernel_thread_record records[KERNEL_THREAD_MAX];
    const unsigned count = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
    uint32_t counter = 0u;
    (void)kernel_guest_read_u32(0x563918u, &counter);
    fprintf(stderr,"vblank-readers: %s thread=0x%X caller=0x%08X counter=%u threads=%u\n",label,
            callback_thread_handle,caller,counter,count);
    for (unsigned i = 0u; i < count; i++) {
        const kernel_thread_record *r = &records[i];
        const main_vblank_thread_view view = {r->handle, r->start_routine, r->block_target,
            (uint32_t)r->block_state, r->suspend_count, r->in_use, r->terminated, r->finished, r->started};
        fprintf(stderr,"vblank-readers:   member=0x%X start=0x%X state=%s%s\n",
                (unsigned)r->handle, (unsigned)r->start_routine,
                tsfp_vblank_quiescence_state_name(tsfp_vblank_quiescence_classify(&view)),
                records[i].handle == callback_thread_handle ? " (this thread)" : "");
    }
}
/* Sampled at every call boundary of the calling thread: a line each time the set of guest thread
 * states it sees changes. Which safepoint sees a change first is host timing, so these lines carry
 * their own prefix and are measurement only, never part of the compared record. */
static TSFP_RECOMP_TLS char sample_last[256];
static TSFP_RECOMP_TLS uint32_t sample_safepoints;
static void trace_reader_sample(void)
{
    kernel_thread_record records[KERNEL_THREAD_MAX];
    const unsigned count = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
    char now[256] = "";
    for (unsigned i = 0u; i < count; i++) {
        const size_t used = strlen(now);
        const kernel_thread_record *r = &records[i];
        const main_vblank_thread_view view = {r->handle, r->start_routine, r->block_target,
            (uint32_t)r->block_state, r->suspend_count, r->in_use, r->terminated, r->finished, r->started};
        snprintf(now + used, sizeof now - used, " 0x%X:%s", (unsigned)records[i].handle,
                 tsfp_vblank_quiescence_state_name(tsfp_vblank_quiescence_classify(&view)));
    }
    if (strcmp(now, sample_last) != 0) {
        fprintf(stderr,"vblank-readers-sample: thread=0x%X safepoint=%u threads=[%s ]\n",
                callback_thread_handle,sample_safepoints,now);
        memcpy(sample_last, now, sizeof now);
    }
}
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
static bool vblank_getter_registered(uint32_t handle, uint32_t caller)
{
    kernel_thread_record record;
    return kernel_thread_get(handle, &record) &&
           tsfp_vblank_quiescence_is_registered_reader(caller, record.start_routine);
}
#endif
static void vblank_reader_hook(uint32_t callee, uint32_t return_address_override)
{
    /* Native C2R cannot leave a synthetic guest call frame on g_esp. Carry the measured
     * return site explicitly for the owner getter census; ordinary guest calls pass zero. */
    uint32_t caller = return_address_override;
    if (vblank_readers_trace) { sample_safepoints++; trace_reader_sample(); }
    if (caller == 0u) (void)kernel_guest_read_u32(g_esp, &caller);
    if (callee == RVQ_GETTER) {
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
        if (!tsfp_second_vblank_quiescence_check_getter(callback_thread_handle, caller,
                                                         vblank_getter_registered)) {
            host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, RVQ_GETTER, 0u,
                          "vblank counter getter reached by an unregistered reader");
            abort();
        }
#else
        recomp_vblank_quiescence_check_getter(callback_thread_handle,caller);
#endif
        if (vblank_readers_trace) {
            kernel_thread_record record;
            const bool known = kernel_thread_get(callback_thread_handle, &record);
            uint32_t counter = 0u;
            (void)kernel_guest_read_u32(0x563918u, &counter);
            reader_repeat = (reader_polls != 0u && counter == reader_last_counter) ? reader_repeat + 1u : 0u;
            reader_last_counter = counter;
            reader_polls++;
            /* The other threads' states at this poll, in creation order. */
            kernel_thread_record records[KERNEL_THREAD_MAX];
            const unsigned count = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
            char others[256] = "";
            for (unsigned i = 0u; i < count; i++) {
                if (records[i].handle == callback_thread_handle) continue;
                const size_t used = strlen(others);
                const kernel_thread_record *r = &records[i];
                const main_vblank_thread_view view = {r->handle, r->start_routine, r->block_target,
                    (uint32_t)r->block_state, r->suspend_count, r->in_use, r->terminated, r->finished, r->started};
                snprintf(others + used, sizeof others - used, " 0x%X:%s", (unsigned)records[i].handle,
                         tsfp_vblank_quiescence_state_name(tsfp_vblank_quiescence_classify(&view)));
            }
            fprintf(stderr,"vblank-readers: getter thread=0x%X start=0x%X caller=0x%08X counter=%u poll=%u repeat=%u registered=%d checks=%u others=[%s ]\n",
                    callback_thread_handle,known ? record.start_routine : 0u,caller,counter,reader_polls,
                    reader_repeat,(int)(known && tsfp_vblank_quiescence_is_registered_reader(caller,record.start_routine)),
                    tsfp_second_vblank_quiescence_getter_checks(),others);
        }
    } else if (callee == 0x1538C0u) {
        if (vblank_readers_trace) trace_reader_census("frame-entry",caller);
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
        main_vblank_verdict verdict = {.holds = true};
        if (tsfp_second_vblank_quiescence_enabled()) {
            kernel_thread_record records[KERNEL_THREAD_MAX];
            main_vblank_thread_view views[KERNEL_THREAD_MAX];
            const unsigned count = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
            for (unsigned i = 0u; i < count; i++) {
                const kernel_thread_record *r = &records[i];
                views[i] = (main_vblank_thread_view){r->handle, r->start_routine, r->block_target,
                    (uint32_t)r->block_state, r->suspend_count, r->in_use, r->terminated, r->finished, r->started};
            }
            uint32_t table_count = 0u;
            const uint32_t *table = tsfp_vblank_quiescence_default_non_readers(&table_count);
            tsfp_vblank_quiescence_evaluate(views, count, callback_thread_handle, table, table_count,
                                             NULL, 0u, &verdict);
            if (!verdict.holds) {
                static _Thread_local char detail[192];
                snprintf(detail, sizeof detail, "vblank delivery quiescence refused: %s (thread %#x started at %#x)",
                         tsfp_vblank_quiescence_reason_name(verdict.reason),
                         (unsigned)verdict.handle, (unsigned)verdict.start_routine);
                host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, detail);
                abort();
            }
        }
#else
        const rvq_verdict verdict = recomp_vblank_quiescence_check(callback_thread_handle,0x1538C0u);
#endif
        if (vblank_readers_trace && tsfp_second_vblank_quiescence_enabled())
            fprintf(stderr,"vblank-readers: quiescence holds=%d examined=%u thread=0x%X\n",(int)verdict.holds,
                    verdict.examined,callback_thread_handle);
    }
}

bool host_timers_prepare(bool second, recomp_cooperative_provider provider)
{
    guest_region_request request = {0};
    request.bytes = FIRST_VBLANK_STACK_BYTES;
    request.alignment = 0x1000u;
    request.protect = 0x04u; /* PAGE_READWRITE */
    request.state = 0x1000u; /* MEM_COMMIT */
    nt_status status = STATUS_SUCCESS;
    first_vblank_stack = kmem_region_alloc(&request, &status);
    if (first_vblank_stack == 0u ||
        (uint64_t)first_vblank_stack + FIRST_VBLANK_STACK_BYTES > UINT32_MAX) {
        if (first_vblank_stack != 0u) {
            (void)kmem_region_free(first_vblank_stack);
        }
        first_vblank_stack = 0u;
        fprintf(stderr, "could not allocate private startup callback stack (status 0x%08X)\n",
                (unsigned)status);
        return false;
    }
    second_vblank_policy=second;
    if (second) {
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
        tsfp_second_vblank_host_lock();
        const bool configured = tsfp_second_vblank_host_configure(true, recomp_callback_run,
            second_vblank_producer_confirmed, first_vblank_stack, first_vblank_stack+FIRST_VBLANK_STACK_BYTES);
        tsfp_second_vblank_host_unlock();
        if (configured) {
            d3d8_first_vblank_configure(true, recomp_callback_run, first_vblank_stack,
                                        first_vblank_stack+FIRST_VBLANK_STACK_BYTES);
            tsfp_startup_vblank_clock_reset();
        }
        if (!configured)
#else
        if (!recomp_second_vblank_configure(true,recomp_callback_run,first_vblank_stack,
                first_vblank_stack+FIRST_VBLANK_STACK_BYTES,second_vblank_producer_confirmed))
#endif
            return false;
        d3d8_gpu_set_vblank_observer(second_vblank_wait_observer);
        d3d8_callbacks_set_vblank_observer(second_vblank_registration_observer);
    } else {
        d3d8_first_vblank_configure(true, recomp_callback_run, first_vblank_stack,
                                    first_vblank_stack + FIRST_VBLANK_STACK_BYTES);
    }
    if (!recomp_cooperative_configure(provider, NULL)) {
        abort();
    }
    return true;
}

bool host_timers_cleanup(void)
{
    /* Only after every guest thread joined; watchdog exits before reaching here. */
    if (!recomp_cooperative_configure(NULL, NULL)) {
        abort();
    }
    d3d8_gpu_set_vblank_observer(NULL);
    d3d8_callbacks_set_vblank_observer(NULL);
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
    tsfp_second_vblank_host_lock();
    const bool configured = tsfp_second_vblank_host_configure(false, NULL, NULL, 0u, 0u);
    tsfp_second_vblank_host_unlock();
    if (!configured) abort();
    d3d8_first_vblank_configure(false, NULL, 0u, 0u);
    tsfp_startup_vblank_clock_reset();
    if (!tsfp_second_vblank_host_reset()) abort();
    d3d8_first_vblank_reset();
    tsfp_startup_vblank_clock_reset();
#else
    if (!recomp_second_vblank_configure(false,NULL,0u,0u,NULL)) abort();
    if (!recomp_second_vblank_reset()) abort();
#endif
    second_vblank_policy=false;
    d3d8_first_vblank_configure(false, NULL, 0u, 0u);
    d3d8_first_vblank_reset();
    if (first_vblank_stack == 0u) {
        return true;
    }
    bool okay = kmem_region_free(first_vblank_stack);
    if (okay) {
        first_vblank_stack = 0u;
    } else {
        fprintf(stderr, "startup callback stack cleanup refused; region preserved\n");
    }
    return okay;
}


void host_timers_safepoint(uint32_t callee, uint32_t return_address_override)
{
    if (vblank_readers_trace || tsfp_second_vblank_quiescence_enabled())
        vblank_reader_hook(callee, return_address_override);
    if (vblank_schedule_trace) {
        schedule_safepoints++;
        /* 0x1538C0 is the title frame wait, 0x22030 its counter getter. */
        if (callee == 0x1538C0u || callee == 0x22030u) trace_schedule_point("safepoint",callee);
    }
    if (callee == 0x1538C0u) gft_enter(GFT_SAFEPOINT, callee); /* the title frame wait: where a blank may be delivered */
    if (second_vblank_policy) {
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
        if (callback_thread_handle != 0u) {
            const bool budgeted = tsfp_second_vblank_worker_budget_get() != 0u;
            bool startup_clock = false;
            if (!budgeted) {
                tsfp_second_vblank_host_lock();
                startup_clock = tsfp_vblank_startup_clock_get();
                tsfp_second_vblank_host_unlock();
            }
            const uint32_t role = tsfp_second_vblank_call_role(callback_thread_handle,
                tsfp_vblank_owner_id_get(), budgeted, startup_clock);
            if (role == 1u && callee == 0x156840u)
                tsfp_second_vblank_host_observe_gate(kernel_guest_read_u32);
            else if (role == 1u) {
                const uint32_t decision = tsfp_second_vblank_host_owner_call(
                    callee, budgeted, second_vblank_worker_epoch_active);
                if (decision == 2u || decision == 3u) {
                    const char *reason = decision == 2u ?
                        "owner left the loading bar gate through a call outside the proof (T592)" :
                        "owner sampled the vblank counter while the worker's blanks were delivered (T592)";
                    host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, reason);
                    abort();
                }
            } else if (role == 2u && budgeted) {
                if (tsfp_second_vblank_running_flag_entry(callee))
                    second_vblank_check_running_flag_caller(callee, callback_thread_handle);
                if (callee == 0x22030u) {
                    tsfp_second_vblank_host_lock();
                    const bool armed = tsfp_second_vblank_host_worker_getter_armed(callback_thread_handle);
                    tsfp_second_vblank_host_unlock();
                    kernel_thread_record worker_record;
                    const bool worker_known = armed && callback_thread_handle != 0u &&
                        kernel_thread_get(callback_thread_handle, &worker_record) &&
                        tsfp_second_vblank_worker_record_matches(worker_record.in_use, worker_record.started,
                            worker_record.finished, worker_record.start_routine, worker_record.system_routine,
                            worker_record.start_context);
                    if (worker_known) {
                        main_worker_hold(callback_thread_handle);
                        kernel_thread_record owner_record;
                        if (kernel_thread_get(tsfp_vblank_owner_id_get(), &owner_record) &&
                            owner_record.block_state == KERNEL_THREAD_BLOCK_THREAD &&
                            owner_record.block_target == callback_thread_handle) {
                            host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u,
                                "worker polled the vblank counter after the owner blocked on its termination, no blank is owed (T592)");
                            abort();
                        }
                    }
                }
            }
        }
#else
        recomp_second_vblank_note_call(callee,callback_thread_handle);
#endif
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
        if (callee == RVQ_GETTER) {
            const uint32_t irql = kernel_sync_current_irql();
            if (!second_vblank_startup_worker_poll(callback_thread_handle, g_fs_base, g_esp, irql))
                second_vblank_startup_owner_frame_poll(callback_thread_handle, g_fs_base, g_esp,
                                                                 return_address_override, irql);
        } else if (callee == 0x1538C0u) {
            second_vblank_await_credit();
            tsfp_second_vblank_host_lock();
            const bool owner = tsfp_second_vblank_is_owner_thread();
            uint32_t counter = 0u, last = 0u;
            bool valid = false;
            if (tsfp_second_vblank_host_frame_needs_counters(callback_thread_handle, g_fs_base, owner))
                valid = tsfp_second_vblank_host_title_counter_read(&counter) &&
                        tsfp_second_vblank_host_frame_last_exit_read(&last);
            const main_frame_result result = tsfp_second_vblank_host_frame(callback_thread_handle, g_fs_base,
                owner, tsfp_second_vblank_owner_is_producer(), valid, counter, last);
            if (result.action == 0u || result.action == 3u)
                tsfp_second_vblank_host_unlock();
            else if (result.action == 4u) {
                tsfp_second_vblank_host_unlock();
                const char *error = NULL;
                if (!tsfp_second_vblank_quiescence_enabled())
                    error = "owner-wait callback requires the quiescence check";
                else if (!d3d8_vblank_effects_enabled())
                    error = "owner-wait callback requires the coupled vblank effects";
                else if (!d3d8_gpu_model_blank())
                    error = "poll blank: the clock or the helper effects refused the blank";
                if (error) {
                    tsfp_second_vblank_host_lock();
                    tsfp_vblank_inflight_set(false);
                    tsfp_vblank_refused_set(true);
                    tsfp_second_vblank_host_unlock();
                    host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, error);
                    abort();
                }
                second_vblank_deliver_blank(callback_thread_handle, g_fs_base, false, false);
                tsfp_second_vblank_host_lock();
                tsfp_vblank_poll_delivered_set(tsfp_vblank_poll_delivered_get() + 1u);
                tsfp_second_vblank_host_unlock();
            }
            else if (result.action == 1u) {
                tsfp_second_vblank_host_unlock();
                second_vblank_deliver_first(g_fs_base, g_esp, kernel_sync_current_irql());
            } else {
                const char *error = NULL;
                char detail[160];
                if (result.action == 5u) error = "two-event owner/registration/reentry refused";
                else if (result.action == 6u) error = "frame-wait counter or last exit unreadable";
                else if (result.action == 7u) {
                    snprintf(detail, sizeof detail, "frame wait would spin for a blank nobody delivers (counter %u, last exit %u, "
                             "owner blanks delivered %u of %u)", (unsigned)counter, (unsigned)last,
                             (unsigned)tsfp_vblank_owner_delivered_get(), (unsigned)tsfp_vblank_owner_budget_get());
                    error = detail;
                } else if (result.action != 1u && result.action != 2u && result.action != 4u)
                    error = "second-event credit/attempt refused";
                if (error) {
                    tsfp_second_vblank_host_unlock();
                    host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u, error);
                    abort();
                }
                second_vblank_deliver_second(g_fs_base, g_esp,
                                             kernel_sync_current_irql(), result);
            }
        }
#else
        recomp_second_vblank_poll_with_caller(callee,callback_thread_handle,g_fs_base,g_esp,
                                              kernel_sync_current_irql(),return_address_override);
#endif
    }
    else
        d3d8_first_vblank_poll(callee, g_fs_base, g_esp, kernel_sync_current_irql());
    if (callee == 0x1538C0u) gft_leave();
    if (vblank_schedule_trace && callee == 0x1538C0u) trace_schedule_point("after-poll",callee);
    if (vblank_readers_trace && callee == 0x1538C0u) {
        uint32_t caller = 0u;
        (void)kernel_guest_read_u32(g_esp, &caller);
        trace_reader_census("after-poll",caller);
    }
}

void host_timers_bind_owner(const kernel_thread_launch *launch)
{
        if (second_vblank_policy) {
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
            tsfp_second_vblank_host_lock();
            const uint32_t action = tsfp_second_vblank_bind_host_owner(
                launch->handle, launch->fs_base, launch->start_routine, launch->entry_va);
            tsfp_second_vblank_host_unlock();
            if (action == 3u) {
                host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, 0x1538C0u, 0u,
                              "invalid or duplicate two-event owner");
                abort();
            }
            if (action != 0u)
                d3d8_first_vblank_bind_owner(launch->handle, launch->fs_base,
                                             launch->start_routine, launch->entry_va);
#else
            recomp_second_vblank_bind_owner(launch->handle,launch->fs_base,
                                             launch->start_routine,launch->entry_va);
#endif
        } else
            d3d8_first_vblank_bind_owner(launch->handle, launch->fs_base,
                                         launch->start_routine, launch->entry_va);
}

void host_timers_configure_quiescence(bool trace, bool check)
{
    vblank_readers_trace = trace;
    tsfp_second_vblank_quiescence_configure(check);
}

int host_timers_configure_budget(const options *opts)
{
        recomp_callback_set_dispatch_level(opts->vblank_dispatch_level);
        if (opts->vblank_owner_waits != 0u && !tsfp_second_vblank_startup_set_owner_waits(opts->vblank_owner_waits)) {
            fprintf(stderr, "could not configure the owner-wait callback budget\n");
            return 1;
        }
        printf(opts->headless_second_vblank ?
               "callback policy startup plus one credited CPU callback; no general timing\n" :
               "callback policy one startup CPU vblank callback; "
               "no clock/MMIO/event/GPU stats advance or general timing\n");
        if (opts->vblank_owner_waits != 0u) {
            printf("vblank owner waits ON (opt-in, T460): each of the owner thread's next %u completed "
                   "waits delivers one CPU callback from inside the wait (a stand-in for the DPC the "
                   "original runs while the owner is blocked), then a named stop. FABRICATED timing: the "
                   "blank rate is the title's own wait count, not a display clock. Quiescence is "
                   "checked at every delivery and every unmeasured device state refuses\n",
                   (unsigned)opts->vblank_owner_waits);
        }
        if (opts->vblank_poll_blank) {
            if (!tsfp_second_vblank_startup_set_poll_blank(true)) {
                fprintf(stderr, "could not configure the frame wait poll blank\n");
                return 1;
            }
            printf("vblank poll blank ON (opt-in, T696): a frame wait 0x1538C0 whose exit test fails with the "
                   "counter equal to its last exit delivers ONE CPU callback from inside the poll (a stand-in "
                   "for the 60 Hz interrupt), counted in the owner-wait budget, then a named stop. FABRICATED "
                   "timing: the blank rate is the title's own poll count, not a display clock. The counter and "
                   "[0x78AD2C] are never written\n");
        }
        if (opts->vblank_worker_blanks != 0u) {
            if (!tsfp_second_vblank_startup_set_worker_blanks(opts->vblank_worker_blanks)) {
                fprintf(stderr, "could not configure the worker blank budget\n");
                return 1;
            }
            /* A hold that cannot end must be a named stop, so it ends before the thread watchdog does. */
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
            tsfp_second_vblank_policy_set_worker_hold_ms(opts->interactive ? 30000u : opts->thread_timeout_ms / 10u * 8u);
#else
            tsfp_second_vblank_policy_set_worker_hold_ms(opts->interactive ? 30000u : opts->thread_timeout_ms / 10u * 8u);
#endif
            printf("vblank worker blanks ON (opt-in, T592): each of the loading bar worker's next %u completed "
                   "waits delivers one CPU callback from inside the wait while the owner is PROVEN not to read "
                   "the counter (parked in the loading bar gate or blocked on the worker), the worker idles "
                   "in its wait until then, then a named stop. FABRICATED timing: the blank rate is the "
                   "worker's own wait count, not a display clock\n",
                   (unsigned)opts->vblank_worker_blanks);
        }
        if (opts->vblank_dispatch_level) {
            printf("vblank callback IRQL ON (opt-in, T370): raised to DISPATCH_LEVEL around the "
                   "callback through the kernel IRQL model and restored on every exit\n");
        }
    return 0;
}

int host_timers_configure_interactive(bool interactive, bool (*closed)(void))
{
    if (!tsfp_second_vblank_startup_set_interactive(interactive)) {
        fprintf(stderr, "--interactive callback continuation refused\n"); return 2;
    }
    const char *startup_clock_env = getenv("TSFP_RUST_STARTUP_VBLANK_CLOCK");
    const bool startup_clock_enabled = startup_clock_env && strcmp(startup_clock_env, "1") == 0;
    if (!tsfp_second_vblank_startup_set_clock(startup_clock_enabled)) {
        fprintf(stderr, "T3409 startup vblank clock configuration refused\n"); return 2;
    }
    if (startup_clock_enabled)
        fprintf(stderr,"T3409 Rust startup vblank clock ON: one default 50/60 Hz display period for the exact terminal loading-worker wait\n");
    tsfp_second_vblank_set_shutdown_query(interactive ? closed : NULL);
    return 0;
}

void host_timers_report(const options *opts)
{
    if (opts->headless_second_vblank) {
        recomp_second_vblank_snapshot callbacks;
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
        callbacks = tsfp_second_vblank_snapshot();
#else
        recomp_second_vblank_get_snapshot(&callbacks);
#endif
        printf("CPU callback policy startup %u, continuation %u, refused %u, outstanding credits %llu\n",
               (unsigned)callbacks.first_delivered,(unsigned)callbacks.second_delivered,
               (unsigned)callbacks.refused,(unsigned long long)callbacks.credits);
        if (callbacks.owner_budget != 0u)
            printf("owner-wait callbacks delivered %u of %u budget\n",
                   (unsigned)callbacks.owner_delivered,(unsigned)callbacks.owner_budget);
        if (opts->vblank_poll_blank)
            printf("poll blanks delivered %u (of the owner-wait callbacks)\n",(unsigned)callbacks.poll_delivered);
        if (callbacks.worker_budget != 0u)
            printf("worker-wait callbacks delivered %u of %u budget, waits held %u, held after the last blank %u, "
                   "owner gate entries %u (T592)\n",
                   (unsigned)callbacks.worker_delivered,(unsigned)callbacks.worker_budget,
                   (unsigned)callbacks.worker_held,(unsigned)callbacks.worker_final_held,
                   (unsigned)callbacks.owner_gate_entries);
        if (callbacks.owner_budget != 0u && callbacks.frames_admitted != 0u)
            printf("frame waits admitted with nothing to wait for %u (T549, no delivery)\n",
                   (unsigned)callbacks.frames_admitted);
    }
}

#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
static bool second_vblank_worker_epoch_active(void)
{
    const uint32_t worker = tsfp_vblank_worker_id_get();
    kernel_thread_record record;
    return worker != 0u && kernel_thread_get(worker, &record) && !record.terminated;
}
#endif
