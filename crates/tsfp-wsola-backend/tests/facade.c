/* SPDX-License-Identifier: GPL-3.0-or-later */
#define TSFP_RUST_WSOLA 1
#include "../../../src/host/wsola_backend.h"
#include "wsola_reference.h"
/* Unset selector builds the C reference oracle engine (the sink no longer can); "rust" builds the native one. */
void *fixture_create(uint16_t channels, size_t overlap) {
    if (getenv("TSFP_WSOLA_BACKEND")) return present_wsola_create(channels,overlap);
    const tsfp_wsola_reference_ops ops = {wref_create,wref_destroy,wref_overlap,wref_search,wref_needed,wref_primed,wref_prime,wref_next,wref_hold,wref_leave,wref_fade};
    return tsfp_wsola_create(0,channels,overlap,&ops);
}
void fixture_destroy(void *p) { present_wsola_destroy(p); }
bool fixture_prime(void *p,const int16_t *i,size_t n,int16_t *o,size_t c) { return present_wsola_prime(p,i,n,o,c); }
bool fixture_next(void *p,const int16_t *i,size_t n,size_t nominal,int16_t *o,size_t c,size_t *chosen) { return present_wsola_next(p,i,n,nominal,o,c,chosen); }
bool fixture_hold(void *p,int16_t *o,size_t c) { return present_wsola_hold(p,o,c); }
bool fixture_leave(void *p,const int16_t *i,size_t n,int16_t *o,size_t c) { return present_wsola_leave(p,i,n,o,c); }
bool fixture_fade(void *p,int16_t *o,size_t c) { return present_wsola_fade_out(p,o,c); }
static void *fixture_refuse_create(uint16_t c, size_t o) { (void)c; (void)o; return NULL; }
void *fixture_failed_reference(void) {
    const tsfp_wsola_reference_ops ops = {fixture_refuse_create,wref_destroy,wref_overlap,wref_search,wref_needed,wref_primed,wref_prime,wref_next,wref_hold,wref_leave,wref_fade};
    return tsfp_wsola_create(0,2,4,&ops);
}
