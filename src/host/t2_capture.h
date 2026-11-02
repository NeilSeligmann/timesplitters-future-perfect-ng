/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3739: capture macros for capture-based Tier 2 (T3738, rule 43). The build transform
 * tools/t2capture/transform.py rewrites each lifted chunk so that guest memory is touched only
 * through these macros and every lifted function is bracketed by CAP_ENTER / CAP_EXIT. This header
 * is included by the transformed chunk right after "recomp_funcs.h" (which brings MEMn and
 * PUSH32/POP32 from recomp_types.h).
 *
 * PLAIN MODE (TSFP_CAPTURE undefined, the normal build): every macro expands to today's access,
 * CAP_LDn(a) is exactly MEMn(a), CAP_STn(a, v) is (MEMn(a) = (v)), CAP_ENTER/CAP_EXIT are empty.
 * tests/test_t2capture_transform.py compiles a transformed chunk and the original with the build's
 * flags and compares the text sections.
 *
 * CAPTURE MODE (TSFP_CAPTURE defined): calls into the runtime declared below. The runtime is
 * T3740 (src/host/t2_capture.c). Semantics the runtime must keep:
 *  - the load/store really performs the volatile access at XBOX_PTR(addr) and returns the value;
 *  - a store evaluates its VALUE first, then its ADDRESS, as clang does for `MEMn(a) = v`
 *    (the loads inside either operand are recorded in that order);
 *  - an address is truncated to 32 bits first, like XBOX_PTR.
 *
 * Capture holes this header closes: PUSH32/POP32 (recomp_types.h macros over MEM32) and the
 * XMM_MEM/XMM_STORE/MMX_MEM/MMX_STORE helpers (renamed by the transform to CAP_XMM_*, CAP_MMX_*).
 * T3740 closes memcpy/memmove (recomp_copy_redirect.h) and memset (rep stos) and brackets every
 * RECOMP_ABI_CALL. Still not closed: accesses made by hand-written host code (kernel/HLE thunks,
 * Rust ports) on behalf of the guest; see docs/t3738-capture-design.md "T3740 results".
 *
 * A tail call (`sub_B(); return;`) runs the callee's CAP_ENTER/CAP_EXIT inside the caller's frame,
 * then the caller's CAP_EXIT, so frames still nest.
 */
#ifndef TSFP_HOST_T2_CAPTURE_H
#define TSFP_HOST_T2_CAPTURE_H

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

/* Bounded input registry pre-state; only metadata, never guest snapshots. */
#define T2_INPUT_MAX_ENTRIES 64u
typedef struct { uint32_t address, handler, default_return; } t2_input_entry;
typedef struct {
    uint64_t generation, lifecycle_sequence, selection_sequence;
    uint32_t address, route, handler, default_return, complete, entry_count;
    t2_input_entry entries[T2_INPUT_MAX_ENTRIES];
} t2_input_annotation;

#ifndef TSFP_CAPTURE

#define CAP_LD8(a) MEM8(a)
#define CAP_LD16(a) MEM16(a)
#define CAP_LD32(a) MEM32(a)
#define CAP_LDS8(a) SMEM8(a)
#define CAP_LDS16(a) SMEM16(a)
#define CAP_LDS32(a) SMEM32(a)
#define CAP_LDS64(a) SMEM64(a)
#define CAP_LDF(a) MEMF(a)
#define CAP_LDD(a) MEMD(a)

#define CAP_ST8(a, v) (MEM8(a) = (v))
#define CAP_ST16(a, v) (MEM16(a) = (v))
#define CAP_ST32(a, v) (MEM32(a) = (v))
#define CAP_STS8(a, v) (SMEM8(a) = (v))
#define CAP_STS16(a, v) (SMEM16(a) = (v))
#define CAP_STS32(a, v) (SMEM32(a) = (v))
#define CAP_STS64(a, v) (SMEM64(a) = (v))
#define CAP_STF(a, v) (MEMF(a) = (v))
#define CAP_STD(a, v) (MEMD(a) = (v))

#define CAP_XMM_LD(a) XMM_MEM(a)
#define CAP_XMM_ST(a, v) XMM_STORE(a, v)
#define CAP_MMX_LD(a) MMX_MEM(a)
#define CAP_MMX_ST(a, v) MMX_STORE(a, v)

#define CAP_ENTER(va)
#define CAP_EXIT()

#endif /* !TSFP_CAPTURE */

#if defined(TSFP_CAPTURE) || defined(TSFP_CAPTURE_RUNTIME)
#include <stddef.h>
/* The caller supplies validity in complete at begin. End confirms normal return;
 * an abandoned or unmatched frame remains incomplete. */
void t2cap_input_begin(const t2_input_annotation *annotation);
void t2cap_input_end(uint32_t address);
/* Runtime entry points (T3740, src/host/t2_capture.c). Loads return the value read, stores write it. */
uint8_t t2cap_ld8(uint32_t addr);
uint16_t t2cap_ld16(uint32_t addr);
uint32_t t2cap_ld32(uint32_t addr);
uint64_t t2cap_ld64(uint32_t addr);
float t2cap_ldf(uint32_t addr);
double t2cap_ldd(uint32_t addr);
void t2cap_st8(uint32_t addr, uint8_t value);
void t2cap_st16(uint32_t addr, uint16_t value);
void t2cap_st32(uint32_t addr, uint32_t value);
/* T3906 opt-in store witness: prepare() before each lifted store, observer(token, addr, bytes, len) after it. */
typedef uint64_t (*t2cap_store_prepare_fn)(void);
typedef void (*t2cap_store_observer_fn)(uint64_t token, uint32_t addr, const void *bytes, size_t len);
void t2cap_store_witness_set(t2cap_store_prepare_fn prepare, t2cap_store_observer_fn observer);
/* T3906 opt-in witness of the range writers t2cap_memcpy/memmove (COPY), t2cap_memset (FILL) and the successful
 * t2cap_atomic_add32/cas32 write (ATOMIC). The observer gets the bytes now in guest memory after the writer finished.
 * INVALID reports a guest-window range that cannot be proven (straddling the window or over 4 GiB). */
typedef enum { T2CAP_WRITE_COPY = 1, T2CAP_WRITE_FILL, T2CAP_WRITE_ATOMIC, T2CAP_WRITE_INVALID } t2cap_write_class;
typedef void (*t2cap_write_observer_fn)(uint64_t token, int cls, uint32_t addr, const void *bytes, size_t len); /* cls: t2cap_write_class */
void t2cap_write_witness_set(t2cap_store_prepare_fn prepare, t2cap_write_observer_fn observer);
/* Record an already completed host write without writing guest memory again. */
void t2cap_host_write_u32(uint32_t addr, uint32_t value);
/* Record an already completed host write of an arbitrary byte range. */
void t2cap_host_write_bytes(uint32_t addr, const void *bytes, uint32_t size);
void t2cap_st64(uint32_t addr, uint64_t value);
void t2cap_stf(uint32_t addr, float value);
void t2cap_std(uint32_t addr, double value);
/* Returns true only inside the offline runner's lifted control (T3767): the entered function was
 * replayed from the capture, so CAP_ENTER returns from its body at once. */
bool t2cap_enter(uint32_t va);
void t2cap_exit(void);
/* T3740: the call site of every RECOMP_ABI_CALL (direct and indirect, lifted, kernel and HLE
 * targets alike), so a callee without CAP_ENTER is still a recorded callee. */
void t2cap_call_begin(uint32_t va);
void t2cap_call_end(uint32_t va);
/* T3740: the cooperative safe point (recomp_cooperative.h) runs host work, including guest vblank
 * callbacks, on this thread before a resolved callee. The runtime brackets it as an asynchronous
 * barrier: frames entered inside it are roots of their own, and if it wrote guest memory every open
 * frame is flagged thread_switch. Calls recomp_call_safepoint(va) itself. */
void t2cap_safepoint(uint32_t va);
/* T3740: guest copies and fills (recomp_copy_redirect.h redirects memcpy/memmove to
 * recomp_guest_memcpy; rep stos uses memset). Host pointers inside the guest window count as
 * reads of the source and writes of the destination; the copy itself still goes through
 * recomp_guest_memcpy/memmove. */
void *t2cap_memcpy(void *destination, const void *source, size_t bytes);
void *t2cap_memmove(void *destination, const void *source, size_t bytes);
void *t2cap_memset(void *destination, int value, size_t bytes);
/* T3740 fidelity: lock xadd / lock cmpxchg (recomp_types.h RECOMP_ATOMIC_*) take a host pointer;
 * the word counts as read before and written after (cmpxchg writes only when it swapped). */
uint32_t t2cap_atomic_add32(volatile void *word, uint32_t value);
uint32_t t2cap_atomic_cas32(volatile void *word, uint32_t expected, uint32_t value);
/* Flush the spool and write the stats sidecar (also run at exit). Safe to call repeatedly. */
void t2cap_flush(void);
/* Close the last segment, write the stats and stop recording; for exit paths that skip atexit (_exit). */
void t2cap_finish(void);
#endif

#ifdef TSFP_CAPTURE

#include "recomp_cooperative.h" /* recomp_direct_call_intercept, also outside the force-included chunks */

#define CAP_LD8(a) t2cap_ld8((uint32_t)(a))
#define CAP_LD16(a) t2cap_ld16((uint32_t)(a))
#define CAP_LD32(a) t2cap_ld32((uint32_t)(a))
#define CAP_LDS8(a) ((int8_t)t2cap_ld8((uint32_t)(a)))
#define CAP_LDS16(a) ((int16_t)t2cap_ld16((uint32_t)(a)))
#define CAP_LDS32(a) ((int32_t)t2cap_ld32((uint32_t)(a)))
#define CAP_LDS64(a) ((int64_t)t2cap_ld64((uint32_t)(a)))
#define CAP_LDF(a) t2cap_ldf((uint32_t)(a))
#define CAP_LDD(a) t2cap_ldd((uint32_t)(a))

/* Value first, then address: the order clang uses for MEMn(a) = v. Statement use only. */
#define CAP_ST_(fn, type, wire, a, v) \
    do { \
        type t2cap_v_ = (v); \
        fn((uint32_t)(a), (wire)t2cap_v_); \
    } while (0)
#define CAP_ST8(a, v) CAP_ST_(t2cap_st8, uint8_t, uint8_t, a, v)
#define CAP_ST16(a, v) CAP_ST_(t2cap_st16, uint16_t, uint16_t, a, v)
#define CAP_ST32(a, v) CAP_ST_(t2cap_st32, uint32_t, uint32_t, a, v)
#define CAP_STS8(a, v) CAP_ST_(t2cap_st8, int8_t, uint8_t, a, v)
#define CAP_STS16(a, v) CAP_ST_(t2cap_st16, int16_t, uint16_t, a, v)
#define CAP_STS32(a, v) CAP_ST_(t2cap_st32, int32_t, uint32_t, a, v)
#define CAP_STS64(a, v) CAP_ST_(t2cap_st64, int64_t, uint64_t, a, v)
#define CAP_STF(a, v) CAP_ST_(t2cap_stf, float, float, a, v)
#define CAP_STD(a, v) CAP_ST_(t2cap_std, double, double, a, v)

/* T3767: `if (...) return` so a function entered while the offline lifted control runs (a tail
 * callee, or a lifted function a host C replacement calls) is skipped after its captured outcome is
 * applied, and its caller continues as after a RET. Every lifted body returns void. */
#define CAP_ENTER(va) \
    if (t2cap_enter((uint32_t)(va))) return
#define CAP_EXIT() t2cap_exit()

/* T3740: every generated call (recomp_direct_call.h's wrapper, the same body) is bracketed by
 * the call-site hooks. The native frame of the caller is what pairs begin and end. The safe point
 * runs first and outside the call frame, so host work it does is never part of the callee. */
#ifdef RECOMP_ENABLE_COOPERATIVE_CALLS
#undef RECOMP_CALL_SAFEPOINT
#define RECOMP_CALL_SAFEPOINT(va) t2cap_safepoint((uint32_t)(va))
#endif
#undef RECOMP_ABI_CALL
#define RECOMP_ABI_CALL(va, fn) \
    do { \
        uint32_t t2cap_cva_ = (uint32_t)(va); \
        RECOMP_CALL_SAFEPOINT(t2cap_cva_); \
        t2cap_call_begin(t2cap_cva_); \
        if (recomp_direct_call_intercept(t2cap_cva_, &g_eax)) \
            g_esp += 4u; \
        else \
            (fn)(); \
        t2cap_call_end(t2cap_cva_); \
    } while (0)

/* T3740: guest copies and fills (the memcpy hole of T3739). */
#undef memcpy
#undef memmove
#undef memset
#define memcpy t2cap_memcpy
#define memmove t2cap_memmove
#define memset t2cap_memset

/* recomp_types.h stack macros, rebuilt over the capture accessors. */
#undef PUSH32
#undef POP32
#define PUSH32(sp, val) \
    do { \
        uint32_t _pv = (uint32_t)(val); \
        (sp) -= 4; \
        CAP_ST32(sp, _pv); \
    } while (0)
#define POP32(sp, dst) \
    do { \
        (dst) = CAP_LD32(sp); \
        (sp) += 4; \
    } while (0)

/* T3740 fidelity: movlps/movhps (XMM_LOAD_LOW/HIGH, XMM_STORE_LOW/HIGH) go through
 * recomp_xmm_load_half/store_half, whose MEM32 lives in recomp_types.h where the transform never
 * looks: their stores were missing from the write-set (expected 0xCD poison in the lifted control,
 * 0x153270 and its callers). Rebuilt over the capture accessors, same lane layout. */
#undef XMM_LOAD_LOW
#undef XMM_LOAD_HIGH
#undef XMM_STORE_LOW
#undef XMM_STORE_HIGH
#define XMM_LOAD_LOW(dst, addr) t2cap_xmm_load_half(&(dst), (addr), 0)
#define XMM_LOAD_HIGH(dst, addr) t2cap_xmm_load_half(&(dst), (addr), 1)
#define XMM_STORE_LOW(addr, src) t2cap_xmm_store_half((addr), (src), 0)
#define XMM_STORE_HIGH(addr, src) t2cap_xmm_store_half((addr), (src), 1)
static inline void t2cap_xmm_load_half(RecompXmm *dst, uint32_t addr, int high)
{
    dst->u[high * 2] = CAP_LD32(addr);
    dst->u[high * 2 + 1] = CAP_LD32(addr + 4);
}
static inline void t2cap_xmm_store_half(uint32_t addr, RecompXmm src, int high)
{
    CAP_ST32(addr, src.u[high * 2]);
    CAP_ST32(addr + 4, src.u[high * 2 + 1]);
}
#undef RECOMP_ATOMIC_ADD32
#undef RECOMP_ATOMIC_CAS32
/* the lift passes XBOX_PTR(...), an integer: cast as recomp_types.h does */
#define RECOMP_ATOMIC_ADD32(p, v) t2cap_atomic_add32((volatile void *)(p), (uint32_t)(v))
#define RECOMP_ATOMIC_CAS32(p, cmp, val) \
    t2cap_atomic_cas32((volatile void *)(p), (uint32_t)(cmp), (uint32_t)(val))

/* recomp_types.h XMM/MMX memory helpers over the capture accessors (same lane layout). */
static inline RecompXmm CAP_XMM_LD(uint32_t addr)
{
    RecompXmm r;
    r.u[0] = CAP_LD32(addr);
    r.u[1] = CAP_LD32(addr + 4);
    r.u[2] = CAP_LD32(addr + 8);
    r.u[3] = CAP_LD32(addr + 12);
    return r;
}

static inline void CAP_XMM_ST(uint32_t addr, RecompXmm v)
{
    CAP_ST32(addr, v.u[0]);
    CAP_ST32(addr + 4, v.u[1]);
    CAP_ST32(addr + 8, v.u[2]);
    CAP_ST32(addr + 12, v.u[3]);
}

static inline RecompMmx CAP_MMX_LD(uint32_t addr)
{
    RecompMmx r;
    r.q = t2cap_ld64(addr);
    return r;
}

static inline void CAP_MMX_ST(uint32_t addr, RecompMmx v)
{
    t2cap_st64(addr, v.q);
}

#endif /* TSFP_CAPTURE */

#endif /* TSFP_HOST_T2_CAPTURE_H */
