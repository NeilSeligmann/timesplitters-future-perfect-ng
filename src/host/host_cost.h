/* SPDX-License-Identifier: GPL-3.0-or-later
 * T3824: process CPU time and user instruction count of a run (TSFP_HOST_COST=1), see host_cost.c. */
#ifndef TSFP_HOST_HOST_COST_H
#define TSFP_HOST_HOST_COST_H

#include <stdio.h>

void host_cost_start(void);
void host_cost_report(FILE *out);

#endif /* TSFP_HOST_HOST_COST_H */
