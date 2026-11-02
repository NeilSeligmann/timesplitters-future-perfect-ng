/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3824: the process cost of a run, for A/B overhead measurements on noisy shared hosts (wall time is not usable
 * there). With TSFP_HOST_COST=1 the host opens a user-space instruction counter at start (perf_event_open, inherit
 * so every guest thread created later counts) and prints at exit:
 *   host cost: user_s=U sys_s=S instructions=N
 * instructions=unavailable when perf events are not permitted (perf_event_paranoid). Off by default: nothing runs.
 */
#define _GNU_SOURCE
#include "host_cost.h"

#include <linux/perf_event.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <unistd.h>

static int counter_fd = -1;
static int enabled;

void host_cost_start(void)
{
    const char *flag = getenv("TSFP_HOST_COST");
    enabled = flag != NULL && strcmp(flag, "1") == 0;
    if (!enabled || counter_fd >= 0) return;
    struct perf_event_attr attr;
    memset(&attr, 0, sizeof attr);
    attr.size = sizeof attr;
    attr.type = PERF_TYPE_HARDWARE;
    attr.config = PERF_COUNT_HW_INSTRUCTIONS;
    attr.exclude_kernel = 1;
    attr.exclude_hv = 1;
    attr.inherit = 1;
    counter_fd = (int)syscall(SYS_perf_event_open, &attr, 0, -1, -1, 0);
}

void host_cost_report(FILE *out)
{
    if (!enabled) return;
    struct rusage usage;
    if (getrusage(RUSAGE_SELF, &usage) != 0) memset(&usage, 0, sizeof usage);
    const double user = (double)usage.ru_utime.tv_sec + (double)usage.ru_utime.tv_usec / 1e6;
    const double sys = (double)usage.ru_stime.tv_sec + (double)usage.ru_stime.tv_usec / 1e6;
    uint64_t instructions = 0;
    const int have = counter_fd >= 0 && read(counter_fd, &instructions, sizeof instructions) == (ssize_t)sizeof instructions;
    if (have)
        fprintf(out, "host cost: user_s=%.3f sys_s=%.3f instructions=%llu\n", user, sys, (unsigned long long)instructions);
    else
        fprintf(out, "host cost: user_s=%.3f sys_s=%.3f instructions=unavailable\n", user, sys);
    fflush(out);
}
