#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "recomp_abi.h"
#include "recomp_cooperative.h"
#include "host_options.h"
#include "kernel_thread.h"
extern bool second_vblank_policy, vblank_schedule_trace, vblank_readers_trace;
extern TSFP_RECOMP_TLS uint32_t callback_thread_handle;
void host_timers_safepoint(uint32_t callee, uint32_t return_address_override);
bool host_timers_prepare(bool second, recomp_cooperative_provider provider);
bool host_timers_cleanup(void);
void host_timers_bind_owner(const kernel_thread_launch *launch);
int host_timers_configure_budget(const options *opts);
int host_timers_configure_interactive(bool interactive, bool (*closed)(void));
void host_timers_report(const options *opts);
void host_timers_configure_quiescence(bool trace, bool check);
