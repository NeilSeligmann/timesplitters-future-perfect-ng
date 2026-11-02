/* SPDX-License-Identifier: GPL-3.0-or-later
 * T3742: the two pieces of the offline runner that need C.
 *  1. A per-thread sigsetjmp guard: the Rust fault handler escapes a guest run that faulted outside
 *     the tracked arena (or trapped) back to the runner, which records the entry as `fault`.
 *     sigsetjmp saves the signal mask, so SIGSEGV is unblocked again after the jump.
 *  2. The host API lookup: `tsfp_t2o_host_api` is defined by the C host (src/host/rust_ports.c) only
 *     in binaries that link the lifted C, the Rust ports and the dispatch. The weak reference is
 *     NULL everywhere else (unit tests, the tsfp-bevy stub build), so this crate links anywhere. */
#include <setjmp.h>
#include <stddef.h>
#include <stdint.h>

static __thread sigjmp_buf *t2o_jump;
static __thread uintptr_t t2o_fault_address;
static __thread int t2o_fault_signal;

int t2o_guarded(void (*body)(void *), void *context)
{
    sigjmp_buf here;
    sigjmp_buf *outer = t2o_jump;
    if (sigsetjmp(here, 1) != 0) {
        t2o_jump = outer;
        return 1;
    }
    t2o_jump = &here;
    body(context);
    t2o_jump = outer;
    return 0;
}

/* Called from the signal handler. Returns only when no guard is armed on this thread. */
void t2o_escape(int signal, uintptr_t address)
{
    if (t2o_jump == NULL) return;
    t2o_fault_signal = signal;
    t2o_fault_address = address;
    siglongjmp(*t2o_jump, 1);
}

int t2o_guard_armed(void) { return t2o_jump != NULL; }
uintptr_t t2o_last_fault_address(void) { return t2o_fault_address; }
int t2o_last_fault_signal(void) { return t2o_fault_signal; }

struct tsfp_t2o_host_api;
extern const struct tsfp_t2o_host_api tsfp_t2o_host_api __attribute__((weak));
const struct tsfp_t2o_host_api *t2o_host_api(void) { return &tsfp_t2o_host_api; }
