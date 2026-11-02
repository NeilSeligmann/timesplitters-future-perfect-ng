/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_HOST_FILEIO_REPORT_H
#define TSFP_HOST_FILEIO_REPORT_H

#include <stdbool.h>
#include <stdio.h>

/* T3879 FIO-1: the Rust option selects snapshot aggregation, not table ownership.
 * TEMPORARY-SHIM: C counter/attempt/link access remains until FIO-3 retires the
 * physical file runtime. Both paths retain the existing report formatter. */
void host_fileio_report_attempts(FILE *out, bool rust_setup);
void host_fileio_report_links(FILE *out);

#endif
