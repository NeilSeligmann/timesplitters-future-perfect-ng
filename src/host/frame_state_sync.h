/* SPDX-License-Identifier: GPL-3.0-or-later
 * T3879 TEMPORARY-SHIM: keeps raw lifted memory and the Rust-owned frame words equal at call boundaries. */
#ifndef TSFP_FRAME_STATE_SYNC_H
#define TSFP_FRAME_STATE_SYNC_H
#include <stddef.h>
/* Weak: targets that do not link the shim (offline runners, ext tests) skip it. */
void tsfp_frame_state_sync_in(void) __attribute__((weak));
void tsfp_frame_state_sync_out(void) __attribute__((weak));
#define TSFP_FRAME_SYNC_IN() do { if (tsfp_frame_state_sync_in != NULL) tsfp_frame_state_sync_in(); } while (0)
#define TSFP_FRAME_SYNC_OUT() do { if (tsfp_frame_state_sync_out != NULL) tsfp_frame_state_sync_out(); } while (0)
#endif
