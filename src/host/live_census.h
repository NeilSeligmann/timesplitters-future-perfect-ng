/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3824: LIVE HIT CENSUS. Which lifted functions ran in a play session, as one executed byte per function, written to
 * the run dir at exit and on a fatal SIGABRT. Compiled in only with -DTSFP_LIVE_CENSUS=ON (build.sh decides the
 * default from tools/data/live_census.json, the measured overhead against its threshold). OBSERVATION ONLY, NOT A
 * MODEL: it changes no guest state.
 *
 * Hot path (live_census_hit.h): a census build compiles the lifted chunks through tools.live_census.transform,
 * which puts LIVE_HIT(0xVA) at the top of every lifted function: a test-and-set of one byte at a constant address.
 * The first measured design (-finstrument-functions, a call per entry and exit) cost about 2x and was dropped.
 *
 * Rust ports: a root's Rust dispatch already counts calls (tsfp_rust_count); at write time a root with any call is
 * marked hit. Functions inside a Rust port that the lifted C no longer enters are not seen.
 *
 * File (little endian; reader and format: tools/live_census/format.py, docs/t3824-live-hit-census.md):
 *   header LIVE_CENSUS_HEADER_SIZE bytes, then u32 va[function_count] ascending, then the bitmap
 *   (bit i of byte i>>3, LSB first) where bit i means va[i] ran.
 */
#ifndef TSFP_HOST_LIVE_CENSUS_H
#define TSFP_HOST_LIVE_CENSUS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define LIVE_CENSUS_MAGIC "TSFPHIT1"
#define LIVE_CENSUS_VERSION 1u
#define LIVE_CENSUS_HEADER_SIZE 192u

enum { LIVE_CENSUS_REASON_EXIT = 0, LIVE_CENSUS_REASON_SIGNAL = 1 };

/** Index the lifted functions of the image (recomp_lookup over [base, base+size)) and arm the hot path.
 * `out_path` NULL reads TSFP_LIVE_CENSUS_OUT; no path, or TSFP_LIVE_CENSUS=0, leaves the census unarmed.
 * Call once, before guest threads start. Returns true when armed. */
bool live_census_start(uint32_t image_base, uint32_t image_size, const char *out_path);

/** The --rust-ports spec of this run, recorded in the header with the enabled root set. Call before writing. */
void live_census_set_port_spec(const char *spec);

/** Write the file (temp name, then rename). Async-signal-safe after live_census_start: no allocation, no stdio. */
bool live_census_write(int reason);

/** Hits and functions of the last successful write (0, 0 before one). */
void live_census_last(uint32_t *hits, uint32_t *functions);

/** For tests: mark a VA as entered, exactly as LIVE_HIT does. */
void live_census_note_va(uint32_t va);

/** For tests: replace recomp_lookup and the Rust root sources. NULL restores the defaults. */
typedef void (*live_census_func)(void);
void live_census_set_sources(live_census_func (*lookup)(uint32_t va), uint32_t (*root)(size_t index),
                             uint64_t (*count)(uint32_t va, bool rust));

#endif /* TSFP_HOST_LIVE_CENSUS_H */
