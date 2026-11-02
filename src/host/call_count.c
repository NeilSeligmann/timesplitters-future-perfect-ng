/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T1888: opt-in per-function ENTRY counter for the lifted code. Compiled in only with
 * `-DTSFP_CALL_COUNT=ON`, which also builds the lifted chunks with `-finstrument-functions`
 * (gcc then calls __cyg_profile_func_enter on every entry, direct or indirect, no lifter change) and
 * links tsfp_host with --export-dynamic so dladdr can name `sub_XXXXXXXX`. The default build contains
 * none of this. OBSERVATION ONLY, NOT A MODEL: it changes no guest state.
 *
 * Table: open addressing, lock free (CAS on the key, atomic add on the count), safe for guest threads.
 * Dump (function_census phase dump calls it through a weak symbol, and at exit when
 * TSFP_CALL_COUNT_OUT names a file): lines `function 0xVA calls N`, counts reset after each phase dump.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <execinfo.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <unistd.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define NOINSTR __attribute__((no_instrument_function))
#define SLOTS (1u << 18)

static struct {
    _Atomic uintptr_t key;
    _Atomic uint64_t count;
} slots[SLOTS];
static _Atomic uint64_t dropped;

/* T2002 frontier mode (opt-in by env, off by default, observation only until a panic):
 *   TSFP_FRONTIER_LOG=FILE    one line per function the first time it is entered, in first-reach order:
 *                             `first seq N va 0xVA caller 0xCA frame F`
 *   TSFP_FRONTIER_OWNED=FILE  hex VAs (one per line, `#` comments) that a Rust port owns; with this set the host
 *                             PANICS on the first entry of any other guest function: a report with the unported
 *                             VA, its call count (1, it is the first entry), the present index and the C caller
 *                             chain on stderr and TSFP_FRONTIER_PANIC=FILE, then _exit(3).
 * Both are decided at the first entry of a function, so the steady state cost is the existing counter alone. */
extern uint64_t d3d8_frame_queue_total(void) __attribute__((weak));
static _Atomic uint64_t first_seq;
static FILE *frontier_log;
static uint32_t *owned;
static size_t owned_count;
static bool frontier_panic;
static pthread_mutex_t frontier_lock = PTHREAD_MUTEX_INITIALIZER;

NOINSTR static void frontier_first(uintptr_t key, void *caller);

NOINSTR void __cyg_profile_func_enter(void *function, void *caller)
{
    const uintptr_t key = (uintptr_t)function;
    uint32_t index = (uint32_t)((key >> 4) * 2654435761u) & (SLOTS - 1u);
    for (uint32_t probe = 0u; probe < 64u; probe++, index = (index + 1u) & (SLOTS - 1u)) {
        uintptr_t seen = atomic_load_explicit(&slots[index].key, memory_order_relaxed);
        if (seen == 0u) {
            uintptr_t empty = 0u;
            if (!atomic_compare_exchange_strong(&slots[index].key, &empty, key)) {
                seen = empty;
            } else {
                seen = key;
                frontier_first(key, caller);
            }
        }
        if (seen == key) {
            atomic_fetch_add_explicit(&slots[index].count, 1u, memory_order_relaxed);
            return;
        }
    }
    atomic_fetch_add_explicit(&dropped, 1u, memory_order_relaxed);
}

NOINSTR void __cyg_profile_func_exit(void *function, void *caller)
{
    (void)function;
    (void)caller;
}

/* sub_XXXXXXXX -> VA, 0 when the symbol is not a lifted function name. */
NOINSTR static uint32_t va_of(uintptr_t key)
{
    Dl_info info;
    if (dladdr((void *)key, &info) == 0 || info.dli_sname == NULL || strncmp(info.dli_sname, "sub_", 4) != 0) {
        return 0u;
    }
    char *end = NULL;
    const unsigned long value = strtoul(info.dli_sname + 4, &end, 16);
    return (end != NULL && *end == '\0') ? (uint32_t)value : 0u;
}

NOINSTR static bool is_owned(uint32_t va)
{
    for (size_t index = 0u; index < owned_count; index++) {
        if (owned[index] == va) {
            return true;
        }
    }
    return false;
}

NOINSTR static void frontier_first(uintptr_t key, void *caller)
{
    if (frontier_log == NULL && !frontier_panic) {
        return;
    }
    const uint32_t va = va_of(key);
    if (va == 0u) {
        return;
    }
    const uint64_t frame = d3d8_frame_queue_total != NULL ? d3d8_frame_queue_total() : 0u;
    pthread_mutex_lock(&frontier_lock);
    const uint32_t from = va_of((uintptr_t)caller);
    if (frontier_log != NULL) {
        fprintf(frontier_log, "first seq %llu va 0x%08X caller 0x%08X frame %llu\n",
                (unsigned long long)atomic_fetch_add(&first_seq, 1u), va, from, (unsigned long long)frame);
        fflush(frontier_log);
    }
    if (frontier_panic && !is_owned(va)) {
        void *frames[64];
        const int depth = backtrace(frames, 64);
        const char *panic_path = getenv("TSFP_FRONTIER_PANIC");
        FILE *outputs[2] = {stderr, panic_path != NULL && panic_path[0] != '\0' ? fopen(panic_path, "w") : NULL};
        for (int index = 0; index < 2; index++) {
            if (outputs[index] == NULL) {
                continue;
            }
            fprintf(outputs[index], "FRONTIER PANIC unported 0x%08X calls 1 frame %llu caller 0x%08X\n", va,
                    (unsigned long long)frame, from);
            for (int level = 0; level < depth; level++) {
                const uint32_t chain = va_of((uintptr_t)frames[level]);
                if (chain != 0u) {
                    fprintf(outputs[index], "FRONTIER chain %d 0x%08X\n", level, chain);
                }
            }
            fflush(outputs[index]);
        }
        _exit(3);
    }
    pthread_mutex_unlock(&frontier_lock);
}

/* Write and reset. Returns false when the file cannot be opened. */
NOINSTR bool tsfp_call_count_dump(const char *path)
{
    FILE *file = fopen(path, "w");
    if (file == NULL) {
        return false;
    }
    uint64_t total = 0u, unnamed = 0u;
    for (uint32_t index = 0u; index < SLOTS; index++) {
        const uintptr_t key = atomic_load(&slots[index].key);
        if (key == 0u) {
            continue;
        }
        const uint64_t count = atomic_exchange(&slots[index].count, 0u);
        if (count == 0u) {
            continue;
        }
        const uint32_t va = va_of(key);
        if (va == 0u) {
            unnamed += count;
            continue;
        }
        total += count;
        fprintf(file, "function 0x%08X calls %llu\n", va, (unsigned long long)count);
    }
    fprintf(file, "# call-count 1 total %llu unnamed %llu dropped %llu\n", (unsigned long long)total,
            (unsigned long long)unnamed, (unsigned long long)atomic_exchange(&dropped, 0u));
    fclose(file);
    return true;
}

NOINSTR static void dump_at_exit(void)
{
    const char *path = getenv("TSFP_CALL_COUNT_OUT");
    if (path != NULL && path[0] != '\0') {
        (void)tsfp_call_count_dump(path);
    }
}

__attribute__((constructor, no_instrument_function)) static void call_count_init(void)
{
    atexit(dump_at_exit);
    const char *log_path = getenv("TSFP_FRONTIER_LOG");
    if (log_path != NULL && log_path[0] != '\0') {
        frontier_log = fopen(log_path, "w");
    }
    const char *owned_path = getenv("TSFP_FRONTIER_OWNED");
    FILE *list = owned_path != NULL && owned_path[0] != '\0' ? fopen(owned_path, "r") : NULL;
    if (list != NULL) {
        char line[128];
        size_t capacity = 0u;
        while (fgets(line, sizeof line, list) != NULL) {
            char *end = NULL;
            const unsigned long value = strtoul(line, &end, 16);
            if (end == line || line[0] == '#') {
                continue;
            }
            if (owned_count == capacity) {
                capacity = capacity != 0u ? capacity * 2u : 4096u;
                owned = realloc(owned, capacity * sizeof *owned);
            }
            owned[owned_count++] = (uint32_t)value;
        }
        fclose(list);
        frontier_panic = true;
    }
}
