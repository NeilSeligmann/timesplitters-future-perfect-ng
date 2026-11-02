/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "fileio_setup.h"
#include "disk_identity_xemu.h"
#include "kernel_file.h"
#include "kernel_thunk.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/* TEMPORARY-SHIM T3879 FIO-3: physical volumes still belong to kernel_file.c.
 * This adapter does not constitute removal of those C dependencies. */
static bool rust_setup;
static const char partition0_device[] = "\\Device\\Harddisk0\\partition0";
static const char partition0_image[] = ".tsfp-partition0.bin";
static const uint64_t partition0_capacity = 0x80000u;
#ifdef TSFP_RUST_FILEIO
extern bool tsfp_rfio_host_setup(const host_fileio_setup_config *, host_fileio_action_fn, void *);
#endif
extern bool tsfp_rfio_host_identity_prepare(const char *, void *, uint32_t, uint32_t *);
extern bool tsfp_rfio_host_apply_action(const host_fileio_action *);
extern void tsfp_rfio_host_unmount_all(void);

bool host_fileio_setup_configure(bool rust_fileio)
{
    const char *requested = getenv("TSFP_RUST_FILEIO_HOST_SETUP");
    rust_setup = requested != NULL && strcmp(requested, "1") == 0;
#ifndef TSFP_RUST_FILEIO
    if (rust_setup) return false;
#endif
    if (rust_setup && !rust_fileio) return false;
    if (rust_setup) fprintf(stderr, "rust-fileio-setup: Rust orchestration selected; C volume adapter remains\n");
    return true;
}

bool host_fileio_rust_setup_enabled(void) { return rust_setup; }
void host_fileio_unmount_all(void) { tsfp_rfio_host_unmount_all(); }

/* Independent C oracle for early setup, retained until the opt-in gate passes. */
bool host_fileio_identity_legacy(const host_fileio_identity_backend *backend,
                                void *context, uint32_t *status)
{
    if (!backend || !status || !backend->mount || !backend->acquire ||
        !backend->probe || !backend->release) return false;
    if (!backend->mount(context)) return false;
    uint32_t value = backend->no_backing;
    if (backend->acquire(context)) {
        value = backend->probe(context);
        backend->release(context);
    }
    *status = value;
    return true;
}

bool host_fileio_prepare_identity(const char *hdd_path,
                                 kernel_disk_identity_snapshot *snapshot,
                                 unsigned *status)
{
    if (!snapshot || !status) return false;
    uint32_t value;
    if (!tsfp_rfio_host_identity_prepare(hdd_path, snapshot,
                                         DISK_XEMU_IDENTITY_NO_BACKING, &value)) return false;
    *status = (unsigned)value;
    return true;
}

/* Volume mutations run in the Rust setup bridge. This callback only preserves
 * host diagnostics and the guest cache-count thunk side effect. */
static bool report_action(void *context, const host_fileio_action *a)
{
    const bool actions_in_rust = context != NULL && *(const bool *)context;
    if (!actions_in_rust && !tsfp_rfio_host_apply_action(a)) {
        fprintf(stderr, "could not apply host file-I/O setup action %u\n", a->kind);
        return false;
    }
    switch (a->kind) {
    case HOST_FILEIO_EMPTY:
        printf("mounted        \"%s\" as an EMPTY stand-in (no content behind it)\n", a->path);
        break;
    case HOST_FILEIO_DISC:
        printf("disc           \"%s\" mounted on \"%s\" -- REAL bytes from real sectors\n", a->path, a->device);
        break;
    case HOST_FILEIO_DIRECTORY:
        printf("hdd            \"%s\" backs \"%s\" -- WRITABLE: NtCreateFile really creates here,\n"
               "               and what it creates is still there next run\n", a->path, a->device);
        break;
    case HOST_FILEIO_PARTITION0:
        printf("partition0     \"%s\" is a VIRTUAL device backed by %s/%s (0x%llx bytes)\n",
               a->device, a->path, a->image, (unsigned long long)a->capacity);
        break;
    case HOST_FILEIO_CACHE_COUNT:
        *(volatile uint32_t *)(uintptr_t)KERNEL_THUNK_VA(40u) = a->number;
        printf("cache parts    HalDiskCachePartitionCount reads %u -- FABRICATED (no HDD geometry exists behind this host)\n",
               a->number);
        break;
    case HOST_FILEIO_CACHE:
        printf("cache part %u   \"%s\" is a VIRTUAL cache partition: raw image %s/%s, directory view %s/%s\n",
               a->number, a->device, a->path, a->image, a->path, a->directory);
        break;
    case HOST_FILEIO_MISSING_EMPTY:
        printf("open policy    a name with no volume behind it opens as an EMPTY, FABRICATED file\n");
        break;
    default: return false;
    }
    return true;
}

/* Independent legacy ordering is retained for the A/B gate and non-Rust builds. */
bool host_fileio_setup_legacy(const host_fileio_setup_config *c,
                             host_fileio_action_fn execute, void *context)
{
    for (uint32_t i = 0; i < c->mount_count; i++) {
        const host_fileio_action a = {.kind = HOST_FILEIO_EMPTY, .path = c->mounts[i]};
        if (!execute(context, &a)) return false;
    }
    if (c->disc_path) {
        const host_fileio_action a = {.kind = HOST_FILEIO_DISC, .device = c->disc_device, .path = c->disc_path};
        if (!execute(context, &a)) return false;
    }
    if (c->hdd_path) {
        const host_fileio_action dir = {.kind = HOST_FILEIO_DIRECTORY, .device = c->hdd_device, .path = c->hdd_path};
        const host_fileio_action part = {.kind = HOST_FILEIO_PARTITION0, .device = c->partition0_device,
            .path = c->hdd_path, .image = c->partition0_image, .capacity = c->partition0_capacity,
            .skip_mount = c->pre_mounted};
        if (!execute(context, &dir) || !execute(context, &part)) return false;
        if (c->cache_count != 0 && c->window_mapped) {
            const host_fileio_action a = {.kind = HOST_FILEIO_CACHE_COUNT, .number = c->cache_count};
            if (!execute(context, &a)) return false;
        }
        for (uint32_t slot = 0; slot < c->cache_count; slot++) {
            const uint32_t number = c->cache_first + slot;
            char device[64], image[64], directory[64];
            (void)snprintf(device, sizeof(device), "\\Device\\Harddisk0\\Partition%u", number);
            (void)snprintf(image, sizeof(image), ".tsfp-cache%u.bin", number);
            (void)snprintf(directory, sizeof(directory), ".tsfp-cache%u", number);
            const host_fileio_action a = {.kind = HOST_FILEIO_CACHE, .device = device, .path = c->hdd_path,
                .image = image, .directory = directory, .number = number, .capacity = c->cache_capacity};
            if (!execute(context, &a)) return false;
        }
    }
    if (c->missing_empty) {
        const host_fileio_action a = {.kind = HOST_FILEIO_MISSING_EMPTY};
        if (!execute(context, &a)) return false;
    }
    return true;
}

bool host_fileio_setup_mounts(const options *opts, bool pre_mounted, bool window_mapped)
{
    const bool actions_in_rust = rust_setup;
    const host_fileio_setup_config config = {
        .mounts = opts->mounts, .mount_count = opts->mount_count,
        .disc_device = opts->disc_device, .disc_path = opts->disc_path,
        .hdd_device = opts->hdd_device, .hdd_path = opts->hdd_path,
        .partition0_device = partition0_device, .partition0_image = partition0_image,
        .partition0_capacity = partition0_capacity, .cache_count = opts->cache_partitions,
        .cache_first = KERNEL_FILE_CACHE_PARTITION_FIRST, .cache_capacity = KERNEL_FILE_DEVICE_CAPACITY_MAX,
        .pre_mounted = pre_mounted, .window_mapped = window_mapped, .missing_empty = opts->open_missing_as_empty,
    };
#ifdef TSFP_RUST_FILEIO
    if (rust_setup) return tsfp_rfio_host_setup(&config, report_action, (void *)&actions_in_rust);
#endif
    return host_fileio_setup_legacy(&config, report_action, (void *)&actions_in_rust);
}
