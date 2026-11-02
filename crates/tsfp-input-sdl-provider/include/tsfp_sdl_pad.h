/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_SDL_PAD_H
#define TSFP_SDL_PAD_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "controller_config.h"

/* All calls, including destruction, run on the SDL presenter thread. The
 * caller initializes the SDL gamepad subsystem before pumping the provider. */
typedef struct tsfp_sdl_pad_provider tsfp_sdl_pad_provider;
typedef struct {
    uint8_t buttons[15];
    int16_t axes[6];
} tsfp_sdl_pad_sample_data;

tsfp_sdl_pad_provider *tsfp_sdl_pad_create(void);
void tsfp_sdl_pad_destroy(tsfp_sdl_pad_provider *provider);
bool tsfp_sdl_pad_assign(tsfp_sdl_pad_provider *provider, size_t port,
                         bool enabled, const char *guid, int32_t occurrence);
bool tsfp_sdl_pad_pump(tsfp_sdl_pad_provider *provider);
uint32_t tsfp_sdl_pad_sample(const tsfp_sdl_pad_provider *provider, size_t port,
                             tsfp_sdl_pad_sample_data *out);
bool tsfp_sdl_pad_feedback(tsfp_sdl_pad_provider *provider, size_t port,
                           uint16_t left, uint16_t right);
bool tsfp_sdl_pad_details(const tsfp_sdl_pad_provider *provider, size_t port,
                          char *name, size_t name_capacity, char *guid,
                          size_t guid_capacity, bool *rumble);
size_t tsfp_sdl_pad_unmapped(const tsfp_sdl_pad_provider *provider,
                             char *buffer, size_t capacity);
size_t tsfp_sdl_pad_error(const tsfp_sdl_pad_provider *provider,
                          char *buffer, size_t capacity);
/* Rust applies a validated profile to the provider's raw sample. */
bool tsfp_sdl_pad_profile_sample(const controller_profile *profile,
                                 const uint8_t buttons[CONTROLLER_BUTTONS],
                                 const int16_t axes[CONTROLLER_AXES],
                                 xinput_pad_state *out);
#endif
