/* SPDX-License-Identifier: GPL-3.0-or-later */
/*
 * T2260/T3934: per-root timer for the title frame 0x3C440. The implementation is Rust (crates/tsfp-frame-phase, C ABI in
 * crates/tsfp-kernel-clock-capi/src/frame_phase.rs); this header only declares that ABI. Off unless --frame-phase-timer FILE.
 * The Rust clock library is required (TSFP_BUILD_KCLOCK=0 no longer links, T3934).
 */
#ifndef TSFP_FRAME_PHASE_TIMER_H
#define TSFP_FRAME_PHASE_TIMER_H

#include <stdbool.h>
#include <stdint.h>

bool tsfp_frame_phase_start(const char *path);
bool tsfp_frame_phase_enabled(void);
void tsfp_frame_phase_note(uint32_t return_site);
void tsfp_frame_phase_stop(int report_fd);

#endif
