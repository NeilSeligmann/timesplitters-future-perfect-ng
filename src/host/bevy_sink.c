/* SPDX-License-Identifier: GPL-3.0-or-later
 * T2023: stand-ins for the tsfp_wi_* window contract and the tsfp_bevy_* functions, plus a C main, so ninja can link
 * tsfp_host_bevylink and report its link line. crates/tsfp-bevy/build.rs drops this object from the real link, where
 * crates/tsfp-bevy defines the symbols. Never linked into a runnable binary. */
#include "bevy_sink.h"

int tsfp_host_entry(int argc, char **argv);

int main(int argc, char **argv)
{
    return tsfp_host_entry(argc, argv);
}

void tsfp_bevy_picture(uint64_t number, uint32_t width, uint32_t height, const uint8_t *rgb)
{ (void)number; (void)width; (void)height; (void)rgb; }
void tsfp_bevy_picture_rgba(uint64_t number, uint32_t width, uint32_t height, const uint8_t *rgba)
{ (void)number; (void)width; (void)height; (void)rgba; }
void tsfp_bevy_movie_overlay(bool active) { (void)active; }
void tsfp_bevy_audio_attach(tsfp_bevy_audio_pull_fn pull, void *context, uint32_t sample_rate, uint32_t channels)
{ (void)pull; (void)context; (void)sample_rate; (void)channels; }
void tsfp_bevy_audio_detach(void) {}
int tsfp_bevy_audio_write(const int16_t *stereo, size_t frames, uint64_t end_vt_ticks,
                          uint64_t ticks_per_second)
{ (void)stereo; (void)frames; (void)end_vt_ticks; (void)ticks_per_second; return 0; }
int tsfp_bevy_audio_runtime_write(void *context, const int16_t *stereo, size_t frames,
                                  uint64_t end_vt_ticks, uint64_t ticks_per_second)
{ (void)context; (void)stereo; (void)frames; (void)end_vt_ticks; (void)ticks_per_second; return 0; }
bool tsfp_bevy_audio_direct_enabled(void) { return false; }
bool tsfp_bevy_audio_media_time(uint64_t *vt_ns, int64_t av_offset_ns)
{ (void)vt_ns; (void)av_offset_ns; return false; }

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
