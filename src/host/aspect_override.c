/* SPDX-License-Identifier: GPL-3.0-or-later
 * T2094: see aspect_override.h. */
#include "aspect_override.h"

#include <ctype.h>
#include <string.h>

static uint32_t gcd(uint32_t a, uint32_t b)
{
    while (b != 0u) {
        const uint32_t t = a % b;
        a = b;
        b = t;
    }
    return a;
}

static uint32_t scale_bits(uint32_t num, uint32_t den)
{
    const float scale = (float)((4.0 / 3.0) * (double)den / (double)num);
    uint32_t bits;
    memcpy(&bits, &scale, sizeof bits);
    return bits;
}

static bool term(const char **cursor, uint32_t *out)
{
    uint32_t value = 0u;
    const char *at = *cursor;
    if (!isdigit((unsigned char)*at)) {
        return false;
    }
    while (isdigit((unsigned char)*at)) {
        value = value * 10u + (uint32_t)(*at - '0');
        if (value > ASPECT_OVERRIDE_MAX_TERM) {
            return false;
        }
        at++;
    }
    *cursor = at;
    *out = value;
    return value >= 1u;
}

bool aspect_override_parse(const char *text, aspect_override *out)
{
    uint32_t num = 0u, den = 0u;
    if (text == NULL || out == NULL || !term(&text, &num) || *text++ != ':' || !term(&text, &den) || *text != '\0') {
        return false;
    }
    if (num < den || num > 8u * den) {
        return false;
    }
    const uint32_t g = gcd(num, den);
    out->num = num / g;
    out->den = den / g;
    out->scale_bits = scale_bits(out->num, out->den);
    return true;
}

bool aspect_override_from_ratio(double ratio, uint32_t *num, uint32_t *den)
{
    if (!(ratio >= 1.0 && ratio <= 8.0) || num == NULL || den == NULL) {
        return false;
    }
    for (uint32_t d = 1u; d <= 30u; d++) {
        const uint32_t n = (uint32_t)(ratio * (double)d + 0.5);
        if (n >= d && ((double)n / (double)d - ratio < ratio * 0.001 && ratio - (double)n / (double)d < ratio * 0.001)) {
            const uint32_t g = gcd(n, d);
            *num = n / g;
            *den = d / g;
            return true;
        }
    }
    return false;
}

aspect_apply_status aspect_override_apply(const aspect_override *value, aspect_read_u32 read, aspect_write_u32 write, void *context)
{
    uint32_t num = 0u, den = 0u;
    if (value == NULL || read == NULL || write == NULL || !read(context, ASPECT_GLOBAL_NUM, &num) ||
        !read(context, ASPECT_GLOBAL_DEN, &den)) {
        return ASPECT_APPLY_FAILED;
    }
    if (num == value->num && den == value->den) {
        return ASPECT_APPLY_CURRENT;
    }
    const bool game_setup = (num == 4u && den == 3u) || (num == 16u && den == 9u);
    if (!game_setup) {
        return ASPECT_APPLY_WAITING;
    }
    if (!write(context, ASPECT_GLOBAL_SCALE, value->scale_bits) || !write(context, ASPECT_GLOBAL_NUM, value->num) ||
        !write(context, ASPECT_GLOBAL_DEN, value->den)) {
        return ASPECT_APPLY_FAILED;
    }
    return ASPECT_APPLY_WROTE;
}
