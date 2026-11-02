/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_HOST_FILEIO_SETUP_H
#define TSFP_HOST_FILEIO_SETUP_H
#include <stdbool.h>
#include <stdint.h>
#include "host_options.h"
#include "kernel_disk_identity.h"

/* T3879 FIO-1. Rust owns setup ordering; the callback is a TEMPORARY-SHIM to
 * C-owned physical volumes until FIO-3 retires that state. Not runtime retirement. */
enum host_fileio_action_kind {
    HOST_FILEIO_EMPTY, HOST_FILEIO_DISC, HOST_FILEIO_DIRECTORY,
    HOST_FILEIO_PARTITION0, HOST_FILEIO_CACHE_COUNT, HOST_FILEIO_CACHE,
    HOST_FILEIO_MISSING_EMPTY
};
typedef struct {
    uint32_t kind;
    const char *device, *path, *image, *directory;
    uint32_t number;
    uint64_t capacity;
    bool skip_mount;
} host_fileio_action;
typedef struct {
    const char *const *mounts;
    uint32_t mount_count;
    const char *disc_device, *disc_path, *hdd_device, *hdd_path;
    const char *partition0_device, *partition0_image;
    uint64_t partition0_capacity;
    uint32_t cache_count, cache_first;
    uint64_t cache_capacity;
    bool pre_mounted, window_mapped, missing_empty;
} host_fileio_setup_config;
typedef bool (*host_fileio_action_fn)(void *, const host_fileio_action *);
/* Independent oracle driver, also used by the remote transcript comparison. */
bool host_fileio_setup_legacy(const host_fileio_setup_config *, host_fileio_action_fn, void *);

/* Early identity callbacks retain one backing between successful acquire and
 * release. Probe borrows that backing even when it returns a refusal status. */
typedef struct {
    bool (*mount)(void *);
    bool (*acquire)(void *);
    uint32_t (*probe)(void *);
    void (*release)(void *);
    uint32_t no_backing;
} host_fileio_identity_backend;
bool host_fileio_identity_legacy(const host_fileio_identity_backend *, void *, uint32_t *status);

/* Build-enabled hosts use the native Rust implementation. */
bool host_fileio_setup_configure(bool rust_fileio);
bool host_fileio_rust_setup_enabled(void);
bool host_fileio_setup_mounts(const options *opts, bool pre_mounted, bool window_mapped);
/* Early xemu phase remains before identity publication/import patching. */
bool host_fileio_prepare_identity(const char *hdd_path,
                                 kernel_disk_identity_snapshot *snapshot,
                                 unsigned *status);
void host_fileio_unmount_all(void);
#endif
