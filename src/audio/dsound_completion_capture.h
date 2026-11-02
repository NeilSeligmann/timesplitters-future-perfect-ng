/* SPDX-License-Identifier: GPL-3.0-or-later
 * Opt-in ordered completion observer. Caller holds the completion mutex. */
#ifndef TSFP_DSOUND_COMPLETION_CAPTURE_H
#define TSFP_DSOUND_COMPLETION_CAPTURE_H
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef COMPLETION_CAPTURE_LIMIT
#define COMPLETION_CAPTURE_LIMIT (64u * 1024u * 1024u)
#endif
static FILE *completion_capture_file;
static bool completion_capture_checked, completion_capture_failed, completion_capture_closed;
static uint64_t completion_capture_hash = UINT64_C(1469598103934665603);
static uint64_t completion_capture_lines, completion_capture_bytes;
static bool completion_capture_process_pending;
static uint32_t completion_capture_stream;
static uint64_t completion_capture_serial, completion_capture_ticks;

static void completion_capture_write(const char *line)
{
    if (!completion_capture_file || completion_capture_failed || completion_capture_closed) return;
    const size_t n = strlen(line);
    /* Reserve the longest seal in advance. Never write part of a record. */
    if (n == 0u || n > 255u || completion_capture_bytes + n + 64u > COMPLETION_CAPTURE_LIMIT ||
        fwrite(line, 1u, n, completion_capture_file) != n) {
        completion_capture_failed = true;
        return;
    }
    for (size_t i=0u; i<n; ++i)
        completion_capture_hash = (completion_capture_hash ^ (unsigned char)line[i]) * UINT64_C(1099511628211);
    completion_capture_bytes += n;
    completion_capture_lines++;
}
static void completion_capture_open(uint64_t frequency, bool dowork)
{
    if (completion_capture_checked) return;
    completion_capture_checked = true;
    const char *path = getenv("TSFP_AUDIO_COMPLETION_CAPTURE");
    if (!path || !*path) return;
    completion_capture_file = fopen(path, "wx");
    if (!completion_capture_file) return;
    completion_capture_write("ACMP1\n");
    char config[96];
    snprintf(config, sizeof config, "CFG %llu %u\n", (unsigned long long)frequency, (unsigned)dowork);
    completion_capture_write(config);
}
static void completion_capture_observe(const char *line, uint64_t frequency, bool dowork)
{
    /* Buffer-only records do not mutate stream completion state. */
    if (!strncmp(line, "CB", 2) || !strncmp(line, "CCFG ", 5) || !strncmp(line, "CSD ", 4) || !strncmp(line, "CSM ", 4)) return;
    completion_capture_open(frequency, dowork);
    if (!strncmp(line, "CSP ", 4)) {
        const size_t n=strlen(line);
        if (n >= 2u && !strcmp(line+n-2u, " 1")) return; /* full was recorded at begin */
        unsigned stream=0u; unsigned long long serial=0u,ticks=0u;
        if (sscanf(line, "CSP %u %llu %llu", &stream, &serial, &ticks)!=3 ||
            !completion_capture_process_pending || stream!=completion_capture_stream ||
            serial!=completion_capture_serial || ticks!=completion_capture_ticks)
            completion_capture_write("GAP process-pairing\n");
        completion_capture_process_pending=false;
    } else if (completion_capture_process_pending) {
        completion_capture_write("GAP interleaved-process\n");
    }
    const bool unsupported=!strncmp(line, "CSF ", 4);
    char record[256];
    const int n=snprintf(record, sizeof record, "%s\n", line);
    if (n < 0 || (size_t)n >= sizeof record) { completion_capture_failed=true; return; }
    completion_capture_write(record);
    if (unsupported) completion_capture_write("GAP unsupported-format-or-frequency\n");
}
static void completion_capture_format(uint32_t stream, uint64_t serial, uint64_t ticks,
    uint32_t old_hz, uint32_t new_hz, uint32_t address, bool accepted, uint64_t frequency, bool dowork)
{
    completion_capture_open(frequency,dowork);
    if (completion_capture_process_pending) completion_capture_write("GAP interleaved-process\n");
    char record[192];
    snprintf(record,sizeof record,"CSM %u %llu %llu %u %u %u %u\n",stream,
        (unsigned long long)serial,(unsigned long long)ticks,old_hz,new_hz,address,(unsigned)accepted);
    completion_capture_write(record);
    completion_capture_write("GAP unsupported-format-or-frequency\n");
}
static void completion_capture_begin(uint32_t stream, uint64_t serial, uint64_t ticks,
                                     uint32_t max_packets, bool full, uint64_t frequency, bool dowork)
{
    completion_capture_open(frequency, dowork);
    if (!completion_capture_file) return;
    if (completion_capture_process_pending) completion_capture_write("GAP nested-process\n");
    char record[160];
    snprintf(record,sizeof record,"CPB %u %llu %llu %u %u\n",stream,
             (unsigned long long)serial,(unsigned long long)ticks,max_packets,(unsigned)full);
    completion_capture_write(record);
    completion_capture_process_pending=!full;
    completion_capture_stream=stream;completion_capture_serial=serial;completion_capture_ticks=ticks;
}
static void completion_capture_gap(void)
{
    if (completion_capture_file) completion_capture_write("GAP configuration-change\n");
}
static void completion_capture_seal(bool quiescent)
{
    if (!completion_capture_file || completion_capture_closed) return;
    if (quiescent && !completion_capture_failed && !completion_capture_process_pending) {
        if (fprintf(completion_capture_file, "END %llu %016llx\n",
                    (unsigned long long)completion_capture_lines,
                    (unsigned long long)completion_capture_hash) < 0 || fflush(completion_capture_file) != 0)
            completion_capture_failed=true;
    }
    (void)fclose(completion_capture_file);
    completion_capture_closed=true;
}
#endif
