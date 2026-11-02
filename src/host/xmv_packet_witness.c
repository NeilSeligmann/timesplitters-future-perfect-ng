/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "xmv_packet_witness.h"
#include <inttypes.h>
#include <stdio.h>

static uint32_t dword(const uint8_t *p, size_t offset)
{
    return (uint32_t)p[offset] | (uint32_t)p[offset + 1u] << 8 | (uint32_t)p[offset + 2u] << 16 |
           (uint32_t)p[offset + 3u] << 24;
}

static uint64_t fnv1a(const uint8_t *data, size_t length)
{
    uint64_t hash = 0xCBF29CE484222325ull;
    for (size_t i = 0u; i < length; i++) {
        hash = (hash ^ data[i]) * 0x100000001B3ull;
    }
    return hash;
}

int xmv_packet_witness_need(const uint8_t *head, size_t head_len, uint32_t streams, uint64_t *need)
{
    if (streams > XMV_WITNESS_MAX_STREAMS || head_len < 12u + 4u * (size_t)streams) {
        return 0;
    }
    uint64_t total = 12u + (uint64_t)(dword(head, 4u) & 0x7FFFFFu);
    for (uint32_t i = 0u; i < streams; i++) {
        total += dword(head, 12u + 4u * i);
    }
    *need = total;
    return 1;
}

size_t xmv_packet_witness_format(const uint8_t *p, size_t avail, uint32_t streams, uint32_t index, char *out,
                                 size_t cap)
{
    uint64_t need = 0u;
    if (!xmv_packet_witness_need(p, avail, streams, &need)) {
        int n = snprintf(out, cap, "xmv-packet[%u]: reject=truncated-sub-header", index);
        return n > 0 && (size_t)n < cap ? (size_t)n : 0u;
    }
    const uint32_t word = dword(p, 4u);
    const size_t video = word & 0x7FFFFFu, region_end = 12u + video, fixed = 12u + 4u * (size_t)streams;
    if (region_end < fixed || need > avail) {
        int n = snprintf(out, cap, "xmv-packet[%u]: reject=%s", index,
                         region_end < fixed ? "video-region-short" : "overrun");
        return n > 0 && (size_t)n < cap ? (size_t)n : 0u;
    }
    int n = snprintf(out, cap, "xmv-packet[%u]: next=%u video=%zu frames=%u codec=", index, dword(p, 0u), video,
                     word >> 23 & 0xFFu);
    if (n < 0 || (size_t)n >= cap) return 0u;
    size_t used = (size_t)n;
    if (word >> 31) {
        n = snprintf(out + used, cap - used, "%08X", dword(p, fixed));
    } else {
        n = snprintf(out + used, cap - used, "-");
    }
    if (n < 0 || (size_t)n >= cap - used) return 0u;
    used += (size_t)n;
    n = snprintf(out + used, cap - used, " duration=%u vfnv=%016" PRIx64 " audio=", dword(p, 8u),
                 fnv1a(p + 12u, video));
    if (n < 0 || (size_t)n >= cap - used) return 0u;
    used += (size_t)n;
    size_t offset = region_end;
    for (uint32_t i = 0u; i < streams; i++) {
        const uint32_t length = dword(p, 12u + 4u * i);
        n = snprintf(out + used, cap - used, "%s%u:%016" PRIx64, i ? "," : "", length, fnv1a(p + offset, length));
        if (n < 0 || (size_t)n >= cap - used) return 0u;
        used += (size_t)n;
        offset += length;
    }
    if (streams == 0u) {
        n = snprintf(out + used, cap - used, "-");
        if (n < 0 || (size_t)n >= cap - used) return 0u;
        used += (size_t)n;
    }
    return used;
}
