/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3740: capture runtime for capture-based Tier 2 (T3738, rule 43). Linked only into the capture
 * build (TSFP_BUILD_CAPTURE=1), where tools/t2capture/transform.py has rewritten every lifted
 * chunk to reach guest memory through t2cap_ld* / t2cap_st* and to bracket every function with
 * t2cap_enter / t2cap_exit and every generated call with t2cap_call_begin / t2cap_call_end
 * (src/host/t2_capture.h). The default build never compiles this file.
 *
 * Off unless TSFP_T2_CAPTURE=<spool path> is set: then every access is also recorded.
 *
 * Per guest thread (a pthread each, kernel_thread.c) a frame stack, no lock:
 *  - a frame per lifted function (CAP_ENTER) and per call site (RECOMP_ABI_CALL). A call-site frame
 *    whose callee then runs CAP_ENTER with the same VA is ADOPTED: one frame, two roles. A
 *    call-site frame that is never entered is a callee without lifted code (kernel thunk, HLE,
 *    Rust port, intercepted call): it is still a recorded callee of its parent;
 *  - frames pair by the NATIVE stack position of the lifted C function that opened them
 *    (__builtin_frame_address of the hook; the capture chunks are built with
 *    -fno-optimize-sibling-calls so `CAP_EXIT(); return;` is never a tail jump). An exit or a new
 *    entry at a shallower native position pops the deeper frames: they were left without their own
 *    exit (SEH unwind or longjmp) and carry the seh_unwind flag, as does the frame that absorbs them;
 *  - read-set R: the first read of each byte the frame has not defined, value included;
 *  - write-set W: the final value of every byte the frame or its callees wrote; a byte in W is
 *    "defined" and no longer enters R. A popped frame merges its W into its parent (also when it
 *    is not kept, so the parent's callee stub stays exact), and its R into the parent's (bytes the parent had
 *    not defined before the call);
 *  - both sets are maps keyed by 4-byte word with a byte mask (open addressing over a dense entry
 *    array), capped at 64 KiB of bytes each; callees are capped at 256 (flag `size`).
 *
 * At pop a function frame is offered to the per-function sampler (shared, one spin lock per
 * function): shape = the callee VA sequence plus the R and W words classed esp-relative, this/argument-relative,
 * image-absolute or dynamic (counted only), plus size buckets; digest = the entry registers plus the R addresses
 * and bytes. Both are 64-bit hashes (cheap; tools.t2corpus recomputes its own sha256 digest and shape at ingest).
 * Kept: a new shape, or a new digest of a known shape while the shape has < 8 kept, both while the function has
 * < 256 kept (TSFP_T2_CAPTURE_HARD / _FUNC_CAP); after that 1 call in 1,000 is offered (at most
 * TSFP_T2_CAPTURE_OFFER_MAX, default 64, per function). Duplicate digests are never kept. A frame under a
 * cooperative safe point (t2cap_safepoint) is a root; its writes flag the frames below thread_switch.
 *
 * T3803 per-VA budget (TSFP_T2_CAPTURE_BUDGET = K, default 32, 0 = off): once a VA has K kept entries (the corpus
 * entries of TSFP_T2_CAPTURE_BUDGET_SEED plus this run's) AND min(3, its CFG path count) shapes (an unknown count
 * needs 3), its frames are no longer offered, hashed or written: one lock-free table lookup at push. A skipped
 * frame still tracks R and W while an ancestor records (its subtree writes and first reads reach that ancestor
 * like an unkept sampled frame's). A skipped frame with no recording ancestor (every frame below it skipped, a
 * safe-point root, or an ancestor whose R and W both overflowed) is QUIET: it records nothing but whether it
 * wrote. The stats count budget_skipped calls: entries of those VAs are capped by design.
 *
 * T3833 ALLOW list (TSFP_T2_CAPTURE_TARGETS = file of `<va hex>` lines, live targeted capture from play.sh
 * --capture-targets): only the listed VAs are offered, hashed or written; every other frame is skipped exactly like
 * a budget-full one (it still forwards R and W to a recording ancestor, else it is quiet). The budget still applies
 * to the targets. t2cap_target_listed() lets the host send target Rust roots back to their lifted bodies.
 *
 * Kept frames are written to the spool in exactly the T3741 format (tools/t2corpus/spool.py):
 * magic, u32 header length, JSON header, then u32 length + record, one fflush per record, under
 * one mutex. At exit (and from t2cap_flush) <spool>.stats.json holds the counters.
 *
 * T3745 disk safety (docs/t3745-disk-safety.md), all checked here, in the writer itself:
 *  - the spool directory must not be on the root filesystem when a separate data disk exists
 *    (st_dev of the directory vs TSFP_T2_CAPTURE_ROOT_REF, default "/", and TSFP_DATADISK_ANCHOR,
 *    default "/workspace"); a refused path leaves capture off and the game running;
 *  - segments: with TSFP_T2_CAPTURE_SEGMENT_BYTES (default 256 MiB, 0 = one plain spool file) the
 *    records go to gzip segments <spool>.seg-NNNNNN.gz (written as .gz.part, renamed when closed), each
 *    a complete spool; tools.t2corpus.compact ingests finished segments into the corpus and deletes them;
 *  - hard stop: at TSFP_T2_CAPTURE_MAX_BYTES on disk (default 4 GiB, all segments of the run, also
 *    the deleted ones) or when the spool filesystem has less than TSFP_T2_CAPTURE_MIN_FREE_GB free
 *    (default 60), capture stops cleanly: the open segment is closed, the stats say truncated plus
 *    stop_reason, one stderr line, and the game keeps running with capture off;
 *  - a capture host never writes a core dump (RLIMIT_CORE 0: cores land on the host root disk).
 */
#define _GNU_SOURCE
#define TSFP_CAPTURE_RUNTIME 1
#include "t2_capture.h"
#include "rust_ext_bridge.h"
#include "recomp_copy.h"
#include "tsfp_input_host_wiring.h"

#include <errno.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/uio.h>
#include <time.h>
#include <unistd.h>
#include <zlib.h>

#ifdef TSFP_HAVE_MEASURED_ARITIES
#include "kernel_arity.inc"
#endif

#ifndef TSFP_T2_LIFTED_TREE_HASH
#define TSFP_T2_LIFTED_TREE_HASH ""
#endif

/* ---- guest state (defined by the host runtime, recomp_runtime.c) ---- */
typedef union { uint32_t u[4]; } t2cap_xmm;
extern __thread uint32_t g_eax, g_ecx, g_edx, g_esp, g_ebx, g_esi, g_edi, g_ebp;
extern __thread double g_fp_stack[8];
extern __thread int g_fp_top;
extern __thread uint16_t g_fp_control_word;
extern __thread t2cap_xmm g_xmm0, g_xmm1, g_xmm2, g_xmm3, g_xmm4, g_xmm5, g_xmm6, g_xmm7;
/* T3740 fidelity: thread state the lifted code reads implicitly (not in the register file). Frameless and
 * fpo_leaf bodies start with `ebp = g_ebp` / `ebp = g_seh_ebp` (the caller's frame), SEH prologs hand
 * g_seh_ebp to the caller, flags cross calls through the flag bridge, x87 compares through g_fp_cmp/cc.
 * The same set recomp_callback.c saves around a guest callback. */
extern __thread uint32_t g_seh_ebp, g_fs_base, g_flag_bridge_eflags, g_flag_bridge_mask;
extern __thread int g_df, g_fp_cmp;
extern __thread uint16_t g_fp_cc;
extern ptrdiff_t g_xbox_mem_offset;
extern uint32_t g_xbox_code_lo, g_xbox_code_hi;
/* Provenance helpers; absent in synthetic tests. */
extern uint32_t kernel_thread_current_identity(void) __attribute__((weak));

#define T2_FLAG_MMIO 1u
#define T2_FLAG_THREAD_SWITCH 2u
#define T2_FLAG_SIZE 4u
#define T2_FLAG_SEH 8u
#define T2_FLAG_FPU 16u
#define T2_FLAG_SELFMOD 32u
/* 64 is the readers' synthetic old_format. T3767: a byte the frame knew (wrote, or read) changed
 * behind the capture with no callee to attribute it to (host code between two of its accesses). */
#define T2_FLAG_HOST_WRITE 128u
#define T2_FLAG_INPUT_UNVERIFIED 256u
#define T2_NFLAGS 9
/* T3740 record format 2: the record carries the implicit-state blocks (see spool_frame). Not a
 * capture_incomplete reason; readers strip it and flag records without it `old_format`. */
#define T2_FORMAT_IMPLICIT 0x80000000u
#define T2_FORMAT_INPUT_ANNOTATIONS 0x20000000u
#define T2_RECORD_FORMAT 2

/* T3836: per-run size caps (TSFP_T2_CAPTURE_MAX_SET_BYTES / _MAX_CALLEES), defaults unchanged, clamped to a
 * hard ceiling so one record stays bounded (tools/t2corpus/spool.py MAX_RECORD_BYTES holds the worst case). */
#define T2_SET_CAP_DEFAULT (64u * 1024u)
#define T2_SET_CAP_CEILING (4u * 1024u * 1024u)
#define T2_CALLEES_DEFAULT 256u
#define T2_CALLEES_CEILING 16384u
#define T2_RECORD_CEILING (64u * 1024u * 1024u) /* = spool.py MAX_RECORD_BYTES */
#define T2_MAX_ARGS 16u
#define T2_MAX_DEPTH 65536u
#define T2_MMIO_LO 0xFD000000u /* NV2A 0xFD000000, APU 0xFE800000, AC97 0xFEC00000, USB, NIC, flash to 0xFFFFFFFF */
#define T2_KERNEL_THUNK_BASE 0xFE000000u
#define T2_ESP_WINDOW 0x10000u

static inline bool range_intersects(uint32_t addr, uint32_t size, uint32_t lo, uint64_t hi)
{
    if (size == 0u) return false;
    uint64_t start = addr;
    uint64_t end = start + size;
    return start < hi && end > lo;
}

static inline bool is_mmio_range(uint32_t addr, uint32_t size)
{
    /* NV2A starts at FD000000; APU, AC97, USB, NIC, flash and device thunks
     * occupy the rest of the Xbox high device window. */
    return range_intersects(addr, size, T2_MMIO_LO, 0x100000000ull);
}

#define T2_LIKELY(x) __builtin_expect(!!(x), 1)
#define T2_UNLIKELY(x) __builtin_expect(!!(x), 0)
#define T2_HOOK __attribute__((noinline))
#define T2_SP() ((uintptr_t)__builtin_frame_address(0))

/* ---- XBE sections (t2cap_note_section, from main.c set_code_bounds) ---- */
#define T2_MAX_SECTIONS 32u
typedef struct { uint32_t lo; uint64_t hi; bool exec; } t2_section;
static t2_section g_sections[T2_MAX_SECTIONS];
static uint32_t g_nsections;
static uint32_t g_exec_lo = UINT32_MAX;
static uint64_t g_exec_hi;
static uint32_t g_image_lo = UINT32_MAX;
static uint64_t g_image_hi;

/* Called once per section while the XBE is mapped, before any guest code runs. `executable` means read-only
 * code: main.c passes executable and not writable (the title's .data and the D3D/DSOUND/XMV library sections are
 * flagged executable and writable, and are written all the time). */
void t2cap_note_section(uint32_t va, uint32_t size, bool executable)
{
    if (g_nsections >= T2_MAX_SECTIONS || size == 0u) return;
    t2_section *s = &g_sections[g_nsections++];
    s->lo = va;
    s->hi = (uint64_t)va + size;
    s->exec = executable;
    if (va < g_image_lo) g_image_lo = va;
    if (s->hi > g_image_hi) g_image_hi = s->hi;
    if (executable) {
        if (va < g_exec_lo) g_exec_lo = va;
        if (s->hi > g_exec_hi) g_exec_hi = s->hi;
    }
}

/* A store into an executable section. The host's code span (g_xbox_code_lo/hi) also covers .rdata and .data,
 * so it is not used here (it flagged a quarter of all frames self_modifying). */
static inline bool is_code_store(uint32_t addr, uint32_t size)
{
    uint64_t end = (uint64_t)addr + size;
    if (T2_LIKELY(end <= g_exec_lo || addr >= g_exec_hi)) return false;
    for (uint32_t i = 0; i < g_nsections; i++)
        if (g_sections[i].exec && addr < g_sections[i].hi && end > g_sections[i].lo) return true;
    return false;
}

static inline uintptr_t host_ptr(uint32_t addr)
{
    return (uintptr_t)addr + (uintptr_t)g_xbox_mem_offset;
}

/* ---- 64-bit hashing (sampling keys only) ---- */
static inline uint64_t mix64(uint64_t x)
{
    x ^= x >> 30;
    x *= 0xBF58476D1CE4E5B9ull;
    x ^= x >> 27;
    x *= 0x94D049BB133111EBull;
    x ^= x >> 31;
    return x;
}

static uint64_t hash_bytes(const void *data, size_t len, uint64_t seed)
{
    const uint8_t *p = data;
    uint64_t h = mix64(seed ^ len);
    while (len >= 8) {
        uint64_t w;
        memcpy(&w, p, 8);
        h = mix64(h ^ w) + 0x9E3779B97F4A7C15ull;
        p += 8;
        len -= 8;
    }
    uint64_t tail = 0;
    memcpy(&tail, p, len);
    return mix64(h ^ tail);
}

/* ---- word map: 4-byte words with a byte mask ---- */
typedef struct {
    uint32_t word; /* address & ~3 */
    uint32_t val;  /* little-endian bytes */
    uint32_t mask; /* bit i: byte word+i present */
} t2_went;

typedef struct {
    t2_went *ent;
    uint32_t n, cap;
    uint32_t *idx; /* entry index + 1, 0 = empty */
    uint32_t idx_cap;
    uint32_t bytes;
    bool overflow;
} t2_wmap;

static inline uint32_t wmap_slot(uint32_t word, uint32_t idx_cap)
{
    return (uint32_t)(((uint64_t)(word >> 2) * 0x9E3779B97F4A7C15ull) >> 40) & (idx_cap - 1u);
}

static void wmap_rehash(t2_wmap *m, uint32_t new_cap)
{
    free(m->idx);
    m->idx = calloc(new_cap, sizeof *m->idx);
    if (m->idx == NULL) abort();
    m->idx_cap = new_cap;
    for (uint32_t i = 0; i < m->n; i++) {
        uint32_t s = wmap_slot(m->ent[i].word, new_cap);
        while (m->idx[s] != 0u) s = (s + 1u) & (new_cap - 1u);
        m->idx[s] = i + 1u;
    }
}

static inline t2_went *wmap_find(const t2_wmap *m, uint32_t word)
{
    if (m->n == 0u) return NULL;
    uint32_t s = wmap_slot(word, m->idx_cap);
    for (;;) {
        uint32_t e = m->idx[s];
        if (e == 0u) return NULL;
        if (m->ent[e - 1u].word == word) return &m->ent[e - 1u];
        s = (s + 1u) & (m->idx_cap - 1u);
    }
}

static t2_went *wmap_get(t2_wmap *m, uint32_t word)
{
    if (m->idx_cap == 0u) wmap_rehash(m, 64u);
    uint32_t s = wmap_slot(word, m->idx_cap);
    for (;;) {
        uint32_t e = m->idx[s];
        if (e == 0u) break;
        if (m->ent[e - 1u].word == word) return &m->ent[e - 1u];
        s = (s + 1u) & (m->idx_cap - 1u);
    }
    if (m->n == m->cap) {
        uint32_t nc = m->cap ? m->cap * 2u : 32u;
        t2_went *ne = realloc(m->ent, (size_t)nc * sizeof *ne);
        if (ne == NULL) abort();
        m->ent = ne;
        m->cap = nc;
    }
    t2_went *slot = &m->ent[m->n];
    slot->word = word;
    slot->val = 0u;
    slot->mask = 0u;
    m->n++;
    m->idx[s] = m->n;
    if (m->n * 2u > m->idx_cap) wmap_rehash(m, m->idx_cap * 2u);
    return slot;
}

static void wmap_clear(t2_wmap *m)
{
    if (m->n != 0u) {
        if (m->idx_cap <= 1024u || m->n * 8u > m->idx_cap) {
            memset(m->idx, 0, (size_t)m->idx_cap * sizeof *m->idx);
        } else {
            for (uint32_t i = 0; i < m->n; i++) {
                uint32_t s = wmap_slot(m->ent[i].word, m->idx_cap);
                while (m->idx[s] != i + 1u) s = (s + 1u) & (m->idx_cap - 1u);
                m->idx[s] = 0u;
            }
        }
    }
    m->n = 0u;
    m->bytes = 0u;
    m->overflow = false;
}

static inline uint32_t popcount4(uint32_t mask)
{
    return (uint32_t)__builtin_popcount(mask & 0xFu);
}

/* Put `nbytes` bytes of `bytes` at word-relative offset into an entry; returns the newly set bits. */
static inline void went_put(t2_went *e, uint32_t mask, uint32_t val_bits)
{
    uint32_t bytemask = 0u;
    for (uint32_t i = 0; i < 4u; i++)
        if (mask & (1u << i)) bytemask |= 0xFFu << (8u * i);
    e->val = (e->val & ~bytemask) | (val_bits & bytemask);
    e->mask |= mask;
}

/* ---- frames ---- */
/* BARRIER: the cooperative safe point (t2cap_safepoint). Frames under it are roots, its writes never
 * reach the frames below it; if it wrote anything they are flagged thread_switch instead. */
enum { T2_KIND_LIFTED = 0, T2_KIND_CALL = 1, T2_KIND_BARRIER = 2 };

/* Implicit thread state, 8 little-endian words: seh_ebp, df, fp_cmp, fp_cc, flag_bridge_eflags,
 * flag_bridge_mask, fs_base, reserved 0 (tools/t2corpus/spool.py IMPLICIT_NAMES). */
typedef struct { uint32_t w[8]; } t2_implicit;

typedef struct {
    uint32_t va;
    uint32_t nargs;
    uint32_t args[T2_MAX_ARGS];
    rust_ext_registers entry, exit;
    t2_implicit exit_imp;
    uint32_t w_off, w_n; /* raw subtree W entries in the frame's callee arena */
    t2_input_annotation *input;
} t2_callee;

typedef struct {
    uint32_t va, kind, caller_va, ret_addr, flags, entry_esp, nargs;
    bool entered, exited, have_exit, offered;
    bool skip, quiet, quiet_wrote; /* T3803: budget-full VA; no recording ancestor either; a quiet frame wrote */
    bool off_target; /* T3833: skipped because an ALLOW list is set and the VA is not on it */
    uintptr_t sp_open, sp_body;
    uint64_t switch_epoch;
    uint32_t args[T2_MAX_ARGS];
    rust_ext_registers entry, exit;
    t2_implicit entry_imp, exit_imp;
    t2_wmap R, W;
    t2_callee *callees;
    uint32_t ncallees, callee_cap;
    t2_went *arena; /* callee subtree W entries, raw */
    uint32_t arena_n, arena_cap;
    uint64_t callee_hash;
    t2_input_annotation *input;
    bool input_active, input_valid;
    int fp_top_entry;
} t2_frame;

typedef struct {
    t2_frame **frames;
    uint32_t depth, cap;
    uint64_t switch_epoch;
    bool disabled;
    bool barrier_pending;       /* inside t2cap_safepoint, no barrier frame pushed yet */
    uintptr_t barrier_sp;       /* native position of that safe point */
    uint8_t *buf; /* record scratch */
    size_t buf_len, buf_cap;
    uint32_t ok_pages[4];
    uint32_t ok_next;
} t2_thread;

static __thread t2_thread *tls_thread;

/* ---- global state ---- */
static atomic_int g_state; /* 0 unknown, 1 on, 2 off */
static pthread_mutex_t g_init_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_mutex_t g_spool_lock = PTHREAD_MUTEX_INITIALIZER;
static FILE *g_spool;
static char g_spool_path[4096];
static unsigned g_cap_shape = 8u, g_cap_func = 256u, g_cap_hard = 256u, g_offer_every = 1000u, g_offer_max = 64u;
static bool g_all_calls;

/* ---- T3745 sink state, all under g_spool_lock ---- */
#define T2_DEFAULT_MAX_BYTES (4ull << 30)
#define T2_DEFAULT_SEGMENT_BYTES (256ull << 20)
#define T2_DEFAULT_MIN_FREE_GB 60ull
#define T2_FREE_CHECK_EVERY (16ull << 20)
static gzFile g_gz;                    /* the open segment (segment mode) */
static unsigned long long g_segment_bytes = T2_DEFAULT_SEGMENT_BYTES; /* 0: plain spool file */
static unsigned long long g_max_bytes = T2_DEFAULT_MAX_BYTES;
static uint32_t g_set_cap_bytes = T2_SET_CAP_DEFAULT, g_max_callees = T2_CALLEES_DEFAULT; /* T3836 */
static bool g_allow_only; /* T3836: TSFP_T2_CAPTURE_ALLOW_ONLY=1, only budget-seeded VAs are recorded */
static unsigned long long g_min_free = T2_DEFAULT_MIN_FREE_GB << 30;
static unsigned g_segment_no;
static unsigned long long g_seg_raw;      /* uncompressed bytes in the open segment */
static unsigned long long g_closed_disk;  /* on-disk bytes of closed segments, or of the plain spool */
static unsigned long long g_next_free_check;
static char g_seg_path[4200];
static char g_spool_dir[4096];
static const char *g_stop_reason;         /* NULL while capturing */
static bool g_finished;                    /* t2cap_finish ran: final segment closed, stats written, recording stopped */

static atomic_ullong st_frames, st_kept, st_dup, st_sampled_out, st_call_frames, st_host_callees,
    st_seh_pops, st_spool_bytes, st_unmatched_exit, st_depth_overflow, st_barriers, st_barrier_writes;
static atomic_ullong st_flag_frames[T2_NFLAGS], st_flag_kept[T2_NFLAGS];
static atomic_ullong st_copies, st_copy_bytes, st_fills;
static atomic_ullong st_host_writes, st_host_writes_attributed; /* T3767 */
static atomic_ullong st_budget_skipped, st_budget_quiet, st_budget_full; /* T3803 */
static atomic_ullong st_over_record_ceiling; /* T3836 */
static atomic_ullong st_target_skipped; /* T3833 */

/* ---- per-function sampler ---- */
typedef struct {
    uint64_t *keys; /* 0 = empty (key 0 is remapped to 1) */
    uint32_t *vals;
    uint32_t n, cap;
} t2_u64map;

static uint32_t *u64map_find(t2_u64map *m, uint64_t key)
{
    if (m->cap == 0u) return NULL;
    key = key ? key : 1u;
    uint32_t s = (uint32_t)mix64(key) & (m->cap - 1u);
    while (m->keys[s] != 0u) {
        if (m->keys[s] == key) return &m->vals[s];
        s = (s + 1u) & (m->cap - 1u);
    }
    return NULL;
}

static uint32_t *u64map_add(t2_u64map *m, uint64_t key)
{
    key = key ? key : 1u;
    if ((m->n + 1u) * 2u > m->cap) {
        uint32_t nc = m->cap ? m->cap * 2u : 16u;
        uint64_t *nk = calloc(nc, sizeof *nk);
        uint32_t *nv = calloc(nc, sizeof *nv);
        if (nk == NULL || nv == NULL) abort();
        for (uint32_t i = 0; i < m->cap; i++) {
            if (m->keys[i] == 0u) continue;
            uint32_t s = (uint32_t)mix64(m->keys[i]) & (nc - 1u);
            while (nk[s] != 0u) s = (s + 1u) & (nc - 1u);
            nk[s] = m->keys[i];
            nv[s] = m->vals[i];
        }
        free(m->keys);
        free(m->vals);
        m->keys = nk;
        m->vals = nv;
        m->cap = nc;
    }
    uint32_t s = (uint32_t)mix64(key) & (m->cap - 1u);
    while (m->keys[s] != 0u) {
        if (m->keys[s] == key) return &m->vals[s];
        s = (s + 1u) & (m->cap - 1u);
    }
    m->keys[s] = key;
    m->vals[s] = 0u;
    m->n++;
    return &m->vals[s];
}

typedef struct {
    atomic_flag lock;
    atomic_ullong host_calls; /* calls of this VA that ran no CAP_ENTER (kernel, HLE, C or Rust replacement) */
    uint32_t va;
    uint64_t calls, offers;
    uint32_t kept, kept_offered;
    t2_u64map shapes;  /* shape -> digests kept */
    t2_u64map digests; /* kept digests */
} t2_func;

#define T2_FUNC_SLOTS 65536u
static _Atomic(t2_func *) g_funcs[T2_FUNC_SLOTS];
static pthread_mutex_t g_funcs_lock = PTHREAD_MUTEX_INITIALIZER;
static atomic_uint g_funcs_used;

static t2_func *func_lookup(uint32_t va)
{
    uint32_t s = (uint32_t)(mix64(va) & (T2_FUNC_SLOTS - 1u));
    for (uint32_t probe = 0; probe < T2_FUNC_SLOTS; probe++) {
        t2_func *f = atomic_load_explicit(&g_funcs[s], memory_order_acquire);
        if (f == NULL) {
            pthread_mutex_lock(&g_funcs_lock);
            f = atomic_load_explicit(&g_funcs[s], memory_order_acquire);
            if (f == NULL) {
                f = calloc(1, sizeof *f);
                if (f == NULL) abort();
                atomic_flag_clear(&f->lock);
                f->va = va;
                atomic_store_explicit(&g_funcs[s], f, memory_order_release);
                atomic_fetch_add(&g_funcs_used, 1u);
                pthread_mutex_unlock(&g_funcs_lock);
                return f;
            }
            pthread_mutex_unlock(&g_funcs_lock);
        }
        if (f->va == va) return f;
        s = (s + 1u) & (T2_FUNC_SLOTS - 1u);
    }
    return NULL;
}

/* ---- T3803 per-VA capture budget ---- */
#define T2_BUDGET_SLOTS (1u << 17)
#define T2_BUDGET_SHAPES 3u /* tools/t2capture/path_count.py CAP: an unknown path count needs 3 shapes */
typedef struct {
    _Atomic uint32_t va; /* 0 = empty; claimed once, never freed */
    uint32_t seed_kept, seed_shapes, need; /* corpus entries and shapes at start; need 0 = unknown */
    atomic_bool full;
    bool seeded; /* T3836: named by the seed file (set before recording starts) */
} t2_budget;
static t2_budget g_budget[T2_BUDGET_SLOTS]; /* static: no allocation at run time */
static unsigned g_budget_k = 32u;
static unsigned g_budget_seeded;

static t2_budget *budget_find(uint32_t va, bool create)
{
    if (va == 0u) return NULL;
    uint32_t s = (uint32_t)(mix64(va) & (T2_BUDGET_SLOTS - 1u));
    for (uint32_t probe = 0; probe < T2_BUDGET_SLOTS; probe++) {
        uint32_t cur = atomic_load_explicit(&g_budget[s].va, memory_order_acquire);
        if (cur == 0u && create && atomic_compare_exchange_strong(&g_budget[s].va, &cur, va)) return &g_budget[s];
        if (cur == va) return &g_budget[s];
        if (cur == 0u) return NULL;
        s = (s + 1u) & (T2_BUDGET_SLOTS - 1u);
    }
    return NULL;
}

/* The per-call check: one probe sequence, no lock, no allocation. */
static inline bool budget_full(uint32_t va)
{
    const t2_budget *b = g_budget_k != 0u || g_allow_only ? budget_find(va, false) : NULL;
    if (g_allow_only && (b == NULL || !b->seeded)) return true; /* T3836: not in the seed, skipped */
    return g_budget_k != 0u && b != NULL && atomic_load_explicit(&b->full, memory_order_relaxed);
}

static void budget_check(t2_budget *b, uint64_t kept, uint32_t shapes)
{
    uint32_t need = b->need != 0u ? b->need : T2_BUDGET_SHAPES;
    if (shapes < b->seed_shapes) shapes = b->seed_shapes; /* corpus and run shapes may overlap: the larger */
    if (b->seed_kept + kept >= g_budget_k && shapes >= need && !atomic_exchange(&b->full, true))
        atomic_fetch_add(&st_budget_full, 1u);
}

/* Seed file (tools/t2capture/budget_seed.py): `<va hex> <corpus entries> <corpus shapes> <min(3, paths)>` lines,
 * `#` comments. Read once in capture_init, before recording is published on. A missing file is loud, not fatal. */
static void budget_seed(const char *path)
{
    FILE *in = fopen(path, "r");
    if (in == NULL) {
        fprintf(stderr, "t2_capture: budget seed %s unreadable: %s (no seed, every VA starts at 0)\n", path,
                strerror(errno));
        return;
    }
    char line[256];
    while (fgets(line, sizeof line, in) != NULL) {
        if (line[0] == '#' || line[0] == '\n') continue;
        char *end = NULL;
        unsigned long va = strtoul(line, &end, 16);
        unsigned long kept = strtoul(end, &end, 10), shapes = strtoul(end, &end, 10), need = strtoul(end, &end, 10);
        t2_budget *b = budget_find((uint32_t)va, true);
        if (b == NULL) continue;
        b->seed_kept = (uint32_t)kept;
        b->seed_shapes = (uint32_t)shapes;
        b->need = (uint32_t)need;
        b->seeded = true;
        g_budget_seeded++;
        if (g_budget_k != 0u) budget_check(b, 0u, 0u);
    }
    fclose(in);
}

/* ---- T3833 ALLOW list (live targeted capture) ---- */
#define T2_TARGET_SLOTS (1u << 16)
static uint32_t g_target[T2_TARGET_SLOTS]; /* open addressing, 0 = empty; written once before guest threads */
static unsigned g_target_count;
static bool g_targets_on;
static pthread_once_t g_targets_once = PTHREAD_ONCE_INIT;

static void targets_load(void)
{
    const char *path = getenv("TSFP_T2_CAPTURE_TARGETS");
    if (path == NULL || *path == '\0') return;
    FILE *in = fopen(path, "r");
    g_targets_on = true; /* named: a missing or empty file records nothing rather than everything */
    if (in == NULL) {
        fprintf(stderr, "t2_capture: targets %s unreadable: %s (ALLOW list empty, nothing is recorded)\n", path,
                strerror(errno));
        return;
    }
    char line[256];
    while (fgets(line, sizeof line, in) != NULL) {
        if (line[0] == '#' || line[0] == '\n') continue;
        uint32_t va = (uint32_t)strtoul(line, NULL, 16);
        if (va == 0u || g_target_count + 1u >= T2_TARGET_SLOTS / 2u) continue;
        uint32_t s = (uint32_t)(mix64(va) & (T2_TARGET_SLOTS - 1u));
        while (g_target[s] != 0u && g_target[s] != va) s = (s + 1u) & (T2_TARGET_SLOTS - 1u);
        if (g_target[s] == 0u) {
            g_target[s] = va;
            g_target_count++;
        }
    }
    fclose(in);
    fprintf(stderr, "t2_capture: ALLOW list %s: %u target VAs (only these record)\n", path, g_target_count);
}

bool t2cap_targets_active(void)
{
    pthread_once(&g_targets_once, targets_load);
    return g_targets_on;
}

bool t2cap_target_listed(uint32_t va)
{
    pthread_once(&g_targets_once, targets_load);
    if (va == 0u) return false;
    for (uint32_t s = (uint32_t)(mix64(va) & (T2_TARGET_SLOTS - 1u));; s = (s + 1u) & (T2_TARGET_SLOTS - 1u)) {
        if (g_target[s] == va) return true;
        if (g_target[s] == 0u) return false;
    }
}

/* The per-call check: no list, or the VA is on it. g_targets_on is fixed before recording is published on. */
static inline bool target_allowed(uint32_t va)
{
    return !g_targets_on || t2cap_target_listed(va);
}

/* The sampling decision of design section 1. */
static bool sampler_offer(uint32_t va, uint64_t shape, uint64_t digest)
{
    /* Layer boundary captures must preserve repeated calls and their order. */
    if (g_all_calls) return true;
    t2_func *f = func_lookup(va);
    if (f == NULL) return false;
    while (atomic_flag_test_and_set_explicit(&f->lock, memory_order_acquire)) {
    }
    bool keep = false;
    f->calls++;
    if (u64map_find(&f->digests, digest) != NULL) {
        atomic_fetch_add(&st_dup, 1u);
    } else {
        uint32_t *nd = u64map_find(&f->shapes, shape);
        if (nd == NULL && f->kept < g_cap_hard) {
            *u64map_add(&f->shapes, shape) = 1u;
            keep = true;
        } else if (nd != NULL && *nd < g_cap_shape && f->kept < g_cap_func) {
            (*nd)++;
            keep = true;
        } else {
            f->offers++;
            if (g_offer_every != 0u && f->offers % g_offer_every == 0u && f->kept_offered < g_offer_max) {
                f->kept_offered++;
                keep = true;
            }
        }
        if (keep) {
            *u64map_add(&f->digests, digest) = 1u;
            f->kept++;
            t2_budget *b = g_budget_k != 0u ? budget_find(va, true) : NULL;
            if (b != NULL) budget_check(b, f->kept, f->shapes.n);
        } else {
            atomic_fetch_add(&st_sampled_out, 1u);
        }
    }
    atomic_flag_clear_explicit(&f->lock, memory_order_release);
    return keep;
}

/* ---- spool ---- */
static void json_str(FILE *out, const char *s)
{
    fputc('"', out);
    for (; *s; s++) {
        unsigned char c = (unsigned char)*s;
        if (c == '"' || c == '\\') fprintf(out, "\\%c", c);
        else if (c < 0x20u) fprintf(out, "\\u%04x", c);
        else fputc(c, out);
    }
    fputc('"', out);
}

static const char *env_or(const char *name, const char *fallback)
{
    const char *v = getenv(name);
    return v != NULL ? v : fallback;
}

static void header_json(char **json_out, size_t *len_out, int segment)
{
    char created[64];
    time_t now = time(NULL);
    struct tm tm;
    gmtime_r(&now, &tm);
    strftime(created, sizeof created, "%Y-%m-%dT%H:%M:%SZ", &tm);
    char *json = NULL;
    size_t json_len = 0;
    FILE *mem = open_memstream(&json, &json_len);
    if (mem == NULL) abort();
    /* Keys sorted like tools/t2corpus/spool.py write_spool. */
    fprintf(mem, "{\"all_calls\": %s, \"build_input_hash\": ", g_all_calls ? "true" : "false");
    json_str(mem, env_or("TSFP_T2_CAPTURE_BUILD_INPUT_HASH", ""));
    fputs(", \"created\": ", mem);
    json_str(mem, created);
    fputs(", \"lifted_tree_hash\": ", mem);
    json_str(mem, env_or("TSFP_T2_CAPTURE_LIFTED_TREE_HASH", TSFP_T2_LIFTED_TREE_HASH));
    fprintf(mem, ", \"record_format\": %d", T2_RECORD_FORMAT);
    fputs(", \"recording_sha256\": ", mem);
    json_str(mem, env_or("TSFP_T2_CAPTURE_RECORDING_SHA256", ""));
    if (segment > 0) fprintf(mem, ", \"segment\": %d", segment);
    /* T3836: non-default caps only (older headers unchanged); tools.t2corpus reads defaults when absent */
    if (g_set_cap_bytes != T2_SET_CAP_DEFAULT || g_max_callees != T2_CALLEES_DEFAULT || g_allow_only)
        fprintf(mem, ", \"size_caps\": {\"allow_only\": %s, \"max_callees\": %u, \"max_set_bytes\": %u}",
                g_allow_only ? "true" : "false", g_max_callees, g_set_cap_bytes);
    fputs(", \"tool_version\": \"t2_capture.c/4 (T3794)\"}", mem);
    fclose(mem);
    *json_out = json;
    *len_out = json_len;
}

static const unsigned char g_magic[6] = {'T', '2', 'C', 'A', 'P', 1};

/* ---- T3745 sink: plain spool or gzip segments, byte cap, free floor (caller holds g_spool_lock) ---- */
static unsigned long long disk_bytes_now(void)
{
    if (g_gz != NULL) return g_closed_disk + (unsigned long long)gzoffset(g_gz);
    if (g_spool != NULL) return g_closed_disk;
    return g_closed_disk;
}

static unsigned long long free_bytes(const char *dir)
{
    struct statvfs vfs;
    if (statvfs(dir, &vfs) != 0) return 0ull; /* unknown counts as full: fail closed */
    return (unsigned long long)vfs.f_bavail * (unsigned long long)vfs.f_frsize;
}

static void write_stats(void);

static void close_segment(void)
{
    if (g_gz == NULL) return;
    gzclose(g_gz);
    g_gz = NULL;
    if (g_seg_raw == 0u) { /* header only: no records, nothing to keep */
        unlink(g_seg_path);
        g_segment_no--;
        return;
    }
    char final_path[4200];
    snprintf(final_path, sizeof final_path, "%s.seg-%06u.gz", g_spool_path, g_segment_no);
    struct stat st;
    if (stat(g_seg_path, &st) == 0) g_closed_disk += (unsigned long long)st.st_size;
    rename(g_seg_path, final_path);
}

static bool open_segment(void)
{
    g_segment_no++;
    snprintf(g_seg_path, sizeof g_seg_path, "%s.seg-%06u.gz.part", g_spool_path, g_segment_no);
    g_gz = gzopen(g_seg_path, "wb1");
    if (g_gz == NULL) return false;
    char *json = NULL;
    size_t json_len = 0;
    header_json(&json, &json_len, (int)g_segment_no);
    uint32_t len = (uint32_t)json_len;
    gzwrite(g_gz, g_magic, sizeof g_magic);
    gzwrite(g_gz, &len, 4);
    gzwrite(g_gz, json, (unsigned)json_len);
    free(json);
    g_seg_raw = 0;
    return true;
}

/* Stop recording for good: close the open segment, record why, keep the game running. */
static void capture_stop(const char *reason)
{
    if (g_stop_reason != NULL) return;
    g_stop_reason = reason;
    close_segment();
    if (g_spool != NULL) {
        fflush(g_spool);
        fclose(g_spool);
        g_spool = NULL;
    }
    atomic_store(&g_state, 2);
    fprintf(stderr,
            "t2_capture: STOPPED (%s) after %llu bytes on disk, free %llu MB, cap %llu MB, floor %llu MB; "
            "the spool is truncated, the game keeps running with capture off (docs/t3745-disk-safety.md)\n",
            reason, disk_bytes_now(), free_bytes(g_spool_dir) >> 20, g_max_bytes >> 20, g_min_free >> 20);
    write_stats();
}

/* One encoded record (u32 length + body). False when capture is (now) stopped. */
static bool sink_record(const void *data, size_t len)
{
    if (g_stop_reason != NULL || g_finished || g_spool_path[0] == '\0') return false;
    if (g_segment_bytes != 0u && g_gz == NULL && !open_segment()) { /* the next segment, opened lazily */
        capture_stop("write_error");
        return false;
    }
    unsigned long long now = disk_bytes_now();
    if (now + len > g_max_bytes) {
        capture_stop("byte_cap");
        return false;
    }
    if (now >= g_next_free_check) {
        g_next_free_check = now + T2_FREE_CHECK_EVERY;
        if (free_bytes(g_spool_dir) < g_min_free) {
            capture_stop("free_floor");
            return false;
        }
    }
    if (g_segment_bytes == 0u) {
        if (fwrite(data, 1, len, g_spool) != len || fflush(g_spool) != 0) {
            capture_stop("write_error");
            return false;
        }
        g_closed_disk += len;
        return true;
    }
    if (gzwrite(g_gz, data, (unsigned)len) != (int)len) {
        capture_stop("write_error");
        return false;
    }
    g_seg_raw += len;
    if (g_seg_raw >= g_segment_bytes) close_segment();
    return true;
}

static void write_stats(void)
{
    if (g_spool_path[0] == '\0') return;
    char path[4200], temp[4300];
    snprintf(path, sizeof path, "%s.stats.json", g_spool_path);
    snprintf(temp, sizeof temp, "%s.tmp", path); /* written whole, then renamed: never a half file */
    FILE *out = fopen(temp, "w");
    if (out == NULL) return;
    static const char *const names[T2_NFLAGS] = {"mmio", "thread_switch", "size", "seh_unwind", "fpu_overflow", "self_modifying", "old_format", "host_write", "input_unverified"};
    fprintf(out, "{\n \"frames\": %llu,\n \"kept\": %llu,\n \"duplicate\": %llu,\n \"sampled_out\": %llu,\n",
            (unsigned long long)st_frames, (unsigned long long)st_kept, (unsigned long long)st_dup,
            (unsigned long long)st_sampled_out);
    fprintf(out, " \"call_site_frames\": %llu,\n \"host_callees\": %llu,\n \"seh_pops\": %llu,\n \"unmatched_exits\": %llu,\n",
            (unsigned long long)st_call_frames, (unsigned long long)st_host_callees, (unsigned long long)st_seh_pops,
            (unsigned long long)st_unmatched_exit);
    fprintf(out, " \"depth_overflow\": %llu,\n \"functions\": %u,\n \"spool_bytes\": %llu,\n",
            (unsigned long long)st_depth_overflow, (unsigned)g_funcs_used, (unsigned long long)st_spool_bytes);
    fprintf(out, " \"copies\": %llu,\n \"copy_bytes\": %llu,\n \"fills\": %llu,\n", (unsigned long long)st_copies,
            (unsigned long long)st_copy_bytes, (unsigned long long)st_fills);
    fprintf(out, " \"host_writes\": %llu,\n \"host_writes_attributed\": %llu,\n",
            (unsigned long long)st_host_writes, (unsigned long long)st_host_writes_attributed);
    /* T3803: frames of budget-full VAs are not offered (their entries are capped by design); quiet ones not tracked */
    fprintf(out, " \"max_set_bytes\": %u,\n \"max_callees\": %u,\n \"allow_only\": %s,\n", g_set_cap_bytes,
            g_max_callees, g_allow_only ? "true" : "false");
    fprintf(out, " \"over_record_ceiling\": %llu,\n", (unsigned long long)st_over_record_ceiling);
    fprintf(out, " \"budget_k\": %u,\n \"budget_seeded\": %u,\n \"budget_full\": %llu,\n", g_budget_k,
            g_budget_seeded, (unsigned long long)st_budget_full);
    fprintf(out, " \"targets\": %u,\n \"targets_on\": %s,\n \"target_skipped\": %llu,\n", g_target_count,
            g_targets_on ? "true" : "false", (unsigned long long)st_target_skipped); /* T3833 */
    fprintf(out, " \"budget_skipped\": %llu,\n \"budget_quiet\": %llu,\n", (unsigned long long)st_budget_skipped,
            (unsigned long long)st_budget_quiet);
    /* T3745: unlocked read of the sink counters: a stats snapshot, not a ledger */
    fprintf(out, " \"disk_bytes\": %llu,\n \"max_bytes\": %llu,\n \"min_free_bytes\": %llu,\n",
            disk_bytes_now(), g_max_bytes, g_min_free);
    fprintf(out, " \"segment_bytes\": %llu,\n \"segments\": %u,\n \"truncated\": %s,\n \"stop_reason\": ",
            g_segment_bytes, g_segment_no, g_stop_reason != NULL ? "true" : "false");
    if (g_stop_reason != NULL) fprintf(out, "\"%s\",\n", g_stop_reason);
    else fputs("null,\n", out);
    for (int pass = 0; pass < 2; pass++) {
        fprintf(out, " \"%s\": {", pass == 0 ? "flag_frames" : "flag_kept");
        for (int i = 0; i < T2_NFLAGS; i++)
            fprintf(out, "%s\"%s\": %llu", i ? ", " : "", names[i],
                    (unsigned long long)(pass == 0 ? st_flag_frames[i] : st_flag_kept[i]));
        fprintf(out, "}%s", pass == 0 ? ",\n" : "");
    }
    /* the 64 most called VAs that ran no lifted body: their memory accesses are outside the capture */
    enum { TOP = 64 };
    uint32_t top_va[TOP] = {0};
    unsigned long long top_n[TOP] = {0};
    for (uint32_t i = 0; i < T2_FUNC_SLOTS; i++) {
        t2_func *fn = atomic_load_explicit(&g_funcs[i], memory_order_acquire);
        if (fn == NULL) continue;
        unsigned long long n = atomic_load_explicit(&fn->host_calls, memory_order_relaxed);
        if (n == 0u || n <= top_n[TOP - 1]) continue;
        int j = TOP - 1;
        while (j > 0 && top_n[j - 1] < n) {
            top_n[j] = top_n[j - 1];
            top_va[j] = top_va[j - 1];
            j--;
        }
        top_n[j] = n;
        top_va[j] = fn->va;
    }
    fprintf(out, ",\n \"top_host_callees\": {");
    for (int i = 0; i < TOP && top_n[i] != 0u; i++)
        fprintf(out, "%s\"0x%08x\": %llu", i ? ", " : "", top_va[i], top_n[i]);
    fprintf(out, "}\n}\n");
    if (fclose(out) == 0) rename(temp, path);
}

void t2cap_flush(void)
{
    pthread_mutex_lock(&g_spool_lock);
    if (g_spool != NULL) fflush(g_spool);
    if (g_gz != NULL) gzflush(g_gz, Z_SYNC_FLUSH);
    pthread_mutex_unlock(&g_spool_lock);
    write_stats();
}

static void at_exit_flush(void)
{
    pthread_mutex_lock(&g_spool_lock);
    close_segment(); /* the last segment is complete: renamed from .part */
    g_finished = true; /* guest threads that still run must not open a new segment behind the _exit */
    pthread_mutex_unlock(&g_spool_lock);
    t2cap_flush();
    if (g_spool_path[0] != '\0')
        fprintf(stderr, "t2_capture: %llu frames, %llu kept, %llu duplicate, %llu sampled out, %u functions, spool %s\n",
                (unsigned long long)st_frames, (unsigned long long)st_kept, (unsigned long long)st_dup,
                (unsigned long long)st_sampled_out, (unsigned)g_funcs_used, g_spool_path);
}

/* The host leaves through _exit(1) while guest threads still run (main.c), which skips atexit: without this the
 * last segment stayed a truncated .gz.part and no stats sidecar was written (bulk run 2026-10-25). */
void t2cap_finish(void)
{
    if (g_spool_path[0] != '\0') at_exit_flush();
}

static struct sigaction g_prev_actions[3];
static const int g_fatal_signals[3] = {SIGTERM, SIGINT, SIGABRT};

/* Records are fflushed one by one, so a fatal signal loses at most the record being written
 * (the reader counts it as truncated). The handler only writes the stats sidecar and hands the
 * signal on to the handler that was installed before. */
static void on_fatal_signal(int sig)
{
    write_stats();
    for (int i = 0; i < 3; i++) {
        if (g_fatal_signals[i] != sig) continue;
        sigaction(sig, &g_prev_actions[i], NULL);
    }
    raise(sig);
}

static unsigned env_unsigned(const char *name, unsigned fallback)
{
    const char *v = getenv(name);
    if (v == NULL || *v == '\0') return fallback;
    char *end = NULL;
    unsigned long n = strtoul(v, &end, 10);
    return (end != NULL && *end == '\0') ? (unsigned)n : fallback;
}

static unsigned long long env_ull(const char *name, unsigned long long fallback)
{
    const char *v = getenv(name);
    if (v == NULL || *v == '\0') return fallback;
    char *end = NULL;
    unsigned long long n = strtoull(v, &end, 10);
    return (end != NULL && *end == '\0') ? n : fallback;
}

/* T3836: a cap outside [1, ceiling] is clamped, loudly. */
static uint32_t env_cap(const char *name, uint32_t fallback, uint32_t ceiling)
{
    unsigned long long n = env_ull(name, fallback);
    if (n == 0u || n > ceiling) {
        uint32_t used = n == 0u ? fallback : ceiling;
        fprintf(stderr, "t2_capture: %s=%llu outside [1, %u], using %u (T3836)\n", name, n, ceiling, used);
        return used;
    }
    return (uint32_t)n;
}

static void size_caps_from_env(void)
{
    g_set_cap_bytes = env_cap("TSFP_T2_CAPTURE_MAX_SET_BYTES", T2_SET_CAP_DEFAULT, T2_SET_CAP_CEILING);
    g_max_callees = env_cap("TSFP_T2_CAPTURE_MAX_CALLEES", T2_CALLEES_DEFAULT, T2_CALLEES_CEILING);
    g_allow_only = env_unsigned("TSFP_T2_CAPTURE_ALLOW_ONLY", 0u) != 0u;
}

/* T3745: refuse a spool on the root filesystem when a separate data disk exists. 0 ok, else refuse. */
static int root_fs_refused(const char *dir, char *why, size_t why_len)
{
    const char *root_ref = env_or("TSFP_T2_CAPTURE_ROOT_REF", "/");
    const char *anchor = env_or("TSFP_DATADISK_ANCHOR", "/workspace");
    struct stat sd, sr, sa;
    if (stat(dir, &sd) != 0 || stat(root_ref, &sr) != 0) {
        snprintf(why, why_len, "cannot stat %s or %s", dir, root_ref);
        return 1;
    }
    if (sd.st_dev != sr.st_dev) return 0;
    if (stat(anchor, &sa) != 0) {
        snprintf(why, why_len, "%s is on the root filesystem and the data disk anchor %s is missing", dir, anchor);
        return 1;
    }
    if (sa.st_dev == sr.st_dev) return 0; /* one-disk host: no separate data disk, the free floor guards */
    snprintf(why, why_len, "%s is on the root filesystem, not on the data disk of %s", dir, anchor);
    return 1;
}

static bool capture_init(void)
{
    pthread_mutex_lock(&g_init_lock);
    int state = atomic_load(&g_state);
    if (state == 0) {
        const char *path = getenv("TSFP_T2_CAPTURE");
        state = 2;
        char why[8400] = "";
        if (path != NULL && *path != '\0' && strlen(path) < sizeof g_spool_path) {
            strcpy(g_spool_dir, path);
            char *slash = strrchr(g_spool_dir, '/');
            if (slash == NULL) strcpy(g_spool_dir, ".");
            else if (slash == g_spool_dir) slash[1] = '\0';
            else *slash = '\0';
            g_max_bytes = env_ull("TSFP_T2_CAPTURE_MAX_BYTES", T2_DEFAULT_MAX_BYTES);
            g_min_free = env_ull("TSFP_T2_CAPTURE_MIN_FREE_GB", T2_DEFAULT_MIN_FREE_GB) << 30;
            g_segment_bytes = env_ull("TSFP_T2_CAPTURE_SEGMENT_BYTES", T2_DEFAULT_SEGMENT_BYTES);
            size_caps_from_env(); /* T3836: before the first header is written */
            g_all_calls = env_unsigned("TSFP_T2_CAPTURE_ALL_CALLS", 0u) != 0u;
            if (root_fs_refused(g_spool_dir, why, sizeof why)) {
                fprintf(stderr, "t2_capture: REFUSED %s: %s (capture off, T3745)\n", path, why);
                path = NULL;
            } else if (free_bytes(g_spool_dir) < g_min_free) {
                fprintf(stderr, "t2_capture: REFUSED %s: %llu MB free < floor %llu MB (capture off, T3745)\n", path,
                        free_bytes(g_spool_dir) >> 20, g_min_free >> 20);
                path = NULL;
            }
        }
        if (path != NULL && *path != '\0' && strlen(path) < sizeof g_spool_path) {
            strcpy(g_spool_path, path);
            FILE *out = NULL;
            bool opened;
            if (g_segment_bytes == 0u) {
                out = fopen(path, "wb");
                opened = out != NULL;
            } else {
                opened = open_segment();
            }
            if (!opened) {
                fprintf(stderr, "t2_capture: cannot open %s: %s (capture off)\n", path, strerror(errno));
                g_spool_path[0] = '\0';
            } else {
                struct rlimit no_core = {0, 0};
                setrlimit(RLIMIT_CORE, &no_core); /* T3745: a core of this host would land on the root disk */
                g_cap_shape = env_unsigned("TSFP_T2_CAPTURE_SHAPE_CAP", 8u);
                g_cap_func = env_unsigned("TSFP_T2_CAPTURE_FUNC_CAP", 256u);
                g_cap_hard = env_unsigned("TSFP_T2_CAPTURE_HARD", 256u);
                g_offer_every = env_unsigned("TSFP_T2_CAPTURE_OFFER_EVERY", 1000u);
                g_offer_max = env_unsigned("TSFP_T2_CAPTURE_OFFER_MAX", 64u);
                g_budget_k = g_all_calls ? 0u : env_unsigned("TSFP_T2_CAPTURE_BUDGET", 32u);
                const char *seed = getenv("TSFP_T2_CAPTURE_BUDGET_SEED");
                if (seed != NULL && *seed != '\0') budget_seed(seed);
                (void)t2cap_targets_active(); /* T3833: load the ALLOW list before recording starts */
                if (out != NULL) {
                    char *json = NULL;
                    size_t json_len = 0;
                    header_json(&json, &json_len, 0);
                    uint32_t len = (uint32_t)json_len;
                    fwrite(g_magic, 1, sizeof g_magic, out);
                    fwrite(&len, 4, 1, out);
                    fwrite(json, 1, json_len, out);
                    fflush(out);
                    free(json);
                    g_closed_disk = sizeof g_magic + 4u + json_len;
                    g_spool = out;
                }
                atexit(at_exit_flush);
                for (int i = 0; i < 3; i++) {
                    struct sigaction sa;
                    memset(&sa, 0, sizeof sa);
                    sa.sa_handler = on_fatal_signal;
                    sigemptyset(&sa.sa_mask);
                    sigaction(g_fatal_signals[i], &sa, &g_prev_actions[i]);
                }
                state = 1;
                fprintf(stderr, "t2_capture: recording to %s\n", path);
            }
        }
        atomic_store(&g_state, state);
    }
    pthread_mutex_unlock(&g_init_lock);
    return state == 1;
}

static inline bool capture_on(void)
{
    int state = atomic_load_explicit(&g_state, memory_order_relaxed);
    if (T2_LIKELY(state == 2)) return false;
    if (T2_LIKELY(state == 1)) return true;
    return capture_init();
}

static t2_thread *thread_state(void)
{
    t2_thread *t = tls_thread;
    if (T2_UNLIKELY(t == NULL)) {
        t = calloc(1, sizeof *t);
        if (t == NULL) abort();
        tls_thread = t;
    }
    return t;
}

static inline t2_frame *top_frame(t2_thread *t)
{
    return t->depth ? t->frames[t->depth - 1u] : NULL;
}

/* ---- guest registers ---- */
static void snapshot_implicit(t2_implicit *m)
{
    m->w[0] = g_seh_ebp;
    m->w[1] = (uint32_t)g_df;
    m->w[2] = (uint32_t)g_fp_cmp;
    m->w[3] = g_fp_cc;
    m->w[4] = g_flag_bridge_eflags;
    m->w[5] = g_flag_bridge_mask;
    m->w[6] = g_fs_base;
    m->w[7] = 0u;
}

static void snapshot_regs(rust_ext_registers *r, t2_implicit *m)
{
    snapshot_implicit(m);
    memset(r, 0, sizeof *r);
    r->eax = g_eax;
    r->ecx = g_ecx;
    r->edx = g_edx;
    r->esp = g_esp;
    r->ebx = g_ebx;
    r->ebp = g_ebp;
    r->esi = g_esi;
    r->edi = g_edi;
    r->eflags = 0u; /* the lifted code keeps flags in locals; no flag state crosses a call */
    memcpy(r->fpu.stack, g_fp_stack, sizeof r->fpu.stack);
    r->fpu.top = (uint32_t)g_fp_top & 7u;
    r->fpu.control = g_fp_control_word;
    memcpy(&r->xmm[0], &g_xmm0, 16);
    memcpy(&r->xmm[1], &g_xmm1, 16);
    memcpy(&r->xmm[2], &g_xmm2, 16);
    memcpy(&r->xmm[3], &g_xmm3, 16);
    memcpy(&r->xmm[4], &g_xmm4, 16);
    memcpy(&r->xmm_hi[0], &g_xmm5, 16);
    memcpy(&r->xmm_hi[1], &g_xmm6, 16);
    memcpy(&r->xmm_hi[2], &g_xmm7, 16);
}

/* Reads guest words that the original never read itself (return address, stack argument words).
 * A page is checked once with process_vm_readv (EFAULT instead of SIGSEGV) and then cached. */
static bool page_ok(t2_thread *t, uint32_t page)
{
    for (uint32_t i = 0; i < 4u; i++)
        if (t->ok_pages[i] == page + 1u) return true;
    uint8_t probe;
    struct iovec local = {&probe, 1}, remote = {(void *)host_ptr(page << 12), 1};
    if (process_vm_readv(getpid(), &local, 1, &remote, 1, 0) != 1) return false;
    t->ok_pages[t->ok_next++ & 3u] = page + 1u;
    return true;
}

static uint32_t safe_words(t2_thread *t, uint32_t addr, uint32_t *out, uint32_t n)
{
    uint32_t got = 0;
    for (; got < n; got++) {
        uint32_t a = addr + 4u * got;
        if (a < addr || !page_ok(t, a >> 12) || !page_ok(t, (a + 3u) >> 12)) break;
        memcpy(&out[got], (const void *)host_ptr(a), 4);
    }
    return got;
}

static uint32_t callee_arity(uint32_t va)
{
#ifdef TSFP_HAVE_MEASURED_ARITIES
    if (va >= T2_KERNEL_THUNK_BASE && (va & 3u) == 0u) {
        uint32_t ordinal = (va - T2_KERNEL_THUNK_BASE) >> 2;
        for (size_t i = 0; i < MEASURED_ARITY_COUNT; i++)
            if (MEASURED_ARITIES[i].ordinal == ordinal)
                return MEASURED_ARITIES[i].stack_args < T2_MAX_ARGS ? MEASURED_ARITIES[i].stack_args : T2_MAX_ARGS;
    }
#endif
    (void)va;
    return T2_MAX_ARGS;
}

/* Kernel ordinals that block the calling thread (kernel_ordinals.c): KeDelayExecutionThread 99,
 * KeWaitForMultipleObjects 158, KeWaitForSingleObject 159, NtSignalAndWaitForSingleObjectEx 230,
 * NtWaitForSingleObject 233, NtWaitForSingleObjectEx 234, NtWaitForMultipleObjectsEx 235,
 * NtYieldExecution 238. The host runs guest threads as pthreads and has no scheduler hook, so a
 * blocking wait is where another guest thread can run between two accesses of an open frame. */
static bool is_kernel_wait(uint32_t va)
{
    if (va < T2_KERNEL_THUNK_BASE || (va & 3u) != 0u) return false;
    switch ((va - T2_KERNEL_THUNK_BASE) >> 2) {
    case 99: case 158: case 159: case 230: case 233: case 234: case 235: case 238:
        return true;
    default:
        return false;
    }
}

/* ---- frame push / pop ---- */
/* T3803: nothing below needs this frame's subtree: no parent (a root), a safe point (only "wrote" counts), a
 * quiet parent, or a parent whose R and W both overflowed (its record is size-flagged, capture_incomplete). */
static inline bool untracked(const t2_frame *p)
{
    return p == NULL || p->kind == T2_KIND_BARRIER || p->quiet || (p->R.overflow && p->W.overflow);
}

static t2_frame *push_frame(t2_thread *t, uint32_t va, uint32_t kind, uintptr_t sp)
{
    if (t->depth == t->cap) {
        if (t->cap >= T2_MAX_DEPTH) {
            atomic_fetch_add(&st_depth_overflow, 1u);
            return NULL;
        }
        uint32_t nc = t->cap ? t->cap * 2u : 64u;
        t2_frame **nf = realloc(t->frames, (size_t)nc * sizeof *nf);
        if (nf == NULL) abort();
        memset(nf + t->cap, 0, (size_t)(nc - t->cap) * sizeof *nf);
        t->frames = nf;
        t->cap = nc;
    }
    t2_frame *f = t->frames[t->depth];
    if (f == NULL) {
        f = calloc(1, sizeof *f);
        if (f == NULL) abort();
        t->frames[t->depth] = f;
    }
    t2_frame *parent = top_frame(t);
    t->depth++;
    f->va = va;
    f->kind = kind;
    f->off_target = !target_allowed(va);
    f->skip = f->off_target || budget_full(va);
    f->quiet = f->skip && untracked(parent);
    f->quiet_wrote = false;
    if (f->quiet) atomic_fetch_add_explicit(&st_budget_quiet, 1u, memory_order_relaxed);
    f->caller_va = parent ? parent->va : 0u;
    f->flags = 0u;
    f->entered = kind == T2_KIND_LIFTED;
    f->offered = false;
    f->exited = false;
    f->have_exit = false;
    f->sp_open = sp;
    f->sp_body = sp;
    f->switch_epoch = t->switch_epoch;
    for (uint32_t i = 0; i < f->ncallees; i++) free(f->callees[i].input);
    f->ncallees = 0u;
    f->input_active = false;
    f->input_valid = false;
    f->arena_n = 0u;
    f->callee_hash = 0x5EEDu;
    snapshot_regs(&f->entry, &f->entry_imp);
    f->entry_esp = g_esp;
    f->fp_top_entry = g_fp_top & 7;
    uint32_t ret = 0u;
    (void)safe_words(t, g_esp, &ret, 1u);
    f->ret_addr = ret;
    f->nargs = safe_words(t, g_esp + 4u, f->args, kind == T2_KIND_CALL ? callee_arity(va) : T2_MAX_ARGS);
    return f;
}

static inline uintptr_t frame_key(const t2_frame *f)
{
    if (f->kind == T2_KIND_CALL && (!f->entered || f->exited)) return f->sp_open;
    return f->sp_body;
}

static void record_append_u32(t2_thread *t, uint32_t v);

static void buf_reserve(t2_thread *t, size_t more)
{
    if (t->buf_len + more <= t->buf_cap) return;
    size_t nc = t->buf_cap ? t->buf_cap : 4096u;
    while (nc < t->buf_len + more) nc *= 2u;
    uint8_t *nb = realloc(t->buf, nc);
    if (nb == NULL) abort();
    t->buf = nb;
    t->buf_cap = nc;
}

static void record_append(t2_thread *t, const void *data, size_t len)
{
    buf_reserve(t, len);
    memcpy(t->buf + t->buf_len, data, len);
    t->buf_len += len;
}

static void record_append_u32(t2_thread *t, uint32_t v)
{
    record_append(t, &v, 4);
}

static int went_cmp(const void *a, const void *b)
{
    uint32_t x = ((const t2_went *)a)->word, y = ((const t2_went *)b)->word;
    return x < y ? -1 : x > y;
}

/* Serialize word entries as coalesced byte runs: u32 count, then (u32 addr, u32 len, bytes). */
static void append_runs(t2_thread *t, t2_went *ent, uint32_t n)
{
    static __thread t2_went *sorted;
    static __thread uint32_t sorted_cap;
    if (n > sorted_cap) {
        t2_went *ns = realloc(sorted, (size_t)n * sizeof *ns);
        if (ns == NULL) abort();
        sorted = ns;
        sorted_cap = n;
    }
    if (n) memcpy(sorted, ent, (size_t)n * sizeof *ent);
    qsort(sorted, n, sizeof *sorted, went_cmp);
    size_t count_at = t->buf_len;
    record_append_u32(t, 0u);
    uint32_t runs = 0;
    size_t len_at = 0;
    uint32_t run_len = 0, next_addr = 0;
    bool open = false;
    for (uint32_t i = 0; i < n; i++) {
        for (uint32_t b = 0; b < 4u; b++) {
            if (!(sorted[i].mask & (1u << b))) continue;
            uint32_t addr = sorted[i].word + b;
            uint8_t byte = (uint8_t)(sorted[i].val >> (8u * b));
            if (!open || addr != next_addr) {
                if (open) memcpy(t->buf + len_at, &run_len, 4);
                record_append_u32(t, addr);
                len_at = t->buf_len;
                record_append_u32(t, 0u);
                run_len = 0;
                runs++;
                open = true;
            }
            record_append(t, &byte, 1);
            run_len++;
            next_addr = addr + 1u;
        }
    }
    if (open) memcpy(t->buf + len_at, &run_len, 4);
    memcpy(t->buf + count_at, &runs, 4);
}

/* Shape class of an accessed word: esp-relative (the frame and its arguments), relative to the this pointer
 * (entry ecx) or one of the first four stack arguments (an object or buffer the caller passed), absolute inside
 * the XBE image (a global), else dynamic (heap reached through loaded pointers). Dynamic words return 0 and are
 * only counted: their addresses change from call to call, so hashing them made every call a new shape. */
#define T2_PTR_WINDOW 0x1000u
static uint64_t addr_class(const t2_frame *f, uint32_t word)
{
    uint32_t rel = word - f->entry_esp;
    if (rel + T2_ESP_WINDOW <= 2u * T2_ESP_WINDOW) return 0x100000000ull | rel;
    rel = word - f->entry.ecx;
    if (rel < T2_PTR_WINDOW) return 0x200000000ull | rel;
    for (uint32_t k = 0; k < 4u && k < f->nargs; k++) {
        rel = word - f->args[k];
        if (rel < T2_PTR_WINDOW) return ((uint64_t)(3u + k) << 32) | rel;
    }
    if (word >= g_image_lo && word < g_image_hi) return 0x700000000ull | word;
    return 0u;
}

static uint32_t bit_length(uint32_t v)
{
    return v ? 32u - (uint32_t)__builtin_clz(v) : 0u;
}

static uint64_t frame_shape(const t2_frame *f)
{
    uint64_t r = 0, w = 0;
    uint32_t r_dynamic = 0, w_dynamic = 0;
    for (uint32_t i = 0; i < f->R.n; i++) {
        uint64_t c = addr_class(f, f->R.ent[i].word);
        if (c == 0u) r_dynamic++;
        else r += mix64(c * 31u + f->R.ent[i].mask);
    }
    for (uint32_t i = 0; i < f->W.n; i++) {
        uint64_t c = addr_class(f, f->W.ent[i].word);
        if (c == 0u) w_dynamic++;
        else w += mix64(c * 37u + f->W.ent[i].mask + 7u);
    }
    uint64_t h = mix64(f->callee_hash ^ mix64(r) ^ (mix64(w) * 3u));
    h ^= (uint64_t)bit_length(r_dynamic) << 40 ^ (uint64_t)bit_length(w_dynamic) << 48;
    return mix64(h ^ ((uint64_t)bit_length(f->R.bytes) << 8) ^ bit_length(f->W.bytes) ^ ((uint64_t)f->ncallees << 16));
}

static uint64_t frame_digest(const t2_frame *f)
{
    uint64_t h = hash_bytes(&f->entry, sizeof f->entry, 0xD16E57u);
    h = hash_bytes(&f->entry_imp, sizeof f->entry_imp, h);
    uint64_t r = 0;
    for (uint32_t i = 0; i < f->R.n; i++) {
        const t2_went *e = &f->R.ent[i];
        uint32_t bytemask = 0;
        for (uint32_t b = 0; b < 4u; b++)
            if (e->mask & (1u << b)) bytemask |= 0xFFu << (8u * b);
        r += mix64(((uint64_t)e->word << 32 | (e->val & bytemask)) ^ ((uint64_t)e->mask << 60));
    }
    /* A changed dispatch pre-state is a distinct input even when the guest
     * registers and read-set are identical. Hash explicit fields, not padding. */
    for (uint32_t i = 0; i < f->ncallees; i++) {
        const t2_input_annotation *a = f->callees[i].input;
        if (a == NULL) continue;
        h = hash_bytes(&i, sizeof i, h);
        h = hash_bytes(&a->generation, sizeof a->generation, h);
        h = hash_bytes(&a->address, sizeof a->address, h);
        h = hash_bytes(&a->route, sizeof a->route, h);
        h = hash_bytes(&a->handler, sizeof a->handler, h);
        h = hash_bytes(&a->default_return, sizeof a->default_return, h);
        h = hash_bytes(&a->complete, sizeof a->complete, h);
        h = hash_bytes(&a->entry_count, sizeof a->entry_count, h);
        for (uint32_t j = 0; j < a->entry_count; j++) {
            h = hash_bytes(&a->entries[j].address, sizeof a->entries[j].address, h);
            h = hash_bytes(&a->entries[j].handler, sizeof a->entries[j].handler, h);
            h = hash_bytes(&a->entries[j].default_return, sizeof a->entries[j].default_return, h);
        }
    }
    return mix64(h ^ r);
}

/* T3836: `bare` drops the sets and callees and adds the size flag: the frame of a record over
 * T2_RECORD_CEILING is still spooled (never judged, the function stays incomplete_size), but bounded. */
static void build_record(t2_thread *t, const t2_frame *f, bool bare)
{
    t->buf_len = 0;
    record_append_u32(t, 0u); /* record length, patched below */
    uint64_t poll = tsfp_input_host_poll_count(0u);
    uint32_t input_count = 0u;
    if (!bare) for (uint32_t i = 0; i < f->ncallees; i++)
        if (f->callees[i].input != NULL) input_count++;
    uint32_t flags = f->flags | T2_FORMAT_IMPLICIT | (bare ? T2_FLAG_SIZE : 0u);
    if (input_count) flags |= T2_FORMAT_INPUT_ANNOTATIONS | T2_FLAG_INPUT_UNVERIFIED;
    uint32_t head[6] = {f->va, f->caller_va, f->ret_addr, (uint32_t)poll,
                        kernel_thread_current_identity ? kernel_thread_current_identity() : 0u, flags};
    uint32_t ncallees = bare ? 0u : f->ncallees;
    record_append(t, head, sizeof head);
    record_append(t, &f->entry, sizeof f->entry);
    record_append(t, &f->exit, sizeof f->exit);
    append_runs(t, f->R.ent, bare ? 0u : f->R.n);
    append_runs(t, f->W.ent, bare ? 0u : f->W.n);
    record_append_u32(t, ncallees);
    for (uint32_t i = 0; i < ncallees; i++) {
        const t2_callee *c = &f->callees[i];
        record_append_u32(t, c->va);
        record_append(t, &c->entry, sizeof c->entry);
        record_append_u32(t, c->nargs);
        record_append(t, c->args, (size_t)c->nargs * 4u);
        record_append(t, &c->exit, sizeof c->exit);
        append_runs(t, f->arena + c->w_off, c->w_n);
    }
    /* Format 2 tail: entry and exit implicit state, then each callee's exit implicit state. */
    record_append(t, &f->entry_imp, sizeof f->entry_imp);
    record_append(t, &f->exit_imp, sizeof f->exit_imp);
    for (uint32_t i = 0; i < ncallees; i++)
        record_append(t, &f->callees[i].exit_imp, sizeof f->callees[i].exit_imp);
    if (input_count) {
        record_append_u32(t, 1u); /* input annotation version */
        record_append_u32(t, input_count);
        for (uint32_t i = 0; i < ncallees; i++) {
            const t2_input_annotation *a = f->callees[i].input;
            if (a == NULL) continue;
            record_append_u32(t, i);
            record_append(t, &a->generation, sizeof a->generation);
            record_append(t, &a->lifecycle_sequence, sizeof a->lifecycle_sequence);
            record_append(t, &a->selection_sequence, sizeof a->selection_sequence);
            record_append_u32(t, a->address);
            record_append_u32(t, a->route);
            record_append_u32(t, a->handler);
            record_append_u32(t, a->default_return);
            record_append_u32(t, a->complete);
            record_append_u32(t, a->entry_count);
            for (uint32_t j = 0; j < a->entry_count; j++) {
                record_append_u32(t, a->entries[j].address);
                record_append_u32(t, a->entries[j].handler);
                record_append_u32(t, a->entries[j].default_return);
            }
        }
    }
    uint32_t len = (uint32_t)(t->buf_len - 4u);
    memcpy(t->buf, &len, 4);
}

static void spool_frame(t2_thread *t, const t2_frame *f)
{
    build_record(t, f, false);
    if (t->buf_len - 4u > T2_RECORD_CEILING) {
        atomic_fetch_add(&st_over_record_ceiling, 1u);
        build_record(t, f, true);
    }
    pthread_mutex_lock(&g_spool_lock);
    bool written = sink_record(t->buf, t->buf_len);
    pthread_mutex_unlock(&g_spool_lock);
    if (written) atomic_fetch_add(&st_spool_bytes, t->buf_len);
}

static void count_flags(atomic_ullong *table, uint32_t flags)
{
    for (int i = 0; i < T2_NFLAGS; i++)
        if (flags & (1u << i)) atomic_fetch_add(&table[i], 1u);
}

/* A callee's first reads are the parent's first reads too, unless the parent defined the byte before the
 * call (its W at this point holds only what was written before the call returned... minus the callee's own
 * writes, which merge after this). Ports that inline a callee then still find every byte they read. */
static void merge_r(t2_frame *dst, const t2_frame *src)
{
    if (src->R.overflow) {
        dst->R.overflow = true;
        dst->flags |= T2_FLAG_SIZE;
    }
    if (dst->R.overflow) return;
    for (uint32_t i = 0; i < src->R.n; i++) {
        const t2_went *e = &src->R.ent[i];
        uint32_t mask = e->mask;
        const t2_went *w = wmap_find(&dst->W, e->word);
        if (w != NULL) mask &= ~w->mask;
        if (mask == 0u) continue;
        t2_went *r = wmap_get(&dst->R, e->word);
        uint32_t fresh = mask & ~r->mask;
        if (fresh == 0u) continue;
        went_put(r, fresh, e->val);
        dst->R.bytes += popcount4(fresh);
    }
    if (dst->R.bytes > g_set_cap_bytes) {
        dst->R.overflow = true;
        dst->flags |= T2_FLAG_SIZE;
    }
}

static void merge_w(t2_frame *dst, const t2_wmap *src)
{
    if (src->overflow) {
        dst->W.overflow = true;
        dst->flags |= T2_FLAG_SIZE;
    }
    if (dst->W.overflow) return;
    for (uint32_t i = 0; i < src->n; i++) {
        t2_went *e = wmap_get(&dst->W, src->ent[i].word);
        dst->W.bytes += popcount4(src->ent[i].mask & ~e->mask);
        went_put(e, src->ent[i].mask, src->ent[i].val);
    }
    if (dst->W.bytes > g_set_cap_bytes) {
        dst->W.overflow = true;
        dst->flags |= T2_FLAG_SIZE;
    }
}

static void add_callee(t2_frame *p, const t2_frame *f)
{
    p->callee_hash = mix64(p->callee_hash ^ f->va) + 0x9E37u;
    if (p->ncallees >= g_max_callees) {
        p->flags |= T2_FLAG_SIZE;
        p->ncallees = g_max_callees; /* the record keeps the first g_max_callees; the frame is flagged */
        return;
    }
    if (p->ncallees == p->callee_cap) {
        uint32_t nc = p->callee_cap ? p->callee_cap * 2u : 4u;
        t2_callee *n = realloc(p->callees, (size_t)nc * sizeof *n);
        if (n == NULL) abort();
        p->callees = n;
        p->callee_cap = nc;
    }
    t2_callee *c = &p->callees[p->ncallees++];
    c->va = f->va;
    c->nargs = f->nargs;
    memcpy(c->args, f->args, (size_t)f->nargs * 4u);
    c->entry = f->entry;
    c->exit = f->exit;
    c->exit_imp = f->exit_imp;
    c->input = NULL;
    if (f->input_active) {
        c->input = malloc(sizeof *c->input);
        if (c->input == NULL) abort();
        *c->input = *f->input;
    }
    c->w_off = p->arena_n;
    c->w_n = 0u;
    if (f->W.overflow) return;
    if (p->arena_n + f->W.n > p->arena_cap) {
        uint32_t nc = p->arena_cap ? p->arena_cap : 64u;
        while (nc < p->arena_n + f->W.n) nc *= 2u;
        t2_went *na = realloc(p->arena, (size_t)nc * sizeof *na);
        if (na == NULL) abort();
        p->arena = na;
        p->arena_cap = nc;
    }
    if (f->W.n) memcpy(p->arena + p->arena_n, f->W.ent, (size_t)f->W.n * sizeof *f->W.ent);
    p->arena_n += f->W.n;
    c->w_n = f->W.n;
}

/* Pop the top frame: offer its own record (function frames), hand its outcome to its parent. */
static void pop_barrier(t2_thread *t);

static void pop_frame(t2_thread *t, bool abandoned)
{
    t2_frame *f = t->frames[t->depth - 1u];
    if (f->kind == T2_KIND_BARRIER) {
        pop_barrier(t);
        return;
    }
    if (abandoned) {
        f->flags |= T2_FLAG_SEH;
        atomic_fetch_add(&st_seh_pops, 1u);
    }
    if (!f->have_exit) {
        snapshot_regs(&f->exit, &f->exit_imp);
        f->have_exit = true;
    }
    if (f->switch_epoch != t->switch_epoch) f->flags |= T2_FLAG_THREAD_SWITCH;
    int delta = ((f->fp_top_entry - (g_fp_top & 7)) + 8) & 7; /* net x87 pushes, mod 8 */
    if (delta >= 3 && delta <= 5) f->flags |= T2_FLAG_FPU;     /* +3..+5 or -3..-5: past a plausible call */
    if (f->R.overflow || f->W.overflow) f->flags |= T2_FLAG_SIZE;
    if (f->entered && !f->offered && f->skip) {
        atomic_fetch_add(&st_frames, 1u);
        atomic_fetch_add(f->off_target ? &st_target_skipped : &st_budget_skipped, 1u); /* T3803, T3833 */
    } else if (f->entered && !f->offered) {
        atomic_fetch_add(&st_frames, 1u);
        count_flags(st_flag_frames, f->flags);
        if (sampler_offer(f->va, frame_shape(f), frame_digest(f))) {
            atomic_fetch_add(&st_kept, 1u);
            count_flags(st_flag_kept, f->flags);
            spool_frame(t, f);
        }
    } else if (!f->entered) {
        atomic_fetch_add(&st_host_callees, 1u);
        t2_func *hf = func_lookup(f->va);
        if (hf != NULL) atomic_fetch_add_explicit(&hf->host_calls, 1u, memory_order_relaxed);
    }
    t->depth--;
    t2_frame *p = top_frame(t);
    if (p != NULL && p->kind == T2_KIND_BARRIER) {
        /* A root under a safe point: only whether it wrote matters to the frames below. */
        if (f->W.n != 0u || f->W.overflow) p->W.overflow = true;
        if (f->quiet_wrote) p->W.overflow = true;
    } else if (p != NULL && p->quiet) {
        /* T3803: an untracked parent only learns that its subtree wrote */
        if (f->W.n != 0u || f->W.overflow || f->quiet_wrote) p->quiet_wrote = true;
        if (abandoned) p->flags |= T2_FLAG_SEH;
    } else if (p != NULL) {
        /* Inlined ports can execute the callee's reads instead of its recorded
         * stub. Preserve the warning that its input may change between reads. */
        p->flags |= f->flags & T2_FLAG_MMIO;
        p->flags |= f->flags & T2_FLAG_INPUT_UNVERIFIED;
        add_callee(p, f);
        merge_r(p, f); /* before merge_w: a byte the callee read, then wrote, was still read first */
        merge_w(p, &f->W);
        if (abandoned) p->flags |= T2_FLAG_SEH;
    }
    wmap_clear(&f->R);
    wmap_clear(&f->W);
}

/* Barrier frames reuse the frame slots but record nothing; W.overflow means "something under it wrote". */
static void push_barrier(t2_thread *t, uintptr_t sp)
{
    t->barrier_pending = false;
    if (t->depth == t->cap) {
        if (t->cap >= T2_MAX_DEPTH) {
            atomic_fetch_add(&st_depth_overflow, 1u);
            return;
        }
        uint32_t nc = t->cap ? t->cap * 2u : 64u;
        t2_frame **nf = realloc(t->frames, (size_t)nc * sizeof *nf);
        if (nf == NULL) abort();
        memset(nf + t->cap, 0, (size_t)(nc - t->cap) * sizeof *nf);
        t->frames = nf;
        t->cap = nc;
    }
    t2_frame *f = t->frames[t->depth];
    if (f == NULL) {
        f = calloc(1, sizeof *f);
        if (f == NULL) abort();
        t->frames[t->depth] = f;
    }
    t->depth++;
    f->va = 0u;
    f->kind = T2_KIND_BARRIER;
    f->skip = f->quiet = f->quiet_wrote = false;
    f->flags = 0u;
    f->entered = false;
    f->exited = false;
    f->have_exit = true;
    f->sp_open = sp;
    f->sp_body = sp;
    f->ncallees = 0u;
    f->arena_n = 0u;
    wmap_clear(&f->R);
    wmap_clear(&f->W);
    atomic_fetch_add(&st_barriers, 1u);
}

static void pop_barrier(t2_thread *t)
{
    t2_frame *b = t->frames[t->depth - 1u];
    bool wrote = b->W.overflow;
    wmap_clear(&b->R);
    wmap_clear(&b->W);
    t->depth--;
    if (!wrote) return;
    atomic_fetch_add(&st_barrier_writes, 1u);
    for (uint32_t i = t->depth; i-- > 0u;) {
        t2_frame *f = t->frames[i];
        if (f->kind == T2_KIND_BARRIER) break; /* frames below an outer barrier belong to its own check */
        f->flags |= T2_FLAG_THREAD_SWITCH;
    }
}

/* A frame is about to be pushed at native position `sp`: materialise a pending safe-point barrier if this
 * push happens inside it (deeper), drop it if the safe point is gone (a non-local exit skipped its end). */
static inline void barrier_check(t2_thread *t, uintptr_t sp)
{
    if (T2_LIKELY(!t->barrier_pending)) return;
    if (sp < t->barrier_sp) push_barrier(t, t->barrier_sp);
    else t->barrier_pending = false;
}

/* Pop frames that a non-local exit left behind: native position deeper than `sp`. */
static void unwind_to(t2_thread *t, uintptr_t sp)
{
    while (t->depth && frame_key(t->frames[t->depth - 1u]) < sp) pop_frame(t, true);
}

/* ---- hooks ---- */
/* T3740 fidelity: the offline runner's lifted control (rust_ports.c t2o_run_lifted) sees every CAP_ENTER, so
 * a tail call (`sub_X(); return;`, RECOMP_ITAIL), which no RECOMP_ABI_CALL brackets, is replayed from the
 * capture like any other callee. Set only around that run, on its thread; NULL in a capturing game. */
static _Thread_local bool (*t2cap_offline_enter)(uint32_t va);
void t2cap_set_offline_enter_hook(bool (*hook)(uint32_t va))
{
    t2cap_offline_enter = hook;
}

T2_HOOK bool t2cap_enter(uint32_t va)
{
    if (T2_UNLIKELY(t2cap_offline_enter != NULL) && t2cap_offline_enter(va)) return true;
    if (!capture_on()) return false;
    t2_thread *t = thread_state();
    uintptr_t sp = T2_SP();
    unwind_to(t, sp);
    t2_frame *top = top_frame(t);
    if (top != NULL && top->kind == T2_KIND_CALL && !top->entered && top->va == va && top->sp_open >= sp &&
        !t->barrier_pending) {
        top->entered = true;
        top->sp_body = sp;
        return false;
    }
    barrier_check(t, sp);
    (void)push_frame(t, va, T2_KIND_LIFTED, sp);
    return false;
}

T2_HOOK void t2cap_exit(void)
{
    if (!capture_on()) return;
    t2_thread *t = thread_state();
    uintptr_t sp = T2_SP();
    unwind_to(t, sp);
    t2_frame *top;
    while ((top = top_frame(t)) != NULL && top->kind == T2_KIND_CALL && (!top->entered || top->exited) &&
           top->sp_open == sp)
        pop_frame(t, true); /* a call made by this function whose call_end never ran */
    if (top == NULL || frame_key(top) != sp) {
        atomic_fetch_add(&st_unmatched_exit, 1u);
        return;
    }
    snapshot_regs(&top->exit, &top->exit_imp);
    top->have_exit = true;
    if (top->kind == T2_KIND_LIFTED) {
        pop_frame(t, false);
    } else {
        top->exited = true; /* the call site pops it, with the registers at call_end */
    }
}

T2_HOOK void t2cap_call_begin(uint32_t va)
{
    if (!capture_on()) return;
    t2_thread *t = thread_state();
    uintptr_t sp = T2_SP();
    unwind_to(t, sp);
    barrier_check(t, sp);
    if (push_frame(t, va, T2_KIND_CALL, sp) == NULL) return;
    atomic_fetch_add(&st_call_frames, 1u);
    if (is_kernel_wait(va)) t->switch_epoch++;
}

T2_HOOK void t2cap_input_begin(const t2_input_annotation *annotation)
{
    if (!capture_on()) return;
    t2_thread *t = thread_state();
    t2_frame *f = top_frame(t);
    if (f == NULL) return;
    f->flags |= T2_FLAG_INPUT_UNVERIFIED;
    if (f->kind != T2_KIND_CALL || annotation == NULL || f->input_active ||
        annotation->entry_count > T2_INPUT_MAX_ENTRIES || f->va != annotation->address) {
        if (f->input_active) f->input->complete = 0u;
        f->input_valid = false;
        return;
    }
    if (f->input == NULL) {
        f->input = malloc(sizeof *f->input);
        if (f->input == NULL) abort();
    }
    *f->input = *annotation;
    f->input_valid = annotation->complete == 1u;
    f->input->complete = 0u;
    f->input_active = true;
}

T2_HOOK void t2cap_input_end(uint32_t address)
{
    if (!capture_on()) return;
    t2_frame *f = top_frame(thread_state());
    if (f == NULL) return;
    f->flags |= T2_FLAG_INPUT_UNVERIFIED;
    if (f->kind != T2_KIND_CALL || !f->input_active || f->input->address != address) {
        if (f->input_active) f->input->complete = 0u;
        f->input_valid = false;
        return;
    }
    f->input->complete = f->input_valid ? 1u : 0u;
    f->input_valid = false;
}

/* The cooperative safe point of RECOMP_ABI_CALL (recomp_cooperative.h). The barrier is pushed lazily by the
 * first frame inside it, so a safe point that runs no guest code costs two branches. */
void recomp_call_safepoint(uint32_t callee);

T2_HOOK void t2cap_safepoint(uint32_t va)
{
    if (!capture_on()) {
        recomp_call_safepoint(va);
        return;
    }
    t2_thread *t = thread_state();
    uintptr_t sp = T2_SP();
    unwind_to(t, sp);
    bool outer_pending = t->barrier_pending;
    uintptr_t outer_sp = t->barrier_sp;
    uint32_t depth = t->depth;
    t->barrier_pending = true;
    t->barrier_sp = sp;
    recomp_call_safepoint(va);
    t->barrier_pending = outer_pending;
    t->barrier_sp = outer_sp;
    if (t->depth > depth) {
        while (t->depth > depth + 1u) pop_frame(t, true); /* left inside the safe point without an exit */
        pop_frame(t, false);                              /* the barrier itself */
    }
}

T2_HOOK void t2cap_call_end(uint32_t va)
{
    if (!capture_on()) return;
    t2_thread *t = thread_state();
    uintptr_t sp = T2_SP();
    unwind_to(t, sp);
    t2_frame *top = top_frame(t);
    if (top == NULL || top->kind != T2_KIND_CALL || top->sp_open != sp || top->va != va) {
        atomic_fetch_add(&st_unmatched_exit, 1u);
        return;
    }
    rust_ext_registers own_exit = top->exit;
    t2_implicit own_exit_imp = top->exit_imp;
    bool had_exit = top->have_exit;
    snapshot_regs(&top->exit, &top->exit_imp); /* the callee record: registers after the call returned */
    top->have_exit = true;
    if (top->entered && had_exit) {
        /* The function's own record uses the registers at its CAP_EXIT; they equal these unless
         * the call wrapper changed something, which the callee record must show. */
        rust_ext_registers callee_exit = top->exit;
        t2_implicit callee_exit_imp = top->exit_imp;
        top->exit = own_exit;
        top->exit_imp = own_exit_imp;
        t2_frame *f = top;
        /* pop_frame() spools with f->exit, then add_callee() copies it: swap in between. */
        if (f->switch_epoch != t->switch_epoch) f->flags |= T2_FLAG_THREAD_SWITCH;
        int delta = ((f->fp_top_entry - (g_fp_top & 7)) + 8) & 7;
        if (delta >= 3 && delta <= 5) f->flags |= T2_FLAG_FPU;
        if (f->R.overflow || f->W.overflow) f->flags |= T2_FLAG_SIZE;
        atomic_fetch_add(&st_frames, 1u);
        if (f->skip) {
            atomic_fetch_add(f->off_target ? &st_target_skipped : &st_budget_skipped, 1u); /* T3803, T3833 */
        } else {
            count_flags(st_flag_frames, f->flags);
            if (sampler_offer(f->va, frame_shape(f), frame_digest(f))) {
                atomic_fetch_add(&st_kept, 1u);
                count_flags(st_flag_kept, f->flags);
                spool_frame(t, f);
            }
        }
        f->exit = callee_exit;
        f->exit_imp = callee_exit_imp;
        f->offered = true; /* pop_frame() only hands it to the parent */
    }
    pop_frame(t, false);
}

/* ---- memory accesses ---- */
/* T3767: bytes of `word` (mask `changed`, values in `bits`) that the frame knew hold other values now:
 * host code wrote them, outside every hook. HLE and kernel callees are host callees whose writes the
 * capture never sees (0x27BB0: 0x407AF8 stores its result through ecx = esp, the caller then tests
 * the byte; the record said "callee writes nothing" and the replay read the caller's own push).
 * Attribute them to the frame's last callee, whose outcome is replayed before the read, as the
 * original saw it; with no callee since, flag the frame host_write (capture_incomplete). */
static void host_wrote(t2_frame *f, uint32_t word, uint32_t changed, uint32_t bits)
{
    atomic_fetch_add(&st_host_writes, 1u);
    if (f->ncallees == 0u || f->W.overflow) {
        f->flags |= T2_FLAG_HOST_WRITE;
        return;
    }
    t2_callee *c = &f->callees[f->ncallees - 1u];
    if (c->w_off + c->w_n != f->arena_n) { /* its writes are not the last in the arena (cannot happen) */
        f->flags |= T2_FLAG_HOST_WRITE;
        return;
    }
    t2_went *e = NULL;
    for (uint32_t i = c->w_off; i < f->arena_n; i++)
        if (f->arena[i].word == word) {
            e = &f->arena[i];
            break;
        }
    if (e == NULL) {
        if (f->arena_n == f->arena_cap) {
            uint32_t nc = f->arena_cap ? f->arena_cap * 2u : 64u;
            t2_went *na = realloc(f->arena, (size_t)nc * sizeof *na);
            if (na == NULL) abort();
            f->arena = na;
            f->arena_cap = nc;
        }
        e = &f->arena[f->arena_n++];
        e->word = word;
        e->val = 0u;
        e->mask = 0u;
        c->w_n++;
    }
    went_put(e, changed, bits);
    t2_went *w = wmap_get(&f->W, word); /* the subtree wrote it: defined from now on */
    f->W.bytes += popcount4(changed & ~w->mask);
    went_put(w, changed, bits);
    atomic_fetch_add(&st_host_writes_attributed, 1u);
    if (f->W.bytes > g_set_cap_bytes) {
        f->W.overflow = true;
        f->flags |= T2_FLAG_SIZE;
    }
}

static void note_read(uint32_t addr, uint32_t size, const uint8_t *bytes)
{
    if (T2_UNLIKELY(atomic_load_explicit(&g_state, memory_order_relaxed) != 1)) return; /* stopped (T3745) */
    t2_thread *t = tls_thread;
    t2_frame *f = t ? top_frame(t) : NULL;
    if (f == NULL || f->quiet) return; /* T3803: a quiet frame records no reads */
    if (T2_UNLIKELY(is_mmio_range(addr, size))) f->flags |= T2_FLAG_MMIO;
    if (f->R.overflow) return;
    uint32_t i = 0;
    while (i < size) {
        uint32_t a = addr + i;
        uint32_t word = a & ~3u, off = a & 3u;
        uint32_t take = 4u - off;
        if (take > size - i) take = size - i;
        uint32_t mask = ((1u << take) - 1u) << off;
        uint32_t now = 0;
        for (uint32_t b = 0; b < take; b++) now |= (uint32_t)bytes[i + b] << (8u * (off + b));
        const t2_went *w = wmap_find(&f->W, word);
        const t2_went *seen = wmap_find(&f->R, word);
        uint32_t known_w = w != NULL ? w->mask & mask : 0u;
        uint32_t known_r = seen != NULL ? seen->mask & mask & ~known_w : 0u;
        if (known_w | known_r) {
            uint32_t changed = 0u;
            for (uint32_t b = off; b < off + take; b++) {
                uint32_t bit = 1u << b, shift = 8u * b;
                uint32_t want = (known_w & bit) ? w->val : (known_r & bit) ? seen->val : now;
                if (((want ^ now) >> shift) & 0xFFu) changed |= bit;
            }
            if (changed) {
                host_wrote(f, word, changed, now);
                w = wmap_find(&f->W, word);
            }
        }
        if (w != NULL) mask &= ~w->mask; /* defined by this frame or a callee */
        if (mask) {
            t2_went *r = wmap_get(&f->R, word);
            uint32_t fresh = mask & ~r->mask;
            if (fresh) {
                uint32_t bits = 0;
                for (uint32_t b = 0; b < take; b++) bits |= (uint32_t)bytes[i + b] << (8u * (off + b));
                went_put(r, fresh, bits);
                f->R.bytes += popcount4(fresh);
                if (f->R.bytes > g_set_cap_bytes) {
                    f->R.overflow = true;
                    f->flags |= T2_FLAG_SIZE;
                    return;
                }
            }
        }
        i += take;
    }
}

static void note_write(uint32_t addr, uint32_t size, const uint8_t *bytes)
{
    if (T2_UNLIKELY(atomic_load_explicit(&g_state, memory_order_relaxed) != 1)) return; /* stopped (T3745) */
    t2_thread *t = tls_thread;
    t2_frame *f = t ? top_frame(t) : NULL;
    if (f == NULL) return;
    if (f->quiet) { /* T3803: only that it wrote */
        f->quiet_wrote = true;
        return;
    }
    if (T2_UNLIKELY(is_mmio_range(addr, size))) f->flags |= T2_FLAG_MMIO;
    if (T2_UNLIKELY(is_code_store(addr, size))) f->flags |= T2_FLAG_SELFMOD;
    if (f->W.overflow) return;
    uint32_t i = 0;
    while (i < size) {
        uint32_t a = addr + i;
        uint32_t word = a & ~3u, off = a & 3u;
        uint32_t take = 4u - off;
        if (take > size - i) take = size - i;
        uint32_t mask = ((1u << take) - 1u) << off;
        uint32_t bits = 0;
        for (uint32_t b = 0; b < take; b++) bits |= (uint32_t)bytes[i + b] << (8u * (off + b));
        t2_went *e = wmap_get(&f->W, word);
        f->W.bytes += popcount4(mask & ~e->mask);
        went_put(e, mask, bits);
        if (f->W.bytes > g_set_cap_bytes) {
            f->W.overflow = true;
            f->flags |= T2_FLAG_SIZE;
            return;
        }
        i += take;
    }
}

#define T2_LOAD(name, type)                                                       \
    type name(uint32_t addr)                                                      \
    {                                                                             \
        type v;                                                                   \
        v = *(volatile type *)host_ptr(addr);                                     \
        if (T2_UNLIKELY(atomic_load_explicit(&g_state, memory_order_relaxed) == 1)) \
            note_read(addr, (uint32_t)sizeof v, (const uint8_t *)&v);             \
        return v;                                                                 \
    }
/* T3906: opt-in witness of the lifted-store hooks below (one per width, all CAP_ST*, PUSH32 and XMM/MMX stores funnel here).
 * `prepare` runs on the storing thread just before the store; its token is handed to `observer` just after. Memcpy,
 * memset, atomics and hand-written host writes are NOT routed (see tsfp-host-memory-capture writer_coverage). */
static _Atomic(t2cap_store_prepare_fn) g_store_prepare;
static _Atomic(t2cap_store_observer_fn) g_store_observer;
void t2cap_store_witness_set(t2cap_store_prepare_fn prepare, t2cap_store_observer_fn observer)
{
    atomic_store_explicit(&g_store_prepare, prepare, memory_order_release);
    atomic_store_explicit(&g_store_observer, observer, memory_order_release);
}
#define T2_STORE(name, type)                                                      \
    void name(uint32_t addr, type value)                                          \
    {                                                                             \
        t2cap_store_observer_fn witness = atomic_load_explicit(&g_store_observer, memory_order_acquire); \
        t2cap_store_prepare_fn prepare = witness ? atomic_load_explicit(&g_store_prepare, memory_order_acquire) : NULL; \
        uint64_t token = prepare ? prepare() : UINT64_MAX;                        \
        *(volatile type *)host_ptr(addr) = value;                                 \
        if (T2_UNLIKELY(witness != NULL)) witness(token, addr, &value, sizeof value); \
        if (T2_UNLIKELY(atomic_load_explicit(&g_state, memory_order_relaxed) == 1)) \
            note_write(addr, (uint32_t)sizeof value, (const uint8_t *)&value);    \
    }

T2_LOAD(t2cap_ld8, uint8_t)
T2_LOAD(t2cap_ld16, uint16_t)
T2_LOAD(t2cap_ld32, uint32_t)
T2_LOAD(t2cap_ld64, uint64_t)
T2_LOAD(t2cap_ldf, float)
T2_LOAD(t2cap_ldd, double)
T2_STORE(t2cap_st8, uint8_t)
T2_STORE(t2cap_st16, uint16_t)
T2_STORE(t2cap_st32, uint32_t)
T2_STORE(t2cap_st64, uint64_t)
T2_STORE(t2cap_stf, float)
T2_STORE(t2cap_std, double)

void t2cap_host_write_u32(uint32_t addr, uint32_t value)
{
    note_write(addr, sizeof value, (const uint8_t *)&value);
}

void t2cap_host_write_bytes(uint32_t addr, const void *bytes, uint32_t size)
{
    note_write(addr, size, (const uint8_t *)bytes);
}

/* ---- copies and fills ---- */
static bool guest_range(const void *p, size_t n, uint32_t *addr)
{
    uintptr_t base = (uintptr_t)g_xbox_mem_offset, q = (uintptr_t)p;
    if (q < base || q - base > UINT32_MAX || n > UINT32_MAX || (q - base) + n > 0x100000000ull) return false;
    *addr = (uint32_t)(q - base);
    return true;
}

/* T3906 range-writer witness (see t2_capture.h). Token is taken before the mutation, the record is made after it. */
static _Atomic(t2cap_store_prepare_fn) g_write_prepare;
static _Atomic(t2cap_write_observer_fn) g_write_observer;
void t2cap_write_witness_set(t2cap_store_prepare_fn prepare, t2cap_write_observer_fn observer)
{
    atomic_store_explicit(&g_write_prepare, prepare, memory_order_release);
    atomic_store_explicit(&g_write_observer, observer, memory_order_release);
}
static t2cap_write_observer_fn write_witness_begin(uint64_t *token)
{
    t2cap_write_observer_fn w = atomic_load_explicit(&g_write_observer, memory_order_acquire);
    t2cap_store_prepare_fn prepare = w ? atomic_load_explicit(&g_write_prepare, memory_order_acquire) : NULL;
    *token = prepare ? prepare() : UINT64_MAX;
    return w;
}
/* Report the post-state of a written range; a pointer range touching the guest window without lying fully inside it
 * (or longer than 4 GiB) is reported INVALID, never silently dropped. Zero-length and pure host ranges write no guest byte. */
static void write_witness_range(t2cap_write_observer_fn w, uint64_t token, t2cap_write_class cls, const void *p, const void *bytes, size_t n)
{
    uint32_t addr = 0;
    if (n == 0u) return;
    if (guest_range(p, n, &addr)) {
        w(token, (int)cls, addr, bytes, n);
        return;
    }
    uintptr_t base = (uintptr_t)g_xbox_mem_offset, q = (uintptr_t)p;
    if (q < base + 0x100000000ull && n > 0u && q + n > base) w(token, T2CAP_WRITE_INVALID, 0u, NULL, 0u);
}

static void note_range(uint32_t addr, size_t n, bool write)
{
    if (T2_UNLIKELY(atomic_load_explicit(&g_state, memory_order_relaxed) != 1)) return; /* stopped (T3745) */
    t2_thread *t = tls_thread;
    t2_frame *f = t ? top_frame(t) : NULL;
    if (f == NULL || n == 0u) return;
    if (f->quiet) {
        f->quiet_wrote |= write;
        return;
    }
    /* A read larger than the read-set cap can still fit: bytes already read or
     * written by this frame do not need another input entry.  At most two caps'
     * worth of bytes can already be known; larger ranges must overflow. */
    if (n > (write ? (size_t)g_set_cap_bytes : (size_t)g_set_cap_bytes * 2u)) {
        uint32_t bounded = n > UINT32_MAX ? UINT32_MAX : (uint32_t)n;
        if (is_mmio_range(addr, bounded)) f->flags |= T2_FLAG_MMIO;
        if (write) {
            if (is_code_store(addr, bounded)) f->flags |= T2_FLAG_SELFMOD;
            f->W.overflow = true;
        } else {
            f->R.overflow = true;
        }
        f->flags |= T2_FLAG_SIZE;
        return;
    }
    const uint8_t *bytes = (const uint8_t *)host_ptr(addr);
    for (size_t done = 0; done < n;) {
        uint32_t chunk = n - done > 4096u ? 4096u : (uint32_t)(n - done);
        if (write) note_write(addr + (uint32_t)done, chunk, bytes + done);
        else note_read(addr + (uint32_t)done, chunk, bytes + done);
        if (write ? f->W.overflow : f->R.overflow) break;
        done += chunk;
    }
}

static void *copy_common(void *destination, const void *source, size_t bytes, bool move)
{
    uint32_t src = 0, dst = 0;
    bool on = atomic_load_explicit(&g_state, memory_order_relaxed) == 1;
    bool src_guest = on && guest_range(source, bytes, &src);
    bool dst_guest = on && guest_range(destination, bytes, &dst);
    uint64_t token = 0;
    t2cap_write_observer_fn witness = write_witness_begin(&token);
    if (src_guest) note_range(src, bytes, false); /* before the copy: memmove may overlap */
    void *result = move ? recomp_guest_memmove(destination, source, bytes)
                        : recomp_guest_memcpy(destination, source, bytes);
    /* Post-state readback of the destination records an overlapping memmove exactly. */
    if (T2_UNLIKELY(witness != NULL)) write_witness_range(witness, token, T2CAP_WRITE_COPY, destination, destination, bytes);
    if (dst_guest) note_range(dst, bytes, true);
    if (src_guest || dst_guest) {
        atomic_fetch_add(&st_copies, 1u);
        atomic_fetch_add(&st_copy_bytes, bytes);
    }
    return result;
}

void *t2cap_memcpy(void *destination, const void *source, size_t bytes)
{
    return copy_common(destination, source, bytes, false);
}

void *t2cap_memmove(void *destination, const void *source, size_t bytes)
{
    return copy_common(destination, source, bytes, true);
}

uint32_t t2cap_atomic_add32(volatile void *word, uint32_t value)
{
    uint32_t addr = 0;
    bool guest = atomic_load_explicit(&g_state, memory_order_relaxed) == 1 && guest_range((const void *)word, 4u, &addr);
    if (guest) note_range(addr, 4u, false);
    uint64_t token = 0;
    t2cap_write_observer_fn witness = write_witness_begin(&token);
    uint32_t old = __sync_fetch_and_add((volatile uint32_t *)word, value);
    uint32_t written = old + value; /* what this operation stored, not a racy readback */
    if (T2_UNLIKELY(witness != NULL)) write_witness_range(witness, token, T2CAP_WRITE_ATOMIC, (const void *)word, &written, 4u);
    if (guest) note_range(addr, 4u, true);
    return old;
}

uint32_t t2cap_atomic_cas32(volatile void *word, uint32_t expected, uint32_t value)
{
    uint32_t addr = 0;
    bool guest = atomic_load_explicit(&g_state, memory_order_relaxed) == 1 && guest_range((const void *)word, 4u, &addr);
    if (guest) note_range(addr, 4u, false);
    uint64_t token = 0;
    t2cap_write_observer_fn witness = write_witness_begin(&token);
    uint32_t old = __sync_val_compare_and_swap((volatile uint32_t *)word, expected, value);
    /* A failed compare writes nothing: no witness event and no note. */
    if (T2_UNLIKELY(witness != NULL) && old == expected) write_witness_range(witness, token, T2CAP_WRITE_ATOMIC, (const void *)word, &value, 4u);
    if (guest && old == expected) note_range(addr, 4u, true);
    return old;
}

void *t2cap_memset(void *destination, int value, size_t bytes)
{
    uint64_t token = 0;
    t2cap_write_observer_fn witness = write_witness_begin(&token);
    void *result = memset(destination, value, bytes);
    if (T2_UNLIKELY(witness != NULL)) write_witness_range(witness, token, T2CAP_WRITE_FILL, destination, destination, bytes);
    uint32_t dst = 0;
    if (atomic_load_explicit(&g_state, memory_order_relaxed) == 1 && guest_range(destination, bytes, &dst)) {
        note_range(dst, bytes, true);
        atomic_fetch_add(&st_fills, 1u);
    }
    return result;
}
