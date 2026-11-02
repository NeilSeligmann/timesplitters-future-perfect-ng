/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_SHADOW_PORT_GUARD_H
#define TSFP_SHADOW_PORT_GUARD_H
#include "host_runtime.h"

typedef int (*shadow_guarded_fn)(void *context);
typedef enum { SHADOW_GUARD_RETURNED, SHADOW_GUARD_FAULT, SHADOW_GUARD_UNAVAILABLE } shadow_guard_status;
/* The callback must run on isolated state and must not hold locks or own
 * resources that require unwinding. Only synchronous host stop signals are
 * recovered; SIGABRT and Rust aborting panics are outside this contract.
 * Non-fault host stops are rethrown to the enclosing run. */
shadow_guard_status shadow_port_guarded_call(shadow_guarded_fn fn, void *context,
                                            int *result, host_stop *fault);
#endif
