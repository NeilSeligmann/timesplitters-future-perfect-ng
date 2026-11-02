/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "recomp_vblank_quiescence.h"
#include "host_runtime.h"
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
extern void tsfp_second_vblank_quiescence_configure(bool enabled);
extern bool tsfp_second_vblank_quiescence_enabled(void);
extern void tsfp_second_vblank_quiescence_note_getter(void);
extern uint32_t tsfp_second_vblank_quiescence_getter_checks(void);
extern bool tsfp_second_vblank_quiescence_check_getter(uint32_t handle, uint32_t return_address,
                                                       bool (*registered)(uint32_t, uint32_t));
#else
static atomic_bool enabled_flag;
static atomic_uint getter_checks;
#endif

/* T3934: the rule is Rust (crates/tsfp-vblank-quiescence, ABI in crates/tsfp-kernel-clock-capi); this file marshals the
 * thread table into views and keeps the host stop glue. */
typedef struct {
    uint32_t handle, start_routine, block_target, block_state, suspend_count;
    bool in_use, terminated, finished, started;
} rvq_view;
typedef struct {
    bool holds;
    uint32_t reason, handle, start_routine, examined;
} rvq_c_verdict;
void tsfp_vblank_quiescence_evaluate(const rvq_view *threads, uint32_t count, uint32_t deliverer,
                                     const uint32_t *non_readers, uint32_t non_reader_count,
                                     const uint32_t *parked, uint32_t parked_count, rvq_c_verdict *out);
uint32_t tsfp_vblank_quiescence_classify(const rvq_view *thread);
bool tsfp_vblank_quiescence_is_registered_reader(uint32_t return_address, uint32_t start_routine);
const uint32_t *tsfp_vblank_quiescence_default_non_readers(uint32_t *count);
const char *tsfp_vblank_quiescence_state_name(uint32_t state);
const char *tsfp_vblank_quiescence_reason_name(uint32_t reason);
void tsfp_vblank_quiescence_configure(bool enabled);
bool tsfp_vblank_quiescence_enabled(void);
void tsfp_vblank_quiescence_note_getter_check(void);
uint32_t tsfp_vblank_quiescence_getter_checks(void);

static rvq_view view_of(const kernel_thread_record *record)
{
    return (rvq_view){.handle = record->handle, .start_routine = record->start_routine,
                      .block_target = record->block_target, .block_state = (uint32_t)record->block_state,
                      .suspend_count = record->suspend_count, .in_use = record->in_use,
                      .terminated = record->terminated, .finished = record->finished, .started = record->started};
}

rvq_thread_state recomp_vblank_quiescence_classify(const kernel_thread_record *record)
{
    const rvq_view view = view_of(record);
    return (rvq_thread_state)tsfp_vblank_quiescence_classify(&view);
}

const char *recomp_vblank_quiescence_state_name(rvq_thread_state state)
{
    return tsfp_vblank_quiescence_state_name((uint32_t)state);
}

const char *recomp_vblank_quiescence_reason_name(rvq_reason reason)
{
    return tsfp_vblank_quiescence_reason_name((uint32_t)reason);
}

const uint32_t *recomp_vblank_quiescence_default_non_readers(unsigned *count)
{
    uint32_t n = 0u;
    const uint32_t *table = tsfp_vblank_quiescence_default_non_readers(&n);
    *count = n;
    return table;
}

rvq_verdict recomp_vblank_quiescence_evaluate(const kernel_thread_record *records, unsigned count,
                                              uint32_t deliverer)
{
    unsigned table_count = 0u;
    const uint32_t *table = recomp_vblank_quiescence_default_non_readers(&table_count);
    return recomp_vblank_quiescence_evaluate_with(records, count, deliverer, table, table_count);
}

rvq_verdict recomp_vblank_quiescence_evaluate_with(const kernel_thread_record *records,
                                                   unsigned count, uint32_t deliverer,
                                                   const uint32_t *non_reader_starts,
                                                   unsigned non_reader_count)
{
    return recomp_vblank_quiescence_evaluate_parked(records, count, deliverer, non_reader_starts,
                                                    non_reader_count, NULL, 0u);
}

rvq_verdict recomp_vblank_quiescence_evaluate_parked(const kernel_thread_record *records,
                                                     unsigned count, uint32_t deliverer,
                                                     const uint32_t *non_reader_starts,
                                                     unsigned non_reader_count,
                                                     const uint32_t *parked_handles,
                                                     unsigned parked_count)
{
    rvq_view views[KERNEL_THREAD_MAX];
    const unsigned limit = records == NULL ? 0u : count < KERNEL_THREAD_MAX ? count : KERNEL_THREAD_MAX;
    for (unsigned i = 0u; i < limit; i++) {
        views[i] = view_of(&records[i]);
    }
    rvq_c_verdict out;
    tsfp_vblank_quiescence_evaluate(views, limit, deliverer, non_reader_starts, non_reader_count,
                                    parked_handles, parked_count, &out);
    return (rvq_verdict){.holds = out.holds, .reason = (rvq_reason)out.reason, .handle = out.handle,
                         .start_routine = out.start_routine, .examined = out.examined};
}

void recomp_vblank_quiescence_configure(bool enabled)
{
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
    tsfp_second_vblank_quiescence_configure(enabled);
#else
    atomic_store(&enabled_flag, enabled);
#endif
}

bool recomp_vblank_quiescence_enabled(void)
{
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
    return tsfp_second_vblank_quiescence_enabled();
#else
    return atomic_load(&enabled_flag);
#endif
}

rvq_verdict recomp_vblank_quiescence_check(uint32_t deliverer, uint32_t site)
{
    return recomp_vblank_quiescence_check_parked(deliverer, site, NULL, 0u);
}

rvq_verdict recomp_vblank_quiescence_check_parked(uint32_t deliverer, uint32_t site,
                                                  const uint32_t *parked_handles,
                                                  unsigned parked_count)
{
    rvq_verdict verdict = {.holds = true, .reason = RVQ_HOLDS};
    if (!recomp_vblank_quiescence_enabled()) {
        return verdict;
    }
    kernel_thread_record records[KERNEL_THREAD_MAX];
    const unsigned count = kernel_thread_snapshot(records, KERNEL_THREAD_MAX);
    unsigned table_count = 0u;
    const uint32_t *table = recomp_vblank_quiescence_default_non_readers(&table_count);
    verdict = recomp_vblank_quiescence_evaluate_parked(records, count, deliverer, table, table_count,
                                                       parked_handles, parked_count);
    if (!verdict.holds) {
        static _Thread_local char text[192]; /* host_stop keeps the pointer */
        snprintf(text, sizeof text,
                 "vblank delivery quiescence refused: %s (thread %#x started at %#x)",
                 recomp_vblank_quiescence_reason_name(verdict.reason),
                 (unsigned)verdict.handle, (unsigned)verdict.start_routine);
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, site, 0u, text);
        abort();
    }
    return verdict;
}

bool recomp_vblank_quiescence_is_registered_reader(uint32_t return_address, uint32_t start_routine)
{
    return tsfp_vblank_quiescence_is_registered_reader(return_address, start_routine);
}

static bool getter_registered(uint32_t handle, uint32_t return_address)
{
    kernel_thread_record record;
    return kernel_thread_get(handle, &record) &&
           recomp_vblank_quiescence_is_registered_reader(return_address, record.start_routine);
}

void recomp_vblank_quiescence_check_getter(uint32_t handle, uint32_t return_address)
{
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
    if (!tsfp_second_vblank_quiescence_check_getter(handle, return_address, getter_registered)) {
#else
    if (!recomp_vblank_quiescence_enabled()) {
        return;
    }
    atomic_fetch_add(&getter_checks, 1u);
    if (!getter_registered(handle, return_address)) {
#endif
        host_run_stop(HOST_STOP_XDK_UNIMPLEMENTED, RVQ_GETTER, 0u,
                      "vblank counter getter reached by an unregistered reader");
        abort();
    }
}

unsigned recomp_vblank_quiescence_getter_checks(void)
{
#ifdef TSFP_SECOND_VBLANK_RUST_POLICY
    return tsfp_second_vblank_quiescence_getter_checks();
#else
    return atomic_load(&getter_checks);
#endif
}

void recomp_vblank_quiescence_describe(const kernel_thread_record *record, char *out, unsigned size)
{
    snprintf(out, size, "member=0x%X start=0x%X state=%s", (unsigned)record->handle,
             (unsigned)record->start_routine,
             recomp_vblank_quiescence_state_name(recomp_vblank_quiescence_classify(record)));
}
