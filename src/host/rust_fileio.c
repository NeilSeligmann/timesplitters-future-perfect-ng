/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "rust_fileio.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../xbox/kernel_hle.h"
#include "../xbox/xdvdfs.h"
#include "../xbox/kernel_io.h"
#include "../xbox/kernel_async_io.h"
#include "../xbox/kernel_file_object.h"
#include "../input/mu_fatx.h"

#ifdef TSFP_RUST_FILEIO
/* The host-link target does not receive the XDVDFS alias macros or native flag. */
extern void tsfp_native_xdvdfs_set_backend(const xdvdfs_backend *backend);
#endif

#ifdef TSFP_RUST_FILEIO
extern void tsfp_rfio_set_share_enforcement(bool enabled);
extern bool tsfp_native_xdvdfs_log_open(const char *path);
extern const xdvdfs_backend *tsfp_rfio_backend(void);
extern uint64_t tsfp_rfio_kio_calls(uint32_t ordinal);
extern const kernel_io_transfer_backend *tsfp_rfio_io_backend(void);
extern const kernel_async_io_backend *tsfp_rfio_async_backend(void);
extern const kernel_file_object_backend *tsfp_rfio_file_object_backend(void);
extern const fatx_backend *tsfp_rfio_fatx_backend(void);
extern const kernel_file_hostio_backend *tsfp_rfio_hostio_backend(void);
extern const kernel_file_hostdir_backend *tsfp_rfio_hostdir_backend(void);
extern const kernel_file_namespace_backend *tsfp_rfio_namespace_backend(void);
extern const kernel_file_view_backend *tsfp_rfio_view_backend(void);
extern const kernel_file_glue_backend *tsfp_rfio_glue_backend(void);
extern const kernel_file_handle_backend *tsfp_rfio_handle_backend(void);
extern const kernel_file_dir_backend *tsfp_rfio_dir_backend(void);
extern const kernel_io_info_backend *tsfp_rfio_info_backend(void);
extern const kernel_file_disposition_backend *tsfp_rfio_disposition_backend(void);
extern const kernel_file_commit_backend *tsfp_rfio_commit_backend(void);
extern const kernel_file_payload_backend *tsfp_rfio_payload_backend(void);
extern const kernel_file_desc_backend *tsfp_rfio_desc_backend(void);
extern uint64_t tsfp_rfio_counter(uint32_t index);
extern const char *tsfp_rfio_counter_name(uint32_t index);
extern void tsfp_rfio_install_backends(void);
extern uint32_t tsfp_rfio_fileio_report_counter(uint32_t index);
#endif

#ifdef TSFP_RUST_FILEIO
/* T2210: backend liveness for future Tier 2 comparisons. */
static void report_kernel_io(void)
{
    const kernel_async_io_stats stats = kernel_async_io_get_stats();
    fprintf(stderr, "rust-kio: liveness");
    static const uint32_t ordinals[] = {219u, 236u, 218u, 196u, 200u, 198u, 211u, 226u, 210u, 207u};
    for (unsigned i = 0u; i < sizeof(ordinals) / sizeof(ordinals[0]); i++)
        fprintf(stderr, " ordinal%u=%llu", ordinals[i], (unsigned long long)tsfp_rfio_kio_calls(ordinals[i]));
    fprintf(stderr, " async_submitted=%u async_completed=%u body_fabricated=%u body_queries=%u\n", stats.submitted, stats.completed,
            kernel_file_object_fabricated_count(), kernel_file_object_query_answered_count());
}
/* T2074 slice 3: evidence line for the Tier 2 runs, printed when the host exits. */
static void report_handle_index(void)
{
#ifdef TSFP_NATIVE_FILEIO
    fprintf(stderr, "rust-fileio: native volumes=%u, open slots=%u\n",
            tsfp_rfio_fileio_report_counter(0), tsfp_rfio_fileio_report_counter(1));
#else
    fprintf(stderr, "rust-fileio: open-slot index shadow mismatches=%u, would-be share conflicts=%u, share refusals=%u, open slots=%u\n",
            tsfp_rfio_fileio_report_counter(2), tsfp_rfio_fileio_report_counter(3), tsfp_rfio_fileio_report_counter(4),
            tsfp_rfio_fileio_report_counter(1));
    /* T2092: payload (T2089) and disposition (T2074 slice 7) cross-checks for the Tier 2 route */
    fprintf(stderr, "rust-fileio: payload shadow mismatches=%u, disposition shadow mismatches=%u, commit shadow mismatches=%u, namespace shadow mismatches=%u\n",
            tsfp_rfio_fileio_report_counter(5), tsfp_rfio_fileio_report_counter(6),
            tsfp_rfio_fileio_report_counter(7), tsfp_rfio_fileio_report_counter(8));
    fprintf(stderr, "rust-fileio: view shadow mismatches=%u, glue shadow mismatches=%u\n",
            tsfp_rfio_fileio_report_counter(9), tsfp_rfio_fileio_report_counter(10));
#endif
    /* T2074: liveness of each Rust backend, zero would mean the C path served the route */
    fprintf(stderr, "rust-fileio: liveness");
    for (uint32_t index = 0u; tsfp_rfio_counter_name(index) != NULL; index++) {
        fprintf(stderr, " %s=%llu", tsfp_rfio_counter_name(index), (unsigned long long)tsfp_rfio_counter(index));
    }
    fprintf(stderr, "\n");
}
#endif

bool host_rust_fileio_configure(bool enable, const char *log_path)
{
    /* T2210 is independent of --rust-fileio and remains default off. */
    const char *kio_env = getenv("TSFP_RUST_KIO");
    if (kio_env != NULL && strcmp(kio_env, "1") == 0) {
#ifdef TSFP_RUST_FILEIO
        kernel_io_set_transfer_backend(tsfp_rfio_io_backend());
        kernel_async_io_set_backend(tsfp_rfio_async_backend());
        kernel_file_object_set_backend(tsfp_rfio_file_object_backend());
        fprintf(stderr, "rust-kio: selected Rust NT I/O and completion, experimental Tier 1 synthetic; Tier 2 pending\n");
        (void)atexit(report_kernel_io);
#else
        return false;
#endif
    }
    /* T2074 slice 10: opt-in NT share-mode enforcement (INFERRED, C and Rust paths alike) */
    const char *share_env = getenv("TSFP_ENFORCE_SHARE");
    tsfp_rfio_set_share_enforcement(share_env != NULL && strcmp(share_env, "1") == 0);
    if (log_path == NULL) {
        log_path = getenv("TSFP_FILEIO_LOG");
    }
    if (log_path != NULL && log_path[0] != '\0' && !tsfp_native_xdvdfs_log_open(log_path)) {
        return false;
    }
    if (log_path != NULL && log_path[0] != '\0') {
        /* T2074: guest file-API call log next to the disc log, "<path>.nt" */
        char nt_path[1024];
        if (snprintf(nt_path, sizeof(nt_path), "%s.nt", log_path) >= (int)sizeof(nt_path) ||
            !kernel_hle_file_log_open(nt_path)) {
            return false;
        }
    }
    if (!enable) {
#ifdef TSFP_NATIVE_FILEIO
        fprintf(stderr, "rust-fileio: --legacy-fileio unavailable in the native file-I/O build\n");
        return false;
#else
        return true;
#endif
    }
#ifdef TSFP_RUST_FILEIO
    tsfp_native_xdvdfs_set_backend(tsfp_rfio_backend());
    fatx_set_backend(tsfp_rfio_fatx_backend());
    kernel_io_set_info_backend(tsfp_rfio_info_backend());
#ifndef TSFP_NATIVE_FILEIO
    tsfp_rfio_install_backends();
#endif
    (void)atexit(report_handle_index);
    return true;
#else
    return false;
#endif
}
