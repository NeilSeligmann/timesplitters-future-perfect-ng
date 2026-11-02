/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T2094 (T2036 slice 2): --aspect-override W:H. After the title's own display setup (game_d3d_device_init 0x23930) has stored
 * the aspect globals (4/3/1.0 or, with the widescreen video flag, 16/9/0.75), this rewrites them for an arbitrary aspect so the
 * game's own projection (0xc77c0: ratio = vpW/vpH * (480/den)/(640/num)) gives a Hor+ field of view: the vertical FOV stays
 * 55 degrees, the horizontal extent follows W:H. FABRICATED console setting, opt-in.
 *
 *   0x4c0458 numerator (u32), 0x4c045c denominator (u32), 0x4c0460 horizontal UI scale (f32) = (4/3)/(W/H)
 *
 * Logic only (no kernel dependency): memory access goes through a reader and a writer so a synthetic guest tests it.
 * Known limit: 0x3c420 ("is 16:9", 27 sites, HUD and cutscene layout) is true only for exactly 16:9, so other aspects
 * take the 4:3 layout in those sites (HUD edge anchoring is a follow-up, docs/t-widescreen-design.md section 6).
 */
#ifndef TSFP_HOST_ASPECT_OVERRIDE_H
#define TSFP_HOST_ASPECT_OVERRIDE_H

#include <stdbool.h>
#include <stdint.h>

#define ASPECT_GLOBAL_NUM 0x004C0458u
#define ASPECT_GLOBAL_DEN 0x004C045Cu
#define ASPECT_GLOBAL_SCALE 0x004C0460u
#define ASPECT_OVERRIDE_MAX_TERM 1000u

typedef struct {
    uint32_t num, den; /* reduced */
    uint32_t scale_bits; /* IEEE float of (4/3)/(num/den) */
} aspect_override;

typedef bool (*aspect_read_u32)(void *context, uint32_t address, uint32_t *value);
typedef bool (*aspect_write_u32)(void *context, uint32_t address, uint32_t value);

/** "W:H" with 1..1000 each and W/H in [1, 8]. Reduces by the gcd. false changes nothing. */
bool aspect_override_parse(const char *text, aspect_override *out);

/** Shortest W:H (denominator <= 30) within 0.1 percent of `ratio` (21:9 gives 7:3, 16:9 gives 16:9). */
bool aspect_override_from_ratio(double ratio, uint32_t *num, uint32_t *den);

typedef enum {
    ASPECT_APPLY_WAITING, /* the game has not stored its aspect yet (globals zero or not a known setup value) */
    ASPECT_APPLY_WROTE,
    ASPECT_APPLY_CURRENT, /* already holds the override */
    ASPECT_APPLY_FAILED
} aspect_apply_status;

/** Idempotent, safe every vblank. Writes only when the globals hold what the game's setup stores (4/3 or 16/9) or our own values. */
aspect_apply_status aspect_override_apply(const aspect_override *value, aspect_read_u32 read, aspect_write_u32 write, void *context);

#endif
