/* SPDX-License-Identifier: GPL-3.0-or-later */
/* T3912: trace-only witness of the per-packet spans the original GetNextFrame (0x44525D) consumes. Pure core over a
 * flat copy of the packet; lengths and FNV-1a digests only, never bytes. The Rust printer in tsfp-xmv-container
 * (examples/packet_digest.rs) emits the same line format for offline comparison. */
#ifndef XMV_PACKET_WITNESS_H
#define XMV_PACKET_WITNESS_H

#include <stddef.h>
#include <stdint.h>

#define XMV_WITNESS_MAX_STREAMS 8u

/* Bytes the witness needs from the packet start `p+0`, from its first 12+4*streams bytes (`head_len` available).
 * Returns 0 and leaves *need untouched when the head is too short or streams is out of range. */
int xmv_packet_witness_need(const uint8_t *head, size_t head_len, uint32_t streams, uint64_t *need);

/* Formats one line (no newline) for the packet whose `p+0` is `p`, `avail` bytes valid. `index` is the packet number.
 * Returns the length, or 0 when `cap` is too small. Rejected packets print `reject=<reason>`. */
size_t xmv_packet_witness_format(const uint8_t *p, size_t avail, uint32_t streams, uint32_t index, char *out,
                                 size_t cap);

#endif
