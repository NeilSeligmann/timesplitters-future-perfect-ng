/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_HOST_BEVY_SINK_H
#define TSFP_HOST_BEVY_SINK_H
/* T2023: the C side of the Bevy shell (crates/tsfp-bevy). Built only with -DTSFP_RUST_BEVY=ON, into the
 * tsfp_host_bevylink target (cmake/rust-bevy.cmake). Bevy owns the window, the event loop, input and audio; the C host
 * keeps rendering and runs on a game thread. present_sink.c is compiled with TSFP_RUST_WINDOW and TSFP_RUST_BEVY, so:
 *  - the window/event/input contract is tsfp_wi_* (window_input.h), implemented by crates/tsfp-bevy/src/wi.rs on top of
 *    the Bevy keyboard, gamepad and window events;
 *  - each presented RGB picture goes through tsfp_bevy_picture (below) into a latest-wins mailbox that Bevy shows as a
 *    texture (letterboxed, nearest neighbour). Contract and limits: docs/t-bevy-shell.md. */
#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>
#include "window_input.h"

/* Presenter thread, sink lock held. Copies `width*height*3` RGB bytes into the mailbox, replacing an unread picture
 * (latest wins), and returns at once. `number` is the guest picture number. */
void tsfp_bevy_picture(uint64_t number, uint32_t width, uint32_t height, const uint8_t *rgb);
/* The same for an RGBA8 frame (the live renderer's composed front); alpha is forced opaque. */
void tsfp_bevy_picture_rgba(uint64_t number, uint32_t width, uint32_t height, const uint8_t *rgba);
/* Presenter thread, just before each tsfp_bevy_picture_rgba: true while that frame carries a movie overlay picture. Under
 * --wgpu-share the window shows the render runtime's texture, which has no overlay, except while this is true. */
void tsfp_bevy_movie_overlay(bool active);

/* Audio (T2023 item 4): the PCM ring of the host audio sink is pulled by the Bevy audio thread. `pull` copies up to
 * `frames` interleaved S16 stereo frames and zero-fills the rest, returns the real frames. Called once at sink open. */
typedef size_t (*tsfp_bevy_audio_pull_fn)(void *context, int16_t *out, size_t frames);
void tsfp_bevy_audio_attach(tsfp_bevy_audio_pull_fn pull, void *context, uint32_t sample_rate, uint32_t channels);
void tsfp_bevy_audio_detach(void);
/* T2046: enqueue interleaved S16 stereo directly to Bevy's Rust-owned device ring.
 * Returns 1 when the Rust ring mode handled the block (including intentional discard
 * while no device is pulling), 0 when the caller should use the legacy C sink, -1 on
 * malformed input. PCM is copied synchronously and never retained from this call. */
/* Direct-ring write copies PCM synchronously; Rust converts the supplied guest virtual ticks
 * to nanoseconds with the supplied frequency before anchoring its media clock.
 * With an active device it may wait up to two seconds for ring room, then records any dropped tail. */
int tsfp_bevy_audio_write(const int16_t *stereo, size_t frames, uint64_t end_vt_ticks,
                          uint64_t ticks_per_second);
/* Direct Rust runtime callback; has the dsound_audio_runtime_direct_sink_fn ABI. */
int tsfp_bevy_audio_runtime_write(void *context, const int16_t *stereo, size_t frames,
                                  uint64_t end_vt_ticks, uint64_t ticks_per_second);
bool tsfp_bevy_audio_direct_enabled(void);
/* Reports the consumed-frame media clock, applying the existing signed C A/V offset. */
bool tsfp_bevy_audio_media_time(uint64_t *vt_ns, int64_t av_offset_ns);
#endif
