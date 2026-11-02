/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_INPUT_DISPATCH_H
#define TSFP_INPUT_DISPATCH_H

#include <stdbool.h>
#include <stdint.h>

/* API operations synchronize registry state. select returns a C callback pointer and
 * invokes no callback; the caller owns its context and invokes after select returns.
 * free requires the caller to ensure no API operation is active. */
typedef struct tsfp_input_dispatch tsfp_input_dispatch;
typedef uint32_t (*tsfp_input_dispatch_handler)(void *context);
#define TSFP_INPUT_CAPTURE_MAX_ENTRIES 64u
typedef struct { uint32_t address, handler, default_return; } tsfp_input_capture_entry;
typedef struct {
    uint64_t generation, lifecycle_sequence, selection_sequence;
    uint32_t address, route, handler, default_return, complete, entry_count;
    tsfp_input_capture_entry entries[TSFP_INPUT_CAPTURE_MAX_ENTRIES];
} tsfp_input_capture_selection;

tsfp_input_dispatch *tsfp_input_dispatch_new(void);
void tsfp_input_dispatch_free(tsfp_input_dispatch *handle);
bool tsfp_input_dispatch_register(tsfp_input_dispatch *handle, uint32_t address,
                                  tsfp_input_dispatch_handler handler);
bool tsfp_input_dispatch_register_identified(tsfp_input_dispatch *handle, uint32_t address,
                                             tsfp_input_dispatch_handler handler, uint32_t identity);
bool tsfp_input_dispatch_reset(tsfp_input_dispatch *handle);
bool tsfp_input_dispatch_default(tsfp_input_dispatch *handle, uint32_t address, uint32_t value);
/* Live selectors return the C callback; execution happens after Rust releases state. */
tsfp_input_dispatch_handler tsfp_input_dispatch_select(tsfp_input_dispatch *handle,
                                                       uint32_t address);
tsfp_input_dispatch_handler tsfp_input_dispatch_select_capture(tsfp_input_dispatch *handle,
    uint32_t address, tsfp_input_capture_selection *output);
bool tsfp_input_dispatch_observe_c(tsfp_input_dispatch *handle, uint32_t address,
    tsfp_input_dispatch_handler actual, uint32_t c_default, tsfp_input_capture_selection *output);
/* Offline selector-only replay: 0 incomplete, 1 equal, 2 different. No C handler runs. */
uint32_t tsfp_input_dispatch_replay_selection(const tsfp_input_capture_selection *snapshot);
#endif
