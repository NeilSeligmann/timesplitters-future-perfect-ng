/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Sink-facing facade: one engine selection for its whole lifetime. */
#ifndef TSFP_WSOLA_BACKEND_H
#define TSFP_WSOLA_BACKEND_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#ifdef TSFP_WSOLA_C_ORACLE
#include "wsola_c_direct.h"
#else
typedef struct tsfp_wsola_engine present_wsola;
typedef struct {
    void *(*create)(uint16_t, size_t);
    void (*destroy)(void *);
    size_t (*overlap)(void *);
    size_t (*search)(void *);
    size_t (*needed)(void *, size_t);
    bool (*primed)(void *);
    bool (*prime)(void *, const int16_t *, size_t, int16_t *);
    bool (*next)(void *, const int16_t *, size_t, size_t, int16_t *, size_t *);
    bool (*hold)(void *, int16_t *);
    bool (*leave)(void *, const int16_t *, size_t, int16_t *);
    bool (*fade)(void *, int16_t *);
} tsfp_wsola_reference_ops;
extern present_wsola *tsfp_wsola_create(uint32_t, uint16_t, size_t, const tsfp_wsola_reference_ops *);
extern void tsfp_wsola_destroy(present_wsola *);
extern size_t tsfp_wsola_overlap(const present_wsola *);
extern size_t tsfp_wsola_search(const present_wsola *);
extern bool tsfp_wsola_primed(const present_wsola *);
typedef struct { uint32_t backend, reserved; uint64_t calls, primes; } tsfp_wsola_activity_record;
extern tsfp_wsola_activity_record tsfp_wsola_activity(const present_wsola *);
extern bool tsfp_wsola_needed(const present_wsola *, size_t, size_t *);
extern bool tsfp_wsola_process(present_wsola *, uint32_t, const int16_t *, size_t, size_t, int16_t *, size_t, size_t *);
/* T3879: the C reference engine is retired (oracle only, crates/tsfp-audio-conformance/oracle/wsola_reference.h); the sink
 * always runs the Rust-owned engine. */
static inline present_wsola *present_wsola_create(uint16_t channels, size_t overlap) {
    const char *selected = getenv("TSFP_WSOLA_BACKEND");
    if (selected && strcmp(selected,"rust")) { fprintf(stderr,"WSOLA backend \"%s\" is retired: the Rust engine is the only backend\n", selected); return NULL; }
    present_wsola *engine = tsfp_wsola_create(1u, channels, overlap, NULL);
    if (!engine) fprintf(stderr,"WSOLA backend creation failed\n");
    return engine;
}
#define present_wsola_destroy tsfp_wsola_destroy
#define present_wsola_overlap tsfp_wsola_overlap
#define present_wsola_search tsfp_wsola_search
#define present_wsola_primed tsfp_wsola_primed
#define present_wsola_needed tsfp_wsola_needed
static inline void present_wsola_report(present_wsola *e) {
    const char *enabled = getenv("TSFP_WSOLA_REPORT");
    if (!e || !enabled || strcmp(enabled, "1")) return;
    tsfp_wsola_activity_record a = tsfp_wsola_activity(e);
    fprintf(stderr, "WSOLA activity backend=%s calls=%llu primes=%llu\n",
            a.backend == 2 ? "rust" : a.backend == 1 ? "c" : "invalid",
            (unsigned long long)a.calls, (unsigned long long)a.primes);
}
/* Sink allocates one overlap block of output; pass its checked sample size. */
static inline bool wprocess(present_wsola *e, uint32_t op, const int16_t *in, size_t frames, size_t nominal, int16_t *out, size_t channels, size_t *chosen) {
    size_t overlap = tsfp_wsola_overlap(e);
    if (!channels || overlap > SIZE_MAX / channels) return false;
    return tsfp_wsola_process(e,op,in,frames,nominal,out,overlap*channels,chosen);
}
static inline bool present_wsola_prime(present_wsola *e,const int16_t *i,size_t n,int16_t *o,size_t c) { return wprocess(e,0,i,n,0,o,c,NULL); }
static inline bool present_wsola_next(present_wsola *e,const int16_t *i,size_t n,size_t x,int16_t *o,size_t c,size_t *s) { return wprocess(e,1,i,n,x,o,c,s); }
static inline bool present_wsola_hold(present_wsola *e,int16_t *o,size_t c) { return wprocess(e,2,NULL,0,0,o,c,NULL); }
static inline bool present_wsola_leave(present_wsola *e,const int16_t *i,size_t n,int16_t *o,size_t c) { return wprocess(e,3,i,n,0,o,c,NULL); }
static inline bool present_wsola_fade_out(present_wsola *e,int16_t *o,size_t c) { return wprocess(e,4,NULL,0,0,o,c,NULL); }
#endif /* TSFP_WSOLA_C_ORACLE */
#endif
