/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Oracle only (T3879): reference-engine callbacks over the retired C WSOLA engine, for comparing the Rust backend. */
#ifndef TSFP_WSOLA_REFERENCE_H
#define TSFP_WSOLA_REFERENCE_H
#include "audio_stretch.h"
static inline void *wref_create(uint16_t c, size_t o) { return audio_stretch_create(c, o); }
static inline void wref_destroy(void *p) { audio_stretch_destroy(p); }
static inline size_t wref_overlap(void *p) { return audio_stretch_overlap(p); }
static inline size_t wref_search(void *p) { return audio_stretch_search(p); }
static inline size_t wref_needed(void *p, size_t n) { return audio_stretch_input_needed(p, n); }
static inline bool wref_primed(void *p) { return audio_stretch_primed(p); }
static inline bool wref_prime(void *p,const int16_t *i,size_t n,int16_t *o) { return audio_stretch_prime(p,i,n,o); }
static inline bool wref_next(void *p,const int16_t *i,size_t n,size_t x,int16_t *o,size_t *c) { return audio_stretch_next(p,i,n,x,o,c); }
static inline bool wref_hold(void *p,int16_t *o) { return audio_stretch_hold(p,o); }
static inline bool wref_leave(void *p,const int16_t *i,size_t n,int16_t *o) { return audio_stretch_leave(p,i,n,o); }
static inline bool wref_fade(void *p,int16_t *o) { return audio_stretch_fade_out(p,o); }
#endif
