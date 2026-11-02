/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3745: host memory watchdog (docs/t3745-disk-safety.md "Memory"). A host whose memory grows for
 * hours pushes the owner PC into swap and can take the whole machine down. A detached thread
 * samples /proc/self/statm and /proc/meminfo every TSFP_HOST_RSS_POLL_MS (default 2000):
 *  - soft: RSS over TSFP_HOST_RSS_SOFT_MB, logs once (RSS, MemAvailable) and returns freed heap to
 *    the OS (malloc_trim);
 *  - hard: RSS over TSFP_HOST_RSS_HARD_MB, or system MemAvailable under TSFP_HOST_MEM_AVAIL_FLOOR_MB
 *    while this host holds over a quarter of the RAM, logs and asks the host to stop (SIGTERM, the
 *    normal shutdown path), then exits 75 after TSFP_HOST_RSS_GRACE_MS (default 10000).
 * Defaults (0 or unset): soft 40 % and hard 60 % of MemTotal, floor 2048 MB. tools/dev/play.sh
 * exports the values of tools/data/disk_budget.json ("host_rss_*"). TSFP_HOST_RSS_WATCHDOG=0 is off.
 */
#ifndef TSFP_HOST_RSS_WATCHDOG_H
#define TSFP_HOST_RSS_WATCHDOG_H

#include <malloc.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static inline unsigned long long host_rss_env_mb(const char *name, unsigned long long fallback)
{
    const char *v = getenv(name);
    if (v == NULL || *v == '\0') return fallback;
    unsigned long long n = strtoull(v, NULL, 10);
    return n == 0ull ? fallback : n;
}

/* kB value of one /proc/meminfo field, 0 when absent. */
static inline unsigned long long host_meminfo_kb(const char *field)
{
    FILE *in = fopen("/proc/meminfo", "r");
    if (in == NULL) return 0ull;
    char line[256];
    size_t len = strlen(field);
    unsigned long long kb = 0ull;
    while (fgets(line, sizeof line, in) != NULL) {
        if (strncmp(line, field, len) == 0 && line[len] == ':') {
            kb = strtoull(line + len + 1, NULL, 10);
            break;
        }
    }
    fclose(in);
    return kb;
}

static inline unsigned long long host_rss_mb(void)
{
    FILE *in = fopen("/proc/self/statm", "r");
    if (in == NULL) return 0ull;
    unsigned long long size = 0ull, resident = 0ull;
    if (fscanf(in, "%llu %llu", &size, &resident) != 2) resident = 0ull;
    fclose(in);
    return resident * (unsigned long long)sysconf(_SC_PAGESIZE) >> 20;
}

typedef struct {
    unsigned long long soft_mb, hard_mb, floor_mb, total_mb, poll_ms, grace_ms;
} host_rss_limits;

static inline host_rss_limits host_rss_limits_from_env(void)
{
    host_rss_limits l;
    l.total_mb = host_meminfo_kb("MemTotal") >> 10;
    l.soft_mb = host_rss_env_mb("TSFP_HOST_RSS_SOFT_MB", l.total_mb * 4u / 10u);
    l.hard_mb = host_rss_env_mb("TSFP_HOST_RSS_HARD_MB", l.total_mb * 6u / 10u);
    l.floor_mb = host_rss_env_mb("TSFP_HOST_MEM_AVAIL_FLOOR_MB", 2048u);
    l.poll_ms = host_rss_env_mb("TSFP_HOST_RSS_POLL_MS", 2000u);
    l.grace_ms = host_rss_env_mb("TSFP_HOST_RSS_GRACE_MS", 10000u);
    return l;
}

/* 0 fine, 1 soft, 2 hard. */
static inline int host_rss_verdict(const host_rss_limits *l, unsigned long long rss_mb, unsigned long long avail_mb)
{
    if (rss_mb > l->hard_mb) return 2;
    if (avail_mb != 0ull && avail_mb < l->floor_mb && rss_mb > l->total_mb / 4u) return 2;
    if (rss_mb > l->soft_mb) return 1;
    return 0;
}

static inline void host_rss_sleep_ms(unsigned long long ms)
{
    struct timespec ts = {(time_t)(ms / 1000u), (long)(ms % 1000u) * 1000000L};
    while (nanosleep(&ts, &ts) != 0) {
    }
}

static void *host_rss_watchdog_main(void *arg)
{
    host_rss_limits l = *(host_rss_limits *)arg;
    free(arg);
    int soft_logged = 0;
    for (;;) {
        host_rss_sleep_ms(l.poll_ms);
        unsigned long long rss = host_rss_mb(), avail = host_meminfo_kb("MemAvailable") >> 10;
        int verdict = host_rss_verdict(&l, rss, avail);
        if (verdict == 1 && !soft_logged) {
            soft_logged = 1;
            fprintf(stderr, "host: memory watchdog: RSS %llu MB over the soft limit %llu MB (MemAvailable %llu MB); "
                            "trimming the heap (T3745)\n", rss, l.soft_mb, avail);
            malloc_trim(0);
        } else if (verdict == 2) {
            fprintf(stderr, "host: memory watchdog: RSS %llu MB (hard limit %llu MB), MemAvailable %llu MB (floor %llu MB): "
                            "stopping the host before the PC swaps (T3745, docs/t3745-disk-safety.md)\n",
                    rss, l.hard_mb, avail, l.floor_mb);
            kill(getpid(), SIGTERM);
            host_rss_sleep_ms(l.grace_ms);
            fprintf(stderr, "host: memory watchdog: no clean stop in %llu ms, exiting 75\n", l.grace_ms);
            _exit(75);
        }
    }
    return NULL;
}

/* Starts the watchdog once per process. Returns 1 when it runs. */
static inline int host_rss_watchdog_start(void)
{
    static int started;
    if (started) return 1;
    const char *off = getenv("TSFP_HOST_RSS_WATCHDOG");
    if (off != NULL && strcmp(off, "0") == 0) return 0;
    host_rss_limits *l = malloc(sizeof *l);
    if (l == NULL) return 0;
    *l = host_rss_limits_from_env();
    if (l->total_mb == 0ull && getenv("TSFP_HOST_RSS_HARD_MB") == NULL) {
        free(l);
        return 0; /* no /proc/meminfo and no explicit limit */
    }
    pthread_t thread;
    pthread_attr_t attr;
    pthread_attr_init(&attr);
    pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
    pthread_attr_setstacksize(&attr, 64u * 1024u);
    int rc = pthread_create(&thread, &attr, host_rss_watchdog_main, l);
    pthread_attr_destroy(&attr);
    if (rc != 0) {
        free(l);
        return 0;
    }
    started = 1;
    return 1;
}

#endif
