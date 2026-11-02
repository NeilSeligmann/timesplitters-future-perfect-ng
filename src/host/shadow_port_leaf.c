/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "shadow_port_leaf.h"
#include "shadow_port_guard.h"
#include "shadow_port_stack.h"
#include <stddef.h>

typedef struct { shadow_leaf_rust_fn fn; shadow_leaf_regs *regs; uintptr_t offset; } rust_call;
static int run_rust(void *opaque)
{
    rust_call *call = opaque;
    call->fn(call->regs, call->offset);
    return 0;
}

shadow_leaf_result shadow_leaf_execute(shadow_port_policy *policy, shadow_leaf_regs *regs,
                                       uint32_t argument, shadow_leaf_rust_fn rust,
                                       shadow_leaf_lifted_fn lifted, void *context)
{
    shadow_leaf_result verdict = {0};
    shadow_route route = shadow_policy_begin(policy);
    if (route == SHADOW_LIFTED) { lifted(regs, context); return verdict; }
    uintptr_t offset;
    if (regs->esp > UINT32_MAX - 7u || !shadow_stack_begin(regs->esp + 4u, argument, &offset)) {
        if (route == SHADOW_COMPARE) shadow_policy_cancel(policy);
        verdict.unavailable = 1;
        lifted(regs, context);
        return verdict;
    }
    shadow_leaf_regs rust_regs = *regs;
    rust_call call = {rust, &rust_regs, offset};
    host_stop fault;
    int result;
    shadow_guard_status status = shadow_port_guarded_call(run_rust, &call, &result, &fault);
    shadow_stack_end();
    if (status == SHADOW_GUARD_UNAVAILABLE) {
        if (route == SHADOW_COMPARE) shadow_policy_cancel(policy);
        verdict.unavailable = 1;
        lifted(regs, context);
        return verdict;
    }
    if (status == SHADOW_GUARD_FAULT) {
        if (route == SHADOW_COMPARE) shadow_policy_finish(policy, false);
        else shadow_policy_primary_fault(policy);
        verdict.fault = 1;
        lifted(regs, context);
        return verdict;
    }
    if (route == SHADOW_PRIMARY) {
        shadow_policy_lock(policy);
        bool commit = !policy->disabled;
        if (commit) *regs = rust_regs;
        shadow_policy_unlock(policy);
        if (!commit) lifted(regs, context);
        else verdict.primary = 1;
        return verdict;
    }
    lifted(regs, context);
    const uint32_t left[] = {regs->eax, regs->ecx, regs->edx, regs->esp,
                             regs->ebx, regs->esi, regs->edi, regs->ebp};
    const uint32_t right[] = {rust_regs.eax, rust_regs.ecx, rust_regs.edx, rust_regs.esp,
                              rust_regs.ebx, rust_regs.esi, rust_regs.edi, rust_regs.ebp};
    verdict.compared = 1;
    for (unsigned i = 0; i < 8; ++i) if (left[i] != right[i]) {
        verdict.mismatch = 1; verdict.first_register = i;
        verdict.lifted_value = left[i]; verdict.rust_value = right[i]; break;
    }
    shadow_policy_finish(policy, !verdict.mismatch);
    return verdict;
}
