/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3745: no host core dumps on the root disk (docs/t3745-disk-safety.md, incident class
 * host-core-dumps-on-root). The container's kernel.core_pattern pipes every core to the HOST's
 * systemd-coredump (/var/lib/systemd/coredump on the owner's root disk) and `ulimit -c` is
 * unlimited, so one crash of a host with a 4 GB guest mapping plus its heap streams many GB there.
 *
 * host_core_policy_apply() sets the soft RLIMIT_CORE to 0 (children inherit it). Opt-in:
 * TSFP_CORE_DUMPS=1 allows cores capped at TSFP_CORE_DUMP_MAX_MB (default 2048), and only when
 * core_pattern is a plain path whose directory is on the data disk (not the root filesystem, see
 * TSFP_DATADISK_ANCHOR); a pipe pattern or a root-disk directory keeps cores off and says so.
 * The T2145 crash report (stderr, the play dir's host.err) is unchanged.
 */
#ifndef TSFP_HOST_CORE_POLICY_H
#define TSFP_HOST_CORE_POLICY_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/stat.h>

/* 1 when `dir` is on the root filesystem while a separate data disk exists. */
static inline int host_core_on_root_disk(const char *dir)
{
    const char *root_ref = getenv("TSFP_DATADISK_ROOT_REF");
    const char *anchor = getenv("TSFP_DATADISK_ANCHOR");
    struct stat sd, sr, sa;
    if (stat(dir, &sd) != 0 || stat(root_ref != NULL && *root_ref ? root_ref : "/", &sr) != 0) return 1;
    if (sd.st_dev != sr.st_dev) return 0;
    if (stat(anchor != NULL && *anchor ? anchor : "/workspace", &sa) != 0) return 1;
    return sa.st_dev != sr.st_dev;
}

/* Returns the soft core limit it set, in bytes. `pattern_path` is /proc/sys/kernel/core_pattern
 * (a parameter for the test). */
static inline unsigned long long host_core_policy_apply_from(const char *pattern_path)
{
    struct rlimit lim;
    if (getrlimit(RLIMIT_CORE, &lim) != 0) lim.rlim_max = RLIM_INFINITY;
    unsigned long long allowed = 0ull;
    const char *want = getenv("TSFP_CORE_DUMPS");
    if (want != NULL && strcmp(want, "1") == 0) {
        char pattern[512] = "";
        FILE *in = fopen(pattern_path, "r");
        if (in != NULL) {
            if (fgets(pattern, sizeof pattern, in) == NULL) pattern[0] = '\0';
            fclose(in);
        }
        pattern[strcspn(pattern, "\n")] = '\0';
        char dir[512];
        snprintf(dir, sizeof dir, "%s", pattern);
        char *slash = strrchr(dir, '/');
        if (slash == NULL) snprintf(dir, sizeof dir, ".");
        else if (slash == dir) slash[1] = '\0';
        else *slash = '\0';
        if (pattern[0] == '\0' || pattern[0] == '|' || host_core_on_root_disk(dir)) {
            fprintf(stderr,
                    "host: TSFP_CORE_DUMPS=1 ignored: core_pattern '%s' is a pipe or on the root disk; "
                    "cores stay off (T3745)\n", pattern);
        } else {
            const char *max_mb = getenv("TSFP_CORE_DUMP_MAX_MB");
            allowed = (max_mb != NULL && *max_mb ? strtoull(max_mb, NULL, 10) : 2048ull) << 20;
            if (lim.rlim_max != RLIM_INFINITY && allowed > (unsigned long long)lim.rlim_max)
                allowed = (unsigned long long)lim.rlim_max;
            fprintf(stderr, "host: core dumps on, capped at %llu MB, pattern %s (T3745)\n", allowed >> 20,
                    pattern);
        }
    }
    lim.rlim_cur = (rlim_t)allowed;
    (void)setrlimit(RLIMIT_CORE, &lim);
    return allowed;
}

static inline unsigned long long host_core_policy_apply(void)
{
    return host_core_policy_apply_from("/proc/sys/kernel/core_pattern");
}

#endif
