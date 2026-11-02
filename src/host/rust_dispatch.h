/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_RUST_DISPATCH_H
#define TSFP_RUST_DISPATCH_H
/* T3869 lane B phase 1 (docs/t-rust-dispatcher-plan.md): the Rust dispatch table and guest-call
 * trampoline (crates/tsfp-dispatch-capi), linked only with -DTSFP_RUST_DISPATCH=ON and selected only
 * with TSFP_RUST_DISPATCH=1 at startup. The C dispatcher stays the default and the oracle: on
 * selection every generated dispatch row is checked against C recomp_lookup before any guest code
 * runs, and any disagreement refuses the run. */
#include <stdbool.h>
#include <stdint.h>

/* After recomp_dispatch_init, before the first guest thread. False only when TSFP_RUST_DISPATCH=1
 * was asked for and could not be honoured (library ABI, lift without lifter patch 42, a refused
 * table, or a Rust/C disagreement); the reason is on stderr and the caller must not run. */
bool rust_dispatch_startup(void);
/* True after a successful selection. */
bool rust_dispatch_selected(void);
/* The Rust twins of recomp_guest_call_stdcall and guest_call_cdecl (same contract). */
bool rust_dispatch_guest_call_stdcall(uint32_t address, const uint32_t *arguments, unsigned count);
bool rust_dispatch_guest_call_cdecl(uint32_t address, const uint32_t *arguments, unsigned count,
                                    uint32_t *result, double *st0);
#endif
