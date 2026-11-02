/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Oracle only (T3879): the sink facade bound directly to the retired C WSOLA engine, for sink-fill conformance fixtures
 * that define TSFP_WSOLA_C_ORACLE before including present_sink.c. */
#ifndef TSFP_WSOLA_C_DIRECT_H
#define TSFP_WSOLA_C_DIRECT_H
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "audio_stretch.h"
#define present_wsola audio_stretch
static inline audio_stretch *present_wsola_create(uint16_t c, size_t o) {
    const char *selected = getenv("TSFP_WSOLA_BACKEND");
    if (selected && strcmp(selected,"c")) { fprintf(stderr,"WSOLA backend unavailable: rebuild with TSFP_RUST_WSOLA\n"); return NULL; }
    return audio_stretch_create(c,o);
}
#define present_wsola_destroy audio_stretch_destroy
#define present_wsola_overlap audio_stretch_overlap
#define present_wsola_search audio_stretch_search
#define present_wsola_primed audio_stretch_primed
static inline void present_wsola_report(audio_stretch *e) { (void)e; }
static inline bool present_wsola_prime(audio_stretch *e,const int16_t *i,size_t n,int16_t *o,size_t c) { (void)c; return audio_stretch_prime(e,i,n,o); }
static inline bool present_wsola_next(audio_stretch *e,const int16_t *i,size_t n,size_t x,int16_t *o,size_t c,size_t *s) { (void)c; return audio_stretch_next(e,i,n,x,o,s); }
static inline bool present_wsola_hold(audio_stretch *e,int16_t *o,size_t c) { (void)c; return audio_stretch_hold(e,o); }
static inline bool present_wsola_leave(audio_stretch *e,const int16_t *i,size_t n,int16_t *o,size_t c) { (void)c; return audio_stretch_leave(e,i,n,o); }
static inline bool present_wsola_fade_out(audio_stretch *e,int16_t *o,size_t c) { (void)c; return audio_stretch_fade_out(e,o); }
static inline bool present_wsola_needed(audio_stretch *e, size_t n, size_t *out)
{
    size_t overlap = audio_stretch_overlap(e), search = audio_stretch_search(e);
    if (overlap > (SIZE_MAX - search) / 2u || n > SIZE_MAX - search - 2u * overlap) return false;
    *out = audio_stretch_input_needed(e, n); return true;
}
#endif
