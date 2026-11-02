/* SPDX-License-Identifier: GPL-3.0-or-later
 * Comparison primitive for callers that can provide isolated state buffers. */
#ifndef TSFP_SHADOW_PORT_COMPARE_H
#define TSFP_SHADOW_PORT_COMPARE_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "shadow_port_policy.h"

typedef int (*shadow_port_run)(void *state, size_t size, uint64_t *result, void *context);
typedef struct {
    int fault;
    int lifted_fault;
    int result_differs;
    size_t first_byte; /* size if memory matches */
    uint64_t lifted_result, rust_result;
    unsigned char lifted_byte, rust_byte;
} shadow_port_verdict;

/* The caller supplies reusable, non-overlapping buffers. The lifted buffer
 * becomes authoritative, including when the Rust callback reports a fault. */
static inline shadow_port_verdict shadow_port_compare(
    const void *initial, size_t size, void *lifted_state, void *rust_state,
    shadow_port_run lifted, shadow_port_run rust, void *context)
{
    shadow_port_verdict v = {.first_byte = size};
    memcpy(lifted_state, initial, size);
    memcpy(rust_state, initial, size);
    const int lifted_status = lifted(lifted_state, size, &v.lifted_result, context);
    v.lifted_fault = lifted_status != 0;
    if (v.lifted_fault) { v.fault = 1; return v; }
    const int rust_status = rust(rust_state, size, &v.rust_result, context);
    v.fault = lifted_status != 0 || rust_status != 0;
    v.result_differs = v.lifted_result != v.rust_result;
    const unsigned char *a = lifted_state, *b = rust_state;
    for (size_t i = 0; i < size; ++i) {
        if (a[i] != b[i]) {
            v.first_byte = i; v.lifted_byte = a[i]; v.rust_byte = b[i]; break;
        }
    }
    return v;
}

static inline int shadow_port_equal(shadow_port_verdict v, size_t size)
{
    return !v.fault && !v.result_differs && v.first_byte == size;
}

/* All buffers and callback contexts must be isolated from live state. For a
 * primary fault, discard its scratch writes before invoking the fallback. */
static inline int shadow_port_execute(shadow_port_policy *policy, void *state, size_t size,
                                     void *lifted_scratch, void *rust_scratch,
                                     shadow_port_run lifted, shadow_port_run rust, void *context,
                                     uint64_t *result, shadow_port_verdict *verdict)
{
    *verdict = (shadow_port_verdict){.first_byte = size};
    shadow_route route = shadow_policy_begin(policy);
    if (route == SHADOW_LIFTED) return lifted(state, size, result, context);
    if (route == SHADOW_PRIMARY) {
        memcpy(rust_scratch, state, size);
        if (rust(rust_scratch, size, result, context) == 0) {
            /* Another thread may have disabled this root while it ran. */
            shadow_policy_lock(policy);
            int commit = !policy->disabled;
            if (commit) memcpy(state, rust_scratch, size);
            shadow_policy_unlock(policy);
            if (commit) return 0;
            return lifted(state, size, result, context);
        }
        shadow_policy_primary_fault(policy);
        *verdict = (shadow_port_verdict){.fault = 1, .first_byte = size};
        return lifted(state, size, result, context);
    }
    *verdict = shadow_port_compare(state, size, lifted_scratch, rust_scratch, lifted, rust, context);
    if (verdict->lifted_fault) shadow_policy_cancel(policy);
    else shadow_policy_finish(policy, shadow_port_equal(*verdict, size));
    memcpy(state, lifted_scratch, size);
    *result = verdict->lifted_result;
    return verdict->lifted_fault ? -1 : 0;
}
#endif
