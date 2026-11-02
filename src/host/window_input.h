/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_HOST_WINDOW_INPUT_H
#define TSFP_HOST_WINDOW_INPUT_H
/* T2003 (phase 3a of the Rust-hosted executable): the window, event loop and host input module contract.
 * With -DTSFP_RUST_WINDOW=ON, present_sink.c calls these functions instead of its own SDL window creation, event
 * pump and input queue; crates/tsfp-game/src/sdl_window.rs defines them (#[no_mangle] extern "C"). The
 * default build does not include this header's callers. Contract and behaviour:
 * docs/t-host-window-input-contract.md. Everything except create/destroy runs on the presenter thread
 * (SDL owns its objects per thread) under the sink mutex. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "present_sink.h"

typedef struct tsfp_wi tsfp_wi;

#define TSFP_WI_PUMP_CLOSE 1u  /* a quit, window close or the Alt+F4 chord was seen; *cause is the first cause (static text) */
#define TSFP_WI_PUMP_REDRAW 2u /* exposed, resized, shown or restored: draw the last picture again */

/* SDL_Init(VIDEO) + a 640x480 resizable window (SDL_WINDOW_VULKAN when `vulkan`). NULL with `error` on failure,
 * video already released. */
void *tsfp_wi_window_create(const char *title, bool vulkan, char *error, size_t error_size);
/* SDL_DestroyWindow + release the video subsystem. The renderer/Vulkan objects must be gone first. */
void tsfp_wi_window_destroy(void *window);

tsfp_wi *tsfp_wi_new(void);
void tsfp_wi_free(tsfp_wi *input);
/* SDL_PollEvent until empty: close/redraw flags, keyboard/gamepad events into the queue, pads opened/closed. */
uint32_t tsfp_wi_pump(tsfp_wi *input, const char **cause);
/* Keyboard and or gamepad events on; a gamepad initialises the SDL gamepad subsystem and opens attached pads. */
bool tsfp_wi_enable(tsfp_wi *input, bool keyboard, bool gamepad, char *error, size_t error_size);
bool tsfp_wi_next(tsfp_wi *input, present_input_event *out);
uint64_t tsfp_wi_dropped(tsfp_wi *input);
size_t tsfp_wi_pads_open(tsfp_wi *input);
/* Close every pad, release the gamepad subsystem. Before tsfp_wi_window_destroy. */
void tsfp_wi_shutdown(tsfp_wi *input);
#endif
