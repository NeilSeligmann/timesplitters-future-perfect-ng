/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Opt-in per-engine observer. An incomplete/oversized stream has no seal. */
#ifndef TSFP_AUDIO_STRETCH_CAPTURE_H
#define TSFP_AUDIO_STRETCH_CAPTURE_H
#include <stdatomic.h>
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define WS_CAP_DEFAULT (64u * 1024u * 1024u)
#define WS_CAP_MAX (256u * 1024u * 1024u)
typedef struct { FILE *file; uint64_t hash, bytes, count, limit; int valid; } ws_capture;
static atomic_ulong ws_capture_serial;

static void ws_write(ws_capture *c, const void *data, size_t n)
{
    if (!c || !c->valid) return;
    if (n > c->limit - c->bytes || fwrite(data, 1, n, c->file) != n) {
        fprintf(stderr, "WSOLA capture incomplete: size limit or write failure\n");
        c->valid = 0; return;
    }
    const unsigned char *p = data;
    for (size_t i = 0; i < n; i++) c->hash = (c->hash ^ p[i]) * UINT64_C(1099511628211);
    c->bytes += n;
}
static void ws_u64(ws_capture *c, uint64_t v)
{
    unsigned char b[8];
    for (unsigned i = 0; i < 8; i++) b[i] = (unsigned char)(v >> (8u * i));
    ws_write(c, b, sizeof b);
}
static ws_capture *ws_begin(uint16_t channels, size_t overlap)
{
    const char *prefix = getenv("TSFP_WSOLA_CAPTURE");
    if (!prefix || !*prefix) return NULL;
    uint64_t limit = WS_CAP_DEFAULT;
    const char *setting = getenv("TSFP_WSOLA_CAPTURE_MAX_BYTES");
    if (setting) {
        for (const char *p = setting; *p; p++)
            if (*p < '0' || *p > '9') { fprintf(stderr, "WSOLA capture invalid byte limit\n"); return NULL; }
        char *end = NULL;
        errno = 0;
        unsigned long long parsed = strtoull(setting, &end, 10);
        if (!*setting || *setting == '-' || errno || !end || *end || parsed < 64 || parsed > WS_CAP_MAX) {
            fprintf(stderr, "WSOLA capture invalid byte limit\n"); return NULL;
        }
        limit = parsed;
    }
    char name[4096];
    unsigned long serial = atomic_fetch_add(&ws_capture_serial, 1);
    /* The replay queue retains and transfers .trace artifacts. */
    int len = snprintf(name, sizeof name, "%s.%lu.trace", prefix, serial);
    if (len < 0 || (size_t)len >= sizeof name) { fprintf(stderr, "WSOLA capture path too long\n"); return NULL; }
    FILE *f = fopen(name, "wbx");
    if (!f) { fprintf(stderr, "WSOLA capture open failed\n"); return NULL; }
    ws_capture *c = calloc(1, sizeof *c);
    if (!c) { fprintf(stderr, "WSOLA capture allocation failed\n"); fclose(f); return NULL; }
    c->file = f; c->valid = 1; c->limit = limit; c->hash = UINT64_C(1469598103934665603);
    ws_write(c, "WSOLAC1\n", 8); ws_u64(c, channels); ws_u64(c, overlap);
    return c;
}
static void ws_samples(ws_capture *c, const int16_t *samples, size_t count)
{
    for (size_t i = 0; c && c->valid && i < count; i++) {
        uint16_t v = (uint16_t)samples[i];
        unsigned char b[2] = {(unsigned char)v, (unsigned char)(v >> 8)};
        ws_write(c, b, 2);
    }
}
/* Input is logged after the original operation, so aliased buffers would not
 * preserve its entry state. Refuse observation without changing the call. */
static void ws_admit(ws_capture *c, size_t channels, size_t overlap,
                     const int16_t *in, size_t frames, const int16_t *out)
{
    if (!c || !c->valid) return;
    if (!channels || frames > WS_CAP_MAX / 2 / channels || overlap > WS_CAP_MAX / 2 / channels) {
        fprintf(stderr, "WSOLA capture incomplete: geometry exceeds bound\n"); c->valid = 0; return;
    }
    size_t in_bytes = frames * channels * 2, out_bytes = overlap * channels * 2;
    uintptr_t a = (uintptr_t)in, b = (uintptr_t)out;
    if (in_bytes && (!in || !out || a > UINTPTR_MAX - in_bytes || b > UINTPTR_MAX - out_bytes ||
                     (a < b + out_bytes && b < a + in_bytes))) {
        fprintf(stderr, "WSOLA capture incomplete: overlapping or invalid buffer spans\n"); c->valid = 0;
    }
}
static void ws_admit_chosen(ws_capture *c, const size_t *chosen, const int16_t *in,
                            size_t input_bytes, const int16_t *out, size_t output_bytes)
{
    if (!c || !c->valid || !chosen) return;
    uintptr_t a = (uintptr_t)chosen, b = (uintptr_t)in, d = (uintptr_t)out;
    if (a > UINTPTR_MAX - sizeof *chosen || b > UINTPTR_MAX - input_bytes ||
        d > UINTPTR_MAX - output_bytes ||
        (input_bytes && a < b + input_bytes && b < a + sizeof *chosen) ||
        (output_bytes && a < d + output_bytes && d < a + sizeof *chosen)) {
        fprintf(stderr, "WSOLA capture incomplete: aliased chosen offset\n"); c->valid = 0;
    }
}
static void ws_event(ws_capture *c, unsigned char op, size_t channels, size_t overlap,
                     const int16_t *in, size_t frames, size_t nominal, int ok,
                     size_t chosen, int primed, const int16_t *out)
{
    if (!c || !c->valid) return;
    /* Bound even failed calls, whose input is intentionally not dereferenced. */
    if (!channels || frames > WS_CAP_MAX / 2 / channels || overlap > WS_CAP_MAX / 2 / channels) {
        fprintf(stderr, "WSOLA capture incomplete: geometry exceeds bound\n"); c->valid = 0; return;
    }
    uint64_t payload = ok ? (uint64_t)overlap * channels * 2u : 0u;
    if (ok && in) payload += (uint64_t)frames * channels * 2u;
    /* Reserve a seal as well; reaching the cap deliberately leaves no seal. */
    if (27u + payload + 17u > c->limit - c->bytes) {
        fprintf(stderr, "WSOLA capture incomplete: size limit\n"); c->valid = 0; return;
    }
    ws_write(c, &op, 1); ws_u64(c, frames); ws_u64(c, nominal);
    unsigned char result = ok ? 1 : 0; ws_write(c, &result, 1); ws_u64(c, chosen);
    unsigned char state = primed ? 1 : 0; ws_write(c, &state, 1);
    if (ok && in) ws_samples(c, in, frames * channels);
    if (ok) ws_samples(c, out, overlap * channels);
    c->count++;
}
static void ws_end(ws_capture *c)
{
    if (!c) return;
    if (c->valid) {
        uint64_t hash = c->hash, count = c->count;
        unsigned char seal = 255;
        ws_write(c, &seal, 1); ws_u64(c, count); ws_u64(c, hash);
    }
    fclose(c->file); free(c);
}
#endif
