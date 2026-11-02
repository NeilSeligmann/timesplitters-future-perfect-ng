/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_KERNEL_CLOCK_RUST_H
#define TSFP_KERNEL_CLOCK_RUST_H
#include <stdint.h>
typedef struct {
    uint64_t result, total, floor, reads, reads_above_floor, frames;
    uint64_t stall_ticks, advance_ticks, advance_calls;
} tsfp_kernel_clock_snapshot;
tsfp_kernel_clock_snapshot tsfp_kernel_clock_apply(uint32_t operation, uint64_t value);
/* T2207: timer/DPC/calendar ordinals on the Rust guest adapter. */
#include <stdbool.h>
typedef struct {
    bool (*read_u32)(uint32_t address, uint32_t *out);
    bool (*write_u32)(uint32_t address, uint32_t value);
    bool (*write_u16)(uint32_t address, uint16_t value);
} tsfp_kernel_clock_guest_memory;
bool tsfp_kernel_clock_guest_call(uint32_t ordinal, const uint32_t *slots, uint32_t count,
                                  const tsfp_kernel_clock_guest_memory *memory, uint32_t *eax);
void tsfp_kernel_clock_guest_reset(void);
#endif
