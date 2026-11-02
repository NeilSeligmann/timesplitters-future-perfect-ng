/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3824 live hit census, see live_census.h.
 */
#define _GNU_SOURCE
#include "live_census.h"
#include "live_census_hit.h"

#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define PATH_CAP 1024u

#ifndef TSFP_LIVE_CENSUS_LIFT_KEY
#define TSFP_LIVE_CENSUS_LIFT_KEY "unknown"
#endif
#ifndef TSFP_LIVE_CENSUS_BUILD_SHA
#define TSFP_LIVE_CENSUS_BUILD_SHA "unknown"
#endif

extern live_census_func recomp_lookup(uint32_t xbox_va) __attribute__((weak));
extern uint32_t tsfp_rust_root(size_t index) __attribute__((weak));
extern uint64_t tsfp_rust_count(uint32_t va, bool rust) __attribute__((weak));

/* The hot path: LIVE_HIT(va) at the top of every lifted function (tools.live_census.transform) tests and sets
 * g_live_census_hits[va]. Zero-initialised bss, so untouched pages are never resident. */
uint8_t g_live_census_hits[LIVE_CENSUS_VA_SPAN];

static live_census_func (*s_lookup)(uint32_t va);
static uint32_t (*s_root)(size_t index);
static uint64_t (*s_count)(uint32_t va, bool rust);

/* Write state, prepared at start so the writer never allocates. */
static uint32_t *g_bucket_of; /* per function: its VA (index into g_live_census_hits) */
static uint8_t *g_file;       /* header + va table + bitmap */
static size_t g_file_size, g_count, g_bitmap_at;
static uint32_t g_ambiguous, g_image_base, g_image_size, g_last_hits, g_last_functions;
static char g_path[PATH_CAP], g_temp[PATH_CAP + 8u];
static char g_port_spec[48];
static char g_lift_key[33] = TSFP_LIVE_CENSUS_LIFT_KEY, g_build_sha[41] = TSFP_LIVE_CENSUS_BUILD_SHA;

void live_census_note_va(uint32_t va)
{
    LIVE_HIT(va);
}

void live_census_set_sources(live_census_func (*lookup)(uint32_t va), uint32_t (*root)(size_t index),
                                     uint64_t (*count)(uint32_t va, bool rust))
{
    s_lookup = lookup;
    s_root = root;
    s_count = count;
}

void live_census_set_port_spec(const char *spec)
{
    memset(g_port_spec, 0, sizeof g_port_spec);
    if (spec != NULL) strncpy(g_port_spec, spec, sizeof g_port_spec - 1u);
}

static void put32(uint8_t *at, uint32_t value)
{
    for (unsigned i = 0; i < 4u; i++) at[i] = (uint8_t)(value >> (8u * i));
}

static void put64(uint8_t *at, uint64_t value)
{
    for (unsigned i = 0; i < 8u; i++) at[i] = (uint8_t)(value >> (8u * i));
}

static void put_text(uint8_t *at, size_t cap, const char *text)
{
    memset(at, 0, cap);
    for (size_t i = 0; i < cap && text[i] != '\0'; i++) at[i] = (uint8_t)text[i];
}

static int index_of(uint32_t va)
{
    size_t low = 0, high = g_count;
    const uint8_t *vas = g_file + LIVE_CENSUS_HEADER_SIZE;
    while (low < high) {
        const size_t mid = low + (high - low) / 2u;
        const uint8_t *at = vas + 4u * mid;
        const uint32_t here = (uint32_t)at[0] | (uint32_t)at[1] << 8 | (uint32_t)at[2] << 16 | (uint32_t)at[3] << 24;
        if (here == va) return (int)mid;
        if (here < va) low = mid + 1u; else high = mid;
    }
    return -1;
}

bool live_census_start(uint32_t image_base, uint32_t image_size, const char *out_path)
{
    const char *enabled = getenv("TSFP_LIVE_CENSUS");
    if (enabled != NULL && strcmp(enabled, "0") == 0) return false;
    if (out_path == NULL) out_path = getenv("TSFP_LIVE_CENSUS_OUT");
    if (out_path == NULL || out_path[0] == '\0' || strlen(out_path) >= PATH_CAP) return false;
    live_census_func (*lookup)(uint32_t) = s_lookup != NULL ? s_lookup : recomp_lookup;
    if (lookup == NULL || g_file != NULL) return false;

    if ((uint64_t)image_base + image_size > LIVE_CENSUS_VA_SPAN) return false;
    size_t count = 0;
    for (uint64_t va = image_base; va < (uint64_t)image_base + image_size; va++)
        if (lookup((uint32_t)va) != NULL) count++;
    if (count == 0u) return false;
    g_bitmap_at = LIVE_CENSUS_HEADER_SIZE + 4u * count;
    g_file_size = g_bitmap_at + (count + 7u) / 8u;
    g_file = calloc(1, g_file_size);
    g_bucket_of = calloc(count, sizeof *g_bucket_of);
    if (g_file == NULL || g_bucket_of == NULL) {
        free(g_file);
        free(g_bucket_of);
        g_file = NULL;
        g_bucket_of = NULL;
        return false;
    }
    size_t at = 0;
    for (uint64_t va = image_base; va < (uint64_t)image_base + image_size && at < count; va++) {
        if (lookup((uint32_t)va) == NULL) continue;
        put32(g_file + LIVE_CENSUS_HEADER_SIZE + 4u * at, (uint32_t)va);
        g_bucket_of[at++] = (uint32_t)va;
    }
    g_count = at;
    g_ambiguous = 0u;
    g_image_base = image_base;
    g_image_size = image_size;

    /* play.sh passes the build stamp's sha and lift key (exact for an incremental build); else the configure-time ones. */
    const char *lift = getenv("TSFP_LIVE_CENSUS_LIFT"), *sha = getenv("TSFP_LIVE_CENSUS_BUILD");
    if (lift != NULL && lift[0] != '\0') {
        memset(g_lift_key, 0, sizeof g_lift_key);
        strncpy(g_lift_key, lift, sizeof g_lift_key - 1u);
    }
    if (sha != NULL && sha[0] != '\0') {
        memset(g_build_sha, 0, sizeof g_build_sha);
        strncpy(g_build_sha, sha, sizeof g_build_sha - 1u);
    }
    memcpy(g_path, out_path, strlen(out_path) + 1u);
    memcpy(g_temp, out_path, strlen(out_path));
    memcpy(g_temp + strlen(out_path), ".tmp", 5u);

    return true;
}

bool live_census_write(int reason)
{
    if (g_file == NULL) return false;
    uint8_t *const bits = g_file + g_bitmap_at;
    memset(bits, 0, g_file_size - g_bitmap_at);
    uint32_t hits = 0;
    for (size_t i = 0; i < g_count; i++) {
        const uint32_t bucket = g_bucket_of[i];
        if (g_live_census_hits[bucket] != 0u) {
            bits[i >> 3] |= (uint8_t)(1u << (i & 7u));
            hits++;
        }
    }
    /* Rust roots: the dispatch counts every call, Rust or lifted fallback. FNV-1a 64 over the enabled roots. */
    uint32_t (*root)(size_t) = s_root != NULL ? s_root : tsfp_rust_root;
    uint64_t (*count)(uint32_t, bool) = s_count != NULL ? s_count : tsfp_rust_count;
    uint64_t digest = 0xcbf29ce484222325ull;
    uint32_t roots = 0, root_hits = 0;
    if (root != NULL) {
        for (size_t r = 0; r < 65536u; r++) {
            const uint32_t va = root(r);
            if (va == 0u) break;
            roots++;
            for (unsigned b = 0; b < 4u; b++) {
                digest ^= (uint8_t)(va >> (8u * b));
                digest *= 0x100000001b3ull;
            }
            if (count == NULL || count(va, true) + count(va, false) == 0u) continue;
            root_hits++;
            const int i = index_of(va);
            if (i >= 0 && (bits[i >> 3] & (1u << (i & 7))) == 0u) {
                bits[i >> 3] |= (uint8_t)(1u << (i & 7));
                hits++;
            }
        }
    }
    struct timespec now;
    (void)clock_gettime(CLOCK_REALTIME, &now);
    uint8_t *const h = g_file;
    memcpy(h, LIVE_CENSUS_MAGIC, 8u);
    put32(h + 8, LIVE_CENSUS_VERSION);
    put32(h + 12, LIVE_CENSUS_HEADER_SIZE);
    put32(h + 16, (uint32_t)g_count);
    put32(h + 20, hits);
    put32(h + 24, g_ambiguous);
    put32(h + 28, (uint32_t)reason);
    put64(h + 32, (uint64_t)now.tv_sec);
    put_text(h + 40, 32u, g_lift_key);
    put_text(h + 72, 40u, g_build_sha);
    put32(h + 112, g_image_base);
    put32(h + 116, g_image_size);
    put32(h + 120, 0u); /* bucket shift: the map is indexed by VA */
    put32(h + 124, roots);
    put64(h + 128, digest);
    put_text(h + 136, 48u, g_port_spec);
    put32(h + 184, root_hits);
    put32(h + 188, 0u);

    const int fd = open(g_temp, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0644);
    if (fd < 0) return false;
    size_t done = 0;
    while (done < g_file_size) {
        const ssize_t n = write(fd, g_file + done, g_file_size - done);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0) break;
        done += (size_t)n;
    }
    const bool closed = close(fd) == 0;
    const bool ok = done == g_file_size && closed;
    if (!ok) {
        (void)unlink(g_temp);
        return false;
    }
    if (rename(g_temp, g_path) != 0) return false;
    g_last_hits = hits;
    g_last_functions = (uint32_t)g_count;
    return true;
}

void live_census_last(uint32_t *hits, uint32_t *functions)
{
    if (hits != NULL) *hits = g_last_hits;
    if (functions != NULL) *functions = g_last_functions;
}
