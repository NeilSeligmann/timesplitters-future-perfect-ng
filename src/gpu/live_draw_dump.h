/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T1226: opt-in per draw signature dump of the live renderer, observation only. TSFP_LIVE_DRAW_DUMP=PATH appends one text line per
 * draw (D), per refused draw (R) and per texture stage (T). Nothing changes when the variable is unset. Header only (static), so no
 * test link list needs a new object. Every translation unit appends through its own FILE with line buffering.
 */
#ifndef TSFP_LIVE_DRAW_DUMP_H
#define TSFP_LIVE_DRAW_DUMP_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static inline bool live_draw_dump_full_admitted(uint32_t cull_enable, uint32_t primitive,
                                                uint32_t vertex_count, bool any_nonempty_draw)
{
    if (vertex_count == 0u) return false;
    return any_nonempty_draw ||
           (cull_enable == 1u && primitive == 6u && vertex_count >= 200u);
}

/* An unset selector keeps the capture broad; a set selector is a comma-separated list of decimal frames. */
static inline bool live_draw_dump_full_frame_matches(uint64_t frame, const char *selector)
{
    if (selector == NULL || selector[0] == '\0') return true;
    bool matched = false;
    const char *start = selector;
    for (const char *cursor = selector;; ++cursor) {
        if (*cursor != ',' && *cursor != '\0') continue;
        if (cursor == start) return false;
        uint64_t wanted = 0u;
        for (const char *digit = start; digit != cursor; ++digit) {
            if (*digit < '0' || *digit > '9') return false;
            const uint64_t value = (uint64_t)(*digit - '0');
            if (wanted > (UINT64_MAX - value) / 10u) return false;
            wanted = wanted * 10u + value;
        }
        if (wanted == frame) matched = true;
        if (*cursor == '\0') break;
        start = cursor + 1;
    }
    return matched;
}

/* A frame-N screenshot can be held until the first draw in N+1 is ready. */
static inline bool live_draw_dump_full_frame_wait_matches(uint64_t frame, const char *selector)
{
    return frame != 0u && live_draw_dump_full_frame_matches(frame - 1u, selector);
}

static inline FILE *live_draw_dump_file(void)
{
    static FILE *file;
    static int tried;
    if (!tried) {
        tried = 1;
        const char *path = getenv("TSFP_LIVE_DRAW_DUMP");
        if (path != NULL && path[0] != '\0') {
            file = fopen(path, "a");
            if (file != NULL) setvbuf(file, NULL, _IOLBF, 0);
        }
    }
    return file;
}

static inline void live_draw_dump_line(const char *format, ...) __attribute__((format(printf, 1, 2)));
static inline void live_draw_dump_line(const char *format, ...)
{
    FILE *file = live_draw_dump_file();
    if (file == NULL) return;
    va_list arguments;
    va_start(arguments, format);
    (void)vfprintf(file, format, arguments);
    va_end(arguments);
    (void)fputc('\n', file);
}

#endif
