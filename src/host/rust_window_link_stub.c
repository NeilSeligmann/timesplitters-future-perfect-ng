/* SPDX-License-Identifier: GPL-3.0-or-later */
/* T2003: stand-ins for the tsfp_wi_* contract (window_input.h) so ninja can link tsfp_host_rustlink and report
 * its link line. crates/tsfp-game/build.rs drops this object (any *_link_stub*.o) from the real link, where
 * sdl_window.rs defines the symbols. Never linked into a runnable binary. */
#include "window_input.h"

void *tsfp_wi_window_create(const char *title, bool vulkan, char *error, size_t error_size)
{ (void)title; (void)vulkan; (void)error; (void)error_size; return NULL; }
void tsfp_wi_window_destroy(void *window) { (void)window; }
tsfp_wi *tsfp_wi_new(void) { return NULL; }
void tsfp_wi_free(tsfp_wi *input) { (void)input; }
uint32_t tsfp_wi_pump(tsfp_wi *input, const char **cause) { (void)input; (void)cause; return 0; }
bool tsfp_wi_enable(tsfp_wi *input, bool keyboard, bool gamepad, char *error, size_t error_size)
{ (void)input; (void)keyboard; (void)gamepad; (void)error; (void)error_size; return false; }
bool tsfp_wi_next(tsfp_wi *input, present_input_event *out) { (void)input; (void)out; return false; }
uint64_t tsfp_wi_dropped(tsfp_wi *input) { (void)input; return 0; }
size_t tsfp_wi_pads_open(tsfp_wi *input) { (void)input; return 0; }
void tsfp_wi_shutdown(tsfp_wi *input) { (void)input; }
