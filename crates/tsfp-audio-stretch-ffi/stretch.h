/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Offline preparation: this ABI is not selected by the live audio sink. */
#ifndef TSFP_STRETCH_FFI_H
#define TSFP_STRETCH_FFI_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct tsfp_stretch_engine tsfp_stretch_engine;
tsfp_stretch_engine *tsfp_stretch_create(uint16_t channels, size_t overlap);
void tsfp_stretch_destroy(tsfp_stretch_engine *engine);
/* Returns false on null arguments or frame-count overflow; leaves needed unchanged. */
bool tsfp_stretch_query(const tsfp_stretch_engine *engine, size_t nominal, size_t *needed);
bool tsfp_stretch_primed(const tsfp_stretch_engine *engine);
/* A null handle returns zero. */
size_t tsfp_stretch_overlap(const tsfp_stretch_engine *engine);
size_t tsfp_stretch_search(const tsfp_stretch_engine *engine);
/* op: 0 prime, 1 next, 2 hold, 3 leave, 4 fade out. Lengths are samples,
 * nominal and chosen are frames. Input is ignored for hold and fade out;
 * chosen is optional and written only for a successful next. The caller owns
 * the handle and serializes calls; the handle, arrays and chosen must not
 * overlap. Each pointer must be valid for its declared use. */
bool tsfp_stretch_process(tsfp_stretch_engine *engine, uint32_t op,
                          const int16_t *input, size_t input_len, size_t nominal,
                          int16_t *output, size_t output_len, size_t *chosen);
#endif
