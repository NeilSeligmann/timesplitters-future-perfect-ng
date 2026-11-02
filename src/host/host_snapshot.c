/* SPDX-License-Identifier: GPL-3.0-or-later */
#define _GNU_SOURCE
#include "host_snapshot.h"
#include "tsfp_input_host_wiring.h"
#include <errno.h>
#include <dirent.h>
#include <fcntl.h>
#include <limits.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

/* The libdmtcp.so symbol, weak so a host started without dmtcp_launch still links and runs. */
extern int dmtcp_checkpoint(void) __attribute__((weak));

static void dump_snapshot_descriptors(const char *phase)
{
    if (getenv("TSFP_SNAPSHOT_DEBUG_FDS") == NULL) return;
    DIR *directory = opendir("/proc/self/fd");
    if (directory == NULL) return;
    const int directory_fd = dirfd(directory);
    fprintf(stderr, "snapshot: descriptor inventory %s\n", phase);
    struct dirent *entry;
    while ((entry = readdir(directory)) != NULL) {
        char *end = NULL;
        const long number = strtol(entry->d_name, &end, 10);
        if (end == entry->d_name || *end != '\0' || number < 0 || number == directory_fd) continue;
        char link[64], target[PATH_MAX];
        snprintf(link, sizeof link, "/proc/self/fd/%ld", number);
        const ssize_t length = readlink(link, target, sizeof target - 1u);
        if (length <= 0) continue;
        target[length] = '\0';
        if (strstr(target, "eventfd") == NULL && strstr(target, "epoll") == NULL &&
            strstr(target, "socket:") == NULL && strstr(target, "pipe:") == NULL) continue;
        errno = 0;
        const int flags = fcntl((int)number, F_GETFD);
        fprintf(stderr, "snapshot: fd=%ld flags=%d errno=%d target=%s\n", number, flags, errno, target);
    }
    closedir(directory);
    fflush(stderr);
}

static host_snapshot_checkpoint_fn g_checkpoint;
static _Atomic uint64_t g_target;
static uint64_t g_targets[64];
static unsigned g_target_count;
static _Atomic unsigned g_target_index;
static _Atomic uint64_t g_taken_at;
static _Atomic int g_fired;
static _Atomic uint64_t g_stop_target;
static host_snapshot_stop_fn g_stop_function;
static _Atomic int g_stop_fired;
static void fail_loudly(const char *why);

static void set_isolated_path_prefix(uint64_t polls)
{
    const char *enabled = getenv("TSFP_SNAPSHOT_ISOLATE_RUN_DIR");
    const char *directory = getenv("TSFP_SNAPSHOT_DIR");
    if (enabled == NULL || strcmp(enabled, "1") != 0 || directory == NULL || directory[0] == '\0') return;
    char isolated[PATH_MAX];
    char replacement[PATH_MAX * 4];
    int length = snprintf(isolated, sizeof isolated, "%s/poll-%llu/isolated-run-dir", directory,
                          (unsigned long long)polls);
    if (length < 0 || (size_t)length >= sizeof isolated) fail_loudly("isolated snapshot path is too long");
    length = snprintf(replacement, sizeof replacement, "%s:%s/parallel-x11:%s:%s/parallel-xdg-runtime",
                      isolated, isolated, isolated, isolated);
    if (length < 0 || (size_t)length >= sizeof replacement) fail_loudly("path virtualization prefixes are too long");
    if (setenv("DMTCP_NEW_PATH_PREFIX", replacement, 1) != 0)
        fail_loudly("could not set the per-snapshot path virtualization prefix");
}

static void snapshot_acknowledgement(uint64_t polls, int result)
{
    const char *directory = getenv("TSFP_SNAPSHOT_DIR");
    if (directory == NULL || directory[0] == '\0') return;
    char path[PATH_MAX];
    snprintf(path, sizeof path, "%s/poll-%llu.checkpointed", directory,
             (unsigned long long)polls);
    int marker = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (marker >= 0) close(marker);
    if (result != HOST_SNAPSHOT_AFTER_CHECKPOINT) return;
    snprintf(path, sizeof path, "%s/poll-%llu.ack", directory,
             (unsigned long long)polls);
    const struct timespec delay = {.tv_sec = 0, .tv_nsec = 20000000};
    for (unsigned attempt = 0; attempt < 90000u && access(path, F_OK) != 0; attempt++)
        nanosleep(&delay, NULL);
    if (access(path, F_OK) != 0) fail_loudly("timed out waiting for the batched snapshot copy acknowledgement");
}

static void restore_stop_poll_override(void)
{
    const char *directory = getenv("TSFP_SNAPSHOT_RUN_DIR");
    if (directory == NULL || directory[0] == '\0') return;
    char path[PATH_MAX];
    snprintf(path, sizeof path, "%s/snapshot-stop-poll", directory);
    FILE *file = fopen(path, "r");
    if (file == NULL) return;
    unsigned long long poll = 0ull;
    const int parsed = fscanf(file, "%llu", &poll);
    fclose(file);
    if (parsed == 1 && poll > 0ull) {
        atomic_store(&g_stop_target, (uint64_t)poll);
        atomic_store(&g_stop_fired, 0);
    }
}

static int default_checkpoint(void)
{
    return dmtcp_checkpoint != NULL ? dmtcp_checkpoint() : -1;
}

void host_snapshot_set_checkpoint_function(host_snapshot_checkpoint_fn function)
{
    g_checkpoint = function;
}

bool host_snapshot_find_device_fd(char *path, size_t path_size)
{
    static const char *const prefixes[] = {"/dev/dri/", "/dev/nvidia", "/dev/kfd", "/dev/accel"};
    DIR *directory = opendir("/proc/self/fd");
    if (directory == NULL) return false;
    bool found = false;
    struct dirent *entry;
    while (!found && (entry = readdir(directory)) != NULL) {
        char link[64], target[PATH_MAX];
        snprintf(link, sizeof link, "/proc/self/fd/%s", entry->d_name);
        const ssize_t length = readlink(link, target, sizeof target - 1u);
        if (length <= 0) continue;
        target[length] = '\0';
        for (size_t i = 0u; i < sizeof prefixes / sizeof prefixes[0]; i++) {
            if (strncmp(target, prefixes[i], strlen(prefixes[i])) == 0) {
                snprintf(path, path_size, "%s", target);
                found = true;
                break;
            }
        }
    }
    closedir(directory);
    return found;
}

int host_snapshot_write_mapped_files(const char *out_path)
{
    FILE *maps = fopen("/proc/self/maps", "r");
    FILE *out = fopen(out_path, "w");
    if (maps == NULL || out == NULL) {
        if (maps != NULL) fclose(maps);
        if (out != NULL) fclose(out);
        return -1;
    }
    char (*seen)[PATH_MAX] = calloc(4096u, sizeof *seen);
    size_t seen_count = 0u;
    char line[PATH_MAX + 128];
    int written = 0;
    while (seen != NULL && fgets(line, sizeof line, maps) != NULL) {
        char *path = strchr(line, '/');
        if (path == NULL || strstr(line, "(deleted)") != NULL || strncmp(path, "/memfd:", 7) == 0 ||
            strncmp(path, "/dev/", 5) == 0 || strncmp(path, "/proc/", 6) == 0) continue;
        path[strcspn(path, "\n")] = '\0';
        bool known = false;
        for (size_t i = 0u; i < seen_count && !known; i++) known = strcmp(seen[i], path) == 0;
        if (known || seen_count >= 4096u) continue;
        snprintf(seen[seen_count++], PATH_MAX, "%s", path);
        fprintf(out, "%s\n", path);
        written++;
    }
    free(seen);
    fclose(maps);
    fclose(out);
    return written;
}

static void fail_loudly(const char *why)
{
    fprintf(stderr, "snapshot: REFUSED: %s\n", why);
    fflush(stderr);
    _exit(97);
}

static void take_snapshot(uint64_t polls)
{
    char device[PATH_MAX];
    if (host_snapshot_find_device_fd(device, sizeof device)) {
        char why[PATH_MAX + 200];
        snprintf(why, sizeof why,
                 "the process holds the device file %s, which a process snapshot cannot carry. Run with the "
                 "software Vulkan device (lavapipe) so no GPU file is open (docs/state-snapshot.md, GPU row)",
                 device);
        fail_loudly(why);
    }
    const char *directory = getenv("TSFP_SNAPSHOT_DIR");
    if (directory != NULL && directory[0] != '\0') {
        char list[PATH_MAX];
        snprintf(list, sizeof list, "%s/mapped-files.txt", directory);
        if (host_snapshot_write_mapped_files(list) < 0) fail_loudly("cannot write the mapped-file list");
    }
    set_isolated_path_prefix(polls);
    host_snapshot_checkpoint_fn function = g_checkpoint != NULL ? g_checkpoint : default_checkpoint;
    fprintf(stderr, "snapshot: taking the snapshot at poll %llu of port 0\n", (unsigned long long)polls);
    fflush(stderr);
    dump_snapshot_descriptors("before-checkpoint");
    const int result = function();
    dump_snapshot_descriptors(result == HOST_SNAPSHOT_AFTER_RESTART ? "after-resume" : "after-checkpoint");
    atomic_store(&g_taken_at, polls);
    if (result == HOST_SNAPSHOT_AFTER_CHECKPOINT) {
        fprintf(stderr, "snapshot: checkpoint complete at poll %llu, this run continues\n", (unsigned long long)polls);
    } else if (result == HOST_SNAPSHOT_AFTER_RESTART) {
        fprintf(stderr, "snapshot: RESUMED from the snapshot taken at poll %llu\n", (unsigned long long)polls);
        atomic_store(&g_target_index, g_target_count);
        restore_stop_poll_override();
    } else {
        fail_loudly("the DMTCP layer is not loaded (start the run with python -m tools.snapshot or "
                    "tools.play --snapshot-at-poll) or the checkpoint failed");
    }
    if (g_target_count != 0u) snapshot_acknowledgement(polls, result);
    fflush(stderr);
}

static void on_poll(unsigned port, uint64_t port_polls, void *user)
{
    (void)user;
    if (port != 0u) return;
    if (g_target_count != 0u) {
        const unsigned index = atomic_load(&g_target_index);
        if (index < g_target_count && port_polls == g_targets[index]) {
            unsigned expected_index = index;
            if (atomic_compare_exchange_strong(&g_target_index, &expected_index, index + 1u))
                take_snapshot(port_polls);
        }
    } else {
        const uint64_t target = atomic_load(&g_target);
        int expected = 0;
        if (target != 0u && port_polls == target && atomic_compare_exchange_strong(&g_fired, &expected, 1))
            take_snapshot(port_polls);
    }
    const uint64_t stop_target = atomic_load(&g_stop_target);
    int stop_expected = 0;
    if (stop_target != 0u && port_polls == stop_target && g_stop_function != NULL &&
        atomic_compare_exchange_strong(&g_stop_fired, &stop_expected, 1))
        g_stop_function(port_polls);
}

bool host_snapshot_arm(uint64_t at_poll, char *error, size_t error_size)
{
    if (at_poll == 0u) {
        if (error != NULL && error_size != 0u) snprintf(error, error_size, "--snapshot-at-poll needs a poll index of at least 1");
        return false;
    }
    atomic_store(&g_target, at_poll);
    g_target_count = 0u;
    atomic_store(&g_fired, 0);
    tsfp_input_host_set_poll_observer(on_poll, NULL);
    return true;
}

bool host_snapshot_arm_many(const uint64_t *polls, size_t count, char *error, size_t error_size)
{
    if (polls == NULL || count == 0u || count > sizeof g_targets / sizeof g_targets[0]) {
        if (error != NULL && error_size != 0u) snprintf(error, error_size, "expected 1..64 snapshot polls");
        return false;
    }
    for (size_t index = 0u; index < count; index++) {
        if (polls[index] == 0u || (index != 0u && polls[index] <= polls[index - 1u])) {
            if (error != NULL && error_size != 0u) snprintf(error, error_size, "snapshot polls must be positive, unique and increasing");
            return false;
        }
        g_targets[index] = polls[index];
    }
    g_target_count = (unsigned)count;
    atomic_store(&g_target_index, 0u);
    atomic_store(&g_target, 0u);
    atomic_store(&g_fired, 0);
    tsfp_input_host_set_poll_observer(on_poll, NULL);
    return true;
}

bool host_snapshot_arm_stop(uint64_t at_poll, host_snapshot_stop_fn stop, char *error, size_t error_size)
{
    if (at_poll == 0u || stop == NULL) {
        if (error != NULL && error_size != 0u) snprintf(error, error_size, "--stop-at-poll needs a poll index of at least 1");
        return false;
    }
    g_stop_function = stop;
    atomic_store(&g_stop_fired, 0);
    atomic_store(&g_stop_target, at_poll);
    tsfp_input_host_set_poll_observer(on_poll, NULL);
    return true;
}

uint64_t host_snapshot_taken_at(void)
{
    return atomic_load(&g_taken_at);
}

void host_snapshot_reset(void)
{
    atomic_store(&g_target, 0u);
    atomic_store(&g_taken_at, 0u);
    atomic_store(&g_fired, 0);
    g_target_count = 0u;
    atomic_store(&g_target_index, 0u);
    atomic_store(&g_stop_target, 0u);
    atomic_store(&g_stop_fired, 0);
    g_stop_function = NULL;
    g_checkpoint = NULL;
    tsfp_input_host_set_poll_observer(NULL, NULL);
}
