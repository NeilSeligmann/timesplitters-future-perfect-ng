/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_SHADOW_PORT_LEAF_H
#define TSFP_SHADOW_PORT_LEAF_H
#include "shadow_port_policy.h"
#include <stdint.h>

/* The first four words match the four-register Rust ABI. */
typedef struct { uint32_t eax, ecx, edx, esp, ebx, esi, edi, ebp; } shadow_leaf_regs;
typedef void (*shadow_leaf_rust_fn)(shadow_leaf_regs *, uintptr_t);
typedef void (*shadow_leaf_lifted_fn)(shadow_leaf_regs *, void *);
typedef struct {
    int compared, fault, mismatch, unavailable, primary;
    unsigned first_register;
    uint32_t lifted_value, rust_value;
} shadow_leaf_result;

/* The audited Rust root must read only [esp+4, esp+8), write no guest
 * memory, perform no callbacks, hold no locks and not abort. The lifted
 * callback is authoritative until promotion or after a fault. */
shadow_leaf_result shadow_leaf_execute(shadow_port_policy *policy, shadow_leaf_regs *regs,
                                       uint32_t argument, shadow_leaf_rust_fn rust,
                                       shadow_leaf_lifted_fn lifted, void *context);
#endif
