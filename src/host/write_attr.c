/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T2057 (S3-0): opt-in dynamic WRITE attribution for guest records. NOT linked into any build: it is a standalone
 * LD_PRELOAD shared object that tools/s3_write_attr.py compiles into tmp/ and injects into an unchanged host.
 * Observation only. Default OFF: without TSFP_WRITE_ATTR_OUT the constructor does nothing at all.
 *
 *   TSFP_WRITE_ATTR_OUT=FILE   aggregate output, rewritten every 5 s and at exit
 *   TSFP_WRITE_ATTR_SPEC=...   families, ';' separated: name=cell,count,stride,size,deref[,countcell[,check]] (hex; check = offset in the record of a pointer that must be
 *                              plausible, a cheap liveness test so stale table slots are not protected)
 *                              record = deref+1 chained 32-bit reads of cell+i*stride (deref -1: the cell itself
 *                              is the record base), i < min(count, [countcell]) when countcell is given
 *   TSFP_WRITE_ATTR_GATE=cell=value  start watching only once the 32-bit cell holds value (for example level_state)
 *
 * Mechanism: a poll thread resolves the record pointers and write-protects the pages that hold the records
 * (mprotect PROT_READ). A store faults (SIGSEGV), the handler logs (faulting rip, family, 4-byte field offset),
 * unprotects the page, sets the trap flag and lets that one instruction run, the SIGTRAP handler protects the
 * page again. The host's own SIGSEGV handler is kept: faults that are not ours are forwarded to it.
 * Writer = faulting rip as a link-time offset (rip - load bias), caller = the first exe text address found in the 48
 * words above rsp (a guess, it is the return into the function that called a helper such as memcpy); when the rip is in
 * a library the caller is logged as the writer, flagged `via`. Records are tracked live: a record whose pointer
 * disappeared from its cell is unprotected again. tools/s3_write_attr.py
 * maps offsets to functions with nm.
 * Limits: stores done by the kernel (read() into a watched page) fail with EFAULT instead of being logged; a store
 * by another thread to the same page inside the single-step window is missed; rep string stores are logged once
 * per iteration (counts of those are inflated); store width is not decoded. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <link.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <ucontext.h>
#include <unistd.h>

#define MAXFAM 32
#define MAXREC 8192
#define SLOTS (1u << 20)
#define LO 0x00010000u
#define HI 0x7FFFF000u

typedef struct { char name[24]; uint32_t cell, count, stride, size, countcell, check; int deref; } Family;
typedef struct { uint32_t base, size; uint8_t family, live; uint32_t epoch; } Record;
typedef struct { uint32_t writer, caller, fam_off; uint64_t count; } Slot;

static Family fam[MAXFAM];
static int nfam;
static Record rec[MAXREC];
static _Atomic int nrec;
static Slot *slots;
static volatile uint8_t pflag[HI >> 12]; /* 1: we protected this page at some time, 2: the handler unprotected it, scan must re-protect */
static _Atomic uint64_t faults, other_faults, dropped;
static const char *out_path;
static uint32_t gate_cell, gate_value;
static uintptr_t bias, text_lo, text_hi;
static struct sigaction host_segv;
static int host_segv_set;
static __thread uintptr_t step[4];
static _Atomic uint16_t pending[HI >> 12]; /* threads that are inside their single-step window on this page */
static int (*real_sigaction)(int, const struct sigaction *, struct sigaction *);

static int phdr_cb(struct dl_phdr_info *info, size_t size, void *data)
{
    (void)size; (void)data;
    if (info->dlpi_name[0] != '\0') return 0;
    bias = info->dlpi_addr;
    for (int i = 0; i < info->dlpi_phnum; i++) {
        const ElfW(Phdr) *p = &info->dlpi_phdr[i];
        if (p->p_type == PT_LOAD && (p->p_flags & PF_X)) { text_lo = bias + p->p_vaddr; text_hi = text_lo + p->p_memsz; }
    }
    return 1;
}

static int mapped(uintptr_t page) { unsigned char v; return mincore((void *)page, 1, &v) == 0; }
static int plausible(uint32_t p) { return p >= LO && p < HI && mapped(p & ~0xFFFu); }
static uint32_t rd(uint32_t a) { return *(volatile uint32_t *)(uintptr_t)a; }

static atomic_flag table_lock = ATOMIC_FLAG_INIT;

static void bump(uint32_t writer, uint32_t caller, int family, uint32_t off)
{
    const uint32_t fam_off = ((uint32_t)(family + 1) << 24) | (off & 0xFFFFFFu);
    uint64_t hash = ((uint64_t)writer << 32 | caller) * 0x9E3779B97F4A7C15ull ^ (uint64_t)fam_off * 0xC2B2AE3D27D4EB4Full;
    uint32_t i = (uint32_t)(hash >> 40) & (SLOTS - 1u);
    while (atomic_flag_test_and_set_explicit(&table_lock, memory_order_acquire)) {}
    for (int probe = 0; probe < 256; probe++, i = (i + 1u) & (SLOTS - 1u)) {
        Slot *s = &slots[i];
        if (s->fam_off == 0u) { s->writer = writer; s->caller = caller; s->fam_off = fam_off; }
        if (s->writer == writer && s->caller == caller && s->fam_off == fam_off) { s->count++; goto done; }
    }
    atomic_fetch_add(&dropped, 1u);
done:
    atomic_flag_clear_explicit(&table_lock, memory_order_release);
}

static int find_record(uint32_t addr)
{
    const int n = atomic_load(&nrec);
    for (int i = 0; i < n; i++) if (rec[i].live && addr - rec[i].base < rec[i].size) return i;
    return -1;
}

static int page_watched(uint32_t page)
{
    const int n = atomic_load(&nrec);
    for (int i = 0; i < n; i++)
        if (rec[i].live && page < ((rec[i].base + rec[i].size + 0xFFFu) & ~0xFFFu) && page + 0x1000u > rec[i].base) return 1;
    return 0;
}

static void on_segv(int sig, siginfo_t *info, void *vctx)
{
    ucontext_t *ctx = vctx;
    const uintptr_t addr = (uintptr_t)info->si_addr;
    if (addr >= LO && addr < HI && page_watched((uint32_t)(addr & ~0xFFFu))) {
        const int r = find_record((uint32_t)addr);
        uintptr_t pc = (uintptr_t)ctx->uc_mcontext.gregs[REG_RIP], caller = 0u;
        uint32_t via = 0u;
        const uintptr_t *sp = (const uintptr_t *)ctx->uc_mcontext.gregs[REG_RSP];
        for (int i = 0; i < 48; i++) if (sp[i] >= text_lo && sp[i] < text_hi) { caller = sp[i]; break; }
        if (pc < text_lo || pc >= text_hi) { pc = caller; via = 0x80000000u; }
        atomic_fetch_add(&faults, 1u);
        if (r >= 0) bump(((uint32_t)(pc - bias)) | via, caller ? (uint32_t)(caller - bias) : 0u, rec[r].family, ((uint32_t)addr - rec[r].base) & ~3u);
        else atomic_fetch_add(&other_faults, 1u);
        const uintptr_t page = addr & ~(uintptr_t)0xFFF;
        mprotect((void *)page, 0x1000, PROT_READ | PROT_WRITE);
        int slot = 0;
        while (slot < 4 && step[slot]) slot++;
        if (slot < 4) {
            step[slot] = page;
            atomic_fetch_add(&pending[page >> 12], 1u);
            ctx->uc_mcontext.gregs[REG_EFL] |= 0x100;
        } else {
            pflag[page >> 12] = 2; /* no room to track the step: leave the page writable, the next scan protects it again */
        }
        return;
    }
    if (addr >= LO && addr < HI && pflag[addr >> 12] && !page_watched((uint32_t)(addr & ~0xFFFu))) {
        /* the page was retired between the fault and now: make it writable and retry the instruction */
        pflag[addr >> 12] = 2;
        mprotect((void *)(addr & ~(uintptr_t)0xFFF), 0x1000, PROT_READ | PROT_WRITE);
        return;
    }
    {
        char msg[160];
        const int n = snprintf(msg, sizeof msg, "WRITE_ATTR forward SEGV addr %lx rip %lx code %d pflag %d\n", (unsigned long)addr,
                               (unsigned long)(ctx->uc_mcontext.gregs[REG_RIP] - bias), info->si_code, addr < HI ? pflag[addr >> 12] : -1);
        if (write(2, msg, (size_t)n) < 0) {}
        FILE *maps = fopen("/proc/self/maps", "r");
        char line[300];
        while (maps && fgets(line, sizeof line, maps)) {
            unsigned long lo, hi;
            if (sscanf(line, "%lx-%lx", &lo, &hi) == 2 && addr >= lo && addr < hi) { fputs("WRITE_ATTR map ", stderr); fputs(line, stderr); }
        }
        if (maps) fclose(maps);
    }
    if (host_segv_set && (host_segv.sa_flags & SA_SIGINFO) && host_segv.sa_sigaction) { host_segv.sa_sigaction(sig, info, vctx); return; }
    if (host_segv_set && host_segv.sa_handler != SIG_DFL && host_segv.sa_handler != SIG_IGN) { host_segv.sa_handler(sig); return; }
    {
        char msg[200];
        const int n = snprintf(msg, sizeof msg, "WRITE_ATTR unhandled SEGV addr %lx rip %lx off %lx pflag %d step %lx %lx watched %d\n", (unsigned long)addr,
                               (unsigned long)ctx->uc_mcontext.gregs[REG_RIP], (unsigned long)(ctx->uc_mcontext.gregs[REG_RIP] - bias),
                               addr < HI ? pflag[addr >> 12] : -1, (unsigned long)step[0], (unsigned long)step[1], addr < HI ? page_watched((uint32_t)(addr & ~0xFFFu)) : -1);
        if (write(2, msg, (size_t)n) < 0) {}
    }
    signal(SIGSEGV, SIG_DFL);
}

static void on_trap(int sig, siginfo_t *info, void *vctx)
{
    (void)sig; (void)info;
    ucontext_t *ctx = vctx;
    for (int i = 0; i < 4; i++) if (step[i]) {
        if (atomic_fetch_sub(&pending[step[i] >> 12], 1u) == 1u) mprotect((void *)step[i], 0x1000, PROT_READ);
        step[i] = 0;
    }
    ctx->uc_mcontext.gregs[REG_EFL] &= ~0x100ull;
}

/* Every thread gets a signal stack: a fault on a nearly full thread stack could not otherwise deliver our handler
 * (the kernel needs a few KB for the register frame) and the process would die silently with SIGSEGV. */
typedef struct { void *(*fn)(void *); void *arg; } Trampoline;

static void give_signal_stack(void)
{
    stack_t ss = {.ss_sp = malloc(1u << 18), .ss_size = 1u << 18};
    if (ss.ss_sp) sigaltstack(&ss, NULL);
}

static void *trampoline(void *raw)
{
    const Trampoline t = *(Trampoline *)raw;
    free(raw);
    give_signal_stack();
    return t.fn(t.arg);
}

int pthread_create(pthread_t *thread, const pthread_attr_t *attr, void *(*fn)(void *), void *arg)
{
    static int (*real)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);
    if (!real) real = dlsym(RTLD_NEXT, "pthread_create");
    if (out_path == NULL) return real(thread, attr, fn, arg);
    Trampoline *t = malloc(sizeof *t);
    if (!t) return real(thread, attr, fn, arg);
    t->fn = fn; t->arg = arg;
    return real(thread, attr, trampoline, t);
}

/* The host installs its own SIGSEGV handler after us: keep ours, remember theirs, forward what is not ours. */
int sigaction(int signum, const struct sigaction *act, struct sigaction *old)
{
    if (!real_sigaction) real_sigaction = dlsym(RTLD_NEXT, "sigaction");
    if (signum == SIGSEGV && out_path != NULL && act != NULL) {
        if (old) *old = host_segv_set ? host_segv : (struct sigaction){0};
        host_segv = *act; host_segv_set = 1;
        return 0;
    }
    return real_sigaction(signum, act, old);
}

static uint32_t epoch;

static void protect_pages(const Record *r, int prot)
{
    for (uint32_t page = r->base & ~0xFFFu; page < r->base + r->size; page += 0x1000u)
        if (mapped(page)) { pflag[page >> 12] = 1; mprotect((void *)(uintptr_t)page, 0x1000, prot); }
}

static void add_record(int family, uint32_t base)
{
    const int n = atomic_load(&nrec);
    if (!plausible(base)) return;
    for (int i = 0; i < n; i++) if (rec[i].base == base) {
        rec[i].epoch = epoch;
        if (!rec[i].live) { rec[i].family = (uint8_t)family; rec[i].size = fam[family].size; rec[i].live = 1; protect_pages(&rec[i], PROT_READ); }
        return;
    }
    if (n >= MAXREC) return;
    rec[n] = (Record){base, fam[family].size, (uint8_t)family, 1, epoch};
    atomic_store(&nrec, n + 1);
    protect_pages(&rec[n], PROT_READ);
}

static void scan(void)
{
    epoch++;
    for (int f = 0; f < nfam; f++) {
        const Family *x = &fam[f];
        if (x->deref < 0) { add_record(f, x->cell); continue; }
        uint32_t n = x->count;
        if (x->countcell && plausible(x->countcell) && rd(x->countcell) < n) n = rd(x->countcell);
        for (uint32_t i = 0; i < n; i++) {
            uint32_t p = x->cell + i * x->stride;
            for (int d = 0; d <= x->deref && plausible(p); d++) p = rd(p);
            if (plausible(p) && p >= 0x10000000u && (x->check == 0u || (plausible(p + x->check) && plausible(rd(p + x->check)))))
                add_record(f, p);
        }
    }
}

static void retire(void)
{
    const int n = atomic_load(&nrec);
    for (int i = 0; i < n; i++) if (rec[i].live)
        for (uint32_t page = rec[i].base & ~0xFFFu; page < rec[i].base + rec[i].size; page += 0x1000u)
            if (pflag[page >> 12] == 2 && pending[page >> 12] == 0u) { pflag[page >> 12] = 1; mprotect((void *)(uintptr_t)page, 0x1000, PROT_READ); }
    for (int i = 0; i < n; i++) if (rec[i].live && rec[i].epoch != epoch) {
        rec[i].live = 0;
        for (uint32_t page = rec[i].base & ~0xFFFu; page < rec[i].base + rec[i].size; page += 0x1000u)
            if (!page_watched(page) && mapped(page)) mprotect((void *)(uintptr_t)page, 0x1000, PROT_READ | PROT_WRITE);
    }
}

static uint64_t now_ms(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint64_t)t.tv_sec * 1000u + (uint64_t)t.tv_nsec / 1000000u;
}

static uint64_t start_ms;

static void dump_to(const char *path)
{
    char tmp[600];
    snprintf(tmp, sizeof tmp, "%s.tmp", path);
    FILE *f = fopen(tmp, "w");
    if (!f) return;
    fprintf(f, "META faults %llu other %llu dropped %llu records %d bias %llx elapsed_ms %llu\n", (unsigned long long)faults,
            (unsigned long long)other_faults, (unsigned long long)dropped, atomic_load(&nrec), (unsigned long long)bias, (unsigned long long)(now_ms() - start_ms));
    for (int i = 0; i < nfam; i++) {
        int n = 0;
        for (int r = 0; r < atomic_load(&nrec); r++) n += rec[r].family == i;
        fprintf(f, "FAM %s %d %x\n", fam[i].name, n, fam[i].size);
    }
    while (atomic_flag_test_and_set_explicit(&table_lock, memory_order_acquire)) {}
    for (uint32_t i = 0; i < SLOTS; i++) {
        const Slot *x = &slots[i];
        if (x->fam_off) fprintf(f, "W %08x %08x %s %x %llu\n", x->writer, x->caller, fam[(x->fam_off >> 24) - 1].name,
                                x->fam_off & 0xFFFFFF, (unsigned long long)x->count);
    }
    atomic_flag_clear_explicit(&table_lock, memory_order_release);
    fclose(f);
    rename(tmp, path);
}

static void dump(void) { dump_to(out_path); }

static void *poller(void *arg)
{
    (void)arg;
    unsigned ticks = 0;
    for (;; usleep(20000)) {
        if (gate_cell && (!plausible(gate_cell) || rd(gate_cell) != gate_value)) continue;
        scan();
        retire();
        if (++ticks % 100u == 0u) {
            char snap[600];
            snprintf(snap, sizeof snap, "%s.%llu", out_path, (unsigned long long)(now_ms() - start_ms));
            dump_to(snap);
        }
    }
    return NULL;
}

static void parse(const char *spec)
{
    char *copy = strdup(spec);
    char *outer = NULL;
    for (char *part = strtok_r(copy, ";", &outer); part && nfam < MAXFAM; part = strtok_r(NULL, ";", &outer)) {
        Family *x = &fam[nfam];
        char *eq = strchr(part, '=');
        if (!eq) continue;
        *eq = 0;
        snprintf(x->name, sizeof x->name, "%s", part);
        x->count = 1; x->stride = 4; x->size = 0x100; x->deref = 0; x->countcell = 0; x->check = 0;
        char *inner = NULL;
        int k = 0;
        for (char *t = strtok_r(eq + 1, ",", &inner); t && k < 7; t = strtok_r(NULL, ",", &inner), k++) {
            if (k == 0) x->cell = (uint32_t)strtoul(t, NULL, 16);
            else if (k == 1) x->count = (uint32_t)strtoul(t, NULL, 16);
            else if (k == 2) x->stride = (uint32_t)strtoul(t, NULL, 16);
            else if (k == 3) x->size = (uint32_t)strtoul(t, NULL, 16);
            else if (k == 4) x->deref = (int)strtol(t, NULL, 10);
            else if (k == 5) x->countcell = (uint32_t)strtoul(t, NULL, 16);
            else x->check = (uint32_t)strtoul(t, NULL, 16);
        }
        nfam++;
    }
    free(copy);
}

__attribute__((constructor)) static void init(void)
{
    out_path = getenv("TSFP_WRITE_ATTR_OUT");
    const char *spec = getenv("TSFP_WRITE_ATTR_SPEC");
    if (!out_path || !*out_path || !spec) { out_path = NULL; return; }
    real_sigaction = dlsym(RTLD_NEXT, "sigaction");
    start_ms = now_ms();
    parse(spec);
    const char *gate = getenv("TSFP_WRITE_ATTR_GATE");
    if (gate) { gate_cell = (uint32_t)strtoul(gate, NULL, 16); const char *e = strchr(gate, '='); gate_value = e ? (uint32_t)strtoul(e + 1, NULL, 16) : 0; }
    dl_iterate_phdr(phdr_cb, NULL);
    slots = calloc(SLOTS, sizeof *slots);
    struct sigaction sa = {0};
    sa.sa_sigaction = on_segv; sa.sa_flags = SA_SIGINFO | SA_ONSTACK | SA_NODEFER; sigemptyset(&sa.sa_mask);
    real_sigaction(SIGSEGV, &sa, NULL);
    sa.sa_sigaction = on_trap;
    real_sigaction(SIGTRAP, &sa, NULL);
    pthread_t thread;
    pthread_create(&thread, NULL, poller, NULL);
    give_signal_stack();
    atexit(dump);
}
