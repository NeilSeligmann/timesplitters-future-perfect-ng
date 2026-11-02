/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * See kernel_hle.h for why the diagnostics matter more than the dispatch.
 */

#include "kernel_hle.h"
#include "kernel_services_rust.h"
#include "kernel_call.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

static kernel_entry entries[XBOX_KERNEL_ORDINAL_MAX + 1];
static bool initialised;

static int default_printer(const char *format, ...)
{
    va_list args;
    va_start(args, format);
    int written = vfprintf(stderr, format, args);
    va_end(args);
    return written;
}

static kernel_log_fn log_printer = default_printer;

void kernel_hle_set_log(kernel_log_fn printer)
{
    log_printer = printer ? printer : default_printer;
}

kernel_log_fn kernel_hle_log(void)
{
    return log_printer;
}

void kernel_hle_log_message(const char *message)
{
    if (message != NULL) {
        kernel_hle_log()("%s", message);
    }
}

#include <stdlib.h>

#if defined(__GNUC__) || defined(__clang__)
#define KERNEL_HLE_TLS __thread
#else
#define KERNEL_HLE_TLS
#endif

/* Thread-local because the pointer is BORROWED by the hook (the host keeps it in
 * its stop record), and two guest threads reaching fatal calls at once must not
 * race over one buffer. Same reasoning as the host's firmware_detail. */
/* 224 holds the widest composed detail (a raise with flags, four Info dwords and
 * the elision marker runs to about 170 characters) without truncation. */
static KERNEL_HLE_TLS char fatal_detail[224];

static void default_fatal(unsigned ordinal, const char *detail)
{
    (void)ordinal;
    (void)detail;
    /* No host attached: there is no stop machinery to unwind into, and returning
     * would continue a guest the real console has already halted. */
    abort();
}

static kernel_fatal_fn fatal_hook = default_fatal;

void kernel_hle_set_fatal(kernel_fatal_fn handler)
{
    fatal_hook = handler ? handler : default_fatal;
}

void kernel_hle_fatal(unsigned ordinal, const char *format, ...)
{
    va_list args;
    va_start(args, format);
    (void)vsnprintf(fatal_detail, sizeof(fatal_detail), format, args);
    va_end(args);
    log_printer("kernel: FATAL ordinal %u (%s): %s\n", ordinal,
                xbox_kernel_ordinal_name(ordinal) ? xbox_kernel_ordinal_name(ordinal)
                                                  : "<unknown ordinal>",
                fatal_detail);
    fatal_hook(ordinal, fatal_detail);
}

void kernel_hle_init(void)
{
    memset(entries, 0, sizeof(entries));
    for (unsigned ordinal = 0; ordinal <= XBOX_KERNEL_ORDINAL_MAX; ordinal++) {
        const char *name = xbox_kernel_ordinal_name(ordinal);
        if (!name) {
            continue;
        }
        entries[ordinal].name = name;
        entries[ordinal].state = KERNEL_ENTRY_STUB;
    }
    initialised = true;
}

static kernel_entry *mutable_entry(unsigned ordinal)
{
    if (!initialised) {
        kernel_hle_init();
    }
    if (ordinal > XBOX_KERNEL_ORDINAL_MAX) {
        return NULL;
    }
    if (entries[ordinal].state == KERNEL_ENTRY_ABSENT && !entries[ordinal].name) {
        return NULL;
    }
    return &entries[ordinal];
}

bool kernel_hle_register(unsigned ordinal, kernel_fn handler)
{
    kernel_entry *entry = mutable_entry(ordinal);
    if (!entry || !handler) {
        return false;
    }
    entry->handler = handler;
    entry->state = KERNEL_ENTRY_IMPLEMENTED;
    return true;
}

bool kernel_hle_set_default_return(unsigned ordinal, uint32_t value)
{
    kernel_entry *entry = mutable_entry(ordinal);
    if (!entry) {
        return false;
    }
    entry->default_return = value;
    return true;
}

const kernel_entry *kernel_hle_entry(unsigned ordinal)
{
    return mutable_entry(ordinal);
}

/* T2074: guest file-API call log, the oracle for the guest handle layer (one line per file-related kernel call:
 * ordinal, name, return status; no pointers, no file contents). Identical for the C and the Rust file layer. */
static FILE *file_call_log;

bool kernel_hle_file_log_open(const char *path)
{
    if (file_call_log != NULL) {
        (void)fclose(file_call_log);
        file_call_log = NULL;
    }
    if (path == NULL) {
        return true;
    }
    file_call_log = fopen(path, "w");
    if (file_call_log == NULL) {
        return false;
    }
    /* Replays may stop the host before stdio's block buffer is closed. Preserve
     * each complete guest API record for the per-call C/Rust differential. */
    if (setvbuf(file_call_log, NULL, _IOLBF, 0) != 0) {
        (void)fclose(file_call_log);
        file_call_log = NULL;
        return false;
    }
    return true;
}

static bool is_file_api(const char *name)
{
    static const char *const prefixes[] = {"NtClose", "NtCreateFile", "NtOpenFile", "NtDeleteFile", "NtQuery", "NtRead",
        "NtSet", "NtWrite", "NtFlush", "NtDeviceIo", "NtFsControl", "IoCreateSym", "IoDeleteSym", "IoDismount", "IoMount",
        "NtOpenSymbolic", "NtCreateDirectory"};
    for (size_t i = 0; i < sizeof(prefixes) / sizeof(prefixes[0]); i++) {
        if (strncmp(name, prefixes[i], strlen(prefixes[i])) == 0 && strstr(name, "Thread") == NULL &&
            strstr(name, "Process") == NULL && strstr(name, "Event") == NULL && strstr(name, "Timer") == NULL &&
            strstr(name, "Semaphore") == NULL && strstr(name, "Mutant") == NULL && strstr(name, "System") == NULL &&
            strstr(name, "Section") == NULL && strstr(name, "Virtual") == NULL && strstr(name, "Object") == NULL) {
            return true;
        }
    }
    return false;
}

/* T2074 slice 1: per-API argument positions of the IO_STATUS_BLOCK, the data buffer and its length, and whether the
 * buffer is an input (hashed before the call: write, set) or an output (hashed after: read, query). Payload hashes are
 * FNV-1a64 over min(IoStatus.Information, Length) bytes, never contents. */
typedef struct {
    const char *name;
    int iosb;
    int buffer;
    int length;
    bool input;
} file_log_shape;

static const file_log_shape *file_log_shape_for(const char *name)
{
    static const file_log_shape shapes[] = {
        {"NtCreateFile", 3, -1, -1, false}, {"NtOpenFile", 3, -1, -1, false},
        {"NtReadFile", 4, 5, 6, false}, {"NtWriteFile", 4, 5, 6, true},
        {"NtQueryDirectoryFile", 4, 5, 6, false}, {"NtQueryInformationFile", 1, 2, 3, false},
        {"NtQueryVolumeInformationFile", 1, 2, 3, false}, {"NtSetInformationFile", 1, 2, 3, true},
        {"NtFlushBuffersFile", 1, -1, -1, false}, {"NtDeviceIoControlFile", 4, -1, -1, false},
        {"NtFsControlFile", 4, -1, -1, false},
    };
    for (size_t i = 0; i < sizeof(shapes) / sizeof(shapes[0]); i++) {
        if (strcmp(name, shapes[i].name) == 0) {
            return &shapes[i];
        }
    }
    return NULL;
}

static uint64_t file_log_hash(uint32_t address, uint32_t length, bool *ok)
{
    uint64_t hash = 0xcbf29ce484222325ull;
    uint8_t chunk[256];
    *ok = true;
    while (length > 0u) {
        const uint32_t take = length < sizeof(chunk) ? length : (uint32_t)sizeof(chunk);
        if (!kernel_guest_read_bytes(address, chunk, take)) {
            *ok = false;
            return 0u;
        }
        for (uint32_t i = 0; i < take; i++) {
            hash = (hash ^ chunk[i]) * 0x100000001b3ull;
        }
        address += take;
        length -= take;
    }
    return hash;
}

static uint32_t logged_file_call(unsigned ordinal, const char *name, void *context)
{
    const file_log_shape *shape = file_log_shape_for(name);
    uint32_t args[8] = {0};
    bool have_args = shape != NULL && context != NULL;
    for (unsigned i = 0; have_args && i < 8u; i++) {
        have_args = kernel_frame_arg((const kernel_call_frame *)context, i, &args[i]);
    }
    bool have_hash = false;
    uint64_t hash = 0u;
    if (have_args && shape->buffer >= 0 && shape->input && args[shape->buffer] != 0u && args[shape->length] <= 0x100000u) {
        hash = file_log_hash(args[shape->buffer], args[shape->length], &have_hash);
    }
    const uint32_t rc = kernel_hle_call_unlogged(ordinal, context);
    fprintf(file_call_log, "%u %s rc=0x%08x", ordinal, name, rc);
    if (have_args && args[shape->iosb] != 0u) {
        uint32_t information = 0u;
        /* PENDING: the async completion may already have run, the field is a race. */
        if (rc != 0x103u && kernel_guest_read_u32(args[shape->iosb] + 4u, &information)) {
            fprintf(file_call_log, " info=0x%x", information);
            if (shape->buffer >= 0 && !shape->input && args[shape->buffer] != 0u && information != 0u &&
                information <= args[shape->length] && information <= 0x100000u && rc != 0x103u) {
                hash = file_log_hash(args[shape->buffer], information, &have_hash);
            }
        }
    }
    if (have_hash) {
        fprintf(file_call_log, " h=%016llx", (unsigned long long)hash);
    }
    fputc('\n', file_call_log);
    return rc;
}

uint32_t kernel_hle_call(unsigned ordinal, void *context)
{
    if (file_call_log != NULL) {
        const kernel_entry *named = kernel_hle_entry(ordinal);
        if (named != NULL && named->name != NULL && is_file_api(named->name)) {
            return logged_file_call(ordinal, named->name, context);
        }
    }
    return kernel_hle_call_unlogged(ordinal, context);
}

uint32_t kernel_hle_call_unlogged(unsigned ordinal, void *context)
{
    kernel_entry *entry = mutable_entry(ordinal);
    if (!entry) {
        /* Not a known ordinal at all. This means the guest's import table and our
         * table disagree, which is a different and worse problem than a missing
         * implementation, so it reports every single time. */
        log_printer("kernel: call to UNKNOWN ordinal %u -- import table mismatch\n",
                    ordinal);
        return 0;
    }

    entry->call_count++;

    if (entry->state == KERNEL_ENTRY_IMPLEMENTED && entry->handler) {
#ifdef TSFP_RUST_KSERVICES
        return kernel_services_dispatch(ordinal, entry->handler, context);
#else
        return entry->handler(context);
#endif
    }

    if (!entry->reported) {
        entry->reported = true;
        log_printer("kernel: ordinal %u (%s) is not implemented, returning %#x\n",
                    ordinal, entry->name, entry->default_return);
    }
    return entry->default_return;
}

size_t kernel_hle_implemented_count(const unsigned *ordinals, size_t count)
{
    if (!ordinals) {
        return 0;
    }
    size_t done = 0;
    for (size_t i = 0; i < count; i++) {
        const kernel_entry *entry = kernel_hle_entry(ordinals[i]);
        if (entry && entry->state == KERNEL_ENTRY_IMPLEMENTED) {
            done++;
        }
    }
    return done;
}

void kernel_hle_report_missing(const unsigned *ordinals, size_t count)
{
    if (!ordinals) {
        return;
    }
    if (!initialised) {
        kernel_hle_init();
    }

    /* Selection sort over call_count. The list is at most a few hundred entries
     * and this runs once, so clarity beats cleverness. */
    size_t missing = 0;
    for (size_t i = 0; i < count; i++) {
        const kernel_entry *entry = kernel_hle_entry(ordinals[i]);
        if (entry && entry->state != KERNEL_ENTRY_IMPLEMENTED) {
            missing++;
        }
    }

    log_printer("kernel: %zu of %zu imported ordinals still need implementations\n",
                missing, count);

    bool emitted[XBOX_KERNEL_ORDINAL_MAX + 1] = {false};
    for (size_t printed = 0; printed < missing; printed++) {
        unsigned best = 0;
        uint64_t best_calls = 0;
        bool found = false;
        for (size_t i = 0; i < count; i++) {
            unsigned ordinal = ordinals[i];
            if (ordinal > XBOX_KERNEL_ORDINAL_MAX || emitted[ordinal]) {
                continue;
            }
            const kernel_entry *entry = kernel_hle_entry(ordinal);
            if (!entry || entry->state == KERNEL_ENTRY_IMPLEMENTED) {
                continue;
            }
            if (!found || entry->call_count > best_calls) {
                found = true;
                best = ordinal;
                best_calls = entry->call_count;
            }
        }
        if (!found) {
            break;
        }
        emitted[best] = true;
        const kernel_entry *entry = kernel_hle_entry(best);
        log_printer("  ordinal %3u  %-40s calls %llu\n", best, entry->name,
                    (unsigned long long)entry->call_count);
    }
}
