/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_HOST_RUST_PORTS_H
#define TSFP_HOST_RUST_PORTS_H
#include <stdbool.h>
#include <stdint.h>
bool host_rust_ports_configure(const char *spec);
bool host_rust_shadow_configure(const char *spec, uint32_t promote);
bool host_rust_shadow_stats(uint32_t va, uint64_t *clean, uint64_t *diff, bool *promoted,
                            uint64_t *calls, uint64_t *unavailable);
bool host_rust_weapon_hands_configure(bool enabled);
bool host_rust_rea_ai_configure(bool enabled);
bool host_rust_rea_ai_configure_function(uint32_t va, bool enabled);
/* T2135: --retire-gamestate off|own|audit (NULL: keep the build default); false and a message on stderr when refused. */
bool host_rust_retire_configure(const char *mode);
/* T2138: --retire-player off|own|audit, same contract for the PlayerSet owner. */
bool host_rust_retire_player_configure(const char *mode);
void host_rust_ports_report(void);
/* T2143: --perf-log, per-root cost table (cycles around every root call), printed at the stop. */
void host_rust_ports_perf_enable(void);
void host_rust_ports_perf_report(void);
/* T2143: names the accepted --rust-ports forms and groups after a refusal. */
void host_rust_ports_explain(const char *spec);
/* T2145: crash-report extra: last indirect call targets and the last Rust-port roots (the Rust no_std panic handler already
 * prints RUST-PANIC file:line, T1986). */
void host_crash_extra_guest_calls(int fd);
#endif
