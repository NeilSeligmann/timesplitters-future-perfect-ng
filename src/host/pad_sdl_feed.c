/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L /* clock_gettime for the T1627 hotkey hold time */
#endif
#include "pad_sdl_feed.h"
#include "tsfp_input_host_wiring.h"
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

typedef struct {
    present_video_sink *sink;
    xinput_host_device device;
    void *hotkey_feed; /* Rust owns hotkey scheduling state (T3975). */
} sdl_feed;

static uint64_t monotonic_ms(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (uint64_t)now.tv_sec * 1000u + (uint64_t)now.tv_nsec / 1000000u;
}

static bool sdl_next_due(void *user, uint64_t poll_index, xinput_host_event *out)
{
    sdl_feed *feed = user;
    present_input_event raw;
    tsfp_input_host_hotkey_feed_tick(feed->hotkey_feed, monotonic_ms(), poll_index);
    do {
        if (!present_video_sink_input_next(feed->sink, &raw)) return false;
    } while (!tsfp_input_host_hotkey_feed_event(feed->hotkey_feed, (uint32_t)raw.kind, raw.name,
                                                monotonic_ms(), poll_index));
    tsfp_input_host_translate_sdl_event((uint32_t)raw.kind, raw.name, raw.raw, raw.value, poll_index, out);
    /* T3395: distinguish Bevy's replay queue from the events that actually reach the XInput source. */
    if (getenv("TSFP_POINTER_LOG") != NULL &&
        (raw.kind == PRESENT_INPUT_PAD_DOWN || raw.kind == PRESENT_INPUT_PAD_UP)) {
        fprintf(stderr, "xinput-feed: poll %llu %s %s code=%u raw=%d\n",
                (unsigned long long)poll_index,
                raw.kind == PRESENT_INPUT_PAD_DOWN ? "pad-down" : "pad-up",
                raw.name != NULL ? raw.name : "<unnamed>", out->code, raw.raw);
    }
    return true;
}

static void sdl_close(void *user)
{
    sdl_feed *feed = user;
    tsfp_input_host_hotkey_feed_free(feed->hotkey_feed);
    free(feed);
}

bool pad_sdl_feed_open(present_video_sink *sink, xinput_host_device device, xinput_event_feed *feed, char *error,
                       size_t error_size)
{
    return pad_sdl_feed_open_hotkeys(sink, device, NULL, feed, error, error_size);
}

bool pad_sdl_feed_open_hotkeys(present_video_sink *sink, xinput_host_device device, xinput_hotkeys *hotkeys,
                               xinput_event_feed *feed, char *error, size_t error_size)
{
    const char *reason = NULL;
    /* a hotkey on the other device needs that device's events in the queue too (the source ignores and counts them) */
    const bool keyboard = device == XINPUT_HOST_KEYBOARD ||
                          tsfp_input_host_hotkey_needs_device(hotkeys, XINPUT_HOTKEY_DEVICE_KEYBOARD);
    const bool gamepad = device == XINPUT_HOST_GAMEPAD ||
                         tsfp_input_host_hotkey_needs_device(hotkeys, XINPUT_HOTKEY_DEVICE_PAD);
    if (!present_video_sink_enable_input(sink, keyboard, gamepad, &reason)) {
        if (error != NULL && error_size != 0u) snprintf(error, error_size, "%s", reason != NULL ? reason : "window input failed");
        return false;
    }
    sdl_feed *state = calloc(1u, sizeof(*state));
    if (state == NULL) return false;
    state->sink = sink;
    state->device = device;
    state->hotkey_feed = tsfp_input_host_hotkey_feed_create(hotkeys);
    if (hotkeys != NULL && state->hotkey_feed == NULL) { free(state); return false; }
    feed->next_due = sdl_next_due;
    feed->close = sdl_close;
    feed->user = state;
    return true;
}
