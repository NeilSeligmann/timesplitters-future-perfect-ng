/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "shadow_port_guard.h"
#include <stddef.h>

shadow_guard_status shadow_port_guarded_call(shadow_guarded_fn fn, void *context,
                                            int *result, host_stop *fault)
{
    volatile host_run_scope scope = HOST_RUN_SCOPE_INITIALIZER;
    if (fn == NULL || result == NULL || fault == NULL || !host_run_armed() ||
        !host_run_scope_init(&scope)) return SHADOW_GUARD_UNAVAILABLE;
    if (sigsetjmp(*host_run_scope_jmp(&scope), 0) == 0) {
        if (!host_run_scope_push(&scope)) return SHADOW_GUARD_UNAVAILABLE;
        *result = fn(context);
        if (!host_run_scope_pop(&scope)) return SHADOW_GUARD_UNAVAILABLE;
        return SHADOW_GUARD_RETURNED;
    }
    *fault = *host_run_result();
    if (!host_run_scope_pop(&scope)) return SHADOW_GUARD_UNAVAILABLE;
    if (fault->reason != HOST_STOP_FAULT) host_run_rethrow(fault);
    return SHADOW_GUARD_FAULT;
}
